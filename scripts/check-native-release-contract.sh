#!/usr/bin/env bash

set -Eeuo pipefail

script_directory="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd -P)"
repository_root="$(cd -- "${script_directory}/.." && pwd -P)"
readonly repository_root

cd -- "${repository_root}"

# This gate is deliberately offline. Live BoxFerry conformance is an ignored,
# caller-selected socket entry point and is never enabled here.
cargo test --locked --test cassette_contract --test native_release_contract
