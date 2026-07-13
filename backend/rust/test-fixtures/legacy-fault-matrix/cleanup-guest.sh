#!/usr/bin/env bash
set -euo pipefail

MARKER=/etc/v2board/bare-metal-fault-matrix-disposable.json
RUN_ROOT=/run/v2board-fault-matrix
STATE_ROOT=/var/lib/v2board/fault-matrix
INSTALL_ROOT=/opt/v2board/fault-matrix
GUEST_UNIT='v2board-fault-matrix-guest.service'

apply=0
remove_marker=0
expected_guest_id=

usage() {
  cat <<'EOF'
Usage: cleanup-guest.sh --expected-guest-id ID [--apply] [--remove-marker]

The default is a read-only preview. --apply removes only the matrix-specific
run/state/install roots and unit fixtures. It never removes /var/lib/v2board,
datastore files, native releases, or migration archives. Revert/destroy the
entire disposable guest through the external hypervisor adapter for full
cleanup.
EOF
}

while (($#)); do
  case "$1" in
    --expected-guest-id)
      (($# >= 2)) || { echo 'missing --expected-guest-id value' >&2; exit 64; }
      expected_guest_id=$2
      shift 2
      ;;
    --apply)
      apply=1
      shift
      ;;
    --remove-marker)
      remove_marker=1
      shift
      ;;
    --help|-h)
      usage
      exit 0
      ;;
    *)
      echo "unknown argument: $1" >&2
      usage >&2
      exit 64
      ;;
  esac
done

[[ $expected_guest_id =~ ^[A-Za-z0-9][A-Za-z0-9._-]{0,127}$ ]] || {
  echo 'a safe --expected-guest-id is required' >&2
  exit 64
}
[[ -f $MARKER && ! -L $MARKER ]] || {
  echo "refusing cleanup: disposable marker is missing or not a regular file: $MARKER" >&2
  exit 65
}
[[ $(stat -c '%u:%g:%a:%h' "$MARKER") == '0:0:600:1' ]] || {
  echo 'refusing cleanup: disposable marker must be root:root, mode 0600, nlink 1' >&2
  exit 65
}
command -v jq >/dev/null || { echo 'jq is required' >&2; exit 69; }
marker_protocol=$(jq -er '.protocol' "$MARKER")
marker_guest_id=$(jq -er '.guest_id' "$MARKER")
marker_machine_id_sha256=$(jq -er '.machine_id_sha256 | select(test("^[0-9a-f]{64}$"))' "$MARKER")
machine_id=$(tr -d '\n' </etc/machine-id)
[[ $machine_id =~ ^[0-9a-f]{32}$ ]] || {
  echo 'refusing cleanup: host machine-id is not canonical' >&2
  exit 65
}
current_machine_id_sha256=$(printf '%s' "$machine_id" | sha256sum | awk '{print $1}')
[[ $marker_protocol == v2board-bare-metal-fault-matrix-disposable-v1 ]] || {
  echo 'refusing cleanup: disposable marker protocol mismatch' >&2
  exit 65
}
[[ $marker_guest_id == "$expected_guest_id" ]] || {
  echo 'refusing cleanup: disposable marker guest_id mismatch' >&2
  exit 65
}
[[ $marker_machine_id_sha256 == "$current_machine_id_sha256" ]] || {
  echo 'refusing cleanup: disposable marker belongs to a different machine-id' >&2
  exit 65
}

printf '%s\n' 'Matrix-only cleanup plan:'
printf '  stop/reset: %s\n' "$GUEST_UNIT"
printf '  remove:     %s\n' "$RUN_ROOT"
printf '  remove:     %s\n' "$STATE_ROOT"
printf '  remove:     %s\n' "$INSTALL_ROOT"
printf '  remove:     /etc/systemd/system/%s\n' "$GUEST_UNIT"
if ((remove_marker)); then
  printf '  remove:     %s (last)\n' "$MARKER"
fi

if ((!apply)); then
  echo 'Preview only; pass --apply after verifying this guest is disposable.'
  exit 0
fi
[[ $(id -u) -eq 0 ]] || { echo '--apply must run as root' >&2; exit 77; }

systemctl stop "$GUEST_UNIT" >/dev/null 2>&1 || true
systemctl reset-failed "$GUEST_UNIT" >/dev/null 2>&1 || true

# Every destructive path is a literal constant above. Never accept a cleanup
# root from argv or an environment variable.
rm -rf --one-file-system -- "$RUN_ROOT" "$STATE_ROOT" "$INSTALL_ROOT"
rm -f -- "/etc/systemd/system/$GUEST_UNIT"
systemctl daemon-reload
if ((remove_marker)); then
  rm -f -- "$MARKER"
fi
echo 'Matrix-only guest fixture cleanup completed.'
