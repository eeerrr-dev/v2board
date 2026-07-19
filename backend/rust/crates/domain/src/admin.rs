use std::{
    collections::{BTreeMap, HashMap, HashSet},
    sync::Arc,
};

use chrono::{Datelike, TimeZone, Utc};
use hmac::{Hmac, KeyInit, Mac};
use lettre::{AsyncTransport, Message, message::header::ContentType};
use openssl::pkey::PKey;
use redis::AsyncCommands;
use rust_decimal::Decimal;
use serde::Serialize;
use serde_json::{Map, Value, json};
use sha2::{Digest, Sha256};
use sqlx::{AssertSqlSafe, FromRow, Postgres, QueryBuilder, types::Json};
use uuid::Uuid;
use v2board_compat::{ApiError, Code, Problem};
use v2board_config::{
    AppConfig, MAX_CONFIG_DURATION_MINUTES, RedisKeyspace, app_now, app_timezone,
};
use v2board_db::{DbPool, DbTransaction};

use crate::payment_provider::payment_provider_form;
use crate::{
    auth::PasswordKdf,
    mail::outbox::{
        MailOutboxError, PreparedMailEnvelope, enqueue_prepared_mail, mail_batch_key,
        mail_payload_hash as hash_mail_payload, prepared_mail_payload_hash,
        reserve_mail_outbox_batch, validate_mail_recipient, validate_mail_sender,
    },
    operator_config,
    order::{OrderService, commission_amount_cents},
    smtp::{SmtpSettings, SmtpTransportCache},
};

mod codes;
mod commerce;
mod configuration;
mod content;
mod repository;
mod servers;
mod statistics;
mod support;
mod tickets;
mod users;

const REDIS_MGET_BATCH_SIZE: usize = 500;

/// The §7 filter/sort DSL (docs/api-dialect.md §7), shipped in W9 with
/// `GET system/logs` and reused by the W11/W12 admin list waves.
pub use support::filter_dsl;
use support::*;

pub use codes::{
    AdminCouponItem, AdminGiftcardItem, ContentGenerateOutcome, CouponGenerate, CouponPatch,
    GiftcardGenerate, GiftcardPatch,
};
pub use commerce::{
    AdminPaymentItem, AdminPlanItem, OrderAssign, OrderPatch, PaymentCreate, PaymentPatch,
    PlanCreate, PlanPatch, ReconciliationResolveRequest, SortIdsRequest,
};
pub use configuration::ConfigPatchOutcome;
pub use content::{
    AdminKnowledgeDetail, AdminKnowledgeSummary, AdminNoticeItem, KnowledgeCreate, KnowledgePatch,
    KnowledgeSortRequest, NoticeCreate, NoticePatch,
};
pub use servers::{RouteCreate, RoutePatch, ServerBody, ServerGroupBody};
pub use users::{
    AdminSetInviterBody, AdminUserFilterBody, AdminUserGenerate, AdminUserMailBody, AdminUserPatch,
    StaffUserPatch, UserGenerateOutcome,
};

const GIB: i64 = 1_073_741_824;

fn mail_outbox_api_error(error: MailOutboxError) -> ApiError {
    match error {
        MailOutboxError::Database(error) => ApiError::Database(error),
        MailOutboxError::IdempotencyConflict => {
            ApiError::from(Problem::new(Code::MailIdempotencyConflict))
        }
        // W14 teardown: these mail-envelope failures are operator
        // misconfiguration on internal routes — 500 `internal_error`
        // problems, no longer minted through the legacy constructor.
        MailOutboxError::InvalidSender => ApiError::internal("Email sender is invalid"),
        MailOutboxError::InvalidRecipient => ApiError::internal("Email recipient is invalid"),
        MailOutboxError::InvalidContent => ApiError::internal("Email content is invalid"),
        MailOutboxError::BatchLost => ApiError::internal("mail outbox batch envelope was lost"),
    }
}

/// Telegram accepts 1-256 ASCII letters, digits, `_`, and `-` for its webhook secret header.
/// A keyed digest keeps the bot token out of both the public callback URL and access logs.
pub fn telegram_webhook_secret(app_key: &str, bot_token: &str) -> String {
    let mut mac = <Hmac<Sha256> as KeyInit>::new_from_slice(app_key.as_bytes())
        .expect("HMAC accepts keys of any length");
    mac.update(bot_token.as_bytes());
    hex::encode(mac.finalize().into_bytes())
}

fn payment_verification_version_blocks_update(driver_changed: bool, config_changed: bool) -> bool {
    driver_changed || config_changed
}

#[derive(Clone)]
pub struct AdminService {
    db: DbPool,
    installation_id: Uuid,
    redis_keys: RedisKeyspace,
    redis: redis::Client,
    config: Arc<AppConfig>,
    http: reqwest::Client,
    password_kdf: PasswordKdf,
    smtp: SmtpTransportCache,
}

impl AdminService {
    pub fn new(
        db: DbPool,
        redis: redis::Client,
        installation_id: Uuid,
        config: Arc<AppConfig>,
        http: reqwest::Client,
        password_kdf: PasswordKdf,
        smtp: SmtpTransportCache,
    ) -> Self {
        Self {
            db,
            installation_id,
            redis_keys: RedisKeyspace::new(installation_id),
            redis,
            config,
            http,
            password_kdf,
            smtp,
        }
    }

    pub(super) fn redis_key(&self, logical_key: &str) -> String {
        self.redis_keys.key(logical_key)
    }
}

#[cfg(test)]
mod tests;
