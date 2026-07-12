use super::*;
const PLAN_USER_LOCK_PAGE_SIZE: i64 = 500;
const PLAN_FORCE_UPDATE_MAX_USERS: usize = 10_000;
const ADMIN_ASSIGN_UNFINISHED_ORDER_SQL: &str = r#"
SELECT id
FROM v2_order
WHERE user_id = ? AND status IN (0, 1)
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
    // below because each payment row is an immutable verification version.
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

pub(super) fn payment_config_input(params: &HashMap<String, String>, payment: &str) -> Value {
    if crate::payment_provider::payment_provider_manifest(payment).is_none() {
        return nested_json(params, "config");
    }
    let mut root = Value::Object(Map::new());
    for (raw_key, raw_value) in params {
        if let Some(path) = bracket_path(raw_key, "config") {
            // Built-in provider manifests are text forms. Preserve exact strings
            // (including numeric-looking IDs and boolean-looking secrets) rather
            // than applying the generic JSON scalar coercion used elsewhere.
            insert_nested_json(&mut root, &path, Value::String(raw_value.clone()));
        }
    }
    root
}

fn payment_reconciliation_identity_hash(value: &str) -> [u8; 32] {
    Sha256::digest(value.as_bytes()).into()
}

pub(super) fn reconciliation_resolved_filter(
    params: &HashMap<String, String>,
) -> Result<i8, ApiError> {
    match optional_string(params, "resolved").as_deref() {
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
        return ApiError::legacy("该用户还有待支付的订单，无法分配");
    }
    if matches!(database_error.code().as_deref(), Some("1205" | "1213")) {
        return ApiError::legacy("订单状态正在被其他请求修改，请重试");
    }
    ApiError::Database(error)
}

async fn lock_server_group_for_share(
    tx: &mut Transaction<'_, MySql>,
    group_id: i64,
) -> Result<(), ApiError> {
    let exists: Option<i64> =
        sqlx::query_scalar("SELECT id FROM v2_server_group WHERE id = ? LIMIT 1 FOR SHARE")
            .bind(group_id)
            .fetch_optional(&mut **tx)
            .await?;
    if exists.is_none() {
        return Err(ApiError::legacy("该服务器组不存在"));
    }
    Ok(())
}

async fn lock_plan_users_for_update(
    tx: &mut Transaction<'_, MySql>,
    plan_id: i64,
) -> Result<(), ApiError> {
    let mut after_id = 0_i64;
    let mut locked = 0_usize;
    loop {
        let ids = sqlx::query_scalar::<_, i64>(
            r#"
            SELECT id
            FROM v2_user
            WHERE plan_id = ? AND id > ?
            ORDER BY id
            LIMIT ?
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
            return Err(ApiError::legacy(
                "该订阅用户过多，单次最多强制更新 10000 个用户",
            ));
        }
        after_id = last_id;
    }
}

pub(super) fn required_nonnegative_i32(
    params: &HashMap<String, String>,
    field: &str,
) -> Result<i64, ApiError> {
    let value = required_i64(params, field)?;
    if !(0..=i64::from(i32::MAX)).contains(&value) {
        return Err(ApiError::validation_field(
            field,
            "Value must be a non-negative 32-bit integer",
        ));
    }
    Ok(value)
}

pub(super) fn optional_nonnegative_i32(
    params: &HashMap<String, String>,
    field: &str,
) -> Result<Option<i64>, ApiError> {
    let Some(raw) = params.get(field).map(|value| value.trim()) else {
        return Ok(None);
    };
    if raw.is_empty() || raw.eq_ignore_ascii_case("null") {
        return Ok(None);
    }
    let value = raw.parse::<i64>().map_err(|_| {
        ApiError::validation_field(field, "Value must be a non-negative 32-bit integer")
    })?;
    if !(0..=i64::from(i32::MAX)).contains(&value) {
        return Err(ApiError::validation_field(
            field,
            "Value must be a non-negative 32-bit integer",
        ));
    }
    Ok(Some(value))
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

impl AdminService {
    pub(super) async fn plan_fetch(&self) -> Result<AdminOutput, ApiError> {
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

    pub(super) async fn plan_save(
        &self,
        params: &HashMap<String, String>,
    ) -> Result<AdminOutput, ApiError> {
        let now = Utc::now().timestamp();
        let id = optional_i64(params, "id");
        let transfer_enable = required_nonnegative_i32(params, "transfer_enable")?;
        let device_limit = optional_nonnegative_i32(params, "device_limit")?;
        let speed_limit = optional_nonnegative_i32(params, "speed_limit")?;
        let capacity_limit = optional_nonnegative_i32(params, "capacity_limit")?;
        let month_price = optional_nonnegative_i32(params, "month_price")?;
        let quarter_price = optional_nonnegative_i32(params, "quarter_price")?;
        let half_year_price = optional_nonnegative_i32(params, "half_year_price")?;
        let year_price = optional_nonnegative_i32(params, "year_price")?;
        let two_year_price = optional_nonnegative_i32(params, "two_year_price")?;
        let three_year_price = optional_nonnegative_i32(params, "three_year_price")?;
        let onetime_price = optional_nonnegative_i32(params, "onetime_price")?;
        let reset_price = optional_nonnegative_i32(params, "reset_price")?;
        let group_id = required_i64(params, "group_id")?;
        let force_update = id.is_some() && truthy(params.get("force_update"));
        let transfer_enable_bytes = force_update
            .then(|| checked_gib_bytes(transfer_enable, "transfer_enable"))
            .transpose()?;
        let mut tx = self.db.begin().await?;
        // Group writers use group -> user -> plan ordering.  The shared parent
        // lock makes a concurrent group drop wait before either the plan or its
        // users can be changed.
        lock_server_group_for_share(&mut tx, group_id).await?;
        if let Some(id) = id {
            if force_update {
                // Order lifecycle writers take user before plan.  Acquire every
                // affected user in primary-key pages before the plan row so the
                // force propagation cannot invert that order or materialize an
                // unbounded id list.
                lock_plan_users_for_update(&mut tx, id).await?;
            }
            let plan_exists: Option<i64> =
                sqlx::query_scalar("SELECT id FROM v2_plan WHERE id = ? LIMIT 1 FOR UPDATE")
                    .bind(id)
                    .fetch_optional(&mut *tx)
                    .await?;
            if plan_exists.is_none() {
                return Err(ApiError::legacy("该订阅ID不存在"));
            }
            // PlanSave excludes show/renew/sort, so edit never touches them.
            sqlx::query(
                r#"
                UPDATE v2_plan
                SET group_id = ?, transfer_enable = ?, device_limit = ?, name = ?,
                    speed_limit = ?, content = ?,
                    month_price = ?, quarter_price = ?, half_year_price = ?, year_price = ?,
                    two_year_price = ?, three_year_price = ?, onetime_price = ?, reset_price = ?,
                    reset_traffic_method = ?, capacity_limit = ?, updated_at = ?
                WHERE id = ?
                "#,
            )
            .bind(group_id)
            .bind(transfer_enable)
            .bind(device_limit)
            .bind(required_string(params, "name")?)
            .bind(speed_limit)
            .bind(params.get("content"))
            .bind(month_price)
            .bind(quarter_price)
            .bind(half_year_price)
            .bind(year_price)
            .bind(two_year_price)
            .bind(three_year_price)
            .bind(onetime_price)
            .bind(reset_price)
            .bind(optional_i64(params, "reset_traffic_method"))
            .bind(capacity_limit)
            .bind(now)
            .bind(id)
            .execute(&mut *tx)
            .await?;
            if force_update {
                sqlx::query(
                    r#"
                    UPDATE v2_user
                    SET group_id = ?, transfer_enable = ?, device_limit = ?, speed_limit = ?, updated_at = ?
                    WHERE plan_id = ?
                    "#,
                )
                .bind(group_id)
                .bind(transfer_enable_bytes)
                .bind(device_limit)
                .bind(speed_limit)
                .bind(now)
                .bind(id)
                .execute(&mut *tx)
                .await?;
            }
        } else {
            // PlanSave excludes show/renew/sort, so create leaves the DB defaults
            // (show = 0, renew = 1, sort = NULL).
            sqlx::query(
                r#"
                INSERT INTO v2_plan (
                    group_id, transfer_enable, device_limit, name, speed_limit,
                    content, month_price, quarter_price, half_year_price, year_price,
                    two_year_price, three_year_price, onetime_price, reset_price,
                    reset_traffic_method, capacity_limit, created_at, updated_at
                )
                VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
                "#,
            )
            .bind(group_id)
            .bind(transfer_enable)
            .bind(device_limit)
            .bind(required_string(params, "name")?)
            .bind(speed_limit)
            .bind(params.get("content"))
            .bind(month_price)
            .bind(quarter_price)
            .bind(half_year_price)
            .bind(year_price)
            .bind(two_year_price)
            .bind(three_year_price)
            .bind(onetime_price)
            .bind(reset_price)
            .bind(optional_i64(params, "reset_traffic_method"))
            .bind(capacity_limit)
            .bind(now)
            .bind(now)
            .execute(&mut *tx)
            .await?;
        }
        tx.commit().await?;
        Ok(AdminOutput::Data(json!(true)))
    }

    pub(super) async fn plan_update(
        &self,
        params: &HashMap<String, String>,
    ) -> Result<AdminOutput, ApiError> {
        let id = required_i64(params, "id")?;
        // PlanUpdate validates show/renew as in:0,1 before the controller runs.
        for (field, message) in [
            ("show", "销售状态格式不正确"),
            ("renew", "续费状态格式不正确"),
        ] {
            if let Some(value) = params.get(field).map(|value| value.trim())
                && value != "0"
                && value != "1"
            {
                return Err(validation_error(field, message));
            }
        }
        // PlanController::update aborts 500 when the plan id does not exist.
        let exists: Option<i64> = sqlx::query_scalar("SELECT id FROM v2_plan WHERE id = ? LIMIT 1")
            .bind(id)
            .fetch_optional(&self.db)
            .await?;
        if exists.is_none() {
            return Err(ApiError::legacy("该订阅不存在"));
        }
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

    pub(super) async fn payment_fetch(&self) -> Result<AdminOutput, ApiError> {
        let rows = sqlx::query_as::<_, PaymentRow>(
            r#"
            SELECT id, name, payment, icon, handling_fee_fixed,
                   CAST(handling_fee_percent AS DOUBLE) AS handling_fee_percent,
                   uuid, CAST(config AS CHAR) AS config, notify_domain, enable, sort,
                   created_at, updated_at
            FROM v2_payment
            WHERE archived_at IS NULL
            ORDER BY sort ASC
            "#,
        )
        .fetch_all(&self.db)
        .await?;
        let data = rows
            .into_iter()
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
                let legacy_md5_signature =
                    crate::payment_provider::payment_provider_uses_legacy_md5(&row.payment);
                let security_warning =
                    crate::payment_provider::payment_provider_security_warning(&row.payment);
                Ok(json!({
                    "id": row.id,
                    "name": row.name,
                    "payment": row.payment,
                    "icon": row.icon,
                    "handling_fee_fixed": row.handling_fee_fixed,
                    "handling_fee_percent": row.handling_fee_percent,
                    "uuid": row.uuid,
                    "config": config,
                    "notify_domain": row.notify_domain,
                    "notify_url": notify_url,
                    "enable": row.enable,
                    "sort": row.sort,
                    "created_at": row.created_at,
                    "updated_at": row.updated_at,
                    "legacy_md5_signature": legacy_md5_signature,
                    "security_warning": security_warning,
                }))
            })
            .collect::<Result<Vec<_>, ApiError>>()?;
        Ok(AdminOutput::Data(json!(data)))
    }

    pub(super) async fn payment_form(
        &self,
        params: &HashMap<String, String>,
    ) -> Result<AdminOutput, ApiError> {
        let mut payment = params.get("payment").cloned().unwrap_or_default();
        let config = if let Some(id) = optional_i64(params, "id") {
            let (stored_payment, raw_config) = sqlx::query_as::<_, (String, String)>(
                "SELECT payment, CAST(config AS CHAR) FROM v2_payment \
                 WHERE id = ? AND archived_at IS NULL LIMIT 1",
            )
            .bind(id)
            .fetch_optional(&self.db)
            .await?
            .ok_or_else(|| ApiError::legacy("支付方式不存在"))?;
            if payment.is_empty() {
                payment.clone_from(&stored_payment);
            }
            if payment == stored_payment {
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
        Ok(AdminOutput::Data(payment_provider_form(
            &payment,
            config.as_ref(),
        )))
    }

    pub(super) async fn payment_save(
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
        // PaymentController::save validates name/payment/config plus the optional
        // notify_domain url and handling fee formats. It does NOT check that the
        // gateway manifest exists, so no "gate is not found" gate here.
        let name = required_string(params, "name")
            .map_err(|_| validation_error("name", "显示名称不能为空"))?;
        let payment = required_string(params, "payment")
            .map_err(|_| validation_error("payment", "网关参数不能为空"))?;
        if !param_present(params, "config") {
            return Err(validation_error("config", "配置参数不能为空"));
        }
        if let Some(domain) = optional_string(params, "notify_domain")
            && !is_valid_url(&domain)
        {
            return Err(validation_error("notify_domain", "自定义通知域名格式有误"));
        }
        let handling_fee_fixed = optional_nonnegative_i32(params, "handling_fee_fixed")?;
        if let Some(value) = optional_string(params, "handling_fee_percent") {
            match value.parse::<Decimal>() {
                Ok(number) if (Decimal::new(1, 1)..=Decimal::from(100)).contains(&number) => {}
                _ => {
                    return Err(validation_error(
                        "handling_fee_percent",
                        "百分比手续费范围须在0.1-100之间",
                    ));
                }
            }
        }
        let submitted_config = payment_config_input(params, &payment);
        if crate::payment_provider::payment_provider_uses_legacy_md5(&payment) {
            tracing::warn!(
                provider = payment,
                "administrator saved a legacy MD5 payment provider; HTTPS and migration are strongly recommended"
            );
        }
        let now = Utc::now().timestamp();
        if let Some(id) = optional_i64(params, "id") {
            let mut tx = self.db.begin().await?;
            let current = sqlx::query_as::<_, (String, String)>(
                "SELECT payment, CAST(config AS CHAR) FROM v2_payment \
                 WHERE id = ? AND archived_at IS NULL LIMIT 1 FOR UPDATE",
            )
            .bind(id)
            .fetch_optional(&mut *tx)
            .await?
            .ok_or_else(|| ApiError::legacy("支付方式不存在"))?;
            let config_value = resolve_redacted_payment_config(
                &payment,
                Some((&current.0, &current.1)),
                submitted_config,
            )?;
            let config = serde_json::to_string(&config_value)
                .map_err(|_| ApiError::internal("failed to encode payment config"))?;
            let config_changed = serde_json::from_str::<Value>(&current.1)
                .map(|value| value != config_value)
                .unwrap_or(true);
            if payment_verification_version_blocks_update(current.0 != payment, config_changed) {
                return Err(ApiError::legacy(
                    "支付方式是不可变验签版本，网关类型和密钥配置不可原地修改；请归档后新建支付方式",
                ));
            }
            sqlx::query(
                r#"
                UPDATE v2_payment
                SET name = ?, icon = ?, payment = ?, config = ?, notify_domain = ?,
                    handling_fee_fixed = ?, handling_fee_percent = ?, updated_at = ?
                WHERE id = ? AND archived_at IS NULL
                "#,
            )
            .bind(&name)
            .bind(optional_string(params, "icon"))
            .bind(&payment)
            .bind(config)
            .bind(optional_string(params, "notify_domain"))
            .bind(handling_fee_fixed)
            .bind(optional_decimal(params, "handling_fee_percent"))
            .bind(now)
            .bind(id)
            .execute(&mut *tx)
            .await?;
            tx.commit().await?;
        } else {
            let config_value = resolve_redacted_payment_config(&payment, None, submitted_config)?;
            let config = serde_json::to_string(&config_value)
                .map_err(|_| ApiError::internal("failed to encode payment config"))?;
            sqlx::query(
                r#"
                INSERT INTO v2_payment (
                    name, icon, payment, uuid, config, notify_domain, handling_fee_fixed,
                    handling_fee_percent, enable, sort, created_at, updated_at
                )
                VALUES (?, ?, ?, ?, ?, ?, ?, ?, 0, ?, ?, ?)
                "#,
            )
            .bind(&name)
            .bind(params.get("icon"))
            .bind(&payment)
            .bind(random_payment_uuid())
            .bind(config)
            .bind(params.get("notify_domain"))
            .bind(handling_fee_fixed)
            .bind(optional_decimal(params, "handling_fee_percent"))
            .bind(optional_i64(params, "sort"))
            .bind(now)
            .bind(now)
            .execute(&self.db)
            .await?;
        }
        Ok(AdminOutput::Data(json!(true)))
    }

    pub(super) async fn payment_drop(&self, id: i64) -> Result<AdminOutput, ApiError> {
        let mut tx = self.db.begin().await?;
        let exists: Option<i64> = sqlx::query_scalar(
            "SELECT id FROM v2_payment \
                 WHERE id = ? AND archived_at IS NULL LIMIT 1 FOR UPDATE",
        )
        .bind(id)
        .fetch_optional(&mut *tx)
        .await?;
        if exists.is_none() {
            return Err(ApiError::legacy("支付方式不存在"));
        }
        let now = Utc::now().timestamp();
        let archived = sqlx::query(
            "UPDATE v2_payment \
             SET enable = 0, archived_at = COALESCE(archived_at, ?), updated_at = ? \
             WHERE id = ? AND archived_at IS NULL",
        )
        .bind(now)
        .bind(now)
        .bind(id)
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(AdminOutput::Data(json!(archived.rows_affected() != 0)))
    }

    pub(super) async fn payment_sort(&self, ids: &[i64]) -> Result<AdminOutput, ApiError> {
        let mut tx = self.db.begin().await?;
        for (index, id) in ids.iter().enumerate() {
            sqlx::query(
                "UPDATE v2_payment SET sort = ?, updated_at = ? \
                 WHERE id = ? AND archived_at IS NULL",
            )
            .bind((index + 1) as i64)
            .bind(Utc::now().timestamp())
            .bind(id)
            .execute(&mut *tx)
            .await?;
        }
        tx.commit().await?;
        Ok(AdminOutput::Data(json!(true)))
    }

    pub(super) async fn payment_show(&self, id: i64) -> Result<AdminOutput, ApiError> {
        // PaymentController::show aborts 500 when the id does not exist before
        // flipping the enable flag. This toggle intentionally remains available
        // with pending orders: it stops new checkouts, while authenticated
        // in-flight callbacks continue using the immutable driver/config binding.
        let updated = sqlx::query(
            "UPDATE v2_payment \
             SET enable = IF(enable = 1, 0, 1), updated_at = ? \
             WHERE id = ? AND archived_at IS NULL",
        )
        .bind(Utc::now().timestamp())
        .bind(id)
        .execute(&self.db)
        .await?;
        if updated.rows_affected() != 1 {
            return Err(ApiError::legacy("支付方式不存在"));
        }
        Ok(AdminOutput::Data(json!(true)))
    }

    pub(super) async fn payment_reconciliation_fetch(
        &self,
        params: &HashMap<String, String>,
    ) -> Result<AdminOutput, ApiError> {
        let pagination = page(params)?;
        let resolved = reconciliation_resolved_filter(params)?;
        let payment_id = optional_i64(params, "payment_id");
        let reason = optional_string(params, "reason");
        let trade_no_hash = optional_string(params, "trade_no")
            .map(|value| hex::encode(payment_reconciliation_identity_hash(&value)));
        let callback_no_hash = optional_string(params, "callback_no")
            .map(|value| hex::encode(payment_reconciliation_identity_hash(&value)));

        let total: i64 = sqlx::query_scalar(
            r#"
            SELECT COUNT(*)
            FROM v2_payment_reconciliation r
            WHERE (
                ? = 2
                OR (? = 0 AND r.resolved_at IS NULL)
                OR (? = 1 AND r.resolved_at IS NOT NULL)
            )
              AND (? IS NULL OR r.payment_id = ?)
              AND (? IS NULL OR r.reason = ?)
              AND (? IS NULL OR r.trade_no_hash = UNHEX(?))
              AND (? IS NULL OR r.callback_no_hash = UNHEX(?))
            "#,
        )
        .bind(resolved)
        .bind(resolved)
        .bind(resolved)
        .bind(payment_id)
        .bind(payment_id)
        .bind(reason.as_deref())
        .bind(reason.as_deref())
        .bind(trade_no_hash.as_deref())
        .bind(trade_no_hash.as_deref())
        .bind(callback_no_hash.as_deref())
        .bind(callback_no_hash.as_deref())
        .fetch_one(&self.db)
        .await?;

        let rows = sqlx::query_scalar::<_, Json<Value>>(
            r#"
            SELECT JSON_OBJECT(
                'id', r.id,
                'payment_id', r.payment_id,
                'payment_name', p.name,
                'payment_archived_at', p.archived_at,
                'provider', r.provider,
                'trade_no', r.trade_no,
                'trade_no_hash', HEX(r.trade_no_hash),
                'callback_no', r.callback_no,
                'callback_no_hash', HEX(r.callback_no_hash),
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
            FROM v2_payment_reconciliation r
            JOIN v2_payment p ON p.id = r.payment_id
            WHERE (
                ? = 2
                OR (? = 0 AND r.resolved_at IS NULL)
                OR (? = 1 AND r.resolved_at IS NOT NULL)
            )
              AND (? IS NULL OR r.payment_id = ?)
              AND (? IS NULL OR r.reason = ?)
              AND (? IS NULL OR r.trade_no_hash = UNHEX(?))
              AND (? IS NULL OR r.callback_no_hash = UNHEX(?))
            ORDER BY (r.resolved_at IS NOT NULL) ASC, r.first_seen_at DESC, r.id DESC
            LIMIT ? OFFSET ?
            "#,
        )
        .bind(resolved)
        .bind(resolved)
        .bind(resolved)
        .bind(payment_id)
        .bind(payment_id)
        .bind(reason.as_deref())
        .bind(reason.as_deref())
        .bind(trade_no_hash.as_deref())
        .bind(trade_no_hash.as_deref())
        .bind(callback_no_hash.as_deref())
        .bind(callback_no_hash.as_deref())
        .bind(pagination.limit)
        .bind(pagination.offset)
        .fetch_all(&self.db)
        .await?;
        Ok(AdminOutput::Page {
            data: json_rows(rows),
            total,
        })
    }

    pub(super) async fn order_fetch(
        &self,
        params: &HashMap<String, String>,
    ) -> Result<AdminOutput, ApiError> {
        let pagination = page(params)?;
        let is_commission = truthy(params.get("is_commission"));
        let clauses = self.order_filter_clauses(params).await?;

        let mut count_builder =
            QueryBuilder::<MySql>::new("SELECT COUNT(*) FROM v2_order o WHERE 1 = 1");
        push_order_where(&mut count_builder, is_commission, &clauses);
        let total: i64 = count_builder
            .build_query_scalar()
            .fetch_one(&self.db)
            .await?;

        let mut builder = QueryBuilder::<MySql>::new(
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
                'commission_balance', o.commission_balance,
                'actual_commission_balance', o.actual_commission_balance,
                'payment_id', o.payment_id,
                'payment_reconciliation_open_count', (
                    SELECT COUNT(*) FROM v2_payment_reconciliation r
                    WHERE r.trade_no_hash = UNHEX(SHA2(o.trade_no, 256))
                      AND r.resolved_at IS NULL
                ),
                'paid_at', o.paid_at, 'created_at', o.created_at, 'updated_at', o.updated_at
            )
            FROM v2_order o
            LEFT JOIN v2_user u ON u.id = o.user_id
            LEFT JOIN v2_plan p ON p.id = o.plan_id
            WHERE 1 = 1
            "#,
        );
        push_order_where(&mut builder, is_commission, &clauses);
        builder.push(" ORDER BY o.created_at DESC LIMIT ");
        builder.push_bind(pagination.limit);
        builder.push(" OFFSET ");
        builder.push_bind(pagination.offset);
        let rows = builder
            .build_query_scalar::<Json<Value>>()
            .fetch_all(&self.db)
            .await?;
        let data = rows.into_iter().map(|row| row.0).collect();
        Ok(AdminOutput::Page { data, total })
    }

    /// Ports OrderController::filter (:21-38): reconstructs filter[] into
    /// injection-safe WHERE clauses. The `email` key looks a user up by the
    /// literal `%value%` (reproducing the Laravel bug — it is an exact match, not
    /// a LIKE) and scopes to that user's id, skipping the filter when no user
    /// matches; `模糊` becomes LIKE %value%. Unknown columns/operators are dropped
    /// rather than interpolated.
    async fn order_filter_clauses(
        &self,
        params: &HashMap<String, String>,
    ) -> Result<Vec<OrderFilterClause>, ApiError> {
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
            if key == "email" {
                let user_id: Option<i64> =
                    sqlx::query_scalar("SELECT id FROM v2_user WHERE email = ? LIMIT 1")
                        .bind(format!("%{value}%"))
                        .fetch_optional(&self.db)
                        .await?;
                if let Some(user_id) = user_id {
                    clauses.push(OrderFilterClause::Compare {
                        column: "user_id",
                        op: "=",
                        value: user_id.to_string(),
                    });
                }
                continue;
            }
            if condition == "模糊" {
                condition = "like".to_string();
                value = format!("%{value}%");
            }
            let (Some(column), Some(op)) = (order_column(key), user_filter_operator(&condition))
            else {
                continue;
            };
            clauses.push(OrderFilterClause::Compare { column, op, value });
        }
        Ok(clauses)
    }

    pub(super) async fn order_detail(&self, id: i64) -> Result<AdminOutput, ApiError> {
        let mut value = fetch_json_one(
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
                'commission_balance', o.commission_balance,
                'actual_commission_balance', o.actual_commission_balance,
                'payment_id', o.payment_id,
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

        // OrderController::detail always attaches commission_log (the CommissionLog
        // rows for this order's trade_no; an empty array when there are none).
        let trade_no = value
            .get("trade_no")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string();
        let trade_no_hash = payment_reconciliation_identity_hash(&trade_no);
        let commission_rows = sqlx::query_scalar::<_, Json<Value>>(
            r#"
            SELECT JSON_OBJECT(
                'id', id, 'invite_user_id', invite_user_id, 'user_id', user_id,
                'trade_no', trade_no, 'order_amount', order_amount, 'get_amount', get_amount,
                'created_at', created_at, 'updated_at', updated_at
            )
            FROM v2_commission_log
            WHERE trade_no = ?
            "#,
        )
        .bind(&trade_no)
        .fetch_all(&self.db)
        .await?;
        let commission_log = json_rows(commission_rows);
        let payment_reconciliations = sqlx::query_scalar::<_, Json<Value>>(
            r#"
            SELECT JSON_OBJECT(
                'id', id, 'payment_id', payment_id, 'provider', provider,
                'trade_no', trade_no, 'trade_no_hash', HEX(trade_no_hash),
                'callback_no', callback_no, 'callback_no_hash', HEX(callback_no_hash),
                'reason', reason,
                'order_status', order_status, 'expected_amount', expected_amount,
                'settled_amount', settled_amount, 'occurrence_count', occurrence_count,
                'first_seen_at', first_seen_at, 'last_seen_at', last_seen_at,
                'resolved_at', resolved_at, 'resolution', resolution
            )
            FROM v2_payment_reconciliation
            WHERE trade_no_hash = ?
            ORDER BY first_seen_at DESC, id DESC
            "#,
        )
        .bind(trade_no_hash.as_slice())
        .fetch_all(&self.db)
        .await?;
        let payment_reconciliations = json_rows(payment_reconciliations);

        // surplus_orders is attached only when surplus_order_ids is a non-empty array
        // (PHP `if ($order->surplus_order_ids)` on the array cast).
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
                let mut builder = QueryBuilder::<MySql>::new(
                    r#"
                    SELECT JSON_OBJECT(
                        'id', o.id, 'invite_user_id', o.invite_user_id, 'user_id', o.user_id,
                        'plan_id', o.plan_id, 'coupon_id', o.coupon_id, 'type', o.type, 'period', o.period,
                        'trade_no', o.trade_no, 'callback_no', o.callback_no, 'total_amount', o.total_amount,
                        'handling_amount', o.handling_amount, 'discount_amount', o.discount_amount,
                        'surplus_amount', o.surplus_amount, 'refund_amount', o.refund_amount,
                        'balance_amount', o.balance_amount, 'surplus_order_ids', CAST(o.surplus_order_ids AS JSON),
                        'status', o.status, 'commission_status', o.commission_status,
                        'commission_balance', o.commission_balance,
                        'actual_commission_balance', o.actual_commission_balance,
                        'payment_id', o.payment_id,
                        'paid_at', o.paid_at, 'created_at', o.created_at, 'updated_at', o.updated_at
                    )
                    FROM v2_order o
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
            };
            Some(rows)
        } else {
            None
        };

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
        Ok(AdminOutput::Data(value))
    }

    pub(super) async fn order_update(
        &self,
        params: &HashMap<String, String>,
    ) -> Result<AdminOutput, ApiError> {
        if param_present(params, "reconciliation_id") {
            return self.resolve_payment_reconciliation(params).await;
        }
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

    async fn resolve_payment_reconciliation(
        &self,
        params: &HashMap<String, String>,
    ) -> Result<AdminOutput, ApiError> {
        let reconciliation_id = required_i64(params, "reconciliation_id")?;
        let note = required_string(params, "resolution")?;
        let actor = required_string(params, "_admin_email")?;
        let resolution = reconciliation_resolution(&actor, &note)?;
        let now = Utc::now().timestamp();
        let mut tx = self.db.begin().await?;
        let current = sqlx::query_as::<_, (String, Option<i64>, Option<String>)>(
            r#"
            SELECT trade_no, resolved_at, resolution
            FROM v2_payment_reconciliation
            WHERE id = ?
            LIMIT 1
            FOR UPDATE
            "#,
        )
        .bind(reconciliation_id)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or_else(|| ApiError::legacy("付款核对记录不存在"))?;
        if current.1.is_some() {
            if current.2.as_deref() == Some(&resolution) {
                tx.commit().await?;
                return Ok(AdminOutput::Data(json!(true)));
            }
            return Err(ApiError::legacy("付款核对记录已处理"));
        }
        let updated = sqlx::query(
            r#"
            UPDATE v2_payment_reconciliation
            SET resolved_at = ?, resolution = ?
            WHERE id = ? AND resolved_at IS NULL
            "#,
        )
        .bind(now)
        .bind(&resolution)
        .bind(reconciliation_id)
        .execute(&mut *tx)
        .await?;
        if updated.rows_affected() != 1 {
            return Err(ApiError::legacy("付款核对记录已处理"));
        }
        tx.commit().await?;
        tracing::info!(
            reconciliation_id,
            trade_no = current.0,
            actor,
            "administrator resolved payment reconciliation"
        );
        Ok(AdminOutput::Data(json!(true)))
    }

    pub(super) async fn order_paid(&self, trade_no: String) -> Result<AdminOutput, ApiError> {
        OrderService::new(self.db.clone(), self.config.clone())
            .paid_manually(&trade_no)
            .await?;
        Ok(AdminOutput::Data(json!(true)))
    }

    pub(super) async fn order_cancel(&self, trade_no: String) -> Result<AdminOutput, ApiError> {
        // Ports Admin\OrderController::cancel + OrderService::cancel (:273-291):
        // only pending orders can be cancelled, and the balance paid toward the
        // order is refunded to the user via addBalance.
        let order: (i8, i64, Option<i64>, Option<i32>, Option<String>) = sqlx::query_as(
            r#"
            SELECT status, user_id, balance_amount, payment_id, callback_no
            FROM v2_order
            WHERE trade_no = ?
            LIMIT 1
            "#,
        )
        .bind(&trade_no)
        .fetch_optional(&self.db)
        .await?
        .ok_or_else(|| ApiError::legacy("订单不存在"))?;
        let (status, user_id, balance_amount, payment_id, callback_no) = order;
        if status != 0 {
            return Err(ApiError::legacy("只能对待支付的订单进行操作"));
        }
        let order_service = OrderService::new(self.db.clone(), self.config.clone());
        if !order_service
            .cancel_stripe_intent_binding(payment_id, callback_no.as_deref())
            .await?
        {
            return Err(ApiError::legacy("只能对待支付的订单进行操作"));
        }
        let now = Utc::now().timestamp();
        let mut tx = self.db.begin().await?;
        let updated = sqlx::query(
            r#"
            UPDATE v2_order SET status = 2, updated_at = ?
            WHERE trade_no = ? AND status = 0
              AND payment_id <=> ? AND callback_no <=> ?
            "#,
        )
        .bind(now)
        .bind(&trade_no)
        .bind(payment_id)
        .bind(&callback_no)
        .execute(&mut *tx)
        .await?;
        if updated.rows_affected() != 1 {
            return Err(ApiError::legacy("只能对待支付的订单进行操作"));
        }
        if let Some(balance) = balance_amount.filter(|value| *value != 0) {
            // UserService::addBalance: lock the row, add, and reject a negative result.
            let current: i64 =
                sqlx::query_scalar("SELECT balance FROM v2_user WHERE id = ? FOR UPDATE")
                    .bind(user_id)
                    .fetch_optional(&mut *tx)
                    .await?
                    .ok_or_else(|| ApiError::legacy("更新失败"))?;
            let updated = current
                .checked_add(balance)
                .ok_or_else(|| ApiError::legacy("更新失败"))?;
            if !(0..=i64::from(i32::MAX)).contains(&updated) {
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

    pub(super) async fn order_assign(
        &self,
        params: &HashMap<String, String>,
    ) -> Result<AdminOutput, ApiError> {
        let email = required_string(params, "email")?;
        let plan_id = required_i64(params, "plan_id")?;
        // Resolve the stable key before entering the locking transaction.  The
        // row is loaded again only after the user's unfinished-order range has
        // been locked, preserving the global order -> user -> plan sequence.
        let user_id: Option<i64> =
            sqlx::query_scalar("SELECT id FROM v2_user WHERE email = ? LIMIT 1")
                .bind(email)
                .fetch_optional(&self.db)
                .await?;
        let user_id = user_id.ok_or_else(|| ApiError::legacy("该用户不存在"))?;
        let mut tx = self.db.begin().await?;
        let has_incomplete: Option<i64> = sqlx::query_scalar(ADMIN_ASSIGN_UNFINISHED_ORDER_SQL)
            .bind(user_id)
            .fetch_optional(&mut *tx)
            .await?;
        if has_incomplete.is_some() {
            return Err(ApiError::legacy("该用户还有待支付的订单，无法分配"));
        }

        // Load the fields setInvite / setOrderType need alongside the id:
        // (id, plan_id, expired_at, invite_user_id).
        type AssignUserRow = (i64, Option<i64>, Option<i64>, Option<i64>);
        let user: Option<AssignUserRow> = sqlx::query_as(
            "SELECT id, plan_id, expired_at, invite_user_id FROM v2_user WHERE id = ? LIMIT 1 FOR UPDATE",
        )
        .bind(user_id)
        .fetch_optional(&mut *tx)
        .await?;
        let (user_id, user_plan_id, user_expired_at, user_invite_user_id) =
            user.ok_or_else(|| ApiError::legacy("该用户不存在"))?;
        let plan_exists: Option<i64> =
            sqlx::query_scalar("SELECT id FROM v2_plan WHERE id = ? LIMIT 1 FOR SHARE")
                .bind(plan_id)
                .fetch_optional(&mut *tx)
                .await?;
        if plan_exists.is_none() {
            return Err(ApiError::legacy("该订阅不存在"));
        }
        let now = Utc::now().timestamp();
        let period = required_string(params, "period")?;
        let total_amount = optional_nonnegative_i32(params, "total_amount")?.unwrap_or_default();
        // OrderController::assign order-type branches (:167-175).
        let order_type: i64 = if period == "reset_price" {
            4
        } else if user_plan_id.is_some() && user_plan_id != Some(plan_id) {
            3
        } else if user_expired_at.is_some_and(|value| value > now) && user_plan_id == Some(plan_id)
        {
            2
        } else {
            1
        };
        // OrderService::setInvite (:138-165): resolve invite_user_id + commission_balance.
        let (invite_user_id, commission_balance) = self
            .assign_invite_in_tx(&mut tx, user_id, user_invite_user_id, total_amount)
            .await?;
        let trade_no = format!("{}{}", now, Uuid::new_v4().simple());
        sqlx::query(
            r#"
            INSERT INTO v2_order (
                user_id, invite_user_id, plan_id, period, trade_no, total_amount, type,
                status, commission_status, commission_balance, created_at, updated_at
            )
            VALUES (?, ?, ?, ?, ?, ?, ?, 0, 0, ?, ?, ?)
            "#,
        )
        .bind(user_id)
        .bind(invite_user_id)
        .bind(plan_id)
        .bind(period)
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
        Ok(AdminOutput::Data(json!(trade_no)))
    }

    /// Ports OrderService::setInvite (:138-165) for the assign flow. Returns the
    /// order's `(invite_user_id, commission_balance)`. A referred user whose order
    /// is free keeps no invite link; otherwise the inviter's commission_type and
    /// commission_rate (falling back to config invite_commission) decide the cut.
    async fn assign_invite_in_tx(
        &self,
        tx: &mut Transaction<'_, MySql>,
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
        let inviter: Option<(i8, Option<i32>)> = sqlx::query_as(
            "SELECT commission_type, commission_rate FROM v2_user WHERE id = ? LIMIT 1",
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
        tx: &mut Transaction<'_, MySql>,
        user_id: i64,
    ) -> Result<bool, ApiError> {
        let found: Option<i64> = sqlx::query_scalar(
            "SELECT id FROM v2_order WHERE user_id = ? AND status NOT IN (0, 2) LIMIT 1",
        )
        .bind(user_id)
        .fetch_optional(&mut **tx)
        .await?;
        Ok(found.is_some())
    }

    pub(super) async fn plan_drop(
        &self,
        params: &HashMap<String, String>,
    ) -> Result<AdminOutput, ApiError> {
        // Ports PlanController::drop (:70-87): reject deletion while any order or
        // user still references the plan.  The whole decision is one locking
        // transaction; migration 22's conditional order FK excludes only the
        // real deposit sentinel (plan_id = 0).
        let id = required_i64(params, "id")?;
        let mut tx = self.db.begin().await?;
        let has_order: Option<i64> = sqlx::query_scalar(
            "SELECT id FROM v2_order WHERE referenced_plan_id = ? LIMIT 1 FOR UPDATE",
        )
        .bind(id)
        .fetch_optional(&mut *tx)
        .await?;
        if has_order.is_some() {
            return Err(ApiError::legacy("该订阅下存在订单无法删除"));
        }
        let has_user: Option<i64> =
            sqlx::query_scalar("SELECT id FROM v2_user WHERE plan_id = ? LIMIT 1 FOR UPDATE")
                .bind(id)
                .fetch_optional(&mut *tx)
                .await?;
        if has_user.is_some() {
            return Err(ApiError::legacy("该订阅下存在用户无法删除"));
        }
        let has_giftcard: Option<i64> =
            sqlx::query_scalar("SELECT id FROM v2_giftcard WHERE plan_id = ? LIMIT 1 FOR UPDATE")
                .bind(id)
                .fetch_optional(&mut *tx)
                .await?;
        if has_giftcard.is_some() {
            return Err(ApiError::legacy("该订阅仍被礼品卡使用，无法删除"));
        }
        let exists: Option<i64> =
            sqlx::query_scalar("SELECT id FROM v2_plan WHERE id = ? LIMIT 1 FOR UPDATE")
                .bind(id)
                .fetch_optional(&mut *tx)
                .await?;
        if exists.is_none() {
            return Err(ApiError::legacy("该订阅ID不存在"));
        }
        let deleted = sqlx::query("DELETE FROM v2_plan WHERE id = ?")
            .bind(id)
            .execute(&mut *tx)
            .await?;
        if deleted.rows_affected() != 1 {
            return Err(ApiError::legacy("该订阅ID不存在"));
        }
        tx.commit().await?;
        Ok(AdminOutput::Data(json!(true)))
    }
}
