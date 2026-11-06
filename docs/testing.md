# Testing

PodmanLens captured-native tests are deterministic and offline. Live API conformance remains
explicit and opt-in for development, and mandatory for Release.

## Complete gate

Run this after the final edit:

```console
./scripts/check-all.sh
```

For bounded feedback use `python3 scripts/validation-plan.py run-local`. Public guides run
file/link checks and the guide contract. Mixed, unknown, policy and code changes, plus `--full`,
retain complete validation. `--docs-only` cannot bypass required checks. PR and queue plans use
trusted base policy; Required CI accepts only successful selected jobs and planned skips.
Main, manual and Release remain complete. Local modes reject external `CARGO_TARGET_DIR`.

Shared locked Python owns uv, Ruff and ty; regressions verify wiring and failures.
Ordinary full hosted and local validation preserve existing main checks without image jobs or
local image leaves. Direct Dev Container inputs and the legacy image installer remain held
pending qualification (BoxFerry #563). Ordinary shared metadata,
package and compiler updates remain automatic; their resulting image closure is unqualified.
Use VS Code Dev Containers with existing Podman storage, preferably rootless. No private store or
host sudo is required. GitHub CI image qualification and updater activation remain separate.

## Test layers

| Layer                        | Purpose                                                                   |
| ---------------------------- | ------------------------------------------------------------------------- |
| Unit tests                   | Constructors, validation, ordering, redaction, and pure conversion rules  |
| Public API tests             | Compile supported workflows as an external crate consumer                 |
| Fixture transport tests      | Probe, list, inspect, malformed input, and partial acquisition            |
| Discovery tests              | Roots, closure, grouping, shared boundaries, findings, and explanations   |
| Planning and rendering tests | Target gates, exact CLI and Libpod semantics, and all-or-nothing outcomes |
| Schema and golden tests      | Exact snapshot and deployment artifacts plus invalid mutations            |
| Complex cassette tests       | Complete request matching across reviewed versions and simulated contexts |
| Captured-native tests        | Privacy admission, exact replay, provenance, and semantic regressions     |
| Repository policy tests      | Governance, package contents, workflows, and documentation contracts      |
| Rustdoc and guide tests      | Compilable examples, page inventory, claims, and navigation               |

## Fixtures and cassettes

Committed fixtures are deterministic, offline, provenance-bearing, and synthetic or sanitized.
Captured-native cassettes replace raw environment data. Complex cassettes bind each response to
its exact Libpod method, path and query; replay rejects unexpected, repeated, reordered, missing,
duplicate or unconsumed requests. Each reviewed version covers simulated rootless/rootful
containers, pods, networks, volumes, images and secret metadata.

Update fixture provenance, SHA-256 manifests and focused behavioral assertions together.
Fixtures, failures, snapshots and goldens must exclude real endpoints, environment values,
credentials, secret payloads, protected health commands and sensitive native data.

`podman-lens-cassette-v1` and native capture manifests are test formats, not public APIs or
compatibility promises. Production modules never parse them. Test support and
`scripts/check-native-release-contract.sh` enforce schemas, hashes, privacy, replay and semantics.

## Focused commands

Public documentation and examples:

```console
cargo test --test public_guides
cargo test --doc
cargo check --examples
```

Complex request-aware acquisition:

```console
cargo test --test cassette_contract
cargo test --test complex_corpus
cargo test --test input_corpus corpus_manifest_verifies_fixed_provenance_and_hashes
```

Captured-native release evidence:

```console
./scripts/check-native-release-contract.sh
```

Public API and repository contracts:

```console
cargo test --test public_api
cargo test --test repository_policy
```

The complete local gate always runs both the positive `tests/public_api.rs` contract and the
shared development-purpose API check against a trusted Git base. Legacy
`PODMAN_LENS_SEMVER_CHECK` values do not disable this comparison. `scripts/check-public-api.sh`
(or `--development`) selects the same declared shared task; `--published` explicitly checks the
canonical published baseline with manifest-inferred release allowance for release preparation.
The shared helper owns trusted-base selection, initial-crate absence and positive-contract proof.
Missing or stale Git authority fails with fetch/retry guidance; local breaking-history markers do
not waive the development check.

Before compiler work, the shared loader verifies the declared Cargo plugin and selects its Rust
channel. The API check uses that channel and the same isolated Cargo home and target directory.
Unavailable or mismatched tools fail closed. `RUSTUP_CARGO_HOME` still selects the host Cargo home
for MSRV checks. CI uses development purpose for ordinary events; Release and validation-only
explicitly require published-purpose evidence for the exact candidate.

## Native conformance

The ignored current-patch test permits only the fixed read-only acquisition probe. It requires an
explicit socket and exact expected version:

```console
source scripts/native-runtime-pins.sh
PODMAN_LENS_CONFORMANCE_UNIX_SOCKET=/absolute/podman.sock \
PODMAN_LENS_CONFORMANCE_EXPECTED_VERSION="${PODMAN_NATIVE_VERSION#v}" \
PODMAN_LENS_CONFORMANCE_EXPECTED_ROOT_MODE=rootless \
PODMAN_LENS_CONFORMANCE_EXPECTED_IMAGE="${PODMAN_NATIVE_BASE_IMAGE}" \
cargo test --test current_patch_conformance --test native_service_conformance -- --ignored
```

BoxFerry owns the broader completed digest-pinned live version and root-mode matrix. Its 48 cells passed
through PodmanLens's production read-only acquisition path without tolerated skips. PodmanLens
keeps only a bounded ignored Unix-socket entry point for explicit consumer runs and does not
discover an ambient endpoint. Privileged execution must not expose a host Podman socket to
untrusted code.

The captured Podman 6.1 rootful cassettes
anchor native response behavior without creating a second live matrix; expanding that evidence
requires a new privacy review and immutable provenance.

After independent review of the whole same-repository issue branch, including its workflow and
admission script, manually run `gh workflow run native-podman-conformance.yml --ref
TheRealBecks/issue98 -f candidate_sha=<reviewed-head-sha>` with the exact 40-character head SHA.
Admission checks that SHA, the current branch and checkout heads, and both original and rerun
actors' write-or-higher permission. Moved branches, tags, forks, and unavailable permission APIs
fail before the native matrix. This validation-only path never runs for untrusted PR events or
replaces fresh Release evidence.

`Native Podman conformance` creates a fresh disposable Podman service, passes only its run-scoped
socket to the ignored tests, and uploads SHA/run/attempt/task-bound evidence. Release calls that reusable
worker; an unavailable, failed, timed-out, cancelled, or skipped worker blocks publication. It
provisions a bounded sanitized resource set before bodyless-GET acquisition checks containers,
pods, networks, volumes, images, and secret metadata; it captures no service payload and resets
the temporary store. Both reviewed images run through a rootful host-Podman launcher with isolated
per-cell root, runroot, temporary, file-lock, and socket state. Common service arguments provide
`/dev/fuse` and `label=disable`; the rootful image alone uses a privileged outer container, while
the rootless image stays unprivileged and adds only
`apparmor=unconfined`. The workflow independently requires UID 1000 and
`Host.Security.Rootless=true` for rootless, and UID 0 plus `Host.Security.Rootless=false` for
rootful. Only reviewed, checksum-verified runtime inputs from an admitted exact candidate enter
that no-secret boundary. Release admission remains current-default-branch-only. Provisioning
precedes API startup through a bounded status handshake.
[ADR 0017](decisions/0017-isolated-native-release-conformance.md) records the rootless launcher,
initial-process requirement, socket isolation, and failure limits.
Setup failures still upload compact evidence because expected-version provenance is derived
directly from the candidate's active pin file. Cleanup removes the service, image, and isolated
launcher state.
Cleanup derives the isolated state path from validated run, attempt, and cell IDs; it preserves
leaks. SIGKILL cannot run traps.
Build storage budgets and peak evidence follow [ADR 0017](decisions/0017-isolated-native-release-conformance.md).
The `ubuntu-24.04` host launcher is infrastructure. Each cell builds from a pinned Fedora 45
base, fixed Beta compose, and checksum-verified Koji RPM. Release evidence records the package
closure and image ID. Beta is test infrastructure; native root-mode and API checks still gate
support. The qualified runtime tuple stays fixed; target changes require separate qualification. The 6.1.0 cassettes remain historical.
The current-patch probe simulation is synthetic; live checks cover authored hints and 404.

## Coverage and compatibility

Machine catalogues bind fields to API accessors, planners, renderers, diagnostics and target
versions. Mutation tests reject incorrect targets and owners. Change catalogue evidence, tests and
public compatibility guidance together; keep mutable row counts out of prose.

## Agent-assisted verification

Role settings are in [`.codex/`](../.codex/); authority is in [`AGENTS.md`](../AGENTS.md).
Verifiers run `./scripts/check-all.sh --check`; the primary owns final validation. Neither mode
authorizes publication. Shell-runner tests target Linux; macOS has no runner evidence.

The fuller BoxFerry consumer remains ignored and cannot discover an ambient socket. Invoke it
only with the explicit fixture socket and one exact reviewed expected version (`5.8.6` or
`6.1.0`):

```console
PODMAN_LENS_BOXFERRY_UNIX_SOCKET=/absolute/fixture-podman.sock \
PODMAN_LENS_BOXFERRY_EXPECTED_VERSION=6.1.0 \
cargo test --test native_release_contract \
  boxferry_consumer_replays_only_an_explicit_bounded_unix_service -- --ignored
```

The ordinary `scripts/check-native-release-contract.sh` gate never runs ignored or live tests.
