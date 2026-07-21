use hmac::{Hmac, KeyInit, Mac as _};
use sha2::Sha256;

const TELEGRAM_WEBHOOK_SECRET_DOMAIN: &[u8] = b"v2board/telegram-webhook-secret/v1\0";

/// Derive the stable secret installed through Telegram's `setWebhook` API and
/// checked by the frozen inbound webhook adapter.
pub fn telegram_webhook_secret(app_key: &str, bot_token: &str) -> String {
    let mut mac = Hmac::<Sha256>::new_from_slice(app_key.as_bytes())
        .expect("HMAC accepts application keys of every length");
    mac.update(TELEGRAM_WEBHOOK_SECRET_DOMAIN);
    mac.update(bot_token.as_bytes());
    hex::encode(mac.finalize().into_bytes())
}

#[cfg(test)]
mod tests {
    use super::telegram_webhook_secret;

    #[test]
    fn webhook_secret_is_stable_and_binds_both_inputs() {
        let first = telegram_webhook_secret("app-key", "123:bot-token");
        assert_eq!(first, telegram_webhook_secret("app-key", "123:bot-token"));
        assert_ne!(first, telegram_webhook_secret("other-key", "123:bot-token"));
        assert_ne!(first, telegram_webhook_secret("app-key", "456:bot-token"));
        assert_eq!(first.len(), 64);
        assert!(first.bytes().all(|byte| byte.is_ascii_hexdigit()));
    }
}
