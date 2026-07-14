use std::sync::{
    Arc,
    atomic::{AtomicI64, Ordering},
};

use arc_swap::ArcSwap;
use uuid::Uuid;
use v2board_config::{AppConfig, RedisKeyspace};
use v2board_db::DbPool;
use v2board_domain::{
    operator_config::{self, OperatorConfigConsumer, OperatorConfigError},
    smtp::SmtpTransportCache,
};

#[derive(Clone)]
pub(crate) struct WorkerState {
    pub(crate) config: Arc<AppConfig>,
    config_store: Arc<ArcSwap<AppConfig>>,
    config_reload: Arc<tokio::sync::Mutex<Option<String>>>,
    pending_operator_ack: Arc<AtomicI64>,
    pub(crate) db: DbPool,
    pub(crate) installation_id: Uuid,
    pub(crate) redis_keys: RedisKeyspace,
    pub(crate) redis: redis::Client,
    pub(crate) smtp: SmtpTransportCache,
}

impl WorkerState {
    pub(crate) fn new(
        config: Arc<AppConfig>,
        db: DbPool,
        installation_id: Uuid,
        redis: redis::Client,
        smtp: SmtpTransportCache,
    ) -> Self {
        Self {
            config: config.clone(),
            config_store: Arc::new(ArcSwap::from(config)),
            config_reload: Arc::new(tokio::sync::Mutex::new(None)),
            pending_operator_ack: Arc::new(AtomicI64::new(0)),
            db,
            installation_id,
            redis_keys: RedisKeyspace::new(installation_id),
            redis,
            smtp,
        }
    }

    pub(crate) fn redis_key(&self, logical_key: &str) -> String {
        self.redis_keys.key(logical_key)
    }

    /// Refreshes the worker's last-known-good snapshot from the shared
    /// PostgreSQL authority. The worker can only read revisions and write its
    /// own acknowledgement table; it never creates or activates a revision.
    pub(crate) async fn refresh_operator_config(&self) -> anyhow::Result<Arc<AppConfig>> {
        let mut last_error = self.config_reload.lock().await;
        let current = self.config_store.load_full();
        let result = async {
            let snapshot = match operator_config::load_active(
                &self.db,
                self.installation_id,
                &current.app_key,
            )
            .await
            {
                Ok(Some(snapshot)) => snapshot,
                Ok(None) => return Err(OperatorConfigError::MissingAuthority.into()),
                Err(error) => {
                    if let Some((observed_revision, error_code)) = error.observed_rejection()
                        && current
                            .operator_revision()
                            .is_none_or(|revision| revision < observed_revision)
                    {
                        let _ = operator_config::acknowledge(
                            &self.db,
                            self.installation_id,
                            OperatorConfigConsumer::Worker,
                            observed_revision,
                            current.operator_revision(),
                            Some(error_code),
                        )
                        .await;
                    }
                    return Err(error.into());
                }
            };
            if current.operator_revision() == Some(snapshot.revision) {
                if self.pending_operator_ack.load(Ordering::Acquire) == snapshot.revision {
                    operator_config::acknowledge(
                        &self.db,
                        self.installation_id,
                        OperatorConfigConsumer::Worker,
                        snapshot.revision,
                        Some(snapshot.revision),
                        None,
                    )
                    .await?;
                    self.pending_operator_ack.store(0, Ordering::Release);
                }
                return Ok(current.clone());
            }

            let observed_revision = snapshot.revision;
            let reload_base = current.clone();
            let values = snapshot.values;
            let config = match tokio::task::spawn_blocking(move || {
                reload_base.with_operator_config(&values, observed_revision)
            })
            .await?
            {
                Ok(config) => Arc::new(config),
                Err(error) => {
                    let _ = operator_config::acknowledge(
                        &self.db,
                        self.installation_id,
                        OperatorConfigConsumer::Worker,
                        observed_revision,
                        current.operator_revision(),
                        Some("typed_validation_failed"),
                    )
                    .await;
                    return Err(anyhow::Error::from(error));
                }
            };
            self.config_store.store(config.clone());
            if let Err(error) = operator_config::acknowledge(
                &self.db,
                self.installation_id,
                OperatorConfigConsumer::Worker,
                observed_revision,
                Some(observed_revision),
                None,
            )
            .await
            {
                self.pending_operator_ack
                    .store(observed_revision, Ordering::Release);
                return Err(error.into());
            }
            self.pending_operator_ack.store(0, Ordering::Release);
            Ok(config)
        }
        .await;

        match &result {
            Ok(_) => {
                if last_error.take().is_some() {
                    tracing::info!("worker operator configuration reload recovered");
                }
            }
            Err(error) => {
                let message = error.to_string();
                if last_error.as_deref() != Some(message.as_str()) {
                    tracing::warn!(?error, "keeping last-known-good worker configuration");
                }
                *last_error = Some(message);
            }
        }
        result
    }

    pub(crate) async fn snapshot_config_for_job(&self) -> anyhow::Result<Self> {
        let config = self.refresh_operator_config().await?;
        Ok(Self {
            config,
            config_store: self.config_store.clone(),
            config_reload: self.config_reload.clone(),
            pending_operator_ack: self.pending_operator_ack.clone(),
            db: self.db.clone(),
            installation_id: self.installation_id,
            redis_keys: self.redis_keys.clone(),
            redis: self.redis.clone(),
            smtp: self.smtp.clone(),
        })
    }
}
