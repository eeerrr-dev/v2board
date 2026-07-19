use super::configuration::config_activation_response;
use super::*;

#[test]
fn bulk_mail_idempotency_header_is_optional_trimmed_and_bounded() {
    let generated = mail_idempotency_key(&HeaderMap::new()).unwrap();
    assert!(Uuid::parse_str(&generated).is_ok());

    let mut headers = HeaderMap::new();
    headers.insert(
        "idempotency-key",
        HeaderValue::from_static("  admin-mail-7  "),
    );
    assert_eq!(
        mail_idempotency_key(&headers).unwrap(),
        "admin-mail-7".to_string()
    );

    headers.insert(
        "idempotency-key",
        HeaderValue::from_str(&"x".repeat(513)).unwrap(),
    );
    assert!(mail_idempotency_key(&headers).is_err());
}

#[test]
fn malformed_bulk_mail_idempotency_header_is_rejected() {
    let mut headers = HeaderMap::new();
    headers.insert("idempotency-key", HeaderValue::from_bytes(&[0xff]).unwrap());
    assert!(mail_idempotency_key(&headers).is_err());
}

#[test]
fn node_control_plane_credentials_require_recent_password_authentication() {
    // W13: the node-credential read left the legacy dispatch; its
    // step-up gate lives in the modern `nodes_list` handler (same
    // pattern as `reconciliations_list` since W11), so the legacy GET
    // path must no longer carry a sensitive-read allowance at all.
    let source = [
        include_str!("../admin.rs"),
        include_str!("commerce.rs"),
        include_str!("configuration.rs"),
        include_str!("content.rs"),
        include_str!("servers.rs"),
        include_str!("statistics.rs"),
        include_str!("support.rs"),
        include_str!("users.rs"),
    ]
    .concat();
    // Built by concatenation so this test does not match itself.
    let legacy_gate = ["sensitive_admin", "_get"].concat();
    assert!(!source.contains(&legacy_gate));
    let handler = source
        .split("async fn nodes_list")
        .nth(1)
        .and_then(|rest| rest.split("async fn nodes_sort").next())
        .expect("nodes_list handler exists");
    assert!(handler.contains("require_privileged_step_up"));
}

#[test]
fn mandatory_mfa_exempts_exactly_the_account_mfa_family() {
    // §6.10 `admin_mfa_force`: enrollment must stay reachable for an
    // unenrolled session, and nothing else may be.
    assert!(mfa_exempt_path("/account/mfa"));
    assert!(mfa_exempt_path("/account/mfa/totp"));
    assert!(mfa_exempt_path("/account/mfa/totp/confirm"));
    assert!(mfa_exempt_path("/account/mfa/totp/disable"));
    assert!(!mfa_exempt_path("/account/mfariver"));
    assert!(!mfa_exempt_path("/config"));
    assert!(!mfa_exempt_path("/system/audit-logs"));
}

#[test]
fn both_privileged_guards_run_the_mandatory_mfa_gate() {
    // Structural pin mirroring the step-up test style: the admin and
    // staff guards must consult `require_enrolled_mfa` before dispatch.
    let source = include_str!("../admin.rs");
    let admin_guard = source
        .split("async fn admin_guard")
        .nth(1)
        .and_then(|rest| rest.split("async fn require_enrolled_mfa").next())
        .expect("admin_guard precedes require_enrolled_mfa");
    assert!(admin_guard.contains("require_enrolled_mfa(&state, &admin, request.uri().path())"));
    let staff_guard = source
        .split("async fn staff_guard")
        .nth(1)
        .and_then(|rest| rest.split("async fn staff_tickets_list").next())
        .expect("staff_guard handler exists");
    assert!(staff_guard.contains("require_enrolled_mfa(&state, &staff, request.uri().path())"));
}

#[test]
fn config_activation_splits_204_full_activation_from_202_pending() {
    // §6.1: a committed-and-activated PATCH is an empty 204; a durable
    // write this process could not activate is 202 activation-pending
    // (never an error — retrying the PATCH would 409).
    assert_eq!(
        config_activation_response(true).status(),
        StatusCode::NO_CONTENT
    );
    let pending = config_activation_response(false);
    assert_eq!(pending.status(), StatusCode::ACCEPTED);
}
