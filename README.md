# PodmanLens

PodmanLens provides typed, version-aware Podman inspection and deployment planning for Rust
applications. It is the native Podman boundary used by
[BoxFerry](https://github.com/Strukturpiloten/boxferry), but it does not depend on BoxFerry.

> [!NOTE]
> The stable native input contract, transport-neutral deployment semantics, bounded M6-B1–B4
> CLI/API renderings, M7 BoxFerry integration-readiness contract, and post-0.1 deterministic
> complex-cassette layer are complete. Broader native output coverage and live Podman conformance
> remain explicitly deferred.

PodmanLens:

- inspects containers, pods, networks, volumes, images, and secret metadata through the native
  Libpod REST API;
- builds an evidence-backed resource graph from explicit selectors;
- preserves Podman-specific data and the Podman version that gives it meaning;
- produces ordered transport-neutral deployment semantics before CLI and Libpod API rendering; and
- keeps environment and secret material protected by default.

PodmanLens does not choose cross-format mappings, parse Compose or Quadlet, execute deployment
plans, or depend on BoxFerry.

## Project documents

- [Task-oriented public guides](docs/public/index.md)
- [Architecture](docs/architecture.md)
- [Library API](docs/library-api.md)
- [BoxFerry integration contract](docs/boxferry-integration.md)
- [Compatibility matrices](docs/compatibility.md)
- [First-release readiness](docs/release-readiness.md)
- [Roadmap](docs/roadmap.md)
- [Accepted decisions](docs/decisions/README.md)

## Status

M2 provides read-only acquisition of containers, pods, networks, named volumes, images, and secret
metadata. M3 adds exact resource and label roots, deterministic dependency closure, evidence-backed
groups, explicit shared-network crossings, structured findings, and explanations. M4 stabilizes
that input boundary and adds strict, serialization-only `snapshot::v1` exports plus fixed rootless,
rootful, malformed, and graph-boundary corpora.

The stable input call flow is `acquire_inventory` → `DiscoveryRequest` → `discover` →
`ResourceGraph`. Label roots use `LabelSelector::presence` or `LabelSelector::exact`. The returned
graph exposes requested and resolved roots, groups, prerequisites, findings, and explanations.
Runtime environment values are redacted by default; secret payload endpoints are never requested,
and snapshots redact protected values even when a caller included environment values in memory.

M5 adds `DeploymentIntent` → `plan_deployment` → `PlanningOutcome`. It uses target-side resource
identities, explicit managed resources or external network/volume/image/secret preconditions,
portable host-qualified managed image sources, external secret-material references, and deterministic
create/start semantics. A pod with members gets one `StartPod`; unpodded containers get
`StartContainer`. The plan contains no shell, HTTP, environment, or secret-payload representation;
M6 owns those renderings.

M6 completes the ledger-backed B1–B4 output surface. It renders the reviewed CLI and Libpod forms,
versioned deployment artifact, and shell review script without executing them. The bounded surface
covers settings, networking, runtime controls, typed mounts and secret grants, volume ownership,
and explicit image policies. Sensitive environment and secret payload values remain redacted or
external; unsupported source portability and target-version boundaries return structured findings.

The inventory's currently accepted native fields are declared in a strict machine-readable ledger.
Each resource is a typed observation whose fields preserve absence, malformed/unavailable state,
and configured/effective/runtime/local-resolution provenance. Unmodeled data is retained only as
bounded, redacted metadata; an incomplete observation or overflow finding means that metadata is
explicitly incomplete rather than an exhaustive native configuration export.

M7 adds the exact origin-gated PodmanLens-to-BoxFerry mapping contract, public compatibility
matrices, a pinned bounded all-six-resource Podman 6.1 corpus, and a public-only downstream
scenario from acquisition through CLI and Libpod rendering. Version 0.1.0 is the
maintainer-controlled first-release semver baseline.

Post-0.1 M8 conformance hardening adds a strict request-aware cassette contract and 14 complex
offline scenarios: Podman 5.4.0, 5.5.0, 5.6.0, 5.7.0, 5.8.6, 6.0.0, and 6.1.0, each with simulated
rootless and rootful context. Every cassette binds each response to its expected Libpod method and
path and contains all six resource kinds with interacting pods, containers, networks, volumes,
images, and secret metadata. These fixtures are source-derived, synthetic, and sanitized; they are
not exports of live Podman environments. Together with seven retained focused regression and
golden artifacts, the corpus manifest hash-verifies 21 artifacts. The complete live matrix remains deferred to
[GitHub issue #3](https://github.com/Strukturpiloten/podman-lens/issues/3), with no nightly or
pull-request workflow claimed here.

The website-ready public guides cover explicit read-only acquisition, discovery, grouping and
network boundaries, non-executing planning and rendering, diagnostics and privacy, and exact
version selection. Their examples compile or run through the ordinary repository test gate.
