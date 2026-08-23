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

The built-in transport accepts only bodyless `GET` requests over one explicit Unix socket. No
dependency provides redirects, decompression, retries, SSH, TLS, process execution, or plan
application.

`jsonschema` is test-only and validates committed local schemas and fixtures. It does not shape
runtime types or retrieve remote schemas.

Dependencies are locked in `Cargo.lock`, audited by `cargo deny`, and updated through Renovate.
Git dependencies and unknown registries are denied. A dependency that changes public types, wire
decoding, serialization, the async runtime, or the security boundary requires an ADR.
