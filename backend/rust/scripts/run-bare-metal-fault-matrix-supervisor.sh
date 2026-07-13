#!/usr/bin/env bash
set -euo pipefail

PROTOCOL=v2board-bare-metal-fault-matrix-supervisor-v1
MARKER_PATH=/etc/v2board/bare-metal-fault-matrix-disposable.json

execute=0
adapter=
manifest=
release=
guest_binary=
revision=
output=
hard_reset_mode=not-run
active_case=
cleanup_needed=0

usage() {
  cat <<'EOF'
Usage:
  run-bare-metal-fault-matrix-supervisor.sh [options]

Default: print a dry-run summary and do not contact or mutate a guest.

Execution options (all required with --execute):
  --execute                 Run every case in the guest catalog.
  --adapter ABSOLUTE_PATH   External guest/hypervisor adapter (see fixture README).
  --manifest ABSOLUTE_PATH  Owner-only v4 migration manifest staged into each guest.
  --release ABSOLUTE_PATH   Authorized native release archive.
  --guest-binary ABS_PATH   Feature-gated guest built from the bound clean revision.
  --revision GIT_SHA        Lowercase 40-hex source revision.
  --output ABSOLUTE_PATH    New local evidence directory; no-clobber.
  --hard-reset MODE         not-run (default), adapter, or manual.

The standard catalog is discovered from the matrix guest binary. Its size is
never hard-coded. `adapter` additionally runs every sigkill_ready fault point
through a machine-observed hypervisor hard reset. `manual` only emits an
explicit not-run request; a human receipt can never be counted as a pass.
EOF
}

die() {
  printf 'fault-matrix supervisor: %s\n' "$*" >&2
  exit 64
}

sha256_file() {
  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum -- "$1" | awk '{print $1}'
  else
    shasum -a 256 -- "$1" | awk '{print $1}'
  fi
}

is_sha256() {
  [[ $1 =~ ^[0-9a-f]{64}$ ]]
}

is_safe_identifier() {
  [[ $1 =~ ^[a-z0-9][a-z0-9._:-]{0,191}$ ]]
}

require_regular() {
  [[ -f $1 && ! -L $1 ]] || die "$2 must be a regular non-symlink file"
}

file_mode() {
  stat -c '%a' "$1" 2>/dev/null || stat -f '%Lp' "$1"
}

file_nlink() {
  stat -c '%h' "$1" 2>/dev/null || stat -f '%l' "$1"
}

adapter_call() {
  local action=$1
  shift
  "$adapter" "$action" "$@"
}

cleanup_active_case() {
  if ((cleanup_needed)) && [[ -n $active_case && -x $adapter ]]; then
    adapter_call cleanup "$active_case" >/dev/null 2>&1 || true
  fi
}

trap cleanup_active_case EXIT INT TERM

while (($#)); do
  case "$1" in
    --execute)
      execute=1
      shift
      ;;
    --adapter|--manifest|--release|--guest-binary|--revision|--output|--hard-reset)
      (($# >= 2)) || die "missing value for $1"
      case "$1" in
        --adapter) adapter=$2 ;;
        --manifest) manifest=$2 ;;
        --release) release=$2 ;;
        --guest-binary) guest_binary=$2 ;;
        --revision) revision=$2 ;;
        --output) output=$2 ;;
        --hard-reset) hard_reset_mode=$2 ;;
      esac
      shift 2
      ;;
    --help|-h)
      usage
      exit 0
      ;;
    *) die "unknown argument: $1" ;;
  esac
done

case "$hard_reset_mode" in
  not-run|adapter|manual) ;;
  *) die '--hard-reset must be not-run, adapter, or manual' ;;
esac

if ((!execute)); then
  cat <<EOF
Bare-metal matrix dry run
  protocol:        $PROTOCOL
  guest marker:    $MARKER_PATH
  standard cases:  discovered dynamically from guest list-cases
  interruption:    before, lost_acknowledgement, sigkill_ready
  hard reset:      $hard_reset_mode (separate evidence set; never inferred)

No guest was contacted and no fault was injected. Use --execute with all
required paths after reviewing the adapter contract and disposable-guest gate.
EOF
  exit 0
fi

command -v jq >/dev/null 2>&1 || die 'jq is required'
[[ $guest_binary == /* ]] || die '--guest-binary must be absolute'
require_regular "$guest_binary" '--guest-binary'
[[ $adapter == /* && -x $adapter && -f $adapter && ! -L $adapter ]] || \
  die '--adapter must be an absolute executable regular file'
[[ $manifest == /* ]] || die '--manifest must be absolute'
[[ $release == /* ]] || die '--release must be absolute'
[[ $output == /* ]] || die '--output must be absolute'
require_regular "$manifest" '--manifest'
require_regular "$release" '--release'
[[ $(file_mode "$manifest") == 600 ]] || \
  die '--manifest must have mode 0600'
[[ $(file_nlink "$manifest") == 1 ]] || die '--manifest must have exactly one hard link'
[[ $(file_nlink "$release") == 1 ]] || die '--release must have exactly one hard link'
[[ $(file_nlink "$guest_binary") == 1 ]] || die '--guest-binary must have exactly one hard link'
case $(file_mode "$guest_binary") in
  500|555|700|755) ;;
  *) die '--guest-binary must use exact executable mode 0500, 0555, 0700, or 0755' ;;
esac
[[ $revision =~ ^[0-9a-f]{40}$ ]] || \
  die '--revision must be a lowercase 40-hex source revision'
[[ ! -e $output ]] || die '--output already exists (no-clobber)'
repo_root=$(git -C "$(dirname "$0")" rev-parse --show-toplevel 2>/dev/null) || \
  die 'supervisor must run from a git worktree'
case "$output" in
  "$repo_root"|"$repo_root"/*) die '--output must be outside the source worktree' ;;
esac
[[ $(git -C "$repo_root" rev-parse HEAD) == "$revision" ]] || \
  die '--revision does not match the supervisor worktree HEAD'
[[ -z $(git -C "$repo_root" status --porcelain=v1 --untracked-files=all) ]] || \
  die 'matrix evidence requires a clean worktree at the bound revision'
command -v make >/dev/null 2>&1 || die 'make is required for guest rebuild verification'
make --no-print-directory -C "$repo_root" bare-metal-fault-matrix-verify-guest \
  "BARE_METAL_MATRIX_GUEST_BINARY=$guest_binary" \
  "BARE_METAL_MATRIX_REVISION=$revision" || \
  die 'guest binary does not byte-match a clean-revision Docker rebuild'
guest_rebuild_verified=true

umask 077
mkdir "$output"
mkdir "$output/cases" "$output/logs"

manifest_sha256=$(sha256_file "$manifest")
release_sha256=$(sha256_file "$release")
guest_binary_sha256=$(sha256_file "$guest_binary")
adapter_sha256=$(sha256_file "$adapter")
operation_id=$(jq -er '.operation_id | select(test("^[0-9a-f]{8}-[0-9a-f]{4}-[1-8][0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$"))' \
  "$manifest") || die 'manifest operation_id must be a canonical lowercase UUID'
run_seed=$(printf '%s\0%s\0%s\0%s\0%s\0%s\0%s\0' "$operation_id" "$revision" "$manifest_sha256" \
  "$release_sha256" "$guest_binary_sha256" "$output" "$(date -u +%Y-%m-%dT%H:%M:%SZ)" | {
    if command -v sha256sum >/dev/null 2>&1; then sha256sum; else shasum -a 256; fi
  } | awk '{print $1}')
run_hex="${run_seed:0:12}4${run_seed:13:3}8${run_seed:17:15}"
run_id="${run_hex:0:8}-${run_hex:8:4}-${run_hex:12:4}-${run_hex:16:4}-${run_hex:20:12}"

adapter_call preflight "$output/adapter-preflight.json" \
  >"$output/logs/preflight.stdout" 2>"$output/logs/preflight.stderr"
jq -e --arg protocol "$PROTOCOL" --arg marker "$MARKER_PATH" '
  .protocol == $protocol and
  .adapter_ready == true and
  .snapshot_reset_capable == true and
  .disposable_marker_path == $marker and
  (.adapter_kind | type == "string" and length > 0) and
  (.guest_id | type == "string" and length > 0)
' "$output/adapter-preflight.json" >/dev/null || die 'adapter preflight contract failed'

adapter_call catalog "$run_id" "$operation_id" "$guest_binary" "$guest_binary_sha256" \
  "$output/catalog.json" \
  >"$output/logs/catalog.stdout" 2>"$output/logs/catalog.stderr"
jq -e --arg run "$run_id" --arg operation "$operation_id" '
  .protocol == "v2board-bare-metal-fault-catalog-v1" and
  .run_id == $run and .operation_id == $operation and
  .production_capability_available == false and
  (.fault_point_set_sha256 | test("^[0-9a-f]{64}$")) and
  (.cases | type == "array" and length > 0) and
  ([.cases[].case_id] | length == (unique | length)) and
  ([.cases[] | [.fault_point,.mode]] | length == (unique | length)) and
  ([.cases | group_by(.fault_point)[] |
      ([.[].mode] | sort) == ["before","lost_acknowledgement","sigkill_ready"]] | all) and
  all(.cases[];
    (.case_id | type == "string" and test("^[a-z0-9][a-z0-9._:-]{0,191}$")) and
    (.fault_point | type == "string" and test("^[a-z0-9][a-z0-9._:-]{0,191}$")) and
    (.mode == "before" or .mode == "lost_acknowledgement" or .mode == "sigkill_ready") and
    (.case_binding_sha256 | test("^[0-9a-f]{64}$")))
' "$output/catalog.json" >/dev/null || die 'guest catalog contract failed'

catalog_sha256=$(sha256_file "$output/catalog.json")
fault_point_set_sha256=$(jq -r '.fault_point_set_sha256' "$output/catalog.json")
production_capability_available=$(jq -r '.production_capability_available' "$output/catalog.json")
expected_case_count=$(jq '.cases | length' "$output/catalog.json")
jq -cS '.cases[]' "$output/catalog.json" >"$output/expected-standard.ndjson"
jq -r '.cases[] | [.case_id,.fault_point,.mode,.case_binding_sha256] | @tsv' \
  "$output/catalog.json" >"$output/expected-standard.tsv"
expected_case_set_sha256=$(sha256_file "$output/expected-standard.ndjson")
jq -cS '.cases[] | select(.mode == "sigkill_ready") |
  {case_id:(.case_id + ":hard-reset"),guest_case_id:.case_id,
   fault_point:.fault_point,guest_mode:.mode,case_binding_sha256:.case_binding_sha256,
   interruption_mechanism:"hard_reset"}' \
  "$output/catalog.json" >"$output/expected-hard-reset.ndjson"
jq -r '.cases[] | select(.mode == "sigkill_ready") |
  [.case_id,.fault_point,.mode,.case_binding_sha256] | @tsv' \
  "$output/catalog.json" >"$output/expected-hard-reset.tsv"
hard_reset_expected_case_count=$(wc -l <"$output/expected-hard-reset.ndjson" | tr -d ' ')
hard_reset_case_set_sha256=$(sha256_file "$output/expected-hard-reset.ndjson")

validate_prepare() {
  local file=$1 case_id=$2 point=$3 guest_mode=$4 case_binding=$5 mechanism=$6
  jq -e --arg protocol "$PROTOCOL" --arg case_id "$case_id" --arg point "$point" \
    --arg mode "$guest_mode" --arg binding "$case_binding" --arg run "$run_id" \
    --arg operation "$operation_id" \
    --arg revision "$revision" --arg manifest "$manifest_sha256" --arg release "$release_sha256" \
    --arg guest_binary "$guest_binary_sha256" \
    --arg catalog "$fault_point_set_sha256" --arg marker "$MARKER_PATH" --arg mechanism "$mechanism" '
      .protocol == $protocol and .case_id == $case_id and .fault_point == $point and
      .guest_mode == $mode and .case_binding_sha256 == $binding and .run_id == $run and
      .operation_id == $operation and
      .source_revision == $revision and .manifest_sha256 == $manifest and
      .release_sha256 == $release and .guest_binary_sha256 == $guest_binary and
      .fault_point_set_sha256 == $catalog and
      .disposable_marker_path == $marker and .disposable_marker_verified == true and
      .snapshot_reset_verified == true and .interruption_mechanism == $mechanism and
      (.machine_id_sha256 | test("^[0-9a-f]{64}$")) and
      (.guest_id | type == "string" and length > 0) and
      (.snapshot_id | type == "string" and length > 0)
    ' "$file" >/dev/null
}

validate_ready() {
  local file=$1 point=$2 guest_mode=$3 case_binding=$4
  local expected_phase=after_success expected_action=sigkill_ready
  if [[ $guest_mode == before ]]; then
    expected_phase=before
    expected_action=injected_before
  elif [[ $guest_mode == lost_acknowledgement ]]; then
    expected_action=injected_lost_acknowledgement
  fi
  jq -e --arg point "$point" --arg mode "$guest_mode" --arg binding "$case_binding" \
    --arg run "$run_id" --arg operation "$operation_id" --arg catalog "$fault_point_set_sha256" \
    --arg phase "$expected_phase" --arg action "$expected_action" '
      .protocol_version == 1 and .point == $point and .mode == $mode and
      .case_binding_sha256 == $binding and .run_id == $run and
      .operation_id == $operation and
      .catalog_sha256 == $catalog and .phase == $phase and .action == $action and
      (.pid | type == "number" and . > 1)
    ' "$file" >/dev/null
}

validate_result() {
  local file=$1 guest_case_id=$2 point=$3 guest_mode=$4 case_binding=$5 ready_sha=$6
  local mechanism=$7 prepare=$8 guest_id snapshot_id
  guest_id=$(jq -r '.guest_id' "$prepare")
  snapshot_id=$(jq -r '.snapshot_id' "$prepare")
  jq -e --arg case_id "$guest_case_id" --arg point "$point" \
    --arg mode "$guest_mode" --arg binding "$case_binding" --arg run "$run_id" \
    --arg operation "$operation_id" \
    --arg revision "$revision" --arg manifest "$manifest_sha256" --arg release "$release_sha256" \
    --arg catalog "$fault_point_set_sha256" --arg ready "$ready_sha" \
    --arg mechanism "$mechanism" --arg guest_id "$guest_id" --arg snapshot_id "$snapshot_id" '
      .protocol == "v2board-bare-metal-fault-matrix-guest-outcome-v1" and
      .status == "passed" and .case_id == $case_id and .fault_point == $point and
      .mode == $mode and .case_binding_sha256 == $binding and .run_id == $run and
      .operation_id == $operation and
      .interruption_mechanism == $mechanism and .guest_id == $guest_id and
      .snapshot_id == $snapshot_id and
      .source_revision == $revision and .manifest_sha256 == $manifest and
      .release_sha256 == $release and .fault_point_set_sha256 == $catalog and
      .ready_sha256 == $ready and .secrets_redacted == true and
      (.admission_evidence_sha256 | test("^[0-9a-f]{64}$")) and
      .result.operation_id == $operation and .result.completed == true and
      .result.mysql_runtime_retired == true and
      (.result.journal_event_sha256 | test("^[0-9a-f]{64}$")) and
      (.result.journal_generation | type == "number" and . > 0)
    ' "$file" >/dev/null
}

validate_admission() {
  local file=$1 guest_case_id=$2 point=$3 guest_mode=$4 case_binding=$5 mechanism=$6
  local prepare=$7
  local guest_id snapshot_id machine_id_sha256
  guest_id=$(jq -r '.guest_id' "$prepare")
  snapshot_id=$(jq -r '.snapshot_id' "$prepare")
  machine_id_sha256=$(jq -r '.machine_id_sha256' "$prepare")
  jq -e --arg case_id "$guest_case_id" --arg point "$point" --arg mode "$guest_mode" \
    --arg binding "$case_binding" --arg mechanism "$mechanism" --arg run "$run_id" \
    --arg operation "$operation_id" --arg catalog "$fault_point_set_sha256" \
    --arg revision "$revision" --arg manifest "$manifest_sha256" \
    --arg release "$release_sha256" --arg guest_binary "$guest_binary_sha256" \
    --arg guest_id "$guest_id" --arg snapshot_id "$snapshot_id" \
    --arg machine_id_sha256 "$machine_id_sha256" '
      .format == "v2board-bare-metal-fault-matrix-admission-v1" and .status == "admitted" and
      .operation_id == $operation and .run_id == $run and .fault_case == $case_id and
      .fault_point == $point and .mode == $mode and .case_binding_sha256 == $binding and
      .fault_catalog_sha256 == $catalog and .guest_id == $guest_id and
      .snapshot_id == $snapshot_id and .interruption_mechanism == $mechanism and
      .manifest_sha256 == $manifest and .source_revision == $revision and
      .release.archive_sha256 == $release and .release.source_revision == $revision and
      .guest_binary_sha256 == $guest_binary and
      .release.target_os == "linux" and .release.target_arch == "amd64" and
      .machine.machine_id_sha256 == $machine_id_sha256 and
      (.machine.os_release_sha256 | test("^[0-9a-f]{64}$")) and
      all([.machine.kernel_release,.machine.systemd_version,.machine.mysql_version,
        .machine.postgres_version,.machine.redis_version,.machine.clickhouse_version][];
        type == "string" and length > 0) and .secrets_redacted == true
    ' "$file" >/dev/null
}

validate_postconditions() {
  local file=$1 case_id=$2 guest_case_id=$3 mechanism=$4 result_sha=$5
  jq -e --arg protocol "$PROTOCOL" --arg case_id "$case_id" \
    --arg guest_case_id "$guest_case_id" --arg mechanism "$mechanism" --arg run "$run_id" \
    --arg operation "$operation_id" --arg result "$result_sha" '
      .protocol == $protocol and .case_id == $case_id and .guest_case_id == $guest_case_id and
      .run_id == $run and .operation_id == $operation and
      .interruption_mechanism == $mechanism and .guest_outcome_sha256 == $result and
      .postconditions_checked_after_resume == true and
      .logical_effect_count_exact == true and .permanent_ledger_completed == true and
      .native_services_ready == true and .source_runtime_retired == true and
      .outcome == "passed"
    ' "$file" >/dev/null
}

validate_interruption() {
  local file=$1 case_id=$2 guest_mode=$3 mechanism=$4 ready_sha=$5 ready_pid=$6
  local prepare=$7 guest_id snapshot_id machine_id_sha256
  guest_id=$(jq -r '.guest_id' "$prepare")
  snapshot_id=$(jq -r '.snapshot_id' "$prepare")
  machine_id_sha256=$(jq -r '.machine_id_sha256' "$prepare")
  jq -e --arg protocol "$PROTOCOL" --arg case_id "$case_id" --arg run "$run_id" \
    --arg operation "$operation_id" --arg mechanism "$mechanism" --arg ready "$ready_sha" '
      .protocol == $protocol and .case_id == $case_id and .run_id == $run and
      .operation_id == $operation and .interruption_mechanism == $mechanism and
      .ready_sha256 == $ready
    ' "$file" >/dev/null || return 1
  case "$mechanism" in
    process_error)
      local expected_code=matrix_injected_lost_acknowledgement
      [[ $guest_mode == before ]] && expected_code=matrix_injected_before_mutation
      jq -e --arg code "$expected_code" '
        .unit == "v2board-fault-matrix-guest.service" and .unit_failed == true and
        .service_result == "exit-code" and .guest_error_code == $code and
        (.exit_status | type == "number" and . != 0) and .main_pid_exited == true
      ' "$file" >/dev/null
      ;;
    sigkill)
      jq -e --argjson pid "$ready_pid" '
        .unit == "v2board-fault-matrix-guest.service" and
        .ready_pid == $pid and .main_pid_before == $pid and .signal == "SIGKILL" and
        .main_pid_after == 0 and .process_dead == true and .unit_failed == true
      ' "$file" >/dev/null
      ;;
    hard_reset)
      jq -e --argjson pid "$ready_pid" --arg guest_id "$guest_id" \
        --arg snapshot_id "$snapshot_id" --arg machine "$machine_id_sha256" '
        .ready_pid == $pid and .main_pid_before == $pid and
        .process_alive_before_reset == true and
        .guest_id == $guest_id and .snapshot_id == $snapshot_id and
        .machine_id_sha256_before == $machine and .machine_id_sha256_after == $machine and
        .hypervisor_reset_observed == true and
        (.boot_id_before | type == "string" and length > 0) and
        (.boot_id_after | type == "string" and length > 0) and
        .boot_id_before != .boot_id_after and
        (.hypervisor_event_id | type == "string" and length > 0)' "$file" >/dev/null
      ;;
    *) return 1 ;;
  esac
}

run_case() {
  local case_id=$1 guest_case_id=$2 point=$3 guest_mode=$4 case_binding=$5 mechanism=$6
  local case_dir="$output/cases/$case_id"
  is_safe_identifier "$case_id" || die "unsafe case id from catalog: $case_id"
  mkdir "$case_dir"
  active_case=$case_id
  cleanup_needed=1

  adapter_call prepare "$case_id" "$guest_case_id" "$point" "$guest_mode" "$case_binding" \
    "$mechanism" "$run_id" "$operation_id" "$revision" "$manifest_sha256" "$release_sha256" \
    "$guest_binary_sha256" "$fault_point_set_sha256" "$manifest" "$release" "$guest_binary" \
    "$case_dir/prepare.json" \
    >"$output/logs/$case_id.prepare.stdout" 2>"$output/logs/$case_id.prepare.stderr"
  validate_prepare "$case_dir/prepare.json" "$case_id" "$point" "$guest_mode" \
    "$case_binding" "$mechanism" || die "prepare evidence failed for $case_id"

  adapter_call start "$case_id" \
    >"$output/logs/$case_id.start.stdout" 2>"$output/logs/$case_id.start.stderr"
  adapter_call wait-ready "$case_id" "$case_dir/fault-ready.json" \
    >"$output/logs/$case_id.ready.stdout" 2>"$output/logs/$case_id.ready.stderr"
  validate_ready "$case_dir/fault-ready.json" "$point" "$guest_mode" "$case_binding" || \
    die "fault-ready evidence failed for $case_id"
  ready_sha256=$(sha256_file "$case_dir/fault-ready.json")
  ready_pid=$(jq -r '.pid' "$case_dir/fault-ready.json")

  case "$mechanism" in
    process_error)
      adapter_call wait-injected-failure "$case_id" "$case_dir/interruption.json" \
        >"$output/logs/$case_id.interrupt.stdout" 2>"$output/logs/$case_id.interrupt.stderr"
      ;;
    sigkill)
      adapter_call sigkill "$case_id" "$case_dir/interruption.json" \
        >"$output/logs/$case_id.interrupt.stdout" 2>"$output/logs/$case_id.interrupt.stderr"
      ;;
    hard_reset)
      adapter_call hard-reset "$case_id" "$case_dir/interruption.json" \
        >"$output/logs/$case_id.interrupt.stdout" 2>"$output/logs/$case_id.interrupt.stderr"
      ;;
    *) die "internal interruption mechanism error: $mechanism" ;;
  esac
  validate_interruption "$case_dir/interruption.json" "$case_id" "$guest_mode" \
    "$mechanism" "$ready_sha256" "$ready_pid" "$case_dir/prepare.json" || \
    die "interruption evidence failed for $case_id"

  adapter_call resume "$case_id" \
    >"$output/logs/$case_id.resume.stdout" 2>"$output/logs/$case_id.resume.stderr"
  adapter_call collect "$case_id" "$case_dir/result.json" "$case_dir/admission.json" \
    >"$output/logs/$case_id.collect.stdout" 2>"$output/logs/$case_id.collect.stderr"
  validate_admission "$case_dir/admission.json" "$guest_case_id" "$point" "$guest_mode" \
    "$case_binding" "$mechanism" "$case_dir/prepare.json" || die "admission evidence failed for $case_id"
  admission_sha256=$(sha256_file "$case_dir/admission.json")
  validate_result "$case_dir/result.json" "$guest_case_id" "$point" "$guest_mode" \
    "$case_binding" "$ready_sha256" "$mechanism" "$case_dir/prepare.json" || \
    die "result evidence failed for $case_id"
  [[ $(jq -r '.admission_evidence_sha256' "$case_dir/result.json") == "$admission_sha256" ]] || \
    die "result/admission digest mismatch for $case_id"

  case_sha256=$(sha256_file "$case_dir/result.json")
  adapter_call verify-postconditions "$case_id" "$case_sha256" "$case_dir/postconditions.json" \
    >"$output/logs/$case_id.postconditions.stdout" 2>"$output/logs/$case_id.postconditions.stderr"
  validate_postconditions "$case_dir/postconditions.json" "$case_id" "$guest_case_id" \
    "$mechanism" "$case_sha256" || die "postcondition evidence failed for $case_id"
  postconditions_sha256=$(sha256_file "$case_dir/postconditions.json")
  jq -cn --arg case_id "$case_id" --arg guest_case_id "$guest_case_id" \
    --arg point "$point" --arg mode "$guest_mode" --arg mechanism "$mechanism" \
    --arg binding "$case_binding" --arg ready "$ready_sha256" --arg admission "$admission_sha256" \
    --arg result "$case_sha256" \
    --arg postconditions "$postconditions_sha256" \
    '{case_id:$case_id,guest_case_id:$guest_case_id,fault_point:$point,guest_mode:$mode,
      interruption_mechanism:$mechanism,case_binding_sha256:$binding,
      ready_sha256:$ready,admission_sha256:$admission,result_sha256:$result,
      postconditions_sha256:$postconditions,
      outcome:"passed"}' \
    >>"$output/completed.ndjson"

  adapter_call cleanup "$case_id" \
    >"$output/logs/$case_id.cleanup.stdout" 2>"$output/logs/$case_id.cleanup.stderr"
  cleanup_needed=0
  active_case=
}

while IFS=$'\t' read -r case_id point mode case_binding; do
  mechanism=process_error
  [[ $mode == sigkill_ready ]] && mechanism=sigkill
  run_case "$case_id" "$case_id" "$point" "$mode" "$case_binding" "$mechanism"
done <"$output/expected-standard.tsv"

hard_reset_status=not_run
hard_reset_complete_case_count=0
if [[ $hard_reset_mode == adapter ]]; then
  jq -e '.hard_reset_capable == true' "$output/adapter-preflight.json" >/dev/null || \
    die 'adapter preflight does not declare hard_reset_capable=true'
  while IFS=$'\t' read -r guest_case_id point guest_mode case_binding; do
    case_id="${guest_case_id}:hard-reset"
    run_case "$case_id" "$guest_case_id" "$point" "$guest_mode" "$case_binding" \
      hard_reset
    ((hard_reset_complete_case_count+=1))
  done <"$output/expected-hard-reset.tsv"
  [[ $hard_reset_expected_case_count -gt 0 ]] || \
    die 'hard-reset catalog set is unexpectedly empty'
  [[ $hard_reset_complete_case_count -eq $hard_reset_expected_case_count ]] || \
    die 'completed hard-reset case set is not the exact expected closure'
  hard_reset_status=passed
elif [[ $hard_reset_mode == manual ]]; then
  jq -cn --arg protocol "$PROTOCOL" --arg run_id "$run_id" \
    --arg catalog "$fault_point_set_sha256" '
      {protocol:$protocol,run_id:$run_id,fault_point_set_sha256:$catalog,
       status:"not_run",reason:"manual_hard_reset_requires_machine_observed_adapter_evidence",
       counts_toward_pass:false}' >"$output/manual-hard-reset-request.json"
fi

complete_case_count=$(wc -l <"$output/completed.ndjson" | tr -d ' ')
standard_complete_count=$(jq -s '[.[] | select(.interruption_mechanism != "hard_reset")] | length' \
  "$output/completed.ndjson")
[[ $standard_complete_count -eq $expected_case_count ]] || \
  die 'completed standard case set is not the exact catalog closure'

jq -sS 'sort_by(.case_id)' "$output/completed.ndjson" >"$output/case-index.json"
case_index_sha256=$(sha256_file "$output/case-index.json")
generated_at=$(date -u +%Y-%m-%dT%H:%M:%SZ)
jq -cn --arg protocol "$PROTOCOL" --arg run_id "$run_id" --arg operation_id "$operation_id" --arg revision "$revision" \
  --arg manifest "$manifest_sha256" --arg release "$release_sha256" \
  --arg guest_binary "$guest_binary_sha256" --arg adapter "$adapter_sha256" \
  --arg catalog_file "$catalog_sha256" \
  --arg point_set "$fault_point_set_sha256" --arg expected_cases "$expected_case_set_sha256" \
  --arg hard_cases "$hard_reset_case_set_sha256" --arg index "$case_index_sha256" \
  --arg hard_reset_status "$hard_reset_status" --arg generated_at "$generated_at" \
  --argjson guest_rebuild_verified "$guest_rebuild_verified" \
  --argjson production_capability_available "$production_capability_available" \
  --argjson expected "$expected_case_count" --argjson complete "$standard_complete_count" \
  --argjson all_complete "$complete_case_count" \
  --argjson hard_expected "$hard_reset_expected_case_count" \
  --argjson hard_complete "$hard_reset_complete_case_count" '
  {protocol:$protocol,run_id:$run_id,operation_id:$operation_id,source_revision:$revision,
   manifest_sha256:$manifest,release_sha256:$release,
   guest_binary_sha256:$guest_binary,adapter_sha256:$adapter,
   catalog_file_sha256:$catalog_file,fault_point_set_sha256:$point_set,
   expected_case_set_sha256:$expected_cases,
   expected_case_count:$expected,complete_case_count:$complete,
   exact_standard_closure:($expected == $complete),
   hard_reset:{status:$hard_reset_status,expected_case_count:$hard_expected,
     expected_case_set_sha256:$hard_cases,
     complete_case_count:$hard_complete},all_completed_case_count:$all_complete,
   case_index_sha256:$index,generated_at:$generated_at,
   guest_rebuild_verified:$guest_rebuild_verified,
   production_capability_available:$production_capability_available}' >"$output/summary.json"

summary_sha256=$(sha256_file "$output/summary.json")
printf 'Bare-metal matrix evidence completed.\n  summary: %s\n  sha256: %s\n' \
  "$output/summary.json" "$summary_sha256"
