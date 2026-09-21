# Dependency policy

Every dependency needs one narrow, reviewed role. PodmanLens does not add a general Podman client,
process executor, ambient connection resolver, or cross-format model through dependencies.

| Role                                    | Dependencies                                              |
| --------------------------------------- | --------------------------------------------------------- |
| Version parsing                         | `semver`                                                  |
| Bounded JSON decoding and serialization | `serde`, `serde_json`                                     |
| Explicit endpoint validation            | `url`                                                     |
| Read-only Unix HTTP transport           | `bytes`, `tokio`, `hyper`, `hyper-util`, `http-body-util` |
| Local schema tests only                 | `jsonschema` with retrieval features disabled             |
| Rust 1.85 test graph compatibility      | exact `yoke-derive` 0.8.2 temporary dev-only constraint   |

The built-in transport accepts only bodyless `GET` requests over one explicit Unix socket. No
dependency provides redirects, decompression, retries, SSH, TLS, process execution, or plan
application.

`jsonschema` is test-only and validates committed local schemas and fixtures. It does not shape
runtime types or retrieve remote schemas.

The direct, dev-only `yoke-derive = "=0.8.2"` constraint keeps `jsonschema`'s ICU derive graph
compatible with the declared Rust 1.85.0 MSRV. Version 0.8.3 is not compatible with that compiler;
remove this temporary constraint only after an upstream `yoke-derive` release restores Rust 1.85
support and the refreshed lockfile passes the MSRV gate. This explicit constraint prevents ordinary
Renovate lock-file maintenance from silently selecting the incompatible transitive release. Renovate
also excludes the known-incompatible 0.8.3 direct update; remove that exclusion with the constraint
only after verifying an upstream replacement on Rust 1.85.

Dependencies are locked in `Cargo.lock`, audited by `cargo deny`, and updated through Renovate.
Git dependencies and unknown registries are denied. A dependency that changes public types, wire
decoding, serialization, the async runtime, or the security boundary requires an ADR.
