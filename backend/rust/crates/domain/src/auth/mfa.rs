//! Admin/staff TOTP two-factor authentication (RFC 6238 over RFC 4226,
//! HMAC-SHA1, 6 digits, 30-second steps, ±1-step verification window).
//!
//! The shared secret is generated server-side (20 random bytes), sealed with
//! AES-256-GCM under a key derived from the installation `app_key`, and bound
//! to the owning account through the AEAD associated data so a sealed secret
//! cannot be moved between rows. Replay protection is a monotonic
//! `last_step` compare-and-set in `admin_mfa`: every accepted code consumes
//! its time-step exactly once, even under concurrent logins.

use chrono::Utc;
use hmac::{Hmac, KeyInit, Mac};
use openssl::symm::{Cipher, Crypter, Mode};
use serde::Serialize;
use sha1::Sha1;
use sha2::{Digest, Sha256};
use url::Url;
use v2board_compat::{ApiError, Code, Problem, json::rfc3339_option};
use v2board_db as db;
use v2board_db::admin_mfa::AdminMfaRow;

use super::AuthService;

const TOTP_STEP_SECONDS: i64 = 30;
const TOTP_MODULUS: u32 = 1_000_000;
const SECRET_BYTES: usize = 20;
/// Domain separation for the AES-256-GCM key derived from `app_key`.
const KEY_DERIVATION_DOMAIN: &[u8] = b"v2board/admin-mfa/aes-256-gcm/v1\0";
/// AEAD associated-data prefix; the owning user id is appended so a sealed
/// secret is only ever valid for the row it was written to.
const AAD_DOMAIN: &[u8] = b"v2board/admin-mfa/secret/v1\0";
const BASE32_ALPHABET: &[u8; 32] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ234567";

/// GET `account/mfa` — the account's two-factor state.
#[derive(Serialize)]
pub struct MfaStatus {
    pub totp_enabled: bool,
    #[serde(with = "rfc3339_option")]
    pub totp_enabled_at: Option<i64>,
    /// §6.10 `admin_mfa_force`: whether the deployment demands an enabled
    /// factor before this session may leave the `account/mfa` family. Set by
    /// the API layer from the config snapshot; the domain read reports `false`.
    pub totp_required: bool,
}

/// POST `account/mfa/totp` — the pending enrollment the operator loads into
/// an authenticator app. Returned exactly once per setup call; the plaintext
/// secret is never readable again afterwards.
#[derive(Serialize)]
pub struct TotpProvisioning {
    pub secret: String,
    pub otpauth_url: String,
}

/// RFC 4226 HOTP-SHA1 truncated to 6 digits.
fn hotp(secret: &[u8], counter: u64) -> u32 {
    let mut mac =
        <Hmac<Sha1> as KeyInit>::new_from_slice(secret).expect("HMAC accepts keys of any length");
    mac.update(&counter.to_be_bytes());
    let digest = mac.finalize().into_bytes();
    let offset = (digest[19] & 0x0f) as usize;
    let truncated = u32::from_be_bytes([
        digest[offset] & 0x7f,
        digest[offset + 1],
        digest[offset + 2],
        digest[offset + 3],
    ]);
    truncated % TOTP_MODULUS
}

/// Verify `code` against the current time with a ±1-step window, skipping
/// steps at or below `last_step` (already consumed). Returns the accepted
/// step for the caller's compare-and-set consumption.
fn accept_code(secret: &[u8], code: &str, now: i64, last_step: i64) -> Option<i64> {
    if code.len() != 6 || !code.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    let code: u32 = code.parse().ok()?;
    let current_step = now.div_euclid(TOTP_STEP_SECONDS);
    for offset in [0, -1, 1] {
        let step = current_step + offset;
        if step <= last_step || step < 0 {
            continue;
        }
        if hotp(secret, step as u64) == code {
            return Some(step);
        }
    }
    None
}

/// RFC 4648 base32, no padding (the authenticator-app secret alphabet).
fn base32_encode(bytes: &[u8]) -> String {
    let mut encoded = String::with_capacity(bytes.len().div_ceil(5) * 8);
    for chunk in bytes.chunks(5) {
        let mut buffer = [0_u8; 5];
        buffer[..chunk.len()].copy_from_slice(chunk);
        let bits = u64::from_be_bytes([
            0, 0, 0, buffer[0], buffer[1], buffer[2], buffer[3], buffer[4],
        ]);
        let quintets = (chunk.len() * 8).div_ceil(5);
        for index in 0..quintets {
            let shift = 35 - index * 5;
            encoded.push(BASE32_ALPHABET[((bits >> shift) & 0x1f) as usize] as char);
        }
    }
    encoded
}

fn derive_key(app_key: &str) -> [u8; 32] {
    let mut digest = Sha256::new();
    digest.update(KEY_DERIVATION_DOMAIN);
    digest.update(app_key.as_bytes());
    digest.finalize().into()
}

fn seal_aad(user_id: i64) -> Vec<u8> {
    let mut aad = Vec::with_capacity(AAD_DOMAIN.len() + 8);
    aad.extend_from_slice(AAD_DOMAIN);
    aad.extend_from_slice(&user_id.to_be_bytes());
    aad
}

struct SealedSecret {
    nonce: [u8; 12],
    ciphertext: Vec<u8>,
    tag: [u8; 16],
}

fn seal_secret(app_key: &str, user_id: i64, secret: &[u8]) -> Result<SealedSecret, ApiError> {
    let mut nonce = [0_u8; 12];
    getrandom::fill(&mut nonce)
        .map_err(|error| ApiError::Internal(format!("mfa secret nonce: {error}")))?;
    let cipher = Cipher::aes_256_gcm();
    let key = derive_key(app_key);
    let mut crypter = Crypter::new(cipher, Mode::Encrypt, &key, Some(&nonce))
        .map_err(|error| ApiError::Internal(format!("mfa secret seal: {error}")))?;
    crypter.pad(false);
    let mut seal = || -> Result<SealedSecret, openssl::error::ErrorStack> {
        crypter.aad_update(&seal_aad(user_id))?;
        let mut ciphertext = vec![0_u8; secret.len() + cipher.block_size()];
        let mut length = crypter.update(secret, &mut ciphertext)?;
        length += crypter.finalize(&mut ciphertext[length..])?;
        ciphertext.truncate(length);
        let mut tag = [0_u8; 16];
        crypter.get_tag(&mut tag)?;
        Ok(SealedSecret {
            nonce,
            ciphertext,
            tag,
        })
    };
    seal().map_err(|error| ApiError::Internal(format!("mfa secret seal: {error}")))
}

fn open_secret(app_key: &str, user_id: i64, row: &AdminMfaRow) -> Result<Vec<u8>, ApiError> {
    let open = || -> Result<Vec<u8>, openssl::error::ErrorStack> {
        let cipher = Cipher::aes_256_gcm();
        let key = derive_key(app_key);
        let mut crypter = Crypter::new(cipher, Mode::Decrypt, &key, Some(&row.secret_nonce))?;
        crypter.pad(false);
        crypter.aad_update(&seal_aad(user_id))?;
        crypter.set_tag(&row.secret_tag)?;
        let mut plaintext = vec![0_u8; row.secret_ciphertext.len() + cipher.block_size()];
        let mut length = crypter.update(&row.secret_ciphertext, &mut plaintext)?;
        length += crypter.finalize(&mut plaintext[length..])?;
        plaintext.truncate(length);
        Ok(plaintext)
    };
    open().map_err(|error| ApiError::Internal(format!("mfa secret open: {error}")))
}

fn otpauth_url(issuer: &str, email: &str, secret_base32: &str) -> String {
    let mut url = Url::parse("otpauth://totp").expect("static otpauth base URL parses");
    url.path_segments_mut()
        .expect("otpauth URLs accept path segments")
        .push(&format!("{issuer}:{email}"));
    url.query_pairs_mut()
        .append_pair("secret", secret_base32)
        .append_pair("issuer", issuer)
        .append_pair("algorithm", "SHA1")
        .append_pair("digits", "6")
        .append_pair("period", "30");
    url.to_string()
}

impl AuthService {
    pub async fn admin_mfa_status(&self, user_id: i64) -> Result<MfaStatus, ApiError> {
        let row = db::admin_mfa::find(&self.db, user_id).await?;
        let enabled_at = row.and_then(|row| row.enabled_at);
        Ok(MfaStatus {
            totp_enabled: enabled_at.is_some(),
            totp_enabled_at: enabled_at,
            totp_required: false,
        })
    }

    /// Start (or restart) a pending TOTP enrollment. Enabled accounts must
    /// disable first — re-keying an active factor silently would defeat the
    /// audit trail of the disable step.
    pub async fn admin_mfa_totp_setup(
        &self,
        user_id: i64,
        email: &str,
    ) -> Result<TotpProvisioning, ApiError> {
        let mut secret = [0_u8; SECRET_BYTES];
        getrandom::fill(&mut secret)
            .map_err(|error| ApiError::Internal(format!("mfa secret generation: {error}")))?;
        let sealed = seal_secret(&self.config.app_key, user_id, &secret)?;
        let stored = db::admin_mfa::upsert_pending(
            &self.db,
            user_id,
            &sealed.nonce,
            &sealed.ciphertext,
            &sealed.tag,
            Utc::now().timestamp(),
        )
        .await?;
        if stored == 0 {
            return Err(Problem::new(Code::MfaAlreadyEnabled).into());
        }
        let secret_base32 = base32_encode(&secret);
        let otpauth_url = otpauth_url(&self.config.app_name, email, &secret_base32);
        Ok(TotpProvisioning {
            secret: secret_base32,
            otpauth_url,
        })
    }

    /// Confirm a pending enrollment with a live code, flipping it to enabled.
    pub async fn admin_mfa_totp_confirm(&self, user_id: i64, code: &str) -> Result<(), ApiError> {
        let Some(row) = db::admin_mfa::find(&self.db, user_id).await? else {
            return Err(Problem::new(Code::MfaSetupMissing).into());
        };
        if row.enabled_at.is_some() {
            return Err(Problem::new(Code::MfaAlreadyEnabled).into());
        }
        let secret = open_secret(&self.config.app_key, user_id, &row)?;
        let now = Utc::now().timestamp();
        let Some(step) = accept_code(&secret, code, now, row.last_step) else {
            return Err(Problem::new(Code::MfaCodeInvalid).into());
        };
        if db::admin_mfa::enable(&self.db, user_id, step, now).await? == 0 {
            // Lost a race against a concurrent confirm.
            return Err(Problem::new(Code::MfaAlreadyEnabled).into());
        }
        Ok(())
    }

    /// Disable an enabled factor. Requires a live code so a hijacked session
    /// (even one that passed the step-up password gate) cannot silently
    /// remove the second factor it does not control.
    pub async fn admin_mfa_totp_disable(&self, user_id: i64, code: &str) -> Result<(), ApiError> {
        let Some(row) = db::admin_mfa::find(&self.db, user_id).await? else {
            return Err(Problem::new(Code::MfaNotEnabled).into());
        };
        if row.enabled_at.is_none() {
            return Err(Problem::new(Code::MfaNotEnabled).into());
        }
        let secret = open_secret(&self.config.app_key, user_id, &row)?;
        let Some(step) = accept_code(&secret, code, Utc::now().timestamp(), row.last_step) else {
            return Err(Problem::new(Code::MfaCodeInvalid).into());
        };
        if db::admin_mfa::disable(&self.db, user_id, step).await? == 0 {
            // The step was consumed concurrently — treat the code as spent.
            return Err(Problem::new(Code::MfaCodeInvalid).into());
        }
        Ok(())
    }

    /// The login-path gate, called after password proof for privileged
    /// accounts. `Ok(())` means either no enabled factor or a fresh accepted
    /// code; the caller maps the two failure codes onto the login response.
    pub(super) async fn verify_login_totp(
        &self,
        user_id: i64,
        totp_code: Option<&str>,
    ) -> Result<(), ApiError> {
        let Some(row) = db::admin_mfa::find(&self.db, user_id).await? else {
            return Ok(());
        };
        if row.enabled_at.is_none() {
            return Ok(());
        }
        let Some(code) = totp_code else {
            return Err(Problem::new(Code::MfaCodeRequired).into());
        };
        let secret = open_secret(&self.config.app_key, user_id, &row)?;
        let now = Utc::now().timestamp();
        let Some(step) = accept_code(&secret, code, now, row.last_step) else {
            return Err(Problem::new(Code::MfaCodeInvalid).into());
        };
        if db::admin_mfa::consume_step(&self.db, user_id, step, now).await? == 0 {
            // Replay race: a concurrent login consumed this step first.
            return Err(Problem::new(Code::MfaCodeInvalid).into());
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// RFC 4226 Appendix D reference secret and 6-digit HOTP values.
    const RFC4226_SECRET: &[u8] = b"12345678901234567890";
    const RFC4226_CODES: [u32; 10] = [
        755_224, 287_082, 359_152, 969_429, 338_314, 254_676, 287_922, 162_583, 399_871, 520_489,
    ];

    #[test]
    fn hotp_matches_rfc4226_appendix_d() {
        for (counter, expected) in RFC4226_CODES.iter().enumerate() {
            assert_eq!(hotp(RFC4226_SECRET, counter as u64), *expected);
        }
    }

    #[test]
    fn totp_matches_rfc6238_sha1_vectors() {
        // RFC 6238 Appendix B (SHA-1 rows), truncated from 8 to 6 digits.
        for (time, expected_8_digit) in [
            (59_i64, 94_287_082_u32),
            (1_111_111_109, 7_081_804),
            (1_111_111_111, 14_050_471),
            (1_234_567_890, 89_005_924),
            (2_000_000_000, 69_279_037),
            (20_000_000_000, 65_353_130),
        ] {
            let step = time.div_euclid(TOTP_STEP_SECONDS);
            assert_eq!(
                hotp(RFC4226_SECRET, step as u64),
                expected_8_digit % TOTP_MODULUS
            );
        }
    }

    #[test]
    fn accept_code_covers_adjacent_steps_and_blocks_replay() {
        let now = 1_111_111_111_i64;
        let step = now.div_euclid(TOTP_STEP_SECONDS);
        let code_now = format!("{:06}", hotp(RFC4226_SECRET, step as u64));
        let code_prev = format!("{:06}", hotp(RFC4226_SECRET, (step - 1) as u64));
        let code_next = format!("{:06}", hotp(RFC4226_SECRET, (step + 1) as u64));
        let code_far = format!("{:06}", hotp(RFC4226_SECRET, (step + 5) as u64));

        assert_eq!(accept_code(RFC4226_SECRET, &code_now, now, 0), Some(step));
        assert_eq!(
            accept_code(RFC4226_SECRET, &code_prev, now, 0),
            Some(step - 1)
        );
        assert_eq!(
            accept_code(RFC4226_SECRET, &code_next, now, 0),
            Some(step + 1)
        );
        assert_eq!(accept_code(RFC4226_SECRET, &code_far, now, 0), None);
        // A consumed step is dead even inside the window.
        assert_eq!(accept_code(RFC4226_SECRET, &code_now, now, step), None);
        assert_eq!(accept_code(RFC4226_SECRET, &code_prev, now, step - 1), None);
        // Malformed inputs never match.
        assert_eq!(accept_code(RFC4226_SECRET, "12345", now, 0), None);
        assert_eq!(accept_code(RFC4226_SECRET, "12345a", now, 0), None);
        assert_eq!(accept_code(RFC4226_SECRET, "1234567", now, 0), None);
    }

    #[test]
    fn base32_matches_rfc4648_vectors() {
        assert_eq!(base32_encode(b""), "");
        assert_eq!(base32_encode(b"f"), "MY");
        assert_eq!(base32_encode(b"fo"), "MZXQ");
        assert_eq!(base32_encode(b"foo"), "MZXW6");
        assert_eq!(base32_encode(b"foob"), "MZXW6YQ");
        assert_eq!(base32_encode(b"fooba"), "MZXW6YTB");
        assert_eq!(base32_encode(b"foobar"), "MZXW6YTBOI");
        assert_eq!(
            base32_encode(RFC4226_SECRET),
            "GEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQ"
        );
    }

    #[test]
    fn seal_round_trips_and_binds_the_owning_user() {
        let sealed = seal_secret("test-app-key", 7, RFC4226_SECRET).unwrap();
        let row = AdminMfaRow {
            secret_nonce: sealed.nonce.to_vec(),
            secret_ciphertext: sealed.ciphertext.clone(),
            secret_tag: sealed.tag.to_vec(),
            enabled_at: None,
            last_step: 0,
        };
        assert_eq!(
            open_secret("test-app-key", 7, &row).unwrap(),
            RFC4226_SECRET
        );
        assert!(open_secret("test-app-key", 8, &row).is_err());
        assert!(open_secret("other-app-key", 7, &row).is_err());
    }

    #[test]
    fn otpauth_url_encodes_label_and_query() {
        let url = otpauth_url("My Panel", "admin@example.com", "MZXW6YTB");
        assert_eq!(
            url,
            "otpauth://totp/My%20Panel:admin@example.com?secret=MZXW6YTB&issuer=My+Panel&algorithm=SHA1&digits=6&period=30"
        );
    }
}
