#!/usr/bin/env bash
# Admit one exact, current candidate before any native runtime or cleanup step runs.
set -Eeuo pipefail

[[ "${GITHUB_REPOSITORY:-}" == 'Strukturpiloten/podman-lens' ]]
[[ "${GITHUB_SHA:-}" =~ ^[a-f0-9]{40}$ ]]
[[ "${NATIVE_DEFAULT_BRANCH:-}" != '' ]]
git check-ref-format --branch "${NATIVE_DEFAULT_BRANCH}" > /dev/null
[[ "$(git rev-parse HEAD)" == "${GITHUB_SHA}" ]] || {
  echo 'Native candidate checkout does not match this run SHA.' >&2
  exit 1
}

case "${NATIVE_RELEASE_VALIDATION:-}" in
  true)
    [[ "${GITHUB_REF}" == "refs/heads/${NATIVE_DEFAULT_BRANCH}" ]] || {
      echo 'Release native conformance requires the current default branch.' >&2
      exit 1
    }
    ;;
  false)
    [[ "${NATIVE_CANDIDATE_SHA:-}" == "${GITHUB_SHA}" ]] || {
      echo 'The explicit 40-character candidate SHA must match this run.' >&2
      exit 1
    }
    case "${GITHUB_EVENT_NAME:-}" in
      pull_request)
        [[ "${NATIVE_PR_NUMBER:-}" =~ ^[1-9][0-9]*$ ]]
        [[ "${GITHUB_REF}" == "refs/pull/${NATIVE_PR_NUMBER}/merge" ]]
        ;;
      merge_group)
        [[ "${NATIVE_QUEUE_BASE_REF:-}" == "refs/heads/${NATIVE_DEFAULT_BRANCH}" ]]
        [[ "${GITHUB_REF}" == "refs/heads/gh-readonly-queue/${NATIVE_DEFAULT_BRANCH}/"* ]]
        [[ "${GITHUB_REF}" == "${NATIVE_QUEUE_HEAD_REF:-}" && "${GITHUB_SHA}" == "${NATIVE_QUEUE_HEAD_SHA:-}" ]]
        ;;
      workflow_dispatch)
        [[ "${GITHUB_REF}" =~ ^refs/heads/TheRealBecks/issue[0-9]+$ ]] || {
          echo 'Manual native conformance requires a same-repository issue branch.' >&2
          exit 1
        }
        [[ "${GITHUB_REF}" != "refs/heads/${NATIVE_DEFAULT_BRANCH}" ]]
        [[ "${ORIGINAL_ACTOR:-}" =~ ^[A-Za-z0-9][A-Za-z0-9-]{0,38}$ ]]
        [[ "${RERUN_ACTOR:-}" =~ ^[A-Za-z0-9][A-Za-z0-9-]{0,38}$ ]]
        for actor in "${ORIGINAL_ACTOR}" "${RERUN_ACTOR}"; do
          permission="$(gh api --header 'X-GitHub-Api-Version: 2022-11-28' "repos/${GITHUB_REPOSITORY}/collaborators/${actor}/permission")" || {
            echo "Unable to verify native dispatch authorization for ${actor}." >&2
            exit 1
          }
          jq --exit-status 'type == "object" and (.permission == "write" or .permission == "admin" or .role_name == "maintain" or .role_name == "write" or .role_name == "admin")' <<< "${permission}" > /dev/null || {
            echo "Native dispatch actor ${actor} lacks write, maintain, or admin permission." >&2
            exit 1
          }
        done
        ;;
      *)
        echo 'Unsupported event for native candidate validation.' >&2
        exit 1
        ;;
    esac
    ;;
  *)
    echo 'Native conformance mode must be explicit.' >&2
    exit 1
    ;;
esac

# Require the exact live repository ref, including synthetic PR or queue refs.
remote_head="$(git ls-remote --exit-code --refs origin "${GITHUB_REF}")" || {
  echo 'Native candidate ref no longer exists.' >&2
  exit 1
}
[[ "${remote_head}" == "${GITHUB_SHA}"$'\t'"${GITHUB_REF}" ]] || {
  echo 'Native candidate ref moved after the run was created.' >&2
  exit 1
}
printf 'candidate_sha=%s\n' "${GITHUB_SHA}" >> "${GITHUB_OUTPUT}"
