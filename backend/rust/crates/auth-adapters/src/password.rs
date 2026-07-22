use std::sync::{Arc, OnceLock};

use argon2::{
    Argon2, Params, PasswordHash, PasswordHasher, PasswordVerifier, password_hash::SaltString,
};
use sha2::{Digest, Sha256};
use tokio::sync::Semaphore;
use uuid::Uuid;
use v2board_compat::ApiError;

/// Bounds CPU-heavy password work so a login burst cannot exhaust Tokio's blocking pool.
#[derive(Clone)]
pub struct PasswordKdf {
    permits: Arc<Semaphore>,
}

impl PasswordKdf {
    pub fn new(max_parallel: usize) -> Self {
        Self {
            permits: Arc::new(Semaphore::new(max_parallel.clamp(1, 64))),
        }
    }

    pub async fn hash(&self, password: &str) -> Result<String, ApiError> {
        let password = password.to_string();
        let permit = self
            .permits
            .clone()
            .acquire_owned()
            .await
            .map_err(|_| ApiError::internal("password worker is unavailable"))?;
        tokio::task::spawn_blocking(move || {
            let _permit = permit;
            hash_password(&password)
        })
        .await
        .map_err(|error| ApiError::internal(format!("password worker failed: {error}")))?
    }

    pub async fn verify(
        &self,
        algo: Option<&str>,
        salt: Option<&str>,
        password: &str,
        stored_hash: &str,
    ) -> Result<bool, ApiError> {
        let algo = algo.map(ToOwned::to_owned);
        let salt = salt.map(ToOwned::to_owned);
        let password = password.to_string();
        let stored_hash = stored_hash.to_string();
        let permit = self
            .permits
            .clone()
            .acquire_owned()
            .await
            .map_err(|_| ApiError::internal("password worker is unavailable"))?;
        tokio::task::spawn_blocking(move || {
            let _permit = permit;
            verify_password(algo.as_deref(), salt.as_deref(), &password, &stored_hash)
        })
        .await
        .map_err(|error| ApiError::internal(format!("password worker failed: {error}")))
    }
}

pub fn verify_password(
    algo: Option<&str>,
    salt: Option<&str>,
    password: &str,
    stored_hash: &str,
) -> bool {
    match algo {
        Some("md5") => {
            let digest = md5::compute(password);
            let matched = verify_legacy_password_hex(&digest.0, stored_hash);
            pad_legacy_verification_cost(password);
            matched
        }
        Some("sha256") => {
            let mut hasher = Sha256::new();
            hasher.update(password.as_bytes());
            let matched = verify_legacy_password_hex(&hasher.finalize(), stored_hash);
            pad_legacy_verification_cost(password);
            matched
        }
        Some("md5salt") => {
            let salt = salt.unwrap_or_default();
            let digest = md5::compute(format!("{password}{salt}"));
            let matched = verify_legacy_password_hex(&digest.0, stored_hash);
            pad_legacy_verification_cost(password);
            matched
        }
        _ => verify_modern_password(password, stored_hash),
    }
}

/// Not-yet-migrated accounts are still verified against a cheap legacy digest
/// (md5/sha256), which finishes orders of magnitude faster than the Argon2/bcrypt
/// path used for migrated accounts. Left alone, that gap is a response-time
/// side channel that lets an attacker distinguish legacy-hashed accounts from
/// modern-hashed ones without needing the password at all, and it persists
/// indefinitely because the legacy hash is only replaced on a successful login.
///
/// Pad every legacy verification (match or mismatch) with an extra Argon2
/// verification of the same cost as the modern path, discarding its result, so
/// the observable latency of the legacy branch tracks the modern branch.
fn pad_legacy_verification_cost(password: &str) {
    let _ = verify_modern_password(password, legacy_padding_hash());
}

fn legacy_padding_hash() -> &'static str {
    static LEGACY_PADDING_HASH: OnceLock<String> = OnceLock::new();
    LEGACY_PADDING_HASH.get_or_init(|| {
        hash_password("v2board-legacy-padding-not-an-account")
            .expect("legacy padding hash must be computable")
    })
}

fn verify_legacy_password_hex(expected: &[u8], stored_hash: &str) -> bool {
    let Ok(stored) = hex::decode(stored_hash) else {
        return false;
    };
    v2board_compat::constant_time_bytes_eq(expected, &stored)
}

fn verify_modern_password(password: &str, stored_hash: &str) -> bool {
    if stored_hash.starts_with("$argon2") {
        let Ok(parsed) = PasswordHash::new(stored_hash) else {
            return false;
        };
        return Argon2::default()
            .verify_password(password.as_bytes(), &parsed)
            .is_ok();
    }

    let bcrypt_hash = stored_hash
        .strip_prefix("$2y$")
        .map(|rest| format!("$2b${rest}"))
        .unwrap_or_else(|| stored_hash.to_string());
    bcrypt::verify(password, &bcrypt_hash).unwrap_or(false)
}

pub fn hash_password(password: &str) -> Result<String, ApiError> {
    let salt = SaltString::encode_b64(Uuid::new_v4().as_bytes())
        .map_err(|error| ApiError::internal(format!("password salt error: {error}")))?;
    Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .map(|hash| hash.to_string())
        .map_err(|error| ApiError::internal(format!("password hash error: {error}")))
}

pub fn password_needs_rehash(algo: Option<&str>, stored_hash: &str) -> bool {
    if algo.is_some() {
        return true;
    }
    let Ok(hash) = PasswordHash::new(stored_hash) else {
        return true;
    };
    if hash.algorithm.as_str() != "argon2id" || hash.version != Some(19) {
        return true;
    }
    let sufficiently_strong = hash
        .params
        .get_decimal("m")
        .is_some_and(|value| value >= Params::DEFAULT_M_COST)
        && hash
            .params
            .get_decimal("t")
            .is_some_and(|value| value >= Params::DEFAULT_T_COST)
        && hash
            .params
            .get_decimal("p")
            .is_some_and(|value| value >= Params::DEFAULT_P_COST)
        && hash.hash.as_ref().is_some_and(|output| output.len() >= 32);
    !sufficiently_strong
}

#[cfg(test)]
mod tests {
    use sha2::Digest as _;

    use super::{PasswordKdf, hash_password, password_needs_rehash, verify_password};

    #[test]
    fn new_passwords_use_argon2id_and_legacy_hashes_request_an_upgrade() {
        let password = "correct horse battery staple";
        let hash = hash_password(password).unwrap();
        assert!(hash.starts_with("$argon2id$"));
        assert!(verify_password(None, None, password, &hash));
        assert!(!password_needs_rehash(None, &hash));

        let bcrypt = bcrypt::hash(password, bcrypt::DEFAULT_COST).unwrap();
        assert!(verify_password(None, None, password, &bcrypt));
        assert!(password_needs_rehash(None, &bcrypt));
        assert!(password_needs_rehash(Some("md5"), "not-relevant"));

        let weak = hash.replace("m=19456,t=2,p=1", "m=4096,t=1,p=1");
        assert!(password_needs_rehash(None, &weak));
        let old_version = hash.replace("v=19", "v=16");
        assert!(password_needs_rehash(None, &old_version));
        let wrong_variant = hash.replacen("$argon2id$", "$argon2i$", 1);
        assert!(password_needs_rehash(None, &wrong_variant));
    }

    #[test]
    fn legacy_password_hex_is_strictly_decoded_and_case_insensitive() {
        let password = "legacy-password";
        let md5 = format!("{:x}", md5::compute(password));
        assert!(verify_password(
            Some("md5"),
            None,
            password,
            &md5.to_ascii_uppercase()
        ));
        assert!(!verify_password(Some("md5"), None, password, "not-hex"));
        assert!(!verify_password(Some("md5"), None, password, "00"));

        let mut hasher = sha2::Sha256::new();
        hasher.update(password.as_bytes());
        let sha256 = hex::encode(hasher.finalize());
        assert!(verify_password(
            Some("sha256"),
            None,
            password,
            &sha256.to_ascii_uppercase()
        ));
    }

    #[test]
    fn legacy_verification_is_padded_to_modern_cost_for_both_outcomes() {
        use std::time::{Duration, Instant};

        let password = "legacy-password";
        let md5 = format!("{:x}", md5::compute(password));

        // Warm the lazily-initialized padding hash so its one-time Argon2 hash
        // cost is not counted against either measurement below.
        assert!(verify_password(Some("md5"), None, password, &md5));

        let time_it = |matching: bool| -> Duration {
            let stored = if matching { md5.as_str() } else { "00" };
            let started = Instant::now();
            let matched = verify_password(Some("md5"), None, password, stored);
            assert_eq!(matched, matching);
            started.elapsed()
        };

        let matched_elapsed = time_it(true);
        let mismatched_elapsed = time_it(false);

        // Both the legacy-hash-matches and legacy-hash-does-not-match cases now
        // pay the same extra Argon2 verification, so neither is a cheap,
        // near-zero-cost branch relative to the other: the mismatch path must
        // not be dramatically faster than the match path.
        let ratio =
            mismatched_elapsed.as_secs_f64().max(1e-9) / matched_elapsed.as_secs_f64().max(1e-9);
        assert!(
            (0.2..5.0).contains(&ratio),
            "legacy match ({matched_elapsed:?}) and mismatch ({mismatched_elapsed:?}) durations diverged too much (ratio {ratio})"
        );

        // Both legacy verifications should be dominated by the padding Argon2
        // cost, not the near-instant raw digest comparison.
        assert!(matched_elapsed >= Duration::from_millis(1));
        assert!(mismatched_elapsed >= Duration::from_millis(1));
    }

    #[tokio::test]
    async fn bounded_password_worker_hashes_and_verifies_off_runtime_threads() {
        let worker = PasswordKdf::new(1);
        let hash = worker.hash("correct horse battery staple").await.unwrap();
        assert!(
            worker
                .verify(None, None, "correct horse battery staple", &hash)
                .await
                .unwrap()
        );
        assert!(
            !worker
                .verify(None, None, "wrong password", &hash)
                .await
                .unwrap()
        );
    }
}
