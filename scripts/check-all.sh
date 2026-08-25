#!/usr/bin/env bash

set -Eeuo pipefail

script_directory="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd -P)"
repository_root="$(cd -- "${script_directory}/.." && pwd -P)"
readonly repository_root
cd -- "${repository_root}"

current_step="preflight"
step=0
readonly total_steps=20

fail() {
  printf 'PodmanLens local validation failed: %s\n' "$1" >&2
  exit 2
}
report_failure() {
  local status=$?
  printf '\nPodmanLens local validation failed during: %s (exit %d)\n' "${current_step}" "${status}" >&2
  exit "${status}"
}
trap report_failure ERR
run_step() {
  local label=$1
  shift
  step=$((step + 1))
  current_step="${label}"
  printf '\n[%02d/%02d] %s\n  +' "${step}" "${total_steps}" "${label}"
  printf ' %q' "$@"
  printf '\n'
  "$@"
}

required_tools=(actionlint cargo cargo-deny cargo-llvm-cov cargo-semver-checks curl git hadolint jq lychee markdownlint-cli2 prettier rustup shellcheck shfmt tombi zizmor)
missing_tools=()
for tool in "${required_tools[@]}"; do
  command -v "${tool}" > /dev/null 2>&1 || missing_tools+=("${tool}")
done
if ((${#missing_tools[@]} != 0)); then
  printf -v missing_list ' %s' "${missing_tools[@]}"
  fail "missing required tool(s):${missing_list}. Use the PodmanLens Dev Container."
fi

list_existing_files() {
  while IFS= read -r -d '' file; do
    [[ -f "${file}" ]] && printf '%s\0' "${file}"
  done < <(git ls-files --cached --others --exclude-standard -z -- "$@")
}
mapfile -d '' markdown_files < <(list_existing_files '*.md')
((${#markdown_files[@]} != 0)) || fail "the repository contains no tracked or untracked Markdown files"

msrv="$({ cargo metadata --locked --no-deps --format-version 1; } | jq -er '[.packages[].rust_version] | unique | if length == 1 and .[0] != null then .[0] else error("workspace packages must declare one rust-version") end')"
readonly msrv
rustup_cargo_home="${RUSTUP_CARGO_HOME:-/usr/local/cargo}"
readonly rustup_cargo_home
if ! env CARGO_HOME="${rustup_cargo_home}" rustup run "${msrv}" rustc --version > /dev/null 2>&1; then
  printf 'Installing the workspace MSRV toolchain %s.\n' "${msrv}"
  rustup toolchain install "${msrv}" --profile minimal
fi

validation_storage_root="${CARGO_TARGET_DIR:-${repository_root}/target}/check-all/podman-lens"
coverage_target_dir="${validation_storage_root}/coverage"
semver_cargo_home="${validation_storage_root}/cargo-semver-home"
semver_target_dir="${validation_storage_root}/cargo-semver-checks-target"
for directory in "${coverage_target_dir}" "${semver_cargo_home}" "${semver_target_dir}"; do
  mkdir -p -- "${directory}"
  [[ -w "${directory}" ]] || fail "isolated validation storage is not writable: ${directory}"
done
readonly coverage_target_dir semver_cargo_home semver_target_dir

run_step "Format Rust" cargo fmt --all
run_step "Format and lint non-Rust files" bash scripts/check-files.sh --fix
run_step "Check whitespace errors" git --no-pager diff --check
run_step "Lint GitHub Actions syntax" actionlint
run_step "Audit GitHub Actions security" zizmor .github/workflows
run_step "Check all workspace targets and features" cargo ci-check
run_step "Check repository policies" cargo ci-policy
run_step "Run Clippy with warnings denied" cargo ci-clippy
run_step "Run workspace tests" cargo ci-test
run_step "Run documentation tests" cargo ci-doctest
run_step "Build documentation with warnings denied" env RUSTDOCFLAGS="-D warnings" cargo ci-doc
run_step "Verify the release package" cargo package --locked --allow-dirty
run_step "Clean coverage artifacts" env CARGO_TARGET_DIR="${coverage_target_dir}" cargo llvm-cov clean --locked
run_step "Measure coverage" env CARGO_TARGET_DIR="${coverage_target_dir}" cargo llvm-cov --locked --no-clean --workspace --all-features --all-targets --summary-only
run_step "Check all targets with the MSRV" env CARGO_HOME="${rustup_cargo_home}" cargo "+${msrv}" ci-check
run_step "Check repository policies with the MSRV" env CARGO_HOME="${rustup_cargo_home}" cargo "+${msrv}" ci-policy
run_step "Audit dependencies, licenses, bans, and sources" cargo deny --all-features check
run_step "Check local documentation links" lychee --config lychee.toml --root-dir . --offline "${markdown_files[@]}"
check_api_compatibility() {
  if [[ "${PODMAN_LENS_SEMVER_CHECK:-0}" == "1" ]]; then
    env CARGO_HOME="${semver_cargo_home}" CARGO_TARGET_DIR="${semver_target_dir}" \
      bash scripts/check-public-api.sh
    return
  fi

  cargo test --locked --test public_api
}

run_step "Check initial public API contract" check_api_compatibility
run_step "Check release metadata" bash scripts/check-release-metadata.sh
printf '\nPodmanLens local validation passed all %d steps.\n' "${total_steps}"
