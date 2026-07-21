use std::sync::Arc;

use chrono::TimeZone as _;
use redis::AsyncCommands as _;
use serde_json::Value;
use tokio::{sync::Mutex, task::JoinSet};
use uuid::Uuid;
use v2board_application::admin_user::{
    AccountCredential, AdminUserExternal, AdminUserExternalError, AdminUserListItem,
    PreparedAccount, UserSecret,
};
use v2board_auth_adapters::{PasswordKdf, remove_user_sessions_from_client};
use v2board_config::{AppConfig, RedisKeyspace, app_timezone};

const REDIS_MGET_BATCH_SIZE: usize = 500;
const SESSION_CLEANUP_CONCURRENCY: usize = 8;

#[derive(Clone)]
pub(crate) struct RuntimeAdminUserExternal {
    redis: redis::Client,
    redis_keys: RedisKeyspace,
    config: Arc<AppConfig>,
    password_kdf: PasswordKdf,
    mint_connection: Arc<Mutex<Option<redis::aio::MultiplexedConnection>>>,
}

impl RuntimeAdminUserExternal {
    pub(crate) fn new(
        redis: redis::Client,
        redis_keys: RedisKeyspace,
        config: Arc<AppConfig>,
        password_kdf: PasswordKdf,
    ) -> Self {
        Self {
            redis,
            redis_keys,
            config,
            password_kdf,
            mint_connection: Arc::new(Mutex::new(None)),
        }
    }

    async fn ensure_mint_connection(
        &self,
        connection: &mut Option<redis::aio::MultiplexedConnection>,
    ) -> Result<(), AdminUserExternalError> {
        if self.config.show_subscribe_method == 1 && connection.is_none() {
            *connection = Some(
                self.redis
                    .get_multiplexed_async_connection()
                    .await
                    .map_err(AdminUserExternalError::new)?,
            );
        }
        Ok(())
    }
}

pub(crate) struct AdminUserCsvWriter {
    writer: csv::Writer<Vec<u8>>,
    include_utf8_bom: bool,
}

impl AdminUserExternal for RuntimeAdminUserExternal {
    type CsvWriter = AdminUserCsvWriter;

    async fn hash_password(&self, password: &str) -> Result<String, AdminUserExternalError> {
        self.password_kdf
            .hash(password)
            .await
            .map_err(AdminUserExternalError::new)
    }

    async fn prepare_accounts(
        &self,
        credentials: Vec<AccountCredential>,
    ) -> Result<Vec<PreparedAccount>, AdminUserExternalError> {
        let mut tasks = JoinSet::new();
        for (index, credential) in credentials.into_iter().enumerate() {
            let password_kdf = self.password_kdf.clone();
            tasks.spawn(async move {
                let password_hash = password_kdf.hash(&credential.password).await?;
                Ok::<_, v2board_compat::ApiError>((
                    index,
                    PreparedAccount {
                        email: credential.email,
                        password: credential.password,
                        password_hash,
                        uuid: Uuid::new_v4().to_string(),
                        token: Uuid::new_v4().simple().to_string(),
                    },
                ))
            });
        }
        let mut prepared = Vec::new();
        while let Some(result) = tasks.join_next().await {
            prepared.push(
                result
                    .map_err(AdminUserExternalError::new)?
                    .map_err(AdminUserExternalError::new)?,
            );
        }
        prepared.sort_unstable_by_key(|(index, _)| *index);
        Ok(prepared.into_iter().map(|(_, account)| account).collect())
    }

    async fn enrich_users(
        &self,
        users: &mut [AdminUserListItem],
    ) -> Result<(), AdminUserExternalError> {
        if users.is_empty() {
            return Ok(());
        }
        for user in users.iter_mut() {
            user.subscribe_url = self.subscribe_url(user.user.id, &user.user.token).await?;
        }

        let mut connection = match self.redis.get_multiplexed_async_connection().await {
            Ok(connection) => connection,
            Err(error) => {
                tracing::warn!(?error, "admin user device-cache connection unavailable");
                return Ok(());
            }
        };
        for chunk in users.chunks_mut(REDIS_MGET_BATCH_SIZE) {
            let keys = chunk
                .iter()
                .map(|user| {
                    self.redis_keys
                        .key(&format!("ALIVE_IP_USER_{}", user.user.id))
                })
                .collect::<Vec<_>>();
            let cached = match connection.mget::<_, Vec<Option<String>>>(&keys).await {
                Ok(cached) => cached,
                Err(error) => {
                    tracing::warn!(?error, "admin user device-cache batch read failed");
                    return Ok(());
                }
            };
            for (user, raw) in chunk.iter_mut().zip(cached) {
                if let Some(raw) = raw {
                    (user.alive_ip, user.ips) = parse_alive_ip(&raw);
                }
            }
        }
        Ok(())
    }

    async fn subscribe_url(
        &self,
        user_id: i64,
        token: &str,
    ) -> Result<String, AdminUserExternalError> {
        let mut connection = self.mint_connection.lock().await;
        self.ensure_mint_connection(&mut connection).await?;
        v2board_subscription_adapters::subscribe_url_for_user(
            &self.config,
            &self.redis_keys,
            &mut *connection,
            user_id,
            token,
        )
        .await
        .map_err(AdminUserExternalError::new)
    }

    async fn remove_sessions(&self, user_ids: &[i64]) {
        for chunk in user_ids.chunks(SESSION_CLEANUP_CONCURRENCY) {
            let mut tasks = JoinSet::new();
            for user_id in chunk {
                let redis = self.redis.clone();
                let redis_keys = self.redis_keys.clone();
                let user_id = *user_id;
                tasks.spawn(async move {
                    let result =
                        remove_user_sessions_from_client(&redis, &redis_keys, user_id).await;
                    (user_id, result)
                });
            }
            while let Some(result) = tasks.join_next().await {
                match result {
                    Ok((_user_id, Ok(()))) => {}
                    Ok((user_id, Err(error))) => tracing::warn!(
                        ?error,
                        user_id,
                        "admin session cache cleanup failed after durable user mutation"
                    ),
                    Err(error) => tracing::warn!(
                        ?error,
                        "admin session cache cleanup task failed after durable user mutation"
                    ),
                }
            }
        }
    }

    fn random_email(&self, suffix: &str) -> String {
        format!("{}@{suffix}", random_char(6))
    }

    fn new_secret(&self) -> UserSecret {
        UserSecret {
            token: Uuid::new_v4().simple().to_string(),
            uuid: Uuid::new_v4().to_string(),
        }
    }

    fn local_datetime(&self, epoch_seconds: i64) -> String {
        app_timezone()
            .timestamp_opt(epoch_seconds, 0)
            .single()
            .map(|value| value.format("%Y-%m-%d %H:%M:%S").to_string())
            .unwrap_or_default()
    }

    fn start_csv(
        &self,
        headers: &[&str],
        include_utf8_bom: bool,
    ) -> Result<Self::CsvWriter, AdminUserExternalError> {
        let mut writer = csv::WriterBuilder::new()
            .has_headers(false)
            .terminator(csv::Terminator::CRLF)
            .from_writer(Vec::new());
        writer
            .write_record(headers)
            .map_err(AdminUserExternalError::new)?;
        Ok(AdminUserCsvWriter {
            writer,
            include_utf8_bom,
        })
    }

    fn write_csv(
        &self,
        writer: &mut Self::CsvWriter,
        row: Vec<String>,
    ) -> Result<(), AdminUserExternalError> {
        writer
            .writer
            .write_record(row.into_iter().map(|value| neutralize_formula(&value)))
            .map_err(AdminUserExternalError::new)
    }

    fn finish_csv(&self, writer: Self::CsvWriter) -> Result<String, AdminUserExternalError> {
        let bytes = writer
            .writer
            .into_inner()
            .map_err(AdminUserExternalError::new)?;
        let body = String::from_utf8(bytes).map_err(AdminUserExternalError::new)?;
        if writer.include_utf8_bom {
            Ok(format!("\u{feff}{body}"))
        } else {
            Ok(body)
        }
    }
}

fn random_char(length: usize) -> String {
    const CHARACTERS: &[u8] = b"abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789";
    let mut bytes = Vec::with_capacity(length);
    while bytes.len() < length {
        bytes.extend_from_slice(Uuid::new_v4().as_bytes());
    }
    (0..length)
        .map(|index| char::from(CHARACTERS[usize::from(bytes[index]) % CHARACTERS.len()]))
        .collect()
}

fn parse_alive_ip(raw: &str) -> (i64, String) {
    let Ok(Value::Object(object)) = serde_json::from_str::<Value>(raw) else {
        return (0, String::new());
    };
    let alive_ip = object
        .get("alive_ip")
        .and_then(Value::as_i64)
        .unwrap_or_default();
    let mut ips = Vec::new();
    for (node_type_id, data) in object {
        if node_type_id == "alive_ip" {
            continue;
        }
        let Some(entries) = data.get("aliveips").and_then(Value::as_array) else {
            continue;
        };
        for entry in entries.iter().filter_map(Value::as_str) {
            ips.push(format!(
                "{}_{}",
                entry.split('_').next().unwrap_or_default(),
                node_type_id
            ));
        }
    }
    (alive_ip, ips.join(", "))
}

fn neutralize_formula(value: &str) -> String {
    if matches!(
        value.trim_start().as_bytes().first(),
        Some(b'=' | b'+' | b'-' | b'@' | b'\t' | b'\r')
    ) {
        format!("'{value}")
    } else {
        value.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cache_projection_and_csv_formula_hardening_are_adapter_owned() {
        assert_eq!(
            parse_alive_ip(r#"{"alive_ip":2,"vmess_7":{"aliveips":["1.2.3.4_9"]}}"#),
            (2, "1.2.3.4_vmess_7".to_string())
        );
        assert_eq!(neutralize_formula(" =cmd"), "' =cmd");
        assert_eq!(neutralize_formula("ordinary"), "ordinary");
    }
}
