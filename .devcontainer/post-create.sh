#!/usr/bin/env bash
# Editor-only bootstrap: install the frontend workspace into the container's named
# volumes so the TypeScript server resolves `react`, `@v2board/*`, etc. Nothing
# here touches the host working tree — node_modules and the pnpm store live in
# Docker volumes (see docker-compose.yml).
set -euo pipefail

corepack enable
corepack prepare pnpm@11.11.0 --activate

cd /workspaces/v2board/frontend

# Keep this explicit, as in the main Docker workflow: the workspace-root path is
# shadowed by a named volume in docker-compose.yml, so the store never lands in
# the host tree.
pnpm config set store-dir /workspaces/v2board/.pnpm-store
pnpm install --frozen-lockfile

cd /workspaces/v2board/backend/rust
cargo fetch --locked

echo "Dev container ready: Rust and frontend dependencies are in named volumes."
