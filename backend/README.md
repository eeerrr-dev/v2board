# Backend

`backend/rust` is the only production backend runtime. It owns HTTP APIs, frontend delivery,
PostgreSQL transactions, ClickHouse analytics projection, Redis-backed sessions/leases, payments,
subscriptions, admin operations and background work. There is no PHP/Laravel runtime, dual-server
mode or native MySQL backend.

The pinned project under `references/wyx2685-v2board` is read-only compatibility evidence. The one-time
importer reads a complete Oracle MySQL 8.0/8.4 dump through a temporary, disposable MySQL 8 engine; it
never connects to or modifies the live old database. After the old site stops writing, the default topology
runs a separate loopback-only staging instance and the converter on that old host; a disposable migration VM
is the fallback when the old host lacks capacity. The new production host never runs MySQL. Staging is deleted
after the import, and no MySQL service belongs to the native runtime.
Reference code, schema or packaged frontend assets are never deployed.

## Runtime architecture

- PostgreSQL 18 is the only authoritative transactional database.
- ClickHouse 26.3 LTS stores derived, rebuildable analytics facts; aggregate projections are added
  only by an explicit, versioned ClickHouse migration after their retry semantics are proven.
- Redis stores session lookup, rate limits, leases, locks, worker heartbeat and bounded cache; it is
  not a business ledger.
- API and worker use separate PostgreSQL principals against the same database.
- ClickHouse schema migrator and outbox writer are separate least-privilege principals. The relay
  writer has raw-table `INSERT` plus the narrow `SELECT` needed to verify its immutable batches, but
  no DDL. A reader principal may be provisioned for a future analytics consumer, but the current API
  does not receive it.
- API/worker never synchronously dual-write PostgreSQL and ClickHouse. A PostgreSQL transaction writes
  the business result and typed outbox; the analytics relay publishes later.

Short ClickHouse failures do not synchronously block authentication, order or payment. Traffic
accounting continues inside the manifest-bound normal/soft PostgreSQL outbox budget; hard row/byte/
age/headroom pressure fails only analytics-producing traffic transactions before commit while the
relay keeps draining. Exact sampling and hysteresis reopen traffic automatically after recovery. See
[the persistence invariants](../docs/postgresql-clickhouse-invariants.md) for ownership, batch
integrity, replay and failure semantics.

The fixed single-node ClickHouse topology has serialized, crash-recoverable schema migration, exact
lineage/installation checks and manifest-bound raw/aggregate TTL. HA/Keeper is a separate availability
deployment choice; standalone evidence must not be extrapolated to a replicated topology. Runtime
secret isolation or one successful datastore test does not prove that the MySQL importer is complete.

## Workspace

```text
backend/rust/
  migrations-postgres/       current pre-release PostgreSQL baseline
  clickhouse-migrations/     independent ClickHouse schema lineage
  resources/rules/           embedded subscription rule templates
  crates/analytics/          typed events, PostgreSQL outbox and ClickHouse relay/schema
  crates/api/                Axum API and frontend delivery; no legacy source adapter
  crates/config/             native JSON/environment configuration
  crates/db/                 PostgreSQL-only runtime access
  crates/domain/             business rules and external integrations
  crates/provision/          mysql-import.v1 validation and fixed converter policy
  crates/lifecycle/          disposable MySQL-import CLI; production importer not linked
  crates/workers/            scheduler, durable work and analytics relay
  crates/contract/           route, SQL and production invariant gates
```

Mutable native state lives under `/var/lib/v2board`. Production uses distinct no-login
`v2board-api` and `v2board-worker` Unix users with separate writable directories. The frontend
release is a root-owned read-only input selected by `V2BOARD_FRONTEND_DIR`; production installs it
under `/opt/v2board/current/frontend`, while local Compose uses the `frontend-deploy` volume.

Built-in subscription templates are compiled into the API. `/var/lib/v2board/rules/custom.*` is
reserved for explicit operator overrides. Only a missing override selects the embedded default;
an unreadable, malformed or wrong-root override fails closed.

## Docker-first local workflow

Run setup, builds and tests through the repository Docker workflow:

```bash
make up
make doctor
make rust-check
make rust-test
make rust-integration
make rust-route-audit
make rust-worker-reconcile
make rust-target-gate
```

`make rust-integration` exercises the native PostgreSQL, ClickHouse and Redis lanes, including
ClickHouse lost-ack/outbox replay and live production invariants. It also prepares every generated
MySQL-import target query against a freshly migrated disposable PostgreSQL database and verifies the
derived and fixed-empty target tables. The single `mysql-import.v1` manifest, fixed row conversion
and loss policy are covered by Rust tests; the real dump → staging → PostgreSQL integration is not yet
linked, so production import is unavailable. No legacy Redis service exists.
`make rust-route-audit` reads the pinned reference only as contract evidence.
`make native-database-audit` rejects MySQL driver/dialect use in native runtime crates and in the current
inspection-only import graph. The one-time staging adapter cannot enter native runtime graphs.

Do not run host Cargo commands that create `target/` in the repository. The workspace targets Rust
1.97, Edition 2024 and Cargo resolver 3; `unsafe` is forbidden, CI denies warnings and validates the
locked dependency graph.

Local Compose pins PostgreSQL 18.4 and ClickHouse 26.3.17.4 images by content digest. It uses
plaintext only inside the isolated Docker network and one local account per service for convenience;
those shortcuts are not production topology.

No native version has been released or installed. PostgreSQL and ClickHouse migrations therefore form
one editable initial baseline; local-only history receives no compatibility migrations. Disposable
Docker volumes created from an older baseline may be recreated with `make reset` after confirming they
contain no needed development data.

## Production configuration and principals

The `mysql-import.v1` manifest derives two `configuration_source: "file_only"` documents:
`/var/lib/v2board/api/config.json` and `/var/lib/v2board/worker/config.json`. The API and worker use
explicit role loaders; missing, unknown, wrong-role, placeholder or invalid typed values are
rejected. Validation derives and checks both maps; the importer installs them only after data conversion
has passed. That production write path is not linked because the executor does not yet exist.

Long-running runtime configuration includes:

- API: its PostgreSQL `database_url`, the non-secret worker role name, and `redis_url`;
- worker: its PostgreSQL `database_url`, the non-secret API role name, `redis_url`, and the
  ClickHouse writer endpoint/database/credential.

Production PostgreSQL URLs require `sslmode=verify-full`, Redis requires `rediss://`, and ClickHouse
requires HTTPS. API and worker PostgreSQL usernames must differ. Bootstrap and schema/migration
credentials and the ClickHouse reader credential must not be retained in either runtime file.
Each file is `0600` and owned by its dedicated Unix user inside that user's `0700` directory.
Sharing a writable parent directory defeats atomic-rename isolation and is forbidden in production.

PostgreSQL connection pools are controlled by:

- `V2BOARD_DATABASE_MIN_CONNECTIONS`;
- `V2BOARD_DATABASE_MAX_CONNECTIONS`;
- `V2BOARD_DATABASE_ACQUIRE_TIMEOUT_SECONDS`;
- `V2BOARD_DATABASE_IDLE_TIMEOUT_SECONDS`;
- `V2BOARD_DATABASE_MAX_LIFETIME_SECONDS`.

The runtime sets PostgreSQL timezone UTC, `search_path=public` and transaction isolation
`READ COMMITTED`. Calendar/statistics/reset boundaries remain Asia/Shanghai business semantics.

## Native bare-metal release and schema jobs

Docker remains the reproducible local/CI builder, not the production runtime. Export
`Dockerfile.rust` target `native-release` to obtain the three native Linux binaries, validated
frontend tree, systemd units, release metadata and checksums. The intended bare-metal layout installs
the payload under `/opt/v2board/releases/<release-id>` and atomically updates
`/opt/v2board/current`; the server never builds the project and does not require Docker.

The first production installation is created only by the one-time MySQL importer. That importer is not
yet linked for production writes, so exporting or verifying a payload is not permission to hand-create
the initial database. Native release archive inspection is a deployment check and is not part of the
MySQL import manifest.

Before starting a native release, run serialized one-shot schema jobs with the exact release
artifacts and secrets:

```text
v2board-api migrate
v2board-analytics-schema
```

`v2board-api migrate` loads the API runtime config and, in production, additionally requires a
transient migration URL from the `v2board-migration-database-url` systemd credential or
`V2BOARD_MIGRATION_DATABASE_URL_FILE`. That principal must target the same PostgreSQL database,
use `sslmode=verify-full`, and differ from the API principal and declared worker principal. The MySQL
importer owns the empty-database baseline; the ordinary runtime command cannot substitute for the
converter or manufacture a partially initialized target.

`v2board-analytics-schema` uses only the following transient variables:

```text
V2BOARD_ENV=production
V2BOARD_CLICKHOUSE_SCHEMA_URL
V2BOARD_CLICKHOUSE_SCHEMA_DATABASE
V2BOARD_CLICKHOUSE_SCHEMA_USERNAME
V2BOARD_CLICKHOUSE_SCHEMA_PASSWORD_FILE
```

The schema password may instead use the `v2board-clickhouse-schema-password` systemd credential.
Direct value environment variables remain a local/CI compatibility path and must not be persisted
in a production unit or shell history.

It applies the independent ClickHouse ledger idempotently. Neither schema command converts a MySQL 8
dump or creates the full production principal topology.

Local Compose runs `rust-migrate` and `clickhouse-migrate` as one-shot services. The local PostgreSQL
migration job may create the documented development-only `admin@example.com` seed when
`V2BOARD_SEED_LOCAL=1`; production never enables that seed.

## MySQL import v1

There is one pre-release data path and one manifest example:
[mysql-import.v1](../docs/examples/mysql-import.v1.example.json). The operator stops the old site,
exports a complete Oracle MySQL 8.0/8.4 dump, loads it into a disposable staging MySQL, converts the
fixed retained rows into a brand-new PostgreSQL database, starts ClickHouse and Redis empty, generates
new role configs, verifies the result and starts the native services.

Staging is a temporary MySQL 8 engine on the stopped old production host, not a component of the new
machine. It is a second instance with a separate data directory or volume, port/socket, credentials and
loopback-only bind; it must not be created inside the source instance or mount the source data directory.
The converter writes outbound to the new PostgreSQL target with a temporary migration principal. Use a
disposable migration VM only when the old host lacks capacity, and remove staging data after success or
failure. The legacy MySQL source keeps its real `v2_*` table names; native PostgreSQL and ClickHouse targets
are unprefixed, using `users` and `orders` for the two PostgreSQL keyword conflicts.

The disposable CLI commands are:

```bash
v2board-lifecycle validate --manifest /secure/private/mysql-import.json
v2board-lifecycle inspect --manifest /secure/private/mysql-import.json
```

The manifest contains only `schema_version: 1`, `source`, `target` and `runtime`. It rejects duplicate,
unknown and missing keys, validates both role configs through their real typed parsers, and records the
exact manifest SHA-256. It contains secrets and must be a regular non-symlink file without Unix
group/world permissions.

`source` contains the dump path, dump SHA-256 and a loopback-only staging MySQL URL. It does not contain
a live old-MySQL URL, old Redis URL, Stripe credential, service unit, release archive or per-run loss
choices. `inspect` reads and hashes the dump but does not contact the old system, old Redis or Stripe and
does not mutate a target. There is currently no production write command or executor; the repository
cannot claim a production import until the real dump → staging → PostgreSQL path passes end-to-end tests.

The fixed converter preserves durable business rows, permanent user tokens, MySQL-persisted traffic and
balances. It discards old Redis, failed jobs, old nodes/routes/credentials, detailed legacy traffic,
operational logs, themes and runtime files. Stripe configuration and status 0/1 Stripe orders are
discarded; status 2/3/4 Stripe history is retained with provider bindings cleared. The importer never
contacts Stripe. Non-Stripe payment configuration and unfinished orders remain ordinary retained data.

The importer does not modify the old MySQL database. Staging is disposable conversion input, not a
recovery system. If conversion fails, the incomplete new target is discarded and the same simple import
is run again from the dump into a new empty target; no persistent intermediate state is kept.

`v2board-lifecycle` is absent from the long-running native release and is removed after accepted import.
API/worker dependency graphs contain neither the lifecycle crate nor `sqlx-mysql`. See the
[import guide](../docs/mysql-import.md) and
[fixed import contract](../docs/mysql-import-invariants.md).

## Analytics pipeline

The API durably accepts `traffic.reported.v1`; settlement workers produce
`traffic.accounted.v1` with `applied`, `stale_epoch` or `missing_user` outcome. Business updates,
idempotency state and the analytics outbox row commit in one PostgreSQL transaction.

The relay claims bounded rows with `FOR UPDATE SKIP LOCKED`, freezes an immutable delivery batch,
inserts to an append-only ClickHouse MergeTree table and verifies the entire batch before marking it
published in PostgreSQL. An uncertain acknowledgement retries the same ID/content/order. Partial or
conflicting writes are quarantined for generation rebuild, not patched into an authority claim.

PostgreSQL stores raw and charged bytes explicitly; ClickHouse never recomputes billing with Float.
Reported and accounted events remain separate facts. Runtime pruning is disabled: PostgreSQL keeps
published replay history until a second drilled archive/backup and generation-replay orchestrator
exist. That deliberate retention is recovery evidence, not a substitute for the still-missing
capacity budget and restore drill.

## Health and worker behavior

For the API, `GET /healthz` is liveness. `GET /readyz` requires PostgreSQL reachability, an exact
successful PostgreSQL migration ledger, Redis `PING`, and both immutable frontend entry points.
ClickHouse is deliberately not a core readiness dependency.

The worker has no HTTP listener. On bare metal it sends systemd `READY=1` only after bounded
PostgreSQL/Redis probes and exact migration-ledger validation, then refreshes `WATCHDOG=1` on the
same healthy cadence. Each scheduler/outbox/analytics loop records a heartbeat; unexpected loop
exit terminates the complete worker so systemd can restart it.

Relevant bounds include:

- `V2BOARD_WORKER_HEARTBEAT_INTERVAL_SECONDS` (1–300, default 10);
- `V2BOARD_WORKER_SHUTDOWN_TIMEOUT_SECONDS` (1–600, default 30);
- `V2BOARD_WORKER_CLEANUP_INTERVAL_SECONDS`;
- `V2BOARD_MAIL_RETENTION_DAYS`;
- `V2BOARD_IDEMPOTENCY_RETENTION_DAYS`.

Cleanup is bounded and never deletes analytics replay history, pending or leased work. Analytics
relay failure retries without making PostgreSQL/Redis worker health fail. A separate admission loop
samples exact pending rows/oldest age, heap/index/TOAST/total relation bytes, database bytes and
manifest-bound headroom; `/readyz` and `RUST_ANALYTICS_ADMISSION` expose the state without removing
unrelated API traffic. ClickHouse merge/part pressure and host disk/WAL still require infrastructure
monitoring.

## Authentication and operational contracts

New logins use 256-bit opaque bearer tokens; Redis stores only SHA-256 lookup keys with absolute TTL.
Password change, ban, staff/admin reset and explicit revocation delete lookups and advance the durable
session epoch. `mysql-import.v1` does not import sessions, so every user signs in again; the native
runtime contains no old JWT decoder, query/form authentication fallback or cutoff setting.

Privileged sessions have a shorter TTL and password step-up through
`POST /api/v1/passport/auth/stepUp`; disabling the gate is an explicit compatibility escape hatch,
not the production recommendation. Admin config reads redact stored credentials with `********`;
posting that sentinel preserves the existing secret.

Payment method config is immutable verification history. Rotate by archiving the old row and creating
a new one; callbacks can still resolve archived rows. Invalid, late, mismatched or under-specified
signed callbacks enter `payment_reconciliation` and never silently open an order.

New nodes use scoped `n1_...` credentials and a stable `Idempotency-Key` per retryable payload. A key
must never be reused with different bytes. Rotating one node credential advances only that node epoch.

Reset an existing administrator password with the `v2board-new-password` systemd credential or an
owner-only one-line file:

```bash
V2BOARD_NEW_PASSWORD_FILE=/run/credentials/v2board-new-password \
  v2board-api reset-admin-password '<email>'
```

The API retains established `/api/v1` and `/api/v2` external contracts where real clients or
integrations depend on them. Compatibility comes from the Rust contract tests and read-only reference
evidence, never from retaining an old runtime.
