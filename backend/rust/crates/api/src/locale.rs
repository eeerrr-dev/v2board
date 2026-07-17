//! `Accept-Language` resolution for the modern internal dialect
//! (docs/api-dialect.md §4.3).
//!
//! Modern internal routes read the request locale from `Accept-Language`
//! (standard HTTP list syntax, e.g. `ja-JP,ja;q=0.9,en;q=0.5`) and resolve it
//! against the single enabled-locale anchor in frontend.rs; the default
//! remains `zh-CN`. Problem `detail` localization happens at error
//! construction time with the locale resolved here
//! (`v2board_compat::Code::localized_detail`). The legacy `Content-Language`
//! response-rewrite middleware (localization.rs) is untouched: it keeps
//! serving the frozen §2 external namespaces and never rewrites a
//! problem+json body (docs/api-dialect.md §3.1).

use std::cmp::Ordering;

use axum::http::{HeaderMap, header};

use crate::frontend::enabled_locales;

/// docs/api-dialect.md §4.3: the default locale remains `zh-CN`.
const DEFAULT_LOCALE: &str = "zh-CN";

/// Resolve the request locale from the `Accept-Language` header.
// consumed from W2 (docs/api-dialect.md Appendix A)
#[allow(dead_code)]
pub(crate) fn request_locale(headers: &HeaderMap) -> &'static str {
    resolve_accept_language(
        headers
            .get(header::ACCEPT_LANGUAGE)
            .and_then(|value| value.to_str().ok()),
    )
}

/// Resolve a raw `Accept-Language` value against the enabled-locale anchor.
///
/// Candidates are honored in q-weight order (listing order breaks ties); each
/// candidate tries an exact case-insensitive match first, then a
/// primary-subtag match (`ja` → `ja-JP`, `en-GB` → `en-US`), before the next
/// candidate is considered. Wildcards, `q=0`, and malformed entries are
/// skipped; anything unresolvable falls back to `zh-CN`.
// consumed from W2 (docs/api-dialect.md Appendix A)
#[allow(dead_code)]
pub(crate) fn resolve_accept_language(header: Option<&str>) -> &'static str {
    let Some(header) = header else {
        return DEFAULT_LOCALE;
    };
    parse_candidates(header)
        .into_iter()
        .find_map(match_enabled)
        .unwrap_or(DEFAULT_LOCALE)
}

/// Match one language range against the anchor: exact (case-insensitive)
/// first, then by primary subtag in anchor order.
fn match_enabled(candidate: &str) -> Option<&'static str> {
    let exact = enabled_locales()
        .iter()
        .copied()
        .find(|locale| locale.eq_ignore_ascii_case(candidate));
    exact.or_else(|| {
        let primary = primary_subtag(candidate);
        enabled_locales()
            .iter()
            .copied()
            .find(|locale| primary_subtag(locale).eq_ignore_ascii_case(primary))
    })
}

/// Parse the header into language-range candidates ordered by q-weight
/// (descending; listing order breaks ties). Skips `*`, `q=0`, and malformed
/// entries.
fn parse_candidates(header: &str) -> Vec<&str> {
    let mut weighted: Vec<(&str, f32)> = Vec::new();
    'entries: for entry in header.split(',') {
        let mut parts = entry.split(';');
        let tag = parts.next().unwrap_or("").trim();
        if tag.is_empty()
            || tag == "*"
            || !tag.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
        {
            continue;
        }
        let mut quality = 1.0_f32;
        for parameter in parts {
            if let Some(value) = parameter.trim().strip_prefix("q=") {
                match value.trim().parse::<f32>() {
                    Ok(parsed) if (0.0..=1.0).contains(&parsed) => quality = parsed,
                    _ => continue 'entries,
                }
            }
        }
        if quality <= 0.0 {
            continue;
        }
        weighted.push((tag, quality));
    }
    weighted.sort_by(|left, right| right.1.partial_cmp(&left.1).unwrap_or(Ordering::Equal));
    weighted.into_iter().map(|(tag, _)| tag).collect()
}

fn primary_subtag(tag: &str) -> &str {
    tag.split('-').next().unwrap_or(tag)
}

#[cfg(test)]
mod tests {
    use axum::http::HeaderValue;

    use super::*;

    #[test]
    fn default_locale_is_a_member_of_the_anchor() {
        assert_eq!(DEFAULT_LOCALE, "zh-CN");
        assert!(enabled_locales().contains(&DEFAULT_LOCALE));
    }

    #[test]
    fn missing_or_empty_header_falls_back_to_zh_cn() {
        assert_eq!(resolve_accept_language(None), "zh-CN");
        assert_eq!(resolve_accept_language(Some("")), "zh-CN");
    }

    #[test]
    fn exact_match_is_case_insensitive_and_returns_the_canonical_tag() {
        assert_eq!(resolve_accept_language(Some("en-US")), "en-US");
        assert_eq!(resolve_accept_language(Some("EN-us")), "en-US");
        assert_eq!(resolve_accept_language(Some("zh-tw")), "zh-TW");
    }

    #[test]
    fn q_weights_order_the_candidates() {
        assert_eq!(
            resolve_accept_language(Some("en-US;q=0.4,ja-JP;q=0.9")),
            "ja-JP"
        );
        assert_eq!(
            resolve_accept_language(Some("ja-JP,ja;q=0.9,en;q=0.5")),
            "ja-JP"
        );
    }

    #[test]
    fn listing_order_breaks_q_ties() {
        assert_eq!(resolve_accept_language(Some("ja-JP,ko-KR")), "ja-JP");
        assert_eq!(resolve_accept_language(Some("ko-KR,ja-JP")), "ko-KR");
    }

    #[test]
    fn primary_subtag_falls_back_within_the_anchor() {
        assert_eq!(resolve_accept_language(Some("ja")), "ja-JP");
        assert_eq!(resolve_accept_language(Some("en-GB")), "en-US");
        // First anchor entry with the primary subtag wins: zh → zh-CN.
        assert_eq!(resolve_accept_language(Some("zh")), "zh-CN");
    }

    #[test]
    fn a_higher_preference_fallback_beats_a_lower_exact_match() {
        // The user prefers English (any region) over Chinese; honor the
        // per-candidate fallback before considering the next candidate.
        assert_eq!(resolve_accept_language(Some("en-GB,zh-CN;q=0.9")), "en-US");
    }

    #[test]
    fn wildcard_zero_q_and_malformed_entries_are_skipped() {
        assert_eq!(resolve_accept_language(Some("*")), "zh-CN");
        assert_eq!(resolve_accept_language(Some("en-US;q=0")), "zh-CN");
        assert_eq!(
            resolve_accept_language(Some("en-US;q=0,ja-JP;q=0.5")),
            "ja-JP"
        );
        assert_eq!(
            resolve_accept_language(Some("en-US;q=nope,ja;q=0.5")),
            "ja-JP"
        );
        assert_eq!(resolve_accept_language(Some("en US, {bad}")), "zh-CN");
    }

    #[test]
    fn unknown_locales_fall_back_to_zh_cn() {
        assert_eq!(resolve_accept_language(Some("fr-FR,de;q=0.7")), "zh-CN");
    }

    #[test]
    fn whitespace_around_entries_and_parameters_is_tolerated() {
        assert_eq!(
            resolve_accept_language(Some(" vi-VN ; q=0.8 , ko-KR ; q=0.2 ")),
            "vi-VN"
        );
    }

    #[test]
    fn request_locale_reads_the_accept_language_header() {
        let mut headers = HeaderMap::new();
        assert_eq!(request_locale(&headers), "zh-CN");

        headers.insert(
            header::ACCEPT_LANGUAGE,
            HeaderValue::from_static("ko-KR,en;q=0.5"),
        );
        assert_eq!(request_locale(&headers), "ko-KR");
    }
}
