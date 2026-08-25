# Select a reviewed Podman target

PodmanLens never infers a target from the development machine. Input retains the observed Podman
engine and Libpod API versions; output requires a caller-selected `TargetProfile` and explicit
rootful, rootless, or unknown execution context.

The current immutable output catalogue has these reviewed anchors:

| Podman line | Reviewed engine/API evidence |
| ----------- | ---------------------------- |
| 5.4         | 5.4.0                        |
| 5.5         | 5.5.0                        |
| 5.6         | 5.6.0                        |
| 5.7         | 5.7.0                        |
| 5.8         | 5.8.6                        |
| 6.0         | 6.0.0                        |
| 6.1         | 6.1.0                        |

Deterministic rendering requires semantically identical engine and API versions present in the
renderer evidence. A new Podman release does not become supported merely because its version
parses; it needs source evidence plus positive and negative boundary tests.

## Input-only migration anchors

The following exact source runtimes can be acquired and discovered, but cannot be selected as a
PodmanLens deployment target. They exist to migrate an older host through a neutral model to a
separately chosen modern target.

| Runtime | Distribution package families |
| ------- | ----------------------------- |
| 3.0.1   | Debian 11                     |
| 3.4.4   | Ubuntu 22.04                  |
| 4.3.1   | Debian 12                     |
| 4.9.3   | Ubuntu 24.04                  |
| 4.9.4   | Red Hat UBI 8 (`4.9.4-rhel`)  |

These are finite source-backed upstream API anchors, not a claim for every Podman 3.x or 4.x patch
or for distribution runtime behavior. Input acquisition records the exact engine and Libpod API
evidence. In Podman 3.0.1, secret metadata endpoints do not exist; its secret section is reported
as version-inapplicable without requesting it. Rootful and rootless distribution-image validation
will be recorded only after the live matrix succeeds.

The UBI 8 package reports `4.9.4-rhel` on the wire. PodmanLens retains that exact observation while
matching only its reviewed `4.9.4` semantic core; other prerelease-style vendor spellings remain
invalid.

Ubuntu 22.04 Podman 3.4 returns its raw CNI network document as an object even though the remote
CLI displays an array. PodmanLens accepts that proven endpoint shape without widening later
object-shaped network contracts or ambiguous multi-configuration arrays.

Target availability is field-specific. Examples include explicit image pull policies and volume
UID/GID ownership beginning at reviewed Podman 5.6, and non-unicast network route types, network
ordering, and journald label selection beginning at reviewed Podman 6.0. Root-dependent networking
and cgroup intent also requires explicit execution-context evidence.

## What the offline matrix proves

Fourteen committed request-aware cassettes cover every output anchor in simulated rootful and
rootless contexts. They exercise all six resource kinds, complex pods and standalone containers,
isolated and intentionally shared networks, volumes, dependencies, version-bound fields, request
matching, diagnostics, and redaction. Separate focused response fixtures exercise unavailable,
disappeared, and malformed observations. Deployment catalogues and renderer tests prove target
gates independently of observed-inventory cassettes. These source-derived, synthetic, sanitized
fixtures are not exports of live Podman environments.

Live rootful/rootless conformance remains deferred in
[GitHub issue #3](https://github.com/Strukturpiloten/podman-lens/issues/3) until all 14 reproducible,
digest-pinned environments exist. That future workflow is manual-only, has no nightly or
pull-request trigger, and may not skip unavailable cells.

See the packaged `catalogue/v1/podman-capabilities.json`,
`catalogue/v1/podman-deployment-rendering.json`, and
`catalogue/v1/native-field-coverage.json` for machine-readable evidence.
