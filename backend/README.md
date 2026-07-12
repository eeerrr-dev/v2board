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
make rust-route-audit
make rust-worker-reconcile
make rust-target-gate
```

`rust-route-audit` parses the pinned project under
`references/wyx2685-v2board` as a read-only contract reference and verifies that
every required API route is represented by Rust. Its five deliberately retired
endpoints (the package-theme API and pre-PaymentIntent Stripe key endpoint) are
an exact, self-validating list—not runtime fallbacks. The audit does not boot,
call, or deploy the reference backend. `worker-reconcile` checks only the live
Rust worker's MySQL/Redis outcomes.

Local Compose sets `V2BOARD_ENV=local` and `V2BOARD_SEED_LOCAL=1`. Its one-shot
`rust-migrate` service runs schema migrations and creates the minimal
`admin@example.com` development seed before either long-running Rust process is
started. API and worker replicas only open their connection pools; they never
race each other by running migrations during startup. Production sets
`V2BOARD_ENV=production`, requires an explicit non-development `APP_KEY`, and
never enables the local seed.

## Operations

The default production image command runs `v2board-api`; run the same image with
`/usr/local/bin/v2board-workers` for background work. Before rolling out API or
worker replicas, run the image once as an explicit migration job:

```bash
v2board-api migrate
```

The command is safe to repeat because SQLx records completed migrations, but it
must be serialized by the deployment platform. Database pool capacity and
lifetimes are controlled by `V2BOARD_DATABASE_MIN_CONNECTIONS`,
`V2BOARD_DATABASE_MAX_CONNECTIONS`,
`V2BOARD_DATABASE_ACQUIRE_TIMEOUT_SECONDS`,
`V2BOARD_DATABASE_IDLE_TIMEOUT_SECONDS`, and
`V2BOARD_DATABASE_MAX_LIFETIME_SECONDS`.

`GET /healthz` is the process liveness probe. `GET /readyz` fails closed until
MySQL is reachable and every applied migration version, success flag, and
checksum exactly matches the migrations embedded in the running binary, Redis
responds, and both immutable frontend entry points are present. Deploy traffic
only after `/readyz` succeeds.

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
