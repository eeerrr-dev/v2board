use super::*;

/// Only declared operator settings are persisted. Request-only fields such as
/// `_admin_email`, `auth_data`, and any stray input never reach config storage.
pub(in super::super) fn config_save_whitelisted(base: &str) -> bool {
    const KEYS: &[&str] = &[
        "deposit_bounus",
        "ticket_status",
        "invite_force",
        "invite_commission",
        "invite_gen_limit",
        "invite_never_expire",
        "commission_first_time_enable",
        "commission_auto_check_enable",
        "commission_withdraw_limit",
        "commission_withdraw_method",
        "withdraw_close_enable",
        "commission_distribution_enable",
        "commission_distribution_l1",
        "commission_distribution_l2",
        "commission_distribution_l3",
        "logo",
        "force_https",
        "stop_register",
        "app_name",
        "app_description",
        "app_url",
        "subscribe_url",
        "subscribe_path",
        "try_out_enable",
        "try_out_plan_id",
        "try_out_hour",
        "tos_url",
        "currency",
        "currency_symbol",
        "plan_change_enable",
        "reset_traffic_method",
        "surplus_enable",
        "allow_new_period",
        "new_order_event_id",
        "renew_order_event_id",
        "change_order_event_id",
        "show_info_to_server_enable",
        "show_subscribe_method",
        "show_subscribe_expire",
        "server_api_url",
        "server_token",
        "server_pull_interval",
        "server_push_interval",
        "device_limit_mode",
        "server_node_report_min_traffic",
        "server_device_online_min_traffic",
        "frontend_theme_color",
        "frontend_background_url",
        "frontend_custom_html",
        "email_template",
        "email_host",
        "email_port",
        "email_username",
        "email_password",
        "email_encryption",
        "email_from_address",
        "telegram_bot_enable",
        "telegram_bot_token",
        "telegram_discuss_id",
        "telegram_channel_id",
        "telegram_discuss_link",
        "windows_version",
        "windows_download_url",
        "macos_version",
        "macos_download_url",
        "android_version",
        "android_download_url",
        "email_whitelist_enable",
        "email_whitelist_suffix",
        "email_gmail_limit_enable",
        "recaptcha_enable",
        "recaptcha_key",
        "recaptcha_site_key",
        "email_verify",
        "safe_mode_enable",
        "register_limit_by_ip_enable",
        "register_limit_count",
        "register_limit_expire",
        "secure_path",
        "password_limit_enable",
        "password_limit_count",
        "password_limit_expire",
    ];
    KEYS.contains(&base)
}

/// Validates a config/save payload against ConfigSave::RULES before it is
/// written. Ports the enum (`in:...`), integer/numeric, url, regex and length
/// rules that guard against corrupt config; only present, non-empty values are
/// checked, matching Laravel's implicit skipping of empty optional inputs.
/// Returns the first failure as a Laravel-style 422.
pub(in super::super) fn validate_config_params(
    params: &HashMap<String, String>,
) -> Result<(), ApiError> {
    // `secure_path` is the live admin route, so an explicitly submitted empty
    // value must never be interpreted as "unset/use the fallback".  The fetch
    // round-trip of an unchanged fallback path is removed before validation.
    if params
        .get("secure_path")
        .is_some_and(|value| value.trim().is_empty())
    {
        return Err(validation_error("secure_path", "后台路径不能为空"));
    }

    let value = |key: &str| -> Option<&str> {
        params
            .get(key)
            .map(|value| value.trim())
            .filter(|value| !value.is_empty())
    };

    // These settings have exactly two accepted wire shapes: indexed form
    // (`field[0]=...`) or the literal `field=[]` used to clear the list.  A
    // bare scalar is ambiguous (and was previously parsed as a comma list by
    // AppConfig), so reject it instead of silently changing its meaning.
    for key in [
        "deposit_bounus",
        "commission_withdraw_method",
        "email_whitelist_suffix",
    ] {
        let indexed = params
            .keys()
            .filter(|raw_key| raw_key.starts_with(&format!("{key}[")))
            .collect::<Vec<_>>();
        if indexed
            .iter()
            .any(|raw_key| bracket_index(raw_key, key).is_none())
        {
            return Err(validation_error(key, "数组参数格式有误"));
        }
        if let Some(raw) = params.get(key)
            && (raw.trim() != "[]" || !indexed.is_empty())
        {
            return Err(validation_error(key, "数组参数格式有误"));
        }
    }

    const IN_0_1: &[&str] = &[
        "invite_force",
        "invite_never_expire",
        "commission_first_time_enable",
        "commission_auto_check_enable",
        "withdraw_close_enable",
        "commission_distribution_enable",
        "force_https",
        "stop_register",
        "try_out_enable",
        "plan_change_enable",
        "surplus_enable",
        "allow_new_period",
        "new_order_event_id",
        "renew_order_event_id",
        "change_order_event_id",
        "show_info_to_server_enable",
        "device_limit_mode",
        "telegram_bot_enable",
        "email_whitelist_enable",
        "email_gmail_limit_enable",
        "recaptcha_enable",
        "email_verify",
        "safe_mode_enable",
        "register_limit_by_ip_enable",
        "password_limit_enable",
    ];
    for key in IN_0_1 {
        if let Some(value) = value(key)
            && value != "0"
            && value != "1"
        {
            return Err(validation_error(key, "参数格式有误"));
        }
    }
    for key in ["ticket_status", "show_subscribe_method"] {
        if let Some(value) = value(key)
            && !matches!(value, "0" | "1" | "2")
        {
            return Err(validation_error(key, "参数格式有误"));
        }
    }
    if let Some(value) = value("reset_traffic_method")
        && !matches!(value, "0" | "1" | "2" | "3" | "4")
    {
        return Err(validation_error("reset_traffic_method", "参数格式有误"));
    }
    if let Some(value) = value("frontend_theme_color")
        && !matches!(value, "default" | "darkblue" | "black" | "green")
    {
        return Err(validation_error("frontend_theme_color", "参数格式有误"));
    }

    for (key, message) in [
        ("logo", "LOGO URL格式不正确，必须携带https(s)://"),
        ("app_url", "站点URL格式不正确，必须携带http(s)://"),
        ("tos_url", "服务条款URL格式不正确，必须携带http(s)://"),
        (
            "telegram_discuss_link",
            "Telegram群组地址必须为URL格式，必须携带http(s)://",
        ),
        ("frontend_background_url", "参数格式有误"),
    ] {
        if let Some(value) = value(key)
            && !is_valid_url(value)
        {
            return Err(validation_error(key, message));
        }
    }

    if let Some(value) = value("subscribe_path")
        && !value.starts_with('/')
    {
        return Err(validation_error("subscribe_path", "订阅路径必须以/开头"));
    }
    if let Some(value) = value("server_token")
        && value.chars().count() < 16
    {
        return Err(validation_error("server_token", "通讯密钥长度必须大于16位"));
    }
    if let Some(value) = value("secure_path") {
        if value.chars().count() < 8 {
            return Err(validation_error("secure_path", "后台路径长度最小为8位"));
        }
        if !value
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-'))
        {
            return Err(validation_error("secure_path", "后台路径只能为字母或数字"));
        }
    }

    const INTEGERS: &[&str] = &[
        "invite_commission",
        "invite_gen_limit",
        "try_out_plan_id",
        "show_subscribe_expire",
        "server_pull_interval",
        "server_push_interval",
        "server_node_report_min_traffic",
        "server_device_online_min_traffic",
        "register_limit_count",
        "register_limit_expire",
        "password_limit_count",
        "password_limit_expire",
    ];
    const DURATION_MINUTES: &[&str] = &[
        "show_subscribe_expire",
        "register_limit_expire",
        "password_limit_expire",
    ];
    for key in INTEGERS {
        if let Some(value) = value(key) {
            let parsed = value
                .parse::<i64>()
                .map_err(|_| validation_error(key, "参数格式有误"))?;
            if DURATION_MINUTES.contains(key)
                && !(1..=MAX_CONFIG_DURATION_MINUTES).contains(&parsed)
            {
                return Err(validation_error(key, "分钟数必须在安全范围内"));
            }
            let (minimum, maximum) = match *key {
                "show_subscribe_expire" | "register_limit_expire" | "password_limit_expire" => {
                    (1, MAX_CONFIG_DURATION_MINUTES)
                }
                "server_pull_interval" | "server_push_interval" => (1, i64::from(i32::MAX)),
                "register_limit_count" | "password_limit_count" => (1, i64::MAX),
                "invite_commission"
                | "try_out_plan_id"
                | "server_node_report_min_traffic"
                | "server_device_online_min_traffic" => (0, i64::from(i32::MAX)),
                "invite_gen_limit" => (0, i64::MAX),
                _ => (i64::MIN, i64::MAX),
            };
            if !(minimum..=maximum).contains(&parsed) {
                return Err(validation_error(key, "参数超出支持范围"));
            }
        }
    }
    if let Some(port) = value("email_port") {
        let port = port
            .parse::<u16>()
            .map_err(|_| validation_error("email_port", "端口格式有误"))?;
        if port == 0 {
            return Err(validation_error("email_port", "端口必须在1到65535之间"));
        }
    }
    const NUMERICS: &[&str] = &[
        "try_out_hour",
        "commission_withdraw_limit",
        "commission_distribution_l1",
        "commission_distribution_l2",
        "commission_distribution_l3",
    ];
    for key in NUMERICS {
        if let Some(value) = value(key) {
            let parsed = value
                .parse::<Decimal>()
                .map_err(|_| validation_error(key, "参数格式有误"))?;
            if parsed.is_sign_negative() {
                return Err(validation_error(key, "参数不能为负数"));
            }
            let maximum = match *key {
                "try_out_hour" => Decimal::from(i64::MAX) / Decimal::from(3_600),
                "commission_withdraw_limit" => Decimal::from(i64::MAX) / Decimal::from(100),
                _ => Decimal::MAX,
            };
            if parsed > maximum {
                return Err(validation_error(key, "参数超出支持范围"));
            }
        }
    }

    // deposit_bounus[] tiers must match `<amount>:<bounus>` (empty tiers allowed).
    for tier in json_array_param(params, "deposit_bounus") {
        let Value::String(tier) = tier else {
            return Err(validation_error("deposit_bounus", "数组参数格式有误"));
        };
        let tier = tier.trim();
        if tier.is_empty() {
            continue;
        }
        if !is_deposit_bounus_tier(tier) {
            return Err(validation_error(
                "deposit_bounus",
                "充值奖励格式不正确，必须为充值金额:奖励金额",
            ));
        }
    }
    Ok(())
}

/// Ports `CouponGenerate::rules()`. Every failable rule declares a custom Chinese
/// message, so this returns the first failure in Laravel's field-declaration
/// order with the exact message the FormRequest emits (HTTP 422).
pub(in super::super) fn coupon_generate_validation(
    params: &HashMap<String, String>,
) -> Result<(), ApiError> {
    // generate_count: nullable|integer|max:500
    if let Some(value) = present_value(params, "generate_count") {
        let Ok(count) = value.parse::<i64>() else {
            return Err(validation_error("generate_count", "生成数量必须为数字"));
        };
        if count > 500 {
            return Err(validation_error("generate_count", "生成数量最大为500个"));
        }
    }
    // name: required
    if present_value(params, "name").is_none() {
        return Err(validation_error("name", "名称不能为空"));
    }
    // type: required|in:1,2
    let coupon_type = match present_value(params, "type") {
        None => return Err(validation_error("type", "类型不能为空")),
        Some(value) if !matches!(value, "1" | "2") => {
            return Err(validation_error("type", "类型格式有误"));
        }
        Some(value) => value,
    };
    // value: required|integer
    match present_value(params, "value") {
        None => return Err(validation_error("value", "金额或比例不能为空")),
        Some(value) => match value.parse::<i64>() {
            Ok(value)
                if (0..=i64::from(i32::MAX)).contains(&value)
                    && (coupon_type != "2" || value <= 100) => {}
            _ => return Err(validation_error("value", "金额或比例格式有误")),
        },
    }
    // started_at / ended_at: required|integer
    for (key, required_msg, integer_msg) in [
        ("started_at", "开始时间不能为空", "开始时间格式有误"),
        ("ended_at", "结束时间不能为空", "结束时间格式有误"),
    ] {
        match present_value(params, key) {
            None => return Err(validation_error(key, required_msg)),
            Some(value) if value.parse::<i64>().is_err() => {
                return Err(validation_error(key, integer_msg));
            }
            _ => {}
        }
    }
    // limit_use / limit_use_with_user: nullable|integer
    for (key, integer_msg) in [
        ("limit_use", "最大使用次数格式有误"),
        ("limit_use_with_user", "限制用户使用次数格式有误"),
    ] {
        if let Some(value) = present_value(params, key)
            && value.parse::<i64>().is_err()
        {
            return Err(validation_error(key, integer_msg));
        }
    }
    // limit_plan_ids / limit_period: nullable|array. A scalar (non-bracketed)
    // value fails the `array` rule; a bracketed `key[..]` submission is an array
    // and passes, as does absence.
    for (key, array_msg) in [
        ("limit_plan_ids", "指定订阅格式有误"),
        ("limit_period", "指定周期格式有误"),
    ] {
        if present_value(params, key).is_some() {
            return Err(validation_error(key, array_msg));
        }
    }
    Ok(())
}

/// Ports `GiftcardGenerate::rules()`. `value` and `plan_id` use `required_if`, not
/// `required` — so V2Board's `value.required`/`plan_id.required` custom messages
/// never fire, and with no `zh-CN/validation.php` lang file the `required_if`,
/// `integer` fallbacks surface the untranslated key (e.g. `validation.required_if`)
/// exactly as the real backend does at HTTP 422.
pub(in super::super) fn giftcard_generate_validation(
    params: &HashMap<String, String>,
) -> Result<(), ApiError> {
    // generate_count: nullable|integer|max:500
    if let Some(value) = present_value(params, "generate_count") {
        let Ok(count) = value.parse::<i64>() else {
            return Err(validation_error("generate_count", "生成数量必须为数字"));
        };
        if count > 500 {
            return Err(validation_error("generate_count", "生成数量最大为500个"));
        }
    }
    // name: required
    if present_value(params, "name").is_none() {
        return Err(validation_error("name", "名称不能为空"));
    }
    // type: required|in:1,2,3,4,5
    let card_type = match present_value(params, "type") {
        None => return Err(validation_error("type", "类型不能为空")),
        Some(value) if !matches!(value, "1" | "2" | "3" | "4" | "5") => {
            return Err(validation_error("type", "类型格式有误"));
        }
        Some(value) => value,
    };
    // value: required_if:type,1,2,3,5 | nullable | integer
    match present_value(params, "value") {
        None if matches!(card_type, "1" | "2" | "3" | "5") => {
            return Err(validation_error("value", "validation.required_if"));
        }
        Some(value) => match value.parse::<i64>() {
            Ok(value) if (0..=i64::from(i32::MAX)).contains(&value) => {}
            _ => return Err(validation_error("value", "数值格式有误")),
        },
        None => {}
    }
    // plan_id: required_if:type,5 | nullable | integer (no custom messages)
    match present_value(params, "plan_id") {
        None if card_type == "5" => {
            return Err(validation_error("plan_id", "validation.required_if"));
        }
        Some(value) if value.parse::<i64>().is_err() => {
            return Err(validation_error("plan_id", "validation.integer"));
        }
        _ => {}
    }
    // started_at / ended_at: required|integer
    for (key, required_msg, integer_msg) in [
        ("started_at", "开始时间不能为空", "开始时间格式有误"),
        ("ended_at", "结束时间不能为空", "结束时间格式有误"),
    ] {
        match present_value(params, key) {
            None => return Err(validation_error(key, required_msg)),
            Some(value) if value.parse::<i64>().is_err() => {
                return Err(validation_error(key, integer_msg));
            }
            _ => {}
        }
    }
    // limit_use: nullable|integer
    if let Some(value) = present_value(params, "limit_use")
        && value.parse::<i64>().is_err()
    {
        return Err(validation_error("limit_use", "最大使用次数格式有误"));
    }
    Ok(())
}

/// Ports `UserGenerate::rules()`. Only `generate_count` declares custom messages;
/// `expired_at`/`plan_id` (`integer`) and `email_suffix` (`required`) fall back to
/// the untranslated validation keys because there is no `zh-CN/validation.php`.
pub(in super::super) fn user_generate_validation(
    params: &HashMap<String, String>,
) -> Result<(), ApiError> {
    // generate_count: nullable|integer|max:500
    if let Some(value) = present_value(params, "generate_count") {
        let Ok(count) = value.parse::<i64>() else {
            return Err(validation_error("generate_count", "生成数量必须为数字"));
        };
        if count > 500 {
            return Err(validation_error("generate_count", "生成数量最大为500个"));
        }
    }
    // expired_at / plan_id: nullable|integer
    for key in ["expired_at", "plan_id"] {
        if let Some(value) = present_value(params, key)
            && value.parse::<i64>().is_err()
        {
            return Err(validation_error(key, "validation.integer"));
        }
    }
    // email_suffix: required
    if present_value(params, "email_suffix").is_none() {
        return Err(validation_error("email_suffix", "validation.required"));
    }
    Ok(())
}

/// Matches ConfigSave's deposit_bounus regex `^\d+(\.\d+)?:\d+(\.\d+)?$`.
fn is_deposit_bounus_tier(tier: &str) -> bool {
    let Some((amount, bounus)) = tier.split_once(':') else {
        return false;
    };
    is_unsigned_decimal(amount) && is_unsigned_decimal(bounus)
}

fn is_unsigned_decimal(value: &str) -> bool {
    let (int_part, frac_part) = match value.split_once('.') {
        Some((int_part, frac_part)) => (int_part, Some(frac_part)),
        None => (value, None),
    };
    if int_part.is_empty() || !int_part.bytes().all(|byte| byte.is_ascii_digit()) {
        return false;
    }
    match frac_part {
        Some(frac) => !frac.is_empty() && frac.bytes().all(|byte| byte.is_ascii_digit()),
        None => true,
    }
}

pub(in super::super) fn merge_config_params(
    config: &mut Map<String, Value>,
    params: &HashMap<String, String>,
) {
    let mut arrays = BTreeMap::<String, BTreeMap<usize, Value>>::new();
    for (key, value) in params {
        if let Some((base, index)) = key
            .split_once('[')
            .and_then(|(base, rest)| rest.strip_suffix(']').map(|rest| (base, rest)))
            .and_then(|(base, index)| index.parse::<usize>().ok().map(|index| (base, index)))
        {
            if !config_save_whitelisted(base) {
                continue;
            }
            let value = value.trim();
            if !value.is_empty() {
                arrays
                    .entry(base.to_string())
                    .or_default()
                    .insert(index, Value::String(value.to_string()));
            } else {
                // Preserve an explicitly submitted, all-empty array as an
                // empty list rather than falling back to the previous value.
                arrays.entry(base.to_string()).or_default();
            }
            continue;
        }
        if !config_save_whitelisted(key) {
            continue;
        }
        if matches!(
            key.as_str(),
            "deposit_bounus" | "commission_withdraw_method" | "email_whitelist_suffix"
        ) && value.trim() == "[]"
        {
            config.insert(key.clone(), Value::Array(Vec::new()));
            continue;
        }
        if key == "email_port" && value.trim().is_empty() {
            config.insert(key.clone(), Value::Null);
            continue;
        }
        // Preserve the submitted lexical value until AppConfig performs typed
        // parsing. This avoids f64 round-trips for exact decimals and prevents
        // numeric-looking passwords/tokens from losing leading zeroes.
        config.insert(key.clone(), Value::String(value.clone()));
    }
    for (key, values) in arrays {
        config.insert(key, Value::Array(values.into_values().collect()));
    }
}

/// PHP `array_filter()` (no callback) drops falsy scalars: '', '0', 0, 0.0,
/// false, null, and empty arrays/objects.
pub(in super::super) fn php_falsy(value: &Value) -> bool {
    match value {
        Value::Null => true,
        Value::Bool(value) => !value,
        Value::Number(value) => value.as_f64().map(|value| value == 0.0).unwrap_or(false),
        Value::String(value) => value.is_empty() || value == "0",
        Value::Array(items) => items.is_empty(),
        Value::Object(object) => object.is_empty(),
    }
}

/// Reconstructs the route `match` values from either a raw JSON-array string or
/// bracketed `match[i]` params. Mirrors the `(array)($params['match'] ?? [])`
/// cast in RouteController::save.
pub(in super::super) fn route_match_values(params: &HashMap<String, String>) -> Vec<Value> {
    if let Some(raw) = params.get("match")
        && let Ok(Value::Array(items)) = serde_json::from_str::<Value>(raw)
    {
        return items;
    }
    json_array_param(params, "match")
}
