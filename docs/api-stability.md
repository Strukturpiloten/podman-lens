# API stability policy

PodmanLens is pre-1.0. M0 deliberately exposed no premature runtime, protocol, or planning
contract. M1 slice A now publishes the explicit connection, redacted diagnostic, bounded Libpod
transport-message, version, target-profile, and evidence-catalogue contracts. They are exercised
by the external-consumer `public_api` integration test.

These public contracts intentionally do not promise a transport implementation, API probe decoder,
resource inventory, discovery graph, or deployment plan. Those APIs remain private or absent until
their respective evidence and positive and negative tests are ready.

Within a released `0.x.y` patch line, supported public APIs remain source compatible. A user-visible
break must use a breaking Conventional Commit title, be documented, and receive the appropriate
pre-1.0 minor release. Private Libpod wire types never become public compatibility commitments.

The integration test named `public_api` becomes the compile-time consumer contract when the first
public API is introduced. The ordinary local gate stays offline and validates that contract. Set
`PODMAN_LENS_SEMVER_CHECK=1` after the first publication to run `cargo-semver-checks` with its
isolated cache; hosted CI selects that comparison automatically once a release exists.
