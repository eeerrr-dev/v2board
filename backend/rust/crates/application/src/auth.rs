//! Authentication and session use cases over explicit outbound ports.
//!
//! This module owns validation, ordering, security policy, and durable/cache
//! consistency decisions. PostgreSQL transactions, Redis scripts, password
//! hashing, TOTP cryptography, reCAPTCHA, SMTP, and subscription-link minting
//! are outer adapters.

use crate::RepositoryError;

pub type RepositoryResult<T> = Result<T, RepositoryError>;

const MAX_EMAIL_CHARS: usize = 254;
const MAX_PASSWORD_CHARS: usize = 128;
const MAX_INVITE_CODE_BYTES: usize = 255;
const MAX_EMAIL_CODE_BYTES: usize = 64;
const MAX_RECAPTCHA_DATA_BYTES: usize = 4_096;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AuthCode {
    AccountSuspended,
    EmailAlreadyRegistered,
    EmailNotRegistered,
    EmailSendRateLimited,
    EmailSuffixNotAllowed,
    GmailAliasNotSupported,
    InvalidCredentials,
    InvalidEmailCode,
    InvalidInviteCode,
    InvalidParameter,
    InvalidToken,
    MailInvalid,
    MailSendFailed,
    MailSenderNotConfigured,
    MfaAlreadyEnabled,
    MfaCodeInvalid,
    MfaCodeRequired,
    MfaNotEnabled,
    MfaSetupMissing,
    OldPasswordIncorrect,
    PasswordAttemptsRateLimited,
    PasswordResetFailed,
    RecaptchaFailed,
    RegisterIpRateLimited,
    RegistrationClosed,
    UserNotRegistered,
}

#[derive(Debug, thiserror::Error)]
pub enum AuthError {
    #[error("validation failed for {field}: {message}")]
    Validation {
        field: &'static str,
        message: &'static str,
    },
    #[error("authentication business error: {code:?}")]
    Business {
        code: AuthCode,
        detail: Option<String>,
    },
    #[error("session is not authorized")]
    Unauthorized,
    #[error("authentication internal error: {0}")]
    Internal(String),
    #[error(transparent)]
    Repository(#[from] RepositoryError),
}

impl AuthError {
    pub fn business(code: AuthCode) -> Self {
        Self::Business { code, detail: None }
    }

    pub fn business_detail(code: AuthCode, detail: impl Into<String>) -> Self {
        Self::Business {
            code,
            detail: Some(detail.into()),
        }
    }

    pub const fn validation(field: &'static str, message: &'static str) -> Self {
        Self::Validation { field, message }
    }

    pub fn is_code(&self, expected: AuthCode) -> bool {
        matches!(self, Self::Business { code, .. } if *code == expected)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TrialDuration {
    Seconds(i64),
    Negative,
    OutOfRange,
}

#[derive(Clone, Debug)]
pub struct AuthPolicy {
    pub app_name: String,
    pub app_url: Option<String>,
    pub password_limit_enable: bool,
    pub password_limit_count: i64,
    pub password_limit_expire_minutes: i64,
    pub password_limit_ttl_seconds: u64,
    pub register_limit_by_ip_enable: bool,
    pub register_limit_count: i64,
    pub register_limit_expire_minutes: i64,
    pub register_limit_ttl_seconds: u64,
    pub stop_register: bool,
    pub invite_force: bool,
    pub invite_never_expire: bool,
    pub email_verify: bool,
    pub email_whitelist_enable: bool,
    pub email_whitelist_suffix: Vec<String>,
    pub email_gmail_limit_enable: bool,
    pub recaptcha_enable: bool,
    pub trial_plan_id: i32,
    pub trial_duration: TrialDuration,
    pub auth_session_ttl_seconds: u64,
    pub privileged_auth_session_ttl_seconds: u64,
    pub auth_session_max_per_user: i64,
    pub privileged_step_up_max_attempts: i64,
    pub privileged_step_up_attempt_window_seconds: u64,
    pub privileged_step_up_ttl_seconds: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AuthData {
    pub is_admin: bool,
    pub auth_data: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AuthUser {
    pub id: i64,
    pub email: String,
    pub is_admin: i16,
    pub is_staff: i16,
    pub admin_permissions: Vec<String>,
    pub session_id: String,
    pub authenticated_at: i64,
    pub password_authenticated: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UserSession {
    pub session_id: String,
    pub ip: String,
    pub ua: String,
    pub login_at: i64,
    pub current: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AuthAccount {
    pub id: i64,
    pub email: String,
    pub password_hash: String,
    pub password_algo: Option<String>,
    pub password_salt: Option<String>,
    pub session_epoch: i64,
    pub banned: bool,
    pub is_admin: i16,
    pub is_staff: i16,
    pub admin_permissions: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SessionIdentity {
    pub user_id: i64,
    pub session_id: String,
    pub session_epoch: i64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SessionMetadata {
    pub ip: Option<String>,
    pub login_at: i64,
    pub user_agent: Option<String>,
    pub expires_at: Option<i64>,
    pub password_authenticated: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StoredSession {
    pub session_id: String,
    pub metadata: SessionMetadata,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MfaRecord {
    pub secret_nonce: Vec<u8>,
    pub secret_ciphertext: Vec<u8>,
    pub secret_tag: Vec<u8>,
    pub enabled_at: Option<i64>,
    pub last_step: i64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SealedMfaSecret {
    pub secret_nonce: Vec<u8>,
    pub secret_ciphertext: Vec<u8>,
    pub secret_tag: Vec<u8>,
    pub public_secret: String,
    pub otpauth_url: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MfaStatus {
    pub totp_enabled: bool,
    pub totp_enabled_at: Option<i64>,
    pub totp_required: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TotpProvisioning {
    pub secret: String,
    pub otpauth_url: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RegisterInput {
    pub email: String,
    pub password: String,
    pub invite_code: Option<String>,
    pub email_code: Option<String>,
    pub recaptcha_data: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ForgetInput {
    pub email: String,
    pub email_code: String,
    pub password: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EmailVerifyInput {
    pub email: String,
    pub is_forget: Option<bool>,
    pub recaptcha_data: Option<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EmailCodeScope {
    Registration,
    PasswordReset,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LimitedEmailCodeResult {
    Consumed,
    Incorrect,
    Limited,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RegistrationReservation {
    pub client_ip: String,
    pub token: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InviteCodeRecord {
    pub id: i32,
    pub user_id: i64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TrialPlanRecord {
    pub id: i32,
    pub group_id: i32,
    pub transfer_gib: i64,
    pub device_limit: Option<i32>,
    pub speed_limit: Option<i32>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NewAuthAccount {
    pub invite_user_id: Option<i64>,
    pub email: String,
    pub password_hash: String,
    pub uuid: String,
    pub token: String,
    pub transfer_enable: i64,
    pub device_limit: Option<i32>,
    pub group_id: Option<i32>,
    pub plan_id: Option<i32>,
    pub speed_limit: Option<i32>,
    pub expired_at: Option<i64>,
    pub created_at: i64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InsertAuthAccountOutcome {
    Inserted(i64),
    EmailAlreadyRegistered,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MailDeliveryError {
    SenderNotConfigured { detail: Option<String> },
    InvalidSender,
    InvalidRecipient,
    BuildFailed(String),
    TimedOut,
    SendFailed(String),
    Infrastructure(RepositoryError),
}

#[allow(async_fn_in_trait)]
pub trait RegistrationTransaction: Send {
    async fn lock_invite_code(&mut self, code: &str) -> RepositoryResult<Option<InviteCodeRecord>>;
    async fn consume_invite_code(&mut self, id: i32, updated_at: i64) -> RepositoryResult<bool>;
    async fn lock_trial_plan(&mut self, plan_id: i32) -> RepositoryResult<Option<TrialPlanRecord>>;
    async fn insert_account(
        &mut self,
        account: NewAuthAccount,
    ) -> RepositoryResult<InsertAuthAccountOutcome>;
    async fn commit(self) -> RepositoryResult<()>;
}

#[allow(async_fn_in_trait)]
pub trait AuthRepository: Send + Sync {
    type Registration<'a>: RegistrationTransaction
    where
        Self: 'a;

    async fn begin_registration(&self) -> RepositoryResult<Self::Registration<'_>>;
    async fn find_account_by_email(&self, email: &str) -> RepositoryResult<Option<AuthAccount>>;
    async fn find_account_by_id(&self, user_id: i64) -> RepositoryResult<Option<AuthAccount>>;
    async fn active_session_epoch(&self, user_id: i64) -> RepositoryResult<Option<i64>>;
    async fn rehash_password(
        &self,
        user_id: i64,
        expected_hash: &str,
        password_hash: &str,
        updated_at: i64,
    ) -> RepositoryResult<()>;
    async fn update_password(
        &self,
        user_id: i64,
        password_hash: &str,
        updated_at: i64,
    ) -> RepositoryResult<bool>;
    async fn change_password_if_current(
        &self,
        user_id: i64,
        expected_hash: &str,
        expected_session_epoch: i64,
        password_hash: &str,
        updated_at: i64,
    ) -> RepositoryResult<bool>;
    async fn update_security(
        &self,
        user_id: i64,
        uuid: &str,
        token: &str,
        updated_at: i64,
    ) -> RepositoryResult<bool>;
    async fn increment_invite_view(&self, code: &str, updated_at: i64) -> RepositoryResult<()>;
    async fn find_mfa(&self, user_id: i64) -> RepositoryResult<Option<MfaRecord>>;
    async fn upsert_pending_mfa(
        &self,
        user_id: i64,
        secret: &SealedMfaSecret,
        updated_at: i64,
    ) -> RepositoryResult<bool>;
    async fn enable_mfa(
        &self,
        user_id: i64,
        accepted_step: i64,
        updated_at: i64,
    ) -> RepositoryResult<bool>;
    async fn consume_mfa_step(
        &self,
        user_id: i64,
        accepted_step: i64,
        updated_at: i64,
    ) -> RepositoryResult<bool>;
    async fn disable_mfa(&self, user_id: i64, accepted_step: i64) -> RepositoryResult<bool>;
}

#[allow(async_fn_in_trait)]
pub trait AuthCache: Send + Sync {
    async fn reserve_login_attempt(
        &self,
        email: &str,
        client_ip: Option<&str>,
        account_limit: i64,
        ip_limit: i64,
        ttl_seconds: u64,
    ) -> RepositoryResult<bool>;
    async fn release_login_attempt(&self, email: &str, client_ip: Option<&str>);
    async fn reserve_registration_slot(
        &self,
        reservation: &RegistrationReservation,
        now: i64,
        expires_at: i64,
        limit: i64,
    ) -> RepositoryResult<bool>;
    async fn release_registration_slot(&self, reservation: &RegistrationReservation);
    async fn consume_email_code(
        &self,
        email: &str,
        code: &str,
        scope: EmailCodeScope,
        limit: i64,
        ttl_seconds: u64,
    ) -> RepositoryResult<LimitedEmailCodeResult>;
    async fn increment_email_send_limit(
        &self,
        client_ip: &str,
        limit: i64,
        ttl_seconds: u64,
    ) -> RepositoryResult<bool>;
    async fn reserve_email_code(&self, email: &str, code: &str, now: i64)
    -> RepositoryResult<bool>;
    async fn release_email_code(&self, email: &str, code: &str);
    async fn put_temporary_token(
        &self,
        token: &str,
        user_id: i64,
        session_epoch: i64,
        ttl_seconds: u64,
    ) -> RepositoryResult<()>;
    async fn take_temporary_token(&self, token: &str) -> RepositoryResult<Option<SessionIdentity>>;
    async fn add_session(
        &self,
        identity: &SessionIdentity,
        metadata: &SessionMetadata,
        bearer: &str,
        ttl_seconds: u64,
        maximum_sessions: i64,
        now: i64,
    ) -> RepositoryResult<bool>;
    async fn session_identity(&self, bearer: &str) -> RepositoryResult<Option<SessionIdentity>>;
    async fn session_metadata(
        &self,
        user_id: i64,
        session_id: &str,
    ) -> RepositoryResult<Option<SessionMetadata>>;
    async fn sessions(&self, user_id: i64) -> RepositoryResult<Vec<StoredSession>>;
    async fn remove_session(&self, user_id: i64, session_id: &str) -> RepositoryResult<()>;
    async fn remove_all_sessions(&self, user_id: i64) -> RepositoryResult<()>;
    async fn reserve_step_up_attempt(
        &self,
        user_id: i64,
        client_ip: Option<&str>,
        user_limit: i64,
        ip_limit: i64,
        ttl_seconds: u64,
    ) -> RepositoryResult<bool>;
    async fn clear_step_up_attempts(&self, user_id: i64, client_ip: Option<&str>);
    async fn put_step_up(
        &self,
        token: &str,
        user_id: i64,
        session_id: &str,
        ttl_seconds: u64,
    ) -> RepositoryResult<bool>;
    async fn step_up_identity(&self, token: &str) -> RepositoryResult<Option<(i64, String)>>;
}

#[allow(async_fn_in_trait)]
pub trait AuthExternal: Send + Sync {
    fn now(&self) -> i64;
    fn uuid(&self) -> RepositoryResult<String>;
    fn compact_id(&self) -> RepositoryResult<String>;
    fn opaque_token(&self) -> RepositoryResult<String>;
    fn email_code(&self) -> RepositoryResult<String>;
    async fn hash_password(&self, password: &str) -> RepositoryResult<String>;
    async fn verify_password(
        &self,
        algo: Option<&str>,
        salt: Option<&str>,
        password: &str,
        stored_hash: &str,
    ) -> RepositoryResult<bool>;
    async fn verify_dummy_password(&self, password: &str) -> RepositoryResult<()>;
    fn password_needs_rehash(&self, algo: Option<&str>, stored_hash: &str) -> bool;
    async fn verify_recaptcha(&self, token: &str) -> RepositoryResult<bool>;
    async fn send_verification_mail(
        &self,
        to: &str,
        app_name: &str,
        app_url: &str,
        code: &str,
    ) -> Result<(), MailDeliveryError>;
    async fn subscribe_url(&self, user_id: i64, token: &str) -> RepositoryResult<String>;
    fn create_mfa_secret(
        &self,
        user_id: i64,
        email: &str,
        issuer: &str,
    ) -> RepositoryResult<SealedMfaSecret>;
    fn accepted_mfa_step(
        &self,
        user_id: i64,
        record: &MfaRecord,
        code: &str,
        now: i64,
    ) -> RepositoryResult<Option<i64>>;
}

#[derive(Clone, Debug)]
pub struct AuthService<R, C, E> {
    repository: R,
    cache: C,
    external: E,
    policy: AuthPolicy,
}

impl<R, C, E> AuthService<R, C, E>
where
    R: AuthRepository,
    C: AuthCache,
    E: AuthExternal,
{
    pub const fn new(repository: R, cache: C, external: E, policy: AuthPolicy) -> Self {
        Self {
            repository,
            cache,
            external,
            policy,
        }
    }

    pub fn login_redirect_url(&self, token: &str, redirect: Option<&str>) -> String {
        let redirect = redirect
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or("dashboard");
        let path = format!("/login?verify={token}&redirect={redirect}");
        self.policy
            .app_url
            .as_deref()
            .filter(|value| !value.is_empty())
            .map_or_else(|| path.clone(), |app_url| format!("{app_url}{path}"))
    }
}

#[cfg(test)]
mod tests;

fn validate_email(email: &str) -> Result<(), AuthError> {
    let email = email.trim();
    if email.is_empty() {
        return Err(AuthError::validation("email", "Email can not be empty"));
    }
    if !is_valid_email(email) || email.chars().count() > MAX_EMAIL_CHARS {
        return Err(AuthError::validation("email", "Email format is incorrect"));
    }
    Ok(())
}

fn normalize_email(email: &str) -> String {
    email.trim().to_ascii_lowercase()
}

fn validate_password(password: &str) -> Result<(), AuthError> {
    if password.is_empty() {
        return Err(AuthError::validation(
            "password",
            "Password can not be empty",
        ));
    }
    let length = password.chars().count();
    if !(8..=MAX_PASSWORD_CHARS).contains(&length) {
        return Err(AuthError::validation(
            "password",
            "Password must be greater than 8 digits",
        ));
    }
    Ok(())
}

fn validate_forget(email: &str, password: &str, email_code: &str) -> Result<(), AuthError> {
    validate_email(email)?;
    if email.trim().chars().count() > MAX_EMAIL_CHARS {
        return Err(AuthError::validation("email", "Email format is incorrect"));
    }
    validate_password(password)?;
    if password.chars().count() > 64 {
        return Err(AuthError::validation(
            "password",
            "Password must be greater than 8 digits",
        ));
    }
    if email_code.trim().is_empty() {
        return Err(AuthError::validation(
            "email_code",
            "Email verification code cannot be empty",
        ));
    }
    if email_code.chars().count() != 6
        || !email_code
            .chars()
            .all(|character| character.is_ascii_digit())
    {
        return Err(AuthError::validation(
            "email_code",
            "Incorrect email verification code",
        ));
    }
    Ok(())
}

fn validate_change_password(old_password: &str, new_password: &str) -> Result<(), AuthError> {
    if old_password.is_empty() {
        return Err(AuthError::validation(
            "old_password",
            "Old password cannot be empty",
        ));
    }
    if old_password.chars().count() > MAX_PASSWORD_CHARS {
        return Err(AuthError::validation(
            "old_password",
            "The old password is wrong",
        ));
    }
    if new_password.is_empty() {
        return Err(AuthError::validation(
            "new_password",
            "New password cannot be empty",
        ));
    }
    let length = new_password.chars().count();
    if !(8..=MAX_PASSWORD_CHARS).contains(&length) {
        return Err(AuthError::validation(
            "new_password",
            "Password must be greater than 8 digits",
        ));
    }
    Ok(())
}

fn validate_registration_auxiliary_inputs(input: &RegisterInput) -> Result<(), AuthError> {
    if input
        .invite_code
        .as_deref()
        .is_some_and(|value| value.len() > MAX_INVITE_CODE_BYTES)
    {
        return Err(AuthError::validation(
            "invite_code",
            "Invalid invitation code",
        ));
    }
    if input
        .email_code
        .as_deref()
        .is_some_and(|value| value.len() > MAX_EMAIL_CODE_BYTES)
    {
        return Err(AuthError::validation(
            "email_code",
            "Incorrect email verification code",
        ));
    }
    if input
        .recaptcha_data
        .as_deref()
        .is_some_and(|value| value.len() > MAX_RECAPTCHA_DATA_BYTES)
    {
        return Err(AuthError::validation(
            "recaptcha_data",
            "Invalid code is incorrect",
        ));
    }
    Ok(())
}

fn is_valid_email(email: &str) -> bool {
    if email.chars().any(char::is_whitespace) {
        return false;
    }
    match email.split_once('@') {
        Some((local, host)) => !local.is_empty() && !host.is_empty() && !host.contains('@'),
        None => false,
    }
}

fn checked_trial_transfer_bytes(transfer_gib: i64) -> Result<i64, AuthError> {
    if transfer_gib < 0 {
        return Err(AuthError::business_detail(
            AuthCode::InvalidParameter,
            "Trial plan traffic allowance must not be negative",
        ));
    }
    transfer_gib.checked_mul(1_073_741_824).ok_or_else(|| {
        AuthError::business_detail(
            AuthCode::InvalidParameter,
            "Trial plan traffic allowance exceeds the supported range",
        )
    })
}

fn truncate_utf8(mut value: String, maximum_bytes: usize) -> String {
    if value.len() <= maximum_bytes {
        return value;
    }
    let mut boundary = maximum_bytes;
    while !value.is_char_boundary(boundary) {
        boundary -= 1;
    }
    value.truncate(boundary);
    value
}

fn session_ttl_seconds(ordinary: u64, privileged: u64, is_admin: i16, is_staff: i16) -> u64 {
    if is_admin != 0 || is_staff != 0 {
        privileged
    } else {
        ordinary
    }
}

#[derive(Default)]
struct TrialPlanBinding {
    transfer_enable: i64,
    device_limit: Option<i32>,
    group_id: Option<i32>,
    plan_id: Option<i32>,
    speed_limit: Option<i32>,
    expired_at: Option<i64>,
}

impl<R, C, E> AuthService<R, C, E>
where
    R: AuthRepository,
    C: AuthCache,
    E: AuthExternal,
{
    pub async fn register(
        &self,
        input: RegisterInput,
        client_ip: Option<String>,
        user_agent: Option<String>,
    ) -> Result<AuthData, AuthError> {
        validate_email(&input.email)?;
        validate_password(&input.password)?;
        validate_registration_auxiliary_inputs(&input)?;
        let email = input.email.trim().to_string();
        let reservation = self.reserve_registration_slot(client_ip.as_deref()).await?;
        let registered = self.register_account(&input, &email).await;
        let user_id = match registered {
            Ok(user_id) => user_id,
            Err(error) => {
                if let Some(reservation) = reservation.as_ref() {
                    self.cache.release_registration_slot(reservation).await;
                }
                return Err(error);
            }
        };
        self.auth_data_for_user(user_id, Some(0), client_ip, user_agent, true)
            .await
    }

    async fn reserve_registration_slot(
        &self,
        client_ip: Option<&str>,
    ) -> Result<Option<RegistrationReservation>, AuthError> {
        let Some(client_ip) = client_ip.filter(|_| self.policy.register_limit_by_ip_enable) else {
            return Ok(None);
        };
        let reservation = RegistrationReservation {
            client_ip: client_ip.to_string(),
            token: self.external.compact_id()?,
        };
        let now = self.external.now();
        let ttl = i64::try_from(self.policy.register_limit_ttl_seconds)
            .unwrap_or(i64::MAX)
            .max(1);
        if !self
            .cache
            .reserve_registration_slot(
                &reservation,
                now,
                now.saturating_add(ttl),
                self.policy.register_limit_count,
            )
            .await?
        {
            return Err(AuthError::business_detail(
                AuthCode::RegisterIpRateLimited,
                format!(
                    "Register frequently, please try again after {} minute",
                    self.policy.register_limit_expire_minutes
                ),
            ));
        }
        Ok(Some(reservation))
    }

    async fn register_account(&self, input: &RegisterInput, email: &str) -> Result<i64, AuthError> {
        self.verify_recaptcha(input.recaptcha_data.as_deref())
            .await?;
        self.validate_register_email(email)?;
        if self.policy.stop_register {
            return Err(AuthError::business(AuthCode::RegistrationClosed));
        }
        let invite_code = input
            .invite_code
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty());
        if self.policy.invite_force && invite_code.is_none() {
            return Err(AuthError::business_detail(
                AuthCode::InvalidInviteCode,
                "You must use the invitation code to register",
            ));
        }
        if self.policy.email_verify {
            let email_code = input
                .email_code
                .as_deref()
                .map(str::trim)
                .filter(|value| {
                    value.len() == 6 && value.chars().all(|character| character.is_ascii_digit())
                })
                .ok_or_else(|| AuthError::business(AuthCode::InvalidEmailCode))?;
            match self
                .cache
                .consume_email_code(
                    &normalize_email(email),
                    email_code,
                    EmailCodeScope::Registration,
                    3,
                    300,
                )
                .await?
            {
                LimitedEmailCodeResult::Consumed => {}
                LimitedEmailCodeResult::Incorrect => {
                    return Err(AuthError::business(AuthCode::InvalidEmailCode));
                }
                LimitedEmailCodeResult::Limited => {
                    return Err(AuthError::business(AuthCode::RegisterIpRateLimited));
                }
            }
        }

        let password_hash = self.external.hash_password(&input.password).await?;
        let uuid = self.external.uuid()?;
        let token = self.external.compact_id()?;
        let now = self.external.now();
        let mut transaction = self.repository.begin_registration().await?;
        let invite_user_id = match invite_code {
            Some(code) => match transaction.lock_invite_code(code).await? {
                Some(invite) => {
                    if !self.policy.invite_never_expire
                        && !transaction.consume_invite_code(invite.id, now).await?
                    {
                        return Err(AuthError::business(AuthCode::InvalidInviteCode));
                    }
                    Some(invite.user_id)
                }
                None if self.policy.invite_force => {
                    return Err(AuthError::business(AuthCode::InvalidInviteCode));
                }
                None => None,
            },
            None => None,
        };
        let trial = self.trial_plan(&mut transaction, now).await?;
        let account = NewAuthAccount {
            invite_user_id,
            email: email.to_string(),
            password_hash,
            uuid,
            token,
            transfer_enable: trial.transfer_enable,
            device_limit: trial.device_limit,
            group_id: trial.group_id,
            plan_id: trial.plan_id,
            speed_limit: trial.speed_limit,
            expired_at: trial.expired_at,
            created_at: now,
        };
        let user_id = match transaction.insert_account(account).await? {
            InsertAuthAccountOutcome::Inserted(user_id) => user_id,
            InsertAuthAccountOutcome::EmailAlreadyRegistered => {
                return Err(AuthError::business(AuthCode::EmailAlreadyRegistered));
            }
        };
        transaction.commit().await?;
        Ok(user_id)
    }

    async fn trial_plan<T>(
        &self,
        transaction: &mut T,
        now: i64,
    ) -> Result<TrialPlanBinding, AuthError>
    where
        T: RegistrationTransaction,
    {
        if self.policy.trial_plan_id <= 0 {
            return Ok(TrialPlanBinding::default());
        }
        let Some(plan) = transaction
            .lock_trial_plan(self.policy.trial_plan_id)
            .await?
        else {
            return Ok(TrialPlanBinding::default());
        };
        let transfer_enable = checked_trial_transfer_bytes(plan.transfer_gib)?;
        let duration_seconds = match self.policy.trial_duration {
            TrialDuration::Seconds(seconds) => seconds,
            TrialDuration::Negative => {
                return Err(AuthError::business_detail(
                    AuthCode::InvalidParameter,
                    "Trial plan duration must not be negative",
                ));
            }
            TrialDuration::OutOfRange => {
                return Err(AuthError::business_detail(
                    AuthCode::InvalidParameter,
                    "Trial plan duration exceeds the supported range",
                ));
            }
        };
        let expired_at = now.checked_add(duration_seconds).ok_or_else(|| {
            AuthError::business_detail(
                AuthCode::InvalidParameter,
                "Trial plan expiry exceeds the supported range",
            )
        })?;
        Ok(TrialPlanBinding {
            transfer_enable,
            device_limit: plan.device_limit,
            group_id: Some(plan.group_id),
            plan_id: Some(plan.id),
            speed_limit: plan.speed_limit,
            expired_at: Some(expired_at),
        })
    }

    pub async fn passport_pv(&self, invite_code: Option<&str>) -> Result<bool, AuthError> {
        if let Some(code) = invite_code.map(str::trim).filter(|value| !value.is_empty()) {
            if code.len() > MAX_INVITE_CODE_BYTES {
                return Err(AuthError::validation(
                    "invite_code",
                    "Invalid invitation code",
                ));
            }
            self.repository
                .increment_invite_view(code, self.external.now())
                .await?;
        }
        Ok(true)
    }

    pub async fn forget(&self, input: ForgetInput) -> Result<bool, AuthError> {
        validate_forget(&input.email, &input.password, &input.email_code)?;
        let email = input.email.trim();
        match self
            .cache
            .consume_email_code(
                &normalize_email(email),
                &input.email_code,
                EmailCodeScope::PasswordReset,
                3,
                300,
            )
            .await?
        {
            LimitedEmailCodeResult::Consumed => {}
            LimitedEmailCodeResult::Incorrect => {
                return Err(AuthError::business(AuthCode::InvalidEmailCode));
            }
            LimitedEmailCodeResult::Limited => {
                return Err(AuthError::business(AuthCode::PasswordResetFailed));
            }
        }
        let account = self
            .repository
            .find_account_by_email(email)
            .await?
            .ok_or_else(|| AuthError::business(AuthCode::EmailNotRegistered))?;
        let password_hash = self.external.hash_password(&input.password).await?;
        if !self
            .repository
            .update_password(account.id, &password_hash, self.external.now())
            .await?
        {
            return Err(AuthError::business(AuthCode::PasswordResetFailed));
        }
        let _ = self.cache.remove_all_sessions(account.id).await;
        Ok(true)
    }

    pub async fn change_password(
        &self,
        user_id: i64,
        old_password: &str,
        new_password: &str,
    ) -> Result<(), AuthError> {
        validate_change_password(old_password, new_password)?;
        let account = self
            .repository
            .find_account_by_id(user_id)
            .await?
            .ok_or_else(|| AuthError::business(AuthCode::UserNotRegistered))?;
        if !self
            .external
            .verify_password(
                account.password_algo.as_deref(),
                account.password_salt.as_deref(),
                old_password,
                &account.password_hash,
            )
            .await?
        {
            return Err(AuthError::business(AuthCode::OldPasswordIncorrect));
        }
        let password_hash = self.external.hash_password(new_password).await?;
        if !self
            .repository
            .change_password_if_current(
                user_id,
                &account.password_hash,
                account.session_epoch,
                &password_hash,
                self.external.now(),
            )
            .await?
        {
            return Err(AuthError::business_detail(
                AuthCode::InvalidParameter,
                "Save failed",
            ));
        }
        let _ = self.cache.remove_all_sessions(user_id).await;
        Ok(())
    }

    pub async fn reset_security(&self, user_id: i64) -> Result<String, AuthError> {
        let uuid = self.external.uuid()?;
        let token = self.external.compact_id()?;
        if !self
            .repository
            .update_security(user_id, &uuid, &token, self.external.now())
            .await?
        {
            return Err(AuthError::business(AuthCode::PasswordResetFailed));
        }
        Ok(self.external.subscribe_url(user_id, &token).await?)
    }

    pub async fn send_email_verify(
        &self,
        input: EmailVerifyInput,
        client_ip: Option<String>,
    ) -> Result<bool, AuthError> {
        validate_email(&input.email)?;
        let email = input.email.trim();
        let cache_email = normalize_email(email);
        if let Some(client_ip) = client_ip.as_deref()
            && !self
                .cache
                .increment_email_send_limit(client_ip, 3, 60)
                .await?
        {
            return Err(AuthError::business_detail(
                AuthCode::EmailSendRateLimited,
                "Too many requests, please try again later.",
            ));
        }
        self.verify_recaptcha(input.recaptcha_data.as_deref())
            .await?;
        self.validate_register_email(email)?;
        let exists = self
            .repository
            .find_account_by_email(email)
            .await?
            .is_some();
        match input.is_forget {
            Some(false) if exists => {
                return Err(AuthError::business_detail(
                    AuthCode::EmailAlreadyRegistered,
                    "This email is registered",
                ));
            }
            Some(true) if !exists => {
                return Err(AuthError::business(AuthCode::EmailNotRegistered));
            }
            _ => {}
        }
        let code = self.external.email_code()?;
        if !self
            .cache
            .reserve_email_code(&cache_email, &code, self.external.now())
            .await?
        {
            return Err(AuthError::business_detail(
                AuthCode::EmailSendRateLimited,
                "Email verification code has been sent, please request again later",
            ));
        }
        if let Err(error) = self
            .external
            .send_verification_mail(
                email,
                &self.policy.app_name,
                self.policy.app_url.as_deref().unwrap_or_default(),
                &code,
            )
            .await
        {
            self.cache.release_email_code(&cache_email, &code).await;
            return Err(mail_delivery_error(error));
        }
        Ok(true)
    }

    async fn verify_recaptcha(&self, token: Option<&str>) -> Result<(), AuthError> {
        let failed = || AuthError::business(AuthCode::RecaptchaFailed);
        if token.is_some_and(|value| value.len() > MAX_RECAPTCHA_DATA_BYTES) {
            return Err(failed());
        }
        if !self.policy.recaptcha_enable {
            return Ok(());
        }
        let token = token
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .ok_or_else(failed)?;
        match self.external.verify_recaptcha(token).await {
            Ok(true) => Ok(()),
            Ok(false) | Err(_) => Err(failed()),
        }
    }

    fn validate_register_email(&self, email: &str) -> Result<(), AuthError> {
        if self.policy.email_whitelist_enable {
            let email = email.to_ascii_lowercase();
            let allowed = self.policy.email_whitelist_suffix.iter().any(|suffix| {
                let suffix = suffix.trim().trim_start_matches('@').to_ascii_lowercase();
                !suffix.is_empty() && email.ends_with(&format!("@{suffix}"))
            });
            if !allowed {
                return Err(AuthError::business(AuthCode::EmailSuffixNotAllowed));
            }
        }
        if self.policy.email_gmail_limit_enable
            && let Some(prefix) = email.split('@').next()
            && (prefix.contains('.') || prefix.contains('+'))
        {
            return Err(AuthError::business(AuthCode::GmailAliasNotSupported));
        }
        Ok(())
    }

    pub async fn admin_mfa_status(&self, user_id: i64) -> Result<MfaStatus, AuthError> {
        let enabled_at = self
            .repository
            .find_mfa(user_id)
            .await?
            .and_then(|record| record.enabled_at);
        Ok(MfaStatus {
            totp_enabled: enabled_at.is_some(),
            totp_enabled_at: enabled_at,
            totp_required: false,
        })
    }

    pub async fn admin_mfa_totp_setup(
        &self,
        user_id: i64,
        email: &str,
    ) -> Result<TotpProvisioning, AuthError> {
        let secret = self
            .external
            .create_mfa_secret(user_id, email, &self.policy.app_name)?;
        if !self
            .repository
            .upsert_pending_mfa(user_id, &secret, self.external.now())
            .await?
        {
            return Err(AuthError::business(AuthCode::MfaAlreadyEnabled));
        }
        Ok(TotpProvisioning {
            secret: secret.public_secret,
            otpauth_url: secret.otpauth_url,
        })
    }

    pub async fn admin_mfa_totp_confirm(&self, user_id: i64, code: &str) -> Result<(), AuthError> {
        let record = self
            .repository
            .find_mfa(user_id)
            .await?
            .ok_or_else(|| AuthError::business(AuthCode::MfaSetupMissing))?;
        if record.enabled_at.is_some() {
            return Err(AuthError::business(AuthCode::MfaAlreadyEnabled));
        }
        let now = self.external.now();
        let step = self
            .external
            .accepted_mfa_step(user_id, &record, code, now)?
            .ok_or_else(|| AuthError::business(AuthCode::MfaCodeInvalid))?;
        if !self.repository.enable_mfa(user_id, step, now).await? {
            return Err(AuthError::business(AuthCode::MfaAlreadyEnabled));
        }
        Ok(())
    }

    pub async fn admin_mfa_totp_disable(&self, user_id: i64, code: &str) -> Result<(), AuthError> {
        let record = self
            .repository
            .find_mfa(user_id)
            .await?
            .filter(|record| record.enabled_at.is_some())
            .ok_or_else(|| AuthError::business(AuthCode::MfaNotEnabled))?;
        let step = self
            .external
            .accepted_mfa_step(user_id, &record, code, self.external.now())?
            .ok_or_else(|| AuthError::business(AuthCode::MfaCodeInvalid))?;
        if !self.repository.disable_mfa(user_id, step).await? {
            return Err(AuthError::business(AuthCode::MfaCodeInvalid));
        }
        Ok(())
    }
}

fn mail_delivery_error(error: MailDeliveryError) -> AuthError {
    match error {
        MailDeliveryError::SenderNotConfigured {
            detail: Some(detail),
        } => AuthError::business_detail(AuthCode::MailSenderNotConfigured, detail),
        MailDeliveryError::SenderNotConfigured { detail: None } => {
            AuthError::business(AuthCode::MailSenderNotConfigured)
        }
        MailDeliveryError::InvalidSender => {
            AuthError::business_detail(AuthCode::MailInvalid, "Invalid email sender")
        }
        MailDeliveryError::InvalidRecipient => {
            AuthError::business_detail(AuthCode::MailInvalid, "Invalid recipient email")
        }
        MailDeliveryError::BuildFailed(error) => AuthError::business_detail(
            AuthCode::MailSendFailed,
            format!("Build mail failed: {error}"),
        ),
        MailDeliveryError::TimedOut => {
            AuthError::business_detail(AuthCode::MailSendFailed, "Send mail timed out")
        }
        MailDeliveryError::SendFailed(error) => AuthError::business_detail(
            AuthCode::MailSendFailed,
            format!("Send mail failed: {error}"),
        ),
        MailDeliveryError::Infrastructure(error) => AuthError::Repository(error),
    }
}

impl<R, C, E> AuthService<R, C, E>
where
    R: AuthRepository,
    C: AuthCache,
    E: AuthExternal,
{
    pub async fn login(
        &self,
        email: &str,
        password: &str,
        totp_code: Option<&str>,
        client_ip: Option<String>,
        user_agent: Option<String>,
    ) -> Result<AuthData, AuthError> {
        validate_email(email)?;
        validate_password(password)?;
        let email = normalize_email(email);

        let limiter_reserved = if self.policy.password_limit_enable {
            let account_limit = self.policy.password_limit_count.max(1);
            let reserved = self
                .cache
                .reserve_login_attempt(
                    &email,
                    client_ip.as_deref(),
                    account_limit,
                    account_limit.saturating_mul(10),
                    self.policy.password_limit_ttl_seconds,
                )
                .await?;
            if !reserved {
                return Err(AuthError::business_detail(
                    AuthCode::PasswordAttemptsRateLimited,
                    format!(
                        "There are too many password errors, please try again after {} minutes.",
                        self.policy.password_limit_expire_minutes
                    ),
                ));
            }
            true
        } else {
            false
        };

        let account = match self.repository.find_account_by_email(&email).await {
            Ok(account) => account,
            Err(error) => {
                self.release_login_attempt(limiter_reserved, &email, client_ip.as_deref())
                    .await;
                return Err(error.into());
            }
        };
        let Some(account) = account else {
            if let Err(error) = self.external.verify_dummy_password(password).await {
                self.release_login_attempt(limiter_reserved, &email, client_ip.as_deref())
                    .await;
                return Err(error.into());
            }
            return Err(AuthError::business(AuthCode::InvalidCredentials));
        };

        let password_matches = match self
            .external
            .verify_password(
                account.password_algo.as_deref(),
                account.password_salt.as_deref(),
                password,
                &account.password_hash,
            )
            .await
        {
            Ok(password_matches) => password_matches,
            Err(error) => {
                self.release_login_attempt(limiter_reserved, &email, client_ip.as_deref())
                    .await;
                return Err(error.into());
            }
        };
        if !password_matches {
            return Err(AuthError::business(AuthCode::InvalidCredentials));
        }

        if (account.is_admin != 0 || account.is_staff != 0)
            && let Err(error) = self.verify_login_totp(account.id, totp_code).await
        {
            if !error.is_code(AuthCode::MfaCodeInvalid) {
                self.release_login_attempt(limiter_reserved, &email, client_ip.as_deref())
                    .await;
            }
            return Err(error);
        }

        self.release_login_attempt(limiter_reserved, &email, client_ip.as_deref())
            .await;
        if account.banned {
            return Err(AuthError::business(AuthCode::AccountSuspended));
        }

        if self
            .external
            .password_needs_rehash(account.password_algo.as_deref(), &account.password_hash)
        {
            let upgraded = self.external.hash_password(password).await?;
            self.repository
                .rehash_password(
                    account.id,
                    &account.password_hash,
                    &upgraded,
                    self.external.now(),
                )
                .await?;
        }

        self.auth_data_for_user(
            account.id,
            Some(account.session_epoch),
            client_ip,
            user_agent,
            true,
        )
        .await
    }

    async fn release_login_attempt(&self, reserved: bool, email: &str, client_ip: Option<&str>) {
        if reserved {
            self.cache.release_login_attempt(email, client_ip).await;
        }
    }

    async fn verify_login_totp(
        &self,
        user_id: i64,
        totp_code: Option<&str>,
    ) -> Result<(), AuthError> {
        let Some(record) = self.repository.find_mfa(user_id).await? else {
            return Ok(());
        };
        if record.enabled_at.is_none() {
            return Ok(());
        }
        let code = totp_code.ok_or_else(|| AuthError::business(AuthCode::MfaCodeRequired))?;
        let now = self.external.now();
        let Some(step) = self
            .external
            .accepted_mfa_step(user_id, &record, code, now)?
        else {
            return Err(AuthError::business(AuthCode::MfaCodeInvalid));
        };
        if !self.repository.consume_mfa_step(user_id, step, now).await? {
            return Err(AuthError::business(AuthCode::MfaCodeInvalid));
        }
        Ok(())
    }

    pub async fn quick_login_url(
        &self,
        user_id: i64,
        redirect: Option<&str>,
    ) -> Result<String, AuthError> {
        let token = self.external.compact_id()?;
        let session_epoch = self
            .repository
            .active_session_epoch(user_id)
            .await?
            .ok_or(AuthError::Unauthorized)?;
        self.cache
            .put_temporary_token(&token, user_id, session_epoch, 60)
            .await?;
        Ok(self.login_redirect_url(&token, redirect))
    }

    pub async fn token_login(
        &self,
        verify: &str,
        client_ip: Option<String>,
        user_agent: Option<String>,
    ) -> Result<AuthData, AuthError> {
        let identity = self
            .cache
            .take_temporary_token(verify)
            .await?
            .ok_or_else(|| AuthError::business(AuthCode::InvalidToken))?;
        self.auth_data_for_user(
            identity.user_id,
            Some(identity.session_epoch),
            client_ip,
            user_agent,
            false,
        )
        .await
    }

    async fn auth_data_for_user(
        &self,
        user_id: i64,
        expected_session_epoch: Option<i64>,
        client_ip: Option<String>,
        user_agent: Option<String>,
        password_authenticated: bool,
    ) -> Result<AuthData, AuthError> {
        let account = self
            .repository
            .find_account_by_id(user_id)
            .await?
            .ok_or_else(|| AuthError::business(AuthCode::UserNotRegistered))?;
        if account.banned {
            return Err(AuthError::business(AuthCode::AccountSuspended));
        }
        if expected_session_epoch.is_some_and(|expected| expected != account.session_epoch) {
            return Err(AuthError::Unauthorized);
        }

        let now = self.external.now();
        let session_id = self.external.compact_id()?;
        let ttl_seconds = session_ttl_seconds(
            self.policy.auth_session_ttl_seconds,
            self.policy.privileged_auth_session_ttl_seconds,
            account.is_admin,
            account.is_staff,
        );
        let metadata = SessionMetadata {
            ip: client_ip.map(|value| truncate_utf8(value, 64)),
            login_at: now,
            user_agent: user_agent.map(|value| truncate_utf8(value, 512)),
            expires_at: Some(now.saturating_add(ttl_seconds as i64)),
            password_authenticated,
        };
        let identity = SessionIdentity {
            user_id: account.id,
            session_id,
            session_epoch: account.session_epoch,
        };
        let mut bearer = None;
        for _ in 0..3 {
            let candidate = self.external.opaque_token()?;
            if self
                .cache
                .add_session(
                    &identity,
                    &metadata,
                    &candidate,
                    ttl_seconds,
                    self.policy.auth_session_max_per_user,
                    now,
                )
                .await?
            {
                bearer = Some(candidate);
                break;
            }
        }
        Ok(AuthData {
            is_admin: account.is_admin != 0,
            auth_data: bearer.ok_or_else(|| {
                AuthError::Internal("could not allocate a unique session token".to_string())
            })?,
        })
    }

    pub async fn user_from_auth_data(&self, bearer: &str) -> Result<AuthUser, AuthError> {
        if bearer.is_empty() || bearer.len() > 4_096 {
            return Err(AuthError::Unauthorized);
        }
        let identity = self
            .cache
            .session_identity(bearer)
            .await?
            .ok_or(AuthError::Unauthorized)?;
        let account = self
            .repository
            .find_account_by_id(identity.user_id)
            .await?
            .ok_or(AuthError::Unauthorized)?;
        if account.banned || account.session_epoch != identity.session_epoch {
            return Err(AuthError::Unauthorized);
        }
        let metadata = self
            .cache
            .session_metadata(identity.user_id, &identity.session_id)
            .await?
            .filter(|metadata| {
                !metadata
                    .expires_at
                    .is_some_and(|expires_at| expires_at <= self.external.now())
            })
            .ok_or(AuthError::Unauthorized)?;
        Ok(AuthUser {
            id: account.id,
            email: account.email,
            is_admin: account.is_admin,
            is_staff: account.is_staff,
            admin_permissions: account.admin_permissions,
            session_id: identity.session_id,
            authenticated_at: metadata.login_at,
            password_authenticated: metadata.password_authenticated,
        })
    }

    pub async fn sessions(
        &self,
        user_id: i64,
        current_session_id: Option<&str>,
    ) -> Result<Vec<UserSession>, AuthError> {
        let now = self.external.now();
        let mut visible = self
            .cache
            .sessions(user_id)
            .await?
            .into_iter()
            .filter(|session| {
                !session
                    .metadata
                    .expires_at
                    .is_some_and(|expires_at| expires_at <= now)
            })
            .map(|session| UserSession {
                current: current_session_id == Some(session.session_id.as_str()),
                ip: session.metadata.ip.unwrap_or_default(),
                login_at: session.metadata.login_at,
                ua: session.metadata.user_agent.unwrap_or_default(),
                session_id: session.session_id,
            })
            .collect::<Vec<_>>();
        visible.sort_by(|left, right| {
            right
                .login_at
                .cmp(&left.login_at)
                .then_with(|| left.session_id.cmp(&right.session_id))
        });
        Ok(visible)
    }

    pub async fn logout(&self, bearer: &str) -> Result<bool, AuthError> {
        if bearer.is_empty() || bearer.len() > 4_096 {
            return Ok(false);
        }
        let Some(identity) = self.cache.session_identity(bearer).await? else {
            return Ok(false);
        };
        self.remove_session(identity.user_id, &identity.session_id)
            .await?;
        Ok(true)
    }

    pub async fn remove_session(&self, user_id: i64, session_id: &str) -> Result<bool, AuthError> {
        self.cache.remove_session(user_id, session_id).await?;
        Ok(true)
    }

    pub async fn remove_all_sessions(&self, user_id: i64) -> Result<bool, AuthError> {
        self.cache.remove_all_sessions(user_id).await?;
        Ok(true)
    }

    pub async fn create_privileged_step_up(
        &self,
        user_id: i64,
        session_id: &str,
        password: &str,
        client_ip: Option<&str>,
    ) -> Result<String, AuthError> {
        validate_password(password)?;
        let user_limit = self.policy.privileged_step_up_max_attempts;
        if !self
            .cache
            .reserve_step_up_attempt(
                user_id,
                client_ip,
                user_limit,
                user_limit.saturating_mul(5),
                self.policy.privileged_step_up_attempt_window_seconds,
            )
            .await?
        {
            return Err(AuthError::business_detail(
                AuthCode::PasswordAttemptsRateLimited,
                "Too many password verification attempts; try again later",
            ));
        }
        let account = self
            .repository
            .find_account_by_id(user_id)
            .await?
            .ok_or(AuthError::Unauthorized)?;
        if account.banned {
            return Err(AuthError::Unauthorized);
        }
        if !self
            .external
            .verify_password(
                account.password_algo.as_deref(),
                account.password_salt.as_deref(),
                password,
                &account.password_hash,
            )
            .await?
        {
            return Err(AuthError::business(AuthCode::InvalidCredentials));
        }
        if self
            .cache
            .session_metadata(user_id, session_id)
            .await?
            .filter(|metadata| {
                !metadata
                    .expires_at
                    .is_some_and(|expires_at| expires_at <= self.external.now())
            })
            .is_none()
        {
            return Err(AuthError::Unauthorized);
        }
        self.cache.clear_step_up_attempts(user_id, client_ip).await;
        for _ in 0..3 {
            let token = self.external.opaque_token()?;
            if self
                .cache
                .put_step_up(
                    &token,
                    user_id,
                    session_id,
                    self.policy.privileged_step_up_ttl_seconds,
                )
                .await?
            {
                return Ok(token);
            }
        }
        Err(AuthError::Internal(
            "could not allocate a step-up token".to_string(),
        ))
    }

    pub async fn verify_privileged_step_up(
        &self,
        user_id: i64,
        session_id: &str,
        token: &str,
    ) -> Result<bool, AuthError> {
        if token.is_empty() || token.len() > 256 {
            return Ok(false);
        }
        Ok(self.cache.step_up_identity(token).await?.is_some_and(
            |(bound_user_id, bound_session_id)| {
                bound_user_id == user_id && bound_session_id == session_id
            },
        ))
    }
}
