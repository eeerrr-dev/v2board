//! Empty-node proof for the one-shot legacy migration.
//!
//! External node processes are outside this repository. Schema v4 therefore
//! accepts only an empty embedded inventory and `not_required_no_nodes`. The
//! online inspection proves that the legacy MySQL node tables are empty; the
//! production stage independently proves that every PostgreSQL node table and
//! the credential table remain empty before and after native authority.

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

#[derive(Debug, thiserror::Error)]
pub enum NodeCutoverError {
    #[error("schema-v4 legacy execution inputs are required")]
    MissingExecution,
    #[error("legacy migration requires an empty manifest node inventory")]
    InvalidManifestInventory,
    #[error("legacy source node inventory could not be inspected")]
    SourceRead,
    #[error("the reserved installation identity is invalid")]
    InvalidInstallationIdentity,
    #[error("target node inventory could not be verified")]
    TargetRead,
    #[error("target node inventory is not empty")]
    TargetNotEmpty,
    #[error("native start permit does not bind the empty-node proof")]
    InvalidNativeStartPermit,
    #[error("empty-node proof could not be serialized")]
    Serialization,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum NodeCutoverProductionBlocker {
    ExternalNodeCoordinatorUnavailable,
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
    pub blockers: Vec<NodeCutoverProductionBlocker>,
    pub summary_sha256: String,
}

#[derive(Serialize)]
struct PreAuthorizationSummaryMaterial<'a> {
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

/// Compare the only supported empty manifest inventory with a read-only
/// snapshot of every legacy MySQL node table. A non-empty source is reported
/// with one stable blocker before the maintenance window begins.
pub async fn inspect_pre_authorization_node_inventory(
    spec: &ProvisionSpec,
    source_pool: &MySqlPool,
) -> Result<PreAuthorizationNodeInventorySummary, NodeCutoverError> {
    require_empty_manifest(spec)?;
    let nodes = read_legacy_source_nodes(source_pool).await?;
    let legacy_source_node_set_sha256 = node_set_sha256(&nodes)?;
    let exact_legacy_source_match = nodes.is_empty();
    let blockers = if exact_legacy_source_match {
        Vec::new()
    } else {
        vec![NodeCutoverProductionBlocker::ExternalNodeCoordinatorUnavailable]
    };
    let nodes = nodes.into_iter().collect::<Vec<_>>();
    let node_count = u64::try_from(nodes.len()).map_err(|_| NodeCutoverError::Serialization)?;
    let material = PreAuthorizationSummaryMaterial {
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
    let canonical = serde_json::to_vec(&material).map_err(|_| NodeCutoverError::Serialization)?;
    let summary_sha256 = domain_sha256(
        b"v2board-pre-authorization-empty-node-summary-v1\0",
        &canonical,
    );
    Ok(PreAuthorizationNodeInventorySummary {
        operation_id: spec.operation_id.clone(),
        node_count,
        nodes,
        legacy_source_node_set_sha256,
        manifest_inventory_empty: true,
        exact_legacy_source_match,
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
pub struct ActivatedNodesReport {
    report_sha256: String,
    activation_request_id: String,
    activated_node_set_sha256: String,
}

impl ActivatedNodesReport {
    pub fn report_sha256(&self) -> &str {
        &self.report_sha256
    }

    pub fn activation_request_id(&self) -> &str {
        &self.activation_request_id
    }

    pub fn activated_node_set_sha256(&self) -> &str {
        &self.activated_node_set_sha256
    }

    pub const fn node_count(&self) -> usize {
        0
    }
}

#[derive(Clone, Copy)]
enum TargetInstallationPhase {
    LegacyPending,
    NativeActive,
}

#[derive(Serialize)]
struct EmptyTargetProofMaterial<'a> {
    schema_version: u32,
    operation_id: &'a str,
    manifest_binding_hmac_sha256: &'a str,
    installation_id: &'a str,
    release_id: &'a str,
    manifest_node_count: u64,
    target_node_count: i64,
    target_credential_count: i64,
}

/// Production verifier for the only supported node topology: no external
/// nodes. It owns no transport, staging directory, coordinator, or receipt
/// protocol; the journal stores the returned proof hash.
pub struct NativeNodeCutover<'a> {
    spec: &'a ProvisionSpec,
    pool: &'a PgPool,
    installation_id: String,
    release_id: String,
}

impl<'a> NativeNodeCutover<'a> {
    pub fn new(
        spec: &'a ProvisionSpec,
        pool: &'a PgPool,
        installation_id: &str,
    ) -> Result<Self, NodeCutoverError> {
        require_empty_manifest(spec)?;
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
        })
    }

    pub async fn verify_empty_before_authority(
        &self,
    ) -> Result<NodeCutoverReport, NodeCutoverError> {
        self.empty_report(TargetInstallationPhase::LegacyPending)
            .await
    }

    pub async fn complete_empty_inventory_after_native_authority(
        &self,
        expected_offline_report_sha256: &str,
        permit: &DurableNativeStartPermit,
        api_ready: &ServiceReadinessProof,
    ) -> Result<ActivatedNodesReport, NodeCutoverError> {
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
        let activation_request_id = domain_hash_fields(
            b"v2board-post-authority-empty-node-request-v1\0",
            [
                self.spec.operation_id.as_bytes(),
                self.installation_id.as_bytes(),
                expected_offline_report_sha256.as_bytes(),
                permit.event_sha256().as_bytes(),
                permit.checkpoint_proof_sha256().as_bytes(),
                readiness_sha256.as_bytes(),
            ],
        );
        let report_sha256 = domain_sha256(
            b"v2board-post-authority-empty-node-activation-v1\0",
            activation_request_id.as_bytes(),
        );
        Ok(ActivatedNodesReport {
            report_sha256,
            activation_request_id,
            activated_node_set_sha256: empty_node_set_sha256(),
        })
    }

    async fn empty_report(
        &self,
        phase: TargetInstallationPhase,
    ) -> Result<NodeCutoverReport, NodeCutoverError> {
        let (target_node_count, target_credential_count) =
            self.read_empty_target_snapshot(phase).await?;
        if target_node_count != 0 || target_credential_count != 0 {
            return Err(NodeCutoverError::TargetNotEmpty);
        }
        let material = EmptyTargetProofMaterial {
            schema_version: 1,
            operation_id: &self.spec.operation_id,
            manifest_binding_hmac_sha256: self.spec.manifest_binding_hmac_sha256(),
            installation_id: &self.installation_id,
            release_id: &self.release_id,
            manifest_node_count: 0,
            target_node_count,
            target_credential_count,
        };
        let canonical =
            serde_json::to_vec(&material).map_err(|_| NodeCutoverError::Serialization)?;
        Ok(NodeCutoverReport {
            report_sha256: domain_sha256(b"v2board-empty-node-cutover-report-v1\0", &canonical),
        })
    }

    async fn read_empty_target_snapshot(
        &self,
        phase: TargetInstallationPhase,
    ) -> Result<(i64, i64), NodeCutoverError> {
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
        transaction
            .commit()
            .await
            .map_err(|_| NodeCutoverError::TargetRead)?;
        Ok((node_count, credential_count))
    }
}

fn require_empty_manifest(spec: &ProvisionSpec) -> Result<(), NodeCutoverError> {
    if spec.kind != ProvisionKind::LegacyReferenceMigration {
        return Err(NodeCutoverError::InvalidManifestInventory);
    }
    let nodes = &spec
        .legacy_apply_execution()
        .ok_or(NodeCutoverError::MissingExecution)?
        .nodes;
    if !nodes.inventory.is_empty()
        || !matches!(
            nodes.activation_transport,
            LegacyNodeActivationTransportSpec::NotRequiredNoNodes
        )
    {
        return Err(NodeCutoverError::InvalidManifestInventory);
    }
    Ok(())
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
}
