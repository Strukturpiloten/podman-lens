#!/usr/bin/env bash
# Build one run-scoped native service image from a fixed Fedora compose and checked RPM.
set -Eeuo pipefail
[[ $# == 2 ]] || {
  echo 'usage: build-native-runtime.sh STATE_KEY ROOT_MODE' >&2
  exit 64
}
state_key=$1
root_mode=$2
[[ "${state_key}" =~ ^(rf|rl)$ && "${root_mode}" =~ ^(rootful|rootless)$ ]]
[[ ("${state_key}" == rf && "${root_mode}" == rootful) || ("${state_key}" == rl && "${root_mode}" == rootless) ]]
[[ "${GITHUB_RUN_ID}" =~ ^[0-9]+$ && "${GITHUB_RUN_ATTEMPT}" =~ ^[0-9]+$ ]]
[[ "$(uname -m)" == x86_64 ]]
[[ -n "${GITHUB_OUTPUT:-}" ]]
# shellcheck source=scripts/native-runtime-pins.sh
source "$(dirname "${BASH_SOURCE[0]}")/native-runtime-pins.sh"
# shellcheck source=scripts/native-runtime-helpers.sh
source "$(dirname "${BASH_SOURCE[0]}")/native-runtime-helpers.sh"
state_directory="/tmp/pl-${GITHUB_RUN_ID}-${GITHUB_RUN_ATTEMPT}-${state_key}"
runroot_directory="${state_directory}/runroot"
[[ "${#runroot_directory}" -le 50 ]]
context="${state_directory}/build"
mkdir -m 0700 -- "${state_directory}"
cleanup_failed_build() {
  status=$?
  if ((status != 0)); then
    bash "$(dirname "${BASH_SOURCE[0]}")/cleanup-native-runtime.sh" "${state_key}" || true
  fi
}
trap cleanup_failed_build EXIT
trap 'exit 143' TERM INT
install -d -m 0700 "${state_directory}/root" "${runroot_directory}" "${state_directory}/tmp" "${context}"
printf '[engine]\nlock_type = "file"\n' > "${state_directory}/containers.conf"
host_podman=(sudo env "CONTAINERS_CONF_OVERRIDE=${state_directory}/containers.conf" podman --root "${state_directory}/root" --runroot "${runroot_directory}" --tmpdir "${state_directory}/tmp")
readonly rpm_name="podman-${PODMAN_NATIVE_VERSION#v}-${PODMAN_NATIVE_RPM_RELEASE}.x86_64.rpm"
readonly rpm_url="https://kojipkgs.fedoraproject.org/packages/podman/${PODMAN_NATIVE_VERSION#v}/${PODMAN_NATIVE_RPM_RELEASE}/x86_64/${rpm_name}"
readonly source_rpm_url="https://kojipkgs.fedoraproject.org/packages/podman/${PODMAN_NATIVE_VERSION#v}/${PODMAN_NATIVE_RPM_RELEASE}/src/podman-${PODMAN_NATIVE_VERSION#v}-${PODMAN_NATIVE_RPM_RELEASE}.src.rpm"
curl --fail --location --silent --show-error --max-time 120 --output "${context}/podman.rpm" "${rpm_url}"
printf '%s  %s\n' "${PODMAN_NATIVE_RPM_SHA256}" "${context}/podman.rpm" | sha256sum --check --status
curl --fail --location --silent --show-error --max-time 120 --output "${context}/podman.src.rpm" "${source_rpm_url}"
printf '%s  %s\n' "${PODMAN_NATIVE_SOURCE_RPM_SHA256}" "${context}/podman.src.rpm" | sha256sum --check --status
tag_revision="$(git ls-remote https://github.com/containers/podman.git "refs/tags/${PODMAN_NATIVE_VERSION}^{}")"
[[ "${tag_revision}" == "${PODMAN_NATIVE_SOURCE_REVISION}"$'\t'"refs/tags/${PODMAN_NATIVE_VERSION}^{}" ]]
curl --fail --location --silent --show-error --max-time 30 --output "${context}/repomd.xml" "${PODMAN_NATIVE_COMPOSE_URL}/repodata/repomd.xml"
printf '%s  %s\n' "${PODMAN_NATIVE_REPOMD_SHA256}" "${context}/repomd.xml" | sha256sum --check --status
primary_href="$(sed -n '/<data type="primary">/,/<\/data>/s/.*<location href="\([^"]*\)".*/\1/p' "${context}/repomd.xml")"
[[ "${primary_href}" == "repodata/${PODMAN_NATIVE_PRIMARY_SHA256}-primary.xml.zst" ]]
curl --fail --location --silent --show-error --max-time 120 --output "${context}/primary.xml.zst" "${PODMAN_NATIVE_COMPOSE_URL}/${primary_href}"
printf '%s  %s\n' "${PODMAN_NATIVE_PRIMARY_SHA256}" "${context}/primary.xml.zst" | sha256sum --check --status
cp containers/native-podman/Containerfile "${context}/Containerfile"
printf '[podman-lens-fixed]\nname=PodmanLens fixed Fedora 45 Beta compose\nbaseurl=%s\nenabled=0\ngpgcheck=0\nmetadata_expire=-1\n' "${PODMAN_NATIVE_COMPOSE_URL}" > "${context}/fedora-beta.repo"
image="localhost/podman-lens-native:${PODMAN_NATIVE_VERSION}-${state_key}"
"${host_podman[@]}" pull --quiet "${PODMAN_NATIVE_BASE_IMAGE}"
"${host_podman[@]}" build --pull=never --layers=false --target "${root_mode}" \
  --build-arg "BASE_IMAGE=${PODMAN_NATIVE_BASE_IMAGE}" \
  --build-arg "PODMAN_VERSION=${PODMAN_NATIVE_VERSION#v}" \
  --build-arg "PODMAN_RPM_RELEASE=${PODMAN_NATIVE_RPM_RELEASE}" \
  --build-arg "SOURCE_ARCHIVE_SHA256=${PODMAN_NATIVE_SOURCE_ARCHIVE_SHA256}" \
  --tag "${image}" "${context}"
if [[ "${root_mode}" == rootless ]]; then
  # Verify that file capabilities survive the final image layer and runtime mount.
  # The single-quoted script runs inside the image.
  # shellcheck disable=SC2016
  "${host_podman[@]}" run --rm --entrypoint /bin/bash "${image}" -o pipefail -c '
    [[ "$(id -u)" == 1000 ]] \
      && getcap -n -- /usr/bin/newuidmap | grep -Fx "/usr/bin/newuidmap cap_setuid=ep" \
      && getcap -n -- /usr/bin/newgidmap | grep -Fx "/usr/bin/newgidmap cap_setgid=ep"
  '
fi
# The fixed compose must still serve the reviewed index after dependency resolution.
curl --fail --location --silent --show-error --max-time 30 --output "${context}/repomd-after.xml" "${PODMAN_NATIVE_COMPOSE_URL}/repodata/repomd.xml"
printf '%s  %s\n' "${PODMAN_NATIVE_REPOMD_SHA256}" "${context}/repomd-after.xml" | sha256sum --check --status
raw_image_id="$("${host_podman[@]}" image inspect --format '{{.Id}}' "${image}")"
image_id="$(normalize_native_image_id "${raw_image_id}")"
"${host_podman[@]}" run --rm --entrypoint cat "${image}" /usr/share/podman-lens/native-package-closure.txt > "${state_directory}/package-closure.txt"
grep -Fx "podman 5:${PODMAN_NATIVE_VERSION#v}-${PODMAN_NATIVE_RPM_RELEASE}.x86_64" "${state_directory}/package-closure.txt" > /dev/null
[[ "$(stat --format=%s -- "${state_directory}/package-closure.txt")" -le 65536 ]]
[[ "$(wc -l < "${state_directory}/package-closure.txt")" -le 1000 ]]
closure_sha="$(sha256sum "${state_directory}/package-closure.txt")"
closure_sha="${closure_sha%% *}"
[[ "${closure_sha}" =~ ^[a-f0-9]{64}$ ]]
printf 'image=%s\nimage_id=%s\nexpected_version=%s\nbase_image=%s\nrepomd_sha256=%s\nprimary_sha256=%s\nrpm_sha256=%s\nclosure_sha256=%s\n' \
  "${image}" "${image_id}" "${PODMAN_NATIVE_VERSION#v}" "${PODMAN_NATIVE_BASE_IMAGE}" \
  "${PODMAN_NATIVE_REPOMD_SHA256}" "${PODMAN_NATIVE_PRIMARY_SHA256}" "${PODMAN_NATIVE_RPM_SHA256}" "${closure_sha}" >> "${GITHUB_OUTPUT}"
