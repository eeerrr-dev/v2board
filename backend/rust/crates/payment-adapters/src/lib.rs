use std::collections::BTreeMap;

mod gateway;
pub mod payment_provider;
pub mod payment_secrets;

pub use gateway::RuntimePaymentGateway;

use serde_json::{Map, Value};
use sha2::{Digest, Sha256};
use uuid::Uuid;
use v2board_application::payment::{
    PaymentSecurity, PaymentSecurityError, ProviderConfig, ProviderForm, ProviderFormField,
};
use v2board_application::reconciliation::ReconciliationIdentityHasher;

#[derive(Clone)]
pub struct EncryptedPaymentSecurity {
    app_key: String,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Sha256ReconciliationIdentityHasher;

impl ReconciliationIdentityHasher for Sha256ReconciliationIdentityHasher {
    fn hash(&self, value: &str) -> [u8; 32] {
        Sha256::digest(value.as_bytes()).into()
    }
}

impl EncryptedPaymentSecurity {
    pub fn new(app_key: String) -> Self {
        Self { app_key }
    }

    fn plaintext(
        &self,
        provider: &str,
        uuid: &str,
        sealed_config: &str,
    ) -> Result<Value, PaymentSecurityError> {
        payment_secrets::decrypt_payment_config(&self.app_key, provider, uuid, sealed_config)
            .map_err(|error| PaymentSecurityError::new("decrypt configuration", error))
    }
}

impl PaymentSecurity for EncryptedPaymentSecurity {
    fn provider_codes(&self) -> Vec<String> {
        payment_provider::payment_provider_codes()
            .into_iter()
            .map(str::to_owned)
            .collect()
    }

    fn new_uuid(&self) -> String {
        Uuid::new_v4().simple().to_string()
    }

    fn seal(
        &self,
        provider: &str,
        uuid: &str,
        config: &ProviderConfig,
    ) -> Result<String, PaymentSecurityError> {
        if payment_provider::payment_provider_uses_legacy_md5(provider) {
            tracing::warn!(
                provider,
                "administrator saved a legacy MD5 payment provider; HTTPS and migration are strongly recommended"
            );
        }
        let config = config
            .iter()
            .map(|(key, value)| (key.clone(), Value::String(value.clone())))
            .collect::<Map<_, _>>();
        let envelope =
            payment_secrets::encrypt_payment_config(&self.app_key, provider, uuid, &config)
                .map_err(|error| PaymentSecurityError::new("encrypt configuration", error))?;
        serde_json::to_string(&envelope)
            .map_err(|error| PaymentSecurityError::new("encode encrypted configuration", error))
    }

    fn redact(
        &self,
        provider: &str,
        uuid: &str,
        sealed_config: &str,
    ) -> Result<ProviderConfig, PaymentSecurityError> {
        let plaintext = self.plaintext(provider, uuid, sealed_config)?;
        let redacted = payment_provider::redact_payment_config(provider, &plaintext);
        redacted
            .as_object()
            .ok_or_else(|| {
                PaymentSecurityError::new("redact configuration", "redactor returned a non-object")
            })?
            .iter()
            .map(|(key, value)| {
                value
                    .as_str()
                    .map(|value| (key.clone(), value.to_owned()))
                    .ok_or_else(|| {
                        PaymentSecurityError::new(
                            "redact configuration",
                            format!("redacted field {key} is not text"),
                        )
                    })
            })
            .collect()
    }

    fn provider_form(&self, provider: &str, config: Option<&ProviderConfig>) -> ProviderForm {
        let Some(manifest) = payment_provider::payment_provider_manifest(provider) else {
            return BTreeMap::new();
        };
        manifest
            .fields
            .iter()
            .map(|field| {
                (
                    field.key.to_string(),
                    ProviderFormField {
                        label: field.label.to_string(),
                        description: field.description.to_string(),
                        kind: "input".to_string(),
                        value: config.and_then(|config| config.get(field.key).cloned()),
                    },
                )
            })
            .collect()
    }

    fn uses_legacy_md5(&self, provider: &str) -> bool {
        payment_provider::payment_provider_uses_legacy_md5(provider)
    }

    fn security_warning(&self, provider: &str) -> Option<String> {
        payment_provider::payment_provider_security_warning(provider).map(str::to_owned)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encrypted_adapter_never_returns_plaintext_secrets() {
        let adapter = EncryptedPaymentSecurity::new("test-app-key".to_string());
        let config = ProviderConfig::from([
            ("currency".to_string(), "usd".to_string()),
            ("stripe_sk_live".to_string(), "sk_live_secret".to_string()),
            ("stripe_pk_live".to_string(), "pk_live_public".to_string()),
            ("stripe_webhook_key".to_string(), "whsec_secret".to_string()),
            ("stripe_custom_field_name".to_string(), String::new()),
        ]);
        let sealed = adapter
            .seal("StripeCheckout", "fixed-uuid", &config)
            .unwrap();
        assert!(!sealed.contains("sk_live_secret"));
        let redacted = adapter
            .redact("StripeCheckout", "fixed-uuid", &sealed)
            .unwrap();
        assert_eq!(redacted["currency"], "usd");
        assert_eq!(redacted["stripe_sk_live"], "********");
        assert_eq!(redacted["stripe_pk_live"], "pk_live_public");
    }

    #[test]
    fn provider_form_is_a_typed_projection_of_the_manifest() {
        let adapter = EncryptedPaymentSecurity::new("unused".to_string());
        let form = adapter.provider_form(
            "EPay",
            Some(&ProviderConfig::from([(
                "url".to_string(),
                "https://pay.example".to_string(),
            )])),
        );
        assert_eq!(form["url"].label, "URL");
        assert_eq!(form["url"].value.as_deref(), Some("https://pay.example"));
        assert!(form["key"].description.contains("MD5"));
    }
}
