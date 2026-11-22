#!/usr/bin/env bash
# Verify the exact inputs before starting Podman or resolving any dependencies.
set -Eeuo pipefail
[[ $# == 1 && -d "$1" ]] || {
  echo 'usage: fetch-native-runtime-inputs.sh SCRATCH_DIRECTORY' >&2
  exit 64
}
readonly context=$1
# shellcheck source=scripts/native-runtime-pins.sh
source "$(dirname "${BASH_SOURCE[0]}")/native-runtime-pins.sh"

fail() {
  printf 'Native runtime inputs: %s\n' "$1" >&2
  exit 1
}

[[ "${PODMAN_NATIVE_RPM_BUILD}" =~ ^6\.1\.[0-9]+-[1-9][0-9]*([.][0-9]+)?\.fc45$ ]] ||
  fail 'the Fedora 45 build must select a stable Libpod 6.1 patch'

download() {
  local url=$1 destination=$2 seconds=$3
  curl --fail --location --silent --show-error --proto '=https' --proto-redir '=https' \
    --connect-timeout 10 --max-time "${seconds}" --max-filesize 67108864 \
    --output "${destination}" "${url}"
}

verify() {
  local digest=$1 file=$2 label=$3
  printf '%s  %s\n' "${digest}" "${file}" | sha256sum --check --status ||
    fail "${label} SHA-256 mismatch; review the candidate integrity pins"
}

compose_roots=(
  https://dl.fedoraproject.org/pub/fedora/linux
  https://mirror.math.princeton.edu/pub/fedora/linux
  https://ftp.luis.uni-hannover.de/fedora
)
compose_url=
for root in "${compose_roots[@]}"; do
  url="${root}/${PODMAN_NATIVE_COMPOSE_PATH}/x86_64/os"
  download "${url}/repodata/repomd.xml" "${context}/repomd.xml" 20 || continue
  verify "${PODMAN_NATIVE_REPOMD_SHA256}" "${context}/repomd.xml" 'Fedora compose repomd'
  primary_href="$(sed -n '/<data type="primary">/,/<\/data>/s/.*<location href="\([^"]*\)".*/\1/p' "${context}/repomd.xml")"
  [[ "${primary_href}" == "repodata/${PODMAN_NATIVE_PRIMARY_SHA256}-primary.xml.zst" ]] ||
    fail 'the compose does not name the pinned primary index'
  if download "${url}/${primary_href}" "${context}/primary.xml.zst" 45; then
    verify "${PODMAN_NATIVE_PRIMARY_SHA256}" "${context}/primary.xml.zst" 'Fedora compose primary index'
    compose_url=${url}
    break
  fi
done
[[ -n "${compose_url}" ]] || fail 'the fixed Fedora compose repomd or primary index is unavailable'

fetch_rpm() {
  local digest=$1 output=$2 relative_path=$3 koji_url=$4 root url
  local release_urls=()
  for root in "${compose_roots[@]}"; do
    release_urls+=("${root}/${PODMAN_NATIVE_COMPOSE_PATH}/${relative_path}")
  done
  for url in "${release_urls[@]}" "${koji_url}"; do
    if download "${url}" "${output}" 35; then
      verify "${digest}" "${output}" "${relative_path##*/}"
      return
    fi
  done
  fail "${relative_path##*/} is unavailable from the reviewed Fedora providers"
}

rpm_name="podman-${PODMAN_NATIVE_RPM_BUILD}"
# The Fedora 45 signing key ID selects exactly the signed copies of the RPMs.
signed_koji="https://kojipkgs.fedoraproject.org/packages/podman/${PODMAN_NATIVE_VERSION#v}/${PODMAN_NATIVE_RPM_RELEASE}/data/signed/f577861e"
binary_rpm="${rpm_name}.x86_64.rpm"
source_rpm="${rpm_name}.src.rpm"
fetch_rpm "${PODMAN_NATIVE_RPM_SHA256}" "${context}/podman.rpm" "x86_64/os/Packages/p/${binary_rpm}" \
  "${signed_koji}/x86_64/${binary_rpm}"
fetch_rpm "${PODMAN_NATIVE_SOURCE_RPM_SHA256}" "${context}/podman.src.rpm" "source/tree/Packages/p/${source_rpm}" \
  "${signed_koji}/src/${source_rpm}"

tag_ref="refs/tags/${PODMAN_NATIVE_VERSION}^{}"
tag_revision="$(timeout 20s git ls-remote https://github.com/containers/podman.git "${tag_ref}")" ||
  fail 'the upstream Podman tag is unavailable'
[[ "${tag_revision}" == "${PODMAN_NATIVE_SOURCE_REVISION}"$'\t'"${tag_ref}" ]] ||
  fail 'the upstream annotated tag does not resolve to the pinned commit'
download "https://codeload.github.com/containers/podman/tar.gz/refs/tags/${PODMAN_NATIVE_VERSION}" \
  "${context}/upstream.tar.gz" 45 || fail 'the upstream Podman archive is unavailable'
verify "${PODMAN_NATIVE_SOURCE_ARCHIVE_SHA256}" "${context}/upstream.tar.gz" 'upstream Podman archive'
rm -- "${context}/upstream.tar.gz"

image_digest="${PODMAN_NATIVE_BASE_IMAGE#*@}"
[[ "${PODMAN_NATIVE_BASE_IMAGE}" =~ ^registry\.fedoraproject\.org/fedora:45@sha256:[a-f0-9]{64}$ ]] ||
  fail 'the base image must pin a Fedora 45 manifest digest'
curl --fail --location --silent --show-error --proto '=https' --proto-redir '=https' --connect-timeout 10 \
  --max-time 20 --max-filesize 1048576 \
  --header 'Accept: application/vnd.oci.image.index.v1+json, application/vnd.docker.distribution.manifest.list.v2+json' \
  --output "${context}/base-index.json" "https://registry.fedoraproject.org/v2/fedora/manifests/${image_digest}" ||
  fail 'the pinned Fedora base image manifest is unavailable'
verify "${image_digest#sha256:}" "${context}/base-index.json" 'Fedora base image manifest'
jq -e '[.manifests[] | select(.platform.os == "linux" and .platform.architecture == "amd64")] | length == 1' \
  "${context}/base-index.json" > /dev/null || fail 'the Fedora base index must select one linux/amd64 manifest'

printf '%s\n' "${compose_url}" > "${context}/compose-url"
printf 'Verified native runtime inputs for %s and the Fedora 45 release compose.\n' "${rpm_name}"
