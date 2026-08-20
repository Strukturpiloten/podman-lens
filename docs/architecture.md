# Architecture

PodmanLens owns Podman-native protocol handling. It turns reviewed Libpod API evidence into typed
input observations and ordered deployment plans. Callers such as BoxFerry own orchestration and
cross-format semantics.

## System boundary

| PodmanLens owns                              | The caller owns                                  |
| -------------------------------------------- | ------------------------------------------------ |
| Connection and transport abstraction         | User-facing command-line structure               |
| Libpod API request and response types        | Compose, Quadlet, Docker, and Kubernetes parsing |
| Podman version and capability evidence       | Mapping to and from a neutral application model  |
| Native resource inspection and relationships | Loss-policy authorization                        |
| Native resource discovery and boundaries     | Output-directory lifecycle and reports           |
| Ordered Podman deployment operations         | Deciding pod layout for cross-format input       |
| M5 semantic operation planning               | Whether and how a plan is executed               |
| M6 CLI and Libpod API operation renderings   | Whether and how a plan is executed               |

PodmanLens is a library. It must not shell out to `podman` to acquire input, depend on BoxFerry, or
execute generated operations.

## Data flow

```text
Connection + discovery request
             |
             v
      replaceable transport
             |
             v
    versioned Libpod protocol
             |
             v
 typed native resource inventory
             |
             v
 evidence-backed resource graph
             |
             +----> caller maps input semantics

explicit Podman deployment intent
             |
             v
  version-aware operation planner
             |
             v
 ordered semantic operations
             |
             v
 M6 CLI/API/JSON renderers
```

The Docker-compatible API is not the canonical protocol because it cannot represent every
Podman-native feature, including pods. A replaceable transport carries versioned Libpod requests
over a local socket or an explicitly selected remote connection.

## Core contracts

M1 makes this small foundation public:

- `ConnectionSpec` identifies only explicit local Unix, verified SSH, or mutual-TLS TCP service
  endpoints. It does not read environment variables or Podman connection configuration.
- `LibpodTransport` is object-safe and asynchronous. Callers may implement it for their selected
  Unix, SSH, or TLS client. `ReadOnlyUnixTransport` is the sole built-in implementation: it uses
  one explicit Unix socket for acquisition and is available only on Unix targets. It rejects every
  method except bodyless `GET`, caller-supplied `Host`, and over-limit requests before opening the
  socket. Its configurable HTTP/1 header ceiling must be at least the parser's documented 8 KiB
  minimum. PodmanLens provides no mutating client or executor.
- `LibpodRequest`, `LibpodResponse`, and duplicate-preserving `LibpodHeaders` are bounded
  transport messages. Their `Debug` output redacts paths, headers, and bodies.
- `ObservedPodmanVersion`, `ObservedApiVersion`, and `TargetProfile` retain original version
  spelling, reject prereleases, and fail closed outside the reviewed 5.4.0–6.2.0 engine range.
  A selected Libpod API must be at least 4.0.0 and no newer than that selected engine.
- `capability_catalogue()` returns the published immutable evidence for reviewed 5.4–6.1 lines.
- `probe_libpod_service()` performs exactly `GET /libpod/_ping` and then
  `GET /v4.0.0/libpod/version`, validates bounded protocol evidence, and returns independent
  observed engine/API versions with their reviewed target profile.

M2 and M3 add these provisional public input contracts:

- `ResourceInventory` retains typed Podman resources, identifiers, relationships, findings, and
  native evidence from `acquire_inventory`.
- `ResourceSelector` selects one exact resource name, ID, or image alias.
- `LabelSelector` selects exact label-key presence or exact label-key/value equality.
- `DiscoveryRequest` contains resource and label roots, `all` selection, and exact network
  name-or-ID boundary authorizations.
- `ResourceGraph` retains requested selectors, the `all` choice, resolved roots with redacted
  origin positions, deterministic groups, shared prerequisites, directed dependencies,
  non-directed grouping evidence, findings, and explanations.

The following output names remain conceptual until their milestones:

- `DeploymentIntent` contains fully resolved Podman-native resources supplied by a caller.
- `DeploymentPlan` contains an ordered list of semantic operations.
- `Operation` contains identity, dependencies, resource action, and all exact supported transport
  representations.

## M2 inventory acquisition

`acquire_inventory` is read-only and first performs the fixed service probe. It then lists the six
resource kinds in this exact order—containers, pods, networks, volumes, images, secrets—and only
then inspects each canonical-sorted stable ID. Containers use `all=true&sync=true`; images use
`all=true`. All inventory requests use the API version observed by the probe. The protocol decoder
creates only `GET` requests; it never calls a secret payload endpoint or adds a secret-revealing
query parameter.

The acquisition is deliberately non-atomic. An unavailable list makes only that section
unavailable. A `404` or malformed inspect response creates a partial record with its stable list
identity, leaving every unrelated list and record available. Unknown data is represented only by
path, JSON kind, record identity, and source/version evidence—not raw JSON. Podman image names and
IDs are treated as raw identifiers and percent-encoded once before an inspect path is generated.

## M4 stable input and snapshots

`acquire_inventory` and `discover` form the stable native input façade. Inventory and graph types
expose typed observations, evidence, findings, deterministic topology, and explanations. Libpod
response DTOs stay private so protocol revisions do not become accidental public Rust contracts.

`snapshot::v1` copies that state into strict serialization-only inventory and graph views. The
views never contain environment values, secret payloads, connection details, raw unknown JSON,
label values, driver-option values, or Compose ownership values. They intentionally retain useful
operational metadata and are therefore redacted, not anonymous. PodmanLens does not deserialize
snapshots or treat them as trusted input.

## Ordered deployment plans

The order of `operations` is authoritative. `depends_on` records why the order exists and permits a
consumer to validate or safely parallelize independent work. The semantic resource action is the
source of truth. M5 owns only this semantic plan. M6-A renders evidence-backed basic topology into
argument arrays, Libpod request descriptions, a versioned JSON export, and a POSIX review script;
it never opens a connection or executes either representation. M6-B1a retains typed named-volume
mounts for containers and explicitly infra-container-scoped mounts for pods, plus bounded container
settings (command, entrypoint, user, workdir, hostname, labels, environment, restart policy), while
preserving their declared order where relevant. The renderer
does not yet have per-field target-version evidence, so each populated setting or mount yields
`PLN0046` rather than being omitted. Sensitive inline environment values and external references
are redacted in plan diagnostics, debug output, observational snapshots, and deployment artifacts. The caller-selected non-sensitive
output connection survives in the rendering and JSON export only as a validated Podman connection
name, never a URI, endpoint, path, credential, or token; CLI arrays carry it as `--connection`.
The review script emits deterministic, shell-quoted comments for every external network, volume,
image, or secret prerequisite, but never a secret-material reference or value. It renders pod
networks, pod-member assignment, and unpodded-container networks. Container mounts, infra-container
mounts, and every typed container
setting remain structured `PLN0046` findings until exact renderer coverage exists; secret attachment
targets are not yet part of the semantic model.

Executing every operation sequentially in array order must always be valid. Parallel execution is
an optional optimization derived from `depends_on`, not a requirement for consuming the plan.

Managed target-side resources have `DeploymentResourceId`, not acquired `ResourceIdentity`.
`ExternalPrecondition` makes an intentional unmanaged network, volume, image, or secret prerequisite
visible and `DeploymentPlan` retains those preconditions in deterministic identity order. Every
managed operation retains its complete typed resource intent, so an M6 renderer has the image
source, resource configuration, or redacted secret-material reference it needs without rebuilding
meaning from IDs. Omitted references are findings, never an implicit external boundary. Pods and
containers remain managed in M5. Image acquisition is explicit with migration-safe
`ImagePullPolicy::Missing` and a bounded portable pull-reference grammar; container creation must
not hide a pull.

Pods with members have one `StartPod` after all member create operations. Unpodded containers have
`StartContainer`. Intent-level container order is lifted to pod starts across pod boundaries;
same-pod order is rejected because Podman starts those members as a unit. PodmanLens never executes
future rendered commands or HTTP requests.

## Expected BoxFerry integration

PodmanLens does not own BoxFerry's CLI, but these option groups map directly to its public
contracts:

- `--input-connection` selects the `ConnectionSpec` used for read-only acquisition.
- Container, pod, network, volume, image, secret, label, and `all` selectors build a
  `DiscoveryRequest`.
- Complete resource-group closure is the default; an exact network-boundary override authorizes an
  exceptional crossing.
- `--pod-layout` is a BoxFerry mapping policy applied before Podman output intent reaches
  PodmanLens.
- `--output-connection` is recorded only as a validated Podman connection name and rendered into
  CLI/API descriptions. PodmanLens never accepts a destination URI here or connects to it.
- `artifact::deployment_v1` and `DeploymentRendering::shell_script` provide review-only
  `deployment-plan.json` and `deployment.sh` content; BoxFerry will own output-directory lifecycle
  and reporting.

PodmanLens returns structured data and, from M6, rendered artifacts. BoxFerry remains responsible
for output-directory safety, diagnostic presentation, and loss-policy authorization.

## Discovery and resource groups

Every exact resource selector and label selector is a root. `ResourceGraph` records requested
selectors separately from resolved identities. Default discovery follows the complete evidenced
dependent-to-prerequisite closure. Dependency edges retain their direction for later planning;
grouping edges are non-directed evidence and cannot be interpreted as deployment order.

Pod membership, native container dependencies, and complete matching Docker/Podman Compose
ownership aliases are strong grouping evidence. Closures merge only through strong evidence.
Shared network, volume, image, and secret prerequisites remain boundaries and do not bridge
otherwise disjoint groups. Explicitly selecting a shared resource crosses to its direct consumers;
an exact network name-or-ID authorization crosses only that resolved network boundary.

Group IDs are the smallest member `(kind, id)`, and groups are ordered by that identity. The
explanation trace accounts for every root, included member, prerequisite, stopped boundary,
authorized crossing, strong-evidence merge, and group-order position. Unresolved or ambiguous
selectors, relationships, and boundary authorizations remain structured findings.

`all` applies the same rules to eligible roots: pods, unpodded non-infra containers, standalone
networks, volumes, and secrets, plus images with validated Compose ownership. Cached images are not
roots merely because they exist.

Podman's `network.internal` property controls connectivity, not ownership. It cannot by itself
decide whether traversal should cross a network. Exact user-authorized boundary crossings handle
exceptions; no separate grouping file is required.

## Pod layout

Resource groups describe dependency topology. Pod layout describes target runtime placement. They
must remain separate.

PodmanLens preserves observed pod membership on input and renders explicit pod membership on
output. The caller decides whether previously unpodded containers are preserved, wrapped one per
pod, combined per resource group, or left without pods. Combining a whole resource group in one pod
can change namespace and lifecycle semantics and must not be inferred silently.

## Sensitive values

Normal secret inspection retrieves metadata only. Secret payload access requires explicit caller
authorization. Environment values obtained from a runtime are redacted by default because Podman
cannot prove which values are sensitive.

Sensitive bytes must not appear in diagnostics, `Debug` or `Display` output, snapshots, or the
serialized deployment plan. A plan refers to external sensitive inputs. An explicit unsafe-file
renderer may materialize restricted files later. Base64 is an encoding, not protection.

## Compatibility

Input records the observed Podman version. Output requires an explicit target profile. Capability
data governs both CLI flags and Libpod request fields; the development machine never supplies an
implicit target version.

The embedded catalogue identifies reviewed Podman 5.4 through 6.1 source releases and current
patch provenance for 5.8.6 and 6.1.0. The fixed probe validates status, duplicate-preserving
headers, bounded JSON shape, and exactly one `Podman Engine` component. It preserves engine/API
versions independently; compatible versions do not have to be textually equal. A version is not
fully supported until its fixtures, boundaries, and positive and negative tests are committed.

## Primary references

- [Podman system service](https://docs.podman.io/en/latest/markdown/podman-system-service.1.html)
- [Libpod REST API](https://docs.podman.io/en/latest/_static/api.html)
- [Podman remote connections](https://docs.podman.io/en/latest/markdown/podman-system-connection.1.html)
