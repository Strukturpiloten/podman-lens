# Select a reviewed Podman target

PodmanLens never infers a target from the development machine. Input retains the observed Podman
engine and Libpod API versions; output requires a caller-selected `TargetProfile` and explicit
rootful, rootless, or unknown execution context.

The current immutable evidence catalogue has these exact reviewed anchors:

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

Target availability is field-specific. Examples include explicit image pull policies and volume
UID/GID ownership beginning at reviewed Podman 5.6, and non-unicast network route types, network
ordering, and journald label selection beginning at reviewed Podman 6.0. Root-dependent networking
and cgroup intent also requires explicit execution-context evidence.

## What the offline matrix proves

Fourteen committed request-aware cassettes cover every reviewed anchor in simulated rootful and
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
