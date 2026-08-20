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
optional non-sensitive output-connection name; `snapshot::deployment_v1` exports it as an explicit
validated string or `null` while always redacting secret material. The accepted name is 1–64 ASCII
bytes, begins with an ASCII alphanumeric character, and otherwise uses only ASCII alphanumeric
characters, dots, underscores, or hyphens. URI, endpoint, socket-path, credential, token, and
whitespace spellings are rejected with `PLN0034` before rendering or serialization.
`DeploymentRendering::shell_script` is generated solely from those argument arrays, requires
explicit secret file paths, and safely names every external prerequisite in a review comment.
Rendering accepts only an identical engine/API version listed in its committed per-operation renderer
evidence. Pod networks, pod membership, and unpodded-container networks are exact. Pod volumes and
container volumes or secrets are rejected with `PLN0046` until the semantic model carries their
mount/target/mode data.

`PlanningFinding::occurrence` is a one-based list position for a duplicate prerequisite or
startup edge. Grouped duplicate or conflicting resource declarations have no single position;
their `PlanningFinding::count` reports the number of declarations instead.

## Non-goals

The input API does not discover an ambient connection, parse `podman` command output, execute
commands, choose BoxFerry mappings, or decide a target pod layout.
