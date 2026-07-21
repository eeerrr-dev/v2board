use anyhow::{Context, Result, bail};
use serde::de::DeserializeOwned;
use serde_json::Value;
use v2board_api_contract::{
    admin_servers::{
        ServerGroupView, ServerNodeView, ServerRouteAction as WireRouteAction, ServerRouteView,
        ShadowsocksNodeView,
    },
    time::Rfc3339Timestamp,
};
use v2board_application::server_management::{
    ServerCredentialError, ServerCredentialProvisioner, ServerGroup, ServerHealth,
    ServerManagementService, ServerNode, ServerNodeDetails, ServerPresence, ServerPresenceKey,
    ServerRoute, ServerSettingValue,
};
use v2board_db::{DbPool, admin_server::PostgresServerManagementRepository};
use v2board_domain_model::{ServerKind, ServerRouteAction};

pub(crate) type ContractServerService = ServerManagementService<
    PostgresServerManagementRepository,
    EmptyServerPresence,
    EmptyServerCredentials,
>;

pub(crate) fn contract_server_service(pool: DbPool) -> ContractServerService {
    ServerManagementService::new(
        PostgresServerManagementRepository::new(pool),
        EmptyServerPresence,
        EmptyServerCredentials,
    )
}

#[derive(Clone, Copy)]
pub(crate) struct EmptyServerPresence;

impl ServerPresence for EmptyServerPresence {
    async fn health(&self, keys: &[ServerPresenceKey]) -> Vec<ServerHealth> {
        vec![ServerHealth::default(); keys.len()]
    }
}

#[derive(Clone, Copy)]
pub(crate) struct EmptyServerCredentials;

impl ServerCredentialProvisioner for EmptyServerCredentials {
    fn prepare_tls_settings(
        &self,
        _: Option<&ServerSettingValue>,
        _: i64,
        _: bool,
    ) -> std::result::Result<ServerSettingValue, ServerCredentialError> {
        Err(ServerCredentialError::Generation(
            "contract read adapter cannot generate credentials".into(),
        ))
    }

    fn prepare_encryption_settings(
        &self,
        _: Option<&ServerSettingValue>,
        _: Option<&str>,
        _: bool,
    ) -> std::result::Result<ServerSettingValue, ServerCredentialError> {
        Err(ServerCredentialError::Generation(
            "contract read adapter cannot generate credentials".into(),
        ))
    }

    fn generate_obfs_password(&self, _: i64) -> std::result::Result<String, ServerCredentialError> {
        Err(ServerCredentialError::Generation(
            "contract read adapter cannot generate credentials".into(),
        ))
    }

    fn node_token(&self, _: ServerKind, _: i32, _: i64) -> Option<String> {
        None
    }

    fn v2node_install_command(&self, _: i32, _: Option<&str>) -> String {
        String::new()
    }
}

pub(crate) fn server_group_view(group: ServerGroup) -> ServerGroupView {
    ServerGroupView {
        id: group.id,
        name: group.name,
        user_count: group.user_count,
        server_count: group.server_count,
        created_at: Rfc3339Timestamp::from_epoch_seconds(group.created_at),
        updated_at: Rfc3339Timestamp::from_epoch_seconds(group.updated_at),
    }
}

pub(crate) fn server_route_view(route: ServerRoute) -> ServerRouteView {
    ServerRouteView {
        id: route.id,
        remarks: route.remarks,
        match_rules: route.match_rules,
        action: match route.action {
            ServerRouteAction::Block => WireRouteAction::Block,
            ServerRouteAction::BlockIp => WireRouteAction::BlockIp,
            ServerRouteAction::BlockPort => WireRouteAction::BlockPort,
            ServerRouteAction::Protocol => WireRouteAction::Protocol,
            ServerRouteAction::Dns => WireRouteAction::Dns,
            ServerRouteAction::Route => WireRouteAction::Route,
            ServerRouteAction::RouteIp => WireRouteAction::RouteIp,
            ServerRouteAction::DefaultOut => WireRouteAction::DefaultOut,
        },
        action_value: route.action_value,
        created_at: Rfc3339Timestamp::from_epoch_seconds(route.created_at),
        updated_at: Rfc3339Timestamp::from_epoch_seconds(route.updated_at),
    }
}

/// The disposable contract fixtures seed the one protocol needed to pin the
/// common node projection. API tests own exhaustive eight-way conversion.
pub(crate) fn server_node_view(node: ServerNode) -> Result<ServerNodeView> {
    let common = node.common;
    let ServerNodeDetails::Shadowsocks {
        cipher,
        obfs,
        obfs_settings,
    } = node.details
    else {
        bail!("contract fixture unexpectedly contains a non-shadowsocks node")
    };
    Ok(ServerNodeView::Shadowsocks(Box::new(ShadowsocksNodeView {
        id: common.id,
        group_id: common.group_id,
        route_id: common.route_id,
        parent_id: common.parent_id,
        tags: common.tags,
        name: common.name,
        rate: common.rate,
        host: common.host,
        port: common.port,
        server_port: common.server_port,
        show: common.show,
        sort: common.sort,
        created_at: Rfc3339Timestamp::from_epoch_seconds(common.created_at),
        updated_at: Rfc3339Timestamp::from_epoch_seconds(common.updated_at),
        online: common.online,
        last_check_at: common
            .last_check_at
            .map(Rfc3339Timestamp::from_epoch_seconds),
        last_push_at: common
            .last_push_at
            .map(Rfc3339Timestamp::from_epoch_seconds),
        available_status: common.available_status,
        api_key: common.api_key,
        cipher,
        obfs,
        obfs_settings: setting_contract(obfs_settings, "shadowsocks obfs settings")?,
    })))
}

fn setting_contract<T: DeserializeOwned>(
    value: Option<ServerSettingValue>,
    context: &'static str,
) -> Result<Option<T>> {
    value
        .map(|value| serde_json::from_value(setting_json(value)).context(context))
        .transpose()
}

fn setting_json(value: ServerSettingValue) -> Value {
    match value {
        ServerSettingValue::Null => Value::Null,
        ServerSettingValue::Bool(value) => Value::Bool(value),
        ServerSettingValue::Integer(value) => Value::from(value),
        ServerSettingValue::Decimal(value) => value
            .parse::<serde_json::Number>()
            .map(Value::Number)
            .unwrap_or(Value::String(value)),
        ServerSettingValue::String(value) => Value::String(value),
        ServerSettingValue::Array(values) => {
            Value::Array(values.into_iter().map(setting_json).collect())
        }
        ServerSettingValue::Object(values) => Value::Object(
            values
                .into_iter()
                .map(|(key, value)| (key, setting_json(value)))
                .collect(),
        ),
    }
}
