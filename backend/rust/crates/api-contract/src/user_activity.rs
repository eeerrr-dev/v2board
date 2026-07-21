//! Authenticated service-usage, invite/commission, and ticket contracts.

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::{
    Page,
    admin_servers::{
        ServerEncryptionSettings, ServerNetworkSettings, ServerSettingBool, ServerTlsSettings,
        ShadowsocksObfsSettings,
    },
    time::Rfc3339Timestamp,
};

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct UserServerFields {
    pub id: i32,
    #[schema(required)]
    pub parent_id: Option<i32>,
    pub group_id: Vec<i32>,
    #[schema(required)]
    pub route_id: Option<Vec<i32>>,
    pub name: String,
    pub rate: f64,
    pub host: String,
    pub port: i64,
    pub cache_key: String,
    #[schema(required)]
    pub last_check_at: Option<Rfc3339Timestamp>,
    pub is_online: bool,
    #[schema(required)]
    pub tags: Option<Vec<String>>,
    #[schema(required)]
    pub sort: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct UserShadowsocksExtra {
    pub cipher: String,
    #[schema(required)]
    pub obfs: Option<String>,
    #[schema(required)]
    pub obfs_settings: Option<ShadowsocksObfsSettings>,
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct UserVmessExtra {
    pub network: String,
    pub tls: i16,
    #[schema(required)]
    pub network_settings: Option<ServerNetworkSettings>,
    #[schema(required)]
    pub tls_settings: Option<ServerTlsSettings>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct UserTrojanExtra {
    #[schema(required)]
    pub network: Option<String>,
    #[schema(required)]
    pub network_settings: Option<ServerNetworkSettings>,
    pub allow_insecure: ServerSettingBool,
    #[schema(required)]
    pub server_name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct UserTuicExtra {
    #[schema(required)]
    pub server_name: Option<String>,
    pub insecure: ServerSettingBool,
    pub disable_sni: ServerSettingBool,
    #[schema(required)]
    pub udp_relay_mode: Option<String>,
    pub zero_rtt_handshake: ServerSettingBool,
    #[schema(required)]
    pub congestion_control: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct UserHysteriaExtra {
    pub version: i32,
    pub up_mbps: i32,
    pub down_mbps: i32,
    #[schema(required)]
    pub obfs: Option<String>,
    #[schema(required)]
    pub obfs_password: Option<String>,
    #[schema(required)]
    pub server_name: Option<String>,
    pub insecure: ServerSettingBool,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct UserVlessExtra {
    pub tls: i16,
    #[schema(required)]
    pub tls_settings: Option<ServerTlsSettings>,
    #[schema(required)]
    pub flow: Option<String>,
    pub network: String,
    #[schema(required)]
    pub network_settings: Option<ServerNetworkSettings>,
    #[schema(required)]
    pub encryption: Option<String>,
    #[schema(required)]
    pub encryption_settings: Option<ServerEncryptionSettings>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct UserAnytlsExtra {
    #[schema(required)]
    pub server_name: Option<String>,
    pub insecure: ServerSettingBool,
    #[schema(required)]
    pub padding_scheme: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct UserV2nodeExtra {
    pub protocol: String,
    pub tls: i16,
    #[schema(required)]
    pub tls_settings: Option<ServerTlsSettings>,
    #[schema(required)]
    pub flow: Option<String>,
    pub network: String,
    #[schema(required)]
    pub network_settings: Option<ServerNetworkSettings>,
    #[schema(required)]
    pub encryption: Option<String>,
    #[schema(required)]
    pub encryption_settings: Option<ServerEncryptionSettings>,
    pub disable_sni: ServerSettingBool,
    #[schema(required)]
    pub udp_relay_mode: Option<String>,
    pub zero_rtt_handshake: ServerSettingBool,
    #[schema(required)]
    pub congestion_control: Option<String>,
    #[schema(required)]
    pub cipher: Option<String>,
    pub up_mbps: i32,
    pub down_mbps: i32,
    #[schema(required)]
    pub obfs: Option<String>,
    #[schema(required)]
    pub obfs_password: Option<String>,
    #[schema(required)]
    pub padding_scheme: Option<Vec<String>>,
}

/// The service-node response is discriminated by `type`; each `extra` object
/// is the exact projection built for that protocol by the persistence adapter.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum UserServerView {
    Shadowsocks {
        #[serde(flatten)]
        server: UserServerFields,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        extra: Option<UserShadowsocksExtra>,
    },
    Vmess {
        #[serde(flatten)]
        server: UserServerFields,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        extra: Option<UserVmessExtra>,
    },
    Trojan {
        #[serde(flatten)]
        server: UserServerFields,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        extra: Option<UserTrojanExtra>,
    },
    Tuic {
        #[serde(flatten)]
        server: UserServerFields,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        extra: Option<UserTuicExtra>,
    },
    Hysteria {
        #[serde(flatten)]
        server: UserServerFields,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        extra: Option<UserHysteriaExtra>,
    },
    Vless {
        #[serde(flatten)]
        server: UserServerFields,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        extra: Option<UserVlessExtra>,
    },
    Anytls {
        #[serde(flatten)]
        server: UserServerFields,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        extra: Option<UserAnytlsExtra>,
    },
    V2node {
        #[serde(flatten)]
        server: UserServerFields,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        extra: Option<UserV2nodeExtra>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct TrafficLogView {
    pub u: i64,
    pub d: i64,
    pub record_at: Rfc3339Timestamp,
    pub user_id: i64,
    pub server_rate: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct InviteCodeView {
    pub id: i32,
    pub code: String,
    pub pv: i32,
    pub created_at: Rfc3339Timestamp,
    pub updated_at: Rfc3339Timestamp,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct InviteStatView {
    pub registered_count: i64,
    pub valid_commission: i64,
    pub pending_commission: i64,
    pub commission_rate: i64,
    pub available_commission: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct InviteView {
    pub codes: Vec<InviteCodeView>,
    pub stat: InviteStatView,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct CommissionView {
    pub id: i64,
    pub trade_no: String,
    pub order_amount: i32,
    pub get_amount: i32,
    pub created_at: Rfc3339Timestamp,
}

pub type CommissionPage = Page<CommissionView>;

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct UserTicketView {
    pub id: i64,
    pub user_id: i64,
    pub subject: String,
    pub level: i16,
    pub status: i16,
    pub reply_status: i16,
    #[schema(required)]
    pub last_reply_user_id: Option<i64>,
    pub created_at: Rfc3339Timestamp,
    pub updated_at: Rfc3339Timestamp,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct UserTicketMessageView {
    pub id: i64,
    pub user_id: i64,
    pub ticket_id: i64,
    pub message: String,
    pub is_me: bool,
    pub created_at: Rfc3339Timestamp,
    pub updated_at: Rfc3339Timestamp,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct UserTicketDetailView {
    pub id: i64,
    pub user_id: i64,
    pub subject: String,
    pub level: i16,
    pub status: i16,
    pub reply_status: i16,
    #[schema(required)]
    pub last_reply_user_id: Option<i64>,
    pub created_at: Rfc3339Timestamp,
    pub updated_at: Rfc3339Timestamp,
    pub message: Vec<UserTicketMessageView>,
}

#[derive(Debug, Clone, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct UserTicketCreateRequest {
    pub subject: String,
    pub level: i16,
    pub message: String,
}

#[derive(Debug, Clone, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct UserTicketReplyRequest {
    pub message: String,
}

#[derive(Debug, Clone, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct WithdrawalTicketCreateRequest {
    pub withdraw_method: String,
    pub withdraw_account: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ticket_create_requires_every_business_field() {
        assert!(
            serde_json::from_value::<UserTicketCreateRequest>(serde_json::json!({
                "subject": "Need help",
                "message": "Details"
            }))
            .is_err()
        );
    }

    #[test]
    fn nested_server_extension_does_not_open_the_root_object() {
        let value = serde_json::json!({
            "id": 1,
            "parent_id": null,
            "group_id": [1],
            "route_id": null,
            "name": "edge",
            "rate": 1.0,
            "type": "vmess",
            "host": "edge.example.test",
            "port": 443,
            "cache_key": "VMESS_1",
            "last_check_at": null,
            "is_online": false,
            "tags": null,
            "sort": null,
            "extra": {"network": "ws"},
            "typo": true
        });
        assert!(serde_json::from_value::<UserServerView>(value).is_err());
    }
}
