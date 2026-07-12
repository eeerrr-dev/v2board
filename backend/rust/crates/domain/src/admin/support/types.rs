use super::*;

#[derive(FromRow)]
pub(in super::super) struct PaymentRow {
    pub(in super::super) id: i64,
    pub(in super::super) name: String,
    pub(in super::super) payment: String,
    pub(in super::super) icon: Option<String>,
    pub(in super::super) handling_fee_fixed: Option<i64>,
    pub(in super::super) handling_fee_percent: Option<f64>,
    pub(in super::super) uuid: String,
    pub(in super::super) config: String,
    pub(in super::super) notify_domain: Option<String>,
    pub(in super::super) enable: i8,
    pub(in super::super) sort: Option<i64>,
    pub(in super::super) created_at: i64,
    pub(in super::super) updated_at: i64,
}

#[derive(Debug, FromRow)]
pub(in super::super) struct NoticeRaw {
    id: i64,
    title: String,
    content: String,
    img_url: Option<String>,
    tags: Option<String>,
    show: i8,
    created_at: i64,
    updated_at: i64,
}

#[derive(Debug, Serialize)]
pub(in super::super) struct NoticeDto {
    id: i64,
    title: String,
    content: String,
    img_url: Option<String>,
    tags: Option<Vec<String>>,
    show: i8,
    created_at: i64,
    updated_at: i64,
}

impl From<NoticeRaw> for NoticeDto {
    fn from(row: NoticeRaw) -> Self {
        let tags = row.tags.and_then(|value| {
            serde_json::from_str::<Vec<String>>(&value)
                .ok()
                .or_else(|| (!value.trim().is_empty()).then_some(vec![value]))
        });
        Self {
            id: row.id,
            title: row.title,
            content: row.content,
            img_url: row.img_url,
            tags,
            show: row.show,
            created_at: row.created_at,
            updated_at: row.updated_at,
        }
    }
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
