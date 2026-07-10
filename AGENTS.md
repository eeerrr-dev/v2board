# AGENTS.md

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
  `make clean-host-apply` only after reviewing the `make clean-host` preview and
  confirming every listed path is disposable host output.

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

- Behavioral/contract parity is permanent, but the anchor is the shared backend
  and external integrations, not the frozen frontend. The legacy oracle only
  witnesses what those already expect; matching it is a proxy for matching the
  real contract, never an end in itself. Two tiers follow:
  - Tier 1 — non-negotiable (permanent): true external contracts — API endpoints
    and request payloads, auth/session persistence keys, hash route paths (the
    backend emails links into them, e.g. `?verify=`), and payloads sent to
    external integrations (e.g. the Stripe card token and Crisp/Tawk session
    data) — where changing one breaks a real external party; plus the security-
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
    as Tier 1. Do not treat a Tier-2 pin as an external contract, and do not
    relax one on a surface still on the replica.
- The per-surface "must remain covered" lists further down inherit this Tier
  model; read them through it rather than as flat byte-pins. An item there is
  Tier 1 only if a real external party consumes it — a request payload, an
  externally-read URL, an auth/session key, a security/redirect outcome, or a
  backend-field interpretation. Items that are display formatting,
  spinner/toast/modal/poll/debounce/refetch timing, query/cache cleanup, scroll
  observability, or popup-vs-mobile navigation are Tier 2 — relaxable on a
  redesigned surface as long as a behavior/interaction scenario still covers the
  route.
- Visual/pixel parity is retired only for redesigned surfaces: mark their visual
  scenarios `visualRetired: true` in `frontend/scripts/visual-parity.mjs` and
  keep a behavior/interaction scenario for the route. This holds for every
  redesigned surface below, so the per-surface sections do not restate it.
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
`.form-control`; prefixed utilities avoid accidental collisions. No surface is
currently gradual-reskin — the user surfaces are all pure shadcn islands (the
user app even asserts `tw:` is absent) and the admin app is a redesigned shadcn
surface (zero Ant Design imports) — so this prefix rule and the `@v2board/tokens`
/ local `components/ui` gradual-reskin guidance above apply only if such a surface
is reintroduced.

For pure shadcn islands, unprefixed Tailwind utilities and shadcn token names are
allowed intentionally. Keep those islands route- or component-scoped, avoid
leaking their assumptions into replica surfaces, and verify behavior with tests.

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
  payment-method selection, Stripe-token / QR / redirect checkout, routing, and
  i18n must remain covered. Handling-fee math (a display estimate; the server
  value always wins), polling cadence, query/cache cleanup, confirmation-dialog
  copy, and failure-state presentation are Tier-2 defaults — relaxable on these
  redesigned routes.
- Preserve stable commerce behavior hooks only where tests or interaction parity
  need them — the live ones are the `#cashier` container and the `data-testid`
  values `coupon-input`, `order-info`, `payment-qrcode`, `payment-option`, and
  `commerce-submit`. (The `.v2board-*` class names are legacy oracle-side selector
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
- Avoid bare Tailwind utility classes that collide with legacy global selectors
  on redesigned user surfaces. In particular, do not use `.block` as a layout
  utility because OneUI owns that class; use `flex`, `grid`, `inline-block`, or
  no display class instead.

### Admin Surface Direction

The entire admin app (`frontend/apps/admin/src/pages/*`) is a redesigned shadcn
surface — every admin page is shadcn/Radix with zero Ant Design imports. Its
visual-parity scenarios are retired (`visualRetired: true`), so the admin
interaction-parity scenarios are the standing contract guard, run with
`INTERACTION_PARITY_SCENARIOS="admin" make interaction-parity` (desktop +
mobile). Admin copy stays Chinese-only; preserve exact Chinese labels and titles.

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
  frozen Ant oracle, with a Tier-1 `normalize*InteractionResult` reducer dropping
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
