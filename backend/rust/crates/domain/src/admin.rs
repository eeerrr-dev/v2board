use std::collections::{BTreeMap, HashMap, HashSet};

use chrono::{Datelike, Local, TimeZone, Utc};
use lettre::{
    AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor,
    transport::smtp::authentication::Credentials,
};
use openssl::pkey::PKey;
use serde::Serialize;
use serde_json::{Map, Value, json};
use sqlx::{AssertSqlSafe, FromRow, MySql, MySqlPool, QueryBuilder, types::Json};
use uuid::Uuid;
use v2board_compat::ApiError;
use v2board_config::AppConfig;

use crate::order::{OrderService, SUPPORTED_PAYMENT_GATEWAYS};

const GIB: i64 = 1_073_741_824;

#[derive(Clone)]
pub struct AdminService {
    db: MySqlPool,
    config: AppConfig,
}

#[derive(Debug)]
pub enum AdminOutput {
    Data(Value),
    Page { data: Vec<Value>, total: i64 },
    Csv { filename: String, body: String },
}

impl AdminService {
    pub fn new(db: MySqlPool, config: AppConfig) -> Self {
        Self { db, config }
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
            "system/getSystemStatus" => Ok(AdminOutput::Data(json!({
                "schedule": true,
                "horizon": false,
                "logChannel": "rust",
                "logLevel": std::env::var("RUST_LOG").unwrap_or_else(|_| "info".to_string()),
                "cacheDriver": "redis",
                "backendVersion": env!("CARGO_PKG_VERSION"),
                "frontendVersion": env!("CARGO_PKG_VERSION"),
            }))),
            "system/getQueueStats" => Ok(AdminOutput::Data(json!({
                "failedJobs": 0,
                "jobsPerMinute": 0,
                "pausedMasters": 0,
                "periods": { "failedJobs": 0, "recentJobs": 0 },
                "processes": 0,
                "queueWithMaxRuntime": null,
                "queueWithMaxThroughput": null,
                "recentJobs": 0,
                "status": "running",
                "wait": {},
            }))),
            "system/getQueueWorkload" => Ok(AdminOutput::Data(json!([{
                "name": "rust-worker",
                "length": 0,
                "wait": 0,
                "processes": 1,
                "recent_jobs": 0,
                "failed_jobs": 0,
            }]))),
            "system/getQueueMasters" => Ok(AdminOutput::Data(json!([{
                "name": "rust-worker",
                "status": "running",
                "pid": std::process::id(),
                "supervisors": [],
            }]))),
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
                "force_https": 0,
                "stop_register": 0,
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
                "show_info_to_server_enable": 0,
                "show_subscribe_method": self.config.show_subscribe_method,
                "show_subscribe_expire": self.config.show_subscribe_expire,
            },
            "frontend": {
                "frontend_theme": "default",
                "frontend_theme_sidebar": "light",
                "frontend_theme_header": "dark",
                "frontend_theme_color": "default",
                "frontend_background_url": null,
            },
            "server": {
                "server_api_url": null,
                "server_token": null,
                "server_pull_interval": 60,
                "server_push_interval": 60,
                "server_node_report_min_traffic": 0,
                "server_device_online_min_traffic": 0,
                "device_limit_mode": 0,
            },
            "email": {
                "email_template": "default",
                "email_host": null,
                "email_port": null,
                "email_username": null,
                "email_password": null,
                "email_encryption": null,
                "email_from_address": null,
            },
            "telegram": {
                "telegram_bot_enable": bool_i(self.config.telegram_bot_enable),
                "telegram_bot_token": null,
                "telegram_discuss_link": self.config.telegram_discuss_link,
            },
            "app": {
                "windows_version": null,
                "windows_download_url": null,
                "macos_version": null,
                "macos_download_url": null,
                "android_version": null,
                "android_download_url": null,
            },
            "safe": {
                "email_verify": bool_i(self.config.email_verify),
                "safe_mode_enable": 0,
                "secure_path": "admin",
                "email_whitelist_enable": bool_i(self.config.email_whitelist_enable),
                "email_whitelist_suffix": self.config.email_whitelist_suffix,
                "email_gmail_limit_enable": 0,
                "recaptcha_enable": bool_i(self.config.recaptcha_enable),
                "recaptcha_key": null,
                "recaptcha_site_key": self.config.recaptcha_site_key,
                "register_limit_by_ip_enable": 0,
                "register_limit_count": 3,
                "register_limit_expire": 60,
                "password_limit_enable": 1,
                "password_limit_count": 5,
                "password_limit_expire": 60,
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

#[derive(Debug, FromRow)]
struct PaymentRow {
    id: i64,
    name: String,
    payment: String,
    icon: Option<String>,
    handling_fee_fixed: Option<i64>,
    handling_fee_percent: Option<f64>,
    uuid: String,
    config: String,
    notify_domain: Option<String>,
    enable: i8,
    sort: Option<i64>,
    created_at: i64,
    updated_at: i64,
}

#[derive(Debug, FromRow)]
struct NoticeRaw {
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
struct NoticeDto {
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

#[derive(Debug, FromRow)]
struct UserCsvRow {
    id: i64,
    email: String,
    token: String,
    uuid: String,
    created_at: i64,
}

struct MailSettings {
    host: String,
    port: Option<u16>,
    username: Option<String>,
    password: Option<String>,
    encryption: Option<String>,
    from_address: Option<String>,
}

impl MailSettings {
    fn load(_config: &AppConfig) -> Result<Self, ApiError> {
        let values = read_php_config("/laravel/config/v2board.php");
        let host = config_string(&values, "email_host")
            .ok_or_else(|| ApiError::legacy("Email host is not configured"))?;
        Ok(Self {
            host,
            port: config_string(&values, "email_port").and_then(|value| value.parse().ok()),
            username: config_string(&values, "email_username"),
            password: config_string(&values, "email_password"),
            encryption: config_string(&values, "email_encryption")
                .map(|value| value.to_ascii_lowercase()),
            from_address: config_string(&values, "email_from_address"),
        })
    }
}

const SERVER_TABLES: &[(&str, &str)] = &[
    ("shadowsocks", "v2_server_shadowsocks"),
    ("vmess", "v2_server_vmess"),
    ("trojan", "v2_server_trojan"),
    ("tuic", "v2_server_tuic"),
    ("vless", "v2_server_vless"),
    ("hysteria", "v2_server_hysteria"),
    ("anytls", "v2_server_anytls"),
    ("v2node", "v2_server_v2node"),
];

const SERVER_SHADOWSOCKS_COLUMNS: &[&str] = &[
    "group_id",
    "route_id",
    "parent_id",
    "tags",
    "name",
    "rate",
    "host",
    "port",
    "server_port",
    "cipher",
    "obfs",
    "obfs_settings",
    "show",
    "sort",
];

const SERVER_TROJAN_COLUMNS: &[&str] = &[
    "group_id",
    "route_id",
    "parent_id",
    "tags",
    "name",
    "rate",
    "host",
    "port",
    "server_port",
    "network",
    "network_settings",
    "allow_insecure",
    "server_name",
    "show",
    "sort",
];

const SERVER_VMESS_COLUMNS: &[&str] = &[
    "group_id",
    "route_id",
    "name",
    "parent_id",
    "host",
    "port",
    "server_port",
    "tls",
    "tags",
    "rate",
    "network",
    "rules",
    "networkSettings",
    "tlsSettings",
    "ruleSettings",
    "dnsSettings",
    "show",
    "sort",
];

const SERVER_TUIC_COLUMNS: &[&str] = &[
    "group_id",
    "route_id",
    "name",
    "parent_id",
    "host",
    "port",
    "server_port",
    "tags",
    "rate",
    "show",
    "sort",
    "server_name",
    "insecure",
    "disable_sni",
    "udp_relay_mode",
    "zero_rtt_handshake",
    "congestion_control",
];

const SERVER_HYSTERIA_COLUMNS: &[&str] = &[
    "version",
    "group_id",
    "route_id",
    "name",
    "parent_id",
    "host",
    "port",
    "server_port",
    "tags",
    "rate",
    "show",
    "sort",
    "up_mbps",
    "down_mbps",
    "obfs",
    "obfs_password",
    "server_name",
    "insecure",
];

const SERVER_VLESS_COLUMNS: &[&str] = &[
    "group_id",
    "route_id",
    "name",
    "parent_id",
    "host",
    "port",
    "server_port",
    "tls",
    "tls_settings",
    "flow",
    "network",
    "network_settings",
    "encryption",
    "encryption_settings",
    "tags",
    "rate",
    "show",
    "sort",
];

const SERVER_ANYTLS_COLUMNS: &[&str] = &[
    "group_id",
    "route_id",
    "name",
    "parent_id",
    "host",
    "port",
    "server_port",
    "tags",
    "rate",
    "show",
    "sort",
    "server_name",
    "insecure",
    "padding_scheme",
];

const SERVER_V2NODE_COLUMNS: &[&str] = &[
    "group_id",
    "route_id",
    "name",
    "parent_id",
    "host",
    "listen_ip",
    "port",
    "server_port",
    "tags",
    "rate",
    "show",
    "sort",
    "protocol",
    "tls",
    "tls_settings",
    "flow",
    "network",
    "network_settings",
    "encryption",
    "encryption_settings",
    "disable_sni",
    "udp_relay_mode",
    "zero_rtt_handshake",
    "congestion_control",
    "cipher",
    "up_mbps",
    "down_mbps",
    "obfs",
    "obfs_password",
    "padding_scheme",
];

#[derive(Debug, Clone)]
enum AdminSqlValue {
    Null,
    Integer(i64),
    Text(String),
}

fn push_admin_sql_value(
    separated: &mut sqlx::query_builder::Separated<'_, MySql, &str>,
    value: &AdminSqlValue,
) {
    match value {
        AdminSqlValue::Null => {
            separated.push_bind(Option::<String>::None);
        }
        AdminSqlValue::Integer(value) => {
            separated.push_bind(*value);
        }
        AdminSqlValue::Text(value) => {
            separated.push_bind(value.clone());
        }
    }
}

fn push_admin_sql_bind(builder: &mut QueryBuilder<MySql>, value: &AdminSqlValue) {
    match value {
        AdminSqlValue::Null => {
            builder.push_bind(Option::<String>::None);
        }
        AdminSqlValue::Integer(value) => {
            builder.push_bind(*value);
        }
        AdminSqlValue::Text(value) => {
            builder.push_bind(value.clone());
        }
    }
}

fn server_copy_columns(kind: &str) -> Result<&'static [&'static str], ApiError> {
    match kind {
        "shadowsocks" => Ok(SERVER_SHADOWSOCKS_COLUMNS),
        "trojan" => Ok(SERVER_TROJAN_COLUMNS),
        "vmess" => Ok(SERVER_VMESS_COLUMNS),
        "tuic" => Ok(SERVER_TUIC_COLUMNS),
        "hysteria" => Ok(SERVER_HYSTERIA_COLUMNS),
        "vless" => Ok(SERVER_VLESS_COLUMNS),
        "anytls" => Ok(SERVER_ANYTLS_COLUMNS),
        "v2node" => Ok(SERVER_V2NODE_COLUMNS),
        _ => Err(ApiError::legacy("Invalid server type")),
    }
}

fn server_save_values(
    kind: &str,
    params: &HashMap<String, String>,
) -> Result<Vec<(&'static str, AdminSqlValue)>, ApiError> {
    let mut values = Vec::new();
    push_common_server_values(&mut values, params)?;
    match kind {
        "shadowsocks" => {
            values.push(("cipher", text_value(required_string(params, "cipher")?)));
            values.push(("obfs", optional_text_value(params, "obfs")));
            values.push((
                "obfs_settings",
                optional_json_text_value(params, "obfs_settings"),
            ));
        }
        "trojan" => {
            values.push(("network", text_value(required_string(params, "network")?)));
            values.push((
                "network_settings",
                optional_json_text_value(params, "network_settings"),
            ));
            values.push((
                "allow_insecure",
                optional_int_value(params, "allow_insecure", 0),
            ));
            values.push(("server_name", optional_text_value(params, "server_name")));
        }
        "vmess" => {
            values.push(("tls", optional_int_value(params, "tls", 0)));
            values.push(("network", text_value(required_string(params, "network")?)));
            values.push(("rules", optional_json_text_value(params, "rules")));
            values.push((
                "networkSettings",
                optional_json_text_value(params, "networkSettings"),
            ));
            values.push((
                "tlsSettings",
                optional_json_text_value(params, "tlsSettings"),
            ));
            values.push((
                "ruleSettings",
                optional_json_text_value(params, "ruleSettings"),
            ));
            values.push((
                "dnsSettings",
                optional_json_text_value(params, "dnsSettings"),
            ));
        }
        "tuic" => {
            values.push(("server_name", optional_text_value(params, "server_name")));
            values.push(("insecure", optional_int_value(params, "insecure", 0)));
            values.push(("disable_sni", optional_int_value(params, "disable_sni", 0)));
            values.push((
                "udp_relay_mode",
                optional_text_value(params, "udp_relay_mode"),
            ));
            values.push((
                "zero_rtt_handshake",
                optional_int_value(params, "zero_rtt_handshake", 0),
            ));
            values.push((
                "congestion_control",
                optional_text_value(params, "congestion_control"),
            ));
        }
        "hysteria" => {
            values.push(("version", optional_int_value(params, "version", 2)));
            values.push(("up_mbps", optional_int_value(params, "up_mbps", 0)));
            values.push(("down_mbps", optional_int_value(params, "down_mbps", 0)));
            values.push(("obfs", optional_text_value(params, "obfs")));
            values.push((
                "obfs_password",
                hysteria_obfs_password(params, params.get("obfs")),
            ));
            values.push(("server_name", optional_text_value(params, "server_name")));
            values.push(("insecure", optional_int_value(params, "insecure", 0)));
        }
        "vless" => {
            let tls = optional_i64(params, "tls").unwrap_or_default();
            let network = required_string(params, "network")?;
            let encryption = optional_string(params, "encryption");
            let mut flow = optional_string(params, "flow");
            if network != "tcp" {
                flow = None;
            }
            values.push(("tls", AdminSqlValue::Integer(tls)));
            values.push((
                "tls_settings",
                json_value(prepare_tls_settings(params, tls)?),
            ));
            values.push(("flow", optional_text(flow)));
            values.push(("network", text_value(network.clone())));
            values.push((
                "network_settings",
                json_value(prepare_network_settings(
                    params,
                    "network_settings",
                    &network,
                    false,
                )),
            ));
            values.push(("encryption", optional_text(encryption.clone())));
            values.push((
                "encryption_settings",
                json_value(prepare_encryption_settings(
                    params,
                    encryption.as_deref(),
                    false,
                )),
            ));
        }
        "anytls" => {
            values.push(("server_name", optional_text_value(params, "server_name")));
            values.push(("insecure", optional_int_value(params, "insecure", 0)));
            values.push((
                "padding_scheme",
                optional_decoded_json_text_value(params, "padding_scheme"),
            ));
        }
        "v2node" => {
            let protocol = required_string(params, "protocol")?;
            let mut tls = optional_i64(params, "tls").unwrap_or_default();
            if (protocol == "anytls" && tls == 0)
                || matches!(protocol.as_str(), "hysteria2" | "trojan" | "tuic")
            {
                tls = 1;
            }
            let network = required_string(params, "network")?;
            let encryption = optional_string(params, "encryption");
            let mut flow = optional_string(params, "flow");
            if network != "tcp" && encryption.as_deref() != Some("mlkem768x25519plus") {
                flow = None;
            }
            values.push((
                "listen_ip",
                text_value(
                    optional_string(params, "listen_ip").unwrap_or_else(|| "0.0.0.0".to_string()),
                ),
            ));
            values.push(("protocol", text_value(protocol.clone())));
            values.push(("tls", AdminSqlValue::Integer(tls)));
            values.push((
                "tls_settings",
                json_value(prepare_v2node_tls_settings(params, tls)?),
            ));
            values.push(("flow", optional_text(flow)));
            values.push(("network", text_value(network.clone())));
            values.push((
                "network_settings",
                json_value(prepare_network_settings(
                    params,
                    "network_settings",
                    &network,
                    true,
                )),
            ));
            values.push(("encryption", optional_text(encryption.clone())));
            values.push((
                "encryption_settings",
                json_value(prepare_encryption_settings(
                    params,
                    encryption.as_deref(),
                    true,
                )),
            ));
            values.push(("disable_sni", optional_int_value(params, "disable_sni", 0)));
            values.push((
                "udp_relay_mode",
                optional_text_value(params, "udp_relay_mode"),
            ));
            values.push((
                "zero_rtt_handshake",
                optional_int_value(params, "zero_rtt_handshake", 0),
            ));
            values.push((
                "congestion_control",
                optional_text_value(params, "congestion_control"),
            ));
            values.push((
                "cipher",
                optional_text(
                    optional_string(params, "cipher")
                        .or_else(|| (protocol == "shadowsocks").then(|| "aes-128-gcm".to_string())),
                ),
            ));
            values.push(("up_mbps", optional_int_value(params, "up_mbps", 0)));
            values.push(("down_mbps", optional_int_value(params, "down_mbps", 0)));
            values.push(("obfs", optional_text_value(params, "obfs")));
            values.push((
                "obfs_password",
                hysteria_obfs_password(params, params.get("obfs")),
            ));
            values.push((
                "padding_scheme",
                optional_decoded_json_text_value(params, "padding_scheme"),
            ));
        }
        _ => return Err(ApiError::legacy("Invalid server type")),
    }
    Ok(values)
}

fn push_common_server_values(
    values: &mut Vec<(&'static str, AdminSqlValue)>,
    params: &HashMap<String, String>,
) -> Result<(), ApiError> {
    values.push((
        "group_id",
        text_value(required_json_array_string(params, "group_id")?),
    ));
    values.push((
        "route_id",
        optional_json_array_text_value(params, "route_id"),
    ));
    values.push(("parent_id", optional_int_or_null_value(params, "parent_id")));
    values.push(("tags", optional_json_array_text_value(params, "tags")));
    values.push(("name", text_value(required_string(params, "name")?)));
    values.push(("rate", text_value(required_string(params, "rate")?)));
    values.push(("host", text_value(required_string(params, "host")?)));
    values.push(("port", text_value(required_string(params, "port")?)));
    values.push((
        "server_port",
        AdminSqlValue::Integer(required_i64(params, "server_port")?),
    ));
    values.push(("show", optional_int_value(params, "show", 0)));
    values.push(("sort", optional_int_or_null_value(params, "sort")));
    Ok(())
}

fn text_value(value: String) -> AdminSqlValue {
    AdminSqlValue::Text(value)
}

fn optional_text(value: Option<String>) -> AdminSqlValue {
    value
        .filter(|value| !value.trim().is_empty() && !value.eq_ignore_ascii_case("null"))
        .map(AdminSqlValue::Text)
        .unwrap_or(AdminSqlValue::Null)
}

fn optional_text_value(params: &HashMap<String, String>, key: &str) -> AdminSqlValue {
    optional_text(optional_string(params, key))
}

fn optional_int_value(params: &HashMap<String, String>, key: &str, default: i64) -> AdminSqlValue {
    AdminSqlValue::Integer(optional_i64(params, key).unwrap_or(default))
}

fn optional_int_or_null_value(params: &HashMap<String, String>, key: &str) -> AdminSqlValue {
    optional_i64(params, key)
        .map(AdminSqlValue::Integer)
        .unwrap_or(AdminSqlValue::Null)
}

fn optional_string(params: &HashMap<String, String>, key: &str) -> Option<String> {
    params
        .get(key)
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty() && !value.eq_ignore_ascii_case("null"))
}

fn required_json_array_string(
    params: &HashMap<String, String>,
    key: &str,
) -> Result<String, ApiError> {
    json_array_string(params, key)?.ok_or_else(|| ApiError::legacy("参数有误"))
}

fn optional_json_array_text_value(params: &HashMap<String, String>, key: &str) -> AdminSqlValue {
    json_array_string(params, key)
        .ok()
        .flatten()
        .map(AdminSqlValue::Text)
        .unwrap_or(AdminSqlValue::Null)
}

fn json_array_string(
    params: &HashMap<String, String>,
    key: &str,
) -> Result<Option<String>, ApiError> {
    if let Some(value) = optional_string(params, key) {
        if serde_json::from_str::<Value>(&value).is_ok() {
            return Ok(Some(value));
        }
        return Ok(Some(json_string(&Value::Array(vec![json_scalar(&value)]))));
    }
    let values = json_array_param(params, key);
    Ok((!values.is_empty()).then(|| json_string(&Value::Array(values))))
}

fn optional_json_text_value(params: &HashMap<String, String>, key: &str) -> AdminSqlValue {
    optional_json_value(params, key)
        .map(json_value)
        .unwrap_or(AdminSqlValue::Null)
}

fn optional_decoded_json_text_value(params: &HashMap<String, String>, key: &str) -> AdminSqlValue {
    let Some(value) = optional_string(params, key) else {
        return optional_json_text_value(params, key);
    };
    serde_json::from_str::<Value>(&value)
        .map(json_value)
        .unwrap_or(AdminSqlValue::Null)
}

fn optional_json_value(params: &HashMap<String, String>, key: &str) -> Option<Value> {
    if let Some(value) = optional_string(params, key)
        && let Ok(parsed) = serde_json::from_str::<Value>(&value)
    {
        return Some(parsed);
    }
    let value = nested_json(params, key);
    match &value {
        Value::Object(object) if object.is_empty() => None,
        _ => Some(value),
    }
}

fn json_value(value: Value) -> AdminSqlValue {
    AdminSqlValue::Text(json_string(&value))
}

fn prepare_tls_settings(params: &HashMap<String, String>, tls: i64) -> Result<Value, ApiError> {
    let mut settings = optional_json_value(params, "tls_settings").unwrap_or_else(|| json!({}));
    if tls == 2 {
        ensure_reality_keys(&mut settings)?;
    }
    Ok(settings)
}

fn prepare_v2node_tls_settings(
    params: &HashMap<String, String>,
    tls: i64,
) -> Result<Value, ApiError> {
    let mut settings = prepare_tls_settings(params, tls)?;
    if let Some(object) = settings.as_object_mut()
        && object.get("ech").and_then(Value::as_str) == Some("custom")
    {
        let outer_sni = object
            .get("ech_server_name")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string();
        if outer_sni.is_empty() {
            object.insert("ech".to_string(), Value::String(String::new()));
        } else if object
            .get("ech_key")
            .and_then(Value::as_str)
            .filter(|value| !value.is_empty())
            .is_none()
            || object
                .get("ech_config")
                .and_then(Value::as_str)
                .filter(|value| !value.is_empty())
                .is_none()
        {
            let (ech_key, ech_config) = generate_ech_key_pair(&outer_sni)?;
            object
                .entry("ech_key".to_string())
                .or_insert(json!(ech_key));
            object
                .entry("ech_config".to_string())
                .or_insert(json!(ech_config));
        }
    }
    Ok(settings)
}

fn ensure_reality_keys(settings: &mut Value) -> Result<(), ApiError> {
    let object = settings
        .as_object_mut()
        .ok_or_else(|| ApiError::legacy("TLS settings format is invalid"))?;
    let missing_public = object
        .get("public_key")
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .is_none();
    let missing_private = object
        .get("private_key")
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .is_none();
    if missing_public || missing_private {
        let (public_key, private_key) = x25519_key_pair_urlsafe()?;
        object
            .entry("public_key".to_string())
            .or_insert(json!(public_key));
        object
            .entry("private_key".to_string())
            .or_insert(json!(private_key));
    }
    if object
        .get("short_id")
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .is_none()
        && let Some(private_key) = object.get("private_key").and_then(Value::as_str)
    {
        object.insert(
            "short_id".to_string(),
            json!(format!("{:x}", md5::compute(private_key))[..8].to_string()),
        );
    }
    object
        .entry("server_port".to_string())
        .or_insert(json!("443"));
    Ok(())
}

fn prepare_network_settings(
    params: &HashMap<String, String>,
    key: &str,
    network: &str,
    v2node: bool,
) -> Value {
    let mut settings = optional_json_value(params, key).unwrap_or_else(|| json!({}));
    if v2node && let Some(object) = settings.as_object_mut() {
        coerce_object_bool(object, "acceptProxyProtocol");
    }
    if network == "xhttp" {
        normalize_xhttp_settings(&mut settings, v2node);
    }
    settings
}

fn normalize_xhttp_settings(settings: &mut Value, v2node: bool) {
    let Some(object) = settings.as_object_mut() else {
        return;
    };
    let Some(extra) = object.get_mut("extra").and_then(Value::as_object_mut) else {
        return;
    };
    if v2node {
        coerce_object_bool(extra, "xPaddingObfsMode");
    }
    coerce_object_bool(extra, "noGRPCHeader");
    coerce_object_bool(extra, "noSSEHeader");
    coerce_object_i64(extra, "scMaxBufferedPosts");
    if let Some(xmux) = extra.get_mut("xmux").and_then(Value::as_object_mut) {
        coerce_object_i64(xmux, "hKeepAlivePeriod");
    }
    if let Some(download) = extra
        .get_mut("downloadSettings")
        .and_then(Value::as_object_mut)
    {
        coerce_object_i64(download, "port");
    }
}

fn prepare_encryption_settings(
    params: &HashMap<String, String>,
    encryption: Option<&str>,
    v2node: bool,
) -> Value {
    let mut settings =
        optional_json_value(params, "encryption_settings").unwrap_or_else(|| json!({}));
    if encryption != Some("mlkem768x25519plus") {
        return settings;
    }
    let Some(object) = settings.as_object_mut() else {
        return json!({});
    };
    if v2node {
        object.entry("mode".to_string()).or_insert(json!("native"));
    }
    match object.get("rtt").and_then(Value::as_str) {
        Some("1rtt") => {
            object.insert("ticket".to_string(), json!("0s"));
        }
        Some(_) => {}
        None if v2node => {
            object.insert("rtt".to_string(), json!("0rtt"));
            object.insert("ticket".to_string(), json!("600s"));
        }
        None => {}
    }
    if object
        .get("private_key")
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .is_none()
        || object
            .get("password")
            .and_then(Value::as_str)
            .filter(|value| !value.is_empty())
            .is_none()
    {
        let private_key = random_urlsafe_key(32);
        let password = random_urlsafe_key(32);
        object
            .entry("private_key".to_string())
            .or_insert(json!(private_key));
        object
            .entry("password".to_string())
            .or_insert(json!(password));
    }
    settings
}

fn coerce_object_bool(object: &mut Map<String, Value>, key: &str) {
    if let Some(value) = object.get_mut(key) {
        *value = Value::Bool(match value {
            Value::Bool(value) => *value,
            Value::Number(value) => value.as_i64().unwrap_or_default() != 0,
            Value::String(value) => matches!(
                value.to_ascii_lowercase().as_str(),
                "1" | "true" | "yes" | "on"
            ),
            _ => false,
        });
    }
}

fn coerce_object_i64(object: &mut Map<String, Value>, key: &str) {
    if let Some(value) = object.get_mut(key)
        && let Some(parsed) = match value {
            Value::Number(value) => value.as_i64(),
            Value::String(value) => value.parse::<i64>().ok(),
            Value::Bool(value) => Some(i64::from(*value)),
            _ => None,
        }
    {
        *value = json!(parsed);
    }
}

fn hysteria_obfs_password(
    params: &HashMap<String, String>,
    obfs: Option<&String>,
) -> AdminSqlValue {
    if obfs
        .map(|value| value.trim().is_empty() || value.eq_ignore_ascii_case("null"))
        .unwrap_or(true)
    {
        return AdminSqlValue::Null;
    }
    optional_string(params, "obfs_password")
        .map(AdminSqlValue::Text)
        .unwrap_or_else(|| {
            AdminSqlValue::Text(server_key(
                optional_i64(params, "created_at").unwrap_or_else(|| Utc::now().timestamp()),
                16,
            ))
        })
}

fn server_key(timestamp: i64, length: usize) -> String {
    let digest = format!("{:x}", md5::compute(timestamp.to_string()));
    standard_base64_encode(&digest.as_bytes()[..length.min(digest.len())])
}

fn x25519_key_pair_urlsafe() -> Result<(String, String), ApiError> {
    let key = PKey::generate_x25519()
        .map_err(|error| ApiError::legacy(format!("X25519 key generation failed: {error}")))?;
    let public_key = key
        .raw_public_key()
        .map_err(|error| ApiError::legacy(format!("X25519 public key export failed: {error}")))?;
    let private_key = key
        .raw_private_key()
        .map_err(|error| ApiError::legacy(format!("X25519 private key export failed: {error}")))?;
    Ok((
        base64_url_no_pad(&public_key),
        base64_url_no_pad(&private_key),
    ))
}

fn generate_ech_key_pair(outer_sni: &str) -> Result<(String, String), ApiError> {
    let key = PKey::generate_x25519()
        .map_err(|error| ApiError::legacy(format!("ECH key generation failed: {error}")))?;
    let public_key = key
        .raw_public_key()
        .map_err(|error| ApiError::legacy(format!("ECH public key export failed: {error}")))?;
    let private_key = key
        .raw_private_key()
        .map_err(|error| ApiError::legacy(format!("ECH private key export failed: {error}")))?;
    let config_id = Uuid::new_v4().as_bytes()[0];

    let mut config_data = Vec::new();
    config_data.push(config_id);
    config_data.extend_from_slice(&0x0020_u16.to_be_bytes());
    config_data.extend_from_slice(&(public_key.len() as u16).to_be_bytes());
    config_data.extend_from_slice(&public_key);
    let suites = [0x0001_u16, 0x0001, 0x0001, 0x0002, 0x0001, 0x0003];
    config_data.extend_from_slice(&((suites.len() * 2) as u16).to_be_bytes());
    for suite in suites {
        config_data.extend_from_slice(&suite.to_be_bytes());
    }
    config_data.push(0);
    config_data.push(outer_sni.len().min(u8::MAX as usize) as u8);
    config_data.extend_from_slice(&outer_sni.as_bytes()[..outer_sni.len().min(u8::MAX as usize)]);
    config_data.extend_from_slice(&0_u16.to_be_bytes());

    let mut ech_config = Vec::new();
    ech_config.extend_from_slice(&0xfe0d_u16.to_be_bytes());
    ech_config.extend_from_slice(&(config_data.len() as u16).to_be_bytes());
    ech_config.extend_from_slice(&config_data);

    let mut ech_keys = Vec::new();
    ech_keys.extend_from_slice(&(ech_config.len() as u16).to_be_bytes());
    ech_keys.extend_from_slice(&ech_config);
    ech_keys.extend_from_slice(&1_u16.to_be_bytes());
    ech_keys.push(config_id);
    ech_keys.extend_from_slice(&(private_key.len() as u16).to_be_bytes());
    ech_keys.extend_from_slice(&private_key);

    Ok((
        standard_base64_encode(&ech_keys),
        standard_base64_encode(&ech_config),
    ))
}

fn random_urlsafe_key(length: usize) -> String {
    let mut bytes = Vec::with_capacity(length);
    while bytes.len() < length {
        bytes.extend_from_slice(Uuid::new_v4().as_bytes());
    }
    bytes.truncate(length);
    base64_url_no_pad(&bytes)
}

fn base64_url_no_pad(bytes: &[u8]) -> String {
    standard_base64_encode(bytes)
        .replace('+', "-")
        .replace('/', "_")
        .replace('=', "")
}

fn standard_base64_encode(bytes: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let b0 = chunk[0];
        let b1 = *chunk.get(1).unwrap_or(&0);
        let b2 = *chunk.get(2).unwrap_or(&0);
        out.push(TABLE[(b0 >> 2) as usize] as char);
        out.push(TABLE[(((b0 & 0x03) << 4) | (b1 >> 4)) as usize] as char);
        if chunk.len() > 1 {
            out.push(TABLE[(((b1 & 0x0f) << 2) | (b2 >> 6)) as usize] as char);
        } else {
            out.push('=');
        }
        if chunk.len() > 2 {
            out.push(TABLE[(b2 & 0x3f) as usize] as char);
        } else {
            out.push('=');
        }
    }
    out
}

fn normalize_admin_path(path: &str) -> String {
    path.trim_matches('/').to_string()
}

fn bool_i(value: bool) -> i32 {
    if value { 1 } else { 0 }
}

fn payment_methods() -> Vec<&'static str> {
    SUPPORTED_PAYMENT_GATEWAYS.to_vec()
}

fn payment_form(payment: &str) -> Value {
    let mut form = Map::new();
    match payment {
        "EPay" => {
            form_field(&mut form, "url", "接口地址");
            form_field(&mut form, "pid", "PID");
            form_field(&mut form, "key", "KEY");
            form_field(&mut form, "type", "支付类型");
        }
        "MGate" => {
            form_field(&mut form, "mgate_url", "接口地址");
            form_field(&mut form, "mgate_app_id", "APP ID");
            form_field(&mut form, "mgate_app_secret", "APP SECRET");
            form_field(&mut form, "mgate_source_currency", "源货币");
        }
        "BEasyPaymentUSDT" => {
            form_field(&mut form, "bepusdt_url", "接口地址");
            form_field(&mut form, "bepusdt_apitoken", "API Token");
            form_field(&mut form, "bepusdt_trade_type", "Trade Type");
        }
        "CoinPayments" => {
            form_field(&mut form, "coinpayments_merchant_id", "Merchant ID");
            form_field(&mut form, "coinpayments_ipn_secret", "IPN Secret");
            form_field(&mut form, "coinpayments_currency", "货币代码");
        }
        "Coinbase" => {
            form_field(&mut form, "coinbase_url", "接口地址");
            form_field(&mut form, "coinbase_api_key", "API KEY");
            form_field(&mut form, "coinbase_webhook_key", "WEBHOOK KEY");
        }
        "BTCPay" => {
            form_field(&mut form, "btcpay_url", "API 接口地址");
            form_field(&mut form, "btcpay_storeId", "storeId");
            form_field(&mut form, "btcpay_api_key", "API KEY");
            form_field(&mut form, "btcpay_webhook_key", "WEBHOOK KEY");
        }
        "WechatPayNative" => {
            form_field(&mut form, "app_id", "APPID");
            form_field(&mut form, "mch_id", "商户号");
            form_field(&mut form, "api_key", "APIKEY(v1)");
        }
        "AlipayF2F" => {
            form_field(&mut form, "app_id", "支付宝APPID");
            form_field(&mut form, "private_key", "支付宝私钥");
            form_field(&mut form, "public_key", "支付宝公钥");
            form_field(&mut form, "product_name", "自定义商品名称");
        }
        "StripeCredit" | "StripeAlipay" | "StripeWepay" | "StripeCheckout" => {
            form_field(&mut form, "currency", "货币单位");
            form_field(&mut form, "stripe_sk_live", "SK_LIVE");
            form_field(&mut form, "stripe_pk_live", "PK_LIVE");
            form_field(&mut form, "stripe_webhook_key", "WebHook 密钥签名");
            if payment == "StripeCheckout" {
                form_field(&mut form, "stripe_custom_field_name", "自定义字段名称");
            }
        }
        "StripeALL" => {
            form_field(&mut form, "currency", "货币单位");
            form_field(&mut form, "stripe_sk_live", "SK_LIVE");
            form_field(&mut form, "stripe_webhook_key", "WebHook 密钥签名");
            form_field(&mut form, "payment_method", "支付方式");
        }
        _ => {}
    }
    Value::Object(form)
}

fn form_field(form: &mut Map<String, Value>, key: &str, label: &str) {
    form.insert(
        key.to_string(),
        json!({
            "label": label,
            "description": "",
            "type": "input",
        }),
    );
}

async fn fetch_json_list(db: &MySqlPool, sql: &str) -> Result<Vec<Value>, ApiError> {
    let rows = sqlx::query_scalar::<_, Json<Value>>(AssertSqlSafe(sql))
        .fetch_all(db)
        .await?;
    Ok(json_rows(rows))
}

async fn fetch_json_list_bind(
    db: &MySqlPool,
    sql: &str,
    bind: i64,
) -> Result<Vec<Value>, ApiError> {
    let rows = sqlx::query_scalar::<_, Json<Value>>(AssertSqlSafe(sql))
        .bind(bind)
        .fetch_all(db)
        .await?;
    Ok(json_rows(rows))
}

async fn fetch_json_list_page(
    db: &MySqlPool,
    sql: &str,
    limit: i64,
    offset: i64,
) -> Result<Vec<Value>, ApiError> {
    let rows = sqlx::query_scalar::<_, Json<Value>>(AssertSqlSafe(sql))
        .bind(limit)
        .bind(offset)
        .fetch_all(db)
        .await?;
    Ok(json_rows(rows))
}

async fn fetch_json_list_page_bind(
    db: &MySqlPool,
    sql: &str,
    bind: i64,
    limit: i64,
    offset: i64,
) -> Result<Vec<Value>, ApiError> {
    let rows = sqlx::query_scalar::<_, Json<Value>>(AssertSqlSafe(sql))
        .bind(bind)
        .bind(limit)
        .bind(offset)
        .fetch_all(db)
        .await?;
    Ok(json_rows(rows))
}

async fn fetch_json_list_page_bind_text(
    db: &MySqlPool,
    sql: &str,
    bind: &str,
    limit: i64,
    offset: i64,
) -> Result<Vec<Value>, ApiError> {
    let rows = sqlx::query_scalar::<_, Json<Value>>(AssertSqlSafe(sql))
        .bind(bind)
        .bind(limit)
        .bind(offset)
        .fetch_all(db)
        .await?;
    Ok(json_rows(rows))
}

async fn fetch_json_one(db: &MySqlPool, sql: &str, bind: i64) -> Result<Option<Value>, ApiError> {
    let Some(row) = sqlx::query_scalar::<_, Json<Value>>(AssertSqlSafe(sql))
        .bind(bind)
        .fetch_optional(db)
        .await?
    else {
        return Ok(None);
    };
    Ok(Some(row.0))
}

fn json_rows(rows: Vec<Json<Value>>) -> Vec<Value> {
    rows.into_iter().map(|row| row.0).collect()
}

fn required_string(params: &HashMap<String, String>, key: &str) -> Result<String, ApiError> {
    params
        .get(key)
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .ok_or_else(|| ApiError::legacy(format!("{key} cannot be empty")))
}

fn optional_i64(params: &HashMap<String, String>, key: &str) -> Option<i64> {
    params
        .get(key)
        .map(String::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty() && !value.eq_ignore_ascii_case("null"))
        .and_then(|value| value.parse::<i64>().ok())
}

fn optional_f64(params: &HashMap<String, String>, key: &str) -> Option<f64> {
    params
        .get(key)
        .map(String::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty() && !value.eq_ignore_ascii_case("null"))
        .and_then(|value| value.parse::<f64>().ok())
}

fn required_i64(params: &HashMap<String, String>, key: &str) -> Result<i64, ApiError> {
    optional_i64(params, key).ok_or_else(|| ApiError::legacy(format!("{key} cannot be empty")))
}

fn page(params: &HashMap<String, String>) -> (i64, i64) {
    let current = optional_i64(params, "current").unwrap_or(1).max(1);
    let page_size = optional_i64(params, "pageSize")
        .or_else(|| optional_i64(params, "page_size"))
        .unwrap_or(10)
        .max(10);
    (current, page_size)
}

fn offset(current: i64, page_size: i64) -> i64 {
    (current - 1) * page_size
}

fn array_param(params: &HashMap<String, String>, key: &str) -> Result<Vec<i64>, ApiError> {
    let mut values = BTreeMap::<usize, i64>::new();
    for (raw_key, raw_value) in params {
        if let Some(index) = bracket_index(raw_key, key)
            && let Ok(value) = raw_value.parse::<i64>()
        {
            values.insert(index, value);
        }
    }
    if let Some(value) = params.get(key)
        && let Ok(parsed) = serde_json::from_str::<Vec<i64>>(value)
    {
        return Ok(parsed);
    }
    let values = values.into_values().collect::<Vec<_>>();
    if values.is_empty() {
        return Err(ApiError::legacy("参数有误"));
    }
    Ok(values)
}

fn json_array_param(params: &HashMap<String, String>, key: &str) -> Vec<Value> {
    let mut values = BTreeMap::<usize, Value>::new();
    for (raw_key, raw_value) in params {
        if let Some(index) = bracket_index(raw_key, key) {
            values.insert(index, json_scalar(raw_value));
        }
    }
    values.into_values().collect()
}

fn optional_json_array_string(params: &HashMap<String, String>, key: &str) -> Option<String> {
    if let Some(value) = params.get(key)
        && serde_json::from_str::<Value>(value).is_ok()
    {
        return Some(value.clone());
    }
    let values = json_array_param(params, key);
    (!values.is_empty()).then(|| json_string(&Value::Array(values)))
}

fn bracket_index(raw_key: &str, key: &str) -> Option<usize> {
    raw_key
        .strip_prefix(&format!("{key}["))
        .and_then(|value| value.strip_suffix(']'))
        .and_then(|value| value.parse::<usize>().ok())
}

fn nested_json(params: &HashMap<String, String>, key: &str) -> Value {
    let mut root = Value::Object(Map::new());
    for (raw_key, raw_value) in params {
        if let Some(path) = bracket_path(raw_key, key) {
            insert_nested_json(&mut root, &path, json_scalar(raw_value));
        }
    }
    if matches!(&root, Value::Object(object) if object.is_empty())
        && let Some(value) = params.get(key)
        && let Ok(parsed) = serde_json::from_str::<Value>(value)
    {
        return parsed;
    }
    root
}

fn bracket_path(raw_key: &str, key: &str) -> Option<Vec<String>> {
    let mut rest = raw_key.strip_prefix(key)?;
    if rest.is_empty() {
        return None;
    }
    let mut parts = Vec::new();
    while let Some(value) = rest.strip_prefix('[') {
        let (part, tail) = value.split_once(']')?;
        parts.push(part.to_string());
        rest = tail;
    }
    (rest.is_empty() && !parts.is_empty()).then_some(parts)
}

fn insert_nested_json(root: &mut Value, path: &[String], value: Value) {
    let Some((head, tail)) = path.split_first() else {
        *root = value;
        return;
    };
    if tail.is_empty() {
        if let Value::Object(object) = root {
            object.insert(head.clone(), value);
        }
        return;
    }
    if !root.is_object() {
        *root = Value::Object(Map::new());
    }
    let Value::Object(object) = root else {
        return;
    };
    let child = object
        .entry(head.clone())
        .or_insert_with(|| Value::Object(Map::new()));
    insert_nested_json(child, tail, value);
}

fn json_scalar(value: &str) -> Value {
    if value.eq_ignore_ascii_case("null") {
        Value::Null
    } else if value == "true" {
        Value::Bool(true)
    } else if value == "false" {
        Value::Bool(false)
    } else if let Ok(value) = value.parse::<i64>() {
        json!(value)
    } else if let Ok(value) = value.parse::<f64>() {
        json!(value)
    } else {
        json!(value)
    }
}

fn json_string(value: &Value) -> String {
    serde_json::to_string(value).unwrap_or_else(|_| "[]".to_string())
}

fn truthy(value: Option<&String>) -> bool {
    matches!(
        value.map(String::as_str),
        Some("1" | "true" | "TRUE" | "yes" | "YES")
    )
}

fn random_short() -> String {
    Uuid::new_v4().simple().to_string()[..8].to_string()
}

fn random_token() -> String {
    Uuid::new_v4().simple().to_string()
}

fn list_names(path: &str) -> Vec<String> {
    std::fs::read_dir(path)
        .ok()
        .into_iter()
        .flatten()
        .filter_map(|entry| entry.ok())
        .filter_map(|entry| entry.file_name().into_string().ok())
        .collect()
}

fn read_php_config(path: &str) -> Map<String, Value> {
    let Ok(content) = std::fs::read_to_string(path) else {
        return Map::new();
    };
    let mut values = Map::new();
    for line in content.lines() {
        let line = line.trim().trim_end_matches(',');
        if !line.starts_with('\'') || !line.contains("=>") {
            continue;
        }
        let Some((raw_key, raw_value)) = line.split_once("=>") else {
            continue;
        };
        let key = raw_key.trim().trim_matches('\'');
        if key.is_empty() {
            continue;
        }
        values.insert(key.to_string(), parse_php_value(raw_value.trim()));
    }
    values
}

fn config_string(config: &Map<String, Value>, key: &str) -> Option<String> {
    match config.get(key)? {
        Value::String(value) => (!value.trim().is_empty()).then(|| value.trim().to_string()),
        Value::Number(value) => Some(value.to_string()),
        Value::Bool(value) => Some(if *value { "1" } else { "0" }.to_string()),
        Value::Null | Value::Array(_) | Value::Object(_) => None,
    }
}

fn parse_php_value(value: &str) -> Value {
    let value = value.trim().trim_end_matches(',').trim();
    if value.eq_ignore_ascii_case("null") {
        Value::Null
    } else if value.eq_ignore_ascii_case("true") {
        Value::Bool(true)
    } else if value.eq_ignore_ascii_case("false") {
        Value::Bool(false)
    } else if value.starts_with('\'') && value.ends_with('\'') && value.len() >= 2 {
        json!(
            value[1..value.len() - 1]
                .replace("\\'", "'")
                .replace("\\\\", "\\")
        )
    } else if let Ok(value) = value.parse::<i64>() {
        json!(value)
    } else if let Ok(value) = value.parse::<f64>() {
        json!(value)
    } else {
        json!(value)
    }
}

fn merge_config_params(config: &mut Map<String, Value>, params: &HashMap<String, String>) {
    let mut arrays = BTreeMap::<String, BTreeMap<usize, Value>>::new();
    for (key, value) in params {
        if key == "auth_data" {
            continue;
        }
        if let Some((base, index)) = key
            .split_once('[')
            .and_then(|(base, rest)| rest.strip_suffix(']').map(|rest| (base, rest)))
            .and_then(|(base, index)| index.parse::<usize>().ok().map(|index| (base, index)))
        {
            arrays
                .entry(base.to_string())
                .or_default()
                .insert(index, json_scalar(value));
            continue;
        }
        config.insert(key.clone(), json_scalar(value));
    }
    for (key, values) in arrays {
        config.insert(key, Value::Array(values.into_values().collect()));
    }
}

fn write_php_config(path: &str, value: &Value) -> Result<(), ApiError> {
    let path = std::path::Path::new(path);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|_| ApiError::legacy("配置目录不可写"))?;
    }
    let content = format!("<?php\n return {} ;", php_export(value, 0));
    std::fs::write(path, content).map_err(|_| ApiError::legacy("修改失败"))
}

fn php_export(value: &Value, indent: usize) -> String {
    match value {
        Value::Null => "NULL".to_string(),
        Value::Bool(value) => {
            if *value {
                "true".to_string()
            } else {
                "false".to_string()
            }
        }
        Value::Number(value) => value.to_string(),
        Value::String(value) => format!("'{}'", value.replace('\\', "\\\\").replace('\'', "\\'")),
        Value::Array(items) => {
            let inner_indent = " ".repeat(indent + 2);
            let closing_indent = " ".repeat(indent);
            let items = items
                .iter()
                .map(|value| format!("{inner_indent}{},", php_export(value, indent + 2)))
                .collect::<Vec<_>>()
                .join("\n");
            format!("array (\n{items}\n{closing_indent})")
        }
        Value::Object(object) => {
            let mut keys = object.keys().collect::<Vec<_>>();
            keys.sort();
            let inner_indent = " ".repeat(indent + 2);
            let closing_indent = " ".repeat(indent);
            let items = keys
                .into_iter()
                .map(|key| {
                    format!(
                        "{inner_indent}'{}' => {},",
                        key.replace('\'', "\\'"),
                        php_export(&object[key], indent + 2)
                    )
                })
                .collect::<Vec<_>>()
                .join("\n");
            format!("array (\n{items}\n{closing_indent})")
        }
    }
}

fn ensure_theme_name(name: &str) -> Result<(), ApiError> {
    let valid = !name.is_empty()
        && name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'));
    if valid {
        Ok(())
    } else {
        Err(ApiError::legacy("主题不存在"))
    }
}

fn standard_base64_decode(value: &str) -> Option<Vec<u8>> {
    let mut normalized = value.trim().replace('-', "+").replace('_', "/");
    match normalized.len() % 4 {
        0 => {}
        2 => normalized.push_str("=="),
        3 => normalized.push('='),
        _ => return None,
    }
    let bytes = normalized.as_bytes();
    let mut output = Vec::with_capacity(bytes.len() / 4 * 3);
    for chunk in bytes.chunks(4) {
        let c0 = base64_value(chunk[0])?;
        let c1 = base64_value(chunk[1])?;
        let c2 = if chunk[2] == b'=' {
            0
        } else {
            base64_value(chunk[2])?
        };
        let c3 = if chunk[3] == b'=' {
            0
        } else {
            base64_value(chunk[3])?
        };
        let combined = ((c0 as u32) << 18) | ((c1 as u32) << 12) | ((c2 as u32) << 6) | c3 as u32;
        output.push(((combined >> 16) & 0xff) as u8);
        if chunk[2] != b'=' {
            output.push(((combined >> 8) & 0xff) as u8);
        }
        if chunk[3] != b'=' {
            output.push((combined & 0xff) as u8);
        }
    }
    Some(output)
}

fn base64_value(byte: u8) -> Option<u8> {
    match byte {
        b'A'..=b'Z' => Some(byte - b'A'),
        b'a'..=b'z' => Some(byte - b'a' + 26),
        b'0'..=b'9' => Some(byte - b'0' + 52),
        b'+' => Some(62),
        b'/' => Some(63),
        _ => None,
    }
}

fn is_server_path(path: &str, action: &str) -> bool {
    path.starts_with("server/") && path.ends_with(&format!("/{action}"))
}

fn server_table_from_path(path: &str) -> Result<&'static str, ApiError> {
    let kind = server_kind_from_path(path)?;
    SERVER_TABLES
        .iter()
        .find(|(item, _)| *item == kind)
        .map(|(_, table)| *table)
        .ok_or_else(|| ApiError::legacy("Invalid server type"))
}

fn server_kind_from_path(path: &str) -> Result<&str, ApiError> {
    let mut parts = path.split('/');
    let _server = parts.next();
    parts
        .next()
        .ok_or_else(|| ApiError::legacy("Invalid server type"))
}

fn ensure_safe_table(table: &str) -> Result<(), ApiError> {
    let allowed = [
        "v2_plan",
        "v2_payment",
        "v2_notice",
        "v2_knowledge",
        "v2_coupon",
        "v2_giftcard",
        "v2_server_group",
        "v2_server_route",
        "v2_user",
        "v2_server_shadowsocks",
        "v2_server_vmess",
        "v2_server_trojan",
        "v2_server_tuic",
        "v2_server_vless",
        "v2_server_hysteria",
        "v2_server_anytls",
        "v2_server_v2node",
    ];
    if allowed.contains(&table) {
        Ok(())
    } else {
        Err(ApiError::legacy("Invalid table"))
    }
}

fn ensure_toggle_column(column: &str) -> Result<(), ApiError> {
    if matches!(column, "show" | "enable") {
        Ok(())
    } else {
        Err(ApiError::legacy("Invalid column"))
    }
}

fn first_day_of_month() -> i64 {
    let now = Local::now();
    Local
        .with_ymd_and_hms(now.year(), now.month(), 1, 0, 0, 0)
        .single()
        .map(|value| value.timestamp())
        .unwrap_or_else(|| Utc::now().timestamp())
}

fn first_day_of_previous_month() -> i64 {
    let now = Local::now();
    let (year, month) = if now.month() == 1 {
        (now.year() - 1, 12)
    } else {
        (now.year(), now.month() - 1)
    };
    Local
        .with_ymd_and_hms(year, month, 1, 0, 0, 0)
        .single()
        .map(|value| value.timestamp())
        .unwrap_or_else(|| Utc::now().timestamp())
}

fn start_of_today() -> i64 {
    let now = Local::now();
    Local
        .with_ymd_and_hms(now.year(), now.month(), now.day(), 0, 0, 0)
        .single()
        .map(|value| value.timestamp())
        .unwrap_or_else(|| Utc::now().timestamp())
}

fn start_of_yesterday() -> i64 {
    start_of_today() - 86_400
}
