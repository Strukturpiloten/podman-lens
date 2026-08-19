# Dependency policy

PodmanLens started dependency-free. M1 slice A adds only reviewed, maintained dependencies with a
specific contract role: `semver` validates Podman and Libpod versions, `serde` and `serde_json`
decode the published offline evidence catalogue, and `url` validates explicit endpoint spellings.
No async runtime, HTTP, Unix-socket, SSH, or TLS client is selected by this library.

Cargo dependencies are locked in `Cargo.lock`, audited by `cargo deny`, and updated through
Renovate. Git dependencies and unknown registries are denied. Any dependency that shapes public
types, wire decoding, serialization, async runtime choice, or security posture requires an ADR.
