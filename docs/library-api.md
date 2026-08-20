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

## Inspect native-field coverage

`native_field_coverage_catalogue()` returns the packaged, strict ledger for the currently accepted
M2 input fields. Each row links one native path to its decoder, output applicability, public
accessor, diagnostic rule, and focused positive and negative tests. `not_applicable` in the planner
and renderer references means only that the observed field has no native deployment-output contract
yet; it does not authorize a conversion.

`ResourceRecord::unknown_fields()` retains bounded metadata for unmodeled native fields without raw
values. Call `ResourceRecord::unknown_fields_complete()` before treating that metadata as an
exhaustive account: it is false for partial records and after `PLN0021` unknown-field overflow.
`HostConfig.MemorySwappiness` is typed, while any other direct `HostConfig` member is explicitly
unsupported metadata. `Secret.Spec.Driver` is typed metadata; secret payload material is discarded
and reported as `PLN0018`.

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

Snapshots still contain operational metadata such as resource IDs and names, image aliases,
environment variable names, network subnets, evidence URLs, and source field paths. Always-redacted
does not mean anonymous; callers must still handle reports as operational data.

## Plan native deployment semantics

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
with either a tag or a `sha256` digest. Local, unqualified, and malformed spellings are rejected.
They use explicit migration-safe `ImagePullPolicy::Missing`.

Pods can depend on networks and named volumes; containers declare matching membership in both
directions. A pod with members gets one
`StartPod` after every member create; unpodded containers get `StartContainer`. Add semantic
container ordering with `StartupDependency`: cross-pod edges are lifted to pod starts, while a
same-pod edge is rejected. Secret material uses `SensitiveInputReference` and never becomes plan
content; the typed external reference is retained on the managed secret operation so M6 can render
it without recovering data from the caller. Every operation likewise retains its complete typed
managed-resource intent, and `DeploymentPlan::external_preconditions` preserves all explicit
network, volume, image, and secret boundaries in deterministic order. M6 will add CLI, Libpod
HTTP, JSON, and shell representations.

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
attachments remain unmodelled because their target and option semantics are not yet retained.
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
