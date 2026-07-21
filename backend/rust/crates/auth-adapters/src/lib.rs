//! Production outer adapters for the infrastructure-free authentication use cases.

mod cache;
mod external;
mod operator_access;
mod password;
mod session_cleanup;

use std::sync::Arc;

pub use cache::RedisAuthCache;
pub use external::RuntimeAuthExternal;
pub use operator_access::RuntimeOperatorAccessExternal;
pub use password::{PasswordKdf, hash_password, password_needs_rehash, verify_password};
use redis::aio::ConnectionManager;
pub use session_cleanup::remove_user_sessions_from_client;
use uuid::Uuid;
use v2board_application::auth::{AuthPolicy, AuthService};
use v2board_application::operator_access::OperatorAccessService;
use v2board_config::AppConfig;
use v2board_db::operator_access::PostgresOperatorAccessRepository;
use v2board_db::{DbPool, auth::PostgresAuthRepository};
use v2board_mail_adapters::smtp::SmtpTransportCache;

pub type RuntimeAuthService =
    AuthService<PostgresAuthRepository, RedisAuthCache, RuntimeAuthExternal>;

pub type RuntimeOperatorAccessService =
    OperatorAccessService<PostgresOperatorAccessRepository, RuntimeOperatorAccessExternal>;

pub fn runtime_operator_access_service(
    db: DbPool,
    password_kdf: PasswordKdf,
    redis_url: String,
    installation_id: Uuid,
) -> RuntimeOperatorAccessService {
    OperatorAccessService::new(
        PostgresOperatorAccessRepository::new(db),
        RuntimeOperatorAccessExternal::new(password_kdf, redis_url, installation_id),
    )
}

pub fn runtime_auth_service(
    db: DbPool,
    redis: ConnectionManager,
    installation_id: Uuid,
    config: Arc<AppConfig>,
    http: reqwest::Client,
    password_kdf: PasswordKdf,
    smtp: SmtpTransportCache,
) -> RuntimeAuthService {
    let policy: AuthPolicy = external::policy_from_config(&config);
    AuthService::new(
        PostgresAuthRepository::new(db),
        RedisAuthCache::new(redis.clone(), installation_id),
        RuntimeAuthExternal::new(redis, installation_id, config, http, password_kdf, smtp),
        policy,
    )
}
