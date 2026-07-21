use redis::AsyncCommands;
use v2board_application::{
    RepositoryError,
    service_usage::{RepositoryResult, ServerPresence, ServerPresenceKey},
};
use v2board_config::RedisKeyspace;

const REDIS_MGET_BATCH_SIZE: usize = 500;

#[derive(Clone)]
pub(crate) struct RedisServerPresence {
    redis: redis::Client,
    keys: RedisKeyspace,
}

impl RedisServerPresence {
    pub(crate) fn new(redis: redis::Client, keys: RedisKeyspace) -> Self {
        Self { redis, keys }
    }
}

impl ServerPresence for RedisServerPresence {
    async fn last_checks(
        &self,
        servers: &[ServerPresenceKey],
    ) -> RepositoryResult<Vec<Option<i64>>> {
        if servers.is_empty() {
            return Ok(Vec::new());
        }
        let mut connection = self
            .redis
            .get_multiplexed_async_connection()
            .await
            .map_err(|error| RepositoryError::new("connect server presence cache", error))?;
        let mut output = Vec::with_capacity(servers.len());
        for batch in servers.chunks(REDIS_MGET_BATCH_SIZE) {
            let keys = batch
                .iter()
                .map(|server| {
                    self.keys.key(&format!(
                        "SERVER_{}_LAST_CHECK_AT_{}",
                        server.kind.to_ascii_uppercase(),
                        server.check_id
                    ))
                })
                .collect::<Vec<_>>();
            let mut values = connection
                .mget::<_, Vec<Option<i64>>>(&keys)
                .await
                .map_err(|error| RepositoryError::new("load server presence cache", error))?;
            output.append(&mut values);
        }
        Ok(output)
    }
}
