# Bare-metal production deployment

Production does not run Docker or Compose. Docker is only the reproducible local/CI build
environment that exports a native Linux artifact. The only supported artifact target is
Debian 13 Linux amd64 with glibc and the native `libssl3t64` runtime.

Install the exact native runtime package on both the new host and the stopped old host that will run the
one-shot lifecycle artifact; there is no package-name branch for another distribution or Debian release:

```bash
apt-get update
apt-get install --no-install-recommends ca-certificates libssl3t64
```

## Release contents and trust boundary

CI publishes `v2board-native-debian-13-amd64.tar.gz` plus its external SHA-256. After verifying that
digest, unpack into a new root-owned `/opt/v2board/releases/<release-id>` directory. The payload
contains exactly three long-lived/native administration binaries, the validated frontend tree,
systemd units including the canonical Cloudflare Tunnel service, `RELEASE`, and an internal
`SHA256SUMS`:

```text
bin/v2board-api
bin/v2board-workers
bin/v2board-analytics-schema
frontend/releases/<content-id>/{user,admin}
frontend/current
frontend/previous
systemd/v2board-api.service
systemd/v2board-worker.service
systemd/v2board-cloudflared.service
```

The workflow creates a signed GitHub/Sigstore build-provenance attestation over both release archives,
but only for a successful `push` to `main`; pull-request artifacts are intentionally not trusted for
production. Before accepting a downloaded archive, bind verification to this repository, this workflow,
the `main` ref and the exact 40-character commit recorded for the release:

```bash
REPOSITORY=eeerrr-dev/v2board
EXPECTED_SHA=<40-character-main-commit>
SIGNER_WORKFLOW="$REPOSITORY/.github/workflows/native-ci.yml"

gh attestation verify v2board-native-debian-13-amd64.tar.gz \
  --repo "$REPOSITORY" \
  --signer-workflow "$SIGNER_WORKFLOW" \
  --source-ref refs/heads/main \
  --source-digest "$EXPECTED_SHA" \
  --deny-self-hosted-runners
gh attestation verify v2board-lifecycle-debian-13-amd64.tar.gz \
  --repo "$REPOSITORY" \
  --signer-workflow "$SIGNER_WORKFLOW" \
  --source-ref refs/heads/main \
  --source-digest "$EXPECTED_SHA" \
  --deny-self-hosted-runners
sha256sum --check v2board-native-debian-13-amd64.tar.gz.sha256
sha256sum --check v2board-lifecycle-debian-13-amd64.tar.gz.sha256
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
tree, both frontend links, internal checksums, and every exact canonical systemd unit, including the
Cloudflare Tunnel unit. It is independent of the MySQL import manifest. Passing it proves archive shape
and integrity, not authenticity.

The same CI job publishes a separate `v2board-lifecycle-debian-13-amd64` artifact containing
`v2board-lifecycle-debian-13-amd64.tar.gz` and its external SHA-256. Verify that outer digest, unpack the
archive, run `sha256sum --check SHA256SUMS`, and confirm `RELEASE` names the expected source revision
before transferring the executable to the stopped old host. The tool has the same Debian 13 Linux amd64,
glibc and `libssl3t64` ABI boundary as the native release. The stopped old host that runs it must also satisfy
this Debian 13 amd64 boundary; the repository provides no second lifecycle build. The lifecycle artifact is
disposable and is not installed into the long-running new release.

## Operating-system identities and paths

Create two static application users and one static ingress user, all without login shells. Do not add
cross-role group memberships:

```bash
useradd --system --user-group --home-dir /var/lib/v2board/api --shell /usr/sbin/nologin v2board-api
useradd --system --user-group --home-dir /var/lib/v2board/worker --shell /usr/sbin/nologin v2board-worker
useradd --system --user-group --home-dir /var/lib/cloudflared --shell /usr/sbin/nologin cloudflared
install -d -m 0700 -o v2board-api -g v2board-api /var/lib/v2board/api
install -d -m 0700 -o v2board-worker -g v2board-worker /var/lib/v2board/worker
install -d -m 0750 -o cloudflared -g cloudflared /var/lib/cloudflared
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
`systemctl daemon-reload`. The API listens only on `127.0.0.1:8080`. PostgreSQL 18, ClickHouse 26.3 LTS,
and Redis are separate native/managed services, not dependencies installed by these units.

## Canonical Cloudflare Tunnel ingress

A remotely-managed named Cloudflare Tunnel is the only production ingress. The Debian 13 host does not
install Nginx, does not terminate public TLS, and does not listen on public TCP 80, 443, or 8080.
`cloudflared` runs on the same host under the supplied systemd unit and makes outbound connections to
Cloudflare; the one public hostname maps directly to `http://127.0.0.1:8080`. Rust continues to serve every
page, API route and asset. There is no local tunnel YAML or second ingress configuration.

Install `cloudflared` from Cloudflare's stable, signed Debian repository. Package upgrades are deliberate
operator actions; package-managed cloudflared does not replace its own executable, and the unit makes that
boundary explicit with `--no-autoupdate`:

```bash
apt-get update
apt-get install --no-install-recommends curl
install -d -m 0755 -o root -g root /usr/share/keyrings
curl -fsSL https://pkg.cloudflare.com/cloudflare-main.gpg \
  -o /usr/share/keyrings/cloudflare-main.gpg
printf '%s\n' \
  'deb [signed-by=/usr/share/keyrings/cloudflare-main.gpg] https://pkg.cloudflare.com/cloudflared any main' \
  > /etc/apt/sources.list.d/cloudflared.list
apt-get update
apt-get install --no-install-recommends cloudflared
cloudflared --version
```

In Cloudflare Zero Trust, create one remotely-managed named Tunnel and add exactly one **Published
application** route with these values:

```text
Public hostname:  panel.example.com
Service type:     HTTP
Service URL:      http://127.0.0.1:8080
HTTP Host Header: <empty>
```

The public hostname must be the exact host in runtime `app_url` and `cors_allowed_origins`. Do not add an
alias, wildcard, path-specific service, private-network route, or second published application. Leave the
HTTP Host Header override empty so Cloudflare forwards the public hostname unchanged. Enable Cloudflare's
**Always Use HTTPS** setting before admitting traffic; Cloudflare owns public TLS and redirects cleartext
edge requests before they reach the tunnel.

Do not put Cloudflare Access in front of this hostname: ordinary users, payment webhooks, and node callbacks
must reach the application's own authentication boundary. Keep `CF-Connecting-IP` in its canonical form by
leaving **Remove visitor IP headers** disabled and never selecting **Pseudo IPv4: Overwrite Headers**. Do not
enable Cache Everything or a cache rule that overrides origin policy; Cloudflare must continue to respect
the Cache-Control emitted by Rust.

Cloudflare WAF/Bot rules must never issue an interactive challenge for
`/api/v1/guest/payment/notify/*`, `/api/v1/guest/telegram/webhook`, `/api/v1/client/*`,
`/api/v1/server/*`, or `/api/v2/server/config`. Those callers cannot complete a browser challenge; the Rust
signature and token checks remain authoritative for them.

Copy only that remotely-managed Tunnel's connector token into a protected file on the production host.
Do not put it in a command line, environment variable, source file, release, report, or shell history:

```bash
install -d -m 0700 -o root -g root /etc/v2board/cloudflared
install -m 0600 -o root -g root \
  /secure-transfer/tunnel-token /etc/v2board/cloudflared/tunnel-token
test "$(stat -c '%U:%G %a' /etc/v2board/cloudflared/tunnel-token)" = 'root:root 600'
```

Install `v2board-cloudflared.service` from the same verified release. Do not run `cloudflared service
install`; that would generate an untracked alternative unit:

```bash
install -m 0644 -o root -g root \
  /opt/v2board/current/systemd/v2board-cloudflared.service \
  /etc/systemd/system/v2board-cloudflared.service
systemctl daemon-reload
systemd-analyze verify v2board-cloudflared.service
```

The unit uses systemd `LoadCredential=` to copy the root-only token into a private runtime credential
directory, then supplies only that ephemeral path through cloudflared's `--token-file`. A separate
non-secret systemd `SetCredential=` contains exactly the route-free YAML mapping `{}`; passing its private
runtime path through `--config` disables cloudflared's automatic discovery in `/etc/cloudflared` or the
service home without creating an operator-owned local Tunnel mode. It runs as the unprivileged
`cloudflared` user, disables self-update, writes logs only to journald, and contains no hostname, tunnel
UUID, token, or locally managed ingress rules. Rotate the connector token in Cloudflare, atomically replace
the root-only source file, and restart this unit if the credential is ever exposed.

The host firewall must reject inbound 80, 443, and 8080; management access is outside this application
ingress contract. Allow the existing application egress plus DNS and Cloudflare Tunnel egress on port 7844
over TCP and UDP so automatic HTTP/2/QUIC fallback remains available. Do not install another web server,
reverse proxy, ingress agent, or public load balancer on the host. Production runtime configuration trusts
only the same-host connector with `trusted_proxy_cidrs: ["127.0.0.1/32"]`. `cloudflared` writes only to
journald at `info` through systemd; keep journald bounded and do not enable a cloudflared access log or
`debug`, which records raw request URLs and headers.

`/healthz` and `/readyz` remain direct-loopback probes. In production Rust serves them only when the TCP
peer is loopback, `CF-Connecting-IP` is absent, and Host is exactly `127.0.0.1:8080`. A request through the
Tunnel retains the public Host and therefore receives 404 even if a Cloudflare setting accidentally removes
the visitor-IP header; the public hostname never exposes installation or readiness state.

Persistent Cloudflare HTTP Logpush is disabled by default. If the operator enables it, the only permitted
request-target field is `ClientRequestPath`; the job must not select `ClientRequestURI`, any query-string
field, Referer, request-header, or Cookie fields. Subscription, verification, and payment query tokens must
not be written to operator-managed persistent Logpush or origin logs. Cloudflare, as the TLS/CDN entry
point, necessarily processes the full request URL; provider-side security-event processing and retention
remain part of that external trust boundary.

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
3. create the one remotely-managed Tunnel route, leave its HTTP Host Header empty, enable Always Use
   HTTPS, and install the connector token while leaving the connector offline;
4. atomically point `/opt/v2board/current` at the staged release and install/verify its three systemd
   units, including the still-disabled `v2board-cloudflared.service`;
5. start `v2board-api.service` and require `GET http://127.0.0.1:8080/readyz` to pass locally;
6. start `v2board-worker.service` and require systemd `READY=1` plus a healthy watchdog;
7. only now run `systemctl enable --now v2board-cloudflared.service`, require the unit to reach active
   state, verify the canonical public HTTPS URL, and use `curl --head --max-redirs 0` against the same
   canonical `http://` URL to require a Cloudflare 3xx whose `Location` is the exact `https://` authority
   and path before admitting users; an HTTP response from the application is an activation failure;
8. on later native deployments, run serialized schema jobs only when that release actually contains a
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
