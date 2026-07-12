use std::{
    collections::BTreeSet,
    time::{SystemTime, UNIX_EPOCH},
};

use redis::aio::ConnectionManager;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sqlx::{AssertSqlSafe, FromRow, MySql, MySqlPool, PgPool, QueryBuilder};
use uuid::Uuid;
use v2board_db::{DbPoolConfig, connect_postgres_with_config};

use crate::legacy_mysql::connect_legacy_mysql_with_config;
use crate::manifest::{
    ClickHouseTargetSpec, FreshInstallAttestationSpec, LegacyAttestationSpec,
    NativeInstallationSpec, NativeUpgradeAttestationSpec, NativeUpgradeChangeSpec,
    NativeUpgradeDecisionSpec, NativeUpgradeImpactSpec, PostgresTargetSpec, ProvisionFlow,
    ProvisionKind, ProvisionSpec, SourceSpec, SourceTransportSecurity, TargetSpec,
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
const MAX_REDIS_SCAN_KEYS: usize = 100_000;
const LEGACY_JSON_SCAN_PAGE_SIZE: usize = 1_000;
const LEGACY_JSON_REFERENCE_BATCH_SIZE: usize = 1_000;
const CONVERTER_AVAILABLE: bool = false;
const APPLY_AVAILABLE: bool = false;

#[derive(Serialize)]
pub struct ProvisionPlan {
    pub report_version: u32,
    pub scope: &'static str,
    pub kind: ProvisionKind,
    pub converter_available: bool,
    pub apply_available: bool,
    pub operation_id: String,
    pub manifest_binding_hmac_sha256: String,
    pub report_sha256: String,
    pub report_binding_hmac_sha256: String,
    pub verdict: PreflightVerdict,
    pub next_action: NextAction,
    pub operator_attestations_complete: bool,
    pub source: Option<DatabaseInspection>,
    pub target_postgres: Option<PostgresInspection>,
    pub target_clickhouse: Option<ClickHouseInspection>,
    pub data: Option<DataInspection>,
    pub source_redis: Option<SourceRedisInspection>,
    pub target_redis: Option<TargetRedisInspection>,
    pub native_upgrade: Option<NativeUpgradeInspection>,
    pub implementation_blockers: Vec<String>,
    pub blockers: Vec<String>,
    pub pending_final_requirements: Vec<String>,
    pub warnings: Vec<String>,
}

#[derive(Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PreflightVerdict {
    Blocked,
}

#[derive(Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum NextAction {
    ResolveBlockers,
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

#[derive(Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DatabaseVendor {
    MySql,
    Percona,
    MariaDb,
    Unknown,
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
    pub legacy_subscription_not_after_unix: i64,
    pub legacy_subscription_window_elapsed: bool,
}

#[derive(Serialize)]
pub struct TargetRedisInspection {
    pub key_count: u64,
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
    seq_in_index: u64,
    column_name: Option<String>,
    sub_part: Option<u64>,
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
        } => build_legacy_migration_inspection(spec, source, target, attestations, mode).await,
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

struct TargetBundle {
    postgres: PostgresInspection,
    clickhouse: ClickHouseInspection,
    redis: TargetRedisInspection,
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
        "analytics outbox capacity/backpressure admission gate and bounded ClickHouse-outage policy are not implemented"
            .to_string(),
        "ClickHouse TTL/archive/restore lifecycle and HA/Keeper topology are not implemented; the implemented single-node schema lock and installation binding do not satisfy those gates"
            .to_string(),
    ];
    let plan = ProvisionPlan {
        report_version: 3,
        scope: mode.scope(),
        kind: spec.kind,
        converter_available: CONVERTER_AVAILABLE,
        apply_available: APPLY_AVAILABLE,
        operation_id: spec.operation_id.clone(),
        manifest_binding_hmac_sha256: spec.manifest_binding_hmac_sha256().to_string(),
        report_sha256: String::new(),
        report_binding_hmac_sha256: String::new(),
        verdict: PreflightVerdict::Blocked,
        next_action: NextAction::ResolveBlockers,
        operator_attestations_complete,
        source: None,
        target_postgres: Some(target_bundle.postgres),
        target_clickhouse: Some(target_bundle.clickhouse),
        data: None,
        source_redis: None,
        target_redis: Some(target_bundle.redis),
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
    attestations: &LegacyAttestationSpec,
    mode: InspectionMode,
) -> Result<ProvisionPlan, ProvisionPlanError> {
    let pool_config = inspection_pool_config();
    let source_pool = connect_legacy_mysql_with_config(&source.database_url, &pool_config)
        .await
        .map_err(ProvisionPlanError::SourceDatabase)?;
    let source_server = inspect_server(&source_pool)
        .await
        .map_err(ProvisionPlanError::SourceQuery)?;
    let source_transaction_read_only = inspect_source_transaction_read_only(&source_pool)
        .await
        .map_err(ProvisionPlanError::SourceQuery)?;
    let source_vendor = database_vendor(&source_server.version, &source_server.version_comment);
    let source_server_uuid_raw = if matches!(
        source_vendor,
        DatabaseVendor::MySql | DatabaseVendor::Percona
    ) {
        inspect_mysql_server_uuid(&source_pool)
            .await
            .map_err(ProvisionPlanError::SourceQuery)?
    } else {
        String::new()
    };
    let source_server_uuid = canonical_mysql_server_uuid(&source_server_uuid_raw);
    let source_topology = match source_vendor {
        DatabaseVendor::MySql | DatabaseVendor::Percona => {
            inspect_mysql_topology(&source_pool).await
        }
        DatabaseVendor::MariaDb => inspect_mariadb_topology(&source_pool).await,
        DatabaseVendor::Unknown => MysqlTopology::default(),
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
    let source_redis = inspect_source_redis(
        &source.redis_default_url,
        &source.redis_cache_url,
        &source.redis_connection_prefix,
        &source.redis_cache_prefix,
        source.legacy_show_subscribe_method,
        source.legacy_show_subscribe_expire_minutes,
        source.legacy_subscription_issuance_stopped_at_unix,
    )
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
    let supported_source = match source_vendor {
        DatabaseVendor::MySql | DatabaseVendor::Percona => {
            version_at_least(&source_server.version, 5, 7, 0)
        }
        DatabaseVendor::MariaDb => version_at_least(&source_server.version, 10, 2, 0),
        DatabaseVendor::Unknown => false,
    };
    if !supported_source {
        blockers.push("source must be supported MySQL/Percona 5.7+ or MariaDB 10.2+".to_string());
    }
    if !source_transaction_read_only {
        blockers.push("source SQL inspector session is not read-only".into());
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
    if matches!(
        source_vendor,
        DatabaseVendor::MySql | DatabaseVendor::Percona
    ) && source_server_uuid.is_none()
    {
        blockers.push("source MySQL/Percona server_uuid is missing or invalid".into());
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
        source,
        &data,
        &source_redis,
        mode,
        &mut blockers,
        &mut pending_final_requirements,
    );

    let operator_attestations_complete = attestations.source_writers_stopped
        && attestations.source_workers_stopped
        && attestations.node_reporters_stopped
        && attestations.legacy_queues_drained
        && attestations
            .backup_reference
            .as_deref()
            .is_some_and(|reference| !reference.trim().is_empty())
        && attestations.restore_tested;
    if !operator_attestations_complete {
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
            "all node reporters must stay stopped until every scoped token and stable idempotency key passes offline verification; only then may the native reporters start together"
                .into(),
        );
    }
    if source_redis.source_cache_key_count != 0 || source_redis.source_default_key_count != 0 {
        warnings.push(
            "legacy Redis contains classified ephemeral state plus namespaces that still require converter ownership rules"
                .into(),
        );
    }
    if source_redis.queue_notify_item_count != 0 {
        warnings.push(
            "legacy Redis queue notify wake tokens remain; they are excluded from durable queued_item_count"
                .into(),
        );
    }
    if data.legacy_json_id_arrays.requires_normalization() != 0 {
        warnings.push(
            "legacy JSON ID arrays contain canonical positive-decimal strings that a future converter must normalize to JSON numbers"
                .into(),
        );
    }

    let implementation_blockers = vec![
        "one-shot offline MySQL-to-PostgreSQL converter, type mapping, crash-resume journal, and value verification are not implemented"
            .to_string(),
        "PostgreSQL/ClickHouse bootstrap, backup binding, pre-commit abort, atomic cutover, source retirement, and forward-recovery apply are not implemented"
            .to_string(),
        "source read grants, cross-datastore snapshot consistency, Stripe provider zero-state, and physical topology separation are not fully machine-bound"
            .to_string(),
        "legacy Redis durable unknown-key ownership is not completely classified".to_string(),
        "analytics outbox capacity/backpressure admission gate and bounded ClickHouse-outage policy are not implemented"
            .to_string(),
        "ClickHouse TTL/archive/restore lifecycle and HA/Keeper topology are not implemented; the implemented single-node schema lock and installation binding do not satisfy those gates"
            .to_string(),
        "role-owned API/worker 0700 directory and 0600 config writes plus atomic bare-metal promotion are not implemented"
            .to_string(),
    ];
    let source_report = DatabaseInspection {
        vendor: source_vendor,
        version: source_server.version,
        version_comment: source_server.version_comment,
        database_name: source_server.database_name.unwrap_or_default(),
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
    };
    let plan = ProvisionPlan {
        report_version: 3,
        scope: mode.scope(),
        kind: spec.kind,
        converter_available: CONVERTER_AVAILABLE,
        apply_available: APPLY_AVAILABLE,
        operation_id: spec.operation_id.clone(),
        manifest_binding_hmac_sha256: spec.manifest_binding_hmac_sha256().to_string(),
        report_sha256: String::new(),
        report_binding_hmac_sha256: String::new(),
        verdict: PreflightVerdict::Blocked,
        next_action: NextAction::ResolveBlockers,
        operator_attestations_complete,
        source: Some(source_report),
        target_postgres: Some(target_bundle.postgres),
        target_clickhouse: Some(target_bundle.clickhouse),
        data: Some(data),
        source_redis: Some(source_redis),
        target_redis: Some(target_bundle.redis),
        native_upgrade: None,
        implementation_blockers,
        blockers,
        pending_final_requirements,
        warnings,
    };
    Ok(finalize_plan(spec, plan))
}

fn append_legacy_data_blockers(
    source: &SourceSpec,
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
    if source_redis.legacy_subscription_token_keys != 0 {
        record_final_requirement(
            mode,
            blockers,
            pending,
            "legacy Redis still contains issued OTP/TOTP subscription tokens",
        );
    }
    if source_redis.ambiguous_subscription_token_keys != 0 {
        blockers.push("source Redis has OTP/TOTP-like keys outside the declared prefix".into());
    }
    if source.legacy_show_subscribe_method != 0
        && source.legacy_subscription_issuance_stopped_at_unix <= 0
    {
        record_final_requirement(
            mode,
            blockers,
            pending,
            "legacy subscription issuance has no recorded fence time",
        );
    } else if !source_redis.legacy_subscription_window_elapsed {
        record_final_requirement(
            mode,
            blockers,
            pending,
            "legacy ephemeral subscription URLs have not reached their expiry window",
        );
    }
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
        "analytics outbox capacity/backpressure admission gate and bounded ClickHouse-outage policy are not implemented"
            .to_string(),
        "ClickHouse TTL/archive/restore lifecycle and HA/Keeper topology are not implemented; the implemented single-node schema lock and installation binding do not satisfy those gates"
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
            report_version: 3,
            scope: mode.scope(),
            kind: spec.kind,
            converter_available: CONVERTER_AVAILABLE,
            apply_available: APPLY_AVAILABLE,
            operation_id: spec.operation_id.clone(),
            manifest_binding_hmac_sha256: spec.manifest_binding_hmac_sha256().to_string(),
            report_sha256: String::new(),
            report_binding_hmac_sha256: String::new(),
            verdict: PreflightVerdict::Blocked,
            next_action: NextAction::ResolveBlockers,
            operator_attestations_complete,
            source: None,
            target_postgres: None,
            target_clickhouse: None,
            data: None,
            source_redis: None,
            target_redis: None,
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

fn finalize_plan(spec: &ProvisionSpec, mut plan: ProvisionPlan) -> ProvisionPlan {
    let bytes = serde_json::to_vec(&plan).expect("provision plan is serializable");
    plan.report_sha256 = hex::encode(Sha256::digest(&bytes));
    plan.report_binding_hmac_sha256 = spec.report_binding_hmac_sha256(&bytes);
    plan
}

async fn inspect_target_bundle(target: &TargetSpec) -> Result<TargetBundle, ProvisionPlanError> {
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
               has_database_privilege(current_user, current_database(), 'CREATE') AS has_database_create
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
    pub const fn passed(&self) -> bool {
        false
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

async fn inspect_source_transaction_read_only(pool: &MySqlPool) -> Result<bool, sqlx::Error> {
    match sqlx::query_scalar::<_, i64>("SELECT @@SESSION.transaction_read_only")
        .fetch_one(pool)
        .await
    {
        Ok(value) => Ok(value != 0),
        Err(_) => sqlx::query_scalar::<_, i64>("SELECT @@SESSION.tx_read_only")
            .fetch_one(pool)
            .await
            .map(|value| value != 0),
    }
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

async fn inspect_mariadb_topology(pool: &MySqlPool) -> MysqlTopology {
    let replication_channel_count = sqlx::query("SHOW ALL SLAVES STATUS")
        .fetch_all(pool)
        .await
        .ok()
        .and_then(|rows| i64::try_from(rows.len()).ok());
    let registered_replica_count = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM information_schema.PROCESSLIST \
         WHERE COMMAND IN ('Binlog Dump', 'Binlog Dump GTID')",
    )
    .fetch_one(pool)
    .await
    .ok();
    MysqlTopology {
        replication_channel_count,
        group_replication_member_count: Some(0),
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

async fn semantic_schema_hash(pool: &MySqlPool) -> Result<String, sqlx::Error> {
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
               SEQ_IN_INDEX AS seq_in_index, COLUMN_NAME AS column_name, SUB_PART AS sub_part,
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
        hash_parts(
            &mut hasher,
            &[
                "column",
                &row.table_name,
                &row.ordinal_position.to_string(),
                &row.column_name,
                &normalize_column_type(&row.column_type),
                &row.is_nullable,
                row.column_default.as_deref().unwrap_or("<NULL>"),
                &row.extra,
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
    default_url: &str,
    cache_url: &str,
    connection_prefix: &str,
    cache_prefix: &str,
    legacy_show_subscribe_method: i32,
    legacy_show_subscribe_expire_minutes: i64,
    legacy_subscription_issuance_stopped_at_unix: i64,
) -> Result<SourceRedisInspection, redis::RedisError> {
    let mut default_connection = redis_connection(default_url).await?;
    let (_, source_default_identity) = redis_server_identity(&mut default_connection).await?;
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
    let queues = inspect_queues(&mut default_connection, connection_prefix).await?;
    let retryable_failed_job_items =
        pattern_collection_item_count(&mut default_connection, "*failed_jobs").await?;
    let configured_upload_key = format!("{connection_prefix}v2board_upload_traffic");
    let configured_download_key = format!("{connection_prefix}v2board_download_traffic");
    let mut traffic_candidates =
        scan_keys(&mut default_connection, "*v2board_upload_traffic").await?;
    traffic_candidates
        .extend(scan_keys(&mut default_connection, "*v2board_download_traffic").await?);
    traffic_candidates.sort();
    traffic_candidates.dedup();
    let unexpected_traffic_key_candidates = traffic_candidates
        .iter()
        .filter(|key| *key != &configured_upload_key && *key != &configured_download_key)
        .count() as u64;
    let traffic_reset_lock_keys = scan_keys(&mut default_connection, "*traffic_reset_lock")
        .await?
        .len() as u64;
    let source_default_key_count = scan_keys(&mut default_connection, "*").await?.len() as u64;

    let mut cache_connection = redis_connection(cache_url).await?;
    let (_, source_cache_identity) = redis_server_identity(&mut cache_connection).await?;
    let declared_cache_key_prefix = laravel_cache_physical_prefix(connection_prefix, cache_prefix);
    let (legacy_subscription_token_keys, ambiguous_subscription_token_keys) =
        subscription_token_key_counts(&mut cache_connection, &declared_cache_key_prefix).await?;
    let source_cache_key_count = scan_keys(&mut cache_connection, "*").await?.len() as u64;
    let (legacy_subscription_not_after_unix, legacy_subscription_window_elapsed) =
        legacy_subscription_window(
            legacy_show_subscribe_method,
            legacy_show_subscribe_expire_minutes,
            legacy_subscription_issuance_stopped_at_unix,
        );
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
        upload_traffic_fields: upload.fields,
        download_traffic_fields: download.fields,
        upload_traffic_sum: upload.sum.to_string(),
        download_traffic_sum: download.sum.to_string(),
        malformed_traffic_values: upload.malformed + download.malformed,
        unexpected_traffic_key_candidates,
        traffic_reset_lock_keys,
        queued_item_count: queues.durable_items,
        queue_notify_item_count: queues.notify_items,
        ambiguous_queue_key_candidates: queues.ambiguous_key_candidates,
        retryable_failed_job_items,
        legacy_subscription_token_keys,
        ambiguous_subscription_token_keys,
        legacy_subscription_not_after_unix,
        legacy_subscription_window_elapsed,
    })
}

fn legacy_subscription_window(
    method: i32,
    expire_minutes: i64,
    issuance_stopped_at_unix: i64,
) -> (i64, bool) {
    if method == 0 {
        return (0, true);
    }
    let ttl_seconds = if method == 1 {
        24 * 60 * 60
    } else {
        // A method-2 URL can first be accepted near the end of its issuance
        // bucket and then remain in Laravel Cache for one more full timestep.
        expire_minutes.saturating_mul(60).saturating_mul(2)
    };
    // The old method-2 token is valid for the current time bucket. Five
    // minutes cover clock skew between the fenced old host and this inspector.
    let not_after = issuance_stopped_at_unix
        .saturating_add(ttl_seconds)
        .saturating_add(5 * 60);
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .ok()
        .and_then(|duration| i64::try_from(duration.as_secs()).ok())
        .unwrap_or(0);
    (not_after, now >= not_after)
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

struct QueueInspection {
    durable_items: u64,
    notify_items: u64,
    ambiguous_key_candidates: u64,
}

async fn inspect_queues(
    connection: &mut ConnectionManager,
    connection_prefix: &str,
) -> Result<QueueInspection, redis::RedisError> {
    let all = scan_keys(connection, "*queues:*")
        .await?
        .into_iter()
        .collect::<BTreeSet<_>>();
    let owned_prefix = format!("{connection_prefix}queues:");
    let owned = scan_keys(connection, &format!("{owned_prefix}*"))
        .await?
        .into_iter()
        .collect::<BTreeSet<_>>();
    let mut durable_items = 0_u64;
    let mut notify_items = 0_u64;
    for key in &owned {
        let count = redis_collection_item_count(connection, key).await?;
        if key.strip_prefix(&owned_prefix).is_some_and(|logical| {
            logical
                .strip_suffix(":notify")
                .is_some_and(|queue| !queue.is_empty())
        }) {
            notify_items = notify_items.saturating_add(count);
        } else {
            durable_items = durable_items.saturating_add(count);
        }
    }
    Ok(QueueInspection {
        durable_items,
        notify_items,
        ambiguous_key_candidates: all.difference(&owned).count() as u64,
    })
}

async fn pattern_collection_item_count(
    connection: &mut ConnectionManager,
    pattern: &str,
) -> Result<u64, redis::RedisError> {
    let mut total = 0_u64;
    for key in scan_keys(connection, pattern).await? {
        total = total.saturating_add(redis_collection_item_count(connection, &key).await?);
    }
    Ok(total)
}

async fn redis_collection_item_count(
    connection: &mut ConnectionManager,
    key: &str,
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

async fn subscription_token_key_counts(
    connection: &mut ConnectionManager,
    declared_prefix: &str,
) -> Result<(u64, u64), redis::RedisError> {
    let mut keys = BTreeSet::new();
    for pattern in ["*otp_*", "*otpn_*", "*totp_*"] {
        keys.extend(scan_keys(connection, pattern).await?);
    }
    let mut declared = BTreeSet::new();
    for suffix in ["otp_*", "otpn_*", "totp_*"] {
        declared.extend(scan_keys(connection, &format!("{declared_prefix}{suffix}")).await?);
    }
    Ok((
        declared.len() as u64,
        keys.difference(&declared).count() as u64,
    ))
}

struct TargetRedisStatus {
    key_count: u64,
    version: String,
    identity: RedisServerIdentity,
    redis_6_2_or_newer: bool,
    getdel_available: bool,
    evalsha_available: bool,
    script_available: bool,
}

async fn inspect_target_redis(redis_url: &str) -> Result<TargetRedisStatus, redis::RedisError> {
    let mut connection = redis_connection(redis_url).await?;
    let key_count = scan_keys(&mut connection, "*").await?.len() as u64;
    let (version, identity) = redis_server_identity(&mut connection).await?;
    let redis_6_2_or_newer = version_at_least(&version, 6, 2, 0);
    let getdel_available = redis_command_available(&mut connection, "GETDEL").await?;
    let evalsha_available = redis_command_available(&mut connection, "EVALSHA").await?;
    let script_available = redis_command_available(&mut connection, "SCRIPT").await?;
    Ok(TargetRedisStatus {
        key_count,
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

async fn scan_keys(
    connection: &mut ConnectionManager,
    pattern: &str,
) -> Result<Vec<String>, redis::RedisError> {
    let mut cursor = 0_u64;
    let mut keys = Vec::new();
    loop {
        let (next, mut page) = redis::cmd("SCAN")
            .arg(cursor)
            .arg("MATCH")
            .arg(pattern)
            .arg("COUNT")
            .arg(256)
            .query_async::<(u64, Vec<String>)>(connection)
            .await?;
        if keys.len().saturating_add(page.len()) > MAX_REDIS_SCAN_KEYS {
            return Err(redis::RedisError::from((
                redis::ErrorKind::Client,
                "provision Redis scan exceeded the 100000-key safety limit",
            )));
        }
        keys.append(&mut page);
        if next == 0 {
            break;
        }
        cursor = next;
    }
    keys.sort();
    keys.dedup();
    Ok(keys)
}

async fn redis_connection(redis_url: &str) -> Result<ConnectionManager, redis::RedisError> {
    let client = redis::Client::open(redis_url)?;
    ConnectionManager::new(client).await
}

fn database_vendor(version: &str, comment: &str) -> DatabaseVendor {
    let text = format!("{version} {comment}").to_ascii_lowercase();
    if text.contains("mariadb") {
        DatabaseVendor::MariaDb
    } else if text.contains("percona server") {
        DatabaseVendor::Percona
    } else if text.contains("mysql") {
        DatabaseVendor::MySql
    } else {
        DatabaseVendor::Unknown
    }
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
    fn vendor_and_version_detection_is_fail_closed_for_mariadb() {
        assert!(matches!(
            database_vendor("8.4.4", "MySQL Community Server - GPL"),
            DatabaseVendor::MySql
        ));
        assert!(matches!(
            database_vendor("5.7.29", "MySQL Community Server"),
            DatabaseVendor::MySql
        ));
        assert!(matches!(
            database_vendor("8.0.37-29", "Percona Server (GPL)"),
            DatabaseVendor::Percona
        ));
        assert!(matches!(
            database_vendor("10.11.8-MariaDB", "mariadb.org binary distribution"),
            DatabaseVendor::MariaDb
        ));
        assert!(matches!(
            database_vendor("8.4.4-compatible", "Unknown SQL proxy"),
            DatabaseVendor::Unknown
        ));
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
    fn canonicalization_ignores_integer_display_width_and_utf8_alias() {
        assert_eq!(normalize_column_type("int(11) unsigned"), "int unsigned");
        assert_eq!(normalize_column_type("bigint(20)"), "bigint");
        assert_eq!(normalize_charset("utf8mb3_unicode_ci"), "utf8_unicode_ci");
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
    fn legacy_cache_prefix_and_subscription_window_match_laravel_behavior() {
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

        let (not_after, _) = legacy_subscription_window(2, 5, 1_000);
        assert_eq!(not_after, 1_900);
        let (not_after, _) = legacy_subscription_window(1, 5, 1_000);
        assert_eq!(not_after, 87_700);
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
