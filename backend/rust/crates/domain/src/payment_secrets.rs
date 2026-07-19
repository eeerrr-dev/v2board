//! Encryption at rest for payment-gateway credentials.
//!
//! `payment_method.config` stores each gateway's key material as a version-1
//! AES-256-GCM envelope in the same JSONB column that previously held the
//! plaintext object. The key derives from the runtime `app_key` under a
//! payment-specific domain constant, so it is distinct from the operator-config
//! key, and the AAD binds the gateway driver plus the row `uuid`, so an
//! envelope cannot be swapped between rows or drivers. Pre-release there are
//! no plaintext rows: a stored non-envelope config is a hard integrity error,
//! never a compatibility fallback.

use base64::{Engine as _, engine::general_purpose::STANDARD};
use hmac::{Hmac, KeyInit, Mac};
use openssl::symm::{Cipher, Crypter, Mode};
use serde_json::{Map, Value, json};
use sha2::{Digest, Sha256};

const KEY_DERIVATION_DOMAIN: &[u8] = b"v2board/payment-config/key/v1\0";
const ENCRYPTION_DOMAIN: &[u8] = b"v2board/payment-config/aes-256-gcm/v1\0";
const NONCE_DOMAIN: &[u8] = b"v2board/payment-config/nonce/v1\0";
pub const PAYMENT_CONFIG_FORMAT_VERSION: u64 = 1;
const ENVELOPE_KEYS: [&str; 4] = ["ciphertext", "format_version", "nonce", "tag"];

#[derive(Debug, thiserror::Error)]
pub enum PaymentSecretsError {
    #[error("payment configuration JSON error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("payment configuration cryptography error: {0}")]
    Crypto(#[from] openssl::error::ErrorStack),
    #[error("payment configuration integrity check failed: {0}")]
    Integrity(&'static str),
}

/// Encrypts one gateway config object into the stored envelope value. The
/// object serializes through `serde_json`'s sorted-key `Map`, so the same
/// config always produces the same plaintext bytes and therefore the same
/// envelope.
pub fn encrypt_payment_config(
    app_key: &str,
    payment: &str,
    uuid: &str,
    config: &Map<String, Value>,
) -> Result<Value, PaymentSecretsError> {
    let plaintext = serde_json::to_vec(&Value::Object(config.clone()))?;
    encrypt_payment_config_canonical(app_key, payment, uuid, &plaintext)
}

/// Encrypts caller-canonicalized plaintext JSON bytes. The MySQL importer uses
/// this entry with its exact-JSON compact encoding so the converted envelope —
/// and with it the whole import — stays byte-deterministic across runs.
pub fn encrypt_payment_config_canonical(
    app_key: &str,
    payment: &str,
    uuid: &str,
    plaintext: &[u8],
) -> Result<Value, PaymentSecretsError> {
    serde_json::from_slice::<Value>(plaintext)
        .map_err(|_| PaymentSecretsError::Integrity("payment configuration is not valid JSON"))?;
    // Deterministic nonce: a keyed PRF over (domain, driver, uuid, plaintext).
    // A given (key, nonce) pair therefore only ever encrypts this one
    // plaintext, which is the sole condition AES-GCM needs to stay safe
    // without random nonces. Determinism is required so the MySQL importer's
    // conversion, canonical row hash, and post-COPY verification scan all
    // reproduce the identical envelope, and it makes envelope equality
    // equivalent to config equality for the immutable checkout snapshot
    // comparison.
    let nonce = derive_nonce(app_key, payment, uuid, plaintext);
    let aad = encryption_aad(payment, uuid);
    let (ciphertext, tag) = encrypt(app_key, &nonce, plaintext, &aad)?;
    Ok(json!({
        "format_version": PAYMENT_CONFIG_FORMAT_VERSION,
        "nonce": STANDARD.encode(nonce),
        "ciphertext": STANDARD.encode(&ciphertext),
        "tag": STANDARD.encode(tag),
    }))
}

/// Decrypts one stored envelope back to the plaintext gateway config object.
pub fn decrypt_payment_config(
    app_key: &str,
    payment: &str,
    uuid: &str,
    raw: &str,
) -> Result<Value, PaymentSecretsError> {
    let plaintext = decrypt_payment_config_canonical(app_key, payment, uuid, raw)?;
    let config = serde_json::from_slice::<Value>(&plaintext)?;
    if !config.is_object() {
        return Err(PaymentSecretsError::Integrity(
            "decrypted payment configuration is not a JSON object",
        ));
    }
    Ok(config)
}

/// Decrypts one stored envelope to the exact plaintext bytes that were
/// encrypted, without re-parsing them into `serde_json` numbers.
pub fn decrypt_payment_config_canonical(
    app_key: &str,
    payment: &str,
    uuid: &str,
    raw: &str,
) -> Result<Vec<u8>, PaymentSecretsError> {
    let envelope = serde_json::from_str::<Value>(raw)?;
    let envelope = envelope
        .as_object()
        .filter(|object| {
            object.len() == ENVELOPE_KEYS.len()
                && ENVELOPE_KEYS.iter().all(|key| object.contains_key(*key))
        })
        .ok_or(PaymentSecretsError::Integrity(
            "stored payment configuration is not an encrypted envelope",
        ))?;
    // JSONB and the importer's exact-JSON encoding respell numbers (`1`,
    // `1e0`), so the version check compares numeric value, not spelling.
    let format_version = envelope
        .get("format_version")
        .and_then(Value::as_f64)
        .ok_or(PaymentSecretsError::Integrity(
            "encrypted payment configuration format version is not a number",
        ))?;
    if format_version != PAYMENT_CONFIG_FORMAT_VERSION as f64 {
        return Err(PaymentSecretsError::Integrity(
            "unsupported payment configuration format version",
        ));
    }
    let nonce: [u8; 12] = envelope_bytes(envelope, "nonce")?
        .try_into()
        .map_err(|_| PaymentSecretsError::Integrity("invalid nonce length"))?;
    let tag: [u8; 16] = envelope_bytes(envelope, "tag")?
        .try_into()
        .map_err(|_| PaymentSecretsError::Integrity("invalid tag length"))?;
    let ciphertext = envelope_bytes(envelope, "ciphertext")?;
    let aad = encryption_aad(payment, uuid);
    decrypt(app_key, &nonce, &ciphertext, &tag, &aad)
        .map_err(|_| PaymentSecretsError::Integrity("payment configuration authentication failed"))
}

fn envelope_bytes(
    envelope: &Map<String, Value>,
    key: &'static str,
) -> Result<Vec<u8>, PaymentSecretsError> {
    envelope
        .get(key)
        .and_then(Value::as_str)
        .and_then(|value| STANDARD.decode(value).ok())
        .ok_or(PaymentSecretsError::Integrity(
            "encrypted payment configuration field is not base64",
        ))
}

fn derive_encryption_key(app_key: &str) -> [u8; 32] {
    let mut digest = Sha256::new();
    digest.update(KEY_DERIVATION_DOMAIN);
    digest.update(app_key.as_bytes());
    digest.finalize().into()
}

fn encryption_aad(payment: &str, uuid: &str) -> Vec<u8> {
    let mut aad = Vec::with_capacity(ENCRYPTION_DOMAIN.len() + 16 + payment.len() + uuid.len());
    aad.extend_from_slice(ENCRYPTION_DOMAIN);
    aad_field(&mut aad, payment.as_bytes());
    aad_field(&mut aad, uuid.as_bytes());
    aad
}

fn aad_field(aad: &mut Vec<u8>, value: &[u8]) {
    aad.extend_from_slice(&(value.len() as u64).to_be_bytes());
    aad.extend_from_slice(value);
}

fn derive_nonce(app_key: &str, payment: &str, uuid: &str, plaintext: &[u8]) -> [u8; 12] {
    let mut mac = <Hmac<Sha256> as KeyInit>::new_from_slice(app_key.as_bytes())
        .expect("HMAC accepts keys of any length");
    mac.update(NONCE_DOMAIN);
    for field in [payment.as_bytes(), uuid.as_bytes(), plaintext] {
        mac.update(&(field.len() as u64).to_be_bytes());
        mac.update(field);
    }
    let digest = mac.finalize().into_bytes();
    let mut nonce = [0_u8; 12];
    nonce.copy_from_slice(&digest[..12]);
    nonce
}

fn encrypt(
    app_key: &str,
    nonce: &[u8; 12],
    plaintext: &[u8],
    aad: &[u8],
) -> Result<(Vec<u8>, [u8; 16]), openssl::error::ErrorStack> {
    let cipher = Cipher::aes_256_gcm();
    let key = derive_encryption_key(app_key);
    let mut crypter = Crypter::new(cipher, Mode::Encrypt, &key, Some(nonce))?;
    crypter.pad(false);
    crypter.aad_update(aad)?;
    let mut ciphertext = vec![0_u8; plaintext.len() + cipher.block_size()];
    let mut length = crypter.update(plaintext, &mut ciphertext)?;
    length += crypter.finalize(&mut ciphertext[length..])?;
    ciphertext.truncate(length);
    let mut tag = [0_u8; 16];
    crypter.get_tag(&mut tag)?;
    Ok((ciphertext, tag))
}

fn decrypt(
    app_key: &str,
    nonce: &[u8; 12],
    ciphertext: &[u8],
    tag: &[u8; 16],
    aad: &[u8],
) -> Result<Vec<u8>, openssl::error::ErrorStack> {
    let cipher = Cipher::aes_256_gcm();
    let key = derive_encryption_key(app_key);
    let mut crypter = Crypter::new(cipher, Mode::Decrypt, &key, Some(nonce))?;
    crypter.pad(false);
    crypter.aad_update(aad)?;
    crypter.set_tag(tag)?;
    let mut plaintext = vec![0_u8; ciphertext.len() + cipher.block_size()];
    let mut length = crypter.update(ciphertext, &mut plaintext)?;
    length += crypter.finalize(&mut plaintext[length..])?;
    plaintext.truncate(length);
    Ok(plaintext)
}

#[cfg(test)]
mod tests {
    use super::*;

    const APP_KEY: &str = "payment-secrets-test-app-key-32-bytes-long";

    fn fixture_config() -> Map<String, Value> {
        let Value::Object(config) = json!({
            "url": "https://pay.example.test",
            "pid": "1000",
            "key": "epay-super-secret",
        }) else {
            unreachable!("fixture is an object");
        };
        config
    }

    #[test]
    fn round_trip_restores_the_plaintext_object() {
        let config = fixture_config();
        let envelope = encrypt_payment_config(APP_KEY, "EPay", "uuid-1", &config).unwrap();
        let raw = envelope.to_string();
        assert!(!raw.contains("epay-super-secret"));
        let decrypted = decrypt_payment_config(APP_KEY, "EPay", "uuid-1", &raw).unwrap();
        assert_eq!(decrypted, Value::Object(config));
    }

    #[test]
    fn aad_binds_driver_uuid_and_key() {
        let envelope = encrypt_payment_config(APP_KEY, "EPay", "uuid-1", &fixture_config())
            .unwrap()
            .to_string();
        assert!(decrypt_payment_config(APP_KEY, "MGate", "uuid-1", &envelope).is_err());
        assert!(decrypt_payment_config(APP_KEY, "EPay", "uuid-2", &envelope).is_err());
        assert!(
            decrypt_payment_config(
                "other-app-key-32-bytes-material-x",
                "EPay",
                "uuid-1",
                &envelope
            )
            .is_err()
        );
        assert!(decrypt_payment_config(APP_KEY, "EPay", "uuid-1", &envelope).is_ok());
    }

    #[test]
    fn same_inputs_produce_the_identical_envelope() {
        let first = encrypt_payment_config(APP_KEY, "EPay", "uuid-1", &fixture_config()).unwrap();
        let second = encrypt_payment_config(APP_KEY, "EPay", "uuid-1", &fixture_config()).unwrap();
        assert_eq!(first.to_string(), second.to_string());
        let other_row =
            encrypt_payment_config(APP_KEY, "EPay", "uuid-2", &fixture_config()).unwrap();
        assert_ne!(first["ciphertext"], other_row["ciphertext"]);
    }

    #[test]
    fn non_envelope_configs_are_a_hard_integrity_error() {
        for raw in [
            r#"{"url":"https://pay.example.test","key":"plaintext"}"#,
            r#"["not","an","object"]"#,
            r#"{"format_version":1,"nonce":"AAAA","ciphertext":"AAAA","tag":"AAAA","extra":1}"#,
        ] {
            assert!(matches!(
                decrypt_payment_config(APP_KEY, "EPay", "uuid-1", raw),
                Err(PaymentSecretsError::Integrity(_))
            ));
        }
        assert!(matches!(
            decrypt_payment_config(APP_KEY, "EPay", "uuid-1", "not-json"),
            Err(PaymentSecretsError::Json(_))
        ));
    }

    #[test]
    fn unsupported_format_version_is_rejected_before_decryption() {
        let mut envelope =
            encrypt_payment_config(APP_KEY, "EPay", "uuid-1", &fixture_config()).unwrap();
        envelope["format_version"] = json!(2);
        let error = decrypt_payment_config(APP_KEY, "EPay", "uuid-1", &envelope.to_string())
            .expect_err("unsupported format must fail closed");
        assert!(error.to_string().contains("unsupported"));
    }

    #[test]
    fn tampered_ciphertext_fails_authentication() {
        let mut envelope =
            encrypt_payment_config(APP_KEY, "EPay", "uuid-1", &fixture_config()).unwrap();
        let mut ciphertext = STANDARD
            .decode(envelope["ciphertext"].as_str().unwrap())
            .unwrap();
        ciphertext[0] ^= 0x01;
        envelope["ciphertext"] = json!(STANDARD.encode(ciphertext));
        assert!(matches!(
            decrypt_payment_config(APP_KEY, "EPay", "uuid-1", &envelope.to_string()),
            Err(PaymentSecretsError::Integrity(_))
        ));
    }

    #[test]
    fn canonical_entry_round_trips_exact_plaintext_bytes() {
        let plaintext = br#"{"account":"import","exact":900719925474099325e-2}"#;
        let envelope =
            encrypt_payment_config_canonical(APP_KEY, "Manual", "uuid-9", plaintext).unwrap();
        let decrypted =
            decrypt_payment_config_canonical(APP_KEY, "Manual", "uuid-9", &envelope.to_string())
                .unwrap();
        assert_eq!(decrypted, plaintext);
        assert!(matches!(
            encrypt_payment_config_canonical(APP_KEY, "Manual", "uuid-9", b"not-json"),
            Err(PaymentSecretsError::Integrity(_))
        ));
    }

    #[test]
    fn decrypted_non_object_plaintext_is_rejected() {
        let envelope =
            encrypt_payment_config_canonical(APP_KEY, "Manual", "uuid-9", b"[1,2]").unwrap();
        assert!(matches!(
            decrypt_payment_config(APP_KEY, "Manual", "uuid-9", &envelope.to_string()),
            Err(PaymentSecretsError::Integrity(_))
        ));
    }
}
