mod credentials;
mod password;
mod registration;
mod sessions;
mod validation;
mod verification;

pub use credentials::ForgetInput;
pub use password::{PasswordKdf, hash_password, verify_password};
pub use registration::RegisterInput;
pub use sessions::{AuthData, AuthUser, SessionMeta, remove_user_sessions_from_client};
pub use verification::EmailVerifyInput;

use std::sync::Arc;

use redis::aio::ConnectionManager;
use uuid::Uuid;
use v2board_config::AppConfig;
use v2board_db::DbPool;

use crate::smtp::SmtpTransportCache;

#[derive(Clone)]
pub struct AuthService {
    db: DbPool,
    redis: ConnectionManager,
    config: Arc<AppConfig>,
    http: reqwest::Client,
    password_kdf: PasswordKdf,
    smtp: SmtpTransportCache,
}

impl AuthService {
    pub fn new(
        db: DbPool,
        redis: ConnectionManager,
        config: Arc<AppConfig>,
        http: reqwest::Client,
        password_kdf: PasswordKdf,
        smtp: SmtpTransportCache,
    ) -> Self {
        Self {
            db,
            redis,
            config,
            http,
            password_kdf,
            smtp,
        }
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
