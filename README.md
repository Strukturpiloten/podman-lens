# PodmanLens

PodmanLens will provide typed, version-aware Podman inspection and deployment planning for Rust
applications. It is the native Podman boundary used by
[BoxFerry](https://github.com/Strukturpiloten/boxferry), but it will not depend on BoxFerry.

> [!NOTE]
> The stable native input contract is complete. Deployment planning is under active development,
> and the library is not ready for production use yet.

PodmanLens will:

- inspect containers, pods, networks, volumes, images, and secret metadata through the native
  Libpod REST API;
- build an evidence-backed resource graph from explicit selectors;
- preserve Podman-specific data and the Podman version that gives it meaning;
- produce ordered deployment operations with CLI and Libpod API representations; and
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
