//! Administrative platform transport contracts: operator configuration,
//! runtime health, logs and statistics.
//!
//! These are wire DTOs only. Infrastructure rows and Redis snapshots are
//! converted at an adapter boundary before they reach an HTTP handler.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::{common::Page, patch, patch::NonNull, time::Rfc3339Timestamp};

#[derive(Debug, Clone, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct MfaCodeRequest {
    pub code: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct MfaStatusView {
    pub totp_enabled: bool,
    #[schema(required)]
    pub totp_enabled_at: Option<Rfc3339Timestamp>,
    pub totp_required: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct TotpProvisioningView {
    pub secret: String,
    pub otpauth_url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct TicketConfigView {
    pub ticket_status: i16,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct DepositConfigView {
    pub deposit_bounus: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct InviteConfigView {
    pub invite_force: bool,
    pub invite_commission: i32,
    pub invite_gen_limit: i64,
    pub invite_never_expire: bool,
    pub commission_first_time_enable: bool,
    pub commission_auto_check_enable: bool,
    pub commission_withdraw_limit: String,
    pub commission_withdraw_method: Vec<String>,
    pub withdraw_close_enable: bool,
    pub commission_distribution_enable: bool,
    #[schema(required)]
    pub commission_distribution_l1: Option<f64>,
    #[schema(required)]
    pub commission_distribution_l2: Option<f64>,
    #[schema(required)]
    pub commission_distribution_l3: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct SiteConfigView {
    #[schema(required)]
    pub logo: Option<String>,
    pub force_https: bool,
    pub stop_register: bool,
    pub app_name: String,
    #[schema(required)]
    pub app_description: Option<String>,
    #[schema(required)]
    pub app_url: Option<String>,
    #[schema(required)]
    pub subscribe_url: Option<String>,
    #[schema(required)]
    pub subscribe_path: Option<String>,
    pub try_out_plan_id: i32,
    pub try_out_hour: f64,
    #[schema(required)]
    pub tos_url: Option<String>,
    pub currency: String,
    pub currency_symbol: String,
    pub legacy_hash_redirect_enable: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct SubscribeConfigView {
    pub plan_change_enable: bool,
    pub reset_traffic_method: i16,
    pub surplus_enable: bool,
    pub allow_new_period: bool,
    pub new_order_event_id: bool,
    pub renew_order_event_id: bool,
    pub change_order_event_id: bool,
    pub show_info_to_server_enable: bool,
    pub show_subscribe_method: i16,
    pub show_subscribe_expire: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct FrontendConfigView {
    pub frontend_theme_color: String,
    #[schema(required)]
    pub frontend_background_url: Option<String>,
    #[schema(required)]
    pub chat_widget_provider: Option<String>,
    #[schema(required)]
    pub chat_widget_crisp_website_id: Option<String>,
    #[schema(required)]
    pub chat_widget_tawk_property_id: Option<String>,
    #[schema(required)]
    pub chat_widget_tawk_widget_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct ServerConfigView {
    #[schema(required)]
    pub server_api_url: Option<String>,
    #[schema(required)]
    pub server_token: Option<String>,
    pub server_pull_interval: i32,
    pub server_push_interval: i32,
    pub server_node_report_min_traffic: i32,
    pub server_device_online_min_traffic: i32,
    pub device_limit_mode: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct EmailConfigView {
    #[schema(required)]
    pub email_template: Option<String>,
    #[schema(required)]
    pub email_host: Option<String>,
    #[schema(required)]
    pub email_port: Option<i32>,
    #[schema(required)]
    pub email_username: Option<String>,
    #[schema(required)]
    pub email_password: Option<String>,
    #[schema(required)]
    pub email_encryption: Option<String>,
    #[schema(required)]
    pub email_from_address: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct TelegramConfigView {
    pub telegram_bot_enable: bool,
    #[schema(required)]
    pub telegram_bot_token: Option<String>,
    #[schema(required)]
    pub telegram_discuss_link: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct AppDownloadConfigView {
    #[schema(required)]
    pub windows_version: Option<String>,
    #[schema(required)]
    pub windows_download_url: Option<String>,
    #[schema(required)]
    pub macos_version: Option<String>,
    #[schema(required)]
    pub macos_download_url: Option<String>,
    #[schema(required)]
    pub android_version: Option<String>,
    #[schema(required)]
    pub android_download_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct SafeConfigView {
    pub email_verify: bool,
    pub safe_mode_enable: bool,
    pub admin_mfa_force: bool,
    pub secure_path: String,
    pub email_whitelist_enable: bool,
    pub email_whitelist_suffix: Vec<String>,
    pub email_gmail_limit_enable: bool,
    pub recaptcha_enable: bool,
    #[schema(required)]
    pub recaptcha_key: Option<String>,
    #[schema(required)]
    pub recaptcha_site_key: Option<String>,
    pub register_limit_by_ip_enable: bool,
    pub register_limit_count: i64,
    pub register_limit_expire: i64,
    pub password_limit_enable: bool,
    pub password_limit_count: i64,
    pub password_limit_expire: i64,
}

/// GET config may contain all groups or exactly one requested group. The
/// optimistic-concurrency revision is always present.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct AdminConfigView {
    pub revision: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ticket: Option<TicketConfigView>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub deposit: Option<DepositConfigView>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub invite: Option<InviteConfigView>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub site: Option<SiteConfigView>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subscribe: Option<SubscribeConfigView>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub frontend: Option<FrontendConfigView>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub server: Option<ServerConfigView>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub email: Option<EmailConfigView>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub telegram: Option<TelegramConfigView>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub app: Option<AppDownloadConfigView>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub safe: Option<SafeConfigView>,
}

macro_rules! config_patch_request {
    ($($name:ident: $ty:ty),* $(,)?) => {
        /// Fixed, exhaustive PATCH whitelist. Absence retains; `null` resets
        /// a field to its backend-owned default; a value sets it.
        /// `secure_path` is the sole set-only setting.
        #[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
        #[serde(deny_unknown_fields)]
        pub struct AdminConfigPatchRequest {
            pub expected_revision: i64,
            #[serde(default, skip_serializing_if = "NonNull::is_retain")]
            #[schema(value_type = Option<String>)]
            pub secure_path: NonNull<String>,
            $(
                #[serde(default, skip_serializing_if = "Option::is_none", with = "patch")]
                #[schema(nullable = true)]
                pub $name: Option<Option<$ty>>,
            )*
        }
    };
}

config_patch_request! {
        ticket_status: i16,
        deposit_bounus: Vec<String>,
        invite_force: bool,
        invite_commission: i32,
        invite_gen_limit: i64,
        invite_never_expire: bool,
        commission_first_time_enable: bool,
        commission_auto_check_enable: bool,
        commission_withdraw_limit: String,
        commission_withdraw_method: Vec<String>,
        withdraw_close_enable: bool,
        commission_distribution_enable: bool,
        commission_distribution_l1: f64,
        commission_distribution_l2: f64,
        commission_distribution_l3: f64,
        logo: String,
        force_https: bool,
        stop_register: bool,
        app_name: String,
        app_description: String,
        app_url: String,
        subscribe_url: String,
        subscribe_path: String,
        try_out_plan_id: i32,
        try_out_hour: f64,
        tos_url: String,
        currency: String,
        currency_symbol: String,
        legacy_hash_redirect_enable: bool,
        plan_change_enable: bool,
        reset_traffic_method: i16,
        surplus_enable: bool,
        allow_new_period: bool,
        new_order_event_id: bool,
        renew_order_event_id: bool,
        change_order_event_id: bool,
        show_info_to_server_enable: bool,
        show_subscribe_method: i16,
        show_subscribe_expire: i64,
        frontend_theme_color: String,
        frontend_background_url: String,
        chat_widget_provider: String,
        chat_widget_crisp_website_id: String,
        chat_widget_tawk_property_id: String,
        chat_widget_tawk_widget_id: String,
        safe_mode_enable: bool,
        admin_mfa_force: bool,
        server_api_url: String,
        server_token: String,
        server_pull_interval: i32,
        server_push_interval: i32,
        server_node_report_min_traffic: i32,
        server_device_online_min_traffic: i32,
        device_limit_mode: bool,
        email_template: String,
        email_host: String,
        email_port: i32,
        email_username: String,
        email_password: String,
        email_encryption: String,
        email_from_address: String,
        telegram_bot_enable: bool,
        telegram_bot_token: String,
        telegram_discuss_link: String,
        windows_version: String,
        windows_download_url: String,
        macos_version: String,
        macos_download_url: String,
        android_version: String,
        android_download_url: String,
        email_verify: bool,
        email_whitelist_enable: bool,
        email_whitelist_suffix: Vec<String>,
        email_gmail_limit_enable: bool,
        recaptcha_enable: bool,
        recaptcha_key: String,
        recaptcha_site_key: String,
        register_limit_by_ip_enable: bool,
        register_limit_count: i64,
        register_limit_expire: i64,
        password_limit_enable: bool,
        password_limit_count: i64,
        password_limit_expire: i64,
}

#[derive(Debug, Clone, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct TelegramWebhookRequest {
    #[serde(default)]
    pub telegram_bot_token: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct TestMailResult {
    pub sent: bool,
    #[schema(required)]
    pub log: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct SystemStatusView {
    pub schedule: bool,
    pub horizon: bool,
    #[schema(required)]
    pub schedule_last_runtime: Option<Rfc3339Timestamp>,
    pub log_channel: String,
    pub log_level: String,
    pub cache_driver: String,
    pub backend_version: String,
    pub frontend_version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct QueuePeriods {
    pub failed_jobs: i64,
    pub recent_jobs: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct QueueStatsView {
    pub failed_jobs: i64,
    pub jobs_per_minute: usize,
    pub paused_masters: i64,
    pub periods: QueuePeriods,
    pub processes: i64,
    #[schema(required)]
    pub queue_with_max_runtime: Option<String>,
    #[schema(required)]
    pub queue_with_max_throughput: Option<String>,
    pub recent_jobs: i64,
    pub status: bool,
    pub wait: BTreeMap<String, i64>,
    pub last_run_at: BTreeMap<String, Rfc3339Timestamp>,
    pub last_success_at: BTreeMap<String, Rfc3339Timestamp>,
    pub last_failure_at: BTreeMap<String, Rfc3339Timestamp>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct QueueWorkloadItem {
    pub name: String,
    pub processes: i64,
    pub length: i64,
    pub wait: i64,
    pub recent_jobs: i64,
    pub failed_jobs: i64,
    #[schema(required)]
    pub last_run_at: Option<Rfc3339Timestamp>,
    #[schema(required)]
    pub last_success_at: Option<Rfc3339Timestamp>,
    #[schema(required)]
    pub last_failure_at: Option<Rfc3339Timestamp>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct QueueMasterView {
    pub name: String,
    pub status: String,
    #[schema(required)]
    pub pid: Option<i64>,
    pub supervisors: Vec<String>,
    #[schema(required)]
    pub last_seen_at: Option<Rfc3339Timestamp>,
    #[schema(required)]
    pub schedule_last_seen_at: Option<Rfc3339Timestamp>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct SystemLogView {
    pub id: i64,
    pub title: String,
    #[schema(required)]
    pub level: Option<String>,
    #[schema(required)]
    pub host: Option<String>,
    pub uri: String,
    pub method: String,
    #[schema(required)]
    pub data: Option<String>,
    #[schema(required)]
    pub ip: Option<String>,
    #[schema(required)]
    pub context: Option<String>,
    pub created_at: Rfc3339Timestamp,
    pub updated_at: Rfc3339Timestamp,
}

pub type SystemLogPage = Page<SystemLogView>;

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct AuditLogView {
    pub id: i64,
    pub actor_id: i64,
    pub actor_email: String,
    pub session_id: String,
    pub surface: String,
    pub method: String,
    pub path: String,
    pub status_code: i32,
    #[schema(required)]
    pub client_ip: Option<String>,
    #[schema(required)]
    pub request_id: Option<String>,
    pub created_at: Rfc3339Timestamp,
}

pub type AuditLogPage = Page<AuditLogView>;

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct AdminStatSummaryView {
    pub online_user: i64,
    pub month_income: i64,
    pub month_register_total: i64,
    pub day_register_total: i64,
    pub ticket_pending_total: i64,
    pub commission_pending_total: i64,
    pub payment_reconciliation_pending_total: i64,
    pub payment_reconciliation_pending_amount: i64,
    pub day_income: i64,
    pub last_month_income: i64,
    pub commission_month_payout: i64,
    pub commission_last_month_payout: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct ServerRankView {
    pub server_id: i64,
    pub server_type: String,
    #[schema(required)]
    pub server_name: Option<String>,
    pub u: i64,
    pub d: i64,
    pub total: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct UserRankView {
    pub user_id: i64,
    pub email: String,
    pub u: i64,
    pub d: i64,
    pub total: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct StatSeriesPointView {
    pub series: String,
    pub date: String,
    pub value: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct AdminUserTrafficView {
    pub record_at: Rfc3339Timestamp,
    pub u: i64,
    pub d: i64,
    pub server_rate: f64,
}

pub type AdminUserTrafficPage = Page<AdminUserTrafficView>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_patch_rejects_unknown_fields() {
        let error = serde_json::from_value::<AdminConfigPatchRequest>(serde_json::json!({
            "expected_revision": 7,
            "made_up": true
        }))
        .expect_err("unknown field must be rejected");
        assert!(error.to_string().contains("unknown field"));
    }

    #[test]
    fn config_patch_preserves_clear_and_absent() {
        let patch = serde_json::from_value::<AdminConfigPatchRequest>(serde_json::json!({
            "expected_revision": 7,
            "app_url": null,
            "secure_path": "operator"
        }))
        .expect("valid patch");
        assert_eq!(patch.app_url, Some(None));
        assert_eq!(patch.logo, None);
        assert_eq!(patch.secure_path, NonNull::Set("operator".to_owned()));
    }
}
