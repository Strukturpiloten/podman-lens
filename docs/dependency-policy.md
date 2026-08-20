# Dependency policy

PodmanLens started dependency-free. M1 adds only reviewed, maintained dependencies with a specific
contract role: `semver` validates Podman and Libpod versions, `serde` and `serde_json` decode the
published offline evidence catalogue and bounded version response, `url` validates explicit
endpoint spellings, and `bytes`, `tokio`, `hyper`, `hyper-util`, and `http-body-util` implement the
single built-in HTTP/1.1 Unix transport. That transport is read-only and accepts only `GET` before
opening its explicit socket. No SSH, TLS, process, redirect, decompression, retry, or plan-execution
client is selected by this library.

Cargo dependencies are locked in `Cargo.lock`, audited by `cargo deny`, and updated through
Renovate. Git dependencies and unknown registries are denied. Any dependency that shapes public
types, wire decoding, serialization, async runtime choice, or security posture requires an ADR.
