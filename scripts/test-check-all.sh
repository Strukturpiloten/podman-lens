#!/usr/bin/env bash
# Exercise the real gate's command dispatch without invoking compilers or downloading tools.
set -Eeuo pipefail

script_directory="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd -P)"
# Capture the selected Bash before PATH is populated with mocks (macOS ships an older /bin/bash).
bash_executable="$(command -v bash)"
# Synthetic metadata proves the gate consumes Cargo's result, not a copied compiler pin.
readonly mock_msrv=9.8.7
readonly mock_semver_rust=9.9.9
test_root="$(mktemp -d)"
trap 'rm -r -- "${test_root}"' EXIT
mkdir -p "${test_root}/repository/scripts" "${test_root}/repository/site" "${test_root}/bin"
cp -- "${script_directory}/check-all.sh" "${test_root}/repository/scripts/check-all.sh"
cp -- "${script_directory}/../README.md" "${test_root}/repository/README.md"

# These mock commands record exactly what the unmodified gate requests.
cat > "${test_root}/bin/mock" << 'MOCK'
#!/bin/bash
set -Eeuo pipefail
command_name="${0##*/}"
# Metadata is piped to jq, so its command order is deliberately not asserted.
case "${command_name}:$*" in
  cargo:metadata* | jq:*) ;;
  *)
    printf -v command_line '%s ' "${command_name}" "$@"
    printf '%s\n' "${command_line% }" >> "${CHECK_ALL_TEST_LOG}"
    ;;
esac
if [[ "${CHECK_ALL_TEST_FAIL:-}" == "${command_name}" ||
  "${CHECK_ALL_TEST_FAIL_COMMAND:-}" == "${command_name} $*" ]]; then
  exit 17
fi
case "${command_name}:$*" in
  git:ls-files*) printf 'README.md\0' ;;
  jq:*)
    if [[ "$*" == '-er .rust_channel' ]]; then
      IFS= read -r selection
      [[ "${selection}" == "{\"rust_channel\":\"${CHECK_ALL_TEST_SEMVER_RUST}\"}" ]] || exit 18
      printf '%s\n' "${CHECK_ALL_TEST_SEMVER_RUST}"
    else
      printf '%s\n' "${CHECK_ALL_TEST_MSRV}"
    fi
    ;;
  rustup:run* | cargo:+*)
    [[ "${CARGO_HOME}" == "${CHECK_ALL_TEST_EXPECTED_HOST_CARGO_HOME}" ]]
    ;;
  python3:scripts/shared-ci.py\ cargo-tools\ verify*)
    [[ "${CARGO_HOME}" == "${CARGO_TARGET_DIR%/cargo-semver-checks-target}/cargo-semver-home" ]]
    if [[ "${CHECK_ALL_TEST_BAD_SEMVER_SELECTION:-0}" == "1" ]]; then
      printf '{}\n'
    else
      printf '{"rust_channel":"%s"}\n' "${CHECK_ALL_TEST_SEMVER_RUST}"
    fi
    ;;
  bash:scripts/check-public-api.sh)
    [[ "${CARGO_HOME}" == "${CARGO_TARGET_DIR%/cargo-semver-checks-target}/cargo-semver-home" ]]
    [[ "${RUSTUP_TOOLCHAIN}" == "${CHECK_ALL_TEST_SEMVER_RUST}" ]]
    [[ "${CARGO}" == cargo ]]
    ;;
esac
MOCK
chmod +x "${test_root}/bin/mock"
for tool in actionlint bash cargo cargo-deny cargo-llvm-cov cargo-semver-checks cspell curl git \
  hadolint jq lychee markdownlint-cli2 npm prettier python3 rustup shellcheck shfmt tombi uv zizmor; do
  ln -s mock "${test_root}/bin/${tool}"
done

run_gate() {
  local label=$1
  shift
  local mock_repository="${CHECK_ALL_TEST_REPOSITORY_ROOT:-${test_root}/repository}"
  local mock_target="${CHECK_ALL_TEST_TARGET_DIR:-${mock_repository}/target}"
  : > "${test_root}/${label}.commands"
  PATH="${test_root}/bin:${PATH}" \
    CHECK_ALL_TEST_LOG="${test_root}/${label}.commands" \
    CHECK_ALL_TEST_MSRV="${CHECK_ALL_TEST_MSRV:-${mock_msrv}}" \
    CHECK_ALL_TEST_SEMVER_RUST="${mock_semver_rust}" \
    CHECK_ALL_TEST_EXPECTED_HOST_CARGO_HOME="${CHECK_ALL_TEST_HOST_CARGO_HOME:-${test_root}/host-cargo}" \
    RUSTUP_CARGO_HOME="${CHECK_ALL_TEST_HOST_CARGO_HOME:-${test_root}/host-cargo}" \
    CARGO_TARGET_DIR="${mock_target}" RUSTUP_TOOLCHAIN=ambient-toolchain CARGO=unverified-cargo \
    BOXFERRY_SEMVER_RELEASE_TYPE="" PODMAN_LENS_SEMVER_CHECK="${CHECK_ALL_TEST_SEMVER_CHECK:-0}" \
    BOXFERRY_WEBSITE_SOURCE_MODE=local \
    "${bash_executable}" "${mock_repository}/scripts/check-all.sh" "$@" \
    > "${test_root}/${label}.output" 2>&1
}

assert_contains() {
  if ! grep -Fxq -- "$2" "${test_root}/$1.commands"; then
    printf 'Missing expected command: %s\n' "$2" >&2
    return 1
  fi
}

export BOXFERRY_WEBSITE_FORMAT_MODE=fix
run_gate default
run_gate fix --fix
run_gate check --check

# A target owned by a former worktree must not be reused after relocation.
mkdir -p "${test_root}/relocated/scripts"
cp -- "${script_directory}/check-all.sh" "${test_root}/relocated/scripts/check-all.sh"
status=0
CHECK_ALL_TEST_REPOSITORY_ROOT="${test_root}/relocated" \
  CHECK_ALL_TEST_TARGET_DIR="${test_root}/repository/target" \
  run_gate relocated --check || status=$?
[[ "${status}" == 2 && ! -s "${test_root}/relocated.commands" ]]
grep --fixed-strings --quiet -- 'CARGO_TARGET_DIR must be inside this worktree' \
  "${test_root}/relocated.output"

diff -u "${test_root}/default.commands" "${test_root}/fix.commands"
assert_contains fix "bash scripts/check-files.sh --fix"
assert_contains check "bash scripts/check-files.sh --check"
assert_contains check "cargo +${mock_msrv} ci-check"
assert_contains check "cargo +${mock_msrv} ci-policy"
CHECK_ALL_TEST_MSRV=9.8.8 run_gate metadata --check
assert_contains metadata "cargo +9.8.8 ci-check"
assert_contains metadata "cargo +9.8.8 ci-policy"
if grep -q '^cargo fmt' "${test_root}/fix.commands"; then
  assert_contains fix "cargo fmt --all"
  assert_contains check "cargo fmt --all -- --check"
else
  assert_contains fix "uv run --frozen ruff format scripts tests"
  assert_contains check "uv run --frozen ruff format --check scripts tests"
  assert_contains check "uv run --frozen python scripts/generate_brand_assets.py --check"
  # The explicit flag wins over the website's existing environment default.
  export BOXFERRY_WEBSITE_FORMAT_MODE=check
  run_gate explicit_fix --fix
  run_gate environment_check
  diff -u "${test_root}/fix.commands" "${test_root}/explicit_fix.commands"
  diff -u "${test_root}/check.commands" "${test_root}/environment_check.commands"
fi

# Both modes must request every non-formatting validation step, in the same order.
for mode in fix check; do
  grep -vE '^(cargo fmt|bash scripts/check-files.sh|uv run --frozen ruff format|uv run --frozen python scripts/generate_brand_assets.py)' \
    "${test_root}/${mode}.commands" > "${test_root}/${mode}.validation"
done
diff -u "${test_root}/fix.validation" "${test_root}/check.validation"

for invalid in --unknown --checks; do
  status=0
  run_gate invalid "${invalid}" || status=$?
  [[ "${status}" == 2 && ! -s "${test_root}/invalid.commands" ]]
done
status=0
run_gate extra --check --fix || status=$?
[[ "${status}" == 2 && ! -s "${test_root}/extra.commands" ]]

# Every local mode retains the positive contract and the trusted-base API check,
# including an explicit legacy opt-out value.
for mode in default fix check; do
  assert_contains "${mode}" "cargo test --locked --test public_api"
  assert_contains "${mode}" "bash scripts/check-public-api.sh"
done
CHECK_ALL_TEST_SEMVER_CHECK=0 run_gate legacy_opt_out --check
assert_contains legacy_opt_out "bash scripts/check-public-api.sh"
# Distinct synthetic MSRV/selected/ambient channels catch accidental compiler reuse.
semver_verify="python3 scripts/shared-ci.py cargo-tools verify --repository-root ${test_root}/repository --package cargo-semver-checks"
for mode in fix check; do
  CHECK_ALL_TEST_SEMVER_CHECK=1 CHECK_ALL_TEST_HOST_CARGO_HOME="${test_root}/custom-host-cargo" \
    run_gate "semver_${mode}" "--${mode}"
  assert_contains "semver_${mode}" "${semver_verify}"
  assert_contains "semver_${mode}" "bash scripts/check-public-api.sh"
  [[ "$(grep -Fxc -- "${semver_verify}" "${test_root}/semver_${mode}.commands")" == 1 ]]
  [[ "$(grep -Fxc -- 'bash scripts/check-public-api.sh' "${test_root}/semver_${mode}.commands")" == 1 ]]
  [[ "$(grep -nF -- "${semver_verify}" "${test_root}/semver_${mode}.commands" | cut -d: -f1)" -lt "$(grep -n '^cargo fmt' "${test_root}/semver_${mode}.commands" | cut -d: -f1)" ]]
  assert_contains "semver_${mode}" "cargo test --locked --test public_api"
  # A refused plugin or malformed selection stops before formatting and compatibility.
  for failure in refused malformed; do
    status=0
    if [[ "${failure}" == refused ]]; then
      CHECK_ALL_TEST_SEMVER_CHECK=1 CHECK_ALL_TEST_FAIL_COMMAND="${semver_verify}" \
        run_gate "semver_${failure}_${mode}" "--${mode}" || status=$?
      [[ "${status}" == 17 ]]
    else
      CHECK_ALL_TEST_SEMVER_CHECK=1 CHECK_ALL_TEST_BAD_SEMVER_SELECTION=1 \
        run_gate "semver_${failure}_${mode}" "--${mode}" || status=$?
      [[ "${status}" == 18 ]]
    fi
    [[ "$(tail -n 1 "${test_root}/semver_${failure}_${mode}.commands")" == "${semver_verify}" ]]
    grep -Fq 'Verify exact local Cargo SemVer selection' "${test_root}/semver_${failure}_${mode}.output"
  done

  status=0
  CHECK_ALL_TEST_SEMVER_CHECK=1 CHECK_ALL_TEST_FAIL_COMMAND='bash scripts/check-public-api.sh' \
    run_gate "semver_failure_${mode}" "--${mode}" || status=$?
  [[ "${status}" == 17 ]]
  [[ "$(tail -n 1 "${test_root}/semver_failure_${mode}.commands")" == 'bash scripts/check-public-api.sh' ]]
done

# A failed check must stop the gate and preserve its exit status in either mode.
export CHECK_ALL_TEST_FAIL=actionlint
for mode in fix check; do
  status=0
  run_gate failure "--${mode}" || status=$?
  [[ "${status}" == 17 ]]
  [[ "$(tail -n 1 "${test_root}/failure.commands")" == "actionlint" ]]
done
printf 'Complete gate mode regression tests passed.\n'
