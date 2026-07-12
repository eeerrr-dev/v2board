use std::{
    sync::{Arc, RwLock},
    time::Duration,
};

use lettre::{AsyncSmtpTransport, Tokio1Executor, transport::smtp::authentication::Credentials};
use v2board_compat::ApiError;
use v2board_config::AppConfig;

#[derive(Clone, PartialEq, Eq)]
pub struct SmtpSettings {
    pub host: String,
    pub port: Option<u16>,
    pub username: Option<String>,
    pub password: Option<String>,
    pub encryption: Option<String>,
    pub from_address: Option<String>,
}

impl SmtpSettings {
    pub fn load(config: &AppConfig) -> Result<Self, ApiError> {
        let host = config
            .email_host
            .clone()
            .filter(|host| !host.trim().is_empty())
            .ok_or_else(|| ApiError::legacy("Email host is not configured"))?;
        Ok(Self {
            host,
            port: config
                .email_port
                .and_then(|value| u16::try_from(value).ok()),
            username: config.email_username.clone(),
            password: config.email_password.clone(),
            encryption: config
                .email_encryption
                .as_deref()
                .map(str::to_ascii_lowercase),
            from_address: config.email_from_address.clone(),
        })
    }
}

#[derive(Clone, Default)]
pub struct SmtpTransportCache {
    current: Arc<RwLock<Option<CachedTransport>>>,
}

#[derive(Clone)]
struct CachedTransport {
    settings: SmtpSettings,
    transport: AsyncSmtpTransport<Tokio1Executor>,
}

impl SmtpTransportCache {
    pub fn transport(
        &self,
        settings: &SmtpSettings,
    ) -> Result<AsyncSmtpTransport<Tokio1Executor>, ApiError> {
        let mut current = self
            .current
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Some(cached) = current
            .as_ref()
            .filter(|cached| cached.settings == *settings)
        {
            return Ok(cached.transport.clone());
        }

        let transport = build_transport(settings)?;
        *current = Some(CachedTransport {
            settings: settings.clone(),
            transport: transport.clone(),
        });
        Ok(transport)
    }
}

fn build_transport(
    settings: &SmtpSettings,
) -> Result<AsyncSmtpTransport<Tokio1Executor>, ApiError> {
    let mut builder = if matches!(settings.encryption.as_deref(), Some("ssl" | "smtps")) {
        AsyncSmtpTransport::<Tokio1Executor>::relay(&settings.host)
    } else {
        AsyncSmtpTransport::<Tokio1Executor>::starttls_relay(&settings.host)
    }
    .map_err(|error| ApiError::legacy(format!("SMTP config error: {error}")))?;
    if let Some(port) = settings.port {
        builder = builder.port(port);
    }
    builder = builder.timeout(Some(Duration::from_secs(30)));
    if let (Some(username), Some(password)) = (&settings.username, &settings.password) {
        builder = builder.credentials(Credentials::new(username.clone(), password.clone()));
    }
    Ok(builder.build())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn cached_transport_is_reused_for_identical_settings() {
        let cache = SmtpTransportCache::default();
        let settings = SmtpSettings {
            host: "mail.example.test".to_string(),
            port: Some(587),
            username: Some("user".to_string()),
            password: Some("secret".to_string()),
            encryption: Some("starttls".to_string()),
            from_address: Some("sender@example.test".to_string()),
        };
        let first = cache.transport(&settings).unwrap();
        let cached_before =
            cache.current.read().unwrap().as_ref().unwrap() as *const CachedTransport;
        let second = cache.transport(&settings).unwrap();
        let cached_after =
            cache.current.read().unwrap().as_ref().unwrap() as *const CachedTransport;
        assert_eq!(cached_before, cached_after);
        drop((first, second));
    }
}
