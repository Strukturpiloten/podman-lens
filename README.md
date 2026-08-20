# PodmanLens

PodmanLens will provide typed, version-aware Podman inspection and deployment planning for Rust
applications. It is the native Podman boundary used by
[BoxFerry](https://github.com/Strukturpiloten/boxferry), but it will not depend on BoxFerry.

> [!NOTE]
> Read-only inventory acquisition and evidence-backed resource discovery are complete. Deployment
> planning is under active development, and the library is not ready for production use yet.

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
groups, explicit shared-network crossings, structured findings, and explanations for inclusion,
boundaries, merging, and ordering. Runtime environment values are redacted by default; explicit
inclusion uses a non-serializing opaque type. Secret payload endpoints are never requested. See the
roadmap for the stable input boundary and deployment work that remain.

The provisional M3 call flow is `acquire_inventory` → `DiscoveryRequest` → `discover_resources` →
`ResourceGraph`. Label roots use `LabelSelector::presence` or `LabelSelector::exact`. The returned
graph exposes requested and resolved roots, groups, prerequisites, findings, and explanations. M4
will stabilize this boundary and rename the discovery operation to its final public name.
