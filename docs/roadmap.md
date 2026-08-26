# Roadmap

PodmanLens has a released, BoxFerry-ready baseline. It provides explicit read-only acquisition,
typed observations, deterministic discovery, version-aware Podman intent, ordered planning, and
non-executing CLI and Libpod renderings for the reviewed Podman 5.4 through 6.1 range.

## Completed baseline

- [x] Explicit connections and a replaceable acquisition transport
- [x] Six-kind native inventory with typed state and provenance
- [x] Evidence-backed resource discovery and network boundaries
- [x] Redacted observational snapshots
- [x] Ordered deployment semantics and deterministic review artifacts
- [x] Machine-readable native-field and renderer evidence
- [x] Complex offline coverage for every reviewed version in simulated rootless and rootful contexts
- [x] Public task guides and external-consumer examples
- [x] Finite legacy input-only anchors separate from reviewed output targets
- [x] Digest-pinned live conformance delegated to BoxFerry, covering 48 rootful and rootless
      container cells, every reviewed Podman 5.4 through 6.1 line, and the published distribution
      anchors through PodmanLens's production read-only transport

Completed implementation detail lives in the changelog, accepted decisions, catalogues, tests, and
Git history.

## Next

- [ ] Add one bounded native output field family at a time, with planner, CLI, Libpod, diagnostics,
      catalogue evidence, and positive and negative target tests in the same change.
- [ ] Review new Podman releases explicitly before extending the supported target range.

BoxFerry is the primary downstream live-conformance owner. Any future source-catalogue expansion
must add its digest-pinned rootful and rootless cells there before PodmanLens claims compatibility.

## Deferred

- Executing or applying deployment plans
- Docker and Kubernetes protocol support
- Persistent resource-group configuration
- Arbitrary native mutation
- Built-in secret encryption

These items require a new reviewed scope; they are not implied by the current library.
