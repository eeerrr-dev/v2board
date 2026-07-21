use std::sync::Arc;

use anyhow::{Result, ensure};
use sqlx::PgPool;
use uuid::Uuid;
use v2board_application::configuration::{
    ConfigurationCode, ConfigurationError, ConfigurationMap, ConfigurationPatchOutcome,
    ConfigurationValue,
};
use v2board_configuration_adapters::operator_config;
use v2board_configuration_adapters::runtime_configuration_service;
use v2board_db::installation_id;
use v2board_mail_adapters::smtp::SmtpTransportCache;

use super::harness::operator_authority_config;

/// Drives the application use case through the real authenticated PostgreSQL
/// authority adapter. This pins redaction, normalized commit, database-issued
/// revisions, and stale-writer rejection at the architecture boundary.
pub(super) async fn operator_configuration_application_flow(pool: &PgPool) -> Result<()> {
    let bootstrap = operator_authority_config()?;
    let installation_id = installation_id(pool).await?;
    let authority = operator_config::load_active(pool, installation_id, &bootstrap.app_key)
        .await?
        .ok_or_else(|| anyhow::anyhow!("operator configuration authority is missing"))?;
    let active = bootstrap.with_operator_config(&authority.values, authority.revision)?;
    let original_revision = authority.revision;
    let service = runtime_configuration_service(
        pool.clone(),
        installation_id,
        Arc::new(active),
        reqwest::Client::new(),
        SmtpTransportCache::default(),
    );

    let view = service.view(Some("server"))?;
    ensure!(view.revision == original_revision && view.groups.len() == 1);
    if let Some(ConfigurationValue::String(token)) = view
        .groups
        .get("server")
        .and_then(|group| group.get("server_token"))
    {
        ensure!(
            token == "********",
            "server token escaped the redacted view"
        );
    }

    let description = format!("contract-config-{}", Uuid::new_v4().simple());
    let outcome = service
        .patch(
            ConfigurationMap::from([(
                "app_description".to_string(),
                ConfigurationValue::String(description.clone()),
            )]),
            original_revision,
            "contract-admin@example.test",
        )
        .await?;
    let ConfigurationPatchOutcome::Committed {
        activation,
        revision,
    } = outcome
    else {
        anyhow::bail!("changed operator configuration was treated as a no-op");
    };
    ensure!(
        revision == original_revision + 1
            && activation.operator_revision() == Some(revision)
            && activation.app_description.as_deref() == Some(description.as_str()),
        "configuration commit did not return the database revision and validated activation"
    );
    let committed = operator_config::load_active(pool, installation_id, &activation.app_key)
        .await?
        .ok_or_else(|| anyhow::anyhow!("committed operator authority disappeared"))?;
    ensure!(
        committed.revision == revision
            && committed.values.get("app_description")
                == Some(&serde_json::Value::String(description)),
        "configuration adapter did not durably commit the normalized candidate"
    );

    let stale = service
        .patch(
            ConfigurationMap::new(),
            original_revision,
            "contract-admin@example.test",
        )
        .await;
    ensure!(
        matches!(
            stale,
            Err(ConfigurationError::Business {
                code: ConfigurationCode::ConfigRevisionConflict,
                ..
            })
        ),
        "stale operator writer was not rejected by optimistic concurrency"
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use anyhow::Context;
    use sqlx::postgres::PgPoolOptions;
    use v2board_db::DbPoolConfig;

    use super::*;
    use crate::production_invariants::{
        DEFAULT_ROOT_DATABASE_URL, GeneratedDatabaseName, MIGRATOR, create_database,
        database_url_for, drop_database, env_or,
        schema::{install_contract_operator_config_authority, installation_identity_invariant},
    };

    #[tokio::test]
    async fn real_repository_pins_revision_redaction_and_stale_writer_rejection() -> Result<()> {
        let root_database_url = env_or(
            "RUST_INTEGRATION_DATABASE_ROOT_URL",
            DEFAULT_ROOT_DATABASE_URL,
        );
        let database_name = GeneratedDatabaseName::new("config")?;
        let database_url = database_url_for(&root_database_url, &database_name)?;
        let root = PgPoolOptions::new()
            .max_connections(2)
            .connect(&root_database_url)
            .await
            .context("connect to the disposable-database administrator")?;
        create_database(&root, &database_name).await?;

        let pool_config = DbPoolConfig {
            min_connections: 1,
            max_connections: 5,
            acquire_timeout: Duration::from_secs(10),
            idle_timeout: Duration::from_secs(30),
            max_lifetime: Duration::from_secs(300),
            ..DbPoolConfig::default()
        };
        let pool = match v2board_db::connect_postgres_with_config(&database_url, &pool_config).await
        {
            Ok(pool) => pool,
            Err(error) => {
                let cleanup = drop_database(&root, &database_name).await;
                root.close().await;
                cleanup?;
                return Err(error.into());
            }
        };
        let result = async {
            MIGRATOR.run(&pool).await?;
            installation_identity_invariant(&pool).await?;
            install_contract_operator_config_authority(&pool).await?;
            operator_configuration_application_flow(&pool).await
        }
        .await;

        pool.close().await;
        let cleanup = drop_database(&root, &database_name).await;
        root.close().await;
        result?;
        cleanup
    }
}
