# PodmanLens

PodmanLens will provide typed, version-aware Podman inspection and deployment planning for Rust
applications. It is the native Podman boundary used by
[BoxFerry](https://github.com/Strukturpiloten/boxferry), but it will not depend on BoxFerry.

> [!NOTE]
> The repository foundation is complete. Native Podman functionality is under active development
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
- [Roadmap](docs/roadmap.md)
- [Accepted decisions](docs/decisions/README.md)

## Status

M0 repository and Rust-library scaffolding is complete. M1 establishes version evidence and the
replaceable Libpod transport. See the roadmap for the planned implementation order and acceptance
criteria.
