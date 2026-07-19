mod credentials;
mod mfa;
mod password;
mod registration;
mod sessions;
mod validation;
mod verification;

pub use credentials::ForgetInput;
pub use mfa::{MfaStatus, TotpProvisioning};
pub use password::{PasswordKdf, hash_password, verify_password};
pub use registration::RegisterInput;
pub use sessions::{
    AuthData, AuthUser, SessionMeta, UserSession, remove_user_sessions_from_client,
};
pub use verification::EmailVerifyInput;

use std::sync::Arc;

use redis::aio::ConnectionManager;
use uuid::Uuid;
use v2board_config::{AppConfig, RedisKeyspace};
use v2board_db::DbPool;

use crate::smtp::SmtpTransportCache;

#[derive(Clone)]
pub struct AuthService {
    db: DbPool,
    redis: ConnectionManager,
    redis_keys: RedisKeyspace,
    config: Arc<AppConfig>,
    http: reqwest::Client,
    password_kdf: PasswordKdf,
    smtp: SmtpTransportCache,
}

impl AuthService {
    pub fn new(
        db: DbPool,
        redis: ConnectionManager,
        installation_id: Uuid,
        config: Arc<AppConfig>,
        http: reqwest::Client,
        password_kdf: PasswordKdf,
        smtp: SmtpTransportCache,
    ) -> Self {
        Self {
            db,
            redis,
            redis_keys: RedisKeyspace::new(installation_id),
            config,
            http,
            password_kdf,
            smtp,
        }
    }

    fn redis_key(&self, logical_key: &str) -> String {
        self.redis_keys.key(logical_key)
    }
}

fn legacy_guid(format: bool) -> String {
    let uuid = Uuid::new_v4();
    if format {
        return uuid.hyphenated().to_string();
    }
    uuid.simple().to_string()
}

fn cache_key(key: &str, unique: &str) -> String {
    format!("{key}_{unique}")
}

#[cfg(test)]
mod tests;
