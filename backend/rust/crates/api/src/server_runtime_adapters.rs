use redis::AsyncCommands;
use v2board_application::{
    RepositoryError,
    server_runtime::{AliveUpdate, RepositoryResult, ServerMetric, ServerRuntimeCache},
};
use v2board_config::RedisKeyspace;
use v2board_domain_model::ServerKind;

const MGET_BATCH_SIZE: usize = 500;
const ALIVE_SCRIPT_BATCH_SIZE: usize = 64;

pub(crate) const ALIVE_CACHE_UPDATE_SCRIPT: &str = r#"
local node_bucket = ARGV[1]
local now = tonumber(ARGV[2])
local device_limit_mode = tonumber(ARGV[3])

if #KEYS == 0 or #KEYS > 64 or #ARGV ~= #KEYS + 3 then
    return redis.error_reply('alive-IP batch exceeds its fixed bounds')
end

local prepared = {}
for index, key in ipairs(KEYS) do
    local value = {}
    local current = redis.call('GET', key)
    if current then
        local ok, decoded = pcall(cjson.decode, current)
        if ok and type(decoded) == 'table' then
            value = decoded
        end
    end

    local ok, aliveips = pcall(cjson.decode, ARGV[index + 3])
    if not ok or type(aliveips) ~= 'table' then
        return redis.error_reply('invalid alive-IP payload')
    end
    if #aliveips > 256 then
        return redis.error_reply('alive-IP user payload exceeds its fixed bound')
    end
    value[node_bucket] = { aliveips = aliveips, lastupdateAt = now }

    local stale = {}
    for bucket, node in pairs(value) do
        if bucket ~= 'alive_ip' then
            local last_update = 0
            if type(node) == 'table' then
                last_update = tonumber(node.lastupdateAt) or 0
            end
            if now - last_update > 100 then
                table.insert(stale, bucket)
            end
        end
    end
    for _, bucket in ipairs(stale) do
        value[bucket] = nil
    end

    local bucket_count = 0
    for bucket in pairs(value) do
        if bucket ~= 'alive_ip' then
            bucket_count = bucket_count + 1
        end
    end
    if bucket_count > 32 then
        return redis.error_reply('alive-IP cache exceeds its active-node bound')
    end

    local alive_count = 0
    if device_limit_mode == 1 then
        local unique = {}
        for bucket, node in pairs(value) do
            if bucket ~= 'alive_ip' and type(node) == 'table' and type(node.aliveips) == 'table' then
                for _, ip_node in ipairs(node.aliveips) do
                    if type(ip_node) == 'string' then
                        local separator = string.find(ip_node, '_', 1, true)
                        local ip = separator and string.sub(ip_node, 1, separator - 1) or ip_node
                        unique[ip] = true
                    end
                end
            end
        end
        for _ in pairs(unique) do
            alive_count = alive_count + 1
        end
    else
        for bucket, node in pairs(value) do
            if bucket ~= 'alive_ip' and type(node) == 'table' and type(node.aliveips) == 'table' then
                alive_count = alive_count + #node.aliveips
            end
        end
    end

    value.alive_ip = alive_count
    prepared[index] = cjson.encode(value)
end

for index, key in ipairs(KEYS) do
    redis.call('SET', key, prepared[index], 'EX', 120)
end

return #KEYS
"#;

#[derive(Clone)]
pub(crate) struct RedisServerRuntimeCache {
    redis: redis::Client,
    keys: RedisKeyspace,
}

impl RedisServerRuntimeCache {
    pub(crate) fn new(redis: redis::Client, keys: RedisKeyspace) -> Self {
        Self { redis, keys }
    }

    fn key(&self, logical: &str) -> String {
        self.keys.key(logical)
    }
}

impl ServerRuntimeCache for RedisServerRuntimeCache {
    async fn write_metric(
        &self,
        kind: ServerKind,
        node_id: i32,
        metric: ServerMetric,
        value: i64,
    ) -> RepositoryResult<()> {
        let key = self.key(&format!(
            "SERVER_{}_{}_{node_id}",
            kind.as_str().to_ascii_uppercase(),
            metric.key_suffix(),
        ));
        let mut connection = self
            .redis
            .get_multiplexed_async_connection()
            .await
            .map_err(|error| RepositoryError::new("connect server runtime cache", error))?;
        connection
            .set_ex::<_, _, ()>(key, value, 3_600)
            .await
            .map_err(|error| RepositoryError::new("write server runtime metric", error))
    }

    async fn alive_counts(&self, user_ids: &[i64]) -> RepositoryResult<Vec<Option<i64>>> {
        let mut connection = self
            .redis
            .get_multiplexed_async_connection()
            .await
            .map_err(|error| RepositoryError::new("connect alive-list cache", error))?;
        let mut output = Vec::with_capacity(user_ids.len());
        for batch in user_ids.chunks(MGET_BATCH_SIZE) {
            let keys = batch
                .iter()
                .map(|user_id| self.key(&format!("ALIVE_IP_USER_{user_id}")))
                .collect::<Vec<_>>();
            let values = connection
                .mget::<_, Vec<Option<String>>>(&keys)
                .await
                .map_err(|error| RepositoryError::new("load alive-list cache", error))?;
            output.extend(values.into_iter().map(|value| {
                value
                    .and_then(|value| serde_json::from_str::<serde_json::Value>(&value).ok())
                    .and_then(|value| value.get("alive_ip").and_then(value_to_i64))
            }));
        }
        Ok(output)
    }

    async fn merge_alive(
        &self,
        node_bucket: &str,
        now: i64,
        device_limit_mode: i32,
        updates: &[AliveUpdate],
    ) -> RepositoryResult<()> {
        if updates.is_empty() {
            return Ok(());
        }
        let mut connection = self
            .redis
            .get_multiplexed_async_connection()
            .await
            .map_err(|error| RepositoryError::new("connect alive-update cache", error))?;
        let script = redis::Script::new(ALIVE_CACHE_UPDATE_SCRIPT);
        for batch in updates.chunks(ALIVE_SCRIPT_BATCH_SIZE) {
            let mut invocation = script.prepare_invoke();
            for update in batch {
                invocation.key(self.key(&format!("ALIVE_IP_USER_{}", update.user_id)));
            }
            invocation.arg(node_bucket).arg(now).arg(device_limit_mode);
            for update in batch {
                invocation.arg(&update.ips_json);
            }
            let updated = invocation
                .invoke_async::<i64>(&mut connection)
                .await
                .map_err(|error| RepositoryError::new("merge alive-IP cache", error))?;
            let expected = i64::try_from(batch.len())
                .map_err(|error| RepositoryError::new("validate alive-IP cache result", error))?;
            if updated != expected {
                return Err(RepositoryError::new(
                    "validate alive-IP cache result",
                    "cache returned an unexpected update count",
                ));
            }
        }
        Ok(())
    }
}

fn value_to_i64(value: &serde_json::Value) -> Option<i64> {
    match value {
        serde_json::Value::Number(value) => value.as_i64(),
        serde_json::Value::String(value) => value.parse().ok(),
        serde_json::Value::Bool(value) => Some(i64::from(*value)),
        serde_json::Value::Array(values) => values.first().and_then(value_to_i64),
        serde_json::Value::Null | serde_json::Value::Object(_) => None,
    }
}
