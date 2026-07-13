# AGENTS.md

## Local Workflow

Use Docker for local setup, dependencies, builds, and tests. Do not run
Cargo, pnpm, npm, build, or test commands on the host when they would create
`target/`, `node_modules/`, `.pnpm-store/`, `dist/`, `.vite/`, coverage, cache,
reports, or deploy output inside the repository.

- Use `make up`, `make down`, `make reset`, `make sync`, and `make logs`.
- After source edits that need Docker execution, run `make sync` so the Docker
  frontend workspace and immutable deploy release receive the host changes.
- Use `make doctor` for the general local sanity gate.
- Use `make rust-check`, `make rust-test`, and `make rust-target-gate` for the
  native backend and worker gates.
- Use `make public-bundle-audit` to confirm host build/deploy targets stay empty.
- Use `make parity-config-audit` after changing routes or visual/interaction
  parity scenario lists.
- Use `make deploy-smoke` after deploy-path or public asset changes.
- Use `make visual-smoke` after visual/layout work that needs browser review.
- Use `make behavior-parity` for the durable reskin behavior gate.
- Use `make interaction-parity` (Playwright Test) for focused browser behavior
  checks. Narrow the scenarios with `INTERACTION_PARITY_SCENARIOS=... make
  interaction-parity` and the viewports with `VISUAL_PARITY_VIEWPORTS=desktop`
  (or `mobile`). The old pixel/screenshot `make visual-parity` lane is retired:
  every scenario is `visualRetired`, so parity is behavioral, not byte-for-byte.
- Use `make reference-oracle-check` before relying on the pinned, read-only
  compatibility reference. `make reference-oracle-up` is only for optional
  manual inspection on `http://localhost:8001`.
- Use `make clean-host` to preview ignored host cleanup. Use
  `make clean-host-apply` only after reviewing the `make clean-host` preview and
  confirming every listed path is disposable host output.

If a direct package/test command is needed, run it inside the appropriate Docker
service or one-off container. Keep generated dependency, cache, build, deploy,
visual, interaction, database, and native runtime artifacts in Docker volumes.

## Source And Deploy Rules

- `docker-compose.local.yml` is the canonical tracked local workflow.
  `docker-compose.yml` is ignored and only for personal overrides.
- Docker is only a local development, build, CI, and test boundary. Production
  runs exported native Linux binaries and the source-built frontend directly on
  the server under systemd; do not add production Compose files, container
  runtime dependencies, or production image deployment instructions.
- Production consists only of `backend/rust` plus the source-built frontend.
  Do not add a second backend, server-side template layer, or duplicate worker
  and scheduler runtime.
- Source-owned frontend code lives under `frontend/apps/*/src`.
- Source-built deploy artifacts live in the Docker `frontend-deploy` volume at
  `/app/frontend-deploy` during local/CI builds, outside the mutable frontend
  workspace; do not write `dist-deploy/` on the host. A release export installs
  that validated tree at `/opt/v2board/frontend` on the server.
- The deploy root contains immutable `releases/<content-id>/{user,admin}` trees
  plus atomic `current`/`previous` symlinks. Rust reads the production tree
  directly and read-only through `V2BOARD_FRONTEND_DIR`, renders HTML from `current`, and serves hashed
  assets from `current` with `previous` as the in-flight rollout fallback. The
  public routes are `/`, the dynamic admin path, and `/assets/{user,admin}/*`.
- Rust-owned mutable configuration and application state live under
  `/var/lib/v2board`, never in a source directory. Production API and worker use
  distinct Unix users and `0600` role-owned config files; container mount
  isolation is not a production security boundary.
- Visual and interaction reports live in Docker artifact volumes under
  `/app/frontend/.cache/`; do not write parity reports on the host.
- Production deployment is manifest-driven: the build validates each Vite
  manifest and emits hashed ESM/CSS plus a guarded `index.html`; Rust serves the
  validated index and injects runtime config. Manifests are not public runtime
  inputs. Do not recreate fixed `umi.css`/`umi.js` entry names.
- Never import, concatenate, copy, serve, or deploy the old packaged bundles:
  `umi.js`, `umi.css`, `components.chunk.css`, `vendors.async.js`,
  `components.async.js`, `env.example.js`, copied static/i18n/theme assets, or
  admin equivalents. There are no runtime `custom.css`/`custom.js` fallbacks.
- `references/wyx2685-v2board` is the only retained old implementation. It is a
  pinned git submodule mounted read-only for compatibility tests. Never COPY it
  into an image, source tree, runtime volume, Vite input, or deploy release.

## Pre-Release MySQL Import Direction

The native product has not been released or installed in production. There is
no prior native install, migration schema, or upgrade contract to preserve.
Unpublished lifecycle code, examples, and PostgreSQL/ClickHouse migrations may be
consolidated in place; do not add compatibility branches, aliases, bridge
migrations, or tombstone tables for local-only history.

There is exactly one legacy-data path:

1. stop writes to the old site;
2. export one complete MySQL 8 dump without modifying the old database;
3. on the stopped old production host, load that dump into a separate,
   disposable MySQL 8 engine used only as converter input;
4. deterministically transform the retained rows into a brand-new PostgreSQL
   database; ClickHouse starts with an empty native event history, and a
   brand-new empty Redis is used for native runtime state;
5. generate the new API and worker configuration files from explicit target
   values; and
6. verify the new installation, then start it.

The old MySQL database is never a migration target and is never mutated by the
importer. Loading the dump into staging is import processing, not recovery. Do
not design or add migration rollback, source fencing, CDC, dual-write, shadow
reads, compatibility windows, authorization files, journals, checkpoints,
resume, operation recovery, or cleanup/restart state machines. A failed import
has no resumable migration state: discard that new incomplete target, correct
the input or importer, and run the same simple import again against a new empty
target. This is not restoration of the untouched old database.

The default cutover topology runs the temporary MySQL 8 engine and converter on
the stopped old production host. Staging must be a separate instance with its
own data directory or volume, port/socket, credentials, and loopback-only bind;
never create it inside the source MySQL instance or mount the source data
directory. The converter writes outbound to the new PostgreSQL target through a
temporary migration principal. The new production host never runs staging or
MySQL. If the old host lacks capacity, use a disposable migration VM instead.
Stop and delete staging after success or failure; the native server and
long-running runtime contain only PostgreSQL, ClickHouse, and Redis.

Legacy MySQL source tables retain their real `v2_*` names. Native PostgreSQL and
ClickHouse target tables are first-release names without that prefix; use
`users` and `orders` instead of the PostgreSQL keywords `user` and `order`.
The other non-mechanical PostgreSQL target names are `payment_method`,
`gift_card`, `gift_card_redemption`, `system_log`, `server_traffic`, and
`user_traffic`; `stat` remains `stat`. Converter metadata must always label
source and target names separately. Do not add rename migrations, aliases,
views, or compatibility tables for unpublished target names.

The importer never reads old Redis and never contacts Stripe. Old Redis state,
Stripe configuration, and unfinished Stripe orders are fixed accepted losses;
terminal Stripe orders may remain only as provider-detached business history.
Do not make these fixed decisions configurable per run. Do not add speculative
fresh-install or native-upgrade lifecycle formats before the first release.
Migration code, documentation, examples, and tests must describe only this one
current import contract and must not retain retired schema names or workflows.
These pre-release installation rules do not relax the permanent external API,
integration, or frontend behavior contracts below.

## Frontend Contract Direction

The user and admin applications are fully redesigned shadcn surfaces. The old
frontend is retired; only its read-only reference submodule remains to identify
externally observable compatibility contracts.

- Behavioral/contract parity is permanent, but the anchor is the Rust backend
  and external integrations, not the reference frontend. The reference only
  witnesses what those already expect; matching it is a proxy for matching the
  real contract, never an end in itself. Two tiers follow:
  - Tier 1 — non-negotiable (permanent): true external contracts — API endpoints
    and request payloads, auth/session persistence keys, hash route paths (the
    backend emails links into them, e.g. `?verify=`), and payloads sent to
    external integrations (e.g. Stripe PaymentIntent metadata/webhooks and
    Crisp/Tawk session data) — where changing one breaks a real external party; plus the security-
    and session-critical behavioral OUTCOMES (auth redirects, i18n/language
    persistence) and any edge case that maps to a backend or data contract (e.g.
    which payload is sent for an empty coupon, sold-out handling), pinned as
    behavior, not as bytes. All of this must stay green through
    `make behavior-parity` or a focused interaction parity shard.
  - Tier 2 — conservatively pinned, relaxable per redesigned surface: things no
    external party consumes — display-only formatting (date/number rendering, or
    exact rendered-HTML bytes such as a trailing newline or attribute order) and
    pure-presentation edge-case choices (e.g. whether a transport/network error
    surfaces a toast, spinner timing). These are pinned by tests as a low-risk
    default, not because a contract requires them. On a redesigned surface the
    owner may consciously change a Tier-2 detail — updating or retiring its
    scenario, the same way visual parity is retired — provided Tier 1 stays
    intact and a behavior/interaction scenario still covers the route. When
    unsure whether an edge case is a data contract or pure presentation, treat it
    as Tier 1. Do not treat a Tier-2 pin as an external contract.
- The per-surface "must remain covered" lists further down inherit this Tier
  model; read them through it rather than as flat byte-pins. An item there is
  Tier 1 only if a real external party consumes it — a request payload, an
  externally-read URL, an auth/session key, a security/redirect outcome, or a
  backend-field interpretation. Items that are display formatting,
  spinner/toast/modal/poll/debounce/refetch timing, query/cache cleanup, scroll
  observability, or popup-vs-mobile navigation are Tier 2 — relaxable on a
  redesigned surface as long as a behavior/interaction scenario still covers the
  route.
- Visual/pixel parity is retired for every surface. Keep behavior/interaction
  coverage for every route and prioritize shadcn/Radix composition,
  accessibility, and real contracts over reference DOM or legacy class names.

## Modern Frontend Stack

Continue building on the current verified frontend rather than starting another
application or compatibility layer. New work uses:

- React + TypeScript + Vite.
- React Router for the existing routing/deploy shape.
- TanStack Query for server state.
- `@v2board/api-client` for API contracts.
- Existing i18n infrastructure.
- Radix primitives for accessible low-level behavior.
- shadcn/ui registry components copied into and owned by each app's local
  `components/ui`, composed from Radix primitives.
- `lucide-react` for new icons.
- Tailwind v4.
- Local CSS variables and shared canonical shadcn token names. Do not add a
  token package until a real non-shadcn consumer requires one.

Use unprefixed Tailwind utilities and shadcn token names. Do not reintroduce the
retired `tw:` gradual-reskin convention, legacy global selector ownership, or a
second styling system.

### Auth Surface Direction

The user auth surface (`/login`, `/register`, `/forgetpassword`) is a pure
shadcn island.

- Keep auth contracts strict: API payloads, hash routes, `token2Login`, redirect
  safety, recaptcha, email verification, invite codes, language persistence,
  auth storage, and i18n behavior must remain covered. The TOS gate (block submit
  when `tos_url` is configured and unaccepted) is a UX behavior, not a payload
  field — cover the gate, but its presentation is Tier 2.

### User App Shell Direction

The logged-in user shell and dashboard (`/dashboard`) are redesigned shadcn
surfaces.

- Keep logged-in contracts strict: auth redirects, language persistence,
  subscription import links, QR subscribe, the notice-dialog auto-popup (the
  backend `弹窗` tag), reset-package orders, new-period mutations, and existing
  route contracts must remain covered. Dark-mode persistence (a frontend-only
  `dark_mode` cookie) and dashboard alert routing are Tier-2 presentation
  defaults — relaxable on this redesigned surface.

### User Commerce Direction

The user commerce flow (`/plan`, `/plan/:plan_id`, `/order`, and
`/order/:trade_no`) is a redesigned shadcn surface.

- Keep commerce contracts strict: plan filtering, sold-out handling
  (`capacity_limit`), coupon checks, save-order payloads, unfinished-order and
  order cancellation payloads (`{trade_no}`), change-subscription payload,
  payment-method selection, Stripe PaymentIntent preparation and Payment Element
  confirmation (including signed webhook settlement) / QR / redirect checkout, routing, and
  i18n must remain covered. Handling-fee math (a display estimate; the server
  value always wins), polling cadence, query/cache cleanup, confirmation-dialog
  copy, and failure-state presentation are Tier-2 defaults — relaxable on these
  redesigned routes.
- Preserve stable commerce behavior hooks only where tests or interaction parity
  need them — the live ones are the `#cashier` container and the `data-testid`
  values `coupon-input`, `order-info`, `payment-qrcode`, `payment-option`, and
  `commerce-submit`. (The `.v2board-*` class names are reference-side selector
  fallbacks, not source hooks.)

### User Profile Direction

The user profile/account surface (`/profile`) is a redesigned shadcn surface.

- Keep profile contracts strict: auto-renewal and email-reminder payloads,
  password-change redirect (security), gift-card redeem payload, deposit order
  payloads, Telegram bind/unbind, the reset-subscribe token rotation
  (`/user/resetSecurity`), routing, and i18n must remain covered. Balance-display
  formatting, gift-card redeem loading/timeout UX, subscribe refetch timing, and
  reset/confirm dialog copy are Tier-2 defaults — relaxable on this redesigned
  surface.

### User Service Usage Direction

The user service usage surfaces (`/node` and `/traffic`) are redesigned shadcn
surfaces.

- Keep service contracts strict: subscribe-first fetch ordering, empty-state
  subscribe/renew routing, node `is_online`/`server_rate` interpretation and
  charged math `(u+d)*server_rate`, legacy traffic-charge coercion, routing, and
  i18n must remain covered. Loading/timeout timing, the visual rendering of
  online/rate/tag badges, traffic date and byte formatting, horizontal-scroll
  observability, and tooltip copy are Tier-2 defaults — relaxable on these
  redesigned routes.

### User Invite Direction

The user invite/commission surface (`/invite`) is a redesigned shadcn surface.

- Keep invite contracts strict: copy-link URL, the `/user/invite/save` call,
  transfer payload conversion (`100*amount`) through the API layer, withdraw
  method/account payload, commission cents reading (`amount/100`), routing, and
  i18n must remain covered. Fetch order, invite-success toast and refetch timing,
  commission/`toFixed` and distribution-rate formatting, history pagination
  clamping (display only — the raw page is sent unclamped), failure-modal
  persistence, the post-withdraw in-app nav to `/ticket` (not a backend-emailed
  link), and tooltip copy are Tier-2 defaults — relaxable on this redesigned
  surface.

### User Ticket Direction

The user ticket surfaces (`/ticket`, `/ticket/:ticket_id`) are redesigned shadcn
surfaces.

- Keep ticket contracts strict: ticket-id passthrough, reply / create-ticket /
  close-ticket payloads, routing, and i18n must remain covered. Fetch cleanup on
  unmount, desktop-popup-vs-mobile detail navigation, reply polling cadence,
  reply toast/input clearing, reply / save / close failure persistence,
  cancel-draft persistence, and successful-save reset/refetch timing are Tier-2
  defaults — relaxable on these redesigned routes.

### User Knowledge Direction

The user knowledge surface (`/knowledge`) is a redesigned shadcn surface.

- Keep knowledge contracts strict: fetch locale, URL `id` opening, article
  detail fetches, the `copy()`/`jump()` hooks inside rendered markdown, current-
  article refetches (the backend body is non-idempotent — re-substituted per
  request), routing, and i18n must remain covered. Search debounce, timeout
  fallback, and previous-article persistence while jumping are Tier-2 defaults —
  relaxable on this redesigned surface.

### User Surface System Direction

Every redesigned user surface composes with shadcn/Radix primitives and
`lucide-react` icons, and owns its shared primitives in
`frontend/apps/user/src/components/ui`. (A surface explicitly designated a pure
shadcn island — see Auth — may use unprefixed Tailwind utilities and shadcn token
names.) Do not rebuild any redesigned surface on a legacy foundation — e.g. Ant
Design table shells, fixed-column clones, confirm modals, selects, or
input/drawer shells; Bootstrap blocks or list groups; OneUI forms or block cards;
rc-switch clones; old chat CSS-module classes; legacy tooltip presentation; or
body-scroll drawer wiring. Retire legacy presentation code once its shadcn
replacement lands, and keep tests focused on behavior, accessibility, and shadcn
structure rather than pixel-era class names.

- Use the local shadcn-style table primitives for redesigned user tables instead
  of page-local `thead`/`tbody`/`th`/`td` class systems.
- Keep route-specific `v2board-*` hooks on top of shared primitives when
  behavior or interaction parity needs stable selectors.
- Do not reintroduce Ant table class names such as `ant-table-column-title` or
  `ant-table-tbody` as presentation helpers on redesigned user tables.

### Admin Surface Direction

The entire admin app (`frontend/apps/admin/src/pages/*`) is a redesigned shadcn
surface — every admin page is shadcn/Radix with zero Ant Design imports. Its
visual-parity scenarios are retired (`visualRetired: true`), so the admin
interaction-parity scenarios are the standing contract guard, run with
`INTERACTION_PARITY_SCENARIOS="admin" make interaction-parity` (desktop +
mobile). Do not impose a frontend-only language restriction on the admin app;
shared locale state, document language/direction, and API locale headers must
remain coherent. Existing untranslated copy may stay Chinese until product
translations are supplied, but Chinese-only behavior is not a contract.

- Keep admin contracts strict: every admin API endpoint and request payload
  (config, coupon/giftcard/notice/knowledge/plan/server/user/order/ticket
  create/edit/delete bodies, including form-encoded array shapes like
  `limit_plan_ids[0]`), the cents conversions (e.g. coupon `type===1 →
  value*100`), list/fetch query and pagination/filter parameters, admin
  auth/session persistence, and route contracts must stay covered by an
  interaction-parity scenario.
- Tier-2 defaults are relaxable here: overlay chrome (sheet vs modal vs drawer),
  button order, spinner/toast/poll/refetch timing, close-overlay-on-save timing,
  table truncation and horizontal-scroll observability, and date-picker chrome.
- Admin interaction scenarios use union selectors (shadcn slot/testid/role first,
  Ant class fallback) so one `run(page)` drives both the shadcn source and the
  read-only reference UI, with a Tier-1 `normalize*InteractionResult` reducer dropping
  Tier-2 presentation. Keep that pattern when adding or editing admin scenarios
  rather than branching per world, and reduce cross-world comparison to the
  Tier-1 payload/query/redirect fields while dropping presentation.

For new redesigned surfaces, do not use:

- Ant Design v3 components as new UI foundations.
- Bootstrap or OneUI classes such as `.btn`, `.block`, `.form-control`, or
  `.bg-gray-lighter`.
- Page-local hardcoded color, radius, or shadow systems when tokens/primitives
  can express the design.
- Hidden runtime dependencies on packaged legacy bundles.
- Copying the old packaged bundle DOM, CSS, or file structure as the foundation
  for a redesigned surface.

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
- Visual/layout changes: use focused `make visual-smoke` for a browser-rendered
  smoke of the deployed assets.
