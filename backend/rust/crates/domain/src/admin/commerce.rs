use super::*;

// A callback number is pre-bound only for Payment Element. Every gateway has an
// in-flight external charge as soon as a pending order carries its payment_id.
pub(super) const PENDING_PAYMENT_ORDER_SQL: &str = r#"
    SELECT id FROM v2_order
    WHERE payment_id = ? AND status = 0
    LIMIT 1
    FOR UPDATE
"#;

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
        if let Some(id) = id {
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
            .bind(required_i64(params, "group_id")?)
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
                .bind(checked_gib_bytes(
                    transfer_enable,
                    "transfer_enable",
                )?)
                .bind(device_limit)
                .bind(speed_limit)
                .bind(now)
                .bind(id)
                .execute(&self.db)
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
            .bind(required_i64(params, "group_id")?)
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
            .execute(&self.db)
            .await?;
        }
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
            ORDER BY sort ASC
            "#,
        )
        .fetch_all(&self.db)
        .await?;
        let data = rows
            .into_iter()
            .map(|row| {
                let config = parse_payment_config(&row.config)?;
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
                }))
            })
            .collect::<Result<Vec<_>, ApiError>>()?;
        Ok(AdminOutput::Data(json!(data)))
    }

    pub(super) async fn payment_form(
        &self,
        params: &HashMap<String, String>,
    ) -> Result<AdminOutput, ApiError> {
        let payment = params
            .get("payment")
            .map(String::as_str)
            .unwrap_or_default();
        let config = if let Some(id) = optional_i64(params, "id") {
            let raw_config = sqlx::query_scalar::<_, String>(
                "SELECT CAST(config AS CHAR) FROM v2_payment WHERE id = ? LIMIT 1",
            )
            .bind(id)
            .fetch_optional(&self.db)
            .await?
            .ok_or_else(|| ApiError::legacy("支付方式不存在"))?;
            Some(parse_payment_config(&raw_config)?)
        } else {
            None
        };
        Ok(AdminOutput::Data(payment_provider_form(
            payment,
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
        let config_value = nested_json(params, "config");
        let config = serde_json::to_string(&config_value)
            .map_err(|_| ApiError::internal("failed to encode payment config"))?;
        let now = Utc::now().timestamp();
        if let Some(id) = optional_i64(params, "id") {
            let mut tx = self.db.begin().await?;
            let current = sqlx::query_as::<_, (String, String)>(
                "SELECT payment, CAST(config AS CHAR) FROM v2_payment WHERE id = ? LIMIT 1 FOR UPDATE",
            )
            .bind(id)
            .fetch_optional(&mut *tx)
            .await?
            .ok_or_else(|| ApiError::legacy("支付方式不存在"))?;
            let config_changed = serde_json::from_str::<Value>(&current.1)
                .map(|value| value != config_value)
                .unwrap_or(true);
            if pending_order_blocks_payment_update(
                Self::has_pending_payment_order_in_tx(&mut tx, id).await?,
                current.0 != payment,
                config_changed,
            ) {
                return Err(ApiError::legacy(
                    "该支付方式仍有待支付订单，暂不能修改网关或配置",
                ));
            }
            sqlx::query(
                r#"
                UPDATE v2_payment
                SET name = ?, icon = ?, payment = ?, config = ?, notify_domain = ?,
                    handling_fee_fixed = ?, handling_fee_percent = ?, updated_at = ?
                WHERE id = ?
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
            .bind(random_short())
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
        let exists: Option<i64> =
            sqlx::query_scalar("SELECT id FROM v2_payment WHERE id = ? LIMIT 1 FOR UPDATE")
                .bind(id)
                .fetch_optional(&mut *tx)
                .await?;
        if exists.is_none() {
            return Err(ApiError::legacy("支付方式不存在"));
        }
        if Self::has_pending_payment_order_in_tx(&mut tx, id).await? {
            return Err(ApiError::legacy("该支付方式仍有待支付订单，暂不能删除"));
        }
        let deleted = sqlx::query("DELETE FROM v2_payment WHERE id = ?")
            .bind(id)
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        Ok(AdminOutput::Data(json!(deleted.rows_affected() != 0)))
    }

    async fn has_pending_payment_order_in_tx(
        tx: &mut sqlx::Transaction<'_, MySql>,
        payment_id: i64,
    ) -> Result<bool, ApiError> {
        let order_id: Option<i64> = sqlx::query_scalar(PENDING_PAYMENT_ORDER_SQL)
            .bind(payment_id)
            .fetch_optional(&mut **tx)
            .await?;
        Ok(order_id.is_some())
    }

    pub(super) async fn payment_show(&self, id: i64) -> Result<AdminOutput, ApiError> {
        // PaymentController::show aborts 500 when the id does not exist before
        // flipping the enable flag. This toggle intentionally remains available
        // with pending orders: it stops new checkouts, while authenticated
        // in-flight callbacks continue using the immutable driver/config binding.
        let exists: Option<i64> =
            sqlx::query_scalar("SELECT id FROM v2_payment WHERE id = ? LIMIT 1")
                .bind(id)
                .fetch_optional(&self.db)
                .await?;
        if exists.is_none() {
            return Err(ApiError::legacy("支付方式不存在"));
        }
        self.toggle(
            "v2_payment",
            "enable",
            id,
            ApiError::legacy("支付方式不存在"),
        )
        .await
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
        // Load the fields setInvite / setOrderType need alongside the id:
        // (id, plan_id, expired_at, invite_user_id).
        type AssignUserRow = (i64, Option<i64>, Option<i64>, Option<i64>);
        let user: Option<AssignUserRow> = sqlx::query_as(
            "SELECT id, plan_id, expired_at, invite_user_id FROM v2_user WHERE email = ? LIMIT 1",
        )
        .bind(email)
        .fetch_optional(&self.db)
        .await?;
        let (user_id, user_plan_id, user_expired_at, user_invite_user_id) =
            user.ok_or_else(|| ApiError::legacy("该用户不存在"))?;
        let plan_exists: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM v2_plan WHERE id = ?")
            .bind(plan_id)
            .fetch_one(&self.db)
            .await?;
        if plan_exists == 0 {
            return Err(ApiError::legacy("该订阅不存在"));
        }
        // UserService::isNotCompleteOrderByUserId: a pending/opening order blocks assign.
        let has_incomplete: Option<i64> = sqlx::query_scalar(
            "SELECT id FROM v2_order WHERE user_id = ? AND status IN (0, 1) LIMIT 1",
        )
        .bind(user_id)
        .fetch_optional(&self.db)
        .await?;
        if has_incomplete.is_some() {
            return Err(ApiError::legacy("该用户还有待支付的订单，无法分配"));
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
            .assign_invite(user_id, user_invite_user_id, total_amount)
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
        .execute(&self.db)
        .await?;
        Ok(AdminOutput::Data(json!(trade_no)))
    }

    /// Ports OrderService::setInvite (:138-165) for the assign flow. Returns the
    /// order's `(invite_user_id, commission_balance)`. A referred user whose order
    /// is free keeps no invite link; otherwise the inviter's commission_type and
    /// commission_rate (falling back to config invite_commission) decide the cut.
    async fn assign_invite(
        &self,
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
        .fetch_optional(&self.db)
        .await?;
        let Some((commission_type, commission_rate)) = inviter else {
            // invite_user_id is still recorded even when the inviter is gone.
            return Ok((Some(inviter_id), 0));
        };
        let is_commission = match commission_type {
            0 => {
                !self.config.commission_first_time_enable
                    || !self.user_have_valid_order(user_id).await?
            }
            1 => true,
            2 => !self.user_have_valid_order(user_id).await?,
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
    async fn user_have_valid_order(&self, user_id: i64) -> Result<bool, ApiError> {
        let found: Option<i64> = sqlx::query_scalar(
            "SELECT id FROM v2_order WHERE user_id = ? AND status NOT IN (0, 2) LIMIT 1",
        )
        .bind(user_id)
        .fetch_optional(&self.db)
        .await?;
        Ok(found.is_some())
    }

    pub(super) async fn plan_drop(
        &self,
        params: &HashMap<String, String>,
    ) -> Result<AdminOutput, ApiError> {
        // Ports PlanController::drop (:70-87): reject deletion while any order or
        // user still references the plan.
        let id = required_i64(params, "id")?;
        let has_order: Option<i64> =
            sqlx::query_scalar("SELECT id FROM v2_order WHERE plan_id = ? LIMIT 1")
                .bind(id)
                .fetch_optional(&self.db)
                .await?;
        if has_order.is_some() {
            return Err(ApiError::legacy("该订阅下存在订单无法删除"));
        }
        let has_user: Option<i64> =
            sqlx::query_scalar("SELECT id FROM v2_user WHERE plan_id = ? LIMIT 1")
                .bind(id)
                .fetch_optional(&self.db)
                .await?;
        if has_user.is_some() {
            return Err(ApiError::legacy("该订阅下存在用户无法删除"));
        }
        let exists: Option<i64> = sqlx::query_scalar("SELECT id FROM v2_plan WHERE id = ? LIMIT 1")
            .bind(id)
            .fetch_optional(&self.db)
            .await?;
        if exists.is_none() {
            return Err(ApiError::legacy("该订阅ID不存在"));
        }
        sqlx::query("DELETE FROM v2_plan WHERE id = ?")
            .bind(id)
            .execute(&self.db)
            .await?;
        Ok(AdminOutput::Data(json!(true)))
    }
}
