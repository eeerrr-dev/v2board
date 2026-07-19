# Production monitoring stack

Ready-to-install configuration for the monitoring/alerting contract in the
[operations runbook](../../docs/operations.md) §1. Everything here runs on the
production Debian 13 host from stock Debian packages; nothing is containerized
and nothing opens a public listener. The host firewall already rejects inbound
HTTP — keep every monitoring UI on loopback and reach it over an SSH tunnel.

## Install

```bash
apt-get update
apt-get install --no-install-recommends \
  prometheus prometheus-alertmanager prometheus-blackbox-exporter

install -m 0644 -o root -g root prometheus.yml       /etc/prometheus/prometheus.yml
install -m 0644 -o root -g root v2board-alerts.yml   /etc/prometheus/v2board-alerts.yml
install -m 0600 -o root -g root alertmanager.yml     /etc/prometheus/alertmanager.yml
install -m 0644 -o root -g root blackbox.yml         /etc/prometheus/blackbox.yml
```

Before starting anything, edit the two placeholders:

- `prometheus.yml`: replace `panel.example.com` with the canonical `app_url`
  hostname (the blackbox job probes the real public URL through Cloudflare).
- `alertmanager.yml`: replace the placeholder webhook receiver with the real
  paging integration. An Alertmanager with no working receiver silently drops
  pages.

Bind the daemons to loopback (Debian reads these from
`/etc/default/prometheus`, `/etc/default/prometheus-alertmanager`, and
`/etc/default/prometheus-blackbox-exporter`):

```text
prometheus            --web.listen-address=127.0.0.1:9090
prometheus-alertmanager --web.listen-address=127.0.0.1:9093
prometheus-blackbox-exporter --web.listen-address=127.0.0.1:9115
```

Then `systemctl restart prometheus prometheus-alertmanager
prometheus-blackbox-exporter` and check `promtool check config
/etc/prometheus/prometheus.yml` reports success.

## Why the scrape must be same-host

`/metrics` shares the `/healthz`/`/readyz` origin gate: in production it
answers only a direct loopback peer whose `Host` is exactly `127.0.0.1:8080`
and which carries no `CF-Connecting-IP` header. A Prometheus running on the
application host and targeting `127.0.0.1:8080` satisfies that gate with no
extra configuration; a remote Prometheus cannot reach it, and no Tunnel or WAF
route may ever be created for it. Worker heartbeats and job counters arrive
through the same endpoint (`v2board_worker_*`), re-exported by the API from
Redis — there is no second application scrape target.

## External uptime probing

The `v2board-public` blackbox job fetches the canonical public HTTPS URL, so
the request leaves through Cloudflare's edge and returns via the Tunnel —
proving the full ingress chain (DNS, edge TLS, tunnel connector, origin).
Because it still originates on the same host, it cannot detect problems that
only exist from elsewhere (regional DNS/routing, an expired payment method
suspending the zone). **Also register an independent third-party uptime
service** (UptimeRobot, Better Stack, a second Prometheus at another site, …)
against the same public URL with a several-minute alarm threshold.

## Alerts

`v2board-alerts.yml` implements the required-alert table from the runbook §1.2
one for one (same expressions, `severity: page|warn` labels), plus:

- `PublicURLProbeFailing` — the blackbox external-path probe above;
- `SlowRequestsP95` — p95 per route family above 1s for 10 minutes, from the
  `v2board_http_request_duration_seconds` histogram.

Infrastructure-level monitoring (node_exporter disk/inode alerts,
postgres_exporter, ClickHouse system tables, systemd unit state) remains
required and separate — runbook §1.3; the commented jobs in `prometheus.yml`
are the hook-in points.

## Grafana

Grafana is optional and not part of the alerting path. Install it from
Grafana's own apt repository (Debian does not package it), keep
`http_addr = 127.0.0.1` in `/etc/grafana/grafana.ini`, add the local
Prometheus (`http://127.0.0.1:9090`) as a data source, and import
`grafana-dashboard-v2board.json` (Dashboards → Import). The dashboard covers
availability stats, request/latency panels per route family, top routes,
worker heartbeat/job health, and the analytics outbox against its thresholds.
