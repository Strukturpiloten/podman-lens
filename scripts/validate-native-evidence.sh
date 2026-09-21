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
for cell in podman-6.1-rootful podman-6.1-rootless; do
  evidence="${root}/podman-lens-native-api-${candidate}-${run_id}-${run_attempt}-${cell}/native-podman-evidence.json"
  [[ -f "${evidence}" ]] || {
    echo "Missing current-run evidence for ${cell}." >&2
    exit 1
  }
  root_mode=${cell##*-}
  image_prefix="ghcr.io/strukturpiloten/${cell}:v"
  jq -e --arg sha "${candidate}" --arg run "${run_id}" --arg attempt "${run_attempt}" --arg cell "${cell}" \
    --arg root_mode "${root_mode}" --arg image_prefix "${image_prefix}" \
    '.candidate == $sha and .run_id == $run and .run_attempt == $attempt and .task == "native-api" and
     .cell == $cell and .root_mode == $root_mode and .outcome == "success" and
     .socket_scope == "isolated-disposable-service" and (.image | startswith($image_prefix)) and
     (.image | test("@sha256:[0-9a-f]{64}$")) and
     ((.image | capture(":v(?<version>[^@]+)@sha256:").version) == .expected_version)' \
    "${evidence}" > /dev/null
done
