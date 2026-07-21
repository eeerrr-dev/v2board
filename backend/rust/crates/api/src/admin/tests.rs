use super::configuration::config_activation_response;
use super::*;
use serde_json::Value;
use v2board_api_contract::admin_platform::AdminConfigPatchRequest;

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
fn every_admin_route_maps_into_the_rbac_registry() {
    // §6.12 coverage guard: a new admin route whose first segment is
    // outside `admin_path_access` would silently fail closed for every
    // staff grant. Use the same registry ids that drive the real Axum
    // router, rather than scraping `.route` literals from source text.
    // Construct both prefix-relative routers so Axum also verifies that no
    // operation id resolves to a conflicting method/path registration.
    let _admin_router = admin_operation_router();
    let _staff_router = staff_operation_router();
    let bound = ADMIN_INTERNAL_OPERATION_IDS
        .iter()
        .copied()
        .collect::<std::collections::BTreeSet<_>>();
    let operations = v2board_api_contract::INTERNAL_OPERATIONS
        .iter()
        .filter(|operation| operation.surface == v2board_api_contract::OperationSurface::Admin)
        .collect::<Vec<_>>();
    let registered = operations
        .iter()
        .map(|operation| operation.id)
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(
        bound, registered,
        "admin Axum bindings drifted from registry"
    );
    assert_eq!(operations.len(), 89, "admin registry coverage drifted");
    for operation in operations {
        assert!(
            v2board_domain_model::admin_path_access(operation.path).is_some(),
            "admin route {} is outside the §6.12 RBAC registry mapping",
            operation.path
        );
    }
}

#[test]
fn admin_guard_authorizes_through_the_rbac_namespace_gate() {
    // Structural pin: the admin guard must pass method + prefix-relative
    // path into `require_admin_namespace` so staff grants see the same
    // path shape as `mfa_exempt_path`.
    let source = include_str!("../admin.rs");
    let admin_guard = source
        .split("async fn admin_guard")
        .nth(1)
        .and_then(|rest| rest.split("async fn require_enrolled_mfa").next())
        .expect("admin_guard precedes require_enrolled_mfa");
    assert!(admin_guard.contains("require_admin_namespace("));
    assert!(admin_guard.contains("request.method()"));
    assert!(admin_guard.contains("request.uri().path()"));
}

#[test]
fn config_patch_body_requires_and_separates_the_client_revision() {
    let body = serde_json::from_value::<AdminConfigPatchRequest>(serde_json::json!({
        "expected_revision": 17,
        "app_name": "CAS Site",
        "force_https": true
    }))
    .expect("valid revisioned config patch");
    assert_eq!(body.expected_revision, 17);
    assert_eq!(body.app_name, Some(Some("CAS Site".to_owned())));
    assert_eq!(body.force_https, Some(Some(true)));

    assert!(
        serde_json::from_value::<AdminConfigPatchRequest>(serde_json::json!({
            "app_name": "missing token"
        }))
        .is_err()
    );
    assert!(
        serde_json::from_value::<AdminConfigPatchRequest>(serde_json::json!({
            "expected_revision": "17",
            "app_name": "wrong token type"
        }))
        .is_err()
    );
}

#[tokio::test]
async fn config_activation_splits_204_full_activation_from_revisioned_202_pending() {
    // §6.1: a committed-and-activated PATCH is an empty 204; a durable
    // write this process could not activate is 202 activation-pending
    // (never an error — retrying the PATCH would 409).
    let activated = config_activation_response(true, 41);
    assert_eq!(activated.status(), StatusCode::NO_CONTENT);
    let activated_body = axum::body::to_bytes(activated.into_body(), 1024)
        .await
        .expect("read activated response body");
    assert!(activated_body.is_empty());

    let pending = config_activation_response(false, 42);
    assert_eq!(pending.status(), StatusCode::ACCEPTED);
    let pending_body = axum::body::to_bytes(pending.into_body(), 1024)
        .await
        .expect("read pending response body");
    assert_eq!(
        serde_json::from_slice::<Value>(&pending_body).expect("decode pending response JSON"),
        serde_json::json!({ "activation": "pending", "revision": 42 })
    );
}
