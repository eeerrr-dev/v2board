use serde_json::{Map, Value, json};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PaymentProviderManifest {
    pub code: &'static str,
    pub fields: &'static [PaymentProviderField],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PaymentProviderField {
    pub key: &'static str,
    pub label: &'static str,
    pub description: &'static str,
    pub kind: PaymentProviderFieldKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PaymentProviderFieldKind {
    Input,
    Secret,
}

pub const REDACTED_PAYMENT_SECRET: &str = "********";

const fn input_field(
    key: &'static str,
    label: &'static str,
    description: &'static str,
) -> PaymentProviderField {
    PaymentProviderField {
        key,
        label,
        description,
        kind: PaymentProviderFieldKind::Input,
    }
}

const fn secret_field(
    key: &'static str,
    label: &'static str,
    description: &'static str,
) -> PaymentProviderField {
    PaymentProviderField {
        key,
        label,
        description,
        kind: PaymentProviderFieldKind::Secret,
    }
}

pub const PAYMENT_PROVIDER_MANIFESTS: &[PaymentProviderManifest] = &[
    PaymentProviderManifest {
        code: "AlipayF2F",
        fields: &[
            input_field("app_id", "支付宝APPID", ""),
            secret_field("private_key", "支付宝私钥", ""),
            input_field("public_key", "支付宝公钥", ""),
            input_field("product_name", "自定义商品名称", "将会体现在支付宝账单中"),
        ],
    },
    PaymentProviderManifest {
        code: "BEasyPaymentUSDT",
        fields: &[
            input_field(
                "bepusdt_url",
                "API 地址",
                "您的 BEPUSDT API 接口地址(例如: https://xxx.com)",
            ),
            secret_field(
                "bepusdt_apitoken",
                "API Token",
                "您的 BEPUSDT API Token。兼容旧网关协议：签名算法为 MD5；仅通过 HTTPS 使用并优先迁移到支持 HMAC/非对称签名的网关。",
            ),
            input_field("bepusdt_trade_type", "交易类型", "您的 BEPUSDT 交易类型"),
        ],
    },
    PaymentProviderManifest {
        code: "BTCPay",
        fields: &[
            input_field("btcpay_url", "API接口所在网址(包含最后的斜杠)", ""),
            input_field("btcpay_storeId", "storeId", ""),
            secret_field(
                "btcpay_api_key",
                "API KEY",
                "个人设置中的API KEY(非商店设置中的)",
            ),
            secret_field("btcpay_webhook_key", "WEBHOOK KEY", ""),
        ],
    },
    PaymentProviderManifest {
        code: "CoinPayments",
        fields: &[
            input_field(
                "coinpayments_merchant_id",
                "Merchant ID",
                "商户 ID，填写您在 Account Settings 中得到的 ID",
            ),
            secret_field(
                "coinpayments_ipn_secret",
                "IPN Secret",
                "通知密钥，填写您在 Merchant Settings 中自行设置的值",
            ),
            input_field(
                "coinpayments_currency",
                "货币代码",
                "填写您的货币代码（大写），建议与 Merchant Settings 中的值相同",
            ),
        ],
    },
    PaymentProviderManifest {
        code: "Coinbase",
        fields: &[
            input_field("coinbase_url", "接口地址", ""),
            secret_field("coinbase_api_key", "API KEY", ""),
            secret_field("coinbase_webhook_key", "WEBHOOK KEY", ""),
        ],
    },
    PaymentProviderManifest {
        code: "EPay",
        fields: &[
            input_field("url", "URL", ""),
            input_field("pid", "PID", ""),
            secret_field(
                "key",
                "KEY",
                "兼容旧网关协议：签名算法为 MD5；仅通过 HTTPS 使用并优先迁移到支持 HMAC/非对称签名的网关。",
            ),
            input_field("type", "TYPE", "支付类型，如: alipay, wxpay, qqpay"),
        ],
    },
    PaymentProviderManifest {
        code: "MGate",
        fields: &[
            input_field("mgate_url", "API地址", ""),
            input_field("mgate_app_id", "APPID", ""),
            secret_field(
                "mgate_app_secret",
                "AppSecret",
                "兼容旧网关协议：签名算法为 MD5；仅通过 HTTPS 使用并优先迁移到支持 HMAC/非对称签名的网关。",
            ),
            input_field("mgate_source_currency", "源货币", "默认CNY"),
        ],
    },
    PaymentProviderManifest {
        code: "StripeALL",
        fields: &[
            input_field(
                "currency",
                "货币单位",
                "请使用符合ISO 4217标准的三位字母，例如GBP",
            ),
            secret_field("stripe_sk_live", "SK_LIVE", ""),
            secret_field("stripe_webhook_key", "WebHook密钥签名", "whsec_...."),
            input_field(
                "payment_method",
                "支付方式",
                "请输入alipay, wechat_pay, cards",
            ),
        ],
    },
    PaymentProviderManifest {
        code: "StripeAlipay",
        fields: &[
            input_field("currency", "货币单位", ""),
            secret_field("stripe_sk_live", "SK_LIVE", ""),
            secret_field("stripe_webhook_key", "WebHook密钥签名", ""),
        ],
    },
    PaymentProviderManifest {
        code: "StripeCheckout",
        fields: &[
            input_field("currency", "货币单位", ""),
            secret_field("stripe_sk_live", "SK_LIVE", "API 密钥"),
            input_field("stripe_pk_live", "PK_LIVE", "API 公钥"),
            secret_field("stripe_webhook_key", "WebHook 密钥签名", ""),
            input_field(
                "stripe_custom_field_name",
                "自定义字段名称",
                "例如可设置为“联系方式”，以便及时与客户取得联系",
            ),
        ],
    },
    PaymentProviderManifest {
        code: "StripeCredit",
        fields: &[
            input_field("currency", "货币单位", ""),
            secret_field("stripe_sk_live", "SK_LIVE", ""),
            input_field("stripe_pk_live", "PK_LIVE", ""),
            secret_field("stripe_webhook_key", "WebHook密钥签名", ""),
        ],
    },
    PaymentProviderManifest {
        code: "StripeWepay",
        fields: &[
            input_field("currency", "货币单位", ""),
            secret_field("stripe_sk_live", "SK_LIVE", ""),
            secret_field("stripe_webhook_key", "WebHook密钥签名", ""),
        ],
    },
    PaymentProviderManifest {
        code: "WechatPayNative",
        fields: &[
            input_field("app_id", "APPID", "绑定微信支付商户的APPID"),
            input_field("mch_id", "商户号", "微信支付商户号"),
            secret_field(
                "api_key",
                "APIKEY(v1)",
                "微信支付 v1 兼容模式使用 MD5；新部署应优先采用支持现代签名的支付接口。",
            ),
        ],
    },
];

pub fn payment_provider_manifest(code: &str) -> Option<&'static PaymentProviderManifest> {
    PAYMENT_PROVIDER_MANIFESTS
        .iter()
        .find(|provider| provider.code == code)
}

pub fn payment_provider_codes() -> Vec<&'static str> {
    PAYMENT_PROVIDER_MANIFESTS
        .iter()
        .map(|provider| provider.code)
        .collect()
}

pub fn payment_provider_uses_legacy_md5(code: &str) -> bool {
    matches!(
        code,
        "BEasyPaymentUSDT" | "EPay" | "MGate" | "WechatPayNative"
    )
}

pub fn payment_provider_security_warning(code: &str) -> Option<&'static str> {
    payment_provider_uses_legacy_md5(code).then_some(
        "Legacy MD5 signature protocol: require HTTPS and migrate to a provider with HMAC or asymmetric signatures when available.",
    )
}

pub fn redact_payment_config(code: &str, config: &Value) -> Value {
    let Some(source) = config.as_object() else {
        return Value::Object(Map::new());
    };
    let Some(provider) = payment_provider_manifest(code) else {
        // Compatibility may retain an old external provider code. Its schema is
        // unknown, so every stored value is potentially secret; preserve only
        // field names for operator orientation.
        return Value::Object(
            source
                .keys()
                .map(|key| {
                    (
                        key.clone(),
                        Value::String(REDACTED_PAYMENT_SECRET.to_string()),
                    )
                })
                .collect(),
        );
    };
    let mut redacted = Map::new();
    for field in provider.fields {
        let Some(value) = source.get(field.key) else {
            continue;
        };
        let value = match field.kind {
            PaymentProviderFieldKind::Secret if !value.is_null() && value.as_str() != Some("") => {
                Value::String(REDACTED_PAYMENT_SECRET.to_string())
            }
            PaymentProviderFieldKind::Secret => Value::String(String::new()),
            PaymentProviderFieldKind::Input => value
                .as_str()
                .map(|value| Value::String(value.to_string()))
                .unwrap_or_else(|| Value::String(REDACTED_PAYMENT_SECRET.to_string())),
        };
        redacted.insert(field.key.to_string(), value);
    }
    Value::Object(redacted)
}

pub fn payment_provider_form(code: &str, config: Option<&Value>) -> Value {
    let Some(provider) = payment_provider_manifest(code) else {
        return json!({});
    };
    let mut form = Map::new();
    for field in provider.fields {
        let mut item = Map::new();
        item.insert("label".to_string(), json!(field.label));
        item.insert("description".to_string(), json!(field.description));
        item.insert("type".to_string(), json!(field_type_name(field.kind)));
        if let Some(value) = config.and_then(|config| config.get(field.key)) {
            item.insert("value".to_string(), value.clone());
        }
        form.insert(field.key.to_string(), Value::Object(item));
    }
    Value::Object(form)
}

fn field_type_name(kind: PaymentProviderFieldKind) -> &'static str {
    match kind {
        PaymentProviderFieldKind::Input | PaymentProviderFieldKind::Secret => "input",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn provider_codes_match_legacy_builtin_plugins() {
        assert_eq!(
            payment_provider_codes(),
            vec![
                "AlipayF2F",
                "BEasyPaymentUSDT",
                "BTCPay",
                "CoinPayments",
                "Coinbase",
                "EPay",
                "MGate",
                "StripeALL",
                "StripeAlipay",
                "StripeCheckout",
                "StripeCredit",
                "StripeWepay",
                "WechatPayNative",
            ]
        );
    }

    #[test]
    fn provider_form_includes_legacy_field_metadata_and_saved_values() {
        let form = payment_provider_form(
            "EPay",
            Some(&json!({
                "url": "https://pay.example.test",
                "type": "alipay"
            })),
        );
        assert_eq!(form["url"]["label"], "URL");
        assert_eq!(form["url"]["description"], "");
        assert_eq!(form["url"]["type"], "input");
        assert_eq!(form["url"]["value"], "https://pay.example.test");
        assert_eq!(
            form["type"]["description"],
            "支付类型，如: alipay, wxpay, qqpay"
        );
        assert_eq!(form["type"]["value"], "alipay");
        assert!(form["key"]["description"].as_str().unwrap().contains("MD5"));
    }

    #[test]
    fn legacy_md5_providers_are_explicitly_classified() {
        for code in ["BEasyPaymentUSDT", "EPay", "MGate", "WechatPayNative"] {
            assert!(payment_provider_uses_legacy_md5(code));
            assert!(payment_provider_security_warning(code).is_some());
        }
        assert!(!payment_provider_uses_legacy_md5("StripeCheckout"));
        assert!(payment_provider_security_warning("StripeCheckout").is_none());
    }

    #[test]
    fn payment_secrets_are_redacted_without_hiding_public_configuration() {
        let redacted = redact_payment_config(
            "StripeCheckout",
            &json!({
                "currency": "usd",
                "stripe_sk_live": "sk_live_secret",
                "stripe_pk_live": "pk_live_public",
                "stripe_webhook_key": "whsec_secret"
            }),
        );
        assert_eq!(redacted["currency"], "usd");
        assert_eq!(redacted["stripe_pk_live"], "pk_live_public");
        assert_eq!(redacted["stripe_sk_live"], REDACTED_PAYMENT_SECRET);
        assert_eq!(redacted["stripe_webhook_key"], REDACTED_PAYMENT_SECRET);
    }

    #[test]
    fn redaction_fails_closed_for_non_string_secrets_and_unknown_providers() {
        let known = redact_payment_config(
            "StripeCheckout",
            &json!({
                "stripe_sk_live": { "nested": "secret" },
                "stripe_webhook_key": 42,
                "currency": "usd",
                "undeclared_secret": "must not be returned"
            }),
        );
        assert_eq!(known["stripe_sk_live"], REDACTED_PAYMENT_SECRET);
        assert_eq!(known["stripe_webhook_key"], REDACTED_PAYMENT_SECRET);
        assert!(known.get("undeclared_secret").is_none());

        let malformed_public = redact_payment_config(
            "StripeCheckout",
            &json!({ "stripe_pk_live": { "embedded_secret": "must not escape" } }),
        );
        assert_eq!(malformed_public["stripe_pk_live"], REDACTED_PAYMENT_SECRET);
        assert!(!malformed_public.to_string().contains("must not escape"));

        let unknown = redact_payment_config(
            "ExternalLegacyProvider",
            &json!({ "token": "secret", "nested": { "private": true } }),
        );
        assert_eq!(unknown["token"], REDACTED_PAYMENT_SECRET);
        assert_eq!(unknown["nested"], REDACTED_PAYMENT_SECRET);
    }
}
