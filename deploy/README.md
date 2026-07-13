# Bare-metal production deployment

Production does not run Docker or Compose. Docker is only the reproducible local/CI build
environment that exports a native Linux artifact. The initially supported artifact target is
Debian 12 compatible Linux amd64 with glibc and `libssl3`.

## Release contents and trust boundary

CI publishes `v2board-native-linux-amd64.tar.gz` plus its external SHA-256. After verifying that
digest, unpack into a new root-owned `/opt/v2board/releases/<release-id>` directory. The payload
contains exactly three long-lived/native administration binaries, the validated frontend tree,
systemd units, `RELEASE`, and an internal `SHA256SUMS`:

```text
bin/v2board-api
bin/v2board-workers
bin/v2board-analytics-schema
frontend/releases/<content-id>/{user,admin}
frontend/current
frontend/previous
systemd/v2board-api.service
systemd/v2board-worker.service
```

The checksum proves byte integrity only. The current workflow does not publish an independently
verifiable signature or provenance attestation, and its CI artifact retention is 14 days; those are
unresolved production distribution/retention requirements, not properties supplied by this guide.
Do not treat a checksum downloaded from the same untrusted channel as artifact authenticity.

Verify `(cd <staged-release> && sha256sum --check SHA256SUMS)` before changing any symlink. Never
compile on the server. The separately exported `v2board-lifecycle` utility is not part of the
long-running release; its MySQL commands inspect the initial import inputs, and CI separately uses its
release-archive audit before discarding the utility.

CI additionally submits the packed, root-owned archive to
`v2board-lifecycle inspect-release-archive`; this standalone deployment check verifies the complete tar
tree, both frontend links, internal checksums and systemd contract. It is independent of the MySQL import
manifest. Passing it proves archive shape and integrity, not authenticity.

## Operating-system identities and paths

Create two static, no-login users. Do not place either user in the other's group:

```bash
useradd --system --user-group --home-dir /var/lib/v2board/api --shell /usr/sbin/nologin v2board-api
useradd --system --user-group --home-dir /var/lib/v2board/worker --shell /usr/sbin/nologin v2board-worker
install -d -m 0700 -o v2board-api -g v2board-api /var/lib/v2board/api
install -d -m 0700 -o v2board-worker -g v2board-worker /var/lib/v2board/worker
install -d -m 0755 -o root -g root /var/lib/v2board/rules /opt/v2board/releases
```

The initial importer writes complete role-specific documents atomically:

```text
/var/lib/v2board/api/config.json       v2board-api:v2board-api 0600
/var/lib/v2board/worker/config.json    v2board-worker:v2board-worker 0600
```

Their parent directories must remain separate because the API legitimately atomically rewrites its
own config. A shared writable parent would allow rename/unlink across the role boundary. Rules and
every release directory are root-owned and read-only to both processes.

Install the supplied units into `/etc/systemd/system`, run `systemd-analyze verify`, then
`systemctl daemon-reload`. The API listens only on `127.0.0.1:8080`; Nginx or Caddy terminates TLS and
proxies every page, API route, and `/assets/*` request to Rust. PostgreSQL 18, ClickHouse 26.3 LTS,
and Redis are separate native/managed services, not dependencies installed by these units.

## One-shot secrets

Never retain DDL or password-reset credentials in a unit `Environment=` line or shell history. The
binaries accept systemd credentials (or an absolute owner-only `*_FILE` path):

- `v2board-migration-database-url` for `v2board-api migrate`;
- `v2board-clickhouse-schema-password` for `v2board-analytics-schema`;
- `v2board-new-password` for `v2board-api reset-admin-password`.

Use a transient `systemd-run --wait --collect` unit with `LoadCredential=` and the same Unix user as
the corresponding runtime. Non-secret ClickHouse endpoint/database/username values may be supplied
to that transient unit. Remove the source credential file when the command completes.

## Intended activation order

After the initial database and configuration have been fully verified, native service activation is fixed:

1. verify the outer archive digest, internal `SHA256SUMS`, release identity, available runtime backups,
   and configs;
2. run the serialized PostgreSQL and ClickHouse schema commands with the exact staged
   binaries;
3. atomically point `/opt/v2board/current` at the staged release;
4. start `v2board-api.service` and require `GET /readyz` to pass;
5. start `v2board-worker.service` and require systemd `READY=1` plus a healthy watchdog;
6. on later native deployments, retain the prior immutable frontend release for the documented
   `current`/`previous` asset window.

API and worker refuse to start when the PostgreSQL ledger is not exactly current. Worker readiness
also requires PostgreSQL and Redis; ClickHouse failure only makes analytics stale and grows the
PostgreSQL outbox.

## Initial MySQL import

There is one offline `mysql-import.v1` path:

1. The operator stops the old API, workers, scheduler, payment ingress and external node reporters.
2. The operator exports all business tables and rows from Oracle MySQL 8 into one dump and records its
   SHA-256. The old database is not modified.
3. On the stopped old production host, the dump is loaded into a separate, disposable MySQL 8 engine that
   runs no old trigger, routine or event.
4. The converter copies the fixed retained rows into a brand-new PostgreSQL 18 database. ClickHouse
   starts without old events and the new Redis starts empty.
5. The importer generates the API and worker configs from explicit `target` and `runtime` values.
6. Data, role configs, database schemas and service prerequisites are verified before activation.

The staging engine and converter run by default on the stopped old production host. Staging is a second,
loopback-only instance with a separate data directory or volume, port/socket and credentials; it must not be
created inside the source instance or mount the source data directory. The converter connects outbound to the
new PostgreSQL target with a temporary migration principal. If the old host lacks capacity, use a disposable
migration VM instead. Staging is stopped and deleted after success or failure, and the new production machine
never runs MySQL. Legacy source tables keep their `v2_*` names, while native PostgreSQL/ClickHouse target tables
are unprefixed (`users` and `orders` avoid PostgreSQL keyword conflicts).

The repository currently provides `validate` and read-only `inspect` for this manifest. The executor
for dump → staging → PostgreSQL conversion is not connected, so the repository cannot yet claim that a
production import can be executed. Do not substitute manual partial writes for the missing executor.

Old Redis is never read. Pending Redis traffic, queue/failed work, sessions, OTP, temporary links,
cache/locks and Horizon metadata are explicitly discarded. Stripe configuration and unfinished Stripe
orders are discarded without contacting Stripe; terminal Stripe order history is retained with active
provider bindings cleared. Non-Stripe payment configuration remains ordinary retained business data,
and user balance is never automatically refunded or adjusted.

If an import attempt fails, discard its staging database and incomplete new target, correct the problem,
and run the same conversion again into a new empty target. This is not recovery of the untouched old
database and keeps no resumable intermediate state. After native service starts, normal PostgreSQL
backup/PITR and ClickHouse replay apply as ordinary runtime operations, not as MySQL-import stages.

The operator permanently retires the old site only after the new result is accepted. Full data and
command details are in the [MySQL import guide](../docs/mysql-import.md).

## Logs and shutdown

Both units write stdout/stderr to journald. Configure `SystemMaxUse`, retention, and alerting for the
host; configure PostgreSQL, ClickHouse, and Redis native log rotation separately. A full log disk is
a database outage. API and worker use SIGTERM with bounded graceful shutdown. Worker uses
`Type=notify`, `WatchdogSec=30s`, and `/run/v2board-worker/health`; no persistent `/tmp` health file
or Docker HEALTHCHECK exists in production.
