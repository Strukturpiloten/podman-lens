#!/usr/bin/env bash
# Ordinary host/CI tooling; the separate legacy installer is image-only until #563.
set -Eeuo pipefail

if (($# > 1)); then
  printf 'Usage: %s [install-directory]\n' "$0" >&2
  exit 2
fi

script_directory="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd -P)"
exec python3 "${script_directory}/shared-ci.py" tools \
  --destination "${1:-/usr/local/bin}" --profile core
