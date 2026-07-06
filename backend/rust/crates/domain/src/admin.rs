use std::collections::{BTreeMap, HashMap, HashSet};

use chrono::{Datelike, Local, TimeZone, Utc};
use lettre::{
    AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor, message::header::ContentType,
    transport::smtp::authentication::Credentials,
};
use openssl::pkey::PKey;
use redis::AsyncCommands;
use serde::Serialize;
use serde_json::{Map, Value, json};
use sqlx::{AssertSqlSafe, FromRow, MySql, MySqlPool, QueryBuilder, types::Json};
use uuid::Uuid;
use v2board_compat::ApiError;
use v2board_config::AppConfig;

use crate::order::OrderService;
use crate::payment_provider::{
    payment_provider_codes, payment_provider_form, payment_provider_manifest,
};

mod support;

use support::*;

const GIB: i64 = 1_073_741_824;

#[derive(Clone)]
pub struct AdminService {
    db: MySqlPool,
    redis: redis::Client,
    config: AppConfig,
}

#[derive(Debug)]
pub enum AdminOutput {
    Data(Value),
    Page { data: Vec<Value>, total: i64 },
    Csv { filename: String, body: String },
}

#[derive(Debug, Default)]
struct WorkerSnapshot {
    schedule_last_seen_at: Option<i64>,
    totals: BTreeMap<String, i64>,
    failed: BTreeMap<String, i64>,
    last_run_at: BTreeMap<String, i64>,
    last_success_at: BTreeMap<String, i64>,
    last_failure_at: BTreeMap<String, i64>,
}

impl WorkerSnapshot {
    fn total_jobs(&self) -> i64 {
        self.totals.values().sum()
    }

    fn failed_jobs(&self) -> i64 {
        self.failed.values().sum()
    }

    fn last_seen_at(&self) -> Option<i64> {
        self.schedule_last_seen_at
            .into_iter()
            .chain(self.last_run_at.values().copied())
            .max()
    }

    fn worker_running(&self, now: i64, seconds: i64) -> bool {
        self.last_seen_at()
            .map(|last_seen| now - last_seen <= seconds)
            .unwrap_or(false)
    }

    fn max_counter_key(&self) -> Option<String> {
        self.totals
            .iter()
            .max_by_key(|(_, value)| *value)
            .map(|(key, _)| key.clone())
    }

    fn job_names(&self) -> Vec<String> {
        let mut names = self
            .totals
            .keys()
            .chain(self.failed.keys())
            .chain(self.last_run_at.keys())
            .cloned()
            .collect::<HashSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();
        names.sort();
        names
    }
}

impl AdminService {
    pub fn new(db: MySqlPool, redis: redis::Client, config: AppConfig) -> Self {
        Self { db, redis, config }
    }

    pub async fn get(
        &self,
        path: &str,
        params: HashMap<String, String>,
    ) -> Result<AdminOutput, ApiError> {
        let path = normalize_admin_path(path);
        match path.as_str() {
            "config/fetch" => self.config_fetch(params.get("key").map(String::as_str)),
            "config/getEmailTemplate" => Ok(AdminOutput::Data(json!(list_names(
                &self.config.runtime_paths.mail_templates
            )))),
            "config/getThemeTemplate" => Ok(AdminOutput::Data(json!(list_names(
                &self.config.runtime_paths.themes
            )))),
            "plan/fetch" => self.plan_fetch().await,
            "payment/fetch" => self.payment_fetch().await,
            "payment/getPaymentMethods" => Ok(AdminOutput::Data(json!(payment_provider_codes()))),
            "user/fetch" => self.user_fetch(&params).await,
            "user/getUserInfoById" => self.user_detail(required_i64(&params, "id")?).await,
            "order/fetch" => self.order_fetch(&params).await,
            "notice/fetch" => self.notice_fetch().await,
            "ticket/fetch" => self.ticket_fetch(&params).await,
            "coupon/fetch" => self.coupon_fetch(&params).await,
            "giftcard/fetch" => self.giftcard_fetch(&params).await,
            "knowledge/fetch" => self.knowledge_fetch(&params).await,
            "knowledge/getCategory" => self.knowledge_categories().await,
            "server/group/fetch" => self.server_group_fetch().await,
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
            "system/getSystemStatus" => self.system_status().await,
            "system/getQueueStats" => self.queue_stats().await,
            "system/getQueueWorkload" => self.queue_workload().await,
            "system/getQueueMasters" => self.queue_masters().await,
            "system/getSystemLog" => self.system_log(&params).await,
            "theme/getThemes" => self.themes().await,
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
            "config/save" => self.config_save(&params).await,
            "config/setTelegramWebhook" => self.set_telegram_webhook(&params).await,
            "config/testSendMail" => self.test_send_mail(&params).await,
            "plan/save" => self.plan_save(&params).await,
            "plan/drop" => self.plan_drop(&params).await,
            "plan/update" => self.plan_update(&params).await,
            "plan/sort" => {
                self.sort_ids("v2_plan", &array_param(&params, "plan_ids")?)
                    .await
            }
            "payment/getPaymentForm" => self.payment_form(&params).await,
            "payment/save" => self.payment_save(&params).await,
            "payment/drop" => {
                self.delete_by_id("v2_payment", required_i64(&params, "id")?)
                    .await
            }
            "payment/show" => {
                self.toggle("v2_payment", "enable", required_i64(&params, "id")?)
                    .await
            }
            "payment/sort" => {
                self.sort_ids("v2_payment", &array_param(&params, "ids")?)
                    .await
            }
            "notice/save" => self.notice_save(&params).await,
            "notice/update" => self.notice_update(&params).await,
            "notice/drop" => {
                self.delete_by_id("v2_notice", required_i64(&params, "id")?)
                    .await
            }
            "notice/show" => {
                self.toggle("v2_notice", "show", required_i64(&params, "id")?)
                    .await
            }
            "knowledge/save" => self.knowledge_save(&params).await,
            "knowledge/drop" => {
                self.delete_by_id("v2_knowledge", required_i64(&params, "id")?)
                    .await
            }
            "knowledge/show" => {
                self.toggle("v2_knowledge", "show", required_i64(&params, "id")?)
                    .await
            }
            "knowledge/sort" => {
                self.sort_ids("v2_knowledge", &array_param(&params, "knowledge_ids")?)
                    .await
            }
            "ticket/reply" => self.ticket_reply(&params).await,
            "ticket/close" => self.ticket_close(required_i64(&params, "id")?).await,
            "coupon/generate" => self.coupon_generate(&params).await,
            "coupon/drop" => {
                self.delete_by_id("v2_coupon", required_i64(&params, "id")?)
                    .await
            }
            "coupon/show" => {
                self.toggle("v2_coupon", "show", required_i64(&params, "id")?)
                    .await
            }
            "giftcard/generate" => self.giftcard_generate(&params).await,
            "giftcard/drop" => {
                self.delete_by_id("v2_giftcard", required_i64(&params, "id")?)
                    .await
            }
            "server/group/save" => self.server_group_save(&params).await,
            "server/group/drop" => self.server_group_drop(&params).await,
            "server/route/save" => self.server_route_save(&params).await,
            "server/route/drop" => {
                self.delete_by_id("v2_server_route", required_i64(&params, "id")?)
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
            "theme/getThemeConfig" => self.theme_config(required_string(&params, "name")?).await,
            "theme/saveThemeConfig" => self.theme_save(&params).await,
            _ if is_server_path(&path, "save") => self.server_save(&path, &params).await,
            _ if is_server_path(&path, "drop") => {
                let table = server_table_from_path(&path)?;
                self.delete_by_id(table, required_i64(&params, "id")?).await
            }
            _ if is_server_path(&path, "update") => {
                let table = server_table_from_path(&path)?;
                self.toggle_or_set_show(table, required_i64(&params, "id")?, &params)
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
            "ticket/fetch" => self.ticket_fetch(&params).await,
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
                self.delete_by_id("v2_notice", required_i64(&params, "id")?)
                    .await
            }
            _ => Err(ApiError::not_found("Staff endpoint does not exist")),
        }
    }

    fn config_fetch(&self, key: Option<&str>) -> Result<AdminOutput, ApiError> {
        let data = json!({
            "ticket": { "ticket_status": self.config.ticket_status },
            "deposit": { "deposit_bounus": self.config.deposit_bounus },
            "invite": {
                "invite_force": bool_i(self.config.invite_force),
                "invite_commission": self.config.invite_commission,
                "invite_gen_limit": self.config.invite_gen_limit,
                // Ported from ConfigController::fetch (laravel .../Admin/ConfigController.php:82).
                "invite_never_expire": bool_i(self.config.invite_never_expire),
                "commission_first_time_enable": bool_i(self.config.commission_first_time_enable),
                "commission_auto_check_enable": bool_i(self.config.commission_auto_check_enable),
                "commission_withdraw_limit": self.config.commission_withdraw_limit,
                "commission_withdraw_method": self.config.commission_withdraw_method,
                "withdraw_close_enable": bool_i(self.config.withdraw_close_enable),
                "commission_distribution_enable": bool_i(self.config.commission_distribution_enable),
                "commission_distribution_l1": self.config.commission_distribution_l1,
                "commission_distribution_l2": self.config.commission_distribution_l2,
                "commission_distribution_l3": self.config.commission_distribution_l3,
            },
            "site": {
                "logo": self.config.logo,
                "force_https": bool_i(self.config.force_https),
                "stop_register": bool_i(self.config.stop_register),
                "app_name": self.config.app_name,
                "app_description": self.config.app_description,
                "app_url": self.config.app_url,
                "subscribe_url": self.config.subscribe_url,
                "subscribe_path": self.config.subscribe_path,
                "try_out_plan_id": self.config.try_out_plan_id,
                // Ported from ConfigController::fetch (laravel .../Admin/ConfigController.php:103).
                "try_out_hour": self.config.try_out_hour,
                "tos_url": self.config.tos_url,
                "currency": self.config.currency,
                "currency_symbol": self.config.currency_symbol,
            },
            "subscribe": {
                "plan_change_enable": bool_i(self.config.plan_change_enable),
                "reset_traffic_method": self.config.reset_traffic_method,
                "surplus_enable": bool_i(self.config.surplus_enable),
                "allow_new_period": self.config.allow_new_period,
                "new_order_event_id": self.config.new_order_event_id,
                "renew_order_event_id": self.config.renew_order_event_id,
                "change_order_event_id": self.config.change_order_event_id,
                "show_info_to_server_enable": bool_i(self.config.show_info_to_server_enable),
                "show_subscribe_method": self.config.show_subscribe_method,
                "show_subscribe_expire": self.config.show_subscribe_expire,
            },
            "frontend": {
                "frontend_theme": self.config.frontend_theme,
                "frontend_theme_sidebar": self.config.frontend_theme_sidebar,
                "frontend_theme_header": self.config.frontend_theme_header,
                "frontend_theme_color": self.config.frontend_theme_color,
                "frontend_background_url": self.config.frontend_background_url,
            },
            "server": {
                "server_api_url": self.config.server_api_url,
                "server_token": self.config.server_token,
                "server_pull_interval": self.config.server_pull_interval,
                "server_push_interval": self.config.server_push_interval,
                "server_node_report_min_traffic": self.config.server_node_report_min_traffic,
                "server_device_online_min_traffic": self.config.server_device_online_min_traffic,
                "device_limit_mode": self.config.device_limit_mode,
            },
            "email": {
                "email_template": self.config.email_template,
                "email_host": self.config.email_host,
                "email_port": self.config.email_port,
                "email_username": self.config.email_username,
                "email_password": self.config.email_password,
                "email_encryption": self.config.email_encryption,
                "email_from_address": self.config.email_from_address,
            },
            "telegram": {
                "telegram_bot_enable": bool_i(self.config.telegram_bot_enable),
                "telegram_bot_token": self.config.telegram_bot_token,
                "telegram_discuss_link": self.config.telegram_discuss_link,
            },
            "app": {
                "windows_version": self.config.windows_version,
                "windows_download_url": self.config.windows_download_url,
                "macos_version": self.config.macos_version,
                "macos_download_url": self.config.macos_download_url,
                "android_version": self.config.android_version,
                "android_download_url": self.config.android_download_url,
            },
            "safe": {
                "email_verify": bool_i(self.config.email_verify),
                "safe_mode_enable": bool_i(self.config.safe_mode_enable),
                "secure_path": self.config.admin_path(),
                "email_whitelist_enable": bool_i(self.config.email_whitelist_enable),
                "email_whitelist_suffix": self.config.email_whitelist_suffix,
                "email_gmail_limit_enable": bool_i(self.config.email_gmail_limit_enable),
                "recaptcha_enable": bool_i(self.config.recaptcha_enable),
                "recaptcha_key": self.config.recaptcha_key,
                "recaptcha_site_key": self.config.recaptcha_site_key,
                "register_limit_by_ip_enable": bool_i(self.config.register_limit_by_ip_enable),
                "register_limit_count": self.config.register_limit_count,
                "register_limit_expire": self.config.register_limit_expire,
                "password_limit_enable": bool_i(self.config.password_limit_enable),
                "password_limit_count": self.config.password_limit_count,
                "password_limit_expire": self.config.password_limit_expire,
            },
        });
        if let Some(key) = key
            && let Some(value) = data.get(key)
        {
            return Ok(AdminOutput::Data(json!({ key: value })));
        }
        Ok(AdminOutput::Data(data))
    }

    async fn config_save(&self, params: &HashMap<String, String>) -> Result<AdminOutput, ApiError> {
        let path = &self.config.runtime_paths.v2board_config;
        let mut config = read_php_config(path);
        merge_config_params(&mut config, params);
        write_php_config(path, &Value::Object(config))?;
        Ok(AdminOutput::Data(json!(true)))
    }

    async fn test_send_mail(
        &self,
        params: &HashMap<String, String>,
    ) -> Result<AdminOutput, ApiError> {
        let to = required_string(params, "_admin_email")?;
        self.send_mail(
            &to,
            "This is v2board test email",
            "This is v2board test email",
        )
        .await?;
        Ok(AdminOutput::Data(json!(true)))
    }

    async fn send_mail_to_users(
        &self,
        params: &HashMap<String, String>,
    ) -> Result<AdminOutput, ApiError> {
        let subject = required_string(params, "subject")?;
        let content = required_string(params, "content")?;
        let emails = self.filtered_user_emails(params, false).await?;
        for email in emails {
            self.send_mail(&email, &subject, &content).await?;
        }
        Ok(AdminOutput::Data(json!(true)))
    }

    async fn send_mail(&self, to: &str, subject: &str, content: &str) -> Result<(), ApiError> {
        let settings = MailSettings::load(&self.config)?;
        let from = settings
            .from_address
            .as_deref()
            .or(settings.username.as_deref())
            .ok_or_else(|| ApiError::legacy("Email sender is not configured"))?;
        // Admin/staff/test mail all use the `notify` HTML template (SendEmailJob template_name).
        let body = crate::mail::render_notify(
            &self.config.app_name,
            self.config.app_url.as_deref().unwrap_or_default(),
            content,
        );
        let email = Message::builder()
            .from(
                format!("{} <{}>", self.config.app_name, from)
                    .parse()
                    .map_err(|_| ApiError::legacy("Email sender is invalid"))?,
            )
            .to(to
                .parse()
                .map_err(|_| ApiError::legacy("Email recipient is invalid"))?)
            .subject(subject)
            .header(ContentType::TEXT_HTML)
            .body(body)
            .map_err(|_| ApiError::legacy("Email content is invalid"))?;

        let mut builder = if settings.encryption.as_deref() == Some("ssl") {
            AsyncSmtpTransport::<Tokio1Executor>::relay(&settings.host)
                .map_err(|_| ApiError::legacy("Email host is invalid"))?
        } else {
            AsyncSmtpTransport::<Tokio1Executor>::starttls_relay(&settings.host)
                .map_err(|_| ApiError::legacy("Email host is invalid"))?
        };
        if let Some(port) = settings.port {
            builder = builder.port(port);
        }
        if let (Some(username), Some(password)) = (settings.username, settings.password) {
            builder = builder.credentials(Credentials::new(username, password));
        }
        builder
            .build()
            .send(email)
            .await
            .map_err(|error| ApiError::legacy(format!("Email send failed: {error}")))?;
        Ok(())
    }

    async fn set_telegram_webhook(
        &self,
        params: &HashMap<String, String>,
    ) -> Result<AdminOutput, ApiError> {
        let token = params
            .get("telegram_bot_token")
            .filter(|value| !value.trim().is_empty())
            .ok_or_else(|| ApiError::legacy("Telegram bot token cannot be empty"))?;
        let hook_url = format!(
            "{}/api/v1/guest/telegram/webhook?access_token={:x}",
            self.config
                .app_url
                .as_deref()
                .unwrap_or_default()
                .trim_end_matches('/'),
            md5::compute(token)
        );
        let client = reqwest::Client::builder()
            .build()
            .map_err(|_| ApiError::internal("failed to build telegram client"))?;
        let me = client
            .get(format!("https://api.telegram.org/bot{token}/getMe"))
            .send()
            .await
            .map_err(|_| ApiError::legacy("Telegram request failed"))?
            .json::<Value>()
            .await
            .map_err(|_| ApiError::legacy("Telegram request failed"))?;
        if me.get("ok").and_then(Value::as_bool) != Some(true) {
            return Err(ApiError::legacy("Telegram token is invalid"));
        }
        let result = client
            .post(format!("https://api.telegram.org/bot{token}/setWebhook"))
            .json(&json!({ "url": hook_url }))
            .send()
            .await
            .map_err(|_| ApiError::legacy("Telegram request failed"))?
            .json::<Value>()
            .await
            .map_err(|_| ApiError::legacy("Telegram request failed"))?;
        if result.get("ok").and_then(Value::as_bool) != Some(true) {
            return Err(ApiError::legacy("Telegram webhook failed"));
        }
        Ok(AdminOutput::Data(json!(true)))
    }

    async fn plan_fetch(&self) -> Result<AdminOutput, ApiError> {
        let mut plans = v2board_db::plan::fetch_visible_plans(&self.db).await?;
        let shown_ids = plans.iter().map(|plan| plan.id).collect::<HashSet<_>>();
        let mut hidden = sqlx::query_as::<_, v2board_db::plan::PlanRow>(
            r#"
            SELECT id, group_id, transfer_enable, device_limit, name, speed_limit, `show`, sort,
                   renew, content, month_price, quarter_price, half_year_price, year_price,
                   two_year_price, three_year_price, onetime_price, reset_price,
                   reset_traffic_method, capacity_limit, created_at, updated_at
            FROM v2_plan
            WHERE `show` = 0
            ORDER BY sort ASC
            "#,
        )
        .fetch_all(&self.db)
        .await?;
        plans.append(&mut hidden);
        let counts = v2board_db::plan::count_active_users_by_plan(&self.db).await?;
        let mut data = Vec::with_capacity(plans.len());
        for plan in plans {
            let mut value = serde_json::to_value(&plan)
                .map_err(|_| ApiError::internal("failed to encode plan"))?;
            value["count"] = json!(counts.get(&plan.id).copied().unwrap_or_default());
            if !shown_ids.contains(&plan.id) {
                value["show"] = json!(0);
            }
            data.push(value);
        }
        data.sort_by_key(|value| {
            value
                .get("sort")
                .and_then(Value::as_i64)
                .unwrap_or_default()
        });
        Ok(AdminOutput::Data(json!(data)))
    }

    async fn plan_save(&self, params: &HashMap<String, String>) -> Result<AdminOutput, ApiError> {
        let now = Utc::now().timestamp();
        let id = optional_i64(params, "id");
        if let Some(id) = id {
            sqlx::query(
                r#"
                UPDATE v2_plan
                SET group_id = ?, transfer_enable = ?, device_limit = ?, name = ?,
                    speed_limit = ?, `show` = ?, renew = ?, content = ?,
                    month_price = ?, quarter_price = ?, half_year_price = ?, year_price = ?,
                    two_year_price = ?, three_year_price = ?, onetime_price = ?, reset_price = ?,
                    reset_traffic_method = ?, capacity_limit = ?, updated_at = ?
                WHERE id = ?
                "#,
            )
            .bind(required_i64(params, "group_id")?)
            .bind(required_i64(params, "transfer_enable")?)
            .bind(optional_i64(params, "device_limit"))
            .bind(required_string(params, "name")?)
            .bind(optional_i64(params, "speed_limit"))
            .bind(optional_i64(params, "show").unwrap_or(1))
            .bind(optional_i64(params, "renew").unwrap_or(1))
            .bind(params.get("content"))
            .bind(optional_i64(params, "month_price"))
            .bind(optional_i64(params, "quarter_price"))
            .bind(optional_i64(params, "half_year_price"))
            .bind(optional_i64(params, "year_price"))
            .bind(optional_i64(params, "two_year_price"))
            .bind(optional_i64(params, "three_year_price"))
            .bind(optional_i64(params, "onetime_price"))
            .bind(optional_i64(params, "reset_price"))
            .bind(optional_i64(params, "reset_traffic_method"))
            .bind(optional_i64(params, "capacity_limit"))
            .bind(now)
            .bind(id)
            .execute(&self.db)
            .await?;
            if truthy(params.get("force_update")) {
                sqlx::query(
                    r#"
                    UPDATE v2_user
                    SET group_id = ?, transfer_enable = ?, device_limit = ?, speed_limit = ?, updated_at = ?
                    WHERE plan_id = ?
                    "#,
                )
                .bind(required_i64(params, "group_id")?)
                .bind(required_i64(params, "transfer_enable")? * GIB)
                .bind(optional_i64(params, "device_limit"))
                .bind(optional_i64(params, "speed_limit"))
                .bind(now)
                .bind(id)
                .execute(&self.db)
                .await?;
            }
        } else {
            sqlx::query(
                r#"
                INSERT INTO v2_plan (
                    group_id, transfer_enable, device_limit, name, speed_limit, `show`, sort,
                    renew, content, month_price, quarter_price, half_year_price, year_price,
                    two_year_price, three_year_price, onetime_price, reset_price,
                    reset_traffic_method, capacity_limit, created_at, updated_at
                )
                VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
                "#,
            )
            .bind(required_i64(params, "group_id")?)
            .bind(required_i64(params, "transfer_enable")?)
            .bind(optional_i64(params, "device_limit"))
            .bind(required_string(params, "name")?)
            .bind(optional_i64(params, "speed_limit"))
            .bind(optional_i64(params, "show").unwrap_or(1))
            .bind(optional_i64(params, "sort"))
            .bind(optional_i64(params, "renew").unwrap_or(1))
            .bind(params.get("content"))
            .bind(optional_i64(params, "month_price"))
            .bind(optional_i64(params, "quarter_price"))
            .bind(optional_i64(params, "half_year_price"))
            .bind(optional_i64(params, "year_price"))
            .bind(optional_i64(params, "two_year_price"))
            .bind(optional_i64(params, "three_year_price"))
            .bind(optional_i64(params, "onetime_price"))
            .bind(optional_i64(params, "reset_price"))
            .bind(optional_i64(params, "reset_traffic_method"))
            .bind(optional_i64(params, "capacity_limit"))
            .bind(now)
            .bind(now)
            .execute(&self.db)
            .await?;
        }
        Ok(AdminOutput::Data(json!(true)))
    }

    async fn plan_update(&self, params: &HashMap<String, String>) -> Result<AdminOutput, ApiError> {
        let id = required_i64(params, "id")?;
        if let Some(show) = optional_i64(params, "show") {
            sqlx::query("UPDATE v2_plan SET `show` = ?, updated_at = ? WHERE id = ?")
                .bind(show)
                .bind(Utc::now().timestamp())
                .bind(id)
                .execute(&self.db)
                .await?;
        }
        if let Some(renew) = optional_i64(params, "renew") {
            sqlx::query("UPDATE v2_plan SET renew = ?, updated_at = ? WHERE id = ?")
                .bind(renew)
                .bind(Utc::now().timestamp())
                .bind(id)
                .execute(&self.db)
                .await?;
        }
        Ok(AdminOutput::Data(json!(true)))
    }

    async fn payment_fetch(&self) -> Result<AdminOutput, ApiError> {
        let rows = sqlx::query_as::<_, PaymentRow>(
            r#"
            SELECT id, name, payment, icon, handling_fee_fixed,
                   CAST(handling_fee_percent AS DOUBLE) AS handling_fee_percent,
                   uuid, CAST(config AS CHAR) AS config, notify_domain, enable, sort,
                   created_at, updated_at
            FROM v2_payment
            ORDER BY sort ASC
            "#,
        )
        .fetch_all(&self.db)
        .await?;
        let data = rows
            .into_iter()
            .map(|row| {
                let notify_path = format!(
                    "/api/v1/guest/payment/notify/{}/{}",
                    row.payment, row.uuid
                );
                let notify_url = if let Some(domain) =
                    row.notify_domain.as_deref().filter(|value| !value.is_empty())
                {
                    format!("{}{}", domain.trim_end_matches('/'), notify_path)
                } else if let Some(app_url) =
                    self.config.app_url.as_deref().filter(|value| !value.is_empty())
                {
                    format!("{}{}", app_url.trim_end_matches('/'), notify_path)
                } else {
                    notify_path
                };
                json!({
                    "id": row.id,
                    "name": row.name,
                    "payment": row.payment,
                    "icon": row.icon,
                    "handling_fee_fixed": row.handling_fee_fixed,
                    "handling_fee_percent": row.handling_fee_percent,
                    "uuid": row.uuid,
                    "config": serde_json::from_str::<Value>(&row.config).unwrap_or_else(|_| json!({})),
                    "notify_domain": row.notify_domain,
                    "notify_url": notify_url,
                    "enable": row.enable,
                    "sort": row.sort,
                    "created_at": row.created_at,
                    "updated_at": row.updated_at,
                })
            })
            .collect::<Vec<_>>();
        Ok(AdminOutput::Data(json!(data)))
    }

    async fn payment_form(
        &self,
        params: &HashMap<String, String>,
    ) -> Result<AdminOutput, ApiError> {
        let payment = params
            .get("payment")
            .map(String::as_str)
            .unwrap_or_default();
        let config = if let Some(id) = optional_i64(params, "id") {
            let raw_config = sqlx::query_scalar::<_, String>(
                "SELECT CAST(config AS CHAR) FROM v2_payment WHERE id = ? LIMIT 1",
            )
            .bind(id)
            .fetch_optional(&self.db)
            .await?
            .ok_or_else(|| ApiError::legacy("支付方式不存在"))?;
            Some(serde_json::from_str::<Value>(&raw_config).unwrap_or_else(|_| json!({})))
        } else {
            None
        };
        Ok(AdminOutput::Data(payment_provider_form(
            payment,
            config.as_ref(),
        )))
    }

    async fn payment_save(
        &self,
        params: &HashMap<String, String>,
    ) -> Result<AdminOutput, ApiError> {
        if self
            .config
            .app_url
            .as_deref()
            .unwrap_or_default()
            .is_empty()
        {
            return Err(ApiError::legacy("请在站点配置中配置站点地址"));
        }
        let payment = required_string(params, "payment")?;
        if payment_provider_manifest(&payment).is_none() {
            return Err(ApiError::legacy("gate is not found"));
        }
        let config = nested_json(params, "config");
        let config = serde_json::to_string(&config)
            .map_err(|_| ApiError::internal("failed to encode payment config"))?;
        let now = Utc::now().timestamp();
        if let Some(id) = optional_i64(params, "id") {
            sqlx::query(
                r#"
                UPDATE v2_payment
                SET name = ?, icon = ?, payment = ?, config = ?, notify_domain = ?,
                    handling_fee_fixed = ?, handling_fee_percent = ?, updated_at = ?
                WHERE id = ?
                "#,
            )
            .bind(required_string(params, "name")?)
            .bind(params.get("icon"))
            .bind(&payment)
            .bind(config)
            .bind(params.get("notify_domain"))
            .bind(optional_i64(params, "handling_fee_fixed"))
            .bind(optional_f64(params, "handling_fee_percent"))
            .bind(now)
            .bind(id)
            .execute(&self.db)
            .await?;
        } else {
            sqlx::query(
                r#"
                INSERT INTO v2_payment (
                    name, icon, payment, uuid, config, notify_domain, handling_fee_fixed,
                    handling_fee_percent, enable, sort, created_at, updated_at
                )
                VALUES (?, ?, ?, ?, ?, ?, ?, ?, 0, ?, ?, ?)
                "#,
            )
            .bind(required_string(params, "name")?)
            .bind(params.get("icon"))
            .bind(&payment)
            .bind(random_short())
            .bind(config)
            .bind(params.get("notify_domain"))
            .bind(optional_i64(params, "handling_fee_fixed"))
            .bind(optional_f64(params, "handling_fee_percent"))
            .bind(optional_i64(params, "sort"))
            .bind(now)
            .bind(now)
            .execute(&self.db)
            .await?;
        }
        Ok(AdminOutput::Data(json!(true)))
    }

    async fn notice_fetch(&self) -> Result<AdminOutput, ApiError> {
        let rows = sqlx::query_as::<_, NoticeRaw>(
            "SELECT id, title, content, img_url, tags, `show`, created_at, updated_at FROM v2_notice ORDER BY id DESC",
        )
        .fetch_all(&self.db)
        .await?;
        Ok(AdminOutput::Data(json!(
            rows.into_iter().map(NoticeDto::from).collect::<Vec<_>>()
        )))
    }

    async fn notice_save(&self, params: &HashMap<String, String>) -> Result<AdminOutput, ApiError> {
        let now = Utc::now().timestamp();
        let tags = array_param(params, "tags")
            .ok()
            .and_then(|items| serde_json::to_string(&items).ok());
        if let Some(id) = optional_i64(params, "id") {
            sqlx::query(
                "UPDATE v2_notice SET title = ?, content = ?, img_url = ?, tags = ?, updated_at = ? WHERE id = ?",
            )
            .bind(required_string(params, "title")?)
            .bind(required_string(params, "content")?)
            .bind(params.get("img_url"))
            .bind(tags)
            .bind(now)
            .bind(id)
            .execute(&self.db)
            .await?;
        } else {
            sqlx::query(
                "INSERT INTO v2_notice (title, content, img_url, tags, `show`, created_at, updated_at) VALUES (?, ?, ?, ?, 1, ?, ?)",
            )
            .bind(required_string(params, "title")?)
            .bind(required_string(params, "content")?)
            .bind(params.get("img_url"))
            .bind(tags)
            .bind(now)
            .bind(now)
            .execute(&self.db)
            .await?;
        }
        Ok(AdminOutput::Data(json!(true)))
    }

    async fn notice_update(
        &self,
        params: &HashMap<String, String>,
    ) -> Result<AdminOutput, ApiError> {
        let id = required_i64(params, "id")?;
        let mut values = Vec::new();
        if let Some(title) = optional_string(params, "title") {
            values.push(("title", AdminSqlValue::Text(title)));
        }
        if let Some(content) = optional_string(params, "content") {
            values.push(("content", AdminSqlValue::Text(content)));
        }
        if params.contains_key("img_url") {
            values.push(("img_url", optional_text_value(params, "img_url")));
        }
        if params
            .keys()
            .any(|key| key == "tags" || key.starts_with("tags["))
        {
            values.push(("tags", optional_json_array_text_value(params, "tags")));
        }
        if let Some(show) = optional_i64(params, "show") {
            values.push(("show", AdminSqlValue::Integer(show)));
        }
        if values.is_empty() {
            return self.toggle("v2_notice", "show", id).await;
        }

        let mut builder = QueryBuilder::<MySql>::new("UPDATE v2_notice SET ");
        let mut first = true;
        for (column, value) in &values {
            if !first {
                builder.push(", ");
            }
            first = false;
            builder.push(format!("`{column}` = "));
            push_admin_sql_bind(&mut builder, value);
        }
        builder.push(", `updated_at` = ");
        builder.push_bind(Utc::now().timestamp());
        builder.push(" WHERE id = ");
        builder.push_bind(id);
        let result = builder.build().execute(&self.db).await?;
        if result.rows_affected() == 0 {
            return Err(ApiError::legacy("公告不存在"));
        }
        Ok(AdminOutput::Data(json!(true)))
    }

    async fn knowledge_fetch(
        &self,
        params: &HashMap<String, String>,
    ) -> Result<AdminOutput, ApiError> {
        if let Some(id) = optional_i64(params, "id") {
            let value = fetch_json_one(
                &self.db,
                r#"
                SELECT JSON_OBJECT(
                    'id', id, 'language', language, 'category', category, 'title', title,
                    'body', body, 'sort', sort, 'show', `show`, 'created_at', created_at,
                    'updated_at', updated_at
                )
                FROM v2_knowledge
                WHERE id = ?
                LIMIT 1
                "#,
                id,
            )
            .await?
            .ok_or_else(|| ApiError::legacy("知识不存在"))?;
            return Ok(AdminOutput::Data(value));
        }
        Ok(AdminOutput::Data(json!(
            fetch_json_list(
                &self.db,
                r#"
            SELECT JSON_OBJECT(
                'id', id, 'category', category, 'title', title, 'sort', sort, 'show', `show`,
                'updated_at', updated_at
            )
            FROM v2_knowledge
            ORDER BY sort ASC
            "#
            )
            .await?
        )))
    }

    async fn knowledge_categories(&self) -> Result<AdminOutput, ApiError> {
        let rows = sqlx::query_scalar::<_, String>(
            "SELECT DISTINCT category FROM v2_knowledge ORDER BY category ASC",
        )
        .fetch_all(&self.db)
        .await?;
        Ok(AdminOutput::Data(json!(rows)))
    }

    async fn knowledge_save(
        &self,
        params: &HashMap<String, String>,
    ) -> Result<AdminOutput, ApiError> {
        let now = Utc::now().timestamp();
        if let Some(id) = optional_i64(params, "id") {
            sqlx::query(
                r#"
                UPDATE v2_knowledge
                SET language = ?, category = ?, title = ?, body = ?, sort = ?, `show` = ?, updated_at = ?
                WHERE id = ?
                "#,
            )
            .bind(required_string(params, "language")?)
            .bind(required_string(params, "category")?)
            .bind(required_string(params, "title")?)
            .bind(required_string(params, "body")?)
            .bind(optional_i64(params, "sort"))
            .bind(optional_i64(params, "show").unwrap_or(1))
            .bind(now)
            .bind(id)
            .execute(&self.db)
            .await?;
        } else {
            sqlx::query(
                r#"
                INSERT INTO v2_knowledge (language, category, title, body, sort, `show`, created_at, updated_at)
                VALUES (?, ?, ?, ?, ?, ?, ?, ?)
                "#,
            )
            .bind(required_string(params, "language")?)
            .bind(required_string(params, "category")?)
            .bind(required_string(params, "title")?)
            .bind(required_string(params, "body")?)
            .bind(optional_i64(params, "sort"))
            .bind(optional_i64(params, "show").unwrap_or(1))
            .bind(now)
            .bind(now)
            .execute(&self.db)
            .await?;
        }
        Ok(AdminOutput::Data(json!(true)))
    }

    async fn ticket_fetch(
        &self,
        params: &HashMap<String, String>,
    ) -> Result<AdminOutput, ApiError> {
        if let Some(id) = optional_i64(params, "id") {
            let ticket = fetch_json_one(
                &self.db,
                r#"
                SELECT JSON_OBJECT(
                    'id', id, 'user_id', user_id, 'subject', subject, 'level', level,
                    'status', status, 'reply_status', reply_status,
                    'last_reply_user_id', (
                        SELECT user_id FROM v2_ticket_message WHERE ticket_id = v2_ticket.id ORDER BY id DESC LIMIT 1
                    ),
                    'created_at', created_at, 'updated_at', updated_at
                )
                FROM v2_ticket
                WHERE id = ?
                LIMIT 1
                "#,
                id,
            )
            .await?
            .ok_or_else(|| ApiError::legacy("工单不存在"))?;
            // is_me marks messages whose author is NOT the ticket owner, i.e. an
            // admin/staff reply (TicketController::fetch :22-30).
            let messages = fetch_json_list_bind(
                &self.db,
                r#"
                SELECT JSON_OBJECT(
                    'id', id, 'user_id', user_id, 'ticket_id', ticket_id, 'message', message,
                    'is_me', CAST(
                        IF(user_id <> (SELECT user_id FROM v2_ticket WHERE id = v2_ticket_message.ticket_id), 'true', 'false')
                        AS JSON
                    ),
                    'created_at', created_at, 'updated_at', updated_at
                )
                FROM v2_ticket_message
                WHERE ticket_id = ?
                ORDER BY id ASC
                "#,
                id,
            )
            .await?;
            let mut ticket = ticket.as_object().cloned().unwrap_or_default();
            ticket.insert("message".to_string(), json!(messages));
            return Ok(AdminOutput::Data(Value::Object(ticket)));
        }

        // List honors the status / reply_status[] / email filters (:37-48).
        fn apply_filters(
            builder: &mut QueryBuilder<MySql>,
            status: Option<i64>,
            reply_statuses: &[i64],
            user_id: Option<i64>,
        ) {
            if let Some(status) = status {
                builder.push(" AND status = ");
                builder.push_bind(status);
            }
            if !reply_statuses.is_empty() {
                builder.push(" AND reply_status IN (");
                let mut separated = builder.separated(", ");
                for value in reply_statuses {
                    separated.push_bind(*value);
                }
                builder.push(")");
            }
            if let Some(user_id) = user_id {
                builder.push(" AND user_id = ");
                builder.push_bind(user_id);
            }
        }

        let status = params
            .get("status")
            .map(|value| value.trim())
            .filter(|value| !value.is_empty())
            .and_then(|value| value.parse::<i64>().ok());
        let reply_statuses: Vec<i64> = json_array_param(params, "reply_status")
            .iter()
            .filter_map(Value::as_i64)
            .collect();
        // email present + user found → scope to that user; present-but-unknown or
        // absent → no scope, matching the Laravel `if ($user)` guard.
        let user_id = if params.contains_key("email") {
            let email = params.get("email").cloned().unwrap_or_default();
            sqlx::query_scalar::<_, i64>("SELECT id FROM v2_user WHERE email = ? LIMIT 1")
                .bind(email)
                .fetch_optional(&self.db)
                .await?
        } else {
            None
        };

        let (current, page_size) = page(params);
        let mut count_builder =
            QueryBuilder::<MySql>::new("SELECT COUNT(*) FROM v2_ticket WHERE 1 = 1");
        apply_filters(&mut count_builder, status, &reply_statuses, user_id);
        let total: i64 = count_builder
            .build_query_scalar()
            .fetch_one(&self.db)
            .await?;

        let mut builder = QueryBuilder::<MySql>::new(
            r#"
            SELECT JSON_OBJECT(
                'id', id, 'user_id', user_id, 'subject', subject, 'level', level,
                'status', status, 'reply_status', reply_status,
                'last_reply_user_id', (
                    SELECT user_id FROM v2_ticket_message WHERE ticket_id = v2_ticket.id ORDER BY id DESC LIMIT 1
                ),
                'created_at', created_at, 'updated_at', updated_at
            )
            FROM v2_ticket
            WHERE 1 = 1
            "#,
        );
        apply_filters(&mut builder, status, &reply_statuses, user_id);
        builder.push(" ORDER BY updated_at DESC LIMIT ");
        builder.push_bind(page_size);
        builder.push(" OFFSET ");
        builder.push_bind(offset(current, page_size));
        let rows = builder
            .build_query_scalar::<Json<Value>>()
            .fetch_all(&self.db)
            .await?;
        let data = rows.into_iter().map(|row| row.0).collect();
        Ok(AdminOutput::Page { data, total })
    }

    async fn ticket_reply(
        &self,
        params: &HashMap<String, String>,
    ) -> Result<AdminOutput, ApiError> {
        // Ports TicketService::replyByAdmin (:34-61): records the reply under the
        // acting admin, reopens the ticket (status = 0), sets reply_status based
        // on authorship, and notifies the owner by email (deduped 30 min).
        let id = required_i64(params, "id")?;
        let message = required_string(params, "message")?;
        let admin_id = self.current_admin_id(params).await?;
        let (ticket_user_id, subject): (i64, String) =
            sqlx::query_as("SELECT user_id, subject FROM v2_ticket WHERE id = ? LIMIT 1")
                .bind(id)
                .fetch_optional(&self.db)
                .await?
                .ok_or_else(|| ApiError::legacy("工单不存在"))?;
        let reply_status = i64::from(admin_id != ticket_user_id);
        let now = Utc::now().timestamp();
        let mut tx = self.db.begin().await?;
        sqlx::query(
            "INSERT INTO v2_ticket_message (user_id, ticket_id, message, created_at, updated_at) VALUES (?, ?, ?, ?, ?)",
        )
        .bind(admin_id)
        .bind(id)
        .bind(&message)
        .bind(now)
        .bind(now)
        .execute(&mut *tx)
        .await?;
        sqlx::query(
            "UPDATE v2_ticket SET status = 0, reply_status = ?, updated_at = ? WHERE id = ?",
        )
        .bind(reply_status)
        .bind(now)
        .bind(id)
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        self.notify_ticket_reply(ticket_user_id, &subject, &message)
            .await;
        Ok(AdminOutput::Data(json!(true)))
    }

    /// Emails the ticket owner that they received a reply, deduped for 30 minutes
    /// via `ticket_sendEmailNotify_<user_id>`. Best-effort and synchronous: the
    /// Laravel oracle queues a `notify`-templated job, but the Rust port sends the
    /// plain subject/content inline and never fails the reply on a mail error.
    async fn notify_ticket_reply(&self, user_id: i64, subject: &str, message: &str) {
        let Ok(mut conn) = self.redis.get_multiplexed_async_connection().await else {
            return;
        };
        let cache_key = format!("ticket_sendEmailNotify_{user_id}");
        if conn.exists::<_, bool>(&cache_key).await.unwrap_or(false) {
            return;
        }
        let _ = conn.set_ex::<_, i64, ()>(&cache_key, 1, 1800).await;
        let email: Option<String> =
            sqlx::query_scalar("SELECT email FROM v2_user WHERE id = ? LIMIT 1")
                .bind(user_id)
                .fetch_optional(&self.db)
                .await
                .ok()
                .flatten();
        let Some(email) = email else {
            return;
        };
        let subject_line = format!("您在{}的工单得到了回复", self.config.app_name);
        let content = format!("主题：{subject}\r\n回复内容：{message}");
        let _ = self.send_mail(&email, &subject_line, &content).await;
    }

    async fn ticket_close(&self, id: i64) -> Result<AdminOutput, ApiError> {
        sqlx::query("UPDATE v2_ticket SET status = 1, updated_at = ? WHERE id = ?")
            .bind(Utc::now().timestamp())
            .bind(id)
            .execute(&self.db)
            .await?;
        Ok(AdminOutput::Data(json!(true)))
    }

    async fn coupon_fetch(
        &self,
        params: &HashMap<String, String>,
    ) -> Result<AdminOutput, ApiError> {
        let (current, page_size) = page(params);
        let total: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM v2_coupon")
            .fetch_one(&self.db)
            .await?;
        let data = fetch_json_list_page(
            &self.db,
            r#"
            SELECT JSON_OBJECT(
                'id', id, 'code', code, 'name', name, 'type', type, 'value', value,
                'show', `show`, 'limit_use', limit_use, 'limit_use_with_user', limit_use_with_user,
                'limit_plan_ids', CAST(limit_plan_ids AS JSON), 'limit_period', CAST(limit_period AS JSON),
                'started_at', started_at, 'ended_at', ended_at, 'created_at', created_at, 'updated_at', updated_at
            )
            FROM v2_coupon
            ORDER BY id DESC
            LIMIT ? OFFSET ?
            "#,
            page_size,
            offset(current, page_size),
        )
        .await?;
        Ok(AdminOutput::Page { data, total })
    }

    async fn giftcard_fetch(
        &self,
        params: &HashMap<String, String>,
    ) -> Result<AdminOutput, ApiError> {
        let (current, page_size) = page(params);
        let total: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM v2_giftcard")
            .fetch_one(&self.db)
            .await?;
        let data = fetch_json_list_page(
            &self.db,
            r#"
            SELECT JSON_OBJECT(
                'id', id, 'code', code, 'name', name, 'type', type, 'value', value,
                'plan_id', plan_id, 'limit_use', limit_use, 'used_user_ids', used_user_ids,
                'started_at', started_at, 'ended_at', ended_at, 'created_at', created_at, 'updated_at', updated_at
            )
            FROM v2_giftcard
            ORDER BY id DESC
            LIMIT ? OFFSET ?
            "#,
            page_size,
            offset(current, page_size),
        )
        .await?;
        Ok(AdminOutput::Page { data, total })
    }

    async fn coupon_generate(
        &self,
        params: &HashMap<String, String>,
    ) -> Result<AdminOutput, ApiError> {
        // Ports CouponController::generate / multiGenerate. A generate_count marks
        // the CSV batch path; otherwise this is a single create (or update by id).
        let now = Utc::now().timestamp();
        if let Some(count) = optional_i64(params, "generate_count").filter(|count| *count > 0) {
            let field_values = coupon_field_values(params);
            let mut codes = Vec::new();
            let mut tx = self.db.begin().await?;
            for _ in 0..count {
                let code = random_char(8);
                let mut builder = QueryBuilder::<MySql>::new("INSERT INTO v2_coupon (");
                let mut columns = builder.separated(", ");
                for (column, _) in &field_values {
                    columns.push(format!("`{column}`"));
                }
                columns.push("`show`");
                columns.push("`code`");
                columns.push("`created_at`");
                columns.push("`updated_at`");
                builder.push(") VALUES (");
                let mut placeholders = builder.separated(", ");
                for (_, value) in &field_values {
                    push_admin_sql_value(&mut placeholders, value);
                }
                placeholders.push_bind(1_i64);
                placeholders.push_bind(code.clone());
                placeholders.push_bind(now);
                placeholders.push_bind(now);
                builder.push(")");
                builder.build().execute(&mut *tx).await?;
                codes.push(code);
            }
            tx.commit().await?;

            let coupon_type = optional_i64(params, "type").unwrap_or_default();
            let value = optional_i64(params, "value").unwrap_or_default();
            let type_label = match coupon_type {
                1 => "金额",
                2 => "比例",
                _ => "",
            };
            let value_display = match coupon_type {
                1 => (value as f64 / 100.0).to_string(),
                2 => value.to_string(),
                _ => String::new(),
            };
            let name = optional_string(params, "name").unwrap_or_default();
            let start = local_datetime(optional_i64(params, "started_at").unwrap_or_default());
            let end = local_datetime(optional_i64(params, "ended_at").unwrap_or_default());
            let limit_use = optional_i64(params, "limit_use")
                .map(|value| value.to_string())
                .unwrap_or_else(|| "不限制".to_string());
            let limit_plan_ids = joined_array_display(params, "limit_plan_ids");
            let create = local_datetime(now);
            let mut body = String::from(
                "名称,类型,金额或比例,开始时间,结束时间,可用次数,可用于订阅,券码,生成时间\r\n",
            );
            for code in codes {
                body.push_str(&format!(
                    "{name},{type_label},{value_display},{start},{end},{limit_use},{limit_plan_ids},{code},{create}\r\n"
                ));
            }
            return Ok(AdminOutput::Csv {
                filename: "coupon.csv".to_string(),
                body,
            });
        }

        let mut values = coupon_field_values(params);
        if let Some(id) = optional_i64(params, "id") {
            if let Some(code) = optional_string(params, "code") {
                values.push(("code", AdminSqlValue::Text(code)));
            }
            self.update_row("v2_coupon", id, &values, now).await?;
        } else {
            let code = optional_string(params, "code").unwrap_or_else(|| random_char(8));
            values.push(("code", AdminSqlValue::Text(code)));
            self.insert_row("v2_coupon", &values, now).await?;
        }
        Ok(AdminOutput::Data(json!(true)))
    }

    async fn giftcard_generate(
        &self,
        params: &HashMap<String, String>,
    ) -> Result<AdminOutput, ApiError> {
        // Ports GiftcardController::generate / multiGenerate. Codes are 16 chars
        // and, in the batch path, retried until unique.
        let now = Utc::now().timestamp();
        if let Some(count) = optional_i64(params, "generate_count").filter(|count| *count > 0) {
            let field_values = giftcard_field_values(params);
            let mut codes = Vec::new();
            let mut tx = self.db.begin().await?;
            for _ in 0..count {
                let code = loop {
                    let candidate = random_char(16);
                    let exists: Option<i64> =
                        sqlx::query_scalar("SELECT id FROM v2_giftcard WHERE code = ? LIMIT 1")
                            .bind(&candidate)
                            .fetch_optional(&mut *tx)
                            .await?;
                    if exists.is_none() {
                        break candidate;
                    }
                };
                let mut builder = QueryBuilder::<MySql>::new("INSERT INTO v2_giftcard (");
                let mut columns = builder.separated(", ");
                for (column, _) in &field_values {
                    columns.push(format!("`{column}`"));
                }
                columns.push("`code`");
                columns.push("`created_at`");
                columns.push("`updated_at`");
                builder.push(") VALUES (");
                let mut placeholders = builder.separated(", ");
                for (_, value) in &field_values {
                    push_admin_sql_value(&mut placeholders, value);
                }
                placeholders.push_bind(code.clone());
                placeholders.push_bind(now);
                placeholders.push_bind(now);
                builder.push(")");
                builder.build().execute(&mut *tx).await?;
                codes.push(code);
            }
            tx.commit().await?;

            let card_type = optional_i64(params, "type").unwrap_or_default();
            let value = optional_i64(params, "value").unwrap_or_default();
            let type_label = match card_type {
                1 => "金额",
                2 => "时长",
                3 => "流量",
                4 => "重置",
                5 => "套餐",
                _ => "",
            };
            let value_display = match card_type {
                1 => format!("{:.2}", value as f64 / 100.0),
                2 | 5 => format!("{value}天"),
                3 => format!("{value}GB"),
                4 => "-".to_string(),
                _ => String::new(),
            };
            let name = optional_string(params, "name").unwrap_or_default();
            let start = local_datetime(optional_i64(params, "started_at").unwrap_or_default());
            let end = local_datetime(optional_i64(params, "ended_at").unwrap_or_default());
            let limit_use = optional_i64(params, "limit_use")
                .map(|value| value.to_string())
                .unwrap_or_else(|| "不限制".to_string());
            let create = local_datetime(now);
            let mut body =
                String::from("名称,类型,数值,开始时间,结束时间,可用次数,礼品卡卡密,生成时间\r\n");
            for code in codes {
                body.push_str(&format!(
                    "{name},{type_label},{value_display},{start},{end},{limit_use},{code},{create}\r\n"
                ));
            }
            return Ok(AdminOutput::Csv {
                filename: "giftcard.csv".to_string(),
                body,
            });
        }

        let mut values = giftcard_field_values(params);
        if let Some(id) = optional_i64(params, "id") {
            let exists: Option<i64> =
                sqlx::query_scalar("SELECT id FROM v2_giftcard WHERE id = ? LIMIT 1")
                    .bind(id)
                    .fetch_optional(&self.db)
                    .await?;
            if exists.is_none() {
                return Err(ApiError::not_found("礼品卡不存在"));
            }
            if let Some(code) = optional_string(params, "code") {
                values.push(("code", AdminSqlValue::Text(code)));
            }
            self.update_row("v2_giftcard", id, &values, now).await?;
        } else {
            let code = optional_string(params, "code").unwrap_or_else(|| random_char(16));
            values.push(("code", AdminSqlValue::Text(code)));
            self.insert_row("v2_giftcard", &values, now).await?;
        }
        Ok(AdminOutput::Data(json!(true)))
    }

    /// Builds and runs a dynamic `INSERT ... (created_at, updated_at)` for the
    /// given whitelisted column/value pairs. Table names are compile-time
    /// literals, so the interpolation is injection-safe.
    async fn insert_row(
        &self,
        table: &str,
        values: &[(&str, AdminSqlValue)],
        now: i64,
    ) -> Result<(), ApiError> {
        let mut builder = QueryBuilder::<MySql>::new(format!("INSERT INTO {table} ("));
        let mut columns = builder.separated(", ");
        for (column, _) in values {
            columns.push(format!("`{column}`"));
        }
        columns.push("`created_at`");
        columns.push("`updated_at`");
        builder.push(") VALUES (");
        let mut placeholders = builder.separated(", ");
        for (_, value) in values {
            push_admin_sql_value(&mut placeholders, value);
        }
        placeholders.push_bind(now);
        placeholders.push_bind(now);
        builder.push(")");
        builder.build().execute(&self.db).await?;
        Ok(())
    }

    /// Builds and runs a dynamic `UPDATE ... SET ..., updated_at WHERE id = ?`.
    async fn update_row(
        &self,
        table: &str,
        id: i64,
        values: &[(&str, AdminSqlValue)],
        now: i64,
    ) -> Result<(), ApiError> {
        let mut builder = QueryBuilder::<MySql>::new(format!("UPDATE {table} SET "));
        let mut first = true;
        for (column, value) in values {
            if !first {
                builder.push(", ");
            }
            first = false;
            builder.push(format!("`{column}` = "));
            push_admin_sql_bind(&mut builder, value);
        }
        if !first {
            builder.push(", ");
        }
        builder.push("`updated_at` = ");
        builder.push_bind(now);
        builder.push(" WHERE id = ");
        builder.push_bind(id);
        builder.build().execute(&self.db).await?;
        Ok(())
    }

    /// Reconstructs the `filter[]` array into injection-safe WHERE clauses.
    /// Ports UserController::filter (laravel .../Admin/UserController.php:36-62):
    /// `模糊` → LIKE %value%, `d`/`transfer_enable` scaled by GiB, `invite_by_email`
    /// resolved to invite_user_id (0 when not found), and `plan_id == 'null'` → IS NULL.
    /// Unknown columns/operators are dropped rather than interpolated (unlike the
    /// Laravel builder, which trusts the raw request key).
    async fn user_filter_clauses(
        &self,
        params: &HashMap<String, String>,
    ) -> Result<Vec<UserFilterClause>, ApiError> {
        let mut clauses = Vec::new();
        for entry in collect_filter_entries(params) {
            let Some(key) = entry.get("key").map(String::as_str) else {
                continue;
            };
            let mut condition = entry
                .get("condition")
                .map(String::as_str)
                .unwrap_or("=")
                .to_string();
            let mut value = entry.get("value").cloned().unwrap_or_default();
            if condition == "模糊" {
                condition = "like".to_string();
                value = format!("%{value}%");
            }
            if key == "d" || key == "transfer_enable" {
                let scaled = (value.trim().parse::<f64>().unwrap_or_default() * GIB as f64) as i64;
                let (Some(column), Some(op)) = (user_column(key), user_filter_operator(&condition))
                else {
                    continue;
                };
                clauses.push(UserFilterClause::Compare {
                    column,
                    op,
                    value: FilterBind::Int(scaled),
                });
                continue;
            }
            if key == "invite_by_email" {
                let op = user_filter_operator(&condition).unwrap_or("=");
                let invite_id: Option<i64> = sqlx::query_scalar(AssertSqlSafe(format!(
                    "SELECT id FROM v2_user WHERE email {op} ? LIMIT 1"
                )))
                .bind(&value)
                .fetch_optional(&self.db)
                .await?;
                clauses.push(UserFilterClause::Compare {
                    column: "invite_user_id",
                    op: "=",
                    value: FilterBind::Int(invite_id.unwrap_or(0)),
                });
                continue;
            }
            if key == "plan_id" && value == "null" {
                clauses.push(UserFilterClause::IsNull { column: "plan_id" });
                continue;
            }
            let (Some(column), Some(op)) = (user_column(key), user_filter_operator(&condition))
            else {
                continue;
            };
            clauses.push(UserFilterClause::Compare {
                column,
                op,
                value: FilterBind::Text(value),
            });
        }
        Ok(clauses)
    }

    /// Returns the ids of the users matching the request `filter[]` (used by ban
    /// and allDel to stay scoped, like UserController::ban/allDel).
    async fn filtered_user_ids(
        &self,
        params: &HashMap<String, String>,
        staff_scoped: bool,
    ) -> Result<Vec<i64>, ApiError> {
        let clauses = self.user_filter_clauses(params).await?;
        let mut builder = QueryBuilder::<MySql>::new("SELECT u.id FROM v2_user u WHERE 1 = 1");
        if staff_scoped {
            builder.push(" AND u.is_admin = 0 AND u.is_staff = 0");
        }
        push_user_where(&mut builder, &clauses);
        let ids = builder
            .build_query_scalar::<i64>()
            .fetch_all(&self.db)
            .await?;
        Ok(ids)
    }

    /// The emails of every user matching the admin list filter (UserController::sendMail applies
    /// `filter($request, $builder)`, so a filtered mass mail hits only the selected audience — not
    /// all users, and not silently skipping banned recipients).
    async fn filtered_user_emails(
        &self,
        params: &HashMap<String, String>,
        staff_scoped: bool,
    ) -> Result<Vec<String>, ApiError> {
        let clauses = self.user_filter_clauses(params).await?;
        let mut builder = QueryBuilder::<MySql>::new("SELECT u.email FROM v2_user u WHERE 1 = 1");
        if staff_scoped {
            builder.push(" AND u.is_admin = 0 AND u.is_staff = 0");
        }
        push_user_where(&mut builder, &clauses);
        let emails = builder
            .build_query_scalar::<String>()
            .fetch_all(&self.db)
            .await?;
        Ok(emails)
    }

    /// Adds `subscribe_url` and the `alive_ip` / `ips` device stats onto fetched
    /// user rows. Ports the tail of UserController::fetch (:88-105); the alive-IP
    /// cache read is best-effort so a Redis outage does not fail the listing.
    async fn enrich_users(&self, users: &mut [Value]) -> Result<(), ApiError> {
        if users.is_empty() {
            return Ok(());
        }
        let mut conn = self.redis.get_multiplexed_async_connection().await.ok();
        for user in users.iter_mut() {
            let Some(object) = user.as_object_mut() else {
                continue;
            };
            if let Some(token) = object.get("token").and_then(Value::as_str) {
                let url = self.config.subscribe_url_for_token(token);
                object.insert("subscribe_url".to_string(), json!(url));
            }
            let id = object.get("id").and_then(Value::as_i64).unwrap_or_default();
            if let Some(conn) = conn.as_mut()
                && let Ok(Some(raw)) = conn
                    .get::<_, Option<String>>(format!("ALIVE_IP_USER_{id}"))
                    .await
            {
                let (alive_ip, ips) = parse_alive_ip(&raw);
                object.insert("alive_ip".to_string(), json!(alive_ip));
                object.insert("ips".to_string(), json!(ips));
            }
        }
        Ok(())
    }

    /// Deletes the Redis session set for a user (AuthService::removeAllSession).
    /// Best-effort: a Redis outage must not abort a ban/delete.
    async fn remove_user_sessions(&self, user_id: i64) {
        if let Ok(mut conn) = self.redis.get_multiplexed_async_connection().await {
            let _ = conn.del::<_, i64>(format!("USER_SESSIONS_{user_id}")).await;
        }
    }

    /// Resolves the acting admin's user id from the `_admin_email` the router
    /// injects (main.rs adds only the email, not the id).
    async fn current_admin_id(&self, params: &HashMap<String, String>) -> Result<i64, ApiError> {
        let email = required_string(params, "_admin_email")?;
        sqlx::query_scalar::<_, i64>("SELECT id FROM v2_user WHERE email = ? LIMIT 1")
            .bind(email)
            .fetch_optional(&self.db)
            .await?
            .ok_or_else(|| ApiError::legacy("管理员不存在"))
    }

    /// Cascade removal for a single user, shared by delUser and allDel.
    /// Ports UserController::delUser (:361-391) / allDel (:328-359): orders,
    /// invite codes, ticket messages + tickets, and detaching referrals.
    async fn delete_user_cascade(
        &self,
        tx: &mut sqlx::Transaction<'_, MySql>,
        user_id: i64,
    ) -> Result<(), ApiError> {
        self.remove_user_sessions(user_id).await;
        sqlx::query("DELETE FROM v2_order WHERE user_id = ?")
            .bind(user_id)
            .execute(&mut **tx)
            .await?;
        sqlx::query("DELETE FROM v2_invite_code WHERE user_id = ?")
            .bind(user_id)
            .execute(&mut **tx)
            .await?;
        sqlx::query(
            "DELETE FROM v2_ticket_message WHERE ticket_id IN (SELECT id FROM (SELECT id FROM v2_ticket WHERE user_id = ?) AS t)",
        )
        .bind(user_id)
        .execute(&mut **tx)
        .await?;
        sqlx::query("DELETE FROM v2_ticket WHERE user_id = ?")
            .bind(user_id)
            .execute(&mut **tx)
            .await?;
        sqlx::query("UPDATE v2_user SET invite_user_id = NULL WHERE invite_user_id = ?")
            .bind(user_id)
            .execute(&mut **tx)
            .await?;
        Ok(())
    }

    async fn user_fetch(&self, params: &HashMap<String, String>) -> Result<AdminOutput, ApiError> {
        let (current, page_size) = page(params);
        let clauses = self.user_filter_clauses(params).await?;
        let (sort_expr, direction) = user_sort(params);

        let mut count_builder =
            QueryBuilder::<MySql>::new("SELECT COUNT(*) FROM v2_user u WHERE 1 = 1");
        push_user_where(&mut count_builder, &clauses);
        let total: i64 = count_builder
            .build_query_scalar()
            .fetch_one(&self.db)
            .await?;

        let mut builder = QueryBuilder::<MySql>::new(
            r#"
            SELECT JSON_OBJECT(
                'id', u.id, 'email', u.email, 'password', '', 'balance', u.balance,
                'commission_balance', u.commission_balance, 'transfer_enable', u.transfer_enable,
                'device_limit', u.device_limit, 'u', u.u, 'd', u.d, 'total_used', u.u + u.d,
                'alive_ip', 0, 'ips', '', 'plan_id', u.plan_id, 'plan_name', p.name,
                'group_id', u.group_id, 'expired_at', u.expired_at, 'uuid', u.uuid,
                'token', u.token, 'subscribe_url', '', 'banned', u.banned,
                'is_admin', u.is_admin, 'is_staff', u.is_staff,
                'invite_user_id', u.invite_user_id, 'discount', u.discount,
                'commission_rate', u.commission_rate, 'telegram_id', u.telegram_id,
                'last_login_at', u.last_login_at, 'created_at', u.created_at, 'updated_at', u.updated_at
            )
            FROM v2_user u
            LEFT JOIN v2_plan p ON p.id = u.plan_id
            WHERE 1 = 1
            "#,
        );
        push_user_where(&mut builder, &clauses);
        // sort_expr and direction are whitelisted by user_sort, so this raw push is safe.
        builder.push(format!(" ORDER BY {sort_expr} {direction} LIMIT "));
        builder.push_bind(page_size);
        builder.push(" OFFSET ");
        builder.push_bind(offset(current, page_size));
        let rows = builder
            .build_query_scalar::<Json<Value>>()
            .fetch_all(&self.db)
            .await?;
        let mut data: Vec<Value> = rows.into_iter().map(|row| row.0).collect();
        self.enrich_users(&mut data).await?;
        Ok(AdminOutput::Page { data, total })
    }

    async fn user_detail(&self, id: i64) -> Result<AdminOutput, ApiError> {
        let value = fetch_json_one(
            &self.db,
            r#"
            SELECT JSON_OBJECT(
                'id', u.id, 'email', u.email, 'password', '', 'balance', u.balance,
                'commission_balance', u.commission_balance, 'transfer_enable', u.transfer_enable,
                'device_limit', u.device_limit, 'u', u.u, 'd', u.d, 'total_used', u.u + u.d,
                'alive_ip', 0, 'ips', '', 'plan_id', u.plan_id, 'plan_name', p.name,
                'group_id', u.group_id, 'expired_at', u.expired_at, 'uuid', u.uuid,
                'token', u.token, 'subscribe_url', '', 'banned', u.banned,
                'is_admin', u.is_admin, 'is_staff', u.is_staff,
                'invite_user_id', u.invite_user_id, 'discount', u.discount,
                'commission_rate', u.commission_rate, 'telegram_id', u.telegram_id,
                'last_login_at', u.last_login_at, 'created_at', u.created_at, 'updated_at', u.updated_at,
                'invite_user', IF(i.id IS NULL, NULL, JSON_OBJECT('id', i.id, 'email', i.email))
            )
            FROM v2_user u
            LEFT JOIN v2_plan p ON p.id = u.plan_id
            LEFT JOIN v2_user i ON i.id = u.invite_user_id
            WHERE u.id = ?
            LIMIT 1
            "#,
            id,
        )
        .await?
        .ok_or_else(|| ApiError::legacy("用户不存在"))?;
        Ok(AdminOutput::Data(value))
    }

    async fn staff_user_detail(&self, id: i64) -> Result<AdminOutput, ApiError> {
        let value = fetch_json_one(
            &self.db,
            r#"
            SELECT JSON_OBJECT(
                'id', u.id, 'email', u.email, 'password', '', 'balance', u.balance,
                'commission_balance', u.commission_balance, 'transfer_enable', u.transfer_enable,
                'device_limit', u.device_limit, 'u', u.u, 'd', u.d, 'total_used', u.u + u.d,
                'alive_ip', 0, 'ips', '', 'plan_id', u.plan_id, 'plan_name', p.name,
                'group_id', u.group_id, 'expired_at', u.expired_at, 'uuid', u.uuid,
                'token', u.token, 'subscribe_url', '', 'banned', u.banned,
                'is_admin', u.is_admin, 'is_staff', u.is_staff,
                'invite_user_id', u.invite_user_id, 'discount', u.discount,
                'commission_rate', u.commission_rate, 'telegram_id', u.telegram_id,
                'last_login_at', u.last_login_at, 'created_at', u.created_at, 'updated_at', u.updated_at
            )
            FROM v2_user u
            LEFT JOIN v2_plan p ON p.id = u.plan_id
            WHERE u.id = ? AND u.is_admin = 0 AND u.is_staff = 0
            LIMIT 1
            "#,
            id,
        )
        .await?
        .ok_or_else(|| ApiError::legacy("用户不存在"))?;
        Ok(AdminOutput::Data(value))
    }

    async fn user_update(&self, params: &HashMap<String, String>) -> Result<AdminOutput, ApiError> {
        // Ports UserController::update (laravel .../Admin/UserController.php:125-172).
        let id = required_i64(params, "id")?;
        let current_email: String =
            sqlx::query_scalar("SELECT email FROM v2_user WHERE id = ? LIMIT 1")
                .bind(id)
                .fetch_optional(&self.db)
                .await?
                .ok_or_else(|| ApiError::legacy("用户不存在"))?;
        let email = required_string(params, "email")?;
        if email != current_email {
            let taken: Option<i64> =
                sqlx::query_scalar("SELECT id FROM v2_user WHERE email = ? LIMIT 1")
                    .bind(&email)
                    .fetch_optional(&self.db)
                    .await?;
            if taken.is_some() {
                return Err(ApiError::legacy("邮箱已被使用"));
            }
        }

        let mut values: Vec<(&str, AdminSqlValue)> = vec![("email", AdminSqlValue::Text(email))];
        // transfer_enable is stored as-is: the admin UI already sends bytes, so the
        // previous `* GIB` double-scaled it. Laravel's update() stores the raw value.
        for key in [
            "transfer_enable",
            "device_limit",
            "expired_at",
            "banned",
            "commission_rate",
            "discount",
            "is_admin",
            "is_staff",
            "u",
            "d",
            "balance",
            "commission_type",
            "commission_balance",
            "speed_limit",
        ] {
            if params.contains_key(key) {
                values.push((key, optional_int_or_null_value(params, key)));
            }
        }
        if params.contains_key("remarks") {
            values.push(("remarks", optional_text_value(params, "remarks")));
        }

        // plan_id drives group_id (:145-153): a set plan_id resolves group_id from
        // the plan, otherwise group_id is reset to NULL.
        let mut group_id = AdminSqlValue::Null;
        if params.contains_key("plan_id") {
            if let Some(plan_id) = optional_i64(params, "plan_id") {
                let plan_group: Option<i64> =
                    sqlx::query_scalar("SELECT group_id FROM v2_plan WHERE id = ? LIMIT 1")
                        .bind(plan_id)
                        .fetch_optional(&self.db)
                        .await?
                        .ok_or_else(|| ApiError::legacy("订阅计划不存在"))?;
                group_id = plan_group
                    .map(AdminSqlValue::Integer)
                    .unwrap_or(AdminSqlValue::Null);
                values.push(("plan_id", AdminSqlValue::Integer(plan_id)));
            } else {
                values.push(("plan_id", AdminSqlValue::Null));
            }
        }
        values.push(("group_id", group_id));

        // invite_user_email → invite_user_id (:155-162). A present-but-unknown
        // email leaves invite_user_id untouched; an absent email resets it to NULL.
        match params
            .get("invite_user_email")
            .map(|value| value.trim())
            .filter(|value| !value.is_empty())
        {
            Some(invite_email) => {
                if let Some(invite_id) =
                    sqlx::query_scalar::<_, i64>("SELECT id FROM v2_user WHERE email = ? LIMIT 1")
                        .bind(invite_email)
                        .fetch_optional(&self.db)
                        .await?
                {
                    values.push(("invite_user_id", AdminSqlValue::Integer(invite_id)));
                }
            }
            None => values.push(("invite_user_id", AdminSqlValue::Null)),
        }

        if let Some(password) = params.get("password").filter(|value| !value.is_empty()) {
            let hash = bcrypt::hash(password, bcrypt::DEFAULT_COST)
                .map_err(|_| ApiError::internal("failed to hash password"))?;
            values.push(("password", AdminSqlValue::Text(hash)));
            values.push(("password_algo", AdminSqlValue::Null));
        }

        // banned == 1 tears down active sessions (:164-167).
        if optional_i64(params, "banned") == Some(1) {
            self.remove_user_sessions(id).await;
        }

        let mut builder = QueryBuilder::<MySql>::new("UPDATE v2_user SET ");
        let mut first = true;
        for (column, value) in &values {
            if !first {
                builder.push(", ");
            }
            first = false;
            builder.push(format!("`{column}` = "));
            push_admin_sql_bind(&mut builder, value);
        }
        builder.push(", `updated_at` = ");
        builder.push_bind(Utc::now().timestamp());
        builder.push(" WHERE id = ");
        builder.push_bind(id);
        builder.build().execute(&self.db).await?;
        Ok(AdminOutput::Data(json!(true)))
    }

    async fn staff_user_update(
        &self,
        params: &HashMap<String, String>,
    ) -> Result<AdminOutput, ApiError> {
        // Ports Staff\UserController::update. Staff cannot touch speed_limit,
        // is_admin, is_staff, commission_type or remarks (Staff\UserUpdate rules),
        // and — unlike Laravel's unscoped find — the target stays restricted to
        // non-admin/non-staff users.
        let id = required_i64(params, "id")?;
        let current_email: String = sqlx::query_scalar(
            "SELECT email FROM v2_user WHERE id = ? AND is_admin = 0 AND is_staff = 0 LIMIT 1",
        )
        .bind(id)
        .fetch_optional(&self.db)
        .await?
        .ok_or_else(|| ApiError::legacy("用户不存在"))?;
        let email = required_string(params, "email")?;
        if email != current_email {
            let taken: Option<i64> =
                sqlx::query_scalar("SELECT id FROM v2_user WHERE email = ? LIMIT 1")
                    .bind(&email)
                    .fetch_optional(&self.db)
                    .await?;
            if taken.is_some() {
                return Err(ApiError::legacy("邮箱已被使用"));
            }
        }

        let mut values: Vec<(&str, AdminSqlValue)> = vec![("email", AdminSqlValue::Text(email))];
        for key in [
            "transfer_enable",
            "device_limit",
            "expired_at",
            "banned",
            "commission_rate",
            "discount",
            "u",
            "d",
            "balance",
            "commission_balance",
        ] {
            if params.contains_key(key) {
                values.push((key, optional_int_or_null_value(params, key)));
            }
        }
        // Staff update only sets group_id when a real plan_id is supplied.
        if params.contains_key("plan_id") {
            if let Some(plan_id) = optional_i64(params, "plan_id") {
                let plan_group: Option<i64> =
                    sqlx::query_scalar("SELECT group_id FROM v2_plan WHERE id = ? LIMIT 1")
                        .bind(plan_id)
                        .fetch_optional(&self.db)
                        .await?
                        .ok_or_else(|| ApiError::legacy("订阅计划不存在"))?;
                values.push(("plan_id", AdminSqlValue::Integer(plan_id)));
                values.push((
                    "group_id",
                    plan_group
                        .map(AdminSqlValue::Integer)
                        .unwrap_or(AdminSqlValue::Null),
                ));
            } else {
                values.push(("plan_id", AdminSqlValue::Null));
            }
        }
        if let Some(password) = params.get("password").filter(|value| !value.is_empty()) {
            let hash = bcrypt::hash(password, bcrypt::DEFAULT_COST)
                .map_err(|_| ApiError::internal("failed to hash password"))?;
            values.push(("password", AdminSqlValue::Text(hash)));
            values.push(("password_algo", AdminSqlValue::Null));
        }

        let mut builder = QueryBuilder::<MySql>::new("UPDATE v2_user SET ");
        let mut first = true;
        for (column, value) in &values {
            if !first {
                builder.push(", ");
            }
            first = false;
            builder.push(format!("`{column}` = "));
            push_admin_sql_bind(&mut builder, value);
        }
        builder.push(", `updated_at` = ");
        builder.push_bind(Utc::now().timestamp());
        builder.push(" WHERE id = ");
        builder.push_bind(id);
        builder.push(" AND is_admin = 0 AND is_staff = 0");
        builder.build().execute(&self.db).await?;
        Ok(AdminOutput::Data(json!(true)))
    }

    async fn staff_send_mail_to_users(
        &self,
        params: &HashMap<String, String>,
    ) -> Result<AdminOutput, ApiError> {
        let subject = required_string(params, "subject")?;
        let content = required_string(params, "content")?;
        let emails = self.filtered_user_emails(params, true).await?;
        for email in emails {
            self.send_mail(&email, &subject, &content).await?;
        }
        Ok(AdminOutput::Data(json!(true)))
    }

    async fn staff_user_bulk_ban(
        &self,
        params: &HashMap<String, String>,
    ) -> Result<AdminOutput, ApiError> {
        // Ports Staff\UserController::ban: scoped to the request filter and, as a
        // staff safety guard, restricted to non-admin/non-staff users. Staff bans
        // do not tear down sessions (unlike the admin ban).
        let ids = self.filtered_user_ids(params, true).await?;
        if ids.is_empty() {
            return Ok(AdminOutput::Data(json!(true)));
        }
        let mut builder =
            QueryBuilder::<MySql>::new("UPDATE v2_user SET banned = 1, updated_at = ");
        builder.push_bind(Utc::now().timestamp());
        builder.push(" WHERE id IN (");
        let mut separated = builder.separated(", ");
        for id in &ids {
            separated.push_bind(*id);
        }
        builder.push(")");
        builder.build().execute(&self.db).await?;
        Ok(AdminOutput::Data(json!(true)))
    }

    async fn user_reset_secret(&self, id: i64) -> Result<AdminOutput, ApiError> {
        sqlx::query("UPDATE v2_user SET token = ?, uuid = ?, updated_at = ? WHERE id = ?")
            .bind(random_token())
            .bind(Uuid::new_v4().to_string())
            .bind(Utc::now().timestamp())
            .bind(id)
            .execute(&self.db)
            .await?;
        Ok(AdminOutput::Data(json!(true)))
    }

    /// Resolves the plan referenced by a generate request into
    /// `(id, group_id, transfer_enable_bytes, device_limit)`. Ports the
    /// `Plan::find` guard shared by generate() and multiGenerate().
    async fn generate_plan(
        &self,
        params: &HashMap<String, String>,
    ) -> Result<Option<(i64, Option<i64>, i64, Option<i64>)>, ApiError> {
        let Some(plan_id) = optional_i64(params, "plan_id") else {
            return Ok(None);
        };
        let row: (i64, Option<i64>, Option<i64>, Option<i64>) = sqlx::query_as(
            "SELECT id, group_id, transfer_enable, device_limit FROM v2_plan WHERE id = ? LIMIT 1",
        )
        .bind(plan_id)
        .fetch_optional(&self.db)
        .await?
        .ok_or_else(|| ApiError::legacy("订阅计划不存在"))?;
        Ok(Some((row.0, row.1, row.2.unwrap_or_default() * GIB, row.3)))
    }

    async fn user_generate(
        &self,
        params: &HashMap<String, String>,
    ) -> Result<AdminOutput, ApiError> {
        // Ports UserController::generate (:204-236) and multiGenerate (:238-279).
        let now = Utc::now().timestamp();
        let plan = self.generate_plan(params).await?;
        let (plan_id, group_id, transfer_enable, device_limit) = match plan {
            Some((id, group_id, transfer_enable, device_limit)) => {
                (Some(id), group_id, transfer_enable, device_limit)
            }
            None => (None, None, 0, None),
        };

        // Single generation returns JSON; the CSV path is multiGenerate only.
        if let Some(prefix) = optional_string(params, "email_prefix") {
            let suffix = required_string(params, "email_suffix")?;
            let email = format!("{prefix}@{suffix}");
            let exists: Option<i64> =
                sqlx::query_scalar("SELECT id FROM v2_user WHERE email = ? LIMIT 1")
                    .bind(&email)
                    .fetch_optional(&self.db)
                    .await?;
            if exists.is_some() {
                return Err(ApiError::legacy("邮箱已存在于系统中"));
            }
            let password_plain = params
                .get("password")
                .filter(|value| !value.is_empty())
                .cloned()
                .unwrap_or_else(|| email.clone());
            let hash = bcrypt::hash(&password_plain, bcrypt::DEFAULT_COST)
                .map_err(|_| ApiError::internal("failed to hash password"))?;
            sqlx::query(
                r#"
                INSERT INTO v2_user (
                    email, plan_id, group_id, transfer_enable, device_limit, expired_at,
                    uuid, token, password, password_algo, created_at, updated_at
                )
                VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, NULL, ?, ?)
                "#,
            )
            .bind(&email)
            .bind(plan_id)
            .bind(group_id)
            .bind(transfer_enable)
            .bind(device_limit)
            .bind(optional_i64(params, "expired_at"))
            .bind(Uuid::new_v4().to_string())
            .bind(random_token())
            .bind(&hash)
            .bind(now)
            .bind(now)
            .execute(&self.db)
            .await?;
            return Ok(AdminOutput::Data(json!(true)));
        }

        let count = optional_i64(params, "generate_count").unwrap_or_default();
        if count <= 0 {
            return Ok(AdminOutput::Data(json!(true)));
        }
        let suffix = required_string(params, "email_suffix")?;
        let input_password = params
            .get("password")
            .filter(|value| !value.is_empty())
            .cloned();
        let expired_at = optional_i64(params, "expired_at");
        let mut generated: Vec<(String, String, String, String)> = Vec::new();
        let mut tx = self.db.begin().await?;
        for _ in 0..count {
            let email = format!("{}@{}", random_char(6), suffix);
            let password_plain = input_password.clone().unwrap_or_else(|| email.clone());
            let hash = bcrypt::hash(&password_plain, bcrypt::DEFAULT_COST)
                .map_err(|_| ApiError::internal("failed to hash password"))?;
            let uuid = Uuid::new_v4().to_string();
            let token = random_token();
            sqlx::query(
                r#"
                INSERT INTO v2_user (
                    email, plan_id, group_id, transfer_enable, device_limit, expired_at,
                    uuid, token, password, password_algo, created_at, updated_at
                )
                VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, NULL, ?, ?)
                "#,
            )
            .bind(&email)
            .bind(plan_id)
            .bind(group_id)
            .bind(transfer_enable)
            .bind(device_limit)
            .bind(expired_at)
            .bind(&uuid)
            .bind(&token)
            .bind(&hash)
            .bind(now)
            .bind(now)
            .execute(&mut *tx)
            .await?;
            generated.push((email, password_plain, uuid, token));
        }
        tx.commit().await?;

        let create_date = local_datetime(now);
        let mut body = String::from("账号,密码,过期时间,UUID,创建时间,订阅地址\r\n");
        let expire = expired_at
            .map(local_datetime)
            .unwrap_or_else(|| "长期有效".to_string());
        for (email, password_plain, uuid, token) in generated {
            let url = self.config.subscribe_url_for_token(&token);
            body.push_str(&format!(
                "{email},{password_plain},{expire},{uuid},{create_date},{url}\r\n"
            ));
        }
        Ok(AdminOutput::Csv {
            filename: "users.csv".to_string(),
            body,
        })
    }

    async fn user_dump_csv(
        &self,
        params: &HashMap<String, String>,
    ) -> Result<AdminOutput, ApiError> {
        // Ports UserController::dumpCSV (:174-202). device_limit is emitted for
        // real here — the Laravel row reads a `devce_limit` typo that always
        // produced an empty column.
        let clauses = self.user_filter_clauses(params).await?;
        let mut builder = QueryBuilder::<MySql>::new(
            "SELECT u.email AS email, u.balance AS balance, \
             u.commission_balance AS commission_balance, u.transfer_enable AS transfer_enable, \
             u.u AS u, u.d AS d, u.device_limit AS device_limit, u.expired_at AS expired_at, \
             p.name AS plan_name, u.token AS token \
             FROM v2_user u LEFT JOIN v2_plan p ON p.id = u.plan_id WHERE 1 = 1",
        );
        push_user_where(&mut builder, &clauses);
        builder.push(" ORDER BY u.id ASC");
        let rows = builder
            .build_query_as::<UserDumpRow>()
            .fetch_all(&self.db)
            .await?;

        let mut body = String::from(
            "\u{feff}邮箱,余额,推广佣金,总流量,设备数限制,剩余流量,套餐到期时间,订阅计划,订阅地址\r\n",
        );
        for row in rows {
            let expire = row
                .expired_at
                .map(local_datetime)
                .unwrap_or_else(|| "长期有效".to_string());
            let balance = row.balance as f64 / 100.0;
            let commission = row.commission_balance as f64 / 100.0;
            let transfer = if row.transfer_enable != 0 {
                row.transfer_enable as f64 / GIB as f64
            } else {
                0.0
            };
            let device = row
                .device_limit
                .map(|value| value.to_string())
                .unwrap_or_default();
            let not_use = (row.transfer_enable - (row.u + row.d)) as f64 / GIB as f64;
            let plan = row.plan_name.unwrap_or_else(|| "无订阅".to_string());
            let url = self.config.subscribe_url_for_token(&row.token);
            body.push_str(&format!(
                "{},{},{},{}, {}, {},{},{},{}\r\n",
                row.email, balance, commission, transfer, device, not_use, expire, plan, url
            ));
        }
        Ok(AdminOutput::Csv {
            filename: "users.csv".to_string(),
            body,
        })
    }

    async fn user_bulk_flag(
        &self,
        params: &HashMap<String, String>,
        column: &str,
        value: i64,
    ) -> Result<AdminOutput, ApiError> {
        // Ports UserController::ban (:305-326): scoped to the request filter,
        // removes each matched user's sessions, then flips the flag in bulk.
        if column != "banned" {
            return Err(ApiError::legacy("Invalid user flag"));
        }
        let ids = self.filtered_user_ids(params, false).await?;
        if ids.is_empty() {
            return Ok(AdminOutput::Data(json!(true)));
        }
        for id in &ids {
            self.remove_user_sessions(*id).await;
        }
        let mut builder = QueryBuilder::<MySql>::new("UPDATE v2_user SET banned = ");
        builder.push_bind(value);
        builder.push(", updated_at = ");
        builder.push_bind(Utc::now().timestamp());
        builder.push(" WHERE id IN (");
        let mut separated = builder.separated(", ");
        for id in &ids {
            separated.push_bind(*id);
        }
        builder.push(")");
        builder.build().execute(&self.db).await?;
        Ok(AdminOutput::Data(json!(true)))
    }

    async fn user_bulk_delete(
        &self,
        params: &HashMap<String, String>,
    ) -> Result<AdminOutput, ApiError> {
        // Ports UserController::allDel (:328-359): scoped to the request filter,
        // cascading orders / invite codes / tickets and detaching referrals for
        // each user inside a single transaction, then deleting the matched users.
        let ids = self.filtered_user_ids(params, false).await?;
        if ids.is_empty() {
            return Ok(AdminOutput::Data(json!(true)));
        }
        let mut tx = self.db.begin().await?;
        for id in &ids {
            self.delete_user_cascade(&mut tx, *id).await?;
        }
        let mut builder = QueryBuilder::<MySql>::new("DELETE FROM v2_user WHERE id IN (");
        let mut separated = builder.separated(", ");
        for id in &ids {
            separated.push_bind(*id);
        }
        builder.push(")");
        builder.build().execute(&mut *tx).await?;
        tx.commit().await?;
        Ok(AdminOutput::Data(json!(true)))
    }

    async fn del_user(&self, id: i64) -> Result<AdminOutput, ApiError> {
        // Ports UserController::delUser (:361-391): single-user cascade delete.
        let exists: Option<i64> = sqlx::query_scalar("SELECT id FROM v2_user WHERE id = ? LIMIT 1")
            .bind(id)
            .fetch_optional(&self.db)
            .await?;
        if exists.is_none() {
            return Err(ApiError::legacy("用户不存在"));
        }
        let mut tx = self.db.begin().await?;
        self.delete_user_cascade(&mut tx, id).await?;
        sqlx::query("DELETE FROM v2_user WHERE id = ?")
            .bind(id)
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        Ok(AdminOutput::Data(json!(true)))
    }

    async fn user_set_invite(
        &self,
        params: &HashMap<String, String>,
    ) -> Result<AdminOutput, ApiError> {
        let user_id = required_i64(params, "user_id")?;
        let invite_user_id = optional_i64(params, "invite_user_id");
        sqlx::query("UPDATE v2_user SET invite_user_id = ?, updated_at = ? WHERE id = ?")
            .bind(invite_user_id)
            .bind(Utc::now().timestamp())
            .bind(user_id)
            .execute(&self.db)
            .await?;
        Ok(AdminOutput::Data(json!(true)))
    }

    async fn order_fetch(&self, params: &HashMap<String, String>) -> Result<AdminOutput, ApiError> {
        let (current, page_size) = page(params);
        let total: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM v2_order")
            .fetch_one(&self.db)
            .await?;
        let data = fetch_json_list_page(
            &self.db,
            r#"
            SELECT JSON_OBJECT(
                'id', o.id, 'invite_user_id', o.invite_user_id, 'user_id', o.user_id,
                'email', u.email, 'plan_id', o.plan_id, 'plan_name', p.name, 'coupon_id', o.coupon_id,
                'type', o.type, 'period', o.period, 'trade_no', o.trade_no,
                'callback_no', o.callback_no, 'total_amount', o.total_amount,
                'handling_amount', o.handling_amount, 'discount_amount', o.discount_amount,
                'surplus_amount', o.surplus_amount, 'refund_amount', o.refund_amount,
                'balance_amount', o.balance_amount, 'surplus_order_ids', CAST(o.surplus_order_ids AS JSON),
                'status', o.status, 'commission_status', o.commission_status,
                'commission_balance', o.commission_balance, 'payment_id', o.payment_id,
                'paid_at', o.paid_at, 'created_at', o.created_at, 'updated_at', o.updated_at
            )
            FROM v2_order o
            LEFT JOIN v2_user u ON u.id = o.user_id
            LEFT JOIN v2_plan p ON p.id = o.plan_id
            ORDER BY o.created_at DESC
            LIMIT ? OFFSET ?
            "#,
            page_size,
            offset(current, page_size),
        )
        .await?;
        Ok(AdminOutput::Page { data, total })
    }

    async fn order_detail(&self, id: i64) -> Result<AdminOutput, ApiError> {
        let value = fetch_json_one(
            &self.db,
            r#"
            SELECT JSON_OBJECT(
                'id', o.id, 'invite_user_id', o.invite_user_id, 'user_id', o.user_id,
                'plan_id', o.plan_id, 'coupon_id', o.coupon_id, 'type', o.type, 'period', o.period,
                'trade_no', o.trade_no, 'callback_no', o.callback_no, 'total_amount', o.total_amount,
                'handling_amount', o.handling_amount, 'discount_amount', o.discount_amount,
                'surplus_amount', o.surplus_amount, 'refund_amount', o.refund_amount,
                'balance_amount', o.balance_amount, 'surplus_order_ids', CAST(o.surplus_order_ids AS JSON),
                'status', o.status, 'commission_status', o.commission_status,
                'commission_balance', o.commission_balance, 'payment_id', o.payment_id,
                'paid_at', o.paid_at, 'created_at', o.created_at, 'updated_at', o.updated_at
            )
            FROM v2_order o
            WHERE o.id = ?
            LIMIT 1
            "#,
            id,
        )
        .await?
        .ok_or_else(|| ApiError::legacy("订单不存在"))?;
        Ok(AdminOutput::Data(value))
    }

    async fn order_update(
        &self,
        params: &HashMap<String, String>,
    ) -> Result<AdminOutput, ApiError> {
        let trade_no = required_string(params, "trade_no")?;
        if let Some(value) = optional_i64(params, "commission_status") {
            sqlx::query(
                "UPDATE v2_order SET commission_status = ?, updated_at = ? WHERE trade_no = ?",
            )
            .bind(value)
            .bind(Utc::now().timestamp())
            .bind(trade_no)
            .execute(&self.db)
            .await?;
        }
        Ok(AdminOutput::Data(json!(true)))
    }

    async fn order_paid(&self, trade_no: String) -> Result<AdminOutput, ApiError> {
        OrderService::new(self.db.clone(), self.config.clone())
            .paid_manually(&trade_no)
            .await?;
        Ok(AdminOutput::Data(json!(true)))
    }

    async fn order_cancel(&self, trade_no: String) -> Result<AdminOutput, ApiError> {
        // Ports Admin\OrderController::cancel + OrderService::cancel (:273-291):
        // only pending orders can be cancelled, and the balance paid toward the
        // order is refunded to the user via addBalance.
        let order: (i64, i64, Option<i64>) = sqlx::query_as(
            "SELECT status, user_id, balance_amount FROM v2_order WHERE trade_no = ? LIMIT 1",
        )
        .bind(&trade_no)
        .fetch_optional(&self.db)
        .await?
        .ok_or_else(|| ApiError::legacy("订单不存在"))?;
        let (status, user_id, balance_amount) = order;
        if status != 0 {
            return Err(ApiError::legacy("只能对待支付的订单进行操作"));
        }
        let now = Utc::now().timestamp();
        let mut tx = self.db.begin().await?;
        sqlx::query("UPDATE v2_order SET status = 2, updated_at = ? WHERE trade_no = ?")
            .bind(now)
            .bind(&trade_no)
            .execute(&mut *tx)
            .await?;
        if let Some(balance) = balance_amount.filter(|value| *value != 0) {
            // UserService::addBalance: lock the row, add, and reject a negative result.
            let current: i64 =
                sqlx::query_scalar("SELECT balance FROM v2_user WHERE id = ? FOR UPDATE")
                    .bind(user_id)
                    .fetch_optional(&mut *tx)
                    .await?
                    .ok_or_else(|| ApiError::legacy("更新失败"))?;
            let updated = current + balance;
            if updated < 0 {
                return Err(ApiError::legacy("更新失败"));
            }
            sqlx::query("UPDATE v2_user SET balance = ?, updated_at = ? WHERE id = ?")
                .bind(updated)
                .bind(now)
                .bind(user_id)
                .execute(&mut *tx)
                .await?;
        }
        tx.commit().await?;
        Ok(AdminOutput::Data(json!(true)))
    }

    async fn order_assign(
        &self,
        params: &HashMap<String, String>,
    ) -> Result<AdminOutput, ApiError> {
        let email = required_string(params, "email")?;
        let plan_id = required_i64(params, "plan_id")?;
        let user_id: i64 = sqlx::query_scalar("SELECT id FROM v2_user WHERE email = ? LIMIT 1")
            .bind(email)
            .fetch_optional(&self.db)
            .await?
            .ok_or_else(|| ApiError::legacy("该用户不存在"))?;
        let plan_exists: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM v2_plan WHERE id = ?")
            .bind(plan_id)
            .fetch_one(&self.db)
            .await?;
        if plan_exists == 0 {
            return Err(ApiError::legacy("该订阅不存在"));
        }
        let now = Utc::now().timestamp();
        let trade_no = format!("{}{}", now, Uuid::new_v4().simple());
        sqlx::query(
            r#"
            INSERT INTO v2_order (
                user_id, plan_id, period, trade_no, total_amount, type, status,
                commission_status, commission_balance, created_at, updated_at
            )
            VALUES (?, ?, ?, ?, ?, 1, 0, 0, 0, ?, ?)
            "#,
        )
        .bind(user_id)
        .bind(plan_id)
        .bind(required_string(params, "period")?)
        .bind(&trade_no)
        .bind(optional_i64(params, "total_amount").unwrap_or_default())
        .bind(now)
        .bind(now)
        .execute(&self.db)
        .await?;
        Ok(AdminOutput::Data(json!(trade_no)))
    }

    async fn plan_drop(&self, params: &HashMap<String, String>) -> Result<AdminOutput, ApiError> {
        // Ports PlanController::drop (:70-87): reject deletion while any order or
        // user still references the plan.
        let id = required_i64(params, "id")?;
        let has_order: Option<i64> =
            sqlx::query_scalar("SELECT id FROM v2_order WHERE plan_id = ? LIMIT 1")
                .bind(id)
                .fetch_optional(&self.db)
                .await?;
        if has_order.is_some() {
            return Err(ApiError::legacy("该订阅下存在订单无法删除"));
        }
        let has_user: Option<i64> =
            sqlx::query_scalar("SELECT id FROM v2_user WHERE plan_id = ? LIMIT 1")
                .bind(id)
                .fetch_optional(&self.db)
                .await?;
        if has_user.is_some() {
            return Err(ApiError::legacy("该订阅下存在用户无法删除"));
        }
        let exists: Option<i64> = sqlx::query_scalar("SELECT id FROM v2_plan WHERE id = ? LIMIT 1")
            .bind(id)
            .fetch_optional(&self.db)
            .await?;
        if exists.is_none() {
            return Err(ApiError::legacy("该订阅ID不存在"));
        }
        sqlx::query("DELETE FROM v2_plan WHERE id = ?")
            .bind(id)
            .execute(&self.db)
            .await?;
        Ok(AdminOutput::Data(json!(true)))
    }

    async fn server_group_drop(
        &self,
        params: &HashMap<String, String>,
    ) -> Result<AdminOutput, ApiError> {
        // Ports GroupController::drop (:58-90): reject while any vmess/vless node,
        // plan, or user still references the group.
        let id = required_i64(params, "id")?;
        let exists: Option<i64> =
            sqlx::query_scalar("SELECT id FROM v2_server_group WHERE id = ? LIMIT 1")
                .bind(id)
                .fetch_optional(&self.db)
                .await?;
        if exists.is_none() {
            return Err(ApiError::legacy("组不存在"));
        }
        for table in ["v2_server_vmess", "v2_server_vless"] {
            let group_ids: Vec<String> =
                sqlx::query_scalar(AssertSqlSafe(format!("SELECT group_id FROM {table}")))
                    .fetch_all(&self.db)
                    .await?;
            if group_ids
                .iter()
                .any(|group_id| group_id_contains(group_id, id))
            {
                return Err(ApiError::legacy("该组已被节点所使用，无法删除"));
            }
        }
        let plan_used: Option<i64> =
            sqlx::query_scalar("SELECT id FROM v2_plan WHERE group_id = ? LIMIT 1")
                .bind(id)
                .fetch_optional(&self.db)
                .await?;
        if plan_used.is_some() {
            return Err(ApiError::legacy("该组已被订阅所使用，无法删除"));
        }
        let user_used: Option<i64> =
            sqlx::query_scalar("SELECT id FROM v2_user WHERE group_id = ? LIMIT 1")
                .bind(id)
                .fetch_optional(&self.db)
                .await?;
        if user_used.is_some() {
            return Err(ApiError::legacy("该组已被用户所使用，无法删除"));
        }
        sqlx::query("DELETE FROM v2_server_group WHERE id = ?")
            .bind(id)
            .execute(&self.db)
            .await?;
        Ok(AdminOutput::Data(json!(true)))
    }

    /// Loads the raw `group_id` JSON of every configured server across all node
    /// tables, for the group `server_count` / drop-guard membership checks.
    async fn all_server_group_ids(&self) -> Result<Vec<String>, ApiError> {
        let mut group_ids = Vec::new();
        for (_, table) in SERVER_TABLES {
            let rows: Vec<String> =
                sqlx::query_scalar(AssertSqlSafe(format!("SELECT group_id FROM {table}")))
                    .fetch_all(&self.db)
                    .await
                    .unwrap_or_default();
            group_ids.extend(rows);
        }
        Ok(group_ids)
    }

    async fn server_group_fetch(&self) -> Result<AdminOutput, ApiError> {
        // server_count counts nodes whose group_id array includes the group,
        // mirroring GroupController::fetch over ServerService::getAllServers.
        let mut groups = fetch_json_list(
            &self.db,
            r#"
            SELECT JSON_OBJECT(
                'id', id, 'name', name, 'created_at', created_at, 'updated_at', updated_at,
                'user_count', (SELECT COUNT(*) FROM v2_user WHERE group_id = v2_server_group.id),
                'server_count', 0
            )
            FROM v2_server_group
            ORDER BY id DESC
            "#,
        )
        .await?;
        let group_ids = self.all_server_group_ids().await?;
        for group in &mut groups {
            let Some(object) = group.as_object_mut() else {
                continue;
            };
            let id = object.get("id").and_then(Value::as_i64).unwrap_or_default();
            let count = group_ids
                .iter()
                .filter(|group_id| group_id_contains(group_id, id))
                .count() as i64;
            object.insert("server_count".to_string(), json!(count));
        }
        Ok(AdminOutput::Data(json!(groups)))
    }

    async fn server_group_save(
        &self,
        params: &HashMap<String, String>,
    ) -> Result<AdminOutput, ApiError> {
        let now = Utc::now().timestamp();
        if let Some(id) = optional_i64(params, "id") {
            sqlx::query("UPDATE v2_server_group SET name = ?, updated_at = ? WHERE id = ?")
                .bind(required_string(params, "name")?)
                .bind(now)
                .bind(id)
                .execute(&self.db)
                .await?;
        } else {
            sqlx::query(
                "INSERT INTO v2_server_group (name, created_at, updated_at) VALUES (?, ?, ?)",
            )
            .bind(required_string(params, "name")?)
            .bind(now)
            .bind(now)
            .execute(&self.db)
            .await?;
        }
        Ok(AdminOutput::Data(json!(true)))
    }

    async fn server_route_fetch(&self) -> Result<AdminOutput, ApiError> {
        Ok(AdminOutput::Data(json!(
            fetch_json_list(
                &self.db,
                r#"
            SELECT JSON_OBJECT(
                'id', id, 'remarks', remarks, 'match', CAST(`match` AS JSON),
                'action', action, 'action_value', action_value,
                'created_at', created_at, 'updated_at', updated_at
            )
            FROM v2_server_route
            ORDER BY id DESC
            "#
            )
            .await?
        )))
    }

    async fn server_route_save(
        &self,
        params: &HashMap<String, String>,
    ) -> Result<AdminOutput, ApiError> {
        let now = Utc::now().timestamp();
        let matches = optional_json_array_string(params, "match")
            .unwrap_or_else(|| json_string(&Value::Array(json_array_param(params, "match"))));
        if let Some(id) = optional_i64(params, "id") {
            sqlx::query(
                "UPDATE v2_server_route SET remarks = ?, `match` = ?, action = ?, action_value = ?, updated_at = ? WHERE id = ?",
            )
            .bind(required_string(params, "remarks")?)
            .bind(matches)
            .bind(required_string(params, "action")?)
            .bind(params.get("action_value"))
            .bind(now)
            .bind(id)
            .execute(&self.db)
            .await?;
        } else {
            sqlx::query(
                "INSERT INTO v2_server_route (remarks, `match`, action, action_value, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?)",
            )
            .bind(required_string(params, "remarks")?)
            .bind(matches)
            .bind(required_string(params, "action")?)
            .bind(params.get("action_value"))
            .bind(now)
            .bind(now)
            .execute(&self.db)
            .await?;
        }
        Ok(AdminOutput::Data(json!(true)))
    }

    async fn server_nodes(&self) -> Result<AdminOutput, ApiError> {
        let mut nodes = Vec::new();
        for (kind, table) in SERVER_TABLES {
            let rows = fetch_json_list(
                &self.db,
                &format!(
                    r#"
                    SELECT JSON_OBJECT(
                        'id', id, 'name', name, 'group_id', CAST(group_id AS JSON),
                        'route_id', CAST(route_id AS JSON), 'type', '{kind}', 'host', host,
                        'port', port, 'server_port', server_port, 'show', `show`, 'rate', rate,
                        'parent_id', parent_id, 'online', 0, 'last_check_at', NULL,
                        'last_push_at', NULL, 'available_status', 0, 'sort', sort,
                        'created_at', created_at, 'updated_at', updated_at
                    )
                    FROM {table}
                    ORDER BY sort ASC
                    "#
                ),
            )
            .await
            .unwrap_or_default();
            nodes.extend(rows);
        }
        // Hydrate node health from the cache keys the node API writes, keyed on
        // `parent_id ?? id`. Ports ServerService::mergeData (:407-421); the read is
        // best-effort so a Redis outage still returns the node list.
        let mut conn = self.redis.get_multiplexed_async_connection().await.ok();
        let now = Utc::now().timestamp();
        for node in &mut nodes {
            let Some(object) = node.as_object_mut() else {
                continue;
            };
            let node_type = object
                .get("type")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_ascii_uppercase();
            let id = object.get("id").and_then(Value::as_i64).unwrap_or_default();
            let check_id = object
                .get("parent_id")
                .and_then(Value::as_i64)
                .unwrap_or(id);
            let (mut online, mut last_check_at, mut last_push_at) = (None, None, None);
            if let Some(conn) = conn.as_mut() {
                online = conn
                    .get::<_, Option<i64>>(format!("SERVER_{node_type}_ONLINE_USER_{check_id}"))
                    .await
                    .ok()
                    .flatten();
                last_check_at = conn
                    .get::<_, Option<i64>>(format!("SERVER_{node_type}_LAST_CHECK_AT_{check_id}"))
                    .await
                    .ok()
                    .flatten();
                last_push_at = conn
                    .get::<_, Option<i64>>(format!("SERVER_{node_type}_LAST_PUSH_AT_{check_id}"))
                    .await
                    .ok()
                    .flatten();
            }
            // ServerService::mergeData (:407-421) sets exactly these four cache-derived
            // fields keyed on parent_id ?? id; it does not add is_online.
            let available_status = node_available_status(now, last_check_at, last_push_at);
            object.insert("online".to_string(), json!(online));
            object.insert("last_check_at".to_string(), json!(last_check_at));
            object.insert("last_push_at".to_string(), json!(last_push_at));
            object.insert("available_status".to_string(), json!(available_status));
        }
        Ok(AdminOutput::Data(json!(nodes)))
    }

    async fn server_save(
        &self,
        path: &str,
        params: &HashMap<String, String>,
    ) -> Result<AdminOutput, ApiError> {
        let table = server_table_from_path(path)?;
        let kind = server_kind_from_path(path)?;
        let now = Utc::now().timestamp();
        let values = server_save_values(kind, params)?;
        if let Some(id) = optional_i64(params, "id") {
            let mut builder = QueryBuilder::<MySql>::new(format!("UPDATE {table} SET "));
            let mut first = true;
            for (column, value) in &values {
                if !first {
                    builder.push(", ");
                }
                first = false;
                builder.push(format!("`{column}` = "));
                push_admin_sql_bind(&mut builder, value);
            }
            builder.push(", `updated_at` = ");
            builder.push_bind(now);
            builder.push(" WHERE id = ");
            builder.push_bind(id);
            let result = builder.build().execute(&self.db).await?;
            if result.rows_affected() == 0 {
                return Err(ApiError::legacy("服务器不存在"));
            }
        } else {
            let mut builder = QueryBuilder::<MySql>::new(format!("INSERT INTO {table} ("));
            let mut columns = builder.separated(", ");
            for (column, _) in &values {
                columns.push(format!("`{column}`"));
            }
            columns.push("`created_at`");
            columns.push("`updated_at`");
            builder.push(") VALUES (");
            let mut placeholders = builder.separated(", ");
            for (_, value) in &values {
                push_admin_sql_value(&mut placeholders, value);
            }
            placeholders.push_bind(now);
            placeholders.push_bind(now);
            builder.push(")");
            builder.build().execute(&self.db).await?;
        }
        Ok(AdminOutput::Data(json!(true)))
    }

    async fn server_copy(
        &self,
        path: &str,
        params: &HashMap<String, String>,
    ) -> Result<AdminOutput, ApiError> {
        let table = server_table_from_path(path)?;
        let kind = server_kind_from_path(path)?;
        let id = required_i64(params, "id")?;
        let columns = server_copy_columns(kind)?;
        let mut builder = QueryBuilder::<MySql>::new(format!("INSERT INTO {table} ("));
        let mut insert_columns = builder.separated(", ");
        for column in columns {
            insert_columns.push(format!("`{column}`"));
        }
        insert_columns.push("`created_at`");
        insert_columns.push("`updated_at`");
        builder.push(") SELECT ");
        let mut select_columns = builder.separated(", ");
        for column in columns {
            if *column == "show" {
                select_columns.push("0");
            } else {
                select_columns.push(format!("`{column}`"));
            }
        }
        select_columns.push("UNIX_TIMESTAMP()");
        select_columns.push("UNIX_TIMESTAMP()");
        builder.push(format!(" FROM {table} WHERE id = "));
        builder.push_bind(id);
        let result = builder.build().execute(&self.db).await?;
        if result.rows_affected() == 0 {
            return Err(ApiError::legacy("服务器不存在"));
        }
        Ok(AdminOutput::Data(json!(true)))
    }

    async fn server_sort(&self, params: &HashMap<String, String>) -> Result<AdminOutput, ApiError> {
        for (key, value) in params {
            let Some((kind, raw_id)) = key.split_once('[') else {
                continue;
            };
            let id = raw_id.trim_end_matches(']');
            let Some((_, table)) = SERVER_TABLES.iter().find(|(item, _)| *item == kind) else {
                continue;
            };
            if let (Ok(id), Ok(sort)) = (id.parse::<i64>(), value.parse::<i64>()) {
                sqlx::query(AssertSqlSafe(format!(
                    "UPDATE {table} SET sort = ? WHERE id = ?"
                )))
                .bind(sort)
                .bind(id)
                .execute(&self.db)
                .await?;
            }
        }
        Ok(AdminOutput::Data(json!(true)))
    }

    async fn order_income_between(&self, start: i64, end: i64) -> Result<i64, ApiError> {
        Ok(sqlx::query_scalar(
            "SELECT CAST(COALESCE(SUM(total_amount), 0) AS SIGNED) FROM v2_order \
             WHERE created_at >= ? AND created_at < ? AND status NOT IN (0, 2)",
        )
        .bind(start)
        .bind(end)
        .fetch_one(&self.db)
        .await?)
    }

    async fn commission_payout_between(&self, start: i64, end: i64) -> Result<i64, ApiError> {
        Ok(sqlx::query_scalar(
            "SELECT CAST(COALESCE(SUM(get_amount), 0) AS SIGNED) FROM v2_commission_log \
             WHERE created_at >= ? AND created_at < ?",
        )
        .bind(start)
        .bind(end)
        .fetch_one(&self.db)
        .await?)
    }

    async fn stat_summary(&self) -> Result<AdminOutput, ApiError> {
        // Ports StatController::getOverride (:26-66).
        let now = Utc::now().timestamp();
        let today = start_of_today();
        let month = first_day_of_month();
        let last_month = first_day_of_previous_month();

        let online_user: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM v2_user WHERE t >= ?")
            .bind(now - 600)
            .fetch_one(&self.db)
            .await?;
        let month_income = self.order_income_between(month, now).await?;
        let day_income = self.order_income_between(today, now).await?;
        let last_month_income = self.order_income_between(last_month, month).await?;
        let month_register_total: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM v2_user WHERE created_at >= ? AND created_at < ?",
        )
        .bind(month)
        .bind(now)
        .fetch_one(&self.db)
        .await?;
        let day_register_total: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM v2_user WHERE created_at >= ? AND created_at < ?",
        )
        .bind(today)
        .bind(now)
        .fetch_one(&self.db)
        .await?;
        let ticket_pending_total: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM v2_ticket WHERE status = 0 AND reply_status = 0",
        )
        .fetch_one(&self.db)
        .await?;
        let commission_pending_total: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM v2_order WHERE commission_status = 0 AND invite_user_id IS NOT NULL \
             AND status NOT IN (0, 2) AND commission_balance > 0",
        )
        .fetch_one(&self.db)
        .await?;
        let commission_month_payout = self.commission_payout_between(month, now).await?;
        let commission_last_month_payout =
            self.commission_payout_between(last_month, month).await?;

        Ok(AdminOutput::Data(json!({
            "online_user": online_user,
            "month_income": month_income,
            "month_register_total": month_register_total,
            "day_register_total": day_register_total,
            "ticket_pending_total": ticket_pending_total,
            "commission_pending_total": commission_pending_total,
            "day_income": day_income,
            "last_month_income": last_month_income,
            "commission_month_payout": commission_month_payout,
            "commission_last_month_payout": commission_last_month_payout,
        })))
    }

    /// Resolves `(canonical_type, id) -> name` for every root (parent_id IS NULL)
    /// node, used to label the server rank rows.
    async fn server_name_map(&self) -> Result<HashMap<(String, i64), String>, ApiError> {
        let mut names = HashMap::new();
        for (kind, table) in SERVER_TABLES {
            let rows: Vec<(i64, String)> = QueryBuilder::<MySql>::new(format!(
                "SELECT id, name FROM {table} WHERE parent_id IS NULL"
            ))
            .build_query_as()
            .fetch_all(&self.db)
            .await
            .unwrap_or_default();
            for (id, name) in rows {
                names.insert(((*kind).to_string(), id), name);
            }
        }
        Ok(names)
    }

    async fn server_rank(&self, today: bool) -> Result<AdminOutput, ApiError> {
        // Ports StatController::getServerLastRank / getServerTodayRank.
        let (start, end) = if today {
            (start_of_today(), Utc::now().timestamp())
        } else {
            (start_of_yesterday(), start_of_today())
        };
        let rows: Vec<(i64, String, i64, i64)> = sqlx::query_as(
            "SELECT server_id, server_type, u, d FROM v2_stat_server \
             WHERE record_at >= ? AND record_at < ? AND record_type = 'd' \
             ORDER BY (u + d) DESC LIMIT 15",
        )
        .bind(start)
        .bind(end)
        .fetch_all(&self.db)
        .await
        .unwrap_or_default();

        let names = self.server_name_map().await?;
        let mut result: Vec<Value> = rows
            .into_iter()
            .map(|(server_id, server_type, u, d)| {
                let total = (u + d) as f64 / GIB as f64;
                let key = (normalize_stat_server_type(&server_type), server_id);
                let server_name = names.get(&key).cloned();
                json!({
                    "server_id": server_id,
                    "server_type": server_type,
                    "u": u,
                    "d": d,
                    "total": total,
                    "server_name": server_name,
                })
            })
            .collect();
        result.sort_by(|a, b| {
            let left = a["total"].as_f64().unwrap_or_default();
            let right = b["total"].as_f64().unwrap_or_default();
            right.total_cmp(&left)
        });
        Ok(AdminOutput::Data(json!(result)))
    }

    async fn user_rank(&self, today: bool) -> Result<AdminOutput, ApiError> {
        // Ports StatController::getUserTodayRank / getUserLastRank: weight traffic
        // by server_rate, aggregate per user, then keep the top 15.
        let (start, end) = if today {
            (start_of_today(), Utc::now().timestamp())
        } else {
            (start_of_yesterday(), start_of_today())
        };
        let rows: Vec<(i64, f64, i64, i64, Option<String>)> = sqlx::query_as(
            "SELECT s.user_id, CAST(s.server_rate AS DOUBLE), s.u, s.d, u.email \
             FROM v2_stat_user s LEFT JOIN v2_user u ON u.id = s.user_id \
             WHERE s.record_at >= ? AND s.record_at < ? AND s.record_type = 'd' \
             ORDER BY (s.u + s.d) DESC LIMIT 30",
        )
        .bind(start)
        .bind(end)
        .fetch_all(&self.db)
        .await
        .unwrap_or_default();

        // Keep the first row's raw u/d per user (Laravel only sums `total`, not the
        // displayed u/d columns) alongside the aggregated weighted total and email.
        let mut order: Vec<i64> = Vec::new();
        let mut totals: HashMap<i64, (String, f64, i64, i64)> = HashMap::new();
        for (user_id, server_rate, u, d, email) in rows {
            let total = (u + d) as f64 * server_rate / GIB as f64;
            match totals.get_mut(&user_id) {
                Some(entry) => entry.1 += total,
                None => {
                    order.push(user_id);
                    totals.insert(
                        user_id,
                        (email.unwrap_or_else(|| "null".to_string()), total, u, d),
                    );
                }
            }
        }
        let mut result: Vec<Value> = order
            .into_iter()
            .filter_map(|user_id| {
                totals.get(&user_id).map(|(email, total, u, d)| {
                    json!({ "user_id": user_id, "email": email, "u": u, "d": d, "total": total })
                })
            })
            .collect();
        result.sort_by(|a, b| {
            let left = a["total"].as_f64().unwrap_or_default();
            let right = b["total"].as_f64().unwrap_or_default();
            right.total_cmp(&left)
        });
        result.truncate(15);
        Ok(AdminOutput::Data(json!(result)))
    }

    async fn order_stat(&self) -> Result<AdminOutput, ApiError> {
        // Ports StatController::getOrder (:68-108): five series per recorded day,
        // newest 31 days, flattened then reversed to run oldest-first.
        let rows: Vec<(i64, i64, i64, i64, i64, i64)> = sqlx::query_as(
            "SELECT record_at, register_count, paid_total, paid_count, commission_total, commission_count \
             FROM v2_stat WHERE record_type = 'd' ORDER BY record_at DESC LIMIT 31",
        )
        .fetch_all(&self.db)
        .await
        .unwrap_or_default();

        let mut result: Vec<Value> = Vec::with_capacity(rows.len() * 5);
        for (
            record_at,
            register_count,
            paid_total,
            paid_count,
            commission_total,
            commission_count,
        ) in rows
        {
            let date = local_month_day(record_at);
            result.push(json!({ "type": "注册人数", "date": date, "value": register_count }));
            result.push(
                json!({ "type": "收款金额", "date": date, "value": paid_total as f64 / 100.0 }),
            );
            result.push(json!({ "type": "收款笔数", "date": date, "value": paid_count }));
            result.push(json!({
                "type": "佣金金额(已发放)", "date": date, "value": commission_total as f64 / 100.0
            }));
            result.push(
                json!({ "type": "佣金笔数(已发放)", "date": date, "value": commission_count }),
            );
        }
        result.reverse();
        Ok(AdminOutput::Data(json!(result)))
    }

    async fn stat_user(&self, params: &HashMap<String, String>) -> Result<AdminOutput, ApiError> {
        let user_id = required_i64(params, "user_id")?;
        let (current, page_size) = page(params);
        let total: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM v2_stat_user WHERE user_id = ?")
            .bind(user_id)
            .fetch_one(&self.db)
            .await
            .unwrap_or_default();
        let data = fetch_json_list_page_bind(
            &self.db,
            r#"
            SELECT JSON_OBJECT('record_at', record_at, 'u', u, 'd', d, 'server_rate', server_rate)
            FROM v2_stat_user
            WHERE user_id = ?
            ORDER BY record_at DESC
            LIMIT ? OFFSET ?
            "#,
            user_id,
            page_size,
            offset(current, page_size),
        )
        .await
        .unwrap_or_default();
        Ok(AdminOutput::Page { data, total })
    }

    async fn stat_record(&self, params: &HashMap<String, String>) -> Result<AdminOutput, ApiError> {
        let (current, page_size) = page(params);
        let record_type = params
            .get("record_type")
            .map(String::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty());
        let total: i64 = if let Some(record_type) = record_type {
            sqlx::query_scalar("SELECT COUNT(*) FROM v2_stat WHERE record_type = ?")
                .bind(record_type)
                .fetch_one(&self.db)
                .await
                .unwrap_or_default()
        } else {
            sqlx::query_scalar("SELECT COUNT(*) FROM v2_stat")
                .fetch_one(&self.db)
                .await
                .unwrap_or_default()
        };
        let data = if let Some(record_type) = record_type {
            fetch_json_list_page_bind_text(
                &self.db,
                r#"
                SELECT JSON_OBJECT(
                    'id', id, 'record_at', record_at, 'record_type', record_type,
                    'order_count', order_count, 'order_total', order_total,
                    'commission_count', commission_count, 'commission_total', commission_total,
                    'paid_count', paid_count, 'paid_total', paid_total,
                    'register_count', register_count, 'invite_count', invite_count,
                    'transfer_used_total', transfer_used_total,
                    'created_at', created_at, 'updated_at', updated_at
                )
                FROM v2_stat
                WHERE record_type = ?
                ORDER BY record_at DESC
                LIMIT ? OFFSET ?
                "#,
                record_type,
                page_size,
                offset(current, page_size),
            )
            .await
            .unwrap_or_default()
        } else {
            fetch_json_list_page(
                &self.db,
                r#"
                SELECT JSON_OBJECT(
                    'id', id, 'record_at', record_at, 'record_type', record_type,
                    'order_count', order_count, 'order_total', order_total,
                    'commission_count', commission_count, 'commission_total', commission_total,
                    'paid_count', paid_count, 'paid_total', paid_total,
                    'register_count', register_count, 'invite_count', invite_count,
                    'transfer_used_total', transfer_used_total,
                    'created_at', created_at, 'updated_at', updated_at
                )
                FROM v2_stat
                ORDER BY record_at DESC
                LIMIT ? OFFSET ?
                "#,
                page_size,
                offset(current, page_size),
            )
            .await
            .unwrap_or_default()
        };
        Ok(AdminOutput::Page { data, total })
    }

    async fn system_status(&self) -> Result<AdminOutput, ApiError> {
        let snapshot = self.worker_snapshot().await?;
        let now = Utc::now().timestamp();
        let schedule_recent = snapshot
            .schedule_last_seen_at
            .map(|last_seen| now - last_seen <= 180)
            .unwrap_or(false);
        let worker_running = snapshot.worker_running(now, 180);
        Ok(AdminOutput::Data(json!({
            "schedule": schedule_recent,
            "horizon": worker_running,
            "schedule_last_runtime": snapshot.schedule_last_seen_at.unwrap_or_default(),
            "logChannel": "rust",
            "logLevel": std::env::var("RUST_LOG").unwrap_or_else(|_| "info".to_string()),
            "cacheDriver": "redis",
            "backendVersion": env!("CARGO_PKG_VERSION"),
            "frontendVersion": env!("CARGO_PKG_VERSION"),
        })))
    }

    async fn queue_stats(&self) -> Result<AdminOutput, ApiError> {
        let snapshot = self.worker_snapshot().await?;
        let now = Utc::now().timestamp();
        let worker_running = snapshot.worker_running(now, 180);
        let jobs_per_minute = snapshot
            .last_run_at
            .values()
            .filter(|last_run| now - **last_run <= 60)
            .count();
        Ok(AdminOutput::Data(json!({
            "failedJobs": snapshot.failed_jobs(),
            "jobsPerMinute": jobs_per_minute,
            "pausedMasters": 0,
            "periods": {
                "failedJobs": snapshot.failed_jobs(),
                "recentJobs": snapshot.total_jobs(),
            },
            "processes": if worker_running { 1 } else { 0 },
            "queueWithMaxRuntime": null,
            "queueWithMaxThroughput": snapshot.max_counter_key(),
            "recentJobs": snapshot.total_jobs(),
            "status": worker_running,
            "wait": {},
            "lastRunAt": snapshot.last_run_at,
            "lastSuccessAt": snapshot.last_success_at,
            "lastFailureAt": snapshot.last_failure_at,
        })))
    }

    async fn queue_workload(&self) -> Result<AdminOutput, ApiError> {
        let snapshot = self.worker_snapshot().await?;
        let now = Utc::now().timestamp();
        let rows = snapshot
            .job_names()
            .into_iter()
            .map(|name| {
                let total = snapshot.totals.get(&name).copied().unwrap_or_default();
                let failed = snapshot.failed.get(&name).copied().unwrap_or_default();
                let last_run_at = snapshot.last_run_at.get(&name).copied();
                json!({
                    "name": name,
                    "length": 0,
                    "wait": 0,
                    "processes": if last_run_at.map(|seen| now - seen <= 180).unwrap_or(false) { 1 } else { 0 },
                    "recent_jobs": total,
                    "failed_jobs": failed,
                    "last_run_at": last_run_at,
                    "last_success_at": snapshot.last_success_at.get(&name).copied(),
                    "last_failure_at": snapshot.last_failure_at.get(&name).copied(),
                })
            })
            .collect::<Vec<_>>();
        Ok(AdminOutput::Data(json!(rows)))
    }

    async fn queue_masters(&self) -> Result<AdminOutput, ApiError> {
        let snapshot = self.worker_snapshot().await?;
        let now = Utc::now().timestamp();
        let worker_running = snapshot.worker_running(now, 180);
        Ok(AdminOutput::Data(json!([{
            "name": "rust-worker",
            "status": if worker_running { "running" } else { "stale" },
            "pid": null,
            "supervisors": snapshot.job_names(),
            "last_seen_at": snapshot.last_seen_at(),
            "schedule_last_seen_at": snapshot.schedule_last_seen_at,
        }])))
    }

    async fn worker_snapshot(&self) -> Result<WorkerSnapshot, ApiError> {
        let mut conn = self
            .redis
            .get_multiplexed_async_connection()
            .await
            .map_err(|_| ApiError::internal("failed to connect redis for worker metrics"))?;
        let schedule_last_seen_at = conn
            .get::<_, Option<i64>>("SCHEDULE_LAST_CHECK_AT_")
            .await
            .map_err(|_| ApiError::internal("failed to read scheduler heartbeat"))?;
        let totals = conn
            .hgetall::<_, BTreeMap<String, i64>>("RUST_WORKER_JOBS_TOTAL")
            .await
            .map_err(|_| ApiError::internal("failed to read worker totals"))?;
        let failed = conn
            .hgetall::<_, BTreeMap<String, i64>>("RUST_WORKER_JOBS_FAILED")
            .await
            .map_err(|_| ApiError::internal("failed to read worker failures"))?;
        let last_run_at = conn
            .hgetall::<_, BTreeMap<String, i64>>("RUST_WORKER_LAST_RUN_AT")
            .await
            .map_err(|_| ApiError::internal("failed to read worker last run"))?;
        let last_success_at = conn
            .hgetall::<_, BTreeMap<String, i64>>("RUST_WORKER_LAST_SUCCESS_AT")
            .await
            .map_err(|_| ApiError::internal("failed to read worker last success"))?;
        let last_failure_at = conn
            .hgetall::<_, BTreeMap<String, i64>>("RUST_WORKER_LAST_FAILURE_AT")
            .await
            .map_err(|_| ApiError::internal("failed to read worker last failure"))?;
        Ok(WorkerSnapshot {
            schedule_last_seen_at,
            totals,
            failed,
            last_run_at,
            last_success_at,
            last_failure_at,
        })
    }

    async fn system_log(&self, params: &HashMap<String, String>) -> Result<AdminOutput, ApiError> {
        let (current, page_size) = page(params);
        let level = params
            .get("level")
            .map(String::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty());
        let total: i64 = if let Some(level) = level {
            sqlx::query_scalar("SELECT COUNT(*) FROM v2_log WHERE level = ?")
                .bind(level)
                .fetch_one(&self.db)
                .await
                .unwrap_or_default()
        } else {
            sqlx::query_scalar("SELECT COUNT(*) FROM v2_log")
                .fetch_one(&self.db)
                .await
                .unwrap_or_default()
        };
        let data = if let Some(level) = level {
            fetch_json_list_page_bind_text(
                &self.db,
                r#"
                SELECT JSON_OBJECT(
                    'id', id, 'title', title, 'level', level, 'host', host, 'uri', uri,
                    'method', method, 'data', data, 'ip', ip, 'context', context,
                    'created_at', created_at, 'updated_at', updated_at
                )
                FROM v2_log
                WHERE level = ?
                ORDER BY created_at DESC
                LIMIT ? OFFSET ?
                "#,
                level,
                page_size,
                offset(current, page_size),
            )
            .await
            .unwrap_or_default()
        } else {
            fetch_json_list_page(
                &self.db,
                r#"
                SELECT JSON_OBJECT(
                    'id', id, 'title', title, 'level', level, 'host', host, 'uri', uri,
                    'method', method, 'data', data, 'ip', ip, 'context', context,
                    'created_at', created_at, 'updated_at', updated_at
                )
                FROM v2_log
                ORDER BY created_at DESC
                LIMIT ? OFFSET ?
                "#,
                page_size,
                offset(current, page_size),
            )
            .await
            .unwrap_or_default()
        };
        Ok(AdminOutput::Page { data, total })
    }

    async fn themes(&self) -> Result<AdminOutput, ApiError> {
        let mut themes = Map::new();
        if let Ok(entries) = std::fs::read_dir(&self.config.runtime_paths.themes) {
            for entry in entries.flatten() {
                let Ok(name) = entry.file_name().into_string() else {
                    continue;
                };
                let config_path = entry.path().join("config.json");
                let Ok(content) = std::fs::read_to_string(config_path) else {
                    continue;
                };
                let Ok(config) = serde_json::from_str::<Value>(&content) else {
                    continue;
                };
                if config.get("configs").and_then(Value::as_array).is_some() {
                    themes.insert(name, config);
                }
            }
        }
        Ok(AdminOutput::Data(json!({
            "themes": themes,
            "active": "default",
        })))
    }

    async fn theme_config(&self, name: String) -> Result<AdminOutput, ApiError> {
        ensure_theme_name(&name)?;
        let path = self
            .config
            .runtime_paths
            .theme_configs
            .join(format!("{name}.php"));
        Ok(AdminOutput::Data(Value::Object(read_php_config(&path))))
    }

    async fn theme_save(&self, params: &HashMap<String, String>) -> Result<AdminOutput, ApiError> {
        let name = required_string(params, "name")?;
        ensure_theme_name(&name)?;
        let config = required_string(params, "config")?;
        let decoded =
            standard_base64_decode(&config).ok_or_else(|| ApiError::legacy("参数有误"))?;
        let config =
            serde_json::from_slice::<Value>(&decoded).map_err(|_| ApiError::legacy("参数有误"))?;
        if !config.is_object() {
            return Err(ApiError::legacy("参数有误"));
        }
        let theme_config_file = self
            .config
            .runtime_paths
            .themes
            .join(&name)
            .join("config.json");
        if !theme_config_file.exists() {
            return Err(ApiError::legacy("主题不存在"));
        }
        let path = self
            .config
            .runtime_paths
            .theme_configs
            .join(format!("{name}.php"));
        write_php_config(&path, &config)?;
        Ok(AdminOutput::Data(config))
    }

    async fn delete_by_id(&self, table: &str, id: i64) -> Result<AdminOutput, ApiError> {
        ensure_safe_table(table)?;
        sqlx::query(AssertSqlSafe(format!("DELETE FROM {table} WHERE id = ?")))
            .bind(id)
            .execute(&self.db)
            .await?;
        Ok(AdminOutput::Data(json!(true)))
    }

    async fn toggle(&self, table: &str, column: &str, id: i64) -> Result<AdminOutput, ApiError> {
        ensure_safe_table(table)?;
        ensure_toggle_column(column)?;
        sqlx::query(AssertSqlSafe(format!(
            "UPDATE {table} SET `{column}` = IF(`{column}` = 1, 0, 1), updated_at = ? WHERE id = ?"
        )))
        .bind(Utc::now().timestamp())
        .bind(id)
        .execute(&self.db)
        .await?;
        Ok(AdminOutput::Data(json!(true)))
    }

    async fn toggle_or_set_show(
        &self,
        table: &str,
        id: i64,
        params: &HashMap<String, String>,
    ) -> Result<AdminOutput, ApiError> {
        ensure_safe_table(table)?;
        let show = optional_i64(params, "show").unwrap_or(1);
        sqlx::query(AssertSqlSafe(format!(
            "UPDATE {table} SET `show` = ?, updated_at = ? WHERE id = ?"
        )))
        .bind(show)
        .bind(Utc::now().timestamp())
        .bind(id)
        .execute(&self.db)
        .await?;
        Ok(AdminOutput::Data(json!(true)))
    }

    async fn sort_ids(&self, table: &str, ids: &[i64]) -> Result<AdminOutput, ApiError> {
        ensure_safe_table(table)?;
        for (index, id) in ids.iter().enumerate() {
            sqlx::query(AssertSqlSafe(format!(
                "UPDATE {table} SET sort = ? WHERE id = ?"
            )))
            .bind((index + 1) as i64)
            .bind(id)
            .execute(&self.db)
            .await?;
        }
        Ok(AdminOutput::Data(json!(true)))
    }
}
