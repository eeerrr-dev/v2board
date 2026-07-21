use std::sync::{Arc, OnceLock};

use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use chrono::Utc;
use hmac::{Hmac, KeyInit, Mac};
use lettre::{AsyncTransport, Message, message::header::ContentType};
use openssl::symm::{Cipher, Crypter, Mode};
use redis::aio::ConnectionManager;
use rust_decimal::{Decimal, prelude::ToPrimitive};
use sha1::Sha1;
use sha2::{Digest, Sha256};
use url::Url;
use uuid::Uuid;
use v2board_application::{
    RepositoryError,
    auth::{
        AuthExternal, AuthPolicy, MailDeliveryError, MfaRecord, RepositoryResult, SealedMfaSecret,
        TrialDuration,
    },
};
use v2board_config::{AppConfig, RedisKeyspace, duration_minutes_to_seconds};
use v2board_mail_adapters::{
    mail::render_verify,
    smtp::{SmtpSettings, SmtpTransportCache},
};

use crate::{PasswordKdf, password_needs_rehash};

const TOTP_STEP_SECONDS: i64 = 30;
const TOTP_MODULUS: u32 = 1_000_000;
const SECRET_BYTES: usize = 20;
const KEY_DERIVATION_DOMAIN: &[u8] = b"v2board/admin-mfa/aes-256-gcm/v1\0";
const AAD_DOMAIN: &[u8] = b"v2board/admin-mfa/secret/v1\0";
const BASE32_ALPHABET: &[u8; 32] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ234567";

static DUMMY_PASSWORD_HASH: OnceLock<String> = OnceLock::new();

#[derive(Clone)]
pub struct RuntimeAuthExternal {
    redis: ConnectionManager,
    redis_keys: RedisKeyspace,
    config: Arc<AppConfig>,
    http: reqwest::Client,
    password_kdf: PasswordKdf,
    smtp: SmtpTransportCache,
}

impl RuntimeAuthExternal {
    pub fn new(
        redis: ConnectionManager,
        installation_id: Uuid,
        config: Arc<AppConfig>,
        http: reqwest::Client,
        password_kdf: PasswordKdf,
        smtp: SmtpTransportCache,
    ) -> Self {
        Self {
            redis,
            redis_keys: RedisKeyspace::new(installation_id),
            config,
            http,
            password_kdf,
            smtp,
        }
    }
}

pub(crate) fn policy_from_config(config: &AppConfig) -> AuthPolicy {
    AuthPolicy {
        app_name: config.app_name.clone(),
        app_url: config.app_url.clone(),
        password_limit_enable: config.password_limit_enable,
        password_limit_count: config.password_limit_count,
        password_limit_expire_minutes: config.password_limit_expire,
        password_limit_ttl_seconds: duration_minutes_to_seconds(config.password_limit_expire),
        register_limit_by_ip_enable: config.register_limit_by_ip_enable,
        register_limit_count: config.register_limit_count,
        register_limit_expire_minutes: config.register_limit_expire,
        register_limit_ttl_seconds: duration_minutes_to_seconds(config.register_limit_expire),
        stop_register: config.stop_register,
        invite_force: config.invite_force,
        invite_never_expire: config.invite_never_expire,
        email_verify: config.email_verify,
        email_whitelist_enable: config.email_whitelist_enable,
        email_whitelist_suffix: config.email_whitelist_suffix.clone(),
        email_gmail_limit_enable: config.email_gmail_limit_enable,
        recaptcha_enable: config.recaptcha_enable,
        trial_plan_id: config.try_out_plan_id,
        trial_duration: trial_duration(config.try_out_hour),
        auth_session_ttl_seconds: config.auth_session_ttl_seconds,
        privileged_auth_session_ttl_seconds: config.privileged_auth_session_ttl_seconds,
        auth_session_max_per_user: i64::try_from(config.auth_session_max_per_user)
            .unwrap_or(i64::MAX),
        privileged_step_up_max_attempts: i64::try_from(config.privileged_step_up_max_attempts)
            .unwrap_or(i64::MAX),
        privileged_step_up_attempt_window_seconds: config.privileged_step_up_attempt_window_seconds,
        privileged_step_up_ttl_seconds: config.privileged_step_up_ttl_seconds,
    }
}

fn trial_duration(value: Decimal) -> TrialDuration {
    if value.is_sign_negative() {
        return TrialDuration::Negative;
    }
    let hours = if value.is_zero() { Decimal::ONE } else { value };
    hours
        .checked_mul(Decimal::from(3_600))
        .and_then(|seconds| seconds.trunc().to_i64())
        .map_or(TrialDuration::OutOfRange, TrialDuration::Seconds)
}

impl AuthExternal for RuntimeAuthExternal {
    fn now(&self) -> i64 {
        Utc::now().timestamp()
    }

    fn uuid(&self) -> RepositoryResult<String> {
        Ok(Uuid::new_v4().hyphenated().to_string())
    }

    fn compact_id(&self) -> RepositoryResult<String> {
        Ok(Uuid::new_v4().simple().to_string())
    }

    fn opaque_token(&self) -> RepositoryResult<String> {
        let mut bytes = [0_u8; 32];
        getrandom::fill(&mut bytes)
            .map_err(|error| repository_error("generate authentication token", error))?;
        Ok(URL_SAFE_NO_PAD.encode(bytes))
    }

    fn email_code(&self) -> RepositoryResult<String> {
        let bytes = *Uuid::new_v4().as_bytes();
        let number =
            u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]) % 900_000 + 100_000;
        Ok(number.to_string())
    }

    async fn hash_password(&self, password: &str) -> RepositoryResult<String> {
        self.password_kdf
            .hash(password)
            .await
            .map_err(|error| repository_error("hash authentication password", error))
    }

    async fn verify_password(
        &self,
        algo: Option<&str>,
        salt: Option<&str>,
        password: &str,
        stored_hash: &str,
    ) -> RepositoryResult<bool> {
        self.password_kdf
            .verify(algo, salt, password, stored_hash)
            .await
            .map_err(|error| repository_error("verify authentication password", error))
    }

    async fn verify_dummy_password(&self, password: &str) -> RepositoryResult<()> {
        let hash = if let Some(hash) = DUMMY_PASSWORD_HASH.get() {
            hash.clone()
        } else {
            let candidate = self
                .password_kdf
                .hash("v2board-dummy-password-not-an-account")
                .await
                .map_err(|error| repository_error("hash dummy authentication password", error))?;
            let _ = DUMMY_PASSWORD_HASH.set(candidate);
            DUMMY_PASSWORD_HASH
                .get()
                .expect("dummy password hash was initialized")
                .clone()
        };
        let _ = self
            .password_kdf
            .verify(None, None, password, &hash)
            .await
            .map_err(|error| repository_error("verify dummy authentication password", error))?;
        Ok(())
    }

    fn password_needs_rehash(&self, algo: Option<&str>, stored_hash: &str) -> bool {
        password_needs_rehash(algo, stored_hash)
    }

    async fn verify_recaptcha(&self, token: &str) -> RepositoryResult<bool> {
        let Some(secret) = self
            .config
            .recaptcha_key
            .as_deref()
            .filter(|value| !value.is_empty())
        else {
            return Ok(false);
        };
        let request_body = serde_urlencoded::to_string([("secret", secret), ("response", token)])
            .map_err(|error| repository_error("encode recaptcha request", error))?;
        let response = self
            .http
            .post("https://www.google.com/recaptcha/api/siteverify")
            .header(
                reqwest::header::CONTENT_TYPE,
                "application/x-www-form-urlencoded",
            )
            .body(request_body)
            .send()
            .await
            .map_err(|error| repository_error("verify recaptcha", error))?;
        let body: serde_json::Value = v2board_http_adapters::bounded_json(
            response,
            v2board_http_adapters::MAX_EXTERNAL_RESPONSE_BYTES,
            "Invalid code is incorrect",
        )
        .await
        .map_err(|error| repository_error("decode recaptcha response", error))?;
        Ok(body
            .get("success")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false))
    }

    async fn send_verification_mail(
        &self,
        to: &str,
        app_name: &str,
        app_url: &str,
        code: &str,
    ) -> Result<(), MailDeliveryError> {
        let settings = SmtpSettings::load(&self.config).map_err(|_| {
            MailDeliveryError::SenderNotConfigured {
                detail: Some("Email host is not configured".to_string()),
            }
        })?;
        let from = settings
            .from_address
            .clone()
            .or_else(|| settings.username.clone())
            .ok_or(MailDeliveryError::SenderNotConfigured { detail: None })?;
        let email = Message::builder()
            .from(from.parse().map_err(|_| MailDeliveryError::InvalidSender)?)
            .to(to
                .parse()
                .map_err(|_| MailDeliveryError::InvalidRecipient)?)
            .subject(format!("{app_name}邮箱验证码"))
            .header(ContentType::TEXT_HTML)
            .body(render_verify(app_name, app_url, code))
            .map_err(|error| MailDeliveryError::BuildFailed(error.to_string()))?;
        let transport = self.smtp.transport(&settings).map_err(|error| {
            MailDeliveryError::Infrastructure(repository_error("build smtp transport", error))
        })?;
        tokio::time::timeout(
            std::time::Duration::from_secs(self.config.http_request_timeout_seconds),
            transport.send(email),
        )
        .await
        .map_err(|_| MailDeliveryError::TimedOut)?
        .map_err(|error| MailDeliveryError::SendFailed(error.to_string()))?;
        Ok(())
    }

    async fn subscribe_url(&self, user_id: i64, token: &str) -> RepositoryResult<String> {
        v2board_subscription_adapters::subscribe_url_for_user(
            &self.config,
            &self.redis_keys,
            &mut Some(self.redis.clone()),
            user_id,
            token,
        )
        .await
        .map_err(|error| repository_error("mint rotated subscription URL", error))
    }

    fn create_mfa_secret(
        &self,
        user_id: i64,
        email: &str,
        issuer: &str,
    ) -> RepositoryResult<SealedMfaSecret> {
        let mut secret = [0_u8; SECRET_BYTES];
        getrandom::fill(&mut secret)
            .map_err(|error| repository_error("generate mfa secret", error))?;
        let sealed = seal_secret(&self.config.app_key, user_id, &secret)?;
        let public_secret = base32_encode(&secret);
        Ok(SealedMfaSecret {
            secret_nonce: sealed.nonce.to_vec(),
            secret_ciphertext: sealed.ciphertext,
            secret_tag: sealed.tag.to_vec(),
            otpauth_url: otpauth_url(issuer, email, &public_secret),
            public_secret,
        })
    }

    fn accepted_mfa_step(
        &self,
        user_id: i64,
        record: &MfaRecord,
        code: &str,
        now: i64,
    ) -> RepositoryResult<Option<i64>> {
        let secret = open_secret(&self.config.app_key, user_id, record)?;
        Ok(accept_code(&secret, code, now, record.last_step))
    }
}

fn repository_error(operation: &'static str, error: impl std::fmt::Display) -> RepositoryError {
    RepositoryError::new(operation, error)
}

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

fn accept_code(secret: &[u8], code: &str, now: i64, last_step: i64) -> Option<i64> {
    if code.len() != 6 || !code.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    let code: u32 = code.parse().ok()?;
    let current_step = now.div_euclid(TOTP_STEP_SECONDS);
    for offset in [0, -1, 1] {
        let step = current_step + offset;
        if step > last_step && step >= 0 && hotp(secret, step as u64) == code {
            return Some(step);
        }
    }
    None
}

fn base32_encode(bytes: &[u8]) -> String {
    let mut encoded = String::with_capacity(bytes.len().div_ceil(5) * 8);
    for chunk in bytes.chunks(5) {
        let mut buffer = [0_u8; 5];
        buffer[..chunk.len()].copy_from_slice(chunk);
        let bits = u64::from_be_bytes([
            0, 0, 0, buffer[0], buffer[1], buffer[2], buffer[3], buffer[4],
        ]);
        for index in 0..(chunk.len() * 8).div_ceil(5) {
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

fn seal_secret(app_key: &str, user_id: i64, secret: &[u8]) -> RepositoryResult<SealedSecret> {
    let mut nonce = [0_u8; 12];
    getrandom::fill(&mut nonce)
        .map_err(|error| repository_error("generate mfa secret nonce", error))?;
    let cipher = Cipher::aes_256_gcm();
    let key = derive_key(app_key);
    let mut crypter = Crypter::new(cipher, Mode::Encrypt, &key, Some(&nonce))
        .map_err(|error| repository_error("seal mfa secret", error))?;
    crypter.pad(false);
    crypter
        .aad_update(&seal_aad(user_id))
        .map_err(|error| repository_error("bind mfa secret", error))?;
    let mut ciphertext = vec![0_u8; secret.len() + cipher.block_size()];
    let mut length = crypter
        .update(secret, &mut ciphertext)
        .map_err(|error| repository_error("seal mfa secret", error))?;
    length += crypter
        .finalize(&mut ciphertext[length..])
        .map_err(|error| repository_error("seal mfa secret", error))?;
    ciphertext.truncate(length);
    let mut tag = [0_u8; 16];
    crypter
        .get_tag(&mut tag)
        .map_err(|error| repository_error("read mfa authentication tag", error))?;
    Ok(SealedSecret {
        nonce,
        ciphertext,
        tag,
    })
}

fn open_secret(app_key: &str, user_id: i64, record: &MfaRecord) -> RepositoryResult<Vec<u8>> {
    let cipher = Cipher::aes_256_gcm();
    let key = derive_key(app_key);
    let mut crypter = Crypter::new(cipher, Mode::Decrypt, &key, Some(&record.secret_nonce))
        .map_err(|error| repository_error("open mfa secret", error))?;
    crypter.pad(false);
    crypter
        .aad_update(&seal_aad(user_id))
        .map_err(|error| repository_error("bind mfa secret", error))?;
    crypter
        .set_tag(&record.secret_tag)
        .map_err(|error| repository_error("set mfa authentication tag", error))?;
    let mut plaintext = vec![0_u8; record.secret_ciphertext.len() + cipher.block_size()];
    let mut length = crypter
        .update(&record.secret_ciphertext, &mut plaintext)
        .map_err(|error| repository_error("open mfa secret", error))?;
    length += crypter
        .finalize(&mut plaintext[length..])
        .map_err(|error| repository_error("open mfa secret", error))?;
    plaintext.truncate(length);
    Ok(plaintext)
}

fn otpauth_url(issuer: &str, email: &str, secret: &str) -> String {
    let mut url = Url::parse("otpauth://totp").expect("static otpauth URL parses");
    url.path_segments_mut()
        .expect("otpauth URL accepts path segments")
        .push(&format!("{issuer}:{email}"));
    url.query_pairs_mut()
        .append_pair("secret", secret)
        .append_pair("issuer", issuer)
        .append_pair("algorithm", "SHA1")
        .append_pair("digits", "6")
        .append_pair("period", "30");
    url.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

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
        for (time, expected_eight_digits) in [
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
                expected_eight_digits % TOTP_MODULUS
            );
        }
    }

    #[test]
    fn accept_code_covers_adjacent_steps_and_blocks_replay() {
        let now = 1_111_111_111_i64;
        let step = now.div_euclid(TOTP_STEP_SECONDS);
        let current = format!("{:06}", hotp(RFC4226_SECRET, step as u64));
        let previous = format!("{:06}", hotp(RFC4226_SECRET, (step - 1) as u64));
        let next = format!("{:06}", hotp(RFC4226_SECRET, (step + 1) as u64));
        let distant = format!("{:06}", hotp(RFC4226_SECRET, (step + 5) as u64));

        assert_eq!(accept_code(RFC4226_SECRET, &current, now, 0), Some(step));
        assert_eq!(
            accept_code(RFC4226_SECRET, &previous, now, 0),
            Some(step - 1)
        );
        assert_eq!(accept_code(RFC4226_SECRET, &next, now, 0), Some(step + 1));
        assert_eq!(accept_code(RFC4226_SECRET, &distant, now, 0), None);
        assert_eq!(accept_code(RFC4226_SECRET, &current, now, step), None);
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
    fn sealed_secret_round_trip_is_bound_to_owner_and_app_key() {
        let sealed = seal_secret("test-app-key", 7, RFC4226_SECRET).unwrap();
        let record = MfaRecord {
            secret_nonce: sealed.nonce.to_vec(),
            secret_ciphertext: sealed.ciphertext,
            secret_tag: sealed.tag.to_vec(),
            enabled_at: None,
            last_step: 0,
        };
        assert_eq!(
            open_secret("test-app-key", 7, &record).unwrap(),
            RFC4226_SECRET
        );
        assert!(open_secret("test-app-key", 8, &record).is_err());
        assert!(open_secret("other-app-key", 7, &record).is_err());
    }

    #[test]
    fn otpauth_url_encodes_label_and_query() {
        assert_eq!(
            otpauth_url("My Panel", "admin@example.com", "MZXW6YTB"),
            "otpauth://totp/My%20Panel:admin@example.com?secret=MZXW6YTB&issuer=My+Panel&algorithm=SHA1&digits=6&period=30"
        );
    }
}
