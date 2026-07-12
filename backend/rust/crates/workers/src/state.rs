use std::sync::Arc;

use arc_swap::ArcSwap;
use uuid::Uuid;
use v2board_config::AppConfig;
use v2board_db::DbPool;
use v2board_domain::smtp::SmtpTransportCache;

#[derive(Clone)]
pub(crate) struct WorkerState {
    pub(crate) config: Arc<AppConfig>,
    config_store: Arc<ArcSwap<AppConfig>>,
    config_reload: Arc<tokio::sync::Mutex<Option<String>>>,
    pub(crate) db: DbPool,
    pub(crate) installation_id: Uuid,
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
            db,
            installation_id,
            redis,
            smtp,
        }
    }

    pub(crate) async fn snapshot_config_for_job(&self) -> Self {
        let mut last_error = self.config_reload.lock().await;
        let current = self.config_store.load_full();
        let reload_base = current.clone();
        let config = match tokio::task::spawn_blocking(move || reload_base.reload()).await {
            Ok(Ok(config)) => {
                let config = Arc::new(config);
                self.config_store.store(config.clone());
                if last_error.take().is_some() {
                    tracing::info!("worker configuration reload recovered");
                }
                config
            }
            Ok(Err(error)) => {
                let message = error.to_string();
                if last_error.as_deref() != Some(message.as_str()) {
                    tracing::warn!(?error, "keeping last-known-good worker configuration");
                }
                *last_error = Some(message);
                current
            }
            Err(error) => {
                let message = error.to_string();
                if last_error.as_deref() != Some(message.as_str()) {
                    tracing::warn!(?error, "worker configuration reload task failed");
                }
                *last_error = Some(message);
                current
            }
        };
        Self {
            config,
            config_store: self.config_store.clone(),
            config_reload: self.config_reload.clone(),
            db: self.db.clone(),
            installation_id: self.installation_id,
            redis: self.redis.clone(),
            smtp: self.smtp.clone(),
        }
    }
}
