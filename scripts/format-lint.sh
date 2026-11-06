#!/usr/bin/env bash
set -Eeuo pipefail
script_directory="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd -P)"
cd -- "${script_directory}/.."
(($# <= 1)) || {
  printf 'Usage: %s [--fix|--check]\n' "$0" >&2
  exit 2
}
case "${1:---fix}" in
  --fix)
    cargo fmt --all
    bash scripts/check-files.sh --fix
    ;;
  --check)
    cargo fmt --all -- --check
    bash scripts/check-files.sh --check
    ;;
  *)
    printf 'Usage: %s [--fix|--check]\n' "$0" >&2
    exit 2
    ;;
esac
python3 scripts/shared-ci.py run --task python-lint
cargo ci-clippy
