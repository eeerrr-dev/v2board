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
  supports only Debian 13 (Trixie) amd64 and runs the Debian-13-built native
  binaries plus the source-built frontend directly under systemd; do not add
  another operating system or Debian version, production Compose files,
  container runtime dependencies, or production image deployment instructions.
- The production application consists only of `backend/rust` plus the
  source-built frontend. The official stable-APT `cloudflared` package runs the
  one remotely managed named Cloudflare Tunnel and is the only public ingress;
  it connects outbound and forwards the one canonical public hostname to Rust
  at `http://127.0.0.1:8080`. The host has no public HTTP/HTTPS listener. Do not
  add Nginx, a second ingress, backend, server-side template layer, or duplicate
  worker and scheduler runtime.
- Source-owned frontend code lives under `frontend/apps/*/src`.
- Source-built deploy artifacts live in the Docker `frontend-deploy` volume at
  `/app/frontend-deploy` during local/CI builds, outside the mutable frontend
  workspace; do not write `dist-deploy/` on the host. A release export installs
  that validated tree at `/opt/v2board/current/frontend` on the server.
- The deploy root contains immutable `releases/<content-id>/{user,admin}` trees
  plus atomic `current`/`previous` symlinks. Rust reads the production tree
  directly and read-only through `V2BOARD_FRONTEND_DIR`, renders HTML from `current`, and serves hashed
  assets from `current` with `previous` as the in-flight rollout fallback. The
  public routes are `/`, the dynamic admin path, and `/assets/{user,admin}/*`.
- Rust-owned mutable configuration and application state live under
  `/var/lib/v2board`, never in a source directory. Production API and worker use
  distinct Unix users and `0600` role-owned config files; container mount
  isolation is not a production security boundary.
- `deploy/systemd/v2board-cloudflared.service` is the only tracked production
  ingress service and is shipped read-only with every native release. Its
  remotely managed Tunnel token is an operator-owned root-only systemd
  credential and never release content, configuration, an environment value,
  or a command-line literal. The unit's route-free `{}` `SetCredential` and
  explicit `--config` path disable cloudflared's implicit local-config search;
  this sentinel must contain no tunnel, hostname, ingress, or origin setting.
  Do not add an operator-managed local Tunnel configuration or a second
  Cloudflare connection mode.
- The Cloudflare route must map exactly the canonical `app_url` hostname to
  `http://127.0.0.1:8080`, leave HTTP Host Header empty so the visitor Host is
  preserved, and enable Cloudflare Always Use HTTPS. Production runtime
  `trusted_proxy_cidrs` is exactly `["127.0.0.1/32"]`. Rust accepts a visitor IP
  only from the single, strict `CF-Connecting-IP` value delivered by that local
  peer; it never trusts `Forwarded`, `X-Forwarded-For`, or
  `X-Forwarded-Proto`. Cloudflare must not remove visitor-IP headers, overwrite
  them with Pseudo IPv4, place a Worker/stacked proxy before this route, or put
  Cloudflare Access in front of the public application. WAF rules must not issue
  interactive challenges to `/api/v1/guest/payment/notify/*`,
  `/api/v1/guest/telegram/webhook`, `/api/v1/client/*`, `/api/v1/server/*`, or
  `/api/v2/server/config`.
- Cloudflare owns public TLS, CDN, WAF, DDoS protection, HTTP-to-HTTPS redirects,
  and edge logs. Rust continues to own HTML/assets, compression, cache policy,
  CORS, security headers, and the loopback-only `/healthz` and `/readyz` probes.
  The Tunnel must never expose those probes or publish any database service.
  Activation must prove that the canonical public `http://` URL receives a
  Cloudflare 3xx to the exact `https://` authority and path; an HTTP response
  from Rust is an activation failure.
  Keep Cloudflare caching on the origin-header-respecting default; never add
  Cache Everything for HTML, API, auth, payment, or webhook traffic. If HTTP
  Logpush is enabled, select the path-only field, never the full URI/query,
  Referer, request headers, or cookies, so subscription, verification, and
  payment secrets do not enter durable edge logs.
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
  Operator `custom_html` injection is removed by `docs/api-dialect.md`
  §10.5/§12 (its config field, HTML marker, and admin UI control still exist
  until that W1 wave lands) — do not extend it, and never reintroduce it
  after removal.
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
2. export one complete MySQL 8 dump as a protected backup artifact without
   modifying the old database;
3. on the stopped old production host, run the converter with a dedicated
   `SELECT`-only account against the original MySQL database;
4. inside one `REPEATABLE READ`, `READ ONLY`, consistent snapshot,
   deterministically transform the retained rows and stream each table directly
   through PostgreSQL `COPY FROM STDIN` over the same-datacenter private network
   into a brand-new dedicated PostgreSQL 18 cluster whose only initial
   non-template database is `postgres`;
   ClickHouse starts with an empty native event history, and a brand-new
   dedicated Redis 8.8 instance is empty across every logical database and uses
   canonical database `/0` for native runtime state;
5. on the old host, generate the new API and worker configuration files plus an
   import report in a new root-owned `config_output_directory`, then securely
   install the two configs at their fixed role-owned paths on the new host; and
6. verify the new installation, then start it.

The old MySQL database is never a migration target and is never mutated by the
importer. The dump is a complete backup and file-integrity artifact; it is not
loaded for conversion and its MySQL SQL is never sent to PostgreSQL. The Rust
converter reads typed MySQL rows, validates and transforms them explicitly, and
streams each target table through exactly one PostgreSQL `COPY FROM STDIN`.
Each source table uses one primary-key-ordered streaming MySQL `SELECT`; the
gift-card stream deterministically feeds both its base target and derived
redemption target. Memory is bounded to the
current decoded row, byte-bounded COPY send buffers, and a hard-capped
4,096-entry payment-id classification index required by the fixed Stripe order
policy. These buffers are not
PostgreSQL batches, and there is no fixed 1,000-row or other batched
`INSERT` path. The converter never writes an intermediate COPY/CSV or other
row-transfer file to either host. After every retained table has completed
COPY, `execute` creates the deferred business/cross-row unique constraints,
secondary indexes, and foreign keys, resets
all affected sequences, runs `ANALYZE`, and then scans each retained target
table exactly once in primary-key order to compare its canonical representation
with the source-derived canonical expectation accumulated during conversion.
The old host and the new PostgreSQL target must communicate over a
same-datacenter private network while retaining the required authenticated TLS.
Do not add a bulk-`INSERT` fallback, per-batch target verification, a second
transfer strategy, or a selectable transfer-mode setting. Do not design or add
migration rollback, source fencing, CDC, dual-write, shadow reads,
compatibility windows, authorization files, journals, checkpoints,
resume, operation recovery, or cleanup/restart state machines. A failed import
has no resumable migration state: delete the incomplete new
PostgreSQL/ClickHouse/Redis targets and configuration output directory; correct
the input or importer; and run the same simple import again against fresh empty
targets. This is not restoration of the untouched old database.

The converter runs on the stopped old production host and connects to the local
legacy MySQL through the manifest's loopback-only `source.database_url`. That
principal must have only database-level `SELECT`: execute rejects every extra
grant, assigned/enabled role, and `GRANT OPTION`. The importer also establishes
and verifies a server-enforced read-only consistent snapshot before schema
inspection or row conversion, and every imported source table must be InnoDB.
It writes outbound to the new PostgreSQL target through a temporary migration
principal. The new production host never runs MySQL. The native server and
long-running runtime contain only PostgreSQL, ClickHouse, and Redis.

`v2board-lifecycle execute` is the only import write path. It reads the stopped
legacy database through the read-only snapshot, requires the dedicated
PostgreSQL cluster plus absent PostgreSQL/ClickHouse targets and a new
whole-instance-empty dedicated Redis
`/0`, and creates `api.config.json`, `worker.config.json`, and
`import-report.json` under the old host's manifest-bound output directory. The
report distinguishes the inspected backup dump-file SHA-256 from the converted
source snapshot SHA-256. `imported_source_schema_sha256` hashes only the 14
imported source-table schemas, including their required InnoDB engine; the snapshot hash binds final retained content
(including inviter relationships), imported row counts, and the
separately represented whole-table-discard presence decisions. Discard-only
tables are presence-audited but not schema-bound, row-scanned, or counted.
`v2_tutorial` is
an allowed optional legacy residue and is discarded as a whole if present.
Never claim that the dump hash proves the contents of the independently read
source snapshot. The importer never treats that old-host directory as the new
machine's
`/var/lib/v2board`; the operator installs the two runtime configs as
`/var/lib/v2board/api/config.json` and `/var/lib/v2board/worker/config.json` with
their respective Unix identities. Do not hand-write a partial target as an
alternative to `execute`.

Every mapped legacy business-table primary key must be a positive integer.
Reject any source `id <= 0` during the read-snapshot preflight, before any
target write; do not coerce or preserve non-positive identities.

The manifest field `target.redis_bootstrap_url` is a one-shot, explicit
non-`default` Redis ACL administrator; it is never copied into a runtime config
or report. The dedicated Redis 8.8 target must start with exactly that user and
a disabled, non-passwordless `default` user, use `/0`, run `noeviction`, and
configure a writable external `aclfile`. `execute` creates random, distinct API
and worker ACL users, limits both to the installation keyspace, gives the API
read/write access only to API-owned keys plus read-only worker metrics, and
gives the worker read/write access only to scheduler, reset, heartbeat, metrics,
admission, and analytics keys. It must `ACL SAVE`, `ACL LOAD`, reconnect with
both generated credentials, and prove positive and negative command/key access
before emitting them as the two standard runtime `redis_url` values. Runtime
users may use `PING` and `INFO memory`, but never `CONFIG`, `DBSIZE`, `SELECT`,
`FLUSH*`, `ACL`, or arbitrary `EVAL`. Rotate or revoke the external bootstrap
credential after installation acceptance; do not turn that operator action into
an importer recovery state machine.

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

## Internal API Dialect Direction

The legacy-inherited internal API dialect is being replaced by a modern one.
`docs/api-dialect.md` is the single source of truth for the new dialect: the
complete old→new route map, the RFC 9457 `application/problem+json` error
model with its stable snake_case `code` registry, JSON-body/Bearer/
Accept-Language transport conventions, null-clear-vs-absent-retain
(double-Option) semantics, the admin filter/sort DSL, `page`/`per_page` +
`{items,total}` pagination, RFC 3339 timestamps, the checkout discriminated
union, history routing with the `legacy_hash_redirect_enable` toggle,
`custom_html` removal plus CSP tightening, the canonical locale key, and the
wave-by-wave migration appendix that later waves are parameterized from.

- All internal namespaces modernize: passport/auth (breaking third-party
  in-app login is an accepted owner decision), user, admin (under the kept
  dynamic `secure_path` prefix), and guest comm/config.
- The external namespaces stay byte-frozen exactly as listed in
  `docs/api-dialect.md` §2: `/api/v1/client/*` (+ the operator
  `subscribe_path` alias), `/api/v1/server/{class}/{action}`,
  `/api/v2/server/config`, `/api/v1/guest/payment/notify/{method}/{uuid}`,
  `/api/v1/guest/telegram/webhook`, the subscribe-URL/token/flag scheme,
  Stripe/reCAPTCHA/Crisp/Tawk integration payloads, and the localStorage
  `authorization` key (legacy locale keys remain one-time migration reads).
- No dual-dialect compatibility branches may ship: each endpoint family
  switches atomically — backend + frontend + api-client + fixtures +
  scenarios + goldens in one commit series, per the appendix waves.
- Frontend error discrimination moves to the problem `code`; exact
  error-string matching and the backend response-rewrite localization
  middleware are retired as families migrate.
- Cross-world parity comparison drops to canonical semantics through the
  per-world adapter layer (URL map, request/error/page-location
  canonicalizers, world-aware fixtures) under
  `frontend/tests/lib/dialect/`; byte-level request equality for the
  internal dialect is retired. The read-only reference oracle stays.
- The internal dialect is henceforth pinned by `docs/api-dialect.md` plus
  the golden wire lane (`frontend/packages/api-client/src/goldens.test.ts`,
  `backend/rust/crates/api/src/golden_wire.rs`) and the source-world
  interaction scenarios — consult the spec, not legacy code or the
  reference frontend, for internal shapes.

## Frontend Contract Direction

The user and admin applications are fully redesigned shadcn surfaces. The old
frontend is retired; only its read-only reference submodule remains to identify
externally observable compatibility contracts.

The internal API dialect migration (see "Internal API Dialect Direction"
above) rewords contract lines in this section wave by wave. Payload, route,
and header items in the per-surface lists below name the current Rust
contract as specified in `docs/api-dialect.md` — the live legacy shape until
a family's Appendix-A wave lands, the modern shape once it has — never a
permanent legacy byte pin. Each wave rewords the affected lines here in the
same commit series that flips its family; `docs/api-dialect.md` Appendix B
maps each affected line to its replacement wording and owning wave. The
behavioral outcomes those lines pin remain contracts throughout.

- Behavioral/contract parity is permanent, but the anchor is the Rust backend
  and external integrations, not the reference frontend. The reference only
  witnesses what those already expect; matching it is a proxy for matching the
  real contract, never an end in itself. Two tiers follow:
  - Tier 1 — non-negotiable (permanent): strictly external-party contracts,
    where changing one breaks a real external party: the byte-frozen external
    namespaces and integration payloads (subscription clients, node agents,
    payment-gateway notify routes and webhooks, Telegram, Google reCAPTCHA
    verification, Stripe PaymentIntent metadata/webhooks, Crisp/Tawk session
    data — `docs/api-dialect.md` §2); history route paths plus the
    `legacy_hash_redirect_enable` translator; backend-minted URLs are
    path-style per `docs/api-dialect.md` §10.4 — the SPA must keep resolving
    the URLs the backend mints (the `?verify=` email login link, the
    `{app_url}/order/{trade_no}` payment return, quick-login redirects;
    `?verify=`/`?redirect=` query names unchanged); the browser-persisted `authorization`
    localStorage key, plus the legacy locale keys strictly as
    one-time-migration reads; imported-data interpretations (the notice
    `弹窗` auto-popup tag, the knowledge `copy()`/`jump()` hooks in rendered
    markdown); and security- and session-critical OUTCOMES — session teardown
    exactly on session expiry (401 + `session_expired`; permission-denied and
    step-up rejections must never tear down the session), cross-account cache
    isolation, no-credentials CORS, server-side registration enforcement,
    and i18n/language persistence. All of this must stay green through
    `make behavior-parity` or a focused interaction-parity shard. The
    internal API dialect itself — routes, request/response shapes, the error
    model, transport headers — is pinned by `docs/api-dialect.md`, the
    golden wire lane, and the source-world interaction scenarios, not by
    this list; the spec also owns the contract-bearing edge cases (e.g. the
    empty-coupon omission rule, sold-out `capacity_limit` handling), which
    change only through a spec revision.
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
  Tier 1 only if a real external party consumes it — an externally-read URL,
  an auth/session persistence key, a security/redirect outcome, or a
  backend-field interpretation of imported data. Payload and route items name
  the current Rust contract as specified in `docs/api-dialect.md`, guarded by
  the golden lane and source-world scenarios — they must remain covered, but
  they are not external contracts. Items that are display formatting,
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

- Keep auth contracts strict: API payloads, routed paths (history routing per
  `docs/api-dialect.md` §10.1, with the §10.3 `legacy_hash_redirect_enable`
  boot translator), `token2Login`, redirect
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
  (`capacity_limit`), coupon checks, save-order payloads, unfinished-order
  handling and order cancellation with `trade_no` in the path
  (`POST /user/orders/{trade_no}/cancel`; W11 owns the admin-order half),
  change-subscription payload,
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
  (POST `/user/subscription/reset-token`), routing, and i18n must remain
  covered. Balance-display formatting, gift-card redeem loading/timeout UX,
  subscribe refetch timing, and reset/confirm dialog copy are Tier-2 defaults —
  relaxable on this redesigned surface.

### User Service Usage Direction

The user service usage surfaces (`/node` and `/traffic`) are redesigned shadcn
surfaces.

- Keep service contracts strict: subscription-gated node visibility (never
  render the node list before subscription state is known — parallel fetch
  with subscription-gated rendering per `docs/api-dialect.md` §4.6),
  empty-state subscribe/renew routing, node `is_online`/`server_rate`
  interpretation and charged math `(u+d)*server_rate` (numeric on the modern
  §5.4 wire), routing, and i18n must remain covered. Loading/timeout timing, the visual rendering of
  online/rate/tag badges, traffic date and byte formatting, horizontal-scroll
  observability, and tooltip copy are Tier-2 defaults — relaxable on these
  redesigned routes.

### User Invite Direction

The user invite/commission surface (`/invite`) is a redesigned shadcn surface.

- Keep invite contracts strict: copy-link URL, `POST /user/invite-codes`
  (§5.6),
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
interaction-parity scenarios are the standing contract guard. They run in the
full `make interaction-parity` suite (desktop + mobile); to narrow a run,
list exact scenario labels — `INTERACTION_PARITY_SCENARIOS` matches each
label end-anchored, so a bare `admin` prefix selects nothing (e.g.
`INTERACTION_PARITY_SCENARIOS="admin-config-tabs admin-users-sort-matrix"
make interaction-parity`). Do not impose a frontend-only language restriction
on the admin app; shared locale state, document language/direction, and the
API locale header (`Content-Language` today, `Accept-Language` per
`docs/api-dialect.md` §4.3) must remain coherent. Existing untranslated copy
may stay Chinese until product translations are supplied, but Chinese-only
behavior is not a contract.

- Keep admin contracts strict: every admin API endpoint and request payload
  (config, coupon/giftcard/notice/knowledge/plan/server/user/order/ticket
  create/edit/delete bodies, encoded per the current Rust contract as
  specified in `docs/api-dialect.md` — JSON bodies with real arrays such as
  `limit_plan_ids` per §4.1 for migrated families), the
  cents conversions (e.g. coupon `type===1 → value*100`), list/fetch query,
  pagination, and filter/sort parameters (bracket `filter[i][…]` params
  today, the spec's §7 filter DSL once its consumer waves land), admin
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
- Cloudflare Tunnel service or production-ingress changes: run
  `make cloudflared-config-audit` and `make deploy-smoke`.
- Redesigned behavior changes: run focused `make interaction-parity` shards.
- Visual/layout changes: use focused `make visual-smoke` for a browser-rendered
  smoke of the deployed assets.
