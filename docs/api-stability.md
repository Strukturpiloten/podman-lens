# API stability policy

PodmanLens is pre-1.0. M0 deliberately exposed no premature runtime, protocol, or planning
contract. M1 publishes the explicit connection, redacted diagnostic, bounded Libpod transport
messages, GET-only Unix acquisition transport, version probe, target-profile, and evidence-catalogue
contracts. They are exercised by the external-consumer `public_api` integration test.

`ConnectionKind`, `ConnectionSpec`, and `LibpodMethod` are intentionally closed protocol and
control enums. Exhaustive matching makes every supported endpoint category and mutating-capability
boundary reviewable; adding one is a deliberate public API change. Evolving native-data enums,
such as `ResourceKind`, observation states, graph evidence, and diagnostics, are
`#[non_exhaustive]` so callers retain a forward-compatible fallback branch.

M2 introduced the read-only `acquire_inventory` boundary,
`AcquisitionOptions`, and redacted typed inventory records. Its wire decoder and Libpod JSON types
remain private. The inventory carries all six fixed sections, per-section availability, partial
records for non-atomic races, labels, relationships, source/version evidence, field-path and JSON
kind metadata for unsupported fields, and structured findings. `SensitiveEnvironmentValue` may be
used only through its callback accessor; it does not serialize or print its plaintext value.

M3 introduced `ResourceSelector`, `LabelSelector`, `DiscoveryRequest`,
`discover`, and deterministic `ResourceGraph` contracts. The graph exposes requested
selectors, the `all` choice, resolved roots with redacted origin positions, directed dependencies,
separate grouping evidence, `PLN0027`–`PLN0033` findings, and an explanation trace. Exact resource and network
boundary references accept a name or ID; they never accept patterns. Label selectors represent
exact key presence or an exact key-value pair, while their `Debug` forms redact values. Graph
explanations account for every included resource, stopped boundary, authorized crossing,
strong-evidence merge, and ordering decision. Public fields remain private and extension enums are
non-exhaustive.

M4 stabilizes `acquire_inventory`, `discover`, the documented inventory/graph accessors, and their
diagnostic/evidence values as the native input contract. Private Libpod decoder DTOs and response
shapes are not public API. `snapshot::v1` is a separate serialization-only schema contract with an
exact committed Draft 2020-12 schema. It has no deserialization API. An incompatible snapshot
shape requires a new versioned module and schema; compatible additions must still preserve the
always-redacted boundary recorded in ADR 0008. These contracts intentionally do not promise SSH or
TLS transport implementations or deployment plans.

M5 introduces provisional typed output semantics: `DeploymentIntent`, target-side
`DeploymentResourceId`, managed resource intents, `ExternalPrecondition`,
`SensitiveInputReference`, `StartupDependency`, `plan_deployment`, `PlanningOutcome`, and ordered
semantic operations. Every operation retains its exact typed managed-resource intent, while the
plan retains deterministic explicit external preconditions. They deliberately contain neither CLI
syntax nor Libpod HTTP DTOs. Planning
returns all sorted structured findings and no partial plan on an error. Exact output renderings,
serialized plan schemas, and shell artifacts remain M6 contracts and are not implied by M5.

Within a released `0.x.y` patch line, supported public APIs remain source compatible. A user-visible
break must use a breaking Conventional Commit title, be documented, and receive the appropriate
pre-1.0 minor release. Private Libpod wire types never become public compatibility commitments.

The integration test named `public_api` becomes the compile-time consumer contract when the first
public API is introduced. The ordinary local gate stays offline and validates that contract. Set
`PODMAN_LENS_SEMVER_CHECK=1` after the first publication to run `cargo-semver-checks` with its
isolated cache; hosted CI selects that comparison automatically once a release exists.
