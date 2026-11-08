#!/usr/bin/env bash
# Canonical shared file tools for local, CI and Dev Container callers.
set -Eeuo pipefail

if (($# > 1)); then
  printf 'Usage: %s [install-directory]\n' "$0" >&2
  exit 2
fi

script_directory="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd -P)"
exec python3 "${script_directory}/shared-ci.py" tools \
  --destination "${1:-/usr/local/bin}" --tools tombi shfmt shellcheck hadolint actionlint lychee
