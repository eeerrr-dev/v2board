//! Code-owned administrative permission registry.

/// Every grantable admin family, in navigation order.
pub const ADMIN_PERMISSION_FAMILIES: [&str; 13] = [
    "config",
    "system",
    "servers",
    "plans",
    "orders",
    "payments",
    "coupons",
    "gift_cards",
    "users",
    "tickets",
    "notices",
    "knowledge",
    "stats",
];

/// Whether `value` is a grantable `{family}:read|write` registry entry.
#[must_use]
pub fn is_registered_permission(value: &str) -> bool {
    value.split_once(':').is_some_and(|(family, access)| {
        ADMIN_PERMISSION_FAMILIES.contains(&family) && matches!(access, "read" | "write")
    })
}

/// How a prefix-relative admin path participates in RBAC.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AdminPathAccess {
    /// The caller's own account family remains reachable so a staff principal
    /// can enroll or rotate MFA even when no business-family grant exists.
    OwnAccount,
    /// A registry family gated by `{family}:read` / `{family}:write`.
    Family(&'static str),
}

/// Maps a prefix-relative admin route to its closed RBAC family.
/// Unknown routes fail closed.
#[must_use]
pub fn admin_path_access(path: &str) -> Option<AdminPathAccess> {
    let first = path.trim_start_matches('/').split('/').next().unwrap_or("");
    let family = match first {
        "account" => return Some(AdminPathAccess::OwnAccount),
        "config" | "email-templates" | "telegram-webhook" | "test-mail" => "config",
        "system" => "system",
        "nodes" | "server-groups" | "server-routes" | "servers" => "servers",
        "plans" => "plans",
        "orders" => "orders",
        "payments" | "payment-providers" | "payment-reconciliations" => "payments",
        "coupons" => "coupons",
        "gift-cards" => "gift_cards",
        "users" => "users",
        "tickets" => "tickets",
        "notices" => "notices",
        "knowledge" | "knowledge-categories" => "knowledge",
        "stats" => "stats",
        _ => return None,
    };
    Some(AdminPathAccess::Family(family))
}

/// Whether a staff grant list allows `path` at the requested access level.
/// Write grants imply read; unknown routes deny access.
#[must_use]
pub fn staff_permissions_allow(permissions: &[String], path: &str, write: bool) -> bool {
    match admin_path_access(path) {
        Some(AdminPathAccess::OwnAccount) => true,
        Some(AdminPathAccess::Family(family)) => {
            let write_grant = format!("{family}:write");
            if write {
                permissions.iter().any(|value| value == &write_grant)
            } else {
                let read_grant = format!("{family}:read");
                permissions
                    .iter()
                    .any(|value| value == &read_grant || value == &write_grant)
            }
        }
        None => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_accepts_only_known_read_and_write_grants() {
        for family in ADMIN_PERMISSION_FAMILIES {
            assert!(is_registered_permission(&format!("{family}:read")));
            assert!(is_registered_permission(&format!("{family}:write")));
            assert!(!is_registered_permission(family));
            assert!(!is_registered_permission(&format!("{family}:admin")));
        }
        assert!(!is_registered_permission("unknown:read"));
        assert!(!is_registered_permission(""));
    }

    #[test]
    fn route_registry_maps_every_admin_family_and_fails_closed() {
        let cases = [
            ("/config", "config"),
            ("/email-templates", "config"),
            ("/telegram-webhook", "config"),
            ("/test-mail", "config"),
            ("/system/audit-logs", "system"),
            ("/nodes/sort", "servers"),
            ("/server-groups/3", "servers"),
            ("/server-routes", "servers"),
            ("/servers/shadowsocks/9/copy", "servers"),
            ("/plans/sort", "plans"),
            ("/orders/T123/mark-paid", "orders"),
            ("/payments/2", "payments"),
            ("/payment-providers/stripe/form", "payments"),
            ("/payment-reconciliations/7/resolve", "payments"),
            ("/coupons/1", "coupons"),
            ("/gift-cards", "gift_cards"),
            ("/users/5/reset-secret", "users"),
            ("/tickets/8/close", "tickets"),
            ("/notices", "notices"),
            ("/knowledge/2", "knowledge"),
            ("/knowledge-categories", "knowledge"),
            ("/stats/summary", "stats"),
        ];
        for (path, family) in cases {
            assert_eq!(
                admin_path_access(path),
                Some(AdminPathAccess::Family(family)),
                "{path} must map to {family}"
            );
        }
        assert_eq!(
            admin_path_access("/account/mfa/totp"),
            Some(AdminPathAccess::OwnAccount)
        );
        assert_eq!(admin_path_access("/no-such-family"), None);
        assert_eq!(admin_path_access("/"), None);
    }

    #[test]
    fn write_grants_imply_read_and_unknown_routes_deny() {
        let grants = vec!["users:write".to_string(), "stats:read".to_string()];
        assert!(staff_permissions_allow(&grants, "/users/5", false));
        assert!(staff_permissions_allow(&grants, "/users/5", true));
        assert!(staff_permissions_allow(&grants, "/stats/summary", false));
        assert!(!staff_permissions_allow(&grants, "/stats/summary", true));
        assert!(!staff_permissions_allow(&grants, "/config", false));
        assert!(!staff_permissions_allow(&grants, "/no-such-family", false));
        assert!(staff_permissions_allow(&[], "/account/mfa", true));
    }
}
