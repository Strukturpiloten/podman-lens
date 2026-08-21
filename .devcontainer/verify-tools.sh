#!/usr/bin/env bash

set -Eeuo pipefail

for cargo_directory_name in CARGO_HOME CARGO_TARGET_DIR GH_CONFIG_DIR; do
  cargo_directory="${!cargo_directory_name:-}"
  [[ -n "${cargo_directory}" ]] || {
    printf 'PodmanLens Dev Container is missing %s.\n' "${cargo_directory_name}" >&2
    exit 1
  }
  if [[ ! -w "${cargo_directory}" ]]; then
    sudo chown -R "$(id -u):$(id -g)" "${cargo_directory}"
  fi
  [[ -w "${cargo_directory}" ]] || {
    printf 'PodmanLens Dev Container cannot make %s writable: %s\n' "${cargo_directory_name}" "${cargo_directory}" >&2
    exit 1
  }
done
chmod 0700 "${GH_CONFIG_DIR}"

tools=(actionlint cargo cargo-clippy cargo-deny cargo-llvm-cov cargo-semver-checks curl gh git hadolint jq lychee markdownlint-cli2 node npm prettier rustc rustfmt rustup shellcheck shfmt tombi zizmor)
for tool in "${tools[@]}"; do
  command -v "${tool}" > /dev/null 2>&1 || {
    printf 'PodmanLens Dev Container is missing required tool: %s\n' "${tool}" >&2
    exit 1
  }
done
rustup component list --installed | grep -q '^llvm-tools-' || {
  printf 'PodmanLens Dev Container is missing Rust component: llvm-tools-preview\n' >&2
  exit 1
}
printf 'PodmanLens Dev Container tooling is ready.\n'
