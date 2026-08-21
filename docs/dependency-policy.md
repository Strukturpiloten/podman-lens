# Dependency policy

PodmanLens started dependency-free. M1 adds only reviewed, maintained dependencies with a specific
contract role: `semver` validates Podman and Libpod versions, `serde` and `serde_json` decode the
published offline evidence catalogue and bounded version response, `url` validates explicit
endpoint spellings, and `bytes`, `tokio`, `hyper`, `hyper-util`, and `http-body-util` implement the
single built-in HTTP/1.1 Unix transport. That transport is read-only and accepts only `GET` before
opening its explicit socket. No SSH, TLS, process, redirect, decompression, retry, or plan-execution
client is selected by this library.

M4 adds `jsonschema` as a test-only Draft 2020-12 validator for the public snapshot schema and
goldens. Version 0.49.9 supports the repository's Rust 1.85 MSRV. Default features are disabled so
schema validation does not add file or network retrieval; tests validate only the committed local
schema and fixtures. It does not shape runtime or public Rust types.

M6-A uses the existing runtime `serde_json` dependency for deterministic, serialization-only
deployment-plan exports. Rendering itself adds no process, HTTP-client, shell, or execution
dependency: it produces data and text that callers may review, but never runs Podman.

Cargo dependencies are locked in `Cargo.lock`, audited by `cargo deny`, and updated through
Renovate. Git dependencies and unknown registries are denied. Any dependency that shapes public
types, wire decoding, serialization, async runtime choice, or security posture requires an ADR.
