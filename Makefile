.PHONY: up down logs shell reset mailpit-ui admin-url

DC := $(shell \
	if docker compose version >/dev/null 2>&1; then echo "docker compose"; \
	elif [ -x /opt/homebrew/lib/docker/cli-plugins/docker-compose ]; then echo "/opt/homebrew/lib/docker/cli-plugins/docker-compose"; \
	elif command -v docker-compose >/dev/null 2>&1; then echo "docker-compose"; \
	else echo ""; fi)

ifeq ($(DC),)
$(error docker compose not found; run 'brew install docker-compose' or add cliPluginsExtraDirs to ~/.docker/config.json)
endif

up:
	$(DC) up -d --build
	@echo ""
	@echo "  user      http://localhost:5173            (new frontend, HMR)"
	@echo "  admin     http://localhost:5174            (new admin, HMR)"
	@echo "  api/legacy http://localhost:8000           (backend; admin@local / 12345678)"
	@echo "  mailpit   http://localhost:8025"
	@echo ""
	@echo "  note: first 'make up' runs pnpm install in the frontend container"
	@echo "        (a few minutes); tail it with 'make logs' or '$(DC) logs -f frontend'"

down:
	$(DC) down

logs:
	$(DC) logs -f --tail=100

shell:
	$(DC) exec app bash

reset:
	$(DC) down -v
	$(DC) up -d --build

mailpit-ui:
	@open http://localhost:8025 2>/dev/null || echo "open http://localhost:8025"

admin-url:
	@echo "http://localhost:8000/admin"
