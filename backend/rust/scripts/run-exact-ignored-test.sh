#!/usr/bin/env bash
set -euo pipefail

if [[ "$#" -ne 2 ]]; then
  echo "usage: run-exact-ignored-test.sh <package> <fully-qualified-test-name>" >&2
  exit 64
fi

package="$1"
test_name="$2"
listed="$(cargo test --locked -p "$package" "$test_name" -- --ignored --exact --list)"
expected="${test_name}: test"
matches="$(awk -v expected="$expected" '$0 == expected { count += 1 } END { print count + 0 }' <<<"$listed")"
if [[ "$matches" -ne 1 ]]; then
  echo "expected exactly one ignored test named ${test_name}, found ${matches}" >&2
  exit 65
fi

cargo test --locked -p "$package" "$test_name" -- --ignored --exact
