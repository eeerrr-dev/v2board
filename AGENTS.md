# AGENTS.md

Project instructions for agents working in this repository. Keep changes
verified, intentional, and aligned with the current frontend migration.

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
The project is now a surface-by-surface frontend migration: behavior stays
strict, appearance may change decisively when a surface is explicitly
redesigned.

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
- A redesigned surface may become a pure shadcn island when the owner explicitly
  chooses that direction. In that case, prioritize shadcn/Radix composition and
  behavior tests over preserving legacy DOM, legacy class names, or old visual
  shape.

## Modern Frontend Stack

Continue building on the current verified project rather than starting a second
frontend. New redesigned surfaces should use:

- React + TypeScript + Vite.
- React Router for the existing routing/deploy shape.
- TanStack Query for server state.
- `@v2board/api-client` for API contracts.
- Existing i18n infrastructure.
- Radix primitives for accessible low-level behavior.
- shadcn/ui registry components are allowed for explicitly designated pure
  shadcn islands. Copy the generated source into the app, own it locally, and
  keep the island coherent instead of mixing half-legacy and half-shadcn UI.
- Local `components/ui` primitives remain preferred for gradual-reskin surfaces
  that are not pure shadcn islands.
- `lucide-react` for new icons.
- Tailwind v4.
- `@v2board/tokens` for gradual-reskin surfaces. Pure shadcn islands may use
  shadcn's canonical token names and utility classes when that produces a
  cleaner, more coherent implementation.

For gradual-reskin code, keep using the `tw:` prefix. Vendored legacy CSS owns
bare class names like `block`, `container`, `badge`, `.btn`, and
`.form-control`; prefixed utilities avoid accidental collisions.

For pure shadcn islands, unprefixed Tailwind utilities and shadcn token names are
allowed intentionally. Keep those islands route- or component-scoped, avoid
leaking their assumptions into replica surfaces, and verify behavior with tests.

### Auth Surface Direction

The user auth surface (`/login`, `/register`, `/forgetpassword`) is a pure
shadcn island.

- Use shadcn registry-style composition for auth: `Button`, `Input`, `Card`,
  `Label`, `Checkbox`, `Alert`, `DropdownMenu`, `Toast`, and related primitives
  should look and read like mature shadcn code.
- Use Radix for low-level accessible behavior and `lucide-react` for icons.
- Unprefixed Tailwind utilities and shadcn canonical token names are allowed in
  auth code.
- Keep auth behavior strict: API payloads, hash routes, `token2Login`, redirect
  safety, recaptcha, email verification, invite codes, TOS handling, language
  persistence, auth storage, and i18n behavior must remain covered.
- Retire legacy auth presentation code when the shadcn version replaces it; do
  not keep compatibility CSS or DOM solely to resemble the packaged frontend.
- Keep auth tests focused on behavior, accessibility, and shadcn structure
  rather than old pixel-era class names.

### User App Shell Direction

The logged-in user shell and dashboard (`/dashboard`) are redesigned shadcn
surfaces.

- Use shadcn/Radix composition for shell navigation, top chrome, menus, alerts,
  cards, dialogs, and primary dashboard controls.
- Keep logged-in behavior strict: auth redirects, language persistence, dark
  mode persistence, subscription import links, QR subscribe, notice dialogs,
  reset-package orders, new-period mutations, alert routing, and existing route
  contracts must remain covered.
- Retire old OneUI/Bootstrap visual parity for `/dashboard`; keep behavior and
  interaction scenarios for the same route.
- Preserve legacy page behavior for non-redesigned logged-in routes, but do not
  keep old shell DOM/classes solely to resemble the packaged frontend.

### User Commerce Direction

The user commerce flow (`/plan`, `/plan/:plan_id`, `/order`, and
`/order/:trade_no`) is a redesigned shadcn surface.

- Use shadcn/Radix composition for plan cards, filters, checkout summaries,
  coupon inputs, order tables/lists, payment method selection, QR checkout, and
  confirmation dialogs.
- Keep commerce behavior strict: plan filtering, sold-out handling, coupon
  checks, save-order payloads, unfinished-order cancellation, change-subscription
  confirmation, order cancellation, payment method selection, handling-fee math,
  Stripe token checkout, QR checkout, redirect checkout, polling, query cleanup,
  routing, i18n, and failure states must remain covered.
- Retire old OneUI/Bootstrap/Ant visual parity for these routes. Preserve stable
  behavior hooks only where tests or interaction parity need them, such as
  `.v2board-select`, `.v2board-input-coupon`, `.v2board-order-info`, and
  `.v2board-payment-qrcode`.

### User Profile Direction

The user profile/account surface (`/profile`) is a redesigned shadcn surface.

- Use shadcn/Radix composition for wallet cards, gift-card redemption, password
  forms, notification switches, Telegram binding, recharge dialogs, and reset
  confirmations.
- Use Radix-backed primitives for accessible dialogs and switches. Do not keep
  Ant Design confirm modals, Bootstrap blocks, OneUI forms, or rc-switch clones
  as the profile foundation.
- Keep profile behavior strict: balance display, auto-renewal and email
  reminder payloads, password-change redirect behavior, gift-card redeem loading
  and timeout behavior, deposit order payloads, Telegram bind/unbind behavior,
  reset-subscribe behavior, subscribe refetch timing, routing, and i18n behavior
  must remain covered.
- Retire old OneUI/Bootstrap/Ant visual parity for `/profile`; keep behavior and
  interaction scenarios for the same route.

### User Service Usage Direction

The user service usage surfaces (`/node` and `/traffic`) are redesigned shadcn
surfaces.

- Use shadcn/Radix composition for service cards, horizontally scrollable data
  tables, status indicators, tags, empty states, loading states, and tooltips.
- Do not keep Ant Design table shells, fixed-column clones, Bootstrap blocks, or
  legacy tooltip presentation as the foundation for these routes.
- Keep service behavior strict: subscribe-first fetch ordering, loading timing,
  empty-state subscribe/renew routing, node online/rate/tag rendering, traffic
  date formatting, byte formatting, legacy traffic charge coercion, horizontal
  scroll observability, tooltip text, timeout behavior, routing, and i18n
  behavior must remain covered.
- Retire old OneUI/Bootstrap/Ant visual parity for `/node` and `/traffic`; keep
  behavior and interaction scenarios for the same routes.

For new redesigned surfaces, do not use:

- Ant Design v3 components as new UI foundations.
- Bootstrap or OneUI classes such as `.btn`, `.block`, `.form-control`, or
  `.bg-gray-lighter`.
- Page-local hardcoded color, radius, or shadow systems on gradual-reskin
  surfaces when tokens/primitives can express the design.
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
