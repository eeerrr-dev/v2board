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
}

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

pub const PAYMENT_PROVIDER_MANIFESTS: &[PaymentProviderManifest] = &[
    PaymentProviderManifest {
        code: "AlipayF2F",
        fields: &[
            input_field("app_id", "支付宝APPID", ""),
            input_field("private_key", "支付宝私钥", ""),
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
            input_field("bepusdt_apitoken", "API Token", "您的 BEPUSDT API Token"),
            input_field("bepusdt_trade_type", "交易类型", "您的 BEPUSDT 交易类型"),
        ],
    },
    PaymentProviderManifest {
        code: "BTCPay",
        fields: &[
            input_field("btcpay_url", "API接口所在网址(包含最后的斜杠)", ""),
            input_field("btcpay_storeId", "storeId", ""),
            input_field(
                "btcpay_api_key",
                "API KEY",
                "个人设置中的API KEY(非商店设置中的)",
            ),
            input_field("btcpay_webhook_key", "WEBHOOK KEY", ""),
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
            input_field(
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
            input_field("coinbase_api_key", "API KEY", ""),
            input_field("coinbase_webhook_key", "WEBHOOK KEY", ""),
        ],
    },
    PaymentProviderManifest {
        code: "EPay",
        fields: &[
            input_field("url", "URL", ""),
            input_field("pid", "PID", ""),
            input_field("key", "KEY", ""),
            input_field("type", "TYPE", "支付类型，如: alipay, wxpay, qqpay"),
        ],
    },
    PaymentProviderManifest {
        code: "MGate",
        fields: &[
            input_field("mgate_url", "API地址", ""),
            input_field("mgate_app_id", "APPID", ""),
            input_field("mgate_app_secret", "AppSecret", ""),
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
            input_field("stripe_sk_live", "SK_LIVE", ""),
            input_field("stripe_webhook_key", "WebHook密钥签名", "whsec_...."),
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
            input_field("stripe_sk_live", "SK_LIVE", ""),
            input_field("stripe_webhook_key", "WebHook密钥签名", ""),
        ],
    },
    PaymentProviderManifest {
        code: "StripeCheckout",
        fields: &[
            input_field("currency", "货币单位", ""),
            input_field("stripe_sk_live", "SK_LIVE", "API 密钥"),
            input_field("stripe_pk_live", "PK_LIVE", "API 公钥"),
            input_field("stripe_webhook_key", "WebHook 密钥签名", ""),
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
            input_field("stripe_sk_live", "SK_LIVE", ""),
            input_field("stripe_pk_live", "PK_LIVE", ""),
            input_field("stripe_webhook_key", "WebHook密钥签名", ""),
        ],
    },
    PaymentProviderManifest {
        code: "StripeWepay",
        fields: &[
            input_field("currency", "货币单位", ""),
            input_field("stripe_sk_live", "SK_LIVE", ""),
            input_field("stripe_webhook_key", "WebHook密钥签名", ""),
        ],
    },
    PaymentProviderManifest {
        code: "WechatPayNative",
        fields: &[
            input_field("app_id", "APPID", "绑定微信支付商户的APPID"),
            input_field("mch_id", "商户号", "微信支付商户号"),
            input_field("api_key", "APIKEY(v1)", ""),
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
        PaymentProviderFieldKind::Input => "input",
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
    }
}
