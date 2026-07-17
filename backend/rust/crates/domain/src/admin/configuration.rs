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

fn without_redacted_config_secrets(params: &HashMap<String, String>) -> HashMap<String, String> {
    params
        .iter()
        .filter(|(key, value)| {
            !(CONFIG_SECRET_KEYS.contains(&key.as_str()) && value.as_str() == REDACTED_SECRET)
        })
        .map(|(key, value)| (key.clone(), value.clone()))
        .collect()
}

impl AdminService {
    pub(super) fn config_fetch(&self, key: Option<&str>) -> Result<AdminOutput, ApiError> {
        let data = json!({
            "ticket": { "ticket_status": self.config.ticket_status },
            "deposit": { "deposit_bounus": self.config.deposit_bounus },
            "invite": {
                "invite_force": bool_i(self.config.invite_force),
                "invite_commission": self.config.invite_commission,
                "invite_gen_limit": self.config.invite_gen_limit,
                // Ported from ConfigController::fetch (laravel .../Admin/ConfigController.php:82).
                "invite_never_expire": bool_i(self.config.invite_never_expire),
                "commission_first_time_enable": bool_i(self.config.commission_first_time_enable),
                "commission_auto_check_enable": bool_i(self.config.commission_auto_check_enable),
                "commission_withdraw_limit": self.config.commission_withdraw_limit.normalize().to_string(),
                "commission_withdraw_method": self.config.commission_withdraw_method,
                "withdraw_close_enable": bool_i(self.config.withdraw_close_enable),
                "commission_distribution_enable": bool_i(self.config.commission_distribution_enable),
                "commission_distribution_l1": self.config.commission_distribution_l1,
                "commission_distribution_l2": self.config.commission_distribution_l2,
                "commission_distribution_l3": self.config.commission_distribution_l3,
            },
            "site": {
                "logo": self.config.logo,
                "force_https": bool_i(self.config.force_https),
                "stop_register": bool_i(self.config.stop_register),
                "app_name": self.config.app_name,
                "app_description": self.config.app_description,
                "app_url": self.config.app_url,
                "subscribe_url": self.config.subscribe_url,
                "subscribe_path": self.config.subscribe_path,
                "try_out_plan_id": self.config.try_out_plan_id,
                // Ported from ConfigController::fetch (laravel .../Admin/ConfigController.php:103).
                "try_out_hour": self.config.try_out_hour.normalize().to_string(),
                "tos_url": self.config.tos_url,
                "currency": self.config.currency,
                "currency_symbol": self.config.currency_symbol,
                // docs/api-dialect.md §10.3/§12: site-group toggle for the
                // client-side legacy `#/…` hash → history-URL translation.
                "legacy_hash_redirect_enable": bool_i(self.config.legacy_hash_redirect_enable),
            },
            "subscribe": {
                "plan_change_enable": bool_i(self.config.plan_change_enable),
                "reset_traffic_method": self.config.reset_traffic_method,
                "surplus_enable": bool_i(self.config.surplus_enable),
                "allow_new_period": self.config.allow_new_period,
                "new_order_event_id": self.config.new_order_event_id,
                "renew_order_event_id": self.config.renew_order_event_id,
                "change_order_event_id": self.config.change_order_event_id,
                "show_info_to_server_enable": bool_i(self.config.show_info_to_server_enable),
                "show_subscribe_method": self.config.show_subscribe_method,
                "show_subscribe_expire": self.config.show_subscribe_expire,
            },
            "frontend": {
                "frontend_theme_color": self.config.frontend_theme_color,
                "frontend_background_url": self.config.frontend_background_url,
                "frontend_custom_html": self.config.frontend_custom_html,
            },
            "server": {
                "server_api_url": self.config.server_api_url,
                "server_token": redacted_secret(self.config.server_token.as_deref()),
                "server_pull_interval": self.config.server_pull_interval,
                "server_push_interval": self.config.server_push_interval,
                "server_node_report_min_traffic": self.config.server_node_report_min_traffic,
                "server_device_online_min_traffic": self.config.server_device_online_min_traffic,
                "device_limit_mode": self.config.device_limit_mode,
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
                "telegram_bot_enable": bool_i(self.config.telegram_bot_enable),
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
                "email_verify": bool_i(self.config.email_verify),
                "safe_mode_enable": bool_i(self.config.safe_mode_enable),
                "secure_path": self.config.admin_path(),
                "email_whitelist_enable": bool_i(self.config.email_whitelist_enable),
                "email_whitelist_suffix": self.config.email_whitelist_suffix,
                "email_gmail_limit_enable": bool_i(self.config.email_gmail_limit_enable),
                "recaptcha_enable": bool_i(self.config.recaptcha_enable),
                "recaptcha_key": redacted_secret(self.config.recaptcha_key.as_deref()),
                "recaptcha_site_key": self.config.recaptcha_site_key,
                "register_limit_by_ip_enable": bool_i(self.config.register_limit_by_ip_enable),
                "register_limit_count": self.config.register_limit_count,
                "register_limit_expire": self.config.register_limit_expire,
                "password_limit_enable": bool_i(self.config.password_limit_enable),
                "password_limit_count": self.config.password_limit_count,
                "password_limit_expire": self.config.password_limit_expire,
            },
        });
        if let Some(key) = key
            && let Some(value) = data.get(key)
        {
            return Ok(AdminOutput::Data(json!({ key: value })));
        }
        Ok(AdminOutput::Data(data))
    }

    pub(super) async fn config_save(
        &self,
        params: &HashMap<String, String>,
    ) -> Result<AdminOutput, ApiError> {
        // ConfigSave validates the payload (enums, urls, secure_path/server_token
        // length, deposit_bounus format) and 422s before anything is written.
        let actor = params
            .get("_admin_email")
            .map(|email| format!("admin:{}", email.trim()))
            .unwrap_or_else(|| "admin:unknown".to_string());
        let mut params = without_redacted_config_secrets(params);
        // The fetch response exposes the effective fallback path. Posting that
        // unchanged value must remain a no-op even when an old installation's
        // fallback is shorter than today's explicit-path validation rule.
        drop_unchanged_effective_secure_path(&mut params, &self.config.admin_path());
        validate_config_params(&params)?;
        let server_token = params
            .get("server_token")
            .map(String::as_str)
            .or(self.config.server_token.as_deref());
        let force_https = params
            .get("force_https")
            .map(|value| truthy(Some(value)))
            .unwrap_or(self.config.force_https);
        let app_url = params
            .get("app_url")
            .map(String::as_str)
            .or(self.config.app_url.as_deref());
        self.config
            .validate_security_update(server_token, force_https, app_url)
            .map_err(|error| ApiError::business(format!("配置安全校验失败: {error}")))?;
        let expected_revision = self
            .config
            .operator_revision()
            .ok_or_else(|| ApiError::internal("operator configuration authority is not active"))?;
        let mut candidate = self.config.operator_config_map();
        merge_config_params(&mut candidate, &params);
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
            ApiError::business(format!("配置校验失败: {error}"))
        })?;
        let normalized = candidate_config.operator_config_map();
        if normalized == self.config.operator_config_map() {
            return Ok(AdminOutput::Data(json!(true)));
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
                ApiError::bad_request("配置已被其他请求更新，请刷新后重试")
            }
            error => {
                tracing::error!(?error, "failed to commit operator configuration");
                ApiError::internal("operator configuration commit failed")
            }
        })?;
        Ok(AdminOutput::ConfigSaved {
            config: Box::new(candidate_config.at_operator_revision(committed.revision)),
        })
    }

    pub(super) async fn test_send_mail(
        &self,
        params: &HashMap<String, String>,
    ) -> Result<AdminOutput, ApiError> {
        let to = required_string(params, "_admin_email")?;
        self.send_test_mail(
            &to,
            "This is v2board test email",
            "This is v2board test email",
        )
        .await?;
        Ok(AdminOutput::Data(json!(true)))
    }

    pub(super) async fn send_mail_to_users(
        &self,
        params: &HashMap<String, String>,
    ) -> Result<AdminOutput, ApiError> {
        self.enqueue_mail_to_users(params, false).await
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
        let batch_key = mail_batch_key(&actor, &idempotency_key);
        let payload_hash = bulk_mail_payload_hash(params);
        let clauses = self.user_filter_clauses(params).await?;
        let now = Utc::now().timestamp();
        let mut tx = self.db.begin().await?;
        if !reserve_mail_outbox_batch(&mut tx, &batch_key, &payload_hash, &actor, now)
            .await
            .map_err(mail_outbox_api_error)?
        {
            tx.commit().await?;
            return Ok(AdminOutput::Data(json!(true)));
        }

        let envelope = self.prepare_notify_mail(&subject, &content)?;
        let mut after_id = 0_i64;
        let mut recipient_count = 0_usize;
        loop {
            let recipients = self
                .filtered_user_email_page_in_tx(&clauses, staff_scoped, after_id, &mut tx)
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

    async fn send_test_mail(&self, to: &str, subject: &str, content: &str) -> Result<(), ApiError> {
        let settings = MailSettings::load(&self.config)?;
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
            .ok_or_else(|| ApiError::legacy("Email sender is not configured"))?;
        // The configuration probe stays synchronous and uses the `notify` HTML template.
        let body = crate::mail::render_notify(
            &self.config.app_name,
            self.config.app_url.as_deref().unwrap_or_default(),
            content,
        );
        let email = Message::builder()
            .from(
                format!("{} <{}>", self.config.app_name, from)
                    .parse()
                    .map_err(|_| ApiError::legacy("Email sender is invalid"))?,
            )
            .to(to
                .parse()
                .map_err(|_| ApiError::legacy("Email recipient is invalid"))?)
            .subject(subject)
            .header(ContentType::TEXT_HTML)
            .body(body)
            .map_err(|_| ApiError::legacy("Email content is invalid"))?;

        let transport = self.smtp.transport(&transport_settings)?;
        let send = transport.send(email);
        tokio::time::timeout(
            std::time::Duration::from_secs(self.config.http_request_timeout_seconds),
            send,
        )
        .await
        .map_err(|_| ApiError::legacy("Email send timed out"))?
        .map_err(|error| ApiError::legacy(format!("Email send failed: {error}")))?;
        Ok(())
    }

    pub(super) async fn set_telegram_webhook(
        &self,
        params: &HashMap<String, String>,
    ) -> Result<AdminOutput, ApiError> {
        let token = params
            .get("telegram_bot_token")
            .map(String::as_str)
            .filter(|value| !value.trim().is_empty() && *value != REDACTED_SECRET)
            .or(self.config.telegram_bot_token.as_deref())
            .ok_or_else(|| ApiError::business("Telegram bot token cannot be empty"))?;
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
            .map_err(|_| ApiError::legacy("Telegram request failed"))?;
        let me: Value = crate::http_response::bounded_json(
            me_response,
            crate::http_response::MAX_EXTERNAL_RESPONSE_BYTES,
            "Telegram request failed",
        )
        .await?;
        if me.get("ok").and_then(Value::as_bool) != Some(true) {
            return Err(ApiError::business("Telegram token is invalid"));
        }
        let result_response = self
            .http
            .post(format!("https://api.telegram.org/bot{token}/setWebhook"))
            .json(&json!({ "url": hook_url, "secret_token": secret_token }))
            .send()
            .await
            .map_err(|_| ApiError::legacy("Telegram request failed"))?;
        let result: Value = crate::http_response::bounded_json(
            result_response,
            crate::http_response::MAX_EXTERNAL_RESPONSE_BYTES,
            "Telegram request failed",
        )
        .await?;
        if result.get("ok").and_then(Value::as_bool) != Some(true) {
            return Err(ApiError::business("Telegram webhook failed"));
        }
        Ok(AdminOutput::Data(json!(true)))
    }
}

pub(super) fn drop_unchanged_effective_secure_path(
    params: &mut HashMap<String, String>,
    effective_admin_path: &str,
) {
    if params
        .get("secure_path")
        .is_some_and(|path| path.trim_matches('/') == effective_admin_path)
    {
        params.remove("secure_path");
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
        let params = HashMap::from([
            ("server_token".to_string(), REDACTED_SECRET.to_string()),
            ("email_password".to_string(), "rotated".to_string()),
            ("app_name".to_string(), "V2Board".to_string()),
        ]);
        let filtered = without_redacted_config_secrets(&params);
        assert!(!filtered.contains_key("server_token"));
        assert_eq!(
            filtered.get("email_password").map(String::as_str),
            Some("rotated")
        );
        assert_eq!(
            filtered.get("app_name").map(String::as_str),
            Some("V2Board")
        );
    }
}
