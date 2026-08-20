# PodmanLens

PodmanLens will provide typed, version-aware Podman inspection and deployment planning for Rust
applications. It is the native Podman boundary used by
[BoxFerry](https://github.com/Strukturpiloten/boxferry), but it will not depend on BoxFerry.

> [!NOTE]
> The stable native input contract and transport-neutral deployment semantics are complete. Exact
> command/API renderings and broad native output coverage remain under active development.

PodmanLens will:

- inspect containers, pods, networks, volumes, images, and secret metadata through the native
  Libpod REST API;
- build an evidence-backed resource graph from explicit selectors;
- preserve Podman-specific data and the Podman version that gives it meaning;
- produce ordered transport-neutral deployment semantics before M6 CLI and Libpod API rendering; and
- keep environment and secret material protected by default.

PodmanLens will not choose cross-format mappings, parse Compose or Quadlet, execute deployment
plans, or depend on BoxFerry.

## Project documents

- [Architecture](docs/architecture.md)
- [Library API](docs/library-api.md)
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

M6-B3a adds bounded semantic-only health, logging, security, CPU, memory, PID, and rlimit intent
for containers, including pod members. Namespace intent is intentionally narrower: it is only
valid on an unpodded container and a pod-member declaration is rejected. All runtime settings
remain deliberately unrendered until per-field version evidence exists; sensitive health commands
stay redacted. Semantic planning accepts journald labels from Podman 6.0 and unlimited rlimits
from 5.6; it accepts CPU quota only as a positive value of at least one millisecond.

The inventory's currently accepted native fields are declared in a strict machine-readable ledger.
Unmodeled data is retained only as bounded, redacted metadata; a partial record or overflow finding
means that metadata is explicitly incomplete rather than an exhaustive native configuration export.
