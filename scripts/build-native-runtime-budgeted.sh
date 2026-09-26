#!/usr/bin/env bash
# Keep the fixed native builder under the shared exact-store lifecycle watchdog.
set -Eeuo pipefail
[[ $# == 2 ]] || {
  echo 'usage: build-native-runtime-budgeted.sh STATE_KEY ROOT_MODE' >&2
  exit 64
}
state_key=$1
root_mode=$2
[[ ("${state_key}" == rf && "${root_mode}" == rootful) || ("${state_key}" == rl && "${root_mode}" == rootless) ]]
exec bash "$(dirname "${BASH_SOURCE[0]}")/run-native-runtime-budgeted.sh" "${state_key}" build -- \
  timeout --signal=TERM --kill-after=30s 12m bash "$(dirname "${BASH_SOURCE[0]}")/build-native-runtime.sh" "${state_key}" "${root_mode}"
