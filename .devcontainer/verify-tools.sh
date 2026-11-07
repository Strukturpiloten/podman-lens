#!/usr/bin/env bash

set -Eeuo pipefail

script_directory="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd -P)"
repository_root="$(cd -- "${script_directory}/.." && pwd -P)"
tooling_root="/opt/podman-lens-tooling"

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

tools=(actionlint cargo cargo-clippy cargo-deny cargo-llvm-cov cargo-semver-checks curl gh git hadolint jq lychee markdownlint-cli2 node npm prettier python3 ruff rustc rustfmt rustup shellcheck shfmt tombi ty uv zizmor)
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
# Compare actual executables with the retained selected authority, not copied version pins.
python3 - "${repository_root}" "${tooling_root}" << 'PYTHON'
import json
from pathlib import Path
import re
import subprocess
import sys
import tomllib

try:
    repository, tooling = map(Path, sys.argv[1:])
    manifest = json.loads((repository / ".github/automation.json").read_text())
    retained = json.loads((tooling / ".github/automation.json").read_text())
    if retained != manifest:
        raise ValueError("retained shared-tool authority differs from the checkout")
    project = tooling / "target/boxferry-automation" / manifest["revision"] / "automation"
    metadata = tomllib.loads((project / "pyproject.toml").read_text())
    uv_requirement = metadata["tool"]["uv"]["required-version"]
    if not uv_requirement.startswith("=="):
        raise ValueError("selected uv requirement must be exact")
    versions = {"uv": uv_requirement[2:]}
    for name in ("ruff", "ty"):
        matches = [value.split("==", 1)[1] for value in metadata["dependency-groups"]["dev"]
                   if value.startswith(name + "==")]
        if len(matches) != 1:
            raise ValueError(f"selected project must declare one exact {name} version")
        versions[name] = matches[0]
    image = (repository / ".devcontainer/Dockerfile").read_text()
    node = re.search(
        r"(?m)^# release: (?P<tag>(?P<version>[0-9]+\.[0-9]+\.[0-9]+)-[^\s]+)\n"
        r"FROM docker\.io/library/node:(?P=tag)@sha256:[0-9a-f]{64}(?: AS [^\s]+)?$",
        image,
    )
    if node is None:
        raise ValueError("Node image must retain matching literal tag and digest declarations")
    versions["node"] = node["version"]
    for name, expected in versions.items():
        if re.fullmatch(r"[0-9]+\.[0-9]+\.[0-9]+", expected) is None:
            raise ValueError(f"selected {name} version must be exact")
        actual = subprocess.run([name, "--version"], check=True, capture_output=True,
                                text=True, timeout=30).stdout.strip()
        pattern = "v" + re.escape(expected) if name == "node" else (
            re.escape(name + " " + expected) + r"(?: \([^\r\n]+\))?"
        )
        if re.fullmatch(pattern, actual) is None:
            raise ValueError(f"{name} executable does not match selected version {expected}")
        subprocess.run([name, "--help"], check=True, stdout=subprocess.DEVNULL,
                       stderr=subprocess.PIPE, timeout=30)
    channel = tomllib.loads((repository / "rust-toolchain.toml").read_text())["toolchain"]["channel"]
    compiler = subprocess.run(["rustup", "run", channel, "rustc", "--version"], check=True,
                              capture_output=True, text=True, timeout=30).stdout.strip()
    if not compiler.startswith("rustc " + channel + " "):
        raise ValueError("installed Rust compiler does not match rust-toolchain.toml")
except (KeyError, OSError, ValueError, subprocess.SubprocessError) as error:
    print(f"PodmanLens Dev Container tool verification failed: {error}", file=sys.stderr)
    raise SystemExit(1) from None
PYTHON
printf 'PodmanLens Dev Container tooling is ready.\n'
