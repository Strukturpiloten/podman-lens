# Project structure

PodmanLens is one Rust library crate. Files are organized by contract boundary rather than by
historical implementation batch.

| Path                                                     | Responsibility                                                              |
| -------------------------------------------------------- | --------------------------------------------------------------------------- |
| `src/connection.rs`, `src/transport.rs`                  | Explicit connections and replaceable bounded transport messages             |
| `src/read_only_unix_transport.rs`                        | Built-in GET-only Unix HTTP transport                                       |
| `src/probe.rs`, `src/version.rs`, `src/evidence.rs`      | Service probe, target profiles, and capability evidence                     |
| `src/inventory.rs`, `src/observation.rs`                 | Native acquisition and typed observation contracts                          |
| `src/discovery.rs`                                       | Roots, dependency closure, grouping, boundaries, and explanations           |
| `src/deployment.rs`                                      | Caller-authored intent and ordered semantic planning                        |
| `src/settings.rs`, `src/networking.rs`, `src/runtime.rs` | Typed output settings                                                       |
| `src/render.rs`                                          | Non-executing CLI and Libpod representations                                |
| `src/snapshot/`                                          | Versioned always-redacted observational exports                             |
| `src/artifact/`                                          | Versioned desired-output artifacts                                          |
| `catalogue/v1/`                                          | Machine-readable capabilities, native-field coverage, and renderer evidence |
| `docs/public/`                                           | Website-ready task guides imported at an exact repository revision          |
| `docs/schemas/`                                          | Public export schemas and test-only cassette schema                         |
| `examples/`                                              | Explicit acquisition and deterministic offline planning examples            |
| `fixtures/api-version/`                                  | Service-probe evidence                                                      |
| `fixtures/inventory/`                                    | Focused inventory and snapshot inputs                                       |
| `fixtures/corpus/`                                       | Complex and regression acquisition corpora                                  |
| `fixtures/deployment/`, `fixtures/snapshots/`            | Exact serialized goldens                                                    |
| `tests/support/cassette.rs`                              | Test-only strict request-aware replay                                       |
| `tests/`                                                 | Public, boundary, fixture, schema, rendering, and policy tests              |
| `scripts/`                                               | Deterministic validation and release helpers                                |

Protocol response shapes remain private to the acquisition implementation. Public native
observations belong in `observation.rs`; caller-authored target settings belong in the output
modules. Do not reuse one type across that boundary for convenience.

The BoxFerry website imports only `docs/public/` and generated Rustdoc. Internal maintainer
documents and fixture schemas are not website task pages.

Cross-format mapping documentation belongs with the downstream adapter and neutral model that own
the policy. PodmanLens documents the native contracts that such adapters consume.
