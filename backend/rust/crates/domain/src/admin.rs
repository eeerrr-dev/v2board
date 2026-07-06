use std::collections::{BTreeMap, HashMap, HashSet};

use chrono::{Datelike, Local, TimeZone, Utc};
use lettre::{
    AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor,
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

use crate::order::{OrderService, SUPPORTED_PAYMENT_GATEWAYS};

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
                "/laravel/resources/views/mail"
            )))),
            "config/getThemeTemplate" => Ok(AdminOutput::Data(json!(list_names(
                "/laravel/public/theme"
            )))),
            "plan/fetch" => self.plan_fetch().await,
            "payment/fetch" => self.payment_fetch().await,
            "payment/getPaymentMethods" => Ok(AdminOutput::Data(json!(payment_methods()))),
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
            "plan/drop" => {
                self.delete_by_id("v2_plan", required_i64(&params, "id")?)
                    .await
            }
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
            "server/group/drop" => {
                self.delete_by_id("v2_server_group", required_i64(&params, "id")?)
                    .await
            }
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
            "user/dumpCSV" => self.user_dump_csv().await,
            "user/sendMail" => self.send_mail_to_users(&params).await,
            "user/ban" => self.user_bulk_flag(&params, "banned", 1).await,
            "user/resetSecret" => self.user_reset_secret(required_i64(&params, "id")?).await,
            "user/delUser" => {
                self.delete_by_id("v2_user", required_i64(&params, "id")?)
                    .await
            }
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
                "invite_never_expire": 0,
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
                "try_out_hour": 1,
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
        let path = "/laravel/config/v2board.php";
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
        let emails = sqlx::query_scalar::<_, String>("SELECT email FROM v2_user WHERE banned = 0")
            .fetch_all(&self.db)
            .await?;
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
            .body(content.to_string())
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
        Ok(AdminOutput::Data(payment_form(payment)))
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
            .bind(required_string(params, "payment")?)
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
            .bind(required_string(params, "payment")?)
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
            let messages = fetch_json_list_bind(
                &self.db,
                r#"
                SELECT JSON_OBJECT(
                    'id', id, 'user_id', user_id, 'ticket_id', ticket_id, 'message', message,
                    'is_me', false, 'created_at', created_at, 'updated_at', updated_at
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
        let (current, page_size) = page(params);
        let total: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM v2_ticket")
            .fetch_one(&self.db)
            .await?;
        let data = fetch_json_list_page(
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
            ORDER BY updated_at DESC
            LIMIT ? OFFSET ?
            "#,
            page_size,
            offset(current, page_size),
        )
        .await?;
        Ok(AdminOutput::Page { data, total })
    }

    async fn ticket_reply(
        &self,
        params: &HashMap<String, String>,
    ) -> Result<AdminOutput, ApiError> {
        let id = required_i64(params, "id")?;
        let message = required_string(params, "message")?;
        let now = Utc::now().timestamp();
        let admin_id = optional_i64(params, "user_id").unwrap_or(0);
        let mut tx = self.db.begin().await?;
        sqlx::query(
            "INSERT INTO v2_ticket_message (user_id, ticket_id, message, created_at, updated_at) VALUES (?, ?, ?, ?, ?)",
        )
        .bind(admin_id)
        .bind(id)
        .bind(message)
        .bind(now)
        .bind(now)
        .execute(&mut *tx)
        .await?;
        sqlx::query("UPDATE v2_ticket SET reply_status = 1, updated_at = ? WHERE id = ?")
            .bind(now)
            .bind(id)
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        Ok(AdminOutput::Data(json!(true)))
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
        let count = optional_i64(params, "generate_count").unwrap_or(1).max(1);
        let now = Utc::now().timestamp();
        let mut csv = String::from("code\n");
        for _ in 0..count {
            let code = random_short();
            csv.push_str(&code);
            csv.push('\n');
            sqlx::query(
                r#"
                INSERT INTO v2_coupon (
                    code, name, type, value, `show`, limit_use, limit_use_with_user,
                    limit_plan_ids, limit_period, started_at, ended_at, created_at, updated_at
                )
                VALUES (?, ?, ?, ?, 1, ?, ?, ?, ?, ?, ?, ?, ?)
                "#,
            )
            .bind(&code)
            .bind(required_string(params, "name").unwrap_or_else(|_| code.clone()))
            .bind(optional_i64(params, "type").unwrap_or(1))
            .bind(optional_i64(params, "value").unwrap_or_default())
            .bind(optional_i64(params, "limit_use"))
            .bind(optional_i64(params, "limit_use_with_user"))
            .bind(optional_json_array_string(params, "limit_plan_ids"))
            .bind(optional_json_array_string(params, "limit_period"))
            .bind(optional_i64(params, "started_at").unwrap_or(now))
            .bind(optional_i64(params, "ended_at").unwrap_or(now + 31_536_000))
            .bind(now)
            .bind(now)
            .execute(&self.db)
            .await?;
        }
        Ok(AdminOutput::Csv {
            filename: "coupon.csv".to_string(),
            body: csv,
        })
    }

    async fn giftcard_generate(
        &self,
        params: &HashMap<String, String>,
    ) -> Result<AdminOutput, ApiError> {
        let count = optional_i64(params, "generate_count").unwrap_or(1).max(1);
        let now = Utc::now().timestamp();
        let mut csv = String::from("code\n");
        for _ in 0..count {
            let code = random_short();
            csv.push_str(&code);
            csv.push('\n');
            sqlx::query(
                r#"
                INSERT INTO v2_giftcard (
                    code, name, type, value, plan_id, limit_use, started_at, ended_at,
                    created_at, updated_at
                )
                VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
                "#,
            )
            .bind(&code)
            .bind(required_string(params, "name").unwrap_or_else(|_| code.clone()))
            .bind(optional_i64(params, "type").unwrap_or(1))
            .bind(optional_i64(params, "value").unwrap_or_default())
            .bind(optional_i64(params, "plan_id"))
            .bind(optional_i64(params, "limit_use"))
            .bind(optional_i64(params, "started_at"))
            .bind(optional_i64(params, "ended_at"))
            .bind(now)
            .bind(now)
            .execute(&self.db)
            .await?;
        }
        Ok(AdminOutput::Csv {
            filename: "giftcard.csv".to_string(),
            body: csv,
        })
    }

    async fn user_fetch(&self, params: &HashMap<String, String>) -> Result<AdminOutput, ApiError> {
        let (current, page_size) = page(params);
        let total: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM v2_user")
            .fetch_one(&self.db)
            .await?;
        let data = fetch_json_list_page(
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
            ORDER BY u.id DESC
            LIMIT ? OFFSET ?
            "#,
            page_size,
            offset(current, page_size),
        )
        .await?;
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
        let id = required_i64(params, "id")?;
        let now = Utc::now().timestamp();
        sqlx::query(
            r#"
            UPDATE v2_user
            SET email = COALESCE(?, email),
                plan_id = ?, expired_at = ?, transfer_enable = COALESCE(?, transfer_enable),
                device_limit = ?, balance = COALESCE(?, balance),
                commission_balance = COALESCE(?, commission_balance),
                commission_rate = ?, discount = ?, speed_limit = ?,
                is_admin = COALESCE(?, is_admin), is_staff = COALESCE(?, is_staff),
                banned = COALESCE(?, banned), remind_expire = COALESCE(?, remind_expire),
                remind_traffic = COALESCE(?, remind_traffic), updated_at = ?
            WHERE id = ?
            "#,
        )
        .bind(params.get("email"))
        .bind(optional_i64(params, "plan_id"))
        .bind(optional_i64(params, "expired_at"))
        .bind(optional_i64(params, "transfer_enable").map(|value| value * GIB))
        .bind(optional_i64(params, "device_limit"))
        .bind(optional_i64(params, "balance"))
        .bind(optional_i64(params, "commission_balance"))
        .bind(optional_i64(params, "commission_rate"))
        .bind(optional_i64(params, "discount"))
        .bind(optional_i64(params, "speed_limit"))
        .bind(optional_i64(params, "is_admin"))
        .bind(optional_i64(params, "is_staff"))
        .bind(optional_i64(params, "banned"))
        .bind(optional_i64(params, "remind_expire"))
        .bind(optional_i64(params, "remind_traffic"))
        .bind(now)
        .bind(id)
        .execute(&self.db)
        .await?;
        if let Some(password) = params.get("password").filter(|value| !value.is_empty()) {
            let hash = bcrypt::hash(password, bcrypt::DEFAULT_COST)
                .map_err(|_| ApiError::internal("failed to hash password"))?;
            sqlx::query("UPDATE v2_user SET password = ?, password_algo = 'bcrypt', password_salt = NULL WHERE id = ?")
                .bind(hash)
                .bind(id)
                .execute(&self.db)
                .await?;
        }
        Ok(AdminOutput::Data(json!(true)))
    }

    async fn staff_user_update(
        &self,
        params: &HashMap<String, String>,
    ) -> Result<AdminOutput, ApiError> {
        let id = required_i64(params, "id")?;
        let plan_id = optional_i64(params, "plan_id");
        let group_id = if let Some(plan_id) = plan_id {
            Some(
                sqlx::query_scalar::<_, i64>("SELECT group_id FROM v2_plan WHERE id = ? LIMIT 1")
                    .bind(plan_id)
                    .fetch_optional(&self.db)
                    .await?
                    .ok_or_else(|| ApiError::legacy("订阅计划不存在"))?,
            )
        } else {
            optional_i64(params, "group_id")
        };
        let now = Utc::now().timestamp();
        let result = sqlx::query(
            r#"
            UPDATE v2_user
            SET email = COALESCE(?, email),
                plan_id = COALESCE(?, plan_id),
                group_id = COALESCE(?, group_id),
                expired_at = COALESCE(?, expired_at),
                transfer_enable = COALESCE(?, transfer_enable),
                device_limit = COALESCE(?, device_limit),
                balance = COALESCE(?, balance),
                commission_balance = COALESCE(?, commission_balance),
                commission_rate = COALESCE(?, commission_rate),
                discount = COALESCE(?, discount),
                speed_limit = COALESCE(?, speed_limit),
                banned = COALESCE(?, banned),
                remind_expire = COALESCE(?, remind_expire),
                remind_traffic = COALESCE(?, remind_traffic),
                updated_at = ?
            WHERE id = ? AND is_admin = 0 AND is_staff = 0
            "#,
        )
        .bind(params.get("email"))
        .bind(plan_id)
        .bind(group_id)
        .bind(optional_i64(params, "expired_at"))
        .bind(optional_i64(params, "transfer_enable").map(|value| value * GIB))
        .bind(optional_i64(params, "device_limit"))
        .bind(optional_i64(params, "balance"))
        .bind(optional_i64(params, "commission_balance"))
        .bind(optional_i64(params, "commission_rate"))
        .bind(optional_i64(params, "discount"))
        .bind(optional_i64(params, "speed_limit"))
        .bind(optional_i64(params, "banned"))
        .bind(optional_i64(params, "remind_expire"))
        .bind(optional_i64(params, "remind_traffic"))
        .bind(now)
        .bind(id)
        .execute(&self.db)
        .await?;
        if result.rows_affected() == 0 {
            return Err(ApiError::legacy("用户不存在"));
        }
        if let Some(password) = params.get("password").filter(|value| !value.is_empty()) {
            let hash = bcrypt::hash(password, bcrypt::DEFAULT_COST)
                .map_err(|_| ApiError::internal("failed to hash password"))?;
            sqlx::query(
                "UPDATE v2_user SET password = ?, password_algo = 'bcrypt', password_salt = NULL WHERE id = ? AND is_admin = 0 AND is_staff = 0",
            )
            .bind(hash)
            .bind(id)
            .execute(&self.db)
            .await?;
        }
        Ok(AdminOutput::Data(json!(true)))
    }

    async fn staff_send_mail_to_users(
        &self,
        params: &HashMap<String, String>,
    ) -> Result<AdminOutput, ApiError> {
        let subject = required_string(params, "subject")?;
        let content = required_string(params, "content")?;
        let emails = sqlx::query_scalar::<_, String>(
            "SELECT email FROM v2_user WHERE banned = 0 AND is_admin = 0 AND is_staff = 0",
        )
        .fetch_all(&self.db)
        .await?;
        for email in emails {
            self.send_mail(&email, &subject, &content).await?;
        }
        Ok(AdminOutput::Data(json!(true)))
    }

    async fn staff_user_bulk_ban(
        &self,
        _params: &HashMap<String, String>,
    ) -> Result<AdminOutput, ApiError> {
        sqlx::query(
            "UPDATE v2_user SET banned = 1, updated_at = ? WHERE is_admin = 0 AND is_staff = 0",
        )
        .bind(Utc::now().timestamp())
        .execute(&self.db)
        .await?;
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

    async fn user_generate(
        &self,
        params: &HashMap<String, String>,
    ) -> Result<AdminOutput, ApiError> {
        let count = optional_i64(params, "generate_count").unwrap_or(1).max(1);
        let prefix = params
            .get("email_prefix")
            .map(String::as_str)
            .unwrap_or("user");
        let suffix = params
            .get("email_suffix")
            .map(String::as_str)
            .unwrap_or("local");
        let password = params
            .get("password")
            .filter(|value| !value.is_empty())
            .cloned()
            .unwrap_or_else(random_short);
        let hash = bcrypt::hash(&password, bcrypt::DEFAULT_COST)
            .map_err(|_| ApiError::internal("failed to hash password"))?;
        let now = Utc::now().timestamp();
        let mut csv = String::from("email,password\n");
        for index in 0..count {
            let email = format!("{prefix}{}@{suffix}", index + 1);
            csv.push_str(&format!("{email},{password}\n"));
            sqlx::query(
                r#"
                INSERT INTO v2_user (
                    email, password, password_algo, token, uuid, plan_id, expired_at,
                    transfer_enable, u, d, balance, commission_balance, banned, is_admin,
                    is_staff, created_at, updated_at
                )
                VALUES (?, ?, 'bcrypt', ?, ?, ?, ?, 0, 0, 0, 0, 0, 0, 0, 0, ?, ?)
                "#,
            )
            .bind(email)
            .bind(&hash)
            .bind(random_token())
            .bind(Uuid::new_v4().to_string())
            .bind(optional_i64(params, "plan_id"))
            .bind(optional_i64(params, "expired_at"))
            .bind(now)
            .bind(now)
            .execute(&self.db)
            .await?;
        }
        Ok(AdminOutput::Csv {
            filename: "users.csv".to_string(),
            body: csv,
        })
    }

    async fn user_dump_csv(&self) -> Result<AdminOutput, ApiError> {
        let rows = sqlx::query_as::<_, UserCsvRow>(
            "SELECT id, email, token, uuid, created_at FROM v2_user ORDER BY id ASC",
        )
        .fetch_all(&self.db)
        .await?;
        let mut csv = String::from("id,email,token,uuid,created_at\n");
        for row in rows {
            csv.push_str(&format!(
                "{},{},{},{},{}\n",
                row.id, row.email, row.token, row.uuid, row.created_at
            ));
        }
        Ok(AdminOutput::Csv {
            filename: "users.csv".to_string(),
            body: csv,
        })
    }

    async fn user_bulk_flag(
        &self,
        _params: &HashMap<String, String>,
        column: &str,
        value: i64,
    ) -> Result<AdminOutput, ApiError> {
        if column != "banned" {
            return Err(ApiError::legacy("Invalid user flag"));
        }
        sqlx::query("UPDATE v2_user SET banned = ?, updated_at = ?")
            .bind(value)
            .bind(Utc::now().timestamp())
            .execute(&self.db)
            .await?;
        Ok(AdminOutput::Data(json!(true)))
    }

    async fn user_bulk_delete(
        &self,
        _params: &HashMap<String, String>,
    ) -> Result<AdminOutput, ApiError> {
        sqlx::query("DELETE FROM v2_user WHERE is_admin = 0")
            .execute(&self.db)
            .await?;
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
        sqlx::query(
            "UPDATE v2_order SET status = 2, updated_at = ? WHERE trade_no = ? AND status = 0",
        )
        .bind(Utc::now().timestamp())
        .bind(trade_no)
        .execute(&self.db)
        .await?;
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

    async fn server_group_fetch(&self) -> Result<AdminOutput, ApiError> {
        Ok(AdminOutput::Data(json!(
            fetch_json_list(
                &self.db,
                r#"
            SELECT JSON_OBJECT(
                'id', id, 'name', name, 'created_at', created_at, 'updated_at', updated_at,
                'user_count', (SELECT COUNT(*) FROM v2_user WHERE group_id = v2_server_group.id),
                'server_count', 0
            )
            FROM v2_server_group
            ORDER BY id DESC
            "#
            )
            .await?
        )))
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
                        'is_online', 0, 'available_status', 0, 'sort', sort,
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

    async fn stat_summary(&self) -> Result<AdminOutput, ApiError> {
        let now = Utc::now().timestamp();
        let today = Local::now()
            .date_naive()
            .and_hms_opt(0, 0, 0)
            .and_then(|value| Local.from_local_datetime(&value).single())
            .map(|value| value.timestamp())
            .unwrap_or(now);
        let month = first_day_of_month();
        let last_month = first_day_of_previous_month();
        let month_income: i64 = sqlx::query_scalar(
            "SELECT CAST(COALESCE(SUM(total_amount), 0) AS SIGNED) FROM v2_order WHERE status NOT IN (0,2) AND created_at >= ?",
        )
        .bind(month)
        .fetch_one(&self.db)
        .await?;
        let day_income: i64 = sqlx::query_scalar(
            "SELECT CAST(COALESCE(SUM(total_amount), 0) AS SIGNED) FROM v2_order WHERE status NOT IN (0,2) AND created_at >= ?",
        )
        .bind(today)
        .fetch_one(&self.db)
        .await?;
        let last_month_income: i64 = sqlx::query_scalar(
            "SELECT CAST(COALESCE(SUM(total_amount), 0) AS SIGNED) FROM v2_order WHERE status NOT IN (0,2) AND created_at >= ? AND created_at < ?",
        )
        .bind(last_month)
        .bind(month)
        .fetch_one(&self.db)
        .await?;
        let month_register_total: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM v2_user WHERE created_at >= ?")
                .bind(month)
                .fetch_one(&self.db)
                .await?;
        let day_register_total: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM v2_user WHERE created_at >= ?")
                .bind(today)
                .fetch_one(&self.db)
                .await?;
        let ticket_pending_total: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM v2_ticket WHERE status = 0")
                .fetch_one(&self.db)
                .await?;
        let commission_pending_total: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM v2_order WHERE commission_status = 0 AND commission_balance > 0",
        )
        .fetch_one(&self.db)
        .await?;
        Ok(AdminOutput::Data(json!({
            "online_user": 0,
            "month_income": month_income,
            "month_register_total": month_register_total,
            "day_register_total": day_register_total,
            "ticket_pending_total": ticket_pending_total,
            "commission_pending_total": commission_pending_total,
            "day_income": day_income,
            "last_month_income": last_month_income,
            "commission_month_payout": 0,
            "commission_last_month_payout": 0,
        })))
    }

    async fn server_rank(&self, today: bool) -> Result<AdminOutput, ApiError> {
        let since = if today {
            start_of_today()
        } else {
            start_of_yesterday()
        };
        let rows = fetch_json_list_bind(
            &self.db,
            r#"
            SELECT JSON_OBJECT('server_id', server_id, 'server_name', server_type, 'total', SUM(u + d))
            FROM v2_stat_server
            WHERE record_at >= ?
            GROUP BY server_id, server_type
            ORDER BY SUM(u + d) DESC
            LIMIT 10
            "#,
            since,
        )
        .await
        .unwrap_or_default();
        Ok(AdminOutput::Data(json!(rows)))
    }

    async fn user_rank(&self, today: bool) -> Result<AdminOutput, ApiError> {
        let since = if today {
            start_of_today()
        } else {
            start_of_yesterday()
        };
        let rows = fetch_json_list_bind(
            &self.db,
            r#"
            SELECT JSON_OBJECT('user_id', s.user_id, 'email', u.email, 'total', SUM(s.u + s.d))
            FROM v2_stat_user s
            LEFT JOIN v2_user u ON u.id = s.user_id
            WHERE s.record_at >= ?
            GROUP BY s.user_id, u.email
            ORDER BY SUM(s.u + s.d) DESC
            LIMIT 10
            "#,
            since,
        )
        .await
        .unwrap_or_default();
        Ok(AdminOutput::Data(json!(rows)))
    }

    async fn order_stat(&self) -> Result<AdminOutput, ApiError> {
        let rows = fetch_json_list(
            &self.db,
            r#"
            SELECT JSON_OBJECT(
                'type', 'income',
                'date', FROM_UNIXTIME(created_at, '%Y-%m-%d'),
                'value', SUM(total_amount)
            )
            FROM v2_order
            WHERE status NOT IN (0, 2)
            GROUP BY FROM_UNIXTIME(created_at, '%Y-%m-%d')
            ORDER BY FROM_UNIXTIME(created_at, '%Y-%m-%d') ASC
            "#,
        )
        .await
        .unwrap_or_default();
        Ok(AdminOutput::Data(json!(rows)))
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
        if let Ok(entries) = std::fs::read_dir("/laravel/public/theme") {
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
        let path = format!("/laravel/config/theme/{name}.php");
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
        let theme_config_file = format!("/laravel/public/theme/{name}/config.json");
        if !std::path::Path::new(&theme_config_file).exists() {
            return Err(ApiError::legacy("主题不存在"));
        }
        let path = format!("/laravel/config/theme/{name}.php");
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
