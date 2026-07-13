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

Verify `(cd <staged-release> && sha256sum --check SHA256SUMS)` before changing any symlink. Never
compile on the server. The separately exported `v2board-lifecycle` tool is not part of this release;
it is staged only for the initial one-shot operation. After the completion ledger commits, the
operator removes it manually with the exact root-only argv returned by the command.

## Operating-system identities and paths

Create two static, no-login users. Do not place either user in the other's group:

```bash
useradd --system --user-group --home-dir /var/lib/v2board/api --shell /usr/sbin/nologin v2board-api
useradd --system --user-group --home-dir /var/lib/v2board/worker --shell /usr/sbin/nologin v2board-worker
install -d -m 0700 -o v2board-api -g v2board-api /var/lib/v2board/api
install -d -m 0700 -o v2board-worker -g v2board-worker /var/lib/v2board/worker
install -d -m 0755 -o root -g root /var/lib/v2board/rules /opt/v2board/releases
```

The lifecycle writes complete role-specific documents atomically:

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

## Activation order

For an already supported native release, the order is fixed:

1. verify the outer archive digest, internal `SHA256SUMS`, release identity, backups, and configs;
2. run the serialized PostgreSQL and ClickHouse schema lifecycle commands with the exact staged
   binaries;
3. atomically point `/opt/v2board/current` at the staged release;
4. start `v2board-api.service` and require `GET /readyz` to pass;
5. start `v2board-worker.service` and require systemd `READY=1` plus a healthy watchdog;
6. retain the prior native release only for the explicitly supported native rollback window.

API and worker refuse to start when the PostgreSQL ledger is not exactly current. Worker readiness
also requires PostgreSQL and Redis; ClickHouse failure only makes analytics stale and grows the
PostgreSQL outbox.

Fresh install is not yet production-supported. Legacy `apply`/`resume` grammar and executor exist
but remain fail-closed through the single typed production capability until the repository's real
crash/lost-ACK matrix is complete.
Do not manually create targets or reinterpret a passing read-only report as permission to write.

## Initial legacy retirement

The legacy migration is one offline operation in one maintenance window. It must not implement CDC,
dual-write, shadow reads, gradual traffic release, or a MySQL runtime fallback. After final
online inspection, the operator authorizes the exact operation and stable review binding once; the
authorization separately records the complete confirmation-time report digest. The one-shot apply then
performs the whole transition without another pause:

1. stop and fence the old API writers, scheduler, and temporary-link issuer; disable worker restart,
   let the existing workers drain and reconcile pending traffic and business work, then stop them
   (schema v4 proves the source node inventory is empty; schema v5 binds the old inventory for
   discard, discards legacy per-user/per-node traffic details plus `v2_log`/`v2_mail_log`, and
   requires the operator to stop old external node processes before rebuilding them; every discarded
   source remains covered by the full fingerprint, encrypted archive, and per-table pre-authority
   discard proof);
2. take and restore-test one consistent source recovery set, run the final read-only recheck, then
   bulk-convert into empty PostgreSQL/ClickHouse targets while every old and new writer stays down;
3. verify exact values, relationships, sequences, configs, and analytics projection; schema v5 must
   retain `v2_stat`, every existing user field, and `v2_payment` including its verification config and
   original `enable` value—migration must never disable payment methods by default; activate the
   native API and worker exactly once;
4. immediately mask and stop the dedicated MySQL 8 and old Redis units and remove their
   network access, then prove all three old credentials are unreachable;
5. after the completion ledger succeeds, run the returned root cleanup argv to remove
   `v2board-lifecycle`; separately remove one-shot MySQL client/source credentials from the
   production server, while permanently retaining the encrypted cold archive with its SHA-256 and
   the operation journal plus signed migration-report receipts.

Completion must prove `source_retired=true`, MySQL and both old Redis endpoints are unreachable,
source access is permanently disabled, legacy runtime compatibility is disabled, and PostgreSQL is
the only transactional authority. The lifecycle tool may then be removed manually with the returned
root argv; its absence is not part of completion proof. Completion does not claim an internal
`DROP USER` after the dedicated database process has already been permanently stopped.
The cleanup argv removes only the one-shot binary; it must not remove the operation journal,
report receipts, or encrypted archive.

After activation, recovery is PostgreSQL/ClickHouse restore or forward repair. Restarting MySQL is
not a supported rollback.

## Logs and shutdown

Both units write stdout/stderr to journald. Configure `SystemMaxUse`, retention, and alerting for the
host; configure PostgreSQL, ClickHouse, and Redis native log rotation separately. A full log disk is
a database outage. API and worker use SIGTERM with bounded graceful shutdown. Worker uses
`Type=notify`, `WatchdogSec=30s`, and `/run/v2board-worker/health`; no persistent `/tmp` health file
or Docker HEALTHCHECK exists in production.
