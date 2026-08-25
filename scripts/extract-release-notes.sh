#!/usr/bin/env bash

set -Eeuo pipefail

if (($# < 1 || $# > 2)); then
  printf 'Usage: %s VERSION [CHANGELOG]\n' "$0" >&2
  exit 2
fi
version=$1
changelog=${2:-CHANGELOG.md}
if [[ ! -f "${changelog}" ]]; then
  printf 'Changelog does not exist: %s\n' "${changelog}" >&2
  exit 1
fi
awk -v heading="## [${version}]" '
  $0 ~ /^## \[/ {
    if (printing) exit
    if (index($0, heading) == 1) {
      printing = 1
      next
    }
  }
  printing { print }
  END { if (!printing) exit 1 }
' "${changelog}"
