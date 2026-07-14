.PHONY: up down reset sync logs ps shell doctor \
	rust-check rust-test rust-integration rust-route-audit rust-worker-reconcile rust-target-gate \
	public-bundle-audit runtime-isolation-audit native-database-audit cloudflared-config-audit native-release-audit frontend-source-audit parity-config-audit ui-sync-audit \
	deploy-smoke visual-smoke interaction-parity accessibility-smoke behavior-parity \
	reference-oracle-check reference-oracle-up reference-oracle-down \
	clean-frontend-runs clean-host clean-host-apply mailpit-ui admin-url

DC := $(shell \
	if docker compose version >/dev/null 2>&1; then echo "docker compose"; else echo ""; fi)

ifeq ($(DC),)
$(error Docker Compose v2 is required)
endif

COMPOSE_FILE ?= docker-compose.local.yml
COMPOSE_PROJECT ?= v2board
DCF := $(DC) -p $(COMPOSE_PROJECT) -f $(COMPOSE_FILE)
REFERENCE_DCF := $(DCF) --profile reference

V2BOARD_FRONTEND_ADMIN_PATH ?= admin
ADMIN_PATH ?= $(V2BOARD_FRONTEND_ADMIN_PATH)
export V2BOARD_FRONTEND_ADMIN_PATH
SOURCE_BASE_URL ?= http://rust-api:8080
RUST_WORKER_RECONCILE_WAIT_SECONDS ?= 75
RUST_WORKER_RECONCILE_STRICT ?= 1
VISUAL_PARITY_VIEWPORTS ?= desktop mobile
PARITY_WORKERS ?=
INTERACTION_PARITY_ARTIFACT_DIR ?= /app/frontend/.cache/interaction-parity
A11Y_SMOKE_SCENARIOS ?= a11y-user-login a11y-admin-login a11y-user-dashboard a11y-admin-users
INTERACTION_PARITY_SCENARIOS ?= user-login-form-language user-login-language-persistence user-home-root-page-state user-register-form-state user-forget-form-state admin-root-page-state admin-login-form-state \
	admin-system-queue-state user-dashboard-header-language-dropdown user-session-expired-redirect user-auth-401-no-redirect user-dashboard-dark-mode-persistence user-dashboard-subscribe-drawer user-dashboard-subscribe-import-links \
	user-dashboard-subscribe-import-ios-ua user-dashboard-subscribe-import-android-ua user-dashboard-subscribe-import-macos-ua user-dashboard-subscribe-import-windows-ua user-dashboard-notice-carousel user-dashboard-reset-package-confirm user-dashboard-new-period-confirm user-dashboard-alert-links \
	user-profile-deposit-modal user-profile-reset-subscribe-confirm user-profile-telegram-bind-modal user-profile-telegram-unbind-confirm user-profile-preference-switches user-profile-redeem-giftcard user-profile-redeem-giftcard-api-500 user-profile-redeem-giftcard-timeout \
	user-profile-change-password-success user-plans-filter-tabs user-plans-fetch-timeout user-plan-checkout-coupon user-plan-checkout-coupon-error user-order-payment-method user-order-qr-checkout user-order-qr-checkout-failure \
	user-order-checkout-network-failure user-orders-fetch-api-500 user-orders-fetch-timeout user-order-stripe-disabled-checkout user-order-stripe-payment-intent-checkout user-order-stripe-confirmation-failure user-order-redirect-checkout user-node-table-scroll \
	user-node-fetch-api-500 user-node-fetch-timeout user-node-tooltips user-traffic-table-scroll user-traffic-fetch-timeout user-traffic-total-tooltip user-knowledge-drawer user-knowledge-extreme-content-matrix \
	user-knowledge-fetch-timeout user-invite-generate user-invite-transfer-modal user-invite-transfer-insufficient-balance user-invite-withdraw-modal user-invite-finance-submit-matrix user-invite-tooltips user-ticket-reply-send \
	user-ticket-error-matrix user-tickets-fetch-timeout user-ticket-create-submit user-ticket-create-validation-failure admin-ticket-reply-send admin-tickets-reply-filter admin-tickets-fetch-timeout admin-dashboard-dark-mode-persistence \
	admin-dashboard-avatar-dropdown admin-session-expired-redirect admin-auth-401-no-redirect admin-dashboard-commission-shortcut user-order-cancel-confirm admin-plan-create-drawer admin-plan-save-failure admin-plan-create-group-select-dropdown \
	admin-plans-fetch-timeout admin-plan-reset-method-matrix admin-plan-drawer-keyboard-close admin-plan-edit-drawer admin-plan-renew-tooltip admin-mutation-failure-matrix admin-config-tabs admin-config-unchanged-blur admin-config-save-failure-matrix \
	admin-server-create-node-drawer admin-server-vless-reality-matrix admin-server-node-save-failure admin-server-protocol-field-matrix admin-server-v2node-protocol-matrix admin-server-v2node-security-transport-matrix admin-server-manage-fetch-timeout \
	admin-server-edit-node-drawer admin-server-route-create-modal admin-server-route-edit-modal admin-server-group-create-modal admin-server-group-save-failure admin-server-group-edit-modal admin-payment-create-modal admin-payment-save-failure \
	admin-payment-edit-modal admin-payment-plugin-field-matrix admin-payment-modal-keyboard-close admin-payments-fetch-timeout admin-payment-notify-tooltip admin-order-detail-modal admin-order-status-tooltips admin-order-assign-modal \
	admin-order-status-dropdown admin-order-commission-dropdown admin-orders-filter-pagination-matrix admin-orders-fetch-api-500 admin-orders-fetch-timeout admin-coupon-create-modal admin-coupon-generate-failure admin-coupon-range-picker \
	admin-coupon-type-matrix admin-coupons-fetch-timeout admin-coupon-edit-modal admin-giftcard-create-modal admin-giftcard-generate-failure admin-giftcard-edit-modal admin-giftcards-fetch-timeout admin-notice-create-modal \
	admin-notice-save-failure admin-notice-edit-modal admin-notices-fetch-timeout admin-knowledge-create-drawer admin-knowledge-save-failure admin-knowledge-edit-drawer admin-knowledge-fetch-timeout admin-users-filter-input \
	admin-users-filter-field-select-dropdown admin-users-filter-expiry-picker admin-users-pagination-matrix admin-users-sort-matrix admin-users-fetch-api-500 admin-users-fetch-timeout admin-user-bulk-ban-confirm admin-user-bulk-delete-confirm \
	admin-user-destructive-failure-matrix admin-user-export-download-matrix admin-user-create-modal admin-user-create-plan-select-dropdown admin-user-create-expiry-picker admin-user-send-mail-modal admin-user-send-mail-submit-matrix admin-user-reset-secret-confirm \
	admin-user-delete-confirm admin-user-copy-action admin-user-edit-action admin-user-update-validation-failure admin-user-assign-action admin-user-orders-action admin-user-invite-action admin-user-traffic-action \
	admin-users-extreme-viewport-matrix a11y-user-login a11y-admin-login a11y-user-dashboard a11y-admin-users

FRONTEND_RUN := $(DCF) run --rm -T --no-deps --entrypoint sh
FRONTEND_WORKSPACE_BOOTSTRAP := if [ ! -f /app/frontend/package.json ]; then mkdir -p /app/frontend && tar --exclude=node_modules --exclude=.pnpm-store --exclude=dist --exclude=dist-deploy -C /src/frontend -cf - . | tar -C /app/frontend -xf -; fi
FRONTEND_SETUP := $(FRONTEND_WORKSPACE_BOOTSTRAP) && pnpm config set store-dir /app/frontend/.pnpm-store >/dev/null && pnpm install --frozen-lockfile
PLAYWRIGHT_SETUP := if ! find /app/frontend/.cache/ms-playwright -path '*/chrome-linux/chrome' -type f 2>/dev/null | grep -q .; then pnpm exec playwright install chromium; fi

up:
	$(DCF) up -d --build
	@echo ""
	@echo "  app       http://localhost:8000"
	@echo "  user dev  http://localhost:5173"
	@echo "  admin dev http://localhost:5174/$(ADMIN_PATH)"
	@echo "  mail      http://localhost:8025"
	@echo ""
	@echo "Native-runtime-shaped local pages and APIs are served by Rust on :8000."

down:
	$(DCF) down --remove-orphans

reset:
	$(DCF) down -v --remove-orphans
	$(MAKE) --no-print-directory up

sync:
	$(DCF) down --remove-orphans
	@for volume in \
		$(COMPOSE_PROJECT)_frontend-workspace \
		$(COMPOSE_PROJECT)_frontend-deploy; do \
		docker volume rm "$$volume" >/dev/null 2>&1 || true; \
		if docker volume inspect "$$volume" >/dev/null 2>&1; then \
			echo "Could not refresh $$volume; remove containers still using it."; exit 1; \
		fi; \
	done
	$(MAKE) --no-print-directory up

logs:
	$(DCF) logs -f --tail=100

ps:
	$(DCF) ps

shell:
	$(DCF) exec rust-api bash

doctor:
	@$(DCF) config --quiet
	$(MAKE) --no-print-directory public-bundle-audit
	$(MAKE) --no-print-directory runtime-isolation-audit
	$(MAKE) --no-print-directory native-database-audit
	$(MAKE) --no-print-directory native-release-audit
	$(MAKE) --no-print-directory frontend-source-audit
	$(MAKE) --no-print-directory parity-config-audit
	$(MAKE) --no-print-directory ui-sync-audit
	@echo "Docker configuration, host cleanliness, runtime isolation, parity configuration, and shared UI synchronization are valid."

rust-check:
	$(DCF) build rust-api
	$(DCF) run --rm -T --no-deps --entrypoint bash rust-api -lc \
		'. /usr/local/cargo/env; cargo fmt --all --check && cargo clippy --workspace --all-targets --locked -- -D warnings'

rust-test:
	$(DCF) build rust-api
	$(DCF) run --rm -T --no-deps --entrypoint bash rust-api -lc \
		'. /usr/local/cargo/env; cargo test --workspace --locked'

rust-integration:
	$(DCF) build rust-api
	$(DCF) up -d --wait postgres clickhouse redis
	$(DCF) --profile migration-test up -d --wait --force-recreate legacy-mysql-source postgres-import-target redis-import-target
	$(DCF) exec -T postgres dropdb --force --if-exists -U v2board v2board_analytics_test
	$(DCF) exec -T postgres createdb -U v2board v2board_analytics_test
	$(DCF) exec -T postgres dropdb --force --if-exists -U v2board v2board_import_target_test
	$(DCF) exec -T postgres createdb -U v2board v2board_import_target_test
	$(DCF) exec -T postgres dropdb --force --if-exists -U v2board v2board_schema_test
	$(DCF) exec -T postgres createdb -U v2board v2board_schema_test
	$(DCF) exec -T clickhouse clickhouse-client --user v2board_analytics --password v2board \
		--query 'DROP DATABASE IF EXISTS v2board_analytics_test SYNC'
	$(DCF) exec -T clickhouse clickhouse-client --user v2board_analytics --password v2board \
		--query 'CREATE DATABASE v2board_analytics_test'
	$(DCF) run --rm -T --no-deps --entrypoint bash \
		-e RUST_INTEGRATION_DATABASE_ROOT_URL=postgresql://v2board:v2board@postgres:5432/postgres \
		-e RUST_INTEGRATION_DATABASE_URL=postgresql://v2board:v2board@postgres:5432/v2board_analytics_test \
		-e RUST_INTEGRATION_CLICKHOUSE_URL=http://clickhouse:8123 \
		-e RUST_INTEGRATION_CLICKHOUSE_DATABASE=v2board_analytics_test \
		-e RUST_INTEGRATION_CLICKHOUSE_USERNAME=v2board_analytics \
		-e RUST_INTEGRATION_CLICKHOUSE_PASSWORD=v2board \
		-e RUST_INTEGRATION_REDIS_URL=redis://redis:6379/15 \
		-e RUST_INTEGRATION_LEGACY_MYSQL_URL=mysql://legacy_reader:LegacySourceReadOnlyTestSecret-32-bytes@legacy-mysql-source:3306/v2board_legacy \
		-e RUST_INTEGRATION_LEGACY_MYSQL_FIXTURE_ADMIN_URL=mysql://root:root-import-test-secret@legacy-mysql-source:3306/v2board_legacy \
		-e RUST_INTEGRATION_IMPORT_POSTGRES_URL=postgresql://v2board:v2board@postgres:5432/v2board_import_target_test \
		-e RUST_INTEGRATION_SCHEMA_DATABASE_URL=postgresql://v2board:v2board@postgres:5432/v2board_schema_test \
		-e RUST_INTEGRATION_EXECUTE_DATABASE_ROOT_URL=postgresql://import_bootstrap:ImportBootstrapTestSecret-32-bytes@postgres-import-target:5432/postgres \
		-e RUST_INTEGRATION_EXECUTE_REDIS_URL=redis://import_bootstrap:RedisImportBootstrapTestSecret-32-bytes@redis-import-target:6379/0 \
		rust-api -lc \
		'set -euo pipefail; . /usr/local/cargo/env; \
		 cargo test --locked -p v2board-lifecycle imported_source_schema_matches_oracle_mysql_8_legacy_fixture; \
		 cargo test --locked -p v2board-lifecycle representative_mysql_rows_copy_into_fresh_postgres; \
		 RUST_INTEGRATION_DATABASE_URL="$$RUST_INTEGRATION_SCHEMA_DATABASE_URL" cargo test --locked -p v2board-provision --test postgres_target_schema; \
		 cargo test --locked -p v2board-lifecycle -- --list | grep -Fx "mysql_import::tests::full_execute_bootstraps_and_retires_every_principal: test"; \
		 cargo test --locked -p v2board-lifecycle mysql_import::tests::full_execute_bootstraps_and_retires_every_principal -- --exact; \
		 cargo test --locked -p v2board-analytics --test clickhouse_roundtrip; \
		 cargo test --locked -p v2board-analytics --test outbox_roundtrip; \
		 cargo build --locked -p v2board-workers; \
		 cargo run --locked -p v2board-contract -- production-invariants'
	$(DCF) --profile migration-test stop legacy-mysql-source postgres-import-target redis-import-target

rust-route-audit:
	$(DCF) build rust-api
	$(DCF) run --rm -T --no-deps --entrypoint bash \
		-v "$(CURDIR)/references:/src/references:ro" \
		-e ROUTE_AUDIT_ADMIN_PATH=$(ADMIN_PATH) \
		rust-api -lc '. /usr/local/cargo/env; cargo run --locked -p v2board-contract -- route-audit'

rust-worker-reconcile:
	$(DCF) up -d --build postgres clickhouse redis clickhouse-migrate rust-api rust-worker
	@sleep $(RUST_WORKER_RECONCILE_WAIT_SECONDS)
	$(DCF) exec -T rust-api cargo run --locked -p v2board-workers -- run-once statistics
	$(DCF) exec -T \
		-e WORKER_RECONCILE_STRICT=$(RUST_WORKER_RECONCILE_STRICT) \
		rust-api cargo run --locked -p v2board-contract -- worker-reconcile

rust-target-gate: rust-check rust-test rust-integration rust-route-audit rust-worker-reconcile
	@echo "Rust API, worker, route, and reconciliation gates passed."

public-bundle-audit:
	@paths='backend/rust/target frontend/.pnpm-store frontend/dist frontend/dist-deploy frontend/.cache frontend/coverage frontend/apps/user/dist frontend/apps/admin/dist public/theme/default public/assets/admin native-release lifecycle-tool lifecycle-package v2board-native-debian-13-amd64.tar.gz v2board-native-debian-13-amd64.tar.gz.sha256 v2board-lifecycle-debian-13-amd64.tar.gz v2board-lifecycle-debian-13-amd64.tar.gz.sha256'; \
	found=0; \
	for path in $$paths; do \
		if [ -e "$$path" ]; then echo "Host-generated deploy/build output found: $$path"; found=1; fi; \
	done; \
	[ "$$found" -eq 0 ] || { echo "Use Docker volumes; preview cleanup with make clean-host."; exit 1; }
	@echo "Host deploy/build targets are empty."

runtime-isolation-audit:
	@echo "Auditing production paths for retired PHP and packaged frontend runtime dependencies..."
	@matches="$$(rg -n \
		'backend/laravel|php artisan|composer(\.json| install)|laravel:|/theme/default/assets|umi\.(js|css)|components\.chunk\.css|vendors\.async\.js|components\.async\.js' \
		Dockerfile.rust Dockerfile.frontend docker-compose.local.yml .github .devcontainer \
		--glob '!**/references/**' || true)"; \
	if [ -n "$$matches" ]; then echo "$$matches"; exit 1; fi
	@$(DCF) config --format json | jq -e \
		'.services | all(.[]; .logging.driver == "local" and .logging.options["max-size"] == "10m" and .logging.options["max-file"] == "3")' \
		>/dev/null || { echo "Every local Compose service must use bounded local logging (10m x 3)."; exit 1; }
	@echo "Production workflow has no retired runtime dependency; local service logs are bounded."

native-database-audit:
	@matches="$$(rg -n \
		'\b(MySql|MySqlPool|MySqlConnection)\b|connect_mysql|migrate_mysql|ON DUPLICATE KEY|INSERT IGNORE|GET_LOCK\(|RELEASE_LOCK\(' \
		backend/rust/crates/api backend/rust/crates/analytics backend/rust/crates/compat backend/rust/crates/config \
		backend/rust/crates/contract backend/rust/crates/db backend/rust/crates/domain \
		backend/rust/crates/workers \
		--glob '*.rs' || true)"; \
	if [ -n "$$matches" ]; then echo "$$matches"; exit 1; fi
	@matches="$$(rg -n \
		'features[[:space:]]*=[[:space:]]*\[[^]]*"mysql"|v2board-provision|v2board-lifecycle' \
		backend/rust/crates/api/Cargo.toml backend/rust/crates/analytics/Cargo.toml backend/rust/crates/db/Cargo.toml \
		backend/rust/crates/domain/Cargo.toml backend/rust/crates/workers/Cargo.toml \
		backend/rust/crates/contract/Cargo.toml || true)"; \
	if [ -n "$$matches" ]; then echo "$$matches"; exit 1; fi
	@$(DCF) build rust-api >/dev/null
	@graph="$$( $(DCF) run --rm -T --no-deps --entrypoint bash rust-api -lc \
		'. /usr/local/cargo/env; cargo tree --locked -e normal -p v2board-api -p v2board-workers -p v2board-analytics' \
		)" || exit $$?; \
	matches="$$(printf '%s\n' "$$graph" | rg 'sqlx-mysql|v2board-provision|v2board-lifecycle' || true)"; \
	if [ -n "$$matches" ]; then echo "$$matches"; exit 1; fi
	@test ! -d backend/rust/migrations || \
		test -z "$$(find backend/rust/migrations -type f -print -quit)"
	@matches="$$(rg -n '\bv2_[a-z0-9_]+' \
		backend/rust/migrations-postgres backend/rust/clickhouse-migrations || true)"; \
	if [ -n "$$matches" ]; then echo "Native database objects must not use the legacy v2_ source prefix:"; echo "$$matches"; exit 1; fi
	@rg -q 'migrations-postgres' backend/rust/crates/db/src/pool.rs
	@echo "API/worker/analytics graphs exclude lifecycle, provision, and MySQL; native schema names are unprefixed; only lifecycle directly reads the stopped legacy MySQL source and bootstraps the new targets."

cloudflared-config-audit:
	@test -f deploy/systemd/v2board-cloudflared.service
	@rg -Fqx 'Wants=network-online.target v2board-api.service v2board-worker.service' deploy/systemd/v2board-cloudflared.service
	@rg -Fqx 'After=network-online.target v2board-api.service v2board-worker.service' deploy/systemd/v2board-cloudflared.service
	@rg -Fqx 'Type=notify' deploy/systemd/v2board-cloudflared.service
	@rg -Fqx 'NotifyAccess=main' deploy/systemd/v2board-cloudflared.service
	@rg -Fqx 'User=cloudflared' deploy/systemd/v2board-cloudflared.service
	@rg -Fqx 'Group=cloudflared' deploy/systemd/v2board-cloudflared.service
	@rg -Fqx 'WorkingDirectory=/var/lib/cloudflared' deploy/systemd/v2board-cloudflared.service
	@rg -Fqx 'SetCredential=cloudflared-remote-only-config:{}' deploy/systemd/v2board-cloudflared.service
	@rg -Fqx 'LoadCredential=cloudflared-tunnel-token:/etc/v2board/cloudflared/tunnel-token' deploy/systemd/v2board-cloudflared.service
	@rg -Fqx 'ExecStartPre=/usr/bin/test -s %d/cloudflared-remote-only-config' deploy/systemd/v2board-cloudflared.service
	@rg -Fqx 'ExecStartPre=/usr/bin/test -s %d/cloudflared-tunnel-token' deploy/systemd/v2board-cloudflared.service
	@rg -Fqx 'ExecStart=/usr/bin/cloudflared tunnel --config %d/cloudflared-remote-only-config --no-autoupdate --loglevel info run --token-file %d/cloudflared-tunnel-token' deploy/systemd/v2board-cloudflared.service
	@test "$$(rg -c '^SetCredential=' deploy/systemd/v2board-cloudflared.service)" -eq 1
	@test "$$(rg -c '^LoadCredential=' deploy/systemd/v2board-cloudflared.service)" -eq 1
	@test "$$(rg -c '^ExecStart=' deploy/systemd/v2board-cloudflared.service)" -eq 1
	@test "$$(rg -o -- '--config' deploy/systemd/v2board-cloudflared.service | wc -l | tr -d ' ')" -eq 1
	@if rg -n '^Environment=.*(TOKEN|TUNNEL|CREDENTIAL)|--token[[:space:]=]|--url([[:space:]=]|$$)' deploy/systemd/v2board-cloudflared.service; then \
		echo "Tunnel credentials and remotely-managed ingress must not be replaced by environment or local configuration."; exit 1; \
	fi
	@for setting in \
		'NoNewPrivileges=true' \
		'CapabilityBoundingSet=' \
		'AmbientCapabilities=' \
		'PrivateDevices=true' \
		'PrivateMounts=true' \
		'PrivateTmp=true' \
		'ProtectHome=true' \
		'ProtectProc=invisible' \
		'ProtectSystem=strict' \
		'RestrictAddressFamilies=AF_UNIX AF_INET AF_INET6' \
		'RestrictNamespaces=true' \
		'SystemCallArchitectures=native' \
		'SystemCallFilter=@system-service' \
		'DevicePolicy=closed'; do \
		rg -Fqx "$$setting" deploy/systemd/v2board-cloudflared.service || exit 1; \
	done
	@echo "Remotely-managed Cloudflare Tunnel credential and hardened systemd contract are present."

native-release-audit: cloudflared-config-audit
	docker build --platform linux/amd64 --target native-release-runtime-audit \
		--build-arg V2BOARD_SOURCE_REVISION=0000000000000000000000000000000000000000 \
		--file Dockerfile.rust .
	@test -f deploy/systemd/v2board-api.service
	@test -f deploy/systemd/v2board-worker.service
	@test -f deploy/systemd/v2board-cloudflared.service
	@rg -q '^FROM scratch AS lifecycle-tool$$' Dockerfile.rust
	@rg -q '^COPY --from=lifecycle-runtime-audit /out/v2board-lifecycle /v2board-lifecycle$$' Dockerfile.rust
	@rg -q '^FROM scratch AS native-release$$' Dockerfile.rust
	@rg -q '^COPY --from=native-release-runtime-audit /release/ /$$' Dockerfile.rust
	@rg -Fq 'FROM --platform=$$BUILDPLATFORM rust:$${RUST_VERSION}-trixie@sha256:8e117cab45b7e67c5d146456107cd0f8bf87df3a668e5cf142be6984cf900ca0 AS development' Dockerfile.rust
	@rg -Fq 'FROM --platform=$$BUILDPLATFORM node:$${NODE_VERSION}-trixie-slim@sha256:ae91dcc111a68c9d2d81ff2a17bda61be126426176fde6fe7d08ab13b7f50573 AS frontend-builder' Dockerfile.rust
	@test "$$(rg -Fxc 'FROM debian:trixie-slim@sha256:020c0d20b9880058cbe785a9db107156c3c75c2ac944a6aa7ab59f2add76a7bd AS native-runtime-audit-base' Dockerfile.rust)" -eq 1
	@test "$$(rg -Fxc 'FROM debian:trixie-slim@sha256:020c0d20b9880058cbe785a9db107156c3c75c2ac944a6aa7ab59f2add76a7bd AS native-release-assembler' Dockerfile.rust)" -eq 1
	@if rg -n 'bookworm' Dockerfile.rust; then echo "Native build stages must stay on Debian 13."; exit 1; fi
	@rg -Fq 'test "$${TARGETARCH}" = amd64' Dockerfile.rust
	@rg -Fqx 'ARG V2BOARD_SOURCE_REVISION' Dockerfile.rust
	@rg -Fq "'target_distribution=debian'" Dockerfile.rust
	@rg -Fq "'target_distribution_version=13'" Dockerfile.rust
	@rg -Fq 'find bin frontend systemd RELEASE -type f -print0' Dockerfile.rust
	@rg -Fq 'test -L frontend/previous' Dockerfile.rust
	@retired_ingress="$$(printf '%s%s' ngi nx)"; \
		matches="$$(rg -ni "$$retired_ingress" Dockerfile.rust Makefile docker-compose.local.yml .github/workflows/native-ci.yml || true)"; \
		if [ -n "$$matches" ]; then echo "Retired reverse-proxy release residue found:"; echo "$$matches"; exit 1; fi
	@if rg -n '^FROM .* AS production(-api|-worker|-base|-lifecycle)?$$|^(HEALTHCHECK|ENTRYPOINT|CMD|VOLUME) ' Dockerfile.rust; then \
		echo "Dockerfile.rust must export a filesystem payload, not a production runtime image."; exit 1; \
	fi
	@for unit in deploy/systemd/v2board-api.service deploy/systemd/v2board-worker.service deploy/systemd/v2board-cloudflared.service; do \
		rg -Fqx 'NoNewPrivileges=true' "$$unit" || exit 1; \
		rg -Fqx 'ProtectSystem=strict' "$$unit" || exit 1; \
		rg -Fqx 'ProtectHome=true' "$$unit" || exit 1; \
		rg -Fqx 'PrivateTmp=true' "$$unit" || exit 1; \
		rg -Fqx 'CapabilityBoundingSet=' "$$unit" || exit 1; \
	done
	@rg -Fqx 'User=v2board-api' deploy/systemd/v2board-api.service
	@rg -Fqx 'ReadWritePaths=/var/lib/v2board/api' deploy/systemd/v2board-api.service
	@rg -Fqx 'User=v2board-worker' deploy/systemd/v2board-worker.service
	@rg -Fqx 'Type=notify' deploy/systemd/v2board-worker.service
	@rg -Fqx 'WatchdogSec=30s' deploy/systemd/v2board-worker.service
	@rg -Fqx 'ReadWritePaths=/var/lib/v2board/worker' deploy/systemd/v2board-worker.service
	@rg -q '^  native-release:$$' .github/workflows/native-ci.yml
	@rg -q '^  native-attest:$$' .github/workflows/native-ci.yml
	@rg -q -- '--target native-release' .github/workflows/native-ci.yml
	@test "$$(rg -c -- '--platform linux/amd64' .github/workflows/native-ci.yml)" -eq 4
	@rg -q -- '--output type=local,dest=native-release' .github/workflows/native-ci.yml
	@rg -Fq 'inspect-release-archive' .github/workflows/native-ci.yml
	@test "$$(rg -Fxc "      - 'deploy/**'" .github/workflows/native-ci.yml)" -eq 2
	@test "$$(rg -Fxc "      - 'docs/**'" .github/workflows/native-ci.yml)" -eq 2
	@test "$$(rg -Fxc "      - '.dockerignore'" .github/workflows/native-ci.yml)" -eq 2
	@test "$$(rg -Fxc "      - '.gitattributes'" .github/workflows/native-ci.yml)" -eq 2
	@test "$$(rg -Fxc "      - '.devcontainer/**'" .github/workflows/native-ci.yml)" -eq 2
	@test "$$(rg -Fxc "      - '.github/**'" .github/workflows/native-ci.yml)" -eq 2
	@if sed -n '/^  native-release:$$/,/^  native-attest:$$/p' .github/workflows/native-ci.yml | \
		rg -q 'id-token: write|attestations: write'; then \
		echo "Pull-request release builds must not receive attestation credentials."; exit 1; \
	fi
	@sed -n '/^  native-attest:$$/,$$p' .github/workflows/native-ci.yml | rg -Fq 'id-token: write'
	@sed -n '/^  native-attest:$$/,$$p' .github/workflows/native-ci.yml | rg -Fq 'attestations: write'
	@echo "Debian 13 amd64 bare-metal release, runtime ABI, and hardened systemd contracts are present."

frontend-source-audit:
	$(DCF) build frontend-build
	$(FRONTEND_RUN) -v "$(CURDIR):/src:ro" frontend -lc \
		'node /src/frontend/scripts/frontend-source-audit.mjs'

parity-config-audit:
	$(DCF) build frontend-build
	$(FRONTEND_RUN) -v "$(CURDIR):/src:ro" frontend -lc \
		'node /src/frontend/scripts/parity-config-audit.mjs'

ui-sync-audit:
	$(DCF) build frontend-build
	$(FRONTEND_RUN) -v "$(CURDIR):/src:ro" frontend -lc \
		'node /src/frontend/scripts/ui-sync-audit.mjs'

reference-oracle-check:
	@test -f references/wyx2685-v2board/public/theme/default/dashboard.blade.php || { \
		echo "Reference submodule is unavailable; run: git submodule update --init --recursive"; exit 1; }
	@test -f references/wyx2685-v2board/public/theme/default/assets/umi.js
	@test -f references/wyx2685-v2board/public/assets/admin/umi.js
	@test -f references/wyx2685-v2board/resources/views/admin.blade.php
	@echo "Pinned reference project is complete and remains read-only."

reference-oracle-up: reference-oracle-check
	$(REFERENCE_DCF) up -d --build reference-oracle
	@echo "Reference UI: http://localhost:8001 (test-only, read-only assets)"

reference-oracle-down:
	$(REFERENCE_DCF) stop reference-oracle >/dev/null 2>&1 || true
	$(REFERENCE_DCF) rm -f reference-oracle >/dev/null 2>&1 || true

deploy-smoke: cloudflared-config-audit
	$(DCF) up -d --build --wait postgres clickhouse redis rust-api
	$(FRONTEND_RUN) \
		-e "DEPLOY_SMOKE_BASE_URL=$(SOURCE_BASE_URL)" \
		-e "DEPLOY_SMOKE_ADMIN_PATH=$(ADMIN_PATH)" \
		frontend -lc 'set -eu; \
		test -L /app/frontend-deploy/current; \
		test -f /app/frontend-deploy/current/user/index.html; \
		test -f /app/frontend-deploy/current/user/manifest.json; \
		test -f /app/frontend-deploy/current/admin/index.html; \
		test -f /app/frontend-deploy/current/admin/manifest.json; \
		! find /app/frontend-deploy/current -type f \( -name "umi.js" -o -name "umi.css" -o -name "components.chunk.css" -o -name "vendors.async.js" -o -name "components.async.js" \) | grep -q .; \
		node /src/frontend/scripts/deploy-smoke.mjs'

visual-smoke: deploy-smoke
	$(DCF) build frontend-build
	$(FRONTEND_RUN) \
		-e VISUAL_SMOKE_BASE_URL=$(SOURCE_BASE_URL) \
		-e VISUAL_SMOKE_ADMIN_PATH=$(ADMIN_PATH) \
		frontend -lc '$(FRONTEND_SETUP) && $(PLAYWRIGHT_SETUP) && node scripts/visual-smoke.mjs'

interaction-parity: reference-oracle-check deploy-smoke
	$(DCF) build frontend-build
	$(FRONTEND_RUN) \
		-v "$(CURDIR)/references/wyx2685-v2board:/reference:ro" \
		-e INTERACTION_PARITY_ARTIFACT_DIR=$(INTERACTION_PARITY_ARTIFACT_DIR) \
		-e INTERACTION_PARITY_SCENARIOS='$(INTERACTION_PARITY_SCENARIOS)' \
		-e PARITY_WORKERS='$(PARITY_WORKERS)' \
		-e REFERENCE_ORACLE_ROOT=/reference \
		-e REFERENCE_ORACLE_STATE_ROOT=/tmp/v2board-reference-oracle \
		-e VISUAL_PARITY_ADMIN_PATH=$(ADMIN_PATH) \
		-e VISUAL_PARITY_SOURCE_BASE_URL=$(SOURCE_BASE_URL) \
		-e VISUAL_PARITY_VIEWPORTS='$(VISUAL_PARITY_VIEWPORTS)' \
		frontend -lc '$(FRONTEND_SETUP) && $(PLAYWRIGHT_SETUP) && rm -rf /tmp/v2board-reference-oracle && pnpm exec playwright test'

accessibility-smoke:
	$(MAKE) --no-print-directory interaction-parity \
		INTERACTION_PARITY_SCENARIOS='$(A11Y_SMOKE_SCENARIOS)'

behavior-parity: interaction-parity

clean-frontend-runs:
	@$(DCF) rm -sf frontend-build reference-oracle >/dev/null 2>&1 || true

clean-host:
	@for path in \
		backend/rust/target \
		frontend/node_modules frontend/.pnpm-store frontend/dist frontend/dist-deploy frontend/.vite frontend/.cache frontend/coverage \
		frontend/apps/user/node_modules frontend/apps/user/dist frontend/apps/user/.vite frontend/apps/user/.cache frontend/apps/user/coverage \
		frontend/apps/admin/node_modules frontend/apps/admin/dist frontend/apps/admin/.vite frontend/apps/admin/.cache frontend/apps/admin/coverage \
		frontend/packages/*/node_modules frontend/packages/*/.vite frontend/packages/*/.cache frontend/packages/*/coverage; do \
		[ ! -e "$$path" ] || echo "$$path"; \
	done
	@echo "Preview only. Run make clean-host-apply after confirming every path is disposable."

clean-host-apply:
	@for path in \
		backend/rust/target \
		frontend/node_modules frontend/.pnpm-store frontend/dist frontend/dist-deploy frontend/.vite frontend/.cache frontend/coverage \
		frontend/apps/user/node_modules frontend/apps/user/dist frontend/apps/user/.vite frontend/apps/user/.cache frontend/apps/user/coverage \
		frontend/apps/admin/node_modules frontend/apps/admin/dist frontend/apps/admin/.vite frontend/apps/admin/.cache frontend/apps/admin/coverage \
		frontend/packages/*/node_modules frontend/packages/*/.vite frontend/packages/*/.cache frontend/packages/*/coverage; do \
		[ ! -e "$$path" ] || rm -rf "$$path"; \
	done
	@echo "Disposable host build output removed."

mailpit-ui:
	@echo "http://localhost:8025"

admin-url:
	@echo "http://localhost:8000/$(ADMIN_PATH)"
