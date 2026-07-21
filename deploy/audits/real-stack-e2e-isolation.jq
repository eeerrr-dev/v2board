# The real-stack browser journey must remain isolated from mutable runtime data.
def named_sources($service):
  [$service.volumes[]? | select(.type == "volume") | .source];

def network_names($service):
  [$service.networks | keys[]] | sort;

.services as $services
| .networks as $networks
| ($services["frontend-build"].depends_on == null)
  and ($services["real-stack-e2e-build"].depends_on == null)
  and ($services["real-stack-e2e-browser-build"].depends_on == null)
  and ($services["real-stack-e2e-runner"].depends_on == null)
  and ($services["real-stack-e2e-runner"].read_only == true)
  and ($services["real-stack-e2e-runtime-clean"].depends_on == null)
  and ($services["real-stack-e2e-runtime-clean"].network_mode == "none")
  and ($services["real-stack-e2e-runtime-clean"].read_only == true)
  and (network_names($services["real-stack-e2e-runner"]) == ["real-stack-e2e-browser"])
  and (network_names($services["real-stack-e2e-build"]) == ["real-stack-e2e-build"])
  and (network_names($services["real-stack-e2e-browser-build"]) == ["real-stack-e2e-build"])
  and (network_names($services["postgres-real-stack-e2e"]) == ["real-stack-e2e-data"])
  and (network_names($services["redis-real-stack-e2e"]) == ["real-stack-e2e-data"])
  and (network_names($services["real-stack-e2e-bootstrap"]) == ["real-stack-e2e-data"])
  and (network_names($services["rust-real-stack-api"]) == ["real-stack-e2e-browser", "real-stack-e2e-data"])
  and ($networks["real-stack-e2e-browser"].internal == true)
  and ($networks["real-stack-e2e-data"].internal == true)
  and (($services["real-stack-e2e-bootstrap"].depends_on | keys | sort) == ["postgres-real-stack-e2e", "redis-real-stack-e2e"])
  and (($services["rust-real-stack-api"].depends_on | keys | sort) == ["frontend-build", "real-stack-e2e-bootstrap"])
  and ($services["real-stack-e2e-browser-build"].command == ["set -eu\npnpm exec playwright install chromium\n"])
  and ($services["postgres-real-stack-e2e"].tmpfs == ["/var/lib/postgresql"])
  and ($services["redis-real-stack-e2e"].tmpfs == ["/data"])
  and (named_sources($services["postgres-real-stack-e2e"]) == [])
  and (named_sources($services["redis-real-stack-e2e"]) == [])
  and ((named_sources($services["real-stack-e2e-runner"]) | sort) == ["frontend-interaction-artifacts", "frontend-node_modules", "frontend-playwright-cache", "frontend-workspace"])
  and ((named_sources($services["real-stack-e2e-build"]) | sort) == ["rust-cargo-git", "rust-cargo-registry", "rust-target"])
  and ((named_sources($services["real-stack-e2e-browser-build"]) | sort) == ["frontend-node_modules", "frontend-playwright-cache", "frontend-workspace"])
  and ([$services["real-stack-e2e-runner"].volumes[] | select(.target == "/app/frontend" and .read_only == true)] | length == 1)
  and ([$services["real-stack-e2e-runner"].volumes[] | select(.source == "frontend-playwright-cache" and .target == "/app/frontend/.cache/ms-playwright" and .read_only == true)] | length == 1)
  and ([$services["real-stack-e2e-browser-build"].volumes[] | select(.source == "frontend-playwright-cache" and .target == "/app/frontend/.cache/ms-playwright" and .read_only != true)] | length == 1)
  and (named_sources($services["real-stack-e2e-runner"]) | all(. != "frontend-deploy"))
  and (named_sources($services["real-stack-e2e-runtime-clean"]) == ["real-stack-e2e-api-runtime"])
  and ((named_sources($services["real-stack-e2e-bootstrap"]) | sort) == ["real-stack-e2e-api-runtime", "rust-target"])
  and ((named_sources($services["rust-real-stack-api"]) | sort) == ["frontend-deploy", "real-stack-e2e-api-runtime", "rust-target"])
  and ([$services["real-stack-e2e-bootstrap"].volumes[] | select(.source == "rust-target" and .read_only == true)] | length == 1)
  and ([$services["rust-real-stack-api"].volumes[] | select(.source == "rust-target" and .read_only == true)] | length == 1)
  and ([$services["real-stack-e2e-bootstrap"].volumes[] | select(.source == "real-stack-e2e-api-runtime" and .target == "/app/real-stack-e2e-runtime/api" and .read_only != true)] | length == 1)
  and ([$services["rust-real-stack-api"].volumes[] | select(.source == "real-stack-e2e-api-runtime" and .target == "/app/real-stack-e2e-runtime/api" and .read_only == true)] | length == 1)
  and ([
    named_sources($services["frontend-build"])[],
    named_sources($services["real-stack-e2e-build"])[],
    named_sources($services["real-stack-e2e-browser-build"])[],
    named_sources($services["real-stack-e2e-runner"])[],
    named_sources($services["real-stack-e2e-bootstrap"])[],
    named_sources($services["rust-real-stack-api"])[],
    named_sources($services["real-stack-e2e-runtime-clean"])[]
  ] | all(
    . != "postgres-data"
      and . != "redis-data"
      and . != "clickhouse-data"
      and . != "clickhouse-logs"
      and . != "v2board-runtime"
  ))
