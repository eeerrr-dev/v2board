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
use v2board_compat::ApiError;
use v2board_config::{
    AppConfig, MAX_CONFIG_DURATION_MINUTES, RedisKeyspace, app_now, app_timezone,
};
use v2board_db::{DbPool, DbTransaction};

use crate::payment_provider::{payment_provider_codes, payment_provider_form};
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

mod commerce;
mod configuration;
mod content;
mod repository;
mod servers;
mod statistics;
mod support;
mod users;

const REDIS_MGET_BATCH_SIZE: usize = 500;

/// The §7 filter/sort DSL (docs/api-dialect.md §7), shipped in W9 with
/// `GET system/logs` and reused by the W11/W12 admin list waves.
pub use support::filter_dsl;
use support::*;

pub use configuration::ConfigPatchOutcome;

const GIB: i64 = 1_073_741_824;

fn mail_outbox_api_error(error: MailOutboxError) -> ApiError {
    match error {
        MailOutboxError::Database(error) => ApiError::Database(error),
        MailOutboxError::IdempotencyConflict => {
            ApiError::bad_request("Mail idempotency key was reused with a different payload")
        }
        MailOutboxError::InvalidSender => ApiError::legacy("Email sender is invalid"),
        MailOutboxError::InvalidRecipient => ApiError::legacy("Email recipient is invalid"),
        MailOutboxError::InvalidContent => ApiError::legacy("Email content is invalid"),
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

/// Allowed `action` values for RouteController::save (`in:...` rule).
const ROUTE_ACTIONS: [&str; 8] = [
    "block",
    "block_ip",
    "block_port",
    "protocol",
    "dns",
    "route",
    "route_ip",
    "default_out",
];

/// Ports RouteController::save's `$request->validate([...])` (V1\Admin\Server),
/// returning HTTP 422 with the Chinese literal messages. Fields are checked in
/// Laravel's declaration order (remarks, match, action) so the reported message is
/// the first failing field. Returns `None` when the payload is valid.
fn route_save_validation(params: &HashMap<String, String>) -> Option<ApiError> {
    let action = optional_string(params, "action");
    // remarks => required
    if optional_string(params, "remarks").is_none() {
        return Some(ApiError::validation_field("remarks", "备注不能为空"));
    }
    // match => array|required_unless:action,default_out. `required` treats an absent
    // or empty array as missing (a non-empty array like ["0"] passes even though
    // array_filter later drops it), so check raw presence before filtering.
    if action.as_deref() != Some("default_out") && route_match_values(params).is_empty() {
        return Some(ApiError::validation_field("match", "匹配值不能为空"));
    }
    // action => required|in:block,block_ip,block_port,protocol,dns,route,route_ip,default_out
    match action.as_deref() {
        None => Some(ApiError::validation_field("action", "动作类型不能为空")),
        Some(value) if !ROUTE_ACTIONS.contains(&value) => {
            Some(ApiError::validation_field("action", "动作类型参数有误"))
        }
        Some(_) => None,
    }
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

pub enum AdminOutput {
    Data(Value),
    Page { data: Vec<Value>, total: i64 },
    Csv { filename: String, body: String },
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

    pub async fn get(
        &self,
        path: &str,
        params: HashMap<String, String>,
    ) -> Result<AdminOutput, ApiError> {
        let path = normalize_admin_path(path);
        match path.as_str() {
            "plan/fetch" => self.plan_fetch().await,
            "payment/fetch" => self.payment_fetch().await,
            "payment/getPaymentMethods" => Ok(AdminOutput::Data(json!(payment_provider_codes()))),
            "user/fetch" => self.user_fetch(&params).await,
            "user/getUserInfoById" => self.user_detail(required_i64(&params, "id")?).await,
            "order/fetch" => self.order_fetch(&params).await,
            "order/reconciliation/fetch" => self.payment_reconciliation_fetch(&params).await,
            "notice/fetch" => self.notice_fetch().await,
            "ticket/fetch" => self.ticket_fetch(&params, false).await,
            "coupon/fetch" => self.coupon_fetch(&params).await,
            "giftcard/fetch" => self.giftcard_fetch(&params).await,
            "knowledge/fetch" => self.knowledge_fetch(&params).await,
            "knowledge/getCategory" => self.knowledge_categories().await,
            "server/group/fetch" => self.server_group_fetch(&params).await,
            "server/route/fetch" => self.server_route_fetch().await,
            "server/manage/getNodes" => self.server_nodes().await,
            "stat/getStat" | "stat/getOverride" => self.stat_summary().await,
            "stat/getServerLastRank" => self.server_rank(false).await,
            "stat/getServerTodayRank" => self.server_rank(true).await,
            "stat/getUserLastRank" => self.user_rank(false).await,
            "stat/getUserTodayRank" => self.user_rank(true).await,
            "stat/getOrder" => self.order_stat().await,
            "stat/getStatUser" => self.stat_user(&params).await,
            "stat/getRanking" => self.stat_summary().await,
            "stat/getStatRecord" => self.stat_record(&params).await,
            _ => Err(ApiError::not_found("Admin endpoint does not exist")),
        }
    }

    pub async fn post(
        &self,
        path: &str,
        params: HashMap<String, String>,
    ) -> Result<AdminOutput, ApiError> {
        let path = normalize_admin_path(path);
        match path.as_str() {
            "plan/save" => self.plan_save(&params).await,
            "plan/drop" => self.plan_drop(&params).await,
            "plan/update" => self.plan_update(&params).await,
            "plan/sort" => {
                self.sort_ids("plan", &array_param(&params, "plan_ids")?)
                    .await
            }
            "payment/getPaymentForm" => self.payment_form(&params).await,
            "payment/save" => self.payment_save(&params).await,
            "payment/drop" => self.payment_drop(required_i64(&params, "id")?).await,
            "payment/show" => self.payment_show(required_i64(&params, "id")?).await,
            "payment/sort" => self.payment_sort(&array_param(&params, "ids")?).await,
            "notice/save" => self.notice_save(&params).await,
            "notice/update" => self.notice_update(&params).await,
            "notice/drop" => {
                self.delete_by_id(
                    "notice",
                    required_i64(&params, "id")?,
                    ApiError::business("公告不存在"),
                )
                .await
            }
            "notice/show" => {
                self.toggle(
                    "notice",
                    "show",
                    required_i64(&params, "id")?,
                    ApiError::business("公告不存在"),
                )
                .await
            }
            "knowledge/save" => self.knowledge_save(&params).await,
            "knowledge/drop" => {
                self.delete_by_id(
                    "knowledge",
                    required_i64(&params, "id")?,
                    ApiError::business("知识不存在"),
                )
                .await
            }
            "knowledge/show" => {
                self.toggle(
                    "knowledge",
                    "show",
                    required_i64(&params, "id")?,
                    ApiError::business("知识不存在"),
                )
                .await
            }
            "knowledge/sort" => {
                self.sort_ids("knowledge", &array_param(&params, "knowledge_ids")?)
                    .await
            }
            "ticket/reply" => self.ticket_reply(&params).await,
            "ticket/close" => self.ticket_close(required_i64(&params, "id")?).await,
            "coupon/generate" => self.coupon_generate(&params).await,
            "coupon/drop" => {
                self.delete_by_id(
                    "coupon",
                    required_i64(&params, "id")?,
                    ApiError::business("优惠券不存在"),
                )
                .await
            }
            "coupon/show" => {
                self.toggle(
                    "coupon",
                    "show",
                    required_i64(&params, "id")?,
                    ApiError::business("优惠券不存在"),
                )
                .await
            }
            "giftcard/generate" => self.giftcard_generate(&params).await,
            "giftcard/drop" => {
                self.delete_by_id(
                    "gift_card",
                    required_i64(&params, "id")?,
                    ApiError::not_found("礼品卡不存在"),
                )
                .await
            }
            "server/group/save" => self.server_group_save(&params).await,
            "server/group/drop" => self.server_group_drop(&params).await,
            "server/route/save" => self.server_route_save(&params).await,
            "server/route/drop" => {
                self.delete_by_id(
                    "server_route",
                    required_i64(&params, "id")?,
                    ApiError::business("路由不存在"),
                )
                .await
            }
            "server/manage/sort" => self.server_sort(&params).await,
            "order/detail" => self.order_detail(required_i64(&params, "id")?).await,
            "order/update" => self.order_update(&params).await,
            "order/paid" => self.order_paid(required_string(&params, "trade_no")?).await,
            "order/cancel" => {
                self.order_cancel(required_string(&params, "trade_no")?)
                    .await
            }
            "order/assign" => self.order_assign(&params).await,
            "user/update" => self.user_update(&params).await,
            "user/generate" => self.user_generate(&params).await,
            "user/dumpCSV" => self.user_dump_csv(&params).await,
            "user/sendMail" => self.send_mail_to_users(&params).await,
            "user/ban" => self.user_bulk_flag(&params, "banned", 1).await,
            "user/resetSecret" => self.user_reset_secret(required_i64(&params, "id")?).await,
            "user/delUser" => self.del_user(required_i64(&params, "id")?).await,
            "user/allDel" => self.user_bulk_delete(&params).await,
            "user/setInviteUser" => self.user_set_invite(&params).await,
            _ if is_server_path(&path, "save") => self.server_save(&path, &params).await,
            _ if is_server_path(&path, "drop") => self.server_drop(&path, &params).await,
            _ if is_server_path(&path, "update") => {
                let table = server_table_from_path(&path)?;
                self.toggle_or_set_show(
                    table,
                    required_i64(&params, "id")?,
                    &params,
                    ApiError::business("该服务器不存在"),
                )
                .await
            }
            _ if is_server_path(&path, "copy") => self.server_copy(&path, &params).await,
            _ => Err(ApiError::not_found("Admin endpoint does not exist")),
        }
    }

    pub async fn staff_get(
        &self,
        path: &str,
        params: HashMap<String, String>,
    ) -> Result<AdminOutput, ApiError> {
        let path = normalize_admin_path(path);
        match path.as_str() {
            "ticket/fetch" => self.ticket_fetch(&params, true).await,
            "user/getUserInfoById" => self.staff_user_detail(required_i64(&params, "id")?).await,
            "plan/fetch" => self.plan_fetch().await,
            "notice/fetch" => self.notice_fetch().await,
            _ => Err(ApiError::not_found("Staff endpoint does not exist")),
        }
    }

    pub async fn staff_post(
        &self,
        path: &str,
        params: HashMap<String, String>,
    ) -> Result<AdminOutput, ApiError> {
        let path = normalize_admin_path(path);
        match path.as_str() {
            "ticket/reply" => self.ticket_reply(&params).await,
            "ticket/close" => self.ticket_close(required_i64(&params, "id")?).await,
            "user/update" => self.staff_user_update(&params).await,
            "user/sendMail" => self.staff_send_mail_to_users(&params).await,
            "user/ban" => self.staff_user_bulk_ban(&params).await,
            "notice/save" => self.notice_save(&params).await,
            "notice/update" => self.notice_update(&params).await,
            "notice/drop" => {
                self.delete_by_id(
                    "notice",
                    required_i64(&params, "id")?,
                    ApiError::business("公告不存在"),
                )
                .await
            }
            _ => Err(ApiError::not_found("Staff endpoint does not exist")),
        }
    }
}

#[cfg(test)]
mod tests;
