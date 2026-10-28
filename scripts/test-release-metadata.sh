#!/usr/bin/env bash

set -Eeuo pipefail

temporary_directory="$(mktemp -d)"
readonly temporary_directory
trap 'rm -rf -- "${temporary_directory}"' EXIT
changelog="${temporary_directory}/CHANGELOG.md"
readonly changelog

# Model all former tag-discovery outcomes without creating tags. No context may
# inspect Git history to decide whether pending notes are allowed.
mkdir "${temporary_directory}/bin"
cat > "${temporary_directory}/bin/git" << 'GIT'
#!/usr/bin/env bash
printf 'called\n' >> "${PODMAN_LENS_TEST_GIT_CALLS}"
printf '%s\n' "${PODMAN_LENS_TEST_TAG}"
GIT
chmod +x "${temporary_directory}/bin/git"
export PATH="${temporary_directory}/bin:${PATH}"
export PODMAN_LENS_TEST_GIT_CALLS="${temporary_directory}/git-calls"
export PODMAN_LENS_TEST_TAG=""

write_changelog() {
  printf '%s\n' "$@" > "${changelog}"
}

check_metadata() {
  local version=$1
  shift
  env \
    PODMAN_LENS_RELEASE_METADATA_TEST_MODE=1 \
    PODMAN_LENS_TEST_CHANGELOG="${changelog}" \
    PODMAN_LENS_TEST_VERSION="${version}" \
    bash scripts/check-release-metadata.sh "$@"
}

expect_failure() {
  local label=$1
  local expected_status=$2
  local expected_message=$3
  shift 3
  local status=0
  check_metadata "$@" > "${temporary_directory}/output" 2>&1 || status=$?
  if [[ "${status}" != "${expected_status}" ]] ||
    ! grep --fixed-strings --quiet -- "${expected_message}" "${temporary_directory}/output"; then
    printf 'Release metadata negative case failed: %s (exit %s, expected %s)\n' \
      "${label}" "${status}" "${expected_status}" >&2
    cat "${temporary_directory}/output" >&2
    exit 1
  fi
}

expect_invalid_metadata() {
  local label=$1
  local expected_message=$2
  local version=${3:-0.10.0}
  local context
  for context in --development --release; do
    expect_failure "${label} (${context})" 1 "${expected_message}" "${version}" "${context}"
  done
}

write_pending_changelog() {
  write_changelog \
    '# Changelog' \
    '' \
    '## [Unreleased]' \
    '' \
    '### Fixed' \
    '' \
    '- Pending fix after release preparation.' \
    '' \
    '## [0.10.0] - 2026-10-27' \
    '' \
    '### Fixed' \
    '' \
    '- Prepared release notes.'
}

write_pending_changelog
for PODMAN_LENS_TEST_TAG in '' v0.10.0 v0.9.0; do
  check_metadata 0.10.0 --development > /dev/null
  expect_failure 'pending notes rejected for every tag state' 1 \
    'Unreleased must be empty' 0.10.0 --release
  expect_failure 'default remains strict for every tag state' 1 \
    'Unreleased must be empty' 0.10.0
done

write_changelog \
  '# Changelog' \
  '' \
  '## [Unreleased]' \
  '' \
  '## [0.10.0] - 2026-10-27' \
  '' \
  '### Fixed' \
  '' \
  '- Complete release notes.'
for PODMAN_LENS_TEST_TAG in '' v0.10.0 v0.9.0; do
  check_metadata 0.10.0 --development > /dev/null
  check_metadata 0.10.0 --release > /dev/null
  check_metadata 0.10.0 > /dev/null
done

expect_failure 'unknown context' 2 'Usage:' 0.10.0 --unknown
expect_failure 'empty context' 2 'Usage:' 0.10.0 ''
expect_failure 'positional context' 2 'Usage:' 0.10.0 development
expect_failure 'conflicting contexts' 2 'Usage:' 0.10.0 --development --release
expect_failure 'repeated context' 2 'Usage:' 0.10.0 --release --release
expect_failure 'extra argument' 2 'Usage:' 0.10.0 --development extra
expect_invalid_metadata 'newest release differs from package version' \
  'must match crate version' 0.9.0

write_changelog \
  '# Changelog' \
  '' \
  '## [Unreleased]' \
  '' \
  '## [0.10.0] - 2026-10-27' \
  '' \
  '- First section.' \
  '' \
  '## [0.10.0] - 2026-10-27' \
  '' \
  '- Duplicate section.'
expect_invalid_metadata 'duplicate current-version release section' \
  'exactly one release section'

write_pending_changelog
printf '\n## [Unreleased]\n' >> "${changelog}"
expect_invalid_metadata 'duplicate Unreleased section' 'exactly one Unreleased section'

write_changelog '# Changelog' '' '## [0.10.0] - 2026-10-27' '' '- Notes.'
expect_invalid_metadata 'missing Unreleased section' 'exactly one Unreleased section'

write_changelog '# Changelog' '' '## [Unreleased]'
expect_invalid_metadata 'missing release section' 'must match crate version'

write_changelog '# Changelog' '' '## [Unreleased]' '' '## [0.10.0] - 2026-10-27' '' '---'
expect_invalid_metadata 'release section without usable notes' 'contains no usable notes'

rm "${changelog}"
expect_invalid_metadata 'missing changelog file' 'Changelog does not exist'

if [[ -e "${PODMAN_LENS_TEST_GIT_CALLS}" ]]; then
  printf 'Release metadata validation must not infer context from Git tags.\n' >&2
  exit 1
fi
printf 'PodmanLens release metadata policy tests passed.\n'
