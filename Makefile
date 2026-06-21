.PHONY: up down logs shell reset sync ps doctor public-bundle-audit replica-audit parity-config-audit legacy-oracle-check legacy-oracle-up legacy-oracle-serve legacy-oracle-down deploy-smoke deploy-public-sync deploy-public-check deploy-public-ensure visual-smoke visual-parity interaction-parity clean-frontend-runs clean-host clean-host-apply mailpit-ui admin-url

DC := $(shell \
	if docker compose version >/dev/null 2>&1; then echo "docker compose"; \
	elif [ -x /opt/homebrew/lib/docker/cli-plugins/docker-compose ]; then echo "/opt/homebrew/lib/docker/cli-plugins/docker-compose"; \
	elif command -v docker-compose >/dev/null 2>&1; then echo "docker-compose"; \
	else echo ""; fi)

COMPOSE_FILE ?= docker-compose.local.yml
COMPOSE_PROJECT ?= v2board
DCF := $(DC) -p $(COMPOSE_PROJECT) -f $(COMPOSE_FILE)
FRONTEND_RUN := $(DCF) run --rm -T --no-deps --entrypoint sh
FRONTEND_SERVE_RUN = $(DCF) run --rm -T --no-deps --entrypoint sh -p $(LEGACY_ORACLE_PORT):$(LEGACY_ORACLE_PORT)
FRONTEND_WORKSPACE_BOOTSTRAP := if [ ! -f /app/frontend/package.json ]; then mkdir -p /app/frontend && tar --exclude=node_modules --exclude=.pnpm-store --exclude=dist --exclude=dist-deploy -C /src/frontend -cf - . | tar -C /app/frontend -xf -; fi
FRONTEND_SETUP := $(FRONTEND_WORKSPACE_BOOTSTRAP) && corepack enable && corepack prepare pnpm@11.0.0 --activate >/dev/null && pnpm config set store-dir /app/frontend/.pnpm-store >/dev/null
FRONTEND_INSTALL := HUSKY=0 pnpm install --frozen-lockfile
FRONTEND_FAST_INSTALL := if [ ! -x /app/frontend/node_modules/.bin/playwright ]; then HUSKY=0 pnpm install --frozen-lockfile; fi
FRONTEND_BOOTSTRAP := $(FRONTEND_SETUP) && $(FRONTEND_INSTALL)
FRONTEND_FAST_BOOTSTRAP := $(FRONTEND_SETUP) && $(FRONTEND_FAST_INSTALL)
PLAYWRIGHT_CHROMIUM_BOOTSTRAP := if ! find /app/frontend/.cache/ms-playwright -path "*/chrome-linux/chrome" -type f 2>/dev/null | grep -q .; then pnpm exec playwright install chromium >/dev/null; fi
LEGACY_ORACLE_REF_FILE := frontend/fixtures/legacy-oracle.ref
LEGACY_ORACLE_REF ?= $(shell test -s $(LEGACY_ORACLE_REF_FILE) && sed -n '1p' $(LEGACY_ORACLE_REF_FILE))
LEGACY_ORACLE_CONTAINER ?= $(COMPOSE_PROJECT)-legacy-oracle
LEGACY_ORACLE_VOLUME ?= $(COMPOSE_PROJECT)_legacy-oracle
LEGACY_ORACLE_PORT ?= 8001
LEGACY_ORACLE_NODE_OPTIONS ?= --max-old-space-size=128
LEGACY_ORACLE_PAUSE_SERVICES ?= frontend horizon scheduler mailpit mysql redis
LEGACY_ORACLE_RESUME_SERVICES ?= mysql redis mailpit app
LEGACY_ORACLE_REQUIRED_PATHS := \
	public/theme/default/dashboard.blade.php \
	public/theme/default/config.json \
	public/theme/default/assets/components.chunk.css \
	public/theme/default/assets/vendors.async.js \
	public/theme/default/assets/components.async.js \
	public/theme/default/assets/umi.css \
	public/theme/default/assets/umi.js \
	public/theme/default/assets/i18n/zh-CN.js \
	public/theme/default/assets/i18n/zh-TW.js \
	public/theme/default/assets/i18n/en-US.js \
	public/theme/default/assets/i18n/ja-JP.js \
	public/theme/default/assets/i18n/vi-VN.js \
	public/theme/default/assets/i18n/ko-KR.js \
	public/theme/default/assets/i18n/fa-IR.js \
	public/theme/default/assets/images \
	public/theme/default/assets/static \
	public/theme/default/assets/theme/default.css \
	public/theme/default/assets/theme/black.css \
	public/theme/default/assets/theme/darkblue.css \
	public/theme/default/assets/theme/green.css \
	public/assets/admin/components.chunk.css \
	public/assets/admin/vendors.async.js \
	public/assets/admin/components.async.js \
	public/assets/admin/umi.css \
	public/assets/admin/umi.js \
	public/assets/admin/static \
	public/assets/admin/theme/default.css \
	public/assets/admin/theme/black.css \
	public/assets/admin/theme/darkblue.css \
	public/assets/admin/theme/green.css \
	resources/views/admin.blade.php
VISUAL_SOURCE_BASE_URL ?= http://host.docker.internal:8000
VISUAL_PARITY_ARTIFACT_DIR ?= /app/frontend/.cache/visual-parity
INTERACTION_PARITY_ARTIFACT_DIR ?= /app/frontend/.cache/interaction-parity
INTERACTION_PARITY_SCENARIOS ?= user-login-form-language user-login-language-persistence user-dashboard-header-language-dropdown user-dashboard-avatar-dropdown user-dashboard-dark-mode-persistence user-dashboard-subscribe-drawer user-dashboard-subscribe-import-links user-dashboard-notice-carousel user-dashboard-reset-package-confirm user-dashboard-new-period-confirm user-dashboard-alert-links user-profile-deposit-modal user-profile-reset-subscribe-confirm user-profile-telegram-bind-modal user-profile-telegram-unbind-confirm user-profile-preference-switches user-profile-redeem-giftcard user-profile-change-password-success \
	user-plans-filter-tabs user-plan-checkout-coupon user-order-payment-method user-order-qr-checkout user-order-stripe-disabled-checkout user-order-redirect-checkout \
	user-node-table-scroll user-node-tooltips user-traffic-table-scroll user-traffic-total-tooltip user-knowledge-drawer user-invite-generate user-invite-transfer-modal user-invite-withdraw-modal user-invite-tooltips user-ticket-reply-send \
	user-ticket-create-submit admin-ticket-reply-send admin-tickets-reply-filter admin-dashboard-dark-mode-persistence admin-dashboard-avatar-dropdown admin-dashboard-commission-shortcut user-order-cancel-confirm \
	admin-plan-create-drawer admin-plan-create-group-select-dropdown admin-plan-edit-drawer admin-plan-renew-tooltip admin-config-tabs admin-theme-settings-modal admin-server-create-node-drawer admin-server-edit-node-drawer admin-server-route-create-modal admin-server-route-edit-modal admin-server-group-create-modal admin-server-group-edit-modal admin-payment-create-modal admin-payment-edit-modal admin-payment-notify-tooltip \
	admin-order-detail-modal admin-order-status-tooltips admin-order-assign-modal admin-order-status-dropdown admin-order-commission-dropdown \
	admin-coupon-create-modal admin-coupon-range-picker admin-coupon-edit-modal admin-giftcard-create-modal admin-giftcard-edit-modal admin-notice-create-modal admin-notice-edit-modal admin-knowledge-create-drawer admin-knowledge-edit-drawer admin-users-filter-input admin-users-filter-field-select-dropdown admin-users-filter-expiry-picker admin-user-bulk-ban-confirm admin-user-bulk-delete-confirm admin-user-create-modal admin-user-create-plan-select-dropdown admin-user-create-expiry-picker admin-user-send-mail-modal admin-user-reset-secret-confirm admin-user-delete-confirm admin-user-copy-action admin-user-edit-action admin-user-assign-action admin-user-orders-action admin-user-invite-action admin-user-traffic-action
INTERACTION_PARITY_RETRIES ?= 4
INTERACTION_PARITY_SHARD_DELAY ?= 20
INTERACTION_PARITY_PAUSE_SERVICES ?= frontend horizon scheduler
INTERACTION_PARITY_RESUME_SERVICES ?= mysql redis mailpit app
VISUAL_PARITY_RESTART_SERVICES ?= 1
VISUAL_PARITY_SHARD_DELAY ?= 30
VISUAL_PARITY_RETRIES ?= 2
VISUAL_PARITY_RETRY_DELAY ?= 20
VISUAL_PARITY_MODE ?= screenshots
VISUAL_PARITY_PUBLIC_CHECKED ?= 0
VISUAL_PARITY_APP_READY ?= 0
VISUAL_PARITY_FILTER ?=
VISUAL_PARITY_INTERACTION_FILTER ?=
VISUAL_PARITY_VIEWPORT_FILTER ?=
VISUAL_PARITY_SKIP_DEPLOY ?= 0
VISUAL_PARITY_SCENARIOS ?= user-home-root user-login user-register-rich user-forget user-dashboard user-dashboard-dark user-plans user-plan-checkout user-orders user-order-detail user-node user-traffic user-invite user-tickets user-ticket-detail user-knowledge user-profile admin-dashboard admin-dashboard-dark admin-plans admin-orders admin-users admin-tickets admin-ticket-detail admin-config admin-theme admin-system admin-server-groups admin-server-manage admin-server-routes admin-payments admin-coupons admin-giftcards admin-notices admin-knowledge admin-root admin-login
VISUAL_PARITY_VIEWPORTS ?= desktop mobile
VISUAL_PARITY_CHECK_EACH_SHARD ?= 0
DEPLOY_BUILD_PAUSE_SERVICES ?= app frontend horizon scheduler mysql redis mailpit
DEPLOY_RESUME_SERVICES ?= mysql redis mailpit app
DEPLOY_FINAL_RESUME_SERVICES ?= mysql redis mailpit app
DEPLOY_NODE_OPTIONS ?= --max-old-space-size=256
DEPLOY_PUBLIC_ENSURE_RETRIES ?= 3
DEPLOY_PUBLIC_ENSURE_RETRY_DELAY ?= 20
VISUAL_PARITY_NODE_OPTIONS ?= --max-old-space-size=128
VISUAL_PARITY_FRESH_BROWSER ?= auto
VISUAL_PARITY_PAUSE_SERVICES ?= frontend horizon scheduler
VISUAL_PARITY_RESUME_SERVICES ?= mysql redis mailpit app

ifeq ($(DC),)
$(error docker compose not found; run 'brew install docker-compose' or add cliPluginsExtraDirs to ~/.docker/config.json)
endif

up:
	$(DCF) up -d --build
	@echo ""
	@echo "  user      http://localhost:5173            (new frontend)"
	@echo "  admin     http://localhost:5174            (new admin)"
	@echo "  laravel   http://localhost:8000            (source-built deploy/API; admin@local / 12345678)"
	@echo "  oracle    make legacy-oracle-up            (old packaged frontend on http://localhost:8001)"
	@echo "  mailpit   http://localhost:8025"
	@echo ""
	@echo "  note: source is copied into Docker volumes so the host repo stays clean."
	@echo "        after source edits, run 'make sync' to refresh app/frontend volumes."
	@echo "        tail startup with 'make logs' or '$(DCF) logs -f frontend'"

down:
	@docker rm -f $(LEGACY_ORACLE_CONTAINER) >/dev/null 2>&1 || true
	$(DCF) down

logs:
	$(DCF) logs -f --tail=100

shell:
	$(DCF) exec app bash

ps:
	$(DCF) ps

reset:
	@docker rm -f $(LEGACY_ORACLE_CONTAINER) >/dev/null 2>&1 || true
	$(DCF) down -v
	$(DCF) up -d --build

sync:
	@docker rm -f $(LEGACY_ORACLE_CONTAINER) >/dev/null 2>&1 || true
	$(DCF) down --remove-orphans
	@for attempt in 1 2 3 4 5 6; do \
		containers="$$(docker ps -aq --filter label=com.docker.compose.project=$(COMPOSE_PROJECT))"; \
		if [ -n "$$containers" ]; then \
			docker rm -f $$containers >/dev/null 2>&1 || true; \
		fi; \
		sleep 1; \
	done
	@docker volume rm $(COMPOSE_PROJECT)_app-workspace $(COMPOSE_PROJECT)_frontend-workspace $(COMPOSE_PROJECT)_frontend-deploy >/dev/null 2>&1 || true
	@for volume in $(COMPOSE_PROJECT)_app-workspace $(COMPOSE_PROJECT)_frontend-workspace $(COMPOSE_PROJECT)_frontend-deploy; do \
		if docker volume inspect $$volume >/dev/null 2>&1; then \
			echo "Failed to remove Docker volume $$volume; stop/remove containers that still use it."; \
			exit 1; \
		fi; \
	done
	$(DCF) up -d --build mysql redis mailpit app

doctor:
	@$(DCF) config >/dev/null
	@! $(DCF) config | grep -q '/app/public' || (echo "Unexpected /app/public mount; local frontend must not read packaged public assets." && exit 1)
	@$(MAKE) --no-print-directory public-bundle-audit
	@$(MAKE) --no-print-directory parity-config-audit
	@artifacts="$$(find . -maxdepth 4 \( -type d \( -name node_modules -o -name vendor -o -name .pnpm-store -o -name dist -o -name dist-deploy -o -name .vite -o -name .cache -o -name coverage \) \) -print)"; \
	if [ -n "$$artifacts" ]; then \
		echo "Host-generated artifacts found:"; \
		echo "$$artifacts"; \
		echo ""; \
		echo "Inspect ignored cleanup with 'make clean-host' or remove it with 'make clean-host-apply'."; \
		exit 1; \
	fi; \
	echo "Compose config OK: $(COMPOSE_FILE)"; \
	echo "No host dependency/cache/build directories found."; \
	echo "Run 'make replica-audit' to list remaining packaged frontend bundle dependencies."; \
	echo "Run 'make legacy-oracle-check' to verify the frozen packaged frontend oracle."; \
	echo "Run 'make deploy-smoke' to test a Docker-only Laravel public deployment."

public-bundle-audit:
	@artifacts="$$(find public/theme/default/assets public/assets/admin -mindepth 1 -print 2>/dev/null)"; \
	if [ -n "$$artifacts" ]; then \
		echo "Generated or packaged frontend files found under Laravel public targets:"; \
		echo "$$artifacts"; \
		echo ""; \
		echo "Do not keep old packaged bundles or source-built deploy output in the host public tree."; \
		echo "Use make deploy-smoke to build into Docker volumes, or deploy dist-deploy/ with delete-sync semantics outside this working tree."; \
		exit 1; \
	fi; \
	echo "Public bundle audit OK: host Laravel public frontend targets are empty."

replica-audit:
	@echo "Auditing runtime/build references to packaged legacy frontend bundles..."
	@matches="$$(rg -n '/theme/default/assets/(umi|components\.chunk)\.css|/theme/default/assets/(umi\.js|(vendors|components)\.async\.js|env\.example\.js)|/theme/default/assets/(i18n|images|static|theme)/|/assets/admin/components\.chunk\.css|/assets/admin/((vendors|components)\.async\.js|env\.example\.js)|/assets/admin/theme/|\.\./\.\./\.\./public/theme(?:/default/assets)?|\.\./\.\./\.\./public/assets/admin|legacyThemeRoot|copyLegacy|themeRuntimeAssetsPlugin|legacyThemePlugin|legacyAdminAssetsPlugin|copyLegacyAdminAssets' frontend/apps frontend/packages frontend/scripts resources/views public/theme/default/dashboard.blade.php public/theme/default/config.json docker-compose.local.yml --glob '!**/*.test.*' --glob '!frontend/scripts/visual-parity.mjs' || true)"; \
	if [ -n "$$matches" ]; then \
		echo "$$matches"; \
		echo ""; \
		echo "Replica audit failed: packaged legacy runtime/build dependencies remain."; \
		exit 1; \
	fi; \
	echo "Replica audit OK: no packaged legacy runtime/build dependencies found."

parity-config-audit:
	@docker image inspect v2board-frontend >/dev/null 2>&1 || $(DCF) build frontend
	@$(DCF) run --rm -T --no-deps --entrypoint sh -v "$(CURDIR):/src:ro" frontend -lc 'node /src/frontend/scripts/parity-config-audit.mjs'

legacy-oracle-check:
	@if [ -z "$(LEGACY_ORACLE_REF)" ]; then \
		echo "Legacy oracle ref missing: create $(LEGACY_ORACLE_REF_FILE) with a commit that contains the packaged public frontend."; \
		exit 1; \
	fi
	@git cat-file -e $(LEGACY_ORACLE_REF)^{commit} || (echo "Legacy oracle ref $(LEGACY_ORACLE_REF) is not a commit" && exit 1)
	@for path in $(LEGACY_ORACLE_REQUIRED_PATHS); do \
		git cat-file -e "$(LEGACY_ORACLE_REF):$$path" || { \
			echo "Legacy oracle ref $(LEGACY_ORACLE_REF) is missing $$path"; \
			exit 1; \
		}; \
	done
	@echo "Legacy oracle OK: $(LEGACY_ORACLE_REF) ($(words $(LEGACY_ORACLE_REQUIRED_PATHS)) required paths)"

legacy-oracle-up: legacy-oracle-check
	@$(MAKE) --no-print-directory legacy-oracle-down
	@if [ "$(VISUAL_PARITY_SKIP_DEPLOY)" = "1" ]; then \
		echo "Skipping deploy-smoke; reusing the current Docker app public assets for oracle settings."; \
		$(DCF) up -d app; \
	else \
		$(MAKE) --no-print-directory deploy-smoke; \
	fi
	@docker image inspect v2board-frontend >/dev/null 2>&1 || $(DCF) build frontend
	@docker volume create $(LEGACY_ORACLE_VOLUME) >/dev/null
	@git archive $(LEGACY_ORACLE_REF) public/theme/default public/assets/admin resources/views/admin.blade.php | docker run --rm -i -v $(LEGACY_ORACLE_VOLUME):/oracle v2board-app sh -lc 'cat > /oracle/oracle.tar'
	@admin_path="$$( $(DCF) exec -T app sh -lc 'php artisan tinker --execute='\''echo config("v2board.secure_path", config("v2board.frontend_admin_path", hash("crc32b", config("app.key"))));'\'' 2>/dev/null || true' )"; \
	[ -n "$$admin_path" ] || admin_path=admin; \
	docker run -d --name $(LEGACY_ORACLE_CONTAINER) \
		--add-host host.docker.internal:host-gateway \
		-p $(LEGACY_ORACLE_PORT):$(LEGACY_ORACLE_PORT) \
		-e COREPACK_ENABLE_DOWNLOAD_PROMPT=0 \
		-e PLAYWRIGHT_BROWSERS_PATH=/app/frontend/.cache/ms-playwright \
		-e NODE_OPTIONS=$(LEGACY_ORACLE_NODE_OPTIONS) \
		-e VISUAL_PARITY_ADMIN_PATH=$$admin_path \
		-e VISUAL_PARITY_SERVE_ONLY=1 \
		-e VISUAL_PARITY_ORACLE_HOST=0.0.0.0 \
		-e VISUAL_PARITY_PUBLIC_ORACLE_HOST=localhost \
		-e VISUAL_PARITY_ORACLE_PORT=$(LEGACY_ORACLE_PORT) \
		-e VISUAL_PARITY_SOURCE_BASE_URL=$(VISUAL_SOURCE_BASE_URL) \
		-e VISUAL_PARITY_ORACLE_ROOT=/tmp/v2board-legacy-oracle \
		-v "$(CURDIR)/frontend:/src/frontend:ro" \
		-v $(LEGACY_ORACLE_VOLUME):/oracle:ro \
		-v $(COMPOSE_PROJECT)_frontend-workspace:/app/frontend \
		-v $(COMPOSE_PROJECT)_frontend-deploy:/app/frontend/dist-deploy \
		-v $(COMPOSE_PROJECT)_frontend-visual-artifacts:/app/frontend/.cache/visual-parity \
		-v $(COMPOSE_PROJECT)_frontend-interaction-artifacts:/app/frontend/.cache/interaction-parity \
		-v $(COMPOSE_PROJECT)_frontend-node_modules:/app/frontend/node_modules \
		-v $(COMPOSE_PROJECT)_frontend-pnpm-store:/app/frontend/.pnpm-store \
		-v $(COMPOSE_PROJECT)_frontend-playwright-cache:/app/frontend/.cache/ms-playwright \
		-v $(COMPOSE_PROJECT)_frontend-admin-node_modules:/app/frontend/apps/admin/node_modules \
		-v $(COMPOSE_PROJECT)_frontend-user-node_modules:/app/frontend/apps/user/node_modules \
		-v $(COMPOSE_PROJECT)_frontend-api-client-node_modules:/app/frontend/packages/api-client/node_modules \
		-v $(COMPOSE_PROJECT)_frontend-config-node_modules:/app/frontend/packages/config/node_modules \
		-v $(COMPOSE_PROJECT)_frontend-i18n-node_modules:/app/frontend/packages/i18n/node_modules \
		-v $(COMPOSE_PROJECT)_frontend-types-node_modules:/app/frontend/packages/types/node_modules \
		v2board-frontend sh -lc 'rm -rf /tmp/v2board-legacy-oracle && mkdir -p /tmp/v2board-legacy-oracle && tar -C /tmp/v2board-legacy-oracle -xf /oracle/oracle.tar && $(FRONTEND_FAST_BOOTSTRAP) && VISUAL_PARITY_SERVE_ONLY=1 VISUAL_PARITY_ORACLE_HOST=0.0.0.0 VISUAL_PARITY_PUBLIC_ORACLE_HOST=localhost VISUAL_PARITY_ORACLE_PORT=$(LEGACY_ORACLE_PORT) VISUAL_PARITY_SOURCE_BASE_URL=$(VISUAL_SOURCE_BASE_URL) VISUAL_PARITY_ORACLE_ROOT=/tmp/v2board-legacy-oracle node scripts/visual-parity.mjs'; \
	echo "Legacy oracle user: http://localhost:$(LEGACY_ORACLE_PORT)/"; \
	echo "Legacy oracle admin: http://localhost:$(LEGACY_ORACLE_PORT)/$$admin_path#/login"

legacy-oracle-serve: legacy-oracle-check
	@$(MAKE) --no-print-directory legacy-oracle-down
	@if [ "$(VISUAL_PARITY_SKIP_DEPLOY)" = "1" ]; then \
		echo "Skipping deploy-smoke; reusing the current Docker app public assets for oracle settings."; \
		$(DCF) up -d app; \
	else \
		$(MAKE) --no-print-directory deploy-smoke; \
	fi
	$(MAKE) --no-print-directory clean-frontend-runs
	@admin_path="$$( $(DCF) exec -T app sh -lc 'php artisan tinker --execute='\''echo config("v2board.secure_path", config("v2board.frontend_admin_path", hash("crc32b", config("app.key"))));'\'' 2>/dev/null || true' )"; \
	[ -n "$$admin_path" ] || admin_path=admin; \
	$(DCF) stop $(LEGACY_ORACLE_PAUSE_SERVICES) >/dev/null 2>&1 || true; \
	trap '$(DCF) up -d $(LEGACY_ORACLE_RESUME_SERVICES) >/dev/null 2>&1 || true' EXIT; \
	git archive $(LEGACY_ORACLE_REF) public/theme/default public/assets/admin resources/views/admin.blade.php | $(FRONTEND_SERVE_RUN) -e NODE_OPTIONS=$(LEGACY_ORACLE_NODE_OPTIONS) -e VISUAL_PARITY_ADMIN_PATH=$$admin_path frontend -lc 'rm -rf /tmp/v2board-legacy-oracle && mkdir -p /tmp/v2board-legacy-oracle && tar -C /tmp/v2board-legacy-oracle -xf - && $(FRONTEND_FAST_BOOTSTRAP) && VISUAL_PARITY_SERVE_ONLY=1 VISUAL_PARITY_ORACLE_HOST=0.0.0.0 VISUAL_PARITY_PUBLIC_ORACLE_HOST=localhost VISUAL_PARITY_ORACLE_PORT=$(LEGACY_ORACLE_PORT) VISUAL_PARITY_SOURCE_BASE_URL=$(VISUAL_SOURCE_BASE_URL) VISUAL_PARITY_ORACLE_ROOT=/tmp/v2board-legacy-oracle node scripts/visual-parity.mjs'

legacy-oracle-down:
	@docker rm -f $(LEGACY_ORACLE_CONTAINER) >/dev/null 2>&1 || true

clean-frontend-runs:
	@containers="$$(docker ps -aq --filter label=com.docker.compose.project=$(COMPOSE_PROJECT) --filter label=com.docker.compose.service=frontend --filter label=com.docker.compose.oneoff=True)"; \
	if [ -n "$$containers" ]; then \
		docker rm -f $$containers >/dev/null 2>&1 || true; \
	fi

deploy-smoke:
	@$(DCF) stop $(DEPLOY_BUILD_PAUSE_SERVICES) >/dev/null 2>&1 || true
	$(MAKE) --no-print-directory clean-frontend-runs
	@status=0; \
	$(FRONTEND_RUN) -e NODE_OPTIONS=$(DEPLOY_NODE_OPTIONS) frontend -lc '$(FRONTEND_BOOTSTRAP) && mkdir -p /app/frontend/dist-deploy && find /app/frontend/dist-deploy -mindepth 1 -maxdepth 1 -exec rm -rf {} + && V2BOARD_DEPLOY_OUT_DIR=/app/frontend/dist-deploy/theme/default/assets pnpm -F @v2board/user exec vite build --config vite.config.deploy.ts' || status=$$?; \
	if [ "$$status" -eq 0 ]; then \
		$(FRONTEND_RUN) -e NODE_OPTIONS=$(DEPLOY_NODE_OPTIONS) frontend -lc '$(FRONTEND_FAST_BOOTSTRAP) && V2BOARD_DEPLOY_OUT_DIR=/app/frontend/dist-deploy/assets/admin pnpm -F @v2board/admin exec vite build --config vite.config.deploy.ts' || status=$$?; \
	fi; \
	if [ "$$status" -eq 0 ]; then \
		$(FRONTEND_RUN) -e NODE_OPTIONS=$(DEPLOY_NODE_OPTIONS) frontend -lc '$(FRONTEND_FAST_BOOTSTRAP) && V2BOARD_DEPLOY_MODE=finalize node scripts/build-deploy.mjs' || status=$$?; \
	fi; \
	if [ "$$status" -ne 0 ]; then \
		$(DCF) up -d $(DEPLOY_RESUME_SERVICES) >/dev/null 2>&1 || true; \
	fi; \
	exit $$status
	$(MAKE) --no-print-directory deploy-public-sync
	@$(DCF) exec -T app sh -lc 'set -eu; \
		for attempt in 1 2 3 4 5 6 7 8 9 10; do \
			code=$$(curl -sS -o /tmp/deploy-smoke-ready.out -w "%{http_code}" http://127.0.0.1:8000/ 2>/dev/null || true); \
			[ "$$code" = "200" ] && break; \
			sleep 1; \
		done; \
		admin_path=$$(php artisan tinker --execute='\''echo config("v2board.secure_path", config("v2board.frontend_admin_path", hash("crc32b", config("app.key"))));'\'' 2>/dev/null || true); \
		[ -n "$$admin_path" ] || admin_path=admin; \
		contains() { \
			file="$$1"; \
			pattern="$$2"; \
			if ! grep -Fq "$$pattern" "$$file"; then \
				echo "Deploy smoke failed: expected $$file to contain $$pattern"; \
				exit 1; \
			fi; \
		}; \
		absent() { \
			file="$$1"; \
			pattern="$$2"; \
			if grep -Fq "$$pattern" "$$file"; then \
				echo "Deploy smoke failed: expected $$file to omit $$pattern"; \
				exit 1; \
			fi; \
		}; \
		check() { \
			expected="$$1"; \
			url="$$2"; \
			code=$$(curl -sS -o /tmp/deploy-smoke.out -w "%{http_code}" "http://127.0.0.1:8000$$url"); \
			if [ "$$code" != "$$expected" ]; then \
				echo "Deploy smoke failed: expected $$expected got $$code for $$url"; \
				exit 1; \
			fi; \
		}; \
		fetch_contains() { \
			file="$$1"; \
			url="$$2"; \
			pattern="$$3"; \
			for attempt in 1 2 3 4 5; do \
				code=$$(curl -sS -o "$$file" -w "%{http_code}" "http://127.0.0.1:8000$$url" 2>/dev/null || true); \
				if [ "$$code" = "200" ] && grep -Fq "$$pattern" "$$file"; then \
					return 0; \
				fi; \
				sleep 1; \
			done; \
			echo "Deploy smoke failed: expected $$file from $$url to contain $$pattern"; \
			exit 1; \
		}; \
		check 200 /; \
		curl -sS http://127.0.0.1:8000/ > /tmp/deploy-smoke-user.html; \
		contains /tmp/deploy-smoke-user.html "/theme/default/assets/umi.css"; \
		contains /tmp/deploy-smoke-user.html "/theme/default/assets/umi.js"; \
		absent /tmp/deploy-smoke-user.html "components.chunk.css"; \
		absent /tmp/deploy-smoke-user.html "vendors.async.js"; \
		absent /tmp/deploy-smoke-user.html "components.async.js"; \
		absent /tmp/deploy-smoke-user.html "/assets/i18n/"; \
		absent /tmp/deploy-smoke-user.html "assets_path"; \
		check 200 /theme/default/assets/umi.css; \
		check 200 /theme/default/assets/umi.js; \
		check 404 /theme/default/assets/components.chunk.css; \
		check 404 /theme/default/assets/vendors.async.js; \
		check 404 /theme/default/assets/components.async.js; \
		check 404 /theme/default/assets/env.example.js; \
		check 404 /theme/default/assets/custom.css; \
		check 404 /theme/default/assets/custom.js; \
		check 404 /theme/default/assets/i18n/zh-CN.js; \
		check 200 /$$admin_path; \
		curl -sS http://127.0.0.1:8000/$$admin_path > /tmp/deploy-smoke-admin.html; \
		contains /tmp/deploy-smoke-admin.html "/assets/admin/umi.css"; \
		contains /tmp/deploy-smoke-admin.html "/assets/admin/umi.js"; \
		absent /tmp/deploy-smoke-admin.html "components.chunk.css"; \
		absent /tmp/deploy-smoke-admin.html "vendors.async.js"; \
		absent /tmp/deploy-smoke-admin.html "components.async.js"; \
		absent /tmp/deploy-smoke-admin.html "/assets/admin/theme/"; \
		check 200 /assets/admin/umi.css; \
		check 200 /assets/admin/umi.js; \
		fetch_contains /tmp/deploy-smoke-admin.js /assets/admin/umi.js "/assets/admin/themes"; \
		absent /tmp/deploy-smoke-admin.js "/assets/admin/theme/"; \
		check 200 /assets/admin/themes/default.css; \
		check 404 /assets/admin/components.chunk.css; \
		check 404 /assets/admin/vendors.async.js; \
		check 404 /assets/admin/components.async.js; \
		check 404 /assets/admin/env.example.js; \
		check 404 /assets/admin/custom.css; \
		check 404 /assets/admin/custom.js; \
		check 404 /assets/admin/theme/default.css; \
		echo "Deploy smoke OK: Laravel serves source-built user/admin assets and rejects old bundle paths."'
	@$(DCF) up -d $(DEPLOY_FINAL_RESUME_SERVICES) >/dev/null 2>&1 || true

deploy-public-sync:
	$(DCF) up -d mysql redis mailpit app
	@docker volume inspect $(COMPOSE_PROJECT)_frontend-deploy >/dev/null 2>&1 || (echo "Deploy artifact volume missing: run make deploy-smoke to rebuild source-built assets." && exit 1)
	docker run --rm \
		-v $(COMPOSE_PROJECT)_frontend-deploy:/deploy:ro \
		-v $(COMPOSE_PROJECT)_app-workspace:/app \
		v2board-app \
		sh -lc 'set -eu; for file in theme/default/assets/umi.css theme/default/assets/umi.js assets/admin/umi.css assets/admin/umi.js; do test -f "/deploy/$$file" || { echo "Deploy artifact missing: /deploy/$$file"; exit 1; }; done; rm -rf /tmp/v2board-public-sync; mkdir -p /tmp/v2board-public-sync/theme/default /tmp/v2board-public-sync/assets/admin; tar -C /deploy/theme/default -cf - . | tar -C /tmp/v2board-public-sync/theme/default -xf -; tar -C /deploy/assets/admin -cf - . | tar -C /tmp/v2board-public-sync/assets/admin -xf -; rm -rf /app/public/theme/default /app/public/assets/admin; mkdir -p /app/public/theme /app/public/assets; mv /tmp/v2board-public-sync/theme/default /app/public/theme/default; mv /tmp/v2board-public-sync/assets/admin /app/public/assets/admin'
	$(MAKE) --no-print-directory clean-frontend-runs

deploy-public-check:
	@docker volume inspect $(COMPOSE_PROJECT)_frontend-deploy >/dev/null 2>&1 || (echo "Deploy artifact volume missing: run make deploy-smoke to rebuild source-built assets." && exit 1)
	@docker run --rm \
		-v $(COMPOSE_PROJECT)_frontend-deploy:/deploy:ro \
		v2board-app \
		sh -lc 'set -eu; for file in theme/default/assets/umi.css theme/default/assets/umi.js assets/admin/umi.css assets/admin/umi.js; do test -f "/deploy/$$file" || { echo "Deploy artifact missing: /deploy/$$file"; exit 1; }; done'
	$(DCF) up -d app
	@status=1; \
	for attempt in 1 2 3 4 5 6 7 8; do \
		$(DCF) up -d app >/dev/null; \
		if $(DCF) exec -T app sh -lc 'set -eu; \
			wait_ready() { \
				url="$$1"; \
				for attempt in 1 2 3 4 5 6 7 8 9 10; do \
				code=$$(curl -sS -o /tmp/deploy-public-check.out -w "%{http_code}" "http://127.0.0.1:8000$$url" 2>/dev/null || true); \
				[ "$$code" = "200" ] && return 0; \
				sleep 1; \
			done; \
			echo "Docker app public asset check failed: $$url did not become ready on 8000. Run make deploy-smoke before VISUAL_PARITY_SKIP_DEPLOY=1."; \
			exit 1; \
		}; \
			check() { \
				url="$$1"; \
				for attempt in 1 2 3 4 5 6 7 8 9 10; do \
					code=$$(curl -sS -o /tmp/deploy-public-check.out -w "%{http_code}" "http://127.0.0.1:8000$$url" 2>/dev/null || true); \
					[ "$$code" = "200" ] && return 0; \
					sleep 1; \
				done; \
			echo "Docker app public asset check failed: $$url returned $$code. Run make deploy-smoke before VISUAL_PARITY_SKIP_DEPLOY=1."; \
			exit 1; \
		}; \
		wait_ready /; \
		check /theme/default/assets/umi.css; \
			check /theme/default/assets/umi.js; \
			check /assets/admin/umi.css; \
			check /assets/admin/umi.js; \
			echo "Deploy public check OK: source-built user/admin assets are available on 8000."'; then \
			status=0; \
			break; \
		fi; \
		if [ "$$attempt" -eq 1 ]; then \
			echo "Deploy public check: source-built assets missing from app public; resyncing existing dist-deploy."; \
			$(MAKE) --no-print-directory deploy-public-sync || true; \
		fi; \
		sleep 1; \
	done; \
	exit $$status

deploy-public-ensure:
	@status=0; \
	if $(MAKE) --no-print-directory deploy-public-check; then \
		exit 0; \
	fi; \
	echo "Deploy public ensure: source-built assets unavailable; rebuilding deploy artifacts."; \
	attempt=1; \
	while [ "$$attempt" -le "$(DEPLOY_PUBLIC_ENSURE_RETRIES)" ]; do \
		$(MAKE) --no-print-directory clean-frontend-runs; \
		$(MAKE) --no-print-directory deploy-smoke; \
		status=$$?; \
		if [ "$$status" -eq 0 ]; then \
			exit 0; \
		fi; \
		echo "Deploy public ensure: deploy-smoke attempt $$attempt failed with $$status."; \
		if [ "$$attempt" -lt "$(DEPLOY_PUBLIC_ENSURE_RETRIES)" ]; then \
			sleep $(DEPLOY_PUBLIC_ENSURE_RETRY_DELAY); \
		fi; \
		attempt=$$((attempt + 1)); \
	done; \
	exit $$status

visual-smoke: deploy-smoke
	@admin_path="$$( $(DCF) exec -T app sh -lc 'php artisan tinker --execute='\''echo config("v2board.secure_path", config("v2board.frontend_admin_path", hash("crc32b", config("app.key"))));'\'' 2>/dev/null || true' )"; \
	[ -n "$$admin_path" ] || admin_path=admin; \
	$(FRONTEND_RUN) \
		-e PLAYWRIGHT_BROWSERS_PATH=/app/frontend/.cache/ms-playwright \
		-e VISUAL_SMOKE_BASE_URL=$(VISUAL_SOURCE_BASE_URL) \
		-e VISUAL_SMOKE_ADMIN_PATH=$$admin_path \
		-w /app/frontend frontend -lc '$(FRONTEND_FAST_BOOTSTRAP) && $(PLAYWRIGHT_CHROMIUM_BOOTSTRAP) && node scripts/visual-smoke.mjs'

visual-parity: legacy-oracle-check
	@if [ "$${VISUAL_PARITY_MODE:-screenshots}" = "screenshots" ] && [ "$${VISUAL_PARITY_SHARD:-}" != "1" ] && [ -z "$${VISUAL_PARITY_FILTER:-}" ] && [ -z "$${VISUAL_PARITY_VIEWPORT_FILTER:-}" ]; then \
		status=0; \
		if [ "$(VISUAL_PARITY_SKIP_DEPLOY)" != "1" ]; then \
			$(MAKE) --no-print-directory deploy-smoke || exit $$?; \
		else \
			$(MAKE) --no-print-directory deploy-public-ensure || exit $$?; \
		fi; \
		$(DCF) up -d app >/dev/null; \
		admin_path="$$( $(DCF) exec -T app sh -lc 'php artisan tinker --execute='\''echo config("v2board.secure_path", config("v2board.frontend_admin_path", hash("crc32b", config("app.key"))));'\'' 2>/dev/null || true' )"; \
		[ -n "$$admin_path" ] || admin_path=admin; \
		$(DCF) stop $(VISUAL_PARITY_PAUSE_SERVICES) >/dev/null 2>&1 || true; \
		needs_public_check=0; \
		for viewport in $(VISUAL_PARITY_VIEWPORTS); do \
			for scenario in $(VISUAL_PARITY_SCENARIOS); do \
				echo "Visual parity shard: $$scenario/$$viewport"; \
					attempt=0; \
					while :; do \
						attempt=$$((attempt + 1)); \
						status=0; \
						if [ "$(VISUAL_PARITY_CHECK_EACH_SHARD)" = "1" ] || [ "$$needs_public_check" = "1" ]; then \
							$(MAKE) --no-print-directory deploy-public-ensure || status=$$?; \
							needs_public_check=0; \
						fi; \
						$(DCF) stop $(VISUAL_PARITY_PAUSE_SERVICES) >/dev/null 2>&1 || true; \
						artifact_dir="$(VISUAL_PARITY_ARTIFACT_DIR)/$$scenario-$$viewport"; \
						$(MAKE) --no-print-directory clean-frontend-runs; \
					if [ "$$status" -eq 0 ] && git archive $(LEGACY_ORACLE_REF) public/theme/default public/assets/admin resources/views/admin.blade.php | $(FRONTEND_RUN) \
						-e NODE_OPTIONS=$(VISUAL_PARITY_NODE_OPTIONS) \
						-e PLAYWRIGHT_BROWSERS_PATH=/app/frontend/.cache/ms-playwright \
						-e VISUAL_PARITY_MODE=$${VISUAL_PARITY_MODE:-screenshots} \
						-e VISUAL_PARITY_SOURCE_BASE_URL=$(VISUAL_SOURCE_BASE_URL) \
						-e VISUAL_PARITY_ADMIN_PATH=$$admin_path \
						-e VISUAL_PARITY_ORACLE_ROOT=/tmp/v2board-legacy-oracle \
						-e VISUAL_PARITY_ARTIFACT_DIR=$$artifact_dir \
						-e VISUAL_PARITY_FILTER=$$scenario \
						-e VISUAL_PARITY_INTERACTION_FILTER="$(VISUAL_PARITY_INTERACTION_FILTER)" \
						-e VISUAL_PARITY_VIEWPORT_FILTER=$$viewport \
						-e VISUAL_PARITY_FRESH_BROWSER=$(VISUAL_PARITY_FRESH_BROWSER) \
						-w /app/frontend frontend -lc 'rm -rf /tmp/v2board-legacy-oracle "$$VISUAL_PARITY_ARTIFACT_DIR" && mkdir -p /tmp/v2board-legacy-oracle "$$VISUAL_PARITY_ARTIFACT_DIR" && tar -C /tmp/v2board-legacy-oracle -xf - && $(FRONTEND_FAST_BOOTSTRAP) && $(PLAYWRIGHT_CHROMIUM_BOOTSTRAP) && node scripts/visual-parity.mjs'; then \
						status=0; \
					else \
						status=$$?; \
					fi; \
					$(MAKE) --no-print-directory clean-frontend-runs; \
					if [ "$$status" -eq 0 ]; then \
						break; \
					fi; \
					if [ "$$status" != "137" ] && [ "$$attempt" -eq 1 ]; then \
						echo "Visual parity shard failed; rechecking Docker public assets before one retry of $$scenario/$$viewport."; \
						needs_public_check=1; \
						sleep 1; \
						continue; \
					fi; \
					if [ "$$status" != "137" ] || [ "$$attempt" -gt "$(VISUAL_PARITY_RETRIES)" ]; then \
						break; \
					fi; \
					echo "Visual parity shard hit 137; retrying $$scenario/$$viewport after $(VISUAL_PARITY_RETRY_DELAY)s."; \
					needs_public_check=1; \
					sleep $(VISUAL_PARITY_RETRY_DELAY); \
				done; \
				if [ "$$status" -ne 0 ]; then \
					$(DCF) up -d $(VISUAL_PARITY_RESUME_SERVICES) >/dev/null 2>&1 || true; \
					exit $$status; \
				fi; \
				if [ "$(VISUAL_PARITY_SHARD_DELAY)" != "0" ]; then \
					sleep $(VISUAL_PARITY_SHARD_DELAY); \
				fi; \
			done; \
		done; \
		echo "Visual parity OK: all configured shards passed."; \
		echo "Artifacts: $(VISUAL_PARITY_ARTIFACT_DIR)"; \
		$(DCF) up -d $(VISUAL_PARITY_RESUME_SERVICES) >/dev/null 2>&1 || true; \
		exit 0; \
	fi
	@if [ "$${VISUAL_PARITY_MODE:-screenshots}" = "interactions" ] && [ "$${VISUAL_PARITY_SHARD:-}" != "1" ] && [ -z "$${VISUAL_PARITY_VIEWPORT_FILTER:-}" ]; then \
		if [ -z "$${VISUAL_PARITY_INTERACTION_FILTER:-}" ] && [ -z "$${VISUAL_PARITY_FILTER:-}" ]; then \
			$(MAKE) --no-print-directory interaction-parity || exit $$?; \
			exit 0; \
		fi; \
		if [ "$(VISUAL_PARITY_SKIP_DEPLOY)" = "1" ]; then \
			$(MAKE) --no-print-directory deploy-public-ensure || exit $$?; \
		else \
			$(MAKE) --no-print-directory deploy-smoke || exit $$?; \
		fi; \
		$(DCF) up -d app >/dev/null; \
		status=0; \
		for viewport in $(VISUAL_PARITY_VIEWPORTS); do \
			echo "Interaction parity viewport shard: $$viewport"; \
			artifact_dir="$(VISUAL_PARITY_ARTIFACT_DIR)/$$viewport"; \
			VISUAL_PARITY_MODE=interactions \
			VISUAL_PARITY_SHARD=1 \
			VISUAL_PARITY_SKIP_DEPLOY=1 \
			VISUAL_PARITY_PUBLIC_CHECKED=1 \
			VISUAL_PARITY_APP_READY=1 \
			VISUAL_PARITY_RESTART_SERVICES=0 \
			VISUAL_PARITY_ARTIFACT_DIR="$$artifact_dir" \
			VISUAL_PARITY_VIEWPORT_FILTER="$$viewport" \
			$(MAKE) --no-print-directory visual-parity || status=$$?; \
			if [ "$$status" -ne 0 ]; then \
				$(DCF) up -d $(VISUAL_PARITY_RESUME_SERVICES) >/dev/null 2>&1 || true; \
				exit $$status; \
			fi; \
		done; \
		if [ "$(VISUAL_PARITY_RESTART_SERVICES)" != "0" ]; then \
			$(DCF) up -d $(VISUAL_PARITY_RESUME_SERVICES) >/dev/null 2>&1 || true; \
		fi; \
		echo "Interaction parity OK: filtered viewport shards passed."; \
		echo "Artifacts: $(VISUAL_PARITY_ARTIFACT_DIR)/<viewport>"; \
		exit 0; \
	fi
	@if [ "$${VISUAL_PARITY_MODE:-screenshots}" = "screenshots" ] && [ "$${VISUAL_PARITY_SHARD:-}" != "1" ] && [ -z "$${VISUAL_PARITY_FILTER:-}" ] && [ -z "$${VISUAL_PARITY_VIEWPORT_FILTER:-}" ]; then \
		exit 0; \
	fi; \
	if [ "$${VISUAL_PARITY_MODE:-screenshots}" = "interactions" ] && [ "$${VISUAL_PARITY_SHARD:-}" != "1" ] && [ -z "$${VISUAL_PARITY_VIEWPORT_FILTER:-}" ]; then \
		exit 0; \
	fi; \
	status=0; \
	$(MAKE) --no-print-directory clean-frontend-runs; \
	if [ "$(VISUAL_PARITY_SKIP_DEPLOY)" = "1" ]; then \
		echo "Skipping deploy-smoke; reusing the current Docker app public assets."; \
		if [ "$(VISUAL_PARITY_PUBLIC_CHECKED)" != "1" ]; then \
			$(MAKE) --no-print-directory deploy-public-ensure || status=$$?; \
		elif [ "$(VISUAL_PARITY_APP_READY)" != "1" ]; then \
			$(DCF) up -d app || status=$$?; \
		fi; \
	else \
		$(MAKE) --no-print-directory deploy-smoke || status=$$?; \
	fi; \
	if [ "$$status" -eq 0 ]; then \
		admin_path="$${VISUAL_PARITY_ADMIN_PATH:-}"; \
		if [ -z "$$admin_path" ]; then \
			admin_path="$$( $(DCF) exec -T app sh -lc 'php artisan tinker --execute='\''echo config("v2board.secure_path", config("v2board.frontend_admin_path", hash("crc32b", config("app.key"))));'\'' 2>/dev/null || true' )"; \
		fi; \
		[ -n "$$admin_path" ] || admin_path=admin; \
		artifact_dir="$(VISUAL_PARITY_ARTIFACT_DIR)"; \
		if [ "$$artifact_dir" = "/app/frontend/.cache/visual-parity" ]; then \
			artifact_name="$${VISUAL_PARITY_FILTER:-$(VISUAL_PARITY_INTERACTION_FILTER)}"; \
			[ -n "$$artifact_name" ] || artifact_name=filtered; \
			artifact_viewport="$${VISUAL_PARITY_VIEWPORT_FILTER:-all}"; \
			artifact_dir="$$artifact_dir/$$artifact_name-$$artifact_viewport"; \
		fi; \
		$(DCF) stop $(VISUAL_PARITY_PAUSE_SERVICES) >/dev/null 2>&1 || true; \
		if [ "$(VISUAL_PARITY_SHARD_DELAY)" != "0" ]; then \
			sleep $(VISUAL_PARITY_SHARD_DELAY); \
		fi; \
			attempt=0; \
			needs_public_check=0; \
				while :; do \
					attempt=$$((attempt + 1)); \
					status=0; \
					if [ "$$needs_public_check" = "1" ]; then \
						$(MAKE) --no-print-directory deploy-public-ensure || status=$$?; \
						needs_public_check=0; \
					fi; \
					$(DCF) stop $(VISUAL_PARITY_PAUSE_SERVICES) >/dev/null 2>&1 || true; \
					$(MAKE) --no-print-directory clean-frontend-runs; \
				if [ "$$status" -eq 0 ] && git archive $(LEGACY_ORACLE_REF) public/theme/default public/assets/admin resources/views/admin.blade.php | $(FRONTEND_RUN) \
					-e NODE_OPTIONS=$(VISUAL_PARITY_NODE_OPTIONS) \
					-e PLAYWRIGHT_BROWSERS_PATH=/app/frontend/.cache/ms-playwright \
					-e VISUAL_PARITY_MODE=$${VISUAL_PARITY_MODE:-screenshots} \
					-e VISUAL_PARITY_SOURCE_BASE_URL=$(VISUAL_SOURCE_BASE_URL) \
					-e VISUAL_PARITY_ADMIN_PATH=$$admin_path \
					-e VISUAL_PARITY_ORACLE_ROOT=/tmp/v2board-legacy-oracle \
					-e VISUAL_PARITY_ARTIFACT_DIR=$$artifact_dir \
					-e VISUAL_PARITY_FILTER="$${VISUAL_PARITY_FILTER:-}" \
					-e VISUAL_PARITY_INTERACTION_FILTER="$(VISUAL_PARITY_INTERACTION_FILTER)" \
					-e VISUAL_PARITY_VIEWPORT_FILTER="$${VISUAL_PARITY_VIEWPORT_FILTER:-}" \
					-e VISUAL_PARITY_FRESH_BROWSER=$(VISUAL_PARITY_FRESH_BROWSER) \
					-w /app/frontend frontend -lc 'rm -rf /tmp/v2board-legacy-oracle "$$VISUAL_PARITY_ARTIFACT_DIR" && mkdir -p /tmp/v2board-legacy-oracle "$$VISUAL_PARITY_ARTIFACT_DIR" && tar -C /tmp/v2board-legacy-oracle -xf - && $(FRONTEND_FAST_BOOTSTRAP) && $(PLAYWRIGHT_CHROMIUM_BOOTSTRAP) && node scripts/visual-parity.mjs'; then \
					status=0; \
				else \
					status=$$?; \
				fi; \
				$(MAKE) --no-print-directory clean-frontend-runs; \
				if [ "$$status" -eq 0 ]; then \
					break; \
				fi; \
				if [ "$$status" != "137" ] && [ "$$attempt" -eq 1 ]; then \
					echo "Visual parity filtered run failed; rechecking Docker public assets before one retry."; \
					needs_public_check=1; \
					sleep 1; \
					continue; \
				fi; \
				if [ "$$status" != "137" ] || [ "$$attempt" -gt "$(VISUAL_PARITY_RETRIES)" ]; then \
					break; \
				fi; \
				echo "Visual parity hit 137; retrying filtered run after $(VISUAL_PARITY_RETRY_DELAY)s."; \
				needs_public_check=1; \
				sleep $(VISUAL_PARITY_RETRY_DELAY); \
				$(DCF) stop $(VISUAL_PARITY_PAUSE_SERVICES) >/dev/null 2>&1 || true; \
			done; \
		fi; \
		if [ "$(VISUAL_PARITY_RESTART_SERVICES)" != "0" ]; then \
		$(DCF) up -d $(VISUAL_PARITY_RESUME_SERVICES) >/dev/null 2>&1 || true; \
	fi; \
		exit $$status

interaction-parity:
	@case "$(INTERACTION_PARITY_ARTIFACT_DIR)" in \
		/app/frontend/*) ;; \
		*) echo "INTERACTION_PARITY_ARTIFACT_DIR must be inside /app/frontend so shard artifacts persist across Docker one-off containers."; exit 1 ;; \
	esac
	@if [ "$(VISUAL_PARITY_SKIP_DEPLOY)" = "1" ]; then \
		$(MAKE) --no-print-directory deploy-public-ensure || exit $$?; \
	else \
		$(MAKE) --no-print-directory deploy-smoke || exit $$?; \
	fi; \
	$(DCF) up -d app >/dev/null; \
	$(DCF) stop $(INTERACTION_PARITY_PAUSE_SERVICES) >/dev/null 2>&1 || true; \
	if [ "$(INTERACTION_PARITY_SHARD_DELAY)" != "0" ]; then \
		sleep $(INTERACTION_PARITY_SHARD_DELAY); \
	fi; \
	expected_artifacts=""; \
	for viewport in $(VISUAL_PARITY_VIEWPORTS); do \
		for interaction in $(INTERACTION_PARITY_SCENARIOS); do \
			echo "Interaction parity shard: $$interaction/$$viewport"; \
			artifact_dir="$(INTERACTION_PARITY_ARTIFACT_DIR)/$$interaction-$$viewport"; \
			expected_artifacts="$$expected_artifacts $$interaction-$$viewport"; \
			status=0; \
			VISUAL_PARITY_MODE=interactions \
			VISUAL_PARITY_SKIP_DEPLOY=1 \
			VISUAL_PARITY_PUBLIC_CHECKED=1 \
			VISUAL_PARITY_RESTART_SERVICES=0 \
			VISUAL_PARITY_RETRIES=$(INTERACTION_PARITY_RETRIES) \
			VISUAL_PARITY_PAUSE_SERVICES="$(INTERACTION_PARITY_PAUSE_SERVICES)" \
			VISUAL_PARITY_RESUME_SERVICES="$(INTERACTION_PARITY_RESUME_SERVICES)" \
			VISUAL_PARITY_ARTIFACT_DIR="$$artifact_dir" \
			VISUAL_PARITY_INTERACTION_FILTER="$$interaction" \
			VISUAL_PARITY_VIEWPORT_FILTER="$$viewport" \
			VISUAL_PARITY_SHARD_DELAY=0 \
			$(MAKE) --no-print-directory visual-parity || status=$$?; \
			if [ "$${status:-0}" -ne 0 ]; then \
				$(DCF) up -d $(INTERACTION_PARITY_RESUME_SERVICES) >/dev/null 2>&1 || true; \
				exit $$status; \
			fi; \
			if [ "$(INTERACTION_PARITY_SHARD_DELAY)" != "0" ]; then \
				sleep $(INTERACTION_PARITY_SHARD_DELAY); \
			fi; \
		done; \
	done; \
	$(MAKE) --no-print-directory clean-frontend-runs; \
	$(FRONTEND_RUN) -e INTERACTION_PARITY_ARTIFACT_DIR=$(INTERACTION_PARITY_ARTIFACT_DIR) -e EXPECTED_INTERACTION_ARTIFACTS="$$expected_artifacts" frontend -lc 'for artifact in $$EXPECTED_INTERACTION_ARTIFACTS; do test -s "$$INTERACTION_PARITY_ARTIFACT_DIR/$$artifact/report.json" || { echo "Missing interaction parity artifact: $$artifact/report.json"; exit 1; }; done'; \
	$(MAKE) --no-print-directory clean-frontend-runs; \
	$(DCF) up -d $(INTERACTION_PARITY_RESUME_SERVICES) >/dev/null 2>&1 || true; \
	echo "Interaction parity OK: all configured shards passed."; \
	echo "Artifacts: $(INTERACTION_PARITY_ARTIFACT_DIR)"

clean-host:
	git clean -fdX -n

clean-host-apply:
	git clean -fdX

mailpit-ui:
	@open http://localhost:8025 2>/dev/null || echo "open http://localhost:8025"

admin-url:
	@echo "http://localhost:8000/admin"
