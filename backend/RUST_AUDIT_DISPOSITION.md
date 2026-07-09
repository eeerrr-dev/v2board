# Rust Backend Audit — Findings Disposition

This records how the Laravel-vs-Rust behavioral audit of the Rust backend
rewrite was resolved. Every finding is either **fixed** (Rust now matches the
Laravel oracle), **intentional** (Rust deliberately diverges and the divergence
is safe and consumer-invisible), **deferred** (an internal refactor with no
behavior/contract impact), or **needs-infra** (a check that cannot run in the
hermetic `--no-deps` gate and belongs to the pre-cutover DB/oracle gates).

Reconciliation snapshot (2026-07-06): of the audited findings, 59 were already
fixed at HEAD, 8 partially fixed, 14 intentional Tier-2, 34 open, and 5 need
infrastructure. The open set was worked in the seven batches below; the
remaining categories are documented here as the standing disposition.

The tiering language (Tier-1 external contract vs Tier-2 presentation) is defined
in the repository `AGENTS.md`. When a case was ambiguous it was treated as
Tier-1 and matched to the oracle.

## Fixed (matched to the oracle)

Each batch verified the exact Laravel behavior before implementing and gated with
`cargo fmt --check`, `cargo clippy --workspace --all-targets --locked -D
warnings`, and `cargo test --workspace --locked`.

- **Batch 1 — subscription client URI bytes** (`5c9e330b`). Hysteria v1
  `obfsParam` emitted without `=`, and the `obfs=http` shadowsocks path dropped
  the `/`, matching `Helper.php`. Tier-1 (client-consumed URI bytes).
- **Batch 2 — server/node API parity** (`c49e3bb8`). Stat `record_at` uses the
  configured app timezone instead of the container `Local` (a real bug under a
  UTC container); the legacy `submit` path forces the fallback node type;
  duplicate `user_id` rows are last-write-wins; UniProxy `alivelist` validates
  the node; and the charged-traffic math `(u+d)*server_rate` is a tested pure
  function.
- **Batch 3 — workers** (`0f5abc03`). The Redis scheduler lock is released even
  when a scheduled job panics (the job runs on a joined task and a panic is
  logged and recorded as a failure), and `reset:traffic` excludes plan-less /
  orphan users via an inner join on `v2_plan`.
- **Batch 4 — order / payment** (`08427200`). AlipayF2F `biz_content`
  `total_amount` is emitted as an integer for whole-yuan amounts (Tier-1
  external payload); added a signed BTCPay happy-path test, a commission-amount
  pure-function test, and a hardcoded gateway MD5 notify digest fixture.
- **Batch 5 — auth / client contracts** (`c4164eb8`). `getStripePublicKey`
  returns `{data:null}` at 200 when the key is missing and only errors when the
  gateway row is missing; `token2Login` no longer trims a trailing `/` from
  `app_url`; `getResetDay` returns null for plan-less users and `newPeriod`
  rejects them.
- **Batch 6 — admin strict-replica** (`d002b3cb`). Coupon/giftcard fetch honor
  `sort`/`sort_type`; every admin drop/show/update runs a row-existence probe
  with the resource's own not-found error; `server/group/fetch` short-circuits on
  a truthy `group_id` and group/route fetch return id-ascending order;
  `order/detail` attaches `commission_log` and `surplus_orders`, and the order
  projections regained `actual_commission_balance`; coupon/giftcard/user
  `generate` validate up front like their FormRequests (HTTP 422, exact
  messages, and the untranslated `validation.*` keys Laravel actually emits
  where no custom message maps); and server `save` writes only present keys plus
  each controller's post-validation assignments, so a partial node edit no
  longer clobbers `sort` (and other unsubmitted columns) to a default.
- **Batch 7 — custom Clash YAML** (`0ecf37d0`). `custom.clash.yaml` /
  `custom.stash.yaml` operator overrides are parsed as YAML (via `serde_yaml`
  into a preserve-order value), matching `Symfony\Yaml::parseFile`; JSON-encoded
  custom files still load because YAML is a JSON superset.

## Intentional divergences (kept, documented)

These are deliberate; the Rust behavior is safe and no external party consumes
the difference. They are Tier-2 unless noted.

- **Payment notify non-success outcome returns 200, not 500.** For an ignored /
  non-success callback outcome the Rust handler acknowledges with 200 rather
  than the oracle's 500. The money path (signature verification and paid-order
  side effects) is identical and separately tested; a 200 ack is the safer
  response to a gateway retry and the transient status is not externally
  observable. This was verified per gateway before being kept.
- **Cross-gateway FX / cent amounts.** Gateways that convert via a live exchange
  rate cannot be reproduced cent-for-cent; the Rust value is computed from its
  own rate source. Matching the oracle byte-for-byte is impossible by design.
- **Hysteria / v2node `obfs_password` generation.** Rust derives a stable
  per-node value instead of Laravel's effectively constant `md5("")`; it is
  self-consistent and node-facing only.
- **CSV export `device_limit` column.** Laravel's `dumpCSV` reads a misspelled
  `devce_limit` key, so the column is always blank; Rust emits the real
  `device_limit`. A more-correct, display-only export difference.
- **Alive-list freshness.** Laravel caches the device-limited alive computation
  for 60s; Rust recomputes live, so mid-window counts can differ (fresher) and
  the DB/Redis load profile differs. Presentation/perf only.
- **Traffic-update accumulation key.** Rust keys accumulation off the union of
  upload/download rather than the download hash; strictly safer and unreachable
  under the normal single producer.
- **Reminder-mail subject language.** Laravel routes the expiry/traffic reminder
  subject through the translator (Chinese under a zh-CN locale); Rust emits the
  English subject. The trigger conditions match exactly; only the subject text
  differs (mail-rendering domain, Tier-2).
- **V2 `/server/config` ETag.** Rust uses a substring match where Laravel uses
  strict equality; node-facing and behavior-equivalent for the real client.
- **Free-renew of NULL/zero-price periods.** Rust disables the renew rather than
  mirroring Laravel's null-price free-renew; the safer choice, noted as a
  conscious deviation.

## Deferred — internal refactors (no behavior/contract impact)

Tracked for later cleanup; none change an external contract:

- A single data-access seam (the code currently mixes three SQL styles).
- A `PaymentGateway` trait to replace the gateway match arms.
- Splitting `subscription.rs` into a domain/protocols module tree.
- Splitting the `admin.rs` god-file, and the `main.rs` telegram/i18n sections.
- Shared worker structs, and an arc-swap for hot `AppConfig` reloads.

## Explicitly not ported (cutover scope)

- The `CheckServer` / `ClearUser` / `ResetUser` / `ResetPassword` artisan
  commands are manual-operations CLI, out of the first-release scope. Reintroduce
  as run-once worker subcommands if a deployment needs them.
- The `getThemes` init seed (theme subsystem) is low priority and not ported.

## Needs infrastructure (pre-cutover gates)

The hermetic gate runs `--no-deps` (no MySQL, Redis, or live Laravel), so these
cannot be asserted there and belong to the DB/oracle-backed cutover gates:

- Contract value-promotion checks (Exact/Selected) and subscription golden-byte
  fixtures.
- Admin write-path DB assertions, and `sqlx::test` / axum-handler integration
  tests.
- A normal-user (non-admin) contract login path.
- A stricter check than "Rust may improve a Laravel 5xx" that still inspects the
  Rust body so a wrong stat payload cannot pass unchecked.
- A `route:list`-based (rather than static) route audit.
