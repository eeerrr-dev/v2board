# Rust Backend Rewrite Plan

Updated: 2026-07-05

## Goal

Rewrite the backend in Rust as a complete replacement for the Laravel runtime.
Development is a full rewrite, but production cutover is a single controlled
switch after the Rust backend passes compatibility, smoke, and behavior gates.

This is not a long-term route-by-route hybrid migration. Laravel remains only
as the behavior oracle during development.

## Core Decision

Use Rust to fully replace the backend implementation while preserving the v2
external contract for the first release:

- Keep `/api/v1/*` and `/api/v2/*` routes.
- Keep frontend request fields and response envelope shapes.
- Keep payment callback URLs and payload contracts.
- Keep subscription URLs and node server APIs.
- Keep the existing `v2_*` MySQL schema for the first Rust release.
- Keep auth compatibility for `authorization` and `auth_data`.

Do not combine the first Rust rewrite with a full API redesign, frontend API
rewrite, and database redesign. That should be a later v3 cleanup after the
Rust backend is stable.

## Stack

- Rust toolchain: stable `1.96.1`
- Rust edition: `2024`
- HTTP: `axum`
- Runtime: `tokio`
- Database: `sqlx` + MySQL
- Cache/session/queue: Redis
- HTTP client: `reqwest`
- Serialization: `serde`, `serde_json`, `serde_urlencoded`, `rmp-serde`
- Passwords: `bcrypt` for new hashes, with legacy fallback for existing bcrypt,
  argon2id, md5, sha256, and md5salt
- JWT: `jsonwebtoken`
- Worker queue: `apalis` + Redis. Use the latest non-prerelease line
  (`0.7.4` as of 2026-07-05); do not adopt `1.0.0-rc.*` until it becomes a
  stable release.
- Scheduler: `apalis-cron` on the same latest non-prerelease line (`0.7.4` as
  of 2026-07-05)
- Observability: `tracing` + `tower-http`
- OpenAPI: `utoipa`
- Testing: `rstest`, `insta`, `testcontainers`

Dependency policy: use the latest stable crate releases that compile on the
current stable Rust toolchain. Commit `Cargo.lock` for reproducible application
builds, run Rust commands only inside Docker, and keep `make rust-check` as the
style gate with `rustfmt` and Clippy warnings treated as errors.

## Why This Stack

Axum is selected over Actix because the main risk is compatibility migration,
not raw router throughput. The project needs a thin HTTP layer, custom
extractors, predictable response conversion, and reusable Tower middleware for
tracing, timeout, compression, auth, and test harnesses.

SQLx is selected over an ORM because the existing schema is legacy-shaped:
integer timestamps, JSON columns, money in cents, traffic in bytes, and
Laravel-style casts. Explicit SQL makes compatibility easier to audit.

Redis stays central because auth sessions, temporary tokens, subscribe OTP/TOTP
state, node online stats, and worker coordination already depend on cache
semantics.

Apalis-cron is selected for scheduled jobs because the rewrite already uses
Apalis for workers. Cron jobs should enter the same worker, middleware,
observability, retry, load-shed, and concurrency model instead of using a
separate scheduler runtime.

## Proposed Workspace

```text
backend/rust/
  Cargo.toml
  crates/api          # Axum routes, middleware, HTTP compatibility
  crates/domain       # Business services
  crates/db           # SQLx repositories, transactions, row mapping
  crates/compat       # Laravel-compatible envelope, auth, forms, errors, cache keys
  crates/config       # Runtime config replacing PHP config/v2board.php
  crates/protocols    # Subscription outputs: Clash, Sing-box, V2rayN, General, etc.
  crates/payments     # Payment gateway trait and gateway implementations
  crates/workers      # Queue jobs and scheduled jobs
  crates/admin        # Admin management domain
  crates/contract     # Laravel-vs-Rust black-box contract tests
```

The current implementation keeps payments, protocols, and admin domain code
inside `crates/domain` and `crates/api`; they can be split into the proposed
dedicated crates later if that reduces maintenance cost.

## Compatibility Contracts

These are release blockers for the first Rust version:

- API success responses continue to use the legacy envelope, usually
  `{ "data": ... }`, sometimes with `type`, `total`, `buffer`, or extra fields.
- The frontend still treats only HTTP 200 as success.
- POST requests must accept `application/x-www-form-urlencoded`.
- Endpoints that currently accept JSON, especially node push/sort endpoints,
  must continue accepting JSON.
- Error responses must preserve the current `message` behavior closely enough
  for the frontend and tests.
- `Content-Language` behavior and i18n outcomes must remain compatible.
- JWT auth uses the existing app key and HS256 compatibility.
- Redis session keys and temporary-token keys remain compatible.
- Password verification supports old hashes. New registrations and password
  changes write bcrypt hashes for Laravel compatibility.
- Money remains stored and transferred as cents.
- Traffic remains bytes; plan traffic remains GiB-derived.
- ETag, 304, and msgpack behavior remain compatible for node APIs.
- Subscription token modes remain compatible: plain token, OTP, and TOTP.
- Admin secure path behavior remains compatible.

## High-Risk Domains

### Auth And Session

Must preserve:

- `authorization` header and `auth_data` request input.
- JWT payload compatibility.
- Redis-backed active sessions.
- Login rate limiting.
- Email verification codes.
- `token2Login` and quick-login links.
- Invite-code registration.
- Legacy password verification and login-time rehash.

### Commerce And Orders

Must preserve:

- Plan filtering and sold-out handling.
- Coupon checks and coupon usage side effects.
- Save-order payloads.
- Deposit orders.
- Balance deduction and cancellation refund.
- Reset-traffic package behavior.
- Change-subscription surplus/refund calculations.
- Handling-fee calculation.
- Payment selection and checkout payloads.
- Payment notify idempotency.
- Order open logic and order status transitions.

All order/payment writes must run inside database transactions. Payment success
must be safe to receive more than once.

### Payments

Implement gateways behind a common trait:

```text
trait PaymentGateway {
  fn form(&self) -> PaymentForm;
  async fn pay(&self, order: PaymentOrder) -> Result<PaymentIntent>;
  async fn notify(&self, request: NotifyRequest) -> Result<NotifyResult>;
}
```

Suggested implementation order:

1. StripeCredit / StripeCheckout
2. EPay
3. BTCPay
4. Coinbase / CoinPayments
5. AlipayF2F / WechatPayNative
6. Remaining low-frequency gateways

### Subscription And Node APIs

Must preserve:

- `/api/v1/client/subscribe`
- `/api/v1/server/*`
- `/api/v2/server/config`
- User-agent based protocol selection.
- General, Clash, Sing-box, V2rayN, Shadowrocket, Surfboard, Surge, Stash,
  Loon, QuantumultX, Passwall, and related subscription formats.
- Node token validation.
- Node config fetch.
- Node user fetch.
- Node traffic push.
- Node alive list and alive push.
- ETag and 304 behavior.
- msgpack response support.

First implementation priority is byte-level contract compatibility where node
clients depend on it. Refactoring protocol generation can happen later.

### Admin

Must preserve the admin frontend contract:

- Config fetch/save.
- User management.
- Plan management.
- Node/server management.
- Payment management.
- Coupon and giftcard management.
- Notice and knowledge management.
- Ticket management.
- Statistics and queue/status endpoints.

Internals may be redesigned, but response fields and payload semantics should
stay compatible for the first Rust release.

### Workers And Scheduler

Replace Laravel Horizon and scheduler with Rust worker processes.

Job mapping:

```text
traffic:update       -> traffic_update
check:order          -> check_order
check:commission     -> check_commission
check:ticket         -> check_ticket
check:renewal        -> check_renewal
reset:traffic        -> reset_traffic
reset:log            -> reset_log
send:remindMail      -> send_remind_mail
v2board:statistics   -> statistics
```

Only one runtime may own a job class at cutover. Do not run Laravel scheduler
and Rust scheduler for the same job at the same time.

For multi-instance deployments, cron triggers must either be persisted/distributed
through a shared backend or guarded by a distributed lock. Every scheduled job
must also be idempotent, because duplicated ticks are possible during restarts,
deploys, lock expiration, or manual replay.

## Implementation Phases

## Current Rewrite Status

Status as of 2026-07-05:

Completed in the Rust tree:

- Axum API service, Docker workflow, config loading from Laravel-compatible
  config, MySQL pool, Redis pool, legacy response envelopes, auth extraction,
  and tracing.
- Passport/auth surface: login, register, forget password, email verification,
  token2Login, quick-login URLs, invite PV, active sessions, bcrypt writes, and
  legacy password verification.
- User surface: profile/info/stat/subscribe, preferences, password change,
  reset security, unbind Telegram, new period, gift cards, commission transfer,
  plans, coupon checks, orders, checkout, cancellation, payment methods,
  invite, tickets, notice, knowledge, traffic logs, Stripe public key, and
  Telegram bot info.
- Payment implementations for the built-in gateways currently mapped in Rust:
  EPay, MGate, BEasyPaymentUSDT, CoinPayments, Coinbase, BTCPay,
  WechatPayNative, AlipayF2F, StripeCredit, StripeAlipay, StripeWepay,
  StripeCheckout, and StripeALL.
- Subscription and client endpoints: `/api/v1/client/subscribe`,
  `/api/v1/client/app/getConfig`, and `/api/v1/client/app/getVersion`.
  Rust subscription rendering now routes the legacy client flags for General,
  Clash, Meta/Mihomo/Stash/Clash Verge/Nyanpasu, Sing-box, Surge, Surfboard,
  Loon, Shadowrocket, SIP008 Shadowsocks, Quantumult X, SagerNet, V2rayN,
  V2rayNG, v2RayTun, Passwall, and SSRPlus.
- Node/server APIs: `/api/v1/server/{class}/{action}` and
  `/api/v2/server/config`, including token validation, config fetch, user fetch,
  traffic push, alive push/list, ETag, and msgpack for UniProxy users.
- Guest endpoints: guest config, payment notify, and Telegram webhook join
  approval/decline.
- Admin/staff route surface through the Rust admin service, including users,
  plans, payments, coupons, giftcards, notices, knowledge, tickets, stats,
  server management, theme config, mail test/send, Telegram webhook setup, and
  config save.
- Rust worker runtime with Apalis Redis queue and Apalis cron scheduler,
  including `traffic:update`, `check:order`, `check:commission`,
  `check:ticket`, `check:renewal`, `reset:traffic`, `reset:log`,
  `send:remindMail`, and `v2board:statistics`.
- Worker-side order handling, traffic aggregation, commission settlement,
  stale-ticket closure, auto-renewal, traffic reset, log cleanup, reminder
  mail dispatch/logging, daily statistics, Redis heartbeat, queue counters,
  and Redis distributed scheduler locks.
- Admin system status, queue stats, queue workload, queue masters, system log,
  and statistics record endpoints now return Rust worker/database-backed data
  instead of static placeholders.
- Contract/parity runner in `crates/contract`, with Docker Make targets for
  Laravel-vs-Rust HTTP contract checks and worker DB/Redis reconciliation.
- Payment gateway registration is centralized in Rust and guarded by tests so
  admin payment methods, checkout, and notify support cannot drift silently.

Verified in Docker on 2026-07-06:

- `make rust-check` passes: `cargo fmt --all --check` and
  `cargo clippy --workspace --all-targets --locked -- -D warnings`.
- `cargo test --workspace --locked` passes, including the Rust payment gateway
  matrix tests and worker scheduler/queue matrix tests.
- API smoke passed inside the Rust container for health, guest config, login,
  user info/stat, knowledge categories, client app version, staff plan fetch,
  admin plan fetch, and quick-login URL generation.
- Node smoke passed with a temporary V2Node and temporary local `server_token`
  for `/api/v2/server/config`, UniProxy config, UniProxy user JSON, UniProxy
  user msgpack, traffic push, alive push, and alive list. The temporary node,
  test stats, Redis keys, and test token were cleaned up afterward.
- Rust API and Rust worker runtime smoke passed inside Docker:
  `/healthz` returned healthy, the worker entered the Apalis runtime, cron ticks
  executed, scheduler heartbeat was written to Redis, job counters advanced, and
  scheduler locks were released after execution.
- Admin system smoke passed against the Rust API: `system/getSystemStatus`
  reported `schedule=true`, `system/getQueueStats` reported a running worker
  with recent jobs, and `system/getQueueWorkload` included executed Rust jobs.
- `make rust-contract` passes 59 Laravel-vs-Rust black-box contract scenarios
  across auth, guest config, client app endpoints, user profile/order/invite/
  ticket/server/knowledge/notice/traffic surfaces, admin config/users/orders/
  plans/servers/payments/knowledge/system queues, legacy-compatible status
  behavior, staff routing boundaries, and Rust subscription renderer flags.
- `make rust-worker-reconcile` passes all strict worker reconciliation checks:
  scheduler heartbeat, recent worker metrics, released scheduler locks, drained
  traffic Redis buffers, absent reset lock, opened paid orders, cancelled
  expired unpaid orders, closed stale answered tickets, and drained commission
  queue. The local seed run warns when yesterday's daily statistics row is
  absent, because the daily statistics cron may not have elapsed in the local
  fixture.
- `make rust-target-gate` passes for the configured Docker target data:
  Laravel-vs-Rust contract parity plus Rust worker reconciliation.
- `make rust-interaction-parity` passes with the frontend rebuilt against
  `VITE_API_BASE=http://rust-api:8080`, covering the configured focused user
  and admin desktop interaction shards.
- `docker compose -p v2board -f docker-compose.local.yml config` passes after
  separating API and worker workspaces.
- `git diff --check` passes.

Implementation scope that remains explicitly outside the built-in rewrite:

- The Rust backend does not load PHP, JavaScript, shell, or other non-Rust
  payment plugins. The first Rust release has no cross-language plugin runtime.
- Operator-added custom payment plugins outside the built-in Rust gateway map
  must be reimplemented as Rust gateway modules and compiled into the Rust
  workspace before cutover if a deployment depends on them.

Release gates still required before production cutover:

- If the production cutover uses data/config different from the configured
  Docker target, rerun `make rust-target-gate` or the equivalent contract and
  reconciliation gates against that copied target data before switching traffic.
- Built-in payment gateways must be exercised against the operator's sandbox or
  production-equivalent callback credentials before accepting real money.
- Production-like worker reconciliation must review traffic totals, order
  states, commission balances, subscription expiration, reminder mail logs, and
  statistics after the Rust worker has run on copied production-like data.

### Phase 1: Freeze Contracts

Create black-box contract tests that run the same seeded requests against
Laravel and Rust.

Minimum contract suites:

- Auth
- User/profile
- Commerce
- Subscription
- Node/server
- Admin
- Worker side effects

### Phase 2: Rust Infrastructure

Build the Rust workspace, Docker services, config loading, DB pool, Redis pool,
request tracing, legacy response envelope, legacy error conversion, form parser,
JSON parser, auth extractor, and i18n handling.

Minimum smoke targets:

- `GET /api/v1/guest/comm/config`
- `POST /api/v1/passport/auth/login`
- `GET /api/v1/user/info`

### Phase 3: Auth And User Domain

Implement login, register, forget password, token login, quick login,
user info, sessions, password change, reset security, subscribe info, Telegram
unbind, and profile updates.

### Phase 4: Commerce

Implement plans, coupons, orders, checkout, payment methods, payment notify,
deposits, reset packages, cancellation, and order open jobs.

### Phase 5: Subscription And Nodes

Implement subscribe rendering, node config, node users, traffic push, alive
tracking, msgpack, and ETag handling.

### Phase 6: Admin

Implement admin config, users, plans, servers, payments, coupons, giftcards,
notices, knowledge, tickets, stats, and system status.

### Phase 7: Workers

Replace all scheduled and queued Laravel jobs with Rust jobs. Run reconciliation
checks for traffic totals, order states, commission balances, subscription
expiration, and reminder mail behavior.

### Phase 8: Cutover

Cutover should happen during a maintenance window:

1. Enable maintenance mode.
2. Stop Laravel app, Horizon, and scheduler.
3. Back up MySQL.
4. Back up Redis.
5. Run Rust compatibility checks.
6. Run smoke checks.
7. Start Rust API and worker services.
8. Route traffic to Rust.
9. Monitor logs, queues, payments, node pushes, and order opens.
10. Keep Laravel image and backups available for rollback.

## Verification Gates

Before Rust can replace Laravel:

- Contract tests are green.
- Focused frontend behavior/interaction parity is green.
- Auth, order, payment, subscription, node, and admin smoke checks pass.
- Worker dry-run/reconciliation checks pass.
- No double-owned scheduled jobs remain.
- Payment notify replay is idempotent.
- Node traffic push is idempotent or safely accumulative according to existing
  behavior.
- Backups and rollback steps are tested.

## Explicit Non-Goals For First Release

- Do not redesign the frontend API.
- Do not redesign the database schema.
- Do not change payment callback contracts.
- Do not change subscription URL semantics.
- Do not change node API semantics.
- Do not remove legacy password support before all active users have migrated.
- Do not upgrade or redesign the admin frontend as part of this backend rewrite.

## Later V3 Cleanup

After the Rust backend is stable:

- Design a clean OpenAPI-first v3 API.
- Move from legacy response envelopes to typed REST or RPC responses.
- Normalize database schema.
- Replace PHP config file compatibility with native config tables only.
- Retire legacy password hashes after migration coverage is proven.
- Simplify subscription protocol generators.
- Remove Laravel oracle and compatibility shims.
