//! Administrative user-management use cases and outbound ports.
//!
//! The module owns validation, scoping, bounded bulk policies, projection
//! shaping, and post-commit session-revocation policy. PostgreSQL, Redis,
//! password hashing, subscribe-link minting, CSV encoding, and transport DTOs
//! are implemented by outer adapters.

use std::collections::{BTreeMap, BTreeSet};

use v2board_domain_model::is_registered_permission;

use crate::{
    RepositoryError,
    filter_dsl::{self, FilterField},
};

/// Back-compat aliases: `admin_user` was the first consumer of the admin
/// filter DSL, so its per-resource names stay put while the operator/value
/// vocabulary and validity check move to the shared, table-driven engine in
/// `crate::filter_dsl` (docs/api-dialect.md §7.1).
pub use crate::filter_dsl::{
    ColumnKind as UserColumnKind, FilterOperator as UserFilterOperator,
    FilterValue as UserFilterValue,
};
pub type UserFilterClause = filter_dsl::FilterClause<UserFilterField>;

const USER_BULK_MAX_ROWS: usize = 10_000;
const USER_CSV_PAGE_SIZE: i64 = 500;
const USER_CSV_MAX_ROWS: usize = 50_000;
const GENERATED_USER_MAX_ROWS: usize = 500;
const GENERATED_EMAIL_ATTEMPT_FACTOR: usize = 32;
const GIB: i64 = 1_073_741_824;

pub type RepositoryResult<T> = Result<T, RepositoryError>;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AdminUserCode {
    EmailAlreadyRegistered,
    InvalidParameter,
    PlanNotFound,
    PlanUnavailable,
    UserNotFound,
}

#[derive(Debug, thiserror::Error)]
pub enum AdminUserError {
    #[error("validation failed for {field}: {message}")]
    Validation { field: String, message: String },
    #[error("admin user business error: {code:?}")]
    Business {
        code: AdminUserCode,
        detail: Option<String>,
    },
    #[error("admin user external adapter failed: {0}")]
    External(String),
    #[error(transparent)]
    Repository(#[from] RepositoryError),
}

impl AdminUserError {
    pub fn validation(field: impl Into<String>, message: impl Into<String>) -> Self {
        Self::Validation {
            field: field.into(),
            message: message.into(),
        }
    }

    pub const fn business(code: AdminUserCode) -> Self {
        Self::Business { code, detail: None }
    }

    pub fn business_detail(code: AdminUserCode, detail: impl Into<String>) -> Self {
        Self::Business {
            code,
            detail: Some(detail.into()),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AdminUser {
    pub id: i64,
    pub email: String,
    pub balance: i32,
    pub commission_balance: i32,
    pub transfer_enable: i64,
    pub device_limit: Option<i32>,
    pub uploaded: i64,
    pub downloaded: i64,
    pub plan_id: Option<i32>,
    pub plan_name: Option<String>,
    pub group_id: Option<i32>,
    pub expired_at: Option<i64>,
    pub uuid: String,
    pub token: String,
    pub banned: bool,
    pub is_admin: bool,
    pub is_staff: bool,
    pub admin_permissions: Vec<String>,
    pub invite_user_id: Option<i64>,
    pub discount: Option<i32>,
    pub commission_type: i16,
    pub commission_rate: Option<i32>,
    pub speed_limit: Option<i32>,
    pub auto_renewal: Option<bool>,
    pub remind_expire: Option<bool>,
    pub remind_traffic: Option<bool>,
    pub remarks: Option<String>,
    pub telegram_id: Option<i64>,
    pub last_login_at: Option<i64>,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AdminUserListItem {
    pub user: AdminUser,
    pub total_used: u64,
    pub alive_ip: i64,
    pub ips: String,
    pub subscribe_url: String,
}

impl AdminUserListItem {
    fn from_user(user: AdminUser) -> Self {
        let total_used = u64::try_from(user.uploaded)
            .unwrap_or_default()
            .saturating_add(u64::try_from(user.downloaded).unwrap_or_default());
        Self {
            user,
            total_used,
            alive_ip: 0,
            ips: String::new(),
            subscribe_url: String::new(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AdminInviter {
    pub id: i64,
    pub email: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AdminUserDetailRecord {
    pub user: AdminUser,
    pub inviter: Option<AdminInviter>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AdminUserDetail {
    pub user: AdminUserListItem,
    pub inviter: Option<AdminInviter>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StaffUserDetail {
    pub user: AdminUserListItem,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AdminUserPage {
    pub items: Vec<AdminUserListItem>,
    pub total: i64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UserFilterField {
    Id,
    Email,
    TelegramId,
    Balance,
    Discount,
    CommissionType,
    CommissionRate,
    CommissionBalance,
    LastTrafficResetAt,
    Uploaded,
    Downloaded,
    TransferEnable,
    DeviceLimit,
    Banned,
    IsAdmin,
    IsStaff,
    LastLoginAt,
    Uuid,
    GroupId,
    PlanId,
    SpeedLimit,
    Token,
    ExpiredAt,
    Remarks,
    InviteUserId,
    CreatedAt,
    UpdatedAt,
}

impl UserFilterField {
    #[must_use]
    pub const fn kind(self) -> UserColumnKind {
        match self {
            Self::Email => UserColumnKind::Email,
            Self::Banned | Self::IsAdmin | Self::IsStaff => UserColumnKind::Boolean,
            Self::Uuid | Self::Token | Self::Remarks => UserColumnKind::Text,
            Self::LastLoginAt | Self::ExpiredAt | Self::CreatedAt | Self::UpdatedAt => {
                UserColumnKind::Timestamp
            }
            _ => UserColumnKind::Integer,
        }
    }

    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        Some(match value {
            "id" => Self::Id,
            "email" => Self::Email,
            "telegram_id" => Self::TelegramId,
            "balance" => Self::Balance,
            "discount" => Self::Discount,
            "commission_type" => Self::CommissionType,
            "commission_rate" => Self::CommissionRate,
            "commission_balance" => Self::CommissionBalance,
            "t" => Self::LastTrafficResetAt,
            "u" => Self::Uploaded,
            "d" => Self::Downloaded,
            "transfer_enable" => Self::TransferEnable,
            "device_limit" => Self::DeviceLimit,
            "banned" => Self::Banned,
            "is_admin" => Self::IsAdmin,
            "is_staff" => Self::IsStaff,
            "last_login_at" => Self::LastLoginAt,
            "uuid" => Self::Uuid,
            "group_id" => Self::GroupId,
            "plan_id" => Self::PlanId,
            "speed_limit" => Self::SpeedLimit,
            "token" => Self::Token,
            "expired_at" => Self::ExpiredAt,
            "remarks" => Self::Remarks,
            "invite_user_id" => Self::InviteUserId,
            "created_at" => Self::CreatedAt,
            "updated_at" => Self::UpdatedAt,
            _ => return None,
        })
    }

    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Id => "id",
            Self::Email => "email",
            Self::TelegramId => "telegram_id",
            Self::Balance => "balance",
            Self::Discount => "discount",
            Self::CommissionType => "commission_type",
            Self::CommissionRate => "commission_rate",
            Self::CommissionBalance => "commission_balance",
            Self::LastTrafficResetAt => "t",
            Self::Uploaded => "u",
            Self::Downloaded => "d",
            Self::TransferEnable => "transfer_enable",
            Self::DeviceLimit => "device_limit",
            Self::Banned => "banned",
            Self::IsAdmin => "is_admin",
            Self::IsStaff => "is_staff",
            Self::LastLoginAt => "last_login_at",
            Self::Uuid => "uuid",
            Self::GroupId => "group_id",
            Self::PlanId => "plan_id",
            Self::SpeedLimit => "speed_limit",
            Self::Token => "token",
            Self::ExpiredAt => "expired_at",
            Self::Remarks => "remarks",
            Self::InviteUserId => "invite_user_id",
            Self::CreatedAt => "created_at",
            Self::UpdatedAt => "updated_at",
        }
    }
}

impl FilterField for UserFilterField {
    fn parse(name: &str) -> Option<Self> {
        Self::parse(name)
    }

    fn name(self) -> &'static str {
        self.name()
    }

    fn kind(self) -> UserColumnKind {
        self.kind()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UserSortField {
    Field(UserFilterField),
    TotalUsed,
}

impl UserSortField {
    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        if value == "total_used" {
            Some(Self::TotalUsed)
        } else {
            UserFilterField::parse(value).map(Self::Field)
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct UserSort {
    pub field: UserSortField,
    pub descending: bool,
}

impl Default for UserSort {
    fn default() -> Self {
        Self {
            field: UserSortField::Field(UserFilterField::CreatedAt),
            descending: true,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AdminUserListRequest {
    pub limit: i64,
    pub offset: i64,
    pub filters: Vec<UserFilterClause>,
    pub sort: UserSort,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RepositoryUserPage {
    pub items: Vec<AdminUser>,
    pub total: i64,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct AdminUserPatchInput {
    pub email: Option<String>,
    pub password: Option<String>,
    pub transfer_enable: Option<i64>,
    pub uploaded: Option<i64>,
    pub downloaded: Option<i64>,
    pub balance: Option<i64>,
    pub commission_balance: Option<i64>,
    pub commission_type: Option<i64>,
    pub banned: Option<bool>,
    pub is_admin: Option<bool>,
    pub is_staff: Option<bool>,
    pub admin_permissions: Option<Vec<String>>,
    pub device_limit: Option<Option<i64>>,
    pub commission_rate: Option<Option<i64>>,
    pub discount: Option<Option<i64>>,
    pub speed_limit: Option<Option<i64>>,
    pub plan_id: Option<Option<i64>>,
    pub remarks: Option<Option<String>>,
    pub expired_at: Option<Option<i64>>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct StaffUserPatchInput {
    pub email: Option<String>,
    pub password: Option<String>,
    pub transfer_enable: Option<i64>,
    pub uploaded: Option<i64>,
    pub downloaded: Option<i64>,
    pub balance: Option<i64>,
    pub commission_balance: Option<i64>,
    pub banned: Option<bool>,
    pub device_limit: Option<Option<i64>>,
    pub commission_rate: Option<Option<i64>>,
    pub discount: Option<Option<i64>>,
    pub plan_id: Option<Option<i64>>,
    pub expired_at: Option<Option<i64>>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct AdminUserChanges {
    pub email: Option<String>,
    pub password_hash: Option<String>,
    pub transfer_enable: Option<i64>,
    pub uploaded: Option<i64>,
    pub downloaded: Option<i64>,
    pub balance: Option<i32>,
    pub commission_balance: Option<i32>,
    pub commission_type: Option<i16>,
    pub banned: Option<bool>,
    pub is_admin: Option<bool>,
    pub is_staff: Option<bool>,
    pub admin_permissions: Option<Vec<String>>,
    pub device_limit: Option<Option<i32>>,
    pub commission_rate: Option<Option<i32>>,
    pub discount: Option<Option<i32>>,
    pub speed_limit: Option<Option<i32>>,
    pub plan_id: Option<Option<i32>>,
    pub remarks: Option<Option<String>>,
    pub expired_at: Option<Option<i64>>,
    pub revoke_sessions: bool,
    pub reset_traffic_epoch: bool,
    pub updated_at: i64,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct StaffUserChanges {
    pub email: Option<String>,
    pub password_hash: Option<String>,
    pub transfer_enable: Option<i64>,
    pub uploaded: Option<i64>,
    pub downloaded: Option<i64>,
    pub balance: Option<i32>,
    pub commission_balance: Option<i32>,
    pub banned: Option<bool>,
    pub device_limit: Option<Option<i32>>,
    pub commission_rate: Option<Option<i32>>,
    pub discount: Option<Option<i32>>,
    pub plan_id: Option<Option<i32>>,
    pub expired_at: Option<Option<i64>>,
    pub revoke_sessions: bool,
    pub reset_traffic_epoch: bool,
    pub updated_at: i64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UserUpdateOutcome {
    Updated,
    UserNotFound,
    EmailAlreadyRegistered,
    PlanNotFound,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UserGenerateInput {
    pub email_prefix: Option<String>,
    pub email_suffix: String,
    pub password: Option<String>,
    pub plan_id: Option<i64>,
    pub expired_at: Option<i64>,
    pub generate_count: Option<i64>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AccountCredential {
    pub email: String,
    pub password: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PreparedAccount {
    pub email: String,
    pub password: String,
    pub password_hash: String,
    pub uuid: String,
    pub token: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CreateUsersCommand {
    pub accounts: Vec<PreparedAccount>,
    pub plan_id: Option<i32>,
    pub expired_at: Option<i64>,
    pub created_at: i64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CreatedAccount {
    pub id: i64,
    pub token: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CreateUsersOutcome {
    Created(Vec<CreatedAccount>),
    EmailAlreadyRegistered,
    PlanUnavailable,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum UserGenerateOutcome {
    Created { id: i64 },
    Csv { filename: String, body: String },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UserSecret {
    pub token: String,
    pub uuid: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SetInviterOutcome {
    Updated,
    UserNotFound,
    InviterNotFound,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum BanUsersOutcome {
    Banned(Vec<i64>),
    TooMany,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DeleteUsersOutcome {
    Deleted(Vec<i64>),
    UserNotFound,
    PendingStripeOrder,
    TooMany,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UserExportRow {
    pub id: i64,
    pub email: String,
    pub balance: i32,
    pub commission_balance: i32,
    pub transfer_enable: i64,
    pub uploaded: i64,
    pub downloaded: i64,
    pub device_limit: Option<i32>,
    pub expired_at: Option<i64>,
    pub plan_name: Option<String>,
    pub token: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UserExportPage {
    pub items: Vec<UserExportRow>,
}

#[allow(async_fn_in_trait)]
pub trait AdminUserRepository: Send + Sync {
    async fn list(&self, request: &AdminUserListRequest) -> RepositoryResult<RepositoryUserPage>;
    async fn detail(
        &self,
        user_id: i64,
        staff_scoped: bool,
    ) -> RepositoryResult<Option<AdminUserDetailRecord>>;
    async fn update_admin(
        &self,
        user_id: i64,
        changes: AdminUserChanges,
    ) -> RepositoryResult<UserUpdateOutcome>;
    async fn update_staff(
        &self,
        user_id: i64,
        changes: StaffUserChanges,
    ) -> RepositoryResult<UserUpdateOutcome>;
    async fn create_users(
        &self,
        command: CreateUsersCommand,
    ) -> RepositoryResult<CreateUsersOutcome>;
    async fn reset_secret(
        &self,
        user_id: i64,
        secret: UserSecret,
        updated_at: i64,
    ) -> RepositoryResult<bool>;
    async fn set_inviter(
        &self,
        user_id: i64,
        inviter_email: Option<&str>,
        updated_at: i64,
    ) -> RepositoryResult<SetInviterOutcome>;
    async fn export_page(
        &self,
        filters: &[UserFilterClause],
        after_id: i64,
        limit: i64,
    ) -> RepositoryResult<UserExportPage>;
    async fn ban_users(
        &self,
        filters: &[UserFilterClause],
        staff_scoped: bool,
        maximum: usize,
        updated_at: i64,
    ) -> RepositoryResult<BanUsersOutcome>;
    async fn delete_users(
        &self,
        filters: &[UserFilterClause],
        maximum: usize,
    ) -> RepositoryResult<DeleteUsersOutcome>;
    async fn delete_user(&self, user_id: i64) -> RepositoryResult<DeleteUsersOutcome>;
}

#[derive(Debug, Clone, Eq, PartialEq, thiserror::Error)]
#[error("{0}")]
pub struct AdminUserExternalError(String);

impl AdminUserExternalError {
    pub fn new(error: impl std::fmt::Display) -> Self {
        Self(error.to_string())
    }
}

#[allow(async_fn_in_trait)]
pub trait AdminUserExternal: Send + Sync {
    type CsvWriter: Send;

    async fn hash_password(&self, password: &str) -> Result<String, AdminUserExternalError>;
    async fn prepare_accounts(
        &self,
        credentials: Vec<AccountCredential>,
    ) -> Result<Vec<PreparedAccount>, AdminUserExternalError>;
    async fn enrich_users(
        &self,
        users: &mut [AdminUserListItem],
    ) -> Result<(), AdminUserExternalError>;
    async fn subscribe_url(
        &self,
        user_id: i64,
        token: &str,
    ) -> Result<String, AdminUserExternalError>;
    async fn remove_sessions(&self, user_ids: &[i64]);
    fn random_email(&self, suffix: &str) -> String;
    fn new_secret(&self) -> UserSecret;
    fn local_datetime(&self, epoch_seconds: i64) -> String;
    fn start_csv(
        &self,
        headers: &[&str],
        include_utf8_bom: bool,
    ) -> Result<Self::CsvWriter, AdminUserExternalError>;
    fn write_csv(
        &self,
        writer: &mut Self::CsvWriter,
        row: Vec<String>,
    ) -> Result<(), AdminUserExternalError>;
    fn finish_csv(&self, writer: Self::CsvWriter) -> Result<String, AdminUserExternalError>;
}

#[derive(Clone, Debug)]
pub struct AdminUserService<R, E> {
    repository: R,
    external: E,
}

impl<R, E> AdminUserService<R, E>
where
    R: AdminUserRepository,
    E: AdminUserExternal,
{
    pub const fn new(repository: R, external: E) -> Self {
        Self {
            repository,
            external,
        }
    }

    pub async fn users(
        &self,
        request: AdminUserListRequest,
    ) -> Result<AdminUserPage, AdminUserError> {
        validate_list_request(&request)?;
        let page = self.repository.list(&request).await?;
        let mut items = page
            .items
            .into_iter()
            .map(AdminUserListItem::from_user)
            .collect::<Vec<_>>();
        self.external
            .enrich_users(&mut items)
            .await
            .map_err(external_error)?;
        Ok(AdminUserPage {
            items,
            total: page.total,
        })
    }

    pub async fn user_detail(&self, user_id: i64) -> Result<AdminUserDetail, AdminUserError> {
        let record = self
            .repository
            .detail(user_id, false)
            .await?
            .ok_or_else(|| AdminUserError::business(AdminUserCode::UserNotFound))?;
        let mut users = vec![AdminUserListItem::from_user(record.user)];
        self.external
            .enrich_users(&mut users)
            .await
            .map_err(external_error)?;
        Ok(AdminUserDetail {
            user: users.pop().expect("one user detail was prepared"),
            inviter: record.inviter,
        })
    }

    pub async fn staff_user_detail(&self, user_id: i64) -> Result<StaffUserDetail, AdminUserError> {
        let record = self
            .repository
            .detail(user_id, true)
            .await?
            .ok_or_else(|| AdminUserError::business(AdminUserCode::UserNotFound))?;
        let mut users = vec![AdminUserListItem::from_user(record.user)];
        self.external
            .enrich_users(&mut users)
            .await
            .map_err(external_error)?;
        Ok(StaffUserDetail {
            user: users.pop().expect("one staff user detail was prepared"),
        })
    }

    pub async fn update_user(
        &self,
        user_id: i64,
        input: AdminUserPatchInput,
        now: i64,
    ) -> Result<(), AdminUserError> {
        validate_admin_patch(&input)?;
        let password_changed = input
            .password
            .as_deref()
            .is_some_and(|password| !password.is_empty());
        let password_hash = self
            .hash_changed_password(input.password.as_deref())
            .await?;
        let permissions = input.admin_permissions.map(deduplicate_permissions);
        let changes = AdminUserChanges {
            email: input.email,
            password_hash,
            transfer_enable: input.transfer_enable,
            uploaded: input.uploaded,
            downloaded: input.downloaded,
            balance: narrow_optional("balance", input.balance)?,
            commission_balance: narrow_optional("commission_balance", input.commission_balance)?,
            commission_type: narrow_optional("commission_type", input.commission_type)?,
            banned: input.banned,
            is_admin: input.is_admin,
            is_staff: input.is_staff,
            admin_permissions: permissions,
            device_limit: narrow_nullable("device_limit", input.device_limit)?,
            commission_rate: narrow_nullable("commission_rate", input.commission_rate)?,
            discount: narrow_nullable("discount", input.discount)?,
            speed_limit: narrow_nullable("speed_limit", input.speed_limit)?,
            plan_id: narrow_nullable_code(input.plan_id, AdminUserCode::PlanNotFound)?,
            remarks: input.remarks,
            expired_at: input.expired_at,
            revoke_sessions: password_changed
                || input.banned == Some(true)
                || input.is_admin.is_some()
                || input.is_staff.is_some(),
            reset_traffic_epoch: input.uploaded.is_some() || input.downloaded.is_some(),
            updated_at: now,
        };
        let revoke_sessions = changes.revoke_sessions;
        match self.repository.update_admin(user_id, changes).await? {
            UserUpdateOutcome::Updated => {}
            UserUpdateOutcome::UserNotFound => {
                return Err(AdminUserError::business(AdminUserCode::UserNotFound));
            }
            UserUpdateOutcome::EmailAlreadyRegistered => {
                return Err(AdminUserError::business(
                    AdminUserCode::EmailAlreadyRegistered,
                ));
            }
            UserUpdateOutcome::PlanNotFound => {
                return Err(AdminUserError::business(AdminUserCode::PlanNotFound));
            }
        }
        if revoke_sessions {
            self.external.remove_sessions(&[user_id]).await;
        }
        Ok(())
    }

    pub async fn update_staff_user(
        &self,
        user_id: i64,
        input: StaffUserPatchInput,
        now: i64,
    ) -> Result<(), AdminUserError> {
        let password_changed = input
            .password
            .as_deref()
            .is_some_and(|password| !password.is_empty());
        let password_hash = self
            .hash_changed_password(input.password.as_deref())
            .await?;
        let changes = StaffUserChanges {
            email: input.email,
            password_hash,
            transfer_enable: input.transfer_enable,
            uploaded: input.uploaded,
            downloaded: input.downloaded,
            balance: narrow_optional("balance", input.balance)?,
            commission_balance: narrow_optional("commission_balance", input.commission_balance)?,
            banned: input.banned,
            device_limit: narrow_nullable("device_limit", input.device_limit)?,
            commission_rate: narrow_nullable("commission_rate", input.commission_rate)?,
            discount: narrow_nullable("discount", input.discount)?,
            plan_id: narrow_nullable_code(input.plan_id, AdminUserCode::PlanNotFound)?,
            expired_at: input.expired_at,
            revoke_sessions: password_changed || input.banned == Some(true),
            reset_traffic_epoch: input.uploaded.is_some() || input.downloaded.is_some(),
            updated_at: now,
        };
        let revoke_sessions = changes.revoke_sessions;
        match self.repository.update_staff(user_id, changes).await? {
            UserUpdateOutcome::Updated => {}
            UserUpdateOutcome::UserNotFound => {
                return Err(AdminUserError::business(AdminUserCode::UserNotFound));
            }
            UserUpdateOutcome::EmailAlreadyRegistered => {
                return Err(AdminUserError::business(
                    AdminUserCode::EmailAlreadyRegistered,
                ));
            }
            UserUpdateOutcome::PlanNotFound => {
                return Err(AdminUserError::business(AdminUserCode::PlanNotFound));
            }
        }
        if revoke_sessions {
            self.external.remove_sessions(&[user_id]).await;
        }
        Ok(())
    }

    async fn hash_changed_password(
        &self,
        password: Option<&str>,
    ) -> Result<Option<String>, AdminUserError> {
        let Some(password) = password.filter(|password| !password.is_empty()) else {
            return Ok(None);
        };
        self.external
            .hash_password(password)
            .await
            .map(Some)
            .map_err(external_error)
    }

    pub async fn generate_users(
        &self,
        input: UserGenerateInput,
        now: i64,
    ) -> Result<UserGenerateOutcome, AdminUserError> {
        let suffix = input.email_suffix.trim();
        if suffix.is_empty() {
            return Err(AdminUserError::validation(
                "email_suffix",
                "邮箱后缀不能为空",
            ));
        }
        let plan_id = narrow_optional_code(input.plan_id, AdminUserCode::PlanUnavailable)?;
        if let Some(prefix) = input
            .email_prefix
            .as_deref()
            .map(str::trim)
            .filter(|prefix| !prefix.is_empty())
        {
            let email = format!("{prefix}@{suffix}");
            let password = input
                .password
                .as_deref()
                .filter(|password| !password.is_empty())
                .unwrap_or(&email)
                .to_string();
            let accounts = self
                .external
                .prepare_accounts(vec![AccountCredential { email, password }])
                .await
                .map_err(external_error)?;
            return match self
                .repository
                .create_users(CreateUsersCommand {
                    accounts,
                    plan_id,
                    expired_at: input.expired_at,
                    created_at: now,
                })
                .await?
            {
                CreateUsersOutcome::Created(accounts) if accounts.len() == 1 => {
                    Ok(UserGenerateOutcome::Created { id: accounts[0].id })
                }
                CreateUsersOutcome::Created(_) => Err(AdminUserError::External(
                    "repository returned an invalid single-create cardinality".into(),
                )),
                CreateUsersOutcome::EmailAlreadyRegistered => Err(AdminUserError::business(
                    AdminUserCode::EmailAlreadyRegistered,
                )),
                CreateUsersOutcome::PlanUnavailable => {
                    Err(AdminUserError::business(AdminUserCode::PlanUnavailable))
                }
            };
        }

        let count = usize::try_from(input.generate_count.unwrap_or_default())
            .ok()
            .filter(|count| *count > 0)
            .ok_or_else(|| AdminUserError::validation("generate_count", "生成数量必须为正整数"))?;
        if count > GENERATED_USER_MAX_ROWS {
            return Err(AdminUserError::validation(
                "generate_count",
                "生成数量最大为500个",
            ));
        }
        let mut emails = BTreeSet::new();
        for _ in 0..count.saturating_mul(GENERATED_EMAIL_ATTEMPT_FACTOR) {
            if emails.len() == count {
                break;
            }
            emails.insert(self.external.random_email(suffix));
        }
        if emails.len() != count {
            return Err(AdminUserError::External(
                "random email adapter could not produce a unique batch".into(),
            ));
        }
        let configured_password = input.password.filter(|password| !password.is_empty());
        let credentials = emails
            .into_iter()
            .map(|email| AccountCredential {
                password: configured_password.clone().unwrap_or_else(|| email.clone()),
                email,
            })
            .collect::<Vec<_>>();
        let prepared = self
            .external
            .prepare_accounts(credentials)
            .await
            .map_err(external_error)?;
        if prepared.len() != count {
            return Err(AdminUserError::External(
                "password adapter changed generated account cardinality".into(),
            ));
        }
        let created = match self
            .repository
            .create_users(CreateUsersCommand {
                accounts: prepared.clone(),
                plan_id,
                expired_at: input.expired_at,
                created_at: now,
            })
            .await?
        {
            CreateUsersOutcome::Created(created) => created,
            CreateUsersOutcome::EmailAlreadyRegistered => {
                return Err(AdminUserError::business(
                    AdminUserCode::EmailAlreadyRegistered,
                ));
            }
            CreateUsersOutcome::PlanUnavailable => {
                return Err(AdminUserError::business(AdminUserCode::PlanUnavailable));
            }
        };
        let ids = created
            .into_iter()
            .map(|created| (created.token, created.id))
            .collect::<BTreeMap<_, _>>();
        let create_date = self.external.local_datetime(now);
        let expire = input
            .expired_at
            .map(|expired_at| self.external.local_datetime(expired_at))
            .unwrap_or_else(|| "长期有效".to_string());
        let mut csv = self
            .external
            .start_csv(
                &["账号", "密码", "过期时间", "UUID", "创建时间", "订阅地址"],
                false,
            )
            .map_err(external_error)?;
        for account in prepared {
            let user_id = ids.get(&account.token).copied().ok_or_else(|| {
                AdminUserError::External("created user is missing its inserted id".into())
            })?;
            let subscribe_url = self
                .external
                .subscribe_url(user_id, &account.token)
                .await
                .map_err(external_error)?;
            self.external
                .write_csv(
                    &mut csv,
                    vec![
                        account.email,
                        account.password,
                        expire.clone(),
                        account.uuid,
                        create_date.clone(),
                        subscribe_url,
                    ],
                )
                .map_err(external_error)?;
        }
        Ok(UserGenerateOutcome::Csv {
            filename: "users.csv".to_string(),
            body: self.external.finish_csv(csv).map_err(external_error)?,
        })
    }

    pub async fn reset_secret(&self, user_id: i64, now: i64) -> Result<(), AdminUserError> {
        let secret = self.external.new_secret();
        let _updated = self.repository.reset_secret(user_id, secret, now).await?;
        Ok(())
    }

    pub async fn set_inviter(
        &self,
        user_id: i64,
        inviter_email: Option<String>,
        now: i64,
    ) -> Result<(), AdminUserError> {
        let inviter_email = inviter_email
            .as_deref()
            .map(str::trim)
            .filter(|email| !email.is_empty());
        match self
            .repository
            .set_inviter(user_id, inviter_email, now)
            .await?
        {
            SetInviterOutcome::Updated => Ok(()),
            SetInviterOutcome::UserNotFound => {
                Err(AdminUserError::business(AdminUserCode::UserNotFound))
            }
            SetInviterOutcome::InviterNotFound => Err(AdminUserError::validation(
                "invite_user_email",
                "邀请人不存在",
            )),
        }
    }

    pub async fn export_users(
        &self,
        filters: Vec<UserFilterClause>,
    ) -> Result<(String, String), AdminUserError> {
        validate_filters(&filters)?;
        let mut csv = self
            .external
            .start_csv(
                &[
                    "邮箱",
                    "余额",
                    "推广佣金",
                    "总流量",
                    "设备数限制",
                    "剩余流量",
                    "套餐到期时间",
                    "订阅计划",
                    "订阅地址",
                ],
                true,
            )
            .map_err(external_error)?;
        let mut after_id = 0_i64;
        let mut exported = 0_usize;
        loop {
            let page = self
                .repository
                .export_page(&filters, after_id, USER_CSV_PAGE_SIZE)
                .await?;
            let Some(last_id) = page.items.last().map(|row| row.id) else {
                break;
            };
            exported = exported.checked_add(page.items.len()).ok_or_else(|| {
                AdminUserError::business_detail(
                    AdminUserCode::InvalidParameter,
                    "导出用户数量超出支持范围，请缩小筛选范围",
                )
            })?;
            if exported > USER_CSV_MAX_ROWS {
                return Err(AdminUserError::business_detail(
                    AdminUserCode::InvalidParameter,
                    "单次最多导出 50000 个用户，请缩小筛选范围",
                ));
            }
            for row in page.items {
                let expire = row
                    .expired_at
                    .map(|expired_at| self.external.local_datetime(expired_at))
                    .unwrap_or_else(|| "长期有效".to_string());
                let balance = f64::from(row.balance) / 100.0;
                let commission = f64::from(row.commission_balance) / 100.0;
                let transfer = row.transfer_enable as f64 / GIB as f64;
                let remaining = (i128::from(row.transfer_enable)
                    - i128::from(row.uploaded)
                    - i128::from(row.downloaded)) as f64
                    / GIB as f64;
                let subscribe_url = self
                    .external
                    .subscribe_url(row.id, &row.token)
                    .await
                    .map_err(external_error)?;
                self.external
                    .write_csv(
                        &mut csv,
                        vec![
                            row.email,
                            balance.to_string(),
                            commission.to_string(),
                            transfer.to_string(),
                            row.device_limit
                                .map(|value| value.to_string())
                                .unwrap_or_default(),
                            remaining.to_string(),
                            expire,
                            row.plan_name.unwrap_or_else(|| "无订阅".to_string()),
                            subscribe_url,
                        ],
                    )
                    .map_err(external_error)?;
            }
            after_id = last_id;
        }
        Ok((
            "users.csv".to_string(),
            self.external.finish_csv(csv).map_err(external_error)?,
        ))
    }

    pub async fn ban_users(
        &self,
        filters: Vec<UserFilterClause>,
        staff_scoped: bool,
        now: i64,
    ) -> Result<(), AdminUserError> {
        validate_filters(&filters)?;
        let ids = match self
            .repository
            .ban_users(&filters, staff_scoped, USER_BULK_MAX_ROWS, now)
            .await?
        {
            BanUsersOutcome::Banned(ids) => ids,
            BanUsersOutcome::TooMany => {
                return Err(AdminUserError::business_detail(
                    AdminUserCode::InvalidParameter,
                    "单次最多批量操作 10000 个用户，请缩小筛选范围",
                ));
            }
        };
        self.external.remove_sessions(&ids).await;
        Ok(())
    }

    pub async fn delete_users(&self, filters: Vec<UserFilterClause>) -> Result<(), AdminUserError> {
        validate_filters(&filters)?;
        let ids = match self
            .repository
            .delete_users(&filters, USER_BULK_MAX_ROWS)
            .await?
        {
            DeleteUsersOutcome::Deleted(ids) => ids,
            DeleteUsersOutcome::PendingStripeOrder => {
                return Err(AdminUserError::business_detail(
                    AdminUserCode::InvalidParameter,
                    "所选用户仍有待支付的 Stripe 订单，请先取消订单",
                ));
            }
            DeleteUsersOutcome::TooMany => {
                return Err(AdminUserError::business_detail(
                    AdminUserCode::InvalidParameter,
                    "单次最多批量操作 10000 个用户，请缩小筛选范围",
                ));
            }
            DeleteUsersOutcome::UserNotFound => {
                return Err(AdminUserError::business(AdminUserCode::UserNotFound));
            }
        };
        self.external.remove_sessions(&ids).await;
        Ok(())
    }

    pub async fn delete_user(&self, user_id: i64) -> Result<(), AdminUserError> {
        match self.repository.delete_user(user_id).await? {
            DeleteUsersOutcome::Deleted(ids) => {
                self.external.remove_sessions(&ids).await;
                Ok(())
            }
            DeleteUsersOutcome::PendingStripeOrder => Err(AdminUserError::business_detail(
                AdminUserCode::InvalidParameter,
                "该用户仍有待支付的 Stripe 订单，请先取消订单",
            )),
            DeleteUsersOutcome::UserNotFound => {
                Err(AdminUserError::business(AdminUserCode::UserNotFound))
            }
            DeleteUsersOutcome::TooMany => Err(AdminUserError::External(
                "single-user delete returned a bulk-limit outcome".into(),
            )),
        }
    }
}

fn external_error(error: AdminUserExternalError) -> AdminUserError {
    AdminUserError::External(error.to_string())
}

fn validate_list_request(request: &AdminUserListRequest) -> Result<(), AdminUserError> {
    if request.limit <= 0 || request.offset < 0 {
        return Err(AdminUserError::validation(
            "page",
            "pagination bounds must be positive",
        ));
    }
    validate_filters(&request.filters)
}

pub type UserFilterViolation = filter_dsl::FilterViolation;

/// Validates the closed user-query vocabulary before any SQL adapter sees
/// it, via the shared table-driven engine (`crate::filter_dsl`).
pub fn validate_user_filters(filters: &[UserFilterClause]) -> Result<(), UserFilterViolation> {
    filter_dsl::validate_filters(filters)
}

fn validate_filters(filters: &[UserFilterClause]) -> Result<(), AdminUserError> {
    validate_user_filters(filters)
        .map_err(|violation| AdminUserError::validation(violation.field, violation.message))
}

fn validate_admin_patch(input: &AdminUserPatchInput) -> Result<(), AdminUserError> {
    if let Some(password) = input.password.as_deref()
        && !password.is_empty()
        && password.chars().count() < 8
    {
        return Err(AdminUserError::validation("password", "密码长度最小8位"));
    }
    for (field, value) in [
        ("commission_rate", input.commission_rate),
        ("discount", input.discount),
    ] {
        if let Some(Some(value)) = value
            && !(0..=100).contains(&value)
        {
            return Err(AdminUserError::validation(field, "参数范围为0-100"));
        }
    }
    if let Some(permissions) = &input.admin_permissions
        && let Some(unknown) = permissions
            .iter()
            .find(|permission| !is_registered_permission(permission))
    {
        return Err(AdminUserError::validation(
            "admin_permissions",
            format!("未注册的权限项:{unknown}"),
        ));
    }
    Ok(())
}

fn deduplicate_permissions(permissions: Vec<String>) -> Vec<String> {
    let mut output = Vec::with_capacity(permissions.len());
    for permission in permissions {
        if !output.contains(&permission) {
            output.push(permission);
        }
    }
    output
}

fn narrow_optional<T>(field: &str, value: Option<i64>) -> Result<Option<T>, AdminUserError>
where
    T: TryFrom<i64>,
{
    value
        .map(|value| {
            T::try_from(value).map_err(|_| {
                AdminUserError::validation(field, "value is outside the supported range")
            })
        })
        .transpose()
}

fn narrow_nullable<T>(
    field: &str,
    value: Option<Option<i64>>,
) -> Result<Option<Option<T>>, AdminUserError>
where
    T: TryFrom<i64>,
{
    value
        .map(|value| {
            value
                .map(|value| {
                    T::try_from(value).map_err(|_| {
                        AdminUserError::validation(field, "value is outside the supported range")
                    })
                })
                .transpose()
        })
        .transpose()
}

fn narrow_optional_code<T>(
    value: Option<i64>,
    code: AdminUserCode,
) -> Result<Option<T>, AdminUserError>
where
    T: TryFrom<i64>,
{
    value
        .map(|value| T::try_from(value).map_err(|_| AdminUserError::business(code)))
        .transpose()
}

fn narrow_nullable_code<T>(
    value: Option<Option<i64>>,
    code: AdminUserCode,
) -> Result<Option<Option<T>>, AdminUserError>
where
    T: TryFrom<i64>,
{
    value
        .map(|value| {
            value
                .map(|value| T::try_from(value).map_err(|_| AdminUserError::business(code)))
                .transpose()
        })
        .transpose()
}

#[cfg(test)]
mod tests {
    use std::{
        future::Future,
        pin::pin,
        sync::{Arc, Mutex},
        task::{Context, Poll, Waker},
    };

    use super::*;

    #[derive(Default)]
    struct FakeState {
        calls: usize,
        update: Option<AdminUserChanges>,
        cleanup: Vec<i64>,
        create: Option<CreateUsersCommand>,
    }

    #[derive(Clone, Default)]
    struct FakeRepository(Arc<Mutex<FakeState>>);

    impl AdminUserRepository for FakeRepository {
        async fn list(&self, _: &AdminUserListRequest) -> RepositoryResult<RepositoryUserPage> {
            self.0.lock().unwrap().calls += 1;
            Ok(RepositoryUserPage {
                items: vec![sample_user()],
                total: 1,
            })
        }

        async fn detail(&self, _: i64, _: bool) -> RepositoryResult<Option<AdminUserDetailRecord>> {
            Ok(Some(AdminUserDetailRecord {
                user: sample_user(),
                inviter: None,
            }))
        }

        async fn update_admin(
            &self,
            _: i64,
            changes: AdminUserChanges,
        ) -> RepositoryResult<UserUpdateOutcome> {
            self.0.lock().unwrap().update = Some(changes);
            Ok(UserUpdateOutcome::Updated)
        }

        async fn update_staff(
            &self,
            _: i64,
            _: StaffUserChanges,
        ) -> RepositoryResult<UserUpdateOutcome> {
            Ok(UserUpdateOutcome::Updated)
        }

        async fn create_users(
            &self,
            command: CreateUsersCommand,
        ) -> RepositoryResult<CreateUsersOutcome> {
            let created = command
                .accounts
                .iter()
                .enumerate()
                .map(|(index, account)| CreatedAccount {
                    id: i64::try_from(index).unwrap() + 1,
                    token: account.token.clone(),
                })
                .collect();
            self.0.lock().unwrap().create = Some(command);
            Ok(CreateUsersOutcome::Created(created))
        }

        async fn reset_secret(&self, _: i64, _: UserSecret, _: i64) -> RepositoryResult<bool> {
            Ok(true)
        }

        async fn set_inviter(
            &self,
            _: i64,
            _: Option<&str>,
            _: i64,
        ) -> RepositoryResult<SetInviterOutcome> {
            Ok(SetInviterOutcome::Updated)
        }

        async fn export_page(
            &self,
            _: &[UserFilterClause],
            _: i64,
            _: i64,
        ) -> RepositoryResult<UserExportPage> {
            Ok(UserExportPage { items: Vec::new() })
        }

        async fn ban_users(
            &self,
            _: &[UserFilterClause],
            _: bool,
            _: usize,
            _: i64,
        ) -> RepositoryResult<BanUsersOutcome> {
            Ok(BanUsersOutcome::Banned(vec![7]))
        }

        async fn delete_users(
            &self,
            _: &[UserFilterClause],
            _: usize,
        ) -> RepositoryResult<DeleteUsersOutcome> {
            Ok(DeleteUsersOutcome::Deleted(vec![7]))
        }

        async fn delete_user(&self, _: i64) -> RepositoryResult<DeleteUsersOutcome> {
            Ok(DeleteUsersOutcome::Deleted(vec![7]))
        }
    }

    #[derive(Clone)]
    struct FakeExternal(Arc<Mutex<FakeState>>);

    impl AdminUserExternal for FakeExternal {
        type CsvWriter = Vec<Vec<String>>;

        async fn hash_password(&self, password: &str) -> Result<String, AdminUserExternalError> {
            Ok(format!("hash-{password}"))
        }

        async fn prepare_accounts(
            &self,
            credentials: Vec<AccountCredential>,
        ) -> Result<Vec<PreparedAccount>, AdminUserExternalError> {
            Ok(credentials
                .into_iter()
                .enumerate()
                .map(|(index, credential)| PreparedAccount {
                    email: credential.email,
                    password: credential.password,
                    password_hash: format!("hash-{index}"),
                    uuid: format!("uuid-{index}"),
                    token: format!("token-{index}"),
                })
                .collect())
        }

        async fn enrich_users(
            &self,
            users: &mut [AdminUserListItem],
        ) -> Result<(), AdminUserExternalError> {
            for user in users {
                user.subscribe_url = format!("https://sub/{}", user.user.token);
                user.alive_ip = 2;
            }
            Ok(())
        }

        async fn subscribe_url(
            &self,
            _: i64,
            token: &str,
        ) -> Result<String, AdminUserExternalError> {
            Ok(format!("https://sub/{token}"))
        }

        async fn remove_sessions(&self, user_ids: &[i64]) {
            self.0.lock().unwrap().cleanup.extend(user_ids);
        }

        fn random_email(&self, suffix: &str) -> String {
            let index = self.0.lock().unwrap().calls;
            self.0.lock().unwrap().calls += 1;
            format!("random-{index}@{suffix}")
        }

        fn new_secret(&self) -> UserSecret {
            UserSecret {
                token: "new-token".into(),
                uuid: "new-uuid".into(),
            }
        }

        fn local_datetime(&self, epoch_seconds: i64) -> String {
            epoch_seconds.to_string()
        }

        fn start_csv(
            &self,
            headers: &[&str],
            _: bool,
        ) -> Result<Self::CsvWriter, AdminUserExternalError> {
            Ok(vec![
                headers.iter().map(|value| value.to_string()).collect(),
            ])
        }

        fn write_csv(
            &self,
            writer: &mut Self::CsvWriter,
            row: Vec<String>,
        ) -> Result<(), AdminUserExternalError> {
            writer.push(row);
            Ok(())
        }

        fn finish_csv(&self, writer: Self::CsvWriter) -> Result<String, AdminUserExternalError> {
            Ok(writer
                .into_iter()
                .map(|row| row.join(","))
                .collect::<Vec<_>>()
                .join("\n"))
        }
    }

    fn service() -> (
        AdminUserService<FakeRepository, FakeExternal>,
        Arc<Mutex<FakeState>>,
    ) {
        let state = Arc::new(Mutex::new(FakeState::default()));
        (
            AdminUserService::new(FakeRepository(state.clone()), FakeExternal(state.clone())),
            state,
        )
    }

    fn sample_user() -> AdminUser {
        AdminUser {
            id: 7,
            email: "user@example.test".into(),
            balance: 0,
            commission_balance: 0,
            transfer_enable: 10,
            device_limit: None,
            uploaded: 3,
            downloaded: 4,
            plan_id: None,
            plan_name: None,
            group_id: None,
            expired_at: None,
            uuid: "uuid".into(),
            token: "token".into(),
            banned: false,
            is_admin: false,
            is_staff: false,
            admin_permissions: Vec::new(),
            invite_user_id: None,
            discount: None,
            commission_type: 0,
            commission_rate: None,
            speed_limit: None,
            auto_renewal: None,
            remind_expire: None,
            remind_traffic: None,
            remarks: None,
            telegram_id: None,
            last_login_at: None,
            created_at: 1,
            updated_at: 1,
        }
    }

    fn block_on<F: Future>(future: F) -> F::Output {
        let mut context = Context::from_waker(Waker::noop());
        let mut future = pin!(future);
        loop {
            match future.as_mut().poll(&mut context) {
                Poll::Ready(output) => return output,
                Poll::Pending => std::thread::yield_now(),
            }
        }
    }

    #[test]
    fn invalid_permissions_and_short_passwords_fail_before_outer_ports() {
        let (service, state) = service();
        for input in [
            AdminUserPatchInput {
                password: Some("short".into()),
                ..AdminUserPatchInput::default()
            },
            AdminUserPatchInput {
                admin_permissions: Some(vec!["root:all".into()]),
                ..AdminUserPatchInput::default()
            },
        ] {
            assert!(matches!(
                block_on(service.update_user(7, input, 10)),
                Err(AdminUserError::Validation { .. })
            ));
        }
        assert_eq!(state.lock().unwrap().calls, 0);
    }

    #[test]
    fn update_narrows_values_deduplicates_grants_and_revokes_sessions() {
        let (service, state) = service();
        block_on(service.update_user(
            7,
            AdminUserPatchInput {
                password: Some("long-enough".into()),
                balance: Some(42),
                is_staff: Some(true),
                admin_permissions: Some(vec!["users:read".into(), "users:read".into()]),
                ..AdminUserPatchInput::default()
            },
            10,
        ))
        .unwrap();
        let guard = state.lock().unwrap();
        let changes = guard.update.as_ref().unwrap();
        assert_eq!(changes.balance, Some(42));
        assert_eq!(changes.admin_permissions, Some(vec!["users:read".into()]));
        assert_eq!(changes.password_hash.as_deref(), Some("hash-long-enough"));
        assert_eq!(guard.cleanup, vec![7]);
    }

    #[test]
    fn list_computes_usage_and_enriches_outside_the_repository() {
        let (service, _) = service();
        let page = block_on(service.users(AdminUserListRequest {
            limit: 10,
            offset: 0,
            filters: Vec::new(),
            sort: UserSort::default(),
        }))
        .unwrap();
        assert_eq!(page.items[0].total_used, 7);
        assert_eq!(page.items[0].alive_ip, 2);
        assert_eq!(page.items[0].subscribe_url, "https://sub/token");
    }

    #[test]
    fn invalid_filter_combinations_never_reach_postgres() {
        let (service, state) = service();
        let error = block_on(service.users(AdminUserListRequest {
            limit: 10,
            offset: 0,
            filters: vec![UserFilterClause {
                field: UserFilterField::Banned,
                operator: UserFilterOperator::Like,
                value: UserFilterValue::Text("1".into()),
            }],
            sort: UserSort::default(),
        }))
        .expect_err("boolean like must be rejected");
        assert!(matches!(error, AdminUserError::Validation { .. }));
        assert_eq!(state.lock().unwrap().calls, 0);
    }

    #[test]
    fn bulk_generation_is_bounded_and_returns_the_external_csv_artifact() {
        let (service, state) = service();
        let outcome = block_on(service.generate_users(
            UserGenerateInput {
                email_prefix: None,
                email_suffix: "example.test".into(),
                password: None,
                plan_id: None,
                expired_at: None,
                generate_count: Some(2),
            },
            20,
        ))
        .unwrap();
        let UserGenerateOutcome::Csv { body, .. } = outcome else {
            panic!("bulk generation must return CSV");
        };
        assert!(body.contains("random-"));
        assert_eq!(
            state
                .lock()
                .unwrap()
                .create
                .as_ref()
                .unwrap()
                .accounts
                .len(),
            2
        );

        assert!(matches!(
            block_on(service.generate_users(
                UserGenerateInput {
                    email_prefix: None,
                    email_suffix: "example.test".into(),
                    password: None,
                    plan_id: None,
                    expired_at: None,
                    generate_count: Some(501),
                },
                20,
            )),
            Err(AdminUserError::Validation { .. })
        ));
    }

    #[test]
    fn committed_bulk_mutations_trigger_best_effort_cleanup() {
        let (service, state) = service();
        block_on(service.ban_users(Vec::new(), true, 20)).unwrap();
        block_on(service.delete_users(Vec::new())).unwrap();
        block_on(service.delete_user(7)).unwrap();
        assert_eq!(state.lock().unwrap().cleanup, vec![7, 7, 7]);
    }
}
