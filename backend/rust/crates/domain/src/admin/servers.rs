use super::*;
use serde::Deserialize;
use v2board_compat::{Code, Problem, json::double_option};

const SERVER_GROUP_LOCK_BATCH_SIZE: usize = 500;

/// Allowed `action` values for the §6.7 server-route bodies (the legacy
/// RouteController `in:` rule — the `ROUTE_ACTIONS` vocabulary is unchanged).
const ROUTE_ACTIONS: [&str; 8] = [
    "block",
    "block_ip",
    "block_port",
    "protocol",
    "dns",
    "route",
    "route_ip",
    "default_out",
];

fn parse_server_group_ids(raw: &str) -> Result<Vec<i64>, ApiError> {
    let Value::Array(values) = serde_json::from_str::<Value>(raw)
        .map_err(|_| ApiError::from(Problem::validation_field("group_id", "节点组格式不正确")))?
    else {
        return Err(ApiError::from(Problem::validation_field(
            "group_id",
            "节点组格式不正确",
        )));
    };
    let mut ids = Vec::with_capacity(values.len());
    for value in values {
        let id = value
            .as_i64()
            .or_else(|| value.as_str().and_then(|value| value.parse::<i64>().ok()))
            .filter(|id| *id > 0)
            .ok_or_else(|| {
                ApiError::from(Problem::validation_field("group_id", "节点组格式不正确"))
            })?;
        ids.push(id);
    }
    ids.sort_unstable();
    ids.dedup();
    if ids.is_empty() {
        return Err(ApiError::from(Problem::validation_field(
            "group_id",
            "节点组不能为空",
        )));
    }
    Ok(ids)
}

/// Canonicalizes a request `group_id` array into the sorted, deduplicated
/// lock list; the submitted array itself is what gets stored.
fn requested_group_lock_ids(ids: &[i64]) -> Result<Vec<i64>, ApiError> {
    if ids.iter().any(|id| *id <= 0) {
        return Err(ApiError::from(Problem::validation_field(
            "group_id",
            "节点组格式不正确",
        )));
    }
    let mut lock_ids = ids.to_vec();
    lock_ids.sort_unstable();
    lock_ids.dedup();
    if lock_ids.is_empty() {
        return Err(ApiError::from(Problem::validation_field(
            "group_id",
            "节点组不能为空",
        )));
    }
    Ok(lock_ids)
}

async fn lock_server_groups(tx: &mut DbTransaction<'_>, group_ids: &[i64]) -> Result<(), ApiError> {
    let mut found = 0_usize;
    for chunk in group_ids.chunks(SERVER_GROUP_LOCK_BATCH_SIZE) {
        let mut builder =
            QueryBuilder::<Postgres>::new("SELECT id::bigint FROM server_group WHERE id IN (");
        let mut ids = builder.separated(", ");
        for id in chunk {
            ids.push_bind(*id);
        }
        ids.push_unseparated(") ORDER BY id FOR SHARE");
        found += builder
            .build_query_scalar::<i64>()
            .fetch_all(&mut **tx)
            .await?
            .len();
    }
    if found != group_ids.len() {
        return Err(ApiError::from(Problem::new(Code::ServerGroupNotFound)));
    }
    Ok(())
}

async fn server_table_uses_group(
    tx: &mut DbTransaction<'_>,
    table: &str,
    group_id: i64,
) -> Result<bool, ApiError> {
    let sql = AssertSqlSafe(format!(
        "SELECT id::bigint FROM {table} \
         WHERE group_id @> jsonb_build_array($1::bigint)
            OR group_id @> jsonb_build_array($1::text) \
         LIMIT 1 FOR SHARE"
    ));
    Ok(sqlx::query_scalar::<_, i64>(sql)
        .bind(group_id)
        .fetch_optional(&mut **tx)
        .await?
        .is_some())
}

/// POST `server-groups` / PATCH `server-groups/{id}` (docs/api-dialect.md
/// §6.7): the one-field `{name}` body, required in both verbs.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ServerGroupBody {
    pub name: String,
}

/// POST `server-routes` (§6.7): the legacy required rule set. `match` is a
/// real JSON array (§4.1); `action_value` is the nullable extra.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RouteCreate {
    pub remarks: String,
    /// `required_unless:action,default_out` — absence is only valid for
    /// `default_out`, which `validated_route_matches` enforces.
    #[serde(default, rename = "match")]
    pub match_rules: Vec<String>,
    pub action: String,
    #[serde(default)]
    pub action_value: Option<String>,
}

/// PATCH `server-routes/{id}` (§6.7): §4.4 partial update. `remarks`,
/// `match`, and `action` are NOT NULL (set-only); `action_value` is the one
/// nullable column (double-Option).
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RoutePatch {
    #[serde(default)]
    pub remarks: Option<String>,
    #[serde(default, rename = "match")]
    pub match_rules: Option<Vec<String>>,
    #[serde(default)]
    pub action: Option<String>,
    #[serde(default, with = "double_option")]
    pub action_value: Option<Option<String>>,
}

/// POST `servers/{type}` / PATCH `servers/{type}/{id}` (§6.7): the union of
/// the eight protocol payload matrices. Which keys a given `{type}` accepts
/// is enforced per protocol against `SERVER_BODY_FIELDS`; the legacy
/// `param_present` gates map 1:1 onto the §4.4 tri-state — every nullable
/// column is a double-Option, NOT-NULL columns are plain set-only options.
/// The four vmess settings keys keep their legacy camelCase spelling on the
/// wire (R22 — a recorded, deliberate divergence from snake_case).
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ServerBody {
    #[serde(default)]
    pub group_id: Option<Vec<i64>>,
    #[serde(default, with = "double_option")]
    pub route_id: Option<Option<Vec<i64>>>,
    #[serde(default, with = "double_option")]
    pub parent_id: Option<Option<i64>>,
    #[serde(default, with = "double_option")]
    pub tags: Option<Option<Vec<String>>>,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub rate: Option<serde_json::Number>,
    #[serde(default)]
    pub host: Option<String>,
    #[serde(default)]
    pub port: Option<i64>,
    #[serde(default)]
    pub server_port: Option<i64>,
    #[serde(default)]
    pub show: Option<bool>,
    #[serde(default)]
    pub rotate_credential: Option<bool>,
    #[serde(default, with = "double_option")]
    pub cipher: Option<Option<String>>,
    #[serde(default, with = "double_option")]
    pub obfs: Option<Option<String>>,
    #[serde(default, with = "double_option")]
    pub obfs_settings: Option<Option<Value>>,
    #[serde(default, with = "double_option")]
    pub obfs_password: Option<Option<String>>,
    #[serde(default)]
    pub network: Option<String>,
    #[serde(default, with = "double_option")]
    pub network_settings: Option<Option<Value>>,
    #[serde(default)]
    pub allow_insecure: Option<bool>,
    #[serde(default, with = "double_option")]
    pub server_name: Option<Option<String>>,
    #[serde(default)]
    pub tls: Option<i64>,
    #[serde(default, with = "double_option")]
    pub tls_settings: Option<Option<Value>>,
    #[serde(default, with = "double_option", rename = "networkSettings")]
    pub vmess_network_settings: Option<Option<Value>>,
    #[serde(default, with = "double_option", rename = "tlsSettings")]
    pub vmess_tls_settings: Option<Option<Value>>,
    #[serde(default, with = "double_option", rename = "ruleSettings")]
    pub vmess_rule_settings: Option<Option<Value>>,
    #[serde(default, with = "double_option", rename = "dnsSettings")]
    pub vmess_dns_settings: Option<Option<Value>>,
    #[serde(default)]
    pub insecure: Option<bool>,
    #[serde(default)]
    pub disable_sni: Option<bool>,
    #[serde(default, with = "double_option")]
    pub udp_relay_mode: Option<Option<String>>,
    #[serde(default)]
    pub zero_rtt_handshake: Option<bool>,
    #[serde(default, with = "double_option")]
    pub congestion_control: Option<Option<String>>,
    #[serde(default)]
    pub version: Option<i64>,
    #[serde(default, with = "double_option")]
    pub up_mbps: Option<Option<i64>>,
    #[serde(default, with = "double_option")]
    pub down_mbps: Option<Option<i64>>,
    #[serde(default, with = "double_option")]
    pub flow: Option<Option<String>>,
    #[serde(default, with = "double_option")]
    pub encryption: Option<Option<String>>,
    #[serde(default, with = "double_option")]
    pub encryption_settings: Option<Option<Value>>,
    #[serde(default, with = "double_option")]
    pub sort: Option<Option<i64>>,
    #[serde(default, with = "double_option")]
    pub padding_scheme: Option<Option<Value>>,
    #[serde(default)]
    pub listen_ip: Option<String>,
    #[serde(default)]
    pub protocol: Option<String>,
}

/// The wire keys every protocol accepts (the legacy common rule set plus the
/// `rotate_credential` action flag).
const COMMON_BODY_FIELDS: &[&str] = &[
    "group_id",
    "route_id",
    "parent_id",
    "tags",
    "name",
    "rate",
    "host",
    "port",
    "server_port",
    "show",
    "rotate_credential",
];

/// Per-protocol extra wire keys — the eight §6.7 payload matrices, mirroring
/// each legacy `Server*Save` rule set (vmess keeps its camelCase keys, R22).
const SERVER_BODY_FIELDS: &[(&str, &[&str])] = &[
    ("shadowsocks", &["cipher", "obfs", "obfs_settings"]),
    (
        "trojan",
        &[
            "network",
            "network_settings",
            "allow_insecure",
            "server_name",
        ],
    ),
    (
        "vmess",
        &[
            "tls",
            "network",
            "networkSettings",
            "tlsSettings",
            "ruleSettings",
            "dnsSettings",
        ],
    ),
    (
        "tuic",
        &[
            "server_name",
            "insecure",
            "disable_sni",
            "udp_relay_mode",
            "zero_rtt_handshake",
            "congestion_control",
        ],
    ),
    (
        "hysteria",
        &[
            "version",
            "up_mbps",
            "down_mbps",
            "obfs",
            "obfs_password",
            "server_name",
            "insecure",
        ],
    ),
    (
        "vless",
        &[
            "tls",
            "tls_settings",
            "flow",
            "network",
            "network_settings",
            "encryption",
            "encryption_settings",
            "sort",
        ],
    ),
    ("anytls", &["server_name", "insecure", "padding_scheme"]),
    (
        "v2node",
        &[
            "listen_ip",
            "protocol",
            "tls",
            "tls_settings",
            "flow",
            "network",
            "network_settings",
            "encryption",
            "encryption_settings",
            "disable_sni",
            "udp_relay_mode",
            "zero_rtt_handshake",
            "congestion_control",
            "cipher",
            "up_mbps",
            "down_mbps",
            "obfs",
            "obfs_password",
            "padding_scheme",
            "sort",
        ],
    ),
];

impl ServerBody {
    /// Wire keys present in this request, for the per-protocol matrix check.
    fn present_fields(&self) -> Vec<&'static str> {
        let mut fields = Vec::new();
        let mut add = |present: bool, name: &'static str| {
            if present {
                fields.push(name);
            }
        };
        add(self.group_id.is_some(), "group_id");
        add(self.route_id.is_some(), "route_id");
        add(self.parent_id.is_some(), "parent_id");
        add(self.tags.is_some(), "tags");
        add(self.name.is_some(), "name");
        add(self.rate.is_some(), "rate");
        add(self.host.is_some(), "host");
        add(self.port.is_some(), "port");
        add(self.server_port.is_some(), "server_port");
        add(self.show.is_some(), "show");
        add(self.rotate_credential.is_some(), "rotate_credential");
        add(self.cipher.is_some(), "cipher");
        add(self.obfs.is_some(), "obfs");
        add(self.obfs_settings.is_some(), "obfs_settings");
        add(self.obfs_password.is_some(), "obfs_password");
        add(self.network.is_some(), "network");
        add(self.network_settings.is_some(), "network_settings");
        add(self.allow_insecure.is_some(), "allow_insecure");
        add(self.server_name.is_some(), "server_name");
        add(self.tls.is_some(), "tls");
        add(self.tls_settings.is_some(), "tls_settings");
        add(self.vmess_network_settings.is_some(), "networkSettings");
        add(self.vmess_tls_settings.is_some(), "tlsSettings");
        add(self.vmess_rule_settings.is_some(), "ruleSettings");
        add(self.vmess_dns_settings.is_some(), "dnsSettings");
        add(self.insecure.is_some(), "insecure");
        add(self.disable_sni.is_some(), "disable_sni");
        add(self.udp_relay_mode.is_some(), "udp_relay_mode");
        add(self.zero_rtt_handshake.is_some(), "zero_rtt_handshake");
        add(self.congestion_control.is_some(), "congestion_control");
        add(self.version.is_some(), "version");
        add(self.up_mbps.is_some(), "up_mbps");
        add(self.down_mbps.is_some(), "down_mbps");
        add(self.flow.is_some(), "flow");
        add(self.encryption.is_some(), "encryption");
        add(self.encryption_settings.is_some(), "encryption_settings");
        add(self.sort.is_some(), "sort");
        add(self.padding_scheme.is_some(), "padding_scheme");
        add(self.listen_ip.is_some(), "listen_ip");
        add(self.protocol.is_some(), "protocol");
        fields
    }
}

/// Rejects wire keys outside the protocol's payload matrix, so a field typo
/// or cross-protocol key is a 422 instead of a silent write to a column the
/// legacy rule set never accepted.
fn validate_protocol_fields(kind: &str, body: &ServerBody) -> Result<(), ApiError> {
    let extras = SERVER_BODY_FIELDS
        .iter()
        .find(|(item, _)| *item == kind)
        .map(|(_, fields)| *fields)
        .ok_or_else(|| ApiError::from(Problem::new(Code::InvalidServerType)))?;
    for field in body.present_fields() {
        if !COMMON_BODY_FIELDS.contains(&field) && !extras.contains(&field) {
            return Err(validation_error(
                field,
                "This field is not accepted for this server type",
            ));
        }
    }
    Ok(())
}

/// POST (create: required fields enforced, legacy defaults applied) vs PATCH
/// (§4.4: absent retains, null clears, value sets).
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum ValueMode {
    Create,
    Patch,
}

type ServerValues = Vec<(&'static str, AdminSqlValue)>;

fn required_missing(field: &str) -> ApiError {
    validation_error(field, "validation.required")
}

/// Pushes a NOT NULL text column: required on create, set-only on patch.
fn set_required_text(
    values: &mut ServerValues,
    column: &'static str,
    field: &Option<String>,
    mode: ValueMode,
) -> Result<(), ApiError> {
    match (field, mode) {
        (Some(value), _) => {
            values.push((column, text_value(value.clone())));
            Ok(())
        }
        (None, ValueMode::Create) => Err(required_missing(column)),
        (None, ValueMode::Patch) => Ok(()),
    }
}

/// Pushes a nullable text column from its double-Option tri-state.
fn set_nullable_text(
    values: &mut ServerValues,
    column: &'static str,
    field: &Option<Option<String>>,
) {
    if let Some(value) = field {
        values.push((column, optional_text(value.clone())));
    }
}

/// Pushes a nullable JSON column from its double-Option tri-state.
fn set_nullable_json(
    values: &mut ServerValues,
    column: &'static str,
    field: &Option<Option<Value>>,
) {
    if let Some(value) = field {
        values.push((column, AdminSqlValue::Json(value.clone())));
    }
}

/// Pushes a NOT NULL boolean flag column: the legacy controllers always
/// assigned these with a default, so create applies the default; patch is
/// present-gated.
fn set_bool_flag(
    values: &mut ServerValues,
    column: &'static str,
    field: Option<bool>,
    default: bool,
    mode: ValueMode,
) {
    match (field, mode) {
        (Some(value), _) => values.push((column, AdminSqlValue::Integer(i64::from(value)))),
        (None, ValueMode::Create) => {
            values.push((column, AdminSqlValue::Integer(i64::from(default))));
        }
        (None, ValueMode::Patch) => {}
    }
}

/// Pushes a NOT NULL integer column the legacy controllers always assigned
/// with a default.
fn set_int_default(
    values: &mut ServerValues,
    column: &'static str,
    field: Option<i64>,
    default: i64,
    mode: ValueMode,
) {
    match (field, mode) {
        (Some(value), _) => values.push((column, AdminSqlValue::Integer(value))),
        (None, ValueMode::Create) => values.push((column, AdminSqlValue::Integer(default))),
        (None, ValueMode::Patch) => {}
    }
}

/// Double-Option integer over an INTEGER NOT NULL column (§4.4): an explicit
/// JSON `null` clears back to the legacy always-assigned default; absence
/// retains on PATCH and applies the default on create.
fn set_int_clear_default(
    values: &mut ServerValues,
    column: &'static str,
    field: Option<Option<i64>>,
    default: i64,
    mode: ValueMode,
) {
    set_int_default(
        values,
        column,
        field.map(|value| value.unwrap_or(default)),
        default,
        mode,
    );
}

fn validated_port(field: Option<i64>, column: &'static str) -> Result<Option<i64>, ApiError> {
    match field {
        Some(port) if (1..=65_535).contains(&port) => Ok(Some(port)),
        Some(_) => Err(validation_error(column, "Port must be between 1 and 65535")),
        None => Ok(None),
    }
}

/// The common §6.7 columns shared by every protocol. Returns the sorted
/// deduplicated group-id lock list (empty when a PATCH omits `group_id`).
fn push_common_values(
    values: &mut ServerValues,
    kind: &str,
    body: &ServerBody,
    mode: ValueMode,
) -> Result<Vec<i64>, ApiError> {
    let group_ids = match (&body.group_id, mode) {
        (Some(ids), _) => {
            let lock_ids = requested_group_lock_ids(ids)?;
            values.push(("group_id", AdminSqlValue::Json(Some(json!(ids)))));
            lock_ids
        }
        (None, ValueMode::Create) => return Err(required_missing("group_id")),
        (None, ValueMode::Patch) => Vec::new(),
    };
    set_required_text(values, "name", &body.name, mode)?;
    match (&body.rate, mode) {
        (Some(rate), _) => values.push(("rate", text_value(rate.to_string()))),
        (None, ValueMode::Create) => return Err(required_missing("rate")),
        (None, ValueMode::Patch) => {}
    }
    set_required_text(values, "host", &body.host, mode)?;
    match (validated_port(body.port, "port")?, mode) {
        (Some(port), _) => {
            // Unlike every other protocol table, PostgreSQL stores VLESS
            // `port` as INTEGER; the rest keep the legacy VARCHAR.
            if kind == "vless" {
                values.push(("port", AdminSqlValue::Integer(port)));
            } else {
                values.push(("port", text_value(port.to_string())));
            }
        }
        (None, ValueMode::Create) => return Err(required_missing("port")),
        (None, ValueMode::Patch) => {}
    }
    match (validated_port(body.server_port, "server_port")?, mode) {
        (Some(port), _) => values.push(("server_port", AdminSqlValue::Integer(port))),
        (None, ValueMode::Create) => return Err(required_missing("server_port")),
        (None, ValueMode::Patch) => {}
    }
    if let Some(route_id) = &body.route_id {
        values.push((
            "route_id",
            AdminSqlValue::Json(route_id.as_ref().map(|ids| json!(ids))),
        ));
    }
    if let Some(parent_id) = &body.parent_id {
        values.push((
            "parent_id",
            parent_id.map_or(AdminSqlValue::IntegerNull, AdminSqlValue::Integer),
        ));
    }
    if let Some(tags) = &body.tags {
        values.push(("tags", AdminSqlValue::Json(tags.as_ref().map(|t| json!(t)))));
    }
    if let Some(show) = body.show {
        values.push(("show", AdminSqlValue::Integer(i64::from(show))));
    }
    Ok(group_ids)
}

/// The legacy hysteria/v2node obfs↔obfs_password coupling under §4.4: a
/// request that touches `obfs` recomputes the password exactly like the
/// legacy save (obfs off forces NULL, obfs on takes the submitted password or
/// generates one); a PATCH touching only `obfs_password` is a plain tri-state
/// write; create always assigns the column (legacy parity).
fn push_obfs_password(values: &mut ServerValues, body: &ServerBody, mode: ValueMode) {
    let obfs_touched = body.obfs.is_some();
    let password_touched = body.obfs_password.is_some();
    if mode == ValueMode::Patch && !obfs_touched && !password_touched {
        return;
    }
    if obfs_touched || mode == ValueMode::Create {
        let obfs_on = body
            .obfs
            .as_ref()
            .and_then(|value| value.as_deref())
            .is_some_and(|value| !value.trim().is_empty());
        if !obfs_on {
            values.push(("obfs_password", AdminSqlValue::TextNull));
            return;
        }
        let provided = body
            .obfs_password
            .clone()
            .flatten()
            .filter(|value| !value.trim().is_empty());
        values.push((
            "obfs_password",
            AdminSqlValue::Text(provided.unwrap_or_else(|| server_key(Utc::now().timestamp(), 16))),
        ));
        return;
    }
    set_nullable_text(values, "obfs_password", &body.obfs_password);
}

/// The vless/v2node tls_settings write gate: written when the request pins
/// reality (`tls == 2`, generating missing keys) or carries the field. An
/// explicit non-reality `null` clears the column; v2node values additionally
/// run the ECH preparation.
fn push_tls_settings(
    values: &mut ServerValues,
    body: &ServerBody,
    tls: Option<i64>,
    v2node: bool,
) -> Result<(), ApiError> {
    let reality = tls == Some(2);
    if !reality && body.tls_settings.is_none() {
        return Ok(());
    }
    let input = body.tls_settings.as_ref().and_then(|value| value.as_ref());
    if !reality && input.is_none() {
        values.push(("tls_settings", AdminSqlValue::Json(None)));
        return Ok(());
    }
    let tls = tls.unwrap_or_default();
    let prepared = if v2node {
        prepare_v2node_tls_settings(input, tls)?
    } else {
        prepare_tls_settings(input, tls)?
    };
    values.push(("tls_settings", json_value(prepared)));
    Ok(())
}

/// The vless/v2node encryption_settings write gate: mlkem768x25519plus in the
/// same request forces the write (generating the keypair); otherwise the
/// field's own tri-state applies.
fn push_encryption_settings(
    values: &mut ServerValues,
    body: &ServerBody,
    v2node: bool,
) -> Result<(), ApiError> {
    let encryption = body.encryption.as_ref().and_then(|value| value.as_deref());
    let mlkem = encryption == Some("mlkem768x25519plus");
    if !mlkem && body.encryption_settings.is_none() {
        return Ok(());
    }
    let input = body
        .encryption_settings
        .as_ref()
        .and_then(|value| value.as_ref());
    if !mlkem && input.is_none() {
        values.push(("encryption_settings", AdminSqlValue::Json(None)));
        return Ok(());
    }
    values.push((
        "encryption_settings",
        json_value(prepare_encryption_settings(input, encryption, v2node)?),
    ));
    Ok(())
}

/// The network_settings tri-state with the legacy value hygiene, keyed on the
/// `network` submitted in the same request (a PATCH that omits `network`
/// skips the xhttp normalization).
fn push_network_settings(values: &mut ServerValues, body: &ServerBody, v2node: bool) {
    if let Some(entry) = &body.network_settings {
        match entry {
            Some(settings) => values.push((
                "network_settings",
                json_value(prepare_network_settings(
                    settings,
                    body.network.as_deref(),
                    v2node,
                )),
            )),
            None => values.push(("network_settings", AdminSqlValue::Json(None))),
        }
    }
}

/// The vless/v2node flow forcing: a request that sets a non-tcp network
/// forces `flow` NULL (v2node additionally requires a non-mlkem encryption in
/// the same request); otherwise the field's own tri-state applies.
fn push_flow(values: &mut ServerValues, body: &ServerBody, force_null: bool) {
    if force_null {
        values.push(("flow", AdminSqlValue::TextNull));
    } else if let Some(flow) = &body.flow {
        values.push(("flow", optional_text(flow.clone())));
    }
}

/// Ports the V2nodeController tls forcing: `anytls` with tls 0 and the
/// hysteria2/trojan/tuic protocols pin tls to 1.
fn v2node_effective_tls(requested: i64, protocol: &str) -> i64 {
    if (protocol == "anytls" && requested == 0)
        || matches!(protocol, "hysteria2" | "trojan" | "tuic")
    {
        1
    } else {
        requested
    }
}

/// Builds the column writes for one §6.7 create/patch request. Returns the
/// values plus the group-id lock list. Each protocol arm mirrors its legacy
/// `Server*Save` rule set with `param_present` gates mapped 1:1 onto the
/// §4.4 tri-state.
pub(super) fn build_server_values(
    kind: &str,
    body: &ServerBody,
    mode: ValueMode,
) -> Result<(ServerValues, Vec<i64>), ApiError> {
    validate_protocol_fields(kind, body)?;
    let mut values = Vec::new();
    let group_ids = push_common_values(&mut values, kind, body, mode)?;
    match kind {
        "shadowsocks" => {
            match (&body.cipher, mode) {
                (Some(Some(cipher)), _) => values.push(("cipher", text_value(cipher.clone()))),
                (Some(None), _) | (None, ValueMode::Create) => {
                    return Err(required_missing("cipher"));
                }
                (None, ValueMode::Patch) => {}
            }
            set_nullable_text(&mut values, "obfs", &body.obfs);
            set_nullable_json(&mut values, "obfs_settings", &body.obfs_settings);
        }
        "trojan" => {
            set_required_text(&mut values, "network", &body.network, mode)?;
            push_network_settings(&mut values, body, false);
            if let Some(allow_insecure) = body.allow_insecure {
                values.push((
                    "allow_insecure",
                    AdminSqlValue::Integer(i64::from(allow_insecure)),
                ));
            }
            set_nullable_text(&mut values, "server_name", &body.server_name);
        }
        "vmess" => {
            // ServerVmessSave has no `rules` rule, so the legacy `rules`
            // column is never written by create/patch.
            set_int_default(&mut values, "tls", body.tls, 0, mode);
            set_required_text(&mut values, "network", &body.network, mode)?;
            set_nullable_json(&mut values, "networkSettings", &body.vmess_network_settings);
            set_nullable_json(&mut values, "tlsSettings", &body.vmess_tls_settings);
            set_nullable_json(&mut values, "ruleSettings", &body.vmess_rule_settings);
            set_nullable_json(&mut values, "dnsSettings", &body.vmess_dns_settings);
        }
        "tuic" => {
            set_nullable_text(&mut values, "server_name", &body.server_name);
            set_bool_flag(&mut values, "insecure", body.insecure, false, mode);
            set_bool_flag(&mut values, "disable_sni", body.disable_sni, false, mode);
            set_nullable_text(&mut values, "udp_relay_mode", &body.udp_relay_mode);
            set_bool_flag(
                &mut values,
                "zero_rtt_handshake",
                body.zero_rtt_handshake,
                false,
                mode,
            );
            set_nullable_text(&mut values, "congestion_control", &body.congestion_control);
        }
        "hysteria" => {
            set_int_default(&mut values, "version", body.version, 2, mode);
            set_int_clear_default(&mut values, "up_mbps", body.up_mbps, 0, mode);
            set_int_clear_default(&mut values, "down_mbps", body.down_mbps, 0, mode);
            set_nullable_text(&mut values, "obfs", &body.obfs);
            push_obfs_password(&mut values, body, mode);
            set_nullable_text(&mut values, "server_name", &body.server_name);
            set_bool_flag(&mut values, "insecure", body.insecure, false, mode);
        }
        "vless" => {
            if body.network.is_none() && mode == ValueMode::Create {
                return Err(required_missing("network"));
            }
            let tls = match mode {
                ValueMode::Create => Some(body.tls.unwrap_or_default()),
                ValueMode::Patch => body.tls,
            };
            if let Some(tls) = tls {
                values.push(("tls", AdminSqlValue::Integer(tls)));
            }
            push_tls_settings(&mut values, body, tls, false)?;
            push_flow(
                &mut values,
                body,
                body.network.as_deref().is_some_and(|net| net != "tcp"),
            );
            if let Some(network) = &body.network {
                values.push(("network", text_value(network.clone())));
            }
            push_network_settings(&mut values, body, false);
            if let Some(encryption) = &body.encryption {
                values.push(("encryption", optional_text(encryption.clone())));
            }
            push_encryption_settings(&mut values, body, false)?;
            if let Some(sort) = &body.sort {
                values.push((
                    "sort",
                    sort.map_or(AdminSqlValue::IntegerNull, AdminSqlValue::Integer),
                ));
            }
        }
        "anytls" => {
            set_nullable_text(&mut values, "server_name", &body.server_name);
            set_bool_flag(&mut values, "insecure", body.insecure, false, mode);
            set_nullable_json(&mut values, "padding_scheme", &body.padding_scheme);
        }
        "v2node" => {
            push_v2node_values(&mut values, body, mode)?;
        }
        _ => return Err(Problem::new(Code::InvalidServerType).into()),
    }
    Ok((values, group_ids))
}

fn push_v2node_values(
    values: &mut ServerValues,
    body: &ServerBody,
    mode: ValueMode,
) -> Result<(), ApiError> {
    if let Some(listen_ip) = &body.listen_ip {
        values.push(("listen_ip", text_value(listen_ip.clone())));
    }
    let protocol = match (&body.protocol, mode) {
        (Some(protocol), _) => {
            values.push(("protocol", text_value(protocol.clone())));
            Some(protocol.as_str())
        }
        (None, ValueMode::Create) => return Err(required_missing("protocol")),
        (None, ValueMode::Patch) => None,
    };
    if body.network.is_none() && mode == ValueMode::Create {
        return Err(required_missing("network"));
    }
    // tls is written when the request carries it or the submitted protocol
    // forces it; a protocol-only PATCH that does not force tls retains the
    // stored value.
    let tls = match (protocol, body.tls, mode) {
        (Some(protocol), requested, ValueMode::Create) => Some(v2node_effective_tls(
            requested.unwrap_or_default(),
            protocol,
        )),
        (Some(protocol), Some(requested), ValueMode::Patch) => {
            Some(v2node_effective_tls(requested, protocol))
        }
        (Some(protocol), None, ValueMode::Patch) => {
            (v2node_effective_tls(0, protocol) == 1).then_some(1)
        }
        (None, requested, _) => requested,
    };
    if let Some(tls) = tls {
        values.push(("tls", AdminSqlValue::Integer(tls)));
    }
    push_tls_settings(values, body, tls, true)?;
    let encryption = body.encryption.as_ref().and_then(|value| value.as_deref());
    // Laravel only nulls flow when encryption is *present* and not mlkem
    // (V2nodeController.php: `... && isset($params['encryption']) && ...`).
    let force_flow_null = body.network.as_deref().is_some_and(|net| net != "tcp")
        && encryption.is_some()
        && encryption != Some("mlkem768x25519plus");
    push_flow(values, body, force_flow_null);
    if let Some(network) = &body.network {
        values.push(("network", text_value(network.clone())));
    }
    push_network_settings(values, body, true);
    if let Some(encryption_entry) = &body.encryption {
        values.push(("encryption", optional_text(encryption_entry.clone())));
    }
    push_encryption_settings(values, body, true)?;
    set_bool_flag(values, "disable_sni", body.disable_sni, false, mode);
    set_nullable_text(values, "udp_relay_mode", &body.udp_relay_mode);
    set_bool_flag(
        values,
        "zero_rtt_handshake",
        body.zero_rtt_handshake,
        false,
        mode,
    );
    set_nullable_text(values, "congestion_control", &body.congestion_control);
    // cipher defaults to aes-128-gcm when the request pins the shadowsocks
    // protocol; otherwise its own tri-state applies.
    let protocol_is_shadowsocks = protocol == Some("shadowsocks");
    if protocol_is_shadowsocks || body.cipher.is_some() {
        let cipher = body
            .cipher
            .clone()
            .flatten()
            .filter(|value| !value.trim().is_empty())
            .or_else(|| protocol_is_shadowsocks.then(|| "aes-128-gcm".to_string()));
        values.push(("cipher", optional_text(cipher)));
    }
    set_int_clear_default(values, "up_mbps", body.up_mbps, 0, mode);
    set_int_clear_default(values, "down_mbps", body.down_mbps, 0, mode);
    set_nullable_text(values, "obfs", &body.obfs);
    push_obfs_password(values, body, mode);
    set_nullable_json(values, "padding_scheme", &body.padding_scheme);
    if let Some(sort) = &body.sort {
        values.push((
            "sort",
            sort.map_or(AdminSqlValue::IntegerNull, AdminSqlValue::Integer),
        ));
    }
    Ok(())
}

/// PHP array_filter parity for route match entries: '' and '0' are dropped.
fn filtered_match_rules(rules: &[String]) -> Vec<String> {
    rules
        .iter()
        .filter(|rule| !rule.is_empty() && rule.as_str() != "0")
        .cloned()
        .collect()
}

/// RouteController::save parity: the `ROUTE_ACTIONS` vocabulary is closed,
/// `default_out` forces an empty match set, and every other action requires a
/// non-empty raw match array (`required_unless`). Laravel's `required` counts
/// a non-empty array even when every entry is falsy — `array_filter` drops
/// those afterwards, so an all-falsy submission stores an empty match set.
fn validated_route_matches(action: &str, rules: &[String]) -> Result<Vec<String>, ApiError> {
    if !ROUTE_ACTIONS.contains(&action) {
        return Err(validation_error("action", "动作类型参数有误"));
    }
    if action == "default_out" {
        return Ok(Vec::new());
    }
    if rules.is_empty() {
        return Err(validation_error("match", "匹配值不能为空"));
    }
    Ok(filtered_match_rules(rules))
}

/// §4.1 flag columns that cross as JSON booleans in the GET `nodes` rows.
const NODE_BOOL_KEYS: &[&str] = &[
    "show",
    "allow_insecure",
    "insecure",
    "disable_sni",
    "zero_rtt_handshake",
];

/// Converts one stored node row to the §6.7 dialect-v2 wire shape: `0|1`
/// flags become booleans, the VARCHAR `rate`/`port` become JSON numbers, and
/// legacy string members of the id arrays are normalized to numbers. True
/// enums (`tls`, `available_status`) and the vmess camelCase settings keys
/// (R22) stay as stored.
fn modernize_node_row(node: Value) -> Value {
    let mut node = node;
    if let Some(object) = node.as_object_mut() {
        for key in NODE_BOOL_KEYS {
            if let Some(value) = object.get_mut(*key)
                && let Some(flag) = value.as_i64()
            {
                *value = Value::Bool(flag != 0);
            }
        }
        for key in ["rate", "port"] {
            if let Some(value) = object.get_mut(key)
                && let Some(text) = value.as_str()
            {
                if let Ok(int) = text.trim().parse::<i64>() {
                    *value = json!(int);
                } else if let Ok(float) = text.trim().parse::<f64>()
                    && float.is_finite()
                {
                    *value = json!(float);
                }
            }
        }
        for key in ["group_id", "route_id"] {
            if let Some(Value::Array(items)) = object.get_mut(key) {
                for item in items {
                    if let Some(text) = item.as_str()
                        && let Ok(int) = text.trim().parse::<i64>()
                    {
                        *item = json!(int);
                    }
                }
            }
        }
    }
    statistics::epoch_fields_to_rfc3339(
        node,
        &["created_at", "updated_at", "last_check_at", "last_push_at"],
    )
}

impl AdminService {
    /// Loads the raw `group_id` JSON of every configured server across all node
    /// tables, for the group `server_count` / drop-guard membership checks.
    async fn all_server_group_ids(&self) -> Result<Vec<String>, ApiError> {
        let mut group_ids = Vec::new();
        for (_, table) in SERVER_TABLES {
            let rows: Vec<String> =
                sqlx::query_scalar(AssertSqlSafe(format!("SELECT group_id::text FROM {table}")))
                    .fetch_all(&self.db)
                    .await?;
            group_ids.extend(rows);
        }
        Ok(group_ids)
    }

    /// GET `server-groups` `?group_id=` (docs/api-dialect.md §6.7): bare
    /// array, id-ascending, every row enriched with `user_count` /
    /// `server_count`. The legacy single-group short-circuit (raw row,
    /// `[null]` miss) is retired: the filter narrows the same enriched shape
    /// and a miss is an empty array.
    pub async fn server_groups_list(&self, group_id: Option<i64>) -> Result<Vec<Value>, ApiError> {
        const SELECT: &str = r#"
            SELECT jsonb_build_object(
                'id', id, 'name', name, 'created_at', created_at, 'updated_at', updated_at,
                'user_count', (SELECT COUNT(*) FROM users WHERE group_id = server_group.id),
                'server_count', 0
            )
            FROM server_group
        "#;
        let mut groups = match group_id {
            Some(group_id) => {
                fetch_json_list_bind(
                    &self.db,
                    &format!("{SELECT} WHERE id = $1 ORDER BY id ASC"),
                    group_id,
                )
                .await?
            }
            None => fetch_json_list(&self.db, &format!("{SELECT} ORDER BY id ASC")).await?,
        };
        // server_count counts nodes whose group_id array includes the group,
        // mirroring GroupController::fetch over ServerService::getAllServers.
        let group_ids = self.all_server_group_ids().await?;
        for group in &mut groups {
            let Some(object) = group.as_object_mut() else {
                continue;
            };
            let id = object.get("id").and_then(Value::as_i64).unwrap_or_default();
            let count = group_ids
                .iter()
                .filter(|group_id| group_id_contains(group_id, id))
                .count() as i64;
            object.insert("server_count".to_string(), json!(count));
        }
        Ok(groups
            .into_iter()
            .map(|group| statistics::epoch_fields_to_rfc3339(group, &["created_at", "updated_at"]))
            .collect())
    }

    /// POST `server-groups` (§6.7): 201 bare `{id}`.
    pub async fn server_group_create(&self, body: &ServerGroupBody) -> Result<i64, ApiError> {
        if body.name.trim().is_empty() {
            return Err(required_missing("name"));
        }
        let now = Utc::now().timestamp();
        let id: i32 = sqlx::query_scalar(
            "INSERT INTO server_group (name, created_at, updated_at) \
             VALUES ($1, $2, $2) RETURNING id",
        )
        .bind(&body.name)
        .bind(now)
        .fetch_one(&self.db)
        .await?;
        Ok(i64::from(id))
    }

    /// PATCH `server-groups/{id}` (§6.7): 404 `server_group_not_found` on a
    /// miss; empty 204.
    pub async fn server_group_patch(
        &self,
        id: i64,
        body: &ServerGroupBody,
    ) -> Result<(), ApiError> {
        if body.name.trim().is_empty() {
            return Err(required_missing("name"));
        }
        let result =
            sqlx::query("UPDATE server_group SET name = $1, updated_at = $2 WHERE id = $3")
                .bind(&body.name)
                .bind(Utc::now().timestamp())
                .bind(id)
                .execute(&self.db)
                .await?;
        if result.rows_affected() == 0 {
            return Err(Problem::new(Code::ServerGroupNotFound).into());
        }
        Ok(())
    }

    /// DELETE `server-groups/{id}` (§6.7). Rejects with 400
    /// `server_group_in_use` (the blocking dependency stays in `detail`)
    /// while any node, plan, or user still references the group. The group
    /// row is the serialization point: node/plan writers first take a shared
    /// group lock, so none can create a late reference after these checks and
    /// before the delete.
    pub async fn server_group_delete(&self, id: i64) -> Result<(), ApiError> {
        let mut tx = self.db.begin().await?;
        let exists: Option<i32> =
            sqlx::query_scalar("SELECT id FROM server_group WHERE id = $1 LIMIT 1 FOR UPDATE")
                .bind(id)
                .fetch_optional(&mut *tx)
                .await?;
        if exists.is_none() {
            return Err(Problem::new(Code::ServerGroupNotFound).into());
        }
        for (_, table) in SERVER_TABLES {
            if server_table_uses_group(&mut tx, table, id).await? {
                return Err(Problem::new(Code::ServerGroupInUse)
                    .with_detail("该组已被节点所使用，无法删除")
                    .into());
            }
        }
        let plan_used: Option<i32> =
            sqlx::query_scalar("SELECT id FROM plan WHERE group_id = $1 LIMIT 1 FOR SHARE")
                .bind(id)
                .fetch_optional(&mut *tx)
                .await?;
        if plan_used.is_some() {
            return Err(Problem::new(Code::ServerGroupInUse)
                .with_detail("该组已被订阅所使用，无法删除")
                .into());
        }
        let user_used: Option<i64> =
            sqlx::query_scalar("SELECT id FROM users WHERE group_id = $1 LIMIT 1 FOR SHARE")
                .bind(id)
                .fetch_optional(&mut *tx)
                .await?;
        if user_used.is_some() {
            return Err(Problem::new(Code::ServerGroupInUse)
                .with_detail("该组已被用户所使用，无法删除")
                .into());
        }
        let deleted = sqlx::query("DELETE FROM server_group WHERE id = $1")
            .bind(id)
            .execute(&mut *tx)
            .await?;
        if deleted.rows_affected() != 1 {
            return Err(Problem::new(Code::ServerGroupNotFound).into());
        }
        tx.commit().await?;
        Ok(())
    }

    /// GET `server-routes` (§6.7): bare array; `match` is always an array.
    pub async fn server_routes_list(&self) -> Result<Vec<Value>, ApiError> {
        Ok(fetch_json_list(
            &self.db,
            r#"
            SELECT jsonb_build_object(
                'id', id, 'remarks', remarks, 'match', CAST("match" AS JSONB),
                'action', action, 'action_value', action_value,
                'created_at', created_at, 'updated_at', updated_at
            )
            FROM server_route
            ORDER BY id ASC
            "#,
        )
        .await?
        .into_iter()
        .map(|route| statistics::epoch_fields_to_rfc3339(route, &["created_at", "updated_at"]))
        .collect())
    }

    /// POST `server-routes` (§6.7): 201 bare `{id}`. `default_out` forces an
    /// empty match set; everything else drops PHP-falsy match entries before
    /// storing (RouteController::save parity).
    pub async fn server_route_create(&self, body: &RouteCreate) -> Result<i64, ApiError> {
        if body.remarks.trim().is_empty() {
            return Err(validation_error("remarks", "备注不能为空"));
        }
        let matches = validated_route_matches(&body.action, &body.match_rules)?;
        let action_value = body
            .action_value
            .clone()
            .filter(|value| !value.trim().is_empty());
        let now = Utc::now().timestamp();
        let id: i32 = sqlx::query_scalar(
            "INSERT INTO server_route (remarks, \"match\", action, action_value, created_at, updated_at) \
             VALUES ($1, $2, $3, $4, $5, $5) RETURNING id",
        )
        .bind(&body.remarks)
        .bind(Json(json!(matches)))
        .bind(&body.action)
        .bind(action_value.map(|value| Json(Value::String(value))))
        .bind(now)
        .fetch_one(&self.db)
        .await?;
        Ok(i64::from(id))
    }

    /// PATCH `server-routes/{id}` (§6.7): §4.4 partial update; 404
    /// `route_not_found` on a miss.
    pub async fn server_route_patch(&self, id: i64, body: &RoutePatch) -> Result<(), ApiError> {
        let mut values = Vec::new();
        if let Some(remarks) = &body.remarks {
            if remarks.trim().is_empty() {
                return Err(validation_error("remarks", "备注不能为空"));
            }
            values.push(("remarks", text_value(remarks.clone())));
        }
        if let Some(action) = &body.action {
            if !ROUTE_ACTIONS.contains(&action.as_str()) {
                return Err(validation_error("action", "动作类型参数有误"));
            }
            values.push(("action", text_value(action.clone())));
        }
        if body.action.as_deref() == Some("default_out") {
            values.push(("match", AdminSqlValue::Json(Some(json!([])))));
        } else if let Some(rules) = &body.match_rules {
            // Same `required` + array_filter split as create: an empty raw
            // array is a 422, an all-falsy one stores an empty match set.
            if rules.is_empty() {
                return Err(validation_error("match", "匹配值不能为空"));
            }
            values.push((
                "match",
                AdminSqlValue::Json(Some(json!(filtered_match_rules(rules)))),
            ));
        }
        if let Some(action_value) = &body.action_value {
            values.push((
                "action_value",
                AdminSqlValue::Json(action_value.clone().map(Value::String)),
            ));
        }
        self.patch_row(
            "server_route",
            id,
            &values,
            Problem::new(Code::RouteNotFound).into(),
        )
        .await
    }

    /// DELETE `server-routes/{id}` (§6.7): 404 `route_not_found` on a miss.
    pub async fn server_route_delete(&self, id: i64) -> Result<(), ApiError> {
        self.delete_by_id("server_route", id, Problem::new(Code::RouteNotFound).into())
            .await
    }
}

impl AdminService {
    /// GET `nodes` (docs/api-dialect.md §6.7, step-up-gated in the handler):
    /// every protocol table's full rows plus the health-cache merge, in the
    /// dialect-v2 projection (`show` bool, `rate`/`port` numbers, RFC 3339
    /// timestamps; vmess camelCase settings keys stay — R22).
    pub async fn nodes_list(&self) -> Result<Vec<Value>, ApiError> {
        // Ports ServerService::getAllServers (:424-440): each getAll<Protocol> getter
        // returns every model column (with array casts applied) plus `type`, ordered by
        // sort; the tables are concatenated in SERVER_TABLES order and later stable-sorted.
        let mut nodes = Vec::new();
        for (kind, table) in SERVER_TABLES {
            let rows = fetch_json_list(&self.db, &server_node_select(kind, table)).await?;
            nodes.extend(rows);
        }
        // getAllV2node (:381-405) appends a node install script per v2node using the
        // node API host (server_api_url ?? app_url) and token, shell-escaped.
        let install_api_host = self
            .config
            .server_api_url
            .clone()
            .or_else(|| self.config.app_url.clone())
            .unwrap_or_default();
        let credential_rows = sqlx::query_as::<_, (String, i32, i64)>(
            "SELECT node_type, node_id, credential_epoch FROM server_credential",
        )
        .fetch_all(&self.db)
        .await?
        .into_iter()
        .map(|(node_type, node_id, epoch)| ((node_type, i64::from(node_id)), epoch))
        .collect::<HashMap<_, _>>();
        let credential_master = self.config.server_token.as_deref().unwrap_or_default();
        // Hydrate node health from the cache keys the node API writes, keyed on
        // `parent_id ?? id`. Ports ServerService::mergeData (:407-421); the read is
        // best-effort so a Redis outage still returns the node list. Fetch every
        // field with one MGET instead of issuing three sequential round trips per
        // node.
        let identities = nodes
            .iter()
            .map(|node| {
                let node_type = node
                    .get("type")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_ascii_uppercase();
                let id = node.get("id").and_then(Value::as_i64).unwrap_or_default();
                let check_id = node.get("parent_id").and_then(Value::as_i64).unwrap_or(id);
                (node_type, id, check_id)
            })
            .collect::<Vec<_>>();
        let health_keys = identities
            .iter()
            .flat_map(|(node_type, _, check_id)| {
                [
                    self.redis_key(&format!("SERVER_{node_type}_ONLINE_USER_{check_id}")),
                    self.redis_key(&format!("SERVER_{node_type}_LAST_CHECK_AT_{check_id}")),
                    self.redis_key(&format!("SERVER_{node_type}_LAST_PUSH_AT_{check_id}")),
                ]
            })
            .collect::<Vec<_>>();
        let mut health_values = vec![None; health_keys.len()];
        if !health_keys.is_empty() {
            match self.redis.get_multiplexed_async_connection().await {
                Ok(mut conn) => {
                    let mut malformed = 0_usize;
                    for (batch_index, keys) in health_keys.chunks(REDIS_MGET_BATCH_SIZE).enumerate()
                    {
                        match conn.mget::<_, Vec<Option<String>>>(keys).await {
                            Ok(values) => {
                                let offset = batch_index * REDIS_MGET_BATCH_SIZE;
                                for (index, value) in values.into_iter().enumerate() {
                                    health_values[offset + index] =
                                        value.and_then(|value| match value.parse::<i64>() {
                                            Ok(value) => Some(value),
                                            Err(_) => {
                                                malformed += 1;
                                                None
                                            }
                                        });
                                }
                            }
                            Err(error) => {
                                tracing::warn!(
                                    ?error,
                                    "admin server health-cache batch read failed"
                                );
                                break;
                            }
                        }
                    }
                    if malformed > 0 {
                        tracing::warn!(
                            malformed,
                            "admin server health cache contained invalid integers"
                        );
                    }
                }
                Err(error) => {
                    tracing::warn!(?error, "admin server health-cache connection unavailable");
                }
            }
        }
        let now = Utc::now().timestamp();
        for ((node, (node_type, id, _)), health) in nodes
            .iter_mut()
            .zip(identities)
            .zip(health_values.chunks_exact(3))
        {
            let Some(object) = node.as_object_mut() else {
                continue;
            };
            let [online, last_check_at, last_push_at] = health else {
                unreachable!("each server health cache tuple has exactly three values")
            };
            // ServerService::mergeData (:407-421) sets exactly these four cache-derived
            // fields keyed on parent_id ?? id; it does not add is_online.
            let available_status = node_available_status(now, *last_check_at, *last_push_at);
            object.insert("online".to_string(), json!(online));
            object.insert("last_check_at".to_string(), json!(last_check_at));
            object.insert("last_push_at".to_string(), json!(last_push_at));
            object.insert("available_status".to_string(), json!(available_status));
            let normalized_type = node_type.to_ascii_lowercase();
            let scoped_token = credential_rows
                .get(&(normalized_type.clone(), id))
                .and_then(|epoch| {
                    crate::server_credentials::derive_node_token(
                        credential_master,
                        &normalized_type,
                        i32::try_from(id).ok()?,
                        *epoch,
                    )
                });
            object.insert("api_key".to_string(), json!(scoped_token.as_deref()));
            if node_type == "V2NODE" {
                let install_command = format!(
                    "wget -N https://raw.githubusercontent.com/wyx2685/v2node/master/script/install.sh && bash install.sh --api-host {} --node-id {} --api-key {}",
                    escapeshellarg(&install_api_host),
                    id,
                    escapeshellarg(scoped_token.as_deref().unwrap_or_default())
                );
                object.insert("install_command".to_string(), json!(install_command));
            }
        }
        // array_multisort($tmp, SORT_ASC, $servers) over the `sort` column; PHP 8's
        // sort is stable and treats a null sort as 0, so key null -> 0 and rely on the
        // stable sort to preserve the concatenation tie order.
        nodes.sort_by_key(|node| node.get("sort").and_then(Value::as_i64).unwrap_or(0));
        Ok(nodes.into_iter().map(modernize_node_row).collect())
    }

    /// POST `nodes/sort` (§6.7): json `{<type>: {<id>: sort}}` — the legacy
    /// JSON shape kept as-is; empty 204. Unknown types and non-integer ids
    /// are 422s instead of the legacy silent skip.
    pub async fn nodes_sort(
        &self,
        body: &BTreeMap<String, BTreeMap<String, i64>>,
    ) -> Result<(), ApiError> {
        let mut updates = Vec::new();
        for (kind, entries) in body {
            let Some((_, table)) = SERVER_TABLES
                .iter()
                .find(|(item, _)| *item == kind.as_str())
            else {
                return Err(validation_error(kind, "Unknown server type"));
            };
            for (raw_id, sort) in entries {
                let id = raw_id
                    .trim()
                    .parse::<i64>()
                    .map_err(|_| validation_error(kind, "Node id must be an integer"))?;
                updates.push((*table, id, *sort));
            }
        }
        for (table, id, sort) in updates {
            sqlx::query(AssertSqlSafe(format!(
                "UPDATE {table} SET sort = CAST($1::BIGINT AS INTEGER) WHERE id = $2::BIGINT"
            )))
            .bind(sort)
            .bind(id)
            .execute(&self.db)
            .await?;
        }
        Ok(())
    }
}

impl AdminService {
    /// POST `servers/{type}` (§6.7): create; 201 bare `{id}`. Locks the
    /// referenced groups (shared) so a concurrent group delete cannot orphan
    /// the new node, then seeds the node credential at epoch 0.
    pub async fn server_create(&self, kind: &str, body: &ServerBody) -> Result<i64, ApiError> {
        let table = server_table_for_kind(kind)?;
        let (values, group_ids) = build_server_values(kind, body, ValueMode::Create)?;
        let now = Utc::now().timestamp();
        let mut tx = self.db.begin().await?;
        lock_server_groups(&mut tx, &group_ids).await?;
        let mut builder = QueryBuilder::<Postgres>::new(format!("INSERT INTO {table} ("));
        let mut columns = builder.separated(", ");
        for (column, _) in &values {
            columns.push(format!("\"{column}\""));
        }
        columns.push("\"created_at\"");
        columns.push("\"updated_at\"");
        builder.push(") VALUES (");
        let mut placeholders = builder.separated(", ");
        for (column, value) in &values {
            push_admin_sql_value(&mut placeholders, column, value);
        }
        placeholders.push_bind(now);
        placeholders.push_bind(now);
        builder.push(") RETURNING id");
        let node_id = builder
            .build_query_scalar::<i32>()
            .fetch_one(&mut *tx)
            .await?;
        self.upsert_server_credential(&mut tx, kind, node_id, body.rotate_credential == Some(true))
            .await?;
        tx.commit().await?;
        Ok(i64::from(node_id))
    }

    /// PATCH `servers/{type}/{id}` (§6.7): §4.4 partial update (the legacy
    /// `save`-with-id and the `update` show toggle merged); 404
    /// `server_not_found` on a miss; empty 204.
    pub async fn server_patch(
        &self,
        kind: &str,
        id: i64,
        body: &ServerBody,
    ) -> Result<(), ApiError> {
        let table = server_table_for_kind(kind)?;
        let (values, group_ids) = build_server_values(kind, body, ValueMode::Patch)?;
        let node_id =
            i32::try_from(id).map_err(|_| ApiError::from(Problem::new(Code::ServerNotFound)))?;
        let mut tx = self.db.begin().await?;
        if !group_ids.is_empty() {
            lock_server_groups(&mut tx, &group_ids).await?;
        }
        if values.is_empty() {
            let exists: Option<i64> = sqlx::query_scalar(AssertSqlSafe(format!(
                "SELECT id::bigint FROM {table} WHERE id = $1"
            )))
            .bind(id)
            .fetch_optional(&mut *tx)
            .await?;
            if exists.is_none() {
                return Err(Problem::new(Code::ServerNotFound).into());
            }
        } else {
            let mut builder = QueryBuilder::<Postgres>::new(format!("UPDATE {table} SET "));
            for (column, value) in &values {
                builder.push(format!("\"{column}\" = "));
                push_admin_sql_bind(&mut builder, column, value);
                builder.push(", ");
            }
            builder.push("\"updated_at\" = ");
            builder.push_bind(Utc::now().timestamp());
            builder.push(" WHERE id = ");
            builder.push_bind(id);
            let result = builder.build().execute(&mut *tx).await?;
            if result.rows_affected() == 0 {
                return Err(Problem::new(Code::ServerNotFound).into());
            }
        }
        self.upsert_server_credential(&mut tx, kind, node_id, body.rotate_credential == Some(true))
            .await?;
        tx.commit().await?;
        Ok(())
    }

    /// The save-path credential row: insert at epoch 0, bump the epoch only
    /// when the request asked for a rotation.
    async fn upsert_server_credential(
        &self,
        tx: &mut DbTransaction<'_>,
        kind: &str,
        node_id: i32,
        rotate: bool,
    ) -> Result<(), ApiError> {
        sqlx::query(
            r#"
            INSERT INTO server_credential
                (node_type, node_id, credential_epoch, updated_at)
            VALUES ($1, $2, 0, $3)
            ON CONFLICT (node_type, node_id) DO UPDATE SET
                credential_epoch = CASE WHEN $4
                    THEN server_credential.credential_epoch + 1
                    ELSE server_credential.credential_epoch END,
                updated_at = EXCLUDED.updated_at
            "#,
        )
        .bind(kind)
        .bind(node_id)
        .bind(Utc::now().timestamp())
        .bind(rotate)
        .execute(&mut **tx)
        .await?;
        Ok(())
    }

    /// DELETE `servers/{type}/{id}` (§6.7): 404 `server_not_found` on a
    /// miss; drops the node credential with the row.
    pub async fn server_delete(&self, kind: &str, id: i64) -> Result<(), ApiError> {
        let table = server_table_for_kind(kind)?;
        let mut tx = self.db.begin().await?;
        let result = sqlx::query(AssertSqlSafe(format!("DELETE FROM {table} WHERE id = $1")))
            .bind(id)
            .execute(&mut *tx)
            .await?;
        if result.rows_affected() == 0 {
            return Err(Problem::new(Code::ServerNotFound).into());
        }
        sqlx::query("DELETE FROM server_credential WHERE node_type = $1 AND node_id = $2")
            .bind(kind)
            .bind(id)
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        Ok(())
    }

    /// POST `servers/{type}/{id}/copy` (§6.7): 201 bare `{id}` of the new
    /// copy (the legacy copy returned no id). The copy starts hidden and
    /// keeps the source timestamps, exactly like the legacy replicate.
    pub async fn server_copy(&self, kind: &str, id: i64) -> Result<i64, ApiError> {
        let table = server_table_for_kind(kind)?;
        let columns = server_copy_columns(kind)?;
        let source_group_ids: Option<String> = sqlx::query_scalar(AssertSqlSafe(format!(
            "SELECT group_id::text FROM {table} WHERE id = $1 LIMIT 1"
        )))
        .bind(id)
        .fetch_optional(&self.db)
        .await?;
        let source_group_ids =
            source_group_ids.ok_or_else(|| ApiError::from(Problem::new(Code::ServerNotFound)))?;
        let group_ids = parse_server_group_ids(&source_group_ids)?;
        let mut builder = QueryBuilder::<Postgres>::new(format!("INSERT INTO {table} ("));
        let mut insert_columns = builder.separated(", ");
        for column in columns {
            insert_columns.push(format!("\"{column}\""));
        }
        insert_columns.push("\"created_at\"");
        insert_columns.push("\"updated_at\"");
        builder.push(") SELECT ");
        let mut select_columns = builder.separated(", ");
        for column in columns {
            if *column == "show" {
                select_columns.push("0::SMALLINT");
            } else {
                select_columns.push(format!("\"{column}\""));
            }
        }
        // Laravel's copy replicates the row via create($server->toArray()): because
        // created_at/updated_at are fillable (guarded = ['id']) they are set from the
        // source row, so updateTimestamps() leaves them untouched. Preserve the
        // original timestamps rather than stamping now().
        select_columns.push("\"created_at\"");
        select_columns.push("\"updated_at\"");
        builder.push(format!(" FROM {table} WHERE id = "));
        builder.push_bind(id);
        builder.push(" AND group_id = ");
        builder.push_bind(Json(
            serde_json::from_str::<Value>(&source_group_ids)
                .map_err(|_| ApiError::internal("stored server group_id is invalid"))?,
        ));
        builder.push(" RETURNING id");
        let mut tx = self.db.begin().await?;
        lock_server_groups(&mut tx, &group_ids).await?;
        let node_id = builder
            .build_query_scalar::<i32>()
            .fetch_optional(&mut *tx)
            .await?
            .ok_or_else(|| ApiError::from(Problem::new(Code::ServerNotFound)))?;
        sqlx::query(
            "INSERT INTO server_credential \
             (node_type, node_id, credential_epoch, updated_at) VALUES ($1, $2, 0, $3)",
        )
        .bind(kind)
        .bind(node_id)
        .bind(Utc::now().timestamp())
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(i64::from(node_id))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn node_group_references_are_nonempty_positive_and_canonicalized() {
        assert_eq!(parse_server_group_ids(r#"[3,"1",3]"#).unwrap(), vec![1, 3]);
        for invalid in ["[]", "{}", "1", r#"["missing"]"#, "[0]", "[-1]"] {
            assert!(
                parse_server_group_ids(invalid).is_err(),
                "accepted {invalid}"
            );
        }
        assert_eq!(requested_group_lock_ids(&[3, 1, 3]).unwrap(), vec![1, 3]);
        assert!(requested_group_lock_ids(&[]).is_err());
        assert!(requested_group_lock_ids(&[0]).is_err());
        assert!(requested_group_lock_ids(&[-2]).is_err());
    }

    fn body(value: Value) -> ServerBody {
        serde_json::from_value(value).expect("valid server body")
    }

    fn patch_values(kind: &str, value: Value) -> ServerValues {
        build_server_values(kind, &body(value), ValueMode::Patch)
            .expect("patch body must build")
            .0
    }

    fn create_values(kind: &str, value: Value) -> ServerValues {
        build_server_values(kind, &body(value), ValueMode::Create)
            .expect("create body must build")
            .0
    }

    fn columns(values: &ServerValues) -> Vec<&'static str> {
        values.iter().map(|(column, _)| *column).collect()
    }

    fn value_of<'a>(values: &'a ServerValues, column: &str) -> &'a AdminSqlValue {
        values
            .iter()
            .find(|(item, _)| *item == column)
            .map(|(_, value)| value)
            .unwrap_or_else(|| panic!("column {column} missing"))
    }

    fn is_null(value: &AdminSqlValue) -> bool {
        matches!(
            value,
            AdminSqlValue::TextNull | AdminSqlValue::IntegerNull | AdminSqlValue::Json(None)
        )
    }

    /// A minimal valid create body per protocol.
    fn create_base(kind: &str) -> Value {
        let mut base = json!({
            "group_id": [1],
            "name": "node",
            "rate": 1,
            "host": "example.test",
            "port": 443,
            "server_port": 443,
        });
        let object = base.as_object_mut().unwrap();
        match kind {
            "shadowsocks" => {
                object.insert("cipher".into(), json!("aes-128-gcm"));
            }
            "trojan" | "vmess" | "vless" => {
                object.insert("network".into(), json!("tcp"));
            }
            "v2node" => {
                object.insert("protocol".into(), json!("vless"));
                object.insert("network".into(), json!("tcp"));
            }
            _ => {}
        }
        base
    }

    fn sample(field: &str) -> Value {
        match field {
            "route_id" => json!([1, 2]),
            "parent_id" | "sort" => json!(7),
            "tags" => json!(["edge"]),
            "obfs" => json!("salamander"),
            "obfs_password" => json!("obfs-secret"),
            "server_name" => json!("sni.example.test"),
            "listen_ip" => json!("127.0.0.1"),
            "udp_relay_mode" => json!("native"),
            "congestion_control" => json!("bbr"),
            "flow" => json!("xtls-rprx-vision"),
            "encryption" => json!("none"),
            "cipher" => json!("aes-128-gcm"),
            "padding_scheme" => json!(["30-30"]),
            _ => json!({"key": "value"}),
        }
    }

    /// §4.4 double-Option parity per legacy `param_present` gate: for every
    /// nullable field of every protocol matrix, absent retains (no write),
    /// null clears (NULL write), value sets.
    #[test]
    fn patch_tri_state_matches_every_protocol_param_present_gate() {
        let matrix: &[(&str, &[(&str, &str)])] = &[
            (
                "shadowsocks",
                &[
                    ("route_id", "route_id"),
                    ("parent_id", "parent_id"),
                    ("tags", "tags"),
                    ("obfs", "obfs"),
                    ("obfs_settings", "obfs_settings"),
                ],
            ),
            (
                "trojan",
                &[
                    ("network_settings", "network_settings"),
                    ("server_name", "server_name"),
                ],
            ),
            (
                "vmess",
                &[
                    ("networkSettings", "networkSettings"),
                    ("tlsSettings", "tlsSettings"),
                    ("ruleSettings", "ruleSettings"),
                    ("dnsSettings", "dnsSettings"),
                ],
            ),
            (
                "tuic",
                &[
                    ("server_name", "server_name"),
                    ("udp_relay_mode", "udp_relay_mode"),
                    ("congestion_control", "congestion_control"),
                ],
            ),
            (
                "hysteria",
                &[
                    ("obfs", "obfs"),
                    ("obfs_password", "obfs_password"),
                    ("server_name", "server_name"),
                ],
            ),
            (
                "vless",
                &[
                    ("tls_settings", "tls_settings"),
                    ("flow", "flow"),
                    ("network_settings", "network_settings"),
                    ("encryption", "encryption"),
                    ("encryption_settings", "encryption_settings"),
                    ("sort", "sort"),
                ],
            ),
            (
                "anytls",
                &[
                    ("server_name", "server_name"),
                    ("padding_scheme", "padding_scheme"),
                ],
            ),
            (
                "v2node",
                &[
                    ("tls_settings", "tls_settings"),
                    ("flow", "flow"),
                    ("network_settings", "network_settings"),
                    ("encryption", "encryption"),
                    ("encryption_settings", "encryption_settings"),
                    ("udp_relay_mode", "udp_relay_mode"),
                    ("congestion_control", "congestion_control"),
                    ("cipher", "cipher"),
                    ("obfs", "obfs"),
                    ("obfs_password", "obfs_password"),
                    ("padding_scheme", "padding_scheme"),
                    ("sort", "sort"),
                ],
            ),
        ];
        for (kind, fields) in matrix {
            let absent = patch_values(kind, json!({}));
            assert!(
                absent.is_empty(),
                "{kind}: an empty PATCH must retain every column, wrote {:?}",
                columns(&absent)
            );
            for (field, column) in *fields {
                let set = patch_values(kind, json!({ *field: sample(field) }));
                assert!(
                    !is_null(value_of(&set, column)),
                    "{kind}.{field}: a value must set {column}"
                );
                let cleared = patch_values(kind, json!({ *field: Value::Null }));
                assert!(
                    is_null(value_of(&cleared, column)),
                    "{kind}.{field}: null must clear {column}"
                );
            }
        }
    }

    #[test]
    fn patch_show_toggle_writes_only_show() {
        for (kind, _) in SERVER_TABLES {
            let values = patch_values(kind, json!({"show": true}));
            assert_eq!(columns(&values), vec!["show"], "{kind}");
            assert!(matches!(
                value_of(&values, "show"),
                AdminSqlValue::Integer(1)
            ));
        }
    }

    #[test]
    fn create_requires_the_per_protocol_required_fields() {
        let required: &[(&str, &[&str])] = &[
            (
                "shadowsocks",
                &[
                    "group_id",
                    "name",
                    "rate",
                    "host",
                    "port",
                    "server_port",
                    "cipher",
                ],
            ),
            ("trojan", &["network"]),
            ("vmess", &["network"]),
            ("vless", &["network"]),
            ("v2node", &["protocol", "network"]),
        ];
        for (kind, fields) in required {
            for field in *fields {
                let mut base = create_base(kind);
                base.as_object_mut().unwrap().remove(*field);
                assert!(
                    build_server_values(kind, &body(base), ValueMode::Create).is_err(),
                    "{kind}: create without {field} must fail"
                );
            }
        }
    }

    #[test]
    fn create_applies_the_legacy_always_assigned_defaults() {
        let tuic = create_values("tuic", create_base("tuic"));
        for column in ["insecure", "disable_sni", "zero_rtt_handshake"] {
            assert!(matches!(value_of(&tuic, column), AdminSqlValue::Integer(0)));
        }
        let hysteria = create_values("hysteria", create_base("hysteria"));
        assert!(matches!(
            value_of(&hysteria, "version"),
            AdminSqlValue::Integer(2)
        ));
        for column in ["up_mbps", "down_mbps"] {
            assert!(matches!(
                value_of(&hysteria, column),
                AdminSqlValue::Integer(0)
            ));
        }
        assert!(is_null(value_of(&hysteria, "obfs_password")));
        let vmess = create_values("vmess", create_base("vmess"));
        assert!(matches!(value_of(&vmess, "tls"), AdminSqlValue::Integer(0)));
        // The legacy `rules` column is never written by create/patch.
        assert!(!columns(&vmess).contains(&"rules"));
        let vless = create_values("vless", create_base("vless"));
        assert!(matches!(value_of(&vless, "tls"), AdminSqlValue::Integer(0)));
        // vless port is the one INTEGER port column.
        assert!(matches!(
            value_of(&vless, "port"),
            AdminSqlValue::Integer(443)
        ));
        assert!(matches!(
            value_of(&tuic, "port"),
            AdminSqlValue::Text(port) if port == "443"
        ));
    }

    /// §4.4: an explicit JSON `null` clears the NOT NULL bandwidth columns
    /// back to the legacy always-assigned 0; absence retains on PATCH.
    #[test]
    fn patch_null_bandwidth_clears_to_the_legacy_default() {
        for kind in ["hysteria", "v2node"] {
            let cleared = patch_values(kind, json!({ "up_mbps": null, "down_mbps": null }));
            for column in ["up_mbps", "down_mbps"] {
                assert!(matches!(
                    value_of(&cleared, column),
                    AdminSqlValue::Integer(0)
                ));
            }
            let retained = patch_values(kind, json!({ "name": "node" }));
            assert!(!columns(&retained).contains(&"up_mbps"));
            assert!(!columns(&retained).contains(&"down_mbps"));
        }
    }

    /// R22: the vmess protocol-settings keys keep their legacy camelCase
    /// spelling on the wire — snake_case spellings are rejected, and the
    /// stored columns keep the camelCase names.
    #[test]
    fn vmess_camelcase_settings_keys_are_pinned() {
        let values = patch_values(
            "vmess",
            json!({
                "networkSettings": {"path": "/ws"},
                "tlsSettings": {"server_name": "sni"},
                "ruleSettings": {"domain": []},
                "dnsSettings": {"servers": []},
            }),
        );
        assert_eq!(
            columns(&values),
            vec![
                "networkSettings",
                "tlsSettings",
                "ruleSettings",
                "dnsSettings"
            ]
        );
        // snake_case is a cross-protocol key vmess does not accept.
        assert!(
            build_server_values(
                "vmess",
                &body(json!({"network_settings": {"path": "/ws"}})),
                ValueMode::Patch,
            )
            .is_err()
        );
        // A key outside every matrix is a serde deny_unknown_fields 422.
        assert!(serde_json::from_value::<ServerBody>(json!({"networkSetting": {}})).is_err());
    }

    #[test]
    fn cross_protocol_fields_are_rejected_per_matrix() {
        for (kind, field) in [
            ("shadowsocks", "flow"),
            ("trojan", "cipher"),
            ("tuic", "obfs"),
            ("hysteria", "padding_scheme"),
            ("vless", "listen_ip"),
            ("anytls", "networkSettings"),
        ] {
            assert!(
                build_server_values(
                    kind,
                    &body(json!({ field: sample(field) })),
                    ValueMode::Patch
                )
                .is_err(),
                "{kind} must reject {field}"
            );
        }
    }

    #[test]
    fn vless_reality_generates_missing_keys_when_tls_is_2() {
        let values = create_values("vless", {
            let mut base = create_base("vless");
            base.as_object_mut().unwrap().insert("tls".into(), json!(2));
            base
        });
        assert!(matches!(
            value_of(&values, "tls"),
            AdminSqlValue::Integer(2)
        ));
        let AdminSqlValue::Json(Some(settings)) = value_of(&values, "tls_settings") else {
            panic!("reality settings must be written");
        };
        for key in ["public_key", "private_key", "short_id", "server_port"] {
            assert!(settings.get(key).is_some(), "missing reality key {key}");
        }
        assert_eq!(
            settings["short_id"].as_str().map(str::len),
            Some(8),
            "short_id is the first 8 hex chars of sha1(private_key)"
        );
        // A PATCH that pins tls=2 regenerates missing keys too.
        let patched = patch_values("vless", json!({"tls": 2}));
        assert!(matches!(
            value_of(&patched, "tls_settings"),
            AdminSqlValue::Json(Some(_))
        ));
    }

    #[test]
    fn hysteria_obfs_password_coupling_matches_legacy_save() {
        // create without obfs: password forced NULL.
        let values = create_values("hysteria", create_base("hysteria"));
        assert!(is_null(value_of(&values, "obfs_password")));
        // create with obfs and no password: generated.
        let mut with_obfs = create_base("hysteria");
        with_obfs
            .as_object_mut()
            .unwrap()
            .insert("obfs".into(), json!("salamander"));
        let values = create_values("hysteria", with_obfs);
        assert!(matches!(
            value_of(&values, "obfs_password"),
            AdminSqlValue::Text(password) if !password.is_empty()
        ));
        // PATCH clearing obfs forces the password NULL even when supplied.
        let values = patch_values(
            "hysteria",
            json!({"obfs": Value::Null, "obfs_password": "kept?"}),
        );
        assert!(is_null(value_of(&values, "obfs_password")));
        // PATCH touching neither leaves the column alone.
        let values = patch_values("hysteria", json!({"server_name": "sni"}));
        assert!(!columns(&values).contains(&"obfs_password"));
    }

    #[test]
    fn v2node_tls_forcing_and_shadowsocks_cipher_default() {
        let mut base = create_base("v2node");
        base.as_object_mut()
            .unwrap()
            .insert("protocol".into(), json!("trojan"));
        let values = create_values("v2node", base);
        assert!(matches!(
            value_of(&values, "tls"),
            AdminSqlValue::Integer(1)
        ));

        let mut base = create_base("v2node");
        base.as_object_mut()
            .unwrap()
            .insert("protocol".into(), json!("shadowsocks"));
        let values = create_values("v2node", base);
        assert!(matches!(
            value_of(&values, "cipher"),
            AdminSqlValue::Text(cipher) if cipher == "aes-128-gcm"
        ));

        // A protocol-only PATCH that does not force tls retains the stored value.
        let values = patch_values("v2node", json!({"protocol": "vless"}));
        assert!(!columns(&values).contains(&"tls"));
        let values = patch_values("v2node", json!({"protocol": "tuic"}));
        assert!(matches!(
            value_of(&values, "tls"),
            AdminSqlValue::Integer(1)
        ));
    }

    #[test]
    fn ports_must_be_real_tcp_udp_ports() {
        for (field, value) in [("port", 0), ("server_port", 70_000)] {
            let mut base = create_base("tuic");
            base.as_object_mut()
                .unwrap()
                .insert(field.into(), json!(value));
            assert!(
                build_server_values("tuic", &body(base), ValueMode::Create).is_err(),
                "{field}={value} must be rejected"
            );
        }
    }

    #[test]
    fn route_matches_keep_the_legacy_action_vocabulary_and_falsy_filter() {
        assert!(validated_route_matches("bogus", &["1.1.1.1".into()]).is_err());
        assert!(validated_route_matches("block", &[]).is_err());
        assert_eq!(
            validated_route_matches("default_out", &["ignored".into()]).unwrap(),
            Vec::<String>::new()
        );
        assert_eq!(
            validated_route_matches("block", &["".into(), "0".into(), "1.1.1.1".into()]).unwrap(),
            vec!["1.1.1.1".to_string()]
        );
        // Laravel `required` counts a non-empty all-falsy array; array_filter
        // then stores an empty match set rather than rejecting.
        assert_eq!(
            validated_route_matches("block", &["".into(), "0".into()]).unwrap(),
            Vec::<String>::new()
        );
    }
}
