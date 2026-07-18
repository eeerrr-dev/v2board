use super::*;

#[derive(FromRow)]
pub(in super::super) struct PaymentRow {
    pub(in super::super) id: i32,
    pub(in super::super) name: String,
    pub(in super::super) payment: String,
    pub(in super::super) icon: Option<String>,
    pub(in super::super) handling_fee_fixed: Option<i32>,
    pub(in super::super) handling_fee_percent: Option<f64>,
    pub(in super::super) uuid: String,
    pub(in super::super) config: String,
    pub(in super::super) notify_domain: Option<String>,
    pub(in super::super) enable: i16,
    pub(in super::super) sort: Option<i32>,
    pub(in super::super) created_at: i64,
    pub(in super::super) updated_at: i64,
}

pub(in super::super) struct MailSettings {
    pub(in super::super) host: String,
    pub(in super::super) port: Option<u16>,
    pub(in super::super) username: Option<String>,
    pub(in super::super) password: Option<String>,
    pub(in super::super) encryption: Option<String>,
    pub(in super::super) from_address: Option<String>,
}

impl MailSettings {
    pub(in super::super) fn load(config: &AppConfig) -> Result<Self, ApiError> {
        let host = config
            .email_host
            .clone()
            .ok_or_else(|| ApiError::internal("Email host is not configured"))?;
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
