# Project structure

The repository is a single Rust library crate. This remains intentional until independent modules
have a demonstrated public-contract boundary.

| Path                   | Responsibility                                                         |
| ---------------------- | ---------------------------------------------------------------------- |
| `src/`                 | Public library and private implementation modules                      |
| `src/observation.rs`   | Typed resource headers, field states, provenance, and protected values |
| `src/snapshot/`        | Versioned serialization-only always-redacted observation snapshots     |
| `src/artifact/`        | Versioned serialization-only deployment artifact contracts             |
| `src/deployment.rs`    | Typed deployment intent and ordered transport-neutral semantics        |
| `src/runtime.rs`       | Bounded redaction-safe container runtime and unpodded namespace intent |
| `src/networking.rs`    | Typed declared networking, IPAM, DNS, ports, and host aliases          |
| `src/render.rs`        | Review-only CLI and Libpod deployment representations                  |
| `catalogue/v1/`        | Versioned compatibility, rendering evidence, and native-field ledger   |
| `fixtures/corpus/`     | Fixed sanitized input and graph-boundary corpus                        |
| `fixtures/snapshots/`  | Exact versioned snapshot goldens                                       |
| `fixtures/deployment/` | Byte-exact deployment JSON and POSIX-script goldens                    |
| `docs/schemas/`        | Public JSON Schemas                                                    |
| `tests/`               | Public-contract, fixture, and repository-policy integration tests      |
| `docs/`                | Architecture, decisions, policy, and evidence documentation            |
| `scripts/`             | Deterministic local validation helpers                                 |
| `.github/workflows/`   | CI, release preparation, and protected publication                     |

Future protocol, resource-model, discovery, and plan modules must follow the layers in the
[architecture](architecture.md), not a root-level convenience API.
