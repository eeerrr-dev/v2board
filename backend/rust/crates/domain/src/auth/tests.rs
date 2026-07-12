use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use v2board_compat::ApiError;

use super::{
    password::{PasswordKdf, hash_password, password_needs_rehash, verify_password},
    registration::{checked_trial_expired_at, checked_trial_transfer_bytes},
    sessions::{
        ADD_OPAQUE_SESSION_SCRIPT, AUTH_SESSION_KEY_PREFIX, AuthClaims, auth_session_key,
        decode_session_metadata, effective_legacy_jwt_cutoff, generate_auth_token,
        looks_like_legacy_jwt, parse_temp_token,
    },
    validation::{
        is_valid_email, validate_change_password, validate_email, validate_forget,
        validate_password,
    },
};

#[test]
fn trial_plan_math_rejects_negative_and_overflowing_configuration() {
    assert_eq!(checked_trial_transfer_bytes(2).unwrap(), 2_147_483_648);
    assert!(checked_trial_transfer_bytes(-1).is_err());
    assert!(checked_trial_transfer_bytes(i64::MAX).is_err());

    assert_eq!(checked_trial_expired_at(100, 0).unwrap(), 3_700);
    assert_eq!(checked_trial_expired_at(100, 2).unwrap(), 7_300);
    assert!(checked_trial_expired_at(100, -1).is_err());
    assert!(checked_trial_expired_at(100, i64::MAX).is_err());
    assert!(checked_trial_expired_at(i64::MAX, 1).is_err());
}

#[test]
fn is_valid_email_accepts_structural_addresses_and_rejects_malformed() {
    assert!(is_valid_email("user@example.com"));
    assert!(is_valid_email("user@localhost"));
    assert!(!is_valid_email("notanemail"));
    assert!(!is_valid_email("@example.com"));
    assert!(!is_valid_email("user@"));
    assert!(!is_valid_email("a@b@c"));
    assert!(!is_valid_email("user name@example.com"));
}

#[test]
fn validate_email_reports_validation_error_with_laravel_messages() {
    assert!(validate_email("user@example.com").is_ok());
    let empty = validate_email("   ").unwrap_err();
    assert_eq!(empty.to_string(), "Email can not be empty");
    assert!(matches!(empty, ApiError::Validation { .. }));
    let malformed = validate_email("bad").unwrap_err();
    assert_eq!(malformed.to_string(), "Email format is incorrect");
    assert!(matches!(malformed, ApiError::Validation { .. }));
}

#[test]
fn validate_password_counts_characters_not_bytes() {
    assert!(validate_password("password").is_ok());
    // Six multibyte characters (18 bytes): a byte-length check would pass, char count fails.
    assert_eq!(
        validate_password("七个中文密码").unwrap_err().to_string(),
        "Password must be greater than 8 digits"
    );
    assert_eq!(
        validate_password("").unwrap_err().to_string(),
        "Password can not be empty"
    );
}

#[test]
fn validate_forget_mirrors_authforget_rules() {
    assert!(validate_forget("user@example.com", "password", "123456").is_ok());

    // email: required -> format -> max:64 (character count)
    let empty_email = validate_forget("", "password", "123456").unwrap_err();
    assert_eq!(empty_email.to_string(), "Email can not be empty");
    assert!(matches!(empty_email, ApiError::Validation { .. }));
    assert_eq!(
        validate_forget("bad", "password", "123456")
            .unwrap_err()
            .to_string(),
        "Email format is incorrect"
    );
    let long_email = format!("{}@example.com", "a".repeat(60)); // 72 chars > 64
    assert_eq!(
        validate_forget(&long_email, "password", "123456")
            .unwrap_err()
            .to_string(),
        "Email format is incorrect"
    );

    // password: min:8 and max:64 are character counts, not bytes
    assert_eq!(
        validate_forget("user@example.com", "七个中文密码", "123456")
            .unwrap_err()
            .to_string(),
        "Password must be greater than 8 digits"
    );
    assert_eq!(
        validate_forget("user@example.com", &"a".repeat(65), "123456")
            .unwrap_err()
            .to_string(),
        "Password must be greater than 8 digits"
    );

    // email_code: required (distinct message) then digits:6
    let empty_code = validate_forget("user@example.com", "password", "  ").unwrap_err();
    assert_eq!(
        empty_code.to_string(),
        "Email verification code cannot be empty"
    );
    assert!(matches!(empty_code, ApiError::Validation { .. }));
    assert_eq!(
        validate_forget("user@example.com", "password", "12345")
            .unwrap_err()
            .to_string(),
        "Incorrect email verification code"
    );
    assert_eq!(
        validate_forget("user@example.com", "password", "12345a")
            .unwrap_err()
            .to_string(),
        "Incorrect email verification code"
    );
}

#[test]
fn validate_change_password_mirrors_userchangepassword_rules() {
    assert!(validate_change_password("old-secret", "new-secret").is_ok());

    // old_password required takes precedence over new_password rules.
    let empty_old = validate_change_password("", "short").unwrap_err();
    assert_eq!(empty_old.to_string(), "Old password cannot be empty");
    assert!(matches!(empty_old, ApiError::Validation { .. }));

    // new_password required reports its own message, not the min message.
    assert_eq!(
        validate_change_password("old-secret", "")
            .unwrap_err()
            .to_string(),
        "New password cannot be empty"
    );

    // min:8 counts characters (mb_strlen), so a 6-glyph multibyte password fails.
    assert_eq!(
        validate_change_password("old-secret", "七个中文密码")
            .unwrap_err()
            .to_string(),
        "Password must be greater than 8 digits"
    );
}

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

#[test]
fn temporary_login_tokens_bind_the_session_epoch_and_read_legacy_values() {
    assert_eq!(parse_temp_token("42:7"), Some((42, 7)));
    assert_eq!(parse_temp_token("42"), Some((42, 0)));
    assert_eq!(parse_temp_token("bad"), None);
    assert_eq!(parse_temp_token("42:bad"), None);
}

#[test]
fn pre_epoch_auth_claims_deserialize_as_epoch_zero() {
    let claims: AuthClaims =
        serde_json::from_value(serde_json::json!({ "id": 9, "session": "legacy" })).unwrap();
    assert_eq!(claims.session_epoch, 0);
}

#[test]
fn opaque_tokens_are_256_bit_url_safe_secrets_and_redis_keys_are_hashed() {
    let first = generate_auth_token().unwrap();
    let second = generate_auth_token().unwrap();
    assert_eq!(URL_SAFE_NO_PAD.decode(&first).unwrap().len(), 32);
    assert_eq!(first.len(), 43);
    assert_ne!(first, second);

    let key = auth_session_key(&first);
    assert!(key.starts_with(AUTH_SESSION_KEY_PREFIX));
    assert!(!key.contains(&first));
    assert_eq!(key.len(), AUTH_SESSION_KEY_PREFIX.len() + 64);
}

#[test]
fn missing_session_metadata_is_empty_but_corruption_fails_closed() {
    assert!(decode_session_metadata(None).unwrap().is_empty());
    assert!(matches!(
        decode_session_metadata(Some("not-json")),
        Err(ApiError::Internal(_))
    ));
    let decoded = decode_session_metadata(Some(r#"{"session":{"expires_at":1}}"#)).unwrap();
    assert!(decoded.contains_key("session"));
}

#[test]
fn only_three_segment_values_enter_the_bounded_legacy_jwt_path() {
    assert!(looks_like_legacy_jwt("header.payload.signature"));
    assert!(!looks_like_legacy_jwt("opaque-token"));
    assert!(!looks_like_legacy_jwt("two.parts"));
    assert!(!looks_like_legacy_jwt("too.many.jwt.parts"));
}

#[test]
fn configured_legacy_cutoff_can_only_shorten_the_persisted_window() {
    assert_eq!(effective_legacy_jwt_cutoff(2_000, 3_000), 2_000);
    assert_eq!(effective_legacy_jwt_cutoff(3_000, 2_000), 2_000);
    assert_eq!(effective_legacy_jwt_cutoff(3_000, 0), 0);
    assert_eq!(effective_legacy_jwt_cutoff(3_000, -1), 0);
}

#[test]
fn opaque_session_script_sets_absolute_ttl_without_storing_the_bearer() {
    assert!(ADD_OPAQUE_SESSION_SCRIPT.contains("'NX'"));
    assert!(ADD_OPAQUE_SESSION_SCRIPT.contains("'EX', ARGV[4]"));
    assert!(!ADD_OPAQUE_SESSION_SCRIPT.contains("auth_data"));
}

#[test]
fn registration_limiter_failure_is_best_effort_after_commit() {
    let source = include_str!("registration.rs");
    let commit = source.find("tx.commit().await?").unwrap();
    let limiter_warning = source
        .find("registration IP limiter update failed after committed account creation")
        .unwrap();
    let auth_data = source.find("self.auth_data_for_user").unwrap();
    assert!(commit < limiter_warning);
    assert!(limiter_warning < auth_data);
    assert!(source.contains("duration_minutes_to_seconds(self.config.register_limit_expire)"));
    let best_effort = source[commit..auth_data].find("if let Err(error)").unwrap() + commit;
    assert!(!source[best_effort..auth_data].contains(".await?"));
}
