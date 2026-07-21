use std::sync::Arc;

use chrono::TimeZone as _;
use v2board_api_contract::{
    admin_business::{
        AdminInviterItem, AdminUserDetail, AdminUserFields, AdminUserListItem, StaffUserDetail,
    },
    time::Rfc3339Timestamp,
};
use v2board_application::admin_user::{
    AccountCredential, AdminUser, AdminUserDetail as ApplicationAdminUserDetail, AdminUserExternal,
    AdminUserExternalError, AdminUserListItem as ApplicationAdminUserListItem, AdminUserService,
    PreparedAccount, StaffUserDetail as ApplicationStaffUserDetail, UserSecret,
};
use v2board_config::{AppConfig, app_timezone};
use v2board_db::admin_user::PostgresAdminUserRepository;

#[derive(Clone)]
pub(crate) struct ContractAdminUserExternal {
    config: Arc<AppConfig>,
}

pub(crate) fn contract_admin_user_service(
    pool: sqlx::PgPool,
    config: Arc<AppConfig>,
) -> AdminUserService<PostgresAdminUserRepository, ContractAdminUserExternal> {
    AdminUserService::new(
        PostgresAdminUserRepository::new(pool),
        ContractAdminUserExternal { config },
    )
}

impl AdminUserExternal for ContractAdminUserExternal {
    type CsvWriter = Vec<Vec<String>>;

    async fn hash_password(&self, _: &str) -> Result<String, AdminUserExternalError> {
        Err(AdminUserExternalError::new(
            "contract user adapter does not create passwords",
        ))
    }

    async fn prepare_accounts(
        &self,
        _: Vec<AccountCredential>,
    ) -> Result<Vec<PreparedAccount>, AdminUserExternalError> {
        Err(AdminUserExternalError::new(
            "contract user adapter does not generate accounts",
        ))
    }

    async fn enrich_users(
        &self,
        users: &mut [ApplicationAdminUserListItem],
    ) -> Result<(), AdminUserExternalError> {
        for user in users {
            user.subscribe_url = self.config.subscribe_url_for_token(&user.user.token);
        }
        Ok(())
    }

    async fn subscribe_url(&self, _: i64, token: &str) -> Result<String, AdminUserExternalError> {
        Ok(self.config.subscribe_url_for_token(token))
    }

    async fn remove_sessions(&self, _: &[i64]) {}

    fn random_email(&self, suffix: &str) -> String {
        format!("contract@{suffix}")
    }

    fn new_secret(&self) -> UserSecret {
        UserSecret {
            token: uuid::Uuid::new_v4().simple().to_string(),
            uuid: uuid::Uuid::new_v4().to_string(),
        }
    }

    fn local_datetime(&self, epoch_seconds: i64) -> String {
        app_timezone()
            .timestamp_opt(epoch_seconds, 0)
            .single()
            .map(|value| value.format("%Y-%m-%d %H:%M:%S").to_string())
            .unwrap_or_default()
    }

    fn start_csv(
        &self,
        headers: &[&str],
        _: bool,
    ) -> Result<Self::CsvWriter, AdminUserExternalError> {
        Ok(vec![
            headers.iter().map(|value| (*value).to_string()).collect(),
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

fn user_fields(user: AdminUser) -> AdminUserFields {
    AdminUserFields {
        id: user.id,
        email: user.email,
        password: String::new(),
        balance: user.balance,
        commission_balance: user.commission_balance,
        transfer_enable: user.transfer_enable,
        device_limit: user.device_limit,
        u: user.uploaded,
        d: user.downloaded,
        plan_id: user.plan_id,
        group_id: user.group_id,
        expired_at: user.expired_at.map(Rfc3339Timestamp::from_epoch_seconds),
        uuid: user.uuid,
        token: user.token,
        banned: i16::from(user.banned),
        is_admin: i16::from(user.is_admin),
        is_staff: i16::from(user.is_staff),
        admin_permissions: user.admin_permissions,
        invite_user_id: user.invite_user_id,
        discount: user.discount,
        commission_type: user.commission_type,
        commission_rate: user.commission_rate,
        speed_limit: user.speed_limit,
        auto_renewal: user.auto_renewal.map(i16::from),
        remind_expire: user.remind_expire.map(i16::from),
        remind_traffic: user.remind_traffic.map(i16::from),
        remarks: user.remarks,
        telegram_id: user.telegram_id,
        last_login_at: user.last_login_at.map(Rfc3339Timestamp::from_epoch_seconds),
        created_at: Rfc3339Timestamp::from_epoch_seconds(user.created_at),
        updated_at: Rfc3339Timestamp::from_epoch_seconds(user.updated_at),
    }
}

pub(crate) fn admin_user_list_item(mut item: ApplicationAdminUserListItem) -> AdminUserListItem {
    let plan_name = item.user.plan_name.take();
    AdminUserListItem {
        user: user_fields(item.user),
        total_used: item.total_used,
        alive_ip: item.alive_ip,
        ips: item.ips,
        plan_name,
        subscribe_url: item.subscribe_url,
    }
}

pub(crate) fn admin_user_detail(detail: ApplicationAdminUserDetail) -> AdminUserDetail {
    AdminUserDetail {
        user: admin_user_list_item(detail.user),
        invite_user: detail.inviter.map(|inviter| AdminInviterItem {
            id: inviter.id,
            email: inviter.email,
        }),
    }
}

pub(crate) fn staff_user_detail(detail: ApplicationStaffUserDetail) -> StaffUserDetail {
    StaffUserDetail {
        user: admin_user_list_item(detail.user),
    }
}
