# Bare-metal fault-matrix fixture

This directory is destructive test infrastructure, not a production deploy
payload. The production release, production systemd units, and production
Docker export must never contain it.

The host-side supervisor is
`backend/rust/scripts/run-bare-metal-fault-matrix-supervisor.sh`. It discovers
the closed case catalog from the feature-gated guest binary; neither the
supervisor nor this fixture assumes a fixed number of fault points. Every case
starts from a clean disposable snapshot and is controlled from outside the
guest.

## Fixed guest paths

- guest binary:
  `/opt/v2board/fault-matrix/v2board-bare-metal-fault-matrix-guest`
- manifest: `/var/lib/v2board/fault-matrix/manifest.json`
- disposable marker:
  `/etc/v2board/bare-metal-fault-matrix-disposable.json`
- durable controller/ready record:
  `/var/lib/v2board/fault-matrix/control/fault-ready.json`
- final guest evidence: `/var/lib/v2board/fault-matrix/outcome-evidence.json`

The dedicated unit in `systemd/` has no `[Install]` section and must be started
transiently by the adapter. Before each invocation the adapter atomically
writes root:root mode-`0600` `/run/v2board-fault-matrix/guest.env`, containing
only `V2BOARD_MATRIX_ACTION=start|resume` and the catalog-bound
`V2BOARD_MATRIX_CASE=<point>--<mode>`. The unit deliberately runs as root because the
test must exercise the real systemd/datastore executor. Do not weaken the
machine-bound marker check to make a non-disposable host pass.
`/run` is used only for that transient systemd environment file. The consumed
fault record is root:root mode-`0600` below the persistent root:root mode-`0700`
`/var/lib/v2board/fault-matrix/control` directory, so a hard reset cannot erase
the single-injection proof before resume.

## External adapter protocol

The supervisor accepts one absolute, non-symlink executable. Each action below
must fail nonzero on ambiguity. It must never print manifest contents or
secrets. JSON files are written atomically, fsynced, mode `0600`, and never
silently replaced.

| Action | Required behavior |
| --- | --- |
| `preflight OUT` | Write supervisor-protocol JSON declaring `adapter_ready`, `snapshot_reset_capable`, `hard_reset_capable`, `adapter_kind`, `guest_id`, and the fixed marker path. |
| `catalog RUN OPERATION GUEST_BINARY GUEST_SHA OUT` | Run `list-cases` from the exact supervisor-supplied feature artifact for the canonical UUID run/operation binding and copy its catalog to `OUT`; reject a binary digest mismatch. |
| `prepare CASE GUEST_CASE POINT GUEST_MODE CASE_BINDING MECHANISM RUN OPERATION REVISION MANIFEST_SHA RELEASE_SHA GUEST_SHA CATALOG_SHA MANIFEST RELEASE GUEST_BINARY OUT` | Revert/create a clean guest; copy exact inputs; install exactly `GUEST_BINARY`; verify `GUEST_SHA`; install the release input as root:root mode-`0400`, nlink-1 at the manifest's fixed operation path; create a root:root, mode-`0600`, nlink-1 marker bound to that digest, machine-id, snapshot, all supplied digests, case and mechanism; write preparation evidence to `OUT`. |
| `start CASE` | Atomically select `start` in `guest.env`, then start `v2board-fault-matrix-guest.service` with `--no-block`. |
| `wait-ready CASE OUT` | Wait with a bounded deadline for `/var/lib/v2board/fault-matrix/control/fault-ready.json`, then copy that exact fsynced record to `OUT`. Never treat a file below `/run` as the consumed fault proof. |
| `wait-injected-failure CASE OUT` | For `before`/`lost_acknowledgement`, prove the unit failed with `Result=exit-code`, its nonzero status and exact guest injected error code; bind case/run/operation/ready SHA in `OUT`. |
| `sigkill CASE OUT` | Verify ready PID equals MainPID, issue `systemctl kill --kill-who=main --signal=SIGKILL`, prove that PID died and the unit failed; bind case/run/operation/ready SHA in `OUT`. |
| `hard-reset CASE OUT` | Before reset, prove ready PID equals the live unit MainPID; then hard-reset through the hypervisor/cloud control plane and prove the same guest/snapshot/machine has a changed Linux boot ID and provider event ID; bind case/run/operation/ready SHA in `OUT`. |
| `resume CASE` | Reset the failed unit, atomically select `resume` in `guest.env`, start the same guest unit, and wait for successful completion and native readiness. |
| `collect CASE OUTCOME ADMISSION` | Copy the exact guest `outcome-evidence.json` and `admission-evidence.json`; do not wrap or rewrite either file. |
| `verify-postconditions CASE GUEST_OUTCOME_SHA OUT` | Freshly verify permanent ledger, source retirement, native systemd/PID readiness, and logical single-effect counts; write the supervisor postcondition envelope. |
| `cleanup CASE` | Destroy or revert the guest. It must be idempotent and may be called after any failed action. |

`CASE` is the supervisor evidence identity. `GUEST_CASE` is the catalog identity;
they differ only for the optional hard-reset replay. The adapter must retain
case state outside the guest so a guest hard reset cannot erase its control
coordinates.
The canonical Make flow rebuilds the release-mode feature binary from the clean
bound worktree inside the pinned Rust Docker environment and byte-compares it
with `BARE_METAL_MATRIX_GUEST_BINARY` before any adapter action. A stale binary
with a self-consistent hash is therefore rejected rather than accepted as
evidence for a different revision.
The supervisor itself invokes `make bare-metal-fault-matrix-verify-guest` after
its clean-HEAD check, so calling the script directly cannot bypass this proof.
The guest catalog must also report the compiled global production capability
as unavailable; the summary records that observed value and never claims that
the matrix changed the production gate.

The adapter JSON envelope uses
`v2board-bare-metal-fault-matrix-supervisor-v1`. The catalog itself uses
`v2board-bare-metal-fault-catalog-v1` and contains unique objects with
`case_id`, `fault_point`, `mode`, and `case_binding_sha256`. Supported guest
modes are exactly `before`, `lost_acknowledgement`, and `sigkill_ready`.
Marker `interruption_mechanism` is exactly `process_error` for the first two
modes, `sigkill` for an ordinary `sigkill_ready` case, and `hard_reset` for its
optional hypervisor replay.

## Hard reset and manual operation

`--hard-reset adapter` adds a separate expected set by replaying every
`sigkill_ready` point with an observed hypervisor hard reset. A pass requires a
provider event ID and different before/after `/proc/sys/kernel/random/boot_id`
values. `--hard-reset manual` only emits a `status=not_run` request. A note,
screenshot, typed confirmation, or manually authored receipt never counts as a
pass. An interactive operator may implement the `hard-reset` adapter action,
but the supervisor still requires machine-observed boot evidence and completes
the run itself.

## Cleanup

`cleanup-guest.sh` is preview-only unless `--apply` is supplied. It refuses to
operate without the exact root-owned `0600` disposable marker and matching
guest ID. It removes only matrix-specific paths; the hypervisor adapter remains
responsible for destroying/reverting the complete guest and its datastores.
