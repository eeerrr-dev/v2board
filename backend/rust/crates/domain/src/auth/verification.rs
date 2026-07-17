use chrono::Utc;
use lettre::{AsyncTransport, Message, message::header::ContentType};
use serde::Deserialize;
use uuid::Uuid;
use v2board_compat::ApiError;
use v2board_db as db;

use crate::smtp::SmtpSettings;

use super::validation::validate_email;
use super::{AuthService, cache_key};

#[derive(Clone, Deserialize)]
pub struct EmailVerifyInput {
    pub email: String,
    pub isforget: Option<i32>,
    pub recaptcha_data: Option<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum LimitedEmailCodeResult {
    Consumed,
    Incorrect,
    Limited,
}

impl AuthService {
    pub async fn send_email_verify(
        &self,
        input: EmailVerifyInput,
        ip: Option<String>,
    ) -> Result<bool, ApiError> {
        // FormRequest validates `email => required|email:strict` (422) before the controller body,
        // which then runs: per-IP rate limit (429), recaptcha, whitelist/gmail, isforget, resend.
        validate_email(&input.email)?;
        // Laravel uses the raw, original-case `$email` for the whitelist check, the
        // `User::where('email', ...)` existence probe and the `SendEmailJob` recipient, and
        // only `strtolower(trim($email))` for the EMAIL_VERIFY_CODE / LAST_SEND cache keys.
        // The repository lookup remains case-insensitive like the legacy database collation.
        let email = input.email.trim();
        let cache_email = email.to_ascii_lowercase();

        // Laravel RateLimiter: 3 attempts per IP in a fixed 60s window, `abort(429)` on exceed
        // (CommController:33-36). INCR + expire-on-first mirrors `Cache::add` + `increment`.
        if let Some(ip) = ip.as_deref() {
            let key = self.redis_key(&cache_key("SEND_EMAIL_VERIFY_LIMIT", ip));
            if !self.check_and_increment_limit(&key, 3, 60).await? {
                return Err(ApiError::too_many_requests(
                    "Too many requests, please try again later.",
                ));
            }
        }

        self.verify_recaptcha(input.recaptcha_data.as_deref())
            .await?;
        self.validate_register_email(email).await?;
        let exists = db::user::find_user_for_auth(&self.db, email)
            .await?
            .is_some();
        match input.isforget {
            Some(0) if exists => return Err(ApiError::legacy("This email is registered")),
            Some(1) if !exists => {
                return Err(ApiError::legacy(
                    "This email is not registered in the system",
                ));
            }
            _ => {}
        }
        let last_key = self.redis_key(&cache_key("LAST_SEND_EMAIL_VERIFY_TIMESTAMP", &cache_email));
        let code = six_digit_code();
        let code_key = self.redis_key(&cache_key("EMAIL_VERIFY_CODE", &cache_email));
        if !self.reserve_email_code(&code_key, &last_key, &code).await? {
            return Err(ApiError::legacy(
                "Email verification code has been sent, please request again later",
            ));
        }
        let subject = verify_mail_subject(&self.config.app_name);
        let body = crate::mail::render_verify(
            &self.config.app_name,
            self.config.app_url.as_deref().unwrap_or_default(),
            &code,
        );
        if let Err(error) = self.send_mail(email, &subject, &body).await {
            self.release_email_code_reservation(&code_key, &last_key, &code)
                .await;
            return Err(error);
        }
        Ok(true)
    }

    pub(super) async fn validate_register_email(&self, email: &str) -> Result<(), ApiError> {
        if self.config.email_whitelist_enable {
            let email = email.to_ascii_lowercase();
            let allowed = self.config.email_whitelist_suffix.iter().any(|suffix| {
                let suffix = suffix.trim().trim_start_matches('@').to_ascii_lowercase();
                !suffix.is_empty() && email.ends_with(&format!("@{suffix}"))
            });
            if !allowed {
                return Err(ApiError::legacy("Email suffix is not in the Whitelist"));
            }
        }
        if self.config.email_gmail_limit_enable
            && let Some(prefix) = email.split('@').next()
            && (prefix.contains('.') || prefix.contains('+'))
        {
            return Err(ApiError::legacy("Gmail alias is not supported"));
        }
        Ok(())
    }

    pub(super) async fn consume_email_code(
        &self,
        email: &str,
        code: Option<&str>,
    ) -> Result<bool, ApiError> {
        let Some(code) = code
            .map(str::trim)
            .filter(|value| value.len() == 6 && value.chars().all(|ch| ch.is_ascii_digit()))
        else {
            return Ok(false);
        };
        let mut conn = self.redis.clone();
        let consumed = redis::Script::new(CONSUME_VALUE_SCRIPT)
            .key(self.redis_key(&cache_key("EMAIL_VERIFY_CODE", email)))
            .arg(code)
            .invoke_async::<i64>(&mut conn)
            .await?;
        Ok(consumed == 1)
    }

    pub(super) async fn consume_email_code_with_failure_limit(
        &self,
        email: &str,
        code: &str,
        limit_key: &str,
        limit: i64,
        ttl_seconds: u64,
    ) -> Result<LimitedEmailCodeResult, ApiError> {
        let mut conn = self.redis.clone();
        let result = redis::Script::new(CONSUME_VALUE_WITH_FAILURE_LIMIT_SCRIPT)
            .key(self.redis_key(&cache_key("EMAIL_VERIFY_CODE", email)))
            .key(limit_key)
            .arg(code)
            .arg(limit.max(1))
            .arg(ttl_seconds.max(1))
            .invoke_async::<i64>(&mut conn)
            .await?;
        Ok(match result {
            1 => LimitedEmailCodeResult::Consumed,
            -1 => LimitedEmailCodeResult::Limited,
            _ => LimitedEmailCodeResult::Incorrect,
        })
    }

    async fn check_and_increment_limit(
        &self,
        key: &str,
        limit: i64,
        ttl_seconds: i64,
    ) -> Result<bool, ApiError> {
        let mut conn = self.redis.clone();
        let result = redis::Script::new(CHECK_AND_INCREMENT_LIMIT_SCRIPT)
            .key(key)
            .arg(limit)
            .arg(ttl_seconds.max(1))
            .invoke_async::<i64>(&mut conn)
            .await?;
        Ok(result == 1)
    }

    async fn reserve_email_code(
        &self,
        code_key: &str,
        last_send_key: &str,
        code: &str,
    ) -> Result<bool, ApiError> {
        let mut conn = self.redis.clone();
        let reserved = redis::Script::new(RESERVE_EMAIL_CODE_SCRIPT)
            .key(code_key)
            .key(last_send_key)
            .arg(code)
            .arg(Utc::now().timestamp())
            .invoke_async::<i64>(&mut conn)
            .await?;
        Ok(reserved == 1)
    }

    async fn release_email_code_reservation(
        &self,
        code_key: &str,
        last_send_key: &str,
        code: &str,
    ) {
        let mut conn = self.redis.clone();
        if let Err(error) = redis::Script::new(RELEASE_EMAIL_CODE_SCRIPT)
            .key(code_key)
            .key(last_send_key)
            .arg(code)
            .invoke_async::<i64>(&mut conn)
            .await
        {
            tracing::warn!(?error, "failed to release email verification reservation");
        }
    }

    pub(super) async fn verify_recaptcha(&self, token: Option<&str>) -> Result<(), ApiError> {
        if token.is_some_and(|value| value.len() > 4096) {
            return Err(ApiError::legacy("Invalid code is incorrect"));
        }
        if !self.config.recaptcha_enable {
            return Ok(());
        }
        let secret = self
            .config
            .recaptcha_key
            .as_deref()
            .filter(|value| !value.is_empty())
            .ok_or_else(|| ApiError::legacy("Invalid code is incorrect"))?;
        let response = token
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| ApiError::legacy("Invalid code is incorrect"))?;
        let request_body =
            serde_urlencoded::to_string([("secret", secret), ("response", response)])
                .map_err(|_| ApiError::legacy("Invalid code is incorrect"))?;
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
            .map_err(|_| ApiError::legacy("Invalid code is incorrect"))?;
        let body: serde_json::Value = crate::http_response::bounded_json(
            response,
            crate::http_response::MAX_EXTERNAL_RESPONSE_BYTES,
            "Invalid code is incorrect",
        )
        .await?;
        if body
            .get("success")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false)
        {
            Ok(())
        } else {
            Err(ApiError::legacy("Invalid code is incorrect"))
        }
    }

    async fn send_mail(&self, to: &str, subject: &str, body: &str) -> Result<(), ApiError> {
        let settings = SmtpSettings::load(&self.config)?;
        let from = settings
            .from_address
            .clone()
            .or_else(|| settings.username.clone())
            .ok_or_else(|| ApiError::legacy("Email sender is not configured"))?;
        let email = Message::builder()
            .from(
                from.parse()
                    .map_err(|_| ApiError::legacy("Invalid email sender"))?,
            )
            .to(to
                .parse()
                .map_err(|_| ApiError::legacy("Invalid recipient email"))?)
            .subject(subject)
            .header(ContentType::TEXT_HTML)
            .body(body.to_string())
            .map_err(|error| ApiError::legacy(format!("Build mail failed: {error}")))?;
        let transport = self.smtp.transport(&settings)?;
        let delivery = transport.send(email);
        tokio::time::timeout(
            std::time::Duration::from_secs(self.config.http_request_timeout_seconds),
            delivery,
        )
        .await
        .map_err(|_| ApiError::legacy("Send mail timed out"))?
        .map_err(|error| ApiError::legacy(format!("Send mail failed: {error}")))?;
        Ok(())
    }
}

/// Legacy subject: `config('v2board.app_name', 'V2Board') . __('Email verification code')`
/// (CommController.php:78). Laravel pins the default (and fallback) locale to zh-CN, so
/// `__()` resolves to "邮箱验证码" and the delivered subject is this literal concatenation —
/// the same hardcoded language as the zh-CN body template in [`crate::mail::render_verify`].
pub(super) fn verify_mail_subject(app_name: &str) -> String {
    format!("{app_name}邮箱验证码")
}

fn six_digit_code() -> String {
    let bytes = *Uuid::new_v4().as_bytes();
    let number = u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]) % 900_000 + 100_000;
    number.to_string()
}

const CHECK_AND_INCREMENT_LIMIT_SCRIPT: &str = r#"
local current = tonumber(redis.call('GET', KEYS[1]) or '0')
if current >= tonumber(ARGV[1]) then
    return 0
end
local value = redis.call('INCR', KEYS[1])
if value == 1 or redis.call('TTL', KEYS[1]) < 0 then
    redis.call('EXPIRE', KEYS[1], ARGV[2])
end
return 1
"#;

pub(super) const CONSUME_VALUE_WITH_FAILURE_LIMIT_SCRIPT: &str = r#"
local current = tonumber(redis.call('GET', KEYS[2]) or '0')
if current >= tonumber(ARGV[2]) then
    return -1
end
if redis.call('GET', KEYS[1]) == ARGV[1] then
    redis.call('DEL', KEYS[1])
    return 1
end
local value = redis.call('INCR', KEYS[2])
if value == 1 or redis.call('TTL', KEYS[2]) < 0 then
    redis.call('EXPIRE', KEYS[2], ARGV[3])
end
return 0
"#;

const CONSUME_VALUE_SCRIPT: &str = r#"
if redis.call('GET', KEYS[1]) == ARGV[1] then
    redis.call('DEL', KEYS[1])
    return 1
end
return 0
"#;

const RESERVE_EMAIL_CODE_SCRIPT: &str = r#"
if redis.call('EXISTS', KEYS[2]) == 1 then
    return 0
end
redis.call('SET', KEYS[1], ARGV[1], 'EX', 300)
redis.call('SET', KEYS[2], ARGV[2], 'EX', 60)
return 1
"#;

const RELEASE_EMAIL_CODE_SCRIPT: &str = r#"
if redis.call('GET', KEYS[1]) == ARGV[1] then
    redis.call('DEL', KEYS[1])
    redis.call('DEL', KEYS[2])
    return 1
end
return 0
"#;
