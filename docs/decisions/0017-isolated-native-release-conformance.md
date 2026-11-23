# 0017: Isolated native service conformance gates release

- Status: Accepted
- Date: 2026-09-21
- Amendments: [ADR 0019](0019-shared-validation-and-dependency-automation.md) recorded the temporary update hold. The Fedora artifact decision below replaces that hold.

## Fedora artifact retention and update ownership

Validation-only Release run [44934186506](https://github.com/Strukturpiloten/podman-lens/actions/runs/44934186506)
failed before either native mode started. Fedora deleted Koji build
[3102806](https://koji.fedoraproject.org/koji/buildinfo?buildID=3102806), and both original
unsigned artifact URLs returned HTTP 404. Signed copies surfaced on a testing mirror with
different full-file hashes. Fedora's later 6.1.4
[advisory](https://bodhi.fedoraproject.org/updates/FEDORA-2026-f91a799f07) was unpushed.

Use the signed stable Podman 6.1 build from the fixed Fedora 45 final release. Fedora's
[stable advisory](https://bodhi.fedoraproject.org/updates/FEDORA-2026-c30886e9ec) and
[Koji build](https://koji.fedoraproject.org/koji/buildinfo?buildID=3111822) record the adopted
build and Fedora packaging revision. The final binary and source package indexes identify both
RPMs. Both signatures verify against Fedora 45's published key, fingerprint
`4F50 A611 4CD5 C697 6A7F 1179 655A 4B02 F577 861E`. Fedora's release server and the
Princeton and Hannover Fedora mirrors served byte-identical copies of both RPMs. The signed
Koji paths served the same bytes. The source RPM includes the upstream tag archive with no RPM
patch files. The archive matches GitHub's tag archive, and the annotated tag peels to the
reviewed upstream commit. The active pin file carries the selected versions and digests.

Both native cells and ordinary CI use the same bounded input check. Fallback applies when bytes
are unavailable. A successful download with a mismatched digest fails the check. The build
still checks the installed NEVRA, binary-to-source RPM relationship, source archive, spec, and
package closure. The pinned OCI index has one Linux amd64 manifest.

Renovate proposes the Fedora build identifier from signed stable Bodhi records. That identifier
selects both the RPM release and the upstream tag, and the build component permits a proposal
when Fedora rebuilds the same upstream patch. Bodhi publishes no binary, SRPM, source archive,
or source commit checksums suitable for replacement. Verify their exact values and provenance in
the same proposal. A build outside the final compose currently depends on retained signed Koji
files. Verify a durable Fedora distribution source before adopting such a build.
The separate Fedora base owner proposes immutable digest updates for the Fedora 45 tag. Both
owners use manual review and require both native modes before adoption. A move beyond Libpod
6.1 requires a separately qualified compatibility decision.

## Context

Captured cassettes prove deterministic replay, but not that the exact release candidate can open a
real Libpod API service and consume its current native response surface. An ambient socket would
make that check dependent on an unreviewed host, expose operational data, and violate the explicit
connection boundary.

## Decision

`Native Podman conformance` is a reusable workflow used for Fedora image qualification, reviewed
manual diagnostics, and every Release.
It has a separate read-only admission job before the native matrix. A manual `workflow_dispatch`
is validation-only and requires an explicit 40-hex candidate SHA equal to the run SHA and checked-out
HEAD, a current same-repository `TheRealBecks/issue<N>` branch head at that SHA, and both the
original actor and rerun actor to have write, maintain, or admin repository permission. Failed
permission lookups, moved branches, tags, forks, and SHA mismatches stop before any privileged
runtime, `always()` evidence, or cleanup step. A manual dispatch trusts the entire independently
reviewed branch workflow and admission script at that candidate SHA; it is not an automatic run
of untrusted pull-request code. GitHub's permission endpoint is read with the job's read-only
token, and an unavailable response fails closed. A Release reusable call must explicitly set
`release_validation=true` and still admits only the current default-branch SHA, even though the
caller itself was dispatched. It does not depend on collaborator status of the automation actor.
Manual evidence is diagnostic and cannot satisfy a later Release run's exact run/attempt gate.

On ordinary pull requests and merge groups, the shared planner compares the Fedora 45 image
digest at the exact trusted base and the complete tested tree. A changed digest independently
requires native conformance. Admission binds the planner's candidate to the run SHA, checkout,
and current PR merge ref or default-branch queue ref. These calls use read-only permissions and
need no collaborator lookup. The native gate verifies both cells succeeded through cleanup and
checks both candidate/run/attempt artifacts before returning a receipt. Required CI independently
recomputes selection and verifies that receipt.

It creates a fresh disposable Podman service on the runner, supplies its unique Unix socket only to
ignored production-acquisition tests, and destroys the store afterwards. The tests permit only an
exact reviewed version and bodyless GET requests. The rootful and rootless cells each build a
disposable image from a Fedora 45 base pinned by tag and digest, signed checksum-verified Podman
RPMs, and the fixed Fedora release compose. The builder verifies the compose repository metadata
and primary package index, disables mutable repositories, and records the installed package closure
and built image ID. The Fedora runtime is isolated test infrastructure. The provider and update
contract is recorded above.
Podman's image inspection reports a bare 64-hex ID; the builder accepts that form or an explicit
`sha256:` prefix, validates the bytes, and records one canonical prefixed evidence value.
This workflow's only active native line is 6.1; older 5.4–6.0 catalogue anchors and 6.1.0
captures remain immutable evidence. BoxFerry #340 coordinates active cross-line patch adoption.

The original Fedora Koji binary RPM was `5:6.1.2-1.fc45.x86_64`, SHA-256
`f3eab0b767203dd940e93b7a5e8df60a101b4d7832c1e5fdf62497bfcdd7e6bb`. Both RPMs
were unsigned and separately checksum-pinned. Inspection found an upstream `v6.1.2.tar.gz` source
archive and no patch files in that source RPM. The upstream annotated tag peels to
`04f3aa430e6df81bea059978bc5bafbc846ba3e7`. Fedora 45 Beta supplied the fixed compose. The Fedora base
digest, RPM and source digests, compose metadata digests, and package closure are carried in
candidate-bound evidence. A new patch requires re-review of those values and native behavior.
The upstream 6.1.1 notes describe a rootless port-binding fix; 6.1.2 notes describe security and
dependency fixes. Neither release note announces a Libpod API or creation-evidence shape change,
which is why the live probe and authored-field checks remain mandatory.
The offline current-patch probe deliberately simulates engine 6.1.2 with API 6.1.0 and a failed
version GET; this is source-derived test input, not a recaptured native service. The live worker
independently asserts the exact fixture image, configured authored image-spelling hint, 404 on a
missing-container GET, and the observed Libpod API header. Existing 6.1.0 captured cassettes
retain their original provenance.

Before the production read-only acquisition test, the worker provisions a bounded sanitized
container, pod, network, volume, image, and secret-metadata set in that disposable service.
Identity checks and every inner Podman provisioning command run beneath the outer container's
initial OCI process. Host-side `podman exec` is forbidden: GitHub-hosted runners can deny the
re-exec required by rootless Podman even when the same operation succeeds from that initial
process. The worker uploads a compact evidence JSON whose stable artifact name includes candidate
SHA, run ID, run attempt, and task. Before the first rootless Podman probe, that initial process
emits bounded numeric UID/GID map, capability, `NoNewPrivs`, seccomp, both ID-map helper modes,
classified file capabilities, and only its own subordinate-range diagnostics. It omits usernames,
helper paths, raw xattrs, and raw host files. Collection failure
is diagnostic, not compatibility evidence, and does not change runtime privileges.
Release selects and validates that exact attempt, so a retry
cannot reuse stale earlier-attempt evidence. Both cells use a rootful host-Podman launcher with isolated
root, runroot, temporary, file-lock, and socket state. Common service arguments provide FUSE and
disable SELinux relabeling. The rootful image alone uses a privileged outer container; the
rootless image stays unprivileged and adds only the AppArmor exception required by the nested
service contract. Its final image stage grants `/usr/bin/newuidmap` only `cap_setuid=ep` and
`/usr/bin/newgidmap` only `cap_setgid=ep`; image construction and a UID-1000 runtime probe must
verify those exact file capabilities before native conformance begins. The rootless image must
start as reviewed UID 1000 and
`Host.Security.Rootless` must report `true`; the rootful image must start as UID 0 and report
`false`. The launcher receives no repository secrets or ambient Podman socket, mounts only the
run-scoped socket directory, and removes its service, image, and state after the cell.
The per-run, per-cell state root uses a bounded `/tmp` name so the host Podman 4.9 launcher never
exceeds its 50-character runroot limit. Cleanup validates the run identity and matrix key, then
recomputes that exact path instead of accepting an environment-controlled removal target.
The build script has an EXIT/TERM trap for its own failure path. The workflow cleanup inventories
containers, volumes, intermediate images, and mounts only in that cell's isolated store; it removes their
exact IDs and deletes the store only after checking it is empty. A cleanup failure fails the job
and preserves the named state directory for diagnosis. A runner SIGKILL or lost host cannot run
traps, so such an interruption needs runner-level disposal and does not create release evidence.
Before building, the worker requires 10 GiB available on the state filesystem. During build,
provisioning, API startup, and conformance it samples the exact run-owned store every five seconds,
requiring at least 2 GiB still available and no more than 8 GiB in that store. It stops an active
phase on a failed sample or exceeded budget and records separate measured build and runtime peaks
plus their maximum in release evidence. The nested service waits without runtime operations
between phases and is stopped under monitoring before evidence upload. Directory accounting is
limited to the backing filesystem, excluding Podman's transient mounted `merged` view while still
counting stored layers and volumes. A disappearing exact-store overlay mount permits at most two
short measurement retries; unrelated read errors and repeated churn fail closed. Accounting is
conservative and cannot prevent
one write from crossing a limit between samples. The 50-minute native job timeout bounds all
pre-cleanup step maxima to 42 minutes, reserving five minutes for cleanup and three more minutes of
job headroom; a killed runner still cannot guarantee its own cleanup.
The hosted runner installs the outer launcher from its package repository. Review the active RPM,
fixed compose, and base image together. Release accepts evidence after the build and native probe
both pass.
The retained 6.1.0 captures keep their original provenance.
Failure evidence derives the reviewed version from the candidate's pinned runtime inputs rather than
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
timeout, cancellation, missing evidence, or a skipped worker is fail-closed. The release gate
checks out the exact candidate SHA with credentials disabled before downloading current-attempt
artifacts and running the candidate-owned evidence validator. This prerequisite-first ordering
keeps a fresh runner fail-closed while making the validator available. `validation_only` cannot
enter the publication job.

## Consequences

Ordinary pull request CI checks public artifact availability and integrity with bounded downloads.
Release gains bounded native API evidence without reusing a host service or retaining native
payloads. Podman installation and reset are limited to the disposable GitHub-hosted runner.
