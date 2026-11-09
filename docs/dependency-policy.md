# Dependency policy

Dependencies need narrow, reviewed roles. PodmanLens adds no ambient connection resolver,
process executor, general Podman client, or cross-format model.

| Role                | Dependencies                                              |
| ------------------- | --------------------------------------------------------- |
| Version parsing     | `semver`                                                  |
| Bounded JSON        | `serde`, `serde_json`                                     |
| Explicit endpoints  | `url`                                                     |
| Read-only Unix HTTP | `bytes`, `tokio`, `hyper`, `hyper-util`, `http-body-util` |
| Local schema tests  | `jsonschema`, retrieval disabled                          |

Transport permits only bodyless `GET` over explicit Unix sockets.
Dependencies provide no redirects, decompression, retries, SSH, TLS, process execution, or plan
application. Test-only `jsonschema` validates committed data without retrieval.

The maintainer owns the support floor in `workspace.package.rust-version`; `clippy.toml` and
README agree. Validation reads Cargo metadata. Renovate owns the separately
pinned development compiler, grouped as `Rust toolchain`, without raising the support floor.
Cargo dependencies remain locked and audited by `cargo deny`; Git dependencies and unknown
registries are denied. Public API, decoding, serialization, runtime, and security changes require
an ADR.

Immutable common and Rust presets in `.github/renovate.json` own routine dependency automation.
Eligible updates merge after integrity and compatibility checks without routine manual
approval. Repository additions own native-runtime extraction, image declarations and nested
GitHub CLI options. Shared inventory owns downloaded versions and verified checksums; the common
preset couples feature declarations and lock integrity. Extraction and effective-rule regressions
reject duplicate owners and accidental approval overrides.

The default-branch eligibility service reads authoritative CI, PR and queue metadata and persists
first-seen evidence. Candidate CI uses a read-only reader. Manifest and policy Git blobs at the
trusted PR or queue base select authority independently of candidate-proposed automation.
Receipts bind the exact tested SHA. Third-party updates retain 72 hours. An isolated maintained
first-party release PR, including necessary young transitive resolution, is exempt only after
trusted closure checks. Unavailable, malformed, future, moved, or stale evidence fails closed.
Main, manual and release admission is explicitly inapplicable; complete integrity, audit and
compatibility checks remain.

The coupled Podman qualification tuple stays fixed. Its exact Renovate owners are disabled without
routine review proposals; target changes require separate qualification. Historical evidence stays
unchanged. Release requires fresh native conformance and protected environment approval.

The selected shared project owns Python, uv, Ruff, ty and lock integrity. Consumers declare lint
scope only. Host/CI tools use `scripts/install-host-tools.sh`, delegating to the selected immutable inventory. Only the retained Dockerfile uses image-only legacy `scripts/install-file-tools.sh`; its original
versions and verified checksums keep one extractor.
A late `enabled:false` hold covers direct image, feature, ARG and legacy-installer updates pending
separate CI image qualification (BoxFerry #563), without manual-review proposals. Shared metadata, package locks and compiler updates retain one automatic owner, including
Dockerfile copies; resulting image closure remains unqualified. Contributors use VS Code
Dev Containers with existing Podman storage, preferably rootless. Image validation and updater
activation remain deferred. [ADR 0019](decisions/0019-shared-validation-and-dependency-automation.md)
records migration and rollback.

Coverage's existing uniquely annotated `CARGO_LLVM_COV_VERSION` ARG owns its readable version.
Locked crates.io installation supplies integrity. Its final exact crate/path/manager Renovate
exception preserves automatic updates and shared age/grouping/approval policy. Other image/native
qualification holds remain; tool updates do not qualify images.
