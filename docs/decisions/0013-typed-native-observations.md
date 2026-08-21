# 0013: Typed native observations preserve state and provenance

- Status: Accepted
- Date: 2026-08-20

## Context

The initial inventory API grouped every resource behind a generic record with generic
relationships and unknown-field metadata. That made it too easy for a caller to mistake a
missing, malformed, unavailable, runtime-assigned, or locally resolved fact for deployable
configuration. It also flattened the configured container image spelling and the image identity
resolved by the local Podman service.

BoxFerry needs a public, type-safe input boundary that reports precisely what was observed without
depending on Libpod response DTOs. PodmanLens is unreleased, so preserving an obsolete projection
or an artificial compatibility layer would make that boundary less clear rather than safer.

## Decision

`ResourceInventory` exposes `ResourceObservation` values. Each observation has an
`ObservationHeader` for identity, evidence, findings, bounded unmodelled metadata, and resource
acquisition state, plus a kind-safe `ResourceDetails` variant.

Every modeled field uses `ObservationField<T>`. Its distinct states are absent, observed,
unavailable, malformed, version-inapplicable, not-applicable, and unmodelled. An observed value
retains `ObservationOrigin`: configured, effective, runtime-assigned, or local-resolution. A
malformed label map marks only that resource's label field malformed and emits `PLN0017`; it never
makes unrelated resource fields or relationships unavailable.
Callers must inspect both the field state and origin before mapping a fact into desired intent.

Container `ImageName` is retained as the configured image spelling. Container `Image` is retained
separately as a local-resolution image identity. Discovery may derive an image dependency only
from configured image evidence; local resolution never becomes a desired-image edge. Protected
environment values remain redacted or explicitly authorized opaque values, and secret payload
bytes remain unrepresentable.

Image repository tags and repository digests remain separate local-resolution collections; neither
is authored deployment intent. Image digest, creation time, architecture, operating system, and
manifest type are effective evidence, while author is configured metadata. Volume driver,
creation time, and anonymous status are effective evidence. Secret creation/update times and its
driver object are effective metadata, but driver option names and values are discarded immediately;
only option state, provenance, and count remain observable.

Native image, volume, and secret timestamps use `NativeTimestamp`. It validates RFC 3339 while
preserving the exact wire spelling, including offset and fractional precision, so observation does
not silently normalize evidence before a caller maps it.

Volume UID and GID are effective native observations, never configured intent. Podman's reviewed
`omitempty` response shape can omit either field while still applying an effective default of zero;
that exceptional wire absence is represented as an observed `WireAbsentMayMeanZero` value with
effective provenance and an explicit ambiguity finding, rather than ordinary `Absent`.

Unmodelled metadata uses a closed semantic identifier enum and a bounded completeness state. It retains
path, JSON kind, resource identity, and immutable evidence but never raw JSON. A partial,
malformed, or overflowed observation is explicitly incomplete.

A native container secret grant carrying both `ID` and `Name` is one relationship with two source
locations. Both spellings must resolve uniquely to the same secret before discovery may traverse
it. A mismatch, unresolved spelling, or ambiguity is a structured non-traversal finding. Container
`ImageName` and `Image` are independently observed; when both uniquely resolve to different local
image resources, `PLN0020` reports the conflict while discovery retains only the configured-image
edge.

`snapshot::v1` remains the same serialization-only module because PodmanLens has not been
released. Its schema and goldens are reset in place to represent the typed observation header and
redacted detail summaries. No deprecated aliases or legacy record projections are retained.

## Consequences

- BoxFerry adapters can distinguish configuration from runtime facts without parsing Libpod JSON.
- A decoder error cannot silently become an empty or deployable field value.
- Native ID/name aliases cannot accidentally become duplicate graph edges or cause one spelling
  to win over contradictory evidence.
- Local image repository references cannot be mistaken for authored image intent, and exact native
  timestamp spelling remains available for evidence and diagnostics.
- Secret driver option names and values cannot escape through public observation values, debug
  output, or snapshots.
- Snapshots retain structural state and provenance while continuing to redact protected values.
- The coverage ledger and focused tests must name typed observation owners, rather than generic
  record accessors, as the API reaches its first release.
