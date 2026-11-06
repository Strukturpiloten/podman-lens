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

Snapshots remain always redacted. `artifact::deployment_v1` remains the public-only serialized
contract: it fails before writing bytes if a rendering contains protected inline environment
values. `artifact::deployment_v2` carries an explicit
`contains_protected_inline_environment` indicator and may include these values only after a
render-time `SensitiveInlineRenderAuthorization`. The ordinary `render_deployment` path still
rejects them. External sensitive input references never enter either artifact version. `Debug`
of renderings and artifacts stays redacted; v2 serialized bytes require sensitive handling when
the indicator is true.
The frozen `snapshot::v1` shape omits native network attachment aliases entirely, including their
state and count. Their exact spelling, field path, and runtime-ID classification are available only
from the typed observation API, where downstream mapping remains an explicit authorization.

## Version and catalogue changes

Podman support is field-specific and evidence-backed. A capability change must update:

- the relevant catalogue in `catalogue/v1/`;
- positive and negative target-boundary tests;
- Rustdoc or task guidance when user behavior changes; and
- a new versioned schema only when serialized shape changes.

Mutable catalogue row counts and completed implementation batches are not copied into current
prose. The catalogue is the source of truth.

## Compatibility checks

The complete gate runs the positive public API contract and a development-purpose comparison
against a trusted Git base:

```console
./scripts/check-all.sh
```

The same development check can run separately with `scripts/check-public-api.sh`. It uses the
shared baseline selector and its fixed minor allowance for compatible additions. It compares the
whole proposal with the unique fork base against canonical default main; clean trusted-main
candidates compare with their first parent. Dirty local main compares actual changed source with
trusted HEAD. Historical breaking markers and manifest version increases do not authorize a new
break. Missing or stale Git authority fails with fetch/retry guidance without changing local refs.

To compare explicitly with the canonical published crate, run:

```console
./scripts/check-public-api.sh --published
```

Release and validation-only always select published purpose. Compatibility is inferred from the
candidate manifest version, without a Conventional Commit intent override. Only successful
published-purpose evidence bound to the exact candidate can satisfy the release guard. The
legacy `PODMAN_LENS_SEMVER_CHECK` variable no longer skips or selects API validation.

The additive bounded
[`ContainerObservation::creation_evidence` contract](decisions/0015-bounded-creation-command-evidence.md)
exposes independent image-spelling and mount-relabel field states and does not change the immutable
redacted `snapshot::v1` schema.

## Repository test formats

The request-aware cassette schema and captured-native capture manifest are repository test formats. They are not public Rust APIs, trusted deserialization APIs, migration interchange formats, or compatibility promises. They may be included in the source package for auditability, but production modules do not import them. Their version and shape may change with the release test harness while the public observation and serialization contracts above remain stable.
