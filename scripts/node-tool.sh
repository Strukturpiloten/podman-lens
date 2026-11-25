#!/usr/bin/env bash
# Acquisition and installation verification belong to the selected shared source.
set -Eeuo pipefail
script_directory="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd -P)"
repository_root="$(cd -- "${script_directory}/.." && pwd -P)"
if [[ -n "${PODMAN_LENS_NODE_TOOLS_ROOT:-}" ]]; then
  if [[ "${PODMAN_LENS_NODE_TOOLS_ROOT}" != /opt/podman-lens-file-tools ]]; then
    printf 'Unexpected PodmanLens image Node installation; rebuild the Dev Container.\n' >&2
    exit 2
  fi
  exec python3 /opt/podman-lens-file-tools/boxferry-node-tools.py image-node-tool \
    --repository-root "${repository_root}" --cwd "${repository_root}" "$@"
fi
exec python3 "${script_directory}/shared-ci.py" node-tool --cwd "${repository_root}" "$@"
