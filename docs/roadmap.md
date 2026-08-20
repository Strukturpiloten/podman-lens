# Roadmap

The roadmap targets a strong first Podman input/output library without building an orchestration
platform. Each milestone must keep malformed input panic-free, preserve unrepresented native data,
and add positive and negative tests.

## M0: Repository foundation

- [x] Define the project boundary and architecture.
- [x] Record the initial accepted decisions.
- [x] Define the implementation roadmap.
- [x] Create the Rust 2024 library workspace and public crate.
- [x] Set the MSRV and API-stability policy.
- [x] Add the shared formatting, linting, testing, coverage, dependency, release-plz, Renovate, and
      VS Code task conventions used by the other BoxFerry repositories.
- [x] Add contribution, security, dependency, testing, and release documentation.

Exit: the empty library builds on the pinned toolchain and the complete local validation task passes.

## M1: Version evidence and transport

- [x] Define explicit connection, observed-version, and target-profile types.
- [x] Define a replaceable asynchronous transport contract with bounded, redacted messages.
- [x] Add explicit remote-connection specifications without ambient endpoint discovery.
- [x] Negotiate and validate Podman and Libpod API versions through the fixed read-only probe.
- [x] Build the initial reviewed capability catalogue for Podman 5.4 through 6.1.
- [x] Pin offline API fixtures and provenance for every reviewed minor line.
- [x] Add ignored, explicit current-patch conformance for Podman 5.8.6 and 6.1.0.

M1 is complete. Its built-in Unix HTTP/1.1 transport is acquisition-only: it permits `GET` and
rejects `POST` and `DELETE` before socket connection. It has no redirect following, decompression,
retries, ambient endpoint discovery, detached tasks, SSH client, mutual-TLS client, or execution
facility. SSH and mutual-TLS remain caller-provided `LibpodTransport` implementations.

Exit: version information is decoded deterministically through fixture transports and an opt-in,
explicit current-patch read-only conformance transport.

## M2: Native resource inventory

- [x] Inspect containers and pods.
- [x] Inspect networks and named volumes.
- [x] Inspect images without treating shared images as application ownership.
- [x] Inspect secret metadata without requesting payloads.
- [x] Preserve identifiers, labels, relationships, native fields, and evidence provenance.
- [x] Report unsupported, malformed, conflicting, and version-inapplicable fields individually.
- [x] Implement explicit redacted and included environment-value policies.

Exit: an explicitly selected connection produces a complete typed inventory with no silent loss.

M2 is complete. Its offline 5.4 and 6.1 fixture provenance covers the reviewed compatibility
boundaries. Every acquisition probes first, lists all six kinds, then inspects every valid stable
identifier exactly once. It retains unavailable sections, malformed list entries, duplicate IDs,
disappeared records, unknown fields, conflicting image evidence, and unexpected secret payload
fields as structured findings rather than silently deleting them. The inventory is a provisional
public API; M4 will stabilize it with the resource graph and broader corpus.

## M3: Resource discovery

- [x] Support exact container, pod, network, volume, image, and secret roots.
- [x] Support exact label-presence and label-value roots without exposing label values in debug output.
- [x] Retain every requested selector, the `all` choice, each resolved root, and redacted root-origin
      provenance in the result.
- [x] Make evidenced dependent-to-prerequisite closure the default.
- [x] Merge closures only through pod membership, native container dependencies, or validated
      Compose ownership evidence.
- [x] Keep shared prerequisites and network boundaries from merging groups.
- [x] Support exact network name-or-ID crossings without a grouping file.
- [x] Discover deterministic eligible roots for an `all` request.
- [x] Expose dependency edges separately from grouping evidence and structured findings.
- [x] Explain every included resource, stopped boundary, authorized crossing, strong-evidence merge,
      and ordering decision.

Exit: selectors return deterministic groups, shared prerequisites, directed dependency evidence,
and a complete explanation trace.

M3 is complete. Group IDs use the smallest member `(kind, id)`; dependency edges always point
dependent to prerequisite; and `network.internal` is connectivity-only evidence. Pod membership is
kept as grouping evidence without creating a pod-to-container dependency cycle. Compose labels are
advisory only when both Docker and Podman project/service pairs agree and are non-empty; matching
non-empty config hashes are validated when present. Explicit shared-resource roots and exact
network name-or-ID crossings may add consumers; ordinary container roots do not reverse-traverse
shared prerequisites. The returned explanation trace accounts for every selected resource and
group-ordering decision.

`all` seeds pods, unpodded non-infra containers, standalone networks, volumes, and secrets, plus
images carrying complete validated Compose ownership evidence. It does not treat every cached
image as an application root.

## M4: Stable input contract

- [x] Expose the native inventory, graph, evidence, and diagnostics as a documented public API.
- [x] Keep protocol response types private to the versioned decoder layer.
- [x] Add serialization-only, always-redacted `snapshot::v1` inventory and graph exports with a
      strict Draft 2020-12 JSON Schema and exact goldens.
- [x] Add fixed malformed-response and graph-boundary corpus tests without fuzzing.
- [x] Add end-to-end sanitized fixtures representative of rootless 5.4 and rootful 6.1 installations.
- [x] Prove deterministic snapshots under fixed list and selector permutations.
- [x] Reach first input-capable development-release readiness without requiring an intermediate
      publication.

Exit: BoxFerry can implement a Podman input adapter without parsing native JSON itself.

M4 is complete. `acquire_inventory` and `discover` are the stable native input façade. Public
inventory and graph types retain typed evidence and structured findings while private Libpod DTOs
remain decoder details. `snapshot::v1` is an export-only support/reporting boundary: it never
deserializes and always removes environment values, secret payloads, connection data, raw unknown
JSON, label values, driver-option values, and Compose ownership values. Corpus artifacts are
sanitized, provenance-bearing, hash-verified, offline, and fixed rather than fuzz-generated.

## M5: Ordered deployment planning

- [x] Define typed, target-side resource identities and fully resolved managed intent.
- [x] Make external preconditions explicit rather than treating omitted resources as external.
- [x] Define semantic image acquisition, create, `StartPod`, and `StartContainer` operations.
- [x] Topologically order networks, volumes, secrets, images, pods, containers, and starts.
- [x] Emit each managed shared prerequisite once before its consumers.
- [x] Validate pod membership, duplicates, conflicts, missing references, unsupported combinations,
      same-pod startup requests, and semantic cycles as sorted structured findings.
- [x] Keep secret material external and protect it from plan/debug output.

Exit: one typed intent produces one deterministic, transport-neutral semantic plan without execution.

M5 is complete. `DeploymentIntent` uses a reviewed `TargetProfile`, typed target-side resources,
and intent-level `StartupDependency` edges. A managed image has a bounded portable pull-reference
grammar and the migration-safe `ImagePullPolicy::Missing`; a plan never hides an image pull inside container
creation. Pod members are created before exactly one `StartPod`; unpodded containers get their own
`StartContainer`. Cross-pod start edges are lifted to those pod starts, while same-pod ordering is
rejected. `PlanningOutcome` returns a plan only when its sorted findings are empty.
Every operation retains the complete typed managed-resource intent and every explicit network,
volume, image, or secret precondition is retained on the plan in deterministic order. Secret bytes
remain external; only a redacted external material reference is available to M6.

## M6: Podman output coverage

- [x] M6-A: Define the versioned, serialization-only deployment-plan JSON schema and render
      evidence-backed basic topology into review-only CLI/Libpod representations; preserve the
      explicit output connection, safely disclose external prerequisites in review scripts, and
      reject fields without sufficient semantics rather than marking a lossy rendering exact.
- [ ] Generate exact CLI program and argument arrays.
- [ ] Generate exact native Libpod HTTP methods, paths, and typed bodies.
- [ ] Prove CLI and API equivalence for every dual-representation operation.
- [ ] Cover pods, containers, networks, named volumes, images, and secrets in dependency order.
- [ ] Preserve explicit pod membership and validate unpodded output intent.
- [ ] Cover environment, mounts, ports, health, restart, security, namespace, and resource settings.
- [ ] Add positive and negative tests for every supported value and version boundary.
- [ ] Emit structured outcomes for every approximation, omission, and manual action.
- [ ] Render `deployment.sh` and `deployment-plan.json` from the same plan.

Exit: supported Podman intent round-trips through CLI and API representations without silent loss.

## M7: BoxFerry integration readiness

- [ ] Document the exact PodmanLens-to-neutral-model mapping contract.
- [ ] Provide large offline input and output scenarios for BoxFerry adapter tests.
- [ ] Publish compatibility matrices for Podman versions, resource kinds, and transports.
- [ ] Stabilize the APIs required by BoxFerry while leaving unimplemented native fields explicit.
- [ ] Reach first BoxFerry-ready release readiness; publication remains a maintainer-controlled
      step after merge.

Exit: BoxFerry can add Podman input and output routes using only public PodmanLens APIs.

## Deferred

- Executing or applying deployment plans.
- Docker and Kubernetes protocol support.
- A persistent resource-grouping configuration file.
- General-purpose output mutation such as adding arbitrary labels.
- Built-in secret encryption; established external formats should be evaluated first.
