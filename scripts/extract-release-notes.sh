#!/usr/bin/env bash

set -Eeuo pipefail

if (($# != 1)); then
  printf 'Usage: %s VERSION\n' "$0" >&2
  exit 2
fi
version=$1
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
' CHANGELOG.md
