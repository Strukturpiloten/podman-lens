# 0017: Isolated native service conformance gates release

- Status: Accepted
- Date: 2026-09-21

## Context

Captured cassettes prove deterministic replay, but not that the exact release candidate can open a
real Libpod API service and consume its current native response surface. An ambient socket would
make that check dependent on an unreviewed host, expose operational data, and violate the explicit
connection boundary.

## Decision

`Native Podman conformance` is a reusable workflow used for manual diagnostics and every Release.
It creates a fresh disposable Podman service on the runner, supplies its unique Unix socket only to
ignored production-acquisition tests, and destroys the store afterwards. The tests permit only an
exact reviewed version and bodyless GET requests. The service matrix contains the reviewed
BoxFerry `podman-6.1-rootful` and `podman-6.1-rootless` images at immutable manifest digests;
Renovate owns each version-and-digest pair as one replacement.

Before the production read-only acquisition test, the worker provisions a bounded sanitized
container, pod, network, volume, image, and secret-metadata set in that disposable service.
Identity checks and every inner Podman provisioning command run beneath the outer container's
initial OCI process. Host-side `podman exec` is forbidden: GitHub-hosted runners can deny the
re-exec required by rootless Podman even when the same operation succeeds from that initial
process. The worker uploads a compact evidence JSON whose stable artifact name includes candidate
SHA, run ID, run attempt, and task. Release selects and validates that exact attempt, so a retry
cannot reuse stale earlier-attempt evidence. Both cells use a rootful host-Podman launcher with isolated
root, runroot, temporary, file-lock, and socket state. Common service arguments provide FUSE and
disable SELinux relabeling. The rootful image alone uses a privileged outer container; the reviewed
source-built rootless image stays unprivileged and adds only the AppArmor exception required by its
canonical image contract. The rootless image must start as reviewed UID 1000 and
`Host.Security.Rootless` must report `true`; the rootful image must start as UID 0 and report
`false`. The launcher receives no repository secrets or ambient Podman socket, mounts only the
run-scoped socket directory, and removes its service, image, and state after the cell.
The per-run, per-cell state root uses a bounded `/tmp` name so the host Podman 4.9 launcher never
exceeds its 50-character runroot limit. Cleanup validates the run identity and matrix key, then
recomputes that exact path instead of accepting an environment-controlled removal target.
The outer launcher is installed from the `ubuntu-24.04` runner's package repository and is
intentionally not a conformance-version input. Renovate continues to own the immutable inner image
versions and digests that define the tested Podman identities.
Failure evidence derives the reviewed version from the immutable matrix image rather than
successful-service environment, so a setup failure still uploads bounded
candidate/run/attempt/cell evidence. The initial process atomically publishes only bounded setup
status, service UID, and root-mode files in the run-scoped socket directory. The host uses a
five-minute polling deadline while also checking container liveness, then requires regular
non-symlink files with one line, strict byte bounds, and allowlisted values. Only after successful
validation does the host create a directory start gate. The initial process waits for that gate and
replaces itself with `podman system service`; socket readiness is separately bounded to 30 seconds and rejects a
symlink. This ordering keeps CLI provisioning and API access from using nested Podman storage
concurrently.
Release requires
the worker result to be successful before the sole publication-permission job is eligible; failure,
timeout, cancellation, missing evidence, or a skipped worker is fail-closed. `validation_only`
cannot enter that job.

## Consequences

Ordinary pull requests remain offline and privacy-safe. Release gains bounded native API evidence
without reusing a host service or retaining native payloads. Podman installation and reset are
limited to the disposable GitHub-hosted runner.
