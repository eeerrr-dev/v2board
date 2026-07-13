# Backend

`backend/rust` is the only production backend runtime. It owns HTTP APIs, frontend delivery,
PostgreSQL transactions, ClickHouse analytics projection, Redis-backed sessions/leases, payments,
subscriptions, admin operations and background work. There is no PHP/Laravel runtime, dual-server
mode or native MySQL backend.

The pinned project under `references/wyx2685-v2board` is read-only compatibility evidence. Its
Oracle MySQL 8.0/8.4 database may be inspected only by the legacy provision source adapter; reference code,
schema or packaged frontend assets are never deployed.

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
  crates/provision/          lifecycle v3/v4 validation, inspection and journaled executors
  crates/lifecycle/          disposable CLI and the only legacy MySQL source-adapter binary
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

`make rust-integration` starts isolated PostgreSQL, ClickHouse, Redis and pinned Oracle MySQL 8.4
targets. It exercises the complete legacy MySQL inspection surface, exact JSON boundaries, atomic
Redis traffic freezing, age-encrypted `mysqldump`/restore and rematerialization, PostgreSQL traffic
fold/ACL checks, ClickHouse lost-ack/outbox replay and the live production invariants.
`make rust-route-audit` reads the pinned reference only as contract evidence.
`make native-database-audit` rejects MySQL driver/dialect use in native runtime crates while allowing
the isolated provision source adapter and its Docker-only integration image.

Do not run host Cargo commands that create `target/` in the repository. The workspace targets Rust
1.97, Edition 2024 and Cargo resolver 3; `unsafe` is forbidden, CI denies warnings and validates the
locked dependency graph.

Local Compose pins PostgreSQL 18.4 and ClickHouse 26.3.17.4 images by content digest. It uses
plaintext only inside the isolated Docker network and one local account per service for convenience;
those shortcuts are not production topology.

## Production configuration and principals

One lifecycle manifest (fresh/native v3 or legacy v4) derives two `configuration_source: "file_only"` documents:
`/var/lib/v2board/api/config.json` and `/var/lib/v2board/worker/config.json`. The API and worker use
explicit role loaders; missing, unknown, wrong-role, placeholder or invalid typed values are
rejected. Validation derives and checks both maps; the legacy executor installs them atomically only
inside the journaled one-shot apply, which remains disabled by the production fault gate.

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
frontend tree, systemd units, release metadata and checksums. The server verifies and installs this
payload under `/opt/v2board/releases/<release-id>`, then atomically updates `/opt/v2board/current`.
It never builds the project and does not require Docker.

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

## Lifecycle schema v3/v4

Only three strict, kind-tagged manifests exist:

- [fresh install example](../docs/examples/fresh-install.v3.example.json);
- [legacy migration example](../docs/examples/legacy-migration.v4.example.json);
- [native upgrade example](../docs/examples/native-upgrade.v3.example.json).

The disposable CLI commands are:

```bash
v2board-lifecycle validate --manifest /secure/private/operation.json
v2board-lifecycle inspect --manifest /secure/private/operation.json
v2board-lifecycle authorize --manifest /secure/private/operation.json --inspect-review-sha256 <sha256> --output <bound-path>
v2board-lifecycle apply --manifest /secure/private/operation.json --authorization <bound-path>
v2board-lifecycle resume --manifest /secure/private/operation.json --authorization <bound-path>
```

The last three commands are present and wired, but the single typed production capability currently
keeps authorization readiness, apply, and resume fail-closed.

`v2board-lifecycle` is deliberately absent from the long-running native release. It is staged only
for the one migration operation; after the completion ledger commits, the result returns the exact
root-only argv for the operator to remove it manually. API/worker dependency graphs contain neither
the lifecycle crate nor `sqlx-mysql`.

The manifest contains secrets and must be a 1-byte-to-1-MiB regular non-symlink file with no Unix
group/world permissions. It rejects duplicate/unknown/missing keys and binds the exact file bytes to
an independent lifecycle audit key using the schema-specific v3 or v4 HMAC domain.

`validate` is offline syntax/semantic validation. `inspect` is the online read-only inventory before
maintenance. There is no public `plan` shortcut. The legacy executor performs full post-fence source
admission before the immutable backup, then a short post-backup identity/target-empty seal internally.

Legacy migration has one human decision: whether to start the irreversible one-shot apply against
the exact `operation_id + stable review_binding_sha256`. Authorization stores that stable digest as
`inspect_review_sha256` and separately records the confirmation-time full report digest as
`authorized_snapshot_report_sha256`. It does not pause for another confirmation inside
the maintenance window. Fresh install and future native destructive upgrades retain their own
separate confirmation rules.

The CLI parses legacy `apply` and same-operation `resume`, but one typed production capability
rejects both before writes. Validate output, inspection verdict/next-action, authorization readiness,
apply, and resume all derive from that same value. Legacy reports truthfully expose `converter_available=true` while
`apply_available=false`, `verdict=blocked` and `next_action=resolve_blockers`; fresh/native retain
their own implementation blockers. Passing
`validate`, or seeing individual target checks pass, is not permission to create targets, copy data
or cut over manually.

### Fresh install

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
`7e77de9f4873b317157490529f7be7d6f8a62421`. The adapter accepts only Oracle MySQL 8.0 or 8.4
in a read-only session; MySQL 5.7, Percona, MariaDB and compatible proxies are rejected. The target
is always fresh PostgreSQL 18 + ClickHouse 26.3 + Redis.

The config is manual-only: old `.env`, PHP config, theme and custom scripts are inventoried but not
executed, merged or imported. Sessions use `logout_all`. Stripe inventory must be zero. Temporary
subscription URL mappings/cache are deliberately invalidated at cutover; permanent `v2_user.token`
values are copied exactly. Old Redis traffic hashes, queues and failed work must be durably drained
and reconciled before ephemeral cache is discarded. The repository contains the panel API consumed
by nodes, but not the external V2bX/XrayR processes or a node-side process controller. Legacy
migration therefore requires an empty node inventory; any source node produces the stable
`external_node_coordinator_unavailable` preflight blocker before maintenance. The manifest accepts
only `inventory=[]` plus `not_required_no_nodes`; before and after native authority the executor proves
all eight PostgreSQL node tables and `v2_server_credential` remain empty with the same proof hash.

This is one offline maintenance operation, not CDC or coexistence. It has no MySQL replication,
dual-write, shadow-read, staged traffic release or MySQL runtime rollback. After final verification,
the operation starts the native services once, permanently masks MySQL 8 and old Redis, and
retains the actually decrypted-and-restore-tested encrypted backup as the only checksummed cold
archive. After the permanent PostgreSQL ledger commits, the result returns a root-only manual
cleanup argv for `v2board-lifecycle`; executable removal is not part of the completion proof.
Post-cutover recovery uses PostgreSQL/ClickHouse or forward repair, never a restarted MySQL service.

See the [legacy v4 guide](../docs/legacy-migration-manifest.md) and
[frozen lifecycle contract](../docs/upgrade-invariants.md). The converter, ownership proof, journal,
target bootstrap, config promotion, backup binding and one-shot cutover executor are connected, but
production apply remains disabled until the required real bare-metal crash/lost-ACK matrix passes.
`make rust-integration` exercises the disposable data path on both MySQL 8.0.44 and 8.4.6: all 27
seeded legacy tables are copied and value-verified in PostgreSQL 18, durable checkpoints are replayed
without duplication, and ClickHouse 26.3 is migrated, installation-bound, kept at an empty native
event epoch, then verified through the read-only cutover readback.

### Native upgrade

The manifest binds installation UUID, current/target build IDs, monotonic PostgreSQL and ClickHouse
schema epochs, runtime principals and maintenance strategy. Destructive changes, TTL shortening,
drops and repartitions are explicit impact lists. A destructive plan additionally requires operator
allowance, impact review, backup/restore proof and a second confirmation bound to a prior v3 report.

Current native inspection does not yet machine-verify the installation/build/schema declarations or
apply changes, so it remains blocked.

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
