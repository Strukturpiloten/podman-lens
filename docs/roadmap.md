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

- [ ] Define explicit connection, observed-version, and target-profile types.
- [ ] Implement a replaceable asynchronous transport for local Libpod sockets.
- [ ] Add explicit remote-connection support without ambient endpoint discovery.
- [ ] Negotiate and validate Podman and Libpod API versions.
- [ ] Build the initial reviewed capability catalogue for Podman 5.4 through 6.1.
- [ ] Pin offline API fixtures and provenance for every reviewed minor line.
- [ ] Add current-patch conformance for Podman 5.8.6 and 6.1.0.

Exit: system information can be decoded deterministically through real and fixture transports.

## M2: Native resource inventory

- [ ] Inspect containers and pods.
- [ ] Inspect networks and named volumes.
- [ ] Inspect images without treating shared images as application ownership.
- [ ] Inspect secret metadata without requesting payloads.
- [ ] Preserve identifiers, labels, relationships, native fields, and evidence provenance.
- [ ] Report unsupported, malformed, conflicting, and version-inapplicable fields individually.
- [ ] Implement explicit redacted and included environment-value policies.

Exit: an explicitly selected connection produces a complete typed inventory with no silent loss.

## M3: Resource discovery

- [ ] Support container, pod, network, volume, image, secret, and label-selector roots.
- [ ] Make complete evidenced dependency closure the default.
- [ ] Merge overlapping closures and retain disjoint resource groups.
- [ ] Detect shared prerequisites and network boundaries.
- [ ] Support exact authorized boundary crossings without a grouping file.
- [ ] Discover and order all groups for an `all` request.
- [ ] Explain every included resource, stopped boundary, merge, and ordering edge.

Exit: selectors return deterministic groups and shared prerequisites in a validated dependency order.

## M4: Stable input contract

- [ ] Expose the native inventory, graph, evidence, and diagnostics as a documented public API.
- [ ] Keep protocol response types private to the versioned decoder layer.
- [ ] Add fixed-seed malformed-response and graph-boundary corpus tests.
- [ ] Add end-to-end fixtures representative of common rootless and rootful installations.
- [ ] Reach first input-capable development-release readiness without requiring an intermediate
      publication.

Exit: BoxFerry can implement a Podman input adapter without parsing native JSON itself.

## M5: Ordered deployment planning

- [ ] Define the versioned deployment-plan JSON schema.
- [ ] Define semantic create and start operations for supported resources.
- [ ] Topologically order networks, volumes, secrets, pods, containers, and start operations.
- [ ] Emit each shared prerequisite once before its consumers.
- [ ] Generate exact CLI program and argument arrays.
- [ ] Generate exact native Libpod HTTP methods, paths, and typed bodies.
- [ ] Prove CLI and API equivalence for every dual-representation operation.
- [ ] Represent sensitive inputs as references rather than serialized payloads.
- [ ] Reject cycles and unresolved prerequisites before rendering.

Exit: one ordered plan can drive both copyable CLI output and a native API client without execution.

## M6: Podman output coverage

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
