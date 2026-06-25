# AGENTS.md

Project instructions for agents working in this repository. Keep changes
small, verified, and aligned with the gradual frontend reskin.

## Local Workflow

Use Docker for local setup, dependencies, builds, and tests. Do not run
Composer, pnpm, npm, build, or test commands on the host when they would create
`vendor/`, `node_modules/`, `.pnpm-store/`, `dist/`, `.vite/`, coverage, cache,
or deploy output inside the repository.

- Use `make up`, `make down`, `make reset`, `make sync`, and `make logs`.
- After source edits that need Docker execution, run `make sync` so the Docker
  app/frontend workspaces receive the host changes.
- Use `make doctor` for the general local sanity gate.
- Use `make public-bundle-audit` to confirm host `public/theme/default/assets/`
  and `public/assets/admin/` stay empty.
- Use `make parity-config-audit` after changing routes or visual/interaction
  parity scenario lists.
- Use `make deploy-smoke` after deploy-path or public asset changes.
- Use `make visual-smoke` after visual/layout work that needs browser review.
- Use `make behavior-parity` for the durable reskin behavior gate.
- Use `make interaction-parity` for focused browser behavior checks. Narrow the
  outer list with `INTERACTION_PARITY_SCENARIOS=... make interaction-parity`.
- Use `make visual-parity` only as a read-only oracle check for surfaces still
  on the replica. Focus it with `VISUAL_PARITY_FILTER=...` and
  `VISUAL_PARITY_VIEWPORT_FILTER=...` when debugging.
- Use `make legacy-oracle-check` before relying on the frozen packaged oracle.
- Use `make legacy-oracle-up` / `make legacy-oracle-down` only for a persistent
  manual oracle on `http://localhost:8001`.
- Use `make clean-host` to preview ignored host cleanup. Use
  `make clean-host-apply` only intentionally.

If a direct package/test command is needed, run it inside the appropriate Docker
service or one-off frontend container. Keep generated dependency, cache, build,
deploy, visual, and interaction artifacts in Docker volumes, not on the host.

## Source And Deploy Rules

- `docker-compose.local.yml` is the canonical tracked local workflow.
  `docker-compose.yml` is ignored and only for personal overrides.
- Do not mount the frontend container to `./public` or make the frontend read
  packaged public assets from `/app/public`.
- Source-owned frontend code lives under `frontend/apps/*/src`.
- Source-built deploy artifacts live in the Docker `frontend-deploy` volume at
  `/app/frontend/dist-deploy`; do not write `dist-deploy/` on the host.
- Visual and interaction reports live in Docker artifact volumes under
  `/app/frontend/.cache/`; do not write parity reports on the host.
- Deployed files may keep legacy-compatible names like `umi.css` and `umi.js`,
  but they must be freshly built from `frontend/apps/*/src`.
- Never import, concatenate, copy, serve, or deploy the old packaged bundles:
  `umi.js`, `umi.css`, `components.chunk.css`, `vendors.async.js`,
  `components.async.js`, `env.example.js`, copied static/i18n/theme assets, or
  admin equivalents.
- `custom.css` and `custom.js` are optional operator-provided hooks only. Build
  and deploy scripts must not generate or copy them.
- The legacy oracle is pinned by `frontend/fixtures/legacy-oracle.ref`. Do not
  remove or rewrite it unless replacing it with an equally complete packaged
  frontend oracle. Old frontend files may be inspected only as oracle/test
  fixtures and only restored into Docker `/tmp` or Docker volumes.

## Frontend Reskin Direction

The source-level replica milestone is complete and frozen as `replica-baseline`.
The project is now a gradual reskin: behavior stays strict, appearance changes
surface by surface.

- Behavioral/contract parity is permanent. API calls and payloads, auth and
  redirects, routing, persistence, i18n behavior, and edge cases must stay green
  through `make behavior-parity` or a focused interaction parity shard.
- Visual/pixel parity is retired only for redesigned surfaces. Mark redesigned
  visual scenarios with `visualRetired: true` in
  `frontend/scripts/visual-parity.mjs`, and keep a behavior/interaction scenario
  for the same route.
- For surfaces not yet redesigned, the old strict oracle standard still holds.
  Do not downgrade an un-redesigned mismatch to acceptable drift.
- Do not claim a redesigned surface is complete unless behavior parity is green,
  its visual scenarios are explicitly retired, and the new design has been
  reviewed.
- Do not claim a still-replica surface is complete unless the relevant
  deploy/visual/behavior checks match the oracle.

## Modern Frontend Stack

Continue building on the current verified project rather than starting a second
frontend. New redesigned surfaces should use:

- React + TypeScript + Vite.
- React Router for the existing routing/deploy shape.
- TanStack Query for server state.
- `@v2board/api-client` for API contracts.
- Existing i18n infrastructure.
- Local `components/ui` primitives.
- Radix primitives for accessible low-level behavior.
- shadcn/ui as a code blueprint only: copy/adapt patterns into local source,
  then make them fit this project. Do not blindly run generated defaults into
  the app.
- `lucide-react` for new icons.
- `@v2board/tokens` for shared color, radius, elevation, and theme values.
- Tailwind v4 with the mandatory `tw:` prefix.

The `tw:` prefix is non-negotiable. Vendored legacy CSS owns bare class names
like `block`, `container`, `badge`, `.btn`, and `.form-control`. Bare class
names belong to vendored/legacy components; `tw:` classes belong to authored
reskin code.

For new redesigned surfaces, do not use:

- Ant Design v3 components as new UI foundations.
- Bootstrap or OneUI classes such as `.btn`, `.block`, `.form-control`, or
  `.bg-gray-lighter`.
- Unprefixed Tailwind utilities.
- Page-local hardcoded color, radius, or shadow systems when tokens/primitives
  can express the design.
- Hidden runtime dependencies on packaged legacy bundles.
- Copying the old packaged bundle DOM, CSS, or file structure as the foundation
  for a redesigned surface.

Legacy Ant Design v3, Bootstrap 4, OneUI, and Font Awesome 5 CSS may remain for
surfaces still on the replica. Those vendored versions are final for replica
parity and must not be upgraded casually; upgrading them is a redesign decision.

## Implementation Discipline

- State assumptions and tradeoffs before substantial implementation.
- Ask when requirements are genuinely ambiguous and a wrong assumption would be
  costly.
- Keep changes surgical. Every changed line should trace to the request.
- Prefer existing local patterns, helpers, primitives, and tests.
- Do not add speculative features, abstractions, or configurability.
- Remove imports, variables, functions, and tests made obsolete by your own
  change. Do not clean unrelated code unless asked.
- Preserve user work in the working tree. Never revert changes you did not make
  unless explicitly requested.
- Use structured parsers/APIs when available instead of fragile string hacks.
- Add tests proportional to risk. For behavior bugs, reproduce the behavior and
  make the test pass.
- Before finishing, run the smallest meaningful verification. If you cannot run
  a relevant check, say so clearly.

## Verification Guide

- Pure docs/comment changes: usually `git diff --check` is enough.
- Frontend component changes: run focused Vitest inside Docker plus
  `pnpm --filter @v2board/user typecheck` or the relevant app typecheck.
- Route/scenario changes: run `make parity-config-audit`.
- Deploy path or public asset changes: run `make deploy-smoke`.
- Redesigned behavior changes: run focused `make interaction-parity` shards.
- Visual/layout changes: use focused `make visual-parity` or `make visual-smoke`
  as appropriate.
