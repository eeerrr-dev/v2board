use rust_decimal::prelude::ToPrimitive;
use v2board_compat::{Code, Problem};

use super::*;

const BULK_MAIL_MAX_RECIPIENTS: usize = 50_000;
const REDACTED_SECRET: &str = "********";
const CONFIG_SECRET_KEYS: &[&str] = &[
    "server_token",
    "email_password",
    "telegram_bot_token",
    "recaptcha_key",
];

fn redacted_secret(value: Option<&str>) -> Option<&'static str> {
    value
        .filter(|value| !value.is_empty())
        .map(|_| REDACTED_SECRET)
}

/// A round-tripped redaction sentinel is "no change", never a literal secret
/// value: the fetch view emits `********` for configured secrets and a PATCH
/// echoing that placeholder must not overwrite the stored secret.
fn without_redacted_config_secrets(body: &Map<String, Value>) -> Map<String, Value> {
    body.iter()
        .filter(|(key, value)| {
            !(CONFIG_SECRET_KEYS.contains(&key.as_str()) && value.as_str() == Some(REDACTED_SECRET))
        })
        .map(|(key, value)| (key.clone(), value.clone()))
        .collect()
}

/// PATCH `config` commit outcome (docs/api-dialect.md §6.1): `Unchanged` when
/// the normalized candidate equals the active snapshot (nothing written);
/// `Committed` carries the validated snapshot at its new revision for the API
/// layer to activate (204 on full activation, 202 `{"activation":"pending"}`
/// when the write persisted but this process could not activate it yet).
pub enum ConfigPatchOutcome {
    Unchanged,
    Committed(Box<AppConfig>),
}

impl AdminService {
    /// GET `config` `?group=` (docs/api-dialect.md §6.1): the grouped operator
    /// view in §4.1 JSON types — real booleans for every flag, JSON numbers
    /// for the unified numeric fields, real arrays, RFC 3339 having no
    /// timestamps here, and the recorded exception that
    /// `commission_withdraw_limit` keeps its exact decimal-string form.
    /// Secrets stay redacted to a fixed-width sentinel. A known `?group=`
    /// returns just that group (still keyed); an unknown or absent group
    /// returns the full object, matching the legacy `?key=` behavior.
    pub fn config_view(&self, group: Option<&str>) -> Value {
        let distribution_rate = |value: Option<&str>| {
            value
                .and_then(|value| value.trim().parse::<f64>().ok())
                .map(|value| json!(value))
                .unwrap_or(Value::Null)
        };
        let data = json!({
            "ticket": { "ticket_status": self.config.ticket_status },
            "deposit": { "deposit_bounus": self.config.deposit_bounus },
            "invite": {
                "invite_force": self.config.invite_force,
                "invite_commission": self.config.invite_commission,
                "invite_gen_limit": self.config.invite_gen_limit,
                "invite_never_expire": self.config.invite_never_expire,
                "commission_first_time_enable": self.config.commission_first_time_enable,
                "commission_auto_check_enable": self.config.commission_auto_check_enable,
                // Exact PostgreSQL NUMERIC round-trip: stays a decimal string
                // so the admin form preserves lexical value (§4.1 exception).
                "commission_withdraw_limit": self.config.commission_withdraw_limit.normalize().to_string(),
                "commission_withdraw_method": self.config.commission_withdraw_method,
                "withdraw_close_enable": self.config.withdraw_close_enable,
                "commission_distribution_enable": self.config.commission_distribution_enable,
                "commission_distribution_l1": distribution_rate(self.config.commission_distribution_l1.as_deref()),
                "commission_distribution_l2": distribution_rate(self.config.commission_distribution_l2.as_deref()),
                "commission_distribution_l3": distribution_rate(self.config.commission_distribution_l3.as_deref()),
            },
            "site": {
                "logo": self.config.logo,
                "force_https": self.config.force_https,
                "stop_register": self.config.stop_register,
                "app_name": self.config.app_name,
                "app_description": self.config.app_description,
                "app_url": self.config.app_url,
                "subscribe_url": self.config.subscribe_url,
                "subscribe_path": self.config.subscribe_path,
                "try_out_plan_id": self.config.try_out_plan_id,
                "try_out_hour": self.config.try_out_hour.to_f64(),
                "tos_url": self.config.tos_url,
                "currency": self.config.currency,
                "currency_symbol": self.config.currency_symbol,
                // docs/api-dialect.md §10.3/§12: site-group toggle for the
                // client-side legacy `#/…` hash → history-URL translation.
                "legacy_hash_redirect_enable": self.config.legacy_hash_redirect_enable,
            },
            "subscribe": {
                "plan_change_enable": self.config.plan_change_enable,
                "reset_traffic_method": self.config.reset_traffic_method,
                "surplus_enable": self.config.surplus_enable,
                "allow_new_period": self.config.allow_new_period != 0,
                "new_order_event_id": self.config.new_order_event_id != 0,
                "renew_order_event_id": self.config.renew_order_event_id != 0,
                "change_order_event_id": self.config.change_order_event_id != 0,
                "show_info_to_server_enable": self.config.show_info_to_server_enable,
                "show_subscribe_method": self.config.show_subscribe_method,
                "show_subscribe_expire": self.config.show_subscribe_expire,
            },
            "frontend": {
                "frontend_theme_color": self.config.frontend_theme_color,
                "frontend_background_url": self.config.frontend_background_url,
                // docs/api-dialect.md §10.6: typed chat-widget integration —
                // the replacement for the removed `frontend_custom_html`
                // injection path.
                "chat_widget_provider": self.config.chat_widget_provider,
                "chat_widget_crisp_website_id": self.config.chat_widget_crisp_website_id,
                "chat_widget_tawk_property_id": self.config.chat_widget_tawk_property_id,
                "chat_widget_tawk_widget_id": self.config.chat_widget_tawk_widget_id,
            },
            "server": {
                "server_api_url": self.config.server_api_url,
                "server_token": redacted_secret(self.config.server_token.as_deref()),
                "server_pull_interval": self.config.server_pull_interval,
                "server_push_interval": self.config.server_push_interval,
                "server_node_report_min_traffic": self.config.server_node_report_min_traffic,
                "server_device_online_min_traffic": self.config.server_device_online_min_traffic,
                "device_limit_mode": self.config.device_limit_mode != 0,
            },
            "email": {
                "email_template": self.config.email_template,
                "email_host": self.config.email_host,
                "email_port": self.config.email_port,
                "email_username": self.config.email_username,
                "email_password": redacted_secret(self.config.email_password.as_deref()),
                "email_encryption": self.config.email_encryption,
                "email_from_address": self.config.email_from_address,
            },
            "telegram": {
                "telegram_bot_enable": self.config.telegram_bot_enable,
                "telegram_bot_token": redacted_secret(self.config.telegram_bot_token.as_deref()),
                "telegram_discuss_link": self.config.telegram_discuss_link,
            },
            "app": {
                "windows_version": self.config.windows_version,
                "windows_download_url": self.config.windows_download_url,
                "macos_version": self.config.macos_version,
                "macos_download_url": self.config.macos_download_url,
                "android_version": self.config.android_version,
                "android_download_url": self.config.android_download_url,
            },
            "safe": {
                "email_verify": self.config.email_verify,
                "safe_mode_enable": self.config.safe_mode_enable,
                "secure_path": self.config.admin_path(),
                "email_whitelist_enable": self.config.email_whitelist_enable,
                "email_whitelist_suffix": self.config.email_whitelist_suffix,
                "email_gmail_limit_enable": self.config.email_gmail_limit_enable,
                "recaptcha_enable": self.config.recaptcha_enable,
                "recaptcha_key": redacted_secret(self.config.recaptcha_key.as_deref()),
                "recaptcha_site_key": self.config.recaptcha_site_key,
                "register_limit_by_ip_enable": self.config.register_limit_by_ip_enable,
                "register_limit_count": self.config.register_limit_count,
                "register_limit_expire": self.config.register_limit_expire,
                "password_limit_enable": self.config.password_limit_enable,
                "password_limit_count": self.config.password_limit_count,
                "password_limit_expire": self.config.password_limit_expire,
            },
        });
        if let Some(group) = group
            && let Some(value) = data.get(group)
        {
            return json!({ group: value });
        }
        data
    }

    /// GET `email-templates` (docs/api-dialect.md §6.1): a bare array of the
    /// installed template names. The native build ships only `default`.
    pub fn email_templates(&self) -> Value {
        json!(["default"])
    }
}

impl AdminService {
    /// PATCH `config` (docs/api-dialect.md §6.1): a partial JSON object of
    /// whitelisted operator settings in §4.1 native types (the `'[]'`-string
    /// array hack is dead — arrays are JSON arrays). §4.4 semantics: an
    /// absent key retains, `null` clears back to the built-in default, a
    /// value sets. A stale operator revision is the 409
    /// `config_revision_conflict` problem.
    pub async fn config_patch(
        &self,
        body: &Map<String, Value>,
        admin_email: &str,
    ) -> Result<ConfigPatchOutcome, ApiError> {
        let actor = format!("admin:{}", admin_email.trim());
        let mut body = without_redacted_config_secrets(body);
        // The fetch response exposes the effective fallback path. Patching
        // that unchanged value must remain a no-op even when an old
        // installation's fallback is shorter than today's explicit-path rule.
        drop_unchanged_effective_secure_path(&mut body, &self.config.admin_path());
        validate_config_json(&body)?;
        let server_token = match body.get("server_token") {
            Some(Value::String(value)) => Some(value.as_str()),
            Some(Value::Null) => None,
            _ => self.config.server_token.as_deref(),
        };
        let force_https = match body.get("force_https") {
            Some(Value::Bool(value)) => *value,
            _ => self.config.force_https,
        };
        let app_url = match body.get("app_url") {
            Some(Value::String(value)) => Some(value.as_str()),
            Some(Value::Null) => None,
            _ => self.config.app_url.as_deref(),
        };
        self.config
            .validate_security_update(server_token, force_https, app_url)
            .map_err(|error| {
                ApiError::from(
                    Problem::new(Code::ConfigValidationFailed)
                        .with_detail(format!("配置安全校验失败: {error}")),
                )
            })?;
        let expected_revision = self
            .config
            .operator_revision()
            .ok_or_else(|| ApiError::internal("operator configuration authority is not active"))?;
        let mut candidate = self.config.operator_config_map();
        merge_config_json(&mut candidate, &body);
        let current = self.config.clone();
        let candidate_config = tokio::task::spawn_blocking(move || {
            current.with_operator_config(&candidate, expected_revision)
        })
        .await
        .map_err(|error| {
            tracing::error!(?error, "config validation task failed");
            ApiError::internal("configuration validator is unavailable")
        })?
        .map_err(|error| {
            tracing::warn!(?error, "rejected invalid operator configuration candidate");
            ApiError::from(
                Problem::new(Code::ConfigValidationFailed)
                    .with_detail(format!("配置校验失败: {error}")),
            )
        })?;
        let normalized = candidate_config.operator_config_map();
        if normalized == self.config.operator_config_map() {
            return Ok(ConfigPatchOutcome::Unchanged);
        }
        let installation_id = self.installation_id;
        let committed = operator_config::commit(
            &self.db,
            installation_id,
            &self.config.app_key,
            Some(expected_revision),
            &normalized,
            &actor,
        )
        .await
        .map_err(|error| match error {
            operator_config::OperatorConfigError::Conflict { .. } => {
                ApiError::from(Problem::new(Code::ConfigRevisionConflict))
            }
            error => {
                tracing::error!(?error, "failed to commit operator configuration");
                ApiError::internal("operator configuration commit failed")
            }
        })?;
        Ok(ConfigPatchOutcome::Committed(Box::new(
            candidate_config.at_operator_revision(committed.revision),
        )))
    }

    /// POST `test-mail` (docs/api-dialect.md §6.1): a synchronous SMTP probe
    /// to the requesting admin. Failures are typed problems — 400
    /// `mail_sender_not_configured` / `mail_invalid`, 502 `mail_send_failed` —
    /// never queue items.
    pub async fn test_mail(&self, to: &str) -> Result<(), ApiError> {
        let settings = MailSettings::load(&self.config)
            .map_err(|_| ApiError::from(Problem::new(Code::MailSenderNotConfigured)))?;
        let transport_settings = SmtpSettings {
            host: settings.host.clone(),
            port: settings.port,
            username: settings.username.clone(),
            password: settings.password.clone(),
            encryption: settings.encryption.clone(),
            from_address: settings.from_address.clone(),
        };
        let from = settings
            .from_address
            .as_deref()
            .or(settings.username.as_deref())
            .ok_or_else(|| ApiError::from(Problem::new(Code::MailSenderNotConfigured)))?;
        // The configuration probe stays synchronous and uses the `notify` HTML
        // template.
        let body = crate::mail::render_notify(
            &self.config.app_name,
            self.config.app_url.as_deref().unwrap_or_default(),
            "This is v2board test email",
        );
        let email = Message::builder()
            .from(
                format!("{} <{}>", self.config.app_name, from)
                    .parse()
                    .map_err(|_| {
                        ApiError::from(
                            Problem::new(Code::MailSenderNotConfigured)
                                .with_detail("Email sender is invalid"),
                        )
                    })?,
            )
            .to(to.parse().map_err(|_| {
                ApiError::from(
                    Problem::new(Code::MailInvalid).with_detail("Email recipient is invalid"),
                )
            })?)
            .subject("This is v2board test email")
            .header(ContentType::TEXT_HTML)
            .body(body)
            .map_err(|_| {
                ApiError::from(
                    Problem::new(Code::MailInvalid).with_detail("Email content is invalid"),
                )
            })?;

        let transport = self.smtp.transport(&transport_settings)?;
        let send = transport.send(email);
        tokio::time::timeout(
            std::time::Duration::from_secs(self.config.http_request_timeout_seconds),
            send,
        )
        .await
        .map_err(|_| {
            ApiError::from(Problem::new(Code::MailSendFailed).with_detail("Email send timed out"))
        })?
        .map_err(|error| {
            tracing::warn!(?error, "test mail send failed");
            ApiError::from(Problem::new(Code::MailSendFailed))
        })?;
        Ok(())
    }

    /// POST `telegram-webhook` (docs/api-dialect.md §6.1): registers the §2
    /// byte-frozen guest webhook with Telegram, using the request token when
    /// given (a round-tripped redaction sentinel means "use the stored one").
    /// Problems: 400 `telegram_token_invalid`, 502 `telegram_request_failed`
    /// / `telegram_webhook_failed`.
    pub async fn set_telegram_webhook(&self, token: Option<&str>) -> Result<(), ApiError> {
        let token = token
            .map(str::trim)
            .filter(|value| !value.is_empty() && *value != REDACTED_SECRET)
            .or(self.config.telegram_bot_token.as_deref())
            .ok_or_else(|| {
                ApiError::from(
                    Problem::new(Code::TelegramTokenInvalid)
                        .with_detail("Telegram bot token cannot be empty"),
                )
            })?;
        let hook_url = format!(
            "{}/api/v1/guest/telegram/webhook",
            self.config
                .app_url
                .as_deref()
                .unwrap_or_default()
                .trim_end_matches('/')
        );
        let secret_token = telegram_webhook_secret(&self.config.app_key, token);
        let me_response = self
            .http
            .get(format!("https://api.telegram.org/bot{token}/getMe"))
            .send()
            .await
            .map_err(|_| ApiError::from(Problem::new(Code::TelegramRequestFailed)))?;
        let me: Value = crate::http_response::bounded_json(
            me_response,
            crate::http_response::MAX_EXTERNAL_RESPONSE_BYTES,
            "Telegram request failed",
        )
        .await
        .map_err(|_| ApiError::from(Problem::new(Code::TelegramRequestFailed)))?;
        if me.get("ok").and_then(Value::as_bool) != Some(true) {
            return Err(Problem::new(Code::TelegramTokenInvalid).into());
        }
        let result_response = self
            .http
            .post(format!("https://api.telegram.org/bot{token}/setWebhook"))
            .json(&json!({ "url": hook_url, "secret_token": secret_token }))
            .send()
            .await
            .map_err(|_| ApiError::from(Problem::new(Code::TelegramRequestFailed)))?;
        let result: Value = crate::http_response::bounded_json(
            result_response,
            crate::http_response::MAX_EXTERNAL_RESPONSE_BYTES,
            "Telegram request failed",
        )
        .await
        .map_err(|_| ApiError::from(Problem::new(Code::TelegramRequestFailed)))?;
        if result.get("ok").and_then(Value::as_bool) != Some(true) {
            return Err(Problem::new(Code::TelegramWebhookFailed).into());
        }
        Ok(())
    }
}

/// The payload-identifying inputs for one bulk-mail enqueue: the visible
/// envelope plus the `(actor, idempotency_key)` replay identity and its
/// canonical `payload_hash`.
struct BulkMailEnqueue<'a> {
    subject: &'a str,
    content: &'a str,
    actor: &'a str,
    idempotency_key: &'a str,
    payload_hash: &'a str,
}

impl AdminService {
    /// Shared bulk-mail enqueue over either clause representation. The batch
    /// reservation is idempotent on `(actor, idempotency_key)`: an identical
    /// payload replays as a no-op success, while a different payload under the
    /// same key is the unchanged `Idempotency-Key` conflict.
    async fn enqueue_mail_core(
        &self,
        mail: BulkMailEnqueue<'_>,
        clauses: &UserWhere<'_>,
        staff_scoped: bool,
    ) -> Result<(), ApiError> {
        let batch_key = mail_batch_key(mail.actor, mail.idempotency_key);
        let now = Utc::now().timestamp();
        let mut tx = self.db.begin().await?;
        if !reserve_mail_outbox_batch(&mut tx, &batch_key, mail.payload_hash, mail.actor, now)
            .await
            .map_err(mail_outbox_api_error)?
        {
            tx.commit().await?;
            return Ok(());
        }

        let envelope = self.prepare_notify_mail(mail.subject, mail.content)?;
        let mut after_id = 0_i64;
        let mut recipient_count = 0_usize;
        loop {
            let recipients = self
                .filtered_user_email_page_in_tx(clauses, staff_scoped, after_id, &mut tx)
                .await?;
            let Some(last_id) = recipients.last().map(|(id, _)| *id) else {
                break;
            };
            recipient_count = recipient_count.saturating_add(recipients.len());
            if recipient_count > BULK_MAIL_MAX_RECIPIENTS {
                return Err(ApiError::business(
                    "单次最多向 50000 个用户发送邮件，请缩小筛选范围",
                ));
            }
            let emails = recipients
                .into_iter()
                .map(|(_, email)| email)
                .collect::<Vec<_>>();
            enqueue_prepared_mail(&mut tx, &batch_key, &envelope, &emails, now)
                .await
                .map_err(mail_outbox_api_error)?;
            after_id = last_id;
        }
        tx.commit().await?;
        Ok(())
    }

    /// POST `users/mail` (§6.6): `{subject, content, filter?}` over the §7 DSL,
    /// with the unchanged `Idempotency-Key` replay contract. Empty 204.
    pub async fn users_mail(
        &self,
        body: &AdminUserMailBody,
        actor_email: &str,
        idempotency_key: &str,
    ) -> Result<(), ApiError> {
        if body.subject.trim().is_empty() {
            return Err(ApiError::validation_field("subject", "邮件主题不能为空"));
        }
        if body.content.trim().is_empty() {
            return Err(ApiError::validation_field("content", "邮件内容不能为空"));
        }
        let filter = body.filter.clone().unwrap_or_default();
        let resolved = filter_dsl::resolve_filters(&filter, USER_FILTER_COLUMNS)?;
        let actor = format!("admin:{actor_email}");
        // The replay identity is the canonical typed payload, so the same
        // Idempotency-Key with an identical subject/content/filter replays.
        let payload_hash = hash_mail_payload(&(&body.subject, &body.content, &body.filter));
        self.enqueue_mail_core(
            BulkMailEnqueue {
                subject: &body.subject,
                content: &body.content,
                actor: &actor,
                idempotency_key,
                payload_hash: &payload_hash,
            },
            &UserWhere::Dsl(&resolved),
            false,
        )
        .await
    }

    pub(super) async fn enqueue_mail_to_users(
        &self,
        params: &HashMap<String, String>,
        staff_scoped: bool,
    ) -> Result<AdminOutput, ApiError> {
        let subject = required_string(params, "subject")?;
        let content = required_string(params, "content")?;
        let actor_email = required_string(params, "_admin_email")?;
        let idempotency_key = optional_string(params, "_idempotency_key")
            .unwrap_or_else(|| Uuid::new_v4().to_string());
        let actor = format!(
            "{}:{actor_email}",
            if staff_scoped { "staff" } else { "admin" }
        );
        let payload_hash = bulk_mail_payload_hash(params);
        let clauses = self.user_filter_clauses(params).await?;
        self.enqueue_mail_core(
            BulkMailEnqueue {
                subject: &subject,
                content: &content,
                actor: &actor,
                idempotency_key: &idempotency_key,
                payload_hash: &payload_hash,
            },
            &UserWhere::Legacy(&clauses),
            staff_scoped,
        )
        .await?;
        Ok(AdminOutput::Data(json!(true)))
    }

    pub(super) fn prepare_notify_mail(
        &self,
        subject: &str,
        content: &str,
    ) -> Result<PreparedMailEnvelope, ApiError> {
        // Snapshot the visible message envelope at enqueue time. SMTP credentials
        // remain runtime configuration so operators can repair a broken relay
        // without rewriting durable items.
        let settings = MailSettings::load(&self.config)?;
        let from = settings
            .from_address
            .as_deref()
            .or(settings.username.as_deref())
            .ok_or_else(|| ApiError::legacy("Email sender is not configured"))?;
        let sender = format!("{} <{}>", self.config.app_name, from);
        validate_mail_sender(&sender).map_err(mail_outbox_api_error)?;
        Ok(PreparedMailEnvelope {
            sender,
            template_name: format!(
                "mail.{}.notify",
                self.config.email_template.as_deref().unwrap_or("default")
            ),
            subject: subject.to_string(),
            body: crate::mail::render_notify(
                &self.config.app_name,
                self.config.app_url.as_deref().unwrap_or_default(),
                content,
            ),
        })
    }
}

/// The GET view exposes the effective fallback admin path; a PATCH echoing
/// that unchanged value is removed before validation so it stays a no-op.
pub(super) fn drop_unchanged_effective_secure_path(
    body: &mut Map<String, Value>,
    effective_admin_path: &str,
) {
    if body
        .get("secure_path")
        .and_then(Value::as_str)
        .is_some_and(|path| path.trim_matches('/') == effective_admin_path)
    {
        body.remove("secure_path");
    }
}

pub(super) fn bulk_mail_payload_hash(params: &HashMap<String, String>) -> String {
    let canonical = params
        .iter()
        .filter(|(key, _)| !key.starts_with('_') && key.as_str() != "auth_data")
        .map(|(key, value)| (key.as_str(), value.as_str()))
        .collect::<BTreeMap<_, _>>();
    hash_mail_payload(&canonical)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn configured_secrets_are_fixed_width_and_redacted_submissions_are_noops() {
        assert_eq!(
            redacted_secret(Some("highly-sensitive")),
            Some(REDACTED_SECRET)
        );
        assert_eq!(redacted_secret(None), None);
        let body = json!({
            "server_token": REDACTED_SECRET,
            "email_password": "rotated",
            "app_name": "V2Board",
        });
        let filtered = without_redacted_config_secrets(body.as_object().expect("object"));
        assert!(!filtered.contains_key("server_token"));
        assert_eq!(
            filtered.get("email_password").and_then(Value::as_str),
            Some("rotated")
        );
        assert_eq!(
            filtered.get("app_name").and_then(Value::as_str),
            Some("V2Board")
        );
    }
}
