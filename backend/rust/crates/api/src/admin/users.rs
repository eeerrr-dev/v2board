use axum::{
    Json,
    extract::{Extension, Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
};
use serde::Deserialize;
use v2board_api_contract::{
    admin_business::{
        AdminFilterClause, AdminFilterNumber, AdminFilterOperator, AdminFilterScalar,
        AdminFilterValue, AdminInviterItem, AdminSetInviterRequest, AdminUserDetail,
        AdminUserFields, AdminUserFilterRequest, AdminUserGenerateRequest, AdminUserListItem,
        AdminUserMailRequest, AdminUserPatchRequest, StaffUserDetail, StaffUserPatchRequest,
    },
    common::{CreatedInt64Id, Page},
    time::Rfc3339Timestamp,
};
use v2board_application::{
    admin_user::{
        AdminInviter, AdminUser, AdminUserCode, AdminUserDetail as AppAdminUserDetail,
        AdminUserError, AdminUserListItem as AppAdminUserListItem, AdminUserListRequest,
        AdminUserPatchInput, StaffUserDetail as AppStaffUserDetail, StaffUserPatchInput,
        UserColumnKind, UserFilterClause, UserFilterField, UserFilterOperator, UserFilterValue,
        UserGenerateInput, UserGenerateOutcome as AppUserGenerateOutcome, UserSort, UserSortField,
    },
    auth::AuthUser,
    configuration::{BulkMailInput, MailAudience},
};
use v2board_compat::{ApiError, Code, Pagination, Problem};

use crate::{
    dialect::{DialectJson, problem_from},
    locale::request_locale,
    runtime::AppState,
};

use super::{csv_attachment, mail_idempotency_key};

/// §8 default for `GET users` (the legacy admin user list default of 10).
const USER_LIST_DEFAULT_PER_PAGE: i64 = 10;

fn user_fields(view: AdminUser) -> AdminUserFields {
    AdminUserFields {
        id: view.id,
        email: view.email,
        password: String::new(),
        balance: view.balance,
        commission_balance: view.commission_balance,
        transfer_enable: view.transfer_enable,
        device_limit: view.device_limit,
        u: view.uploaded,
        d: view.downloaded,
        plan_id: view.plan_id,
        group_id: view.group_id,
        expired_at: view.expired_at.map(Rfc3339Timestamp::from_epoch_seconds),
        uuid: view.uuid,
        token: view.token,
        banned: i16::from(view.banned),
        is_admin: i16::from(view.is_admin),
        is_staff: i16::from(view.is_staff),
        admin_permissions: view.admin_permissions,
        invite_user_id: view.invite_user_id,
        discount: view.discount,
        commission_type: view.commission_type,
        commission_rate: view.commission_rate,
        speed_limit: view.speed_limit,
        auto_renewal: view.auto_renewal.map(i16::from),
        remind_expire: view.remind_expire.map(i16::from),
        remind_traffic: view.remind_traffic.map(i16::from),
        remarks: view.remarks,
        telegram_id: view.telegram_id,
        last_login_at: view.last_login_at.map(Rfc3339Timestamp::from_epoch_seconds),
        created_at: Rfc3339Timestamp::from_epoch_seconds(view.created_at),
        updated_at: Rfc3339Timestamp::from_epoch_seconds(view.updated_at),
    }
}

fn user_list_item(mut view: AppAdminUserListItem) -> AdminUserListItem {
    let plan_name = view.user.plan_name.take();
    AdminUserListItem {
        user: user_fields(view.user),
        total_used: view.total_used,
        alive_ip: view.alive_ip,
        ips: view.ips,
        plan_name,
        subscribe_url: view.subscribe_url,
    }
}

fn inviter_item(view: AdminInviter) -> AdminInviterItem {
    AdminInviterItem {
        id: view.id,
        email: view.email,
    }
}

fn user_detail_item(view: AppAdminUserDetail) -> AdminUserDetail {
    AdminUserDetail {
        user: user_list_item(view.user),
        invite_user: view.inviter.map(inviter_item),
    }
}

pub(super) fn staff_user_detail_item(view: AppStaffUserDetail) -> StaffUserDetail {
    StaffUserDetail {
        user: user_list_item(view.user),
    }
}

fn filter_number(value: AdminFilterNumber) -> Result<i64, ApiError> {
    match value {
        AdminFilterNumber::Integer(value) => Ok(value),
        AdminFilterNumber::Unsigned(value) => i64::try_from(value).map_err(|_| {
            Problem::validation_field("filter", "filter integer is outside the supported range")
                .into()
        }),
        AdminFilterNumber::Decimal(_) => {
            Err(Problem::validation_field("filter", "filter value must be an integer").into())
        }
    }
}

fn timestamp_filter(field: UserFilterField, value: String) -> Result<i64, ApiError> {
    chrono::DateTime::parse_from_rfc3339(&value)
        .map(|instant| instant.timestamp())
        .map_err(|_| {
            Problem::validation_field(
                "filter",
                format!("{} requires an RFC 3339 timestamp value", field.name()),
            )
            .into()
        })
}

fn filter_scalar(
    field: UserFilterField,
    value: AdminFilterScalar,
) -> Result<UserFilterValue, ApiError> {
    match (field.kind(), value) {
        (UserColumnKind::Boolean, AdminFilterScalar::Bool(value)) => {
            Ok(UserFilterValue::Boolean(value))
        }
        (UserColumnKind::Integer, AdminFilterScalar::Number(value)) => {
            filter_number(value).map(UserFilterValue::Integer)
        }
        (UserColumnKind::Timestamp, AdminFilterScalar::String(value)) => {
            timestamp_filter(field, value).map(UserFilterValue::Integer)
        }
        (UserColumnKind::Text | UserColumnKind::Email, AdminFilterScalar::String(value)) => {
            Ok(UserFilterValue::Text(value))
        }
        _ => Err(Problem::validation_field(
            "filter",
            format!("filter value type does not match {}", field.name()),
        )
        .into()),
    }
}

fn filter_value(
    field: UserFilterField,
    value: AdminFilterValue,
) -> Result<UserFilterValue, ApiError> {
    match value {
        AdminFilterValue::Null => Ok(UserFilterValue::Null),
        AdminFilterValue::Bool(value) => Ok(UserFilterValue::Boolean(value)),
        AdminFilterValue::Number(value) => filter_number(value).map(UserFilterValue::Integer),
        AdminFilterValue::String(value) if field.kind() == UserColumnKind::Timestamp => {
            timestamp_filter(field, value).map(UserFilterValue::Integer)
        }
        AdminFilterValue::String(value) => Ok(UserFilterValue::Text(value)),
        AdminFilterValue::Array(values) => {
            let values = values
                .into_iter()
                .map(|value| filter_scalar(field, value))
                .collect::<Result<Vec<_>, _>>()?;
            match field.kind() {
                UserColumnKind::Boolean => values
                    .into_iter()
                    .map(|value| match value {
                        UserFilterValue::Boolean(value) => Ok(value),
                        _ => unreachable!(),
                    })
                    .collect::<Result<Vec<_>, ApiError>>()
                    .map(UserFilterValue::Booleans),
                UserColumnKind::Integer | UserColumnKind::Timestamp => values
                    .into_iter()
                    .map(|value| match value {
                        UserFilterValue::Integer(value) => Ok(value),
                        _ => unreachable!(),
                    })
                    .collect::<Result<Vec<_>, ApiError>>()
                    .map(UserFilterValue::Integers),
                UserColumnKind::Text | UserColumnKind::Email => values
                    .into_iter()
                    .map(|value| match value {
                        UserFilterValue::Text(value) => Ok(value),
                        _ => unreachable!(),
                    })
                    .collect::<Result<Vec<_>, ApiError>>()
                    .map(UserFilterValue::Texts),
            }
        }
    }
}

fn filter_operator(value: AdminFilterOperator) -> UserFilterOperator {
    match value {
        AdminFilterOperator::Eq => UserFilterOperator::Eq,
        AdminFilterOperator::Neq => UserFilterOperator::Neq,
        AdminFilterOperator::Like => UserFilterOperator::Like,
        AdminFilterOperator::Gt => UserFilterOperator::Gt,
        AdminFilterOperator::Gte => UserFilterOperator::Gte,
        AdminFilterOperator::Lt => UserFilterOperator::Lt,
        AdminFilterOperator::Lte => UserFilterOperator::Lte,
        AdminFilterOperator::In => UserFilterOperator::In,
    }
}

fn filter_clauses(
    clauses: Option<Vec<AdminFilterClause>>,
) -> Result<Vec<UserFilterClause>, ApiError> {
    clauses
        .unwrap_or_default()
        .into_iter()
        .map(|clause| {
            let field = UserFilterField::parse(&clause.field).ok_or_else(|| {
                ApiError::from(Problem::validation_field(
                    "filter",
                    format!("field {} is not filterable", clause.field),
                ))
            })?;
            Ok(UserFilterClause {
                field,
                operator: filter_operator(clause.op),
                value: filter_value(field, clause.value)?,
            })
        })
        .collect()
}

fn user_generate_request(body: AdminUserGenerateRequest) -> UserGenerateInput {
    UserGenerateInput {
        email_prefix: body.email_prefix,
        email_suffix: body.email_suffix,
        password: body.password,
        plan_id: body.plan_id,
        expired_at: body.expired_at.map(Rfc3339Timestamp::epoch_seconds),
        generate_count: body.generate_count,
    }
}

fn user_patch_request(body: AdminUserPatchRequest) -> AdminUserPatchInput {
    AdminUserPatchInput {
        email: body.email.into_option(),
        password: body.password.into_option(),
        transfer_enable: body.transfer_enable.into_option(),
        uploaded: body.u.into_option(),
        downloaded: body.d.into_option(),
        balance: body.balance.into_option(),
        commission_balance: body.commission_balance.into_option(),
        commission_type: body.commission_type.into_option(),
        banned: body.banned.into_option(),
        is_admin: body.is_admin.into_option(),
        is_staff: body.is_staff.into_option(),
        admin_permissions: body.admin_permissions.into_option(),
        device_limit: body.device_limit,
        commission_rate: body.commission_rate,
        discount: body.discount,
        speed_limit: body.speed_limit,
        plan_id: body.plan_id,
        remarks: body.remarks,
        expired_at: body
            .expired_at
            .map(|value| value.map(Rfc3339Timestamp::epoch_seconds)),
    }
}

pub(super) fn staff_user_patch_request(body: StaffUserPatchRequest) -> StaffUserPatchInput {
    StaffUserPatchInput {
        email: body.email.into_option(),
        password: body.password.into_option(),
        transfer_enable: body.transfer_enable.into_option(),
        uploaded: body.u.into_option(),
        downloaded: body.d.into_option(),
        balance: body.balance.into_option(),
        commission_balance: body.commission_balance.into_option(),
        banned: body.banned.into_option(),
        device_limit: body.device_limit,
        commission_rate: body.commission_rate,
        discount: body.discount,
        plan_id: body.plan_id,
        expired_at: body
            .expired_at
            .map(|value| value.map(Rfc3339Timestamp::epoch_seconds)),
    }
}

pub(super) fn user_filter_request(
    body: AdminUserFilterRequest,
) -> Result<Vec<UserFilterClause>, ApiError> {
    filter_clauses(body.filter)
}

pub(super) fn admin_user_error(error: AdminUserError) -> ApiError {
    match error {
        AdminUserError::Validation { field, message } => {
            Problem::validation_field(field, message).into()
        }
        AdminUserError::Business { code, detail } => {
            let code = match code {
                AdminUserCode::EmailAlreadyRegistered => Code::EmailAlreadyRegistered,
                AdminUserCode::InvalidParameter => Code::InvalidParameter,
                AdminUserCode::PlanNotFound => Code::PlanNotFound,
                AdminUserCode::PlanUnavailable => Code::PlanUnavailable,
                AdminUserCode::UserNotFound => Code::UserNotFound,
            };
            let problem = detail.map_or_else(
                || Problem::new(code),
                |detail| Problem::new(code).with_detail(detail),
            );
            problem.into()
        }
        AdminUserError::External(error) => ApiError::internal(error),
        AdminUserError::Repository(error) => ApiError::internal(error.to_string()),
    }
}

pub(super) fn user_mail_request(body: AdminUserMailRequest) -> Result<BulkMailInput, ApiError> {
    Ok(BulkMailInput {
        subject: body.subject,
        content: body.content,
        filter: body
            .filter
            .map(|clauses| filter_clauses(Some(clauses)))
            .transpose()?,
    })
}

#[derive(Deserialize)]
pub(super) struct UsersListQuery {
    page: Option<i64>,
    per_page: Option<i64>,
    filter: Option<String>,
    sort_by: Option<String>,
    sort_dir: Option<String>,
}

/// GET `users` (§6.6): §8 pagination + the §7 DSL over the guarded user
/// column whitelist, §7.2 sort (incl. the computed `total_used`), and the W12
/// admin projection (RFC 3339 timestamps, `t` dropped).
pub(super) async fn users_list(
    State(state): State<AppState>,
    Query(query): Query<UsersListQuery>,
    headers: HeaderMap,
) -> Result<Json<Page<AdminUserListItem>>, Problem> {
    let locale = request_locale(&headers);
    let pagination = Pagination::resolve(query.page, query.per_page, USER_LIST_DEFAULT_PER_PAGE)?;
    let filters = query
        .filter
        .map(|raw| {
            serde_json::from_str::<Vec<AdminFilterClause>>(&raw).map_err(|error| {
                ApiError::from(Problem::validation_field(
                    "filter",
                    format!("filter must be a JSON clause array: {error}"),
                ))
            })
        })
        .transpose()
        .and_then(filter_clauses)
        .map_err(|error| problem_from(error, locale))?;
    let sort_field = query.sort_by.as_deref().unwrap_or("created_at");
    let sort_field = UserSortField::parse(sort_field).ok_or_else(|| {
        Problem::validation_field(
            "sort_by",
            format!("sort_by field {sort_field} is not sortable"),
        )
    })?;
    let descending = match query.sort_dir.as_deref() {
        None | Some("desc") => true,
        Some("asc") => false,
        Some(other) => {
            return Err(Problem::validation_field(
                "sort_dir",
                format!("sort_dir must be asc or desc, got {other}"),
            ));
        }
    };
    let page = state
        .admin_user_service()
        .users(AdminUserListRequest {
            limit: pagination.limit(),
            offset: pagination.offset(),
            filters,
            sort: UserSort {
                field: sort_field,
                descending,
            },
        })
        .await
        .map_err(admin_user_error)
        .map_err(|error| problem_from(error, locale))?;
    Ok(Json(Page::new(
        page.items.into_iter().map(user_list_item).collect(),
        page.total,
    )))
}

/// GET `users/{id}` (§6.6): bare W12 projection with the conditional
/// `invite_user` object; `user_not_found` (404) when absent.
pub(super) async fn user_detail(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    headers: HeaderMap,
) -> Result<Json<AdminUserDetail>, Problem> {
    let locale = request_locale(&headers);
    state
        .admin_user_service()
        .user_detail(id)
        .await
        .map(user_detail_item)
        .map(Json)
        .map_err(admin_user_error)
        .map_err(|error| problem_from(error, locale))
}

/// POST `users` (§6.6): a single create (real `email_prefix`) is the §1 201
/// `{id}`; the bulk generate streams the byte-frozen credential CSV.
pub(super) async fn user_generate(
    State(state): State<AppState>,
    headers: HeaderMap,
    DialectJson(body): DialectJson<AdminUserGenerateRequest>,
) -> Result<Response, Problem> {
    let locale = request_locale(&headers);
    let body = user_generate_request(body);
    let outcome = state
        .admin_user_service()
        .generate_users(body, chrono::Utc::now().timestamp())
        .await
        .map_err(admin_user_error)
        .map_err(|error| problem_from(error, locale))?;
    match outcome {
        AppUserGenerateOutcome::Created { id } => {
            Ok((StatusCode::CREATED, Json(CreatedInt64Id { id })).into_response())
        }
        AppUserGenerateOutcome::Csv { filename, body } => {
            csv_attachment(&filename, body).map_err(|error| problem_from(error, locale))
        }
    }
}

/// PATCH `users/{id}` (§6.6): §4.4 partial update; empty 204.
pub(super) async fn user_patch(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    headers: HeaderMap,
    DialectJson(body): DialectJson<AdminUserPatchRequest>,
) -> Result<StatusCode, Problem> {
    let locale = request_locale(&headers);
    let body = user_patch_request(body);
    state
        .admin_user_service()
        .update_user(id, body, chrono::Utc::now().timestamp())
        .await
        .map_err(admin_user_error)
        .map_err(|error| problem_from(error, locale))?;
    Ok(StatusCode::NO_CONTENT)
}

/// DELETE `users/{id}` (§6.6): single-user cascade delete; empty 204.
pub(super) async fn user_delete(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    headers: HeaderMap,
) -> Result<StatusCode, Problem> {
    let locale = request_locale(&headers);
    state
        .admin_user_service()
        .delete_user(id)
        .await
        .map_err(admin_user_error)
        .map_err(|error| problem_from(error, locale))?;
    Ok(StatusCode::NO_CONTENT)
}

/// POST `users/{id}/set-inviter` (§6.6): `{invite_user_email}`; empty 204.
pub(super) async fn user_set_inviter(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    headers: HeaderMap,
    DialectJson(body): DialectJson<AdminSetInviterRequest>,
) -> Result<StatusCode, Problem> {
    let locale = request_locale(&headers);
    state
        .admin_user_service()
        .set_inviter(id, body.invite_user_email, chrono::Utc::now().timestamp())
        .await
        .map_err(admin_user_error)
        .map_err(|error| problem_from(error, locale))?;
    Ok(StatusCode::NO_CONTENT)
}

/// POST `users/{id}/reset-secret` (§6.6): rotates the subscribe token/UUID;
/// empty 204.
pub(super) async fn user_reset_secret(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    headers: HeaderMap,
) -> Result<StatusCode, Problem> {
    let locale = request_locale(&headers);
    state
        .admin_user_service()
        .reset_secret(id, chrono::Utc::now().timestamp())
        .await
        .map_err(admin_user_error)
        .map_err(|error| problem_from(error, locale))?;
    Ok(StatusCode::NO_CONTENT)
}

/// POST `users/export` (§6.6): CSV over the `{filter?}` DSL body.
pub(super) async fn users_export(
    State(state): State<AppState>,
    headers: HeaderMap,
    DialectJson(body): DialectJson<AdminUserFilterRequest>,
) -> Result<Response, Problem> {
    let locale = request_locale(&headers);
    let filters = user_filter_request(body).map_err(|error| problem_from(error, locale))?;
    let (filename, csv) = state
        .admin_user_service()
        .export_users(filters)
        .await
        .map_err(admin_user_error)
        .map_err(|error| problem_from(error, locale))?;
    csv_attachment(&filename, csv).map_err(|error| problem_from(error, locale))
}

/// POST `users/ban` (§6.6): bulk-ban over the `{filter?}` DSL body; empty 204.
pub(super) async fn users_ban(
    State(state): State<AppState>,
    headers: HeaderMap,
    DialectJson(body): DialectJson<AdminUserFilterRequest>,
) -> Result<StatusCode, Problem> {
    let locale = request_locale(&headers);
    let filters = user_filter_request(body).map_err(|error| problem_from(error, locale))?;
    state
        .admin_user_service()
        .ban_users(filters, false, chrono::Utc::now().timestamp())
        .await
        .map_err(admin_user_error)
        .map_err(|error| problem_from(error, locale))?;
    Ok(StatusCode::NO_CONTENT)
}

/// POST `users/bulk-delete` (§6.6): bulk cascade delete over the `{filter?}`
/// DSL body; empty 204.
pub(super) async fn users_bulk_delete(
    State(state): State<AppState>,
    headers: HeaderMap,
    DialectJson(body): DialectJson<AdminUserFilterRequest>,
) -> Result<StatusCode, Problem> {
    let locale = request_locale(&headers);
    let filters = user_filter_request(body).map_err(|error| problem_from(error, locale))?;
    state
        .admin_user_service()
        .delete_users(filters)
        .await
        .map_err(admin_user_error)
        .map_err(|error| problem_from(error, locale))?;
    Ok(StatusCode::NO_CONTENT)
}

/// POST `users/mail` (§6.6): `{subject, content, filter?}` with the unchanged
/// `Idempotency-Key` replay contract; empty 204.
pub(super) async fn users_mail(
    State(state): State<AppState>,
    Extension(admin): Extension<AuthUser>,
    headers: HeaderMap,
    DialectJson(body): DialectJson<AdminUserMailRequest>,
) -> Result<StatusCode, Problem> {
    let locale = request_locale(&headers);
    let body = user_mail_request(body).map_err(|error| problem_from(error, locale))?;
    let idempotency_key =
        mail_idempotency_key(&headers).map_err(|error| problem_from(error, locale))?;
    state
        .configuration_service()
        .send_bulk_mail(MailAudience::Admin, &body, &admin.email, &idempotency_key)
        .await
        .map_err(super::configuration::configuration_error)
        .map_err(|error| problem_from(error, locale))?;
    Ok(StatusCode::NO_CONTENT)
}
