# Testing

PodmanLens tests native behavior without relying on an ambient Podman installation. The ordinary
gate is deterministic and offline. Live checks are explicit and opt-in.

## Complete gate

Run this after the final edit:

```console
./scripts/check-all.sh
```

For measured, change-aware feedback use `python3 scripts/validation-plan.py run-local`.
Every public guide edit runs file/link checks and `tests/public_guides.rs`, which checks page
inventory and navigation; other edits use the full gate. `--full` forces it, while `--docs-only`
rejects current guides. PRs use the trusted-base classifier and initially run full. Main, manual,
and Release run full; Release also requires native conformance. Every selected job must pass.
Both local modes reject `CARGO_TARGET_DIR` outside the worktree to avoid stale test binaries.
Focused checks never replace final `check-all.sh`.

The gate checks formatting, Actions, Rust targets/tests/docs/package, coverage, MSRV,
dependencies, links, public API, and offline captured-native evidence.

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

Every behavior change needs focused positive and negative coverage. Prefer small unit tests plus one
public or end-to-end case when a contract changes.

## Fixtures and cassettes

All committed input fixtures are offline, deterministic, provenance-bearing, and synthetic or
privacy-sanitized. Captured-native cassettes derive from reviewed responses, but contain only
deterministic replacements rather than raw environment data.

The complex cassette schema binds every response to an exact Libpod method and complete path plus
query. Replay rejects unexpected, repeated, reordered, missing, duplicate, or unconsumed requests.
Each reviewed Podman version has simulated rootless and rootful context coverage across containers,
pods, networks, volumes, images, and secret metadata.

Fixture manifests pin provenance and SHA-256 digests. Never edit a fixture without updating its
manifest and the focused behavior assertion that explains why it exists.

Fixtures, failures, snapshots, and goldens must not contain real endpoints, environment values,
credentials, secret payloads, protected health commands, or raw sensitive native data.

`podman-lens-cassette-v1` and the native capture manifest are repository test formats. They are not
public Rust APIs, trusted deserialization APIs, migration interchange formats, or compatibility
promises. They may be packaged for auditability, but production modules do not import or parse
them. Their strict schemas, SHA-256 registry, privacy mutations, replay behavior, and semantic
expectations are enforced by test support and `scripts/check-native-release-contract.sh`.

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

These focused commands help during development but do not replace `./scripts/check-all.sh`.

Set `PODMAN_LENS_SEMVER_CHECK=1` to run the same repository-owned published-API command used by
the CI API job. It requires a reachable release tag and the published crate baseline; the initial
contract path continues to use `tests/public_api.rs`.

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

The ordinary local, CI, and release gate remains offline. The captured Podman 6.1 rootful cassettes
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
support. Renovate updates need manual review. The 6.1.0 cassettes remain historical.
The current-patch probe simulation is synthetic; live checks cover authored hints and 404.

## Coverage and compatibility

The native-field and renderer catalogues link supported fields to their public access point,
planner, renderer, target versions, diagnostic, and focused tests. Catalogue validation includes
mutation tests so a plausible but incorrect target or owner swap fails.

Do not copy mutable catalogue row counts into prose. When a target boundary changes, update the
catalogue, its evidence, positive and negative tests, and the public compatibility guide together.

## Agent-assisted verification

Model defaults and roles are in [`.codex/`](../.codex/); scope and GitHub authority are in
[`AGENTS.md`](../AGENTS.md). Keep primary overrides at Sol/xhigh. The primary owns final
validation and merges; verifiers use `./scripts/check-all.sh --check` without source formatting.
Default/`--fix` formats first; both run the same checks. Shell-runner tests target Linux.
macOS client compatibility is intended but has no runner evidence.

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
