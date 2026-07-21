use std::sync::Arc;

use rust_decimal::prelude::ToPrimitive;
use serde_json::{Map, Value, json};
use uuid::Uuid;
use v2board_application::configuration::{
    ActiveConfiguration, ConfigurationMap, ConfigurationPortError, ConfigurationRepository,
    ConfigurationSnapshot, ConfigurationValue, MaterializedConfiguration,
};
use v2board_config::AppConfig;
use v2board_db::DbPool;

use crate::operator_config;
use crate::values::{groups_from_json, map_from_json, map_to_json};

#[derive(Clone)]
pub struct RuntimeConfigurationRepository {
    db: DbPool,
    installation_id: Uuid,
    config: Arc<AppConfig>,
}

impl RuntimeConfigurationRepository {
    pub const fn new(db: DbPool, installation_id: Uuid, config: Arc<AppConfig>) -> Self {
        Self {
            db,
            installation_id,
            config,
        }
    }
}

impl ConfigurationRepository for RuntimeConfigurationRepository {
    type Activation = AppConfig;

    fn current_snapshot(&self) -> Result<ConfigurationSnapshot, ConfigurationPortError> {
        let revision = self.config.operator_revision().ok_or_else(|| {
            ConfigurationPortError::Internal(
                "operator configuration authority is not active".to_string(),
            )
        })?;
        Ok(ConfigurationSnapshot {
            revision,
            groups: groups_from_json(grouped_view(&self.config))?,
        })
    }

    async fn load_active(&self) -> Result<ActiveConfiguration, ConfigurationPortError> {
        let authority =
            operator_config::load_active(&self.db, self.installation_id, &self.config.app_key)
                .await
                .map_err(|error| {
                    tracing::error!(
                        ?error,
                        "failed to load active operator configuration for patch"
                    );
                    ConfigurationPortError::Internal(
                        "operator configuration authority is unavailable".to_string(),
                    )
                })?
                .ok_or(ConfigurationPortError::Conflict)?;
        let current = self.config.as_ref().clone();
        let authority_values = authority.values;
        let revision = authority.revision;
        let authoritative = tokio::task::spawn_blocking(move || {
            current.with_operator_config(&authority_values, revision)
        })
        .await
        .map_err(|error| {
            tracing::error!(?error, "authoritative configuration task failed");
            ConfigurationPortError::Internal("configuration validator is unavailable".to_string())
        })?
        .map_err(|error| {
            tracing::error!(?error, "active operator configuration failed validation");
            ConfigurationPortError::Internal(
                "operator configuration authority is invalid".to_string(),
            )
        })?;
        Ok(ActiveConfiguration {
            revision,
            values: map_from_json(authoritative.operator_config_map())?,
            effective_admin_path: authoritative.admin_path(),
        })
    }

    async fn materialize(
        &self,
        active: &ActiveConfiguration,
        changes: &ConfigurationMap,
    ) -> Result<MaterializedConfiguration<Self::Activation>, ConfigurationPortError> {
        let current = self.config.as_ref().clone();
        let active_values = map_to_json(&active.values)?;
        let revision = active.revision;
        let authoritative = tokio::task::spawn_blocking(move || {
            current.with_operator_config(&active_values, revision)
        })
        .await
        .map_err(|error| {
            tracing::error!(?error, "authoritative configuration task failed");
            ConfigurationPortError::Internal("configuration validator is unavailable".to_string())
        })?
        .map_err(|error| {
            tracing::error!(?error, "active operator configuration failed validation");
            ConfigurationPortError::Internal(
                "operator configuration authority is invalid".to_string(),
            )
        })?;

        let server_token = optional_string(
            changes,
            "server_token",
            authoritative.server_token.as_deref(),
        );
        let app_url = optional_string(changes, "app_url", authoritative.app_url.as_deref());
        let force_https = match changes.get("force_https") {
            Some(ConfigurationValue::Bool(value)) => *value,
            _ => authoritative.force_https,
        };
        authoritative
            .validate_security_update(server_token, force_https, app_url)
            .map_err(|error| ConfigurationPortError::Validation {
                detail: error.to_string(),
                security: true,
            })?;

        let mut candidate = authoritative.operator_config_map();
        merge_changes(&mut candidate, changes)?;
        let base = authoritative.clone();
        let candidate_config =
            tokio::task::spawn_blocking(move || base.with_operator_config(&candidate, revision))
                .await
                .map_err(|error| {
                    tracing::error!(?error, "configuration validation task failed");
                    ConfigurationPortError::Internal(
                        "configuration validator is unavailable".to_string(),
                    )
                })?
                .map_err(|error| {
                    tracing::warn!(?error, "rejected invalid operator configuration candidate");
                    ConfigurationPortError::Validation {
                        detail: error.to_string(),
                        security: false,
                    }
                })?;
        Ok(MaterializedConfiguration {
            values: map_from_json(candidate_config.operator_config_map())?,
            activation: candidate_config,
        })
    }

    async fn commit(
        &self,
        expected_revision: i64,
        values: &ConfigurationMap,
        actor: &str,
    ) -> Result<i64, ConfigurationPortError> {
        let values = map_to_json(values)?;
        operator_config::commit(
            &self.db,
            self.installation_id,
            &self.config.app_key,
            Some(expected_revision),
            &values,
            actor,
        )
        .await
        .map(|snapshot| snapshot.revision)
        .map_err(|error| match error {
            operator_config::OperatorConfigError::Conflict { .. } => {
                ConfigurationPortError::Conflict
            }
            error => {
                tracing::error!(?error, "failed to commit operator configuration");
                ConfigurationPortError::Internal("operator configuration commit failed".to_string())
            }
        })
    }

    fn at_revision(&self, activation: Self::Activation, revision: i64) -> Self::Activation {
        activation.at_operator_revision(revision)
    }
}

fn optional_string<'a>(
    changes: &'a ConfigurationMap,
    key: &str,
    default: Option<&'a str>,
) -> Option<&'a str> {
    match changes.get(key) {
        Some(ConfigurationValue::String(value)) => Some(value),
        Some(ConfigurationValue::Null) => None,
        _ => default,
    }
}

fn merge_changes(
    candidate: &mut Map<String, Value>,
    changes: &ConfigurationMap,
) -> Result<(), ConfigurationPortError> {
    for (key, value) in changes {
        let value = match value {
            ConfigurationValue::StringList(items) => Value::Array(
                items
                    .iter()
                    .map(|item| item.trim())
                    .filter(|item| !item.is_empty())
                    .map(|item| Value::String(item.to_string()))
                    .collect(),
            ),
            _ => map_to_json(&ConfigurationMap::from([(key.clone(), value.clone())]))?
                .remove(key)
                .expect("single value exists"),
        };
        candidate.insert(key.clone(), value);
    }
    Ok(())
}

fn grouped_view(config: &AppConfig) -> Value {
    let distribution_rate = |value: Option<&str>| {
        value
            .and_then(|value| value.trim().parse::<f64>().ok())
            .map_or(Value::Null, |value| json!(value))
    };
    json!({
        "ticket": { "ticket_status": config.ticket_status },
        "deposit": { "deposit_bounus": config.deposit_bounus },
        "invite": {
            "invite_force": config.invite_force,
            "invite_commission": config.invite_commission,
            "invite_gen_limit": config.invite_gen_limit,
            "invite_never_expire": config.invite_never_expire,
            "commission_first_time_enable": config.commission_first_time_enable,
            "commission_auto_check_enable": config.commission_auto_check_enable,
            "commission_withdraw_limit": config.commission_withdraw_limit.normalize().to_string(),
            "commission_withdraw_method": config.commission_withdraw_method,
            "withdraw_close_enable": config.withdraw_close_enable,
            "commission_distribution_enable": config.commission_distribution_enable,
            "commission_distribution_l1": distribution_rate(config.commission_distribution_l1.as_deref()),
            "commission_distribution_l2": distribution_rate(config.commission_distribution_l2.as_deref()),
            "commission_distribution_l3": distribution_rate(config.commission_distribution_l3.as_deref()),
        },
        "site": {
            "logo": config.logo,
            "force_https": config.force_https,
            "stop_register": config.stop_register,
            "app_name": config.app_name,
            "app_description": config.app_description,
            "app_url": config.app_url,
            "subscribe_url": config.subscribe_url,
            "subscribe_path": config.subscribe_path,
            "try_out_plan_id": config.try_out_plan_id,
            "try_out_hour": config.try_out_hour.to_f64(),
            "tos_url": config.tos_url,
            "currency": config.currency,
            "currency_symbol": config.currency_symbol,
            "legacy_hash_redirect_enable": config.legacy_hash_redirect_enable,
        },
        "subscribe": {
            "plan_change_enable": config.plan_change_enable,
            "reset_traffic_method": config.reset_traffic_method,
            "surplus_enable": config.surplus_enable,
            "allow_new_period": config.allow_new_period != 0,
            "new_order_event_id": config.new_order_event_id != 0,
            "renew_order_event_id": config.renew_order_event_id != 0,
            "change_order_event_id": config.change_order_event_id != 0,
            "show_info_to_server_enable": config.show_info_to_server_enable,
            "show_subscribe_method": config.show_subscribe_method,
            "show_subscribe_expire": config.show_subscribe_expire,
        },
        "frontend": {
            "frontend_theme_color": config.frontend_theme_color,
            "frontend_background_url": config.frontend_background_url,
        },
        "server": {
            "server_api_url": config.server_api_url,
            "server_token": config.server_token,
            "server_pull_interval": config.server_pull_interval,
            "server_push_interval": config.server_push_interval,
            "server_node_report_min_traffic": config.server_node_report_min_traffic,
            "server_device_online_min_traffic": config.server_device_online_min_traffic,
            "device_limit_mode": config.device_limit_mode != 0,
        },
        "email": {
            "email_template": config.email_template,
            "email_host": config.email_host,
            "email_port": config.email_port,
            "email_username": config.email_username,
            "email_password": config.email_password,
            "email_encryption": config.email_encryption,
            "email_from_address": config.email_from_address,
        },
        "telegram": {
            "telegram_bot_enable": config.telegram_bot_enable,
            "telegram_bot_token": config.telegram_bot_token,
            "telegram_discuss_link": config.telegram_discuss_link,
        },
        "app": {
            "windows_version": config.windows_version,
            "windows_download_url": config.windows_download_url,
            "macos_version": config.macos_version,
            "macos_download_url": config.macos_download_url,
            "android_version": config.android_version,
            "android_download_url": config.android_download_url,
        },
        "safe": {
            "email_verify": config.email_verify,
            "safe_mode_enable": config.safe_mode_enable,
            "admin_mfa_force": config.admin_mfa_force,
            "secure_path": config.admin_path(),
            "email_whitelist_enable": config.email_whitelist_enable,
            "email_whitelist_suffix": config.email_whitelist_suffix,
            "email_gmail_limit_enable": config.email_gmail_limit_enable,
            "recaptcha_enable": config.recaptcha_enable,
            "recaptcha_key": config.recaptcha_key,
            "recaptcha_site_key": config.recaptcha_site_key,
            "register_limit_by_ip_enable": config.register_limit_by_ip_enable,
            "register_limit_count": config.register_limit_count,
            "register_limit_expire": config.register_limit_expire,
            "password_limit_enable": config.password_limit_enable,
            "password_limit_count": config.password_limit_count,
            "password_limit_expire": config.password_limit_expire,
        },
    })
}
