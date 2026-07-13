use std::collections::{BTreeMap, BTreeSet};

use redis::aio::ConnectionManager;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sqlx::{AssertSqlSafe, FromRow, MySql, MySqlPool, PgPool, QueryBuilder, Row};
use url::Url;
use uuid::Uuid;
use v2board_db::{DbPoolConfig, connect_postgres_with_config};

use crate::legacy_apply_capability::{
    PRODUCTION_LEGACY_APPLY_CAPABILITY, ProductionLegacyApplyCapability,
    production_legacy_apply_capability_for_spec,
};
use crate::legacy_backup::{
    BackupRestorePrerequisiteInspection, inspect_backup_restore_prerequisites,
};
use crate::legacy_mysql::connect_legacy_mysql_with_config;
use crate::manifest::{
    ClickHouseTargetSpec, FreshInstallAttestationSpec, LegacyAttestationSpec,
    NativeInstallationSpec, NativeUpgradeAttestationSpec, NativeUpgradeChangeSpec,
    NativeUpgradeDecisionSpec, NativeUpgradeImpactSpec, PostgresTargetSpec, ProvisionFlow,
    ProvisionKind, ProvisionSpec, SourceSpec, SourceTransportSecurity, TargetSpec,
};
use crate::native_activation::{
    ReadOnlyReleaseArchiveInspection, inspect_release_archive_read_only,
};
use crate::native_legacy_source::{
    PreAuthorizationSourceControlInspection, SourceError, inspect_pre_authorization_source_control,
};
use crate::native_node_cutover::{
    NodeCutoverError, NodeCutoverProductionBlocker, PreAuthorizationNodeInventorySummary,
    inspect_pre_authorization_node_inventory,
};

const CORE_LEGACY_TABLES: &[&str] = &[
    "v2_commission_log",
    "v2_coupon",
    "v2_giftcard",
    "v2_invite_code",
    "v2_knowledge",
    "v2_log",
    "v2_mail_log",
    "v2_notice",
    "v2_order",
    "v2_payment",
    "v2_plan",
    "v2_server_anytls",
    "v2_server_group",
    "v2_server_hysteria",
    "v2_server_route",
    "v2_server_shadowsocks",
    "v2_server_trojan",
    "v2_server_tuic",
    "v2_server_v2node",
    "v2_server_vless",
    "v2_server_vmess",
    "v2_stat",
    "v2_stat_server",
    "v2_stat_user",
    "v2_ticket",
    "v2_ticket_message",
    "v2_user",
];

// Generated from the pinned reference install.sql in a disposable database by
// `print_reviewed_legacy_profile_hash`. Canonicalization excludes failed_jobs,
// integer display widths, and server-version-dependent default collations. It
// retains the declared character-set family and all core structural objects.
const LEGACY_SCHEMA_SHA256_V1: Option<&str> =
    Some("4b5eaec681531751c79b48188e5a1c665df4f660dffbb88d6853cea6cf04801e");
const REDIS_SCAN_COUNT: usize = 256;
const MAX_REDIS_SCAN_PAGE_KEYS: usize = 4_096;
const MAX_REDIS_KEY_BYTES: usize = 4_096;
const MAX_REDIS_SCAN_PAGE_BYTES: usize = 4 * 1024 * 1024;
const LEGACY_JSON_SCAN_PAGE_SIZE: usize = 1_000;
const LEGACY_JSON_REFERENCE_BATCH_SIZE: usize = 1_000;
const CONVERTER_AVAILABLE: bool = true;
const PROVISION_REPORT_VERSION: u32 = 5;

#[derive(Serialize)]
pub struct ProvisionPlan {
    pub report_version: u32,
    pub scope: &'static str,
    pub kind: ProvisionKind,
    pub converter_available: bool,
    pub apply_available: bool,
    pub operation_id: String,
    pub manifest_binding_hmac_sha256: String,
    pub review_binding_sha256: String,
    pub review_binding_hmac_sha256: String,
    pub report_sha256: String,
    pub report_binding_hmac_sha256: String,
    pub verdict: PreflightVerdict,
    pub next_action: NextAction,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub operator_attestations_complete: Option<bool>,
    pub source: Option<DatabaseInspection>,
    pub target_postgres: Option<PostgresInspection>,
    pub target_clickhouse: Option<ClickHouseInspection>,
    pub data: Option<DataInspection>,
    pub source_redis: Option<SourceRedisInspection>,
    pub target_redis: Option<TargetRedisInspection>,
    pub node_inventory: Option<PreAuthorizationNodeInventorySummary>,
    pub backup_restore: Option<BackupRestorePrerequisiteInspection>,
    pub source_control: Option<PreAuthorizationSourceControlInspection>,
    pub release_archive: Option<ReadOnlyReleaseArchiveInspection>,
    pub native_upgrade: Option<NativeUpgradeInspection>,
    pub implementation_blockers: Vec<String>,
    pub blockers: Vec<String>,
    pub pending_final_requirements: Vec<String>,
    pub warnings: Vec<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PreflightVerdict {
    Blocked,
    Ready,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum NextAction {
    ResolveBlockers,
    AuthorizeApply,
}

#[derive(Clone, Copy, Eq, PartialEq)]
pub enum InspectionMode {
    Online,
    FencedFinal,
}

impl InspectionMode {
    const fn scope(self) -> &'static str {
        match self {
            Self::Online => "online_read_only_compatibility_inspection",
            Self::FencedFinal => "fenced_read_only_final_plan",
        }
    }
}

#[derive(Serialize)]
pub struct DatabaseInspection {
    pub vendor: DatabaseVendor,
    pub version: String,
    pub version_comment: String,
    pub database_name: String,
    pub server_uuid: String,
    pub server_uuid_valid: bool,
    pub replication_channel_count: Option<i64>,
    pub group_replication_member_count: Option<i64>,
    pub registered_replica_count: Option<i64>,
    pub global_sql_mode: String,
    pub inspector_session_sql_mode: String,
    pub inspector_transaction_read_only: bool,
    pub character_set: String,
    pub collation: String,
    pub core_table_count: usize,
    pub missing_core_tables: Vec<String>,
    pub unexpected_source_tables: Vec<String>,
    pub native_migration_ledger_present: bool,
    pub view_count: i64,
    pub routine_count: i64,
    pub event_count: i64,
    pub trigger_count: i64,
    pub semantic_schema_sha256: String,
    pub semantic_profile_match: bool,
    pub source_grants_sha256: String,
    pub source_grants_are_read_only_and_complete: bool,
    pub visible_non_source_schema_count: u64,
    pub physical_instance_contains_only_source_schema: bool,
}

#[derive(Serialize)]
pub struct PostgresInspection {
    pub version: String,
    pub server_version_num: i32,
    pub major_version_18: bool,
    pub bootstrap_database_name: String,
    pub declared_bootstrap_username: String,
    pub current_username: String,
    pub server_encoding: String,
    pub collation: String,
    pub ctype: String,
    pub bootstrap_can_create_database: bool,
    pub bootstrap_can_create_roles: bool,
    pub bootstrap_has_database_create: bool,
    pub jsonb_available: bool,
    pub sha256_available: bool,
    pub fsync_enabled: bool,
    pub full_page_writes_enabled: bool,
    pub synchronous_commit_enabled: bool,
    pub data_checksums_enabled: bool,
    pub wal_level_sufficient: bool,
    pub archive_mode_enabled: bool,
    pub archive_command_configured: bool,
    pub tls_in_use: bool,
    pub target_database_name: String,
    pub target_role_names: Vec<String>,
    pub database_absent: bool,
    pub roles_absent: bool,
    pub desired_collation: String,
    pub desired_ctype: String,
    pub pg_hba_managed_externally: bool,
    pub pg_hba_evidence: String,
    pub network_policy_managed_externally: bool,
    pub network_policy_evidence: String,
}

#[derive(Serialize)]
pub struct ClickHouseInspection {
    pub version: String,
    pub version_26_3_lts: bool,
    pub bootstrap_username: String,
    pub current_username: String,
    pub target_database_name: String,
    pub database_absent: bool,
    pub target_principal_names: Vec<String>,
    pub principals_absent: bool,
    pub replicated_table_count: u64,
    pub configured_cluster_count: u64,
    pub standalone_non_replicated: bool,
    pub bootstrap_grants: Vec<String>,
    pub bootstrap_grants_sufficient: bool,
    pub schema_has_ddl_metadata_read_and_ledger_write_only: bool,
    pub writer_is_insert_and_verify_only: bool,
    pub reader_is_select_only: bool,
    pub privilege_declaration_complete: bool,
    pub privilege_evidence: String,
    pub network_policy_evidence: String,
    pub raw_retention_days: u32,
    pub aggregate_retention_days: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DatabaseVendor {
    MySql,
    Unsupported,
}

#[derive(Serialize)]
pub struct DataInspection {
    pub unfinished_orders: i64,
    pub paid_pending_orders: i64,
    pub users_with_multiple_unfinished_orders: i64,
    pub stripe_payment_rows: i64,
    pub unfinished_stripe_orders: i64,
    pub unfinished_payment_orphans: i64,
    pub node_count: i64,
    pub visible_node_count: i64,
    pub failed_job_count: i64,
    pub malformed_giftcard_redemptions: i64,
    pub giftcard_redemption_orphans: i64,
    pub business_invariant_violations: i64,
    pub relational_integrity_violations: i64,
    pub node_group_violations: i64,
    pub target_collation_unique_collisions: i64,
    pub legacy_json_id_arrays: LegacyJsonIdArrayInspection,
}

#[derive(Default, Serialize)]
pub struct LegacyJsonIdArrayInspection {
    pub node_group_ids: LegacyJsonIdArrayColumnInspection,
    pub coupon_limit_plan_ids: LegacyJsonIdArrayColumnInspection,
    pub order_surplus_order_ids: LegacyJsonIdArrayColumnInspection,
    pub giftcard_used_user_ids: LegacyJsonIdArrayColumnInspection,
}

impl LegacyJsonIdArrayInspection {
    fn requires_normalization(&self) -> i64 {
        [
            &self.node_group_ids,
            &self.coupon_limit_plan_ids,
            &self.order_surplus_order_ids,
            &self.giftcard_used_user_ids,
        ]
        .into_iter()
        .fold(0_i64, |total, column| {
            total.saturating_add(column.requires_normalization)
        })
    }

    fn violations(&self) -> i64 {
        [
            &self.node_group_ids,
            &self.coupon_limit_plan_ids,
            &self.order_surplus_order_ids,
            &self.giftcard_used_user_ids,
        ]
        .into_iter()
        .fold(0_i64, |total, column| {
            total.saturating_add(column.violations)
        })
    }
}

#[derive(Default, Serialize)]
pub struct LegacyJsonIdArrayColumnInspection {
    pub rows_scanned: i64,
    pub sql_null_rows: i64,
    pub array_rows: i64,
    pub requires_normalization: i64,
    pub format_violations: i64,
    pub missing_reference_violations: i64,
    pub violations: i64,
}

impl LegacyJsonIdArrayColumnInspection {
    fn merge(&mut self, other: Self) {
        self.rows_scanned = self.rows_scanned.saturating_add(other.rows_scanned);
        self.sql_null_rows = self.sql_null_rows.saturating_add(other.sql_null_rows);
        self.array_rows = self.array_rows.saturating_add(other.array_rows);
        self.requires_normalization = self
            .requires_normalization
            .saturating_add(other.requires_normalization);
        self.format_violations = self
            .format_violations
            .saturating_add(other.format_violations);
        self.missing_reference_violations = self
            .missing_reference_violations
            .saturating_add(other.missing_reference_violations);
        self.refresh_violations();
    }

    fn refresh_violations(&mut self) {
        self.violations = self
            .format_violations
            .saturating_add(self.missing_reference_violations);
    }
}

#[derive(Serialize)]
pub struct SourceRedisInspection {
    pub source_default_run_id: String,
    pub source_cache_run_id: String,
    pub source_default_role: String,
    pub source_cache_role: String,
    pub source_default_connected_replicas: Option<u64>,
    pub source_cache_connected_replicas: Option<u64>,
    pub source_default_cluster_enabled: Option<bool>,
    pub source_cache_cluster_enabled: Option<bool>,
    pub source_default_key_count: u64,
    pub source_cache_key_count: u64,
    pub source_default_unclassified_key_count: u64,
    pub source_cache_unclassified_key_count: u64,
    pub source_default_other_logical_database_keys: u64,
    pub source_cache_other_logical_database_keys: u64,
    pub physical_redis_ownership_complete: bool,
    pub upload_traffic_fields: u64,
    pub download_traffic_fields: u64,
    pub upload_traffic_sum: String,
    pub download_traffic_sum: String,
    pub malformed_traffic_values: u64,
    pub unexpected_traffic_key_candidates: u64,
    pub traffic_reset_lock_keys: u64,
    pub queued_item_count: u64,
    pub queue_notify_item_count: u64,
    pub ambiguous_queue_key_candidates: u64,
    pub retryable_failed_job_items: u64,
    pub legacy_subscription_token_keys: u64,
    pub ambiguous_subscription_token_keys: u64,
}

#[derive(Serialize)]
pub struct TargetRedisInspection {
    pub key_count: u64,
    pub target_database_index: u32,
    pub target_version: String,
    pub target_run_id: String,
    pub target_role: String,
    pub target_connected_replicas: Option<u64>,
    pub target_cluster_enabled: Option<bool>,
    pub target_redis_6_2_or_newer: bool,
    pub target_getdel_available: bool,
    pub target_evalsha_available: bool,
    pub target_script_available: bool,
}

#[derive(Serialize)]
pub struct NativeUpgradeInspection {
    pub installation_id: String,
    pub current_build_id: String,
    pub target_build_id: String,
    pub current_postgres_schema_epoch: u64,
    pub target_postgres_schema_epoch: u64,
    pub current_clickhouse_schema_epoch: u64,
    pub target_clickhouse_schema_epoch: u64,
    pub schema_has_ddl_metadata_read_and_ledger_write_only: bool,
    pub writer_is_insert_and_verify_only: bool,
    pub reader_is_select_only: bool,
    pub clickhouse_privilege_evidence: String,
    pub destructive_changes: Vec<NativeUpgradeImpactSpec>,
    pub ttl_shortening: Vec<NativeUpgradeImpactSpec>,
    pub drop_operations: Vec<NativeUpgradeImpactSpec>,
    pub repartition_operations: Vec<NativeUpgradeImpactSpec>,
    pub backup_reference: Option<String>,
    pub restore_tested: bool,
    pub impact_reviewed: bool,
    pub second_confirmation_present: bool,
}

#[derive(Debug, thiserror::Error)]
pub enum ProvisionPlanError {
    #[error("source database inspection failed")]
    SourceDatabase(#[source] sqlx::Error),
    #[error("target database inspection failed")]
    TargetDatabase(#[source] v2board_db::DbInitError),
    #[error("source database query failed")]
    SourceQuery(#[source] sqlx::Error),
    #[error("target database query failed")]
    TargetQuery(#[source] sqlx::Error),
    #[error("target ClickHouse inspection failed")]
    TargetClickHouse(#[source] clickhouse::error::Error),
    #[error("source Redis inspection failed")]
    SourceRedis(#[source] redis::RedisError),
    #[error("target Redis inspection failed")]
    TargetRedis(#[source] redis::RedisError),
    #[error("legacy node inventory inspection failed")]
    NodeInventory(#[source] NodeCutoverError),
    #[error("legacy source-control inspection failed")]
    SourceControl(#[source] SourceError),
}

#[derive(FromRow)]
struct ServerRow {
    database_name: Option<String>,
    version: String,
    version_comment: String,
    global_sql_mode: String,
    session_sql_mode: String,
    character_set: String,
    collation: String,
}

#[derive(FromRow)]
struct PostgresServerRow {
    database_name: String,
    current_username: String,
    version: String,
    server_version_num: i32,
    server_encoding: String,
    collation: String,
    ctype: String,
    can_create_database: bool,
    can_create_roles: bool,
    has_database_create: bool,
    fsync_enabled: bool,
    full_page_writes_enabled: bool,
    synchronous_commit_enabled: bool,
    data_checksums_enabled: bool,
    wal_level_sufficient: bool,
    archive_mode_enabled: bool,
    archive_command_configured: bool,
    tls_in_use: bool,
}

#[derive(Deserialize, clickhouse::Row)]
struct ClickHouseServerRow {
    version: String,
    current_username: String,
}

#[derive(Deserialize, clickhouse::Row)]
struct ClickHouseCountRow {
    value: u64,
}

#[derive(Deserialize, clickhouse::Row)]
struct ClickHouseGrantRow {
    access_type: String,
    database: Option<String>,
    table: Option<String>,
    grant_option: u8,
}

#[derive(Default)]
struct MysqlTopology {
    replication_channel_count: Option<i64>,
    group_replication_member_count: Option<i64>,
    registered_replica_count: Option<i64>,
}

impl MysqlTopology {
    fn is_standalone_visible(&self) -> bool {
        self.replication_channel_count == Some(0)
            && self.group_replication_member_count == Some(0)
            && self.registered_replica_count == Some(0)
    }
}

#[derive(FromRow)]
struct TableRow {
    table_name: String,
    engine: Option<String>,
    table_collation: Option<String>,
}

#[derive(FromRow)]
struct ColumnRow {
    table_name: String,
    ordinal_position: u64,
    column_name: String,
    column_type: String,
    is_nullable: String,
    column_default: Option<String>,
    extra: String,
    character_set_name: Option<String>,
    generation_expression: String,
}

#[derive(FromRow)]
struct IndexRow {
    table_name: String,
    index_name: String,
    non_unique: i64,
    seq_in_index: i64,
    column_name: Option<String>,
    sub_part: Option<i64>,
    collation: Option<String>,
    index_type: String,
}

#[derive(FromRow)]
struct ConstraintRow {
    table_name: String,
    constraint_name: String,
    constraint_type: String,
}

#[derive(FromRow)]
struct TriggerRow {
    trigger_name: String,
    event_manipulation: String,
    event_object_table: String,
    action_timing: String,
}
pub async fn build_inspection(
    spec: &ProvisionSpec,
    mode: InspectionMode,
) -> Result<ProvisionPlan, ProvisionPlanError> {
    match &spec.flow {
        ProvisionFlow::FreshInstall {
            target,
            decisions: _,
            attestations,
            ..
        } => build_fresh_install_inspection(spec, target, attestations, mode).await,
        ProvisionFlow::LegacyReferenceMigration {
            source,
            target,
            attestations,
            ..
        } => {
            build_legacy_migration_inspection(spec, source, target, attestations.as_ref(), mode)
                .await
        }
        ProvisionFlow::NativeUpgrade {
            current,
            changes,
            decisions,
            attestations,
            ..
        } => Ok(build_native_upgrade_plan(
            spec,
            current,
            changes,
            decisions,
            attestations,
            mode,
        )),
    }
}

#[derive(Serialize)]
pub(crate) struct TargetBundle {
    pub(crate) postgres: PostgresInspection,
    pub(crate) clickhouse: ClickHouseInspection,
    pub(crate) redis: TargetRedisInspection,
}

async fn build_fresh_install_inspection(
    spec: &ProvisionSpec,
    target: &TargetSpec,
    attestations: &FreshInstallAttestationSpec,
    mode: InspectionMode,
) -> Result<ProvisionPlan, ProvisionPlanError> {
    let target_bundle = inspect_target_bundle(target).await?;
    let mut blockers = Vec::new();
    let mut pending_final_requirements = Vec::new();
    append_target_blockers(&target_bundle, &mut blockers);
    let operator_attestations_complete =
        attestations.target_capacity_reviewed && attestations.external_controls_reviewed;
    if !operator_attestations_complete {
        record_final_requirement(
            mode,
            &mut blockers,
            &mut pending_final_requirements,
            "fresh-install capacity and external-control attestations are incomplete",
        );
    }
    let implementation_blockers = vec![
        "fresh-install PostgreSQL role/database bootstrap is not implemented".to_string(),
        "fresh-install ClickHouse principal/database bootstrap and retention policy apply are not implemented"
            .to_string(),
        "fresh-install role-owned API/worker 0700 directories, 0600 config writes, operation journal, verification, and atomic bare-metal activation are not implemented"
            .to_string(),
    ];
    let plan = ProvisionPlan {
        report_version: PROVISION_REPORT_VERSION,
        scope: mode.scope(),
        kind: spec.kind,
        converter_available: CONVERTER_AVAILABLE,
        apply_available: production_legacy_apply_capability_for_spec(spec).is_available(),
        operation_id: spec.operation_id.clone(),
        manifest_binding_hmac_sha256: spec.manifest_binding_hmac_sha256().to_string(),
        review_binding_sha256: String::new(),
        review_binding_hmac_sha256: String::new(),
        report_sha256: String::new(),
        report_binding_hmac_sha256: String::new(),
        verdict: PreflightVerdict::Blocked,
        next_action: NextAction::ResolveBlockers,
        operator_attestations_complete: Some(operator_attestations_complete),
        source: None,
        target_postgres: Some(target_bundle.postgres),
        target_clickhouse: Some(target_bundle.clickhouse),
        data: None,
        source_redis: None,
        target_redis: Some(target_bundle.redis),
        node_inventory: None,
        backup_restore: None,
        source_control: None,
        release_archive: None,
        native_upgrade: None,
        implementation_blockers,
        blockers,
        pending_final_requirements,
        warnings: Vec::new(),
    };
    Ok(finalize_plan(spec, plan))
}

async fn build_legacy_migration_inspection(
    spec: &ProvisionSpec,
    source: &SourceSpec,
    target: &TargetSpec,
    attestations: Option<&LegacyAttestationSpec>,
    mode: InspectionMode,
) -> Result<ProvisionPlan, ProvisionPlanError> {
    // This is a complete, streaming inspection of the immutable input archive.
    // It deliberately runs before any source connection is opened. A malformed
    // archive is reported as a reviewable plan blocker and never reaches apply.
    let release_archive_result =
        (spec.schema_version == 4).then(|| inspect_release_archive_read_only(spec));
    let pool_config = inspection_pool_config();
    let source_pool = connect_legacy_mysql_with_config(&source.database_url, &pool_config)
        .await
        .map_err(ProvisionPlanError::SourceDatabase)?;
    let source_snapshot_session_nonce = begin_source_consistent_snapshot(&source_pool)
        .await
        .map_err(ProvisionPlanError::SourceQuery)?;
    let source_server = inspect_server(&source_pool)
        .await
        .map_err(ProvisionPlanError::SourceQuery)?;
    let source_database_name = source_server.database_name.clone().unwrap_or_default();
    let source_access = inspect_source_mysql_access(&source_pool, &source_database_name)
        .await
        .map_err(ProvisionPlanError::SourceQuery)?;
    // `@@transaction_read_only` describes the session default for future
    // transactions, not the READ ONLY characteristic of the transaction that
    // is already open.
    // Reaching this point proves that the server accepted the explicit
    // `START ... READ ONLY` statement below; the source principal is
    // independently restricted to the reviewed read-only grant set.
    let source_transaction_read_only = true;
    let source_vendor = database_vendor(&source_server.version, &source_server.version_comment);
    let source_server_uuid_raw = if source_vendor == DatabaseVendor::MySql {
        inspect_mysql_server_uuid(&source_pool)
            .await
            .map_err(ProvisionPlanError::SourceQuery)?
    } else {
        String::new()
    };
    let source_server_uuid = canonical_mysql_server_uuid(&source_server_uuid_raw);
    let source_topology = match source_vendor {
        DatabaseVendor::MySql => inspect_mysql_topology(&source_pool).await,
        DatabaseVendor::Unsupported => MysqlTopology::default(),
    };
    let source_tables = inspect_tables(&source_pool)
        .await
        .map_err(ProvisionPlanError::SourceQuery)?;
    let schema_hash = semantic_schema_hash(&source_pool)
        .await
        .map_err(ProvisionPlanError::SourceQuery)?;
    let source_objects = inspect_non_table_objects(&source_pool)
        .await
        .map_err(ProvisionPlanError::SourceQuery)?;
    let data = inspect_data(&source_pool, &source_tables)
        .await
        .map_err(ProvisionPlanError::SourceQuery)?;
    let node_inventory = if spec.schema_version == 4 {
        Some(
            inspect_pre_authorization_node_inventory(spec, &source_pool)
                .await
                .map_err(ProvisionPlanError::NodeInventory)?,
        )
    } else {
        None
    };
    let schema_hash_after_snapshot = semantic_schema_hash(&source_pool)
        .await
        .map_err(ProvisionPlanError::SourceQuery)?;
    finish_source_consistent_snapshot(&source_pool, &source_snapshot_session_nonce)
        .await
        .map_err(ProvisionPlanError::SourceQuery)?;
    if schema_hash_after_snapshot != schema_hash {
        return Err(ProvisionPlanError::SourceQuery(sqlx::Error::Protocol(
            "legacy source schema changed during the consistent inspection snapshot".into(),
        )));
    }
    let backup_restore = if spec.schema_version == 4 && mode == InspectionMode::Online {
        Some(inspect_backup_restore_prerequisites(spec).await)
    } else {
        None
    };
    let source_control = if spec.schema_version == 4 && mode == InspectionMode::Online {
        Some(
            inspect_pre_authorization_source_control(spec)
                .await
                .map_err(ProvisionPlanError::SourceControl)?,
        )
    } else {
        None
    };
    let source_redis = inspect_source_redis(spec, source, mode)
        .await
        .map_err(ProvisionPlanError::SourceRedis)?;
    let target_bundle = inspect_target_bundle(target).await?;

    let source_table_names = source_tables
        .iter()
        .map(|row| row.table_name.as_str())
        .collect::<BTreeSet<_>>();
    let expected_core = CORE_LEGACY_TABLES.iter().copied().collect::<BTreeSet<_>>();
    let missing_core_tables = expected_core
        .difference(&source_table_names)
        .map(|value| (*value).to_string())
        .collect::<Vec<_>>();
    let allowed_source_tables = expected_core
        .iter()
        .copied()
        .chain(["failed_jobs"])
        .collect::<BTreeSet<_>>();
    let unexpected_source_tables = source_table_names
        .difference(&allowed_source_tables)
        .map(|value| (*value).to_string())
        .collect::<Vec<_>>();
    let native_migration_ledger_present = source_tables
        .iter()
        .any(|row| row.table_name == "_sqlx_migrations");
    let semantic_profile_match = LEGACY_SCHEMA_SHA256_V1 == Some(schema_hash.as_str());

    let mut blockers = Vec::new();
    let mut pending_final_requirements = Vec::new();
    let supported_source = source_vendor == DatabaseVendor::MySql
        && version_is_supported_mysql8(&source_server.version);
    if !supported_source {
        blockers.push("source must be Oracle MySQL 8.0.x or 8.4.x".to_string());
    }
    if !source_transaction_read_only {
        blockers.push("source SQL inspector session is not read-only".into());
    }
    if !source_access.grants_are_read_only_and_complete {
        blockers.push(
            "source credential must have only the reviewed source SELECT/SHOW VIEW, MySQL topology SELECT, SHOW DATABASES/PROCESS and REPLICATION CLIENT grants, and must expose the complete physical schema and replication inventory"
                .into(),
        );
    }
    if source_access.visible_non_source_schema_count != 0 {
        blockers.push(
            "the declared local MySQL systemd service contains non-system schemas outside the V2Board source database"
                .into(),
        );
    }
    if !missing_core_tables.is_empty() || !unexpected_source_tables.is_empty() {
        blockers
            .push("source core table inventory does not match the pinned legacy profile".into());
    }
    if native_migration_ledger_present {
        blockers.push("source already contains a native PostgreSQL migration ledger".into());
    }
    if source_objects.views != 0
        || source_objects.routines != 0
        || source_objects.events != 0
        || source_objects.triggers != 0
    {
        blockers.push(
            "source contains unprofiled views, routines, scheduled events, or triggers".into(),
        );
    }
    if !semantic_profile_match {
        blockers
            .push("source semantic schema fingerprint is not the reviewed legacy profile".into());
    }
    if source_vendor == DatabaseVendor::MySql && source_server_uuid.is_none() {
        blockers.push("source MySQL 8 server_uuid is missing or invalid".into());
    }
    if !source_topology.is_standalone_visible() {
        blockers.push(
            "legacy migration requires visible proof that the source SQL instance has no replication topology"
                .into(),
        );
    }
    append_target_blockers(&target_bundle, &mut blockers);
    if !source_redis_standalone(&source_redis) {
        blockers.push("legacy source Redis must be standalone and non-replicated".into());
    }
    if target_bundle
        .redis
        .target_run_id
        .eq_ignore_ascii_case(&source_redis.source_default_run_id)
        || target_bundle
            .redis
            .target_run_id
            .eq_ignore_ascii_case(&source_redis.source_cache_run_id)
    {
        blockers.push("target Redis is the same server instance as source Redis".into());
    }
    append_legacy_data_blockers(
        &data,
        &source_redis,
        mode,
        &mut blockers,
        &mut pending_final_requirements,
    );
    if let Some(inventory) = &node_inventory {
        blockers.extend(
            inventory
                .blockers
                .iter()
                .map(|blocker| format!("legacy node inventory: {}", node_blocker_code(*blocker))),
        );
    }
    if let Some(backup) = &backup_restore {
        blockers.extend(
            backup
                .blockers
                .iter()
                .map(|blocker| format!("legacy backup prerequisite: {blocker}")),
        );
    }
    if let Some(control) = &source_control
        && !control.datastore_write_fence_capabilities_ready
    {
        blockers.push(
            "source datastore fence requires an independent exact-minimum MySQL fence credential (PROCESS + SYSTEM_VARIABLES_ADMIN with persisted globals enabled) and a dedicated named Redis 6.2+ full-access lifecycle ACL user for drain plus CLIENT PAUSE WRITE"
                .into(),
        );
    }
    let release_archive = match release_archive_result {
        Some(Ok(inspection)) => Some(inspection),
        Some(Err(_)) => {
            blockers.push(
                "native release archive failed the complete read-only structure, checksum, and systemd-contract preflight"
                    .into(),
            );
            None
        }
        None => None,
    };

    let runtime_evidence_is_journaled =
        spec.schema_version == 4 && spec.legacy_apply_execution().is_some();
    let operator_attestations_complete = attestations.is_some_and(|attestations| {
        attestations.source_writers_stopped
            && attestations.source_workers_stopped
            && attestations.node_reporters_stopped
            && attestations.legacy_queues_drained
            && attestations
                .backup_reference
                .as_deref()
                .is_some_and(|reference| !reference.trim().is_empty())
            && attestations.restore_tested
    });
    if runtime_evidence_is_journaled {
        pending_final_requirements.push(
            "after the single authorization, apply must journal and verify the source fence, queue/traffic drain, encrypted backup restore drill, and fenced final recheck before creating targets"
                .to_string(),
        );
    } else if !operator_attestations_complete {
        record_final_requirement(
            mode,
            &mut blockers,
            &mut pending_final_requirements,
            "operator maintenance, drain, backup, and restore attestations are incomplete",
        );
    }

    let mut warnings = Vec::new();
    if source.transport_security == SourceTransportSecurity::TrustedMaintenanceNetwork {
        warnings.push(
            "source credentials and inspected data rely on the declared trusted maintenance network rather than verified TLS"
                .into(),
        );
    }
    if data.unfinished_orders != 0 {
        warnings
            .push("unfinished non-Stripe orders will require preserved payable bindings".into());
    }
    if data.node_count != 0 {
        warnings.push(
            "the source contains external nodes; this repository has no node-side activation coordinator, so apply remains blocked"
                .into(),
        );
    }
    if source_redis.source_cache_key_count != 0 || source_redis.source_default_key_count != 0 {
        warnings.push(
            "legacy Redis contains classified state: traffic is folded exactly once, while sessions/cache/Horizon metadata are discarded with an explicit full logout"
                .into(),
        );
    }
    if source_redis.queue_notify_item_count != 0 {
        warnings.push(
            "legacy Redis queue notify wake tokens remain; they are excluded from durable queued_item_count"
                .into(),
        );
    }
    warnings.push(
        "legacy temporary subscription URLs are intentionally not migrated; users must fetch a new URL after cutover"
            .into(),
    );
    if data.legacy_json_id_arrays.requires_normalization() != 0 {
        warnings.push(
            "legacy JSON ID arrays contain canonical positive-decimal strings that the one-shot converter will normalize to JSON numbers"
                .into(),
        );
    }

    let implementation_blockers = production_legacy_apply_implementation_blockers();
    let source_report = DatabaseInspection {
        vendor: source_vendor,
        version: source_server.version,
        version_comment: source_server.version_comment,
        database_name: source_database_name,
        server_uuid: source_server_uuid.clone().unwrap_or(source_server_uuid_raw),
        server_uuid_valid: source_server_uuid.is_some(),
        replication_channel_count: source_topology.replication_channel_count,
        group_replication_member_count: source_topology.group_replication_member_count,
        registered_replica_count: source_topology.registered_replica_count,
        global_sql_mode: source_server.global_sql_mode,
        inspector_session_sql_mode: source_server.session_sql_mode,
        inspector_transaction_read_only: source_transaction_read_only,
        character_set: source_server.character_set,
        collation: source_server.collation,
        core_table_count: source_table_names.intersection(&expected_core).count(),
        missing_core_tables,
        unexpected_source_tables,
        native_migration_ledger_present,
        view_count: source_objects.views,
        routine_count: source_objects.routines,
        event_count: source_objects.events,
        trigger_count: source_objects.triggers,
        semantic_schema_sha256: schema_hash,
        semantic_profile_match,
        source_grants_sha256: source_access.grants_sha256,
        source_grants_are_read_only_and_complete: source_access.grants_are_read_only_and_complete,
        visible_non_source_schema_count: source_access.visible_non_source_schema_count,
        physical_instance_contains_only_source_schema: source_access
            .grants_are_read_only_and_complete
            && source_access.visible_non_source_schema_count == 0,
    };
    let plan = ProvisionPlan {
        report_version: PROVISION_REPORT_VERSION,
        scope: mode.scope(),
        kind: spec.kind,
        converter_available: CONVERTER_AVAILABLE,
        apply_available: production_legacy_apply_capability_for_spec(spec).is_available(),
        operation_id: spec.operation_id.clone(),
        manifest_binding_hmac_sha256: spec.manifest_binding_hmac_sha256().to_string(),
        review_binding_sha256: String::new(),
        review_binding_hmac_sha256: String::new(),
        report_sha256: String::new(),
        report_binding_hmac_sha256: String::new(),
        verdict: PreflightVerdict::Blocked,
        next_action: NextAction::ResolveBlockers,
        operator_attestations_complete: attestations.map(|_| operator_attestations_complete),
        source: Some(source_report),
        target_postgres: Some(target_bundle.postgres),
        target_clickhouse: Some(target_bundle.clickhouse),
        data: Some(data),
        source_redis: Some(source_redis),
        target_redis: Some(target_bundle.redis),
        node_inventory,
        backup_restore,
        source_control,
        release_archive,
        native_upgrade: None,
        implementation_blockers,
        blockers,
        pending_final_requirements,
        warnings,
    };
    Ok(finalize_plan(spec, plan))
}

const fn node_blocker_code(blocker: NodeCutoverProductionBlocker) -> &'static str {
    blocker.as_str()
}

fn append_legacy_data_blockers(
    data: &DataInspection,
    source_redis: &SourceRedisInspection,
    mode: InspectionMode,
    blockers: &mut Vec<String>,
    pending: &mut Vec<String>,
) {
    if data.users_with_multiple_unfinished_orders != 0 {
        blockers.push("users with multiple unfinished orders require explicit resolution".into());
    }
    if data.paid_pending_orders != 0 {
        record_final_requirement(
            mode,
            blockers,
            pending,
            "paid orders still waiting to be opened must be drained",
        );
    }
    if data.stripe_payment_rows != 0 || data.unfinished_stripe_orders != 0 {
        blockers.push("legacy Stripe inventory is not empty".into());
    }
    if data.unfinished_payment_orphans != 0 {
        blockers.push("unfinished orders reference missing payment rows".into());
    }
    if data.failed_job_count != 0 {
        record_final_requirement(mode, blockers, pending, "legacy failed_jobs is not empty");
    }
    if data.legacy_json_id_arrays.violations() != 0 {
        blockers.push(
            "legacy JSON ID arrays contain format, target-range, or missing-reference violations"
                .into(),
        );
    }
    if data.business_invariant_violations != 0
        || data.relational_integrity_violations != 0
        || data.target_collation_unique_collisions != 0
    {
        blockers.push("legacy data does not satisfy native migration preflights".into());
    }
    if source_redis.upload_traffic_fields != 0 || source_redis.download_traffic_fields != 0 {
        record_final_requirement(
            mode,
            blockers,
            pending,
            "legacy Redis contains traffic that has not reached MySQL",
        );
    }
    if source_redis.malformed_traffic_values != 0 {
        blockers.push("legacy Redis traffic contains malformed values".into());
    }
    if source_redis.unexpected_traffic_key_candidates != 0 {
        blockers.push("legacy Redis traffic keys do not match the declared prefix".into());
    }
    if source_redis.traffic_reset_lock_keys != 0 {
        record_final_requirement(
            mode,
            blockers,
            pending,
            "legacy Redis still contains a traffic reset lock",
        );
    }
    if source_redis.queued_item_count != 0 {
        record_final_requirement(
            mode,
            blockers,
            pending,
            "legacy Redis queues are not drained",
        );
    }
    if source_redis.ambiguous_queue_key_candidates != 0 {
        blockers.push("source Redis has queue-like keys outside the declared prefix".into());
    }
    if source_redis.retryable_failed_job_items != 0 {
        record_final_requirement(
            mode,
            blockers,
            pending,
            "legacy Redis still contains retryable failed-job state",
        );
    }
    if !source_redis.physical_redis_ownership_complete {
        blockers.push(
            "source Redis contains unclassified keys or nonempty logical databases outside the declared V2Board default/cache databases"
                .into(),
        );
    }
    // otp_/otpn_/totp_ are disposable URL mappings or verification cache.
    // The permanent subscription credential is v2_user.token in MySQL and is
    // copied exactly; temporary URLs are explicitly invalidated at cutover.
}

fn build_native_upgrade_plan(
    spec: &ProvisionSpec,
    current: &NativeInstallationSpec,
    changes: &NativeUpgradeChangeSpec,
    decisions: &NativeUpgradeDecisionSpec,
    attestations: &NativeUpgradeAttestationSpec,
    mode: InspectionMode,
) -> ProvisionPlan {
    let destructive = !changes.destructive_changes.is_empty()
        || !changes.ttl_shortening.is_empty()
        || !changes.drop_operations.is_empty()
        || !changes.repartition_operations.is_empty();
    let mut blockers = vec![
        "native installation_id, build ID, PostgreSQL epoch, and ClickHouse epoch are declared but not yet machine-verified"
            .to_string(),
    ];
    let mut pending_final_requirements = Vec::new();
    if destructive && !decisions.allow_destructive_changes {
        blockers.push("destructive native changes are listed but not explicitly allowed".into());
    }
    if destructive
        && (attestations.backup_reference.is_none()
            || !attestations.restore_tested
            || !attestations.impact_reviewed)
    {
        blockers.push(
            "destructive native upgrade requires impact review and bound backup/restore proof"
                .into(),
        );
    }
    if destructive && attestations.second_confirmation.is_none() {
        blockers.push(
            "destructive native upgrade requires a second confirmation bound to a prior v3 report"
                .into(),
        );
    }
    if !attestations.maintenance_window_approved {
        record_final_requirement(
            mode,
            &mut blockers,
            &mut pending_final_requirements,
            "native upgrade maintenance window is not approved",
        );
    }
    let operator_attestations_complete = attestations.maintenance_window_approved
        && (!destructive
            || (decisions.allow_destructive_changes
                && attestations.backup_reference.is_some()
                && attestations.restore_tested
                && attestations.impact_reviewed
                && attestations.second_confirmation.is_some()));
    let implementation_blockers = vec![
        "native installation binding and current schema/build epoch inspector are not implemented"
            .to_string(),
        "native migration dry-run, destructive impact estimator, operation journal, rollback, and apply are not implemented"
            .to_string(),
        "role-owned API/worker 0700 directory and 0600 config writes plus atomic bare-metal promotion are not implemented"
            .to_string(),
    ];
    let native_upgrade = NativeUpgradeInspection {
        installation_id: current.installation_id.clone(),
        current_build_id: current.current_build_id.clone(),
        target_build_id: changes.target_build_id.clone(),
        current_postgres_schema_epoch: current.postgres_schema_epoch,
        target_postgres_schema_epoch: changes.target_postgres_schema_epoch,
        current_clickhouse_schema_epoch: current.clickhouse_schema_epoch,
        target_clickhouse_schema_epoch: changes.target_clickhouse_schema_epoch,
        schema_has_ddl_metadata_read_and_ledger_write_only: current
            .clickhouse_privileges
            .schema_has_ddl_metadata_read_and_ledger_write_only,
        writer_is_insert_and_verify_only: current
            .clickhouse_privileges
            .writer_is_insert_and_verify_only,
        reader_is_select_only: current.clickhouse_privileges.reader_is_select_only,
        clickhouse_privilege_evidence: current.clickhouse_privileges.evidence.clone(),
        destructive_changes: changes.destructive_changes.clone(),
        ttl_shortening: changes.ttl_shortening.clone(),
        drop_operations: changes.drop_operations.clone(),
        repartition_operations: changes.repartition_operations.clone(),
        backup_reference: attestations.backup_reference.clone(),
        restore_tested: attestations.restore_tested,
        impact_reviewed: attestations.impact_reviewed,
        second_confirmation_present: attestations.second_confirmation.is_some(),
    };
    finalize_plan(
        spec,
        ProvisionPlan {
            report_version: PROVISION_REPORT_VERSION,
            scope: mode.scope(),
            kind: spec.kind,
            converter_available: CONVERTER_AVAILABLE,
            apply_available: production_legacy_apply_capability_for_spec(spec).is_available(),
            operation_id: spec.operation_id.clone(),
            manifest_binding_hmac_sha256: spec.manifest_binding_hmac_sha256().to_string(),
            review_binding_sha256: String::new(),
            review_binding_hmac_sha256: String::new(),
            report_sha256: String::new(),
            report_binding_hmac_sha256: String::new(),
            verdict: PreflightVerdict::Blocked,
            next_action: NextAction::ResolveBlockers,
            operator_attestations_complete: Some(operator_attestations_complete),
            source: None,
            target_postgres: None,
            target_clickhouse: None,
            data: None,
            source_redis: None,
            target_redis: None,
            node_inventory: None,
            backup_restore: None,
            source_control: None,
            release_archive: None,
            native_upgrade: Some(native_upgrade),
            implementation_blockers,
            blockers,
            pending_final_requirements,
            warnings: Vec::new(),
        },
    )
}

fn inspection_pool_config() -> DbPoolConfig {
    DbPoolConfig {
        min_connections: 0,
        max_connections: 1,
        ..DbPoolConfig::default()
    }
}

const fn production_apply_available_with_capability(
    kind: ProvisionKind,
    capability: ProductionLegacyApplyCapability,
) -> bool {
    matches!(kind, ProvisionKind::LegacyReferenceMigration) && capability.is_available()
}

fn production_legacy_apply_implementation_blockers() -> Vec<String> {
    PRODUCTION_LEGACY_APPLY_CAPABILITY
        .blocker()
        .map(|blocker| blocker.report_message().to_string())
        .into_iter()
        .collect()
}

async fn begin_source_consistent_snapshot(pool: &MySqlPool) -> Result<String, sqlx::Error> {
    // The inspection pool is deliberately constrained to one connection.
    // Starting the transaction with a raw statement keeps that same session
    // pinned logically across the pool-based inspection helpers without
    // allowing any second connection to observe a different MVCC snapshot.
    let nonce = Uuid::new_v4().hyphenated().to_string();
    sqlx::query("SET @v2board_inspection_session_nonce = ?")
        .bind(&nonce)
        .execute(pool)
        .await?;
    // MySQL rejects START TRANSACTION through COM_STMT_PREPARE (error 1295),
    // so transaction control must use the text protocol. All data queries
    // remain prepared statements on this same nonce-bound session.
    sqlx::raw_sql("START TRANSACTION WITH CONSISTENT SNAPSHOT, READ ONLY")
        .execute(pool)
        .await?;
    Ok(nonce)
}

async fn finish_source_consistent_snapshot(
    pool: &MySqlPool,
    expected_nonce: &str,
) -> Result<(), sqlx::Error> {
    let observed_nonce =
        sqlx::query_scalar::<_, Option<String>>("SELECT @v2board_inspection_session_nonce")
            .fetch_one(pool)
            .await?;
    if observed_nonce.as_deref() != Some(expected_nonce) {
        return Err(sqlx::Error::Protocol(
            "legacy source inspection changed MySQL sessions during its snapshot".into(),
        ));
    }
    sqlx::raw_sql("COMMIT").execute(pool).await?;
    Ok(())
}

fn finalize_plan(spec: &ProvisionSpec, mut plan: ProvisionPlan) -> ProvisionPlan {
    set_plan_outcome(&mut plan, production_legacy_apply_capability_for_spec(spec));
    let review_bytes = inspection_review_binding_bytes(&plan);
    let mut review_digest = Sha256::new();
    review_digest.update(b"v2board-provision-inspection-review-v1\0");
    review_digest.update((review_bytes.len() as u64).to_be_bytes());
    review_digest.update(&review_bytes);
    plan.review_binding_sha256 = hex::encode(review_digest.finalize());
    let mut hmac_material = b"v2board-provision-inspection-review-hmac-v1\0".to_vec();
    hmac_material.extend_from_slice(&(review_bytes.len() as u64).to_be_bytes());
    hmac_material.extend_from_slice(&review_bytes);
    plan.review_binding_hmac_sha256 = spec.report_binding_hmac_sha256(&hmac_material);
    // A JSON document cannot literally contain its own digest. The canonical
    // report payload is the complete finalized inspection with the two report
    // digest output slots still empty; the printed document then carries the
    // values produced from those exact bytes.
    debug_assert!(plan.report_sha256.is_empty());
    debug_assert!(plan.report_binding_hmac_sha256.is_empty());
    let report_payload = serde_json::to_vec(&plan).expect("provision plan is serializable");
    plan.report_sha256 = hex::encode(Sha256::digest(&report_payload));
    plan.report_binding_hmac_sha256 = spec.report_binding_hmac_sha256(&report_payload);
    plan
}

fn set_plan_outcome(plan: &mut ProvisionPlan, capability: ProductionLegacyApplyCapability) {
    plan.apply_available = production_apply_available_with_capability(plan.kind, capability);
    let ready = plan.kind == ProvisionKind::LegacyReferenceMigration
        && plan.converter_available
        && plan.apply_available
        && legacy_inspection_sections_complete(plan)
        && plan.implementation_blockers.is_empty()
        && plan.blockers.is_empty();
    plan.verdict = if ready {
        PreflightVerdict::Ready
    } else {
        PreflightVerdict::Blocked
    };
    plan.next_action = if ready {
        NextAction::AuthorizeApply
    } else {
        NextAction::ResolveBlockers
    };
}

fn legacy_inspection_sections_complete(plan: &ProvisionPlan) -> bool {
    plan.source.is_some()
        && plan.target_postgres.is_some()
        && plan.target_clickhouse.is_some()
        && plan.data.is_some()
        && plan.source_redis.is_some()
        && plan.target_redis.is_some()
        && plan.node_inventory.is_some()
        && plan.backup_restore.is_some()
        && plan.source_control.is_some()
        && plan.release_archive.is_some()
        && plan.native_upgrade.is_none()
}

fn inspection_review_binding_bytes(plan: &ProvisionPlan) -> Vec<u8> {
    let source_redis_identity = plan.source_redis.as_ref().map(|redis| {
        serde_json::json!({
            "source_default_run_id": redis.source_default_run_id,
            "source_cache_run_id": redis.source_cache_run_id,
            "source_default_role": redis.source_default_role,
            "source_cache_role": redis.source_cache_role,
            "source_default_connected_replicas": redis.source_default_connected_replicas,
            "source_cache_connected_replicas": redis.source_cache_connected_replicas,
            "source_default_cluster_enabled": redis.source_default_cluster_enabled,
            "source_cache_cluster_enabled": redis.source_cache_cluster_enabled,
            "physical_redis_ownership_complete": redis.physical_redis_ownership_complete,
        })
    });
    let node_inventory = plan.node_inventory.as_ref().map(|nodes| {
        serde_json::json!({
            "summary_sha256": nodes.summary_sha256,
            "legacy_source_node_set_sha256": nodes.legacy_source_node_set_sha256,
            "manifest_inventory_empty": nodes.manifest_inventory_empty,
            "exact_legacy_source_match": nodes.exact_legacy_source_match,
        })
    });
    let backup_restore = plan.backup_restore.as_ref().map(|backup| {
        serde_json::json!({
            "fixed_root_owned_binaries_ready": backup.fixed_root_owned_binaries_ready,
            "recipient_digest_ready": backup.recipient_digest_ready,
            "runtime_identity_digest_ready": backup.runtime_identity_digest_ready,
            "source_server_version": backup.source_server_version,
            "source_server_version_comment": backup.source_server_version_comment,
            "source_mysql8_supported": backup.source_mysql8_supported,
            "command_limits_valid": backup.command_limits_valid,
            "maximum_encrypted_backup_bytes": backup.maximum_encrypted_backup_bytes,
            "output_capacity_ready": backup.output_capacity_ready,
            "restore_admin_connected": backup.restore_admin_connected,
            "restore_server_version": backup.restore_server_version,
            "restore_server_version_comment": backup.restore_server_version_comment,
            "restore_server_supported": backup.restore_server_supported,
            "restore_identity_distinct": backup.restore_identity_distinct,
            "restore_database_absent_or_empty": backup.restore_database_absent_or_empty,
            "restore_create_drop_privileges_observed": backup.restore_create_drop_privileges_observed,
        })
    });
    serde_json::to_vec(&serde_json::json!({
        "review_version": 2,
        "report_version": plan.report_version,
        "scope": plan.scope,
        "kind": plan.kind,
        "operation_id": plan.operation_id,
        "manifest_binding_hmac_sha256": plan.manifest_binding_hmac_sha256,
        "converter_available": plan.converter_available,
        "apply_available": plan.apply_available,
        "source": plan.source,
        "target_postgres": plan.target_postgres,
        "target_clickhouse": plan.target_clickhouse,
        "target_redis": plan.target_redis,
        "source_redis_identity": source_redis_identity,
        "source_control": plan.source_control,
        "release_archive": plan.release_archive,
        "node_inventory": node_inventory,
        "backup_restore": backup_restore,
        "implementation_blockers": plan.implementation_blockers,
    }))
    .expect("inspection review binding is serializable")
}

pub(crate) async fn inspect_target_bundle(
    target: &TargetSpec,
) -> Result<TargetBundle, ProvisionPlanError> {
    let pool_config = inspection_pool_config();
    let postgres_pool =
        connect_postgres_with_config(&target.postgres.bootstrap_database_url, &pool_config)
            .await
            .map_err(ProvisionPlanError::TargetDatabase)?;
    let postgres = inspect_target_postgres(&postgres_pool, &target.postgres)
        .await
        .map_err(ProvisionPlanError::TargetQuery)?;
    let clickhouse = inspect_target_clickhouse(&target.clickhouse)
        .await
        .map_err(ProvisionPlanError::TargetClickHouse)?;
    let redis = inspect_target_redis(&target.redis_url)
        .await
        .map_err(ProvisionPlanError::TargetRedis)?;
    Ok(TargetBundle {
        postgres,
        clickhouse,
        redis: TargetRedisInspection {
            key_count: redis.key_count,
            target_database_index: redis.database_index,
            target_version: redis.version,
            target_run_id: redis.identity.run_id,
            target_role: redis.identity.role,
            target_connected_replicas: redis.identity.connected_replicas,
            target_cluster_enabled: redis.identity.cluster_enabled,
            target_redis_6_2_or_newer: redis.redis_6_2_or_newer,
            target_getdel_available: redis.getdel_available,
            target_evalsha_available: redis.evalsha_available,
            target_script_available: redis.script_available,
        },
    })
}

async fn inspect_target_postgres(
    pool: &PgPool,
    spec: &PostgresTargetSpec,
) -> Result<PostgresInspection, sqlx::Error> {
    let server = sqlx::query_as::<_, PostgresServerRow>(
        r#"
        SELECT current_database()::text AS database_name,
               current_user::text AS current_username,
               current_setting('server_version') AS version,
               current_setting('server_version_num')::integer AS server_version_num,
               pg_encoding_to_char(d.encoding)::text AS server_encoding,
               d.datcollate::text AS collation,
               d.datctype::text AS ctype,
               (r.rolsuper OR r.rolcreatedb) AS can_create_database,
               (r.rolsuper OR r.rolcreaterole) AS can_create_roles,
               has_database_privilege(current_user, current_database(), 'CREATE') AS has_database_create,
               current_setting('fsync') = 'on' AS fsync_enabled,
               current_setting('full_page_writes') = 'on' AS full_page_writes_enabled,
               current_setting('synchronous_commit') = 'on' AS synchronous_commit_enabled,
               current_setting('data_checksums') = 'on' AS data_checksums_enabled,
               current_setting('wal_level') IN ('replica', 'logical') AS wal_level_sufficient,
               current_setting('archive_mode') IN ('on', 'always') AS archive_mode_enabled,
               (
                   btrim(current_setting('archive_command')) NOT IN ('', '(disabled)')
                   OR btrim(COALESCE(current_setting('archive_library', true), ''))
                       NOT IN ('', '(disabled)')
               ) AS archive_command_configured,
               EXISTS (
                   SELECT 1 FROM pg_stat_ssl
                   WHERE pid = pg_backend_pid() AND ssl
               ) AS tls_in_use
        FROM pg_database d
        JOIN pg_roles r ON r.rolname = current_user
        WHERE d.datname = current_database()
        "#,
    )
    .fetch_one(pool)
    .await?;
    let target_database_name = {
        let url = url::Url::parse(&spec.api_database_url).expect("validated PostgreSQL URL");
        url.path().trim_start_matches('/').to_string()
    };
    let target_role_names = [
        &spec.migration_database_url,
        &spec.api_database_url,
        &spec.worker_database_url,
    ]
    .iter()
    .map(|value| {
        url::Url::parse(value)
            .expect("validated PostgreSQL URL")
            .username()
            .to_string()
    })
    .collect::<Vec<_>>();
    let database_absent: bool =
        sqlx::query_scalar("SELECT NOT EXISTS (SELECT 1 FROM pg_database WHERE datname = $1)")
            .bind(&target_database_name)
            .fetch_one(pool)
            .await?;
    let roles_absent: bool = sqlx::query_scalar(
        "SELECT NOT EXISTS (SELECT 1 FROM pg_roles WHERE rolname IN ($1, $2, $3))",
    )
    .bind(&target_role_names[0])
    .bind(&target_role_names[1])
    .bind(&target_role_names[2])
    .fetch_one(pool)
    .await?;
    let jsonb_available: bool = sqlx::query_scalar(
        "SELECT jsonb_build_object('provision_probe', true) = '{\"provision_probe\": true}'::jsonb",
    )
    .fetch_one(pool)
    .await?;
    let sha256_available: bool = sqlx::query_scalar(
        "SELECT octet_length(sha256(convert_to('provision_probe', 'UTF8'))) = 32",
    )
    .fetch_one(pool)
    .await?;
    let declared_bootstrap_username = url::Url::parse(&spec.bootstrap_database_url)
        .expect("validated PostgreSQL URL")
        .username()
        .to_string();
    Ok(PostgresInspection {
        version: server.version,
        server_version_num: server.server_version_num,
        major_version_18: server.server_version_num / 10_000 == 18,
        bootstrap_database_name: server.database_name,
        declared_bootstrap_username,
        current_username: server.current_username,
        server_encoding: server.server_encoding,
        collation: server.collation,
        ctype: server.ctype,
        bootstrap_can_create_database: server.can_create_database,
        bootstrap_can_create_roles: server.can_create_roles,
        bootstrap_has_database_create: server.has_database_create,
        jsonb_available,
        sha256_available,
        fsync_enabled: server.fsync_enabled,
        full_page_writes_enabled: server.full_page_writes_enabled,
        synchronous_commit_enabled: server.synchronous_commit_enabled,
        data_checksums_enabled: server.data_checksums_enabled,
        wal_level_sufficient: server.wal_level_sufficient,
        archive_mode_enabled: server.archive_mode_enabled,
        archive_command_configured: server.archive_command_configured,
        tls_in_use: server.tls_in_use,
        target_database_name,
        target_role_names,
        database_absent,
        roles_absent,
        desired_collation: spec.database_collation.clone(),
        desired_ctype: spec.database_ctype.clone(),
        pg_hba_managed_externally: spec.external_access.pg_hba_managed_externally,
        pg_hba_evidence: spec.external_access.pg_hba_evidence.clone(),
        network_policy_managed_externally: spec.external_access.network_policy_managed_externally,
        network_policy_evidence: spec.external_access.network_policy_evidence.clone(),
    })
}

async fn inspect_target_clickhouse(
    spec: &ClickHouseTargetSpec,
) -> Result<ClickHouseInspection, clickhouse::error::Error> {
    let client = clickhouse::Client::default()
        .with_url(&spec.endpoint)
        .with_database("default")
        .with_user(&spec.bootstrap_principal.username)
        .with_password(spec.bootstrap_principal.password())
        .with_setting("wait_end_of_query", "1");
    let server = client
        .query("SELECT version() AS version, currentUser() AS current_username")
        .fetch_one::<ClickHouseServerRow>()
        .await?;
    let database_count = client
        .query("SELECT count() AS value FROM system.databases WHERE name = ?")
        .bind(&spec.database)
        .fetch_one::<ClickHouseCountRow>()
        .await?
        .value;
    let target_principal_names = vec![
        spec.schema_principal.username.clone(),
        spec.writer_principal.username.clone(),
        spec.reader_principal.username.clone(),
    ];
    let principal_count = client
        .query("SELECT count() AS value FROM system.users WHERE name IN (?, ?, ?)")
        .bind(&target_principal_names[0])
        .bind(&target_principal_names[1])
        .bind(&target_principal_names[2])
        .fetch_one::<ClickHouseCountRow>()
        .await?
        .value;
    let replicated_table_count = client
        .query("SELECT count() AS value FROM system.replicas")
        .fetch_one::<ClickHouseCountRow>()
        .await?
        .value;
    let configured_cluster_count = client
        .query("SELECT uniqExact(cluster) AS value FROM system.clusters")
        .fetch_one::<ClickHouseCountRow>()
        .await?
        .value;
    let grant_rows = client
        .query(
            "SELECT toString(access_type) AS access_type, database, table, grant_option \
             FROM system.grants WHERE user_name = currentUser() \
             ORDER BY access_type, database, table",
        )
        .fetch_all::<ClickHouseGrantRow>()
        .await?;
    let bootstrap_grants_sufficient = clickhouse_bootstrap_grants_sufficient(&grant_rows);
    let bootstrap_grants = grant_rows
        .iter()
        .map(|grant| {
            format!(
                "{} ON {}.{}{}",
                grant.access_type,
                grant.database.as_deref().unwrap_or("*"),
                grant.table.as_deref().unwrap_or("*"),
                if grant.grant_option != 0 {
                    " WITH GRANT OPTION"
                } else {
                    ""
                }
            )
        })
        .collect();
    let privilege_declaration_complete = spec.privileges.bootstrap_manages_database_and_principals
        && spec
            .privileges
            .schema_has_ddl_metadata_read_and_ledger_write_only
        && spec.privileges.writer_is_insert_and_verify_only
        && spec.privileges.reader_is_select_only;
    Ok(ClickHouseInspection {
        version_26_3_lts: version_family(&server.version) == Some((26, 3)),
        version: server.version,
        bootstrap_username: spec.bootstrap_principal.username.clone(),
        current_username: server.current_username,
        target_database_name: spec.database.clone(),
        database_absent: database_count == 0,
        target_principal_names,
        principals_absent: principal_count == 0,
        replicated_table_count,
        configured_cluster_count,
        standalone_non_replicated: replicated_table_count == 0 && configured_cluster_count == 0,
        bootstrap_grants,
        bootstrap_grants_sufficient,
        schema_has_ddl_metadata_read_and_ledger_write_only: spec
            .privileges
            .schema_has_ddl_metadata_read_and_ledger_write_only,
        writer_is_insert_and_verify_only: spec.privileges.writer_is_insert_and_verify_only,
        reader_is_select_only: spec.privileges.reader_is_select_only,
        privilege_declaration_complete,
        privilege_evidence: spec.privileges.evidence.clone(),
        network_policy_evidence: spec.network_policy_evidence.clone(),
        raw_retention_days: spec.raw_retention_days,
        aggregate_retention_days: spec.aggregate_retention_days,
    })
}

fn clickhouse_bootstrap_grants_sufficient(grants: &[ClickHouseGrantRow]) -> bool {
    let has = |accepted: &[&str]| {
        grants.iter().any(|grant| {
            grant.grant_option != 0
                && accepted
                    .iter()
                    .any(|value| grant.access_type.eq_ignore_ascii_case(value))
        })
    };
    has(&["ALL", "CREATE", "CREATE DATABASE"])
        && has(&["ALL", "ACCESS MANAGEMENT", "CREATE USER"])
        && has(&["ALL", "ACCESS MANAGEMENT", "CREATE ROLE"])
}

fn append_target_blockers(target: &TargetBundle, blockers: &mut Vec<String>) {
    let postgres = &target.postgres;
    if !postgres.major_version_18 {
        blockers.push("target PostgreSQL must be major version 18".into());
    }
    if postgres.server_encoding != "UTF8"
        || postgres.collation != postgres.desired_collation
        || postgres.ctype != postgres.desired_ctype
    {
        blockers
            .push("target PostgreSQL encoding/collation/ctype do not match UTF8 C.UTF-8".into());
    }
    if postgres.current_username != postgres.declared_bootstrap_username
        || !postgres.bootstrap_can_create_database
        || !postgres.bootstrap_can_create_roles
        || !postgres.bootstrap_has_database_create
        || !postgres.jsonb_available
        || !postgres.sha256_available
    {
        blockers.push(
            "target PostgreSQL bootstrap principal lacks required identity or capabilities".into(),
        );
    }
    if !postgres.fsync_enabled
        || !postgres.full_page_writes_enabled
        || !postgres.synchronous_commit_enabled
        || !postgres.data_checksums_enabled
        || !postgres.wal_level_sufficient
        || !postgres.archive_mode_enabled
        || !postgres.archive_command_configured
        || !postgres.tls_in_use
    {
        blockers.push(
            "target PostgreSQL must prove durable WAL, checksums, configured WAL archiving/PITR, and server-observed TLS"
                .into(),
        );
    }
    if !postgres.database_absent || !postgres.roles_absent {
        blockers
            .push("target PostgreSQL database or migration/API/worker roles already exist".into());
    }
    let clickhouse = &target.clickhouse;
    if !clickhouse.version_26_3_lts {
        blockers.push("target ClickHouse must be the 26.3 LTS release family".into());
    }
    if clickhouse.current_username != clickhouse.bootstrap_username
        || !clickhouse.database_absent
        || !clickhouse.principals_absent
        || !clickhouse.standalone_non_replicated
        || !clickhouse.bootstrap_grants_sufficient
        || !clickhouse.privilege_declaration_complete
    {
        blockers.push(
            "target ClickHouse is not an empty, standalone, least-privilege bootstrap target"
                .into(),
        );
    }
    let redis = &target.redis;
    if redis.key_count != 0 {
        blockers.push("target Redis database is not empty".into());
    }
    if !redis.target_redis_6_2_or_newer
        || !redis.target_getdel_available
        || !redis.target_evalsha_available
        || !redis.target_script_available
        || !valid_redis_run_id(&redis.target_run_id)
        || redis.target_role != "master"
        || redis.target_connected_replicas != Some(0)
        || redis.target_cluster_enabled != Some(false)
    {
        blockers.push("target Redis lacks required commands or standalone identity".into());
    }
}

fn version_family(version: &str) -> Option<(u64, u64)> {
    let mut parts = version.split(['.', '-']);
    Some((parts.next()?.parse().ok()?, parts.next()?.parse().ok()?))
}

pub async fn build_plan(spec: &ProvisionSpec) -> Result<ProvisionPlan, ProvisionPlanError> {
    build_inspection(spec, InspectionMode::FencedFinal).await
}

fn record_final_requirement(
    mode: InspectionMode,
    blockers: &mut Vec<String>,
    pending_final_requirements: &mut Vec<String>,
    requirement: &str,
) {
    if mode == InspectionMode::FencedFinal {
        blockers.push(requirement.to_string());
    } else {
        pending_final_requirements.push(requirement.to_string());
    }
}

impl ProvisionPlan {
    pub fn passed(&self) -> bool {
        self.passed_with_capability(PRODUCTION_LEGACY_APPLY_CAPABILITY)
    }

    fn passed_with_capability(&self, capability: ProductionLegacyApplyCapability) -> bool {
        let apply_available = production_apply_available_with_capability(self.kind, capability);
        self.apply_available == apply_available
            && apply_available
            && self.converter_available
            && self.verdict == PreflightVerdict::Ready
            && self.next_action == NextAction::AuthorizeApply
            && legacy_inspection_sections_complete(self)
            && self.implementation_blockers.is_empty()
            && self.blockers.is_empty()
    }

    pub(crate) fn ready_for_legacy_authorization(&self, spec: &ProvisionSpec) -> bool {
        self.kind == ProvisionKind::LegacyReferenceMigration
            && spec.kind == ProvisionKind::LegacyReferenceMigration
            && self.operation_id == spec.operation_id
            && self.scope == InspectionMode::Online.scope()
            && self.passed()
            && self.review_binding_sha256.len() == 64
            && self.review_binding_hmac_sha256.len() == 64
            && self.report_sha256.len() == 64
            && self.report_binding_hmac_sha256.len() == 64
    }

    /// Matrix-only admission uses the same live inspection but permits exactly
    /// the global safety-audit blocker that this matrix is designed to clear.
    /// No datastore, topology, converter, or structural blocker is relaxed.
    #[cfg(feature = "bare-metal-fault-matrix")]
    pub(crate) fn ready_for_bare_metal_fault_matrix_authorization(
        &self,
        spec: &ProvisionSpec,
    ) -> bool {
        let expected_implementation_blocker = production_legacy_apply_implementation_blockers();
        self.kind == ProvisionKind::LegacyReferenceMigration
            && spec.kind == ProvisionKind::LegacyReferenceMigration
            && self.operation_id == spec.operation_id
            && self.scope == InspectionMode::Online.scope()
            && self.converter_available
            && !self.apply_available
            && self.verdict == PreflightVerdict::Blocked
            && self.next_action == NextAction::ResolveBlockers
            && legacy_inspection_sections_complete(self)
            && self.implementation_blockers == expected_implementation_blocker
            && self.implementation_blockers.len() == 1
            && self.blockers.is_empty()
            && self.review_binding_sha256.len() == 64
            && self.review_binding_hmac_sha256.len() == 64
            && self.report_sha256.len() == 64
            && self.report_binding_hmac_sha256.len() == 64
    }
}

async fn inspect_server(pool: &MySqlPool) -> Result<ServerRow, sqlx::Error> {
    sqlx::query_as::<_, ServerRow>(
        r#"
        SELECT
            DATABASE() AS database_name,
            VERSION() AS version,
            @@version_comment AS version_comment,
            @@GLOBAL.sql_mode AS global_sql_mode,
            @@SESSION.sql_mode AS session_sql_mode,
            @@character_set_database AS character_set,
            @@collation_database AS collation
        "#,
    )
    .fetch_one(pool)
    .await
}

struct SourceMysqlAccessInspection {
    grants_sha256: String,
    grants_are_read_only_and_complete: bool,
    visible_non_source_schema_count: u64,
}

async fn inspect_source_mysql_access(
    pool: &MySqlPool,
    source_database: &str,
) -> Result<SourceMysqlAccessInspection, sqlx::Error> {
    let rows = sqlx::query("SHOW GRANTS FOR CURRENT_USER()")
        .fetch_all(pool)
        .await?;
    let mut grants = rows
        .into_iter()
        .map(|row| row.try_get::<String, _>(0))
        .collect::<Result<Vec<_>, _>>()?;
    grants.sort();
    grants.dedup();
    let mut digest = Sha256::new();
    digest.update(b"v2board-source-mysql-grants-v1\0");
    let mut all_read_only = !grants.is_empty();
    let mut source_select = false;
    let mut show_databases = false;
    let mut process_inventory = false;
    let mut replication_inventory = false;
    for grant in &grants {
        digest.update((grant.len() as u64).to_be_bytes());
        digest.update(grant.as_bytes());
        let Some(classification) = classify_source_grant(grant, source_database) else {
            all_read_only = false;
            continue;
        };
        source_select |= classification.source_select;
        show_databases |= classification.show_databases;
        process_inventory |= classification.process_inventory;
        replication_inventory |= classification.replication_inventory;
    }

    let schemas = sqlx::query_scalar::<_, String>(
        "SELECT schema_name FROM information_schema.schemata ORDER BY schema_name",
    )
    .fetch_all(pool)
    .await?;
    let source_visible = schemas.iter().any(|schema| schema == source_database);
    let system_schemas = ["information_schema", "mysql", "performance_schema", "sys"];
    let visible_non_source_schema_count = schemas
        .iter()
        .filter(|schema| {
            schema.as_str() != source_database
                && !system_schemas
                    .iter()
                    .any(|system| schema.eq_ignore_ascii_case(system))
        })
        .count() as u64;
    Ok(SourceMysqlAccessInspection {
        grants_sha256: hex::encode(digest.finalize()),
        grants_are_read_only_and_complete: all_read_only
            && source_select
            && show_databases
            && process_inventory
            && replication_inventory
            && source_visible,
        visible_non_source_schema_count,
    })
}

#[derive(Clone, Copy, Default)]
struct SourceGrantClassification {
    source_select: bool,
    show_databases: bool,
    process_inventory: bool,
    replication_inventory: bool,
}

fn classify_source_grant(grant: &str, source_database: &str) -> Option<SourceGrantClassification> {
    if grant.contains("WITH GRANT OPTION") || !grant.starts_with("GRANT ") {
        return None;
    }
    let (privileges, rest) = grant.strip_prefix("GRANT ")?.split_once(" ON ")?;
    let (object, _) = rest.split_once(" TO ")?;
    let quoted_database = format!("`{}`.*", source_database.replace('`', "``"));
    let source_scope = object == quoted_database;
    let mysql_topology_scope = matches!(
        object,
        "`performance_schema`.`replication_connection_status`"
            | "`performance_schema`.`replication_group_members`"
    );
    let mut classification = SourceGrantClassification::default();
    for privilege in privileges.split(',').map(str::trim) {
        match privilege {
            "USAGE" if object == "*.*" => {}
            "SELECT" if source_scope => classification.source_select = true,
            "SELECT" if mysql_topology_scope => {}
            "SHOW VIEW" if source_scope => {}
            "SHOW DATABASES" if object == "*.*" => classification.show_databases = true,
            "PROCESS" if object == "*.*" => classification.process_inventory = true,
            "REPLICATION CLIENT" if object == "*.*" => {
                classification.replication_inventory = true;
            }
            _ => return None,
        }
    }
    Some(classification)
}

async fn inspect_mysql_server_uuid(pool: &MySqlPool) -> Result<String, sqlx::Error> {
    sqlx::query_scalar::<_, String>("SELECT @@server_uuid")
        .fetch_one(pool)
        .await
}

fn canonical_mysql_server_uuid(value: &str) -> Option<String> {
    let uuid = Uuid::parse_str(value.trim()).ok()?;
    (!uuid.is_nil()).then(|| uuid.hyphenated().to_string())
}

async fn inspect_mysql_topology(pool: &MySqlPool) -> MysqlTopology {
    let replication_channel_count = match schema_table_exists(
        pool,
        "performance_schema",
        "replication_connection_status",
    )
    .await
    {
        Ok(true) => sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM performance_schema.replication_connection_status",
        )
        .fetch_one(pool)
        .await
        .ok(),
        Ok(false) => Some(0),
        Err(_) => None,
    };
    let group_replication_member_count =
        match schema_table_exists(pool, "performance_schema", "replication_group_members").await {
            Ok(true) => sqlx::query_scalar::<_, i64>(
                "SELECT COUNT(*) FROM performance_schema.replication_group_members",
            )
            .fetch_one(pool)
            .await
            .ok(),
            Ok(false) => Some(0),
            Err(_) => None,
        };
    let registered_replica_count = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM information_schema.PROCESSLIST WHERE COMMAND IN ('Binlog Dump', 'Binlog Dump GTID')",
    )
    .fetch_one(pool)
    .await
    .ok();
    MysqlTopology {
        replication_channel_count,
        group_replication_member_count,
        registered_replica_count,
    }
}

async fn schema_table_exists(
    pool: &MySqlPool,
    schema: &str,
    table: &str,
) -> Result<bool, sqlx::Error> {
    let count = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM information_schema.TABLES WHERE TABLE_SCHEMA = ? AND TABLE_NAME = ?",
    )
    .bind(schema)
    .bind(table)
    .fetch_one(pool)
    .await?;
    Ok(count != 0)
}

async fn inspect_tables(pool: &MySqlPool) -> Result<Vec<TableRow>, sqlx::Error> {
    sqlx::query_as::<_, TableRow>(
        r#"
        SELECT TABLE_NAME AS table_name, ENGINE AS engine, TABLE_COLLATION AS table_collation
        FROM information_schema.TABLES
        WHERE TABLE_SCHEMA = DATABASE() AND TABLE_TYPE = 'BASE TABLE'
        ORDER BY TABLE_NAME
        "#,
    )
    .fetch_all(pool)
    .await
}

struct NonTableObjectCounts {
    views: i64,
    routines: i64,
    events: i64,
    triggers: i64,
}

async fn inspect_non_table_objects(pool: &MySqlPool) -> Result<NonTableObjectCounts, sqlx::Error> {
    let views = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM information_schema.VIEWS WHERE TABLE_SCHEMA = DATABASE()",
    )
    .fetch_one(pool)
    .await?;
    let routines = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM information_schema.ROUTINES WHERE ROUTINE_SCHEMA = DATABASE()",
    )
    .fetch_one(pool)
    .await?;
    let events = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM information_schema.EVENTS WHERE EVENT_SCHEMA = DATABASE()",
    )
    .fetch_one(pool)
    .await?;
    let triggers = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM information_schema.TRIGGERS WHERE TRIGGER_SCHEMA = DATABASE()",
    )
    .fetch_one(pool)
    .await?;
    Ok(NonTableObjectCounts {
        views,
        routines,
        events,
        triggers,
    })
}

pub(crate) async fn semantic_schema_hash(pool: &MySqlPool) -> Result<String, sqlx::Error> {
    let core = CORE_LEGACY_TABLES.iter().copied().collect::<BTreeSet<_>>();
    let tables = inspect_tables(pool).await?;
    let columns = sqlx::query_as::<_, ColumnRow>(
        r#"
        SELECT TABLE_NAME AS table_name, ORDINAL_POSITION AS ordinal_position,
               COLUMN_NAME AS column_name, COLUMN_TYPE AS column_type,
               IS_NULLABLE AS is_nullable, COLUMN_DEFAULT AS column_default,
               EXTRA AS extra, CHARACTER_SET_NAME AS character_set_name,
               COALESCE(GENERATION_EXPRESSION, '') AS generation_expression
        FROM information_schema.COLUMNS
        WHERE TABLE_SCHEMA = DATABASE()
        ORDER BY TABLE_NAME, ORDINAL_POSITION
        "#,
    )
    .fetch_all(pool)
    .await?;
    let indexes = sqlx::query_as::<_, IndexRow>(
        r#"
        SELECT TABLE_NAME AS table_name, INDEX_NAME AS index_name, NON_UNIQUE AS non_unique,
               CAST(SEQ_IN_INDEX AS SIGNED) AS seq_in_index, COLUMN_NAME AS column_name,
               CAST(SUB_PART AS SIGNED) AS sub_part,
               COLLATION AS collation, INDEX_TYPE AS index_type
        FROM information_schema.STATISTICS
        WHERE TABLE_SCHEMA = DATABASE()
        ORDER BY TABLE_NAME, INDEX_NAME, SEQ_IN_INDEX
        "#,
    )
    .fetch_all(pool)
    .await?;
    let constraints = sqlx::query_as::<_, ConstraintRow>(
        r#"
        SELECT TABLE_NAME AS table_name, CONSTRAINT_NAME AS constraint_name,
               CONSTRAINT_TYPE AS constraint_type
        FROM information_schema.TABLE_CONSTRAINTS
        WHERE TABLE_SCHEMA = DATABASE()
        ORDER BY TABLE_NAME, CONSTRAINT_NAME
        "#,
    )
    .fetch_all(pool)
    .await?;
    let triggers = sqlx::query_as::<_, TriggerRow>(
        r#"
        SELECT TRIGGER_NAME AS trigger_name, EVENT_MANIPULATION AS event_manipulation,
               EVENT_OBJECT_TABLE AS event_object_table, ACTION_TIMING AS action_timing
        FROM information_schema.TRIGGERS
        WHERE TRIGGER_SCHEMA = DATABASE()
        ORDER BY TRIGGER_NAME
        "#,
    )
    .fetch_all(pool)
    .await?;

    let mut hasher = Sha256::new();
    for row in tables
        .iter()
        .filter(|row| core.contains(row.table_name.as_str()))
    {
        hash_parts(
            &mut hasher,
            &[
                "table",
                &row.table_name,
                row.engine.as_deref().unwrap_or(""),
                &charset_from_collation(row.table_collation.as_deref().unwrap_or("")),
            ],
        );
    }
    for row in columns
        .iter()
        .filter(|row| core.contains(row.table_name.as_str()))
    {
        let normalized_default = normalize_column_default(row.column_default.as_deref());
        let normalized_extra = normalize_column_extra(&row.extra);
        hash_parts(
            &mut hasher,
            &[
                "column",
                &row.table_name,
                &row.ordinal_position.to_string(),
                &row.column_name,
                &normalize_column_type(&row.column_type),
                &row.is_nullable,
                &normalized_default,
                &normalized_extra,
                &normalize_charset(row.character_set_name.as_deref().unwrap_or("")),
                &row.generation_expression,
            ],
        );
    }
    for row in indexes
        .iter()
        .filter(|row| core.contains(row.table_name.as_str()))
    {
        hash_parts(
            &mut hasher,
            &[
                "index",
                &row.table_name,
                &row.index_name,
                &row.non_unique.to_string(),
                &row.seq_in_index.to_string(),
                row.column_name.as_deref().unwrap_or(""),
                &row.sub_part
                    .map(|value| value.to_string())
                    .unwrap_or_default(),
                row.collation.as_deref().unwrap_or(""),
                &row.index_type,
            ],
        );
    }
    for row in constraints
        .iter()
        .filter(|row| core.contains(row.table_name.as_str()))
    {
        hash_parts(
            &mut hasher,
            &[
                "constraint",
                &row.table_name,
                &row.constraint_name,
                &row.constraint_type,
            ],
        );
    }
    for row in triggers
        .iter()
        .filter(|row| core.contains(row.event_object_table.as_str()))
    {
        hash_parts(
            &mut hasher,
            &[
                "trigger",
                &row.trigger_name,
                &row.event_manipulation,
                &row.event_object_table,
                &row.action_timing,
            ],
        );
    }
    Ok(hex::encode(hasher.finalize()))
}

fn hash_parts(hasher: &mut Sha256, parts: &[&str]) {
    for part in parts {
        hasher.update((part.len() as u64).to_be_bytes());
        hasher.update(part.as_bytes());
    }
}

fn normalize_charset(value: &str) -> String {
    value.replace("utf8mb3", "utf8")
}

fn charset_from_collation(value: &str) -> String {
    normalize_charset(value.split('_').next().unwrap_or(value))
}

fn normalize_column_type(value: &str) -> String {
    let lower = value.to_ascii_lowercase();
    for integer in ["tinyint", "smallint", "mediumint", "int", "bigint"] {
        if let Some(rest) = lower.strip_prefix(integer)
            && rest.starts_with('(')
            && let Some(close) = rest.find(')')
        {
            let suffix = &rest[close + 1..];
            return format!("{integer}{suffix}");
        }
    }
    lower
}

fn normalize_column_default(value: Option<&str>) -> String {
    let Some(value) = value.map(str::trim) else {
        return "<NULL>".to_string();
    };
    if value.eq_ignore_ascii_case("NULL") {
        return "<NULL>".to_string();
    }
    if value.eq_ignore_ascii_case("CURRENT_TIMESTAMP")
        || value.eq_ignore_ascii_case("CURRENT_TIMESTAMP()")
    {
        return "current_timestamp".to_string();
    }
    value.to_string()
}

fn normalize_column_extra(value: &str) -> String {
    value
        .split_ascii_whitespace()
        // MySQL 8 exposes DEFAULT_GENERATED for ordinary explicit defaults;
        // it is metadata presentation, not a different source contract.
        .filter(|part| !part.eq_ignore_ascii_case("DEFAULT_GENERATED"))
        .map(str::to_ascii_lowercase)
        .collect::<Vec<_>>()
        .join(" ")
}

async fn inspect_data(
    pool: &MySqlPool,
    tables: &[TableRow],
) -> Result<DataInspection, sqlx::Error> {
    let names = tables
        .iter()
        .map(|row| row.table_name.as_str())
        .collect::<BTreeSet<_>>();
    let unfinished_orders = if names.contains("v2_order") {
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM v2_order WHERE status IN (0, 1)")
            .fetch_one(pool)
            .await?
    } else {
        0
    };
    let paid_pending_orders = if names.contains("v2_order") {
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM v2_order WHERE status = 1")
            .fetch_one(pool)
            .await?
    } else {
        0
    };
    let users_with_multiple_unfinished_orders = if names.contains("v2_order") {
        sqlx::query_scalar::<_, i64>(
            r#"
            SELECT COUNT(*) FROM (
                SELECT user_id FROM v2_order WHERE status IN (0, 1)
                GROUP BY user_id HAVING COUNT(*) > 1
            ) AS duplicate_users
            "#,
        )
        .fetch_one(pool)
        .await?
    } else {
        0
    };
    let stripe_payment_rows = if names.contains("v2_payment") {
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM v2_payment WHERE LOWER(payment) LIKE 'stripe%'",
        )
        .fetch_one(pool)
        .await?
    } else {
        0
    };
    let unfinished_stripe_orders = if names.contains("v2_order") && names.contains("v2_payment") {
        sqlx::query_scalar::<_, i64>(
            r#"
            SELECT COUNT(*)
            FROM v2_order AS orders
            JOIN v2_payment AS payment ON payment.id = orders.payment_id
            WHERE orders.status IN (0, 1) AND LOWER(payment.payment) LIKE 'stripe%'
            "#,
        )
        .fetch_one(pool)
        .await?
    } else {
        0
    };
    let unfinished_payment_orphans = if names.contains("v2_order") && names.contains("v2_payment") {
        sqlx::query_scalar::<_, i64>(
            r#"
            SELECT COUNT(*)
            FROM v2_order AS orders
            LEFT JOIN v2_payment AS payment ON payment.id = orders.payment_id
            WHERE orders.status IN (0, 1)
              AND orders.payment_id IS NOT NULL
              AND payment.id IS NULL
            "#,
        )
        .fetch_one(pool)
        .await?
    } else {
        0
    };
    let mut node_count = 0_i64;
    let mut visible_node_count = 0_i64;
    for (table, sql) in [
        (
            "v2_server_shadowsocks",
            "SELECT COUNT(*), COUNT(CASE WHEN `show` <> 0 THEN 1 END) FROM `v2_server_shadowsocks`",
        ),
        (
            "v2_server_vmess",
            "SELECT COUNT(*), COUNT(CASE WHEN `show` <> 0 THEN 1 END) FROM `v2_server_vmess`",
        ),
        (
            "v2_server_trojan",
            "SELECT COUNT(*), COUNT(CASE WHEN `show` <> 0 THEN 1 END) FROM `v2_server_trojan`",
        ),
        (
            "v2_server_vless",
            "SELECT COUNT(*), COUNT(CASE WHEN `show` <> 0 THEN 1 END) FROM `v2_server_vless`",
        ),
        (
            "v2_server_tuic",
            "SELECT COUNT(*), COUNT(CASE WHEN `show` <> 0 THEN 1 END) FROM `v2_server_tuic`",
        ),
        (
            "v2_server_hysteria",
            "SELECT COUNT(*), COUNT(CASE WHEN `show` <> 0 THEN 1 END) FROM `v2_server_hysteria`",
        ),
        (
            "v2_server_anytls",
            "SELECT COUNT(*), COUNT(CASE WHEN `show` <> 0 THEN 1 END) FROM `v2_server_anytls`",
        ),
        (
            "v2_server_v2node",
            "SELECT COUNT(*), COUNT(CASE WHEN `show` <> 0 THEN 1 END) FROM `v2_server_v2node`",
        ),
    ] {
        if !names.contains(table) {
            continue;
        }
        let (total, visible) = sqlx::query_as::<_, (i64, i64)>(sql).fetch_one(pool).await?;
        node_count += total;
        visible_node_count += visible;
    }
    let failed_job_count = if names.contains("failed_jobs") {
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM failed_jobs")
            .fetch_one(pool)
            .await?
    } else {
        0
    };
    let legacy_json_id_arrays = inspect_legacy_json_id_arrays(pool, &names).await?;
    let malformed_giftcard_redemptions = legacy_json_id_arrays
        .giftcard_used_user_ids
        .format_violations;
    let giftcard_redemption_orphans = legacy_json_id_arrays
        .giftcard_used_user_ids
        .missing_reference_violations;
    let business_invariant_violations = count_business_invariant_violations(pool, &names).await?;
    let relational_integrity_violations =
        count_relational_integrity_violations(pool, &names).await?;
    let node_group_violations = legacy_json_id_arrays.node_group_ids.violations;
    let target_collation_unique_collisions =
        count_target_collation_unique_collisions(pool, &names).await?;
    Ok(DataInspection {
        unfinished_orders,
        paid_pending_orders,
        users_with_multiple_unfinished_orders,
        stripe_payment_rows,
        unfinished_stripe_orders,
        unfinished_payment_orphans,
        node_count,
        visible_node_count,
        failed_job_count,
        malformed_giftcard_redemptions,
        giftcard_redemption_orphans,
        business_invariant_violations,
        relational_integrity_violations,
        node_group_violations,
        target_collation_unique_collisions,
        legacy_json_id_arrays,
    })
}

async fn count_business_invariant_violations(
    pool: &MySqlPool,
    tables: &BTreeSet<&str>,
) -> Result<i64, sqlx::Error> {
    let checks = [
        (
            "v2_coupon",
            "SELECT COUNT(*) FROM (SELECT code FROM v2_coupon GROUP BY code HAVING COUNT(*) > 1) AS violations",
        ),
        (
            "v2_giftcard",
            "SELECT COUNT(*) FROM (SELECT code FROM v2_giftcard GROUP BY code HAVING COUNT(*) > 1) AS violations",
        ),
        (
            "v2_invite_code",
            "SELECT COUNT(*) FROM (SELECT code FROM v2_invite_code GROUP BY code HAVING COUNT(*) > 1) AS violations",
        ),
        (
            "v2_payment",
            "SELECT COUNT(*) FROM (SELECT payment, uuid FROM v2_payment GROUP BY payment, uuid HAVING COUNT(*) > 1) AS violations",
        ),
        (
            "v2_ticket",
            "SELECT COUNT(*) FROM (SELECT user_id FROM v2_ticket WHERE status = 0 GROUP BY user_id HAVING COUNT(*) > 1) AS violations",
        ),
    ];
    let mut total = 0_i64;
    for (table, query) in checks {
        if tables.contains(table) {
            total =
                total.saturating_add(sqlx::query_scalar::<_, i64>(query).fetch_one(pool).await?);
        }
    }
    Ok(total)
}

async fn count_relational_integrity_violations(
    pool: &MySqlPool,
    tables: &BTreeSet<&str>,
) -> Result<i64, sqlx::Error> {
    if !CORE_LEGACY_TABLES
        .iter()
        .all(|table| tables.contains(table))
    {
        return Ok(0);
    }
    let checks = [
        "SELECT COUNT(*) FROM v2_plan c LEFT JOIN v2_server_group p ON p.id = c.group_id WHERE p.id IS NULL",
        "SELECT COUNT(*) FROM v2_user c LEFT JOIN v2_plan p ON p.id = c.plan_id WHERE c.plan_id IS NOT NULL AND p.id IS NULL",
        "SELECT COUNT(*) FROM v2_user c LEFT JOIN v2_server_group p ON p.id = c.group_id WHERE c.group_id IS NOT NULL AND p.id IS NULL",
        "SELECT COUNT(*) FROM v2_order c LEFT JOIN v2_user p ON p.id = c.user_id WHERE p.id IS NULL",
        "SELECT COUNT(*) FROM v2_order c LEFT JOIN v2_plan p ON p.id = c.plan_id WHERE c.plan_id <> 0 AND p.id IS NULL",
        "SELECT COUNT(*) FROM v2_giftcard c LEFT JOIN v2_plan p ON p.id = c.plan_id WHERE c.plan_id IS NOT NULL AND p.id IS NULL",
        "SELECT COUNT(*) FROM v2_invite_code c LEFT JOIN v2_user p ON p.id = c.user_id WHERE p.id IS NULL",
        "SELECT COUNT(*) FROM v2_ticket c LEFT JOIN v2_user p ON p.id = c.user_id WHERE p.id IS NULL",
        "SELECT COUNT(*) FROM v2_ticket_message c LEFT JOIN v2_ticket p ON p.id = c.ticket_id WHERE p.id IS NULL",
    ];
    let mut total = 0_i64;
    for query in checks {
        total = total.saturating_add(sqlx::query_scalar::<_, i64>(query).fetch_one(pool).await?);
    }
    Ok(total)
}

async fn inspect_legacy_json_id_arrays(
    pool: &MySqlPool,
    tables: &BTreeSet<&str>,
) -> Result<LegacyJsonIdArrayInspection, sqlx::Error> {
    let mut inspection = LegacyJsonIdArrayInspection::default();
    let group_reference = tables
        .contains("v2_server_group")
        .then_some("v2_server_group");
    for table in [
        "v2_server_shadowsocks",
        "v2_server_vmess",
        "v2_server_trojan",
        "v2_server_tuic",
        "v2_server_hysteria",
        "v2_server_vless",
        "v2_server_anytls",
        "v2_server_v2node",
    ] {
        if !tables.contains(table) {
            continue;
        }
        inspection.node_group_ids.merge(
            scan_legacy_json_id_array_column(
                pool,
                table,
                "group_id",
                group_reference,
                false,
                true,
                i64::from(i32::MAX),
            )
            .await?,
        );
    }
    if tables.contains("v2_coupon") {
        inspection.coupon_limit_plan_ids = scan_legacy_json_id_array_column(
            pool,
            "v2_coupon",
            "limit_plan_ids",
            tables.contains("v2_plan").then_some("v2_plan"),
            true,
            false,
            i64::from(i32::MAX),
        )
        .await?;
    }
    if tables.contains("v2_order") {
        inspection.order_surplus_order_ids = scan_legacy_json_id_array_column(
            pool,
            "v2_order",
            "surplus_order_ids",
            Some("v2_order"),
            true,
            false,
            i64::MAX,
        )
        .await?;
    }
    if tables.contains("v2_giftcard") {
        inspection.giftcard_used_user_ids = scan_legacy_json_id_array_column(
            pool,
            "v2_giftcard",
            "used_user_ids",
            tables.contains("v2_user").then_some("v2_user"),
            true,
            false,
            i64::MAX,
        )
        .await?;
    }
    Ok(inspection)
}

async fn scan_legacy_json_id_array_column(
    pool: &MySqlPool,
    source_table: &'static str,
    source_column: &'static str,
    reference_table: Option<&'static str>,
    allow_sql_null: bool,
    require_nonempty: bool,
    maximum_id: i64,
) -> Result<LegacyJsonIdArrayColumnInspection, sqlx::Error> {
    let mut inspection = LegacyJsonIdArrayColumnInspection::default();
    let mut last_id = None;
    loop {
        let rows = if let Some(last_id) = last_id {
            let query = format!(
                "SELECT id, CAST(`{source_column}` AS CHAR) FROM `{source_table}` \
                 WHERE id > ? ORDER BY id LIMIT {LEGACY_JSON_SCAN_PAGE_SIZE}"
            );
            sqlx::query_as::<_, (i64, Option<String>)>(AssertSqlSafe(query))
                .bind(last_id)
                .fetch_all(pool)
                .await?
        } else {
            let query = format!(
                "SELECT id, CAST(`{source_column}` AS CHAR) FROM `{source_table}` \
                 ORDER BY id LIMIT {LEGACY_JSON_SCAN_PAGE_SIZE}"
            );
            sqlx::query_as::<_, (i64, Option<String>)>(AssertSqlSafe(query))
                .fetch_all(pool)
                .await?
        };
        if rows.is_empty() {
            break;
        }
        last_id = rows.last().map(|(id, _)| *id);
        let page_complete = rows.len() < LEGACY_JSON_SCAN_PAGE_SIZE;
        let mut referenced_ids = Vec::new();
        for (_, raw) in rows {
            inspection.rows_scanned = inspection.rows_scanned.saturating_add(1);
            let classification = classify_legacy_json_id_array(
                raw.as_deref(),
                allow_sql_null,
                require_nonempty,
                maximum_id,
            );
            inspection.sql_null_rows = inspection
                .sql_null_rows
                .saturating_add(i64::from(classification.sql_null));
            inspection.array_rows = inspection
                .array_rows
                .saturating_add(i64::from(classification.array));
            inspection.requires_normalization = inspection
                .requires_normalization
                .saturating_add(classification.requires_normalization);
            inspection.format_violations = inspection
                .format_violations
                .saturating_add(classification.format_violations);
            referenced_ids.extend(classification.ids);
        }
        let existing_ids =
            fetch_existing_legacy_ids(pool, reference_table, &referenced_ids).await?;
        inspection.missing_reference_violations =
            inspection.missing_reference_violations.saturating_add(
                count_missing_reference_violations(&referenced_ids, &existing_ids),
            );
        if page_complete {
            break;
        }
    }
    inspection.refresh_violations();
    Ok(inspection)
}

async fn fetch_existing_legacy_ids(
    pool: &MySqlPool,
    reference_table: Option<&'static str>,
    ids: &[i64],
) -> Result<BTreeSet<i64>, sqlx::Error> {
    let Some(reference_table) = reference_table else {
        return Ok(BTreeSet::new());
    };
    let unique_ids = ids.iter().copied().collect::<BTreeSet<_>>();
    let unique_ids = unique_ids.into_iter().collect::<Vec<_>>();
    let mut existing = BTreeSet::new();
    for chunk in unique_ids.chunks(LEGACY_JSON_REFERENCE_BATCH_SIZE) {
        let mut query =
            QueryBuilder::<MySql>::new(format!("SELECT id FROM `{reference_table}` WHERE id IN ("));
        let mut separated = query.separated(", ");
        for id in chunk {
            separated.push_bind(*id);
        }
        separated.push_unseparated(")");
        existing.extend(query.build_query_scalar::<i64>().fetch_all(pool).await?);
    }
    Ok(existing)
}

#[derive(Debug, Default, Eq, PartialEq)]
struct LegacyJsonIdArrayClassification {
    sql_null: bool,
    array: bool,
    ids: Vec<i64>,
    requires_normalization: i64,
    format_violations: i64,
}

fn classify_legacy_json_id_array(
    raw: Option<&str>,
    allow_sql_null: bool,
    require_nonempty: bool,
    maximum_id: i64,
) -> LegacyJsonIdArrayClassification {
    let Some(raw) = raw else {
        return LegacyJsonIdArrayClassification {
            sql_null: true,
            format_violations: i64::from(!allow_sql_null),
            ..LegacyJsonIdArrayClassification::default()
        };
    };
    let Ok(serde_json::Value::Array(members)) = serde_json::from_str(raw) else {
        return LegacyJsonIdArrayClassification {
            format_violations: 1,
            ..LegacyJsonIdArrayClassification::default()
        };
    };
    let mut classification = LegacyJsonIdArrayClassification {
        array: true,
        ..LegacyJsonIdArrayClassification::default()
    };
    if require_nonempty && members.is_empty() {
        classification.format_violations = 1;
    }
    for member in members {
        let Some((id, requires_normalization)) = canonical_legacy_json_id(&member, maximum_id)
        else {
            classification.format_violations = classification.format_violations.saturating_add(1);
            continue;
        };
        classification.ids.push(id);
        classification.requires_normalization = classification
            .requires_normalization
            .saturating_add(i64::from(requires_normalization));
    }
    classification
}

fn canonical_legacy_json_id(member: &serde_json::Value, maximum_id: i64) -> Option<(i64, bool)> {
    if let Some(id) = member.as_i64() {
        return (id > 0 && id <= maximum_id).then_some((id, false));
    }
    let value = member.as_str()?;
    let bytes = value.as_bytes();
    if !bytes
        .first()
        .is_some_and(|byte| matches!(byte, b'1'..=b'9'))
        || !bytes.iter().skip(1).all(|byte| byte.is_ascii_digit())
    {
        return None;
    }
    let id = value.parse::<i64>().ok()?;
    (id <= maximum_id).then_some((id, true))
}

fn count_missing_reference_violations(ids: &[i64], existing: &BTreeSet<i64>) -> i64 {
    ids.iter()
        .filter(|id| !existing.contains(id))
        .fold(0_i64, |total, _| total.saturating_add(1))
}

async fn count_target_collation_unique_collisions(
    pool: &MySqlPool,
    tables: &BTreeSet<&str>,
) -> Result<i64, sqlx::Error> {
    let checks = [
        (
            "v2_coupon",
            "SELECT COUNT(*) FROM (SELECT CONVERT(code USING utf8mb4) COLLATE utf8mb4_unicode_ci AS k FROM v2_coupon GROUP BY k HAVING COUNT(*) > 1) AS collisions",
        ),
        (
            "v2_giftcard",
            "SELECT COUNT(*) FROM (SELECT CONVERT(code USING utf8mb4) COLLATE utf8mb4_unicode_ci AS k FROM v2_giftcard GROUP BY k HAVING COUNT(*) > 1) AS collisions",
        ),
        (
            "v2_invite_code",
            "SELECT COUNT(*) FROM (SELECT CONVERT(code USING utf8mb4) COLLATE utf8mb4_unicode_ci AS k FROM v2_invite_code GROUP BY k HAVING COUNT(*) > 1) AS collisions",
        ),
        (
            "v2_user",
            "SELECT COUNT(*) FROM (SELECT CONVERT(email USING utf8mb4) COLLATE utf8mb4_unicode_ci AS k FROM v2_user GROUP BY k HAVING COUNT(*) > 1) AS collisions",
        ),
        (
            "v2_user",
            "SELECT COUNT(*) FROM (SELECT CONVERT(token USING utf8mb4) COLLATE utf8mb4_unicode_ci AS k FROM v2_user GROUP BY k HAVING COUNT(*) > 1) AS collisions",
        ),
        (
            "v2_order",
            "SELECT COUNT(*) FROM (SELECT CONVERT(trade_no USING utf8mb4) COLLATE utf8mb4_unicode_ci AS k FROM v2_order GROUP BY k HAVING COUNT(*) > 1) AS collisions",
        ),
        (
            "v2_payment",
            "SELECT COUNT(*) FROM (SELECT CONVERT(payment USING utf8mb4) COLLATE utf8mb4_unicode_ci AS driver, CONVERT(uuid USING utf8mb4) COLLATE utf8mb4_unicode_ci AS id FROM v2_payment GROUP BY driver, id HAVING COUNT(*) > 1) AS collisions",
        ),
        (
            "v2_stat_server",
            "SELECT COUNT(*) FROM (SELECT server_id, CONVERT(server_type USING utf8mb4) COLLATE utf8mb4_unicode_ci AS server_kind, record_at FROM v2_stat_server GROUP BY server_id, server_kind, record_at HAVING COUNT(*) > 1) AS collisions",
        ),
    ];
    let mut total = 0_i64;
    for (table, query) in checks {
        if tables.contains(table) {
            total =
                total.saturating_add(sqlx::query_scalar::<_, i64>(query).fetch_one(pool).await?);
        }
    }
    Ok(total)
}

async fn inspect_source_redis(
    spec: &ProvisionSpec,
    source: &SourceSpec,
    mode: InspectionMode,
) -> Result<SourceRedisInspection, redis::RedisError> {
    let default_url = source.redis_default_url.as_str();
    let cache_url = source.redis_cache_url.as_str();
    let connection_prefix = source.redis_connection_prefix.as_str();
    let cache_prefix = source.redis_cache_prefix.as_str();
    let horizon_prefix = source.redis_horizon_prefix.as_str();
    let mut default_connection = redis_connection(default_url).await?;
    let (_, source_default_identity) = redis_server_identity(&mut default_connection).await?;
    let source_default_database = redis_database_index(default_url)?;
    let source_default_keyspace = redis_keyspace_counts(&mut default_connection).await?;
    let source_default_key_count = redis_dbsize(&mut default_connection).await?;
    let upload = inspect_traffic_hash(
        &mut default_connection,
        &format!("{connection_prefix}v2board_upload_traffic"),
    )
    .await?;
    let download = inspect_traffic_hash(
        &mut default_connection,
        &format!("{connection_prefix}v2board_download_traffic"),
    )
    .await?;

    let mut cache_connection = redis_connection(cache_url).await?;
    let (_, source_cache_identity) = redis_server_identity(&mut cache_connection).await?;
    let source_cache_database = redis_database_index(cache_url)?;
    let source_cache_keyspace = redis_keyspace_counts(&mut cache_connection).await?;
    let source_cache_key_count = redis_dbsize(&mut cache_connection).await?;
    let declared_cache_key_prefix = laravel_cache_physical_prefix(connection_prefix, cache_prefix);
    let same_server = source_default_identity.run_id == source_cache_identity.run_id;
    let same_logical_database = same_server && source_default_database == source_cache_database;
    let configured_upload_key = format!("{connection_prefix}v2board_upload_traffic");
    let configured_download_key = format!("{connection_prefix}v2board_download_traffic");
    let configured_reset_key = format!("{connection_prefix}traffic_reset_lock");
    let configured_queue_prefix = format!("{connection_prefix}queues:");
    let configured_horizon_prefix = format!("{connection_prefix}{horizon_prefix}");
    let frozen_upload_key = format!(
        "{connection_prefix}v2board_migration:{}:frozen_upload_traffic",
        spec.operation_id
    );
    let frozen_download_key = format!(
        "{connection_prefix}v2board_migration:{}:frozen_download_traffic",
        spec.operation_id
    );
    let migration_key_prefix = format!("{connection_prefix}v2board_migration:");
    let ownership = RedisOwnershipRules {
        configured_upload_key: configured_upload_key.as_bytes(),
        configured_download_key: configured_download_key.as_bytes(),
        configured_reset_key: configured_reset_key.as_bytes(),
        configured_queue_prefix: configured_queue_prefix.as_bytes(),
        configured_horizon_prefix: configured_horizon_prefix.as_bytes(),
        horizon_prefix_enabled: !horizon_prefix.is_empty(),
        declared_cache_key_prefix: declared_cache_key_prefix.as_bytes(),
        migration_key_prefix: migration_key_prefix.as_bytes(),
        frozen_upload_key: frozen_upload_key.as_bytes(),
        frozen_download_key: frozen_download_key.as_bytes(),
        allow_current_operation_frozen_keys: mode == InspectionMode::FencedFinal,
    };
    let default_inventory =
        inspect_default_redis_inventory(&mut default_connection, &ownership, same_logical_database)
            .await?;
    let cache_inventory =
        inspect_cache_redis_inventory(&mut cache_connection, &ownership, same_logical_database)
            .await?;
    let source_default_other_logical_database_keys = keyspace_keys_outside(
        &source_default_keyspace,
        source_default_database,
        same_server.then_some(source_cache_database),
    );
    let source_cache_other_logical_database_keys = keyspace_keys_outside(
        &source_cache_keyspace,
        source_cache_database,
        same_server.then_some(source_default_database),
    );
    let physical_redis_ownership_complete = default_inventory.unclassified_key_count == 0
        && cache_inventory.unclassified_key_count == 0
        && source_default_other_logical_database_keys == 0
        && source_cache_other_logical_database_keys == 0;
    Ok(SourceRedisInspection {
        source_default_run_id: source_default_identity.run_id,
        source_cache_run_id: source_cache_identity.run_id,
        source_default_role: source_default_identity.role,
        source_cache_role: source_cache_identity.role,
        source_default_connected_replicas: source_default_identity.connected_replicas,
        source_cache_connected_replicas: source_cache_identity.connected_replicas,
        source_default_cluster_enabled: source_default_identity.cluster_enabled,
        source_cache_cluster_enabled: source_cache_identity.cluster_enabled,
        source_default_key_count,
        source_cache_key_count,
        source_default_unclassified_key_count: default_inventory.unclassified_key_count,
        source_cache_unclassified_key_count: cache_inventory.unclassified_key_count,
        source_default_other_logical_database_keys,
        source_cache_other_logical_database_keys,
        physical_redis_ownership_complete,
        upload_traffic_fields: upload.fields,
        download_traffic_fields: download.fields,
        upload_traffic_sum: upload.sum.to_string(),
        download_traffic_sum: download.sum.to_string(),
        malformed_traffic_values: upload.malformed + download.malformed,
        unexpected_traffic_key_candidates: default_inventory.unexpected_traffic_key_candidates,
        traffic_reset_lock_keys: default_inventory.traffic_reset_lock_keys,
        queued_item_count: default_inventory.queued_item_count,
        queue_notify_item_count: default_inventory.queue_notify_item_count,
        ambiguous_queue_key_candidates: default_inventory.ambiguous_queue_key_candidates,
        retryable_failed_job_items: default_inventory.retryable_failed_job_items,
        legacy_subscription_token_keys: cache_inventory.legacy_subscription_token_keys,
        ambiguous_subscription_token_keys: cache_inventory.ambiguous_subscription_token_keys,
    })
}

fn laravel_cache_physical_prefix(connection_prefix: &str, cache_prefix: &str) -> String {
    if cache_prefix.is_empty() {
        return connection_prefix.to_string();
    }
    format!("{connection_prefix}{cache_prefix}:")
}

struct TrafficHashInspection {
    fields: u64,
    sum: i128,
    malformed: u64,
}

async fn inspect_traffic_hash(
    connection: &mut ConnectionManager,
    key: &str,
) -> Result<TrafficHashInspection, redis::RedisError> {
    let key_type = redis::cmd("TYPE")
        .arg(key)
        .query_async::<String>(connection)
        .await?;
    if key_type == "none" {
        return Ok(TrafficHashInspection {
            fields: 0,
            sum: 0,
            malformed: 0,
        });
    }
    if key_type != "hash" {
        return Ok(TrafficHashInspection {
            fields: 1,
            sum: 0,
            malformed: 1,
        });
    }
    let fields = redis::cmd("HLEN")
        .arg(key)
        .query_async::<u64>(connection)
        .await?;
    let mut cursor = 0_u64;
    let mut sum = 0_i128;
    let mut malformed = 0_u64;
    loop {
        let (next, values) = redis::cmd("HSCAN")
            .arg(key)
            .arg(cursor)
            .arg("COUNT")
            .arg(256)
            .query_async::<(u64, Vec<(String, String)>)>(connection)
            .await?;
        for (field, value) in values {
            if field.parse::<i64>().is_err() {
                malformed += 1;
                continue;
            }
            match value.parse::<i128>() {
                Ok(value) if value >= 0 => {
                    if let Some(next) = sum.checked_add(value) {
                        sum = next;
                    } else {
                        malformed += 1;
                    }
                }
                _ => malformed += 1,
            }
        }
        if next == 0 {
            break;
        }
        cursor = next;
    }
    Ok(TrafficHashInspection {
        fields,
        sum,
        malformed,
    })
}

async fn redis_collection_item_count(
    connection: &mut ConnectionManager,
    key: &[u8],
) -> Result<u64, redis::RedisError> {
    let key_type = redis::cmd("TYPE")
        .arg(key)
        .query_async::<String>(connection)
        .await?;
    match key_type.as_str() {
        "list" => redis::cmd("LLEN").arg(key).query_async(connection).await,
        "zset" => redis::cmd("ZCARD").arg(key).query_async(connection).await,
        "set" => redis::cmd("SCARD").arg(key).query_async(connection).await,
        "hash" => redis::cmd("HLEN").arg(key).query_async(connection).await,
        "none" => Ok(0),
        _ => Ok(1),
    }
}

struct RedisOwnershipRules<'a> {
    configured_upload_key: &'a [u8],
    configured_download_key: &'a [u8],
    configured_reset_key: &'a [u8],
    configured_queue_prefix: &'a [u8],
    configured_horizon_prefix: &'a [u8],
    horizon_prefix_enabled: bool,
    declared_cache_key_prefix: &'a [u8],
    migration_key_prefix: &'a [u8],
    frozen_upload_key: &'a [u8],
    frozen_download_key: &'a [u8],
    allow_current_operation_frozen_keys: bool,
}

impl RedisOwnershipRules<'_> {
    fn is_allowed_current_operation_frozen_key(&self, key: &[u8]) -> bool {
        self.allow_current_operation_frozen_keys
            && (key == self.frozen_upload_key || key == self.frozen_download_key)
    }

    fn is_migration_key(&self, key: &[u8]) -> bool {
        key.starts_with(self.migration_key_prefix)
    }

    fn default_owned(&self, key: &[u8]) -> bool {
        if self.is_migration_key(key) {
            return self.is_allowed_current_operation_frozen_key(key);
        }
        key == self.configured_upload_key
            || key == self.configured_download_key
            || key == self.configured_reset_key
            || key.starts_with(self.configured_queue_prefix)
            || (self.horizon_prefix_enabled && key.starts_with(self.configured_horizon_prefix))
    }

    fn cache_owned(&self, key: &[u8]) -> bool {
        !self.is_migration_key(key) && key.starts_with(self.declared_cache_key_prefix)
    }

    fn declared_subscription_token(&self, key: &[u8]) -> bool {
        self.cache_owned(key)
            && key
                .strip_prefix(self.declared_cache_key_prefix)
                .is_some_and(subscription_token_logical_key)
    }
}

#[derive(Default)]
struct DefaultRedisInventory {
    unclassified_key_count: u64,
    unexpected_traffic_key_candidates: u64,
    traffic_reset_lock_keys: u64,
    queued_item_count: u64,
    queue_notify_item_count: u64,
    ambiguous_queue_key_candidates: u64,
    retryable_failed_job_items: u64,
}

#[derive(Default)]
struct CacheRedisInventory {
    unclassified_key_count: u64,
    legacy_subscription_token_keys: u64,
    ambiguous_subscription_token_keys: u64,
}

async fn inspect_default_redis_inventory(
    connection: &mut ConnectionManager,
    ownership: &RedisOwnershipRules<'_>,
    cache_shares_logical_database: bool,
) -> Result<DefaultRedisInventory, redis::RedisError> {
    // SCAN may repeat a key while an online source is changing. These are
    // deliberately conservative occurrence counters: migration gates consume
    // zero versus nonzero, and the final pass runs after the durable fence.
    let mut inventory = DefaultRedisInventory::default();
    let mut scanner = RedisKeyScanner::default();
    while let Some(page) = scanner.next_page(connection).await? {
        for key in page {
            if !ownership.default_owned(&key)
                && !(cache_shares_logical_database && ownership.cache_owned(&key))
            {
                inventory.unclassified_key_count =
                    inventory.unclassified_key_count.saturating_add(1);
            }

            if (key.ends_with(b"v2board_upload_traffic") && key != ownership.configured_upload_key)
                || (key.ends_with(b"v2board_download_traffic")
                    && key != ownership.configured_download_key)
            {
                inventory.unexpected_traffic_key_candidates = inventory
                    .unexpected_traffic_key_candidates
                    .saturating_add(1);
            }
            if key.ends_with(b"traffic_reset_lock") {
                inventory.traffic_reset_lock_keys =
                    inventory.traffic_reset_lock_keys.saturating_add(1);
            }
            if contains_bytes(&key, b"queues:") {
                if let Some(logical) = key.strip_prefix(ownership.configured_queue_prefix) {
                    let item_count = redis_collection_item_count(connection, &key).await?;
                    if logical
                        .strip_suffix(b":notify")
                        .is_some_and(|queue| !queue.is_empty())
                    {
                        inventory.queue_notify_item_count =
                            inventory.queue_notify_item_count.saturating_add(item_count);
                    } else {
                        inventory.queued_item_count =
                            inventory.queued_item_count.saturating_add(item_count);
                    }
                } else {
                    inventory.ambiguous_queue_key_candidates =
                        inventory.ambiguous_queue_key_candidates.saturating_add(1);
                }
            }
            if key.ends_with(b"failed_jobs") {
                inventory.retryable_failed_job_items = inventory
                    .retryable_failed_job_items
                    .saturating_add(redis_collection_item_count(connection, &key).await?);
            }
        }
    }
    Ok(inventory)
}

async fn inspect_cache_redis_inventory(
    connection: &mut ConnectionManager,
    ownership: &RedisOwnershipRules<'_>,
    default_shares_logical_database: bool,
) -> Result<CacheRedisInventory, redis::RedisError> {
    let mut inventory = CacheRedisInventory::default();
    let mut scanner = RedisKeyScanner::default();
    while let Some(page) = scanner.next_page(connection).await? {
        for key in page {
            if !ownership.cache_owned(&key)
                && !(default_shares_logical_database && ownership.default_owned(&key))
            {
                inventory.unclassified_key_count =
                    inventory.unclassified_key_count.saturating_add(1);
            }
            if subscription_token_candidate(&key) {
                if ownership.declared_subscription_token(&key) {
                    inventory.legacy_subscription_token_keys =
                        inventory.legacy_subscription_token_keys.saturating_add(1);
                } else {
                    inventory.ambiguous_subscription_token_keys = inventory
                        .ambiguous_subscription_token_keys
                        .saturating_add(1);
                }
            }
        }
    }
    Ok(inventory)
}

fn subscription_token_candidate(key: &[u8]) -> bool {
    [b"otp_".as_slice(), b"otpn_".as_slice(), b"totp_".as_slice()]
        .into_iter()
        .any(|needle| contains_bytes(key, needle))
}

fn subscription_token_logical_key(key: &[u8]) -> bool {
    key.starts_with(b"otp_") || key.starts_with(b"otpn_") || key.starts_with(b"totp_")
}

fn contains_bytes(value: &[u8], needle: &[u8]) -> bool {
    !needle.is_empty() && value.windows(needle.len()).any(|window| window == needle)
}

struct TargetRedisStatus {
    key_count: u64,
    database_index: u32,
    version: String,
    identity: RedisServerIdentity,
    redis_6_2_or_newer: bool,
    getdel_available: bool,
    evalsha_available: bool,
    script_available: bool,
}

async fn inspect_target_redis(redis_url: &str) -> Result<TargetRedisStatus, redis::RedisError> {
    let mut connection = redis_connection(redis_url).await?;
    let database_index = redis_database_index(redis_url)?;
    let key_count = redis_dbsize(&mut connection).await?;
    let keyspace = redis_keyspace_counts(&mut connection).await?;
    validate_selected_redis_database_size(database_index, key_count, &keyspace)?;
    let (version, identity) = redis_server_identity(&mut connection).await?;
    let redis_6_2_or_newer = version_at_least(&version, 6, 2, 0);
    let getdel_available = redis_command_available(&mut connection, "GETDEL").await?;
    let evalsha_available = redis_command_available(&mut connection, "EVALSHA").await?;
    let script_available = redis_command_available(&mut connection, "SCRIPT").await?;
    Ok(TargetRedisStatus {
        key_count,
        database_index,
        version,
        identity,
        redis_6_2_or_newer,
        getdel_available,
        evalsha_available,
        script_available,
    })
}

struct RedisServerIdentity {
    run_id: String,
    role: String,
    connected_replicas: Option<u64>,
    cluster_enabled: Option<bool>,
}

fn redis_database_index(value: &str) -> Result<u32, redis::RedisError> {
    let url = Url::parse(value).map_err(|_| {
        redis::RedisError::from((redis::ErrorKind::InvalidClientConfig, "invalid Redis URL"))
    })?;
    let path = url.path().trim_start_matches('/');
    if path.is_empty() {
        return Ok(0);
    }
    if path.contains('/') {
        return Err(redis::RedisError::from((
            redis::ErrorKind::InvalidClientConfig,
            "invalid Redis logical database path",
        )));
    }
    path.parse::<u32>().map_err(|_| {
        redis::RedisError::from((
            redis::ErrorKind::InvalidClientConfig,
            "invalid Redis logical database index",
        ))
    })
}

async fn redis_keyspace_counts(
    connection: &mut ConnectionManager,
) -> Result<BTreeMap<u32, u64>, redis::RedisError> {
    let info = redis::cmd("INFO")
        .arg("keyspace")
        .query_async::<String>(&mut *connection)
        .await?;
    parse_redis_keyspace_info(&info)
}

fn parse_redis_keyspace_info(info: &str) -> Result<BTreeMap<u32, u64>, redis::RedisError> {
    let mut databases = BTreeMap::new();
    for line in info.lines().map(str::trim) {
        let Some((database, values)) = line.split_once(':') else {
            continue;
        };
        let Some(database) = database.strip_prefix("db") else {
            continue;
        };
        let database = database.parse::<u32>().map_err(|_| {
            redis::RedisError::from((
                redis::ErrorKind::UnexpectedReturnType,
                "invalid Redis keyspace database index",
            ))
        })?;
        let keys = values
            .split(',')
            .find_map(|field| field.strip_prefix("keys="))
            .ok_or_else(|| {
                redis::RedisError::from((
                    redis::ErrorKind::UnexpectedReturnType,
                    "Redis keyspace row has no key count",
                ))
            })?
            .parse::<u64>()
            .map_err(|_| {
                redis::RedisError::from((
                    redis::ErrorKind::UnexpectedReturnType,
                    "invalid Redis keyspace key count",
                ))
            })?;
        if databases.insert(database, keys).is_some() {
            return Err(redis::RedisError::from((
                redis::ErrorKind::UnexpectedReturnType,
                "duplicate Redis keyspace database row",
            )));
        }
    }
    Ok(databases)
}

fn keyspace_keys_outside(
    databases: &BTreeMap<u32, u64>,
    primary: u32,
    secondary: Option<u32>,
) -> u64 {
    databases
        .iter()
        .filter(|(database, _)| **database != primary && Some(**database) != secondary)
        .fold(0_u64, |total, (_, keys)| total.saturating_add(*keys))
}

fn validate_selected_redis_database_size(
    database_index: u32,
    dbsize: u64,
    databases: &BTreeMap<u32, u64>,
) -> Result<(), redis::RedisError> {
    if databases.get(&database_index).copied().unwrap_or(0) != dbsize {
        return Err(redis::RedisError::from((
            redis::ErrorKind::Client,
            "target Redis DBSIZE does not match the selected logical database identity",
        )));
    }
    Ok(())
}

async fn redis_server_identity(
    connection: &mut ConnectionManager,
) -> Result<(String, RedisServerIdentity), redis::RedisError> {
    let server_info = redis::cmd("INFO")
        .arg("server")
        .query_async::<String>(&mut *connection)
        .await?;
    let replication_info = redis::cmd("INFO")
        .arg("replication")
        .query_async::<String>(&mut *connection)
        .await?;
    let cluster_info = redis::cmd("INFO")
        .arg("cluster")
        .query_async::<String>(&mut *connection)
        .await?;
    let field = |info: &str, name: &str| {
        info.lines()
            .find_map(|line| line.strip_prefix(name))
            .map(str::trim)
            .unwrap_or("")
            .to_string()
    };
    let connected_replicas = field(&replication_info, "connected_slaves:").parse().ok();
    let cluster_enabled = match field(&cluster_info, "cluster_enabled:").as_str() {
        "0" => Some(false),
        "1" => Some(true),
        _ => None,
    };
    Ok((
        field(&server_info, "redis_version:"),
        RedisServerIdentity {
            run_id: field(&server_info, "run_id:"),
            role: field(&replication_info, "role:"),
            connected_replicas,
            cluster_enabled,
        },
    ))
}

fn valid_redis_run_id(value: &str) -> bool {
    value.len() == 40 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn source_redis_standalone(redis: &SourceRedisInspection) -> bool {
    valid_redis_run_id(&redis.source_default_run_id)
        && valid_redis_run_id(&redis.source_cache_run_id)
        && redis.source_default_role == "master"
        && redis.source_cache_role == "master"
        && redis.source_default_connected_replicas == Some(0)
        && redis.source_cache_connected_replicas == Some(0)
        && redis.source_default_cluster_enabled == Some(false)
        && redis.source_cache_cluster_enabled == Some(false)
}

async fn redis_command_available(
    connection: &mut ConnectionManager,
    command: &str,
) -> Result<bool, redis::RedisError> {
    let value = redis::cmd("COMMAND")
        .arg("INFO")
        .arg(command)
        .query_async::<redis::Value>(connection)
        .await?;
    Ok(match value {
        redis::Value::Array(values) => values
            .first()
            .is_some_and(|value| !matches!(value, redis::Value::Nil)),
        redis::Value::Nil => false,
        _ => false,
    })
}

#[derive(Default)]
struct RedisKeyScanner {
    cursor: u64,
    complete: bool,
}

impl RedisKeyScanner {
    async fn next_page(
        &mut self,
        connection: &mut ConnectionManager,
    ) -> Result<Option<Vec<Vec<u8>>>, redis::RedisError> {
        if self.complete {
            return Ok(None);
        }
        let (next, page) = redis::cmd("SCAN")
            .arg(self.cursor)
            .arg("COUNT")
            .arg(REDIS_SCAN_COUNT)
            .query_async::<(u64, Vec<Vec<u8>>)>(connection)
            .await?;
        validate_redis_scan_page(&page)?;
        if next != 0 && next == self.cursor {
            return Err(redis::RedisError::from((
                redis::ErrorKind::Client,
                "Redis SCAN cursor did not advance",
            )));
        }
        self.cursor = next;
        self.complete = next == 0;
        Ok(Some(page))
    }
}

fn validate_redis_scan_page(page: &[Vec<u8>]) -> Result<(), redis::RedisError> {
    if page.len() > MAX_REDIS_SCAN_PAGE_KEYS {
        return Err(redis::RedisError::from((
            redis::ErrorKind::Client,
            "Redis SCAN page contains too many keys",
        )));
    }
    let mut page_bytes = 0_usize;
    for key in page {
        if key.len() > MAX_REDIS_KEY_BYTES {
            return Err(redis::RedisError::from((
                redis::ErrorKind::Client,
                "Redis key exceeds the bounded inspection length",
            )));
        }
        page_bytes = page_bytes.checked_add(key.len()).ok_or_else(|| {
            redis::RedisError::from((
                redis::ErrorKind::Client,
                "Redis SCAN page byte length overflowed",
            ))
        })?;
        if page_bytes > MAX_REDIS_SCAN_PAGE_BYTES {
            return Err(redis::RedisError::from((
                redis::ErrorKind::Client,
                "Redis SCAN page exceeds the bounded inspection byte size",
            )));
        }
    }
    Ok(())
}

async fn redis_dbsize(connection: &mut ConnectionManager) -> Result<u64, redis::RedisError> {
    redis::cmd("DBSIZE").query_async(connection).await
}

async fn redis_connection(redis_url: &str) -> Result<ConnectionManager, redis::RedisError> {
    let client = redis::Client::open(redis_url)?;
    ConnectionManager::new(client).await
}

fn database_vendor(version: &str, comment: &str) -> DatabaseVendor {
    let text = format!("{version} {comment}").to_ascii_lowercase();
    if !text.contains("mariadb")
        && !text.contains("percona")
        && (comment
            .to_ascii_lowercase()
            .contains("mysql community server")
            || comment.to_ascii_lowercase().contains("mysql enterprise"))
    {
        DatabaseVendor::MySql
    } else {
        DatabaseVendor::Unsupported
    }
}

fn version_is_supported_mysql8(version: &str) -> bool {
    let mut parts = version.split(['.', '-']);
    let Some(major) = parts.next().and_then(|value| value.parse::<u64>().ok()) else {
        return false;
    };
    let Some(minor) = parts.next().and_then(|value| value.parse::<u64>().ok()) else {
        return false;
    };
    let Some(_patch) = parts.next().and_then(|value| value.parse::<u64>().ok()) else {
        return false;
    };
    major == 8 && matches!(minor, 0 | 4)
}

fn version_at_least(version: &str, major: u64, minor: u64, patch: u64) -> bool {
    let parsed = version
        .split(['.', '-'])
        .take(3)
        .map(|part| part.parse::<u64>().unwrap_or(0))
        .collect::<Vec<_>>();
    let actual = (
        *parsed.first().unwrap_or(&0),
        *parsed.get(1).unwrap_or(&0),
        *parsed.get(2).unwrap_or(&0),
    );
    actual >= (major, minor, patch)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn source_admission_accepts_only_oracle_mysql_major_8() {
        assert!(matches!(
            database_vendor("8.4.4", "MySQL Community Server - GPL"),
            DatabaseVendor::MySql
        ));
        assert!(matches!(
            database_vendor("8.0.37", "MySQL Enterprise Server - Commercial"),
            DatabaseVendor::MySql
        ));
        for (version, comment) in [
            ("8.0.37-29", "Percona Server (GPL)"),
            ("10.11.8-MariaDB", "mariadb.org binary distribution"),
            ("8.4.4-compatible", "Unknown SQL proxy"),
        ] {
            assert!(matches!(
                database_vendor(version, comment),
                DatabaseVendor::Unsupported
            ));
        }
        assert!(version_is_supported_mysql8("8.0.37"));
        assert!(version_is_supported_mysql8("8.4.4"));
        assert!(!version_is_supported_mysql8("8"));
        assert!(!version_is_supported_mysql8("8.1.0"));
        assert!(!version_is_supported_mysql8("5.7.44"));
        assert!(!version_is_supported_mysql8("9.0.1"));
        assert!(version_at_least("8.4.4", 8, 4, 0));
        assert!(!version_at_least("8.0.36", 8, 4, 0));
        assert_eq!(version_family("26.3.17.4"), Some((26, 3)));
        assert_ne!(version_family("26.2.1.1"), Some((26, 3)));
        assert_eq!(
            canonical_mysql_server_uuid("40AA4A80-EB4B-4B25-9C3B-E17ED047873D").as_deref(),
            Some("40aa4a80-eb4b-4b25-9c3b-e17ed047873d")
        );
        assert!(canonical_mysql_server_uuid("").is_none());
        assert!(canonical_mysql_server_uuid("not-a-uuid").is_none());
        assert!(canonical_mysql_server_uuid("00000000-0000-0000-0000-000000000000").is_none());
        assert!(valid_redis_run_id(
            "0123456789abcdef0123456789abcdef01234567"
        ));
        assert!(!valid_redis_run_id("short"));
        assert!(!MysqlTopology::default().is_standalone_visible());
        assert!(
            MysqlTopology {
                replication_channel_count: Some(0),
                group_replication_member_count: Some(0),
                registered_replica_count: Some(0),
            }
            .is_standalone_visible()
        );
    }

    #[test]
    fn clickhouse_bootstrap_grants_are_fail_closed() {
        assert!(!clickhouse_bootstrap_grants_sufficient(&[]));
        assert!(clickhouse_bootstrap_grants_sufficient(&[
            ClickHouseGrantRow {
                access_type: "ALL".to_string(),
                database: None,
                table: None,
                grant_option: 1,
            },
        ]));
        assert!(clickhouse_bootstrap_grants_sufficient(&[
            ClickHouseGrantRow {
                access_type: "CREATE".to_string(),
                database: None,
                table: None,
                grant_option: 1,
            },
            ClickHouseGrantRow {
                access_type: "CREATE USER".to_string(),
                database: None,
                table: None,
                grant_option: 1,
            },
            ClickHouseGrantRow {
                access_type: "CREATE ROLE".to_string(),
                database: None,
                table: None,
                grant_option: 1,
            },
        ]));
    }

    #[test]
    fn mysql8_schema_canonicalization_ignores_metadata_only_differences() {
        assert_eq!(normalize_column_type("int(11) unsigned"), "int unsigned");
        assert_eq!(normalize_column_type("bigint(20)"), "bigint");
        assert_eq!(normalize_charset("utf8mb3_unicode_ci"), "utf8_unicode_ci");
        assert_eq!(normalize_column_default(None), "<NULL>");
        assert_eq!(normalize_column_default(Some("NULL")), "<NULL>");
        assert_eq!(normalize_column_default(Some("0.0.0.0")), "0.0.0.0");
        assert_eq!(
            normalize_column_default(Some("CURRENT_TIMESTAMP()")),
            "current_timestamp"
        );
        assert_eq!(normalize_column_extra("DEFAULT_GENERATED"), "");
        assert_eq!(
            normalize_column_extra("DEFAULT_GENERATED on update CURRENT_TIMESTAMP"),
            "on update current_timestamp"
        );
    }

    #[test]
    fn source_mysql_grants_allow_only_complete_read_inventory_access() {
        let select = classify_source_grant(
            "GRANT SELECT, SHOW VIEW ON `v2board`.* TO `migration`@`localhost` REQUIRE SSL",
            "v2board",
        )
        .expect("source-wide read grant");
        assert!(select.source_select);
        assert!(!select.show_databases);
        let inventory = classify_source_grant(
            "GRANT PROCESS, REPLICATION CLIENT, SHOW DATABASES ON *.* TO `migration`@`localhost`",
            "v2board",
        )
        .expect("physical inventory grant");
        assert!(inventory.show_databases);
        assert!(inventory.process_inventory);
        assert!(inventory.replication_inventory);
        assert!(
            classify_source_grant(
                "GRANT PROCESS, SHOW DATABASES, BINLOG MONITOR, SLAVE MONITOR ON *.* TO `migration`@`localhost`",
                "v2board",
            )
            .is_none()
        );
        assert!(
            classify_source_grant(
                "GRANT SELECT ON `performance_schema`.`replication_connection_status` TO `migration`@`localhost`",
                "v2board",
            )
            .is_some()
        );
        assert!(
            classify_source_grant(
                "GRANT SELECT ON `performance_schema`.`replication_group_members` TO `migration`@`localhost`",
                "v2board",
            )
            .is_some()
        );
        assert!(
            classify_source_grant(
                "GRANT USAGE ON *.* TO `migration`@`localhost` IDENTIFIED BY PASSWORD '*HASH'",
                "v2board",
            )
            .is_some()
        );
        assert!(
            classify_source_grant(
                "GRANT INSERT ON `v2board`.* TO `migration`@`localhost`",
                "v2board",
            )
            .is_none()
        );
        assert!(
            classify_source_grant("GRANT SELECT ON *.* TO `migration`@`localhost`", "v2board",)
                .is_none()
        );
        assert!(
            classify_source_grant(
                "GRANT SELECT ON `other`.* TO `migration`@`localhost`",
                "v2board",
            )
            .is_none()
        );
        assert!(
            classify_source_grant(
                "GRANT SELECT ON `v2board`.* TO `migration`@`localhost` WITH GRANT OPTION",
                "v2board",
            )
            .is_none()
        );
    }

    #[test]
    fn redis_physical_keyspace_inventory_is_exact_and_bounded_to_declared_databases() {
        assert_eq!(redis_database_index("redis://127.0.0.1:6379").unwrap(), 0);
        assert_eq!(
            redis_database_index("redis://127.0.0.1:6379/12").unwrap(),
            12
        );
        assert!(redis_database_index("redis://127.0.0.1:6379/not-a-db").is_err());
        let keyspaces = parse_redis_keyspace_info(
            "# Keyspace\r\ndb0:keys=3,expires=1,avg_ttl=9\r\ndb2:keys=7,expires=0,avg_ttl=0\r\ndb9:keys=11,expires=0,avg_ttl=0\r\n",
        )
        .expect("strict keyspace inventory");
        assert_eq!(keyspaces, BTreeMap::from([(0, 3), (2, 7), (9, 11)]));
        assert_eq!(keyspace_keys_outside(&keyspaces, 0, Some(2)), 11);
        assert_eq!(keyspace_keys_outside(&keyspaces, 9, None), 10);
        assert!(validate_selected_redis_database_size(2, 7, &keyspaces).is_ok());
        assert!(validate_selected_redis_database_size(5, 0, &keyspaces).is_ok());
        assert!(validate_selected_redis_database_size(2, 8, &keyspaces).is_err());
        assert!(validate_selected_redis_database_size(5, 1, &keyspaces).is_err());
        assert!(parse_redis_keyspace_info("db0:expires=1\n").is_err());
        assert!(parse_redis_keyspace_info("db0:keys=1\ndb0:keys=2\n").is_err());
    }

    #[test]
    fn redis_scan_pages_are_bounded_independently_of_total_keyspace_size() {
        assert!(validate_redis_scan_page(&[b"small-key".to_vec()]).is_ok());
        assert!(validate_redis_scan_page(&vec![Vec::new(); MAX_REDIS_SCAN_PAGE_KEYS + 1]).is_err());
        assert!(validate_redis_scan_page(&[vec![b'x'; MAX_REDIS_KEY_BYTES + 1]]).is_err());
        assert!(
            validate_redis_scan_page(&vec![
                vec![b'x'; MAX_REDIS_KEY_BYTES];
                MAX_REDIS_SCAN_PAGE_BYTES / MAX_REDIS_KEY_BYTES + 1
            ])
            .is_err()
        );
    }

    #[test]
    fn fenced_redis_ownership_allows_only_exact_current_operation_frozen_keys() {
        let current_upload = b"v2board_database_v2board_migration:current:frozen_upload_traffic";
        let current_download =
            b"v2board_database_v2board_migration:current:frozen_download_traffic";
        let migration_prefix = b"v2board_database_v2board_migration:";
        let final_rules = RedisOwnershipRules {
            configured_upload_key: b"v2board_database_v2board_upload_traffic",
            configured_download_key: b"v2board_database_v2board_download_traffic",
            configured_reset_key: b"v2board_database_traffic_reset_lock",
            configured_queue_prefix: b"v2board_database_queues:",
            // Deliberately overlaps the migration namespace: migration keys
            // must still take the exact-operation branch first.
            configured_horizon_prefix: migration_prefix,
            horizon_prefix_enabled: true,
            declared_cache_key_prefix: b"v2board_database_v2board_cache:",
            migration_key_prefix: migration_prefix,
            frozen_upload_key: current_upload,
            frozen_download_key: current_download,
            allow_current_operation_frozen_keys: true,
        };
        assert!(final_rules.default_owned(current_upload));
        assert!(final_rules.default_owned(current_download));
        assert!(
            !final_rules
                .default_owned(b"v2board_database_v2board_migration:stale:frozen_upload_traffic")
        );
        assert!(!final_rules.default_owned(
            b"v2board_database_v2board_migration:current:frozen_upload_traffic:extra"
        ));
        assert!(!final_rules.cache_owned(current_upload));

        let online_rules = RedisOwnershipRules {
            allow_current_operation_frozen_keys: false,
            ..final_rules
        };
        assert!(!online_rules.default_owned(current_upload));
    }

    #[test]
    fn redis_subscription_tokens_are_classified_without_collecting_key_sets() {
        assert!(subscription_token_candidate(b"foreign:totp_123"));
        assert!(subscription_token_candidate(b"prefix:otp_123"));
        assert!(!subscription_token_candidate(b"prefix:session_123"));
        assert!(subscription_token_logical_key(b"otpn_123"));
        assert!(!subscription_token_logical_key(b"nested:otp_123"));
    }

    #[test]
    fn legacy_json_id_arrays_classify_normalization_and_violations() {
        let valid = classify_legacy_json_id_array(
            Some(r#"[1,"2",2147483647,"2147483647"]"#),
            false,
            true,
            i64::from(i32::MAX),
        );
        assert_eq!(valid.ids, vec![1, 2, 2_147_483_647, 2_147_483_647]);
        assert_eq!(valid.requires_normalization, 2);
        assert_eq!(valid.format_violations, 0);
        assert!(valid.array);

        let existing = [1, 2].into_iter().collect::<BTreeSet<_>>();
        assert_eq!(count_missing_reference_violations(&valid.ids, &existing), 2);

        let invalid = classify_legacy_json_id_array(
            Some(
                r#"[0,-1,1.0,1e0,"0","01","+1"," 1","1.0",2147483648,"2147483648",9223372036854775808,"9223372036854775808",null,{},[],true]"#,
            ),
            false,
            false,
            i64::from(i32::MAX),
        );
        assert!(invalid.ids.is_empty());
        assert_eq!(invalid.requires_normalization, 0);
        assert_eq!(invalid.format_violations, 17);

        let sql_null = classify_legacy_json_id_array(None, true, false, i64::MAX);
        assert!(sql_null.sql_null);
        assert_eq!(sql_null.format_violations, 0);
        assert_eq!(
            classify_legacy_json_id_array(None, false, false, i64::MAX).format_violations,
            1
        );
        assert_eq!(
            classify_legacy_json_id_array(Some("{}"), true, false, i64::MAX).format_violations,
            1
        );
        assert_eq!(
            classify_legacy_json_id_array(Some("[]"), false, true, i64::MAX).format_violations,
            1
        );
    }

    #[test]
    fn legacy_cache_prefix_matches_laravel_behavior() {
        assert_eq!(
            laravel_cache_physical_prefix("v2board_database_", "v2board_cache"),
            "v2board_database_v2board_cache:"
        );
        assert_eq!(
            laravel_cache_physical_prefix("v2board_database_", "already::"),
            "v2board_database_already:::"
        );
        assert_eq!(
            laravel_cache_physical_prefix("v2board_database_", ""),
            "v2board_database_"
        );
    }

    #[tokio::test]
    #[ignore = "requires V2BOARD_LEGACY_MYSQL_TEST_URL"]
    async fn legacy_inspection_pool_keeps_the_same_raw_snapshot_session() {
        let database_url =
            std::env::var("V2BOARD_LEGACY_MYSQL_TEST_URL").expect("V2BOARD_LEGACY_MYSQL_TEST_URL");
        let pool = connect_legacy_mysql_with_config(&database_url, &inspection_pool_config())
            .await
            .expect("legacy inspection pool");
        let nonce = begin_source_consistent_snapshot(&pool)
            .await
            .expect("consistent source snapshot");
        let observed =
            sqlx::query_scalar::<_, Option<String>>("SELECT @v2board_inspection_session_nonce")
                .fetch_one(&pool)
                .await
                .expect("same pooled session");
        assert_eq!(observed.as_deref(), Some(nonce.as_str()));
        finish_source_consistent_snapshot(&pool, &nonce)
            .await
            .expect("commit same source snapshot");
        pool.close().await;
    }

    #[tokio::test]
    #[ignore = "requires V2BOARD_LEGACY_MYSQL_TEST_URL with the pinned reference schema"]
    async fn pinned_mysql8_source_supports_the_complete_query_surface() {
        let database_url =
            std::env::var("V2BOARD_LEGACY_MYSQL_TEST_URL").expect("V2BOARD_LEGACY_MYSQL_TEST_URL");
        let pool = connect_legacy_mysql_with_config(&database_url, &inspection_pool_config())
            .await
            .expect("legacy inspection pool");
        let nonce = begin_source_consistent_snapshot(&pool)
            .await
            .expect("consistent source snapshot");
        let server = inspect_server(&pool).await.expect("server metadata");
        let database = server.database_name.as_deref().expect("selected database");
        let access = inspect_source_mysql_access(&pool, database)
            .await
            .expect("source grant inventory");
        assert!(access.grants_are_read_only_and_complete);
        assert_eq!(access.visible_non_source_schema_count, 0);
        assert_eq!(
            database_vendor(&server.version, &server.version_comment),
            DatabaseVendor::MySql
        );
        assert!(version_is_supported_mysql8(&server.version));
        assert!(
            !inspect_mysql_server_uuid(&pool)
                .await
                .expect("server UUID")
                .is_empty()
        );
        assert!(inspect_mysql_topology(&pool).await.is_standalone_visible());
        let tables = inspect_tables(&pool).await.expect("table inventory");
        assert_eq!(
            semantic_schema_hash(&pool).await.expect("semantic schema"),
            LEGACY_SCHEMA_SHA256_V1.expect("reviewed legacy schema")
        );
        inspect_non_table_objects(&pool)
            .await
            .expect("non-table object inventory");
        inspect_data(&pool, &tables)
            .await
            .expect("business-data admission queries");
        finish_source_consistent_snapshot(&pool, &nonce)
            .await
            .expect("commit same source snapshot");
        pool.close().await;
    }

    #[test]
    fn operational_requirements_are_pending_online_and_block_final_plan() {
        let mut blockers = Vec::new();
        let mut pending = Vec::new();
        record_final_requirement(
            InspectionMode::Online,
            &mut blockers,
            &mut pending,
            "drain queue",
        );
        assert!(blockers.is_empty());
        assert_eq!(pending, ["drain queue"]);

        blockers.clear();
        pending.clear();
        record_final_requirement(
            InspectionMode::FencedFinal,
            &mut blockers,
            &mut pending,
            "drain queue",
        );
        assert_eq!(blockers, ["drain queue"]);
        assert!(pending.is_empty());
    }

    #[test]
    fn only_the_typed_legacy_capability_can_make_apply_available() {
        assert!(!production_apply_available_with_capability(
            ProvisionKind::LegacyReferenceMigration,
            ProductionLegacyApplyCapability::Unavailable(
                crate::legacy_apply_capability::ProductionLegacyApplyBlocker::
                    AwaitingBareMetalFaultMatrixAndSafetyAudit,
            ),
        ));
        assert!(production_apply_available_with_capability(
            ProvisionKind::LegacyReferenceMigration,
            ProductionLegacyApplyCapability::Available,
        ));
        assert!(!production_apply_available_with_capability(
            ProvisionKind::FreshInstall,
            ProductionLegacyApplyCapability::Available,
        ));
        assert!(!production_apply_available_with_capability(
            ProvisionKind::NativeUpgrade,
            ProductionLegacyApplyCapability::Available,
        ));
    }

    #[test]
    fn review_binding_excludes_live_observations_but_keeps_operation_identity() {
        let mut plan = ProvisionPlan {
            report_version: PROVISION_REPORT_VERSION,
            scope: InspectionMode::Online.scope(),
            kind: ProvisionKind::LegacyReferenceMigration,
            converter_available: true,
            apply_available: true,
            operation_id: "018f47b8-5ab1-7a00-8000-000000000001".to_string(),
            manifest_binding_hmac_sha256: "a".repeat(64),
            review_binding_sha256: String::new(),
            review_binding_hmac_sha256: String::new(),
            report_sha256: String::new(),
            report_binding_hmac_sha256: String::new(),
            verdict: PreflightVerdict::Ready,
            next_action: NextAction::AuthorizeApply,
            operator_attestations_complete: Some(false),
            source: None,
            target_postgres: None,
            target_clickhouse: None,
            data: None,
            source_redis: None,
            target_redis: None,
            node_inventory: None,
            backup_restore: None,
            source_control: None,
            release_archive: None,
            native_upgrade: None,
            implementation_blockers: Vec::new(),
            blockers: Vec::new(),
            pending_final_requirements: vec!["traffic is still moving".to_string()],
            warnings: vec!["snapshot has 10 users".to_string()],
        };
        set_plan_outcome(
            &mut plan,
            ProductionLegacyApplyCapability::Unavailable(
                crate::legacy_apply_capability::ProductionLegacyApplyBlocker::
                    AwaitingBareMetalFaultMatrixAndSafetyAudit,
            ),
        );
        assert!(!plan.apply_available);
        assert_eq!(plan.verdict, PreflightVerdict::Blocked);
        assert_eq!(plan.next_action, NextAction::ResolveBlockers);
        assert!(!plan.passed_with_capability(
            ProductionLegacyApplyCapability::Unavailable(
                crate::legacy_apply_capability::ProductionLegacyApplyBlocker::
                    AwaitingBareMetalFaultMatrixAndSafetyAudit,
            )
        ));
        set_plan_outcome(&mut plan, ProductionLegacyApplyCapability::Available);
        assert!(plan.apply_available);
        assert_eq!(plan.verdict, PreflightVerdict::Blocked);
        assert_eq!(plan.next_action, NextAction::ResolveBlockers);
        assert!(!plan.passed_with_capability(ProductionLegacyApplyCapability::Available));
        assert!(!plan.passed());
        let reviewed = inspection_review_binding_bytes(&plan);
        plan.report_sha256 = "b".repeat(64);
        plan.report_binding_hmac_sha256 = "c".repeat(64);
        plan.pending_final_requirements = vec!["traffic is now drained".to_string()];
        plan.warnings = vec!["snapshot has 11 users".to_string()];
        assert_eq!(inspection_review_binding_bytes(&plan), reviewed);

        plan.operation_id = "018f47b8-5ab1-7a00-8000-000000000002".to_string();
        assert_ne!(inspection_review_binding_bytes(&plan), reviewed);
    }

    #[tokio::test]
    #[ignore = "profile generation requires the pinned reference schema in disposable MySQL"]
    async fn print_reviewed_legacy_profile_hash() {
        let database_url = std::env::var("V2BOARD_LEGACY_PROFILE_DATABASE_URL")
            .expect("V2BOARD_LEGACY_PROFILE_DATABASE_URL");
        let pool = connect_legacy_mysql_with_config(
            &database_url,
            &DbPoolConfig {
                min_connections: 0,
                max_connections: 1,
                ..DbPoolConfig::default()
            },
        )
        .await
        .expect("profile database");
        let hash = semantic_schema_hash(&pool).await.expect("semantic hash");
        println!("LEGACY_SCHEMA_SHA256_V1={hash}");
    }
}
