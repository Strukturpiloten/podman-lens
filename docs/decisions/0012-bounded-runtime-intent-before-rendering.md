# 0012: Bounded runtime intent precedes rendering evidence

- Status: Accepted
- Date: 2026-08-20

## Decision

M6-B3a retains bounded health, logging, security, and resource-control intent for both unpodded
containers and pod members before CLI or Libpod syntax is claimed. Public health commands are
explicitly declassified; inline and external shell and direct-exec forms remain redacted. It also
retains bounded private/host PID, IPC, UTS, and cgroup namespace intent, plus IPC `shareable` and
`none`, only on unpodded containers. A pod member with namespace intent receives `PLN0038`; the
library does not move it to a pod or guess ownership. Pod-level namespaces, user namespaces, ID
maps, paths, and container/pod-reference namespace modes remain deferred without separate
PodSpecGenerator and handler evidence.

Cgroup CPU, memory, and PID controls require caller-supplied hierarchy/controller evidence and an
explicit root context; rlimits do not. PodmanLens does not infer either from the local host.
Rootless or unknown-root-mode cgroup v1 controls fail planning, while private cgroup namespace
requires v2 evidence. M6-B3b renders only the bounded public forms after immutable per-field,
per-version evidence is present, and blocks sensitive/external health inputs with `PLN0046` before
an artifact can be created. Semantic planning rejects journald labels before Podman 6.0 and an
unlimited rlimit before Podman 5.6. CPU quota is accepted only as a positive value of at least one
millisecond because non-positive values are not dual-exact. B3b renderers must repeat these target
gates defensively before claiming exact output.
