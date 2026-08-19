#!/usr/bin/env bash

set -Eeuo pipefail

version="$(cargo metadata --locked --no-deps --format-version 1 | jq -er '.packages[] | select(.name == "podman-lens") | .version')"
grep --fixed-strings --quiet "## [Unreleased]" CHANGELOG.md || {
  printf 'CHANGELOG.md must contain an Unreleased section.\n' >&2
  exit 1
}
printf 'Release metadata is valid for PodmanLens %s.\n' "${version}"
