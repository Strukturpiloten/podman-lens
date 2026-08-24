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

manifest_version() {
  awk '
    /^\[workspace\.package\]$/ { in_workspace_package = 1; next }
    /^\[/ { in_workspace_package = 0 }
    in_workspace_package && /^[[:space:]]*version[[:space:]]*=/ {
      value = $0
      sub(/^[^=]*=[[:space:]]*"/, "", value)
      sub(/"[[:space:]]*$/, "", value)
      print value
      exit
    }
  '
}

release_tag="$(git tag --merged HEAD --list 'v[0-9]*' --sort=-version:refname | sed -n '1p')"
if [[ -z "${release_tag}" ]]; then
  fail 'no reachable release tag exists; use the initial public API contract check instead'
fi

current_version="$(manifest_version < Cargo.toml)"
baseline_version="$(manifest_version < <(git show "${release_tag}:Cargo.toml"))"
[[ -n "${current_version}" && -n "${baseline_version}" ]] || fail 'workspace package version is unavailable'

breaking_change=0
if git log --format=%s "${release_tag}..HEAD" | grep -Eq '^[[:alpha:]][[:alnum:]_-]*(\([^)]*\))?!:' ||
  git log --format=%B "${release_tag}..HEAD" | grep -Eq '^BREAKING CHANGE:'; then
  breaking_change=1
fi

arguments=(semver-checks check-release --package podman-lens)
if [[ "${current_version%%.*}" == '0' && ${breaking_change} -eq 1 ]]; then
  # cargo-semver-checks uses the stable-SemVer category name for an API break. PodmanLens ships
  # that category in the next pre-1.0 minor release, but the tool must still receive `major`.
  arguments+=(--release-type major)
fi

"${CARGO:-cargo}" "${arguments[@]}"
