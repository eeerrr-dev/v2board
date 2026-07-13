# Backend

`backend/rust` is the only production backend runtime. It owns HTTP APIs, frontend delivery,
PostgreSQL transactions, ClickHouse analytics projection, Redis-backed sessions/leases, payments,
subscriptions, admin operations and background work. There is no PHP/Laravel runtime, dual-server
mode or native MySQL backend.

The pinned project under `references/wyx2685-v2board` is read-only compatibility evidence. A complete
Oracle MySQL 8.0/8.4 dump may be read only after age decryption into an operation-owned isolated restore;
the importer never connects to a live old database. Reference code, schema or packaged frontend assets
are never deployed.

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
secret isolation or one successful datastore test does not open a lifecycle apply gate.

## Workspace

```text
backend/rust/
  migrations-postgres/       PostgreSQL final-state baseline and forward SQLx migrations
  clickhouse-migrations/     independent ClickHouse schema lineage
  resources/rules/           embedded subscription rule templates
  crates/analytics/          typed events, PostgreSQL outbox and ClickHouse relay/schema
  crates/api/                Axum API and frontend delivery; no legacy source adapter
  crates/config/             native JSON/environment configuration
  crates/db/                 PostgreSQL-only runtime access
  crates/domain/             business rules and external integrations
  crates/provision/          strict legacy-v5 validation, archive inspection, loss policy and closed capability
  crates/lifecycle/          disposable archive-inspection/cold-import CLI; production importer not linked
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
ClickHouse lost-ack/outbox replay and live production invariants. The unique v5 manifest/archive/loss
policy is covered by Rust tests; encrypted-dump isolated-restore conversion and pre-activation cleanup
integration are intentionally absent and keep production apply closed. No legacy Redis service exists.
`make rust-route-audit` reads the pinned reference only as contract evidence.
`make native-database-audit` rejects MySQL driver/dialect use in native runtime crates and in the current
inspection-only lifecycle graph. A future isolated restore adapter cannot enter native runtime graphs.

Do not run host Cargo commands that create `target/` in the repository. The workspace targets Rust
1.97, Edition 2024 and Cargo resolver 3; `unsafe` is forbidden, CI denies warnings and validates the
locked dependency graph.

Local Compose pins PostgreSQL 18.4 and ClickHouse 26.3.17.4 images by content digest. It uses
plaintext only inside the isolated Docker network and one local account per service for convenience;
those shortcuts are not production topology.

The native PostgreSQL migration lineage is still pre-release and was consolidated in place before
its first supported production release. Disposable local Docker PostgreSQL volumes created by older
local-only commits are therefore not checksum-compatible with the current baseline. After confirming
that such a volume contains no data you need, recreate it with `make reset`; this is a local development
instruction, not a production upgrade procedure.

## Production configuration and principals

The accepted legacy-v5 lifecycle manifest derives two `configuration_source: "file_only"` documents:
`/var/lib/v2board/api/config.json` and `/var/lib/v2board/worker/config.json`. The API and worker use
explicit role loaders; missing, unknown, wrong-role, placeholder or invalid typed values are
rejected. Validation derives and checks both maps; the future cold importer must install them atomically
inside its single apply. That production write path is not linked while the typed gate is closed.

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

The repository does not yet ship a general production fresh-install, native-upgrade or rollback
executor. Legacy v5 currently supplies strict archive inspection and a typed closed capability, not a
linked production cold importer. Exporting or verifying the payload is therefore not permission to
hand-install it or reinterpret the documented target layout as a supported upgrade.

Before starting a native release, run serialized one-shot schema jobs with the exact release
artifacts and secrets:

```text
v2board-api migrate
v2board-analytics-schema
```

`v2board-api migrate` loads the API runtime config and, in production, additionally requires a
transient migration URL from the `v2board-migration-database-url` systemd credential or
`V2BOARD_MIGRATION_DATABASE_URL_FILE`. That principal must target the same PostgreSQL database,
use `sslmode=verify-full`, and differ from the API principal and declared worker principal. Ordinary
production migrate rejects an empty database and only accepts a valid native migration prefix with
an active installation. While the lifecycle apply production gate remains closed it additionally requires the ledger to be
exactly current, so ordinary migrate cannot apply production initialization or forward-upgrade DDL.

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

It applies the independent ClickHouse ledger idempotently. Neither native schema command converts a
legacy MySQL 8 database, creates the full production principal topology or constitutes a
supported lifecycle apply.

Local Compose runs `rust-migrate` and `clickhouse-migrate` as one-shot services. The local PostgreSQL
migration job may create the documented development-only `admin@example.com` seed when
`V2BOARD_SEED_LOCAL=1`; production never enables that seed.

## Lifecycle schema v5

The current CLI accepts only the archive-first legacy-v5 schema. Fresh-install and native-upgrade v3
files remain future design examples and are rejected by both `validate` and `inspect`:

- [fresh install example](../docs/examples/fresh-install.v3.example.json);
- [legacy migration v5 example](../docs/examples/legacy-migration.v5.example.json);
- [native upgrade example](../docs/examples/native-upgrade.v3.example.json).

The disposable CLI commands are:

```bash
v2board-lifecycle validate --manifest /secure/private/operation.json
v2board-lifecycle inspect --manifest /secure/private/operation.json
v2board-lifecycle inspect-release-archive --archive /secure/native-release.tar.gz --release-id <id> --sha256 <sha256>
v2board-lifecycle apply --manifest /secure/private/operation.json
```

`inspect-release-archive` is mutation-free and uses the same archive contract as lifecycle admission;
it neither replaces the manifest-bound archive digest nor authorizes a migration. Legacy v5 has no
`authorize` or `resume` stage. Its typed production capability currently keeps `apply` fail-closed
before any target write.

`v2board-lifecycle` is deliberately absent from the long-running native release. It is staged only
for the one migration operation and removed by the operator after accepted activation. Tool removal is
not migration proof and does not depend on live-source reachability. API/worker dependency graphs contain
neither the lifecycle crate nor `sqlx-mysql`.

The manifest contains secrets and must be a 1-byte-to-2-MiB regular non-symlink file with no Unix
group/world permissions. It rejects duplicate/unknown/missing keys and binds the exact file bytes to
an independent lifecycle audit key using the cold-import v5 HMAC domain.

Legacy v5 uses its own cold-import HMAC domain; authorization, stage-journal and receipt material from
retired schemas is not accepted.

`validate` is offline syntax/semantic validation. Legacy `inspect` safely opens and hashes the encrypted
dump, age identity and native release; it validates static isolated-restore/target declarations but does
not connect to the restore database, a live target, old MySQL or old Redis. There is no public `plan`,
authorization-file or checkpoint-resume shortcut.

The typed production capability rejects legacy `apply` before writes and reports
`apply_available=false` with the incomplete importer/operation-owned-cleanup blocker. Passing
`validate` or archive inspection is not permission to create targets, copy data or cut over manually.

### Fresh install

This is a future target contract; the current lifecycle binary rejects fresh-install manifests.

The target spec declares:

- a PostgreSQL 18 bootstrap URL plus distinct migration/API/worker URLs;
- absent PostgreSQL database and migration/API/worker roles, `C.UTF-8` collation/ctype and external
  `pg_hba`/network evidence; the bootstrap role already exists and has reviewed create privileges;
- a ClickHouse 26.3 endpoint plus an existing bootstrap principal and absent target database plus
  schema/writer/reader principals, standalone topology, least-privilege evidence and explicit
  raw/aggregate retention;
- an empty TLS Redis logical DB/namespace;
- one complete manually reviewed runtime section that derives strict API and worker file-only maps.

The operator supplies bootstrap connections; an enabled journaled apply creates the target databases
and long-lived principals. Do not pre-create those target objects or use `IF NOT EXISTS` to hide an
unknown prior operation.

### Legacy reference migration

The only supported source identity is pinned commit
`7e77de9f4873b317157490529f7be7d6f8a62421`. The operator stops the old site first, exports the
complete Oracle MySQL 8.0/8.4 database, age-encrypts it and binds the encrypted bytes by SHA-256.
MySQL 5.7, Percona, MariaDB and compatible proxies are rejected when the dump is restored into the
isolated inspection database. The target is always fresh PostgreSQL 18 + ClickHouse 26.3 + Redis.

The importer has no live source MySQL URL, legacy Redis URL/prefix, source systemd unit or datastore
fence credential. It never reads old Redis. Pending traffic, queues/failed work, sessions, OTP,
temporary subscription links, caches, locks and Horizon metadata are all covered by the explicit
`discard_all_without_inspection` decision. Permanent `v2_user.token`, MySQL-persisted `u/d`, balance,
plans and other retained business values are still verified.

Stripe is a deliberate row-level exception: Stripe payment configurations and status 0/1 Stripe
orders are excluded. Status 2/3/4 Stripe order history remains with `payment_id=NULL` and
`callback_no=NULL`; user balances are preserved exactly, with no refund or compensation. The importer
does not call Stripe or inspect provider-side objects. Non-Stripe payment configuration and unfinished
orders remain ordinary retained business data.

Nodes, routes, credentials, per-user/per-node traffic details and old request/mail logs do not enter
the native target. New runtime files are rebuilt from the manifest rather than copied from `.env`, PHP,
theme or custom scripts. ClickHouse starts at an empty native event epoch.

This is one offline cold import, not CDC or coexistence. Before activation, failure cleanup removes the
operation-owned isolated restore and every unactivated target object, then a new operation starts from
the exact same encrypted dump. There is no resume. After activation, recovery uses PostgreSQL/
ClickHouse backup or forward repair, never a restarted MySQL service.

See the [legacy v5 guide](../docs/legacy-migration-manifest.md) and
[lifecycle contract](../docs/upgrade-invariants.md). Production apply remains disabled until the
archive-first integration gates and final security audit pass.

### Native upgrade

This is a future target contract; the current lifecycle binary rejects native-upgrade manifests.

The manifest binds installation UUID, current/target build IDs, monotonic PostgreSQL and ClickHouse
schema epochs, runtime principals and maintenance strategy. Destructive changes, TTL shortening,
drops and repartitions are explicit impact lists. A destructive plan additionally requires operator
allowance, impact review, backup/restore proof and a second confirmation bound to a prior v3 report.

Native inspection and apply are not implemented; the CLI rejects the manifest before reporting it as
valid.

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
session epoch. The selected legacy migration performs a full logout; the native runtime contains no
legacy JWT decoder, query/form authentication fallback or cutoff setting.

Privileged sessions have a shorter TTL and password step-up through
`POST /api/v1/passport/auth/stepUp`; disabling the gate is an explicit compatibility escape hatch,
not the production recommendation. Admin config reads redact stored credentials with `********`;
posting that sentinel preserves the existing secret.

Payment method config is immutable verification history. Rotate by archiving the old row and creating
a new one; callbacks can still resolve archived rows. Invalid, late, mismatched or under-specified
signed callbacks enter `v2_payment_reconciliation` and never silently open an order.

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
