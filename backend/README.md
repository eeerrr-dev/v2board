# Backend

`backend/rust` is the only backend runtime. It owns the HTTP API, static frontend
delivery, database migrations, authentication, payments, subscriptions, admin
operations, scheduled work, and background jobs. There is no PHP/Laravel
runtime, dual-server mode, or live old-vs-new HTTP comparator.

## Workspace

```text
backend/rust/
  migrations/       SQLx schema and forward migrations
  resources/rules/  built-in subscription rule templates
  crates/api/       Axum API, frontend delivery, CLI operations
  crates/config/    native JSON/environment configuration
  crates/db/        SQLx access and local development seed
  crates/domain/    business rules and external integrations
  crates/provision/ versioned lifecycle spec validation and bounded read-only preflight
  crates/workers/   scheduler and background jobs
  crates/contract/  static reference route audit + worker reconciliation
```

Mutable native state lives under `/var/lib/v2board`. The frontend release is a
separate read-only input selected by `V2BOARD_FRONTEND_DIR`; production embeds it
at `/opt/v2board/frontend`, while local Compose mounts the `frontend-deploy`
volume at `/app/frontend-deploy`.

Built-in subscription templates under `backend/rust/resources/rules` are
compiled into the API binary. `/var/lib/v2board/rules` is reserved for explicit
`custom.*` operator overrides; only a missing override selects the embedded
default. An unreadable, malformed, or wrong-root override is a configuration
error rather than a silent fallback. Delete the invalid override to return to
the embedded default.

## Local verification

Run all Cargo commands through the repository Docker workflow:

```bash
make rust-check
make rust-test
make rust-integration
make rust-route-audit
make rust-worker-reconcile
make rust-target-gate
```

`rust-route-audit` parses the pinned project under
`references/wyx2685-v2board` as a read-only contract reference and verifies that
every required API route is represented by Rust. Its five deliberately retired
endpoints (the package-theme API and pre-PaymentIntent Stripe key endpoint) are
an exact, self-validating list—not runtime fallbacks. The audit does not boot,
call, or deploy the reference backend. `rust-integration` applies every migration
to a disposable database, prepares statically recoverable runtime SQL against
that schema, and exercises the accounting, invitation, ticket, payment, node,
auth-limit, lease, and worker-health invariants against real MySQL and isolated
Redis state. It drops that database and flushes only Redis DB 15 after the run.
`worker-reconcile` checks the live Rust worker's MySQL/Redis outcomes.

The workspace targets Rust 1.97 with Edition 2024 and Cargo resolver 3. Every
member inherits the workspace MSRV and lint policy; `unsafe` code is forbidden.
CI denies compiler warnings for ordinary builds as well as Clippy, verifies the
locked dependency graph, and checks RustSec advisories, licenses, sources, and
unused dependencies.

Rustls consumers use AWS-LC consistently: reqwest/jsonwebtoken select it
directly, lettre uses its `aws-lc-rs` provider feature, and SQLx uses
`tls-rustls-aws-lc-rs` with WebPKI roots. This avoids installing competing
ring/AWS-LC rustls providers in one process while retaining verified HTTPS,
SMTP STARTTLS/SMTPS, Redis `rediss://`, and MySQL TLS support.

The database layer currently uses runtime SQLx queries. `rust-integration`
parses the Rust AST, sends every statically recoverable `sqlx::query*` statement
through MySQL PREPARE after migrations, and pins an explicit per-file inventory
for dynamic SQL/QueryBuilder sites. A future compile-time SQL migration may
convert static statements to `query!`/`query_as!`, generate and commit workspace
`.sqlx` metadata, and then enable `cargo sqlx prepare --workspace --check` as one
atomic change; enabling that command before conversion would validate no runtime
queries.

Local Compose sets `V2BOARD_ENV=local` and `V2BOARD_SEED_LOCAL=1`. Its one-shot
`rust-migrate` service runs schema migrations and creates the minimal
`admin@example.com` development seed before either long-running Rust process is
started. API and worker replicas only open their connection pools; they never
race each other by running migrations during startup. Production sets
`V2BOARD_ENV=production`, requires an explicit non-development `APP_KEY`, and
never enables the local seed.

## Operations

### Current read-only lifecycle boundary

The only legacy lifecycle commands currently implemented are read-only:

```text
v2board-api provision validate --manifest <path>
v2board-api provision inspect --manifest <path>
v2board-api provision plan --manifest <path>
```

`validate` checks the secret-bearing file's type, size and Unix permissions,
strict JSON v2 structure, complete file-only AppConfig key/type inventory, fixed
`manual_only` / `logout_all` / `discard_ephemeral_after_fence` /
`maintenance_cutover` decisions, `assert_none` for Stripe and temporary
subscription tokens, the same AppConfig semantics used at runtime, and
additional v2 URL/range/path checks. It does not connect to a datastore and is
not a complete deployment validation.

`inspect` runs the online read-only compatibility inventory while the legacy
system may still be serving. Its scope is
`online_read_only_compatibility_inspection` and its verdict is
`compatible|blocked`. It does not enter maintenance. `plan` is the fenced final
read-only check and must be run only after the operator has explicitly chosen a
maintenance window and completed writer/reporter fencing, traffic and queue
drain, a consistent backup, and an isolated restore proof. Its scope is
`fenced_read_only_final_plan` and its verdict is
`ready_for_confirmation|blocked`.

Both checks connect to the declared source MySQL/Redis services, the target
MySQL `bootstrap_database_url`, and the target Redis logical DB. They do not
copy data, drain Redis, create the target database/account/schema, generate node
credentials, materialize `config.json`, create a journal, restore a backup, or
cut traffic over. MySQL versions are server-detected rather than supplied by
the operator: the source must be MySQL/Percona 5.7+, the bootstrap server must
be MySQL 8.4+, and both the database named by `application_database_url` and the
decoded application `'user'@'host'` account must not exist. The bootstrap
principal must be able to prove both facts through `information_schema` and
`mysql.user`. The operator does not pre-create them. A future `apply`, only after final
confirmation, must use the bootstrap credentials to create that database with
`utf8mb4/utf8mb4_unicode_ci`, create/restrict the application principal, and
install the native schema. `application_account_host` is the explicit client
source in MySQL's `'user'@'host'` account identity, not the DSN server host; only
an exact hostname, exact IP, or canonical IPv4 CIDR is accepted, never `%` or `_`
wildcards. `require_database_absent=true` and `require_account_absent=true` are
mandatory. Future creation must not use `IF NOT EXISTS`; only a journal-bound
resume may reuse objects created by that same operation. Bootstrap credentials must never enter runtime
configuration. Redis has no corresponding create-logical-database operation;
the selected target DB/namespace must already be empty and is never made empty
with `FLUSHDB`.

Both reports include a redacted `operation_id`, `manifest_binding_hmac_sha256`,
`report_sha256`, and `apply_available=false`. The manifest binding is an HMAC of
the exact secret-file bytes under an independent `lifecycle_audit_key`; changing
any endpoint, credential, backup reference, runtime value, or even JSON spacing
invalidates it. The audit key must also differ from app/node and target datastore
passwords. The report digest hashes the canonical payload, including that
binding and inspected server identities, while `report_sha256` is empty; it is
not the `sha256sum` of the final printed JSON.

The frozen orchestration is: online `inspect` passes; the operator first
confirms whether to enter maintenance; the system is fenced/drained/backed up
and restore-tested; final `plan` passes and displays the redacted creation,
conversion, downtime, proof, and rollback summary; then the operator confirms
the exact `operation_id + report_sha256`; only then may a future `apply` write a
journal and create/migrate the target. A changed or rerun report invalidates the
old confirmation. The repository has no `provision apply` command: the current
lifecycle boundary is `apply_available=false`. Static capability gaps are listed
in `implementation_blockers`, so the current online command fails closed as
`blocked/resolve_blockers`; it cannot return a successful `compatible` exit code
until apply and those proofs exist. The v2 runtime object covers only file-backed
AppConfig keys. Database-pool settings, worker lifecycle/retention settings, and
runtime/rules/frontend path bootstrap remain deployment inputs and are not yet
materialized or promoted. Operator-declared legacy cache/prefix/subscription
facts and unclassified source Redis keys are not yet machine-proven, so they
also remain apply blockers.

The bounded legacy path accepts only topology that looks standalone: MySQL
`server_uuid` values must be valid, non-nil, and different, with no detected
replication channel, group member, or binlog replica client; Redis endpoints
must have valid, different `run_id` values and report master role, zero connected
replicas, and cluster mode disabled. These checks prevent single-node inventory
from declaring a replica/cluster empty, but cannot prove offline or unregistered
replicas or the underlying storage failure domain. That stronger topology proof
therefore remains an `implementation_blocker`.

The legacy permanent subscription credential is MySQL `v2_user.token`, which
must retain its exact value. Redis `otp_`/`otpn_` entries are expiring temporary
subscription mappings, while a TOTP URL can be generated without writing any
Redis key and only later populate a short-lived `totp_` verification cache.
Conversely, the Redis `v2board_upload_traffic` and
`v2board_download_traffic` hashes contain traffic already accepted from nodes
but not yet committed to MySQL `u/d`; they are authoritative pending increments,
not disposable cache. See the manifest guide for the drain/window rules.

The tracked [v2 example](../docs/examples/legacy-migration.v2.example.json) is a
public, placeholder-bearing template and intentionally cannot be run directly.
Copy it outside the repository, review and fill every field rather than only the
`REPLACE` markers, generate a new operation UUID, keep it out of version control,
and restrict it to its owner:

```bash
cp docs/examples/legacy-migration.v2.example.json /secure/private/legacy-migration.json
chmod 600 /secure/private/legacy-migration.json
v2board-api provision validate --manifest /secure/private/legacy-migration.json
v2board-api provision inspect --manifest /secure/private/legacy-migration.json
# Only after compatible + explicit maintenance entry + fence/drain/backup/restore proof:
v2board-api provision plan --manifest /secure/private/legacy-migration.json
```

Percent-encode special characters in datastore URL credentials. Plaintext source
MySQL/Redis URLs are acceptable only over a trusted private network or encrypted
tunnel; production targets require authenticated TLS. These commands are review
and preflight aids under the
[detailed manifest guide](../docs/legacy-migration-manifest.md) and frozen
[installation and upgrade contract](../docs/upgrade-invariants.md), not a
supported migration workflow.

### Native schema runner

`Dockerfile.rust` has separate `production-api` and `production-worker` targets
with process-appropriate contents, logging, and health checks. The compatibility
target `production` aliases `production-api`. Build and deploy both targets;
before rolling out either, run the API image once as a serialized migration job:

`v2board-api migrate` is only the native SQLx schema runner for a confirmed empty
database or known native lineage. It is not a legacy adoption command and must
never be pointed at the pinned reference database. It also does not by itself
constitute the staged production fresh-install workflow required by the contract.

```bash
docker build --target production-api -t v2board-api -f Dockerfile.rust .
docker build --target production-worker -t v2board-worker -f Dockerfile.rust .
docker run --rm \
  -e DATABASE_URL='mysql://user:password@db.example.com:3306/v2board?ssl-mode=VERIFY_IDENTITY' \
  -e REDIS_URL='rediss://cache.example.com:6380/1' \
  -e APP_URL='https://app.example.com' \
  -e APP_KEY='<inject-at-least-32-random-bytes>' \
  -e V2BOARD_SERVER_TOKEN='<inject-a-different-32-byte-random-secret>' \
  -e V2BOARD_TRUSTED_PROXY_CIDRS='10.42.0.10/32' \
  v2board-api v2board-api migrate
```

The bracketed values are secret-manager placeholders, not reusable credentials.
They are deliberately rejected by production validation; replace them through
the deployment secret store before running the command.
Production startup rejects plaintext datastore URLs: MySQL must use
`ssl-mode=VERIFY_IDENTITY` (and `ssl-ca` when the provider CA is not in the
platform trust store), while Redis must use `rediss://`. The selected SQLx and
Redis rustls features support these URL forms. Local Compose deliberately uses
plaintext only inside its isolated development network.

The command is safe to repeat because SQLx records completed migrations, but it
must be serialized by the deployment platform. Database pool capacity and
lifetimes are controlled by `V2BOARD_DATABASE_MIN_CONNECTIONS`,
`V2BOARD_DATABASE_MAX_CONNECTIONS`,
`V2BOARD_DATABASE_ACQUIRE_TIMEOUT_SECONDS`,
`V2BOARD_DATABASE_IDLE_TIMEOUT_SECONDS`, and
`V2BOARD_DATABASE_MAX_LIFETIME_SECONDS`.

Forward migrations introduced by this hardening pass (versions 9–43) keep at
most one irreversible MySQL DDL statement per SQLx migration file. MySQL can
implicitly commit an `ALTER`/`CREATE`; splitting those statements means a later
metadata-lock, storage, or compatibility failure resumes at its own unrecorded
version instead of replaying an already committed earlier DDL. The production
invariant gate rejects any future version that violates this boundary.

Payment methods are immutable verification versions. Rotating a provider or
secret means archiving the old row and creating a new one; archive never removes
the old callback UUID or verification material. New checkout/form/list paths
exclude archived versions, while authenticated callbacks continue resolving
them. Operators can inspect every unresolved callback, including unknown order
numbers, through `GET /api/v1/{admin_path}/order/reconciliation/fetch` (password
step-up required) and resolve an item with the existing `order/update`
`reconciliation_id` flow.

Migration 0042 appends the nullable order callback digest with
`ALGORITHM=INSTANT` and does not scan or rewrite the order table. Existing paid
rows retain a NULL digest until an exact provider replay, when the locked row is
backfilled lazily. A MySQL release that cannot perform this metadata-only change
fails the migration instead of silently copying a large production table.
Migration 0043 bridges a rolling deployment: if an older API updates only the
legacy callback column, a database trigger derives its matching digest; newer
writers that submit both the bounded label and full raw digest retain the latter.
The serialized migration principal therefore needs MySQL `ALTER` and `TRIGGER`
privileges before this rollout. When binary logging is enabled, MySQL also
requires either an administrator-capable migration principal or the DBA-managed
`log_bin_trust_function_creators=ON` setting; without one of those it rejects
trigger creation with error 1419. A missing privilege or server setting fails
migration startup closed, before application traffic is admitted. The canonical
local Docker workflow enables that setting so development migrations can remain
scoped to the non-root `v2board` account.

For the API, `GET /healthz` is the process liveness probe and `GET /readyz` fails closed until
MySQL is reachable and every applied migration version, success flag, and
checksum exactly matches the migrations embedded in the running binary, Redis
responds, and both immutable frontend entry points are present. Deploy traffic
only after `/readyz` succeeds.

The worker has no HTTP listener. Its image health check reads a timestamp file
written only after bounded MySQL and Redis probes succeed. Each scheduler/outbox
loop also refreshes its own field in the Redis
`RUST_WORKER_LOOP_HEARTBEAT_AT` hash. An unexpected loop exit or panic terminates
the worker so the deployment platform restarts the complete process instead of
leaving a partially dead scheduler. Shutdown is bounded to 30 seconds by
default; tune it with `V2BOARD_WORKER_SHUTDOWN_TIMEOUT_SECONDS` (1–600). The
heartbeat interval is controlled by
`V2BOARD_WORKER_HEARTBEAT_INTERVAL_SECONDS` (1–300); keep the image health max
age greater than twice this interval.

The outbox loop performs bounded retention cleanup every six hours. Terminal
mail-outbox rows, scrubbed/empty mail batches, and mail logs are retained for 90
days by default (`V2BOARD_MAIL_RETENTION_DAYS`, 1–3650). Applied traffic-report
idempotency records use
`V2BOARD_IDEMPOTENCY_RETENTION_DAYS` (default 90, 1–3650). A client must never
replay an idempotency key after that configured window. Set
`V2BOARD_WORKER_CLEANUP_INTERVAL_SECONDS` (60–604800) to control cleanup
frequency. Cleanup deletes at most 1,000 rows from each table per pass and never
deletes pending or leased work; one cleanup cycle runs at most ten passes per
table so a backlog is drained without one unbounded transaction.

A scheduled business-task error (traffic settlement, order opening,
commission, renewal, reset, statistics, ticket, or reminder selection) is fatal
to that loop and therefore to the supervised worker process. It is recorded in
Redis metrics first. This makes a broken target table, permission, or invariant
observable as a restart/failure instead of allowing dependency probes to report
a partially dead worker as healthy.

Before migration 3 adds the one-unfinished-order-per-user constraint, the
migrator checks legacy data and refuses to continue if any user has multiple
orders in status `0` or `1`. It never guesses which order to cancel or rewrites
payment state. Inspect affected users with:

```sql
SELECT user_id, COUNT(*) AS unfinished_orders
FROM v2_order
WHERE status IN (0, 1)
GROUP BY user_id
HAVING COUNT(*) > 1
ORDER BY user_id;
```

Back up the database, then complete or explicitly resolve each affected order
according to its real payment state before rerunning `v2board-api migrate`.

Migrations 10 and 12 also stop before their first persistent DDL statement when
legacy data would violate a new uniqueness or relational invariant. The MySQL
duplicate-key error names
`business_invariant_preflight_failed` or
`relational_integrity_preflight_failed`; it does not identify the offending row.
Run the matching diagnostics below against a backup/read replica before changing
production data.

Migration 10 duplicate/state diagnostics:

```sql
SELECT 'coupon.code' AS invariant_name, code AS conflicting_value, COUNT(*) AS row_count
FROM v2_coupon GROUP BY code HAVING COUNT(*) > 1
UNION ALL
SELECT 'giftcard.code', code, COUNT(*)
FROM v2_giftcard GROUP BY code HAVING COUNT(*) > 1
UNION ALL
SELECT 'invite_code.code', code, COUNT(*)
FROM v2_invite_code GROUP BY code HAVING COUNT(*) > 1
UNION ALL
SELECT 'payment.driver_uuid', CONCAT(payment, ':', uuid), COUNT(*)
FROM v2_payment GROUP BY payment, uuid HAVING COUNT(*) > 1
UNION ALL
SELECT 'ticket.one_open_per_user', CAST(user_id AS CHAR), COUNT(*)
FROM v2_ticket WHERE status = 0 GROUP BY user_id HAVING COUNT(*) > 1;
```

Migration 12 scalar relationship diagnostics (`child_id` is the row to
investigate; deposit orders with `plan_id = 0` are intentionally excluded):

```sql
SELECT 'plan.group_id' AS relation_name, p.id AS child_id, p.group_id AS missing_id
FROM v2_plan p LEFT JOIN v2_server_group g ON g.id = p.group_id WHERE g.id IS NULL
UNION ALL
SELECT 'user.plan_id', u.id, u.plan_id
FROM v2_user u LEFT JOIN v2_plan p ON p.id = u.plan_id
WHERE u.plan_id IS NOT NULL AND p.id IS NULL
UNION ALL
SELECT 'user.group_id', u.id, u.group_id
FROM v2_user u LEFT JOIN v2_server_group g ON g.id = u.group_id
WHERE u.group_id IS NOT NULL AND g.id IS NULL
UNION ALL
SELECT 'order.user_id', o.id, o.user_id
FROM v2_order o LEFT JOIN v2_user u ON u.id = o.user_id WHERE u.id IS NULL
UNION ALL
SELECT 'order.plan_id', o.id, o.plan_id
FROM v2_order o LEFT JOIN v2_plan p ON p.id = o.plan_id
WHERE o.plan_id <> 0 AND p.id IS NULL
UNION ALL
SELECT 'giftcard.plan_id', c.id, c.plan_id
FROM v2_giftcard c LEFT JOIN v2_plan p ON p.id = c.plan_id
WHERE c.plan_id IS NOT NULL AND p.id IS NULL
UNION ALL
SELECT 'invite_code.user_id', c.id, c.user_id
FROM v2_invite_code c LEFT JOIN v2_user u ON u.id = c.user_id WHERE u.id IS NULL
UNION ALL
SELECT 'ticket.user_id', t.id, t.user_id
FROM v2_ticket t LEFT JOIN v2_user u ON u.id = t.user_id WHERE u.id IS NULL
UNION ALL
SELECT 'ticket_message.ticket_id', m.id, m.ticket_id
FROM v2_ticket_message m LEFT JOIN v2_ticket t ON t.id = m.ticket_id WHERE t.id IS NULL
UNION ALL
SELECT 'giftcard_redemption.user_id', r.giftcard_id, r.user_id
FROM v2_giftcard_redemption r LEFT JOIN v2_user u ON u.id = r.user_id WHERE u.id IS NULL
UNION ALL
SELECT 'traffic_report_item.user_id', 0, i.user_id
FROM v2_server_traffic_report_item i LEFT JOIN v2_user u ON u.id = i.user_id
WHERE u.id IS NULL;
```

Node group JSON must be a non-empty array of positive existing group IDs. This
query reports malformed members and missing groups across every node table:

```sql
WITH nodes AS (
    SELECT 'shadowsocks' AS node_type, id, group_id FROM v2_server_shadowsocks
    UNION ALL SELECT 'vmess', id, group_id FROM v2_server_vmess
    UNION ALL SELECT 'trojan', id, group_id FROM v2_server_trojan
    UNION ALL SELECT 'tuic', id, group_id FROM v2_server_tuic
    UNION ALL SELECT 'hysteria', id, group_id FROM v2_server_hysteria
    UNION ALL SELECT 'vless', id, group_id FROM v2_server_vless
    UNION ALL SELECT 'anytls', id, group_id FROM v2_server_anytls
    UNION ALL SELECT 'v2node', id, group_id FROM v2_server_v2node
), members AS (
    SELECT n.node_type, n.id, n.group_id, member.value AS member
    FROM nodes n
    LEFT JOIN JSON_TABLE(
        IF(JSON_VALID(n.group_id) AND JSON_TYPE(n.group_id) = 'ARRAY', n.group_id, JSON_ARRAY()),
        '$[*]' COLUMNS (value JSON PATH '$' NULL ON ERROR)
    ) member ON TRUE
)
SELECT m.node_type, m.id, m.group_id, m.member
FROM members m
LEFT JOIN v2_server_group g
  ON g.id = CAST(JSON_UNQUOTE(m.member) AS UNSIGNED)
WHERE IF(
        JSON_VALID(m.group_id),
        JSON_TYPE(m.group_id) <> 'ARRAY' OR JSON_LENGTH(m.group_id) = 0,
        TRUE
      )
   OR m.member IS NULL
   OR JSON_TYPE(m.member) NOT IN ('INTEGER', 'STRING')
   OR JSON_UNQUOTE(m.member) NOT REGEXP '^[1-9][0-9]*$'
   OR g.id IS NULL;
```

Do not delete duplicate or orphan rows blindly. Resolve them according to the
real payment, ownership, redemption, and support history, take another backup,
rerun the diagnostics until they return no rows, and only then retry the
serialized migration job.

Reset an administrator password with an ephemeral secret environment variable:

```bash
V2BOARD_NEW_PASSWORD='<secret>' v2board-api reset-admin-password '<email>'
```

### Authentication session migration

New logins receive 256-bit opaque bearer tokens. Redis stores only a SHA-256
token lookup key with an absolute TTL (`V2BOARD_AUTH_SESSION_TTL_SECONDS`, 30
days by default); user session lists never return bearer tokens. Password
changes, bans, staff/admin resets, and explicit session revocation delete those
lookups in addition to incrementing the durable database session epoch.

JWT fallback is disabled by default. To preserve pre-migration Rust sessions for
a deliberate, bounded rollout, set `V2BOARD_LEGACY_JWT_CUTOFF_UNIX` to a fixed
future Unix timestamp before deployment. The API records that absolute cutoff in
Redis with `SET NX` and always enforces the earlier of the stored and configured
values, so a restart, configuration change, or lost Redis key cannot move the
window later. Keep the value at `0` for an immediate opaque-token cutover.
Deleting all Redis session state intentionally signs every device out.

Admin and staff sessions use the independently configurable
`V2BOARD_PRIVILEGED_AUTH_SESSION_TTL_SECONDS` (30 minutes by default), which must
remain shorter than the ordinary session TTL. A compatible step-up endpoint is
available at `POST /api/v1/passport/auth/stepUp`: send the current Authorization
token plus `password`, then send the returned opaque token in
`x-v2board-step-up`. Production enables this gate by default. A session created
by a real password login satisfies the first
`V2BOARD_PRIVILEGED_STEP_UP_TTL_SECONDS` window (30 minutes by default), matching
the default privileged-session lifetime so the bundled admin never enters a
write-only-403 state. A deliberately longer session must use the endpoint/header
after that window or the operator must re-authenticate. Token/quick
login sessions do not receive that grace period. Setting
`V2BOARD_PRIVILEGED_STEP_UP_ENABLE=false` is an explicit compatibility escape
hatch with residual account-takeover risk, not the production recommendation.
Password re-verification is a deployable first step, not phishing-resistant
MFA; a subsequent WebAuthn/TOTP implementation should issue the same bound
step-up token only after the second factor, preserving the mutation gate.

Admin configuration reads never return stored credentials. Configured server,
SMTP, Telegram, reCAPTCHA, and payment-provider secrets are represented by the fixed
`********` sentinel; posting that sentinel preserves the existing value, while
posting a different non-empty value rotates it. Telegram webhook setup uses the
stored token, so the browser never needs to receive it again. Node credentials
and generated install commands remain intentionally retrievable for deployment,
but `server/manage/getNodes` additionally requires the recent-password gate.

Production enables registration IP limiting by default; local development does
not. `V2BOARD_REGISTER_LIMIT_BY_IP_ENABLE` remains an explicit override. The
limiter is an abuse-control layer, not a substitute for optional email
verification or reCAPTCHA.

Browser CORS is deny-by-default beyond the canonical `APP_URL` origin. Add
explicit HTTP(S) origins with `V2BOARD_CORS_ALLOWED_ORIGINS` (comma separated)
for trusted third-party browser clients. Wildcards, URL paths, credentials,
queries, and fragments are rejected; non-browser clients without an Origin
header are unaffected, and credentialed CORS remains disabled.

Production also defaults `force_https` to true and refuses to start without a
canonical HTTPS `APP_URL` and a narrowly scoped `trusted_proxy_cidrs`/
`V2BOARD_TRUSTED_PROXY_CIDRS` entry. Only those proxy peers may assert the
original HTTPS transport; application responses then include HSTS. Liveness and
readiness probes are intentionally exempt so the orchestrator can probe the
loopback HTTP listener. An explicit `V2BOARD_FORCE_HTTPS=false` override is for
specialized TLS-in-process/private deployments and transfers transport security
responsibility to the operator.

Authenticated payment callbacks that arrive after cancellation/expiry, refer
to an unknown local trade, use a second provider transaction id, fail the
locked order/payment/user binding, or carry a settled amount different from the
exact payable amount are recorded in
`v2_payment_reconciliation`. Admin order-list rows expose the unresolved count,
order detail exposes the complete ledger, and the statistics summary exposes
the unresolved count and amount. Resolve an investigated entry through the
existing admin `order/update` route with `reconciliation_id` and `resolution`;
the backend records the acting administrator and timestamp, rejects conflicting
second resolutions, and treats an identical retry idempotently. A resolution is
an audit acknowledgement only—it never silently opens, refunds, or rewrites the
order.

Once a payment method has any order history, the admin API refuses to
physically delete its routing UUID or change its driver/verification config;
disable it and create a new method to rotate credentials. Display metadata and
fees remain editable. This preserves the exact historical verification
material needed to authenticate a callback that arrives after cancellation,
expiry, or a long delivery delay.

`EPay`, `MGate`, `BEasyPaymentUSDT`, and `WechatPayNative` are explicitly marked
as legacy-MD5 protocol integrations in admin payment metadata/forms. Their
signatures remain for upstream compatibility, but operators must use HTTPS and
prefer a provider with HMAC or asymmetric signatures for new deployments.

### Node credential rollout

New production deployments use node-scoped `n1_...` credentials from the
start: read each node's derived credential from admin
`server/manage/getNodes`, configure it only on that reporter, and attach a
stable `Idempotency-Key` (or `report_id`/`idempotency_key`) to every retryable
traffic batch. Never reuse a key for different payload bytes.

The pinned-reference migration uses a maintenance cutover rather than an online
global-token compatibility window. Stop the old API, workers, scheduler, and all
node reporters; drain queues and reconcile both legacy Redis traffic hashes
before conversion. Keep `V2BOARD_SERVER_LEGACY_TOKEN_ENABLE=false` and
`V2BOARD_SERVER_REQUIRE_IDEMPOTENCY_KEY=true` on the target, fetch each node's
scoped token, then update its endpoint/token and stable batch IDs while reporters
remain stopped. Verify config/users retrieval and exactly-once traffic charging
for every node before resuming it. The current read-only preflight reports node
inventory and this requirement but performs none of those operations.

For ordinary post-migration credential rotation, setting `rotate_credential=1`
on the existing admin server save/update contract increments that node's
credential epoch and immediately invalidates only that node's previous token;
fetch and deploy the replacement without rotating the master token or disrupting
sibling nodes.

The API keeps the established `/api/v1` and `/api/v2` external contracts where
real clients and integrations depend on them. Compatibility decisions come from
those external contracts and the read-only reference—not from retaining an old
runtime.

### MySQL 8.0 to 8.4 authentication migration

MySQL 8.4 disables `mysql_native_password`. Before moving an existing database
volume or managed instance from 8.0 to 8.4, back it up and convert every account
used by V2Board while the old server is still available:

```sql
SELECT user, host, plugin FROM mysql.user ORDER BY user, host;
ALTER USER 'v2board'@'%' IDENTIFIED WITH caching_sha2_password BY '<rotated-password>';
```

Convert the operational/root account in the same maintenance window, update
`DATABASE_URL`, verify the Rust API can authenticate, and only then complete the
8.4 rollout. If 8.4 was started before the conversion and all administrative
accounts are still native-password accounts, enable
`--mysql-native-password=ON` for one isolated maintenance start, perform the
`ALTER USER` statements, and immediately restart without that flag. Do not keep
the retired plugin enabled as a compatibility fallback.
