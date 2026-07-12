use base64::{Engine as _, engine::general_purpose};
use percent_encoding::{AsciiSet, NON_ALPHANUMERIC, utf8_percent_encode};

const RFC3986_COMPONENT: &AsciiSet = &NON_ALPHANUMERIC
    .remove(b'-')
    .remove(b'_')
    .remove(b'.')
    .remove(b'~');

pub(super) fn prefix_bytes(value: &str, length: usize) -> &[u8] {
    let end = value
        .char_indices()
        .map(|(index, _)| index)
        .chain(std::iter::once(value.len()))
        .nth(length)
        .unwrap_or(value.len());
    &value.as_bytes()[..end]
}

pub(super) fn percent_encode(value: &str) -> String {
    utf8_percent_encode(value, RFC3986_COMPONENT).to_string()
}

pub(super) fn base64_decode_url_safe(value: &str) -> Option<Vec<u8>> {
    general_purpose::URL_SAFE_NO_PAD
        .decode(value)
        .or_else(|_| general_purpose::URL_SAFE.decode(value))
        .or_else(|_| general_purpose::STANDARD_NO_PAD.decode(value))
        .or_else(|_| general_purpose::STANDARD.decode(value))
        .ok()
}

pub(super) fn standard_base64_encode(bytes: &[u8]) -> String {
    general_purpose::STANDARD.encode(bytes)
}

pub(super) fn safe_base64_encode(bytes: &[u8]) -> String {
    general_purpose::URL_SAFE_NO_PAD.encode(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base64_engines_preserve_standard_and_url_safe_contracts() {
        let bytes = [0xfb, 0xff, 0x00];
        assert_eq!(standard_base64_encode(&bytes), "+/8A");
        assert_eq!(safe_base64_encode(&bytes), "-_8A");
        assert_eq!(base64_decode_url_safe("-_8A"), Some(bytes.to_vec()));
        assert_eq!(base64_decode_url_safe("+/8A"), Some(bytes.to_vec()));
        assert_eq!(base64_decode_url_safe("YQ=="), Some(b"a".to_vec()));
        assert_eq!(base64_decode_url_safe("invalid%"), None);
    }

    #[test]
    fn percent_encoding_keeps_only_rfc3986_unreserved_bytes() {
        assert_eq!(percent_encode("a b/中-_.~"), "a%20b%2F%E4%B8%AD-_.~");
    }
}
