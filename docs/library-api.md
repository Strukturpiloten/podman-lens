# Library API

The native input API is stable from M4. It is exercised as an external consumer and by fixed
rootless, rootful, malformed, and graph-boundary corpora.

```text
acquire_inventory → DiscoveryRequest → discover → ResourceGraph
```

## Acquire an inventory

`acquire_inventory` reads an explicitly selected `LibpodTransport`. It probes the service, lists
containers, pods, networks, volumes, images, and secret metadata, then inspects stable resource IDs.
The result preserves partial and malformed observations as evidence or structured findings.

## Inspect coverage

`native_field_coverage_catalogue()` returns a packaged strict two-plane ledger. An
`InputObservation` row links an accepted native field to its observation owner and has no output
renderers. An `OutputIntent` row links a declared field to its planner, exact CLI renderer, exact
Libpod renderer, reviewed target versions, public accessor, diagnostic rule, and focused tests.
`not_applicable` means no contract exists in that plane; it never authorizes a conversion. The B3b
runtime rows name all seven reviewed targets except journald labels (6.0+) and unlimited rlimits
(5.6+). The 16 B4 rows include exact typed mount and secret-grant forms, 5.6+ volume ownership and
all four explicit image pull policies, and manual no-copy-subpath, source-portability, and
pod-infra-mount boundaries. Sensitive and external health commands are manual redacted boundaries
that apply to all reviewed targets and block the complete resource artifact with `PLN0046`.

`ResourceInventory::observations()` returns deterministic `ResourceObservation` values. Each has
an `ObservationHeader` and a kind-safe `ResourceDetails` variant. A modeled field is always an
`ObservationField<T>`: absent, observed, unavailable, malformed, version-inapplicable,
not-applicable, or unmodelled. An observed value also records whether it was configured,
effective, runtime-assigned, or locally resolved. Adapters must not promote an effective,
runtime-assigned, or local-resolution value to declared deployment intent without an explicit
mapping policy.

`ObservationHeader::unmodelled_fields()` retains bounded metadata without raw values. Check
`ObservationHeader::unmodelled_completeness()` before treating it as exhaustive: it is incomplete
for unavailable or malformed observations and after `PLN0021` unknown-field overflow.
`HostConfig.MemorySwappiness` is typed, while any other direct `HostConfig` member is explicitly
unsupported metadata. `Secret.Spec.Driver` is typed metadata; secret payload material is discarded
and reported as `PLN0018`.

For a container, `configured_image()` reflects Libpod `ImageName`; `local_image_id()` reflects the
local `Image` resolver result. Discovery derives an image dependency only from the configured
spelling. Environment observations provide names plus redacted or authorized-opaque value states;
they never expose a deployment value.

M7-B1 adds observation-only core configuration, topology, mount, and secret-grant accessors.
`command`, `entrypoint`, `user`, `working_directory`, and `hostname` retain configured evidence.
`pod_membership` and `native_dependencies` retain native references without deciding a target pod
layout. `mounts` accepts only named-volume and bind forms: backing and bind paths are always
local-resolution evidence. `secret_grants` retains coalesced ID/name references plus effective
direct UID, GID, and mode metadata. Podman inspect does not expose a delivery form or target, so
mounted-versus-environment output semantics cannot be reconstructed. It neither requests nor
represents secret payload material. A malformed or contradictory member
marks the complete field malformed and cannot create a discovery edge.

M7-B2a adds observation-only `NetworkObservation::subnets` and `routes`. These are native
`NativeNetwork*` observation types, deliberately separate from output `NetworkIntent` types.
Subnet CIDRs, optional gateways, and lease ranges retain effective evidence. A present lease-range
object has independently optional `start_ip` and `end_ip` fields; each preserves its own
observation state. PodmanLens defensively rejects an endpoint outside its subnet and a complete
reversed range, even though ordering is not claimed as a native Podman validation rule.
Network-subnet CIDR spelling is retained exactly, including a reviewed host-bit spelling, while
containment normalizes network bits. Exact upstream response normalization across the reviewed
Podman versions is not yet claimed. Generic CIDR parsing preserves valid wire syntax defensively;
it does not claim that host-bit route destinations are native-valid, because pinned Podman 5.4
evidence rejects them. Routes retain destination, optional metric (including explicit zero),
gateway, and route type. A unicast route requires a gateway; blackhole, unreachable, and prohibit
routes must not carry one. Route type is version-inapplicable before Podman 6.0; from 6.0 an
omitted member is effective `unicast`. Snapshots retain only state, origin, and counts.

M7-B2b adds `PodObservation::create_infra`, pod and container `networking`, and
`NativeNetworkingObservation` accessors for bounded port, DNS, namespace, host-management,
network-reference, and deprecated static-address evidence. Unpodded `CreateNetNS` remains a
separate configured observation; callers must not infer `host_network` by negating it. Pod
`HostNetwork` is effective infra evidence. Runtime `NetworkSettings`, assigned addresses, raw
host entries, and option values are not deployment intent.

M7-B3a adds `restart_policy`, `health_check`, `health_failure_action`, `startup_health_check`, and
`logging` accessors to `ContainerObservation`. These are effective inspect observations, not a
conversion to `ContainerIntent`: health may be supplied by the image, and Podman can normalize
restart/logging settings. `NativeHealthCommand` distinguishes disabled, shell, and exec syntax;
its protected arguments are accessible only through `ProtectedHealthCommand::expose`, never debug,
display, or snapshots. Only `["NONE"]` disables health. `Retries`, startup `Successes`, and restart
`MaximumRetryCount` are unsigned; a negative wire value is field-local malformed evidence.
Snapshots expose field state/origin and command argument count, never command, log-size, or other
configuration spelling.

M7-B3b adds `security`, `namespaces`, and `resource_controls` accessors to
`ContainerObservation`. Their input-only native types remain separate from
`ContainerRuntimeSettings`. Capabilities preserve order and duplicates. `SecurityOpt` exposes
only a count, namespace modes expose only the bounded private/host subset plus IPC
shareable/none, and future syntactically valid modes remain unmodelled.
Empty PID, IPC, and UTS mode spellings are the reviewed private defaults; an empty cgroup mode is
malformed.
Resource observations preserve native signed sentinels and ulimit order without enforcing deployment constraints.
Snapshots contain only states, origins, and collection counts.

## Select roots

Add exact resource roots with `ResourceSelector::exact`. A reference is one exact resource name,
ID, or image alias; patterns are rejected. Add label roots with `LabelSelector::presence` or
`LabelSelector::exact`. An empty exact label value is valid and differs from presence-only matching.
`DiscoveryRequest::select_all` selects only eligible application roots, not every cached image.

Discovery follows dependency closure by default. Selecting a shared network, volume, image, or
secret explicitly crosses to its direct consumers. A network boundary can also be crossed by one
exact name-or-ID authorization. There is no grouping configuration file and no wildcard crossing.

## Read the graph

`ResourceGraph` exposes:

- requested resource and label roots;
- the `all` choice and resolved root identities with redacted selector-origin positions;
- deterministic resource groups and their prerequisites;
- shared prerequisites;
- directed dependent-to-prerequisite edges;
- separate non-directed grouping evidence;
- structured findings; and
- explanations for roots, inclusions, prerequisites, stopped boundaries, authorized crossings,
  strong-evidence merges, and group ordering.

Groups merge through pod membership, native container dependencies, or complete matching
Docker/Podman Compose ownership evidence. Merely sharing a network, volume, image, or secret never
merges groups. `network.internal` describes connectivity only.

## Determinism and sensitive values

All collections use deterministic ordering. `ResourceKind::canonical_rank` fixes resource and
group ordering as container, pod, network, volume, image, then secret; a group ID is its smallest
`(kind, id)` member under that order. Debug output redacts label values and Compose ownership
values. Environment values and secret payloads must not appear in diagnostics, debug output, or
versioned snapshots.

## Export a redacted snapshot

`snapshot::v1::inventory` and `snapshot::v1::graph` create serialization-only reports with
`schema_version: 1`. They implement `Serialize`, not `Deserialize`, and conform to the committed
Draft 2020-12 schema. Snapshot creation always removes environment values, secret payloads,
connection data, raw unknown JSON, label values, driver-option values, and Compose ownership
values, regardless of in-memory acquisition policy.

Snapshots retain resource IDs and names, environment variable names, evidence URLs, and source
field paths. Image aliases and network subnets are deliberately exported as counts only, because
their spellings may disclose private registry, topology, or addressing information. Always-redacted
does not mean anonymous; callers must still handle reports as operational data.

## Plan native deployment semantics

M6-B4 replaces the original named-volume-only and raw secret surfaces with `MountIntent`
(named volume, bind, tmpfs), `VolumeSubpath`, `MountAccess`, typed `SecretGrant` mount/environment
forms, and independently optional volume `UnixId` UID/GID ownership. `ImageIntent` requires a
validated `ImageSource` and an explicit `ImagePullPolicy`; its classification exposes portable,
local, unqualified, and tagless sources without changing their spelling. Secret material remains
only in `SecretIntent`'s redacted external input reference.
For mounted secrets, omitted mode retains Podman's documented `0444` default in the Libpod request
description as `Mode: 292`, so the CLI and API renderings have the same semantics.

M6-B3a adds `ContainerIntent::runtime_mut()` for bounded health, logging, security, CPU shares,
period/quota, memory, PID, and rlimit intent on containers, including pod members. Startup health
requires configured normal health. Public health shell and direct-exec forms are explicit; inline
and external forms stay redacted. `HealthInterval::Disabled` is the only representable native-zero
interval; normal retries are at least one, startup retries and successes allow zero, and timeouts
are at least one second. Logging labels retain public key/value pairs. CPU, memory, and PID controls
require explicit `CgroupCapabilityEvidence` and root context, while rlimits do not; PodmanLens
never discovers either locally. Journald labels require Podman 6.0 or newer, unlimited rlimits
require 5.6 or newer, and CPU quota is positive and at least one millisecond; semantic planning
enforces these target gates. `namespaces_mut()` retains only unpodded private/host PID, IPC, UTS,
and cgroup modes plus IPC `shareable`/`none`; cgroup private requires v2 evidence and a pod member
gets `PLN0038`. B3b renders this bounded public subset exactly and repeats the planner's target
gates defensively. Sensitive and external health commands remain redacted `PLN0046` boundaries;
they prevent an artifact for the affected resource rather than creating a partial representation.

Create one `DeploymentIntent` with an explicit reviewed `TargetProfile`, then add fully resolved
target-side `DeploymentResource` values. `DeploymentResourceId` is deliberately separate from an
observed `ResourceIdentity`: output names are declarations, not input observations. A required
network, volume, image, or secret outside the plan must be present as `ExternalPrecondition`; a
missing reference is never assumed to exist. Pods and containers remain managed in M5 so their
lifecycle and membership are validated.

`plan_deployment` returns `PlanningOutcome`. A successful outcome has one deterministic semantic
plan; an unsuccessful outcome has sorted `PlanningFinding` values and no partial plan. The only
M5 operations are image acquisition, resource creation, `StartPod`, and `StartContainer`. Managed
images use a bounded portable pull-reference grammar: a host-qualified lower-case registry/repository
with either a tag or a `sha256` digest. Only malformed spellings are constructor-rejected. Valid
local, unqualified, and tagless spellings are retained with their `ImageSourceClassification`; managed
rendering later blocks them with manual `PLN0048` portability findings rather than rewriting them.
Every managed image uses an explicit `ImagePullPolicy`; no policy is implied. Portable sources can
be acquired from Podman 5.6 onward. Callers that intentionally avoid managed acquisition declare
an `ExternalPrecondition` instead.

Pods can depend on networks and named volumes; containers declare matching membership in both
directions. A pod with members gets one
`StartPod` after every member create; unpodded containers get `StartContainer`. Add semantic
container ordering with `StartupDependency`: cross-pod edges are lifted to pod starts, while a
same-pod edge is rejected. Secret material uses `SensitiveInputReference` and never becomes plan
content; the typed external reference is retained on the managed secret operation so M6 can render
it without recovering data from the caller. Every operation likewise retains its complete typed
managed-resource intent, and `DeploymentPlan::external_preconditions` preserves all explicit
network, volume, image, and secret boundaries in deterministic order. `render_deployment` produces
the reviewed M6 CLI argument arrays, Libpod request descriptions, a versioned deployment artifact,
and a shell review script from that same plan without opening a connection or executing Podman.

M6-A exposes `render_deployment`, producing deterministic CLI argument arrays and versioned Libpod
request descriptions without opening a connection. `DeploymentRendering::connection` preserves the
optional non-sensitive output-connection name; `artifact::deployment_v1` exports it as an explicit
validated string or `null`. A deployment artifact represents declared output, not an observational
snapshot: it may include only values explicitly constructed as public by the caller and never
sensitive values or sensitive-input references. The accepted name is 1–64 ASCII
bytes, begins with an ASCII alphanumeric character, and otherwise uses only ASCII alphanumeric
characters, dots, underscores, or hyphens. URI, endpoint, socket-path, credential, token, and
whitespace spellings are rejected with `PLN0034` before rendering or serialization.
`ContainerIntent::settings_mut` exposes bounded typed command, entrypoint, user, workdir, hostname,
label, environment, and restart-policy values. `NamedVolumeMount` keeps one managed volume identity,
normalized destination, read-only state, and copy mode. `PodIntent::add_infra_mount` deliberately
names the Podman infra-container scope: it does not add the mount to each member container. Labels
and environment assignments retain declaration order and reject duplicate keys;
`SensitiveInlineEnvironmentValue` and external environment references redact their value in `Debug`.
`PublicLabelValue` and `PublicEnvironmentValue` make caller declassification explicit; do not
construct them from observed runtime values. M6-B1b renders these explicitly public values exactly;
`SensitiveInlineEnvironmentValue` and external environment references instead cause a redacted,
all-or-nothing `PLN0046` outcome and never expose an environment name, value, or reference.

`DeploymentRendering::shell_script` is generated solely from those argument arrays, requires
explicit secret file paths, and safely names every external prerequisite in a review comment.
Rendering accepts only an identical engine/API version listed in its committed v5 renderer evidence.
Each reviewed line carries exact CLI, model, and handler provenance for every rendered setting.
Pod membership, unpodded-container topology, typed named-volume mounts, and the bounded public
container settings are exact. The bounded M6-B2 networking subset is also exact with committed
per-release Podman and common-module evidence. Container network order and non-unicast route types
need Podman 6.0 or newer; lower reviewed targets return a field-level finding and no output. Secret
grants retain typed mount/environment targets and bounded mounted-secret UID/GID/mode options, and
render to both planes. Secret payload material remains an external deferred input and is never put
in an artifact, diagnostic, or rendered request body.
For the exact basic topology that is already rendered, Libpod create bodies use the native Go
member spelling `Networks`; the lowercase JSON member is not emitted.

`PlanningFinding::occurrence` is a one-based list position for a duplicate prerequisite or
startup edge. Grouped duplicate or conflicting resource declarations have no single position;
their `PlanningFinding::count` reports the number of declarations instead.

## Declare networking output

Use `NetworkAttachment` for every explicit network attachment. It keeps aliases and caller-declared
static IPv4, IPv6, and MAC values distinct from runtime-assigned addresses. Add attachments, ports,
DNS configuration, and host aliases to `PodIntent` when the target topology has a pod: Podman
places these values on the infra container and shares the namespace with its members. An unpodded
`ContainerIntent` can own the same values directly. The planner rejects these fields on a pod
member; it does not move or silently discard them.

`NetworkIntent` can hold explicit IPAM `NetworkSubnet` declarations and `NetworkRoute` values.
`NetworkCidr::new` canonicalizes its address to the network address and its prefix to canonical
decimal spelling, so host-bit variants identify the same declared subnet.
Gateway and inclusive allocation-range endpoints must be in their declared CIDR; range endpoints
must be ordered. Static IPv4, IPv6, and MAC declarations require an explicitly
`TargetExecutionContext::Rootful` profile during planning. `Unknown` and `Rootless` contexts
produce field-level findings and no plan rather than guessing a privilege boundary.
Unicast routes require a gateway. Explicit unpodded-container network order is validated as an
exact permutation of the attached networks and needs Podman 6.0 or newer; omit it to let Podman
use its native default. Pod networking has no corresponding order contract.

## Non-goals

The input API does not discover an ambient connection, parse `podman` command output, execute
commands, choose BoxFerry mappings, or decide a target pod layout.
