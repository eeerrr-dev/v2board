#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum ContentVisibility {
    Hidden,
    Visible,
}

impl ContentVisibility {
    pub const fn from_visible(visible: bool) -> Self {
        if visible { Self::Visible } else { Self::Hidden }
    }

    pub const fn is_visible(self) -> bool {
        matches!(self, Self::Visible)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum KnowledgeAccess {
    Full,
    Restricted,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct KnowledgeTemplateValues<'a> {
    pub site_name: &'a str,
    pub subscribe_url: &'a str,
    pub percent_encoded_subscribe_url: &'a str,
    pub safe_base64_subscribe_url: &'a str,
    pub subscribe_token: &'a str,
}

const ACCESS_START: &str = "<!--access start-->";
const ACCESS_END: &str = "<!--access end-->";
const NO_ACCESS_BLOCK: &str = "<div class=\"v2board-no-access\">You must have a valid subscription to view content in this area</div>";

/// Applies access blocks and the established per-request knowledge template.
/// Encoding is supplied as a boundary fact so this pure rule owns placeholder
/// semantics without depending on a URL or Base64 implementation.
pub fn render_knowledge_body(
    source: &str,
    access: KnowledgeAccess,
    values: KnowledgeTemplateValues<'_>,
) -> String {
    let body = match access {
        KnowledgeAccess::Full => source.to_string(),
        KnowledgeAccess::Restricted => mask_access_blocks(source),
    };
    body.replace("{{siteName}}", values.site_name)
        .replace("{{subscribeUrl}}", values.subscribe_url)
        .replace(
            "{{urlEncodeSubscribeUrl}}",
            values.percent_encoded_subscribe_url,
        )
        .replace(
            "{{safeBase64SubscribeUrl}}",
            values.safe_base64_subscribe_url,
        )
        .replace("{{subscribeToken}}", values.subscribe_token)
}

fn mask_access_blocks(source: &str) -> String {
    let mut output = source.to_string();
    while let Some(start) = output.find(ACCESS_START) {
        let Some(relative_end) = output[start..].find(ACCESS_END) else {
            break;
        };
        let end = start + relative_end + ACCESS_END.len();
        output.replace_range(start..end, NO_ACCESS_BLOCK);
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    fn values() -> KnowledgeTemplateValues<'static> {
        KnowledgeTemplateValues {
            site_name: "Board",
            subscribe_url: "https://example.test/sub",
            percent_encoded_subscribe_url: "https%3A%2F%2Fexample.test%2Fsub",
            safe_base64_subscribe_url: "base64",
            subscribe_token: "token",
        }
    }

    #[test]
    fn visibility_is_explicit_business_vocabulary() {
        assert_eq!(
            ContentVisibility::from_visible(false),
            ContentVisibility::Hidden
        );
        assert!(ContentVisibility::from_visible(true).is_visible());
    }

    #[test]
    fn restricted_render_masks_every_complete_access_block() {
        let rendered = render_knowledge_body(
            "a<!--access start-->one<!--access end-->b<!--access start-->two<!--access end-->c",
            KnowledgeAccess::Restricted,
            values(),
        );
        assert_eq!(rendered.matches(NO_ACCESS_BLOCK).count(), 2);
        assert!(!rendered.contains("one"));
        assert!(!rendered.contains("two"));
    }

    #[test]
    fn full_access_preserves_blocks_and_substitutes_every_template_value() {
        let rendered = render_knowledge_body(
            "{{siteName}} {{subscribeUrl}} {{urlEncodeSubscribeUrl}} {{safeBase64SubscribeUrl}} {{subscribeToken}} <!--access start-->secret<!--access end-->",
            KnowledgeAccess::Full,
            values(),
        );
        assert_eq!(
            rendered,
            "Board https://example.test/sub https%3A%2F%2Fexample.test%2Fsub base64 token <!--access start-->secret<!--access end-->"
        );
    }

    #[test]
    fn malformed_unclosed_access_block_is_left_unchanged() {
        let source = "before<!--access start-->still open";
        assert_eq!(
            render_knowledge_body(source, KnowledgeAccess::Restricted, values()),
            source
        );
    }
}
