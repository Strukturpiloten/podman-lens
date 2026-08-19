# API stability policy

PodmanLens is pre-1.0. The M0 crate deliberately exposes no premature runtime, protocol, or
planning contract. Public types begin only when their versioned evidence, diagnostics, and positive
and negative tests are ready.

Within a released `0.x.y` patch line, supported public APIs remain source compatible. A user-visible
break must use a breaking Conventional Commit title, be documented, and receive the appropriate
pre-1.0 minor release. Private Libpod wire types never become public compatibility commitments.

The integration test named `public_api` becomes the compile-time consumer contract when the first
public API is introduced. The ordinary local gate stays offline and validates that contract. Set
`PODMAN_LENS_SEMVER_CHECK=1` after the first publication to run `cargo-semver-checks` with its
isolated cache; hosted CI selects that comparison automatically once a release exists.
