use std::{collections::BTreeSet, path::Path, process::Command};

fn workspace_root() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("domain-model lives under <workspace>/crates")
}

fn resolved_dependencies(package: &str, edges: &str, depth: Option<usize>) -> BTreeSet<String> {
    let mut command = Command::new(env!("CARGO"));
    command.current_dir(workspace_root()).args([
        "tree",
        "--locked",
        "--edges",
        edges,
        "--package",
        package,
        "--prefix",
        "none",
        "--format",
        "{p}",
    ]);
    if let Some(depth) = depth {
        command.args(["--depth", &depth.to_string()]);
    }
    let output = command
        .output()
        .unwrap_or_else(|error| panic!("run cargo tree for {package}: {error}"));
    assert!(
        output.status.success(),
        "cargo tree failed for {package}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout)
        .expect("cargo tree output is UTF-8")
        .lines()
        .filter_map(|line| line.split_whitespace().next())
        .filter(|dependency| !dependency.starts_with('['))
        .filter(|dependency| *dependency != package)
        .map(str::to_owned)
        .collect()
}

fn direct_dependencies(package: &str, edges: &str) -> BTreeSet<String> {
    resolved_dependencies(package, edges, Some(1))
}

fn assert_direct_dependencies(package: &str, edges: &str, expected: &[&str]) {
    let actual = direct_dependencies(package, edges);
    let expected = expected
        .iter()
        .map(|dependency| (*dependency).to_owned())
        .collect::<BTreeSet<_>>();
    assert_eq!(
        actual, expected,
        "unexpected direct {edges} dependency set for {package}"
    );
}

fn assert_direct_dependencies_exclude(package: &str, edges: &str, forbidden: &[&str]) {
    let direct = direct_dependencies(package, edges);
    for forbidden in forbidden {
        assert!(
            !direct.contains(*forbidden),
            "direct {edges} dependencies for {package} must not contain {forbidden}: {direct:?}"
        );
    }
}

#[test]
fn pure_model_and_transport_contract_have_exact_direct_dependency_allowlists() {
    assert_direct_dependencies("v2board-domain-model", "normal", &[]);
    assert_direct_dependencies("v2board-domain-model", "build", &[]);
    assert_direct_dependencies("v2board-domain-model", "dev", &[]);

    assert_direct_dependencies("v2board-problem-code", "normal", &[]);
    assert_direct_dependencies("v2board-problem-code", "build", &[]);
    assert_direct_dependencies("v2board-problem-code", "dev", &[]);

    assert_direct_dependencies(
        "v2board-api-contract",
        "normal",
        &[
            "anyhow",
            "chrono",
            "serde",
            "serde_json",
            "utoipa",
            "v2board-problem-code",
        ],
    );
    assert_direct_dependencies("v2board-api-contract", "build", &[]);
    assert_direct_dependencies("v2board-api-contract", "dev", &[]);
}

#[test]
fn application_layer_cannot_reach_transport_through_any_dependency_kind() {
    assert_direct_dependencies_exclude(
        "v2board-domain",
        "normal,build,dev",
        &[
            "axum",
            "axum-core",
            "axum-extra",
            "http",
            "http-body",
            "http-body-util",
            "hyper",
            "tower",
            "tower-http",
            "utoipa",
            "v2board-api-contract",
        ],
    );
}

#[test]
fn http_adapter_directly_depends_on_application_and_transport_contract() {
    let api_direct = direct_dependencies("v2board-api", "normal");
    for required in ["v2board-domain", "v2board-api-contract"] {
        assert!(
            api_direct.contains(required),
            "HTTP adapter must directly depend on {required}: {api_direct:?}"
        );
    }
}
