//! Default-theme mail rendering shared by the request path (verify / notify emails) and the
//! worker (expire / traffic reminders). Every `resources/views/mail/default/*.blade.php`
//! template is the same shell with a different title and body, so a single [`mail_shell`]
//! reproduces the delivered HTML; each renderer supplies the per-template pieces.

pub mod outbox;

pub use v2board_application::worker_mail::ReminderKind;

/// Mirrors Blade's `{{ }}` escaping (htmlspecialchars with ENT_QUOTES).
pub fn html_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#039;")
}

/// Mirrors PHP \"nl2br($value)\" (XHTML): insert \"<br />\" before each line break while keeping
/// the original newline sequence. Used by the \"notify\" template's \"{!! nl2br($content) !!}\".
pub fn nl2br(value: &str) -> String {
    let mut out = String::with_capacity(value.len() + 8);
    let mut chars = value.chars().peekable();
    while let Some(current) = chars.next() {
        match current {
            '\r' => {
                out.push_str("<br />\r");
                if chars.peek() == Some(&'\n') {
                    out.push('\n');
                    chars.next();
                }
            }
            '\n' => {
                out.push_str("<br />\n");
                if chars.peek() == Some(&'\r') {
                    out.push('\r');
                    chars.next();
                }
            }
            other => out.push(other),
        }
    }
    out
}

/// resources/views/mail/default/verify.blade.php: title 邮箱验证码 with the escaped \"{{$code}}\".
pub fn render_verify(name: &str, url: &str, code: &str) -> String {
    let body = format!(
        "您的验证码是：{}，请在 5 分钟内进行验证。如果该验证码不为您本人申请，请无视。",
        html_escape(code)
    );
    mail_shell(name, url, "邮箱验证码", &body)
}

/// resources/views/mail/default/notify.blade.php: title 网站通知 with \"{!! nl2br($content) !!}\"
/// (raw, so operator-authored HTML/newlines pass through exactly as Laravel delivers them).
pub fn render_notify(name: &str, url: &str, content: &str) -> String {
    mail_shell(name, url, "网站通知", &nl2br(content))
}

/// Renders one of the worker's two cron reminders. A typed kind keeps unknown
/// template names from silently falling back to unrelated notification content.
pub fn render_reminder(kind: ReminderKind, name: &str, url: &str) -> String {
    match kind {
        // resources/views/mail/default/remindExpire.blade.php
        ReminderKind::Expire => mail_shell(
            name,
            url,
            "到期通知",
            "你的服务将在24小时内到期。为了不造成使用上的影响请尽快续费。如果你已续费请忽略此邮件。",
        ),
        // resources/views/mail/default/remindTraffic.blade.php
        ReminderKind::Traffic => mail_shell(
            name,
            url,
            "流量通知",
            "你的流量已经使用95%。为了不造成使用上的影响请合理安排流量的使用。",
        ),
    }
}

/// The shared default-theme mail shell used by every \"resources/views/mail/default/*.blade.php\"
/// template: a header band carrying the site name, a title, the greeting plus body, and a footer
/// "return to site" link. \"name\" and \"url\" are HTML-escaped to mirror Blade's \"{{ }}\" output;
/// \"body_html\" is inserted verbatim (callers escape or raw-render per template).
pub fn mail_shell(name: &str, url: &str, title: &str, body_html: &str) -> String {
    let name = html_escape(name);
    let url = html_escape(url);
    format!(
        r##"<div style="background: #eee">
    <table width="600" border="0" align="center" cellpadding="0" cellspacing="0">
        <tbody>
        <tr>
            <td>
                <div style="background:#fff">
                    <table width="100%" border="0" cellspacing="0" cellpadding="0">
                        <thead>
                        <tr>
                            <td valign="middle" style="padding-left:30px;background-color:#415A94;color:#fff;padding:20px 40px;font-size: 21px;">{name}</td>
                        </tr>
                        </thead>
                        <tbody>
                        <tr style="padding:40px 40px 0 40px;display:table-cell">
                            <td style="font-size:24px;line-height:1.5;color:#000;margin-top:40px">{title}</td>
                        </tr>
                        <tr>
                            <td style="font-size:14px;color:#333;padding:24px 40px 0 40px">
                                尊敬的用户您好！
                                <br />
                                <br />
                                {body}
                            </td>
                        </tr>
                        <tr style="padding:40px;display:table-cell">
                        </tr>
                        </tbody>
                    </table>
                </div>
                <div>
                    <table width="100%" border="0" cellspacing="0" cellpadding="0">
                        <tbody>
                        <tr>
                            <td style="padding:20px 40px;font-size:12px;color:#999;line-height:20px;background:#f7f7f7"><a href="{url}" style="font-size:14px;color:#929292">返回{name}</a></td>
                        </tr>
                        </tbody>
                    </table>
                </div></td>
        </tr>
        </tbody>
    </table>
</div>
"##,
        name = name,
        title = title,
        body = body_html,
        url = url,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nl2br_inserts_breaks_and_keeps_newlines() {
        assert_eq!(nl2br("a\nb"), "a<br />\nb");
        assert_eq!(nl2br("a\r\nb"), "a<br />\r\nb");
        assert_eq!(nl2br("plain"), "plain");
    }

    #[test]
    fn render_verify_escapes_code_and_uses_verify_title() {
        let html = render_verify("Site", "https://x/", "12<3>4");
        assert!(html.contains("邮箱验证码"));
        assert!(html.contains("您的验证码是：12&lt;3&gt;4，请在 5 分钟内进行验证。"));
    }

    #[test]
    fn render_notify_raw_renders_content_with_line_breaks() {
        let html = render_notify("Site", "https://x/", "hello\nworld");
        assert!(html.contains("网站通知"));
        assert!(html.contains("hello<br />\nworld"));
    }

    #[test]
    fn reminder_kinds_keep_the_two_legacy_templates_explicit() {
        let expire = render_reminder(ReminderKind::Expire, "Site", "https://x/");
        assert!(expire.contains("到期通知"));
        assert!(expire.contains("你的服务将在24小时内到期。"));
        assert_eq!(ReminderKind::Expire.template_name(), "remindExpire");

        let traffic = render_reminder(ReminderKind::Traffic, "Site", "https://x/");
        assert!(traffic.contains("流量通知"));
        assert!(traffic.contains("你的流量已经使用95%。"));
        assert_eq!(ReminderKind::Traffic.template_name(), "remindTraffic");
    }
}
