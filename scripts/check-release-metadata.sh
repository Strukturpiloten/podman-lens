#!/usr/bin/env bash

set -Eeuo pipefail

if (($# > 1)); then
  printf 'Usage: %s [--development|--release]\n' "$0" >&2
  exit 2
fi
case "${1---release}" in
  --development) validation_context=development ;;
  --release) validation_context=release ;;
  *)
    printf 'Usage: %s [--development|--release]\n' "$0" >&2
    exit 2
    ;;
esac
readonly validation_context

if [[ "${PODMAN_LENS_RELEASE_METADATA_TEST_MODE:-0}" == "1" ]]; then
  version="${PODMAN_LENS_TEST_VERSION:?test version is required}"
  changelog="${PODMAN_LENS_TEST_CHANGELOG:?test changelog is required}"
else
  version="$(cargo metadata --locked --no-deps --format-version 1 | jq -er '.packages[] | select(.name == "podman-lens") | .version')"
  changelog=CHANGELOG.md
fi
readonly version changelog

if [[ ! -f "${changelog}" ]]; then
  printf 'Changelog does not exist: %s\n' "${changelog}" >&2
  exit 1
fi

unreleased_count="$(awk '$0 == "## [Unreleased]" { count += 1 } END { print count + 0 }' "${changelog}")"
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
  ' "${changelog}"
)"
if [[ "${newest_release}" != "${version}" ]]; then
  printf 'Newest CHANGELOG.md release %s must match crate version %s.\n' \
    "${newest_release:-<missing>}" "${version}" >&2
  printf 'Record pending changes under Unreleased; release-plz owns numbered release sections.\n' >&2
  exit 1
fi

release_heading_count="$(
  awk -v prefix="## [${version}]" 'index($0, prefix) == 1 { count += 1 } END { print count + 0 }' \
    "${changelog}"
)"
if [[ "${release_heading_count}" != "1" ]]; then
  printf 'CHANGELOG.md must contain exactly one release section for %s; found %s.\n' \
    "${version}" "${release_heading_count}" >&2
  exit 1
fi

release_notes="$(bash scripts/extract-release-notes.sh "${version}" "${changelog}")"
if ! grep --quiet '[[:alnum:]]' <<< "${release_notes}"; then
  printf 'CHANGELOG.md release section for %s contains no usable notes.\n' "${version}" >&2
  exit 1
fi

if [[ "${validation_context}" == "release" ]]; then
  unreleased_notes="$(
    awk '
      $0 == "## [Unreleased]" {
        reading = 1
        next
      }
      reading && /^## \[/ { exit }
      reading { print }
    ' "${changelog}"
  )"
  if grep --quiet '[[:alnum:]]' <<< "${unreleased_notes}"; then
    printf 'Unreleased must be empty when validating the PodmanLens %s release candidate.\n' \
      "${version}" >&2
    printf 'Move every included change into the numbered release section before merging.\n' >&2
    exit 1
  fi
fi

printf 'Release metadata is valid for PodmanLens %s.\n' "${version}"
