#!/usr/bin/env bash

set -Eeuo pipefail

version="$(cargo metadata --locked --no-deps --format-version 1 | jq -er '.packages[] | select(.name == "podman-lens") | .version')"
unreleased_count="$(awk '$0 == "## [Unreleased]" { count += 1 } END { print count + 0 }' CHANGELOG.md)"
if [[ "${unreleased_count}" != "1" ]]; then
  printf 'CHANGELOG.md must contain exactly one Unreleased section; found %s.\n' \
    "${unreleased_count}" >&2
  exit 1
fi

newest_release="$(
  awk '
    /^## \[/ && $0 != "## [Unreleased]" {
      heading = $0
      sub(/^## \[/, "", heading)
      sub(/\].*$/, "", heading)
      print heading
      exit
    }
  ' CHANGELOG.md
)"
if [[ "${newest_release}" != "${version}" ]]; then
  printf 'Newest CHANGELOG.md release %s must match crate version %s.\n' \
    "${newest_release:-<missing>}" "${version}" >&2
  printf 'Record pending changes under Unreleased; release-plz owns numbered release sections.\n' >&2
  exit 1
fi

release_heading_count="$(
  awk -v prefix="## [${version}]" 'index($0, prefix) == 1 { count += 1 } END { print count + 0 }' \
    CHANGELOG.md
)"
if [[ "${release_heading_count}" != "1" ]]; then
  printf 'CHANGELOG.md must contain exactly one release section for %s; found %s.\n' \
    "${version}" "${release_heading_count}" >&2
  exit 1
fi

release_notes="$(bash scripts/extract-release-notes.sh "${version}")"
if ! grep --quiet '[[:alnum:]]' <<< "${release_notes}"; then
  printf 'CHANGELOG.md release section for %s contains no usable notes.\n' "${version}" >&2
  exit 1
fi

printf 'Release metadata is valid for PodmanLens %s.\n' "${version}"
