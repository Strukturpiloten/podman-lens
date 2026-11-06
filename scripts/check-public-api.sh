#!/usr/bin/env bash

set -Eeuo pipefail

script_directory="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd -P)"
repository_root="$(cd -- "${script_directory}/.." && pwd -P)"
readonly repository_root
cd -- "${repository_root}"

fail() {
  printf 'PodmanLens public API check failed: %s\n' "$1" >&2
  exit 2
}

(($# <= 1)) || fail "Usage: $0 [--development|--published]"
case "${1:---development}" in
  --development) exec python3 scripts/shared-ci.py run --task api-development ;;
  --published) exec python3 scripts/shared-ci.py run --task api-published ;;
  *) fail "Usage: $0 [--development|--published]" ;;
esac
