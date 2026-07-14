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

The workflow creates a signed GitHub/Sigstore build-provenance attestation over both release archives,
but only for a successful `push` to `main`; pull-request artifacts are intentionally not trusted for
production. Before accepting a downloaded archive, bind verification to this repository, this workflow,
the `main` ref and the exact 40-character commit recorded for the release:

```bash
REPOSITORY=eeerrr-dev/v2board
EXPECTED_SHA=<40-character-main-commit>
SIGNER_WORKFLOW="$REPOSITORY/.github/workflows/native-ci.yml"

gh attestation verify v2board-native-linux-amd64.tar.gz \
  --repo "$REPOSITORY" \
  --signer-workflow "$SIGNER_WORKFLOW" \
  --source-ref refs/heads/main \
  --source-digest "$EXPECTED_SHA" \
  --deny-self-hosted-runners
gh attestation verify v2board-lifecycle-linux-amd64.tar.gz \
  --repo "$REPOSITORY" \
  --signer-workflow "$SIGNER_WORKFLOW" \
  --source-ref refs/heads/main \
  --source-digest "$EXPECTED_SHA" \
  --deny-self-hosted-runners
sha256sum --check v2board-native-linux-amd64.tar.gz.sha256
sha256sum --check v2board-lifecycle-linux-amd64.tar.gz.sha256
```

The attestation establishes the archive digest and GitHub Actions origin; it does not prove application
correctness or make a checksum downloaded from the same channel an independent authenticity mechanism.
The outer checksum remains useful for transfer-integrity checks, and `RELEASE` plus the internal
`SHA256SUMS` must still match after unpacking.

Both Actions artifacts have a fixed 30-day transfer window. They are not durable release storage: download
and verify both archives from the same run before that window closes, retain the accepted native archive
under the operator's normal release/backup policy, and discard the lifecycle archive after the one-time
import. Online verification uses the GitHub API. If an offline verification ceremony is required, download
the attestation bundle and trusted-root material while online using GitHub's
[documented offline flow](https://docs.github.com/en/actions/how-tos/secure-your-work/use-artifact-attestations/verify-attestations-offline).
GitHub supports this workflow for public repositories; making the repository private/internal requires
GitHub Enterprise Cloud, and GitHub Enterprise Server does not support it.

Verify `(cd <staged-release> && sha256sum --check SHA256SUMS)` before changing any symlink. Never
compile on the server. The separately exported `v2board-lifecycle` utility is not part of the
long-running release; its MySQL commands validate, inspect and execute the one-time initial import, and CI
separately uses its release-archive audit before discarding the utility.

CI additionally submits the packed, root-owned archive to
`v2board-lifecycle inspect-release-archive`; this standalone deployment check verifies the complete tar
tree, both frontend links, internal checksums and systemd contract. It is independent of the MySQL import
manifest. Passing it proves archive shape and integrity, not authenticity.

The same CI job publishes a separate `v2board-lifecycle-linux-amd64` artifact containing
`v2board-lifecycle-linux-amd64.tar.gz` and its external SHA-256. Verify that outer digest, unpack the
archive, run `sha256sum --check SHA256SUMS`, and confirm `RELEASE` names the expected source revision
before transferring the executable to the stopped old host. The tool has the same Debian 12 compatible
Linux amd64, glibc and `libssl3` ABI boundary as the native release. Confirm that boundary against the old
host before cutover; an incompatible old host requires a separately verified compatible lifecycle build,
not moving MySQL onto the new production machine or compiling source during cutover. The lifecycle artifact
is disposable and is not installed into the long-running new release.

## Operating-system identities and paths

Create two static, no-login users. Do not place either user in the other's group:

```bash
useradd --system --user-group --home-dir /var/lib/v2board/api --shell /usr/sbin/nologin v2board-api
useradd --system --user-group --home-dir /var/lib/v2board/worker --shell /usr/sbin/nologin v2board-worker
install -d -m 0700 -o v2board-api -g v2board-api /var/lib/v2board/api
install -d -m 0700 -o v2board-worker -g v2board-worker /var/lib/v2board/worker
install -d -m 0755 -o root -g root /var/lib/v2board/rules /opt/v2board/releases
```

The operator securely transfers the two role-specific documents generated by the importer on the old host
and installs them atomically as:

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

After a successful initial MySQL import, `execute` has already applied and verified both exact schema
lineages, disabled the PostgreSQL migration login and removed the ClickHouse schema user. Initial native
service activation is therefore fixed and read-only with respect to schema:

1. verify the outer archive digest, internal `SHA256SUMS`, release identity, available runtime backups,
   and configs;
2. verify the import report, PostgreSQL exact-current ledger, ClickHouse bindings/readiness and empty
   installation-bound Redis without rerunning either schema command;
3. atomically point `/opt/v2board/current` at the staged release;
4. start `v2board-api.service` and require `GET /readyz` to pass;
5. start `v2board-worker.service` and require systemd `READY=1` plus a healthy watchdog;
6. on later native deployments, run serialized schema jobs only when that release actually contains a
   new migration, and retain the prior immutable frontend release for the documented
   `current`/`previous` asset window.

API and worker refuse to start when the PostgreSQL ledger is not exactly current. Worker readiness
also requires PostgreSQL and Redis; ClickHouse failure only makes analytics stale and grows the
PostgreSQL outbox.

## Initial MySQL import

There is one offline `mysql-import.v1` path:

1. The operator stops the old API, workers, scheduler, payment ingress and external node reporters.
2. The operator exports all business tables and rows from Oracle MySQL 8 into one dump and records its
   SHA-256. The dump is retained as a protected backup artifact; the old database is not modified.
3. On the stopped old production host, lifecycle uses a dedicated `SELECT`-only account to establish one
   `REPEATABLE READ`, `READ ONLY`, consistent snapshot of the original MySQL database; it rejects extra
   grants/roles/`GRANT OPTION` and requires InnoDB for every imported table.
4. The converter runs one primary-key-ordered streaming MySQL `SELECT` per retained table; memory is bounded
   to the current decoded row, byte-bounded send buffers, and a hard-capped 4,096-entry payment-id
   classification index required by the fixed Stripe order policy. It explicitly validates/transforms each row and
   gives every target table exactly one PostgreSQL `COPY FROM STDIN` stream over the
   same-datacenter private network into a brand-new PostgreSQL 18 database. After all tables complete COPY,
   it creates deferred business/cross-row unique constraints, secondary indexes, and foreign keys, resets
   sequences, runs `ANALYZE`, and performs
   exactly one whole-table canonical verification per retained target table in primary-key order. The gift-card
   source stream deterministically feeds both its base and derived redemption targets. ClickHouse
   starts without old events; the dedicated Redis 8.8 target starts empty with `/0`, `noeviction`, a
   disabled default user and a writable external `aclfile`.
5. The importer generates the API and worker configs plus an import report under the old host's
   root-owned `config_output_directory` from explicit `target` and `runtime` values.
6. The operator securely transfers the two configs to the new host, installs them at the fixed role-owned
   paths, and verifies data, role configs, database schemas and service prerequisites before activation.

The converter runs on the stopped old production host and reaches the source only through the manifest's
loopback-only `source.database_url`. The dump is never loaded for conversion and MySQL SQL is never sent to
PostgreSQL. The converter connects outbound to the new PostgreSQL target with a temporary migration
principal using authenticated TLS over the same-datacenter private network. Its bounded MySQL buffers are
only flow control, not fixed 1,000-row PostgreSQL batches; it has no bulk-`INSERT` fallback and writes no
intermediate COPY/CSV transfer files. Canonical expectations are accumulated during typed conversion and
compared with the one post-load PK-ordered full scan of each target table; there is no per-batch target
verification or second transfer strategy. The new production machine never runs MySQL. Legacy source tables keep their `v2_*` names, while native PostgreSQL/ClickHouse target tables
are unprefixed (`users` and `orders` avoid PostgreSQL keyword conflicts).

Run the disposable utility on the stopped old host after the dump and read-only source account are ready:

```bash
v2board-lifecycle validate --manifest /secure/private/mysql-import.json
v2board-lifecycle inspect --manifest /secure/private/mysql-import.json
v2board-lifecycle execute --manifest /secure/private/mysql-import.json
```

`execute` requires absent PostgreSQL/ClickHouse targets, a new empty Redis and a nonexistent
`config_output_directory` under an existing root-owned `0700` non-symlink parent. Run lifecycle as root; a
different local user's `0600`/`0700` paths are deliberately rejected. It creates the directory as `0700` and emits
`api.config.json`, `worker.config.json` and `import-report.json` as `0600`. It connects to old MySQL only
through the dedicated `SELECT`-only account and never connects to old Redis or Stripe. The manifest's one-shot
`redis_bootstrap_url` is never emitted: execute creates
distinct API/worker ACL users, limits them to their installation and role key patterns, performs
`ACL SAVE` then `ACL LOAD`, and reconnects for positive and negative probes before writing their separate
runtime `redis_url` values. This complete command is the only write path; do not substitute manual partial writes.

Old Redis is never read. Pending Redis traffic, queue/failed work, sessions, OTP, temporary links,
cache/locks and Horizon metadata are explicitly discarded. Stripe configuration and unfinished Stripe
orders are discarded without contacting Stripe; terminal Stripe order history is retained with active
provider bindings cleared. Non-Stripe payment configuration remains ordinary retained business data,
and user balance is never automatically refunded or adjusted.

If an import attempt fails, delete its incomplete new PostgreSQL/ClickHouse/Redis targets and output
directory, correct the problem, and run the same conversion again from the stopped source into fresh empty
targets. This
is not recovery of the untouched old database and keeps no resumable intermediate state; there is no import
rollback, resume, checkpoint, recovery or cleanup/restart workflow. After native service starts, normal
PostgreSQL backup/PITR is ordinary runtime operation, not a MySQL-import stage. ClickHouse history is
sacrificial: an empty rebuild continues only unpublished and new events, with no full history replay.

The operator permanently retires the old site only after the new result is accepted. Full data and
command details are in the [MySQL import guide](../docs/mysql-import.md). After acceptance, delete the
old-host lifecycle binary, manifest and config-output copy; revoke the source MySQL read-only account and
revoke or rotate the external PostgreSQL/ClickHouse/Redis bootstrap credentials. Keep or destroy the dump
only under a separate protected
backup policy, never as part of a secret-bearing migration workspace.

## Logs and shutdown

Both units write stdout/stderr to journald. Configure `SystemMaxUse`, retention, and alerting for the
host; configure PostgreSQL, ClickHouse, and Redis native log rotation separately. A full log disk is
a database outage. API and worker use SIGTERM with bounded graceful shutdown. Worker uses
`Type=notify`, `WatchdogSec=30s`, and `/run/v2board-worker/health`; no persistent `/tmp` health file
or Docker HEALTHCHECK exists in production.
