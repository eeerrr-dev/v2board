//! Administrative server-control transport contracts.

use std::collections::BTreeMap;

use serde::{Deserialize, Deserializer, Serialize, Serializer, de};
use utoipa::ToSchema;

use crate::{patch, time::Rfc3339Timestamp};

#[derive(Debug, Clone, Deserialize, ToSchema)]
#[serde(transparent)]
pub struct NodeSortRequest(pub BTreeMap<String, BTreeMap<String, i64>>);

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct ServerGroupView {
    pub id: i32,
    pub name: String,
    pub user_count: i64,
    pub server_count: i64,
    pub created_at: Rfc3339Timestamp,
    pub updated_at: Rfc3339Timestamp,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct ServerGroupWriteRequest {
    pub name: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum ServerRouteAction {
    Block,
    BlockIp,
    BlockPort,
    Protocol,
    Dns,
    Route,
    RouteIp,
    DefaultOut,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct ServerRouteView {
    pub id: i32,
    pub remarks: String,
    #[serde(rename = "match")]
    pub match_rules: Vec<String>,
    pub action: ServerRouteAction,
    #[schema(required)]
    pub action_value: Option<String>,
    pub created_at: Rfc3339Timestamp,
    pub updated_at: Rfc3339Timestamp,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct ServerRouteCreateRequest {
    pub remarks: String,
    #[serde(default, rename = "match")]
    pub match_rules: Vec<String>,
    pub action: ServerRouteAction,
    #[serde(default)]
    pub action_value: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct ServerRoutePatchRequest {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub remarks: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", rename = "match")]
    pub match_rules: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub action: Option<ServerRouteAction>,
    #[serde(default, skip_serializing_if = "Option::is_none", with = "patch")]
    pub action_value: Option<Option<String>>,
}

/// Boolean field used inside imported protocol settings. The modern wire
/// always serializes a JSON boolean, while deserialization also accepts the
/// historical `0`/`1` spellings so retained MySQL data can be projected once
/// without reopening the schema to arbitrary JSON.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ToSchema)]
#[schema(value_type = bool)]
pub struct ServerSettingBool(pub bool);

impl Serialize for ServerSettingBool {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_bool(self.0)
    }
}

impl<'de> Deserialize<'de> for ServerSettingBool {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct Visitor;

        impl de::Visitor<'_> for Visitor {
            type Value = ServerSettingBool;

            fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                formatter.write_str("a boolean or the historical 0/1 spelling")
            }

            fn visit_bool<E>(self, value: bool) -> Result<Self::Value, E> {
                Ok(ServerSettingBool(value))
            }

            fn visit_i64<E>(self, value: i64) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                match value {
                    0 => Ok(ServerSettingBool(false)),
                    1 => Ok(ServerSettingBool(true)),
                    _ => Err(E::custom("boolean integer must be 0 or 1")),
                }
            }

            fn visit_u64<E>(self, value: u64) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                match value {
                    0 => Ok(ServerSettingBool(false)),
                    1 => Ok(ServerSettingBool(true)),
                    _ => Err(E::custom("boolean integer must be 0 or 1")),
                }
            }

            fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                match value.trim().to_ascii_lowercase().as_str() {
                    "0" | "false" => Ok(ServerSettingBool(false)),
                    "1" | "true" => Ok(ServerSettingBool(true)),
                    _ => Err(E::custom("boolean string must be 0, 1, false, or true")),
                }
            }
        }

        deserializer.deserialize_any(Visitor)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(untagged)]
pub enum ServerHostValue {
    One(String),
    Many(Vec<String>),
}

/// HTTP transport header names are protocol-defined and therefore dynamic,
/// but each value is still constrained to the Xray string/string-list shape.
#[derive(Debug, Clone, Default, Serialize, Deserialize, ToSchema)]
#[serde(transparent)]
pub struct ServerTransportHeaders(pub BTreeMap<String, ServerHostValue>);

#[derive(Debug, Clone, Default, Serialize, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct ServerHttpHeaderRequest {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schema(required = false)]
    pub path: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schema(required = false)]
    pub headers: Option<ServerTransportHeaders>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct EmptyServerSettings {}

#[derive(Debug, Clone, Default, Serialize, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct ServerTransportHeader {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schema(required = false)]
    pub r#type: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schema(required = false)]
    pub request: Option<ServerHttpHeaderRequest>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schema(required = false)]
    pub response: Option<EmptyServerSettings>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct ServerXhttpXmuxSettings {
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        rename = "hKeepAlivePeriod"
    )]
    #[schema(required = false)]
    pub h_keep_alive_period: Option<i64>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        rename = "maxConcurrency"
    )]
    #[schema(required = false)]
    pub max_concurrency: Option<i64>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        rename = "maxConnections"
    )]
    #[schema(required = false)]
    pub max_connections: Option<i64>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        rename = "cMaxReuseTimes"
    )]
    #[schema(required = false)]
    pub client_max_reuse_times: Option<i64>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        rename = "hMaxRequestTimes"
    )]
    #[schema(required = false)]
    pub http_max_request_times: Option<i64>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct ServerXhttpDownloadSettings {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schema(required = false)]
    pub address: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schema(required = false)]
    pub port: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schema(required = false)]
    pub network: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schema(required = false)]
    pub security: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct ServerXhttpExtraSettings {
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        rename = "xPaddingObfsMode"
    )]
    #[schema(required = false)]
    pub padding_obfs_mode: Option<ServerSettingBool>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        rename = "noGRPCHeader"
    )]
    #[schema(required = false)]
    pub no_grpc_header: Option<ServerSettingBool>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        rename = "noSSEHeader"
    )]
    #[schema(required = false)]
    pub no_sse_header: Option<ServerSettingBool>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        rename = "scMaxBufferedPosts"
    )]
    #[schema(required = false)]
    pub max_buffered_posts: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schema(required = false)]
    pub xmux: Option<ServerXhttpXmuxSettings>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        rename = "downloadSettings"
    )]
    #[schema(required = false)]
    pub download_settings: Option<ServerXhttpDownloadSettings>,
}

/// Closed superset of the supported Xray transport settings. The adjacent
/// node `network` field selects which subset is meaningful; unsupported keys
/// are rejected instead of being silently persisted and forwarded.
#[derive(Debug, Clone, Default, Serialize, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct ServerNetworkSettings {
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        rename = "acceptProxyProtocol"
    )]
    #[schema(required = false)]
    pub accept_proxy_protocol: Option<ServerSettingBool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schema(required = false)]
    pub path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schema(required = false)]
    pub host: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", rename = "Host")]
    #[schema(required = false)]
    pub legacy_host: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schema(required = false)]
    pub headers: Option<ServerTransportHeaders>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        rename = "serviceName"
    )]
    #[schema(required = false)]
    pub service_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schema(required = false)]
    pub security: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schema(required = false)]
    pub key: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schema(required = false)]
    pub header: Option<ServerTransportHeader>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schema(required = false)]
    pub seed: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schema(required = false)]
    pub mode: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schema(required = false)]
    pub extra: Option<ServerXhttpExtraSettings>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schema(required = false)]
    pub mtu: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schema(required = false)]
    pub tti: Option<i64>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        rename = "uplinkCapacity"
    )]
    #[schema(required = false)]
    pub uplink_capacity: Option<i64>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        rename = "downlinkCapacity"
    )]
    #[schema(required = false)]
    pub downlink_capacity: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schema(required = false)]
    pub congestion: Option<ServerSettingBool>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        rename = "readBufferSize"
    )]
    #[schema(required = false)]
    pub read_buffer_size: Option<i64>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        rename = "writeBufferSize"
    )]
    #[schema(required = false)]
    pub write_buffer_size: Option<i64>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct ServerTlsSettings {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schema(required = false)]
    pub server_name: Option<String>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        rename = "serverName"
    )]
    #[schema(required = false)]
    pub legacy_server_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schema(required = false)]
    pub cert_mode: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schema(required = false)]
    pub provider: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schema(required = false)]
    pub dns_env: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schema(required = false)]
    pub cert_file: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schema(required = false)]
    pub key_file: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schema(required = false)]
    pub dest: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schema(required = false)]
    pub server_port: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schema(required = false)]
    pub xver: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schema(required = false)]
    pub private_key: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schema(required = false)]
    pub public_key: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schema(required = false)]
    pub short_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schema(required = false)]
    pub fingerprint: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schema(required = false)]
    pub reject_unknown_sni: Option<ServerSettingBool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schema(required = false)]
    pub allow_insecure: Option<ServerSettingBool>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        rename = "allowInsecure"
    )]
    #[schema(required = false)]
    pub legacy_allow_insecure: Option<ServerSettingBool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schema(required = false)]
    pub ech: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schema(required = false)]
    pub ech_server_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schema(required = false)]
    pub ech_key: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schema(required = false)]
    pub ech_config: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct ServerEncryptionSettings {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schema(required = false)]
    pub mode: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schema(required = false)]
    pub rtt: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schema(required = false)]
    pub ticket: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schema(required = false)]
    pub server_padding: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schema(required = false)]
    pub client_padding: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schema(required = false)]
    pub private_key: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schema(required = false)]
    pub password: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct ShadowsocksObfsSettings {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schema(required = false)]
    pub path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schema(required = false)]
    pub host: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct VmessRuleSettings {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schema(required = false)]
    pub domain: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schema(required = false)]
    pub protocol: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(untagged)]
pub enum VmessDnsHostValue {
    One(String),
    Many(Vec<String>),
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct VmessDnsServerObject {
    pub address: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schema(required = false)]
    pub port: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schema(required = false)]
    pub domains: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none", rename = "expectIPs")]
    #[schema(required = false)]
    pub expect_ips: Option<Vec<String>>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        rename = "skipFallback"
    )]
    #[schema(required = false)]
    pub skip_fallback: Option<ServerSettingBool>,
    #[serde(default, skip_serializing_if = "Option::is_none", rename = "clientIp")]
    #[schema(required = false)]
    pub client_ip: Option<String>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        rename = "queryStrategy"
    )]
    #[schema(required = false)]
    pub query_strategy: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(untagged)]
pub enum VmessDnsServer {
    Address(String),
    Detailed(VmessDnsServerObject),
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct VmessDnsSettings {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schema(required = false)]
    pub servers: Option<Vec<VmessDnsServer>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schema(required = false)]
    pub hosts: Option<BTreeMap<String, VmessDnsHostValue>>,
    #[serde(default, skip_serializing_if = "Option::is_none", rename = "clientIp")]
    #[schema(required = false)]
    pub client_ip: Option<String>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        rename = "queryStrategy"
    )]
    #[schema(required = false)]
    pub query_strategy: Option<String>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        rename = "disableCache"
    )]
    #[schema(required = false)]
    pub disable_cache: Option<ServerSettingBool>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        rename = "disableFallback"
    )]
    #[schema(required = false)]
    pub disable_fallback: Option<ServerSettingBool>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        rename = "disableFallbackIfMatch"
    )]
    #[schema(required = false)]
    pub disable_fallback_if_match: Option<ServerSettingBool>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct VmessRoutingRule {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schema(required = false)]
    pub r#type: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schema(required = false)]
    pub domain: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schema(required = false)]
    pub ip: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schema(required = false)]
    pub port: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schema(required = false)]
    pub network: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schema(required = false)]
    pub source: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schema(required = false)]
    pub user: Option<Vec<String>>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        rename = "inboundTag"
    )]
    #[schema(required = false)]
    pub inbound_tag: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schema(required = false)]
    pub protocol: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schema(required = false)]
    pub attrs: Option<String>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        rename = "outboundTag"
    )]
    #[schema(required = false)]
    pub outbound_tag: Option<String>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        rename = "balancerTag"
    )]
    #[schema(required = false)]
    pub balancer_tag: Option<String>,
}

/// The exhaustive union of fields accepted by the eight protocol routes.
/// The `{type}` path segment selects the narrower validation matrix in the
/// application layer; unknown top-level fields are rejected at extraction.
#[derive(Debug, Clone, Default, Serialize, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct ServerWriteRequest {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub group_id: Option<Vec<i64>>,
    #[serde(default, skip_serializing_if = "Option::is_none", with = "patch")]
    pub route_id: Option<Option<Vec<i64>>>,
    #[serde(default, skip_serializing_if = "Option::is_none", with = "patch")]
    pub parent_id: Option<Option<i64>>,
    #[serde(default, skip_serializing_if = "Option::is_none", with = "patch")]
    pub tags: Option<Option<Vec<String>>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rate: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub host: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub port: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub server_port: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub show: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rotate_credential: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none", with = "patch")]
    pub cipher: Option<Option<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none", with = "patch")]
    pub obfs: Option<Option<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none", with = "patch")]
    pub obfs_settings: Option<Option<ShadowsocksObfsSettings>>,
    #[serde(default, skip_serializing_if = "Option::is_none", with = "patch")]
    pub obfs_password: Option<Option<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub network: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", with = "patch")]
    pub network_settings: Option<Option<ServerNetworkSettings>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub allow_insecure: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none", with = "patch")]
    pub server_name: Option<Option<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tls: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none", with = "patch")]
    pub tls_settings: Option<Option<ServerTlsSettings>>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        with = "patch",
        rename = "networkSettings"
    )]
    pub vmess_network_settings: Option<Option<ServerNetworkSettings>>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        with = "patch",
        rename = "tlsSettings"
    )]
    pub vmess_tls_settings: Option<Option<ServerTlsSettings>>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        with = "patch",
        rename = "ruleSettings"
    )]
    pub vmess_rule_settings: Option<Option<VmessRuleSettings>>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        with = "patch",
        rename = "dnsSettings"
    )]
    pub vmess_dns_settings: Option<Option<VmessDnsSettings>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub insecure: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub disable_sni: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none", with = "patch")]
    pub udp_relay_mode: Option<Option<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub zero_rtt_handshake: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none", with = "patch")]
    pub congestion_control: Option<Option<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none", with = "patch")]
    pub up_mbps: Option<Option<i64>>,
    #[serde(default, skip_serializing_if = "Option::is_none", with = "patch")]
    pub down_mbps: Option<Option<i64>>,
    #[serde(default, skip_serializing_if = "Option::is_none", with = "patch")]
    pub flow: Option<Option<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none", with = "patch")]
    pub encryption: Option<Option<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none", with = "patch")]
    pub encryption_settings: Option<Option<ServerEncryptionSettings>>,
    #[serde(default, skip_serializing_if = "Option::is_none", with = "patch")]
    pub sort: Option<Option<i64>>,
    #[serde(default, skip_serializing_if = "Option::is_none", with = "patch")]
    pub padding_scheme: Option<Option<Vec<String>>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub listen_ip: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub protocol: Option<String>,
}

macro_rules! node_view {
    ($name:ident { $($(#[$meta:meta])* $field:ident: $ty:ty),* $(,)? }) => {
        #[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
        #[serde(deny_unknown_fields)]
        pub struct $name {
            pub id: i32,
            pub group_id: Vec<i64>,
            #[schema(required)]
            pub route_id: Option<Vec<i64>>,
            #[schema(required)]
            pub parent_id: Option<i32>,
            #[schema(required)]
            pub tags: Option<Vec<String>>,
            pub name: String,
            pub rate: f64,
            pub host: String,
            pub port: f64,
            pub server_port: i32,
            pub show: bool,
            #[schema(required)]
            pub sort: Option<i32>,
            pub created_at: Rfc3339Timestamp,
            pub updated_at: Rfc3339Timestamp,
            #[schema(required)]
            pub online: Option<i64>,
            #[schema(required)]
            pub last_check_at: Option<Rfc3339Timestamp>,
            #[schema(required)]
            pub last_push_at: Option<Rfc3339Timestamp>,
            pub available_status: i16,
            #[schema(required)]
            pub api_key: Option<String>,
            $(
                $(#[$meta])*
                pub $field: $ty,
            )*
        }
    };
}

node_view!(ShadowsocksNodeView {
    cipher: String,
    #[schema(required)] obfs: Option<String>,
    #[schema(required)] obfs_settings: Option<ShadowsocksObfsSettings>,
});
node_view!(VmessNodeView {
    tls: i16,
    network: String,
    #[schema(required)] rules: Option<Vec<VmessRoutingRule>>,
    #[serde(rename = "networkSettings")] #[schema(required)] network_settings: Option<ServerNetworkSettings>,
    #[serde(rename = "tlsSettings")] #[schema(required)] tls_settings: Option<ServerTlsSettings>,
    #[serde(rename = "ruleSettings")] #[schema(required)] rule_settings: Option<VmessRuleSettings>,
    #[serde(rename = "dnsSettings")] #[schema(required)] dns_settings: Option<VmessDnsSettings>,
});
node_view!(TrojanNodeView {
    #[schema(required)] network: Option<String>,
    #[schema(required)] network_settings: Option<ServerNetworkSettings>,
    allow_insecure: bool,
    #[schema(required)] server_name: Option<String>,
});
node_view!(TuicNodeView {
    #[schema(required)] server_name: Option<String>,
    insecure: bool,
    disable_sni: bool,
    #[schema(required)] udp_relay_mode: Option<String>,
    zero_rtt_handshake: bool,
    #[schema(required)] congestion_control: Option<String>,
});
node_view!(HysteriaNodeView {
    version: i32,
    up_mbps: i32,
    down_mbps: i32,
    #[schema(required)] obfs: Option<String>,
    #[schema(required)] obfs_password: Option<String>,
    #[schema(required)] server_name: Option<String>,
    insecure: bool,
});
node_view!(VlessNodeView {
    tls: i16,
    #[schema(required)] tls_settings: Option<ServerTlsSettings>,
    #[schema(required)] flow: Option<String>,
    network: String,
    #[schema(required)] network_settings: Option<ServerNetworkSettings>,
    #[schema(required)] encryption: Option<String>,
    #[schema(required)] encryption_settings: Option<ServerEncryptionSettings>,
});
node_view!(AnytlsNodeView {
    #[schema(required)] server_name: Option<String>,
    insecure: bool,
    #[schema(required)] padding_scheme: Option<Vec<String>>,
});
node_view!(V2nodeNodeView {
    listen_ip: String,
    protocol: String,
    tls: i16,
    #[schema(required)] tls_settings: Option<ServerTlsSettings>,
    #[schema(required)] flow: Option<String>,
    network: String,
    #[schema(required)] network_settings: Option<ServerNetworkSettings>,
    #[schema(required)] encryption: Option<String>,
    #[schema(required)] encryption_settings: Option<ServerEncryptionSettings>,
    disable_sni: bool,
    #[schema(required)] udp_relay_mode: Option<String>,
    zero_rtt_handshake: bool,
    #[schema(required)] congestion_control: Option<String>,
    #[schema(required)] cipher: Option<String>,
    up_mbps: i32,
    down_mbps: i32,
    #[schema(required)] obfs: Option<String>,
    #[schema(required)] obfs_password: Option<String>,
    #[schema(required)] padding_scheme: Option<Vec<String>>,
    install_command: String,
});

/// Discriminated server-node response; protocol-specific fields cannot leak
/// into a different variant.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum ServerNodeView {
    Shadowsocks(Box<ShadowsocksNodeView>),
    Vmess(Box<VmessNodeView>),
    Trojan(Box<TrojanNodeView>),
    Tuic(Box<TuicNodeView>),
    Hysteria(Box<HysteriaNodeView>),
    Vless(Box<VlessNodeView>),
    Anytls(Box<AnytlsNodeView>),
    V2node(Box<V2nodeNodeView>),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn server_write_rejects_unknown_protocol_fields() {
        assert!(
            serde_json::from_value::<ServerWriteRequest>(serde_json::json!({"networkSetting": {}}))
                .is_err()
        );
    }

    #[test]
    fn server_write_decodes_closed_nested_settings_without_opening_the_root() {
        let request = serde_json::from_value::<ServerWriteRequest>(serde_json::json!({
            "network_settings": {"path": "/ws"},
            "padding_scheme": ["30-30"]
        }))
        .expect("known nested DTO fields");
        assert_eq!(
            serde_json::to_value(request.network_settings).unwrap(),
            serde_json::json!({"path": "/ws"})
        );
        assert_eq!(request.padding_scheme, Some(Some(vec!["30-30".to_owned()])));
    }

    #[test]
    fn nested_settings_reject_unknown_fields_and_non_string_padding_entries() {
        assert!(
            serde_json::from_value::<ServerWriteRequest>(serde_json::json!({
                "network_settings": {"path": "/ws", "pth": "typo"}
            }))
            .is_err()
        );
        assert!(
            serde_json::from_value::<ServerWriteRequest>(serde_json::json!({
                "padding_scheme": ["30-30", 42]
            }))
            .is_err()
        );
    }

    #[test]
    fn transport_headers_allow_dynamic_names_but_reject_untyped_values() {
        let request = serde_json::from_value::<ServerWriteRequest>(serde_json::json!({
            "network_settings": {
                "headers": {
                    "User-Agent": "v2board-node",
                    "Cookie": ["session=one", "region=ca"]
                }
            }
        }))
        .expect("protocol-defined header names with typed values");

        assert_eq!(
            serde_json::to_value(request.network_settings).unwrap(),
            serde_json::json!({
                "headers": {
                    "Cookie": ["session=one", "region=ca"],
                    "User-Agent": "v2board-node"
                }
            })
        );
        assert!(
            serde_json::from_value::<ServerWriteRequest>(serde_json::json!({
                "network_settings": {"headers": {"X-Retry": 3}}
            }))
            .is_err()
        );
        assert!(
            serde_json::from_value::<ServerWriteRequest>(serde_json::json!({
                "network_settings": {"headers": {"X-Metadata": {"open": true}}}
            }))
            .is_err()
        );
    }
}
