#!/usr/bin/env bash
# Pure helpers shared by the isolated native runtime builder and offline tests.

normalize_native_image_id() {
  local raw_id=${1-}
  raw_id=${raw_id#sha256:}
  [[ "${raw_id}" =~ ^[a-f0-9]{64}$ ]] || return 1
  printf 'sha256:%s\n' "${raw_id}"
}

native_storage_budget_allows() {
  local available_kib=${1-}
  local state_kib=${2-}
  local minimum_available_kib=${3-}
  local maximum_state_kib=${4-}
  [[ "${available_kib}" =~ ^[0-9]+$ && "${state_kib}" =~ ^[0-9]+$ ]] || return 1
  [[ "${minimum_available_kib}" =~ ^[0-9]+$ && "${maximum_state_kib}" =~ ^[0-9]+$ ]] || return 1
  ((available_kib >= minimum_available_kib && state_kib <= maximum_state_kib))
}
