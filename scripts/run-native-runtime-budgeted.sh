#!/usr/bin/env bash
# Watch one exact isolated Podman store throughout a native service phase.
set -Eeuo pipefail
[[ $# -ge 4 && "$3" == -- ]] || {
  echo 'usage: run-native-runtime-budgeted.sh STATE_KEY PHASE -- COMMAND...' >&2
  exit 64
}
state_key=$1
phase=$2
shift 3
[[ "${state_key}" =~ ^(rf|rl)$ && "${phase}" =~ ^(build|provision|api|conformance)$ ]]
[[ "${GITHUB_RUN_ID}" =~ ^[0-9]+$ && "${GITHUB_RUN_ATTEMPT}" =~ ^[0-9]+$ ]]
[[ -f "${GITHUB_OUTPUT}" && ! -L "${GITHUB_OUTPUT}" ]]
# shellcheck source=scripts/native-runtime-helpers.sh
source "$(dirname "${BASH_SOURCE[0]}")/native-runtime-helpers.sh"
state_directory="/tmp/pl-${GITHUB_RUN_ID}-${GITHUB_RUN_ATTEMPT}-${state_key}"
[[ "${#state_directory}" -le 42 ]]
if [[ "${phase}" == build ]]; then
  [[ ! -e "${state_directory}" && ! -L "${state_directory}" ]]
else
  [[ -d "${state_directory}" && ! -L "${state_directory}" ]]
fi
readonly state_max_kib=$((8 * 1024 * 1024))
readonly available_floor_kib=$((2 * 1024 * 1024))
readonly preflight_available_kib=$((10 * 1024 * 1024))
peak_state_kib=0
command_pid=''
available_kib() {
  local value
  value="$(df -Pk /tmp | awk 'NR == 2 {print $4}')"
  [[ "${value}" =~ ^[0-9]+$ ]] || return 1
  printf '%s\n' "${value}"
}
measure_state_kib() {
  local report line value status attempt transient summaries
  local overlay_error="^du: fts_read failed: ${state_directory}/root/overlay/[a-f0-9]{64}/merged: No such file or directory$"
  for attempt in 1 2 3; do
    if report="$(timeout --signal=TERM --kill-after=5s 20s sudo env LC_ALL=C du -skx -- "${state_directory}" 2>&1)"; then
      [[ "${report}" == *$'\t'"${state_directory}" ]] || return 1
      value=${report%%$'\t'*}
      [[ "${value}" =~ ^[0-9]+$ && "${#value}" -le 15 ]] || return 1
      printf '%s\n' "${value}"
      return 0
    else
      status=$?
    fi
    # Only a disappearing Podman overlay mount is retryable. Do not reuse du's
    # partial total or forgive unreadable storage, sudo failures, or timeouts.
    [[ "${status}" == 1 ]] || return 1
    transient=0
    summaries=0
    while IFS= read -r line; do
      if [[ "${line}" =~ ${overlay_error} ]]; then
        transient=1
      elif [[ "${line}" == *$'\t'"${state_directory}" ]]; then
        value=${line%%$'\t'*}
        [[ "${value}" =~ ^[0-9]+$ && "${#value}" -le 15 && "${summaries}" == 0 ]] || return 1
        summaries=1
      else
        return 1
      fi
    done <<< "${report}"
    ((transient == 1)) || return 1
    if ((attempt < 3)); then
      printf 'Transient native overlay mount disappeared; retrying exact-store measurement.\n' >&2
      sleep 0.2
    fi
  done
  return 1
}
sample_native_storage() {
  local used=0 available
  if [[ -e "${state_directory}" || -L "${state_directory}" ]]; then
    [[ -d "${state_directory}" && ! -L "${state_directory}" ]] || return 1
    used="$(measure_state_kib)" || return 1
  elif [[ "${phase}" != build ]]; then
    return 1
  fi
  available="$(available_kib)" || return 1
  if ((used > peak_state_kib)); then peak_state_kib=${used}; fi
  native_storage_budget_allows "${available}" "${used}" "${available_floor_kib}" "${state_max_kib}" || {
    printf 'Native %s storage budget exceeded: state=%s KiB (max %s), available=%s KiB (floor %s).\n' \
      "${phase}" "${used}" "${state_max_kib}" "${available}" "${available_floor_kib}" >&2
    return 1
  }
}
command_running() {
  [[ -n "${command_pid}" ]] && jobs -pr | grep -Fx -- "${command_pid}" > /dev/null
}
command_group_is_owned() {
  local group
  group="$(ps -o pgid= -p "${command_pid}" | tr -d '[:space:]')"
  [[ "${group}" == "${command_pid}" ]]
}
stop_command() {
  command_running || return 0
  if command_group_is_owned; then
    kill -TERM -- "-${command_pid}" 2> /dev/null || true
  else
    kill -TERM "${command_pid}" 2> /dev/null || true
  fi
  for _ in {1..40}; do
    command_running || break
    sleep 1
  done
  if command_running; then
    if command_group_is_owned; then
      kill -KILL -- "-${command_pid}" 2> /dev/null || true
    else
      kill -KILL "${command_pid}" 2> /dev/null || true
    fi
  fi
  wait "${command_pid}" 2> /dev/null || true
}
finish() {
  local status=$?
  trap - EXIT
  stop_command
  printf 'peak_state_kib=%s\n' "${peak_state_kib}" >> "${GITHUB_OUTPUT}"
  printf 'Native %s task-owned state peak: %s KiB (limit %s KiB).\n' "${phase}" "${peak_state_kib}" "${state_max_kib}" >&2
  if ((status != 0)); then
    bash "$(dirname "${BASH_SOURCE[0]}")/cleanup-native-runtime.sh" "${state_key}" || status=1
  fi
  exit "${status}"
}
trap finish EXIT
trap 'exit 143' INT TERM
if [[ "${phase}" == build ]]; then
  preflight_free="$(available_kib)"
  native_storage_budget_allows "${preflight_free}" 0 "${preflight_available_kib}" "${state_max_kib}" || {
    printf 'Native preflight requires at least %s KiB free; found %s KiB.\n' "${preflight_available_kib}" "${preflight_free}" >&2
    exit 1
  }
else
  sample_native_storage
fi
setsid "$@" &
command_pid=$!
for _ in {1..10}; do
  command_running || break
  command_group_is_owned && break
  sleep 0.1
done
if command_running && ! command_group_is_owned; then
  echo 'Native phase did not establish an isolated process group.' >&2
  exit 1
fi
while command_running; do
  sample_native_storage || {
    echo 'Native state sampling failed or exceeded its budget.' >&2
    exit 1
  }
  sleep 5
done
wait "${command_pid}"
command_pid=''
sample_native_storage
