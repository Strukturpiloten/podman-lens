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
measures coverage, checks the MSRV, audits dependencies, validates documentation links, and checks
the public API.

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
| Repository policy tests      | Governance, package contents, workflows, and documentation contracts      |
| Rustdoc and guide tests      | Compilable examples, page inventory, claims, and navigation               |

Every behavior change needs focused positive and negative coverage. Prefer small unit tests plus one
public or end-to-end case when a contract changes.

## Fixtures and cassettes

All committed input fixtures are offline, deterministic, provenance-bearing, and synthetic or
sanitized. They are not exports of live Podman environments.

The complex cassette schema binds every response to an exact Libpod method and complete path plus
query. Replay rejects unexpected, repeated, reordered, missing, duplicate, or unconsumed requests.
Each reviewed Podman version has simulated rootless and rootful context coverage across containers,
pods, networks, volumes, images, and secret metadata.

Fixture manifests pin provenance and SHA-256 digests. Never edit a fixture without updating its
manifest and the focused behavior assertion that explains why it exists.

Fixtures, failures, snapshots, and goldens must not contain real endpoints, environment values,
credentials, secret payloads, protected health commands, or raw sensitive native data.

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

Public API and repository contracts:

```console
cargo test --test public_api
cargo test --test repository_policy
```

These focused commands help during development but do not replace `./scripts/check-all.sh`.

## Live conformance

The ignored current-patch test permits only the fixed read-only acquisition probe. It requires an
explicit socket and exact expected version:

```console
PODMAN_LENS_CONFORMANCE_UNIX_SOCKET=/absolute/podman.sock \
PODMAN_LENS_CONFORMANCE_EXPECTED_VERSION=6.1.0 \
cargo test --test current_patch_conformance -- --ignored
```

The complete live version/context workflow is deferred to
[GitHub issue #3](https://github.com/Strukturpiloten/podman-lens/issues/3). It will be manually
dispatched, cover every required cell without tolerated skips, and use isolated runtimes, storage,
and explicit Unix sockets. Privileged execution must not expose a host Podman socket to untrusted
code.

A disposable test harness may eventually provision or apply output for comparison. That does not
add mutation or execution to the PodmanLens production library.

## Coverage and compatibility

The native-field and renderer catalogues link supported fields to their public access point,
planner, renderer, target versions, diagnostic, and focused tests. Catalogue validation includes
mutation tests so a plausible but incorrect target or owner swap fails.

Do not copy mutable catalogue row counts into prose. When a target boundary changes, update the
catalogue, its evidence, positive and negative tests, and the public compatibility guide together.
