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

M1 provides explicit redacted connection specifications, a replaceable asynchronous Libpod
transport contract, a built-in read-only Unix transport, an offline Podman 5.4–6.1 compatibility
catalogue, and deterministic version probing. The built-in transport accepts only `GET` requests;
the public probe itself performs two fixed `GET` requests. SSH and mutual-TLS transports remain
caller-provided. See the roadmap for the next native inventory work.
