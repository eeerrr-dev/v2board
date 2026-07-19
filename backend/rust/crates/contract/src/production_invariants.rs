use std::time::Duration;

use anyhow::{Context, Result, ensure};
use sqlx::{PgPool, postgres::PgPoolOptions};
use v2board_config::RuntimeEnvironment;
use v2board_db::{DbPoolConfig, migrations_current};
use v2board_domain::redis_runtime::verify_redis_runtime;

mod access_control;
mod admin_projections;
mod admin_users;
mod harness;
mod payments;
mod schema;
mod worker_flows;

#[cfg(test)]
mod tests;

pub(crate) use harness::{
    DEFAULT_INTEGRATION_REDIS_URL, DEFAULT_ROOT_DATABASE_URL, GeneratedDatabaseName, MIGRATOR,
    create_database, database_url_for, drop_database, env_or, flush_redis, integration_config,
};

use access_control::{
    auth_rate_limits, invite_single_consumption, node_identity_epoch, redis_lease_ownership,
};
use admin_projections::admin_projection_key_sets;
use admin_users::admin_user_w12_mutations;
#[cfg(test)]
use harness::validate_generated_database_name;
use payments::late_payment_reconciliation;
use schema::{
    analytics_outbox_invariant, audit_log_append_only, install_contract_analytics_admission,
    install_contract_operator_config_authority, installation_identity_invariant,
    migration_readiness_failure_modes, schema_invariants,
};
use worker_flows::{ticket_state_machine, traffic_epoch_invariant, worker_health_process};

const DEFAULT_RUNTIME_REDIS_URL: &str = "redis://redis:6379/1";

pub async fn run() -> Result<()> {
    let root_database_url = env_or(
        "RUST_INTEGRATION_DATABASE_ROOT_URL",
        DEFAULT_ROOT_DATABASE_URL,
    );
    let runtime_redis_url = env_or("REDIS_URL", DEFAULT_RUNTIME_REDIS_URL);
    let integration_redis_url = env_or("RUST_INTEGRATION_REDIS_URL", DEFAULT_INTEGRATION_REDIS_URL);
    ensure!(
        runtime_redis_url != integration_redis_url,
        "RUST_INTEGRATION_REDIS_URL must select a Redis database isolated from REDIS_URL"
    );

    let database_name = GeneratedDatabaseName::new("contract")?;
    let database_url = database_url_for(&root_database_url, &database_name)?;
    let root = PgPoolOptions::new()
        .max_connections(2)
        .connect(&root_database_url)
        .await
        .context("connect to the disposable-database administrator")?;
    create_database(&root, &database_name).await?;

    let pool_config = DbPoolConfig {
        min_connections: 1,
        max_connections: 40,
        acquire_timeout: Duration::from_secs(10),
        idle_timeout: Duration::from_secs(30),
        max_lifetime: Duration::from_secs(300),
    };
    let pool = match v2board_db::connect_postgres_with_config(&database_url, &pool_config).await {
        Ok(pool) => pool,
        Err(error) => {
            let error =
                anyhow::Error::new(error).context("connect to the disposable integration database");
            let cleanup = drop_database(&root, &database_name).await;
            root.close().await;
            return match cleanup {
                Ok(()) => Err(error),
                Err(cleanup) => Err(error.context(format!(
                    "also failed to drop disposable database {}: {cleanup:#}",
                    database_name.as_str()
                ))),
            };
        }
    };
    let result = run_isolated_checks(
        &pool,
        &database_url,
        database_name.as_str(),
        &integration_redis_url,
    )
    .await;

    pool.close().await;
    let drop_result = drop_database(&root, &database_name).await;
    root.close().await;

    match (result, drop_result) {
        (Err(error), Err(cleanup)) => Err(error.context(format!(
            "also failed to drop disposable database {}: {cleanup:#}",
            database_name.as_str()
        ))),
        (Err(error), Ok(())) => Err(error),
        (Ok(()), Err(cleanup)) => Err(cleanup),
        (Ok(()), Ok(())) => {
            println!("Production invariant gate passed; disposable state was removed.");
            Ok(())
        }
    }
}

async fn run_isolated_checks(
    pool: &PgPool,
    database_url: &str,
    database_name: &str,
    integration_redis_url: &str,
) -> Result<()> {
    let integration_redis = redis::Client::open(integration_redis_url)?;
    flush_redis(&integration_redis).await?;

    let result = async {
        crate::sql_schema_prepare::audit_dynamic_inventory()?;
        MIGRATOR
            .run(pool)
            .await
            .context("apply every embedded migration to a fresh PostgreSQL database")?;
        ensure!(
            migrations_current(pool).await?,
            "freshly applied migration ledger is not current"
        );
        installation_identity_invariant(pool).await?;
        pass("installation identity is explicit, unique, and immutable");
        install_contract_operator_config_authority(pool, integration_redis_url).await?;
        pass("operator configuration authority is explicit and authenticated");
        install_contract_analytics_admission(pool).await?;
        pass("analytics admission policy is installation-bound and measurable");
        schema_invariants(pool).await?;
        pass("fresh migrations and production schema constraints");

        audit_log_append_only(pool).await?;
        pass("operator audit trail is append-only and surface-constrained");

        analytics_outbox_invariant(pool).await?;
        pass("analytics outbox uniqueness, batching, and leases are durable");

        crate::sql_schema_prepare::run(pool).await?;
        pass("static runtime SQL prepares against the migrated production schema");

        traffic_epoch_invariant(pool, database_url, database_name, integration_redis_url).await?;
        pass("traffic epoch rejects delayed pre-reset reports");

        invite_single_consumption(pool, integration_redis_url).await?;
        pass("single-use invite remains single-use under concurrency");
        flush_redis(&integration_redis).await?;
        verify_redis_runtime(&integration_redis, RuntimeEnvironment::Production).await?;
        pass("production Redis policy is verifiably noeviction");

        ticket_state_machine(pool, database_url, database_name, integration_redis_url).await?;
        pass("one-open-ticket and reply/auto-close serialization");

        node_identity_epoch(pool).await?;
        pass("node credentials are bound to identity and revocation epoch");

        auth_rate_limits(pool, database_url, integration_redis_url).await?;
        pass("registration and login reservations are atomic in Redis");
        flush_redis(&integration_redis).await?;

        redis_lease_ownership(&integration_redis).await?;
        pass("a stale worker lease owner cannot renew or release a replacement lease");

        worker_health_process(pool, database_url, database_name, integration_redis_url).await?;
        pass("a live isolated worker publishes health and per-loop heartbeats");

        late_payment_reconciliation(pool, database_url, integration_redis_url).await?;
        pass("late authenticated payment reconciliation is durable and idempotent");

        admin_projection_key_sets(pool, integration_redis_url).await?;
        pass("admin projections serialize exactly their pinned key sets");

        admin_user_w12_mutations(pool, integration_redis_url).await?;
        pass("W12 admin user DSL filter, mail idempotency replay, and ban/reset mutations");

        migration_readiness_failure_modes(pool).await?;
        pass("migration readiness fails closed for missing or corrupt ledger state");
        Ok(())
    }
    .await;

    let cleanup = flush_redis(&integration_redis).await;
    match (result, cleanup) {
        (Err(error), Err(cleanup)) => Err(error.context(format!(
            "also failed to flush the isolated integration Redis database: {cleanup:#}"
        ))),
        (Err(error), Ok(())) => Err(error),
        (Ok(()), Err(cleanup)) => Err(cleanup),
        (Ok(()), Ok(())) => Ok(()),
    }
}

fn pass(name: &str) {
    println!("PASS {name}");
}
