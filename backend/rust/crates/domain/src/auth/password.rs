use std::sync::Arc;

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
            verify_legacy_password_hex(&digest.0, stored_hash)
        }
        Some("sha256") => {
            let mut hasher = Sha256::new();
            hasher.update(password.as_bytes());
            verify_legacy_password_hex(&hasher.finalize(), stored_hash)
        }
        Some("md5salt") => {
            let salt = salt.unwrap_or_default();
            let digest = md5::compute(format!("{password}{salt}"));
            verify_legacy_password_hex(&digest.0, stored_hash)
        }
        _ => verify_modern_password(password, stored_hash),
    }
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

pub(super) fn password_needs_rehash(algo: Option<&str>, stored_hash: &str) -> bool {
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
