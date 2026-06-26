#!/usr/bin/env bash
# Editor-only bootstrap: install the frontend workspace into the container's named
# volumes so the TypeScript server resolves `react`, `@v2board/*`, etc. Nothing
# here touches the host working tree — node_modules and the pnpm store live in
# Docker volumes (see docker-compose.yml).
set -euo pipefail

corepack enable
corepack prepare pnpm@11.0.0 --activate

cd /workspaces/v2board/frontend

# Store location is pinned via npm_config_store_dir (see docker-compose.yml) so the
# store lands in the /pnpm-store volume, never on the host tree.
HUSKY=0 pnpm install --frozen-lockfile

echo "Dev container ready: frontend workspace installed; host tree stays clean."
