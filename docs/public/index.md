# PodmanLens

PodmanLens is the version-aware native Podman library used by BoxFerry. It acquires one
explicitly selected Libpod service through a read-only transport, discovers an evidence-backed
resource graph, plans typed deployment intent, and renders deterministic review artifacts.

It never discovers an ambient Podman endpoint, shells out to `podman` for input, executes a
deployment plan, or sends a mutating acquisition request. Callers own cross-format mapping, loss
authorization, output publication, and any later execution decision.

## Choose a task

- [Acquire an inventory](acquisition/): select one explicit Unix socket and retain typed native
  observations without exposing protected values.
- [Discover an application graph](discovery/): choose exact resources or labels and follow
  evidenced dependencies deliberately.
- [Review grouping and boundaries](grouping/): distinguish resource groups, pod membership,
  shared prerequisites, and authorized network crossings.
- [Plan and render](planning-rendering/): turn caller-authored target intent into an ordered
  semantic plan, CLI and Libpod descriptions, deployment JSON, and a review script.
- [Handle diagnostics and privacy](diagnostics-privacy/): preserve structured findings and keep
  runtime-sensitive data out of diagnostics, snapshots, and deployment artifacts.
- [Select a compatible version](compatibility/): use the finite reviewed Podman catalogue and an
  explicit target execution context.

The exhaustive Rust API is published separately as
[PodmanLens Rustdoc](https://boxferry.dev/docs/api/podman-lens/). Cross-format conversion belongs
to [BoxFerry](https://boxferry.dev/docs/).
