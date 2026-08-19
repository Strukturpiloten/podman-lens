# Dependency policy

PodmanLens starts dependency-free. Add a dependency only when it has a specific architectural role,
an acceptable maintained license, and a reviewable update path. Prefer Rust standard-library types
for contracts until concrete protocol work requires otherwise.

Cargo dependencies are locked in `Cargo.lock`, audited by `cargo deny`, and updated through
Renovate. Git dependencies and unknown registries are denied. Any dependency that shapes public
types, wire decoding, serialization, async runtime choice, or security posture requires an ADR.
