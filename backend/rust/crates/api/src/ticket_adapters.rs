//! Infrastructure adapters used by the ticket application service.

use std::sync::Arc;

use uuid::Uuid;
use v2board_application::{
    RepositoryError,
    ticket::{
        DurableMailDelivery, NotificationReservation, OperatorReplyTarget,
        PreparedTicketNotification, TicketAdmissionRequest, TicketReplyNotifications,
        TicketWriteAdmission, TicketWriteKind,
    },
};
use v2board_config::{AppConfig, RedisKeyspace};
use v2board_mail_adapters::{
    mail::{
        outbox::{
            PreparedMailEnvelope, mail_batch_key, prepared_mail_payload_hash,
            validate_mail_recipient, validate_mail_sender, validate_prepared_mail_delivery,
        },
        render_notify,
    },
    smtp::SmtpSettings,
};
use v2board_redis_adapters::reserve_fixed_window_slot;

const NOTIFICATION_GATE_TTL_SECONDS: u64 = 1_800;
const NOTIFICATION_GATE_RELEASE_SCRIPT: &str = r#"
if redis.call("GET", KEYS[1]) == ARGV[1] then
    return redis.call("DEL", KEYS[1])
end
return 0
"#;

#[derive(Clone)]
pub(crate) struct RedisTicketAdmission {
    connection: redis::aio::ConnectionManager,
    keyspace: RedisKeyspace,
}

impl RedisTicketAdmission {
    pub(crate) fn new(connection: redis::aio::ConnectionManager, keyspace: RedisKeyspace) -> Self {
        Self {
            connection,
            keyspace,
        }
    }
}

impl TicketWriteAdmission for RedisTicketAdmission {
    async fn reserve(&self, request: TicketAdmissionRequest) -> Result<bool, RepositoryError> {
        let action = match request.kind {
            TicketWriteKind::Create => "CREATE",
            TicketWriteKind::Reply => "REPLY",
        };
        let key = self
            .keyspace
            .key(&format!("TICKET_WRITE_LIMIT_{action}_{}", request.user_id));
        let mut connection = self.connection.clone();
        reserve_fixed_window_slot(&mut connection, &key, request.limit, request.window_seconds)
            .await
            .map_err(|error| RepositoryError::new("ticket.admission", error))
    }
}

#[derive(Clone)]
pub(crate) struct TicketEmailNotifications {
    redis: redis::Client,
    keyspace: RedisKeyspace,
    config: Arc<AppConfig>,
}

impl TicketEmailNotifications {
    pub(crate) fn new(
        redis: redis::Client,
        keyspace: RedisKeyspace,
        config: Arc<AppConfig>,
    ) -> Self {
        Self {
            redis,
            keyspace,
            config,
        }
    }

    fn envelope(
        &self,
        target: &OperatorReplyTarget,
        message: &str,
    ) -> Option<PreparedMailEnvelope> {
        if let Err(error) = validate_mail_recipient(&target.recipient_email) {
            tracing::warn!(
                ?error,
                user_id = target.user_id,
                "ticket reply notification recipient invalid"
            );
            return None;
        }
        let settings = match SmtpSettings::load(&self.config) {
            Ok(settings) => settings,
            Err(error) => {
                tracing::warn!(
                    ?error,
                    user_id = target.user_id,
                    "ticket reply notification mail configuration invalid"
                );
                return None;
            }
        };
        let Some(from) = settings
            .from_address
            .as_deref()
            .or(settings.username.as_deref())
        else {
            tracing::warn!(
                user_id = target.user_id,
                "ticket reply notification sender is not configured"
            );
            return None;
        };
        let sender = format!("{} <{from}>", self.config.app_name);
        if let Err(error) = validate_mail_sender(&sender) {
            tracing::warn!(
                ?error,
                user_id = target.user_id,
                "ticket reply notification sender invalid"
            );
            return None;
        }
        let content = format!("主题：{}\r\n回复内容：{message}", target.subject);
        Some(PreparedMailEnvelope {
            sender,
            template_name: format!(
                "mail.{}.notify",
                self.config.email_template.as_deref().unwrap_or("default")
            ),
            subject: format!("您在{}的工单得到了回复", self.config.app_name),
            body: render_notify(
                &self.config.app_name,
                self.config.app_url.as_deref().unwrap_or_default(),
                &content,
            ),
        })
    }
}

impl TicketReplyNotifications for TicketEmailNotifications {
    async fn prepare(
        &self,
        target: &OperatorReplyTarget,
        message: &str,
    ) -> Option<PreparedTicketNotification> {
        let envelope = self.envelope(target, message)?;
        let key = self
            .keyspace
            .key(&format!("ticket_sendEmailNotify_{}", target.user_id));
        let token = Uuid::new_v4().to_string();
        let mut connection = match self.redis.get_multiplexed_async_connection().await {
            Ok(connection) => connection,
            Err(error) => {
                tracing::warn!(
                    ?error,
                    user_id = target.user_id,
                    "ticket reply notification Redis unavailable"
                );
                return None;
            }
        };
        let acquired: Result<Option<String>, redis::RedisError> = redis::cmd("SET")
            .arg(&key)
            .arg(&token)
            .arg("NX")
            .arg("EX")
            .arg(NOTIFICATION_GATE_TTL_SECONDS)
            .query_async(&mut connection)
            .await;
        match acquired {
            Ok(Some(_)) => {}
            Ok(None) => return None,
            Err(error) => {
                tracing::warn!(
                    ?error,
                    user_id = target.user_id,
                    "ticket reply notification reservation failed"
                );
                return None;
            }
        }

        let actor = format!("ticket:{}", target.user_id);
        let batch_key = mail_batch_key(&actor, &Uuid::new_v4().to_string());
        let recipients = vec![target.recipient_email.clone()];
        let payload_hash = prepared_mail_payload_hash(&envelope, &recipients);
        let message_id =
            match validate_prepared_mail_delivery(&batch_key, &envelope, &target.recipient_email) {
                Ok(message_id) => message_id,
                Err(error) => {
                    tracing::warn!(
                        ?error,
                        user_id = target.user_id,
                        "ticket reply notification envelope invalid"
                    );
                    self.release(&NotificationReservation { key, token }).await;
                    return None;
                }
            };
        Some(PreparedTicketNotification {
            delivery: DurableMailDelivery {
                batch_key,
                payload_hash,
                actor,
                recipient: target.recipient_email.clone(),
                message_id,
                sender: envelope.sender,
                template_name: envelope.template_name,
                subject: envelope.subject,
                body: envelope.body,
            },
            reservation: NotificationReservation { key, token },
        })
    }

    async fn release(&self, reservation: &NotificationReservation) {
        let mut connection = match self.redis.get_multiplexed_async_connection().await {
            Ok(connection) => connection,
            Err(error) => {
                tracing::warn!(
                    ?error,
                    key = %reservation.key,
                    "ticket reply notification reservation release failed"
                );
                return;
            }
        };
        let released: Result<i64, redis::RedisError> =
            redis::Script::new(NOTIFICATION_GATE_RELEASE_SCRIPT)
                .key(&reservation.key)
                .arg(&reservation.token)
                .invoke_async(&mut connection)
                .await;
        match released {
            Ok(1) => {}
            Ok(_) => tracing::warn!(
                key = %reservation.key,
                "ticket reply notification reservation ownership changed before release"
            ),
            Err(error) => tracing::warn!(
                ?error,
                key = %reservation.key,
                "ticket reply notification reservation release failed"
            ),
        }
    }
}
