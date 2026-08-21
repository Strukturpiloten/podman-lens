# Project structure

The repository is a single Rust library crate. This remains intentional until independent modules
have a demonstrated public-contract boundary.

| Path                        | Responsibility                                                         |
| --------------------------- | ---------------------------------------------------------------------- |
| `src/`                      | Public library and private implementation modules                      |
| `src/observation.rs`        | Typed resource headers, field states, provenance, and protected values |
| `src/snapshot/`             | Versioned serialization-only always-redacted observation snapshots     |
| `src/artifact/`             | Versioned serialization-only deployment artifact contracts             |
| `src/deployment.rs`         | Typed deployment intent and ordered transport-neutral semantics        |
| `src/runtime.rs`            | Bounded redaction-safe container runtime and unpodded namespace intent |
| `src/networking.rs`         | Typed declared networking, IPAM, DNS, ports, and host aliases          |
| `src/render.rs`             | Review-only CLI and Libpod deployment representations                  |
| `catalogue/v1/`             | Versioned compatibility, rendering evidence, and native-field ledger   |
| `fixtures/corpus/`          | Sanitized fixed corpora and 14 request-aware complex offline cassettes |
| `fixtures/snapshots/`       | Exact versioned snapshot goldens                                       |
| `fixtures/deployment/`      | Byte-exact deployment JSON and POSIX-script goldens                    |
| `docs/schemas/`             | Public export schemas and strict test-only cassette schema             |
| `tests/support/cassette.rs` | Test-only cassette parsing, request matching, and deterministic replay |
| `tests/complex_corpus.rs`   | Per-version and simulated-context complex conformance cases            |
| `tests/`                    | Public-contract, fixture, and repository-policy integration tests      |
| `docs/`                     | Architecture, decisions, policy, and evidence documentation            |
| `scripts/`                  | Deterministic local validation helpers                                 |
| `.github/workflows/`        | CI, release preparation, and protected publication                     |

Future protocol, resource-model, discovery, and plan modules must follow the layers in the
[architecture](architecture.md), not a root-level convenience API.

The downstream mapping and release acceptance documents live beside the architecture because they
govern public consumption without introducing a BoxFerry crate dependency.

The cassette support module is not part of the crate's production API. Its strict v1 schema binds
each synthetic response to an expected Libpod method and path, and its 14 stable fixtures cover the
seven reviewed versions in simulated rootless and rootful contexts. They are source-derived,
sanitized offline inputs rather than exports of running Podman environments. Future live
conformance remains separate in
[GitHub issue #3](https://github.com/Strukturpiloten/podman-lens/issues/3).
