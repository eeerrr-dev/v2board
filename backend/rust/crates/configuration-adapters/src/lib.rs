//! Production outer adapters for administrative configuration use cases.

mod bulk_mail;
mod external;
pub mod operator_config;
mod repository;
mod telegram;
mod values;

use std::sync::Arc;

pub use bulk_mail::PostgresBulkMailRepository;
pub use external::RuntimeConfigurationExternal;
pub use repository::RuntimeConfigurationRepository;
pub use telegram::telegram_webhook_secret;
use uuid::Uuid;
use v2board_application::configuration::ConfigurationService;
use v2board_config::AppConfig;
use v2board_db::DbPool;
use v2board_mail_adapters::smtp::SmtpTransportCache;

pub type RuntimeConfigurationService = ConfigurationService<
    RuntimeConfigurationRepository,
    RuntimeConfigurationExternal,
    PostgresBulkMailRepository,
>;

pub fn runtime_configuration_service(
    db: DbPool,
    installation_id: Uuid,
    config: Arc<AppConfig>,
    http: reqwest::Client,
    smtp: SmtpTransportCache,
) -> RuntimeConfigurationService {
    ConfigurationService::new(
        RuntimeConfigurationRepository::new(db.clone(), installation_id, config.clone()),
        RuntimeConfigurationExternal::new(config.clone(), http, smtp),
        PostgresBulkMailRepository::new(db, config),
    )
}
