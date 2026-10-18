# Dependency policy

Every dependency needs one narrow, reviewed role. PodmanLens does not add a general Podman client,
process executor, ambient connection resolver, or cross-format model through dependencies.

| Role                                    | Dependencies                                              |
| --------------------------------------- | --------------------------------------------------------- |
| Version parsing                         | `semver`                                                  |
| Bounded JSON decoding and serialization | `serde`, `serde_json`                                     |
| Explicit endpoint validation            | `url`                                                     |
| Read-only Unix HTTP transport           | `bytes`, `tokio`, `hyper`, `hyper-util`, `http-body-util` |
| Local schema tests only                 | `jsonschema` with retrieval features disabled             |

The built-in transport accepts only bodyless `GET` requests over one explicit Unix socket. No
dependency provides redirects, decompression, retries, SSH, TLS, process execution, or plan
application.

`jsonschema` is test-only and validates committed local schemas and fixtures. It does not shape
runtime types or retrieve remote schemas.

The minimum supported Rust version is the maintainer-owned `workspace.package.rust-version` in
`Cargo.toml`; `clippy.toml` and the README must agree with it. CI and the local gate read Cargo
metadata, and Release reuses CI. Renovate's built-in `rust-toolchain` manager owns the separately
pinned development compiler, grouped as `Rust toolchain`; it does not raise the support floor.
The Cargo manager owns crate updates and lock-file maintenance. The schema-test graph now uses
the ordinary transitive `yoke-derive` resolution without the obsolete compiler-compatibility
constraint or Renovate exclusion.

Dependencies are locked in `Cargo.lock`, audited by `cargo deny`, and updated through Renovate.
Git dependencies and unknown registries are denied. A dependency that changes public types, wire
decoding, serialization, the async runtime, or the security boundary requires an ADR.

Renovate defaults to a three-day minimum release age. Only Cargo/crate updates for the exact
BoxFerry package list and `compose-lens`, `podman-lens`, `quadlet-lens`, and `docker-lens` receive
the first-party zero-day override; review and automerge controls remain unchanged.
Lock-file maintenance also uses a zero-day override because its synthetic updates lack individual
release timestamps. Before the required aggregate PR gate succeeds, the shared, fail-closed guard
checks each newly introduced registry version. Only the four exact Lens names from Cargo's
canonical crates.io registry waive elapsed age, after a bounded successful registry lookup for the
requested package/version returns a valid publication timestamp. Missing or invalid evidence,
unavailable registry data, and future timestamps still fail. Third-party packages and their transitive
dependencies retain the 72-hour cutoff; BoxFerry names receive no lockfile exemption. The guard's
immutable revision has one Renovate owner. Dev Container and checksum-pinned tool updates remain
manual.
The PR documentation job installs those tools as the
runner user into a unique directory under `RUNNER_TEMP`, then adds that directory to `GITHUB_PATH`.
The installer retains its version and SHA-256 checks; this destination-only change leaves the
existing Renovate extraction from `scripts/install-file-tools.sh` and its manual checksum-review
rule unchanged.
