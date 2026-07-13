//! Frozen PostgreSQL runtime ACL policy for the native API and worker.
//!
//! The migration principal owns the schema and lifecycle state. Runtime
//! principals deliberately share the current business-table DML allowlist;
//! lifecycle, copy-checkpoint, and legacy-fold provenance stays invisible to
//! both. New tables and sequences fail closed until this policy is reviewed and
//! re-applied by a lifecycle operation.

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
    let mut sequence_names = Vec::with_capacity(owned_sequences.len());
    for row in owned_sequences {
        let sequence_name: String = row.try_get("sequence_name")?;
        let table_name: String = row.try_get("table_name")?;
        if mutable.contains(table_name.as_str()) {
            sequence_names.push(sequence_name);
        }
    }
    if !sequence_names.is_empty() {
        let sequences = sequence_names
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
         sequence_acl AS ( \
           SELECT COALESCE(bool_and( \
             CASE WHEN expected.policy = 'mutable' THEN \
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
           ) AS exact \
           FROM runtime_role \
         ) \
         SELECT shape.schema_state, \
           (shape.schema_state = 'empty' OR table_acl.exact) AS table_acl_exact, \
           (shape.schema_state = 'empty' OR table_acl.protected_exact) AS protected_acl_exact, \
           sequence_acl.exact AS sequence_acl_exact, \
           default_acl.fail_closed AS default_acl_fail_closed, \
           runtime_boundary.exact AS runtime_boundary_acl_exact \
         FROM shape CROSS JOIN table_acl CROSS JOIN sequence_acl CROSS JOIN default_acl \
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

    #[test]
    fn frozen_acl_tables_exactly_cover_the_embedded_migration_lineage() {
        let migrations = [
            include_str!("../../../migrations-postgres/0001_initial.sql"),
            include_str!(
                "../../../migrations-postgres/0002_legacy_lifecycle_and_analytics_admission.sql"
            ),
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
            .chain(RUNTIME_HIDDEN_TABLES)
            .chain(RUNTIME_MUTABLE_TABLES)
            .map(|table| (*table).to_string())
            .collect::<BTreeSet<_>>();
        assert_eq!(policy, actual);
        assert_eq!(
            policy.len(),
            RUNTIME_READ_ONLY_TABLES.len()
                + RUNTIME_ADMISSION_STATE_TABLES.len()
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
        assert!(sql.contains("aclexplode(defaults.defaclacl)"));
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
        let runtime_roles = format!(
            "{}, {}",
            quote_identifier(&roles.api),
            quote_identifier(&roles.worker)
        );
        sqlx::query(sqlx::AssertSqlSafe(format!(
            "GRANT USAGE ON ALL SEQUENCES IN SCHEMA public TO {runtime_roles}"
        )))
        .execute(&mut *transaction)
        .await
        .expect("grant identity sequence usage");

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

        sqlx::query(sqlx::AssertSqlSafe(format!(
            "GRANT SELECT ON TABLE public.v2_lifecycle_operation TO {}",
            quote_identifier(&roles.api)
        )))
        .execute(&mut *transaction)
        .await
        .expect("inject forbidden protected grant");
        let row = sqlx::query(sqlx::AssertSqlSafe(verification))
            .fetch_one(&mut *transaction)
            .await
            .expect("re-run catalog verifier");
        assert!(!row.get::<bool, _>("protected_acl_exact"));
        transaction.rollback().await.expect("rollback ACL fixture");
    }
}
