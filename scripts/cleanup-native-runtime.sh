#!/usr/bin/env bash
# Remove only one run/attempt/cell's dedicated host-Podman store.
set -Eeuo pipefail
[[ $# == 1 && "$1" =~ ^(rf|rl)$ ]] || {
  echo 'usage: cleanup-native-runtime.sh rf|rl' >&2
  exit 64
}
state_key=$1
[[ "${GITHUB_RUN_ID}" =~ ^[0-9]+$ && "${GITHUB_RUN_ATTEMPT}" =~ ^[0-9]+$ ]]
state_directory="/tmp/pl-${GITHUB_RUN_ID}-${GITHUB_RUN_ATTEMPT}-${state_key}"
[[ "${#state_directory}" -le 42 ]]
[[ -e "${state_directory}" ]] || exit 0
[[ -d "${state_directory}" && ! -L "${state_directory}" ]] || {
  echo 'Native state is not a regular directory.' >&2
  exit 1
}
config="${state_directory}/containers.conf"
if [[ ! -e "${config}" ]]; then
  # An interrupted builder may have created the directory before its store configuration.
  sudo rm --recursive --force -- "${state_directory}"
  exit 0
fi
[[ -f "${config}" && ! -L "${config}" ]] || {
  echo 'Native store configuration is not a regular file.' >&2
  exit 1
}
host_podman=(sudo env "CONTAINERS_CONF_OVERRIDE=${config}" podman --root "${state_directory}/root" --runroot "${state_directory}/runroot" --tmpdir "${state_directory}/tmp")
failed=0
containers="$("${host_podman[@]}" ps --all --external --format '{{.ID}}')" || failed=1
if ((failed == 0)); then
  while IFS= read -r container; do
    [[ -n "${container}" ]] || continue
    [[ "${container}" =~ ^[a-f0-9]{12,64}$ ]] || {
      failed=1
      continue
    }
    "${host_podman[@]}" rm --force --volumes "${container}" ||
      "${host_podman[@]}" rm --force "${container}" || failed=1
  done <<< "${containers}"
fi
volumes="$("${host_podman[@]}" volume ls --format '{{.Name}}')" || failed=1
if ((failed == 0)); then
  while IFS= read -r volume; do
    [[ -n "${volume}" ]] || continue
    [[ "${volume}" =~ ^[a-zA-Z0-9][a-zA-Z0-9_.-]{0,127}$ ]] || {
      failed=1
      continue
    }
    "${host_podman[@]}" volume rm --force "${volume}" || failed=1
  done <<< "${volumes}"
fi
"${host_podman[@]}" unmount --all || failed=1
images="$("${host_podman[@]}" images --all --format '{{.ID}}' | LC_ALL=C sort -u)" || failed=1
if ((failed == 0)); then
  while IFS= read -r image; do
    [[ -n "${image}" ]] || continue
    [[ "${image}" =~ ^[a-f0-9]{12,64}$ ]] || {
      failed=1
      continue
    }
    "${host_podman[@]}" image rm --force "${image}" || failed=1
  done <<< "${images}"
fi
remaining_containers="$("${host_podman[@]}" ps --all --external --format '{{.ID}}')" || failed=1
remaining_volumes="$("${host_podman[@]}" volume ls --format '{{.Name}}')" || failed=1
remaining_mounts="$("${host_podman[@]}" mount)" || failed=1
remaining_images="$("${host_podman[@]}" images --all --format '{{.ID}}')" || failed=1
if ((failed != 0)) || [[ -n "${remaining_containers}" || -n "${remaining_volumes}" || -n "${remaining_mounts}" || -n "${remaining_images}" ]]; then
  printf 'Native cleanup incomplete; preserving task-owned state %s for diagnosis.\n' "${state_directory}" >&2
  printf 'Remaining containers: %s; volumes: %s; images: %s; mounts: %s\n' \
    "${remaining_containers:-none}" "${remaining_volumes:-none}" "${remaining_images:-none}" "${remaining_mounts:-none}" >&2
  exit 1
fi
sudo du --summarize --human-readable -- "${state_directory}" >&2
sudo rm --recursive --force -- "${state_directory}"
