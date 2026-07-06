//! Laravel-compatible backend localization.
//!
//! Laravel wraps its user-facing `abort()`/validation strings in `__()`, and pins BOTH
//! the default and fallback locale to `zh-CN` (`config/app.php`). A request that sends no
//! `Content-Language` header therefore still resolves messages to Chinese. We reproduce
//! that by embedding Laravel's own `resources/lang/zh-CN.json` catalog (keyed by the
//! English source string) and defaulting the locale to `zh-CN`.

use std::{collections::HashMap, sync::LazyLock};

/// Laravel's shipped default/fallback locale (`config/app.php` `locale`/`fallback_locale`).
pub(crate) const DEFAULT_LOCALE: &str = "zh-CN";

/// The embedded `resources/lang/zh-CN.json` catalog, English source -> Chinese.
static ZH_CN: LazyLock<HashMap<String, String>> = LazyLock::new(|| {
    serde_json::from_str::<HashMap<String, String>>(include_str!("i18n/zh-CN.json"))
        .unwrap_or_default()
});

/// Translate an English source message into `zh-CN` using the embedded catalog.
/// Returns `None` when the message is not a catalog key (caller keeps the original).
pub(crate) fn translate_zh_cn(message: &str) -> Option<String> {
    ZH_CN.get(message).cloned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalog_loads_and_covers_representative_keys() {
        // A parse failure would silently degrade every localized message to English, so
        // assert the catalog is non-empty and that keys from distinct controllers resolve.
        assert!(
            ZH_CN.len() >= 90,
            "catalog unexpectedly small: {}",
            ZH_CN.len()
        );
        assert_eq!(
            translate_zh_cn("Incorrect email or password").as_deref(),
            Some("邮箱或密码错误")
        );
        assert_eq!(
            translate_zh_cn("Insufficient balance").as_deref(),
            Some("余额不足")
        );
        assert_eq!(translate_zh_cn("not a catalog key"), None);
    }
}
