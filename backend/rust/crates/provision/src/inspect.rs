use std::{
    collections::BTreeSet,
    time::{SystemTime, UNIX_EPOCH},
};

use redis::aio::ConnectionManager;
use serde::Serialize;
use sha2::{Digest, Sha256};
use sqlx::{AssertSqlSafe, FromRow, MySqlPool};
use uuid::Uuid;
use v2board_db::{DbPoolConfig, connect_mysql_with_config};

use crate::{ProvisionSpec, SourceTransportSecurity};

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
const APPLY_AVAILABLE: bool = false;

#[derive(Serialize)]
pub struct ProvisionPlan {
    pub report_version: u32,
    pub scope: &'static str,
    pub apply_available: bool,
    pub operation_id: String,
    pub manifest_binding_hmac_sha256: String,
    pub report_sha256: String,
    pub verdict: PreflightVerdict,
    pub next_action: NextAction,
    pub operator_attestations_complete: bool,
    pub source: DatabaseInspection,
    pub target: TargetInspection,
    pub data: DataInspection,
    pub redis: RedisInspection,
    pub implementation_blockers: Vec<String>,
    pub blockers: Vec<String>,
    pub pending_final_requirements: Vec<String>,
    pub warnings: Vec<String>,
}

#[derive(Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PreflightVerdict {
    Compatible,
    ReadyForConfirmation,
    Blocked,
}

#[derive(Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum NextAction {
    ResolveBlockers,
    ConfirmEnterMaintenance,
    ConfirmOperationIdAndReportSha,
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
pub struct TargetInspection {
    pub vendor: DatabaseVendor,
    pub version: String,
    pub bootstrap_database_name: String,
    pub server_uuid: String,
    pub server_uuid_valid: bool,
    pub replication_channel_count: Option<i64>,
    pub group_replication_member_count: Option<i64>,
    pub registered_replica_count: Option<i64>,
    pub application_database_name: String,
    pub application_username: String,
    pub application_account_host: String,
    pub database_absent: bool,
    pub application_account_absent: bool,
    pub desired_character_set: &'static str,
    pub desired_collation: &'static str,
    pub empty_redis: bool,
    pub mysql_8_4_or_newer: bool,
}

#[derive(Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DatabaseVendor {
    MySql,
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
}

#[derive(Serialize)]
pub struct RedisInspection {
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
    pub target_key_count: u64,
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

#[derive(Debug, thiserror::Error)]
pub enum ProvisionPlanError {
    #[error("source database inspection failed")]
    SourceDatabase(#[source] v2board_db::DbInitError),
    #[error("target database inspection failed")]
    TargetDatabase(#[source] v2board_db::DbInitError),
    #[error("source database query failed")]
    SourceQuery(#[source] sqlx::Error),
    #[error("target database query failed")]
    TargetQuery(#[source] sqlx::Error),
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
    let pool_config = DbPoolConfig {
        min_connections: 0,
        max_connections: 1,
        ..DbPoolConfig::default()
    };
    let source_pool = connect_mysql_with_config(&spec.source.database_url, &pool_config)
        .await
        .map_err(ProvisionPlanError::SourceDatabase)?;
    let target_pool = connect_mysql_with_config(&spec.target.bootstrap_database_url, &pool_config)
        .await
        .map_err(ProvisionPlanError::TargetDatabase)?;

    let source_server = inspect_server(&source_pool)
        .await
        .map_err(ProvisionPlanError::SourceQuery)?;
    let target_server = inspect_server(&target_pool)
        .await
        .map_err(ProvisionPlanError::TargetQuery)?;
    let source_vendor = database_vendor(&source_server.version, &source_server.version_comment);
    let target_vendor = database_vendor(&target_server.version, &target_server.version_comment);
    let source_server_uuid_raw = if matches!(source_vendor, DatabaseVendor::MySql) {
        inspect_mysql_server_uuid(&source_pool)
            .await
            .map_err(ProvisionPlanError::SourceQuery)?
    } else {
        String::new()
    };
    let target_server_uuid_raw = if matches!(target_vendor, DatabaseVendor::MySql) {
        inspect_mysql_server_uuid(&target_pool)
            .await
            .map_err(ProvisionPlanError::TargetQuery)?
    } else {
        String::new()
    };
    let source_server_uuid = canonical_mysql_server_uuid(&source_server_uuid_raw);
    let target_server_uuid = canonical_mysql_server_uuid(&target_server_uuid_raw);
    let source_mysql_topology = if matches!(source_vendor, DatabaseVendor::MySql) {
        inspect_mysql_topology(&source_pool).await
    } else {
        MysqlTopology::default()
    };
    let target_mysql_topology = if matches!(target_vendor, DatabaseVendor::MySql) {
        inspect_mysql_topology(&target_pool).await
    } else {
        MysqlTopology::default()
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
    let target_application_database_name = spec.target.application_database_name();
    let target_application_username = spec.target.application_username();
    let target_database_absent =
        target_database_absent(&target_pool, &target_application_database_name)
            .await
            .map_err(ProvisionPlanError::TargetQuery)?;
    let target_application_account_absent = target_account_absent(
        &target_pool,
        &target_application_username,
        &spec.target.application_account_host,
    )
    .await
    .map_err(ProvisionPlanError::TargetQuery)?;
    let data = inspect_data(&source_pool, &source_tables)
        .await
        .map_err(ProvisionPlanError::SourceQuery)?;
    let source_redis = inspect_source_redis(
        &spec.source.redis_default_url,
        &spec.source.redis_cache_url,
        &spec.source.redis_connection_prefix,
        &spec.source.redis_cache_prefix,
        spec.source.legacy_show_subscribe_method,
        spec.source.legacy_show_subscribe_expire_minutes,
        spec.source.legacy_subscription_issuance_stopped_at_unix,
    )
    .await
    .map_err(ProvisionPlanError::SourceRedis)?;
    let target_redis = inspect_target_redis(&spec.target.redis_url)
        .await
        .map_err(ProvisionPlanError::TargetRedis)?;
    let target_key_count = target_redis.key_count;

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
    let target_8_4 = matches!(target_vendor, DatabaseVendor::MySql)
        && version_at_least(&target_server.version, 8, 4, 0);

    let mut blockers = Vec::new();
    let mut implementation_blockers = Vec::new();
    let mut pending_final_requirements = Vec::new();
    if !matches!(source_vendor, DatabaseVendor::MySql)
        || !version_at_least(&source_server.version, 5, 7, 0)
    {
        blockers.push("source database must be MySQL 5.7 or newer".to_string());
    }
    if !missing_core_tables.is_empty() || !unexpected_source_tables.is_empty() {
        blockers
            .push("source core table inventory does not match the pinned legacy profile".into());
    }
    if native_migration_ledger_present {
        blockers.push("source already contains a native migration ledger".into());
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
    if !target_8_4 {
        blockers.push("target database must be MySQL 8.4 or newer".into());
    }
    if !target_database_absent {
        blockers.push("target application database already exists".into());
    }
    if !target_application_account_absent {
        blockers.push("target application MySQL account already exists".into());
    }
    if matches!(source_vendor, DatabaseVendor::MySql) && source_server_uuid.is_none() {
        blockers.push("source MySQL server_uuid is missing or invalid".into());
    }
    if matches!(target_vendor, DatabaseVendor::MySql) && target_server_uuid.is_none() {
        blockers.push("target MySQL server_uuid is missing or invalid".into());
    }
    if source_server_uuid.is_some() && source_server_uuid == target_server_uuid {
        blockers.push("source and target MySQL databases are on the same server instance".into());
    }
    if !source_mysql_topology.is_standalone_visible()
        || !target_mysql_topology.is_standalone_visible()
    {
        blockers.push(
            "legacy migration requires visible proof of MySQL instances with no detected replication or group topology"
                .into(),
        );
    }
    if target_key_count != 0 {
        blockers.push("target Redis database is not empty".into());
    }
    if !target_redis.redis_6_2_or_newer
        || !target_redis.getdel_available
        || !target_redis.evalsha_available
        || !target_redis.script_available
    {
        blockers.push("target Redis lacks the required version or commands".into());
    }
    if !valid_redis_run_id(&source_redis.source_default_run_id)
        || !valid_redis_run_id(&source_redis.source_cache_run_id)
        || !valid_redis_run_id(&target_redis.identity.run_id)
    {
        blockers.push("source and target Redis server identities could not be proven".into());
    } else if target_redis
        .identity
        .run_id
        .eq_ignore_ascii_case(&source_redis.source_default_run_id)
        || target_redis
            .identity
            .run_id
            .eq_ignore_ascii_case(&source_redis.source_cache_run_id)
    {
        blockers.push("target Redis is on the same server instance as source Redis".into());
    }
    if !source_redis_standalone(&source_redis) || !target_redis.identity.is_standalone() {
        blockers.push(
            "legacy migration currently supports only standalone, non-replicated Redis instances"
                .into(),
        );
    }
    if data.users_with_multiple_unfinished_orders != 0 {
        blockers.push("users with multiple unfinished orders require explicit resolution".into());
    }
    if data.paid_pending_orders != 0 {
        record_final_requirement(
            mode,
            &mut blockers,
            &mut pending_final_requirements,
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
        record_final_requirement(
            mode,
            &mut blockers,
            &mut pending_final_requirements,
            "legacy failed_jobs is not empty",
        );
    }
    if data.malformed_giftcard_redemptions != 0
        || data.giftcard_redemption_orphans != 0
        || data.business_invariant_violations != 0
        || data.relational_integrity_violations != 0
        || data.node_group_violations != 0
        || data.target_collation_unique_collisions != 0
    {
        blockers.push("legacy data does not satisfy native migration preflights".into());
    }
    if source_redis.upload_traffic_fields != 0 || source_redis.download_traffic_fields != 0 {
        record_final_requirement(
            mode,
            &mut blockers,
            &mut pending_final_requirements,
            "legacy Redis contains traffic that has not reached MySQL",
        );
    }
    if source_redis.malformed_traffic_values != 0 {
        blockers.push("legacy Redis traffic contains malformed values".into());
    }
    if source_redis.unexpected_traffic_key_candidates != 0 {
        blockers
            .push("legacy Redis traffic keys do not match the declared connection prefix".into());
    }
    if source_redis.traffic_reset_lock_keys != 0 {
        record_final_requirement(
            mode,
            &mut blockers,
            &mut pending_final_requirements,
            "legacy Redis still contains a traffic reset lock",
        );
    }
    if source_redis.queued_item_count != 0 {
        record_final_requirement(
            mode,
            &mut blockers,
            &mut pending_final_requirements,
            "legacy Redis queues are not drained",
        );
    }
    if source_redis.ambiguous_queue_key_candidates != 0 {
        blockers.push(
            "source default Redis contains queue-like keys outside the declared V2Board prefix"
                .into(),
        );
    }
    if source_redis.retryable_failed_job_items != 0 {
        record_final_requirement(
            mode,
            &mut blockers,
            &mut pending_final_requirements,
            "legacy Redis still contains retryable failed-job state",
        );
    }
    if source_redis.legacy_subscription_token_keys != 0 {
        record_final_requirement(
            mode,
            &mut blockers,
            &mut pending_final_requirements,
            "legacy Redis still contains issued OTP/TOTP subscription tokens",
        );
    }
    if source_redis.ambiguous_subscription_token_keys != 0 {
        blockers.push(
            "source cache Redis contains OTP/TOTP-like keys outside the declared V2Board prefix"
                .into(),
        );
    }
    if spec.source.legacy_show_subscribe_method != 0
        && spec.source.legacy_subscription_issuance_stopped_at_unix <= 0
    {
        record_final_requirement(
            mode,
            &mut blockers,
            &mut pending_final_requirements,
            "legacy subscription issuance has not been stopped with a recorded fence time",
        );
    } else if !source_redis.legacy_subscription_window_elapsed {
        record_final_requirement(
            mode,
            &mut blockers,
            &mut pending_final_requirements,
            "legacy ephemeral subscription URLs have not reached their declared expiry window",
        );
    }

    for requirement in [
        "source metadata visibility and read-only grants are not yet machine-proven",
        "target CREATE DATABASE/USER privileges, capacity, and trigger/binlog capabilities are not yet machine-proven",
        "target Redis command execution ACL is not yet machine-proven",
        "a fenced cross-datastore snapshot and physical topology separation beyond server process identities are not yet machine-bound",
        "legacy effective cache/queue/prefix/subscription facts are not yet verified from a non-executed config snapshot",
        "source Redis owned namespaces and durable unknown keys are not yet completely classified",
        "Stripe provider-side callback and reconciliation zero-state is not yet machine-proven",
        "operation journal, backup binding, data copy, target creation, verification, and cutover apply are not implemented",
    ] {
        implementation_blockers.push(requirement.to_string());
    }

    let mut warnings = Vec::new();
    if spec.source.transport_security == SourceTransportSecurity::TrustedMaintenanceNetwork {
        warnings.push(
            "source credentials and inspected data rely on the declared trusted maintenance network rather than verified TLS"
                .into(),
        );
    }
    if data.unfinished_orders != 0 {
        warnings
            .push("unfinished non-Stripe orders will be migrated and must remain payable".into());
    }
    if data.node_count != 0 {
        warnings.push(
            "node reporters must remain stopped until each node receives a scoped token and stable idempotency key"
                .into(),
        );
    }
    if source_redis.source_cache_key_count != 0 {
        warnings.push(
            "legacy cache Redis contains keys; OTP/TOTP candidates are counted separately, but remaining entries are not yet fully classified"
                .into(),
        );
    }
    if source_redis.source_default_key_count != 0 {
        warnings.push(
            "legacy default Redis contains keys; traffic/queue/lock candidates are counted separately, but sessions and remaining durable keys are not yet fully classified"
                .into(),
        );
    }
    if source_redis.queue_notify_item_count != 0 {
        warnings.push(
            "legacy Redis queue notify wake tokens remain; they are not durable jobs and are intentionally excluded from queued_item_count"
                .into(),
        );
    }
    let operator_attestations_complete = spec.attestations.source_writers_stopped
        && spec.attestations.source_workers_stopped
        && spec.attestations.node_reporters_stopped
        && spec.attestations.legacy_queues_drained
        && spec
            .attestations
            .backup_reference
            .as_deref()
            .is_some_and(|reference| !reference.trim().is_empty())
        && spec.attestations.restore_tested;
    if !operator_attestations_complete {
        record_final_requirement(
            mode,
            &mut blockers,
            &mut pending_final_requirements,
            "operator maintenance, drain, backup, and restore attestations are incomplete",
        );
    }

    let verdict = if !blockers.is_empty() || !implementation_blockers.is_empty() || !APPLY_AVAILABLE
    {
        PreflightVerdict::Blocked
    } else {
        match mode {
            InspectionMode::Online => PreflightVerdict::Compatible,
            InspectionMode::FencedFinal => PreflightVerdict::ReadyForConfirmation,
        }
    };
    let next_action = match verdict {
        PreflightVerdict::Compatible => NextAction::ConfirmEnterMaintenance,
        PreflightVerdict::ReadyForConfirmation => NextAction::ConfirmOperationIdAndReportSha,
        PreflightVerdict::Blocked => NextAction::ResolveBlockers,
    };
    let mut plan = ProvisionPlan {
        report_version: 2,
        scope: mode.scope(),
        apply_available: APPLY_AVAILABLE,
        operation_id: spec.operation_id.clone(),
        manifest_binding_hmac_sha256: spec.manifest_binding_hmac_sha256().to_string(),
        report_sha256: String::new(),
        verdict,
        next_action,
        operator_attestations_complete,
        source: DatabaseInspection {
            vendor: source_vendor,
            version: source_server.version,
            version_comment: source_server.version_comment,
            database_name: source_server.database_name.unwrap_or_default(),
            server_uuid: source_server_uuid.clone().unwrap_or(source_server_uuid_raw),
            server_uuid_valid: source_server_uuid.is_some(),
            replication_channel_count: source_mysql_topology.replication_channel_count,
            group_replication_member_count: source_mysql_topology.group_replication_member_count,
            registered_replica_count: source_mysql_topology.registered_replica_count,
            global_sql_mode: source_server.global_sql_mode,
            inspector_session_sql_mode: source_server.session_sql_mode,
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
        },
        target: TargetInspection {
            vendor: target_vendor,
            version: target_server.version,
            bootstrap_database_name: target_server.database_name.unwrap_or_default(),
            server_uuid: target_server_uuid.clone().unwrap_or(target_server_uuid_raw),
            server_uuid_valid: target_server_uuid.is_some(),
            replication_channel_count: target_mysql_topology.replication_channel_count,
            group_replication_member_count: target_mysql_topology.group_replication_member_count,
            registered_replica_count: target_mysql_topology.registered_replica_count,
            application_database_name: target_application_database_name,
            application_username: target_application_username,
            application_account_host: spec.target.application_account_host.clone(),
            database_absent: target_database_absent,
            application_account_absent: target_application_account_absent,
            desired_character_set: "utf8mb4",
            desired_collation: "utf8mb4_unicode_ci",
            empty_redis: target_key_count == 0,
            mysql_8_4_or_newer: target_8_4,
        },
        data,
        redis: RedisInspection {
            target_key_count,
            target_version: target_redis.version,
            target_run_id: target_redis.identity.run_id,
            target_role: target_redis.identity.role,
            target_connected_replicas: target_redis.identity.connected_replicas,
            target_cluster_enabled: target_redis.identity.cluster_enabled,
            target_redis_6_2_or_newer: target_redis.redis_6_2_or_newer,
            target_getdel_available: target_redis.getdel_available,
            target_evalsha_available: target_redis.evalsha_available,
            target_script_available: target_redis.script_available,
            ..source_redis
        },
        implementation_blockers,
        blockers,
        pending_final_requirements,
        warnings,
    };
    let bytes = serde_json::to_vec(&plan).expect("provision plan is serializable");
    plan.report_sha256 = hex::encode(Sha256::digest(bytes));
    Ok(plan)
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
        self.apply_available
            && matches!(
                self.verdict,
                PreflightVerdict::Compatible | PreflightVerdict::ReadyForConfirmation
            )
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

async fn target_database_absent(
    bootstrap_pool: &MySqlPool,
    application_database_name: &str,
) -> Result<bool, sqlx::Error> {
    let schema_count = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM information_schema.SCHEMATA WHERE SCHEMA_NAME = ?",
    )
    .bind(application_database_name)
    .fetch_one(bootstrap_pool)
    .await?;
    Ok(schema_count == 0)
}

async fn target_account_absent(
    bootstrap_pool: &MySqlPool,
    application_username: &str,
    application_account_host: &str,
) -> Result<bool, sqlx::Error> {
    let account_count =
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM mysql.user WHERE User = ? AND Host = ?")
            .bind(application_username)
            .bind(application_account_host)
            .fetch_one(bootstrap_pool)
            .await?;
    Ok(account_count == 0)
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
    let malformed_giftcard_redemptions = count_malformed_giftcard_redemptions(pool, &names).await?;
    let giftcard_redemption_orphans = count_giftcard_redemption_orphans(pool, &names).await?;
    let business_invariant_violations = count_business_invariant_violations(pool, &names).await?;
    let relational_integrity_violations =
        count_relational_integrity_violations(pool, &names).await?;
    let node_group_violations = count_node_group_violations(pool, &names).await?;
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
    })
}

async fn count_malformed_giftcard_redemptions(
    pool: &MySqlPool,
    tables: &BTreeSet<&str>,
) -> Result<i64, sqlx::Error> {
    if !tables.contains("v2_giftcard") {
        return Ok(0);
    }
    let rows = sqlx::query_as::<_, (i64, Option<String>)>(
        "SELECT id, used_user_ids FROM v2_giftcard ORDER BY id",
    )
    .fetch_all(pool)
    .await?;
    Ok(rows
        .iter()
        .filter(|(_, raw)| {
            raw.as_deref()
                .is_some_and(|raw| serde_json::from_str::<Vec<i64>>(raw).is_err())
        })
        .count() as i64)
}

async fn count_giftcard_redemption_orphans(
    pool: &MySqlPool,
    tables: &BTreeSet<&str>,
) -> Result<i64, sqlx::Error> {
    if !tables.contains("v2_giftcard") || !tables.contains("v2_user") {
        return Ok(0);
    }
    let known_users = sqlx::query_scalar::<_, i64>("SELECT id FROM v2_user")
        .fetch_all(pool)
        .await?
        .into_iter()
        .collect::<BTreeSet<_>>();
    let rows = sqlx::query_as::<_, (i64, Option<String>)>(
        "SELECT id, used_user_ids FROM v2_giftcard ORDER BY id",
    )
    .fetch_all(pool)
    .await?;
    let mut missing = BTreeSet::new();
    for (_, raw) in rows {
        let Some(users) = raw
            .as_deref()
            .and_then(|raw| serde_json::from_str::<Vec<i64>>(raw).ok())
        else {
            continue;
        };
        for user_id in users {
            if !known_users.contains(&user_id) {
                missing.insert(user_id);
            }
        }
    }
    Ok(missing.len() as i64)
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

async fn count_node_group_violations(
    pool: &MySqlPool,
    tables: &BTreeSet<&str>,
) -> Result<i64, sqlx::Error> {
    if !tables.contains("v2_server_group") {
        return Ok(0);
    }
    let known_groups = sqlx::query_scalar::<_, i64>("SELECT id FROM v2_server_group")
        .fetch_all(pool)
        .await?
        .into_iter()
        .collect::<BTreeSet<_>>();
    let mut violations = 0_i64;
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
        let query = format!("SELECT id, CAST(group_id AS CHAR) FROM `{table}` ORDER BY id");
        let rows = sqlx::query_as::<_, (i64, String)>(AssertSqlSafe(query))
            .fetch_all(pool)
            .await?;
        for (_, raw) in rows {
            let valid = serde_json::from_str::<Vec<serde_json::Value>>(&raw)
                .ok()
                .filter(|members| !members.is_empty())
                .is_some_and(|members| {
                    members.into_iter().all(|member| {
                        let id = canonical_node_group_member_id(&member);
                        id.is_some_and(|id| id > 0 && known_groups.contains(&id))
                    })
                });
            if !valid {
                violations = violations.saturating_add(1);
            }
        }
    }
    Ok(violations)
}

fn canonical_node_group_member_id(member: &serde_json::Value) -> Option<i64> {
    if let Some(id) = member.as_i64() {
        return (id > 0).then_some(id);
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
    value.parse::<i64>().ok()
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
) -> Result<RedisInspection, redis::RedisError> {
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
    Ok(RedisInspection {
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
        target_key_count: 0,
        target_version: String::new(),
        target_run_id: String::new(),
        target_role: String::new(),
        target_connected_replicas: None,
        target_cluster_enabled: None,
        target_redis_6_2_or_newer: false,
        target_getdel_available: false,
        target_evalsha_available: false,
        target_script_available: false,
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

impl RedisServerIdentity {
    fn is_standalone(&self) -> bool {
        valid_redis_run_id(&self.run_id)
            && self.role == "master"
            && self.connected_replicas == Some(0)
            && self.cluster_enabled == Some(false)
    }
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

fn source_redis_standalone(redis: &RedisInspection) -> bool {
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
    } else if text.contains("mysql") || text.contains("percona server") {
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
            database_vendor("10.11.8-MariaDB", "mariadb.org binary distribution"),
            DatabaseVendor::MariaDb
        ));
        assert!(matches!(
            database_vendor("8.4.4-compatible", "Unknown SQL proxy"),
            DatabaseVendor::Unknown
        ));
        assert!(version_at_least("8.4.4", 8, 4, 0));
        assert!(!version_at_least("8.0.36", 8, 4, 0));
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
    fn canonicalization_ignores_integer_display_width_and_utf8_alias() {
        assert_eq!(normalize_column_type("int(11) unsigned"), "int unsigned");
        assert_eq!(normalize_column_type("bigint(20)"), "bigint");
        assert_eq!(normalize_charset("utf8mb3_unicode_ci"), "utf8_unicode_ci");
    }

    #[test]
    fn node_group_members_match_the_native_sql_canonical_decimal_rule() {
        assert_eq!(
            canonical_node_group_member_id(&serde_json::json!(1)),
            Some(1)
        );
        assert_eq!(
            canonical_node_group_member_id(&serde_json::json!("42")),
            Some(42)
        );
        for invalid in [
            serde_json::json!(0),
            serde_json::json!(-1),
            serde_json::json!("0"),
            serde_json::json!("01"),
            serde_json::json!("+1"),
            serde_json::json!("1.0"),
        ] {
            assert_eq!(canonical_node_group_member_id(&invalid), None);
        }
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
        let pool = connect_mysql_with_config(
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
