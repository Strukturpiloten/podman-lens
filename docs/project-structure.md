# Project structure

The repository is a single Rust library crate. This remains intentional until independent modules
have a demonstrated public-contract boundary.

| Path                 | Responsibility                                                    |
| -------------------- | ----------------------------------------------------------------- |
| `src/`               | Public library and private implementation modules                 |
| `catalogue/v1/`      | Versioned offline Podman compatibility evidence                   |
| `tests/`             | Public-contract, fixture, and repository-policy integration tests |
| `docs/`              | Architecture, decisions, policy, and evidence documentation       |
| `scripts/`           | Deterministic local validation helpers                            |
| `.github/workflows/` | CI, release preparation, and protected publication                |

Future protocol, resource-model, discovery, and plan modules must follow the layers in the
[architecture](architecture.md), not a root-level convenience API.
