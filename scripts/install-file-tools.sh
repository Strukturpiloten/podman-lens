#!/usr/bin/env bash
# Compatibility entry point; the selected shared inventory owns versions and integrity.
set -Eeuo pipefail
script_directory="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd -P)"
exec bash "${script_directory}/install-host-tools.sh" "$@"
