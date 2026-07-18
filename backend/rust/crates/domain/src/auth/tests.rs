use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use rust_decimal::Decimal;
use v2board_compat::{ApiError, Code};

use super::{
    credentials::{RELEASE_LOGIN_ATTEMPT_SCRIPT, RESERVE_LOGIN_ATTEMPT_SCRIPT, login_limiter_keys},
    password::{PasswordKdf, hash_password, password_needs_rehash, verify_password},
    registration::{
        MAX_EMAIL_CODE_BYTES, MAX_INVITE_CODE_BYTES, MAX_RECAPTCHA_DATA_BYTES,
        RELEASE_REGISTRATION_SLOT_SCRIPT, RESERVE_REGISTRATION_SLOT_SCRIPT, RegisterInput,
        checked_trial_expired_at, checked_trial_transfer_bytes,
        validate_registration_auxiliary_inputs,
    },
    sessions::{
        ADD_OPAQUE_SESSION_SCRIPT, AUTH_SESSION_KEY_PREFIX, AuthData, REMOVE_SESSION_SCRIPT,
        RESERVE_STEP_UP_ATTEMPT_SCRIPT, auth_session_key, decode_session_metadata,
        generate_auth_token, parse_temp_token, session_ttl_seconds, step_up_limiter_keys,
        truncate_utf8,
    },
    validation::{
        is_valid_email, normalize_email, validate_change_password, validate_email, validate_forget,
        validate_password,
    },
    verification::{CONSUME_VALUE_WITH_FAILURE_LIMIT_SCRIPT, verify_mail_subject},
};

#[test]
fn trial_plan_math_rejects_negative_and_overflowing_configuration() {
    assert_eq!(checked_trial_transfer_bytes(2).unwrap(), 2_147_483_648);
    assert!(checked_trial_transfer_bytes(-1).is_err());
    assert!(checked_trial_transfer_bytes(i64::MAX).is_err());

    assert_eq!(checked_trial_expired_at(100, Decimal::ZERO).unwrap(), 3_700);
    assert_eq!(
        checked_trial_expired_at(100, Decimal::from(2)).unwrap(),
        7_300
    );
    assert_eq!(
        checked_trial_expired_at(100, Decimal::new(15, 1)).unwrap(),
        5_500
    );
    assert!(checked_trial_expired_at(100, Decimal::NEGATIVE_ONE).is_err());
    assert!(checked_trial_expired_at(100, Decimal::MAX).is_err());
    assert!(checked_trial_expired_at(i64::MAX, Decimal::ONE).is_err());
}

#[test]
fn step_up_attempts_are_atomically_bounded_by_user_and_ip() {
    for fragment in ["GET', KEYS[1]", "GET', KEYS[2]", "INCR", "EXPIRE"] {
        assert!(RESERVE_STEP_UP_ATTEMPT_SCRIPT.contains(fragment));
    }
    let first = step_up_limiter_keys(7, Some("203.0.113.9"));
    let same = step_up_limiter_keys(7, Some("203.0.113.9"));
    let other_user = step_up_limiter_keys(8, Some("203.0.113.9"));
    assert_eq!(first, same);
    assert_ne!(first[0], other_user[0]);
    assert_eq!(first[1], other_user[1]);
    assert!(!first[1].contains("203.0.113.9"));
}

#[test]
fn privileged_users_receive_the_short_session_ttl() {
    assert_eq!(
        session_ttl_seconds(30 * 86_400, 12 * 3_600, 0, 0),
        30 * 86_400
    );
    assert_eq!(
        session_ttl_seconds(30 * 86_400, 12 * 3_600, 1, 0),
        12 * 3_600
    );
    assert_eq!(
        session_ttl_seconds(30 * 86_400, 12 * 3_600, 0, 1),
        12 * 3_600
    );
}

#[test]
fn verify_mail_subject_is_the_zh_cn_default_locale_concatenation() {
    // Legacy: `app_name . __('Email verification code')` rendered under Laravel's pinned
    // default locale zh-CN (CommController.php:78 + resources/lang/zh-CN.json), so the
    // subject language matches the hardcoded zh-CN verify body template.
    assert_eq!(verify_mail_subject("V2Board"), "V2Board邮箱验证码");
}

#[test]
fn password_reset_code_consumption_and_failure_limit_share_one_redis_script() {
    let script = CONSUME_VALUE_WITH_FAILURE_LIMIT_SCRIPT;
    let limit_check = script.find("current >=").unwrap();
    let code_check = script.find("GET', KEYS[1]").unwrap();
    let failure_increment = script.find("INCR', KEYS[2]").unwrap();
    assert!(limit_check < code_check && code_check < failure_increment);
    assert!(script.contains("DEL', KEYS[1]"));
    assert!(script.contains("EXPIRE', KEYS[2]"));
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
    assert!(
        matches!(&empty, ApiError::Problem(problem) if problem.code() == Code::ValidationFailed)
    );
    let malformed = validate_email("bad").unwrap_err();
    assert_eq!(malformed.to_string(), "Email format is incorrect");
    assert!(
        matches!(&malformed, ApiError::Problem(problem) if problem.code() == Code::ValidationFailed)
    );
    assert!(validate_email(&format!("{}@x.io", "a".repeat(60))).is_err());
    assert_eq!(normalize_email(" User@Example.COM "), "user@example.com");
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
    assert!(validate_password(&"x".repeat(129)).is_err());
}

#[test]
fn login_limiter_keys_normalize_and_hash_pii() {
    let first = login_limiter_keys(" User@Example.COM ", Some("203.0.113.7"));
    let second = login_limiter_keys("user@example.com", Some("203.0.113.7"));
    assert_eq!(first, second);
    assert!(first.iter().all(|key| !key.contains("example.com")));
    assert!(first.iter().all(|key| !key.contains("203.0.113.7")));
    assert!(RESERVE_LOGIN_ATTEMPT_SCRIPT.contains("account_count >= tonumber(ARGV[1])"));
    assert!(RESERVE_LOGIN_ATTEMPT_SCRIPT.contains("redis.call('INCR', KEYS[index])"));
    assert!(RELEASE_LOGIN_ATTEMPT_SCRIPT.contains("redis.call('DECR', KEYS[index])"));
}

#[test]
fn validate_forget_mirrors_authforget_rules() {
    assert!(validate_forget("user@example.com", "password", "123456").is_ok());

    // email: required -> format -> max:64 (character count)
    let empty_email = validate_forget("", "password", "123456").unwrap_err();
    assert_eq!(empty_email.to_string(), "Email can not be empty");
    assert!(
        matches!(&empty_email, ApiError::Problem(problem) if problem.code() == Code::ValidationFailed)
    );
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
    assert!(
        matches!(&empty_code, ApiError::Problem(problem) if problem.code() == Code::ValidationFailed)
    );
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
    assert!(
        matches!(&empty_old, ApiError::Problem(problem) if problem.code() == Code::ValidationFailed)
    );

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
    use sha2::Digest as _;
    hasher.update(password.as_bytes());
    let sha256 = hex::encode(hasher.finalize());
    assert!(verify_password(
        Some("sha256"),
        None,
        password,
        &sha256.to_ascii_uppercase()
    ));
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
fn temporary_login_tokens_require_the_user_and_session_epoch() {
    assert_eq!(parse_temp_token("42:7"), Some((42, 7)));
    assert_eq!(parse_temp_token("42"), None);
    assert_eq!(parse_temp_token("bad"), None);
    assert_eq!(parse_temp_token("42:bad"), None);
    assert_eq!(parse_temp_token("42:7:extra"), None);
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
fn auth_response_omits_the_permanent_subscription_credential() {
    // Credential minimization: login/register/token2Login return only the
    // opaque session grant plus the role flag. The long-lived `users.token`
    // subscription credential must never ride on the authentication exchange;
    // clients fetch the subscribe URL separately through /user/getSubscribe.
    let value = serde_json::to_value(AuthData {
        is_admin: true,
        auth_data: "opaque-session-grant".to_string(),
    })
    .unwrap();
    assert_eq!(
        value,
        serde_json::json!({ "is_admin": true, "auth_data": "opaque-session-grant" })
    );
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

    let oversized_map = (0..101)
        .map(|index| (index.to_string(), serde_json::json!({})))
        .collect::<serde_json::Map<_, _>>();
    let encoded = serde_json::to_string(&oversized_map).unwrap();
    assert!(matches!(
        decode_session_metadata(Some(&encoded)),
        Err(ApiError::Internal(_))
    ));
}

#[test]
fn session_metadata_truncation_preserves_utf8_boundaries() {
    assert_eq!(truncate_utf8("short".to_string(), 10), "short");
    assert_eq!(truncate_utf8("ab中文".to_string(), 6), "ab中");
    assert_eq!(truncate_utf8("中文".to_string(), 2), "");
}

#[test]
fn opaque_session_script_sets_absolute_ttl_without_storing_the_bearer() {
    assert!(ADD_OPAQUE_SESSION_SCRIPT.contains("'NX'"));
    assert!(ADD_OPAQUE_SESSION_SCRIPT.contains("'EX', ARGV[4]"));
    assert!(!ADD_OPAQUE_SESSION_SCRIPT.contains("auth_data"));
    assert!(ADD_OPAQUE_SESSION_SCRIPT.contains("while count > tonumber(ARGV[6])"));
    assert!(ADD_OPAQUE_SESSION_SCRIPT.contains("redis.call('SREM', KEYS[3], auth_key)"));
}

#[test]
fn logout_revokes_the_presented_bearer_and_repeats_as_a_no_op() {
    // AuthService::logout resolves the presented bearer to its session and runs
    // the removal script. `user_from_auth_data` authenticates through the
    // AUTH_SESSION_<sha256(bearer)> reverse mapping and the per-user metadata
    // entry; the script must delete both halves (the stored token_hash is the
    // same sha256 the reverse-mapping key embeds), so the bearer is invalid
    // immediately after logout instead of surviving to its TTL.
    assert!(REMOVE_SESSION_SCRIPT.contains("local auth_key = ARGV[2] .. meta['token_hash']"));
    assert!(REMOVE_SESSION_SCRIPT.contains("redis.call('DEL', auth_key)"));
    assert!(REMOVE_SESSION_SCRIPT.contains("redis.call('SREM', KEYS[2], auth_key)"));
    assert!(REMOVE_SESSION_SCRIPT.contains("sessions[ARGV[1]] = nil"));
    // A repeated logout finds no session map (or no entry) and must complete as
    // a no-op instead of erroring, keeping POST /user/logout idempotent. The
    // bearer-to-identity lookup that precedes the script already short-circuits
    // to Ok(false) when the reverse mapping is gone.
    assert!(REMOVE_SESSION_SCRIPT.contains("if not current then\n    return 0\nend"));
}

#[test]
fn registration_limiter_atomically_reserves_and_releases_only_its_own_slot() {
    assert!(RESERVE_REGISTRATION_SLOT_SCRIPT.contains("ZREMRANGEBYSCORE"));
    assert!(RESERVE_REGISTRATION_SLOT_SCRIPT.contains("ZCARD"));
    assert!(RESERVE_REGISTRATION_SLOT_SCRIPT.contains("ZADD"));
    assert!(RESERVE_REGISTRATION_SLOT_SCRIPT.contains("EXPIREAT"));
    assert!(RELEASE_REGISTRATION_SLOT_SCRIPT.contains("ZREM"));
    assert!(!RELEASE_REGISTRATION_SLOT_SCRIPT.contains("DECR"));

    let source = include_str!("registration.rs");
    let commit = source.find("tx.commit().await?").unwrap();
    let auth_data = source.find("self.auth_data_for_user").unwrap();
    let reserve = source.find("reserve_registration_slot").unwrap();
    let release = source.find("release_registration_slot").unwrap();
    assert!(reserve < commit);
    assert!(release < auth_data);
    assert!(source.contains("duration_minutes_to_seconds("));
    assert!(source.contains("self.config.register_limit_expire"));
    assert!(source.contains("REGISTER_IP_RATE_LIMIT_V2"));
}

#[test]
fn invitation_code_consumption_is_locked_and_guarded() {
    let source = include_str!("registration.rs");
    assert!(source.contains("WHERE lower(code) = lower($1)"));
    assert!(source.contains("AND status = 0 LIMIT 1 FOR UPDATE"));
    assert!(source.contains("WHERE id = $2 AND status = 0"));
    assert!(source.contains("result.rows_affected() != 1"));
    assert!(!source.contains("email = $1 LIMIT 1 FOR UPDATE"));
    assert!(source.contains("is_email_unique_violation"));
    let finalize = include_str!("../../../../migrations-postgres/0002_import_finalize.sql");
    assert!(finalize.contains("uniq_invite_code_canonical"));
    assert!(
        source.find("consume_invite_code").unwrap() < source.find("INSERT INTO users").unwrap()
    );
}

#[test]
fn registration_auxiliary_inputs_are_bounded_before_expensive_work() {
    let valid = RegisterInput {
        email: "user@example.com".to_string(),
        password: "password".to_string(),
        invite_code: Some("a".repeat(MAX_INVITE_CODE_BYTES)),
        email_code: Some("123456".to_string()),
        recaptcha_data: Some("r".repeat(MAX_RECAPTCHA_DATA_BYTES)),
    };
    assert!(validate_registration_auxiliary_inputs(&valid).is_ok());

    let mut oversized = valid.clone();
    oversized.invite_code = Some("a".repeat(MAX_INVITE_CODE_BYTES + 1));
    assert!(validate_registration_auxiliary_inputs(&oversized).is_err());
    oversized = valid.clone();
    oversized.email_code = Some("1".repeat(MAX_EMAIL_CODE_BYTES + 1));
    assert!(validate_registration_auxiliary_inputs(&oversized).is_err());
    oversized = valid;
    oversized.recaptcha_data = Some("r".repeat(MAX_RECAPTCHA_DATA_BYTES + 1));
    assert!(validate_registration_auxiliary_inputs(&oversized).is_err());

    let source = include_str!("registration.rs");
    let bounds = source
        .find("validate_registration_auxiliary_inputs(&input)?")
        .unwrap();
    let reserve = source
        .find("self.reserve_registration_slot(ip.as_deref())")
        .unwrap();
    let hash = source.find("self.password_kdf.hash").unwrap();
    assert!(bounds < reserve);
    assert!(bounds < hash);
}

#[test]
fn reset_security_mints_the_returned_url_through_the_method_aware_minter() {
    // /user/resetSecurity rotates the permanent token and hands the caller a
    // subscribe URL. Under show_subscribe_method 1/2 that URL must carry the
    // rotating token (subscribe_link mints it — covered by its own mode
    // tests), never the fresh permanent token via a raw URL render.
    let source = include_str!("credentials.rs");
    let reset = source.find("pub async fn reset_security").unwrap();
    let body = &source[reset..];
    let mint = body
        .find("crate::subscribe_link::subscribe_url_for_user")
        .unwrap();
    assert!(mint < body.find("pub(super) fn login_limiter_keys").unwrap());
    assert!(!source.contains("subscribe_url_for_token"));
    // The rotation is durably persisted before any URL is minted.
    assert!(body.find("db::user::update_security").unwrap() < mint);
}
