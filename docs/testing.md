# Testing

PodmanLens tests native behavior without relying on an ambient Podman installation. The ordinary
gate is deterministic and offline. Live checks are explicit and opt-in.

## Complete gate

Run this after the final edit:

```console
./scripts/check-all.sh
```

It formats tracked files, checks GitHub Actions, compiles all targets and examples, runs policy,
unit, integration, and doctests, builds Rustdoc with warnings denied, verifies the release package,
measures coverage, checks the MSRV, audits dependencies, validates documentation links, checks the
public API, and enforces the offline captured-native release contract.

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

## Live conformance

The ignored current-patch test permits only the fixed read-only acquisition probe. It requires an
explicit socket and exact expected version:

```console
PODMAN_LENS_CONFORMANCE_UNIX_SOCKET=/absolute/podman.sock \
PODMAN_LENS_CONFORMANCE_EXPECTED_VERSION=6.1.0 \
cargo test --test current_patch_conformance -- --ignored
```

BoxFerry owns the completed digest-pinned live version and root-mode matrix. Its 48 cells passed
through PodmanLens's production read-only acquisition path without tolerated skips. PodmanLens
keeps only a bounded ignored Unix-socket entry point for explicit consumer runs and does not
discover an ambient endpoint. Privileged execution must not expose a host Podman socket to
untrusted code.

The ordinary local, CI, and release gate remains offline. The captured Podman 6.1 rootful cassettes
anchor native response behavior without creating a second live matrix; expanding that evidence
requires a new privacy review and immutable provenance.

## Coverage and compatibility

The native-field and renderer catalogues link supported fields to their public access point,
planner, renderer, target versions, diagnostic, and focused tests. Catalogue validation includes
mutation tests so a plausible but incorrect target or owner swap fails.

Do not copy mutable catalogue row counts into prose. When a target boundary changes, update the
catalogue, its evidence, positive and negative tests, and the public compatibility guide together.

## Agent-assisted verification

Repository model defaults and role overrides live in [`.codex/`](../.codex/); permissions and
workflow ownership remain defined in [`AGENTS.md`](../AGENTS.md). Reload or start a new trusted
project session after updating configuration; an explicit session override can take precedence.

Use `./scripts/check-all.sh --check` to run the complete gate without formatting repository-owned
files. The default command (or `--fix`) still formats first. Both modes run the same validation;
ignored caches and build artifacts may change. Verifiers report failures without fixing files,
and the primary agent owns the final complete gate and any explicitly authorized merge.

The shell-runner regression tests target the Linux Dev Container gate. Agent-configuration checks
remain platform-independent; the macOS portability lane does not require Linux validation tools.

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
