use serde::Deserialize;
use v2board_compat::{
    Code, Pagination, Problem,
    json::{double_option, rfc3339},
};

use super::*;

// === W11 modern commerce family (docs/api-dialect.md §6.2/§6.4) ===
//
// Plans, payments, orders, and payment reconciliations on dialect-v2
// semantics: JSON bodies, §4.4 double-Option partial updates, §4.5 RFC 3339
// timestamps, §1 201 `{id}`/`{trade_no}` creates, §7 DSL order filtering,
// §8 pagination, and typed §3.4 problem codes. Since W14 the §6.9 staff
// mirror consumes the same modern `plans_list`.

const PLAN_USER_LOCK_PAGE_SIZE: i64 = 500;
const PLAN_FORCE_UPDATE_MAX_USERS: usize = 10_000;
const ADMIN_ASSIGN_UNFINISHED_ORDER_SQL: &str = r#"
SELECT id
FROM orders
WHERE user_id = $1 AND status IN (0, 1)
LIMIT 1
FOR UPDATE
"#;
const UNFINISHED_ORDER_UNIQUE_KEY: &str = "uniq_unfinished_order_per_user";

pub(super) fn resolve_redacted_payment_config(
    payment: &str,
    current: Option<(&str, &str)>,
    mut submitted: Value,
) -> Result<Value, ApiError> {
    let submitted_object = submitted
        .as_object_mut()
        .ok_or_else(|| validation_error("config", "配置参数格式有误"))?;
    let current_config = current
        .filter(|(current_payment, _)| *current_payment == payment)
        .map(|(_, raw)| parse_payment_config(raw))
        .transpose()?;
    if let (Some(provider), Some(current_config)) = (
        crate::payment_provider::payment_provider_manifest(payment),
        current_config.as_ref(),
    ) {
        let redacted_current =
            crate::payment_provider::redact_payment_config(payment, current_config);
        for field in provider.fields {
            match current_config.get(field.key) {
                Some(existing)
                    if submitted_object.get(field.key) == redacted_current.get(field.key) =>
                {
                    submitted_object.insert(field.key.to_string(), existing.clone());
                }
                None if submitted_object.get(field.key).and_then(Value::as_str) == Some("") => {
                    submitted_object.remove(field.key);
                }
                _ => {}
            }
        }
    }
    let preserve_keys = submitted_object
        .iter()
        .filter(|(_, value)| {
            value.as_str() == Some(crate::payment_provider::REDACTED_PAYMENT_SECRET)
        })
        .map(|(key, _)| key.clone())
        .collect::<Vec<_>>();
    for key in preserve_keys {
        let Some(existing) = current_config.as_ref().and_then(|config| config.get(&key)) else {
            return Err(validation_error(
                &format!("config.{key}"),
                "请填写真实密钥，脱敏占位符不能作为新密钥保存",
            ));
        };
        submitted_object.insert(key, existing.clone());
    }

    // Known manifests deliberately omit undeclared legacy keys from every
    // response, while unknown providers mask every value. Preserve those
    // hidden values on a metadata-only edit so a redacted round trip never
    // mutates verification material. Any real driver/config change is rejected
    // by the PATCH handler because each payment row is an immutable
    // verification version.
    if let Some(current_object) = current_config.as_ref().and_then(Value::as_object) {
        let known_fields =
            crate::payment_provider::payment_provider_manifest(payment).map(|provider| {
                provider
                    .fields
                    .iter()
                    .map(|field| field.key)
                    .collect::<HashSet<_>>()
            });
        for (key, value) in current_object {
            let hidden = known_fields
                .as_ref()
                .is_none_or(|fields| !fields.contains(key.as_str()));
            if hidden {
                submitted_object
                    .entry(key.clone())
                    .or_insert_with(|| value.clone());
            }
        }
    }
    Ok(submitted)
}

fn payment_reconciliation_identity_hash(value: &str) -> [u8; 32] {
    Sha256::digest(value.as_bytes()).into()
}

/// §6.4 `?resolved=` vocabulary for `GET payment-reconciliations` — the
/// legacy dedicated scalar filter survives unchanged: absent/`0`/
/// `unresolved`/`open` list open rows, `1`/`resolved`/`closed` list resolved
/// rows, `all` lists both.
pub(super) fn reconciliation_resolved_filter(resolved: Option<&str>) -> Result<i16, ApiError> {
    match resolved {
        None | Some("0" | "unresolved" | "open") => Ok(0),
        Some("1" | "resolved" | "closed") => Ok(1),
        Some("all") => Ok(2),
        Some(_) => Err(validation_error(
            "resolved",
            "resolved must be one of 0, 1, unresolved, resolved, or all",
        )),
    }
}

fn map_admin_order_write_error(error: sqlx::Error) -> ApiError {
    let Some(database_error) = error.as_database_error() else {
        return ApiError::Database(error);
    };
    if database_error.constraint() == Some(UNFINISHED_ORDER_UNIQUE_KEY)
        || database_error
            .message()
            .contains(UNFINISHED_ORDER_UNIQUE_KEY)
    {
        return Problem::new(Code::OrderAssignConflict).into();
    }
    if matches!(
        database_error.code().as_deref(),
        Some("40P01" | "40001" | "55P03")
    ) {
        return Problem::new(Code::OrderUpdateConflict).into();
    }
    ApiError::Database(error)
}

async fn lock_server_group_for_share(
    tx: &mut DbTransaction<'_>,
    group_id: i64,
) -> Result<(), ApiError> {
    let exists: Option<i32> =
        sqlx::query_scalar("SELECT id FROM server_group WHERE id = $1 LIMIT 1 FOR SHARE")
            .bind(group_id)
            .fetch_optional(&mut **tx)
            .await?;
    if exists.is_none() {
        return Err(Problem::new(Code::ServerGroupNotFound).into());
    }
    Ok(())
}

async fn lock_plan_users_for_update(
    tx: &mut DbTransaction<'_>,
    plan_id: i64,
) -> Result<(), ApiError> {
    let mut after_id = 0_i64;
    let mut locked = 0_usize;
    loop {
        let ids = sqlx::query_scalar::<_, i64>(
            r#"
            SELECT id
            FROM users
            WHERE plan_id = $1 AND id > $2
            ORDER BY id
            LIMIT $3
            FOR UPDATE
            "#,
        )
        .bind(plan_id)
        .bind(after_id)
        .bind(PLAN_USER_LOCK_PAGE_SIZE)
        .fetch_all(&mut **tx)
        .await?;
        let Some(last_id) = ids.last().copied() else {
            return Ok(());
        };
        locked = locked.saturating_add(ids.len());
        if locked > PLAN_FORCE_UPDATE_MAX_USERS {
            return Err(Problem::new(Code::PlanForceUpdateLimitExceeded).into());
        }
        after_id = last_id;
    }
}

fn nonnegative_i32(field: &str, value: i64) -> Result<i64, ApiError> {
    if !(0..=i64::from(i32::MAX)).contains(&value) {
        return Err(ApiError::from(Problem::validation_field(
            field,
            "Value must be a non-negative 32-bit integer",
        )));
    }
    Ok(value)
}

pub(super) fn optional_nonnegative_i32(
    field: &str,
    value: Option<i64>,
) -> Result<Option<i64>, ApiError> {
    value.map(|value| nonnegative_i32(field, value)).transpose()
}

fn optional_smallint(field: &str, value: Option<i64>) -> Result<Option<i64>, ApiError> {
    if value.is_some_and(|value| i16::try_from(value).is_err()) {
        return Err(ApiError::from(Problem::validation_field(
            field,
            "Value must be a 16-bit integer",
        )));
    }
    Ok(value)
}

pub(super) fn parse_payment_config(raw: &str) -> Result<Value, ApiError> {
    let config = serde_json::from_str::<Value>(raw).map_err(|error| {
        ApiError::internal(format!("stored payment config is invalid JSON: {error}"))
    })?;
    if !config.is_object() {
        return Err(ApiError::internal(
            "stored payment config must be a JSON object",
        ));
    }
    Ok(config)
}

pub(super) fn reconciliation_resolution(actor: &str, note: &str) -> Result<String, ApiError> {
    if note.chars().count() > 160 {
        return Err(validation_error("resolution", "核对说明不能超过160个字符"));
    }
    let value = serde_json::to_string(&json!({ "actor": actor, "note": note }))
        .map_err(|_| ApiError::internal("failed to encode reconciliation resolution"))?;
    if value.len() > 255 {
        return Err(validation_error("resolution", "核对说明编码后超过存储限制"));
    }
    Ok(value)
}

/// §6.2: `handling_fee_percent` crosses the wire as a JSON number; the exact
/// legacy 0.1–100 range check runs on the decimal representation.
fn handling_fee_percent_decimal(
    value: Option<&serde_json::Number>,
) -> Result<Option<Decimal>, ApiError> {
    let Some(number) = value else {
        return Ok(None);
    };
    match number.to_string().parse::<Decimal>() {
        Ok(percent) if (Decimal::new(1, 1)..=Decimal::from(100)).contains(&percent) => {
            Ok(Some(percent))
        }
        _ => Err(validation_error(
            "handling_fee_percent",
            "百分比手续费范围须在0.1-100之间",
        )),
    }
}

/// One admin plan row (§6.2 `GET plans`): the legacy field set plus the
/// active-user `count`, on modern value types — bool flags, §4.5 RFC 3339
/// timestamps. Prices stay integer cents; `transfer_enable` stays the
/// operator-facing GiB figure.
#[derive(Debug, Serialize)]
pub struct AdminPlanItem {
    pub id: i32,
    pub group_id: i32,
    pub transfer_enable: i64,
    pub device_limit: Option<i32>,
    pub name: String,
    pub speed_limit: Option<i32>,
    pub show: bool,
    pub sort: Option<i32>,
    pub renew: bool,
    pub content: Option<String>,
    pub month_price: Option<i32>,
    pub quarter_price: Option<i32>,
    pub half_year_price: Option<i32>,
    pub year_price: Option<i32>,
    pub two_year_price: Option<i32>,
    pub three_year_price: Option<i32>,
    pub onetime_price: Option<i32>,
    pub reset_price: Option<i32>,
    pub reset_traffic_method: Option<i16>,
    pub capacity_limit: Option<i32>,
    pub count: i64,
    #[serde(with = "rfc3339")]
    pub created_at: i64,
    #[serde(with = "rfc3339")]
    pub updated_at: i64,
}

/// POST `plans` (§6.2): the legacy PlanSave field set as a JSON body.
/// Creates keep the DB defaults PlanSave never touched (`show` = 0,
/// `renew` = 1, `sort` = NULL).
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PlanCreate {
    pub name: String,
    pub group_id: i64,
    pub transfer_enable: i64,
    #[serde(default)]
    pub device_limit: Option<i64>,
    #[serde(default)]
    pub speed_limit: Option<i64>,
    #[serde(default)]
    pub capacity_limit: Option<i64>,
    #[serde(default)]
    pub content: Option<String>,
    #[serde(default)]
    pub month_price: Option<i64>,
    #[serde(default)]
    pub quarter_price: Option<i64>,
    #[serde(default)]
    pub half_year_price: Option<i64>,
    #[serde(default)]
    pub year_price: Option<i64>,
    #[serde(default)]
    pub two_year_price: Option<i64>,
    #[serde(default)]
    pub three_year_price: Option<i64>,
    #[serde(default)]
    pub onetime_price: Option<i64>,
    #[serde(default)]
    pub reset_price: Option<i64>,
    #[serde(default)]
    pub reset_traffic_method: Option<i64>,
}

/// PATCH `plans/{id}` (§6.2): §4.4 partial update merging the legacy
/// `plan/update` show/renew toggles; `force_update` stays a body flag that
/// propagates the final plan limits to every subscribed user.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PlanPatch {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub group_id: Option<i64>,
    #[serde(default)]
    pub transfer_enable: Option<i64>,
    #[serde(default, with = "double_option")]
    pub device_limit: Option<Option<i64>>,
    #[serde(default, with = "double_option")]
    pub speed_limit: Option<Option<i64>>,
    #[serde(default, with = "double_option")]
    pub capacity_limit: Option<Option<i64>>,
    #[serde(default, with = "double_option")]
    pub content: Option<Option<String>>,
    #[serde(default, with = "double_option")]
    pub month_price: Option<Option<i64>>,
    #[serde(default, with = "double_option")]
    pub quarter_price: Option<Option<i64>>,
    #[serde(default, with = "double_option")]
    pub half_year_price: Option<Option<i64>>,
    #[serde(default, with = "double_option")]
    pub year_price: Option<Option<i64>>,
    #[serde(default, with = "double_option")]
    pub two_year_price: Option<Option<i64>>,
    #[serde(default, with = "double_option")]
    pub three_year_price: Option<Option<i64>>,
    #[serde(default, with = "double_option")]
    pub onetime_price: Option<Option<i64>>,
    #[serde(default, with = "double_option")]
    pub reset_price: Option<Option<i64>>,
    #[serde(default, with = "double_option")]
    pub reset_traffic_method: Option<Option<i64>>,
    #[serde(default)]
    pub show: Option<bool>,
    #[serde(default)]
    pub renew: Option<bool>,
    #[serde(default)]
    pub force_update: Option<bool>,
}

/// POST `plans/sort` / POST `payments/sort` (§6.2): JSON `{ids}` full
/// resequencing (the legacy `plan_ids` key becomes `ids` per §4.1).
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SortIdsRequest {
    pub ids: Vec<i64>,
}

/// One admin payment-method row (§6.2 `GET payments`): the legacy field set
/// with `handling_fee_percent` as a JSON number, bool `enable`, redacted
/// `config`, and §4.5 RFC 3339 timestamps.
#[derive(Debug, Serialize)]
pub struct AdminPaymentItem {
    pub id: i32,
    pub name: String,
    pub payment: String,
    pub icon: Option<String>,
    pub handling_fee_fixed: Option<i32>,
    pub handling_fee_percent: Option<f64>,
    pub uuid: String,
    pub config: Value,
    pub notify_domain: Option<String>,
    pub notify_url: String,
    pub enable: bool,
    pub sort: Option<i32>,
    #[serde(with = "rfc3339")]
    pub created_at: i64,
    #[serde(with = "rfc3339")]
    pub updated_at: i64,
    pub legacy_md5_signature: bool,
    pub security_warning: Option<&'static str>,
}

/// POST `payments` (§6.2): JSON body with the gateway config as a real
/// object. Created rows start disabled with NULL sort, as the legacy insert
/// did.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PaymentCreate {
    pub name: String,
    pub payment: String,
    pub config: Map<String, Value>,
    #[serde(default)]
    pub icon: Option<String>,
    #[serde(default)]
    pub notify_domain: Option<String>,
    #[serde(default)]
    pub handling_fee_fixed: Option<i64>,
    #[serde(default)]
    pub handling_fee_percent: Option<serde_json::Number>,
}

/// PATCH `payments/{id}` (§6.2): §4.4 double-Option replaces the legacy
/// present-but-empty=clear convention, and the legacy `payment/show` toggle
/// merges in as the explicit `enable` bool. `payment`/`config` may only be
/// echoed back unchanged — each payment row is an immutable verification
/// version (any real change is `payment_method_in_use`).
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PaymentPatch {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub payment: Option<String>,
    #[serde(default)]
    pub config: Option<Map<String, Value>>,
    #[serde(default, with = "double_option")]
    pub icon: Option<Option<String>>,
    #[serde(default, with = "double_option")]
    pub notify_domain: Option<Option<String>>,
    #[serde(default, with = "double_option")]
    pub handling_fee_fixed: Option<Option<i64>>,
    #[serde(default, with = "double_option")]
    pub handling_fee_percent: Option<Option<serde_json::Number>>,
    #[serde(default)]
    pub enable: Option<bool>,
}

/// PATCH `orders/{trade_no}` (§6.4): **exactly one** of the two fields must
/// be present — both or neither is 422 `validation_failed` (the legacy
/// client only ever sends one).
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OrderPatch {
    #[serde(default)]
    pub status: Option<i64>,
    #[serde(default)]
    pub commission_status: Option<i64>,
}

/// The single §6.4 order-PATCH assignment, resolved by
/// [`order_patch_action`].
#[derive(Debug, PartialEq, Eq)]
pub(super) enum OrderPatchAction {
    Status(i64),
    CommissionStatus(i64),
}

/// §6.4 exactly-one-field rule plus the legacy Laravel `in:` validations
/// (`status` in 0–3, `commission_status` in 0/1/3).
pub(super) fn order_patch_action(body: &OrderPatch) -> Result<OrderPatchAction, ApiError> {
    match (body.status, body.commission_status) {
        (Some(_), Some(_)) | (None, None) => Err(validation_error(
            "status",
            "Provide exactly one of status and commission_status",
        )),
        (Some(status), None) => {
            if !(0..=3).contains(&status) {
                return Err(validation_error("status", "销售状态格式不正确"));
            }
            Ok(OrderPatchAction::Status(status))
        }
        (None, Some(commission_status)) => {
            if !matches!(commission_status, 0 | 1 | 3) {
                return Err(validation_error("commission_status", "佣金状态格式不正确"));
            }
            Ok(OrderPatchAction::CommissionStatus(commission_status))
        }
    }
}

/// POST `orders` (§6.4): assigns an order to a user by email — the legacy
/// `order/assign` body as JSON, answered with a §1 201 bare `{trade_no}`.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OrderAssign {
    pub email: String,
    pub plan_id: i64,
    pub period: String,
    #[serde(default)]
    pub total_amount: Option<i64>,
}

/// POST `payment-reconciliations/{id}/resolve` (§6.4): the demultiplexed
/// legacy `order/update` + `reconciliation_id` arm.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReconciliationResolveRequest {
    pub resolution: String,
}

fn plan_create_validation(body: &PlanCreate) -> Result<(), ApiError> {
    if body.name.trim().is_empty() {
        return Err(validation_error("name", "name cannot be empty"));
    }
    nonnegative_i32("transfer_enable", body.transfer_enable)?;
    for (field, value) in [
        ("device_limit", body.device_limit),
        ("speed_limit", body.speed_limit),
        ("capacity_limit", body.capacity_limit),
        ("month_price", body.month_price),
        ("quarter_price", body.quarter_price),
        ("half_year_price", body.half_year_price),
        ("year_price", body.year_price),
        ("two_year_price", body.two_year_price),
        ("three_year_price", body.three_year_price),
        ("onetime_price", body.onetime_price),
        ("reset_price", body.reset_price),
    ] {
        optional_nonnegative_i32(field, value)?;
    }
    optional_smallint("reset_traffic_method", body.reset_traffic_method)?;
    Ok(())
}

pub(super) fn plan_patch_validation(body: &PlanPatch) -> Result<(), ApiError> {
    if let Some(name) = &body.name
        && name.trim().is_empty()
    {
        return Err(validation_error("name", "name cannot be empty"));
    }
    if let Some(transfer_enable) = body.transfer_enable {
        nonnegative_i32("transfer_enable", transfer_enable)?;
    }
    for (field, value) in [
        ("device_limit", &body.device_limit),
        ("speed_limit", &body.speed_limit),
        ("capacity_limit", &body.capacity_limit),
        ("month_price", &body.month_price),
        ("quarter_price", &body.quarter_price),
        ("half_year_price", &body.half_year_price),
        ("year_price", &body.year_price),
        ("two_year_price", &body.two_year_price),
        ("three_year_price", &body.three_year_price),
        ("onetime_price", &body.onetime_price),
        ("reset_price", &body.reset_price),
    ] {
        if let Some(update) = value {
            optional_nonnegative_i32(field, *update)?;
        }
    }
    if let Some(update) = &body.reset_traffic_method {
        optional_smallint("reset_traffic_method", *update)?;
    }
    Ok(())
}

impl AdminService {
    /// GET `plans` (§6.2): bare array — every plan, shown and hidden, with
    /// its active-user `count`, in the operator sort order.
    pub async fn plans_list(&self) -> Result<Vec<AdminPlanItem>, ApiError> {
        let plans = sqlx::query_as::<_, v2board_db::plan::PlanRow>(
            r#"
            SELECT id, group_id, transfer_enable, device_limit, name, speed_limit, "show", sort,
                   renew, content, month_price, quarter_price, half_year_price, year_price,
                   two_year_price, three_year_price, onetime_price, reset_price,
                   reset_traffic_method, capacity_limit, created_at, updated_at
            FROM plan
            ORDER BY sort ASC NULLS FIRST, id ASC
            "#,
        )
        .fetch_all(&self.db)
        .await?;
        let counts = v2board_db::plan::count_active_users_by_plan(&self.db).await?;
        Ok(plans
            .into_iter()
            .map(|plan| {
                let count = counts.get(&plan.id).copied().unwrap_or_default();
                AdminPlanItem {
                    id: plan.id,
                    group_id: plan.group_id,
                    transfer_enable: plan.transfer_enable,
                    device_limit: plan.device_limit,
                    name: plan.name,
                    speed_limit: plan.speed_limit,
                    show: plan.show != 0,
                    sort: plan.sort,
                    renew: plan.renew != 0,
                    content: plan.content,
                    month_price: plan.month_price,
                    quarter_price: plan.quarter_price,
                    half_year_price: plan.half_year_price,
                    year_price: plan.year_price,
                    two_year_price: plan.two_year_price,
                    three_year_price: plan.three_year_price,
                    onetime_price: plan.onetime_price,
                    reset_price: plan.reset_price,
                    reset_traffic_method: plan.reset_traffic_method,
                    capacity_limit: plan.capacity_limit,
                    count,
                    created_at: plan.created_at,
                    updated_at: plan.updated_at,
                }
            })
            .collect())
    }

    /// POST `plans` (§6.2) → the new id (a 201 `{id}` on the wire).
    pub async fn plan_create(&self, body: &PlanCreate) -> Result<i32, ApiError> {
        plan_create_validation(body)?;
        let now = Utc::now().timestamp();
        let mut tx = self.db.begin().await?;
        // Group writers use group -> user -> plan ordering. The shared parent
        // lock makes a concurrent group drop wait before the plan is created.
        lock_server_group_for_share(&mut tx, body.group_id).await?;
        let id: i32 = sqlx::query_scalar(
            r#"
            INSERT INTO plan (
                group_id, transfer_enable, device_limit, name, speed_limit,
                content, month_price, quarter_price, half_year_price, year_price,
                two_year_price, three_year_price, onetime_price, reset_price,
                reset_traffic_method, capacity_limit, created_at, updated_at
            )
            VALUES (
                CAST($1::BIGINT AS INTEGER), $2, CAST($3::BIGINT AS INTEGER), $4,
                CAST($5::BIGINT AS INTEGER), $6, CAST($7::BIGINT AS INTEGER),
                CAST($8::BIGINT AS INTEGER), CAST($9::BIGINT AS INTEGER),
                CAST($10::BIGINT AS INTEGER), CAST($11::BIGINT AS INTEGER),
                CAST($12::BIGINT AS INTEGER), CAST($13::BIGINT AS INTEGER),
                CAST($14::BIGINT AS INTEGER), CAST($15::BIGINT AS SMALLINT),
                CAST($16::BIGINT AS INTEGER), $17, $18
            )
            RETURNING id
            "#,
        )
        .bind(body.group_id)
        .bind(body.transfer_enable)
        .bind(body.device_limit)
        .bind(&body.name)
        .bind(body.speed_limit)
        .bind(&body.content)
        .bind(body.month_price)
        .bind(body.quarter_price)
        .bind(body.half_year_price)
        .bind(body.year_price)
        .bind(body.two_year_price)
        .bind(body.three_year_price)
        .bind(body.onetime_price)
        .bind(body.reset_price)
        .bind(body.reset_traffic_method)
        .bind(body.capacity_limit)
        .bind(now)
        .bind(now)
        .fetch_one(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(id)
    }

    /// PATCH `plans/{id}` (§6.2): §4.4 partial update over the PlanSave
    /// field set plus the merged show/renew toggles. `force_update` locks
    /// and repropagates the **final** plan limits (post-patch values, with
    /// untouched columns read from the current row) to every subscribed
    /// user, preserving the legacy group -> user -> plan lock ordering.
    pub async fn plan_patch(&self, id: i64, body: &PlanPatch) -> Result<(), ApiError> {
        plan_patch_validation(body)?;
        let force_update = body.force_update.unwrap_or(false);
        let now = Utc::now().timestamp();
        let mut tx = self.db.begin().await?;
        // The current values feed the group lock and the force propagation
        // when the body leaves them untouched. The plain read is safe: the
        // plan row itself is locked FOR UPDATE below before any write.
        #[derive(FromRow)]
        struct CurrentPlan {
            group_id: i32,
            transfer_enable: i64,
            device_limit: Option<i32>,
            speed_limit: Option<i32>,
        }
        let current = sqlx::query_as::<_, CurrentPlan>(
            "SELECT group_id, transfer_enable, device_limit, speed_limit \
             FROM plan WHERE id = $1 LIMIT 1",
        )
        .bind(id)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or_else(|| ApiError::from(Problem::new(Code::PlanNotFound)))?;
        let target_group = body.group_id.unwrap_or(i64::from(current.group_id));
        if body.group_id.is_some() || force_update {
            // Group writers use group -> user -> plan ordering. The shared
            // parent lock makes a concurrent group drop wait before either
            // the plan or its users can be changed.
            lock_server_group_for_share(&mut tx, target_group).await?;
        }
        if force_update {
            // Order lifecycle writers take user before plan. Acquire every
            // affected user in primary-key pages before the plan row so the
            // force propagation cannot invert that order or materialize an
            // unbounded id list.
            lock_plan_users_for_update(&mut tx, id).await?;
        }
        let locked: Option<i32> =
            sqlx::query_scalar("SELECT id FROM plan WHERE id = $1 LIMIT 1 FOR UPDATE")
                .bind(id)
                .fetch_optional(&mut *tx)
                .await?;
        if locked.is_none() {
            return Err(Problem::new(Code::PlanNotFound).into());
        }

        let mut values: Vec<(&str, AdminSqlValue)> = Vec::new();
        if let Some(name) = &body.name {
            values.push(("name", AdminSqlValue::Text(name.clone())));
        }
        if let Some(group_id) = body.group_id {
            values.push(("group_id", AdminSqlValue::Integer(group_id)));
        }
        if let Some(transfer_enable) = body.transfer_enable {
            values.push(("transfer_enable", AdminSqlValue::Integer(transfer_enable)));
        }
        for (column, field) in [
            ("device_limit", &body.device_limit),
            ("speed_limit", &body.speed_limit),
            ("capacity_limit", &body.capacity_limit),
            ("month_price", &body.month_price),
            ("quarter_price", &body.quarter_price),
            ("half_year_price", &body.half_year_price),
            ("year_price", &body.year_price),
            ("two_year_price", &body.two_year_price),
            ("three_year_price", &body.three_year_price),
            ("onetime_price", &body.onetime_price),
            ("reset_price", &body.reset_price),
            ("reset_traffic_method", &body.reset_traffic_method),
        ] {
            if let Some(update) = field {
                values.push((
                    column,
                    update.map_or(AdminSqlValue::IntegerNull, AdminSqlValue::Integer),
                ));
            }
        }
        if let Some(content) = &body.content {
            values.push((
                "content",
                content
                    .clone()
                    .map_or(AdminSqlValue::TextNull, AdminSqlValue::Text),
            ));
        }
        if let Some(show) = body.show {
            values.push(("show", AdminSqlValue::Integer(i64::from(show))));
        }
        if let Some(renew) = body.renew {
            values.push(("renew", AdminSqlValue::Integer(i64::from(renew))));
        }
        let mut builder = QueryBuilder::<Postgres>::new("UPDATE plan SET ");
        for (column, value) in &values {
            builder.push(format!("\"{column}\" = "));
            push_admin_sql_bind(&mut builder, column, value);
            builder.push(", ");
        }
        builder.push("\"updated_at\" = ");
        builder.push_bind(now);
        builder.push(" WHERE id = ");
        builder.push_bind(id);
        builder.build().execute(&mut *tx).await?;

        if force_update {
            let transfer_enable_bytes = checked_gib_bytes(
                body.transfer_enable.unwrap_or(current.transfer_enable),
                "transfer_enable",
            )?;
            let device_limit = match &body.device_limit {
                Some(update) => *update,
                None => current.device_limit.map(i64::from),
            };
            let speed_limit = match &body.speed_limit {
                Some(update) => *update,
                None => current.speed_limit.map(i64::from),
            };
            sqlx::query(
                r#"
                UPDATE users
                SET group_id = CAST($1::BIGINT AS INTEGER), transfer_enable = $2,
                    device_limit = CAST($3::BIGINT AS INTEGER),
                    speed_limit = CAST($4::BIGINT AS INTEGER), updated_at = $5
                WHERE plan_id = $6::BIGINT
                "#,
            )
            .bind(target_group)
            .bind(transfer_enable_bytes)
            .bind(device_limit)
            .bind(speed_limit)
            .bind(now)
            .bind(id)
            .execute(&mut *tx)
            .await?;
        }
        tx.commit().await?;
        Ok(())
    }

    /// DELETE `plans/{id}` (§6.2): rejects deletion while any order, user,
    /// or gift card still references the plan (400 `plan_in_use`, with the
    /// blocking dependency in `detail` per §3.4); a missing id is 404
    /// `plan_not_found`. One locking transaction, as the legacy drop.
    pub async fn plan_delete(&self, id: i64) -> Result<(), ApiError> {
        let mut tx = self.db.begin().await?;
        let has_order: Option<i64> = sqlx::query_scalar(
            "SELECT id FROM orders WHERE referenced_plan_id = $1 LIMIT 1 FOR UPDATE",
        )
        .bind(id)
        .fetch_optional(&mut *tx)
        .await?;
        if has_order.is_some() {
            return Err(Problem::new(Code::PlanInUse)
                .with_detail("该订阅下存在订单无法删除")
                .into());
        }
        let has_user: Option<i64> =
            sqlx::query_scalar("SELECT id FROM users WHERE plan_id = $1 LIMIT 1 FOR UPDATE")
                .bind(id)
                .fetch_optional(&mut *tx)
                .await?;
        if has_user.is_some() {
            return Err(Problem::new(Code::PlanInUse)
                .with_detail("该订阅下存在用户无法删除")
                .into());
        }
        let has_giftcard: Option<i32> =
            sqlx::query_scalar("SELECT id FROM gift_card WHERE plan_id = $1 LIMIT 1 FOR UPDATE")
                .bind(id)
                .fetch_optional(&mut *tx)
                .await?;
        if has_giftcard.is_some() {
            return Err(Problem::new(Code::PlanInUse)
                .with_detail("该订阅仍被礼品卡使用，无法删除")
                .into());
        }
        let exists: Option<i32> =
            sqlx::query_scalar("SELECT id FROM plan WHERE id = $1 LIMIT 1 FOR UPDATE")
                .bind(id)
                .fetch_optional(&mut *tx)
                .await?;
        if exists.is_none() {
            return Err(Problem::new(Code::PlanNotFound).into());
        }
        let deleted = sqlx::query("DELETE FROM plan WHERE id = $1")
            .bind(id)
            .execute(&mut *tx)
            .await?;
        if deleted.rows_affected() != 1 {
            return Err(Problem::new(Code::PlanNotFound).into());
        }
        tx.commit().await?;
        Ok(())
    }

    /// POST `plans/sort` (§6.2): JSON `{ids}` full resequencing; empty 204.
    pub async fn plans_sort(&self, ids: &[i64]) -> Result<(), ApiError> {
        self.sort_ids("plan", ids).await
    }

    fn require_app_url(&self) -> Result<(), ApiError> {
        if self
            .config
            .app_url
            .as_deref()
            .unwrap_or_default()
            .is_empty()
        {
            return Err(Problem::new(Code::AppUrlNotConfigured).into());
        }
        Ok(())
    }

    /// GET `payments` (§6.2): bare array of active (non-archived) payment
    /// methods with redacted configs and composed notify URLs.
    pub async fn payments_list(&self) -> Result<Vec<AdminPaymentItem>, ApiError> {
        let rows = sqlx::query_as::<_, PaymentRow>(
            r#"
            SELECT id, name, payment, icon, handling_fee_fixed,
                   CAST(handling_fee_percent AS DOUBLE PRECISION) AS handling_fee_percent,
                   uuid, CAST(config AS TEXT) AS config, notify_domain, enable, sort,
                   created_at, updated_at
            FROM payment_method
            WHERE archived_at IS NULL
            ORDER BY sort ASC NULLS FIRST, id ASC
            "#,
        )
        .fetch_all(&self.db)
        .await?;
        rows.into_iter()
            .map(|row| {
                let config = crate::payment_provider::redact_payment_config(
                    &row.payment,
                    &parse_payment_config(&row.config)?,
                );
                let notify_path =
                    format!("/api/v1/guest/payment/notify/{}/{}", row.payment, row.uuid);
                let notify_url = if let Some(domain) = row
                    .notify_domain
                    .as_deref()
                    .filter(|value| !value.is_empty())
                {
                    format!("{}{}", domain.trim_end_matches('/'), notify_path)
                } else if let Some(app_url) = self
                    .config
                    .app_url
                    .as_deref()
                    .filter(|value| !value.is_empty())
                {
                    format!("{}{}", app_url.trim_end_matches('/'), notify_path)
                } else {
                    notify_path
                };
                Ok(AdminPaymentItem {
                    id: row.id,
                    legacy_md5_signature: crate::payment_provider::payment_provider_uses_legacy_md5(
                        &row.payment,
                    ),
                    security_warning: crate::payment_provider::payment_provider_security_warning(
                        &row.payment,
                    ),
                    name: row.name,
                    payment: row.payment,
                    icon: row.icon,
                    handling_fee_fixed: row.handling_fee_fixed,
                    handling_fee_percent: row.handling_fee_percent,
                    uuid: row.uuid,
                    config,
                    notify_domain: row.notify_domain,
                    notify_url,
                    enable: row.enable != 0,
                    sort: row.sort,
                    created_at: row.created_at,
                    updated_at: row.updated_at,
                })
            })
            .collect()
    }

    /// GET `payment-providers/{code}/form` `?payment_id=` (§6.2): the
    /// provider's form template, prefilled with the redacted stored config
    /// when `payment_id` names an active row of the same driver. The read
    /// moved off POST (recorded §6 decision) — the response stays
    /// server-redacted.
    pub async fn payment_provider_form_view(
        &self,
        code: &str,
        payment_id: Option<i64>,
    ) -> Result<Value, ApiError> {
        let config = if let Some(id) = payment_id {
            let (stored_payment, raw_config) = sqlx::query_as::<_, (String, String)>(
                "SELECT payment, CAST(config AS TEXT) FROM payment_method \
                 WHERE id = $1 AND archived_at IS NULL LIMIT 1",
            )
            .bind(id)
            .fetch_optional(&self.db)
            .await?
            .ok_or_else(|| ApiError::from(Problem::new(Code::PaymentMethodNotFound)))?;
            if stored_payment == code {
                Some(crate::payment_provider::redact_payment_config(
                    &stored_payment,
                    &parse_payment_config(&raw_config)?,
                ))
            } else {
                None
            }
        } else {
            None
        };
        Ok(payment_provider_form(code, config.as_ref()))
    }

    /// POST `payments` (§6.2) → the new id (a 201 `{id}` on the wire). The
    /// legacy Chinese validation literals stay as the 422 field messages; a
    /// missing site URL is 400 `app_url_not_configured`.
    pub async fn payment_create(&self, body: &PaymentCreate) -> Result<i32, ApiError> {
        self.require_app_url()?;
        if body.name.trim().is_empty() {
            return Err(validation_error("name", "显示名称不能为空"));
        }
        if body.payment.trim().is_empty() {
            return Err(validation_error("payment", "网关参数不能为空"));
        }
        if body.config.is_empty() {
            return Err(validation_error("config", "配置参数不能为空"));
        }
        if let Some(domain) = body.notify_domain.as_deref()
            && !is_valid_url(domain)
        {
            return Err(validation_error("notify_domain", "自定义通知域名格式有误"));
        }
        let handling_fee_fixed =
            optional_nonnegative_i32("handling_fee_fixed", body.handling_fee_fixed)?;
        let handling_fee_percent =
            handling_fee_percent_decimal(body.handling_fee_percent.as_ref())?;
        let config_value = resolve_redacted_payment_config(
            &body.payment,
            None,
            Value::Object(body.config.clone()),
        )?;
        if crate::payment_provider::payment_provider_uses_legacy_md5(&body.payment) {
            tracing::warn!(
                provider = body.payment,
                "administrator saved a legacy MD5 payment provider; HTTPS and migration are strongly recommended"
            );
        }
        let now = Utc::now().timestamp();
        let id: i32 = sqlx::query_scalar(
            r#"
            INSERT INTO payment_method (
                name, icon, payment, uuid, config, notify_domain, handling_fee_fixed,
                handling_fee_percent, enable, created_at, updated_at
            )
            VALUES ($1, $2, $3, $4, $5, $6, CAST($7::BIGINT AS INTEGER), $8, 0, $9, $9)
            RETURNING id
            "#,
        )
        .bind(&body.name)
        .bind(&body.icon)
        .bind(&body.payment)
        .bind(random_payment_uuid())
        .bind(Json(config_value))
        .bind(&body.notify_domain)
        .bind(handling_fee_fixed)
        .bind(handling_fee_percent)
        .bind(now)
        .fetch_one(&self.db)
        .await?;
        Ok(id)
    }

    /// PATCH `payments/{id}` (§6.2): §4.4 metadata update (name/icon/
    /// notify_domain/fees) plus the merged explicit `enable` flag. The
    /// gateway driver and key material are an immutable verification
    /// version: `payment`/`config` may only be echoed back unchanged — any
    /// real change is 400 `payment_method_in_use` (archive and recreate).
    pub async fn payment_patch(&self, id: i64, body: &PaymentPatch) -> Result<(), ApiError> {
        self.require_app_url()?;
        if let Some(name) = &body.name
            && name.trim().is_empty()
        {
            return Err(validation_error("name", "显示名称不能为空"));
        }
        if let Some(Some(domain)) = &body.notify_domain
            && !is_valid_url(domain)
        {
            return Err(validation_error("notify_domain", "自定义通知域名格式有误"));
        }
        let handling_fee_fixed = match body.handling_fee_fixed {
            Some(update) => Some(optional_nonnegative_i32("handling_fee_fixed", update)?),
            None => None,
        };
        let handling_fee_percent = match &body.handling_fee_percent {
            Some(update) => Some(handling_fee_percent_decimal(update.as_ref())?),
            None => None,
        };
        let now = Utc::now().timestamp();
        let mut tx = self.db.begin().await?;
        let current = sqlx::query_as::<_, (String, String)>(
            "SELECT payment, CAST(config AS TEXT) FROM payment_method \
             WHERE id = $1 AND archived_at IS NULL LIMIT 1 FOR UPDATE",
        )
        .bind(id)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or_else(|| ApiError::from(Problem::new(Code::PaymentMethodNotFound)))?;
        let driver_changed = body
            .payment
            .as_deref()
            .is_some_and(|payment| payment != current.0);
        let config_changed = match &body.config {
            Some(config) => {
                let resolved = resolve_redacted_payment_config(
                    &current.0,
                    Some((&current.0, &current.1)),
                    Value::Object(config.clone()),
                )?;
                serde_json::from_str::<Value>(&current.1)
                    .map(|value| value != resolved)
                    .unwrap_or(true)
            }
            None => false,
        };
        if payment_verification_version_blocks_update(driver_changed, config_changed) {
            return Err(Problem::new(Code::PaymentMethodInUse)
                .with_detail(
                    "支付方式是不可变验签版本，网关类型和密钥配置不可原地修改；请归档后新建支付方式",
                )
                .into());
        }
        let mut builder = QueryBuilder::<Postgres>::new("UPDATE payment_method SET ");
        if let Some(name) = &body.name {
            builder.push("name = ");
            builder.push_bind(name.clone());
            builder.push(", ");
        }
        if let Some(icon) = &body.icon {
            builder.push("icon = ");
            builder.push_bind(icon.clone());
            builder.push(", ");
        }
        if let Some(notify_domain) = &body.notify_domain {
            builder.push("notify_domain = ");
            builder.push_bind(notify_domain.clone());
            builder.push(", ");
        }
        if let Some(update) = handling_fee_fixed {
            builder.push("handling_fee_fixed = CAST(");
            builder.push_bind(update);
            builder.push(" AS INTEGER), ");
        }
        if let Some(update) = handling_fee_percent {
            builder.push("handling_fee_percent = ");
            builder.push_bind(update);
            builder.push(", ");
        }
        if let Some(enable) = body.enable {
            builder.push("enable = ");
            builder.push_bind(i16::from(enable));
            builder.push(", ");
        }
        builder.push("updated_at = ");
        builder.push_bind(now);
        builder.push(" WHERE id = ");
        builder.push_bind(id);
        builder.push(" AND archived_at IS NULL");
        builder.build().execute(&mut *tx).await?;
        tx.commit().await?;
        Ok(())
    }

    /// DELETE `payments/{id}` (§6.2): soft archive — the immutable
    /// verification version stays reachable for delayed callbacks; ordinary
    /// reads hide it. A missing/already-archived id is 404
    /// `payment_method_not_found`.
    pub async fn payment_delete(&self, id: i64) -> Result<(), ApiError> {
        let mut tx = self.db.begin().await?;
        let exists: Option<i32> = sqlx::query_scalar(
            "SELECT id FROM payment_method \
                 WHERE id = $1 AND archived_at IS NULL LIMIT 1 FOR UPDATE",
        )
        .bind(id)
        .fetch_optional(&mut *tx)
        .await?;
        if exists.is_none() {
            return Err(Problem::new(Code::PaymentMethodNotFound).into());
        }
        let now = Utc::now().timestamp();
        sqlx::query(
            "UPDATE payment_method \
             SET enable = 0, archived_at = COALESCE(archived_at, $1), updated_at = $2 \
             WHERE id = $3 AND archived_at IS NULL",
        )
        .bind(now)
        .bind(now)
        .bind(id)
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(())
    }

    /// POST `payments/sort` (§6.2): JSON `{ids}` full resequencing.
    pub async fn payments_sort(&self, ids: &[i64]) -> Result<(), ApiError> {
        let mut tx = self.db.begin().await?;
        for (index, id) in ids.iter().enumerate() {
            sqlx::query(
                "UPDATE payment_method SET sort = CAST($1::BIGINT AS INTEGER), updated_at = $2 \
                 WHERE id = $3::BIGINT AND archived_at IS NULL",
            )
            .bind((index + 1) as i64)
            .bind(Utc::now().timestamp())
            .bind(id)
            .execute(&mut *tx)
            .await?;
        }
        tx.commit().await?;
        Ok(())
    }

    /// GET `orders` (§6.4): §8 pagination, the §7 DSL over the guarded
    /// order-column whitelist, §7.2 sort, and the `?commission_only=` bool
    /// scope (the legacy truthy `is_commission`). Rows keep the legacy jsonb
    /// projection with §4.5 RFC 3339 timestamps.
    pub async fn orders_list(
        &self,
        pagination: Pagination,
        filter: Option<&str>,
        sort_by: Option<&str>,
        sort_dir: Option<&str>,
        commission_only: bool,
    ) -> Result<(Vec<Value>, i64), ApiError> {
        let clauses = filter
            .map(filter_dsl::parse_filter_param)
            .transpose()?
            .unwrap_or_default();
        let filters = filter_dsl::resolve_filters(&clauses, ORDER_FILTER_COLUMNS)?;
        let sort = filter_dsl::resolve_sort(sort_by, sort_dir, ORDER_SORT_COLUMNS.as_slice())?;

        let mut count_builder =
            QueryBuilder::<Postgres>::new("SELECT COUNT(*) FROM orders o WHERE 1 = 1");
        push_commission_scope(&mut count_builder, commission_only);
        filter_dsl::push_filter_where(&mut count_builder, &filters);
        let total: i64 = count_builder
            .build_query_scalar()
            .fetch_one(&self.db)
            .await?;

        let mut builder = QueryBuilder::<Postgres>::new(
            r#"
            SELECT jsonb_build_object(
                'id', o.id, 'invite_user_id', o.invite_user_id, 'user_id', o.user_id,
                'email', u.email, 'plan_id', o.plan_id, 'plan_name', p.name, 'coupon_id', o.coupon_id,
                'type', o.type, 'period', o.period, 'trade_no', o.trade_no,
                'callback_no', o.callback_no, 'total_amount', o.total_amount,
                'handling_amount', o.handling_amount, 'discount_amount', o.discount_amount,
                'surplus_amount', o.surplus_amount, 'refund_amount', o.refund_amount,
                'balance_amount', o.balance_amount, 'surplus_order_ids', CAST(o.surplus_order_ids AS JSONB),
                'status', o.status, 'commission_status', o.commission_status,
                'commission_balance', o.commission_balance,
                'actual_commission_balance', o.actual_commission_balance,
                'payment_id', o.payment_id,
                'payment_reconciliation_open_count', (
                    SELECT COUNT(*) FROM payment_reconciliation r
                    WHERE r.trade_no_hash = sha256(convert_to(o.trade_no, 'UTF8'))
                      AND r.resolved_at IS NULL
                ),
                'paid_at', o.paid_at, 'created_at', o.created_at, 'updated_at', o.updated_at
            )
            FROM orders o
            LEFT JOIN users u ON u.id = o.user_id
            LEFT JOIN plan p ON p.id = o.plan_id
            WHERE 1 = 1
            "#,
        );
        push_commission_scope(&mut builder, commission_only);
        filter_dsl::push_filter_where(&mut builder, &filters);
        builder.push(format!(" ORDER BY {}, o.id DESC LIMIT ", sort.order_by()));
        builder.push_bind(pagination.limit());
        builder.push(" OFFSET ");
        builder.push_bind(pagination.offset());
        let items = builder
            .build_query_scalar::<Json<Value>>()
            .fetch_all(&self.db)
            .await?
            .into_iter()
            .map(|row| {
                statistics::epoch_fields_to_rfc3339(row.0, &["paid_at", "created_at", "updated_at"])
            })
            .collect();
        Ok((items, total))
    }

    /// GET `orders/{trade_no}` (§6.4): bare detail — `trade_no` replaces the
    /// legacy numeric-id lookup, and the read left the blanket POST step-up
    /// gate (recorded §6 decision). Always attaches `commission_log` and
    /// `payment_reconciliations`; `surplus_orders` only when
    /// `surplus_order_ids` is a non-empty array, as the legacy detail did.
    pub async fn order_detail(&self, trade_no: &str) -> Result<Value, ApiError> {
        let mut value = sqlx::query_scalar::<_, Json<Value>>(
            r#"
            SELECT jsonb_build_object(
                'id', o.id, 'invite_user_id', o.invite_user_id, 'user_id', o.user_id,
                'plan_id', o.plan_id, 'coupon_id', o.coupon_id, 'type', o.type, 'period', o.period,
                'trade_no', o.trade_no, 'callback_no', o.callback_no, 'total_amount', o.total_amount,
                'handling_amount', o.handling_amount, 'discount_amount', o.discount_amount,
                'surplus_amount', o.surplus_amount, 'refund_amount', o.refund_amount,
                'balance_amount', o.balance_amount, 'surplus_order_ids', CAST(o.surplus_order_ids AS JSONB),
                'status', o.status, 'commission_status', o.commission_status,
                'commission_balance', o.commission_balance,
                'actual_commission_balance', o.actual_commission_balance,
                'payment_id', o.payment_id,
                'paid_at', o.paid_at, 'created_at', o.created_at, 'updated_at', o.updated_at
            )
            FROM orders o
            WHERE o.trade_no = $1
            LIMIT 1
            "#,
        )
        .bind(trade_no)
        .fetch_optional(&self.db)
        .await?
        .map(|row| row.0)
        .ok_or_else(|| ApiError::from(Problem::new(Code::OrderNotFound)))?;

        let trade_no_hash = payment_reconciliation_identity_hash(trade_no);
        let commission_rows = sqlx::query_scalar::<_, Json<Value>>(
            r#"
            SELECT jsonb_build_object(
                'id', id, 'invite_user_id', invite_user_id, 'user_id', user_id,
                'trade_no', trade_no, 'order_amount', order_amount, 'get_amount', get_amount,
                'created_at', created_at, 'updated_at', updated_at
            )
            FROM commission_log
            WHERE trade_no = $1
            "#,
        )
        .bind(trade_no)
        .fetch_all(&self.db)
        .await?;
        let commission_log = json_rows(commission_rows)
            .into_iter()
            .map(|row| statistics::epoch_fields_to_rfc3339(row, &["created_at", "updated_at"]))
            .collect::<Vec<_>>();
        let reconciliation_rows = sqlx::query_scalar::<_, Json<Value>>(
            r#"
            SELECT jsonb_build_object(
                'id', id, 'payment_id', payment_id, 'provider', provider,
                'trade_no', trade_no, 'trade_no_hash', encode(trade_no_hash, 'hex'),
                'callback_no', callback_no, 'callback_no_hash', encode(callback_no_hash, 'hex'),
                'reason', reason,
                'order_status', order_status, 'expected_amount', expected_amount,
                'settled_amount', settled_amount, 'occurrence_count', occurrence_count,
                'first_seen_at', first_seen_at, 'last_seen_at', last_seen_at,
                'resolved_at', resolved_at, 'resolution', resolution
            )
            FROM payment_reconciliation
            WHERE trade_no_hash = $1
            ORDER BY first_seen_at DESC, id DESC
            "#,
        )
        .bind(trade_no_hash.as_slice())
        .fetch_all(&self.db)
        .await?;
        let payment_reconciliations = json_rows(reconciliation_rows)
            .into_iter()
            .map(|row| {
                statistics::epoch_fields_to_rfc3339(
                    row,
                    &["first_seen_at", "last_seen_at", "resolved_at"],
                )
            })
            .collect::<Vec<_>>();

        // surplus_orders is attached only when surplus_order_ids is a non-empty
        // array (PHP `if ($order->surplus_order_ids)` on the array cast).
        let surplus_ids: Vec<i64> = value
            .get("surplus_order_ids")
            .and_then(Value::as_array)
            .map(|items| items.iter().filter_map(Value::as_i64).collect())
            .unwrap_or_default();
        let attach_surplus = value
            .get("surplus_order_ids")
            .and_then(Value::as_array)
            .is_some_and(|items| !items.is_empty());
        let surplus_orders = if attach_surplus {
            let rows = if surplus_ids.is_empty() {
                Vec::new()
            } else {
                let mut builder = QueryBuilder::<Postgres>::new(
                    r#"
                    SELECT jsonb_build_object(
                        'id', o.id, 'invite_user_id', o.invite_user_id, 'user_id', o.user_id,
                        'plan_id', o.plan_id, 'coupon_id', o.coupon_id, 'type', o.type, 'period', o.period,
                        'trade_no', o.trade_no, 'callback_no', o.callback_no, 'total_amount', o.total_amount,
                        'handling_amount', o.handling_amount, 'discount_amount', o.discount_amount,
                        'surplus_amount', o.surplus_amount, 'refund_amount', o.refund_amount,
                        'balance_amount', o.balance_amount, 'surplus_order_ids', CAST(o.surplus_order_ids AS JSONB),
                        'status', o.status, 'commission_status', o.commission_status,
                        'commission_balance', o.commission_balance,
                        'actual_commission_balance', o.actual_commission_balance,
                        'payment_id', o.payment_id,
                        'paid_at', o.paid_at, 'created_at', o.created_at, 'updated_at', o.updated_at
                    )
                    FROM orders o
                    WHERE o.id IN ("#,
                );
                {
                    let mut separated = builder.separated(", ");
                    for surplus_id in &surplus_ids {
                        separated.push_bind(*surplus_id);
                    }
                }
                builder.push(")");
                let rows = builder
                    .build_query_scalar::<Json<Value>>()
                    .fetch_all(&self.db)
                    .await?;
                json_rows(rows)
                    .into_iter()
                    .map(|row| {
                        statistics::epoch_fields_to_rfc3339(
                            row,
                            &["paid_at", "created_at", "updated_at"],
                        )
                    })
                    .collect()
            };
            Some(rows)
        } else {
            None
        };

        value =
            statistics::epoch_fields_to_rfc3339(value, &["paid_at", "created_at", "updated_at"]);
        if let Some(object) = value.as_object_mut() {
            object.insert("commission_log".to_string(), Value::Array(commission_log));
            object.insert(
                "payment_reconciliations".to_string(),
                Value::Array(payment_reconciliations),
            );
            if let Some(surplus_orders) = surplus_orders {
                object.insert("surplus_orders".to_string(), Value::Array(surplus_orders));
            }
        }
        Ok(value)
    }

    /// PATCH `orders/{trade_no}` (§6.4): exactly one of `status` /
    /// `commission_status`; a missing trade_no is 404 `order_not_found`.
    pub async fn order_patch(&self, trade_no: &str, body: &OrderPatch) -> Result<(), ApiError> {
        let updated = match order_patch_action(body)? {
            OrderPatchAction::Status(status) => sqlx::query(
                "UPDATE orders SET status = CAST($1::BIGINT AS SMALLINT), updated_at = $2 \
                 WHERE trade_no = $3",
            )
            .bind(status)
            .bind(Utc::now().timestamp())
            .bind(trade_no)
            .execute(&self.db)
            .await?,
            OrderPatchAction::CommissionStatus(commission_status) => sqlx::query(
                "UPDATE orders SET commission_status = CAST($1::BIGINT AS SMALLINT), updated_at = $2 \
                 WHERE trade_no = $3",
            )
            .bind(commission_status)
            .bind(Utc::now().timestamp())
            .bind(trade_no)
            .execute(&self.db)
            .await?,
        };
        if updated.rows_affected() == 0 {
            return Err(Problem::new(Code::OrderNotFound).into());
        }
        Ok(())
    }

    /// POST `orders/{trade_no}/mark-paid` (§6.4): manual settlement through
    /// the shared order lifecycle.
    pub async fn order_mark_paid(&self, trade_no: &str) -> Result<(), ApiError> {
        OrderService::new(self.db.clone(), self.config.clone())
            .paid_manually(trade_no)
            .await
    }

    /// POST `orders/{trade_no}/cancel` (§6.4). Ports OrderService::cancel:
    /// only pending orders can be cancelled (400 `order_not_pending`), and
    /// the balance paid toward the order is refunded to the user.
    pub async fn order_cancel(&self, trade_no: &str) -> Result<(), ApiError> {
        let order: (i16, i64, Option<i64>, Option<i32>, Option<String>) = sqlx::query_as(
            r#"
            SELECT status, user_id, balance_amount::BIGINT, payment_id, callback_no
            FROM orders
            WHERE trade_no = $1
            LIMIT 1
            "#,
        )
        .bind(trade_no)
        .fetch_optional(&self.db)
        .await?
        .ok_or_else(|| ApiError::from(Problem::new(Code::OrderNotFound)))?;
        let (status, user_id, balance_amount, payment_id, callback_no) = order;
        if status != 0 {
            return Err(Problem::new(Code::OrderNotPending).into());
        }
        let order_service = OrderService::new(self.db.clone(), self.config.clone());
        if !order_service
            .cancel_stripe_intent_binding(payment_id, callback_no.as_deref())
            .await?
        {
            return Err(Problem::new(Code::OrderNotPending).into());
        }
        let now = Utc::now().timestamp();
        let mut tx = self.db.begin().await?;
        let updated = sqlx::query(
            r#"
            UPDATE orders SET status = 2, updated_at = $1
            WHERE trade_no = $2 AND status = 0
              AND payment_id IS NOT DISTINCT FROM $3
              AND callback_no IS NOT DISTINCT FROM $4
            "#,
        )
        .bind(now)
        .bind(trade_no)
        .bind(payment_id)
        .bind(&callback_no)
        .execute(&mut *tx)
        .await?;
        if updated.rows_affected() != 1 {
            return Err(Problem::new(Code::OrderNotPending).into());
        }
        if let Some(balance) = balance_amount.filter(|value| *value != 0) {
            // UserService::addBalance: lock the row, add, and reject a negative result.
            let current: i32 =
                sqlx::query_scalar("SELECT balance FROM users WHERE id = $1 FOR UPDATE")
                    .bind(user_id)
                    .fetch_optional(&mut *tx)
                    .await?
                    .ok_or_else(|| ApiError::from(Problem::new(Code::OrderUpdateFailed)))?;
            let updated = i64::from(current)
                .checked_add(balance)
                .ok_or_else(|| ApiError::from(Problem::new(Code::OrderUpdateFailed)))?;
            if !(0..=i64::from(i32::MAX)).contains(&updated) {
                return Err(Problem::new(Code::OrderUpdateFailed).into());
            }
            sqlx::query("UPDATE users SET balance = $1, updated_at = $2 WHERE id = $3")
                .bind(
                    i32::try_from(updated)
                        .map_err(|_| ApiError::from(Problem::new(Code::OrderUpdateFailed)))?,
                )
                .bind(now)
                .bind(user_id)
                .execute(&mut *tx)
                .await?;
        }
        tx.commit().await?;
        Ok(())
    }

    /// POST `orders` (§6.4): assign an order to a user → the new
    /// `trade_no` (a 201 bare `{trade_no}` on the wire). An unknown email
    /// is 400 `user_not_registered`, a missing plan 400 `plan_unavailable`,
    /// an unfinished order 400 `order_assign_conflict`.
    pub async fn order_assign(&self, body: &OrderAssign) -> Result<String, ApiError> {
        if body.period.trim().is_empty() {
            return Err(validation_error("period", "period cannot be empty"));
        }
        let total_amount =
            optional_nonnegative_i32("total_amount", body.total_amount)?.unwrap_or_default();
        // Resolve the stable key before entering the locking transaction.  The
        // row is loaded again only after the user's unfinished-order range has
        // been locked, preserving the global order -> user -> plan sequence.
        let user_id: Option<i64> = sqlx::query_scalar(
            "SELECT id FROM users WHERE lower(btrim(email)) = lower(btrim($1)) LIMIT 1",
        )
        .bind(&body.email)
        .fetch_optional(&self.db)
        .await?;
        let user_id =
            user_id.ok_or_else(|| ApiError::from(Problem::new(Code::UserNotRegistered)))?;
        let mut tx = self.db.begin().await?;
        let has_incomplete: Option<i64> = sqlx::query_scalar(ADMIN_ASSIGN_UNFINISHED_ORDER_SQL)
            .bind(user_id)
            .fetch_optional(&mut *tx)
            .await?;
        if has_incomplete.is_some() {
            return Err(Problem::new(Code::OrderAssignConflict).into());
        }

        // Load the fields setInvite / setOrderType need alongside the id:
        // (id, plan_id, expired_at, invite_user_id).
        type AssignUserRow = (i64, Option<i64>, Option<i64>, Option<i64>);
        let user: Option<AssignUserRow> = sqlx::query_as(
            "SELECT id, plan_id::bigint, expired_at, invite_user_id FROM users WHERE id = $1 LIMIT 1 FOR UPDATE",
        )
        .bind(user_id)
        .fetch_optional(&mut *tx)
        .await?;
        let (user_id, user_plan_id, user_expired_at, user_invite_user_id) =
            user.ok_or_else(|| ApiError::from(Problem::new(Code::UserNotRegistered)))?;
        let plan_exists: Option<i32> =
            sqlx::query_scalar("SELECT id FROM plan WHERE id = $1 LIMIT 1 FOR SHARE")
                .bind(body.plan_id)
                .fetch_optional(&mut *tx)
                .await?;
        if plan_exists.is_none() {
            return Err(Problem::new(Code::PlanUnavailable).into());
        }
        let now = Utc::now().timestamp();
        // OrderController::assign order-type branches (:167-175).
        let order_type: i64 = if body.period == "reset_price" {
            4
        } else if user_plan_id.is_some() && user_plan_id != Some(body.plan_id) {
            3
        } else if user_expired_at.is_some_and(|value| value > now)
            && user_plan_id == Some(body.plan_id)
        {
            2
        } else {
            1
        };
        // OrderService::setInvite (:138-165): resolve invite_user_id + commission_balance.
        let (invite_user_id, commission_balance) = self
            .assign_invite_in_tx(&mut tx, user_id, user_invite_user_id, total_amount)
            .await?;
        let trade_no = crate::order::generate_order_no();
        sqlx::query(
            r#"
            INSERT INTO orders (
                user_id, invite_user_id, plan_id, period, trade_no, total_amount, type,
                status, commission_status, commission_balance, created_at, updated_at
            )
            VALUES (
                $1, $2, CAST($3::BIGINT AS INTEGER), $4, $5,
                CAST($6::BIGINT AS INTEGER), CAST($7::BIGINT AS INTEGER),
                0, 0, CAST($8::BIGINT AS INTEGER), $9, $10
            )
            "#,
        )
        .bind(user_id)
        .bind(invite_user_id)
        .bind(body.plan_id)
        .bind(&body.period)
        .bind(&trade_no)
        .bind(total_amount)
        .bind(order_type)
        .bind(commission_balance)
        .bind(now)
        .bind(now)
        .execute(&mut *tx)
        .await
        .map_err(map_admin_order_write_error)?;
        tx.commit().await.map_err(map_admin_order_write_error)?;
        Ok(trade_no)
    }

    /// Ports OrderService::setInvite (:138-165) for the assign flow. Returns the
    /// order's `(invite_user_id, commission_balance)`. A referred user whose order
    /// is free keeps no invite link; otherwise the inviter's commission_type and
    /// commission_rate (falling back to config invite_commission) decide the cut.
    async fn assign_invite_in_tx(
        &self,
        tx: &mut DbTransaction<'_>,
        user_id: i64,
        user_invite_user_id: Option<i64>,
        total_amount: i64,
    ) -> Result<(Option<i64>, i64), ApiError> {
        // Laravel `setInvite`: `if ($user->invite_user_id && $order->total_amount <= 0) return;`
        // — invite_user_id is PHP-truthy only when non-null AND non-zero, so a stored 0 does
        // NOT short-circuit; it flows through and is recorded on the order (the inviter lookup
        // for id 0 then finds nothing), matching the missing-inviter branch below.
        if user_invite_user_id.is_some_and(|value| value != 0) && total_amount <= 0 {
            return Ok((None, 0));
        }
        let Some(inviter_id) = user_invite_user_id else {
            return Ok((None, 0));
        };
        let inviter: Option<(i16, Option<i32>)> = sqlx::query_as(
            "SELECT commission_type, commission_rate FROM users WHERE id = $1 LIMIT 1",
        )
        .bind(inviter_id)
        .fetch_optional(&mut **tx)
        .await?;
        let Some((commission_type, commission_rate)) = inviter else {
            // invite_user_id is still recorded even when the inviter is gone.
            return Ok((Some(inviter_id), 0));
        };
        let is_commission = match commission_type {
            0 => {
                !self.config.commission_first_time_enable
                    || !Self::user_have_valid_order_in_tx(tx, user_id).await?
            }
            1 => true,
            2 => !Self::user_have_valid_order_in_tx(tx, user_id).await?,
            _ => false,
        };
        if !is_commission {
            return Ok((Some(inviter_id), 0));
        }
        let commission_balance = i64::from(commission_amount_cents(
            total_amount,
            commission_rate,
            self.config.invite_commission,
        )?);
        Ok((Some(inviter_id), commission_balance))
    }

    /// OrderService::haveValidOrder: the user has any order whose status is not in
    /// {0 pending, 2 cancelled}.
    async fn user_have_valid_order_in_tx(
        tx: &mut DbTransaction<'_>,
        user_id: i64,
    ) -> Result<bool, ApiError> {
        let found: Option<i64> = sqlx::query_scalar(
            "SELECT id FROM orders WHERE user_id = $1 AND status NOT IN (0, 2) LIMIT 1",
        )
        .bind(user_id)
        .fetch_optional(&mut **tx)
        .await?;
        Ok(found.is_some())
    }
}

/// §6.4 `?commission_only=` scope on an order builder aliased `o` (the
/// legacy `is_commission` truthy filter).
fn push_commission_scope(builder: &mut QueryBuilder<Postgres>, commission_only: bool) {
    if commission_only {
        builder.push(
            " AND o.invite_user_id IS NOT NULL AND o.status NOT IN (0, 2) AND o.commission_balance > 0",
        );
    }
}

impl AdminService {
    /// GET `payment-reconciliations` (§6.4): the step-up-gated global
    /// ledger. Keeps its dedicated named scalar params — `trade_no`/
    /// `callback_no` are hashed server-side before matching, which the §7
    /// DSL cannot express — plus §8 pagination.
    #[allow(clippy::too_many_arguments)]
    pub async fn reconciliations_list(
        &self,
        pagination: Pagination,
        resolved: Option<&str>,
        payment_id: Option<i64>,
        reason: Option<&str>,
        trade_no: Option<&str>,
        callback_no: Option<&str>,
    ) -> Result<(Vec<Value>, i64), ApiError> {
        let resolved = reconciliation_resolved_filter(resolved)?;
        let payment_id = payment_id
            .map(|value| {
                i32::try_from(value)
                    .map_err(|_| validation_error("payment_id", "payment_id 超出支持范围"))
            })
            .transpose()?;
        let trade_no_hash =
            trade_no.map(|value| hex::encode(payment_reconciliation_identity_hash(value)));
        let callback_no_hash =
            callback_no.map(|value| hex::encode(payment_reconciliation_identity_hash(value)));

        let total: i64 = sqlx::query_scalar(
            r#"
            SELECT COUNT(*)
            FROM payment_reconciliation r
            WHERE (
                $1::SMALLINT = 2
                OR ($2::SMALLINT = 0 AND r.resolved_at IS NULL)
                OR ($3::SMALLINT = 1 AND r.resolved_at IS NOT NULL)
            )
              AND ($4::INTEGER IS NULL OR r.payment_id = $5)
              AND ($6::TEXT IS NULL OR r.reason = $7::TEXT)
              AND ($8::TEXT IS NULL OR r.trade_no_hash = decode($9::TEXT, 'hex'))
              AND ($10::TEXT IS NULL OR r.callback_no_hash = decode($11::TEXT, 'hex'))
            "#,
        )
        .bind(resolved)
        .bind(resolved)
        .bind(resolved)
        .bind(payment_id)
        .bind(payment_id)
        .bind(reason)
        .bind(reason)
        .bind(trade_no_hash.as_deref())
        .bind(trade_no_hash.as_deref())
        .bind(callback_no_hash.as_deref())
        .bind(callback_no_hash.as_deref())
        .fetch_one(&self.db)
        .await?;

        let rows = sqlx::query_scalar::<_, Json<Value>>(
            r#"
            SELECT jsonb_build_object(
                'id', r.id,
                'payment_id', r.payment_id,
                'payment_name', p.name,
                'payment_archived_at', p.archived_at,
                'provider', r.provider,
                'trade_no', r.trade_no,
                'trade_no_hash', encode(r.trade_no_hash, 'hex'),
                'callback_no', r.callback_no,
                'callback_no_hash', encode(r.callback_no_hash, 'hex'),
                'reason', r.reason,
                'order_status', r.order_status,
                'expected_amount', r.expected_amount,
                'settled_amount', r.settled_amount,
                'occurrence_count', r.occurrence_count,
                'first_seen_at', r.first_seen_at,
                'last_seen_at', r.last_seen_at,
                'resolved_at', r.resolved_at,
                'resolution', r.resolution
            )
            FROM payment_reconciliation r
            JOIN payment_method p ON p.id = r.payment_id
            WHERE (
                $1::SMALLINT = 2
                OR ($2::SMALLINT = 0 AND r.resolved_at IS NULL)
                OR ($3::SMALLINT = 1 AND r.resolved_at IS NOT NULL)
            )
              AND ($4::INTEGER IS NULL OR r.payment_id = $5)
              AND ($6::TEXT IS NULL OR r.reason = $7::TEXT)
              AND ($8::TEXT IS NULL OR r.trade_no_hash = decode($9::TEXT, 'hex'))
              AND ($10::TEXT IS NULL OR r.callback_no_hash = decode($11::TEXT, 'hex'))
            ORDER BY (r.resolved_at IS NOT NULL) ASC, r.first_seen_at DESC, r.id DESC
            LIMIT $12 OFFSET $13
            "#,
        )
        .bind(resolved)
        .bind(resolved)
        .bind(resolved)
        .bind(payment_id)
        .bind(payment_id)
        .bind(reason)
        .bind(reason)
        .bind(trade_no_hash.as_deref())
        .bind(trade_no_hash.as_deref())
        .bind(callback_no_hash.as_deref())
        .bind(callback_no_hash.as_deref())
        .bind(pagination.limit())
        .bind(pagination.offset())
        .fetch_all(&self.db)
        .await?;
        let items = json_rows(rows)
            .into_iter()
            .map(|row| {
                statistics::epoch_fields_to_rfc3339(
                    row,
                    &[
                        "payment_archived_at",
                        "first_seen_at",
                        "last_seen_at",
                        "resolved_at",
                    ],
                )
            })
            .collect();
        Ok((items, total))
    }

    /// POST `payment-reconciliations/{id}/resolve` (§6.4): the demultiplexed
    /// legacy `order/update` + `reconciliation_id` arm. 404
    /// `reconciliation_not_found`; repeating the identical resolution is
    /// idempotent, a different one 409 `reconciliation_already_processed`.
    pub async fn reconciliation_resolve(
        &self,
        reconciliation_id: i64,
        note: &str,
        actor: &str,
    ) -> Result<(), ApiError> {
        let note = note.trim();
        if note.is_empty() {
            return Err(validation_error("resolution", "resolution cannot be empty"));
        }
        let resolution = reconciliation_resolution(actor, note)?;
        let now = Utc::now().timestamp();
        let mut tx = self.db.begin().await?;
        let current = sqlx::query_as::<_, (String, Option<i64>, Option<String>)>(
            r#"
            SELECT trade_no, resolved_at, resolution
            FROM payment_reconciliation
            WHERE id = $1
            LIMIT 1
            FOR UPDATE
            "#,
        )
        .bind(reconciliation_id)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or_else(|| ApiError::from(Problem::new(Code::ReconciliationNotFound)))?;
        if current.1.is_some() {
            if current.2.as_deref() == Some(&resolution) {
                tx.commit().await?;
                return Ok(());
            }
            return Err(Problem::new(Code::ReconciliationAlreadyProcessed).into());
        }
        let updated = sqlx::query(
            r#"
            UPDATE payment_reconciliation
            SET resolved_at = $1, resolution = $2
            WHERE id = $3 AND resolved_at IS NULL
            "#,
        )
        .bind(now)
        .bind(&resolution)
        .bind(reconciliation_id)
        .execute(&mut *tx)
        .await?;
        if updated.rows_affected() != 1 {
            return Err(Problem::new(Code::ReconciliationAlreadyProcessed).into());
        }
        tx.commit().await?;
        tracing::info!(
            reconciliation_id,
            trade_no = current.0,
            actor,
            "administrator resolved payment reconciliation"
        );
        Ok(())
    }
}

#[cfg(test)]
mod commerce_wire_tests {
    use super::*;

    /// §6.4: PATCH `orders/{trade_no}` demands **exactly one** of `status`
    /// and `commission_status` — both or neither is a 422 validation
    /// problem, and each arm keeps its legacy `in:` vocabulary.
    #[test]
    fn order_patch_enforces_the_exactly_one_field_rule() {
        let neither: OrderPatch = serde_json::from_value(json!({})).unwrap();
        assert!(matches!(
            order_patch_action(&neither),
            Err(ApiError::Problem(problem)) if problem.code() == Code::ValidationFailed
        ));
        let both: OrderPatch =
            serde_json::from_value(json!({ "status": 1, "commission_status": 1 })).unwrap();
        assert!(matches!(
            order_patch_action(&both),
            Err(ApiError::Problem(problem)) if problem.code() == Code::ValidationFailed
        ));

        let status: OrderPatch = serde_json::from_value(json!({ "status": 2 })).unwrap();
        assert_eq!(
            order_patch_action(&status).unwrap(),
            OrderPatchAction::Status(2)
        );
        let commission: OrderPatch =
            serde_json::from_value(json!({ "commission_status": 3 })).unwrap();
        assert_eq!(
            order_patch_action(&commission).unwrap(),
            OrderPatchAction::CommissionStatus(3)
        );

        // Legacy Laravel vocabularies: status in 0-3, commission_status in 0/1/3.
        let bad_status: OrderPatch = serde_json::from_value(json!({ "status": 4 })).unwrap();
        assert!(order_patch_action(&bad_status).is_err());
        let bad_commission: OrderPatch =
            serde_json::from_value(json!({ "commission_status": 2 })).unwrap();
        assert!(order_patch_action(&bad_commission).is_err());

        // deny_unknown_fields: the legacy reconciliation_id demultiplex is
        // gone — it must not parse as an order patch.
        assert!(serde_json::from_value::<OrderPatch>(json!({ "reconciliation_id": 7 })).is_err());
    }

    /// §4.4: the payment PATCH distinguishes absent (retain), null (clear),
    /// and value (set) for the nullable metadata columns.
    #[test]
    fn payment_patch_distinguishes_absent_null_and_value() {
        let absent: PaymentPatch = serde_json::from_value(json!({})).unwrap();
        assert!(absent.icon.is_none());
        assert!(absent.notify_domain.is_none());
        assert!(absent.handling_fee_fixed.is_none());
        assert!(absent.enable.is_none());

        let cleared: PaymentPatch = serde_json::from_value(json!({
            "icon": null,
            "notify_domain": null,
            "handling_fee_fixed": null,
            "handling_fee_percent": null
        }))
        .unwrap();
        assert_eq!(cleared.icon, Some(None));
        assert_eq!(cleared.notify_domain, Some(None));
        assert_eq!(cleared.handling_fee_fixed, Some(None));
        assert!(matches!(cleared.handling_fee_percent, Some(None)));

        let set: PaymentPatch = serde_json::from_value(json!({
            "name": "Renamed",
            "handling_fee_fixed": 20,
            "handling_fee_percent": 0.5,
            "enable": true
        }))
        .unwrap();
        assert_eq!(set.handling_fee_fixed, Some(Some(20)));
        assert_eq!(set.enable, Some(true));
        assert_eq!(
            handling_fee_percent_decimal(set.handling_fee_percent.unwrap().as_ref()).unwrap(),
            Some(Decimal::new(5, 1))
        );
    }

    /// §6.2: the legacy 0.1–100 handling-fee window survives on the JSON
    /// number representation.
    #[test]
    fn handling_fee_percent_window_is_exact() {
        for valid in [json!(0.1), json!(100), json!(2.75)] {
            let number = valid.as_number().unwrap().clone();
            assert!(
                handling_fee_percent_decimal(Some(&number)).is_ok(),
                "{number}"
            );
        }
        for invalid in [json!(0), json!(0.09), json!(100.01), json!(-3)] {
            let number = invalid.as_number().unwrap().clone();
            assert!(
                handling_fee_percent_decimal(Some(&number)).is_err(),
                "{number}"
            );
        }
        assert_eq!(handling_fee_percent_decimal(None).unwrap(), None);
    }

    /// §4.4 + amount windows on the plan PATCH: double-Option clears, and
    /// every amount keeps the non-negative 32-bit window.
    #[test]
    fn plan_patch_distinguishes_absent_null_and_value_and_validates_amounts() {
        let patch: PlanPatch = serde_json::from_value(json!({
            "month_price": null,
            "capacity_limit": 50,
            "show": true,
            "force_update": true
        }))
        .unwrap();
        assert_eq!(patch.month_price, Some(None));
        assert_eq!(patch.capacity_limit, Some(Some(50)));
        assert!(patch.quarter_price.is_none());
        assert_eq!(patch.show, Some(true));
        assert_eq!(patch.force_update, Some(true));
        assert!(plan_patch_validation(&patch).is_ok());

        for invalid in [
            json!({ "month_price": -1 }),
            json!({ "transfer_enable": 2_147_483_648_i64 }),
        ] {
            let patch: PlanPatch = serde_json::from_value(invalid).unwrap();
            assert!(matches!(
                plan_patch_validation(&patch),
                Err(ApiError::Problem(problem)) if problem.code() == Code::ValidationFailed
            ));
        }
        let bad_reset: PlanPatch =
            serde_json::from_value(json!({ "reset_traffic_method": 40_000 })).unwrap();
        assert!(plan_patch_validation(&bad_reset).is_err());
    }

    /// §6.2/§4.5: admin plan and payment items serialize bool flags and
    /// RFC 3339 timestamps (prices cents, `handling_fee_percent` a number).
    #[test]
    fn commerce_items_serialize_modern_value_types() {
        let plan = AdminPlanItem {
            id: 1,
            group_id: 1,
            transfer_enable: 100,
            device_limit: None,
            name: "Golden Plan".to_string(),
            speed_limit: None,
            show: true,
            sort: Some(1),
            renew: false,
            content: None,
            month_price: Some(1000),
            quarter_price: None,
            half_year_price: None,
            year_price: None,
            two_year_price: None,
            three_year_price: None,
            onetime_price: None,
            reset_price: None,
            reset_traffic_method: Some(0),
            capacity_limit: None,
            count: 3,
            created_at: 1_700_000_000,
            updated_at: 1_700_000_000,
        };
        let encoded = serde_json::to_value(&plan).unwrap();
        assert_eq!(encoded["show"], json!(true));
        assert_eq!(encoded["renew"], json!(false));
        assert_eq!(encoded["count"], json!(3));
        assert_eq!(encoded["month_price"], json!(1000));
        assert_eq!(encoded["created_at"], json!("2023-11-14T22:13:20Z"));

        let payment = AdminPaymentItem {
            id: 7,
            name: "Golden EPay".to_string(),
            payment: "EPay".to_string(),
            icon: None,
            handling_fee_fixed: Some(20),
            handling_fee_percent: Some(0.5),
            uuid: "goldenepayuuid000000000000000001".to_string(),
            config: json!({ "pid": "1000" }),
            notify_domain: None,
            notify_url: "https://golden.v2board.test/api/v1/guest/payment/notify/EPay/goldenepayuuid000000000000000001".to_string(),
            enable: true,
            sort: Some(1),
            created_at: 1_700_000_000,
            updated_at: 1_700_000_000,
            legacy_md5_signature: true,
            security_warning: Some("warning"),
        };
        let encoded = serde_json::to_value(&payment).unwrap();
        assert_eq!(encoded["enable"], json!(true));
        assert_eq!(encoded["handling_fee_percent"], json!(0.5));
        assert_eq!(encoded["updated_at"], json!("2023-11-14T22:13:20Z"));
        assert_eq!(encoded["legacy_md5_signature"], json!(true));
    }

    /// §6.4: the assign body is the named JSON object with an optional
    /// `total_amount`; unknown fields are rejected.
    #[test]
    fn order_assign_body_is_strict_json() {
        let assign: OrderAssign = serde_json::from_value(json!({
            "email": "member@example.test",
            "plan_id": 1,
            "period": "month_price"
        }))
        .unwrap();
        assert_eq!(assign.total_amount, None);
        assert!(
            serde_json::from_value::<OrderAssign>(json!({
                "email": "member@example.test",
                "plan_id": 1,
                "period": "month_price",
                "id": 9
            }))
            .is_err()
        );
    }
}
