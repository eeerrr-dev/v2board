# Dev Container (editor-only)

This Dev Container exists for **one reason**: give VS Code's TypeScript server (and
ESLint) the `node_modules` it needs to resolve `react`, `@v2board/*`, etc., so the
editor stops showing red "Cannot find module" errors — **without** putting any
`node_modules` on the host.

It is **not** a replacement for the project's Docker workflow. Installing,
building, testing, running the dev server, and deploying still go through `make`
(see `AGENTS.md`). This container only hosts the language server.

## Why it's needed

The `make` workflow deliberately keeps `node_modules`, `.pnpm-store`, `dist`, etc.
in Docker volumes, never on the host. That keeps the repo clean and builds
reproducible — but it also means a host-only VS Code has no types to resolve. This
Dev Container runs the TS server *inside* a container that has those types, so the
host tree stays empty while IntelliSense works fully.

## How to use

1. Install the **Dev Containers** VS Code extension.
2. Command Palette → **Dev Containers: Reopen in Container**.
3. First launch runs `post-create.sh` (`pnpm install --frozen-lockfile`) into the
   container's volumes — a one-time wait. Subsequent launches are fast.
4. The editor now resolves all imports. Keep using `make ...` (in a host terminal
   or this container's terminal) for builds/tests/dev-server/deploy.

## How it stays off the host

- The repo is bind-mounted read-write at `/workspaces/v2board`, so your edits
  persist to the host as normal.
- Named volumes shadow every `node_modules` path (root + `apps/{user,admin}` +
  `packages/{api-client,config,i18n,tokens,types}`) and the pnpm store, so none of
  them land in the host working tree.
- It runs as a **separate compose project**, so it never collides with
  `make sync` / `make up` (those only touch the `v2board` project's volumes).

## Notes

- Runs as `root` in-container so the volumes are writable; on macOS/Windows your
  host files stay owned by you (bind-mount UID mapping).
- To rebuild from scratch: **Dev Containers: Rebuild Container** (re-runs the
  install). To reclaim disk, remove this project's `dev-*` volumes in Docker.
