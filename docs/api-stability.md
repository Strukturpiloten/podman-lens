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
`native_field_coverage_catalogue()` exposes the strict, packaged M2 native-field ledger. Its rows
name decoder, planner/renderer applicability, public access point, diagnostic, and focused tests.
`ResourceRecord::unknown_fields_complete()` prevents callers from treating bounded unknown metadata
as exhaustive after `PLN0021` or a partial inspection.

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

M6-A introduces `render_deployment`, `DeploymentRendering`, including its optional non-sensitive,
strictly validated Podman connection-name `connection`, typed CLI and Libpod invocation descriptions,
rendering findings, and `artifact::deployment_v1`. The deployment artifact is serialization-only,
uses a committed Draft 2020-12 schema, may contain only explicitly caller-authorized public declared
values, and never contains sensitive values or sensitive-input references. URI, endpoint, path,
credential, and token spellings cannot construct this connection type or reach its JSON schema. `shell_script`
derives solely from the stored CLI argument arrays and preserved external preconditions; neither it
nor any M6-A type opens a connection or executes Podman.
M6-B1a adds provisional typed settings to `ContainerIntent` and `PodIntent`: named-volume mounts
and explicitly named `PodIntent::add_infra_mount` infra-container mounts,
plus container command, entrypoint, user, workdir, hostname, labels, environment, and restart
policy. Sensitive inline environment values and external input references redact `Debug`. Public
declared values use `PublicLabelValue`, `PublicEnvironmentValue`, and
`DeploymentEnvironmentValue::Public`, making caller declassification explicit; no conversion from
observed sensitive values is provided. These
M6-B1b makes the bounded public subset exact across the reviewed releases: command follows the
image; entrypoint uses a JSON-array CLI flag; public labels and environment values retain declared
CLI order and become Libpod maps; volume JSON uses `Name`, `Dest`, and `Options`. The v5 catalogue
requires revision-pinned CLI, model, and handler evidence for every emitted field. Inline and
external environment variants remain all-or-nothing redacted `PLN0046` outcomes. Pod-member
hostnames fail planning and pod-member restart policies remain `PLN0046`; bind/tmpfs mounts, ports,
health checks, logging, security, namespace, and secret attachment targets remain later M6 contracts.

M6-B2 adds provisional typed networking output values: `NetworkAttachment`, `PortMapping`,
`DnsConfiguration`, `HostAlias`, `NetworkCidr`, `NetworkSubnet`, `NetworkRoute`, and the bounded
`PortProtocol`, `RouteType`, and `StaticMacAddress` values. `PodIntent` owns pod-network namespace
configuration; `ContainerIntent` owns it only when unpodded. The planner rejects member-owned
attachments, ports, DNS, host aliases, or network order. An explicit container network order must
be a permutation of all attached networks. M6-B2 does not claim runtime-assigned addresses,
arbitrary network drivers/options, or unmanaged network namespace modes. Routes retain an optional
native metric. Static attachment addresses require an explicitly rootful target context at planning
time; unknown and rootless contexts produce field-level findings. The v5 per-release Podman and
common-module evidence matrix makes this bounded subset exact. Container network order and
non-unicast route types are intentionally target-gated to Podman 6.0+; runtime-assigned addresses,
multi-IP attachments, port ranges, interface names, arbitrary driver/options, and unmanaged
namespace modes remain explicit later-ledger work.

M6-B3a adds provisional `ContainerRuntimeSettings` and its bounded health, logging, security, and
resource-control value types for every `ContainerIntent`. It separately exposes bounded namespace
intent only for an unpodded container; a pod member carrying namespace intent is rejected with
`PLN0038`, rather than having it silently reassigned to a pod. Sensitive shell and direct-exec
health forms remain redacted, cgroup support and root context are caller-provided evidence, and
every populated runtime setting blocks rendering with `PLN0046` until per-field renderer evidence
is committed. Semantic planning already rejects journald labels before Podman 6.0, unlimited
rlimits before 5.6, and non-positive or sub-millisecond CPU quota; B3b renderers must repeat
these target gates defensively.

Within a released `0.x.y` patch line, supported public APIs remain source compatible. A user-visible
break must use a breaking Conventional Commit title, be documented, and receive the appropriate
pre-1.0 minor release. Private Libpod wire types never become public compatibility commitments.

The integration test named `public_api` becomes the compile-time consumer contract when the first
public API is introduced. The ordinary local gate stays offline and validates that contract. Set
`PODMAN_LENS_SEMVER_CHECK=1` after the first publication to run `cargo-semver-checks` with its
isolated cache; hosted CI selects that comparison automatically once a release exists.
