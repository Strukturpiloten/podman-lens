# API and schema stability

PodmanLens is pre-1.0. Its released public Rust API, serialized schemas, diagnostic codes, and
machine catalogues are compatibility contracts even though the project may still make deliberate
breaking changes in a new minor version.

## Rust API

Within one released `0.x` line, patch releases remain source compatible. A source-breaking public
API change requires:

1. a breaking Conventional Commit;
2. the next pre-1.0 minor version;
3. migration notes in `CHANGELOG.md`; and
4. a compatibility check against the published baseline.

Additive bounded APIs and compatible fixes may ship in a patch release.

Public control enums such as connection and transport-method categories are intentionally closed
when exhaustive matching is part of the safety boundary. Evolving native-data and diagnostic enums
are non-exhaustive so callers retain a fallback branch.

Private Libpod decoder and response types are implementation details. Changing them is not a public
Rust compatibility event.

## Observation and output contracts

The stable input boundary consists of acquisition, typed resource observations, discovery,
evidence, diagnostics, and their documented accessors. Field state and observation origin are part
of the contract; an unavailable or effective native fact cannot be collapsed into ordinary
absence.

Deployment intent, planning, and rendering are separate public boundaries. Acquired observations
do not implement an automatic conversion into target intent. Rendered output is available only
when the selected target and every populated field have reviewed evidence.

Public API examples are compiled as external consumers by `tests/public_api.rs`. The task guides
and examples are checked independently by `tests/public_guides.rs`.

## Serialized contracts

`snapshot::v1` and `artifact::deployment_v1` are serialization-only formats with committed
Draft 2020-12 schemas. They are not deserialization or trusted-input APIs.

An incompatible shape requires a new versioned module and schema. An existing schema version is
never silently rewritten. Exact golden files and negative schema mutations protect semantics that
the schema alone cannot express.

Snapshots remain always redacted. Deployment artifacts contain only public desired values
explicitly authorized by the caller and never sensitive input references.

## Version and catalogue changes

Podman support is field-specific and evidence-backed. A capability change must update:

- the relevant catalogue in `catalogue/v1/`;
- positive and negative target-boundary tests;
- Rustdoc or task guidance when user behavior changes; and
- a new versioned schema only when serialized shape changes.

Mutable catalogue row counts and completed implementation batches are not copied into current
prose. The catalogue is the source of truth.

## Compatibility checks

The complete gate runs the configured public API check:

```console
./scripts/check-all.sh
```

To compare explicitly with the published crate through `cargo-semver-checks`, run:

```console
PODMAN_LENS_SEMVER_CHECK=1 ./scripts/check-all.sh
```
