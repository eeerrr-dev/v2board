# Production Operations Runbook

Scope: the single Debian 13 (Trixie) amd64 production host running
`v2board-api.service`, `v2board-worker.service`, and
`v2board-cloudflared.service` under systemd, with PostgreSQL 18, ClickHouse,
and Redis 8.8 as backing services. This runbook covers day-2 operation:
monitoring, alerting, backup, restore, drills, rollback, and first-response
triage. Installation and activation live in
[`deploy/README.md`](../deploy/README.md); data-ownership and admission
invariants live in
[`postgresql-clickhouse-invariants.md`](postgresql-clickhouse-invariants.md).

## 1. Monitoring and alerting

Ready-to-install Prometheus/Alertmanager/blackbox configs implementing this
section — including the §1.2 required-alert table — live in
[`deploy/monitoring/`](../deploy/monitoring/README.md).

### 1.1 The `/metrics` endpoint

The API exports Prometheus text metrics at `GET http://127.0.0.1:8080/metrics`.
The endpoint shares the `/healthz`/`/readyz` origin gate: in production it
answers only a direct loopback peer whose Host is exactly `127.0.0.1:8080` and
which carries no `CF-Connecting-IP` header. It is therefore unreachable through
the Cloudflare Tunnel by construction; never create a Tunnel or WAF route for
it. Scrape it with a Prometheus (or compatible agent) running on the host:

```yaml
scrape_configs:
  - job_name: v2board
    static_configs:
      - targets: ['127.0.0.1:8080']
```

Worker metrics do not need a second scrape target: the worker publishes its
heartbeats and job counters into its Redis metrics keys and the API re-exports
them (`v2board_worker_*`).

### 1.2 Required alerts

These implement the release-gate metrics mandated by
`postgresql-clickhouse-invariants.md` §8. Severity: **page** means wake an
operator; **warn** means investigate within a working day.

| Alert | Expression (PromQL) | Severity |
| --- | --- | --- |
| API scrape down | `up{job="v2board"} == 0` for 2m | page |
| PostgreSQL down or ledger drift | `v2board_postgres_up == 0` for 2m | page |
| Redis down | `v2board_redis_up == 0` for 2m | page |
| Operator config authority unhealthy | `v2board_operator_config_authority_healthy == 0` for 5m | page |
| Frontend release missing | `v2board_frontend_release_present == 0` for 5m | page |
| Scheduler stale | `time() - v2board_worker_scheduler_last_check_timestamp_seconds > 180` | page |
| Worker loop stale | `time() - v2board_worker_loop_heartbeat_timestamp_seconds > 300` | page |
| Scheduled job failing | `increase(v2board_worker_jobs_failed_total[1h]) > 0` | warn |
| Analytics admission unobservable | `v2board_analytics_admission_observed == 0` for 10m | warn |
| Outbox soft pressure | `v2board_analytics_pressure_state{state="soft_pressure"} == 1` for 15m | warn |
| Outbox hard stop | `v2board_analytics_pressure_state{state="hard_stop"} == 1` | page |
| Outbox aging | `v2board_analytics_oldest_pending_age_seconds > v2board_analytics_threshold_oldest_age_seconds{level="soft"}` | warn |
| Capacity headroom low | `v2board_analytics_capacity_headroom_bytes < v2board_analytics_threshold_min_headroom_bytes{level="soft"}` | warn |
| Elevated 5xx | `increase(v2board_http_requests_total{class="5xx"}[5m]) > 10` | warn |
| DB pool saturated | `v2board_db_pool_connections_open >= v2board_db_pool_connections_max` for 5m | warn |

### 1.3 Infrastructure-level monitoring

The application exporter does not replace database and host monitoring
(`postgresql-clickhouse-invariants.md` §8 explicitly keeps these
infrastructure-owned). Additionally run:

- `node_exporter` — disk usage/inode alerts on the PostgreSQL, ClickHouse,
  Redis, and journald volumes (log exhaustion of a shared disk must alert as a
  database outage);
- `postgres_exporter` — WAL volume, autovacuum age, lock waits, replication
  lag if a replica exists;
- ClickHouse's built-in `system.metrics`/`system.parts` (parts count, merge
  backlog) via its Prometheus endpoint if enabled;
- `systemctl is-active` state for all three units (e.g. via
  `node_exporter --collector.systemd`), which also covers cloudflared — the
  connector's own health is visible in the Cloudflare dashboard (Tunnel
  status) and `journalctl -u v2board-cloudflared`.

### 1.4 Optional error reporting and trace export

Sentry error reporting and OTLP span export are implemented in
`backend/rust/crates/telemetry` (shared by `v2board-api` and `v2board-worker`)
and, for the browser, in `frontend/apps/{user,admin}/src/lib/sentry.ts`. All
three controls are environment variables, entirely off by default, and an
invalid value logs a warning and stays off rather than failing the service —
none are secrets baked into a release artifact, and none are required for a
working deployment.

| Variable | Read by | Controls | Source the value from |
| --- | --- | --- | --- |
| `V2BOARD_SENTRY_DSN` | `v2board-api`, `v2board-worker` (`v2board_telemetry::init_tracing`) | Sentry error reporting for backend panics/`tracing::error!` events. Sets the Sentry `environment` tag to `production`/`local` from the same `production` flag that selects JSON vs. human-readable log output. | Sentry project → Settings → Client Keys (DSN), for a server-side (Rust) project. |
| `V2BOARD_OTEL_EXPORTER_OTLP_ENDPOINT` | `v2board-api`, `v2board-worker` (`v2board_telemetry::init_tracing`) | OTLP HTTP span export (batched, `protobuf` over HTTP). Accepts the collector's base URL (e.g. `http://127.0.0.1:4318`) or a full `.../v1/traces` URL; also enables W3C `traceparent` propagation on incoming requests. | Your OTLP-compatible tracing backend (e.g. an OpenTelemetry Collector, Grafana Tempo, Honeycomb) — its documented OTLP/HTTP traces endpoint. |
| `V2BOARD_FRONTEND_SENTRY_DSN` | `v2board-api` only (`backend/rust/crates/api/src/frontend.rs`) | Browser-side Sentry error reporting for both SPAs. The API injects the DSN into each app's runtime config and widens the document CSP `connect-src` to the DSN's ingest origin; the DSN never touches the frontend build. Requires a public key (URL username) and an `http(s)` ingest host, or it is treated as absent. | Sentry project → Settings → Client Keys (DSN), for a browser (JavaScript/React) project — typically a separate Sentry project from the backend's. |

`v2board-worker.service` has no frontend to report from, so it only ever reads
the first two. Set these only on the units that need them; a service unit with
none of its telemetry variables set runs exactly as today, with tracing/log
output only.

Example — enable all backend telemetry on `v2board-api.service` and error
reporting only on `v2board-worker.service`, via each unit's
`[Service]` block (either edit the tracked file under
`deploy/systemd/` before install, or use `systemctl edit
<unit>` to add a drop-in on the host):

```ini
# deploy/systemd/v2board-api.service
[Service]
Environment=V2BOARD_SENTRY_DSN=https://<key>@<org>.ingest.sentry.io/<project-id>
Environment=V2BOARD_OTEL_EXPORTER_OTLP_ENDPOINT=http://127.0.0.1:4318
Environment=V2BOARD_FRONTEND_SENTRY_DSN=https://<key>@<org>.ingest.sentry.io/<frontend-project-id>
```

```ini
# deploy/systemd/v2board-worker.service
[Service]
Environment=V2BOARD_SENTRY_DSN=https://<key>@<org>.ingest.sentry.io/<project-id>
```

## 2. Backups

What must survive the loss of the production host, and how:

| Data | Authority | Tool and cadence | Loss tolerance |
| --- | --- | --- | --- |
| PostgreSQL (`postgres` database) | All business data — sole authority | WAL archiving continuously + nightly base backup (§2.1) | None: PITR to minutes |
| Runtime configs and secrets | `/var/lib/v2board/api/config.json`, `/var/lib/v2board/worker/config.json` | Encrypted copy on every change (§2.2) | None: `app_key` loss makes encrypted provider secrets unrecoverable |
| Redis ACL file + server config | Provisioned Redis users/limits | Copy on every change (§2.2) | Re-provisionable, but a copy makes restore hours faster |
| ClickHouse analytics history | Sacrificial per invariants §8 | Optional weekly `BACKUP DATABASE` when history has product value | Full loss accepted: empty rebuild continues new events |
| Redis keyspace | Sessions, leases, heartbeats, counters | None (RDB persistence only for warm restarts) | Full loss accepted: fail-closed contracts rebuild state |
| Release archives | CI artifacts (30-day retention) | Keep each deployed `v2board-native-debian-13-amd64.tar.gz` + `.sha256` beside the DB backups | Needed to reinstall the exact running release |

All off-host copies must be encrypted at rest and inaccessible to the
application identities. Do not back up onto the same physical disk that hosts
PostgreSQL.

### 2.1 PostgreSQL: WAL archiving + base backups (PITR)

One-time setup in `postgresql.conf` (archive target shown as a mounted backup
volume; any off-host object-store command works as long as it is atomic and
verifies the write):

```conf
wal_level = replica
archive_mode = on
archive_command = 'test ! -f /backup/wal/%f && cp %p /backup/wal/%f'
archive_timeout = 300
```

Nightly base backup from a dedicated `pg_basebackup` role (replication
privilege only), via cron or a systemd timer:

```sh
pg_basebackup --pgdata=/backup/base/$(date +%F) --format=tar --gzip \
  --checkpoint=fast --wal-method=none --username=backup --no-password
```

Retention: keep 14 nightly base backups and every WAL segment younger than the
oldest retained base backup; delete older WAL only after the base backup that
needs it has expired. Monthly, additionally take one `pg_dump
--format=custom` logical dump as a version-portable escape hatch. Sync
`/backup` off-host after every nightly run and alert if the sync or
`archive_command` fails (a stuck archiver eventually fills the WAL disk).

### 2.2 Configs, secrets, and provisioning state

After every config change (operator config activation writes to PostgreSQL and
is covered by §2.1; this covers the file-level bootstrap state):

```sh
tar -C / -czf - var/lib/v2board/api/config.json var/lib/v2board/worker/config.json \
    etc/redis/users.acl | age --encrypt -R /root/backup-recipient.pub \
    > /backup/config/v2board-config-$(date +%F).tar.gz.age
```

(Substitute the operator's encryption tooling; the requirement is: encrypted,
off-host, and restorable without the application host.) The cloudflared Tunnel
token is operator-owned and re-issuable from the Cloudflare dashboard; do not
persist it into backups.

## 3. Restore

### 3.1 PostgreSQL point-in-time restore

On a rebuilt or replacement host, after installing PostgreSQL 18:

1. Stop `v2board-api` and `v2board-worker` (or confirm the host is fresh).
2. Restore the newest base backup into a clean data directory:
   `tar -xzf /backup/base/<date>/base.tar.gz -C /var/lib/postgresql/18/main`.
3. Configure recovery in `postgresql.conf`:

   ```conf
   restore_command = 'cp /backup/wal/%f %p'
   recovery_target_time = '<the moment before the incident>'
   recovery_target_action = 'promote'
   ```

4. `touch /var/lib/postgresql/18/main/recovery.signal`, start PostgreSQL, and
   watch the log until it promotes.
5. Verify: the application roles exist, `SELECT` over `users`/`orders` returns
   plausible data, and the migration ledger is exactly current (the API
   refuses to start otherwise).
6. Start `v2board-api`, require `readyz` to pass, start `v2board-worker`,
   require `READY=1`, then re-enable cloudflared per the activation order in
   `deploy/README.md`.

### 3.2 ClickHouse and Redis

- ClickHouse: restore is optional. The documented disaster path
  (invariants §8) is an empty rebuild: re-apply the schema with the release's
  `v2board-analytics-schema` binary, and accept that only unpublished and new
  events continue. Never stitch histories across instances.
- Redis: start empty with the restored ACL file and `noeviction`; sessions,
  leases, heartbeats, and rate-limit state rebuild themselves. Do not restore
  an RDB snapshot across an incident boundary if there is any doubt about its
  consistency — empty is always safe here.

### 3.3 Restore drill

Quarterly, on a scratch VM (never the production host):

1. Restore the newest base backup + WAL to a target time (§3.1 steps 2–4).
2. Run the §3.1 verification queries.
3. Record wall-clock restore time — restore time is a release-gate metric
   (invariants §8); alert the team if it exceeds the previous drill by more
   than 50%.
4. Destroy the scratch instance. A drill must never hold production secrets
   longer than the drill itself.

Treat a failed or skipped drill as an incident, not housekeeping debt: an
unverified backup is not a backup.

## 4. Application release rollback

Releases are immutable under `/opt/v2board/releases/<release-id>`; the running
release is the `/opt/v2board/current` symlink. To roll back to the previously
deployed release:

```sh
ln -sfn /opt/v2board/releases/<previous-release-id> /opt/v2board/releases/.current-next
mv -T /opt/v2board/releases/.current-next /opt/v2board/current
systemctl restart v2board-api.service v2board-worker.service
curl --fail http://127.0.0.1:8080/readyz
```

Constraints:

- The symlink swap is atomic (`mv -T`); in-flight asset requests keep working
  through the frontend `current`/`previous` window inside each release.
- A release that shipped a schema migration cannot be rolled back past that
  migration: the older binaries refuse a newer ledger. Roll forward instead,
  or perform a §3.1 PITR to before the migration **and** accept the data loss
  window that implies. Decide which while both releases are still installed —
  that is why step 8 of the activation order retains the prior release.
- After any rollback, re-run the public verification from the activation
  order (Cloudflare 3xx on the canonical `http://` URL, HTTPS page loads).

## 5. Incident quick reference

First commands for any incident:

```sh
systemctl status v2board-api v2board-worker v2board-cloudflared
curl -sS http://127.0.0.1:8080/readyz | jq .
journalctl -u v2board-api --since -15m --no-pager | tail -50
```

| Symptom | Likely cause | Action |
| --- | --- | --- |
| Public site down, `readyz` OK | Tunnel/connector or Cloudflare edge | `journalctl -u v2board-cloudflared`; Tunnel status in the Cloudflare dashboard; restart the connector; verify the route still maps the canonical hostname to `http://127.0.0.1:8080` |
| `readyz` `database: false` | PostgreSQL down or ledger drift | `systemctl status postgresql`; check disk; a ledger mismatch after a botched deploy means wrong release — see §4 |
| `readyz` `redis: false` | Redis down or ACL broken | `systemctl status redis`; check `noeviction` memory state; verify ACL users still load |
| `readyz` `operator_config_acknowledged: false` persists | Config activation not acknowledged | Check API logs for the rejected revision; a worker that cannot read the authority also blocks acknowledgement |
| Worker restart loop | Watchdog timeout — a loop is stuck | `journalctl -u v2board-worker`; identify the stale loop via `v2board_worker_loop_heartbeat_timestamp_seconds`; the stuck dependency (usually PostgreSQL/Redis) is the real incident |
| `hard_stop` pressure state | Outbox backlog hit the hard watermark | Traffic settlement 503s by design. Fix ClickHouse (usual cause), watch `pending_rows` drain; state auto-recovers at the recovery watermarks. Never widen thresholds mid-incident |
| Disk filling on the WAL volume | `archive_command` failing | Fix the archive target first; never delete un-archived WAL |
| 5xx burst with all checks green | Application regression | Correlate `journalctl -u v2board-api` request logs (request IDs) with the deploy time; roll back per §4 |

After any page-severity incident, write a short post-incident note (what
paged, timeline, root cause, follow-ups) next to the drill records. The
follow-ups feed the next release, not a wiki graveyard.

## 6. Operator audit trail

Every authenticated admin/staff mutation is appended to the PostgreSQL
`audit_log` table (actor, surface, method, prefix-relative path, response
status, client IP, request id, epoch timestamp). The table is append-only at
the database level — `UPDATE`/`DELETE` raise for every application principal —
so it is the authoritative who-did-what-when record for incident forensics.
Request bodies are deliberately not recorded (they can carry secrets);
correlate the `request_id` column with journald API logs when payload-level
context is needed:

```sql
SELECT to_timestamp(created_at), actor_email, surface, method, path, status_code, client_ip
FROM audit_log ORDER BY id DESC LIMIT 50;
```

It lives in the ordinary `postgres` database, so §2.1 backups and §3.1
restores cover it with no extra step.
