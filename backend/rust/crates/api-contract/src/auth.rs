//! Field-level wire contracts for the unauthenticated public surface and the
//! modern authentication/session family.

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// Configuration required before a visitor authenticates.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct PublicConfig {
    #[schema(required)]
    pub tos_url: Option<String>,
    pub is_email_verify: bool,
    pub is_invite_force: bool,
    pub email_whitelist_suffix: Vec<String>,
    pub is_recaptcha: bool,
    #[schema(required)]
    pub recaptcha_site_key: Option<String>,
    #[schema(required)]
    pub app_description: Option<String>,
    #[schema(required)]
    pub app_url: Option<String>,
    #[schema(required)]
    pub logo: Option<String>,
}

#[derive(Debug, Clone, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct InviteViewRequest {
    pub invite_code: String,
}

#[derive(Debug, Clone, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct LoginRequest {
    pub email: String,
    #[schema(min_length = 8)]
    pub password: String,
    #[serde(default)]
    pub totp_code: Option<String>,
}

#[derive(Debug, Clone, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct RegisterRequest {
    pub email: String,
    #[schema(min_length = 8)]
    pub password: String,
    #[serde(default)]
    pub invite_code: Option<String>,
    #[serde(default)]
    pub email_code: Option<String>,
    #[serde(default)]
    pub recaptcha_data: Option<String>,
}

/// Login, registration, and one-time-token exchange response.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct AuthData {
    pub is_admin: bool,
    pub auth_data: String,
}

#[derive(Debug, Clone, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct TokenLoginRequest {
    pub verify: String,
}

#[derive(Debug, Clone, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct PasswordResetRequest {
    pub email: String,
    pub email_code: String,
    #[schema(min_length = 8)]
    pub password: String,
}

#[derive(Debug, Clone, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct EmailCodeRequest {
    pub email: String,
    #[serde(default)]
    pub is_forget: Option<bool>,
    #[serde(default)]
    pub recaptcha_data: Option<String>,
}

#[derive(Debug, Clone, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct StepUpRequest {
    #[schema(min_length = 8)]
    pub password: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct StepUpGrant {
    pub step_up_token: String,
    #[schema(minimum = 1)]
    pub expires_in: u64,
}

#[derive(Debug, Clone, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct QuickLoginUrlRequest {
    #[serde(default)]
    pub redirect: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct QuickLoginUrl {
    pub url: String,
}

/// A session probe deliberately omits privileged fields when they do not
/// apply. For a staff session `is_staff` and `admin_permissions` appear as a
/// pair, even when the permission list is empty.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct SessionState {
    pub is_login: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub is_admin: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub is_staff: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub admin_permissions: Option<Vec<String>>,
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn registration_does_not_accept_login_only_mfa_fields() {
        let value = json!({
            "email": "member@example.test",
            "password": "password123",
            "totp_code": "123456"
        });
        assert!(serde_json::from_value::<RegisterRequest>(value).is_err());
    }

    #[test]
    fn logged_out_session_omits_privileged_fields() {
        let value = serde_json::to_value(SessionState {
            is_login: false,
            is_admin: None,
            is_staff: None,
            admin_permissions: None,
        })
        .expect("session state");
        assert_eq!(value, json!({ "is_login": false }));
    }
}
