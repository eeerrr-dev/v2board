use std::{collections::BTreeSet, env, fs, path::Path};

use anyhow::{Context, Result, bail};

pub fn run() -> Result<()> {
    let admin_path = normalized_admin_path();
    let reference_root = env_or(
        "ROUTE_AUDIT_REFERENCE_ROOT",
        "/src/references/wyx2685-v2board",
    );
    let rust_root = env_or("ROUTE_AUDIT_RUST_ROOT", "/src/backend/rust");
    let reference = collect_reference_routes(Path::new(&reference_root), &admin_path)?;
    let rust = collect_rust_routes(Path::new(&rust_root), &admin_path)?;
    let retired = retired_reference_routes(&admin_path);
    let stale_retirements = retired.difference(&reference).cloned().collect::<Vec<_>>();
    if !stale_retirements.is_empty() {
        for route in &stale_retirements {
            println!("STALE RETIREMENT {} {}", route.method, route.path);
        }
        bail!(
            "route audit failed: {} retired routes no longer exist in the reference",
            stale_retirements.len()
        );
    }
    let required = reference
        .difference(&retired)
        .cloned()
        .collect::<BTreeSet<_>>();
    let missing = required.difference(&rust).cloned().collect::<Vec<_>>();

    if !missing.is_empty() {
        println!("Required reference routes missing in Rust:");
        for route in &missing {
            println!("MISSING {} {}", route.method, route.path);
        }
        bail!(
            "route audit failed: {} reference routes are missing",
            missing.len()
        );
    }
    println!(
        "Route audit OK: {} required reference routes are represented in Rust; {} obsolete routes are explicitly retired.",
        required.len(),
        retired.len()
    );
    Ok(())
}

fn retired_reference_routes(admin_path: &str) -> BTreeSet<RouteKey> {
    // The package-theme API was removed with the server-installed frontend theme
    // subsystem. Branding remains native config (color/background/custom HTML).
    // Stripe's public-key endpoint was replaced by server-created PaymentIntents.
    [
        route_key(
            "GET",
            format!("/api/v1/{admin_path}/config/getThemeTemplate"),
        ),
        route_key("GET", format!("/api/v1/{admin_path}/theme/getThemes")),
        route_key("POST", format!("/api/v1/{admin_path}/theme/getThemeConfig")),
        route_key(
            "POST",
            format!("/api/v1/{admin_path}/theme/saveThemeConfig"),
        ),
        route_key("POST", "/api/v1/user/comm/getStripePublicKey"),
    ]
    .into_iter()
    .collect()
}

fn normalized_admin_path() -> String {
    let configured = env_or("ROUTE_AUDIT_ADMIN_PATH", "admin");
    let configured = configured.trim_matches('/');
    if configured.is_empty() {
        "admin".to_string()
    } else {
        configured.to_string()
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd)]
struct RouteKey {
    method: String,
    path: String,
}

fn route_key(method: impl AsRef<str>, path: impl AsRef<str>) -> RouteKey {
    RouteKey {
        method: method.as_ref().to_ascii_uppercase(),
        path: normalize_route_path(path.as_ref()),
    }
}

fn collect_reference_routes(root: &Path, admin_path: &str) -> Result<BTreeSet<RouteKey>> {
    let v1 = root.join("app/Http/Routes/V1");
    let v2 = root.join("app/Http/Routes/V2");
    let mut routes = BTreeSet::new();
    for (file, prefix, root_segment) in [
        ("GuestRoute.php", "/api/v1/guest", "guest"),
        ("ClientRoute.php", "/api/v1/client", "client"),
        ("PassportRoute.php", "/api/v1/passport", "passport"),
        ("UserRoute.php", "/api/v1/user", "user"),
        ("StaffRoute.php", "/api/v1/staff", "staff"),
        ("AdminRoute.php", "", ""),
        ("ServerRoute.php", "/api/v1/server", "server"),
    ] {
        let prefix = if file == "AdminRoute.php" {
            format!("/api/v1/{admin_path}")
        } else {
            prefix.to_string()
        };
        routes.extend(parse_reference_route_file(
            &v1.join(file),
            &prefix,
            root_segment,
        )?);
    }
    routes.extend(parse_reference_route_file(
        &v2.join("ServerRoute.php"),
        "/api/v2/server",
        "server",
    )?);
    Ok(routes)
}

fn parse_reference_route_file(
    path: &Path,
    root_prefix: &str,
    root_segment: &str,
) -> Result<BTreeSet<RouteKey>> {
    let content =
        fs::read_to_string(path).with_context(|| format!("read reference route file {path:?}"))?;
    let mut routes = BTreeSet::new();
    let mut prefixes = vec![normalize_route_path(root_prefix)];
    for (line_index, line) in content.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed.starts_with("});") && prefixes.len() > 1 {
            prefixes.pop();
            continue;
        }
        if let Some(prefix) = extract_reference_group_prefix(trimmed) {
            if prefixes.len() == 1 && prefix.trim_matches('/') == root_segment {
                continue;
            }
            prefixes.push(prefix);
            continue;
        }
        let Some((methods, route_path)) = extract_reference_route(trimmed) else {
            // A line that registers a routing verb but yields no literal path
            // (multi-line, variable/const path, or an unmapped verb) would be
            // silently dropped from the required set, letting a genuinely-missing
            // Rust route pass the audit. Fail loudly instead. `->group(` lines are
            // handled above and never reach here as verbs.
            if let Some(verb) = reference_route_verb(trimmed) {
                bail!(
                    "route audit: unparseable {verb} route in {path:?} line {}: `{trimmed}` \
                     — could not extract a literal path; this route would be silently \
                     dropped from the audit",
                    line_index + 1
                );
            }
            continue;
        };
        let full_path = join_route_parts(
            prefixes
                .iter()
                .map(String::as_str)
                .chain([route_path.as_str()]),
        );
        for method in methods {
            routes.insert(route_key(method, &full_path));
        }
    }
    Ok(routes)
}

fn collect_rust_routes(root: &Path, admin_path: &str) -> Result<BTreeSet<RouteKey>> {
    let api_main = fs::read_to_string(root.join("crates/api/src/main.rs"))?;
    let api_routes = fs::read_to_string(root.join("crates/api/src/routes.rs"))?;
    let admin = fs::read_to_string(root.join("crates/domain/src/admin.rs"))?;
    let mut routes = collect_rust_axum_routes(&format!("{api_main}\n{api_routes}"));
    routes.retain(|route| {
        !route.path.contains("{*admin_path}") && !route.path.contains("{*staff_path}")
    });

    for path in rust_admin_match_paths(&admin, "get") {
        routes.insert(route_key("GET", format!("/api/v1/{admin_path}/{path}")));
    }
    for path in rust_admin_match_paths(&admin, "post") {
        routes.insert(route_key("POST", format!("/api/v1/{admin_path}/{path}")));
    }
    for path in rust_admin_match_paths(&admin, "staff_get") {
        routes.insert(route_key("GET", format!("/api/v1/staff/{path}")));
    }
    for path in rust_admin_match_paths(&admin, "staff_post") {
        routes.insert(route_key("POST", format!("/api/v1/staff/{path}")));
    }
    for kind in [
        "shadowsocks",
        "vmess",
        "trojan",
        "tuic",
        "hysteria",
        "vless",
        "anytls",
        "v2node",
    ] {
        for action in ["save", "drop", "update", "copy"] {
            routes.insert(route_key(
                "POST",
                format!("/api/v1/{admin_path}/server/{kind}/{action}"),
            ));
        }
    }
    Ok(routes)
}

fn collect_rust_axum_routes(content: &str) -> BTreeSet<RouteKey> {
    let lines = content.lines().collect::<Vec<_>>();
    let mut routes = BTreeSet::new();
    let mut index = 0;
    while index < lines.len() {
        if !lines[index].contains(".route(") {
            index += 1;
            continue;
        }
        let (block, next_index) = rust_route_block(&lines, index);
        index = next_index;
        let Some(path) = quoted_strings(&block)
            .into_iter()
            .find(|value| value.starts_with('/'))
        else {
            continue;
        };
        if path.contains("{*") {
            continue;
        }
        if block.contains("get(") {
            routes.insert(route_key("GET", &path));
        }
        if block.contains("post(") || block.contains(".post(") {
            routes.insert(route_key("POST", &path));
        }
    }
    routes
}

fn rust_route_block(lines: &[&str], start: usize) -> (String, usize) {
    let mut block = String::new();
    let mut depth = 0_i32;
    for (index, line) in lines.iter().enumerate().skip(start) {
        let segment = if index == start {
            line.split_once(".route(")
                .map(|(_, right)| format!(".route({right}"))
                .unwrap_or_else(|| (*line).to_string())
        } else {
            (*line).to_string()
        };
        for ch in segment.chars() {
            match ch {
                '(' => depth += 1,
                ')' => depth -= 1,
                _ => {}
            }
        }
        block.push_str(&segment);
        block.push(' ');
        if depth <= 0 {
            return (block, index + 1);
        }
    }
    (block, lines.len())
}

fn rust_admin_match_paths(content: &str, function_name: &str) -> Vec<String> {
    let start_marker = format!("pub async fn {function_name}");
    let mut in_function = false;
    let mut in_match = false;
    let mut paths = Vec::new();
    for line in content.lines() {
        if line.contains(&start_marker) {
            in_function = true;
            continue;
        }
        if !in_function {
            continue;
        }
        if line.contains("match path.as_str()") {
            in_match = true;
            continue;
        }
        if !in_match {
            continue;
        }
        if line.contains("_ =>") {
            break;
        }
        let Some((left, _)) = line.split_once("=>") else {
            continue;
        };
        if left.trim_start().starts_with('_') {
            continue;
        }
        paths.extend(quoted_strings(left));
    }
    paths
}

fn extract_reference_group_prefix(line: &str) -> Option<String> {
    let (left, right) = line.split_once("=>")?;
    if !left.contains("'prefix'") && !left.contains("\"prefix\"") {
        return None;
    }
    let right = right.trim();
    if !(right.starts_with('\'') || right.starts_with('"')) {
        return None;
    }
    quoted_strings(right).into_iter().next()
}

/// Returns the routing verb of a `$router->verb(` registration line, if it is one
/// of the HTTP verbs (not `group`/`resource`/other builder calls). Used to decide
/// whether a line that failed path extraction is a genuinely-dropped route.
fn reference_route_verb(line: &str) -> Option<String> {
    let rest = line.split_once("$router->")?.1;
    let verb = rest.split_once('(')?.0.trim().to_ascii_lowercase();
    matches!(
        verb.as_str(),
        "get" | "post" | "any" | "match" | "put" | "patch" | "delete"
    )
    .then_some(verb)
}

fn extract_reference_route(line: &str) -> Option<(Vec<&'static str>, String)> {
    let router = line.find("$router->")?;
    let rest = &line[router + "$router->".len()..];
    let method = rest.split_once('(')?.0.trim().to_ascii_lowercase();
    let methods = match method.as_str() {
        "get" => vec!["GET"],
        "post" => vec!["POST"],
        "any" => vec!["GET", "POST"],
        "match" => vec!["GET", "POST"],
        _ => return None,
    };
    let route_path = quoted_strings(rest).into_iter().find(|value| {
        !matches!(value.as_str(), "get" | "post" | "put" | "patch" | "delete")
            && !value.contains('\\')
    })?;
    Some((methods, route_path))
}

fn quoted_strings(input: &str) -> Vec<String> {
    let mut output = Vec::new();
    let mut chars = input.char_indices().peekable();
    while let Some((_, ch)) = chars.next() {
        if ch != '\'' && ch != '"' {
            continue;
        }
        let quote = ch;
        let mut value = String::new();
        let mut escaped = false;
        for (_, ch) in chars.by_ref() {
            if escaped {
                value.push(ch);
                escaped = false;
                continue;
            }
            if ch == '\\' {
                escaped = true;
                continue;
            }
            if ch == quote {
                break;
            }
            value.push(ch);
        }
        output.push(value);
    }
    output
}

fn join_route_parts<'a>(parts: impl IntoIterator<Item = &'a str>) -> String {
    let body = parts
        .into_iter()
        .flat_map(|part| part.split('/'))
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("/");
    format!("/{body}")
}

fn normalize_route_path(path: &str) -> String {
    join_route_parts([path])
}

fn env_or(key: &str, default: &str) -> String {
    env::var(key).unwrap_or_else(|_| default.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reference_route_verb_recognizes_http_verbs_not_group() {
        assert_eq!(
            reference_route_verb("$router->post('/a', 'C@m');").as_deref(),
            Some("post")
        );
        assert_eq!(
            reference_route_verb("$router->match(['get','post'], '/a', 'C@m');").as_deref(),
            Some("match")
        );
        // Capitalized verb (a real occurrence in the reference) normalizes.
        assert_eq!(
            reference_route_verb("$router->Post('/a', 'C@m');").as_deref(),
            Some("post")
        );
        // group / non-route builder calls are not routing verbs.
        assert_eq!(
            reference_route_verb("$router->group(['prefix' => 'x'],"),
            None
        );
        assert_eq!(reference_route_verb("something else"), None);
    }

    #[test]
    fn extract_reference_route_handles_match_array_and_backslash_controller() {
        // The `match` form lists methods first; the path is the first quoted
        // string that is neither a verb nor a backslashed controller reference.
        let (methods, path) = extract_reference_route(
            "$router->match(['get', 'post'], '/payment/notify/{method}/{uuid}', 'V1\\Guest\\PaymentController@notify');",
        )
        .expect("match route parses");
        assert_eq!(methods, vec!["GET", "POST"]);
        assert_eq!(path, "/payment/notify/{method}/{uuid}");
    }

    #[test]
    fn parse_reference_route_file_collects_literal_routes() {
        let path =
            std::env::temp_dir().join(format!("v2board_route_audit_ok_{}.php", std::process::id()));
        std::fs::write(
            &path,
            "<?php\n$router->post('/order/save', 'V1\\\\User\\\\OrderController@save');\n\
             $router->match(['get', 'post'], '/payment/notify/{method}/{uuid}', 'V1\\\\Guest\\\\PaymentController@notify');\n",
        )
        .unwrap();

        let routes = parse_reference_route_file(&path, "/api/v1/user", "user").unwrap();
        let _ = std::fs::remove_file(&path);

        assert!(routes.contains(&route_key("POST", "/api/v1/user/order/save")));
        assert!(routes.contains(&route_key(
            "GET",
            "/api/v1/user/payment/notify/{method}/{uuid}"
        )));
        assert!(routes.contains(&route_key(
            "POST",
            "/api/v1/user/payment/notify/{method}/{uuid}"
        )));
    }

    #[test]
    fn parse_reference_route_file_hard_fails_on_unparseable_route() {
        // A verb registration whose path is a PHP variable (not a literal) used to
        // be silently dropped, hiding a genuinely-missing Rust route.
        let path = std::env::temp_dir().join(format!(
            "v2board_route_audit_bad_{}.php",
            std::process::id()
        ));
        std::fs::write(
            &path,
            "<?php\n$router->post($dynamicPath, 'V1\\\\User\\\\OrderController@save');\n",
        )
        .unwrap();

        let result = parse_reference_route_file(&path, "/api/v1/user", "user");
        let _ = std::fs::remove_file(&path);

        let error = result.expect_err("unparseable route must fail the audit");
        assert!(error.to_string().contains("unparseable"));
    }

    #[test]
    fn retired_reference_routes_are_exact_and_admin_path_aware() {
        let retired = retired_reference_routes("private-admin");
        assert_eq!(retired.len(), 5);
        assert!(retired.contains(&route_key("GET", "/api/v1/private-admin/theme/getThemes")));
        assert!(retired.contains(&route_key("POST", "/api/v1/user/comm/getStripePublicKey")));
    }
}
