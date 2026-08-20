# API stability policy

PodmanLens is pre-1.0. M0 deliberately exposed no premature runtime, protocol, or planning
contract. M1 publishes the explicit connection, redacted diagnostic, bounded Libpod transport
messages, GET-only Unix acquisition transport, version probe, target-profile, and evidence-catalogue
contracts. They are exercised by the external-consumer `public_api` integration test.

M2 additionally publishes the provisional read-only `acquire_inventory` boundary,
`AcquisitionOptions`, and redacted typed inventory records. Its wire decoder and Libpod JSON types
remain private. The inventory carries all six fixed sections, per-section availability, partial
records for non-atomic races, labels, relationships, source/version evidence, field-path and JSON
kind metadata for unsupported fields, and structured findings. `SensitiveEnvironmentValue` may be
used only through its callback accessor; it does not serialize or print its plaintext value.

These public contracts intentionally do not promise SSH or TLS transport implementations, resource
discovery graphs, or deployment plans. M4 will explicitly stabilize the native input contract after
its corpus and graph boundaries are complete.

Within a released `0.x.y` patch line, supported public APIs remain source compatible. A user-visible
break must use a breaking Conventional Commit title, be documented, and receive the appropriate
pre-1.0 minor release. Private Libpod wire types never become public compatibility commitments.

The integration test named `public_api` becomes the compile-time consumer contract when the first
public API is introduced. The ordinary local gate stays offline and validates that contract. Set
`PODMAN_LENS_SEMVER_CHECK=1` after the first publication to run `cargo-semver-checks` with its
isolated cache; hosted CI selects that comparison automatically once a release exists.
