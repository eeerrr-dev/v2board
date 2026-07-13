//! Empty-target node proof for the one-shot legacy migration.
//!
//! External node processes are outside this repository. Schema v4 therefore
//! accepts only an empty source with `not_required_no_nodes`. Schema v5 binds
//! the complete legacy source inventory as intentionally excluded and requires
//! the operator to rebuild nodes manually. Both policies independently prove
//! that the target node inventory remains empty before and after native
//! authority; schema v5 additionally proves that routes and historical node /
//! user traffic details were not copied. Operational-log discard is deliberately
//! owned by the pre-authority PostgreSQL copy receipt instead: after authority,
//! the native API and worker may immediately write new `v2_log`/`v2_mail_log`
//! rows, so this post-authority gate must not require those tables to stay empty.

use std::collections::BTreeSet;

use serde::Serialize;
use sha2::{Digest, Sha256};
use sqlx::{MySqlPool, PgPool};
use uuid::Uuid;

use crate::{
    ProvisionKind, ProvisionSpec, apply_journal::DurableNativeStartPermit,
    manifest::LegacyNodeActivationTransportSpec, target_activation::ServiceReadinessProof,
};

const API_UNIT: &str = "v2board-api.service";
const PRE_AUTHORIZATION_SUMMARY_DOMAIN_V1: &[u8] =
    b"v2board-pre-authorization-empty-node-summary-v1\0";
const PRE_AUTHORIZATION_SUMMARY_DOMAIN_V2: &[u8] = b"v2board-pre-authorization-node-summary-v2\0";
const EMPTY_TARGET_REPORT_DOMAIN_V1: &[u8] = b"v2board-empty-node-cutover-report-v1\0";
const EMPTY_TARGET_REPORT_DOMAIN_V2: &[u8] = b"v2board-empty-target-node-cutover-report-v2\0";
const POST_AUTHORITY_REQUEST_DOMAIN_V1: &[u8] = b"v2board-post-authority-empty-node-request-v1\0";
const POST_AUTHORITY_REQUEST_DOMAIN_V2: &[u8] =
    b"v2board-post-authority-empty-target-node-completion-request-v2\0";
const POST_AUTHORITY_REPORT_DOMAIN_V1: &[u8] = b"v2board-post-authority-empty-node-activation-v1\0";
const POST_AUTHORITY_REPORT_DOMAIN_V2: &[u8] =
    b"v2board-post-authority-empty-target-node-completion-v2\0";

#[derive(Debug, thiserror::Error)]
pub enum NodeCutoverError {
    #[error("schema-v4 or schema-v5 legacy execution inputs are required")]
    MissingExecution,
    #[error("legacy migration node policy is invalid for this schema")]
    InvalidManifestInventory,
    #[error("legacy source node inventory could not be inspected")]
    SourceRead,
    #[error("the reserved installation identity is invalid")]
    InvalidInstallationIdentity,
    #[error("target node inventory could not be verified")]
    TargetRead,
    #[error("target contains state forbidden by the selected node cutover policy")]
    TargetNotEmpty,
    #[error("native start permit does not bind the empty-target node proof")]
    InvalidNativeStartPermit,
    #[error("empty-target node proof could not be serialized")]
    Serialization,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum NodeCutoverProductionBlocker {
    ExternalNodeCoordinatorUnavailable,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LegacyNodeMigrationOutcome {
    RequireEmptySource,
    DiscardAndManualRebuild,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum NodeCutoverPolicy {
    RequireEmptySource,
    DiscardAndManualRebuild,
}

impl NodeCutoverPolicy {
    const fn outcome(self) -> LegacyNodeMigrationOutcome {
        match self {
            Self::RequireEmptySource => LegacyNodeMigrationOutcome::RequireEmptySource,
            Self::DiscardAndManualRebuild => LegacyNodeMigrationOutcome::DiscardAndManualRebuild,
        }
    }
}

impl NodeCutoverProductionBlocker {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ExternalNodeCoordinatorUnavailable => "external_node_coordinator_unavailable",
        }
    }
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(deny_unknown_fields)]
pub struct NodeIdentity {
    pub node_type: String,
    pub node_id: i32,
}

#[derive(Serialize)]
pub struct PreAuthorizationNodeInventorySummary {
    pub operation_id: String,
    pub node_count: u64,
    pub nodes: Vec<NodeIdentity>,
    pub legacy_source_node_set_sha256: String,
    pub manifest_inventory_empty: bool,
    pub exact_legacy_source_match: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub planned_outcome: Option<LegacyNodeMigrationOutcome>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub legacy_nodes_to_discard: Option<u64>,
    pub blockers: Vec<NodeCutoverProductionBlocker>,
    pub summary_sha256: String,
}

#[derive(Serialize)]
struct PreAuthorizationSummaryMaterialV1<'a> {
    schema_version: u32,
    operation_id: &'a str,
    manifest_binding_hmac_sha256: &'a str,
    node_count: u64,
    nodes: &'a [NodeIdentity],
    legacy_source_node_set_sha256: &'a str,
    manifest_inventory_empty: bool,
    exact_legacy_source_match: bool,
    blockers: &'a [NodeCutoverProductionBlocker],
}

#[derive(Serialize)]
struct PreAuthorizationSummaryMaterialV2<'a> {
    schema_version: u32,
    operation_id: &'a str,
    manifest_binding_hmac_sha256: &'a str,
    node_count: u64,
    nodes: &'a [NodeIdentity],
    legacy_source_node_set_sha256: &'a str,
    manifest_inventory_empty: bool,
    exact_legacy_source_match: bool,
    planned_outcome: LegacyNodeMigrationOutcome,
    legacy_nodes_to_discard: u64,
    blockers: &'a [NodeCutoverProductionBlocker],
}

/// Bind a read-only snapshot of all eight legacy MySQL node tables. Schema v4
/// reports a non-empty source as a blocker. Schema v5 binds that same complete
/// inventory as intentionally excluded, without requiring an external node
/// activation coordinator.
pub async fn inspect_pre_authorization_node_inventory(
    spec: &ProvisionSpec,
    source_pool: &MySqlPool,
) -> Result<PreAuthorizationNodeInventorySummary, NodeCutoverError> {
    let policy = require_node_policy(spec)?;
    let nodes = read_legacy_source_nodes(source_pool).await?;
    let legacy_source_node_set_sha256 = node_set_sha256(&nodes)?;
    let exact_legacy_source_match = nodes.is_empty();
    let blockers = source_inventory_blockers(policy, exact_legacy_source_match);
    let nodes = nodes.into_iter().collect::<Vec<_>>();
    let node_count = u64::try_from(nodes.len()).map_err(|_| NodeCutoverError::Serialization)?;
    let (planned_outcome, legacy_nodes_to_discard, summary_sha256) = match policy {
        NodeCutoverPolicy::RequireEmptySource => {
            let material = PreAuthorizationSummaryMaterialV1 {
                schema_version: 1,
                operation_id: &spec.operation_id,
                manifest_binding_hmac_sha256: spec.manifest_binding_hmac_sha256(),
                node_count,
                nodes: &nodes,
                legacy_source_node_set_sha256: &legacy_source_node_set_sha256,
                manifest_inventory_empty: true,
                exact_legacy_source_match,
                blockers: &blockers,
            };
            let canonical =
                serde_json::to_vec(&material).map_err(|_| NodeCutoverError::Serialization)?;
            (
                None,
                None,
                domain_sha256(PRE_AUTHORIZATION_SUMMARY_DOMAIN_V1, &canonical),
            )
        }
        NodeCutoverPolicy::DiscardAndManualRebuild => {
            let material = PreAuthorizationSummaryMaterialV2 {
                schema_version: 2,
                operation_id: &spec.operation_id,
                manifest_binding_hmac_sha256: spec.manifest_binding_hmac_sha256(),
                node_count,
                nodes: &nodes,
                legacy_source_node_set_sha256: &legacy_source_node_set_sha256,
                manifest_inventory_empty: true,
                exact_legacy_source_match,
                planned_outcome: policy.outcome(),
                legacy_nodes_to_discard: node_count,
                blockers: &blockers,
            };
            let canonical =
                serde_json::to_vec(&material).map_err(|_| NodeCutoverError::Serialization)?;
            (
                Some(policy.outcome()),
                Some(node_count),
                domain_sha256(PRE_AUTHORIZATION_SUMMARY_DOMAIN_V2, &canonical),
            )
        }
    };
    Ok(PreAuthorizationNodeInventorySummary {
        operation_id: spec.operation_id.clone(),
        node_count,
        nodes,
        legacy_source_node_set_sha256,
        manifest_inventory_empty: true,
        exact_legacy_source_match,
        planned_outcome,
        legacy_nodes_to_discard,
        blockers,
        summary_sha256,
    })
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NodeCutoverReport {
    report_sha256: String,
}

impl NodeCutoverReport {
    pub fn report_sha256(&self) -> &str {
        &self.report_sha256
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct NodeCutoverCompletionReport {
    report_sha256: String,
    completion_request_id: String,
    target_node_set_sha256: String,
    outcome: LegacyNodeMigrationOutcome,
}

impl NodeCutoverCompletionReport {
    pub fn report_sha256(&self) -> &str {
        &self.report_sha256
    }

    pub fn completion_request_id(&self) -> &str {
        &self.completion_request_id
    }

    pub fn target_node_set_sha256(&self) -> &str {
        &self.target_node_set_sha256
    }

    pub const fn target_node_count(&self) -> usize {
        0
    }

    pub const fn outcome(&self) -> LegacyNodeMigrationOutcome {
        self.outcome
    }
}

#[derive(Clone, Copy)]
enum TargetInstallationPhase {
    LegacyPending,
    NativeActive,
}

#[derive(Serialize)]
struct EmptyTargetProofMaterialV1<'a> {
    schema_version: u32,
    operation_id: &'a str,
    manifest_binding_hmac_sha256: &'a str,
    installation_id: &'a str,
    release_id: &'a str,
    manifest_node_count: u64,
    target_node_count: i64,
    target_credential_count: i64,
}

#[derive(Serialize)]
struct EmptyTargetProofMaterialV2<'a> {
    schema_version: u32,
    operation_id: &'a str,
    manifest_binding_hmac_sha256: &'a str,
    installation_id: &'a str,
    release_id: &'a str,
    planned_outcome: LegacyNodeMigrationOutcome,
    manifest_node_count: u64,
    target_node_count: i64,
    target_route_count: i64,
    target_credential_count: i64,
    target_server_traffic_detail_count: i64,
    target_user_traffic_detail_count: i64,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct EmptyTargetSnapshot {
    node_count: i64,
    route_count: i64,
    credential_count: i64,
    server_traffic_detail_count: i64,
    user_traffic_detail_count: i64,
}

/// Production verifier for the only supported target topology: no migrated
/// nodes. It owns no transport, staging directory, coordinator, or receipt
/// protocol; the journal stores the returned proof hash.
pub struct NativeNodeCutover<'a> {
    spec: &'a ProvisionSpec,
    pool: &'a PgPool,
    installation_id: String,
    release_id: String,
    policy: NodeCutoverPolicy,
}

impl<'a> NativeNodeCutover<'a> {
    pub fn new(
        spec: &'a ProvisionSpec,
        pool: &'a PgPool,
        installation_id: &str,
    ) -> Result<Self, NodeCutoverError> {
        let policy = require_node_policy(spec)?;
        let installation_id = canonical_non_nil_uuid(installation_id)
            .ok_or(NodeCutoverError::InvalidInstallationIdentity)?;
        let release_id = spec
            .legacy_apply_execution()
            .ok_or(NodeCutoverError::MissingExecution)?
            .release
            .release_id
            .clone();
        Ok(Self {
            spec,
            pool,
            installation_id,
            release_id,
            policy,
        })
    }

    pub async fn verify_empty_target_before_authority(
        &self,
    ) -> Result<NodeCutoverReport, NodeCutoverError> {
        self.empty_report(TargetInstallationPhase::LegacyPending)
            .await
    }

    pub async fn complete_empty_target_after_native_authority(
        &self,
        expected_offline_report_sha256: &str,
        permit: &DurableNativeStartPermit,
        api_ready: &ServiceReadinessProof,
    ) -> Result<NodeCutoverCompletionReport, NodeCutoverError> {
        let authority = permit.native_authority_binding();
        if permit.operation_id() != self.spec.operation_id
            || permit.installation_id() != self.installation_id
            || permit.generation() == 0
            || !is_lower_sha256(permit.inspect_review_sha256())
            || !is_lower_sha256(permit.event_sha256())
            || !is_lower_sha256(permit.checkpoint_proof_sha256())
            || authority.node_cutover_report_sha256() != expected_offline_report_sha256
            || authority.nodes_verified_generation() == 0
            || !is_lower_sha256(authority.nodes_verified_event_sha256())
            || api_ready.operation_id != self.spec.operation_id
            || api_ready.installation_id != self.installation_id
            || api_ready.release_id != self.release_id
            || api_ready.unit != API_UNIT
            || !api_ready.postgres_ledger_exactly_current
            || !api_ready.runtime_role_and_config_verified
            || !api_ready.ready
            || api_ready.systemd_notify_ready.is_some()
            || api_ready.watchdog_healthy.is_some()
        {
            return Err(NodeCutoverError::InvalidNativeStartPermit);
        }
        let current = self
            .empty_report(TargetInstallationPhase::NativeActive)
            .await?;
        if current.report_sha256 != expected_offline_report_sha256 {
            return Err(NodeCutoverError::InvalidNativeStartPermit);
        }
        let readiness =
            serde_json::to_vec(api_ready).map_err(|_| NodeCutoverError::Serialization)?;
        let readiness_sha256 =
            domain_sha256(b"v2board-native-api-readiness-proof-v1\0", &readiness);
        let request_domain = match self.policy {
            NodeCutoverPolicy::RequireEmptySource => POST_AUTHORITY_REQUEST_DOMAIN_V1,
            NodeCutoverPolicy::DiscardAndManualRebuild => POST_AUTHORITY_REQUEST_DOMAIN_V2,
        };
        let completion_request_id = domain_hash_fields(
            request_domain,
            [
                self.spec.operation_id.as_bytes(),
                self.installation_id.as_bytes(),
                expected_offline_report_sha256.as_bytes(),
                permit.event_sha256().as_bytes(),
                permit.checkpoint_proof_sha256().as_bytes(),
                readiness_sha256.as_bytes(),
            ],
        );
        let report_domain = match self.policy {
            NodeCutoverPolicy::RequireEmptySource => POST_AUTHORITY_REPORT_DOMAIN_V1,
            NodeCutoverPolicy::DiscardAndManualRebuild => POST_AUTHORITY_REPORT_DOMAIN_V2,
        };
        let report_sha256 = domain_sha256(report_domain, completion_request_id.as_bytes());
        Ok(NodeCutoverCompletionReport {
            report_sha256,
            completion_request_id,
            target_node_set_sha256: empty_node_set_sha256(),
            outcome: self.policy.outcome(),
        })
    }

    async fn empty_report(
        &self,
        phase: TargetInstallationPhase,
    ) -> Result<NodeCutoverReport, NodeCutoverError> {
        let snapshot = self.read_empty_target_snapshot(phase).await?;
        if !target_snapshot_allowed(self.policy, snapshot) {
            return Err(NodeCutoverError::TargetNotEmpty);
        }
        let report_sha256 = match self.policy {
            NodeCutoverPolicy::RequireEmptySource => {
                let material = EmptyTargetProofMaterialV1 {
                    schema_version: 1,
                    operation_id: &self.spec.operation_id,
                    manifest_binding_hmac_sha256: self.spec.manifest_binding_hmac_sha256(),
                    installation_id: &self.installation_id,
                    release_id: &self.release_id,
                    manifest_node_count: 0,
                    target_node_count: snapshot.node_count,
                    target_credential_count: snapshot.credential_count,
                };
                let canonical =
                    serde_json::to_vec(&material).map_err(|_| NodeCutoverError::Serialization)?;
                domain_sha256(EMPTY_TARGET_REPORT_DOMAIN_V1, &canonical)
            }
            NodeCutoverPolicy::DiscardAndManualRebuild => {
                let material = EmptyTargetProofMaterialV2 {
                    schema_version: 2,
                    operation_id: &self.spec.operation_id,
                    manifest_binding_hmac_sha256: self.spec.manifest_binding_hmac_sha256(),
                    installation_id: &self.installation_id,
                    release_id: &self.release_id,
                    planned_outcome: self.policy.outcome(),
                    manifest_node_count: 0,
                    target_node_count: snapshot.node_count,
                    target_route_count: snapshot.route_count,
                    target_credential_count: snapshot.credential_count,
                    target_server_traffic_detail_count: snapshot.server_traffic_detail_count,
                    target_user_traffic_detail_count: snapshot.user_traffic_detail_count,
                };
                let canonical =
                    serde_json::to_vec(&material).map_err(|_| NodeCutoverError::Serialization)?;
                domain_sha256(EMPTY_TARGET_REPORT_DOMAIN_V2, &canonical)
            }
        };
        Ok(NodeCutoverReport { report_sha256 })
    }

    async fn read_empty_target_snapshot(
        &self,
        phase: TargetInstallationPhase,
    ) -> Result<EmptyTargetSnapshot, NodeCutoverError> {
        let mut transaction = self
            .pool
            .begin()
            .await
            .map_err(|_| NodeCutoverError::TargetRead)?;
        sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ READ ONLY")
            .execute(&mut *transaction)
            .await
            .map_err(|_| NodeCutoverError::TargetRead)?;
        let installation = sqlx::query_as::<_, (String, String, String, Option<i64>)>(
            "SELECT installation_id::text, lineage, state, activated_at \
             FROM v2_system_installation WHERE singleton = 1",
        )
        .fetch_optional(&mut *transaction)
        .await
        .map_err(|_| NodeCutoverError::TargetRead)?
        .ok_or(NodeCutoverError::TargetRead)?;
        let phase_matches = match phase {
            TargetInstallationPhase::LegacyPending => {
                installation.1 == "legacy_migrated"
                    && installation.2 == "pending"
                    && installation.3.is_none()
            }
            TargetInstallationPhase::NativeActive => {
                installation.1 == "native"
                    && installation.2 == "active"
                    && installation.3.is_some_and(|value| value > 0)
            }
        };
        if installation.0 != self.installation_id || !phase_matches {
            return Err(NodeCutoverError::TargetRead);
        }
        let node_count = sqlx::query_scalar::<_, i64>(
            "SELECT \
               (SELECT COUNT(*) FROM v2_server_shadowsocks) + \
               (SELECT COUNT(*) FROM v2_server_vmess) + \
               (SELECT COUNT(*) FROM v2_server_trojan) + \
               (SELECT COUNT(*) FROM v2_server_tuic) + \
               (SELECT COUNT(*) FROM v2_server_hysteria) + \
               (SELECT COUNT(*) FROM v2_server_vless) + \
               (SELECT COUNT(*) FROM v2_server_anytls) + \
               (SELECT COUNT(*) FROM v2_server_v2node)",
        )
        .fetch_one(&mut *transaction)
        .await
        .map_err(|_| NodeCutoverError::TargetRead)?;
        let credential_count =
            sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM v2_server_credential")
                .fetch_one(&mut *transaction)
                .await
                .map_err(|_| NodeCutoverError::TargetRead)?;
        let (route_count, server_traffic_detail_count, user_traffic_detail_count) = if self.policy
            == NodeCutoverPolicy::DiscardAndManualRebuild
        {
            let route_count = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM v2_server_route")
                .fetch_one(&mut *transaction)
                .await
                .map_err(|_| NodeCutoverError::TargetRead)?;
            let server_traffic_detail_count =
                sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM v2_stat_server")
                    .fetch_one(&mut *transaction)
                    .await
                    .map_err(|_| NodeCutoverError::TargetRead)?;
            let user_traffic_detail_count =
                sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM v2_stat_user")
                    .fetch_one(&mut *transaction)
                    .await
                    .map_err(|_| NodeCutoverError::TargetRead)?;
            (
                route_count,
                server_traffic_detail_count,
                user_traffic_detail_count,
            )
        } else {
            // Preserve the schema-v4 database read surface exactly: its
            // historical proof only owns nodes and derived credentials.
            (0, 0, 0)
        };
        transaction
            .commit()
            .await
            .map_err(|_| NodeCutoverError::TargetRead)?;
        Ok(EmptyTargetSnapshot {
            node_count,
            route_count,
            credential_count,
            server_traffic_detail_count,
            user_traffic_detail_count,
        })
    }
}

fn require_node_policy(spec: &ProvisionSpec) -> Result<NodeCutoverPolicy, NodeCutoverError> {
    if spec.kind != ProvisionKind::LegacyReferenceMigration {
        return Err(NodeCutoverError::InvalidManifestInventory);
    }
    let nodes = &spec
        .legacy_apply_execution()
        .ok_or(NodeCutoverError::MissingExecution)?
        .nodes;
    if !nodes.inventory.is_empty() {
        return Err(NodeCutoverError::InvalidManifestInventory);
    }
    match (spec.schema_version, &nodes.activation_transport) {
        (4, LegacyNodeActivationTransportSpec::NotRequiredNoNodes) => {
            Ok(NodeCutoverPolicy::RequireEmptySource)
        }
        (5, LegacyNodeActivationTransportSpec::DiscardAndManualRebuild) => {
            Ok(NodeCutoverPolicy::DiscardAndManualRebuild)
        }
        _ => Err(NodeCutoverError::InvalidManifestInventory),
    }
}

fn source_inventory_blockers(
    policy: NodeCutoverPolicy,
    source_is_empty: bool,
) -> Vec<NodeCutoverProductionBlocker> {
    if source_is_empty || policy == NodeCutoverPolicy::DiscardAndManualRebuild {
        Vec::new()
    } else {
        vec![NodeCutoverProductionBlocker::ExternalNodeCoordinatorUnavailable]
    }
}

fn target_snapshot_allowed(policy: NodeCutoverPolicy, snapshot: EmptyTargetSnapshot) -> bool {
    if snapshot.node_count != 0 || snapshot.credential_count != 0 {
        return false;
    }
    policy == NodeCutoverPolicy::RequireEmptySource
        || (snapshot.route_count == 0
            && snapshot.server_traffic_detail_count == 0
            && snapshot.user_traffic_detail_count == 0)
}

async fn read_legacy_source_nodes(
    pool: &MySqlPool,
) -> Result<BTreeSet<NodeIdentity>, NodeCutoverError> {
    let mut transaction = pool
        .begin_with("START TRANSACTION WITH CONSISTENT SNAPSHOT, READ ONLY")
        .await
        .map_err(|_| NodeCutoverError::SourceRead)?;
    let rows = sqlx::query_as::<_, (String, i32)>(
        "SELECT node_type, node_id FROM ( \
             SELECT 'shadowsocks' AS node_type, id AS node_id FROM v2_server_shadowsocks \
             UNION ALL SELECT 'vmess', id FROM v2_server_vmess \
             UNION ALL SELECT 'trojan', id FROM v2_server_trojan \
             UNION ALL SELECT 'tuic', id FROM v2_server_tuic \
             UNION ALL SELECT 'hysteria', id FROM v2_server_hysteria \
             UNION ALL SELECT 'vless', id FROM v2_server_vless \
             UNION ALL SELECT 'anytls', id FROM v2_server_anytls \
             UNION ALL SELECT 'v2node', id FROM v2_server_v2node \
         ) nodes ORDER BY node_type, node_id",
    )
    .fetch_all(&mut *transaction)
    .await
    .map_err(|_| NodeCutoverError::SourceRead)?;
    transaction
        .commit()
        .await
        .map_err(|_| NodeCutoverError::SourceRead)?;
    let mut nodes = BTreeSet::new();
    for (node_type, node_id) in rows {
        if !valid_node_type(&node_type)
            || node_id <= 0
            || !nodes.insert(NodeIdentity { node_type, node_id })
        {
            return Err(NodeCutoverError::SourceRead);
        }
    }
    Ok(nodes)
}

fn valid_node_type(value: &str) -> bool {
    matches!(
        value,
        "shadowsocks" | "vmess" | "trojan" | "tuic" | "hysteria" | "vless" | "anytls" | "v2node"
    )
}

fn node_set_sha256(nodes: &BTreeSet<NodeIdentity>) -> Result<String, NodeCutoverError> {
    let canonical = serde_json::to_vec(nodes).map_err(|_| NodeCutoverError::Serialization)?;
    Ok(domain_sha256(b"v2board-node-set-v1\0", &canonical))
}

fn empty_node_set_sha256() -> String {
    node_set_sha256(&BTreeSet::new()).expect("empty node set is serializable")
}

fn canonical_non_nil_uuid(value: &str) -> Option<String> {
    let uuid = Uuid::parse_str(value).ok()?;
    (!uuid.is_nil()).then(|| uuid.hyphenated().to_string())
}

fn is_lower_sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
}

fn domain_sha256(domain: &[u8], payload: &[u8]) -> String {
    let mut digest = Sha256::new();
    digest.update(domain);
    digest.update((payload.len() as u64).to_be_bytes());
    digest.update(payload);
    hex::encode(digest.finalize())
}

fn domain_hash_fields<'a>(domain: &[u8], fields: impl IntoIterator<Item = &'a [u8]>) -> String {
    let mut digest = Sha256::new();
    digest.update(domain);
    for field in fields {
        digest.update((field.len() as u64).to_be_bytes());
        digest.update(field);
    }
    hex::encode(digest.finalize())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blocker_code_and_empty_node_set_are_stable() {
        assert_eq!(
            NodeCutoverProductionBlocker::ExternalNodeCoordinatorUnavailable.as_str(),
            "external_node_coordinator_unavailable"
        );
        assert!(is_lower_sha256(&empty_node_set_sha256()));
        assert_eq!(
            empty_node_set_sha256(),
            node_set_sha256(&BTreeSet::new()).expect("empty node set")
        );
    }

    #[test]
    fn schema_v4_evidence_material_and_domains_remain_frozen() {
        let nodes = Vec::new();
        let blockers = Vec::new();
        let pre_authorization = PreAuthorizationSummaryMaterialV1 {
            schema_version: 1,
            operation_id: "op",
            manifest_binding_hmac_sha256: "aa",
            node_count: 0,
            nodes: &nodes,
            legacy_source_node_set_sha256: "bb",
            manifest_inventory_empty: true,
            exact_legacy_source_match: true,
            blockers: &blockers,
        };
        let pre_authorization =
            serde_json::to_vec(&pre_authorization).expect("v4 pre-authorization material");
        assert_eq!(
            String::from_utf8(pre_authorization.clone()).expect("JSON is UTF-8"),
            r#"{"schema_version":1,"operation_id":"op","manifest_binding_hmac_sha256":"aa","node_count":0,"nodes":[],"legacy_source_node_set_sha256":"bb","manifest_inventory_empty":true,"exact_legacy_source_match":true,"blockers":[]}"#
        );
        assert_eq!(
            domain_sha256(PRE_AUTHORIZATION_SUMMARY_DOMAIN_V1, &pre_authorization),
            "2507c5863972e16ed95e30052fe3693aec7ececef19f1ae4e32332aed543c7af"
        );

        let target = EmptyTargetProofMaterialV1 {
            schema_version: 1,
            operation_id: "op",
            manifest_binding_hmac_sha256: "aa",
            installation_id: "install",
            release_id: "release",
            manifest_node_count: 0,
            target_node_count: 0,
            target_credential_count: 0,
        };
        let target = serde_json::to_vec(&target).expect("v4 target material");
        assert_eq!(
            String::from_utf8(target.clone()).expect("JSON is UTF-8"),
            r#"{"schema_version":1,"operation_id":"op","manifest_binding_hmac_sha256":"aa","installation_id":"install","release_id":"release","manifest_node_count":0,"target_node_count":0,"target_credential_count":0}"#
        );
        assert_eq!(
            domain_sha256(EMPTY_TARGET_REPORT_DOMAIN_V1, &target),
            "489831d9bfaeb9779665fa56f1c78437d0bc3d4bc7e82a7ce597d85f877ce9e6"
        );
        assert_eq!(
            POST_AUTHORITY_REQUEST_DOMAIN_V1,
            b"v2board-post-authority-empty-node-request-v1\0"
        );
        assert_eq!(
            POST_AUTHORITY_REPORT_DOMAIN_V1,
            b"v2board-post-authority-empty-node-activation-v1\0"
        );
    }

    #[test]
    fn node_set_hash_is_order_independent_and_rejects_invalid_uuid() {
        let nodes = BTreeSet::from([
            NodeIdentity {
                node_type: "vmess".to_string(),
                node_id: 2,
            },
            NodeIdentity {
                node_type: "trojan".to_string(),
                node_id: 1,
            },
        ]);
        let reversed = nodes.iter().rev().cloned().collect::<BTreeSet<_>>();
        assert_eq!(
            node_set_sha256(&nodes).expect("node set"),
            node_set_sha256(&reversed).expect("node set")
        );
        assert!(canonical_non_nil_uuid("00000000-0000-0000-0000-000000000000").is_none());
        assert!(canonical_non_nil_uuid("not-a-uuid").is_none());
    }

    #[test]
    fn schema_v4_requires_an_empty_source_but_v5_discard_does_not() {
        assert_eq!(
            source_inventory_blockers(NodeCutoverPolicy::RequireEmptySource, false),
            [NodeCutoverProductionBlocker::ExternalNodeCoordinatorUnavailable]
        );
        assert!(
            source_inventory_blockers(NodeCutoverPolicy::DiscardAndManualRebuild, false).is_empty()
        );
        assert!(source_inventory_blockers(NodeCutoverPolicy::RequireEmptySource, true).is_empty());
    }

    #[test]
    fn schema_v5_requires_every_discarded_target_table_to_stay_empty() {
        let empty = EmptyTargetSnapshot::default();
        assert!(target_snapshot_allowed(
            NodeCutoverPolicy::DiscardAndManualRebuild,
            empty
        ));

        for forbidden in [
            EmptyTargetSnapshot {
                node_count: 1,
                ..empty
            },
            EmptyTargetSnapshot {
                route_count: 1,
                ..empty
            },
            EmptyTargetSnapshot {
                credential_count: 1,
                ..empty
            },
            EmptyTargetSnapshot {
                server_traffic_detail_count: 1,
                ..empty
            },
            EmptyTargetSnapshot {
                user_traffic_detail_count: 1,
                ..empty
            },
        ] {
            assert!(!target_snapshot_allowed(
                NodeCutoverPolicy::DiscardAndManualRebuild,
                forbidden
            ));
        }

        let retained_v4_details = EmptyTargetSnapshot {
            route_count: 1,
            server_traffic_detail_count: 1,
            user_traffic_detail_count: 1,
            ..empty
        };
        assert!(target_snapshot_allowed(
            NodeCutoverPolicy::RequireEmptySource,
            retained_v4_details
        ));
    }
}
