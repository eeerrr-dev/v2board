use hmac::{Hmac, KeyInit, Mac};
use openssl::symm::{Cipher, Crypter, Mode};
use serde_json::{Map, Value};
use sha2::{Digest, Sha256};
use sqlx::types::Json;
use uuid::Uuid;
use v2board_config::{AppConfig, OPERATOR_CONFIG_KEYS_V1};
use v2board_db::DbPool;

const ENCRYPTION_DOMAIN: &[u8] = b"v2board/operator-config/aes-256-gcm/v1\0";
const HMAC_DOMAIN: &[u8] = b"v2board/operator-config/hmac-sha256/v1\0";
const KEY_DERIVATION_DOMAIN: &[u8] = b"v2board/operator-config/key/v1\0";
pub const OPERATOR_CONFIG_FORMAT_VERSION: i16 = 1;
const API_ACK_SQL: &str = r#"
    INSERT INTO operator_config_api_ack
        (singleton, installation_id, observed_revision, applied_revision,
         status, error_code, observed_at)
    VALUES (1, $1, $2, $3, $4, $5, $6)
    ON CONFLICT (singleton) DO UPDATE SET
        observed_revision = EXCLUDED.observed_revision,
        applied_revision = EXCLUDED.applied_revision,
        status = EXCLUDED.status,
        error_code = EXCLUDED.error_code,
        observed_at = EXCLUDED.observed_at
    "#;
const WORKER_ACK_SQL: &str = r#"
    INSERT INTO operator_config_worker_ack
        (singleton, installation_id, observed_revision, applied_revision,
         status, error_code, observed_at)
    VALUES (1, $1, $2, $3, $4, $5, $6)
    ON CONFLICT (singleton) DO UPDATE SET
        observed_revision = EXCLUDED.observed_revision,
        applied_revision = EXCLUDED.applied_revision,
        status = EXCLUDED.status,
        error_code = EXCLUDED.error_code,
        observed_at = EXCLUDED.observed_at
    "#;
const SECRET_KEYS: &[&str] = &[
    "server_token",
    "email_password",
    "telegram_bot_token",
    "recaptcha_key",
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OperatorConfigConsumer {
    Api,
    Worker,
}

#[derive(Clone)]
pub struct OperatorConfigSnapshot {
    pub revision: i64,
    pub revision_id: Uuid,
    pub format_version: i16,
    pub values: Map<String, Value>,
    pub config_hmac_sha256: String,
}

#[derive(Debug, thiserror::Error)]
pub enum OperatorConfigError {
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),
    #[error("operator configuration JSON error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("operator configuration cryptography error: {0}")]
    Crypto(#[from] openssl::error::ErrorStack),
    #[error("operator configuration entropy source failed: {0}")]
    Random(String),
    #[error("operator configuration integrity check failed: {0}")]
    Integrity(&'static str),
    #[error("operator configuration is invalid: {0}")]
    Invalid(String),
    #[error(
        "operator configuration revision {observed_revision} was rejected ({error_code}): {detail}"
    )]
    RejectedRevision {
        observed_revision: i64,
        error_code: &'static str,
        detail: String,
    },
    #[error(
        "operator configuration changed concurrently (expected revision {expected:?}, active revision {actual:?})"
    )]
    Conflict {
        expected: Option<i64>,
        actual: Option<i64>,
    },
    #[error("operator configuration authority has not been initialized")]
    MissingAuthority,
    #[error(
        "existing operator configuration revision {revision} differs from the requested initial candidate"
    )]
    InitialAuthorityMismatch { revision: i64 },
}

impl OperatorConfigError {
    pub const fn observed_rejection(&self) -> Option<(i64, &'static str)> {
        match self {
            Self::RejectedRevision {
                observed_revision,
                error_code,
                ..
            } => Some((*observed_revision, *error_code)),
            _ => None,
        }
    }
}

struct EncryptedRevision {
    revision_id: Uuid,
    format_version: i16,
    public_config: Map<String, Value>,
    nonce: [u8; 12],
    ciphertext: Vec<u8>,
    tag: [u8; 16],
    config_hmac_sha256: String,
}

/// API-only cold-start initialization. The first API process commits its full,
/// typed bootstrap behavior as the first active revision. A concurrent
/// initializer either wins the singleton insert or observes the winner; no
/// empty/missing authority is treated as a valid running state.
pub async fn ensure_authority(
    db: &DbPool,
    installation_id: Uuid,
    bootstrap: &AppConfig,
) -> Result<OperatorConfigSnapshot, OperatorConfigError> {
    if let Some(snapshot) = load_active(db, installation_id, &bootstrap.app_key).await? {
        return Ok(snapshot);
    }

    let candidate = bootstrap.operator_config_map();
    match commit(
        db,
        installation_id,
        &bootstrap.app_key,
        None,
        &candidate,
        "api:bootstrap",
    )
    .await
    {
        Ok(snapshot) => Ok(snapshot),
        Err(error) => match load_active(db, installation_id, &bootstrap.app_key).await {
            Ok(Some(snapshot)) => Ok(snapshot),
            Ok(None) => Err(error),
            Err(load_error) => Err(load_error),
        },
    }
}

/// Lifecycle-only, crash-resumable initialization. A retry succeeds only when
/// the already-active revision authenticates and is exactly the candidate the
/// caller intended to seed. State/revision drift is never "repaired" by adding
/// another row because that would hide an incomplete or tampered bootstrap.
pub async fn ensure_initial_authority_exact(
    db: &DbPool,
    installation_id: Uuid,
    app_key: &str,
    candidate: &Map<String, Value>,
    actor: &str,
) -> Result<OperatorConfigSnapshot, OperatorConfigError> {
    validate_complete_key_set(candidate)?;
    if let Some(snapshot) = load_active(db, installation_id, app_key).await? {
        return exact_initial_snapshot(snapshot, candidate);
    }

    let (state_exists, revision_exists) = sqlx::query_as::<_, (bool, bool)>(
        r#"
        SELECT
            EXISTS (SELECT 1 FROM operator_config_state),
            EXISTS (SELECT 1 FROM operator_config_revision)
        "#,
    )
    .fetch_one(db)
    .await?;
    if state_exists || revision_exists {
        return Err(OperatorConfigError::Integrity(
            "operator configuration authority contains orphaned or mismatched rows",
        ));
    }

    match commit(db, installation_id, app_key, None, candidate, actor).await {
        Ok(snapshot) => Ok(snapshot),
        Err(commit_error) => match load_active(db, installation_id, app_key).await {
            Ok(Some(snapshot)) => exact_initial_snapshot(snapshot, candidate),
            Ok(None) => Err(commit_error),
            Err(load_error) => Err(load_error),
        },
    }
}

fn exact_initial_snapshot(
    snapshot: OperatorConfigSnapshot,
    candidate: &Map<String, Value>,
) -> Result<OperatorConfigSnapshot, OperatorConfigError> {
    if snapshot.values == *candidate {
        Ok(snapshot)
    } else {
        Err(OperatorConfigError::InitialAuthorityMismatch {
            revision: snapshot.revision,
        })
    }
}

/// Reads, authenticates, and decrypts the one active operator revision. An
/// integrity failure is fatal to the read; callers must retain their
/// last-known-good in-memory snapshot and record a rejection.
pub async fn load_active(
    db: &DbPool,
    installation_id: Uuid,
    app_key: &str,
) -> Result<Option<OperatorConfigSnapshot>, OperatorConfigError> {
    type RevisionRow = (
        i64,
        Uuid,
        i16,
        Uuid,
        Json<Value>,
        Vec<u8>,
        Vec<u8>,
        Vec<u8>,
        String,
    );
    let row = sqlx::query_as::<_, RevisionRow>(
        r#"
        SELECT r.revision, r.revision_id, r.format_version, r.installation_id, r.public_config,
               r.secret_nonce, r.secret_ciphertext, r.secret_tag,
               r.config_hmac_sha256
        FROM operator_config_state AS s
        JOIN operator_config_revision AS r
          ON r.revision = s.active_revision
         AND r.installation_id = s.installation_id
        WHERE s.singleton = 1
        "#,
    )
    .fetch_optional(db)
    .await?;
    let Some((
        revision,
        revision_id,
        format_version,
        stored_installation_id,
        public,
        nonce,
        ciphertext,
        tag,
        hmac,
    )) = row
    else {
        return Ok(None);
    };
    decode_active_revision(
        revision,
        revision_id,
        format_version,
        stored_installation_id,
        public,
        nonce,
        ciphertext,
        tag,
        hmac,
        installation_id,
        app_key,
    )
    .map(Some)
    .map_err(|error| rejected_revision_error(revision, error))
}

#[allow(clippy::too_many_arguments)]
fn decode_active_revision(
    revision: i64,
    revision_id: Uuid,
    format_version: i16,
    stored_installation_id: Uuid,
    public: Json<Value>,
    nonce: Vec<u8>,
    ciphertext: Vec<u8>,
    tag: Vec<u8>,
    hmac: String,
    installation_id: Uuid,
    app_key: &str,
) -> Result<OperatorConfigSnapshot, OperatorConfigError> {
    if format_version != OPERATOR_CONFIG_FORMAT_VERSION {
        return Err(OperatorConfigError::Integrity(
            "unsupported operator configuration format version",
        ));
    }
    if stored_installation_id != installation_id {
        return Err(OperatorConfigError::Integrity("installation mismatch"));
    }
    let public_config = public
        .0
        .as_object()
        .cloned()
        .ok_or(OperatorConfigError::Integrity(
            "public configuration is not an object",
        ))?;
    if SECRET_KEYS
        .iter()
        .any(|key| public_config.contains_key(*key))
    {
        return Err(OperatorConfigError::Integrity(
            "public configuration contains a secret",
        ));
    }
    let nonce: [u8; 12] = nonce
        .try_into()
        .map_err(|_| OperatorConfigError::Integrity("invalid nonce length"))?;
    let tag: [u8; 16] = tag
        .try_into()
        .map_err(|_| OperatorConfigError::Integrity("invalid tag length"))?;
    let public_digest = Sha256::digest(canonical_object_bytes(&public_config)?);
    let aad = encryption_aad(installation_id, revision_id, format_version, &public_digest);
    let plaintext = decrypt(app_key, &nonce, &ciphertext, &tag, &aad)?;
    let secrets = serde_json::from_slice::<Value>(&plaintext)?
        .as_object()
        .cloned()
        .ok_or(OperatorConfigError::Integrity(
            "secret configuration is not an object",
        ))?;
    if secrets
        .keys()
        .any(|key| !SECRET_KEYS.contains(&key.as_str()))
        || SECRET_KEYS.iter().any(|key| !secrets.contains_key(*key))
    {
        return Err(OperatorConfigError::Integrity(
            "secret configuration key set is invalid",
        ));
    }

    let mut values = public_config;
    for (key, value) in secrets {
        values.insert(key, value);
    }
    validate_complete_key_set(&values)?;
    verify_config_hmac(
        app_key,
        installation_id,
        revision_id,
        format_version,
        &values,
        &hmac,
    )?;
    Ok(OperatorConfigSnapshot {
        revision,
        revision_id,
        format_version,
        values,
        config_hmac_sha256: hmac,
    })
}

fn rejected_revision_error(revision: i64, error: OperatorConfigError) -> OperatorConfigError {
    if matches!(error, OperatorConfigError::RejectedRevision { .. }) {
        return error;
    }
    let error_code = match &error {
        OperatorConfigError::Json(_) => "payload_decode_failed",
        OperatorConfigError::Crypto(_) => "authentication_failed",
        OperatorConfigError::Invalid(_) => "invalid_key_set",
        OperatorConfigError::Integrity(_) => "integrity_check_failed",
        _ => "revision_decode_failed",
    };
    OperatorConfigError::RejectedRevision {
        observed_revision: revision,
        error_code,
        detail: error.to_string(),
    }
}

/// Commits an immutable revision and advances the singleton active pointer in
/// one PostgreSQL transaction. Typed AppConfig validation must happen before
/// this function; it independently enforces the exact authority key set.
pub async fn commit(
    db: &DbPool,
    installation_id: Uuid,
    app_key: &str,
    expected_revision: Option<i64>,
    candidate: &Map<String, Value>,
    actor: &str,
) -> Result<OperatorConfigSnapshot, OperatorConfigError> {
    validate_complete_key_set(candidate)?;
    let encrypted = encrypt_revision(app_key, installation_id, candidate)?;
    let now = chrono::Utc::now().timestamp();
    let actor = normalized_actor(actor);
    let mut tx = db.begin().await?;
    let active = sqlx::query_as::<_, (Uuid, i64)>(
        "SELECT installation_id, active_revision FROM operator_config_state WHERE singleton = 1 FOR UPDATE",
    )
    .fetch_optional(&mut *tx)
    .await?;
    if let Some((stored_installation_id, _)) = active
        && stored_installation_id != installation_id
    {
        return Err(OperatorConfigError::Integrity("installation mismatch"));
    }
    let actual = active.map(|(_, revision)| revision);
    if actual != expected_revision {
        return Err(OperatorConfigError::Conflict {
            expected: expected_revision,
            actual,
        });
    }

    let revision = sqlx::query_scalar::<_, i64>(
        r#"
        INSERT INTO operator_config_revision
            (revision_id, format_version, installation_id, public_config, secret_nonce,
             secret_ciphertext, secret_tag, config_hmac_sha256, created_by, created_at)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
        RETURNING revision
        "#,
    )
    .bind(encrypted.revision_id)
    .bind(encrypted.format_version)
    .bind(installation_id)
    .bind(Json(Value::Object(encrypted.public_config.clone())))
    .bind(encrypted.nonce.as_slice())
    .bind(&encrypted.ciphertext)
    .bind(encrypted.tag.as_slice())
    .bind(&encrypted.config_hmac_sha256)
    .bind(actor)
    .bind(now)
    .fetch_one(&mut *tx)
    .await?;

    match actual {
        Some(previous) => {
            let changed = sqlx::query(
                r#"
                UPDATE operator_config_state
                SET active_revision = $1, updated_at = $2
                WHERE singleton = 1 AND installation_id = $3 AND active_revision = $4
                "#,
            )
            .bind(revision)
            .bind(now)
            .bind(installation_id)
            .bind(previous)
            .execute(&mut *tx)
            .await?;
            if changed.rows_affected() != 1 {
                return Err(OperatorConfigError::Conflict {
                    expected: Some(previous),
                    actual: None,
                });
            }
        }
        None => {
            sqlx::query(
                "INSERT INTO operator_config_state (singleton, installation_id, active_revision, updated_at) VALUES (1, $1, $2, $3)",
            )
            .bind(installation_id)
            .bind(revision)
            .bind(now)
            .execute(&mut *tx)
            .await?;
        }
    }
    tx.commit().await?;

    Ok(OperatorConfigSnapshot {
        revision,
        revision_id: encrypted.revision_id,
        format_version: encrypted.format_version,
        values: candidate.clone(),
        config_hmac_sha256: encrypted.config_hmac_sha256,
    })
}

/// Records either a successful application or a typed/integrity rejection.
/// API and worker use physically separate tables so PostgreSQL grants can make
/// the worker incapable of acknowledging on the API's behalf.
pub async fn acknowledge(
    db: &DbPool,
    installation_id: Uuid,
    consumer: OperatorConfigConsumer,
    observed_revision: i64,
    applied_revision: Option<i64>,
    error_code: Option<&str>,
) -> Result<(), OperatorConfigError> {
    if observed_revision <= 0 || applied_revision.is_some_and(|revision| revision <= 0) {
        return Err(OperatorConfigError::Invalid(
            "acknowledgement revisions must be positive".to_string(),
        ));
    }
    let rejected = error_code.is_some();
    if (!rejected && applied_revision != Some(observed_revision))
        || (rejected
            && (applied_revision.is_some_and(|revision| revision >= observed_revision)
                || error_code.is_none_or(|code| {
                    code.is_empty()
                        || code.len() > 64
                        || !code.bytes().all(|byte| {
                            byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_'
                        })
                })))
    {
        return Err(OperatorConfigError::Invalid(
            "acknowledgement status is inconsistent".to_string(),
        ));
    }
    let status = if rejected { "rejected" } else { "applied" };
    let now = chrono::Utc::now().timestamp();
    let sql = match consumer {
        OperatorConfigConsumer::Api => API_ACK_SQL,
        OperatorConfigConsumer::Worker => WORKER_ACK_SQL,
    };
    sqlx::query(sql)
        .bind(installation_id)
        .bind(observed_revision)
        .bind(applied_revision)
        .bind(status)
        .bind(error_code)
        .bind(now)
        .execute(db)
        .await?;
    Ok(())
}

fn validate_complete_key_set(values: &Map<String, Value>) -> Result<(), OperatorConfigError> {
    if let Some(key) = values
        .keys()
        .find(|key| !OPERATOR_CONFIG_KEYS_V1.contains(&key.as_str()))
    {
        return Err(OperatorConfigError::Invalid(format!(
            "unsupported key {key}"
        )));
    }
    if let Some(key) = OPERATOR_CONFIG_KEYS_V1
        .iter()
        .find(|key| !values.contains_key(**key))
    {
        return Err(OperatorConfigError::Invalid(format!(
            "missing required key {key}"
        )));
    }
    Ok(())
}

fn encrypt_revision(
    app_key: &str,
    installation_id: Uuid,
    candidate: &Map<String, Value>,
) -> Result<EncryptedRevision, OperatorConfigError> {
    let revision_id = Uuid::new_v4();
    let format_version = OPERATOR_CONFIG_FORMAT_VERSION;
    let mut public_config = candidate.clone();
    let mut secrets = Map::new();
    for key in SECRET_KEYS {
        let value = public_config
            .remove(*key)
            .ok_or_else(|| OperatorConfigError::Invalid(format!("missing secret key {key}")))?;
        secrets.insert((*key).to_string(), value);
    }
    let public_digest = Sha256::digest(canonical_object_bytes(&public_config)?);
    let aad = encryption_aad(installation_id, revision_id, format_version, &public_digest);
    let plaintext = canonical_object_bytes(&secrets)?;
    let mut nonce = [0_u8; 12];
    getrandom::fill(&mut nonce).map_err(|error| OperatorConfigError::Random(error.to_string()))?;
    let (ciphertext, tag) = encrypt(app_key, &nonce, &plaintext, &aad)?;
    let config_hmac_sha256 = config_hmac(
        app_key,
        installation_id,
        revision_id,
        format_version,
        candidate,
    )?;
    Ok(EncryptedRevision {
        revision_id,
        format_version,
        public_config,
        nonce,
        ciphertext,
        tag,
        config_hmac_sha256,
    })
}

fn derive_encryption_key(app_key: &str) -> [u8; 32] {
    let mut digest = Sha256::new();
    digest.update(KEY_DERIVATION_DOMAIN);
    digest.update(app_key.as_bytes());
    digest.finalize().into()
}

fn encryption_aad(
    installation_id: Uuid,
    revision_id: Uuid,
    format_version: i16,
    public_digest: &[u8],
) -> Vec<u8> {
    let mut aad = Vec::with_capacity(ENCRYPTION_DOMAIN.len() + 16 + 16 + 2 + public_digest.len());
    aad.extend_from_slice(ENCRYPTION_DOMAIN);
    aad.extend_from_slice(installation_id.as_bytes());
    aad.extend_from_slice(revision_id.as_bytes());
    aad.extend_from_slice(&format_version.to_be_bytes());
    aad.extend_from_slice(public_digest);
    aad
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

fn config_hmac(
    app_key: &str,
    installation_id: Uuid,
    revision_id: Uuid,
    format_version: i16,
    values: &Map<String, Value>,
) -> Result<String, OperatorConfigError> {
    let mut mac = <Hmac<Sha256> as KeyInit>::new_from_slice(app_key.as_bytes())
        .expect("HMAC accepts keys of any length");
    mac.update(HMAC_DOMAIN);
    mac.update(installation_id.as_bytes());
    mac.update(revision_id.as_bytes());
    mac.update(&format_version.to_be_bytes());
    mac.update(&canonical_object_bytes(values)?);
    Ok(hex::encode(mac.finalize().into_bytes()))
}

fn verify_config_hmac(
    app_key: &str,
    installation_id: Uuid,
    revision_id: Uuid,
    format_version: i16,
    values: &Map<String, Value>,
    expected_hex: &str,
) -> Result<(), OperatorConfigError> {
    let expected = hex::decode(expected_hex)
        .map_err(|_| OperatorConfigError::Integrity("invalid HMAC encoding"))?;
    let mut mac = <Hmac<Sha256> as KeyInit>::new_from_slice(app_key.as_bytes())
        .expect("HMAC accepts keys of any length");
    mac.update(HMAC_DOMAIN);
    mac.update(installation_id.as_bytes());
    mac.update(revision_id.as_bytes());
    mac.update(&format_version.to_be_bytes());
    mac.update(&canonical_object_bytes(values)?);
    mac.verify_slice(&expected)
        .map_err(|_| OperatorConfigError::Integrity("configuration HMAC mismatch"))
}

fn canonical_object_bytes(values: &Map<String, Value>) -> Result<Vec<u8>, serde_json::Error> {
    serde_json::to_vec(&Value::Object(values.clone()))
}

fn normalized_actor(actor: &str) -> String {
    let actor = actor.trim();
    if !actor.is_empty() && actor.len() <= 64 {
        return actor.to_string();
    }
    let digest = Sha256::digest(actor.as_bytes());
    hex::encode(digest)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn encryption_round_trip_binds_aad() {
        let nonce = [7_u8; 12];
        let plaintext = br#"{"server_token":"not-public"}"#;
        let (ciphertext, tag) = encrypt("test-app-key", &nonce, plaintext, b"revision-a").unwrap();
        assert_eq!(
            decrypt("test-app-key", &nonce, &ciphertext, &tag, b"revision-a").unwrap(),
            plaintext
        );
        assert!(decrypt("test-app-key", &nonce, &ciphertext, &tag, b"revision-b").is_err());
    }

    #[test]
    fn encrypted_revision_keeps_secrets_out_of_public_json() {
        let values = OPERATOR_CONFIG_KEYS_V1
            .iter()
            .map(|key| ((*key).to_string(), Value::Null))
            .collect::<Map<_, _>>();
        let encrypted = encrypt_revision("test-app-key", Uuid::nil(), &values).unwrap();
        for key in SECRET_KEYS {
            assert!(!encrypted.public_config.contains_key(*key));
        }
        let public = serde_json::to_string(&encrypted.public_config).unwrap();
        assert!(!public.contains("server_token"));
    }

    #[test]
    fn full_config_hmac_detects_public_tampering() {
        let installation_id = Uuid::new_v4();
        let revision_id = Uuid::new_v4();
        let mut values = Map::from_iter([
            ("app_name".to_string(), json!("V2Board")),
            ("server_token".to_string(), json!("secret")),
        ]);
        let hmac = config_hmac(
            "test-app-key",
            installation_id,
            revision_id,
            OPERATOR_CONFIG_FORMAT_VERSION,
            &values,
        )
        .unwrap();
        verify_config_hmac(
            "test-app-key",
            installation_id,
            revision_id,
            OPERATOR_CONFIG_FORMAT_VERSION,
            &values,
            &hmac,
        )
        .unwrap();
        assert!(
            verify_config_hmac(
                "test-app-key",
                installation_id,
                revision_id,
                OPERATOR_CONFIG_FORMAT_VERSION + 1,
                &values,
                &hmac,
            )
            .is_err(),
            "the format version is authenticated"
        );
        values.insert("app_name".to_string(), json!("tampered"));
        assert!(
            verify_config_hmac(
                "test-app-key",
                installation_id,
                revision_id,
                OPERATOR_CONFIG_FORMAT_VERSION,
                &values,
                &hmac,
            )
            .is_err()
        );
    }

    #[test]
    fn oversized_actor_is_a_stable_varchar_64_digest() {
        let actor = normalized_actor(&"administrator@example.invalid/".repeat(8));
        assert_eq!(actor.len(), 64);
        assert!(actor.bytes().all(|byte| byte.is_ascii_hexdigit()));
        assert_eq!(
            actor,
            normalized_actor(&"administrator@example.invalid/".repeat(8))
        );
    }

    #[test]
    fn unsupported_format_is_rejected_before_decryption() {
        let error = match decode_active_revision(
            1,
            Uuid::new_v4(),
            OPERATOR_CONFIG_FORMAT_VERSION + 1,
            Uuid::nil(),
            Json(json!({})),
            vec![0; 12],
            vec![0],
            vec![0; 16],
            "0".repeat(64),
            Uuid::nil(),
            "test-app-key",
        ) {
            Err(error) => error,
            Ok(_) => panic!("unsupported format must fail closed"),
        };
        assert!(error.to_string().contains("unsupported"));
    }
}
