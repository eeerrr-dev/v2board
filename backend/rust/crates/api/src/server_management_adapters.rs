use std::collections::BTreeMap;

use base64::{Engine as _, engine::general_purpose};
use openssl::pkey::PKey;
use redis::AsyncCommands;
use sha1::{Digest, Sha1};
use uuid::Uuid;
use v2board_application::server_management::{
    ServerCredentialError, ServerCredentialProvisioner, ServerHealth, ServerPresence,
    ServerPresenceKey, ServerSettingValue,
};
use v2board_config::RedisKeyspace;
use v2board_domain_model::ServerKind;
use v2board_server_adapters::derive_node_token;

const REDIS_MGET_BATCH_SIZE: usize = 500;

#[derive(Clone)]
pub(crate) struct RedisAdminServerPresence {
    redis: redis::Client,
    keys: RedisKeyspace,
}

impl RedisAdminServerPresence {
    pub(crate) fn new(redis: redis::Client, keys: RedisKeyspace) -> Self {
        Self { redis, keys }
    }
}

impl ServerPresence for RedisAdminServerPresence {
    async fn health(&self, servers: &[ServerPresenceKey]) -> Vec<ServerHealth> {
        let mut output = vec![ServerHealth::default(); servers.len()];
        if servers.is_empty() {
            return output;
        }
        let keys = servers
            .iter()
            .flat_map(|server| {
                let prefix = server.kind.as_str().to_ascii_uppercase();
                [
                    self.keys
                        .key(&format!("SERVER_{prefix}_ONLINE_USER_{}", server.node_id)),
                    self.keys
                        .key(&format!("SERVER_{prefix}_LAST_CHECK_AT_{}", server.node_id)),
                    self.keys
                        .key(&format!("SERVER_{prefix}_LAST_PUSH_AT_{}", server.node_id)),
                ]
            })
            .collect::<Vec<_>>();
        let Ok(mut connection) = self.redis.get_multiplexed_async_connection().await else {
            tracing::warn!("admin server presence cache is unavailable");
            return output;
        };
        let mut flat = vec![None; keys.len()];
        for (batch_index, batch) in keys.chunks(REDIS_MGET_BATCH_SIZE).enumerate() {
            match connection.mget::<_, Vec<Option<String>>>(batch).await {
                Ok(values) => {
                    let offset = batch_index * REDIS_MGET_BATCH_SIZE;
                    for (index, value) in values.into_iter().enumerate() {
                        flat[offset + index] = value.and_then(|value| value.parse().ok());
                    }
                }
                Err(error) => {
                    tracing::warn!(?error, "admin server presence batch read failed");
                    break;
                }
            }
        }
        for (health, values) in output.iter_mut().zip(flat.chunks_exact(3)) {
            health.online = values[0];
            health.last_check_at = values[1];
            health.last_push_at = values[2];
        }
        output
    }
}

#[derive(Clone)]
pub(crate) struct OpenSslServerCredentials {
    master_key: String,
    install_api_host: String,
}

impl OpenSslServerCredentials {
    pub(crate) fn new(master_key: String, install_api_host: String) -> Self {
        Self {
            master_key,
            install_api_host,
        }
    }
}

impl ServerCredentialProvisioner for OpenSslServerCredentials {
    fn prepare_tls_settings(
        &self,
        input: Option<&ServerSettingValue>,
        tls: i64,
        v2node: bool,
    ) -> Result<ServerSettingValue, ServerCredentialError> {
        let mut settings = input.cloned().unwrap_or_else(empty_object);
        if tls == 2 {
            ensure_reality_keys(&mut settings)?;
        }
        if v2node {
            ensure_ech_keys(&mut settings)?;
        }
        Ok(settings)
    }

    fn prepare_encryption_settings(
        &self,
        input: Option<&ServerSettingValue>,
        encryption: Option<&str>,
        v2node: bool,
    ) -> Result<ServerSettingValue, ServerCredentialError> {
        let mut settings = input.cloned().unwrap_or_else(empty_object);
        if encryption != Some("mlkem768x25519plus") {
            return Ok(settings);
        }
        let Some(object) = settings.object_mut() else {
            return Ok(empty_object());
        };
        if v2node {
            object
                .entry("mode".into())
                .or_insert_with(|| string("native"));
        }
        match object.get("rtt").and_then(ServerSettingValue::string) {
            Some("1rtt") => {
                object.insert("ticket".into(), string("0s"));
            }
            Some(_) => {}
            None if v2node => {
                object.insert("rtt".into(), string("0rtt"));
                object.insert("ticket".into(), string("600s"));
            }
            None => {}
        }
        if missing_string(object, "private_key") || missing_string(object, "password") {
            let (public_key, private_key) = x25519_key_pair()?;
            object
                .entry("private_key".into())
                .or_insert(ServerSettingValue::String(private_key));
            object
                .entry("password".into())
                .or_insert(ServerSettingValue::String(public_key));
        }
        Ok(settings)
    }

    fn generate_obfs_password(&self, now: i64) -> Result<String, ServerCredentialError> {
        let digest = format!("{:x}", md5::compute(now.to_string()));
        Ok(general_purpose::STANDARD.encode(&digest.as_bytes()[..16]))
    }

    fn node_token(&self, kind: ServerKind, id: i32, epoch: i64) -> Option<String> {
        derive_node_token(&self.master_key, kind.as_str(), id, epoch)
    }

    fn v2node_install_command(&self, id: i32, token: Option<&str>) -> String {
        format!(
            "wget -N https://raw.githubusercontent.com/wyx2685/v2node/master/script/install.sh && bash install.sh --api-host {} --node-id {id} --api-key {}",
            shell_argument(&self.install_api_host),
            shell_argument(token.unwrap_or_default())
        )
    }
}

fn empty_object() -> ServerSettingValue {
    ServerSettingValue::Object(BTreeMap::new())
}

fn string(value: &str) -> ServerSettingValue {
    ServerSettingValue::String(value.to_string())
}

fn missing_string(object: &BTreeMap<String, ServerSettingValue>, key: &str) -> bool {
    object
        .get(key)
        .and_then(ServerSettingValue::string)
        .is_none_or(str::is_empty)
}

fn ensure_reality_keys(settings: &mut ServerSettingValue) -> Result<(), ServerCredentialError> {
    let object = settings
        .object_mut()
        .ok_or(ServerCredentialError::InvalidSettings)?;
    if missing_string(object, "public_key") || missing_string(object, "private_key") {
        let (public_key, private_key) = x25519_key_pair()?;
        object
            .entry("public_key".into())
            .or_insert(ServerSettingValue::String(public_key));
        object
            .entry("private_key".into())
            .or_insert(ServerSettingValue::String(private_key));
    }
    if missing_string(object, "short_id")
        && let Some(private_key) = object
            .get("private_key")
            .and_then(ServerSettingValue::string)
    {
        let digest = Sha1::digest(private_key.as_bytes());
        object.insert(
            "short_id".into(),
            ServerSettingValue::String(hex::encode(digest)[..8].to_string()),
        );
    }
    object
        .entry("server_port".into())
        .or_insert_with(|| string("443"));
    Ok(())
}

fn ensure_ech_keys(settings: &mut ServerSettingValue) -> Result<(), ServerCredentialError> {
    let Some(object) = settings.object_mut() else {
        return Ok(());
    };
    if object.get("ech").and_then(ServerSettingValue::string) != Some("custom") {
        return Ok(());
    }
    let outer_sni = object
        .get("ech_server_name")
        .and_then(ServerSettingValue::string)
        .unwrap_or_default()
        .to_string();
    if outer_sni.is_empty() {
        object.insert("ech".into(), string(""));
    } else if missing_string(object, "ech_key") || missing_string(object, "ech_config") {
        let (key, config) = ech_key_pair(&outer_sni)?;
        object
            .entry("ech_key".into())
            .or_insert(ServerSettingValue::String(key));
        object
            .entry("ech_config".into())
            .or_insert(ServerSettingValue::String(config));
    }
    Ok(())
}

fn x25519_key_pair() -> Result<(String, String), ServerCredentialError> {
    let key = PKey::generate_x25519().map_err(credential_generation)?;
    let public = key.raw_public_key().map_err(credential_generation)?;
    let private = key.raw_private_key().map_err(credential_generation)?;
    Ok((
        general_purpose::URL_SAFE_NO_PAD.encode(public),
        general_purpose::URL_SAFE_NO_PAD.encode(private),
    ))
}

fn ech_key_pair(outer_sni: &str) -> Result<(String, String), ServerCredentialError> {
    let key = PKey::generate_x25519().map_err(credential_generation)?;
    let public = key.raw_public_key().map_err(credential_generation)?;
    let private = key.raw_private_key().map_err(credential_generation)?;
    let config_id = Uuid::new_v4().as_bytes()[0];
    let mut config_data = vec![config_id];
    config_data.extend_from_slice(&0x0020_u16.to_be_bytes());
    config_data.extend_from_slice(&(public.len() as u16).to_be_bytes());
    config_data.extend_from_slice(&public);
    let suites = [0x0001_u16, 0x0001, 0x0001, 0x0002, 0x0001, 0x0003];
    config_data.extend_from_slice(&((suites.len() * 2) as u16).to_be_bytes());
    for suite in suites {
        config_data.extend_from_slice(&suite.to_be_bytes());
    }
    config_data.push(0);
    let name = &outer_sni.as_bytes()[..outer_sni.len().min(usize::from(u8::MAX))];
    config_data.push(name.len() as u8);
    config_data.extend_from_slice(name);
    config_data.extend_from_slice(&0_u16.to_be_bytes());
    let mut config = Vec::new();
    config.extend_from_slice(&0xfe0d_u16.to_be_bytes());
    config.extend_from_slice(&(config_data.len() as u16).to_be_bytes());
    config.extend_from_slice(&config_data);
    let mut keys = Vec::new();
    keys.extend_from_slice(&(config.len() as u16).to_be_bytes());
    keys.extend_from_slice(&config);
    keys.extend_from_slice(&1_u16.to_be_bytes());
    keys.push(config_id);
    keys.extend_from_slice(&(private.len() as u16).to_be_bytes());
    keys.extend_from_slice(&private);
    Ok((
        general_purpose::STANDARD.encode(keys),
        general_purpose::STANDARD.encode(config),
    ))
}

fn credential_generation(error: openssl::error::ErrorStack) -> ServerCredentialError {
    ServerCredentialError::Generation(error.to_string())
}

fn shell_argument(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn credential_adapter_enriches_reality_without_exposing_the_master_key() {
        let adapter =
            OpenSslServerCredentials::new("master-secret".into(), "https://api.test".into());
        let settings = adapter.prepare_tls_settings(None, 2, false).unwrap();
        let object = settings.object().unwrap();
        for key in ["public_key", "private_key", "short_id", "server_port"] {
            assert!(object.contains_key(key));
        }
        let token = adapter.node_token(ServerKind::Vless, 7, 0).unwrap();
        assert!(!token.contains("master-secret"));
    }
}
