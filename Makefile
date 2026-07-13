.PHONY: up down reset sync logs ps shell doctor \
	rust-check rust-test rust-integration rust-lifecycle-ledger-integration rust-legacy-mysql-integration rust-legacy-backup-integration rust-legacy-converter-integration rust-legacy-postgres-integration rust-legacy-redis-integration rust-route-audit rust-worker-reconcile rust-target-gate \
	bare-metal-fault-matrix-plan bare-metal-fault-matrix-audit bare-metal-fault-matrix-verify-guest bare-metal-fault-matrix \
	public-bundle-audit runtime-isolation-audit native-database-audit native-release-audit frontend-source-audit parity-config-audit ui-sync-audit \
	deploy-smoke visual-smoke interaction-parity accessibility-smoke behavior-parity \
	reference-oracle-check reference-oracle-up reference-oracle-down \
	clean-frontend-runs clean-host clean-host-apply mailpit-ui admin-url

# Integration lanes share disposable service identities inside one Compose
# project. Keep a single make invocation strictly sequential; every lane also
# uses a unique database where the datastore permits it.
.NOTPARALLEL: rust-integration rust-lifecycle-ledger-integration rust-legacy-mysql-integration rust-legacy-backup-integration rust-legacy-converter-integration rust-legacy-postgres-integration rust-legacy-redis-integration

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
BARE_METAL_MATRIX_ADAPTER ?=
BARE_METAL_MATRIX_MANIFEST ?=
BARE_METAL_MATRIX_RELEASE ?=
BARE_METAL_MATRIX_GUEST_BINARY ?=
BARE_METAL_MATRIX_REVISION ?=
BARE_METAL_MATRIX_OUTPUT ?=
BARE_METAL_MATRIX_HARD_RESET ?= not-run

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

bare-metal-fault-matrix-plan:
	@backend/rust/scripts/run-bare-metal-fault-matrix-supervisor.sh \
		--hard-reset "$(BARE_METAL_MATRIX_HARD_RESET)"

bare-metal-fault-matrix-audit:
	@bash -n \
		backend/rust/scripts/run-bare-metal-fault-matrix-supervisor.sh \
		backend/rust/test-fixtures/legacy-fault-matrix/cleanup-guest.sh
	@backend/rust/scripts/run-bare-metal-fault-matrix-supervisor.sh >/dev/null
	@if backend/rust/scripts/run-bare-metal-fault-matrix-supervisor.sh --execute >/dev/null 2>&1; then \
		echo 'matrix supervisor accepted an incomplete destructive invocation'; exit 1; \
	fi
	@if backend/rust/scripts/run-bare-metal-fault-matrix-supervisor.sh --execute \
		--guest-binary /definitely-missing/v2board-matrix-guest >/dev/null 2>&1; then \
		echo 'matrix supervisor accepted a missing guest binary'; exit 1; \
	fi
	@if backend/rust/test-fixtures/legacy-fault-matrix/cleanup-guest.sh \
		--expected-guest-id audit-missing-marker >/dev/null 2>&1; then \
		echo 'matrix guest cleanup accepted a host without its disposable marker'; exit 1; \
	fi
	@! rg -n 'v2board-bare-metal-fault-matrix|bare-metal-fault-matrix' \
		Dockerfile.rust deploy/systemd
	@rg -Fqx 'required-features = ["bare-metal-fault-matrix"]' \
		backend/rust/crates/lifecycle/Cargo.toml
	@rg -Fqx 'ConditionPathExists=/etc/v2board/bare-metal-fault-matrix-disposable.json' \
		backend/rust/test-fixtures/legacy-fault-matrix/systemd/v2board-fault-matrix-guest.service
	@rg -Fq '.process_alive_before_reset == true' \
		backend/rust/scripts/run-bare-metal-fault-matrix-supervisor.sh
	@rg -Fq 'hard_reset_complete_case_count -eq $$hard_reset_expected_case_count' \
		backend/rust/scripts/run-bare-metal-fault-matrix-supervisor.sh
	@rg -Fq 'bare-metal-fault-matrix-verify-guest' \
		backend/rust/scripts/run-bare-metal-fault-matrix-supervisor.sh
	@rg -Fq '.production_capability_available == false' \
		backend/rust/scripts/run-bare-metal-fault-matrix-supervisor.sh
	@rg -Fq '.machine.machine_id_sha256 == $$machine_id_sha256' \
		backend/rust/scripts/run-bare-metal-fault-matrix-supervisor.sh
	@! rg -n '^\[Install\]$$' \
		backend/rust/test-fixtures/legacy-fault-matrix/systemd/*.service
	$(DCF) build rust-api
	$(DCF) run --rm -T --no-deps --entrypoint bash rust-api -lc \
		'. /usr/local/cargo/env; set -eu; \
		 cargo check --locked -p v2board-lifecycle --no-default-features --bin v2board-lifecycle; \
		 cargo clippy --locked -p v2board-lifecycle --features bare-metal-fault-matrix \
			--bin v2board-bare-metal-fault-matrix-guest --tests -- -D warnings; \
		 cargo test --locked -p v2board-lifecycle --features bare-metal-fault-matrix \
			--bin v2board-bare-metal-fault-matrix-guest; \
		 cargo test --locked -p v2board-provision --features bare-metal-fault-matrix \
			bare_metal_fault_matrix::tests'
	@echo 'Bare-metal matrix scripts fail closed and test-only fixtures are absent from production release inputs.'

bare-metal-fault-matrix-verify-guest:
	@test -n "$(BARE_METAL_MATRIX_GUEST_BINARY)" || \
		{ echo 'BARE_METAL_MATRIX_GUEST_BINARY is required'; exit 64; }
	@case "$(BARE_METAL_MATRIX_GUEST_BINARY)" in /*) ;; \
		*) echo 'BARE_METAL_MATRIX_GUEST_BINARY must be absolute'; exit 64 ;; esac
	@test -f "$(BARE_METAL_MATRIX_GUEST_BINARY)" && test ! -L "$(BARE_METAL_MATRIX_GUEST_BINARY)" || \
		{ echo 'BARE_METAL_MATRIX_GUEST_BINARY must be a regular non-symlink file'; exit 64; }
	@test -n "$(BARE_METAL_MATRIX_REVISION)" || \
		{ echo 'BARE_METAL_MATRIX_REVISION is required'; exit 64; }
	@test "$$(git rev-parse HEAD)" = "$(BARE_METAL_MATRIX_REVISION)" || \
		{ echo 'BARE_METAL_MATRIX_REVISION must equal clean HEAD'; exit 65; }
	@test -z "$$(git status --porcelain=v1 --untracked-files=all)" || \
		{ echo 'guest rebuild verification requires a clean worktree'; exit 65; }
	$(DCF) build rust-api
	$(DCF) run --rm -T --no-deps --entrypoint bash \
		-v "$(BARE_METAL_MATRIX_GUEST_BINARY):/matrix-input/v2board-bare-metal-fault-matrix-guest:ro" \
		rust-api -lc \
		'. /usr/local/cargo/env; set -eu; \
		 cargo build --release --locked -p v2board-lifecycle --features bare-metal-fault-matrix \
			--bin v2board-bare-metal-fault-matrix-guest; \
		 cmp /app/target/release/v2board-bare-metal-fault-matrix-guest \
			/matrix-input/v2board-bare-metal-fault-matrix-guest'

bare-metal-fault-matrix: bare-metal-fault-matrix-audit
	@test -n "$(BARE_METAL_MATRIX_ADAPTER)" || \
		{ echo 'BARE_METAL_MATRIX_ADAPTER is required'; exit 64; }
	@test -n "$(BARE_METAL_MATRIX_MANIFEST)" || \
		{ echo 'BARE_METAL_MATRIX_MANIFEST is required'; exit 64; }
	@test -n "$(BARE_METAL_MATRIX_RELEASE)" || \
		{ echo 'BARE_METAL_MATRIX_RELEASE is required'; exit 64; }
	@test -n "$(BARE_METAL_MATRIX_GUEST_BINARY)" || \
		{ echo 'BARE_METAL_MATRIX_GUEST_BINARY is required'; exit 64; }
	@case "$(BARE_METAL_MATRIX_GUEST_BINARY)" in /*) ;; \
		*) echo 'BARE_METAL_MATRIX_GUEST_BINARY must be absolute'; exit 64 ;; esac
	@test -f "$(BARE_METAL_MATRIX_GUEST_BINARY)" && test ! -L "$(BARE_METAL_MATRIX_GUEST_BINARY)" || \
		{ echo 'BARE_METAL_MATRIX_GUEST_BINARY must be a regular non-symlink file'; exit 64; }
	@test -n "$(BARE_METAL_MATRIX_REVISION)" || \
		{ echo 'BARE_METAL_MATRIX_REVISION is required'; exit 64; }
	@test -n "$(BARE_METAL_MATRIX_OUTPUT)" || \
		{ echo 'BARE_METAL_MATRIX_OUTPUT is required'; exit 64; }
	@backend/rust/scripts/run-bare-metal-fault-matrix-supervisor.sh --execute \
		--adapter "$(BARE_METAL_MATRIX_ADAPTER)" \
		--manifest "$(BARE_METAL_MATRIX_MANIFEST)" \
		--release "$(BARE_METAL_MATRIX_RELEASE)" \
		--guest-binary "$(BARE_METAL_MATRIX_GUEST_BINARY)" \
		--revision "$(BARE_METAL_MATRIX_REVISION)" \
		--output "$(BARE_METAL_MATRIX_OUTPUT)" \
		--hard-reset "$(BARE_METAL_MATRIX_HARD_RESET)"

rust-integration: rust-lifecycle-ledger-integration rust-legacy-mysql-integration rust-legacy-converter-integration rust-legacy-postgres-integration rust-legacy-redis-integration
	$(DCF) build rust-api
	$(DCF) up -d --wait postgres clickhouse redis
	$(DCF) exec -T postgres dropdb --force --if-exists -U v2board v2board_analytics_test
	$(DCF) exec -T postgres createdb -U v2board v2board_analytics_test
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
		rust-api -lc \
		'set -eu; . /usr/local/cargo/env; \
		 cargo test --locked -p v2board-analytics --test clickhouse_roundtrip; \
		 cargo test --locked -p v2board-analytics --test outbox_roundtrip; \
		 cargo build --locked -p v2board-workers; \
		 cargo run --locked -p v2board-contract -- production-invariants'

rust-lifecycle-ledger-integration:
	$(DCF) build rust-api
	$(DCF) up -d --wait postgres
	@set -eu; \
		database=v2board_lifecycle_ledger_test; \
		migration_role=v2_lifecycle_migration; \
		api_role=v2_lifecycle_api; \
		worker_role=v2_lifecycle_worker; \
		cleanup() { \
			$(DCF) exec -T postgres dropdb --force --if-exists -U v2board "$$database" >/dev/null 2>&1 || true; \
			$(DCF) exec -T postgres psql -v ON_ERROR_STOP=1 -U v2board -d postgres \
				-c "DROP ROLE IF EXISTS $$api_role" \
				-c "DROP ROLE IF EXISTS $$worker_role" \
				-c "DROP ROLE IF EXISTS $$migration_role" >/dev/null 2>&1 || true; \
		}; \
		trap cleanup EXIT INT TERM; \
		cleanup; \
		$(DCF) exec -T postgres psql -v ON_ERROR_STOP=1 -U v2board -d postgres \
			-c "CREATE ROLE $$migration_role LOGIN PASSWORD 'migration-test-password' NOSUPERUSER NOCREATEDB NOCREATEROLE NOINHERIT" \
			-c "CREATE ROLE $$api_role LOGIN PASSWORD 'api-test-password' NOSUPERUSER NOCREATEDB NOCREATEROLE NOINHERIT" \
			-c "CREATE ROLE $$worker_role LOGIN PASSWORD 'worker-test-password' NOSUPERUSER NOCREATEDB NOCREATEROLE NOINHERIT"; \
		$(DCF) exec -T postgres createdb -U v2board -O "$$migration_role" "$$database"; \
		$(DCF) run --rm -T --no-deps --entrypoint bash \
			-e RUST_INTEGRATION_LIFECYCLE_DATABASE_URL=postgresql://$$migration_role:migration-test-password@postgres:5432/$$database \
			-e RUST_INTEGRATION_LIFECYCLE_API_DATABASE_URL=postgresql://$$api_role:api-test-password@postgres:5432/$$database \
			-e RUST_INTEGRATION_LIFECYCLE_WORKER_DATABASE_URL=postgresql://$$worker_role:worker-test-password@postgres:5432/$$database \
		rust-api -lc \
		'. /usr/local/cargo/env; bash scripts/run-exact-ignored-test.sh v2board-provision \
		 lifecycle_ledger::tests::postgres_lifecycle_ledger_is_atomic_idempotent_and_fail_closed'

rust-legacy-mysql-integration:
	@set -eu; \
		cleanup() { $(DCF) rm -sf legacy-mysql legacy-mysql80 legacy-mysql-restore >/dev/null 2>&1 || true; }; \
		trap cleanup EXIT INT TERM; \
		$(DCF) build rust-api legacy-test-runner; \
		cleanup; \
		$(DCF) up -d --wait legacy-mysql legacy-mysql80 legacy-mysql-restore; \
		$(DCF) run --rm -T --no-deps --entrypoint bash \
			-e V2BOARD_LEGACY_MYSQL_TEST_URL=mysql://v2board_reader:v2board-reader-test-password@legacy-mysql:3306/v2board \
			-e V2BOARD_LEGACY_FIXTURE_DATABASE_URL=mysql://root:legacy-root-test-password@legacy-mysql:3306/v2board \
			rust-api -lc \
			'. /usr/local/cargo/env; \
			 bash scripts/run-exact-ignored-test.sh v2board-provision inspect::tests::legacy_inspection_pool_keeps_the_same_raw_snapshot_session; \
			 bash scripts/run-exact-ignored-test.sh v2board-provision inspect::tests::pinned_mysql8_source_supports_the_complete_query_surface; \
			 bash scripts/run-exact-ignored-test.sh v2board-provision legacy_copy::tests::mysql_json_object_preserves_integer_decimal_unicode_and_nul_fixture; \
			 bash scripts/run-exact-ignored-test.sh v2board-provision native_legacy_source::tests::mysql_super_read_only_fence_is_durable_exact_and_retryable'; \
		$(DCF) run --rm -T --no-deps --entrypoint bash \
			-e V2BOARD_LEGACY_MYSQL_TEST_URL=mysql://v2board_reader:v2board-reader-test-password@legacy-mysql80:3306/v2board \
			-e V2BOARD_LEGACY_FIXTURE_DATABASE_URL=mysql://root:legacy80-root-test-password@legacy-mysql80:3306/v2board \
			rust-api -lc \
			'. /usr/local/cargo/env; \
			 bash scripts/run-exact-ignored-test.sh v2board-provision inspect::tests::legacy_inspection_pool_keeps_the_same_raw_snapshot_session; \
			 bash scripts/run-exact-ignored-test.sh v2board-provision inspect::tests::pinned_mysql8_source_supports_the_complete_query_surface; \
			 bash scripts/run-exact-ignored-test.sh v2board-provision legacy_copy::tests::mysql_json_object_preserves_integer_decimal_unicode_and_nul_fixture; \
			 bash scripts/run-exact-ignored-test.sh v2board-provision native_legacy_source::tests::mysql_super_read_only_fence_is_durable_exact_and_retryable'; \
		$(DCF) run --rm -T --no-deps --entrypoint bash \
			-e V2BOARD_BACKUP_E2E_SOURCE_URL=mysql://v2board_reader:v2board-reader-test-password@legacy-mysql:3306/v2board \
			-e V2BOARD_BACKUP_E2E_RESTORE_URL=mysql://root:restore-root-test-password@legacy-mysql-restore:3306/v2board_restore \
			-e V2BOARD_BACKUP_E2E_AGE_IDENTITY_PATH=/tmp/v2board-age/identity.txt \
			-e V2BOARD_BACKUP_E2E_AGE_RECIPIENT_PATH=/tmp/v2board-age/recipient.txt \
			legacy-test-runner -lc \
			'set -eu; install -d -m 0700 /tmp/v2board-age; \
			 age-keygen -o /tmp/v2board-age/identity.txt >/dev/null; \
			 age-keygen -y /tmp/v2board-age/identity.txt > /tmp/v2board-age/recipient.txt; \
			 . /usr/local/cargo/env; \
			 bash scripts/run-exact-ignored-test.sh v2board-provision legacy_backup::tests::real_age_stream_dump_restore_preserves_the_reference_fingerprint'; \
		$(DCF) run --rm -T --no-deps --entrypoint bash \
			-e V2BOARD_BACKUP_E2E_SOURCE_URL=mysql://v2board_reader:v2board-reader-test-password@legacy-mysql80:3306/v2board \
			-e V2BOARD_BACKUP_E2E_RESTORE_URL=mysql://root:restore-root-test-password@legacy-mysql-restore:3306/v2board_restore \
			-e V2BOARD_BACKUP_E2E_AGE_IDENTITY_PATH=/tmp/v2board-age/identity.txt \
			-e V2BOARD_BACKUP_E2E_AGE_RECIPIENT_PATH=/tmp/v2board-age/recipient.txt \
			legacy-test-runner -lc \
			'set -eu; install -d -m 0700 /tmp/v2board-age; \
			 age-keygen -o /tmp/v2board-age/identity.txt >/dev/null; \
			 age-keygen -y /tmp/v2board-age/identity.txt > /tmp/v2board-age/recipient.txt; \
			 . /usr/local/cargo/env; \
			 bash scripts/run-exact-ignored-test.sh v2board-provision legacy_backup::tests::real_age_stream_dump_restore_preserves_the_reference_fingerprint'

rust-legacy-backup-integration: rust-legacy-mysql-integration
	@:

# This target intentionally follows the MySQL/backup gate so both tests own
# the same disposable Compose source container sequentially, even under -j.
rust-legacy-converter-integration: rust-legacy-mysql-integration
	@set -eu; \
		database84v4=v2board_legacy_converter84_v4_test_$$$$; \
		database84v5=v2board_legacy_converter84_v5_test_$$$$; \
		database80v4=v2board_legacy_converter80_v4_test_$$$$; \
		database80v5=v2board_legacy_converter80_v5_test_$$$$; \
		clickhouse84v4=v2board_legacy_converter84_v4_test_$$$$; \
		clickhouse84v5=v2board_legacy_converter84_v5_test_$$$$; \
		clickhouse80v4=v2board_legacy_converter80_v4_test_$$$$; \
		clickhouse80v5=v2board_legacy_converter80_v5_test_$$$$; \
		cleanup() { \
			$(DCF) rm -sf legacy-mysql legacy-mysql80 >/dev/null 2>&1 || true; \
			for database in "$$database84v4" "$$database84v5" "$$database80v4" "$$database80v5"; do \
				$(DCF) exec -T postgres dropdb --force --if-exists -U v2board "$$database" >/dev/null 2>&1 || true; \
			done; \
			for database in "$$clickhouse84v4" "$$clickhouse84v5" "$$clickhouse80v4" "$$clickhouse80v5"; do \
				$(DCF) exec -T clickhouse clickhouse-client --user v2board_analytics --password v2board \
					--query "DROP DATABASE IF EXISTS $$database SYNC" >/dev/null 2>&1 || true; \
			done; \
		}; \
		run_case() { \
			$(DCF) exec -T postgres createdb -U v2board "$$3"; \
			$(DCF) exec -T clickhouse clickhouse-client --user v2board_analytics --password v2board \
				--query "CREATE DATABASE $$4"; \
			$(DCF) run --rm -T --no-deps --entrypoint bash \
				-e V2BOARD_LEGACY_CONVERTER_SCHEMA_VERSION="$$2" \
				-e V2BOARD_LEGACY_CONVERTER_MYSQL_URL=mysql://v2board_reader:v2board-reader-test-password@"$$1":3306/v2board \
				-e V2BOARD_LEGACY_CONVERTER_POSTGRES_URL=postgresql://v2board:v2board@postgres:5432/"$$3" \
				-e V2BOARD_LEGACY_CONVERTER_CLICKHOUSE_URL=http://clickhouse:8123 \
				-e V2BOARD_LEGACY_CONVERTER_CLICKHOUSE_DATABASE="$$4" \
				-e V2BOARD_LEGACY_CONVERTER_CLICKHOUSE_USERNAME=v2board_analytics \
				-e V2BOARD_LEGACY_CONVERTER_CLICKHOUSE_PASSWORD=v2board \
				rust-api -lc \
				'. /usr/local/cargo/env; bash scripts/run-exact-ignored-test.sh v2board-provision \
				 legacy_copy::tests::all_legacy_tables_follow_strategy_to_postgres_project_clickhouse_and_retry_exactly'; \
		}; \
		trap cleanup EXIT INT TERM; \
		$(DCF) build rust-api; \
		$(DCF) up -d --wait postgres clickhouse; \
		cleanup; \
		$(DCF) up -d --wait legacy-mysql legacy-mysql80; \
		run_case legacy-mysql 4 "$$database84v4" "$$clickhouse84v4"; \
		run_case legacy-mysql 5 "$$database84v5" "$$clickhouse84v5"; \
		run_case legacy-mysql80 4 "$$database80v4" "$$clickhouse80v4"; \
		run_case legacy-mysql80 5 "$$database80v5" "$$clickhouse80v5"

rust-legacy-redis-integration:
	@set -eu; \
		cleanup() { $(DCF) rm -sf legacy-redis >/dev/null 2>&1 || true; }; \
		trap cleanup EXIT INT TERM; \
		$(DCF) build rust-api; \
		cleanup; \
		$(DCF) up -d --wait legacy-redis; \
		$(DCF) run --rm -T --no-deps --entrypoint bash \
			-e V2BOARD_LEGACY_REDIS_TEST_URL=redis://legacy-redis:6379/0 \
			rust-api -lc \
			'. /usr/local/cargo/env; bash scripts/run-exact-ignored-test.sh v2board-provision \
			 native_legacy_source::tests::redis_traffic_freeze_is_atomic_retry_exact_and_preserves_direction_presence'

rust-legacy-postgres-integration:
	$(DCF) build rust-api
	$(DCF) up -d --wait postgres
	@set -eu; \
		database=v2board_legacy_copy_test_$$$$; \
		cleanup() { \
			$(DCF) exec -T postgres dropdb --force --if-exists -U v2board "$$database" >/dev/null 2>&1 || true; \
		}; \
		trap cleanup EXIT INT TERM; \
		cleanup; \
		$(DCF) exec -T postgres createdb -U v2board "$$database"; \
		$(DCF) run --rm -T --no-deps --entrypoint bash \
			-e V2BOARD_TRAFFIC_FOLD_POSTGRES_URL=postgresql://v2board:v2board@postgres:5432/$$database \
			-e V2BOARD_RUNTIME_ACL_TEST_POSTGRES_URL=postgresql://v2board:v2board@postgres:5432/$$database \
			rust-api -lc \
			'. /usr/local/cargo/env; \
			 bash scripts/run-exact-ignored-test.sh v2board-provision legacy_copy::tests::postgres_traffic_fold_is_atomic_append_only_and_retry_exact; \
			 bash scripts/run-exact-ignored-test.sh v2board-provision postgres_runtime_grants::tests::postgres_18_catalog_proves_exact_acl_and_detects_protected_grant'

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
	@paths='backend/rust/target frontend/.pnpm-store frontend/dist frontend/dist-deploy frontend/.cache frontend/coverage frontend/apps/user/dist frontend/apps/admin/dist public/theme/default public/assets/admin native-release lifecycle-tool v2board-native-linux-amd64.tar.gz v2board-native-linux-amd64.tar.gz.sha256'; \
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
	@graph="$$( $(DCF) run --rm -T --no-deps --entrypoint bash rust-api -lc \
		'. /usr/local/cargo/env; cargo tree --locked -e normal -p v2board-lifecycle' \
		)" || exit $$?; \
	printf '%s\n' "$$graph" | rg -q 'v2board-provision'; \
	printf '%s\n' "$$graph" | rg -q 'sqlx-mysql'
	@test ! -d backend/rust/migrations || \
		test -z "$$(find backend/rust/migrations -type f -print -quit)"
	@rg -q 'migrations-postgres' backend/rust/crates/db/src/pool.rs
	@echo "API/worker/schema graphs exclude MySQL; only the isolated lifecycle graph contains the adapter."

native-release-audit:
	@test -f deploy/systemd/v2board-api.service
	@test -f deploy/systemd/v2board-worker.service
	@rg -q '^FROM scratch AS lifecycle-tool$$' Dockerfile.rust
	@rg -q '^COPY --from=lifecycle-builder /out/v2board-lifecycle /v2board-lifecycle$$' Dockerfile.rust
	@rg -q '^FROM scratch AS native-release$$' Dockerfile.rust
	@rg -q '^COPY --from=native-release-assembler /release/ /$$' Dockerfile.rust
	@rg -Fq 'test -L frontend/previous' Dockerfile.rust
	@if rg -n '^FROM .* AS production(-api|-worker|-base|-lifecycle)?$$|^(HEALTHCHECK|ENTRYPOINT|CMD|VOLUME) ' Dockerfile.rust; then \
		echo "Dockerfile.rust must export a filesystem payload, not a production runtime image."; exit 1; \
	fi
	@for unit in deploy/systemd/v2board-api.service deploy/systemd/v2board-worker.service; do \
		rg -Fqx 'NoNewPrivileges=true' "$$unit"; \
		rg -Fqx 'ProtectSystem=strict' "$$unit"; \
		rg -Fqx 'ProtectHome=true' "$$unit"; \
		rg -Fqx 'PrivateTmp=true' "$$unit"; \
		rg -Fqx 'CapabilityBoundingSet=' "$$unit"; \
	done
	@rg -Fqx 'User=v2board-api' deploy/systemd/v2board-api.service
	@rg -Fqx 'ReadWritePaths=/var/lib/v2board/api' deploy/systemd/v2board-api.service
	@rg -Fqx 'User=v2board-worker' deploy/systemd/v2board-worker.service
	@rg -Fqx 'Type=notify' deploy/systemd/v2board-worker.service
	@rg -Fqx 'WatchdogSec=30s' deploy/systemd/v2board-worker.service
	@rg -Fqx 'ReadWritePaths=/var/lib/v2board/worker' deploy/systemd/v2board-worker.service
	@rg -q '^  native-release:$$' .github/workflows/native-ci.yml
	@rg -q -- '--target native-release' .github/workflows/native-ci.yml
	@rg -q -- '--output type=local,dest=native-release' .github/workflows/native-ci.yml
	@rg -Fq 'inspect-release-archive' .github/workflows/native-ci.yml
	@echo "Bare-metal release export and hardened systemd unit contracts are present."

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

deploy-smoke:
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
