use std::sync::Arc;

use chrono::Utc;
use sqlx::{Postgres, QueryBuilder};
use v2board_application::{
    admin_user::{UserFilterClause, UserFilterOperator, UserFilterValue},
    configuration::{
        BulkMailCommand, BulkMailRepository, ConfigurationCode, ConfigurationPortError,
        MailAudience,
    },
};
use v2board_config::AppConfig;
use v2board_db::{DbPool, admin_user::push_user_filters};
use v2board_mail_adapters::mail::outbox::{
    MailOutboxError, PreparedMailEnvelope, enqueue_prepared_mail, mail_batch_key,
    mail_payload_hash, reserve_mail_outbox_batch, validate_mail_sender,
};

const PAGE_SIZE: i64 = 500;

#[derive(Clone)]
pub struct PostgresBulkMailRepository {
    db: DbPool,
    config: Arc<AppConfig>,
}

impl PostgresBulkMailRepository {
    pub const fn new(db: DbPool, config: Arc<AppConfig>) -> Self {
        Self { db, config }
    }
}

impl BulkMailRepository for PostgresBulkMailRepository {
    async fn enqueue(&self, command: BulkMailCommand<'_>) -> Result<(), ConfigurationPortError> {
        let filters = command.filter.unwrap_or_default();
        let filter_identity = filters.iter().map(filter_identity).collect::<Vec<_>>();
        let payload_hash = mail_payload_hash(&(command.subject, command.content, &filter_identity));
        let batch_key = mail_batch_key(command.actor, command.idempotency_key);
        let now = Utc::now().timestamp();
        let mut tx = self
            .db
            .begin()
            .await
            .map_err(|error| ConfigurationPortError::Internal(error.to_string()))?;
        if !reserve_mail_outbox_batch(&mut tx, &batch_key, &payload_hash, command.actor, now)
            .await
            .map_err(mail_error)?
        {
            tx.commit()
                .await
                .map_err(|error| ConfigurationPortError::Internal(error.to_string()))?;
            return Ok(());
        }
        let envelope = prepare_notify_mail(&self.config, command.subject, command.content)?;
        let staff_scoped = command.audience == MailAudience::Staff;
        let mut after_id = 0_i64;
        let mut recipient_count = 0_usize;
        loop {
            let mut builder =
                QueryBuilder::<Postgres>::new("SELECT u.id, u.email FROM users u WHERE 1 = 1");
            if staff_scoped {
                builder.push(" AND u.is_admin = 0 AND u.is_staff = 0");
            }
            push_user_filters(&mut builder, filters);
            builder.push(" AND u.id > ");
            builder.push_bind(after_id);
            builder.push(" ORDER BY u.id LIMIT ");
            builder.push_bind(PAGE_SIZE);
            let recipients = builder
                .build_query_as::<(i64, String)>()
                .fetch_all(&mut *tx)
                .await
                .map_err(|error| ConfigurationPortError::Internal(error.to_string()))?;
            let Some(last_id) = recipients.last().map(|(id, _)| *id) else {
                break;
            };
            recipient_count = recipient_count.saturating_add(recipients.len());
            if recipient_count > command.maximum_recipients {
                return Err(ConfigurationPortError::Business {
                    code: ConfigurationCode::InvalidParameter,
                    detail: Some(format!(
                        "单次最多向 {} 个用户发送邮件，请缩小筛选范围",
                        command.maximum_recipients
                    )),
                });
            }
            let emails = recipients
                .into_iter()
                .map(|(_, email)| email)
                .collect::<Vec<_>>();
            enqueue_prepared_mail(&mut tx, &batch_key, &envelope, &emails, now)
                .await
                .map_err(mail_error)?;
            after_id = last_id;
        }
        tx.commit()
            .await
            .map_err(|error| ConfigurationPortError::Internal(error.to_string()))?;
        Ok(())
    }
}

fn prepare_notify_mail(
    config: &AppConfig,
    subject: &str,
    content: &str,
) -> Result<PreparedMailEnvelope, ConfigurationPortError> {
    if config
        .email_host
        .as_deref()
        .is_none_or(|host| host.trim().is_empty())
    {
        return Err(ConfigurationPortError::Business {
            code: ConfigurationCode::MailSenderNotConfigured,
            detail: Some("Email host is not configured".to_string()),
        });
    }
    let from = config
        .email_from_address
        .as_deref()
        .or(config.email_username.as_deref())
        .ok_or(ConfigurationPortError::Business {
            code: ConfigurationCode::MailSenderNotConfigured,
            detail: None,
        })?;
    let sender = format!("{} <{}>", config.app_name, from);
    validate_mail_sender(&sender).map_err(mail_error)?;
    Ok(PreparedMailEnvelope {
        sender,
        template_name: format!(
            "mail.{}.notify",
            config.email_template.as_deref().unwrap_or("default")
        ),
        subject: subject.to_string(),
        body: v2board_mail_adapters::mail::render_notify(
            &config.app_name,
            config.app_url.as_deref().unwrap_or_default(),
            content,
        ),
    })
}

fn mail_error(error: MailOutboxError) -> ConfigurationPortError {
    match error {
        MailOutboxError::IdempotencyConflict => ConfigurationPortError::Business {
            code: ConfigurationCode::MailIdempotencyConflict,
            detail: None,
        },
        MailOutboxError::Database(error) => ConfigurationPortError::Internal(error.to_string()),
        MailOutboxError::InvalidSender => {
            ConfigurationPortError::Internal("Email sender is invalid".to_string())
        }
        MailOutboxError::InvalidRecipient => {
            ConfigurationPortError::Internal("Email recipient is invalid".to_string())
        }
        MailOutboxError::InvalidContent => {
            ConfigurationPortError::Internal("Email content is invalid".to_string())
        }
        MailOutboxError::BatchLost => {
            ConfigurationPortError::Internal("mail outbox batch envelope was lost".to_string())
        }
    }
}

fn filter_identity(filter: &UserFilterClause) -> (&'static str, &'static str, String) {
    let operator = match filter.operator {
        UserFilterOperator::Eq => "eq",
        UserFilterOperator::Neq => "neq",
        UserFilterOperator::Like => "like",
        UserFilterOperator::Gt => "gt",
        UserFilterOperator::Gte => "gte",
        UserFilterOperator::Lt => "lt",
        UserFilterOperator::Lte => "lte",
        UserFilterOperator::In => "in",
    };
    let value = match &filter.value {
        UserFilterValue::Null => "null".to_string(),
        UserFilterValue::Boolean(value) => format!("bool:{value}"),
        UserFilterValue::Integer(value) => format!("integer:{value}"),
        UserFilterValue::Text(value) => {
            format!(
                "text:{}",
                serde_json::to_string(value).expect("string serializes")
            )
        }
        UserFilterValue::Booleans(values) => format!(
            "booleans:{}",
            serde_json::to_string(values).expect("booleans serialize")
        ),
        UserFilterValue::Integers(values) => format!(
            "integers:{}",
            serde_json::to_string(values).expect("integers serialize")
        ),
        UserFilterValue::Texts(values) => format!(
            "texts:{}",
            serde_json::to_string(values).expect("texts serialize")
        ),
    };
    (filter.field.name(), operator, value)
}
