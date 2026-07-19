use std::collections::BTreeSet;

use sqlx::{PgPool, postgres::PgPoolOptions};
use uuid::Uuid;

use super::{
    postgres_acl_registry::{
        API_COLUMN_GRANTS, API_DELETE_TABLES, API_INSERT_TABLES, API_SELECT_TABLES, API_SEQUENCES,
        API_UPDATE_TABLES, PostgresColumnGrant, RUNTIME_SEQUENCES, RUNTIME_TABLES,
        WORKER_COLUMN_GRANTS, WORKER_DELETE_TABLES, WORKER_INSERT_TABLES, WORKER_SELECT_TABLES,
        WORKER_SEQUENCES, WORKER_UPDATE_TABLES,
    },
    postgres_target::{POSTGRES_MIGRATOR, PostgresIdentity, execute_dynamic, postgres_identifier},
};

#[derive(Clone, Copy)]
enum PostgresRuntimeRole {
    Api,
    Worker,
}

pub(crate) async fn install_postgres_runtime_grants(
    target: &PgPool,
    identity: &PostgresIdentity,
) -> anyhow::Result<()> {
    let api = postgres_identifier(&identity.api_role);
    let worker = postgres_identifier(&identity.worker_role);
    let database = postgres_identifier(&identity.database);
    execute_dynamic(
        target,
        format!("REVOKE ALL ON DATABASE {database} FROM PUBLIC, {api}, {worker}"),
    )
    .await?;
    execute_dynamic(
        target,
        format!("GRANT CONNECT ON DATABASE {database} TO {api}, {worker}"),
    )
    .await?;
    execute_dynamic(
        target,
        format!("REVOKE ALL ON SCHEMA public FROM PUBLIC, {api}, {worker}"),
    )
    .await?;
    execute_dynamic(
        target,
        format!("GRANT USAGE ON SCHEMA public TO {api}, {worker}"),
    )
    .await?;

    execute_dynamic(
        target,
        format!("REVOKE ALL ON ALL TABLES IN SCHEMA public FROM PUBLIC, {api}, {worker}"),
    )
    .await?;
    execute_dynamic(
        target,
        format!("REVOKE ALL ON ALL SEQUENCES IN SCHEMA public FROM PUBLIC, {api}, {worker}"),
    )
    .await?;

    for (role, privilege, tables) in [
        (&api, "SELECT", API_SELECT_TABLES),
        (&api, "INSERT", API_INSERT_TABLES),
        (&api, "UPDATE", API_UPDATE_TABLES),
        (&api, "DELETE", API_DELETE_TABLES),
        (&worker, "SELECT", WORKER_SELECT_TABLES),
        (&worker, "INSERT", WORKER_INSERT_TABLES),
        (&worker, "UPDATE", WORKER_UPDATE_TABLES),
        (&worker, "DELETE", WORKER_DELETE_TABLES),
    ] {
        grant_postgres_tables(target, role, privilege, tables).await?;
    }
    for (role, grants) in [(&api, API_COLUMN_GRANTS), (&worker, WORKER_COLUMN_GRANTS)] {
        for grant in grants {
            grant_postgres_columns(target, role, grant.privilege, grant.table, grant.columns)
                .await?;
        }
    }
    grant_postgres_sequences(target, &api, API_SEQUENCES).await?;
    grant_postgres_sequences(target, &worker, WORKER_SEQUENCES).await?;
    verify_postgres_runtime_roles(identity).await
}

async fn grant_postgres_tables(
    target: &PgPool,
    role: &str,
    privilege: &str,
    tables: &[&str],
) -> anyhow::Result<()> {
    let tables = tables
        .iter()
        .map(|table| postgres_identifier(table))
        .collect::<Vec<_>>()
        .join(", ");
    execute_dynamic(
        target,
        format!("GRANT {privilege} ON TABLE {tables} TO {role}"),
    )
    .await
}

async fn grant_postgres_sequences(
    target: &PgPool,
    role: &str,
    sequences: &[&str],
) -> anyhow::Result<()> {
    let sequences = sequences
        .iter()
        .map(|sequence| postgres_identifier(sequence))
        .collect::<Vec<_>>()
        .join(", ");
    execute_dynamic(
        target,
        format!("GRANT USAGE ON SEQUENCE {sequences} TO {role}"),
    )
    .await
}

async fn grant_postgres_columns(
    target: &PgPool,
    role: &str,
    privilege: &str,
    table: &str,
    columns: &[&str],
) -> anyhow::Result<()> {
    let columns = columns
        .iter()
        .map(|column| postgres_identifier(column))
        .collect::<Vec<_>>()
        .join(", ");
    execute_dynamic(
        target,
        format!(
            "GRANT {privilege} ({columns}) ON TABLE {} TO {role}",
            postgres_identifier(table)
        ),
    )
    .await
}

async fn verify_postgres_runtime_roles(identity: &PostgresIdentity) -> anyhow::Result<()> {
    for (kind, url, expected_role) in [
        (PostgresRuntimeRole::Api, &identity.api, &identity.api_role),
        (
            PostgresRuntimeRole::Worker,
            &identity.worker,
            &identity.worker_role,
        ),
    ] {
        let pool = PgPoolOptions::new()
            .min_connections(1)
            .max_connections(1)
            .connect(url.as_str())
            .await?;
        let (role, database, installation): (String, String, Uuid) = sqlx::query_as(
            "SELECT current_user, current_database(), installation_id FROM system_installation WHERE singleton = 1",
        )
        .fetch_one(&pool)
        .await?;
        if role != **expected_role || database != identity.database {
            anyhow::bail!("PostgreSQL runtime role connected to an unexpected identity");
        }
        let (
            can_connect,
            can_create_database_objects,
            can_create_temp,
            can_use_schema,
            can_create_schema_objects,
        ): (bool, bool, bool, bool, bool) = sqlx::query_as(
            "SELECT has_database_privilege(current_user, current_database(), 'CONNECT'), \
                    has_database_privilege(current_user, current_database(), 'CREATE'), \
                    has_database_privilege(current_user, current_database(), 'TEMP'), \
                    has_schema_privilege(current_user, 'public', 'USAGE'), \
                    has_schema_privilege(current_user, 'public', 'CREATE')",
        )
        .fetch_one(&pool)
        .await?;
        if !can_connect
            || can_create_database_objects
            || can_create_temp
            || !can_use_schema
            || can_create_schema_objects
            || installation.is_nil()
        {
            anyhow::bail!(
                "PostgreSQL runtime role retained DDL/TEMP or lost its schema/installation binding"
            );
        }
        let (can_connect_postgres, can_temp_postgres, can_connect_template1, can_temp_template1): (
            bool,
            bool,
            bool,
            bool,
        ) = sqlx::query_as(
            "SELECT has_database_privilege(current_user, 'postgres', 'CONNECT'), \
                    has_database_privilege(current_user, 'postgres', 'TEMP'), \
                    has_database_privilege(current_user, 'template1', 'CONNECT'), \
                    has_database_privilege(current_user, 'template1', 'TEMP')",
        )
        .fetch_one(&pool)
        .await?;
        if can_connect_postgres || can_temp_postgres || can_connect_template1 || can_temp_template1
        {
            anyhow::bail!(
                "PostgreSQL runtime role can escape the target database through the dedicated cluster"
            );
        }
        let databases = sqlx::query_scalar::<_, String>(
            "SELECT datname FROM pg_database WHERE NOT datistemplate ORDER BY datname",
        )
        .fetch_all(&pool)
        .await?;
        let mut expected_databases = vec!["postgres".to_string(), identity.database.clone()];
        expected_databases.sort();
        if databases != expected_databases {
            anyhow::bail!(
                "dedicated PostgreSQL cluster gained an unexpected non-template database: {databases:?}"
            );
        }
        verify_postgres_migration_ledger_access(&pool).await?;
        verify_postgres_table_acl(&pool, kind).await?;
        verify_postgres_column_acl(&pool, kind).await?;
        verify_postgres_sequence_acl(&pool, kind).await?;
        pool.close().await;

        let mut maintenance_url = (*url).clone();
        maintenance_url.set_path("/postgres");
        if PgPoolOptions::new()
            .min_connections(1)
            .max_connections(1)
            .connect(maintenance_url.as_str())
            .await
            .is_ok()
        {
            anyhow::bail!(
                "PostgreSQL runtime role unexpectedly connected to the maintenance database"
            );
        }
    }
    Ok(())
}

async fn verify_postgres_migration_ledger_access(pool: &PgPool) -> anyhow::Result<()> {
    let applied = sqlx::query_as::<_, (i64, Vec<u8>, bool)>(
        "SELECT version, checksum, success FROM _sqlx_migrations ORDER BY version",
    )
    .fetch_all(pool)
    .await?;
    let embedded = POSTGRES_MIGRATOR
        .iter()
        .filter(|migration| migration.migration_type.is_up_migration())
        .collect::<Vec<_>>();
    if applied.len() != embedded.len()
        || applied
            .iter()
            .zip(embedded)
            .any(|((version, checksum, success), migration)| {
                !success
                    || *version != migration.version
                    || checksum.as_slice() != migration.checksum.as_ref()
            })
    {
        anyhow::bail!("PostgreSQL runtime role cannot verify the exact migration ledger");
    }
    Ok(())
}

fn expected_postgres_tables(kind: PostgresRuntimeRole, privilege: &str) -> &'static [&'static str] {
    match (kind, privilege) {
        (PostgresRuntimeRole::Api, "SELECT") => API_SELECT_TABLES,
        (PostgresRuntimeRole::Api, "INSERT") => API_INSERT_TABLES,
        (PostgresRuntimeRole::Api, "UPDATE") => API_UPDATE_TABLES,
        (PostgresRuntimeRole::Api, "DELETE") => API_DELETE_TABLES,
        (PostgresRuntimeRole::Worker, "SELECT") => WORKER_SELECT_TABLES,
        (PostgresRuntimeRole::Worker, "INSERT") => WORKER_INSERT_TABLES,
        (PostgresRuntimeRole::Worker, "UPDATE") => WORKER_UPDATE_TABLES,
        (PostgresRuntimeRole::Worker, "DELETE") => WORKER_DELETE_TABLES,
        _ => &[],
    }
}

async fn verify_postgres_table_acl(pool: &PgPool, kind: PostgresRuntimeRole) -> anyhow::Result<()> {
    for table in RUNTIME_TABLES {
        let qualified = format!("public.{table}");
        for privilege in [
            "SELECT",
            "INSERT",
            "UPDATE",
            "DELETE",
            "TRUNCATE",
            "REFERENCES",
            "TRIGGER",
        ] {
            let observed: bool =
                sqlx::query_scalar("SELECT has_table_privilege(current_user, $1::text, $2::text)")
                    .bind(&qualified)
                    .bind(privilege)
                    .fetch_one(pool)
                    .await?;
            let expected = expected_postgres_tables(kind, privilege).contains(table);
            if observed != expected {
                anyhow::bail!(
                    "PostgreSQL runtime table privilege drifted: table={table}, privilege={privilege}, expected={expected}, observed={observed}"
                );
            }
        }
    }
    Ok(())
}

async fn verify_postgres_sequence_acl(
    pool: &PgPool,
    kind: PostgresRuntimeRole,
) -> anyhow::Result<()> {
    let expected_sequences = match kind {
        PostgresRuntimeRole::Api => API_SEQUENCES,
        PostgresRuntimeRole::Worker => WORKER_SEQUENCES,
    };
    for sequence in RUNTIME_SEQUENCES {
        let qualified = format!("public.{sequence}");
        for privilege in ["USAGE", "SELECT", "UPDATE"] {
            let observed: bool = sqlx::query_scalar(
                "SELECT has_sequence_privilege(current_user, $1::text, $2::text)",
            )
            .bind(&qualified)
            .bind(privilege)
            .fetch_one(pool)
            .await?;
            let expected = privilege == "USAGE" && expected_sequences.contains(sequence);
            if observed != expected {
                anyhow::bail!(
                    "PostgreSQL runtime sequence privilege drifted: sequence={sequence}, privilege={privilege}, expected={expected}, observed={observed}"
                );
            }
        }
    }
    Ok(())
}

async fn verify_postgres_column_acl(
    pool: &PgPool,
    kind: PostgresRuntimeRole,
) -> anyhow::Result<()> {
    let columns = sqlx::query_as::<_, (String, String)>(
        "SELECT c.relname, a.attname \
         FROM pg_catalog.pg_class AS c \
         JOIN pg_catalog.pg_namespace AS n ON n.oid = c.relnamespace \
         JOIN pg_catalog.pg_attribute AS a ON a.attrelid = c.oid \
         WHERE n.nspname = 'public' AND c.relkind IN ('r', 'p') \
           AND a.attnum > 0 AND NOT a.attisdropped \
         ORDER BY c.relname, a.attnum",
    )
    .fetch_all(pool)
    .await?;
    let observed_tables = columns
        .iter()
        .map(|(table, _)| table.as_str())
        .collect::<BTreeSet<_>>();
    let expected_tables = RUNTIME_TABLES.iter().copied().collect::<BTreeSet<_>>();
    if observed_tables != expected_tables {
        anyhow::bail!("PostgreSQL runtime ACL verifier table registry drifted");
    }
    for (table, column) in columns {
        for privilege in ["SELECT", "INSERT", "UPDATE"] {
            let observed: bool = sqlx::query_scalar(
                "SELECT has_column_privilege(current_user, $1::text, $2::text, $3::text)",
            )
            .bind(format!("public.{table}"))
            .bind(&column)
            .bind(privilege)
            .fetch_one(pool)
            .await?;
            let expected = expected_postgres_tables(kind, privilege).contains(&table.as_str())
                || postgres_column_grants(kind).iter().any(|grant| {
                    grant.privilege == privilege
                        && grant.table == table
                        && grant.columns.contains(&column.as_str())
                });
            if observed != expected {
                anyhow::bail!(
                    "PostgreSQL runtime column privilege drifted: table={table}, column={column}, privilege={privilege}, expected={expected}, observed={observed}"
                );
            }
        }
    }
    Ok(())
}

fn postgres_column_grants(kind: PostgresRuntimeRole) -> &'static [PostgresColumnGrant] {
    match kind {
        PostgresRuntimeRole::Api => API_COLUMN_GRANTS,
        PostgresRuntimeRole::Worker => WORKER_COLUMN_GRANTS,
    }
}
