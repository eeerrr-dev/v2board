use std::collections::BTreeMap;

use v2board_domain_model::{
    ServerInputViolation, ServerKind, ServerRouteAction, canonical_server_group_ids,
    filter_server_route_matches, server_available_status, validate_server_port,
    validate_server_route_matches,
};

use crate::RepositoryError;

pub type RepositoryResult<T> = Result<T, RepositoryError>;

/// Adapter-neutral structured protocol setting. HTTP adapters translate the
/// closed wire DTOs into this value and persistence adapters translate it to
/// JSONB. The application layer neither parses nor serializes JSON.
#[derive(Clone, Debug, PartialEq)]
pub enum ServerSettingValue {
    Null,
    Bool(bool),
    Integer(i64),
    Decimal(String),
    String(String),
    Array(Vec<Self>),
    Object(BTreeMap<String, Self>),
}

impl ServerSettingValue {
    pub fn object(&self) -> Option<&BTreeMap<String, Self>> {
        match self {
            Self::Object(value) => Some(value),
            _ => None,
        }
    }

    pub fn object_mut(&mut self) -> Option<&mut BTreeMap<String, Self>> {
        match self {
            Self::Object(value) => Some(value),
            _ => None,
        }
    }

    pub fn string(&self) -> Option<&str> {
        match self {
            Self::String(value) => Some(value),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ServerGroup {
    pub id: i32,
    pub name: String,
    pub user_count: i64,
    pub server_count: i64,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ServerRoute {
    pub id: i32,
    pub remarks: String,
    pub match_rules: Vec<String>,
    pub action: ServerRouteAction,
    pub action_value: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ServerRouteCreateInput {
    pub remarks: String,
    pub match_rules: Vec<String>,
    pub action: ServerRouteAction,
    pub action_value: Option<String>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ServerRoutePatchInput {
    pub remarks: Option<String>,
    pub match_rules: Option<Vec<String>>,
    pub action: Option<ServerRouteAction>,
    pub action_value: Option<Option<String>>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ServerRouteChanges {
    pub remarks: Option<String>,
    pub match_rules: Option<Vec<String>>,
    pub action: Option<ServerRouteAction>,
    pub action_value: Option<Option<String>>,
    pub updated_at: i64,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct ServerWriteInput {
    pub group_id: Option<Vec<i64>>,
    pub route_id: Option<Option<Vec<i64>>>,
    pub parent_id: Option<Option<i64>>,
    pub tags: Option<Option<Vec<String>>>,
    pub name: Option<String>,
    pub rate: Option<f64>,
    pub host: Option<String>,
    pub port: Option<i64>,
    pub server_port: Option<i64>,
    pub show: Option<bool>,
    pub rotate_credential: Option<bool>,
    pub cipher: Option<Option<String>>,
    pub obfs: Option<Option<String>>,
    pub obfs_settings: Option<Option<ServerSettingValue>>,
    pub obfs_password: Option<Option<String>>,
    pub network: Option<String>,
    pub network_settings: Option<Option<ServerSettingValue>>,
    pub allow_insecure: Option<bool>,
    pub server_name: Option<Option<String>>,
    pub tls: Option<i64>,
    pub tls_settings: Option<Option<ServerSettingValue>>,
    pub vmess_network_settings: Option<Option<ServerSettingValue>>,
    pub vmess_tls_settings: Option<Option<ServerSettingValue>>,
    pub vmess_rule_settings: Option<Option<ServerSettingValue>>,
    pub vmess_dns_settings: Option<Option<ServerSettingValue>>,
    pub insecure: Option<bool>,
    pub disable_sni: Option<bool>,
    pub udp_relay_mode: Option<Option<String>>,
    pub zero_rtt_handshake: Option<bool>,
    pub congestion_control: Option<Option<String>>,
    pub version: Option<i64>,
    pub up_mbps: Option<Option<i64>>,
    pub down_mbps: Option<Option<i64>>,
    pub flow: Option<Option<String>>,
    pub encryption: Option<Option<String>>,
    pub encryption_settings: Option<Option<ServerSettingValue>>,
    pub sort: Option<Option<i64>>,
    pub padding_scheme: Option<Option<ServerSettingValue>>,
    pub listen_ip: Option<String>,
    pub protocol: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum ServerColumnValue {
    Text(Option<String>),
    Integer(Option<i64>),
    Structured(Option<ServerSettingValue>),
}

pub type ServerColumnValues = Vec<(&'static str, ServerColumnValue)>;

#[derive(Clone, Debug, PartialEq)]
pub struct PreparedServerWrite {
    pub values: ServerColumnValues,
    pub group_ids: Vec<i32>,
    pub rotate_credential: bool,
    pub updated_at: i64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ServerNodeCommon {
    pub id: i32,
    pub group_id: Vec<i64>,
    pub route_id: Option<Vec<i64>>,
    pub parent_id: Option<i32>,
    pub tags: Option<Vec<String>>,
    pub name: String,
    pub rate: f64,
    pub host: String,
    pub port: f64,
    pub server_port: i32,
    pub show: bool,
    pub sort: Option<i32>,
    pub created_at: i64,
    pub updated_at: i64,
    pub online: Option<i64>,
    pub last_check_at: Option<i64>,
    pub last_push_at: Option<i64>,
    pub available_status: i16,
    pub api_key: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
// Protocol rows are already heap-owned list records; preserving named fields
// in one exhaustive union is clearer than a second indirection per node.
#[allow(clippy::large_enum_variant)]
pub enum ServerNodeDetails {
    Shadowsocks {
        cipher: String,
        obfs: Option<String>,
        obfs_settings: Option<ServerSettingValue>,
    },
    Vmess {
        tls: i16,
        network: String,
        rules: Option<ServerSettingValue>,
        network_settings: Option<ServerSettingValue>,
        tls_settings: Option<ServerSettingValue>,
        rule_settings: Option<ServerSettingValue>,
        dns_settings: Option<ServerSettingValue>,
    },
    Trojan {
        network: Option<String>,
        network_settings: Option<ServerSettingValue>,
        allow_insecure: bool,
        server_name: Option<String>,
    },
    Tuic {
        server_name: Option<String>,
        insecure: bool,
        disable_sni: bool,
        udp_relay_mode: Option<String>,
        zero_rtt_handshake: bool,
        congestion_control: Option<String>,
    },
    Hysteria {
        version: i32,
        up_mbps: i32,
        down_mbps: i32,
        obfs: Option<String>,
        obfs_password: Option<String>,
        server_name: Option<String>,
        insecure: bool,
    },
    Vless {
        tls: i16,
        tls_settings: Option<ServerSettingValue>,
        flow: Option<String>,
        network: String,
        network_settings: Option<ServerSettingValue>,
        encryption: Option<String>,
        encryption_settings: Option<ServerSettingValue>,
    },
    Anytls {
        server_name: Option<String>,
        insecure: bool,
        padding_scheme: Option<ServerSettingValue>,
    },
    V2node {
        listen_ip: String,
        protocol: String,
        tls: i16,
        tls_settings: Option<ServerSettingValue>,
        flow: Option<String>,
        network: String,
        network_settings: Option<ServerSettingValue>,
        encryption: Option<String>,
        encryption_settings: Option<ServerSettingValue>,
        disable_sni: bool,
        udp_relay_mode: Option<String>,
        zero_rtt_handshake: bool,
        congestion_control: Option<String>,
        cipher: Option<String>,
        up_mbps: i32,
        down_mbps: i32,
        obfs: Option<String>,
        obfs_password: Option<String>,
        padding_scheme: Option<ServerSettingValue>,
        install_command: String,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub struct ServerNode {
    pub common: ServerNodeCommon,
    pub details: ServerNodeDetails,
}

impl ServerNode {
    pub const fn kind(&self) -> ServerKind {
        match self.details {
            ServerNodeDetails::Shadowsocks { .. } => ServerKind::Shadowsocks,
            ServerNodeDetails::Vmess { .. } => ServerKind::Vmess,
            ServerNodeDetails::Trojan { .. } => ServerKind::Trojan,
            ServerNodeDetails::Tuic { .. } => ServerKind::Tuic,
            ServerNodeDetails::Hysteria { .. } => ServerKind::Hysteria,
            ServerNodeDetails::Vless { .. } => ServerKind::Vless,
            ServerNodeDetails::Anytls { .. } => ServerKind::Anytls,
            ServerNodeDetails::V2node { .. } => ServerKind::V2node,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct StoredServerNode {
    pub node: ServerNode,
    pub credential_epoch: Option<i64>,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ServerHealth {
    pub online: Option<i64>,
    pub last_check_at: Option<i64>,
    pub last_push_at: Option<i64>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ServerPresenceKey {
    pub kind: ServerKind,
    pub node_id: i32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ServerSortUpdate {
    pub kind: ServerKind,
    pub id: i32,
    pub sort: i32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ServerGroupReference {
    Server,
    Plan,
    User,
    Unknown,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UpdateOutcome {
    Updated,
    NotFound,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DeleteGroupOutcome {
    Deleted,
    NotFound,
    InUse(ServerGroupReference),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ServerPersistenceOutcome {
    Applied,
    ServerNotFound,
    ServerGroupNotFound,
}

#[allow(async_fn_in_trait)]
pub trait ServerManagementRepository: Send + Sync {
    async fn groups(&self, id: Option<i32>) -> RepositoryResult<Vec<ServerGroup>>;
    async fn create_group(&self, name: &str, now: i64) -> RepositoryResult<i32>;
    async fn patch_group(&self, id: i32, name: &str, now: i64) -> RepositoryResult<UpdateOutcome>;
    async fn delete_group(&self, id: i32) -> RepositoryResult<DeleteGroupOutcome>;
    async fn routes(&self) -> RepositoryResult<Vec<ServerRoute>>;
    async fn create_route(&self, input: ServerRouteCreateInput, now: i64) -> RepositoryResult<i32>;
    async fn patch_route(
        &self,
        id: i32,
        changes: ServerRouteChanges,
    ) -> RepositoryResult<UpdateOutcome>;
    async fn delete_route(&self, id: i32) -> RepositoryResult<UpdateOutcome>;
    async fn nodes(&self) -> RepositoryResult<Vec<StoredServerNode>>;
    async fn sort_nodes(&self, updates: &[ServerSortUpdate]) -> RepositoryResult<()>;
    async fn create_server(
        &self,
        kind: ServerKind,
        write: PreparedServerWrite,
    ) -> RepositoryResult<Result<i32, ServerPersistenceOutcome>>;
    async fn patch_server(
        &self,
        kind: ServerKind,
        id: i32,
        write: PreparedServerWrite,
    ) -> RepositoryResult<ServerPersistenceOutcome>;
    async fn delete_server(
        &self,
        kind: ServerKind,
        id: i32,
    ) -> RepositoryResult<ServerPersistenceOutcome>;
    async fn copy_server(
        &self,
        kind: ServerKind,
        id: i32,
        now: i64,
    ) -> RepositoryResult<Result<i32, ServerPersistenceOutcome>>;
}

#[allow(async_fn_in_trait)]
pub trait ServerPresence: Send + Sync {
    async fn health(&self, keys: &[ServerPresenceKey]) -> Vec<ServerHealth>;
}

pub trait ServerCredentialProvisioner: Send + Sync {
    fn prepare_tls_settings(
        &self,
        input: Option<&ServerSettingValue>,
        tls: i64,
        v2node: bool,
    ) -> Result<ServerSettingValue, ServerCredentialError>;
    fn prepare_encryption_settings(
        &self,
        input: Option<&ServerSettingValue>,
        encryption: Option<&str>,
        v2node: bool,
    ) -> Result<ServerSettingValue, ServerCredentialError>;
    fn generate_obfs_password(&self, now: i64) -> Result<String, ServerCredentialError>;
    fn node_token(&self, kind: ServerKind, id: i32, epoch: i64) -> Option<String>;
    fn v2node_install_command(&self, id: i32, token: Option<&str>) -> String;
}

#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum ServerCredentialError {
    #[error("protocol settings are invalid")]
    InvalidSettings,
    #[error("credential generation failed: {0}")]
    Generation(String),
}

#[derive(Debug, thiserror::Error)]
pub enum ServerManagementError {
    #[error("invalid server input: {0:?}")]
    InvalidInput(ServerInputViolation),
    #[error("server group not found")]
    ServerGroupNotFound,
    #[error("server group is in use by {0:?}")]
    ServerGroupInUse(ServerGroupReference),
    #[error("server route not found")]
    RouteNotFound,
    #[error("server not found")]
    ServerNotFound,
    #[error("credential provisioning failed: {0}")]
    Credential(ServerCredentialError),
    #[error("node id or sort value is invalid")]
    InvalidNodeSort,
    #[error("presence adapter returned the wrong number of rows")]
    InvalidPresenceResult,
    #[error(transparent)]
    Repository(#[from] RepositoryError),
}

pub struct ServerManagementService<R, P, C> {
    repository: R,
    presence: P,
    credentials: C,
}

impl<R, P, C> ServerManagementService<R, P, C>
where
    R: ServerManagementRepository,
    P: ServerPresence,
    C: ServerCredentialProvisioner,
{
    pub fn new(repository: R, presence: P, credentials: C) -> Self {
        Self {
            repository,
            presence,
            credentials,
        }
    }

    pub async fn groups(&self, id: Option<i64>) -> Result<Vec<ServerGroup>, ServerManagementError> {
        let id = id
            .map(i32::try_from)
            .transpose()
            .map_err(|_| ServerManagementError::ServerGroupNotFound)?;
        Ok(self.repository.groups(id).await?)
    }

    pub async fn create_group(&self, name: &str, now: i64) -> Result<i32, ServerManagementError> {
        validate_group_name(name)?;
        Ok(self.repository.create_group(name, now).await?)
    }

    pub async fn patch_group(
        &self,
        id: i64,
        name: &str,
        now: i64,
    ) -> Result<(), ServerManagementError> {
        validate_group_name(name)?;
        let id = i32::try_from(id).map_err(|_| ServerManagementError::ServerGroupNotFound)?;
        match self.repository.patch_group(id, name, now).await? {
            UpdateOutcome::Updated => Ok(()),
            UpdateOutcome::NotFound => Err(ServerManagementError::ServerGroupNotFound),
        }
    }

    pub async fn delete_group(&self, id: i64) -> Result<(), ServerManagementError> {
        let id = i32::try_from(id).map_err(|_| ServerManagementError::ServerGroupNotFound)?;
        match self.repository.delete_group(id).await? {
            DeleteGroupOutcome::Deleted => Ok(()),
            DeleteGroupOutcome::NotFound => Err(ServerManagementError::ServerGroupNotFound),
            DeleteGroupOutcome::InUse(reference) => {
                Err(ServerManagementError::ServerGroupInUse(reference))
            }
        }
    }

    pub async fn routes(&self) -> Result<Vec<ServerRoute>, ServerManagementError> {
        Ok(self.repository.routes().await?)
    }

    pub async fn create_route(
        &self,
        mut input: ServerRouteCreateInput,
        now: i64,
    ) -> Result<i32, ServerManagementError> {
        validate_route_remarks(&input.remarks)?;
        input.match_rules = validate_server_route_matches(input.action, &input.match_rules)
            .map_err(ServerManagementError::InvalidInput)?;
        input.action_value = input.action_value.filter(|value| !value.trim().is_empty());
        Ok(self.repository.create_route(input, now).await?)
    }

    pub async fn patch_route(
        &self,
        id: i64,
        input: ServerRoutePatchInput,
        now: i64,
    ) -> Result<(), ServerManagementError> {
        let id = i32::try_from(id).map_err(|_| ServerManagementError::RouteNotFound)?;
        if let Some(remarks) = &input.remarks {
            validate_route_remarks(remarks)?;
        }
        let match_rules = if input.action == Some(ServerRouteAction::DefaultOut) {
            Some(Vec::new())
        } else {
            input
                .match_rules
                .as_deref()
                .map(|rules| {
                    if rules.is_empty() {
                        Err(ServerInputViolation::EmptyRouteMatches)
                    } else {
                        Ok(filter_server_route_matches(rules))
                    }
                })
                .transpose()
                .map_err(ServerManagementError::InvalidInput)?
        };
        let changes = ServerRouteChanges {
            remarks: input.remarks,
            match_rules,
            action: input.action,
            action_value: input.action_value,
            updated_at: now,
        };
        match self.repository.patch_route(id, changes).await? {
            UpdateOutcome::Updated => Ok(()),
            UpdateOutcome::NotFound => Err(ServerManagementError::RouteNotFound),
        }
    }

    pub async fn delete_route(&self, id: i64) -> Result<(), ServerManagementError> {
        let id = i32::try_from(id).map_err(|_| ServerManagementError::RouteNotFound)?;
        match self.repository.delete_route(id).await? {
            UpdateOutcome::Updated => Ok(()),
            UpdateOutcome::NotFound => Err(ServerManagementError::RouteNotFound),
        }
    }

    pub async fn nodes(&self, now: i64) -> Result<Vec<ServerNode>, ServerManagementError> {
        let mut rows = self.repository.nodes().await?;
        let keys = rows
            .iter()
            .map(|row| ServerPresenceKey {
                kind: row.node.kind(),
                node_id: row.node.common.parent_id.unwrap_or(row.node.common.id),
            })
            .collect::<Vec<_>>();
        let health = self.presence.health(&keys).await;
        if health.len() != rows.len() {
            return Err(ServerManagementError::InvalidPresenceResult);
        }
        for (row, health) in rows.iter_mut().zip(health) {
            let kind = row.node.kind();
            let common = &mut row.node.common;
            common.online = health.online;
            common.last_check_at = health.last_check_at;
            common.last_push_at = health.last_push_at;
            common.available_status =
                server_available_status(now, health.last_check_at, health.last_push_at);
            common.api_key = row
                .credential_epoch
                .and_then(|epoch| self.credentials.node_token(kind, common.id, epoch));
            if let ServerNodeDetails::V2node {
                install_command, ..
            } = &mut row.node.details
            {
                *install_command = self
                    .credentials
                    .v2node_install_command(common.id, common.api_key.as_deref());
            }
        }
        rows.sort_by_key(|row| row.node.common.sort.unwrap_or_default());
        Ok(rows.into_iter().map(|row| row.node).collect())
    }

    pub async fn sort_nodes(
        &self,
        body: &BTreeMap<String, BTreeMap<String, i64>>,
    ) -> Result<(), ServerManagementError> {
        let mut updates = Vec::new();
        for (raw_kind, entries) in body {
            let kind = ServerKind::try_from(raw_kind.as_str())
                .map_err(ServerManagementError::InvalidInput)?;
            for (raw_id, raw_sort) in entries {
                let id = raw_id
                    .trim()
                    .parse::<i32>()
                    .ok()
                    .filter(|id| *id > 0)
                    .ok_or(ServerManagementError::InvalidNodeSort)?;
                let sort =
                    i32::try_from(*raw_sort).map_err(|_| ServerManagementError::InvalidNodeSort)?;
                updates.push(ServerSortUpdate { kind, id, sort });
            }
        }
        Ok(self.repository.sort_nodes(&updates).await?)
    }

    pub async fn create_server(
        &self,
        raw_kind: &str,
        input: &ServerWriteInput,
        now: i64,
    ) -> Result<i32, ServerManagementError> {
        let kind = ServerKind::try_from(raw_kind).map_err(ServerManagementError::InvalidInput)?;
        let write = prepare_server_write(&self.credentials, kind, input, WriteMode::Create, now)?;
        self.repository
            .create_server(kind, write)
            .await?
            .map_err(persistence_error)
    }

    pub async fn patch_server(
        &self,
        raw_kind: &str,
        id: i64,
        input: &ServerWriteInput,
        now: i64,
    ) -> Result<(), ServerManagementError> {
        let kind = ServerKind::try_from(raw_kind).map_err(ServerManagementError::InvalidInput)?;
        let id = i32::try_from(id).map_err(|_| ServerManagementError::ServerNotFound)?;
        let write = prepare_server_write(&self.credentials, kind, input, WriteMode::Patch, now)?;
        match self.repository.patch_server(kind, id, write).await? {
            ServerPersistenceOutcome::Applied => Ok(()),
            outcome => Err(persistence_error(outcome)),
        }
    }

    pub async fn delete_server(
        &self,
        raw_kind: &str,
        id: i64,
    ) -> Result<(), ServerManagementError> {
        let kind = ServerKind::try_from(raw_kind).map_err(ServerManagementError::InvalidInput)?;
        let id = i32::try_from(id).map_err(|_| ServerManagementError::ServerNotFound)?;
        match self.repository.delete_server(kind, id).await? {
            ServerPersistenceOutcome::Applied => Ok(()),
            outcome => Err(persistence_error(outcome)),
        }
    }

    pub async fn copy_server(
        &self,
        raw_kind: &str,
        id: i64,
        now: i64,
    ) -> Result<i32, ServerManagementError> {
        let kind = ServerKind::try_from(raw_kind).map_err(ServerManagementError::InvalidInput)?;
        let id = i32::try_from(id).map_err(|_| ServerManagementError::ServerNotFound)?;
        self.repository
            .copy_server(kind, id, now)
            .await?
            .map_err(persistence_error)
    }
}

fn persistence_error(outcome: ServerPersistenceOutcome) -> ServerManagementError {
    match outcome {
        ServerPersistenceOutcome::Applied | ServerPersistenceOutcome::ServerNotFound => {
            ServerManagementError::ServerNotFound
        }
        ServerPersistenceOutcome::ServerGroupNotFound => ServerManagementError::ServerGroupNotFound,
    }
}

fn validate_group_name(name: &str) -> Result<(), ServerManagementError> {
    if name.trim().is_empty() {
        Err(ServerManagementError::InvalidInput(
            ServerInputViolation::Required("name"),
        ))
    } else {
        Ok(())
    }
}

fn validate_route_remarks(remarks: &str) -> Result<(), ServerManagementError> {
    if remarks.trim().is_empty() {
        Err(ServerManagementError::InvalidInput(
            ServerInputViolation::EmptyRouteRemarks,
        ))
    } else {
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum WriteMode {
    Create,
    Patch,
}

const COMMON_FIELDS: &[&str] = &[
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

fn protocol_fields(kind: ServerKind) -> &'static [&'static str] {
    match kind {
        ServerKind::Shadowsocks => &["cipher", "obfs", "obfs_settings"],
        ServerKind::Trojan => &[
            "network",
            "network_settings",
            "allow_insecure",
            "server_name",
        ],
        ServerKind::Vmess => &[
            "tls",
            "network",
            "networkSettings",
            "tlsSettings",
            "ruleSettings",
            "dnsSettings",
        ],
        ServerKind::Tuic => &[
            "server_name",
            "insecure",
            "disable_sni",
            "udp_relay_mode",
            "zero_rtt_handshake",
            "congestion_control",
        ],
        ServerKind::Hysteria => &[
            "version",
            "up_mbps",
            "down_mbps",
            "obfs",
            "obfs_password",
            "server_name",
            "insecure",
        ],
        ServerKind::Vless => &[
            "tls",
            "tls_settings",
            "flow",
            "network",
            "network_settings",
            "encryption",
            "encryption_settings",
            "sort",
        ],
        ServerKind::Anytls => &["server_name", "insecure", "padding_scheme"],
        ServerKind::V2node => &[
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
    }
}

impl ServerWriteInput {
    fn present_fields(&self) -> Vec<&'static str> {
        let mut fields = Vec::new();
        let mut add = |present, name| {
            if present {
                fields.push(name)
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

fn prepare_server_write<C: ServerCredentialProvisioner>(
    credentials: &C,
    kind: ServerKind,
    input: &ServerWriteInput,
    mode: WriteMode,
    now: i64,
) -> Result<PreparedServerWrite, ServerManagementError> {
    for field in input.present_fields() {
        if !COMMON_FIELDS.contains(&field) && !protocol_fields(kind).contains(&field) {
            return Err(ServerManagementError::InvalidInput(
                ServerInputViolation::UnsupportedField(field),
            ));
        }
    }
    let mut values = Vec::new();
    let group_ids = push_common_values(&mut values, kind, input, mode)?;
    match kind {
        ServerKind::Shadowsocks => {
            match (&input.cipher, mode) {
                (Some(Some(cipher)), _) => push_text(&mut values, "cipher", Some(cipher.clone())),
                (Some(None), _) | (None, WriteMode::Create) => return Err(required("cipher")),
                (None, WriteMode::Patch) => {}
            }
            set_nullable_text(&mut values, "obfs", &input.obfs);
            set_nullable_setting(&mut values, "obfs_settings", &input.obfs_settings);
        }
        ServerKind::Trojan => {
            set_required_text(&mut values, "network", &input.network, mode)?;
            push_network_settings(&mut values, input, false);
            if let Some(value) = input.allow_insecure {
                push_integer(&mut values, "allow_insecure", Some(i64::from(value)));
            }
            set_nullable_text(&mut values, "server_name", &input.server_name);
        }
        ServerKind::Vmess => {
            set_integer_default(&mut values, "tls", input.tls, 0, mode);
            set_required_text(&mut values, "network", &input.network, mode)?;
            set_nullable_setting(
                &mut values,
                "networkSettings",
                &input.vmess_network_settings,
            );
            set_nullable_setting(&mut values, "tlsSettings", &input.vmess_tls_settings);
            set_nullable_setting(&mut values, "ruleSettings", &input.vmess_rule_settings);
            set_nullable_setting(&mut values, "dnsSettings", &input.vmess_dns_settings);
        }
        ServerKind::Tuic => {
            set_nullable_text(&mut values, "server_name", &input.server_name);
            set_bool_default(&mut values, "insecure", input.insecure, false, mode);
            set_bool_default(&mut values, "disable_sni", input.disable_sni, false, mode);
            set_nullable_text(&mut values, "udp_relay_mode", &input.udp_relay_mode);
            set_bool_default(
                &mut values,
                "zero_rtt_handshake",
                input.zero_rtt_handshake,
                false,
                mode,
            );
            set_nullable_text(&mut values, "congestion_control", &input.congestion_control);
        }
        ServerKind::Hysteria => {
            set_integer_default(&mut values, "version", input.version, 2, mode);
            set_integer_clear_default(&mut values, "up_mbps", input.up_mbps, 0, mode);
            set_integer_clear_default(&mut values, "down_mbps", input.down_mbps, 0, mode);
            set_nullable_text(&mut values, "obfs", &input.obfs);
            push_obfs_password(credentials, &mut values, input, mode, now)?;
            set_nullable_text(&mut values, "server_name", &input.server_name);
            set_bool_default(&mut values, "insecure", input.insecure, false, mode);
        }
        ServerKind::Vless => {
            if input.network.is_none() && mode == WriteMode::Create {
                return Err(required("network"));
            }
            let tls = match mode {
                WriteMode::Create => Some(input.tls.unwrap_or_default()),
                WriteMode::Patch => input.tls,
            };
            if let Some(tls) = tls {
                push_integer(&mut values, "tls", Some(tls));
            }
            push_tls_settings(credentials, &mut values, input, tls, false)?;
            push_flow(
                &mut values,
                input,
                input
                    .network
                    .as_deref()
                    .is_some_and(|network| network != "tcp"),
            );
            if let Some(network) = &input.network {
                push_text(&mut values, "network", Some(network.clone()));
            }
            push_network_settings(&mut values, input, false);
            if let Some(encryption) = &input.encryption {
                push_text(&mut values, "encryption", encryption.clone());
            }
            push_encryption_settings(credentials, &mut values, input, false)?;
            set_nullable_integer(&mut values, "sort", input.sort);
        }
        ServerKind::Anytls => {
            set_nullable_text(&mut values, "server_name", &input.server_name);
            set_bool_default(&mut values, "insecure", input.insecure, false, mode);
            set_nullable_setting(&mut values, "padding_scheme", &input.padding_scheme);
        }
        ServerKind::V2node => push_v2node_values(credentials, &mut values, input, mode, now)?,
    }
    Ok(PreparedServerWrite {
        values,
        group_ids,
        rotate_credential: input.rotate_credential == Some(true),
        updated_at: now,
    })
}

fn push_common_values(
    values: &mut ServerColumnValues,
    kind: ServerKind,
    input: &ServerWriteInput,
    mode: WriteMode,
) -> Result<Vec<i32>, ServerManagementError> {
    let group_ids = match (&input.group_id, mode) {
        (Some(ids), _) => {
            let lock_ids =
                canonical_server_group_ids(ids).map_err(ServerManagementError::InvalidInput)?;
            push_setting(values, "group_id", Some(integer_array(ids)));
            lock_ids
        }
        (None, WriteMode::Create) => return Err(required("group_id")),
        (None, WriteMode::Patch) => Vec::new(),
    };
    set_required_text(values, "name", &input.name, mode)?;
    match (input.rate, mode) {
        (Some(rate), _) if rate.is_finite() => push_text(values, "rate", Some(rate.to_string())),
        (Some(_), _) => {
            return Err(ServerManagementError::InvalidInput(
                ServerInputViolation::InvalidRate,
            ));
        }
        (None, WriteMode::Create) => return Err(required("rate")),
        (None, WriteMode::Patch) => {}
    }
    set_required_text(values, "host", &input.host, mode)?;
    match (input.port, mode) {
        (Some(port), _) => {
            let port = i64::from(
                validate_server_port(port, "port").map_err(ServerManagementError::InvalidInput)?,
            );
            if kind == ServerKind::Vless {
                push_integer(values, "port", Some(port));
            } else {
                push_text(values, "port", Some(port.to_string()));
            }
        }
        (None, WriteMode::Create) => return Err(required("port")),
        (None, WriteMode::Patch) => {}
    }
    match (input.server_port, mode) {
        (Some(port), _) => push_integer(
            values,
            "server_port",
            Some(i64::from(
                validate_server_port(port, "server_port")
                    .map_err(ServerManagementError::InvalidInput)?,
            )),
        ),
        (None, WriteMode::Create) => return Err(required("server_port")),
        (None, WriteMode::Patch) => {}
    }
    if let Some(route_ids) = &input.route_id {
        push_setting(values, "route_id", route_ids.as_deref().map(integer_array));
    }
    if let Some(parent_id) = input.parent_id {
        push_integer(values, "parent_id", parent_id);
    }
    if let Some(tags) = &input.tags {
        push_setting(
            values,
            "tags",
            tags.as_ref().map(|items| {
                ServerSettingValue::Array(
                    items
                        .iter()
                        .cloned()
                        .map(ServerSettingValue::String)
                        .collect(),
                )
            }),
        );
    }
    if let Some(show) = input.show {
        push_integer(values, "show", Some(i64::from(show)));
    }
    Ok(group_ids)
}

fn push_v2node_values<C: ServerCredentialProvisioner>(
    credentials: &C,
    values: &mut ServerColumnValues,
    input: &ServerWriteInput,
    mode: WriteMode,
    now: i64,
) -> Result<(), ServerManagementError> {
    if let Some(listen_ip) = &input.listen_ip {
        push_text(values, "listen_ip", Some(listen_ip.clone()));
    }
    let protocol = match (&input.protocol, mode) {
        (Some(protocol), _) => {
            push_text(values, "protocol", Some(protocol.clone()));
            Some(protocol.as_str())
        }
        (None, WriteMode::Create) => return Err(required("protocol")),
        (None, WriteMode::Patch) => None,
    };
    if input.network.is_none() && mode == WriteMode::Create {
        return Err(required("network"));
    }
    let tls = match (protocol, input.tls, mode) {
        (Some(protocol), requested, WriteMode::Create) => Some(v2node_effective_tls(
            requested.unwrap_or_default(),
            protocol,
        )),
        (Some(protocol), Some(requested), WriteMode::Patch) => {
            Some(v2node_effective_tls(requested, protocol))
        }
        (Some(protocol), None, WriteMode::Patch) => {
            (v2node_effective_tls(0, protocol) == 1).then_some(1)
        }
        (None, requested, _) => requested,
    };
    if let Some(tls) = tls {
        push_integer(values, "tls", Some(tls));
    }
    push_tls_settings(credentials, values, input, tls, true)?;
    let encryption = input.encryption.as_ref().and_then(|value| value.as_deref());
    let force_flow_null = input
        .network
        .as_deref()
        .is_some_and(|network| network != "tcp")
        && encryption.is_some()
        && encryption != Some("mlkem768x25519plus");
    push_flow(values, input, force_flow_null);
    if let Some(network) = &input.network {
        push_text(values, "network", Some(network.clone()));
    }
    push_network_settings(values, input, true);
    if let Some(encryption) = &input.encryption {
        push_text(values, "encryption", encryption.clone());
    }
    push_encryption_settings(credentials, values, input, true)?;
    set_bool_default(values, "disable_sni", input.disable_sni, false, mode);
    set_nullable_text(values, "udp_relay_mode", &input.udp_relay_mode);
    set_bool_default(
        values,
        "zero_rtt_handshake",
        input.zero_rtt_handshake,
        false,
        mode,
    );
    set_nullable_text(values, "congestion_control", &input.congestion_control);
    let shadowsocks = protocol == Some("shadowsocks");
    if shadowsocks || input.cipher.is_some() {
        let cipher = input
            .cipher
            .clone()
            .flatten()
            .filter(|value| !value.trim().is_empty())
            .or_else(|| shadowsocks.then(|| "aes-128-gcm".to_string()));
        push_text(values, "cipher", cipher);
    }
    set_integer_clear_default(values, "up_mbps", input.up_mbps, 0, mode);
    set_integer_clear_default(values, "down_mbps", input.down_mbps, 0, mode);
    set_nullable_text(values, "obfs", &input.obfs);
    push_obfs_password(credentials, values, input, mode, now)?;
    set_nullable_setting(values, "padding_scheme", &input.padding_scheme);
    set_nullable_integer(values, "sort", input.sort);
    Ok(())
}

fn push_tls_settings<C: ServerCredentialProvisioner>(
    credentials: &C,
    values: &mut ServerColumnValues,
    input: &ServerWriteInput,
    tls: Option<i64>,
    v2node: bool,
) -> Result<(), ServerManagementError> {
    let reality = tls == Some(2);
    if !reality && input.tls_settings.is_none() {
        return Ok(());
    }
    let settings = input.tls_settings.as_ref().and_then(Option::as_ref);
    if !reality && settings.is_none() {
        push_setting(values, "tls_settings", None);
        return Ok(());
    }
    let prepared = credentials
        .prepare_tls_settings(settings, tls.unwrap_or_default(), v2node)
        .map_err(ServerManagementError::Credential)?;
    push_setting(values, "tls_settings", Some(prepared));
    Ok(())
}

fn push_encryption_settings<C: ServerCredentialProvisioner>(
    credentials: &C,
    values: &mut ServerColumnValues,
    input: &ServerWriteInput,
    v2node: bool,
) -> Result<(), ServerManagementError> {
    let encryption = input.encryption.as_ref().and_then(|value| value.as_deref());
    let mlkem = encryption == Some("mlkem768x25519plus");
    if !mlkem && input.encryption_settings.is_none() {
        return Ok(());
    }
    let settings = input.encryption_settings.as_ref().and_then(Option::as_ref);
    if !mlkem && settings.is_none() {
        push_setting(values, "encryption_settings", None);
        return Ok(());
    }
    let prepared = credentials
        .prepare_encryption_settings(settings, encryption, v2node)
        .map_err(ServerManagementError::Credential)?;
    push_setting(values, "encryption_settings", Some(prepared));
    Ok(())
}

fn push_obfs_password<C: ServerCredentialProvisioner>(
    credentials: &C,
    values: &mut ServerColumnValues,
    input: &ServerWriteInput,
    mode: WriteMode,
    now: i64,
) -> Result<(), ServerManagementError> {
    if mode == WriteMode::Patch && input.obfs.is_none() && input.obfs_password.is_none() {
        return Ok(());
    }
    if input.obfs.is_some() || mode == WriteMode::Create {
        let enabled = input
            .obfs
            .as_ref()
            .and_then(|value| value.as_deref())
            .is_some_and(|value| !value.trim().is_empty());
        if !enabled {
            push_text(values, "obfs_password", None);
            return Ok(());
        }
        let password = input
            .obfs_password
            .clone()
            .flatten()
            .filter(|value| !value.trim().is_empty())
            .map(Ok)
            .unwrap_or_else(|| credentials.generate_obfs_password(now))
            .map_err(ServerManagementError::Credential)?;
        push_text(values, "obfs_password", Some(password));
    } else {
        set_nullable_text(values, "obfs_password", &input.obfs_password);
    }
    Ok(())
}

fn push_network_settings(values: &mut ServerColumnValues, input: &ServerWriteInput, v2node: bool) {
    if let Some(entry) = &input.network_settings {
        let normalized = entry.as_ref().map(|settings| {
            normalize_network_settings(settings.clone(), input.network.as_deref(), v2node)
        });
        push_setting(values, "network_settings", normalized);
    }
}

fn normalize_network_settings(
    mut settings: ServerSettingValue,
    network: Option<&str>,
    v2node: bool,
) -> ServerSettingValue {
    if v2node && let Some(object) = settings.object_mut() {
        coerce_bool(object, "acceptProxyProtocol");
    }
    if network != Some("xhttp") {
        return settings;
    }
    let Some(object) = settings.object_mut() else {
        return settings;
    };
    let Some(extra) = object
        .get_mut("extra")
        .and_then(ServerSettingValue::object_mut)
    else {
        return settings;
    };
    if v2node {
        coerce_bool(extra, "xPaddingObfsMode");
    }
    coerce_bool(extra, "noGRPCHeader");
    coerce_bool(extra, "noSSEHeader");
    coerce_integer(extra, "scMaxBufferedPosts");
    if let Some(xmux) = extra
        .get_mut("xmux")
        .and_then(ServerSettingValue::object_mut)
    {
        coerce_integer(xmux, "hKeepAlivePeriod");
    }
    if let Some(download) = extra
        .get_mut("downloadSettings")
        .and_then(ServerSettingValue::object_mut)
    {
        coerce_integer(download, "port");
    }
    settings
}

fn coerce_bool(object: &mut BTreeMap<String, ServerSettingValue>, key: &str) {
    let Some(value) = object.get_mut(key) else {
        return;
    };
    let flag = match value {
        ServerSettingValue::Bool(value) => *value,
        ServerSettingValue::Integer(value) => *value != 0,
        ServerSettingValue::String(value) => matches!(
            value.to_ascii_lowercase().as_str(),
            "1" | "true" | "yes" | "on"
        ),
        _ => false,
    };
    *value = ServerSettingValue::Bool(flag);
}

fn coerce_integer(object: &mut BTreeMap<String, ServerSettingValue>, key: &str) {
    let Some(value) = object.get_mut(key) else {
        return;
    };
    let parsed = match value {
        ServerSettingValue::Integer(value) => Some(*value),
        ServerSettingValue::String(value) => value.parse().ok(),
        ServerSettingValue::Bool(value) => Some(i64::from(*value)),
        _ => None,
    };
    if let Some(parsed) = parsed {
        *value = ServerSettingValue::Integer(parsed);
    }
}

fn v2node_effective_tls(requested: i64, protocol: &str) -> i64 {
    if (protocol == "anytls" && requested == 0)
        || matches!(protocol, "hysteria2" | "trojan" | "tuic")
    {
        1
    } else {
        requested
    }
}

fn push_flow(values: &mut ServerColumnValues, input: &ServerWriteInput, force_null: bool) {
    if force_null {
        push_text(values, "flow", None);
    } else {
        set_nullable_text(values, "flow", &input.flow);
    }
}

fn set_required_text(
    values: &mut ServerColumnValues,
    column: &'static str,
    value: &Option<String>,
    mode: WriteMode,
) -> Result<(), ServerManagementError> {
    match (value, mode) {
        (Some(value), _) => {
            push_text(values, column, Some(value.clone()));
            Ok(())
        }
        (None, WriteMode::Create) => Err(required(column)),
        (None, WriteMode::Patch) => Ok(()),
    }
}

fn set_nullable_text(
    values: &mut ServerColumnValues,
    column: &'static str,
    value: &Option<Option<String>>,
) {
    if let Some(value) = value {
        push_text(values, column, value.clone());
    }
}

fn set_nullable_setting(
    values: &mut ServerColumnValues,
    column: &'static str,
    value: &Option<Option<ServerSettingValue>>,
) {
    if let Some(value) = value {
        push_setting(values, column, value.clone());
    }
}

fn set_nullable_integer(
    values: &mut ServerColumnValues,
    column: &'static str,
    value: Option<Option<i64>>,
) {
    if let Some(value) = value {
        push_integer(values, column, value);
    }
}

fn set_bool_default(
    values: &mut ServerColumnValues,
    column: &'static str,
    value: Option<bool>,
    default: bool,
    mode: WriteMode,
) {
    match (value, mode) {
        (Some(value), _) => push_integer(values, column, Some(i64::from(value))),
        (None, WriteMode::Create) => push_integer(values, column, Some(i64::from(default))),
        (None, WriteMode::Patch) => {}
    }
}

fn set_integer_default(
    values: &mut ServerColumnValues,
    column: &'static str,
    value: Option<i64>,
    default: i64,
    mode: WriteMode,
) {
    match (value, mode) {
        (Some(value), _) => push_integer(values, column, Some(value)),
        (None, WriteMode::Create) => push_integer(values, column, Some(default)),
        (None, WriteMode::Patch) => {}
    }
}

fn set_integer_clear_default(
    values: &mut ServerColumnValues,
    column: &'static str,
    value: Option<Option<i64>>,
    default: i64,
    mode: WriteMode,
) {
    set_integer_default(
        values,
        column,
        value.map(|value| value.unwrap_or(default)),
        default,
        mode,
    );
}

fn push_text(values: &mut ServerColumnValues, column: &'static str, value: Option<String>) {
    values.push((column, ServerColumnValue::Text(value)));
}

fn push_integer(values: &mut ServerColumnValues, column: &'static str, value: Option<i64>) {
    values.push((column, ServerColumnValue::Integer(value)));
}

fn push_setting(
    values: &mut ServerColumnValues,
    column: &'static str,
    value: Option<ServerSettingValue>,
) {
    values.push((column, ServerColumnValue::Structured(value)));
}

fn integer_array(values: &[i64]) -> ServerSettingValue {
    ServerSettingValue::Array(
        values
            .iter()
            .copied()
            .map(ServerSettingValue::Integer)
            .collect(),
    )
}

fn required(field: &'static str) -> ServerManagementError {
    ServerManagementError::InvalidInput(ServerInputViolation::Required(field))
}

#[cfg(test)]
mod tests {
    use std::{
        future::Future,
        pin::pin,
        sync::{Arc, Mutex},
        task::{Context, Poll, Waker},
    };

    use super::*;

    #[derive(Default)]
    struct FakeState {
        calls: usize,
        write: Option<PreparedServerWrite>,
        route: Option<ServerRouteCreateInput>,
        sort: Vec<ServerSortUpdate>,
        nodes: Vec<StoredServerNode>,
    }

    #[derive(Clone, Default)]
    struct FakeRepository(Arc<Mutex<FakeState>>);

    impl ServerManagementRepository for FakeRepository {
        async fn groups(&self, _: Option<i32>) -> RepositoryResult<Vec<ServerGroup>> {
            self.0.lock().unwrap().calls += 1;
            Ok(Vec::new())
        }

        async fn create_group(&self, _: &str, _: i64) -> RepositoryResult<i32> {
            self.0.lock().unwrap().calls += 1;
            Ok(1)
        }

        async fn patch_group(&self, _: i32, _: &str, _: i64) -> RepositoryResult<UpdateOutcome> {
            self.0.lock().unwrap().calls += 1;
            Ok(UpdateOutcome::Updated)
        }

        async fn delete_group(&self, _: i32) -> RepositoryResult<DeleteGroupOutcome> {
            self.0.lock().unwrap().calls += 1;
            Ok(DeleteGroupOutcome::Deleted)
        }

        async fn routes(&self) -> RepositoryResult<Vec<ServerRoute>> {
            self.0.lock().unwrap().calls += 1;
            Ok(Vec::new())
        }

        async fn create_route(
            &self,
            input: ServerRouteCreateInput,
            _: i64,
        ) -> RepositoryResult<i32> {
            let mut state = self.0.lock().unwrap();
            state.calls += 1;
            state.route = Some(input);
            Ok(1)
        }

        async fn patch_route(
            &self,
            _: i32,
            _: ServerRouteChanges,
        ) -> RepositoryResult<UpdateOutcome> {
            self.0.lock().unwrap().calls += 1;
            Ok(UpdateOutcome::Updated)
        }

        async fn delete_route(&self, _: i32) -> RepositoryResult<UpdateOutcome> {
            self.0.lock().unwrap().calls += 1;
            Ok(UpdateOutcome::Updated)
        }

        async fn nodes(&self) -> RepositoryResult<Vec<StoredServerNode>> {
            let mut state = self.0.lock().unwrap();
            state.calls += 1;
            Ok(state.nodes.clone())
        }

        async fn sort_nodes(&self, updates: &[ServerSortUpdate]) -> RepositoryResult<()> {
            let mut state = self.0.lock().unwrap();
            state.calls += 1;
            state.sort = updates.to_vec();
            Ok(())
        }

        async fn create_server(
            &self,
            _: ServerKind,
            write: PreparedServerWrite,
        ) -> RepositoryResult<Result<i32, ServerPersistenceOutcome>> {
            let mut state = self.0.lock().unwrap();
            state.calls += 1;
            state.write = Some(write);
            Ok(Ok(7))
        }

        async fn patch_server(
            &self,
            _: ServerKind,
            _: i32,
            write: PreparedServerWrite,
        ) -> RepositoryResult<ServerPersistenceOutcome> {
            let mut state = self.0.lock().unwrap();
            state.calls += 1;
            state.write = Some(write);
            Ok(ServerPersistenceOutcome::Applied)
        }

        async fn delete_server(
            &self,
            _: ServerKind,
            _: i32,
        ) -> RepositoryResult<ServerPersistenceOutcome> {
            self.0.lock().unwrap().calls += 1;
            Ok(ServerPersistenceOutcome::Applied)
        }

        async fn copy_server(
            &self,
            _: ServerKind,
            _: i32,
            _: i64,
        ) -> RepositoryResult<Result<i32, ServerPersistenceOutcome>> {
            self.0.lock().unwrap().calls += 1;
            Ok(Ok(8))
        }
    }

    #[derive(Clone, Copy)]
    struct FakePresence;

    impl ServerPresence for FakePresence {
        async fn health(&self, keys: &[ServerPresenceKey]) -> Vec<ServerHealth> {
            keys.iter()
                .map(|_| ServerHealth {
                    online: Some(3),
                    last_check_at: Some(990),
                    last_push_at: Some(995),
                })
                .collect()
        }
    }

    #[derive(Clone, Copy)]
    struct FakeCredentials;

    impl ServerCredentialProvisioner for FakeCredentials {
        fn prepare_tls_settings(
            &self,
            input: Option<&ServerSettingValue>,
            _: i64,
            _: bool,
        ) -> Result<ServerSettingValue, ServerCredentialError> {
            Ok(input.cloned().unwrap_or_else(|| {
                ServerSettingValue::Object(BTreeMap::from([(
                    "generated".into(),
                    ServerSettingValue::Bool(true),
                )]))
            }))
        }

        fn prepare_encryption_settings(
            &self,
            input: Option<&ServerSettingValue>,
            _: Option<&str>,
            _: bool,
        ) -> Result<ServerSettingValue, ServerCredentialError> {
            Ok(input
                .cloned()
                .unwrap_or_else(|| ServerSettingValue::Object(BTreeMap::new())))
        }

        fn generate_obfs_password(&self, _: i64) -> Result<String, ServerCredentialError> {
            Ok("generated-password".into())
        }

        fn node_token(&self, kind: ServerKind, id: i32, epoch: i64) -> Option<String> {
            Some(format!("{}-{id}-{epoch}", kind.as_str()))
        }

        fn v2node_install_command(&self, id: i32, token: Option<&str>) -> String {
            format!("install {id} {}", token.unwrap_or_default())
        }
    }

    fn service(
        repository: FakeRepository,
    ) -> ServerManagementService<FakeRepository, FakePresence, FakeCredentials> {
        ServerManagementService::new(repository, FakePresence, FakeCredentials)
    }

    fn block_on<F: Future>(future: F) -> F::Output {
        let mut future = pin!(future);
        let mut context = Context::from_waker(Waker::noop());
        loop {
            match future.as_mut().poll(&mut context) {
                Poll::Ready(value) => return value,
                Poll::Pending => std::thread::yield_now(),
            }
        }
    }

    fn valid_server() -> ServerWriteInput {
        ServerWriteInput {
            group_id: Some(vec![3, 1, 3]),
            name: Some("edge".into()),
            rate: Some(1.0),
            host: Some("edge.test".into()),
            port: Some(443),
            server_port: Some(443),
            network: Some("tcp".into()),
            ..ServerWriteInput::default()
        }
    }

    #[test]
    fn invalid_protocol_matrix_never_reaches_the_repository() {
        let repository = FakeRepository::default();
        let mut input = valid_server();
        input.network = None;
        input.cipher = Some(Some("aes-128-gcm".into()));
        assert!(matches!(
            block_on(service(repository.clone()).create_server("trojan", &input, 10)),
            Err(ServerManagementError::InvalidInput(
                ServerInputViolation::UnsupportedField("cipher")
            ))
        ));
        assert_eq!(repository.0.lock().unwrap().calls, 0);
    }

    #[test]
    fn create_sends_canonical_group_locks_and_preserves_stored_order() {
        let repository = FakeRepository::default();
        let mut input = valid_server();
        input.network = None;
        input.cipher = Some(Some("aes-128-gcm".into()));
        assert_eq!(
            block_on(service(repository.clone()).create_server("shadowsocks", &input, 42)).unwrap(),
            7
        );
        let write = repository.0.lock().unwrap().write.clone().unwrap();
        assert_eq!(write.group_ids, [1, 3]);
        assert_eq!(write.updated_at, 42);
        let stored_groups = write
            .values
            .iter()
            .find(|(column, _)| *column == "group_id")
            .map(|(_, value)| value)
            .unwrap();
        assert_eq!(
            stored_groups,
            &ServerColumnValue::Structured(Some(ServerSettingValue::Array(vec![
                ServerSettingValue::Integer(3),
                ServerSettingValue::Integer(1),
                ServerSettingValue::Integer(3),
            ])))
        );
    }

    #[test]
    fn route_and_sort_commands_are_normalized_before_the_atomic_ports() {
        let repository = FakeRepository::default();
        block_on(service(repository.clone()).create_route(
            ServerRouteCreateInput {
                remarks: "pin".into(),
                match_rules: vec!["".into(), "0".into(), "1.1.1.1".into()],
                action: ServerRouteAction::Block,
                action_value: Some("".into()),
            },
            10,
        ))
        .unwrap();
        let route = repository.0.lock().unwrap().route.clone().unwrap();
        assert_eq!(route.match_rules, ["1.1.1.1"]);
        assert_eq!(route.action_value, None);

        let sort = BTreeMap::from([(
            "vmess".into(),
            BTreeMap::from([("7".into(), 9), ("8".into(), 10)]),
        )]);
        block_on(service(repository.clone()).sort_nodes(&sort)).unwrap();
        assert_eq!(repository.0.lock().unwrap().sort.len(), 2);
    }

    #[test]
    fn node_read_composes_presence_and_scoped_credentials() {
        let repository = FakeRepository::default();
        repository.0.lock().unwrap().nodes.push(StoredServerNode {
            node: ServerNode {
                common: ServerNodeCommon {
                    id: 7,
                    group_id: vec![1],
                    route_id: None,
                    parent_id: None,
                    tags: None,
                    name: "edge".into(),
                    rate: 1.0,
                    host: "edge.test".into(),
                    port: 443.0,
                    server_port: 443,
                    show: true,
                    sort: Some(1),
                    created_at: 1,
                    updated_at: 1,
                    online: None,
                    last_check_at: None,
                    last_push_at: None,
                    available_status: 0,
                    api_key: None,
                },
                details: ServerNodeDetails::Shadowsocks {
                    cipher: "aes-128-gcm".into(),
                    obfs: None,
                    obfs_settings: None,
                },
            },
            credential_epoch: Some(2),
        });
        let nodes = block_on(service(repository).nodes(1_000)).unwrap();
        assert_eq!(nodes[0].common.online, Some(3));
        assert_eq!(nodes[0].common.available_status, 2);
        assert_eq!(nodes[0].common.api_key.as_deref(), Some("shadowsocks-7-2"));
    }
}
