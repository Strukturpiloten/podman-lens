# Dependency policy

Dependencies have reviewed roles; none adds an ambient resolver, process executor, general Podman
client, or cross-format model.

| Role                | Dependencies                                              |
| ------------------- | --------------------------------------------------------- |
| Version parsing     | `semver`                                                  |
| Bounded JSON        | `serde`, `serde_json`                                     |
| Explicit endpoints  | `url`                                                     |
| Read-only Unix HTTP | `bytes`, `tokio`, `hyper`, `hyper-util`, `http-body-util` |
| Local schema tests  | `jsonschema`, retrieval disabled                          |

Transport permits only bodyless `GET` over explicit Unix sockets; dependencies provide no
redirects, decompression, retries, SSH, TLS, process execution, or plan application.

The maintainer owns `workspace.package.rust-version`; `clippy.toml` and README agree, and
validation reads Cargo metadata. Renovate owns the separately pinned `Rust toolchain` development
compiler without raising the support floor.
Cargo dependencies remain locked and audited by `cargo deny`; Git dependencies and unknown
registries are denied. Public API, decoding, serialization, runtime, and security changes require
an ADR.

Immutable common and Rust presets in `.github/renovate.json` own routine automation. Eligible
updates merge after integrity and compatibility checks without routine approval. Repository
additions own native-runtime extraction, image declarations and nested GitHub CLI options.
Shared inventory owns downloaded versions and verified checksums; the common preset couples
feature declarations and lock integrity. Regression checks reject duplicate owners and accidental
approval overrides.

The default-branch eligibility service persists first-seen evidence from authoritative CI, PR and
queue metadata; candidate CI has read-only access. Manifest and policy Git blobs at the trusted PR
or queue base select authority independently of candidate automation. Receipts bind the tested SHA.
Third-party updates wait 72 hours. An isolated maintained first-party release PR, including necessary
young transitive resolution, is exempt only after trusted closure checks. Unavailable, malformed,
future, moved or stale evidence fails closed. Age admission is inapplicable to main, manual and
release runs; complete integrity, audit and compatibility checks remain.

The Podman qualification tuple stays fixed. Its exact Renovate owners are disabled without routine
review proposals; target changes require separate qualification. Historical evidence stays unchanged.
Release requires fresh native conformance and protected environment approval.

The shared project owns Python, uv, Ruff, ty and lock integrity; consumers declare lint scope.
Local, CI and image file tools use the six-tool `scripts/install-host-tools.sh` inventory, including
Actionlint and Lychee; the image also exports standalone uv/Ruff/ty.
`scripts/install-file-tools.sh` is a compatibility launcher without separate pins or owner. The
locked official whole-graph CLI generates Feature integrity.

The `enabled:false` hold covers direct image, Feature and ARG updates, except coverage below.
Shared metadata, package locks and compiler updates retain one automatic owner and select image
qualification when in its input closure. Required native CI image lanes establish fresh qualification;
source checks do not. VS Code uses existing Podman storage, preferably rootless. Updater/App delivery,
schedules and hold removal remain deferred.
[ADR 0019](decisions/0019-shared-validation-and-dependency-automation.md) records this stage.

Coverage's uniquely annotated `CARGO_LLVM_COV_VERSION` ARG owns its readable version; locked
crates.io installation supplies integrity. Its exact crate/path/manager Renovate exception preserves automatic updates and shared
age/grouping/approval policy. Other image/native qualification holds
remain; tool updates do not qualify images.
