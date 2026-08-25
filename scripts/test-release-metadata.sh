#!/usr/bin/env bash

set -Eeuo pipefail

temporary_directory="$(mktemp -d)"
readonly temporary_directory
trap 'rm -rf -- "${temporary_directory}"' EXIT
changelog="${temporary_directory}/CHANGELOG.md"
readonly changelog

write_changelog() {
  printf '%s\n' "$@" > "${changelog}"
}

check_metadata() {
  local version=$1
  local latest_tag=$2
  env \
    PODMAN_LENS_RELEASE_METADATA_TEST_MODE=1 \
    PODMAN_LENS_TEST_CHANGELOG="${changelog}" \
    PODMAN_LENS_TEST_LATEST_TAG="${latest_tag}" \
    PODMAN_LENS_TEST_VERSION="${version}" \
    bash scripts/check-release-metadata.sh
}

expect_failure() {
  local label=$1
  local version=$2
  local latest_tag=$3
  if check_metadata "${version}" "${latest_tag}" > /dev/null 2>&1; then
    printf 'Release metadata negative case unexpectedly passed: %s\n' "${label}" >&2
    exit 1
  fi
}

write_changelog \
  '# Changelog' \
  '' \
  '## [Unreleased]' \
  '' \
  '### Fixed' \
  '' \
  '- Pending fix.' \
  '' \
  '## [0.2.1] - 2026-08-25' \
  '' \
  '### Fixed' \
  '' \
  '- Released fix.'
check_metadata 0.2.1 v0.2.1 > /dev/null

write_changelog \
  '# Changelog' \
  '' \
  '## [Unreleased]' \
  '' \
  '## [0.2.2] - 2026-08-26' \
  '' \
  '### Fixed' \
  '' \
  '- Complete release notes.'
check_metadata 0.2.2 v0.2.1 > /dev/null

write_changelog \
  '# Changelog' \
  '' \
  '## [Unreleased]' \
  '' \
  '### Fixed' \
  '' \
  '- Included code omitted from release notes.' \
  '' \
  '## [0.2.2] - 2026-08-26' \
  '' \
  '### Fixed' \
  '' \
  '- A different fix.'
expect_failure 'non-empty Unreleased during release preparation' 0.2.2 v0.2.1

write_changelog \
  '# Changelog' \
  '' \
  '## [Unreleased]' \
  '' \
  '## [0.2.2] - 2026-08-26' \
  '' \
  '### Fixed' \
  '' \
  '- Notes for the wrong package version.'
expect_failure 'newest release differs from package version' 0.2.1 v0.2.1

write_changelog \
  '# Changelog' \
  '' \
  '## [Unreleased]' \
  '' \
  '## [0.2.2] - 2026-08-26' \
  '' \
  '### Fixed' \
  '' \
  '- First section.' \
  '' \
  '## [0.2.2] - 2026-08-26' \
  '' \
  '### Fixed' \
  '' \
  '- Duplicate section.'
expect_failure 'duplicate current-version release section' 0.2.2 v0.2.1

printf 'PodmanLens release metadata policy tests passed.\n'
