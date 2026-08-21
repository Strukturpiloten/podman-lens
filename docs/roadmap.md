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

The M2 input surface is now guarded by `catalogue/v1/native-field-coverage.json`: a strict,
machine-readable ledger with one row per accepted native field and links to decoder ownership,
public contract, diagnostics, and focused tests. It records observation-only and manual boundaries
without implying output support. Unknown-field metadata is intentionally bounded; overflow and
partial inspection explicitly make the retained metadata incomplete.

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
grammar and an explicit `ImagePullPolicy`; a plan never hides an image pull inside container
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
- [x] M6-B1a: Add bounded typed named-volume mounts, explicitly infra-container-scoped pod mounts,
      and container command, entrypoint, user,
      workdir, hostname, labels, environment, and restart-policy intent. Preserve order where it
      is semantic, redact sensitive environment values, and reject every populated unrendered
      field with `PLN0046`.
- [x] M6-B1b: Render command, entrypoint, user, workdir, unpodded hostname, explicitly public
      labels and environment values, restart policy, and named-volume mounts exactly to CLI and
      Libpod forms for every reviewed release. Require its per-field source matrix before rendering;
      the current v5 catalogue retains this evidence and adds M6-B2 availability plus
      cross-repository common-module claims. Reject ambiguous mount CLI spellings and keep
      sensitive environment variants
      redacted as all-or-nothing `PLN0046` outcomes.
- [x] M6-B2: Add typed network attachments, static addresses and MAC addresses, port mappings,
      DNS, host aliases, IPAM subnets, routes with metrics, and explicit rootful/rootless target
      semantics. Keep pod-owned networking on the infra container and reject member-owned network
      namespace configuration before rendering. Exact CLI and Libpod rendering is recorded for
      every reviewed release with Podman plus pinned common/container-libs source-repository
      evidence. Container network order and non-unicast route types are exact only in Podman 6.0+
      and otherwise produce field-level `PLN0046`; static IPv4, IPv6, and MAC declarations require
      an explicit rootful planner target. Multi-IP attachment forms, port ranges, interface names,
      arbitrary network drivers/options, and unmanaged namespace modes remain explicitly outside
      this bounded model for the later native-field coverage ledger.
- [x] M6-B3a: Retain bounded health, logging, security, and container resource intent for all
      containers; retain bounded namespace intent only for unpodded containers; require explicit
      cgroup/root-context evidence; enforce the reviewed journald-label, unlimited-rlimit, and
      CPU-quota planning boundaries; and block rendering until exact per-field evidence exists.
- [x] M6-B3b: Render the bounded public runtime surface exactly to CLI and Libpod descriptions;
      preserve all-or-nothing redaction for sensitive health commands and repeat runtime target
      gates during rendering.
- [x] M6-B4: Add typed named-volume, bind, and tmpfs mounts; named-volume subpaths; optional
      volume UID/GID ownership; typed mounted and environment secret grants; and explicit image
      pull policy/source classification. Pod infra mounts block because `pod create` has no dual
      CLI mount form. UID/GID ownership and image policy require Podman 5.6+; local,
      unqualified, and tagless managed image sources remain structured manual portability work.
- [x] Generate deterministic CLI program/argument arrays and native Libpod HTTP method, path, and
      typed-body descriptions for the bounded ledger-backed B1–B4 surface.
- [x] Assert revision-pinned two-plane evidence and byte-exact regression renderings for every
      supported bounded field and reviewed target/version gate; reject an unsupported field rather
      than claiming CLI/API equivalence.
- [x] Preserve dependency order, explicit pod membership, and validated unpodded output intent.
- [x] Emit sorted structured outcomes for every bounded approximation, omission, manual action,
      and target boundary.
- [x] Render `deployment.sh` and `deployment-plan.json` from the same semantic plan.

M6 is complete for the B1–B4 bounded, strict-ledger-backed surface. It does not claim exhaustive
Podman output coverage.

## Post-M6 bounded output batches

- [ ] Add another reviewed, evidence-backed batch for selected remaining native container or pod
      fields only after its planner, CLI, Libpod, diagnostic, and focused boundary tests are defined.
- [ ] Extend the strict output ledger one bounded field family at a time; do not claim universal
      coverage of future native fields or values.

Exit: the bounded B1–B4 Podman intent surface renders to its reviewed CLI and Libpod descriptions
without silent loss; all other native output remains explicitly outside that contract.

## M7: BoxFerry integration readiness

- [x] M7-A: Reset the unreleased inventory contract to typed native observations with explicit
      field state, provenance, protected environment handling, bounded semantic unmodelled
      metadata, and redacted snapshot support.
- [x] M7-B1: Add bounded container core configuration, topology, typed named-volume/bind mount,
      and secret-grant observations. Host paths remain local-resolution-only and secret payloads
      remain unavailable.
- [x] M7-B2a: Add observation-only native network IPAM subnet, independently optional lease-range
      endpoints, gateway, and static-route evidence. Network inspect values remain effective
      evidence; reviewed host-bit spellings are retained only for network-subnet CIDRs, with
      normalized containment checks. Generic CIDR parsing preserves raw syntax defensively but
      does not claim host-bit route destinations are valid. Route type is explicitly
      version-inapplicable before Podman 6.0 and defaults to effective `unicast` from 6.0 when
      omitted; this batch adds no output mapping.
- [x] M7-B2b: Add observation-only pod `InfraConfig` and unpodded-container `HostConfig`
      networking with explicit configured/effective provenance. `CreateNetNS` remains a distinct
      configured gate and is not inverted into a host-network claim. Runtime `NetworkSettings`
      addresses and assignments are never promoted.
- [x] M7-B2: Complete bounded native network input coverage without inferring container runtime
      addresses, arbitrary driver/option values, or output behavior.
- [x] M7-B: Document the exact PodmanLens-to-neutral-model mapping contract.
- [x] M7-B3a: Add observation-only container restart policy, normal and startup health checks,
      health-failure action, and logging. These are effective inspect evidence, not authored
      deployment intent: inspect may include image health defaults and normalized host settings.
      Health command arguments remain protected callback-only evidence, and snapshots retain only
      their count. Unknown enum spellings remain bounded `PLN0023` metadata; malformed command
      forms and negative native count values fail closed with `PLN0017`. Security, namespace, and
      resource-control observations follow in M7-B3b.
- [x] M7-B3b: Add observation-only container security, namespace, and resource controls with
      effective provenance. Capability order and duplicates remain native evidence; unknown
      semantics stay bounded `PLN0023` metadata. Security-option values are never retained.
      Namespace observations remain visible on pod members. CPU, memory, PID, and ulimit values
      preserve native zero/`-1` and order without output validation; malformed ulimit members fail
      the complete collection closed. This batch adds no output mapping.
- [x] M7-B4: Add observation-only image repository tags/digests and immutable metadata, volume
      driver/creation/anonymous metadata, and secret creation/update plus count-only driver-option
      metadata. Native timestamps retain an exact validated RFC 3339 spelling. Image repository
      references remain local-resolution evidence, secret option names/values are discarded, and
      this batch adds no output mapping.
- [x] Provide large offline input and output scenarios for BoxFerry adapter tests.
- [x] Publish compatibility matrices for Podman versions, resource kinds, and transports.
- [x] Stabilize the APIs required by BoxFerry while leaving unimplemented native fields explicit.
- [x] Reach first BoxFerry-ready release readiness; publication remains a maintainer-controlled
      step after merge.

Exit: BoxFerry can add Podman input and output routes using only public PodmanLens APIs.

M7 is complete. The normative mapping contract is `docs/boxferry-integration.md`; compatibility
matrices are in `docs/compatibility.md`; `docs/release-readiness.md` records the public API,
schema, diagnostics, redaction, release-policy, semver, and M0-M7 acceptance audits. Revision-pinned
bounded 6.1 input and adapter corpora cover all six kinds. The public-only adapter golden proves
acquisition, discovery, typed observation consumption, explicit neutral decisions, deployment
intent, planning, and both non-executing renderers.

M7-A is complete when `ResourceObservation` exposes one kind-safe `ResourceDetails` payload and
every modeled field reports an `ObservationField` state. Configured, effective,
runtime-assigned, and local-resolution values remain distinct. Container configured image spelling
and local image resolution remain separate; discovery uses only configured image evidence. This is
the in-place reset completed before version 0.1.0 was released; no obsolete record projection is
kept.

## M8: Post-0.1 conformance hardening

- [x] Define a strict request-aware cassette v1 schema and test-only replay transport that bind every
      response to its expected Libpod method and path.
- [x] Reject unexpected, missing, repeated, reordered, or unconsumed requests without exposing
      response bodies through failures.
- [x] Add 14 stable complex cassettes across Podman 5.4.0, 5.5.0, 5.6.0, 5.7.0, 5.8.6, 6.0.0,
      and 6.1.0 in simulated rootless and rootful contexts.
- [x] Cover all six native resource kinds, podded and unpodded containers, dependencies, shared and
      isolated networks, network boundaries, shared volumes, image evidence, secret metadata,
      health, restart, logging, security, namespaces, and resource controls in every scenario.
- [x] Keep every cassette source-derived, synthetic, sanitized, deterministic, provenance-bearing,
      and hash-verified; never represent it as an export of a running Podman environment.
- [x] Run multiple acquisition and discovery cases against every cassette, including deterministic
      permutations, exact and label roots, dependency closure, grouping, shared prerequisites,
      authorized and stopped network crossings, redaction, and version-specific findings.
- [x] Prove matching-version planning and both non-executing renderers accept each simulated
      rootless or rootful target context without turning the cassette harness into an executor.
- [x] Correct compatibility defects exposed by the expanded matrix: pod infra `StaticIP` remains
      applicable throughout supported 5.x, and Podman 6 route type uses native `route_type`.
- [x] Consolidate superseded 5.7 and 6.0 BoxFerry response streams into the request-aware matrix,
      retain seven focused regression/golden artifacts, and verify 21 manifest artifacts.
- [ ] Add the complete live Podman version/context matrix after all reproducible environments
      exist, tracked separately in
      [GitHub issue #3](https://github.com/Strukturpiloten/podman-lens/issues/3).

M8 deterministic offline hardening is complete without changing the production execution boundary.
The 14 contexts are simulated evidence dimensions, not live exports. The deferred live matrix is
`workflow_dispatch` only and must cover every version in rootless and rootful mode without skips;
no nightly schedule or pull-request workflow is claimed or added here.

## Deferred

- Executing or applying deployment plans.
- Docker and Kubernetes protocol support.
- A persistent resource-grouping configuration file.
- General-purpose output mutation such as adding arbitrary labels.
- Built-in secret encryption; established external formats should be evaluated first.
