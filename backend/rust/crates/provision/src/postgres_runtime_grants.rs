//! Frozen PostgreSQL runtime ACL policy for the native API and worker.
//!
//! The migration principal owns the schema and lifecycle state. Runtime
//! principals deliberately share the current business-table DML allowlist,
//! while operator configuration uses an asymmetric boundary: only the API can
//! publish a revision and advance the active pointer, and each role can update
//! only its own application acknowledgement. Lifecycle, copy-checkpoint, and
//! legacy-fold provenance stays invisible to both. New tables and sequences
//! fail closed until this policy is reviewed and re-applied by a lifecycle
//! operation.

use std::collections::BTreeSet;

use sqlx::{PgPool, Row};
use url::Url;

use crate::{ProvisionSpec, apply_journal::DurableTargetMutationPermit, manifest::ProvisionFlow};

pub(crate) const RUNTIME_READ_ONLY_TABLES: &[&str] = &[
    "_sqlx_migrations",
    "v2_system_installation",
    "v2_analytics_admission_policy",
];

pub(crate) const RUNTIME_ADMISSION_STATE_TABLES: &[&str] = &["v2_analytics_admission_state"];

pub(crate) const RUNTIME_OPERATOR_REVISION_TABLES: &[&str] = &["v2_operator_config_revision"];

pub(crate) const RUNTIME_API_OPERATOR_STATE_TABLES: &[&str] =
    &["v2_operator_config_state", "v2_operator_config_api_ack"];

pub(crate) const RUNTIME_WORKER_OPERATOR_STATE_TABLES: &[&str] = &["v2_operator_config_worker_ack"];

pub(crate) const RUNTIME_HIDDEN_TABLES: &[&str] = &[
    "v2_lifecycle_operation",
    "v2_lifecycle_event",
    "v2_lifecycle_activation_commit",
    "v2_legacy_copy_checkpoint",
    "v2_legacy_traffic_fold_item",
    "v2_legacy_traffic_fold",
];

// This is intentionally explicit. API/worker currently share DB helpers, so
// this freezes the business-table boundary without pretending the two runtime
// roles have already been split operation-by-operation. The protected sets
// above are the hard security boundary; future table additions fail closed.
pub(crate) const RUNTIME_MUTABLE_TABLES: &[&str] = &[
    "v2_server_group",
    "v2_plan",
    "v2_payment",
    "v2_coupon",
    "v2_user",
    "v2_order",
    "v2_commission_log",
    "v2_invite_code",
    "v2_giftcard",
    "v2_giftcard_redemption",
    "v2_payment_reconciliation",
    "v2_knowledge",
    "v2_notice",
    "v2_ticket",
    "v2_ticket_message",
    "v2_log",
    "v2_mail_log",
    "v2_mail_outbox_batch",
    "v2_mail_outbox",
    "v2_stat",
    "v2_stat_server",
    "v2_stat_user",
    "v2_analytics_delivery_batch",
    "v2_analytics_outbox",
    "v2_server_traffic_report",
    "v2_server_traffic_report_item",
    "v2_server_credential",
    "v2_server_route",
    "v2_server_shadowsocks",
    "v2_server_vmess",
    "v2_server_trojan",
    "v2_server_tuic",
    "v2_server_hysteria",
    "v2_server_vless",
    "v2_server_anytls",
    "v2_server_v2node",
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RuntimeAclSchemaState {
    Empty,
    FrozenBaseline,
    Drifted,
}

impl RuntimeAclSchemaState {
    pub(crate) fn parse(value: &str) -> Option<Self> {
        match value {
            "empty" => Some(Self::Empty),
            "frozen_baseline" => Some(Self::FrozenBaseline),
            "drifted" => Some(Self::Drifted),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RuntimeRoleNames {
    pub(crate) migration: String,
    pub(crate) api: String,
    pub(crate) worker: String,
}

#[derive(Debug, thiserror::Error)]
pub enum PostgresRuntimeGrantError {
    #[error("runtime grant operation is not bound to the lifecycle permit")]
    BindingMismatch,
    #[error("runtime grant policy does not apply to this provision flow")]
    UnsupportedFlow,
    #[error("runtime grant role URL is invalid")]
    InvalidRoleUrl,
    #[error("runtime grant policy failed PostgreSQL catalog verification")]
    VerificationFailed,
    #[error("runtime grant policy database operation failed: {0}")]
    Database(#[from] sqlx::Error),
}

/// Apply the frozen ACL only after the exact embedded migration lineage exists. The
/// lifecycle mutation permit is required so ordinary runtime startup cannot
/// grant itself additional database access.
pub async fn apply_frozen_runtime_grants(
    pool: &PgPool,
    spec: &ProvisionSpec,
    permit: &DurableTargetMutationPermit,
) -> Result<(), PostgresRuntimeGrantError> {
    if permit.operation_id() != spec.operation_id || permit.generation() == 0 {
        return Err(PostgresRuntimeGrantError::BindingMismatch);
    }
    let roles = runtime_roles(spec)?;
    let mut transaction = pool.begin().await?;
    sqlx::query("SET LOCAL synchronous_commit = 'on'")
        .execute(&mut *transaction)
        .await?;

    let current_user: String = sqlx::query_scalar("SELECT current_user")
        .fetch_one(&mut *transaction)
        .await?;
    if current_user != roles.migration {
        return Err(PostgresRuntimeGrantError::BindingMismatch);
    }

    let current_database: String = sqlx::query_scalar("SELECT current_database()")
        .fetch_one(&mut *transaction)
        .await?;
    let statements = runtime_grant_statements("public", &current_database, &roles);
    for statement in statements {
        sqlx::query(sqlx::AssertSqlSafe(statement))
            .execute(&mut *transaction)
            .await?;
    }

    let owned_sequences = sqlx::query(
        "SELECT sequence.relname AS sequence_name, owner_table.relname AS table_name \
         FROM pg_class AS sequence \
         JOIN pg_namespace AS sequence_namespace ON sequence_namespace.oid = sequence.relnamespace \
         JOIN pg_depend AS dependency ON dependency.objid = sequence.oid \
              AND dependency.classid = 'pg_class'::regclass \
              AND dependency.refclassid = 'pg_class'::regclass \
              AND dependency.deptype IN ('a', 'i') \
         JOIN pg_class AS owner_table ON owner_table.oid = dependency.refobjid \
         JOIN pg_namespace AS owner_namespace ON owner_namespace.oid = owner_table.relnamespace \
         WHERE sequence.relkind = 'S' \
           AND sequence_namespace.nspname = 'public' \
           AND owner_namespace.nspname = 'public' \
         ORDER BY sequence.relname",
    )
    .fetch_all(&mut *transaction)
    .await?;
    let mutable = RUNTIME_MUTABLE_TABLES
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    let operator_revision = RUNTIME_OPERATOR_REVISION_TABLES
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    let mut shared_sequence_names = Vec::with_capacity(owned_sequences.len());
    let mut api_sequence_names = Vec::new();
    for row in owned_sequences {
        let sequence_name: String = row.try_get("sequence_name")?;
        let table_name: String = row.try_get("table_name")?;
        if mutable.contains(table_name.as_str()) {
            shared_sequence_names.push(sequence_name);
        } else if operator_revision.contains(table_name.as_str()) {
            api_sequence_names.push(sequence_name);
        }
    }
    if !shared_sequence_names.is_empty() {
        let sequences = shared_sequence_names
            .iter()
            .map(|name| qualified_identifier("public", name))
            .collect::<Vec<_>>()
            .join(", ");
        let runtime_roles = format!(
            "{}, {}",
            quote_identifier(&roles.api),
            quote_identifier(&roles.worker)
        );
        let statement = format!("GRANT USAGE ON SEQUENCE {sequences} TO {runtime_roles}");
        sqlx::query(sqlx::AssertSqlSafe(statement))
            .execute(&mut *transaction)
            .await?;
    }
    if !api_sequence_names.is_empty() {
        let sequences = api_sequence_names
            .iter()
            .map(|name| qualified_identifier("public", name))
            .collect::<Vec<_>>()
            .join(", ");
        let statement = format!(
            "GRANT USAGE ON SEQUENCE {sequences} TO {}",
            quote_identifier(&roles.api)
        );
        sqlx::query(sqlx::AssertSqlSafe(statement))
            .execute(&mut *transaction)
            .await?;
    }

    let verification = runtime_acl_catalog_sql("public", &roles);
    let row = sqlx::query(sqlx::AssertSqlSafe(verification))
        .fetch_one(&mut *transaction)
        .await?;
    let state: String = row.try_get("schema_state")?;
    let verified = RuntimeAclSchemaState::parse(&state)
        == Some(RuntimeAclSchemaState::FrozenBaseline)
        && row.try_get::<bool, _>("table_acl_exact")?
        && row.try_get::<bool, _>("protected_acl_exact")?
        && row.try_get::<bool, _>("sequence_acl_exact")?
        && row.try_get::<bool, _>("default_acl_fail_closed")?
        && row.try_get::<bool, _>("runtime_boundary_acl_exact")?;
    if !verified {
        return Err(PostgresRuntimeGrantError::VerificationFailed);
    }
    transaction.commit().await?;
    Ok(())
}

pub(crate) fn runtime_roles(
    spec: &ProvisionSpec,
) -> Result<RuntimeRoleNames, PostgresRuntimeGrantError> {
    let (migration, api, worker) = match &spec.flow {
        ProvisionFlow::FreshInstall { target, .. }
        | ProvisionFlow::LegacyReferenceMigration { target, .. } => (
            &target.postgres.migration_database_url,
            &target.postgres.api_database_url,
            &target.postgres.worker_database_url,
        ),
        ProvisionFlow::NativeUpgrade { current, .. } => (
            &current.migration_database_url,
            &current.api_database_url,
            &current.worker_database_url,
        ),
    };
    let parse = |value: &str| {
        let url = Url::parse(value).map_err(|_| PostgresRuntimeGrantError::InvalidRoleUrl)?;
        let username = url.username();
        if username.is_empty()
            || !username
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
        {
            return Err(PostgresRuntimeGrantError::InvalidRoleUrl);
        }
        Ok(username.to_string())
    };
    Ok(RuntimeRoleNames {
        migration: parse(migration)?,
        api: parse(api)?,
        worker: parse(worker)?,
    })
}

fn runtime_grant_statements(schema: &str, database: &str, roles: &RuntimeRoleNames) -> Vec<String> {
    let schema = quote_identifier(schema);
    let database = quote_identifier(database);
    let migration = quote_identifier(&roles.migration);
    let api = quote_identifier(&roles.api);
    let worker = quote_identifier(&roles.worker);
    let runtime_roles = format!("{api}, {worker}");
    let read_only = qualified_table_list("public", RUNTIME_READ_ONLY_TABLES);
    let admission_state = qualified_table_list("public", RUNTIME_ADMISSION_STATE_TABLES);
    let operator_revision = qualified_table_list("public", RUNTIME_OPERATOR_REVISION_TABLES);
    let api_operator_state = qualified_table_list("public", RUNTIME_API_OPERATOR_STATE_TABLES);
    let worker_operator_state =
        qualified_table_list("public", RUNTIME_WORKER_OPERATOR_STATE_TABLES);
    let mutable = qualified_table_list("public", RUNTIME_MUTABLE_TABLES);
    vec![
        format!("REVOKE CONNECT, TEMPORARY ON DATABASE {database} FROM PUBLIC"),
        format!("REVOKE ALL PRIVILEGES ON SCHEMA {schema} FROM PUBLIC"),
        format!("REVOKE ALL PRIVILEGES ON ALL TABLES IN SCHEMA {schema} FROM {runtime_roles}"),
        format!("REVOKE ALL PRIVILEGES ON ALL SEQUENCES IN SCHEMA {schema} FROM {runtime_roles}"),
        format!(
            "ALTER DEFAULT PRIVILEGES FOR ROLE {migration} IN SCHEMA {schema} REVOKE ALL PRIVILEGES ON TABLES FROM {runtime_roles}"
        ),
        format!(
            "ALTER DEFAULT PRIVILEGES FOR ROLE {migration} IN SCHEMA {schema} REVOKE ALL PRIVILEGES ON SEQUENCES FROM {runtime_roles}"
        ),
        format!("REVOKE CONNECT, TEMPORARY ON DATABASE {database} FROM {runtime_roles}"),
        format!("GRANT CONNECT ON DATABASE {database} TO {runtime_roles}"),
        format!("GRANT USAGE ON SCHEMA {schema} TO {runtime_roles}"),
        format!("REVOKE CREATE ON SCHEMA {schema} FROM {runtime_roles}"),
        format!("GRANT SELECT ON TABLE {read_only} TO {runtime_roles}"),
        format!("GRANT SELECT, UPDATE ON TABLE {admission_state} TO {runtime_roles}"),
        format!(
            "GRANT SELECT ON TABLE {operator_revision}, {api_operator_state}, {worker_operator_state} TO {runtime_roles}"
        ),
        format!("GRANT INSERT ON TABLE {operator_revision} TO {api}"),
        format!("GRANT INSERT, UPDATE ON TABLE {api_operator_state} TO {api}"),
        format!("GRANT INSERT, UPDATE ON TABLE {worker_operator_state} TO {worker}"),
        format!("GRANT SELECT, INSERT, UPDATE, DELETE ON TABLE {mutable} TO {runtime_roles}"),
    ]
}

/// Returns one catalog row with the runtime ACL state. It is used by both the
/// mutating lifecycle helper and the read-only bare-metal readiness proof.
pub(crate) fn runtime_acl_catalog_sql(schema: &str, roles: &RuntimeRoleNames) -> String {
    let expected = RUNTIME_READ_ONLY_TABLES
        .iter()
        .map(|table| (*table, "read_only"))
        .chain(
            RUNTIME_ADMISSION_STATE_TABLES
                .iter()
                .map(|table| (*table, "admission_state")),
        )
        .chain(
            RUNTIME_OPERATOR_REVISION_TABLES
                .iter()
                .map(|table| (*table, "operator_revision")),
        )
        .chain(
            RUNTIME_API_OPERATOR_STATE_TABLES
                .iter()
                .map(|table| (*table, "api_operator_state")),
        )
        .chain(
            RUNTIME_WORKER_OPERATOR_STATE_TABLES
                .iter()
                .map(|table| (*table, "worker_operator_state")),
        )
        .chain(RUNTIME_HIDDEN_TABLES.iter().map(|table| (*table, "hidden")))
        .chain(
            RUNTIME_MUTABLE_TABLES
                .iter()
                .map(|table| (*table, "mutable")),
        )
        .map(|(table, policy)| format!("({}, {})", quote_literal(table), quote_literal(policy)))
        .collect::<Vec<_>>()
        .join(", ");
    let migration_literal = quote_literal(&roles.migration);
    let api_literal = quote_literal(&roles.api);
    let worker_literal = quote_literal(&roles.worker);
    let runtime_roles = [&roles.api, &roles.worker]
        .into_iter()
        .map(|role| format!("({})", quote_literal(role)))
        .collect::<Vec<_>>()
        .join(", ");
    let schema_literal = quote_literal(schema);
    format!(
        "WITH expected(table_name, policy) AS (VALUES {expected}), \
         runtime_role(role_name) AS (VALUES {runtime_roles}), \
         actual AS ( \
           SELECT table_name FROM information_schema.tables \
           WHERE table_schema = {schema_literal} AND table_type = 'BASE TABLE' \
         ), \
         expected_table AS ( \
           SELECT expected.*, to_regclass(format('%I.%I', {schema_literal}, expected.table_name)) AS table_oid \
           FROM expected \
         ), \
         shape AS ( \
           SELECT CASE \
             WHEN NOT EXISTS (SELECT 1 FROM actual) THEN 'empty' \
             WHEN NOT EXISTS ((SELECT table_name FROM actual) EXCEPT (SELECT table_name FROM expected)) \
              AND NOT EXISTS ((SELECT table_name FROM expected) EXCEPT (SELECT table_name FROM actual)) \
             THEN 'frozen_baseline' ELSE 'drifted' END AS schema_state \
         ), \
         table_acl AS ( \
           SELECT COALESCE(bool_and(CASE expected.policy \
             WHEN 'read_only' THEN \
               has_table_privilege(runtime_role.role_name, expected.table_oid, 'SELECT') \
               AND NOT has_table_privilege(runtime_role.role_name, expected.table_oid, 'INSERT') \
               AND NOT has_table_privilege(runtime_role.role_name, expected.table_oid, 'UPDATE') \
               AND NOT has_table_privilege(runtime_role.role_name, expected.table_oid, 'DELETE') \
               AND NOT has_table_privilege(runtime_role.role_name, expected.table_oid, 'TRUNCATE') \
               AND NOT has_table_privilege(runtime_role.role_name, expected.table_oid, 'REFERENCES') \
               AND NOT has_table_privilege(runtime_role.role_name, expected.table_oid, 'TRIGGER') \
             WHEN 'hidden' THEN \
               NOT has_table_privilege(runtime_role.role_name, expected.table_oid, 'SELECT') \
               AND NOT has_table_privilege(runtime_role.role_name, expected.table_oid, 'INSERT') \
               AND NOT has_table_privilege(runtime_role.role_name, expected.table_oid, 'UPDATE') \
               AND NOT has_table_privilege(runtime_role.role_name, expected.table_oid, 'DELETE') \
               AND NOT has_table_privilege(runtime_role.role_name, expected.table_oid, 'TRUNCATE') \
               AND NOT has_table_privilege(runtime_role.role_name, expected.table_oid, 'REFERENCES') \
               AND NOT has_table_privilege(runtime_role.role_name, expected.table_oid, 'TRIGGER') \
             WHEN 'admission_state' THEN \
               has_table_privilege(runtime_role.role_name, expected.table_oid, 'SELECT') \
               AND NOT has_table_privilege(runtime_role.role_name, expected.table_oid, 'INSERT') \
               AND has_table_privilege(runtime_role.role_name, expected.table_oid, 'UPDATE') \
               AND NOT has_table_privilege(runtime_role.role_name, expected.table_oid, 'DELETE') \
               AND NOT has_table_privilege(runtime_role.role_name, expected.table_oid, 'TRUNCATE') \
               AND NOT has_table_privilege(runtime_role.role_name, expected.table_oid, 'REFERENCES') \
               AND NOT has_table_privilege(runtime_role.role_name, expected.table_oid, 'TRIGGER') \
             WHEN 'operator_revision' THEN \
               has_table_privilege(runtime_role.role_name, expected.table_oid, 'SELECT') \
               AND (has_table_privilege(runtime_role.role_name, expected.table_oid, 'INSERT') \
                    = (runtime_role.role_name = {api_literal})) \
               AND NOT has_table_privilege(runtime_role.role_name, expected.table_oid, 'UPDATE') \
               AND NOT has_table_privilege(runtime_role.role_name, expected.table_oid, 'DELETE') \
               AND NOT has_table_privilege(runtime_role.role_name, expected.table_oid, 'TRUNCATE') \
               AND NOT has_table_privilege(runtime_role.role_name, expected.table_oid, 'REFERENCES') \
               AND NOT has_table_privilege(runtime_role.role_name, expected.table_oid, 'TRIGGER') \
             WHEN 'api_operator_state' THEN \
               has_table_privilege(runtime_role.role_name, expected.table_oid, 'SELECT') \
               AND (has_table_privilege(runtime_role.role_name, expected.table_oid, 'INSERT') \
                    = (runtime_role.role_name = {api_literal})) \
               AND (has_table_privilege(runtime_role.role_name, expected.table_oid, 'UPDATE') \
                    = (runtime_role.role_name = {api_literal})) \
               AND NOT has_table_privilege(runtime_role.role_name, expected.table_oid, 'DELETE') \
               AND NOT has_table_privilege(runtime_role.role_name, expected.table_oid, 'TRUNCATE') \
               AND NOT has_table_privilege(runtime_role.role_name, expected.table_oid, 'REFERENCES') \
               AND NOT has_table_privilege(runtime_role.role_name, expected.table_oid, 'TRIGGER') \
             WHEN 'worker_operator_state' THEN \
               has_table_privilege(runtime_role.role_name, expected.table_oid, 'SELECT') \
               AND (has_table_privilege(runtime_role.role_name, expected.table_oid, 'INSERT') \
                    = (runtime_role.role_name = {worker_literal})) \
               AND (has_table_privilege(runtime_role.role_name, expected.table_oid, 'UPDATE') \
                    = (runtime_role.role_name = {worker_literal})) \
               AND NOT has_table_privilege(runtime_role.role_name, expected.table_oid, 'DELETE') \
               AND NOT has_table_privilege(runtime_role.role_name, expected.table_oid, 'TRUNCATE') \
               AND NOT has_table_privilege(runtime_role.role_name, expected.table_oid, 'REFERENCES') \
               AND NOT has_table_privilege(runtime_role.role_name, expected.table_oid, 'TRIGGER') \
             WHEN 'mutable' THEN \
               has_table_privilege(runtime_role.role_name, expected.table_oid, 'SELECT') \
               AND has_table_privilege(runtime_role.role_name, expected.table_oid, 'INSERT') \
               AND has_table_privilege(runtime_role.role_name, expected.table_oid, 'UPDATE') \
               AND has_table_privilege(runtime_role.role_name, expected.table_oid, 'DELETE') \
               AND NOT has_table_privilege(runtime_role.role_name, expected.table_oid, 'TRUNCATE') \
               AND NOT has_table_privilege(runtime_role.role_name, expected.table_oid, 'REFERENCES') \
               AND NOT has_table_privilege(runtime_role.role_name, expected.table_oid, 'TRIGGER') \
             ELSE FALSE END), FALSE) AS exact, \
             COALESCE(bool_and(CASE WHEN expected.policy = 'hidden' THEN \
               NOT has_table_privilege(runtime_role.role_name, expected.table_oid, 'SELECT') \
               AND NOT has_table_privilege(runtime_role.role_name, expected.table_oid, 'INSERT') \
               AND NOT has_table_privilege(runtime_role.role_name, expected.table_oid, 'UPDATE') \
               AND NOT has_table_privilege(runtime_role.role_name, expected.table_oid, 'DELETE') \
               AND NOT has_table_privilege(runtime_role.role_name, expected.table_oid, 'TRUNCATE') \
               AND NOT has_table_privilege(runtime_role.role_name, expected.table_oid, 'REFERENCES') \
               AND NOT has_table_privilege(runtime_role.role_name, expected.table_oid, 'TRIGGER') \
               ELSE TRUE END), FALSE) AS protected_exact \
           FROM expected_table AS expected CROSS JOIN runtime_role \
           WHERE expected.table_oid IS NOT NULL \
         ), \
         column_acl AS ( \
           SELECT NOT EXISTS ( \
             SELECT 1 \
             FROM expected_table AS expected \
             JOIN pg_attribute AS attribute ON attribute.attrelid = expected.table_oid \
               AND attribute.attnum > 0 AND NOT attribute.attisdropped \
             CROSS JOIN LATERAL aclexplode(attribute.attacl) AS acl \
             LEFT JOIN pg_roles AS grantee ON grantee.oid = acl.grantee \
             WHERE acl.grantee = 0 \
                OR grantee.rolname IN (SELECT role_name FROM runtime_role) \
           ) AS exact \
         ), \
         sequence_acl AS ( \
           SELECT COALESCE(bool_and( \
             CASE WHEN expected.policy = 'mutable' \
                    OR (expected.policy = 'operator_revision' \
                        AND runtime_role.role_name = {api_literal}) THEN \
               has_sequence_privilege(runtime_role.role_name, sequence.oid, 'USAGE') \
               AND NOT has_sequence_privilege(runtime_role.role_name, sequence.oid, 'SELECT') \
               AND NOT has_sequence_privilege(runtime_role.role_name, sequence.oid, 'UPDATE') \
             ELSE NOT has_sequence_privilege(runtime_role.role_name, sequence.oid, 'USAGE') \
               AND NOT has_sequence_privilege(runtime_role.role_name, sequence.oid, 'SELECT') \
               AND NOT has_sequence_privilege(runtime_role.role_name, sequence.oid, 'UPDATE') END \
           ), TRUE) AS exact \
           FROM pg_class AS sequence \
           JOIN pg_namespace AS sequence_namespace ON sequence_namespace.oid = sequence.relnamespace \
           LEFT JOIN pg_depend AS dependency ON dependency.objid = sequence.oid \
             AND dependency.classid = 'pg_class'::regclass \
             AND dependency.refclassid = 'pg_class'::regclass \
             AND dependency.deptype IN ('a', 'i') \
           LEFT JOIN pg_class AS owner_table ON owner_table.oid = dependency.refobjid \
           LEFT JOIN pg_namespace AS owner_namespace ON owner_namespace.oid = owner_table.relnamespace \
           LEFT JOIN expected ON expected.table_name = owner_table.relname \
             AND owner_namespace.nspname = {schema_literal} \
           CROSS JOIN runtime_role \
           WHERE sequence.relkind = 'S' AND sequence_namespace.nspname = {schema_literal} \
         ), \
         default_acl AS ( \
           SELECT NOT EXISTS ( \
             SELECT 1 FROM pg_default_acl AS defaults \
             CROSS JOIN LATERAL aclexplode(defaults.defaclacl) AS acl \
             LEFT JOIN pg_roles AS grantee ON grantee.oid = acl.grantee \
             WHERE defaults.defaclrole = (SELECT oid FROM pg_roles WHERE rolname = {migration_literal}) \
               AND defaults.defaclnamespace = (SELECT oid FROM pg_namespace WHERE nspname = {schema_literal}) \
               AND defaults.defaclobjtype IN ('r', 'S') \
               AND (acl.grantee = 0 OR grantee.rolname IN (SELECT role_name FROM runtime_role)) \
           ) AS fail_closed \
         ), \
         runtime_boundary AS ( \
           SELECT COALESCE(bool_and( \
             has_database_privilege(runtime_role.role_name, current_database(), 'CONNECT') \
             AND NOT has_database_privilege(runtime_role.role_name, current_database(), 'TEMPORARY') \
             AND has_schema_privilege(runtime_role.role_name, {schema_literal}, 'USAGE') \
             AND NOT has_schema_privilege(runtime_role.role_name, {schema_literal}, 'CREATE') \
           ), FALSE) \
           AND NOT EXISTS ( \
             SELECT 1 FROM pg_database AS database \
             CROSS JOIN LATERAL aclexplode(COALESCE(database.datacl, acldefault('d', database.datdba))) AS acl \
             WHERE database.datname = current_database() AND acl.grantee = 0 \
               AND acl.privilege_type IN ('CONNECT', 'TEMPORARY') \
           ) \
           AND NOT EXISTS ( \
             SELECT 1 FROM pg_namespace AS namespace \
             CROSS JOIN LATERAL aclexplode(COALESCE(namespace.nspacl, acldefault('n', namespace.nspowner))) AS acl \
             WHERE namespace.nspname = {schema_literal} AND acl.grantee = 0 \
               AND acl.privilege_type IN ('USAGE', 'CREATE') \
           ) \
           AND NOT EXISTS ( \
             SELECT 1 FROM pg_auth_members AS membership \
             WHERE membership.member IN ( \
                     SELECT oid FROM pg_roles \
                     WHERE rolname IN (SELECT role_name FROM runtime_role) \
                   ) \
                OR membership.roleid IN ( \
                     SELECT oid FROM pg_roles \
                     WHERE rolname IN (SELECT role_name FROM runtime_role) \
                   ) \
           ) AS exact \
           FROM runtime_role \
         ) \
         SELECT shape.schema_state, \
           (shape.schema_state = 'empty' OR (table_acl.exact AND column_acl.exact)) AS table_acl_exact, \
           (shape.schema_state = 'empty' OR table_acl.protected_exact) AS protected_acl_exact, \
           sequence_acl.exact AS sequence_acl_exact, \
           default_acl.fail_closed AS default_acl_fail_closed, \
           runtime_boundary.exact AS runtime_boundary_acl_exact \
         FROM shape CROSS JOIN table_acl CROSS JOIN column_acl CROSS JOIN sequence_acl CROSS JOIN default_acl \
           CROSS JOIN runtime_boundary"
    )
}

fn qualified_table_list(schema: &str, tables: &[&str]) -> String {
    tables
        .iter()
        .map(|table| qualified_identifier(schema, table))
        .collect::<Vec<_>>()
        .join(", ")
}

fn qualified_identifier(schema: &str, name: &str) -> String {
    format!("{}.{}", quote_identifier(schema), quote_identifier(name))
}

fn quote_identifier(value: &str) -> String {
    format!("\"{}\"", value.replace('"', "\"\""))
}

fn quote_literal(value: &str) -> String {
    format!("'{}'", value.replace('\'', "''"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn operator_revision_insert_sql(
        revision_id: &str,
        format_version: i16,
        created_by: &str,
    ) -> String {
        format!(
            "INSERT INTO public.v2_operator_config_revision (\
             revision_id, format_version, installation_id, public_config, secret_nonce, \
             secret_ciphertext, secret_tag, config_hmac_sha256, created_by, created_at) \
             SELECT {}::uuid, {format_version}, installation_id, '{{}}'::jsonb, \
             decode(repeat('00', 12), 'hex'), decode('01', 'hex'), \
             decode(repeat('00', 16), 'hex'), repeat('a', 64), {}, 1000 \
             FROM public.v2_system_installation WHERE singleton = 1 \
             RETURNING revision",
            quote_literal(revision_id),
            quote_literal(created_by),
        )
    }

    async fn insert_operator_revision(
        transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        revision_id: &str,
        created_by: &str,
    ) -> i64 {
        let statement = operator_revision_insert_sql(revision_id, 1, created_by);
        sqlx::query_scalar::<_, i64>(sqlx::AssertSqlSafe(statement))
            .fetch_one(&mut **transaction)
            .await
            .expect("insert structurally valid operator revision")
    }

    async fn assert_sql_rejected(
        transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        statement: &str,
        expected_sqlstate: &str,
        expected_marker: Option<&str>,
    ) {
        const SAVEPOINT: &str = "operator_config_expected_rejection";
        sqlx::query(sqlx::AssertSqlSafe(format!("SAVEPOINT {SAVEPOINT}")))
            .execute(&mut **transaction)
            .await
            .expect("create expected-rejection savepoint");
        let result = sqlx::query(sqlx::AssertSqlSafe(statement))
            .execute(&mut **transaction)
            .await;
        let error = match result {
            Ok(_) => {
                sqlx::query(sqlx::AssertSqlSafe(format!(
                    "ROLLBACK TO SAVEPOINT {SAVEPOINT}"
                )))
                .execute(&mut **transaction)
                .await
                .expect("roll back unexpectedly accepted statement");
                sqlx::query(sqlx::AssertSqlSafe(format!(
                    "RELEASE SAVEPOINT {SAVEPOINT}"
                )))
                .execute(&mut **transaction)
                .await
                .expect("release unexpectedly accepted statement savepoint");
                panic!("statement unexpectedly succeeded: {statement}");
            }
            Err(error) => error,
        };
        let database_error = error
            .as_database_error()
            .expect("expected PostgreSQL to reject the statement");
        let sqlstate = database_error.code().map(|code| code.into_owned());
        let diagnostic = format!(
            "{} {}",
            database_error.constraint().unwrap_or_default(),
            database_error.message()
        );
        sqlx::query(sqlx::AssertSqlSafe(format!(
            "ROLLBACK TO SAVEPOINT {SAVEPOINT}"
        )))
        .execute(&mut **transaction)
        .await
        .expect("roll back expected rejection");
        sqlx::query(sqlx::AssertSqlSafe(format!(
            "RELEASE SAVEPOINT {SAVEPOINT}"
        )))
        .execute(&mut **transaction)
        .await
        .expect("release expected-rejection savepoint");
        assert_eq!(
            sqlstate.as_deref(),
            Some(expected_sqlstate),
            "unexpected SQLSTATE for {statement}: {diagnostic}"
        );
        if let Some(expected_marker) = expected_marker {
            assert!(
                diagnostic.contains(expected_marker),
                "expected PostgreSQL diagnostic to contain {expected_marker:?}, got {diagnostic:?}"
            );
        }
    }

    #[test]
    fn frozen_acl_tables_exactly_cover_the_embedded_migration_lineage() {
        let migrations = [
            include_str!("../../../migrations-postgres/0001_initial.sql"),
            include_str!(
                "../../../migrations-postgres/0002_legacy_lifecycle_and_analytics_admission.sql"
            ),
            include_str!("../../../migrations-postgres/0003_operator_config_authority.sql"),
        ];
        let mut actual = migrations
            .iter()
            .flat_map(|migration| migration.lines())
            .filter_map(|line| line.strip_prefix("CREATE TABLE "))
            .filter_map(|line| line.split_whitespace().next())
            .map(ToString::to_string)
            .collect::<BTreeSet<_>>();
        actual.insert("_sqlx_migrations".to_string());

        let policy = RUNTIME_READ_ONLY_TABLES
            .iter()
            .chain(RUNTIME_ADMISSION_STATE_TABLES)
            .chain(RUNTIME_OPERATOR_REVISION_TABLES)
            .chain(RUNTIME_API_OPERATOR_STATE_TABLES)
            .chain(RUNTIME_WORKER_OPERATOR_STATE_TABLES)
            .chain(RUNTIME_HIDDEN_TABLES)
            .chain(RUNTIME_MUTABLE_TABLES)
            .map(|table| (*table).to_string())
            .collect::<BTreeSet<_>>();
        assert_eq!(policy, actual);
        assert_eq!(
            policy.len(),
            RUNTIME_READ_ONLY_TABLES.len()
                + RUNTIME_ADMISSION_STATE_TABLES.len()
                + RUNTIME_OPERATOR_REVISION_TABLES.len()
                + RUNTIME_API_OPERATOR_STATE_TABLES.len()
                + RUNTIME_WORKER_OPERATOR_STATE_TABLES.len()
                + RUNTIME_HIDDEN_TABLES.len()
                + RUNTIME_MUTABLE_TABLES.len(),
            "ACL policy sets must not overlap"
        );
    }

    #[test]
    fn catalog_verifier_checks_hidden_tables_defaults_and_sequences() {
        let roles = RuntimeRoleNames {
            migration: "v2_migration".to_string(),
            api: "v2_api".to_string(),
            worker: "v2_worker".to_string(),
        };
        let sql = runtime_acl_catalog_sql("public", &roles);
        assert!(sql.contains("v2_lifecycle_operation"));
        assert!(sql.contains("v2_legacy_traffic_fold_item"));
        assert!(sql.contains("v2_analytics_admission_state"));
        assert!(sql.contains("WHEN 'admission_state'"));
        assert!(sql.contains("WHEN 'operator_revision'"));
        assert!(sql.contains("WHEN 'api_operator_state'"));
        assert!(sql.contains("WHEN 'worker_operator_state'"));
        assert!(sql.contains("aclexplode(defaults.defaclacl)"));
        assert!(sql.contains("pg_attribute"));
        assert!(sql.contains("aclexplode(attribute.attacl)"));
        assert!(sql.contains("pg_auth_members"));
        assert!(sql.contains("has_sequence_privilege"));
        assert!(sql.contains("'v2_migration'"));
        assert!(sql.contains("runtime_boundary_acl_exact"));
        assert!(sql.contains("acl.grantee = 0"));
    }

    #[test]
    fn generated_grants_are_fail_closed_and_never_grant_hidden_tables() {
        let roles = RuntimeRoleNames {
            migration: "v2_migration".to_string(),
            api: "v2_api".to_string(),
            worker: "v2_worker".to_string(),
        };
        let sql = runtime_grant_statements("public", "v2board", &roles).join(";\n");
        assert!(sql.contains("REVOKE ALL PRIVILEGES ON ALL TABLES"));
        assert!(sql.contains("ALTER DEFAULT PRIVILEGES"));
        assert!(sql.contains("REVOKE CONNECT, TEMPORARY"));
        assert!(sql.contains("FROM PUBLIC"));
        assert!(sql.contains("REVOKE ALL PRIVILEGES ON SCHEMA"));
        assert!(
            sql.contains(
                "GRANT SELECT, UPDATE ON TABLE \"public\".\"v2_analytics_admission_state\""
            )
        );
        assert!(sql.contains(
            "GRANT INSERT ON TABLE \"public\".\"v2_operator_config_revision\" TO \"v2_api\""
        ));
        assert!(sql.contains(
            "GRANT INSERT, UPDATE ON TABLE \"public\".\"v2_operator_config_state\", \"public\".\"v2_operator_config_api_ack\" TO \"v2_api\""
        ));
        assert!(sql.contains(
            "GRANT INSERT, UPDATE ON TABLE \"public\".\"v2_operator_config_worker_ack\" TO \"v2_worker\""
        ));
        assert!(!sql.contains(
            "GRANT INSERT, UPDATE ON TABLE \"public\".\"v2_operator_config_worker_ack\" TO \"v2_api\""
        ));
        assert!(!sql.contains(
            "GRANT SELECT, INSERT, UPDATE, DELETE ON TABLE \"public\".\"v2_analytics_admission_state\""
        ));
        for table in RUNTIME_HIDDEN_TABLES {
            assert!(!sql.contains(&format!("\"{table}\"")));
        }
    }

    #[tokio::test]
    #[ignore = "requires V2BOARD_RUNTIME_ACL_TEST_POSTGRES_URL for a disposable PostgreSQL 18 baseline"]
    async fn postgres_18_catalog_proves_exact_acl_and_detects_protected_grant() {
        let database_url = std::env::var("V2BOARD_RUNTIME_ACL_TEST_POSTGRES_URL")
            .expect("V2BOARD_RUNTIME_ACL_TEST_POSTGRES_URL must name a disposable baseline");
        let pool = PgPool::connect(&database_url)
            .await
            .expect("connect ACL fixture");
        let version: i32 =
            sqlx::query_scalar("SELECT current_setting('server_version_num')::INTEGER")
                .fetch_one(&pool)
                .await
                .expect("PostgreSQL version");
        assert_eq!(version / 10_000, 18);

        let suffix = std::process::id();
        let current_user: String = sqlx::query_scalar("SELECT current_user")
            .fetch_one(&pool)
            .await
            .expect("migration role");
        let database: String = sqlx::query_scalar("SELECT current_database()")
            .fetch_one(&pool)
            .await
            .expect("database name");
        let roles = RuntimeRoleNames {
            migration: current_user,
            api: format!("v2_acl_api_{suffix}"),
            worker: format!("v2_acl_worker_{suffix}"),
        };
        let mut transaction = pool.begin().await.expect("ACL transaction");
        for role in [&roles.api, &roles.worker] {
            sqlx::query(sqlx::AssertSqlSafe(format!(
                "CREATE ROLE {} NOLOGIN NOSUPERUSER NOCREATEDB NOCREATEROLE NOINHERIT",
                quote_identifier(role)
            )))
            .execute(&mut *transaction)
            .await
            .expect("create test role");
        }
        for statement in runtime_grant_statements("public", &database, &roles) {
            sqlx::query(sqlx::AssertSqlSafe(statement))
                .execute(&mut *transaction)
                .await
                .expect("apply runtime ACL statement");
        }
        let owned_sequences = sqlx::query_as::<_, (String, String)>(
            "SELECT sequence.relname, owner_table.relname \
             FROM pg_class AS sequence \
             JOIN pg_namespace AS sequence_namespace ON sequence_namespace.oid = sequence.relnamespace \
             JOIN pg_depend AS dependency ON dependency.objid = sequence.oid \
                  AND dependency.classid = 'pg_class'::regclass \
                  AND dependency.refclassid = 'pg_class'::regclass \
                  AND dependency.deptype IN ('a', 'i') \
             JOIN pg_class AS owner_table ON owner_table.oid = dependency.refobjid \
             JOIN pg_namespace AS owner_namespace ON owner_namespace.oid = owner_table.relnamespace \
             WHERE sequence.relkind = 'S' \
               AND sequence_namespace.nspname = 'public' \
               AND owner_namespace.nspname = 'public'",
        )
        .fetch_all(&mut *transaction)
        .await
        .expect("inspect identity sequences");
        let runtime_roles = format!(
            "{}, {}",
            quote_identifier(&roles.api),
            quote_identifier(&roles.worker)
        );
        for (sequence, table) in owned_sequences {
            let grantee = if RUNTIME_MUTABLE_TABLES.contains(&table.as_str()) {
                runtime_roles.clone()
            } else if RUNTIME_OPERATOR_REVISION_TABLES.contains(&table.as_str()) {
                quote_identifier(&roles.api)
            } else {
                continue;
            };
            sqlx::query(sqlx::AssertSqlSafe(format!(
                "GRANT USAGE ON SEQUENCE {} TO {grantee}",
                qualified_identifier("public", &sequence),
            )))
            .execute(&mut *transaction)
            .await
            .expect("grant exact identity sequence usage");
        }

        let verification = runtime_acl_catalog_sql("public", &roles);
        let row = sqlx::query(sqlx::AssertSqlSafe(verification.as_str()))
            .fetch_one(&mut *transaction)
            .await
            .expect("execute catalog verifier");
        assert_eq!(row.get::<String, _>("schema_state"), "frozen_baseline");
        for column in [
            "table_acl_exact",
            "protected_acl_exact",
            "sequence_acl_exact",
            "default_acl_fail_closed",
            "runtime_boundary_acl_exact",
        ] {
            assert!(row.get::<bool, _>(column), "catalog check {column}");
        }

        for role in [&roles.api, &roles.worker] {
            let privileges = sqlx::query_as::<_, (bool, bool, bool, bool, bool, bool)>(
                "SELECT \
                     has_table_privilege($1, 'public.v2_analytics_admission_policy', 'SELECT'), \
                     has_table_privilege($1, 'public.v2_analytics_admission_policy', 'UPDATE'), \
                     has_table_privilege($1, 'public.v2_analytics_admission_state', 'SELECT'), \
                     has_table_privilege($1, 'public.v2_analytics_admission_state', 'UPDATE'), \
                     has_table_privilege($1, 'public.v2_analytics_admission_state', 'INSERT'), \
                     has_table_privilege($1, 'public.v2_analytics_admission_state', 'DELETE')",
            )
            .bind(role)
            .fetch_one(&mut *transaction)
            .await
            .expect("read admission privileges");
            assert_eq!(privileges, (true, false, true, true, false, false));
        }

        sqlx::query(sqlx::AssertSqlSafe(format!(
            "GRANT INSERT ON TABLE public.v2_analytics_admission_state TO {}",
            quote_identifier(&roles.api)
        )))
        .execute(&mut *transaction)
        .await
        .expect("inject forbidden admission-state grant");
        let row = sqlx::query(sqlx::AssertSqlSafe(verification.as_str()))
            .fetch_one(&mut *transaction)
            .await
            .expect("detect forbidden admission-state grant");
        assert!(!row.get::<bool, _>("table_acl_exact"));
        sqlx::query(sqlx::AssertSqlSafe(format!(
            "REVOKE INSERT ON TABLE public.v2_analytics_admission_state FROM {}",
            quote_identifier(&roles.api)
        )))
        .execute(&mut *transaction)
        .await
        .expect("remove forbidden admission-state grant");

        for (grant, revoke) in [
            (
                format!(
                    "GRANT UPDATE (active_revision) ON TABLE public.v2_operator_config_state TO {}",
                    quote_identifier(&roles.worker)
                ),
                format!(
                    "REVOKE UPDATE (active_revision) ON TABLE public.v2_operator_config_state FROM {}",
                    quote_identifier(&roles.worker)
                ),
            ),
            (
                format!(
                    "GRANT UPDATE (revision_id) ON TABLE public.v2_operator_config_revision TO {}",
                    quote_identifier(&roles.api)
                ),
                format!(
                    "REVOKE UPDATE (revision_id) ON TABLE public.v2_operator_config_revision FROM {}",
                    quote_identifier(&roles.api)
                ),
            ),
            (
                "GRANT UPDATE (active_revision) ON TABLE public.v2_operator_config_state TO PUBLIC"
                    .to_string(),
                "REVOKE UPDATE (active_revision) ON TABLE public.v2_operator_config_state FROM PUBLIC"
                    .to_string(),
            ),
        ] {
            sqlx::query(sqlx::AssertSqlSafe(grant))
                .execute(&mut *transaction)
                .await
                .expect("inject forbidden column grant");
            let row = sqlx::query(sqlx::AssertSqlSafe(verification.as_str()))
                .fetch_one(&mut *transaction)
                .await
                .expect("detect forbidden column grant");
            assert!(!row.get::<bool, _>("table_acl_exact"));
            sqlx::query(sqlx::AssertSqlSafe(revoke))
                .execute(&mut *transaction)
                .await
                .expect("remove forbidden column grant");
            let row = sqlx::query(sqlx::AssertSqlSafe(verification.as_str()))
                .fetch_one(&mut *transaction)
                .await
                .expect("verify exact ACL after column revoke");
            assert!(row.get::<bool, _>("table_acl_exact"));
        }

        let group = format!("v2_acl_group_{suffix}");
        sqlx::query(sqlx::AssertSqlSafe(format!(
            "CREATE ROLE {} NOLOGIN NOSUPERUSER NOCREATEDB NOCREATEROLE NOINHERIT",
            quote_identifier(&group)
        )))
        .execute(&mut *transaction)
        .await
        .expect("create test group role");
        sqlx::query(sqlx::AssertSqlSafe(format!(
            "GRANT {} TO {}",
            quote_identifier(&group),
            quote_identifier(&roles.worker)
        )))
        .execute(&mut *transaction)
        .await
        .expect("inject forbidden runtime role membership");
        let row = sqlx::query(sqlx::AssertSqlSafe(verification.as_str()))
            .fetch_one(&mut *transaction)
            .await
            .expect("detect forbidden runtime role membership");
        assert!(!row.get::<bool, _>("runtime_boundary_acl_exact"));
        sqlx::query(sqlx::AssertSqlSafe(format!(
            "REVOKE {} FROM {}",
            quote_identifier(&group),
            quote_identifier(&roles.worker)
        )))
        .execute(&mut *transaction)
        .await
        .expect("remove forbidden runtime role membership");

        sqlx::query(sqlx::AssertSqlSafe(format!(
            "GRANT SELECT ON TABLE public.v2_lifecycle_operation TO {}",
            quote_identifier(&roles.api)
        )))
        .execute(&mut *transaction)
        .await
        .expect("inject forbidden protected grant");
        let row = sqlx::query(sqlx::AssertSqlSafe(verification.as_str()))
            .fetch_one(&mut *transaction)
            .await
            .expect("re-run catalog verifier");
        assert!(!row.get::<bool, _>("protected_acl_exact"));
        sqlx::query(sqlx::AssertSqlSafe(format!(
            "REVOKE SELECT ON TABLE public.v2_lifecycle_operation FROM {}",
            quote_identifier(&roles.api)
        )))
        .execute(&mut *transaction)
        .await
        .expect("remove forbidden protected grant");
        let row = sqlx::query(sqlx::AssertSqlSafe(verification.as_str()))
            .fetch_one(&mut *transaction)
            .await
            .expect("verify exact ACL before role DML checks");
        assert!(row.get::<bool, _>("protected_acl_exact"));

        // Granting the disposable roles to the migration session happens only
        // after the exact-membership catalog proof. It lets this same
        // connection exercise the effective privileges with SET LOCAL ROLE;
        // the outer transaction removes both memberships and all fixture rows.
        sqlx::query(sqlx::AssertSqlSafe(format!(
            "GRANT {}, {} TO {}",
            quote_identifier(&roles.api),
            quote_identifier(&roles.worker),
            quote_identifier(&roles.migration),
        )))
        .execute(&mut *transaction)
        .await
        .expect("allow migration session to assume disposable runtime roles");

        sqlx::query(sqlx::AssertSqlSafe(format!(
            "SET LOCAL ROLE {}",
            quote_identifier(&roles.api)
        )))
        .execute(&mut *transaction)
        .await
        .expect("assume API role");
        let revision_one = insert_operator_revision(
            &mut transaction,
            "00000000-0000-0000-0000-00000000a101",
            "acl-api",
        )
        .await;
        let revision_two = insert_operator_revision(
            &mut transaction,
            "00000000-0000-0000-0000-00000000a102",
            "acl-api",
        )
        .await;
        let revision_three = insert_operator_revision(
            &mut transaction,
            "00000000-0000-0000-0000-00000000a103",
            "acl-api",
        )
        .await;
        assert!(revision_one < revision_two && revision_two < revision_three);
        sqlx::query(sqlx::AssertSqlSafe(format!(
            "INSERT INTO public.v2_operator_config_state (\
             singleton, installation_id, active_revision, updated_at) \
             SELECT 1, installation_id, {revision_one}, 1000 \
             FROM public.v2_system_installation WHERE singleton = 1"
        )))
        .execute(&mut *transaction)
        .await
        .expect("API inserts operator state");
        sqlx::query(sqlx::AssertSqlSafe(format!(
            "UPDATE public.v2_operator_config_state \
             SET active_revision = {revision_two}, updated_at = 1001 \
             WHERE singleton = 1"
        )))
        .execute(&mut *transaction)
        .await
        .expect("API advances operator state");
        sqlx::query(sqlx::AssertSqlSafe(format!(
            "INSERT INTO public.v2_operator_config_api_ack (\
             singleton, installation_id, observed_revision, applied_revision, \
             status, error_code, observed_at) \
             SELECT 1, installation_id, {revision_two}, {revision_two}, \
                    'applied', NULL, 1001 \
             FROM public.v2_system_installation WHERE singleton = 1"
        )))
        .execute(&mut *transaction)
        .await
        .expect("API inserts its acknowledgement");
        let api_worker_ack = format!(
            "INSERT INTO public.v2_operator_config_worker_ack (\
             singleton, installation_id, observed_revision, applied_revision, \
             status, error_code, observed_at) \
             SELECT 1, installation_id, {revision_two}, {revision_two}, \
                    'applied', NULL, 1001 \
             FROM public.v2_system_installation WHERE singleton = 1"
        );
        assert_sql_rejected(&mut transaction, &api_worker_ack, "42501", None).await;
        sqlx::query("RESET ROLE")
            .execute(&mut *transaction)
            .await
            .expect("leave API role");

        sqlx::query(sqlx::AssertSqlSafe(format!(
            "SET LOCAL ROLE {}",
            quote_identifier(&roles.worker)
        )))
        .execute(&mut *transaction)
        .await
        .expect("assume worker role");
        let observed_active: i64 = sqlx::query_scalar(
            "SELECT revision.revision \
             FROM public.v2_operator_config_state AS state \
             JOIN public.v2_operator_config_revision AS revision \
               ON revision.revision = state.active_revision \
              AND revision.installation_id = state.installation_id \
             WHERE state.singleton = 1",
        )
        .fetch_one(&mut *transaction)
        .await
        .expect("worker reads active authority");
        assert_eq!(observed_active, revision_two);
        sqlx::query(sqlx::AssertSqlSafe(api_worker_ack.clone()))
            .execute(&mut *transaction)
            .await
            .expect("worker inserts its acknowledgement");
        let updated = sqlx::query(
            "UPDATE public.v2_operator_config_worker_ack \
             SET observed_at = 1002 WHERE singleton = 1",
        )
        .execute(&mut *transaction)
        .await
        .expect("worker updates its acknowledgement");
        assert_eq!(updated.rows_affected(), 1);
        let worker_revision =
            operator_revision_insert_sql("00000000-0000-0000-0000-00000000a104", 1, "acl-worker");
        assert_sql_rejected(&mut transaction, &worker_revision, "42501", None).await;
        let worker_state_advance = format!(
            "UPDATE public.v2_operator_config_state \
             SET active_revision = {revision_three}, updated_at = 1003 \
             WHERE singleton = 1"
        );
        assert_sql_rejected(&mut transaction, &worker_state_advance, "42501", None).await;
        assert_sql_rejected(
            &mut transaction,
            "UPDATE public.v2_operator_config_api_ack \
             SET observed_at = 1003 WHERE singleton = 1",
            "42501",
            None,
        )
        .await;
        sqlx::query("RESET ROLE")
            .execute(&mut *transaction)
            .await
            .expect("leave worker role");

        let invalid_format = operator_revision_insert_sql(
            "00000000-0000-0000-0000-00000000a105",
            2,
            "acl-migration",
        );
        assert_sql_rejected(
            &mut transaction,
            &invalid_format,
            "23514",
            Some("chk_operator_config_format_version"),
        )
        .await;
        let revision_update = format!(
            "UPDATE public.v2_operator_config_revision \
             SET created_at = created_at + 1 WHERE revision = {revision_one}"
        );
        assert_sql_rejected(
            &mut transaction,
            &revision_update,
            "P0001",
            Some("operator configuration revisions are immutable"),
        )
        .await;
        let revision_delete = format!(
            "DELETE FROM public.v2_operator_config_revision WHERE revision = {revision_one}"
        );
        assert_sql_rejected(
            &mut transaction,
            &revision_delete,
            "P0001",
            Some("operator configuration revisions are immutable"),
        )
        .await;
        let state_rollback = format!(
            "UPDATE public.v2_operator_config_state \
             SET active_revision = {revision_one} WHERE singleton = 1"
        );
        assert_sql_rejected(
            &mut transaction,
            &state_rollback,
            "P0001",
            Some("operator configuration revision cannot move backwards"),
        )
        .await;
        assert_sql_rejected(
            &mut transaction,
            "DELETE FROM public.v2_operator_config_state WHERE singleton = 1",
            "P0001",
            Some("operator configuration state cannot be deleted"),
        )
        .await;
        assert_sql_rejected(
            &mut transaction,
            "UPDATE public.v2_operator_config_state \
             SET installation_id = '00000000-0000-0000-0000-00000000f00d'::uuid \
             WHERE singleton = 1",
            "P0001",
            Some("operator configuration state identity cannot change"),
        )
        .await;
        let invalid_applied_ack = format!(
            "UPDATE public.v2_operator_config_worker_ack \
             SET observed_revision = {revision_three}, applied_revision = {revision_two}, \
                 status = 'applied', error_code = NULL, observed_at = 1003 \
             WHERE singleton = 1"
        );
        assert_sql_rejected(
            &mut transaction,
            &invalid_applied_ack,
            "23514",
            Some("chk_operator_config_worker_ack_status"),
        )
        .await;
        let invalid_rejected_ack = format!(
            "UPDATE public.v2_operator_config_worker_ack \
             SET observed_revision = {revision_three}, applied_revision = {revision_three}, \
                 status = 'rejected', error_code = 'invalid_config', observed_at = 1003 \
             WHERE singleton = 1"
        );
        assert_sql_rejected(
            &mut transaction,
            &invalid_rejected_ack,
            "23514",
            Some("chk_operator_config_worker_ack_status"),
        )
        .await;
        let ack_rollback = format!(
            "UPDATE public.v2_operator_config_worker_ack \
             SET observed_revision = {revision_one}, applied_revision = {revision_one}, \
                 observed_at = 1003 WHERE singleton = 1"
        );
        assert_sql_rejected(
            &mut transaction,
            &ack_rollback,
            "P0001",
            Some("operator configuration acknowledgement cannot move backwards"),
        )
        .await;
        transaction.rollback().await.expect("rollback ACL fixture");
    }
}
