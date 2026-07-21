use chrono::Utc;
use uuid::Uuid;
use v2board_application::{
    RepositoryError,
    operator_access::{OperatorAccessExternal, RepositoryResult},
};
use v2board_config::RedisKeyspace;

use crate::{PasswordKdf, remove_user_sessions_from_client};

#[derive(Clone)]
pub struct RuntimeOperatorAccessExternal {
    password_kdf: PasswordKdf,
    redis_url: String,
    redis_keys: RedisKeyspace,
}

impl RuntimeOperatorAccessExternal {
    pub fn new(password_kdf: PasswordKdf, redis_url: String, installation_id: Uuid) -> Self {
        Self {
            password_kdf,
            redis_url,
            redis_keys: RedisKeyspace::new(installation_id),
        }
    }
}

fn repository_error(operation: &'static str, error: impl std::fmt::Display) -> RepositoryError {
    RepositoryError::new(operation, error)
}

impl OperatorAccessExternal for RuntimeOperatorAccessExternal {
    fn now(&self) -> i64 {
        Utc::now().timestamp()
    }

    async fn hash_password(&self, password: &str) -> RepositoryResult<String> {
        self.password_kdf
            .hash(password)
            .await
            .map_err(|error| repository_error("hash operator administrator password", error))
    }

    async fn revoke_sessions(&self, user_id: i64) -> RepositoryResult<()> {
        let redis = redis::Client::open(self.redis_url.clone())
            .map_err(|error| repository_error("open operator session Redis", error))?;
        remove_user_sessions_from_client(&redis, &self.redis_keys, user_id)
            .await
            .map_err(|error| repository_error("revoke operator administrator sessions", error))
    }
}
