use axum::{
    Json,
    extract::{Extension, Path, Query, State},
    http::{HeaderMap, StatusCode},
};
use chrono::Utc;
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use serde_json::Value;
use v2board_api_contract::{
    CreatedInt32Id,
    admin_servers::{
        AnytlsNodeView, HysteriaNodeView, NodeSortRequest, ServerGroupView,
        ServerGroupWriteRequest, ServerNodeView, ServerRouteAction as WireRouteAction,
        ServerRouteCreateRequest, ServerRoutePatchRequest, ServerRouteView, ServerWriteRequest,
        ShadowsocksNodeView, TrojanNodeView, TuicNodeView, V2nodeNodeView, VlessNodeView,
        VmessNodeView,
    },
    time::Rfc3339Timestamp,
};
use v2board_application::{
    auth::AuthUser,
    server_management::{
        ServerCredentialError, ServerGroupReference, ServerManagementError, ServerNode,
        ServerNodeDetails, ServerRoute, ServerRouteCreateInput, ServerRoutePatchInput,
        ServerSettingValue, ServerWriteInput,
    },
};
use v2board_compat::{ApiError, Code, Problem};
use v2board_domain_model::{ServerInputViolation, ServerRouteAction};

use crate::{
    auth::require_privileged_step_up, dialect::problem_from, locale::request_locale,
    runtime::AppState,
};

pub(super) async fn nodes_list(
    State(state): State<AppState>,
    Extension(admin): Extension<AuthUser>,
    headers: HeaderMap,
) -> Result<Json<Vec<ServerNodeView>>, Problem> {
    let locale = request_locale(&headers);
    require_privileged_step_up(&state, &headers, &admin)
        .await
        .map_err(|error| problem_from(error, locale))?;
    state
        .server_management_service()
        .nodes(Utc::now().timestamp())
        .await
        .map_err(|error| server_problem(error, locale))?
        .into_iter()
        .map(|node| node_view(node, locale))
        .collect::<Result<Vec<_>, _>>()
        .map(Json)
}

pub(super) async fn nodes_sort(
    State(state): State<AppState>,
    headers: HeaderMap,
    DialectJson(body): DialectJson<NodeSortRequest>,
) -> Result<StatusCode, Problem> {
    let locale = request_locale(&headers);
    state
        .server_management_service()
        .sort_nodes(&body.0)
        .await
        .map_err(|error| server_problem(error, locale))?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize)]
pub(super) struct ServerGroupsQuery {
    group_id: Option<i64>,
}

pub(super) async fn server_groups_list(
    State(state): State<AppState>,
    Query(query): Query<ServerGroupsQuery>,
    headers: HeaderMap,
) -> Result<Json<Vec<ServerGroupView>>, Problem> {
    let locale = request_locale(&headers);
    let groups = state
        .server_management_service()
        .groups(query.group_id)
        .await
        .map_err(|error| server_problem(error, locale))?;
    Ok(Json(
        groups
            .into_iter()
            .map(|group| ServerGroupView {
                id: group.id,
                name: group.name,
                user_count: group.user_count,
                server_count: group.server_count,
                created_at: Rfc3339Timestamp::from_epoch_seconds(group.created_at),
                updated_at: Rfc3339Timestamp::from_epoch_seconds(group.updated_at),
            })
            .collect(),
    ))
}

pub(super) async fn server_group_create(
    State(state): State<AppState>,
    headers: HeaderMap,
    DialectJson(body): DialectJson<ServerGroupWriteRequest>,
) -> Result<(StatusCode, Json<CreatedInt32Id>), Problem> {
    let locale = request_locale(&headers);
    let id = state
        .server_management_service()
        .create_group(&body.name, Utc::now().timestamp())
        .await
        .map_err(|error| server_problem(error, locale))?;
    Ok((StatusCode::CREATED, Json(CreatedInt32Id { id })))
}

pub(super) async fn server_group_patch(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    headers: HeaderMap,
    DialectJson(body): DialectJson<ServerGroupWriteRequest>,
) -> Result<StatusCode, Problem> {
    let locale = request_locale(&headers);
    state
        .server_management_service()
        .patch_group(id, &body.name, Utc::now().timestamp())
        .await
        .map_err(|error| server_problem(error, locale))?;
    Ok(StatusCode::NO_CONTENT)
}

pub(super) async fn server_group_delete(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    headers: HeaderMap,
) -> Result<StatusCode, Problem> {
    let locale = request_locale(&headers);
    state
        .server_management_service()
        .delete_group(id)
        .await
        .map_err(|error| server_problem(error, locale))?;
    Ok(StatusCode::NO_CONTENT)
}

pub(super) async fn server_routes_list(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<ServerRouteView>>, Problem> {
    let locale = request_locale(&headers);
    let routes = state
        .server_management_service()
        .routes()
        .await
        .map_err(|error| server_problem(error, locale))?;
    Ok(Json(routes.into_iter().map(route_view).collect()))
}

pub(super) async fn server_route_create(
    State(state): State<AppState>,
    headers: HeaderMap,
    DialectJson(body): DialectJson<ServerRouteCreateRequest>,
) -> Result<(StatusCode, Json<CreatedInt32Id>), Problem> {
    let locale = request_locale(&headers);
    let id = state
        .server_management_service()
        .create_route(
            ServerRouteCreateInput {
                remarks: body.remarks,
                match_rules: body.match_rules,
                action: application_route_action(body.action),
                action_value: body.action_value,
            },
            Utc::now().timestamp(),
        )
        .await
        .map_err(|error| server_problem(error, locale))?;
    Ok((StatusCode::CREATED, Json(CreatedInt32Id { id })))
}

pub(super) async fn server_route_patch(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    headers: HeaderMap,
    DialectJson(body): DialectJson<ServerRoutePatchRequest>,
) -> Result<StatusCode, Problem> {
    let locale = request_locale(&headers);
    state
        .server_management_service()
        .patch_route(
            id,
            ServerRoutePatchInput {
                remarks: body.remarks,
                match_rules: body.match_rules,
                action: body.action.map(application_route_action),
                action_value: body.action_value,
            },
            Utc::now().timestamp(),
        )
        .await
        .map_err(|error| server_problem(error, locale))?;
    Ok(StatusCode::NO_CONTENT)
}

pub(super) async fn server_route_delete(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    headers: HeaderMap,
) -> Result<StatusCode, Problem> {
    let locale = request_locale(&headers);
    state
        .server_management_service()
        .delete_route(id)
        .await
        .map_err(|error| server_problem(error, locale))?;
    Ok(StatusCode::NO_CONTENT)
}

pub(super) async fn server_create(
    State(state): State<AppState>,
    Path(kind): Path<String>,
    headers: HeaderMap,
    DialectJson(body): DialectJson<ServerWriteRequest>,
) -> Result<(StatusCode, Json<CreatedInt32Id>), Problem> {
    let locale = request_locale(&headers);
    let input = application_server_write(body, locale)?;
    let id = state
        .server_management_service()
        .create_server(&kind, &input, Utc::now().timestamp())
        .await
        .map_err(|error| server_problem(error, locale))?;
    Ok((StatusCode::CREATED, Json(CreatedInt32Id { id })))
}

pub(super) async fn server_patch(
    State(state): State<AppState>,
    Path((kind, id)): Path<(String, i64)>,
    headers: HeaderMap,
    DialectJson(body): DialectJson<ServerWriteRequest>,
) -> Result<StatusCode, Problem> {
    let locale = request_locale(&headers);
    let input = application_server_write(body, locale)?;
    state
        .server_management_service()
        .patch_server(&kind, id, &input, Utc::now().timestamp())
        .await
        .map_err(|error| server_problem(error, locale))?;
    Ok(StatusCode::NO_CONTENT)
}

pub(super) async fn server_delete(
    State(state): State<AppState>,
    Path((kind, id)): Path<(String, i64)>,
    headers: HeaderMap,
) -> Result<StatusCode, Problem> {
    let locale = request_locale(&headers);
    state
        .server_management_service()
        .delete_server(&kind, id)
        .await
        .map_err(|error| server_problem(error, locale))?;
    Ok(StatusCode::NO_CONTENT)
}

pub(super) async fn server_copy(
    State(state): State<AppState>,
    Path((kind, id)): Path<(String, i64)>,
    headers: HeaderMap,
) -> Result<(StatusCode, Json<CreatedInt32Id>), Problem> {
    let locale = request_locale(&headers);
    let id = state
        .server_management_service()
        .copy_server(&kind, id, Utc::now().timestamp())
        .await
        .map_err(|error| server_problem(error, locale))?;
    Ok((StatusCode::CREATED, Json(CreatedInt32Id { id })))
}

use crate::dialect::DialectJson;

fn application_server_write(
    body: ServerWriteRequest,
    locale: &str,
) -> Result<ServerWriteInput, Problem> {
    Ok(ServerWriteInput {
        group_id: body.group_id,
        route_id: body.route_id,
        parent_id: body.parent_id,
        tags: body.tags,
        name: body.name,
        rate: body.rate,
        host: body.host,
        port: body.port,
        server_port: body.server_port,
        show: body.show,
        rotate_credential: body.rotate_credential,
        cipher: body.cipher,
        obfs: body.obfs,
        obfs_settings: request_setting(body.obfs_settings, "obfs_settings", locale)?,
        obfs_password: body.obfs_password,
        network: body.network,
        network_settings: request_setting(body.network_settings, "network_settings", locale)?,
        allow_insecure: body.allow_insecure,
        server_name: body.server_name,
        tls: body.tls,
        tls_settings: request_setting(body.tls_settings, "tls_settings", locale)?,
        vmess_network_settings: request_setting(
            body.vmess_network_settings,
            "networkSettings",
            locale,
        )?,
        vmess_tls_settings: request_setting(body.vmess_tls_settings, "tlsSettings", locale)?,
        vmess_rule_settings: request_setting(body.vmess_rule_settings, "ruleSettings", locale)?,
        vmess_dns_settings: request_setting(body.vmess_dns_settings, "dnsSettings", locale)?,
        insecure: body.insecure,
        disable_sni: body.disable_sni,
        udp_relay_mode: body.udp_relay_mode,
        zero_rtt_handshake: body.zero_rtt_handshake,
        congestion_control: body.congestion_control,
        version: body.version,
        up_mbps: body.up_mbps,
        down_mbps: body.down_mbps,
        flow: body.flow,
        encryption: body.encryption,
        encryption_settings: request_setting(
            body.encryption_settings,
            "encryption_settings",
            locale,
        )?,
        sort: body.sort,
        padding_scheme: request_setting(body.padding_scheme, "padding_scheme", locale)?,
        listen_ip: body.listen_ip,
        protocol: body.protocol,
    })
}

fn request_setting<T: Serialize>(
    value: Option<Option<T>>,
    name: &'static str,
    locale: &str,
) -> Result<Option<Option<ServerSettingValue>>, Problem> {
    value
        .map(|value| {
            value
                .map(|value| {
                    serde_json::to_value(value)
                        .map(setting_from_json)
                        .map_err(|error| {
                            tracing::error!(
                                ?error,
                                setting = name,
                                "typed server setting conversion failed"
                            );
                            problem_from(
                                ApiError::internal("typed server setting conversion failed"),
                                locale,
                            )
                        })
                })
                .transpose()
        })
        .transpose()
}

fn setting_from_json(value: Value) -> ServerSettingValue {
    match value {
        Value::Null => ServerSettingValue::Null,
        Value::Bool(value) => ServerSettingValue::Bool(value),
        Value::Number(value) => value
            .as_i64()
            .map(ServerSettingValue::Integer)
            .unwrap_or_else(|| ServerSettingValue::Decimal(value.to_string())),
        Value::String(value) => ServerSettingValue::String(value),
        Value::Array(values) => {
            ServerSettingValue::Array(values.into_iter().map(setting_from_json).collect())
        }
        Value::Object(values) => ServerSettingValue::Object(
            values
                .into_iter()
                .map(|(key, value)| (key, setting_from_json(value)))
                .collect(),
        ),
    }
}

fn setting_to_json(value: ServerSettingValue) -> Value {
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
            Value::Array(values.into_iter().map(setting_to_json).collect())
        }
        ServerSettingValue::Object(values) => Value::Object(
            values
                .into_iter()
                .map(|(key, value)| (key, setting_to_json(value)))
                .collect(),
        ),
    }
}

fn response_setting<T: DeserializeOwned>(
    value: Option<ServerSettingValue>,
    name: &'static str,
    locale: &str,
) -> Result<Option<T>, Problem> {
    value
        .map(|value| {
            serde_json::from_value(setting_to_json(value)).map_err(|error| {
                tracing::error!(
                    ?error,
                    setting = name,
                    "stored server setting violates its typed contract"
                );
                problem_from(
                    ApiError::internal("stored server setting violates its typed contract"),
                    locale,
                )
            })
        })
        .transpose()
}

macro_rules! node_view_struct {
    ($view:ident, $common:ident; $($name:ident: $value:expr),* $(,)?) => {
        $view {
            id: $common.id,
            group_id: $common.group_id,
            route_id: $common.route_id,
            parent_id: $common.parent_id,
            tags: $common.tags,
            name: $common.name,
            rate: $common.rate,
            host: $common.host,
            port: $common.port,
            server_port: $common.server_port,
            show: $common.show,
            sort: $common.sort,
            created_at: Rfc3339Timestamp::from_epoch_seconds($common.created_at),
            updated_at: Rfc3339Timestamp::from_epoch_seconds($common.updated_at),
            online: $common.online,
            last_check_at: $common.last_check_at.map(Rfc3339Timestamp::from_epoch_seconds),
            last_push_at: $common.last_push_at.map(Rfc3339Timestamp::from_epoch_seconds),
            available_status: $common.available_status,
            api_key: $common.api_key,
            $($name: $value),*
        }
    };
}

fn node_view(node: ServerNode, locale: &str) -> Result<ServerNodeView, Problem> {
    let common = node.common;
    Ok(match node.details {
        ServerNodeDetails::Shadowsocks {
            cipher,
            obfs,
            obfs_settings,
        } => ServerNodeView::Shadowsocks(Box::new(node_view_struct!(ShadowsocksNodeView, common;
            cipher: cipher,
            obfs: obfs,
            obfs_settings: response_setting(obfs_settings, "ShadowsocksObfsSettings", locale)?,
        ))),
        ServerNodeDetails::Vmess {
            tls,
            network,
            rules,
            network_settings,
            tls_settings,
            rule_settings,
            dns_settings,
        } => ServerNodeView::Vmess(Box::new(node_view_struct!(VmessNodeView, common;
            tls: tls,
            network: network,
            rules: response_setting(rules, "VmessRoutingRule[]", locale)?,
            network_settings: response_setting(network_settings, "ServerNetworkSettings", locale)?,
            tls_settings: response_setting(tls_settings, "ServerTlsSettings", locale)?,
            rule_settings: response_setting(rule_settings, "VmessRuleSettings", locale)?,
            dns_settings: response_setting(dns_settings, "VmessDnsSettings", locale)?,
        ))),
        ServerNodeDetails::Trojan {
            network,
            network_settings,
            allow_insecure,
            server_name,
        } => ServerNodeView::Trojan(Box::new(node_view_struct!(TrojanNodeView, common;
            network: network,
            network_settings: response_setting(network_settings, "ServerNetworkSettings", locale)?,
            allow_insecure: allow_insecure,
            server_name: server_name,
        ))),
        ServerNodeDetails::Tuic {
            server_name,
            insecure,
            disable_sni,
            udp_relay_mode,
            zero_rtt_handshake,
            congestion_control,
        } => ServerNodeView::Tuic(Box::new(node_view_struct!(TuicNodeView, common;
            server_name: server_name,
            insecure: insecure,
            disable_sni: disable_sni,
            udp_relay_mode: udp_relay_mode,
            zero_rtt_handshake: zero_rtt_handshake,
            congestion_control: congestion_control,
        ))),
        ServerNodeDetails::Hysteria {
            version,
            up_mbps,
            down_mbps,
            obfs,
            obfs_password,
            server_name,
            insecure,
        } => ServerNodeView::Hysteria(Box::new(node_view_struct!(HysteriaNodeView, common;
            version: version,
            up_mbps: up_mbps,
            down_mbps: down_mbps,
            obfs: obfs,
            obfs_password: obfs_password,
            server_name: server_name,
            insecure: insecure,
        ))),
        ServerNodeDetails::Vless {
            tls,
            tls_settings,
            flow,
            network,
            network_settings,
            encryption,
            encryption_settings,
        } => ServerNodeView::Vless(Box::new(node_view_struct!(VlessNodeView, common;
            tls: tls,
            tls_settings: response_setting(tls_settings, "ServerTlsSettings", locale)?,
            flow: flow,
            network: network,
            network_settings: response_setting(network_settings, "ServerNetworkSettings", locale)?,
            encryption: encryption,
            encryption_settings: response_setting(encryption_settings, "ServerEncryptionSettings", locale)?,
        ))),
        ServerNodeDetails::Anytls {
            server_name,
            insecure,
            padding_scheme,
        } => ServerNodeView::Anytls(Box::new(node_view_struct!(AnytlsNodeView, common;
            server_name: server_name,
            insecure: insecure,
            padding_scheme: response_setting(padding_scheme, "padding_scheme", locale)?,
        ))),
        ServerNodeDetails::V2node {
            listen_ip,
            protocol,
            tls,
            tls_settings,
            flow,
            network,
            network_settings,
            encryption,
            encryption_settings,
            disable_sni,
            udp_relay_mode,
            zero_rtt_handshake,
            congestion_control,
            cipher,
            up_mbps,
            down_mbps,
            obfs,
            obfs_password,
            padding_scheme,
            install_command,
        } => ServerNodeView::V2node(Box::new(node_view_struct!(V2nodeNodeView, common;
            listen_ip: listen_ip,
            protocol: protocol,
            tls: tls,
            tls_settings: response_setting(tls_settings, "ServerTlsSettings", locale)?,
            flow: flow,
            network: network,
            network_settings: response_setting(network_settings, "ServerNetworkSettings", locale)?,
            encryption: encryption,
            encryption_settings: response_setting(encryption_settings, "ServerEncryptionSettings", locale)?,
            disable_sni: disable_sni,
            udp_relay_mode: udp_relay_mode,
            zero_rtt_handshake: zero_rtt_handshake,
            congestion_control: congestion_control,
            cipher: cipher,
            up_mbps: up_mbps,
            down_mbps: down_mbps,
            obfs: obfs,
            obfs_password: obfs_password,
            padding_scheme: response_setting(padding_scheme, "padding_scheme", locale)?,
            install_command: install_command,
        ))),
    })
}

fn route_view(route: ServerRoute) -> ServerRouteView {
    ServerRouteView {
        id: route.id,
        remarks: route.remarks,
        match_rules: route.match_rules,
        action: wire_route_action(route.action),
        action_value: route.action_value,
        created_at: Rfc3339Timestamp::from_epoch_seconds(route.created_at),
        updated_at: Rfc3339Timestamp::from_epoch_seconds(route.updated_at),
    }
}

const fn application_route_action(action: WireRouteAction) -> ServerRouteAction {
    match action {
        WireRouteAction::Block => ServerRouteAction::Block,
        WireRouteAction::BlockIp => ServerRouteAction::BlockIp,
        WireRouteAction::BlockPort => ServerRouteAction::BlockPort,
        WireRouteAction::Protocol => ServerRouteAction::Protocol,
        WireRouteAction::Dns => ServerRouteAction::Dns,
        WireRouteAction::Route => ServerRouteAction::Route,
        WireRouteAction::RouteIp => ServerRouteAction::RouteIp,
        WireRouteAction::DefaultOut => ServerRouteAction::DefaultOut,
    }
}

const fn wire_route_action(action: ServerRouteAction) -> WireRouteAction {
    match action {
        ServerRouteAction::Block => WireRouteAction::Block,
        ServerRouteAction::BlockIp => WireRouteAction::BlockIp,
        ServerRouteAction::BlockPort => WireRouteAction::BlockPort,
        ServerRouteAction::Protocol => WireRouteAction::Protocol,
        ServerRouteAction::Dns => WireRouteAction::Dns,
        ServerRouteAction::Route => WireRouteAction::Route,
        ServerRouteAction::RouteIp => WireRouteAction::RouteIp,
        ServerRouteAction::DefaultOut => WireRouteAction::DefaultOut,
    }
}

fn server_problem(error: ServerManagementError, locale: &str) -> Problem {
    let error = match error {
        ServerManagementError::InvalidInput(violation) => match violation {
            ServerInputViolation::InvalidServerType => Problem::new(Code::InvalidServerType).into(),
            ServerInputViolation::Required(field) => {
                Problem::validation_field(field, "validation.required").into()
            }
            ServerInputViolation::UnsupportedField(field) => {
                Problem::validation_field(field, "This field is not accepted for this server type")
                    .into()
            }
            ServerInputViolation::InvalidPort(field) => {
                Problem::validation_field(field, "Port must be between 1 and 65535").into()
            }
            ServerInputViolation::InvalidRate => {
                Problem::validation_field("rate", "Rate must be a finite number").into()
            }
            ServerInputViolation::InvalidGroupIds => {
                Problem::validation_field("group_id", "节点组格式不正确").into()
            }
            ServerInputViolation::EmptyGroupIds => {
                Problem::validation_field("group_id", "节点组不能为空").into()
            }
            ServerInputViolation::EmptyRouteRemarks => {
                Problem::validation_field("remarks", "备注不能为空").into()
            }
            ServerInputViolation::InvalidRouteAction => {
                Problem::validation_field("action", "动作类型参数有误").into()
            }
            ServerInputViolation::EmptyRouteMatches => {
                Problem::validation_field("match", "匹配值不能为空").into()
            }
            ServerInputViolation::InvalidTlsSettings => Problem::new(Code::InvalidParameter)
                .with_detail("TLS settings format is invalid")
                .into(),
        },
        ServerManagementError::ServerGroupNotFound => {
            Problem::new(Code::ServerGroupNotFound).into()
        }
        ServerManagementError::ServerGroupInUse(reference) => {
            let problem = Problem::new(Code::ServerGroupInUse);
            match reference {
                ServerGroupReference::Server => problem.with_detail("该组已被节点所使用，无法删除"),
                ServerGroupReference::Plan => problem.with_detail("该组已被订阅所使用，无法删除"),
                ServerGroupReference::User => problem.with_detail("该组已被用户所使用，无法删除"),
                ServerGroupReference::Unknown => problem,
            }
            .into()
        }
        ServerManagementError::RouteNotFound => Problem::new(Code::RouteNotFound).into(),
        ServerManagementError::ServerNotFound => Problem::new(Code::ServerNotFound).into(),
        ServerManagementError::Credential(ServerCredentialError::InvalidSettings) => {
            Problem::new(Code::InvalidParameter)
                .with_detail("TLS settings format is invalid")
                .into()
        }
        ServerManagementError::Credential(ServerCredentialError::Generation(error)) => {
            ApiError::internal(error)
        }
        ServerManagementError::InvalidNodeSort => {
            Problem::validation_field("nodes", "Node ids and sort values must be 32-bit integers")
                .into()
        }
        ServerManagementError::InvalidPresenceResult => {
            ApiError::internal("server presence adapter returned an invalid result")
        }
        ServerManagementError::Repository(error) => ApiError::internal(error.to_string()),
    };
    problem_from(error, locale)
}
