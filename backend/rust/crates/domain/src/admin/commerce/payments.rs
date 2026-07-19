use super::*;
use serde::Deserialize;
use v2board_compat::json::{double_option, rfc3339};

pub(in super::super) fn resolve_redacted_payment_config(
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

/// Admin reads decrypt the stored at-rest envelope back to the plaintext
/// gateway config before redaction/merging, so the wire keeps the redacted
/// PLAINTEXT shape and never shows the envelope. A stored non-envelope config
/// is a hard integrity error — there is no plaintext fallback.
fn decrypt_stored_payment_config(
    app_key: &str,
    payment: &str,
    uuid: &str,
    raw: &str,
) -> Result<Value, ApiError> {
    crate::payment_secrets::decrypt_payment_config(app_key, payment, uuid, raw).map_err(|error| {
        ApiError::internal(format!("stored payment config failed decryption: {error}"))
    })
}

pub(in super::super) fn parse_payment_config(raw: &str) -> Result<Value, ApiError> {
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

/// §6.2: `handling_fee_percent` crosses the wire as a JSON number; the exact
/// legacy 0.1–100 range check runs on the decimal representation.
pub(super) fn handling_fee_percent_decimal(
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

impl AdminService {
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
                    &decrypt_stored_payment_config(
                        &self.config.app_key,
                        &row.payment,
                        &row.uuid,
                        &row.config,
                    )?,
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
            let (stored_payment, stored_uuid, raw_config) =
                sqlx::query_as::<_, (String, String, String)>(
                    "SELECT payment, uuid, CAST(config AS TEXT) FROM payment_method \
                     WHERE id = $1 AND archived_at IS NULL LIMIT 1",
                )
                .bind(id)
                .fetch_optional(&self.db)
                .await?
                .ok_or_else(|| ApiError::from(Problem::new(Code::PaymentMethodNotFound)))?;
            if stored_payment == code {
                Some(crate::payment_provider::redact_payment_config(
                    &stored_payment,
                    &decrypt_stored_payment_config(
                        &self.config.app_key,
                        &stored_payment,
                        &stored_uuid,
                        &raw_config,
                    )?,
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
        // The uuid participates in the at-rest AAD, so it is generated before
        // the resolved plaintext is sealed into the stored envelope.
        let uuid = random_payment_uuid();
        let stored_config = match &config_value {
            Value::Object(config) => crate::payment_secrets::encrypt_payment_config(
                &self.config.app_key,
                &body.payment,
                &uuid,
                config,
            )
            .map_err(|error| {
                ApiError::internal(format!("payment config encryption failed: {error}"))
            })?,
            _ => return Err(validation_error("config", "配置参数格式有误")),
        };
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
        .bind(uuid)
        .bind(Json(stored_config))
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
        let current = sqlx::query_as::<_, (String, String, String)>(
            "SELECT payment, uuid, CAST(config AS TEXT) FROM payment_method \
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
                // The immutability comparison runs on the DECRYPTED plaintext:
                // redacted round trips resolve against the real stored secrets,
                // exactly as before encryption at rest.
                let current_config = decrypt_stored_payment_config(
                    &self.config.app_key,
                    &current.0,
                    &current.1,
                    &current.2,
                )?;
                let current_config_text = current_config.to_string();
                let resolved = resolve_redacted_payment_config(
                    &current.0,
                    Some((&current.0, &current_config_text)),
                    Value::Object(config.clone()),
                )?;
                current_config != resolved
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
}
