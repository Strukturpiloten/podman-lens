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
container, pod, network, volume, image, and secret-metadata set in that disposable service. The
worker uploads a compact evidence JSON whose stable artifact name includes candidate SHA, run
ID, run attempt, and task. Release selects and validates that exact attempt, so a retry cannot
reuse stale earlier-attempt evidence. Its rootless
cell follows the reviewed image contract without Docker privilege, using only FUSE and the
required SELinux/AppArmor exceptions. Each image's reviewed numeric service UID is verified inside
the running container. Resource provisioning finishes before the API service starts, avoiding
concurrent CLI/API access to nested Podman storage; the socket is then handed off to the runner.
Release requires
the worker result to be successful before the sole publication-permission job is eligible; failure,
timeout, cancellation, missing evidence, or a skipped worker is fail-closed. `validation_only`
cannot enter that job.

## Consequences

Ordinary pull requests remain offline and privacy-safe. Release gains bounded native API evidence
without reusing a host service or retaining native payloads. Podman installation and reset are
limited to the disposable GitHub-hosted runner.
