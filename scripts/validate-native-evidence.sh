#!/usr/bin/env bash
# Validate compact Release evidence without accepting an earlier workflow run.
set -Eeuo pipefail

if [[ $# -ne 4 ]]; then
  echo "usage: $0 EVIDENCE_ROOT CANDIDATE_SHA RUN_ID RUN_ATTEMPT" >&2
  exit 64
fi

root=$1
candidate=$2
run_id=$3
run_attempt=$4
# shellcheck source=scripts/native-runtime-pins.sh
source "$(dirname "${BASH_SOURCE[0]}")/native-runtime-pins.sh"
for cell in podman-6.1-rootful podman-6.1-rootless; do
  evidence="${root}/podman-lens-native-api-${candidate}-${run_id}-${run_attempt}-${cell}/native-podman-evidence.json"
  [[ -f "${evidence}" ]] || {
    echo "Missing current-run evidence for ${cell}." >&2
    exit 1
  }
  root_mode=${cell##*-}
  case "${root_mode}" in rootful) state_key=rf ;; rootless) state_key=rl ;; esac
  image="localhost/podman-lens-native:${PODMAN_NATIVE_VERSION}-${state_key}"
  podman_nevra="podman 5:${PODMAN_NATIVE_VERSION#v}-${PODMAN_NATIVE_RPM_RELEASE}.x86_64"
  jq -e --arg sha "${candidate}" --arg run "${run_id}" --arg attempt "${run_attempt}" --arg cell "${cell}" \
    --arg root_mode "${root_mode}" --arg image "${image}" --arg base_image "${PODMAN_NATIVE_BASE_IMAGE}" \
    --arg expected_version "${PODMAN_NATIVE_VERSION#v}" --arg rpm_sha256 "${PODMAN_NATIVE_RPM_SHA256}" \
    --arg source_revision "${PODMAN_NATIVE_SOURCE_REVISION}" \
    --arg source_rpm_sha256 "${PODMAN_NATIVE_SOURCE_RPM_SHA256}" \
    --arg source_archive_sha256 "${PODMAN_NATIVE_SOURCE_ARCHIVE_SHA256}" \
    --arg repomd_sha256 "${PODMAN_NATIVE_REPOMD_SHA256}" --arg primary_sha256 "${PODMAN_NATIVE_PRIMARY_SHA256}" --arg podman_nevra "${podman_nevra}" \
    '.candidate == $sha and .run_id == $run and .run_attempt == $attempt and .task == "native-api" and
     .cell == $cell and .root_mode == $root_mode and .build_outcome == "success" and .outcome == "success" and
     .socket_scope == "isolated-disposable-service" and .image == $image and
     (.api_version | test("^[0-9]+\\.[0-9]+\\.[0-9]+$")) and
     (.image_id | test("^sha256:[0-9a-f]{64}$")) and
     .base_image == $base_image and .expected_version == $expected_version and
     .rpm_sha256 == $rpm_sha256 and .source_revision == $source_revision and
     .source_rpm_sha256 == $source_rpm_sha256 and
     .source_archive_sha256 == $source_archive_sha256 and .repomd_sha256 == $repomd_sha256 and
     .primary_sha256 == $primary_sha256 and (.closure_sha256 | test("^[0-9a-f]{64}$")) and
     (.build_peak_state_kib | test("^[0-9]+$")) and (.runtime_peak_state_kib | test("^[0-9]+$")) and
     (.peak_state_kib | test("^[0-9]+$")) and
     (.build_peak_state_kib | tonumber) > 0 and (.runtime_peak_state_kib | tonumber) > 0 and
     (.peak_state_kib | tonumber) <= 8388608 and
     (.peak_state_kib | tonumber) == ([.build_peak_state_kib, .runtime_peak_state_kib] | map(tonumber) | max) and
     (.packages | type == "array" and length > 0 and length <= 1000 and index($podman_nevra) != null and
       all(.[]; type == "string" and test("^[A-Za-z0-9_+.-]+ [0-9]+:[A-Za-z0-9_+.:~-]+$")))' \
    "${evidence}" > /dev/null
  recorded_closure_sha="$(jq -r '.closure_sha256' "${evidence}")"
  actual_closure_sha="$(jq -r '.packages[]' "${evidence}" | sha256sum)"
  actual_closure_sha="${actual_closure_sha%% *}"
  [[ "${actual_closure_sha}" == "${recorded_closure_sha}" ]] || {
    echo "Package closure hash mismatch for ${cell}." >&2
    exit 1
  }
done
