# First-release readiness

Milestones M0 through M7 are accepted for the first BoxFerry-ready PodmanLens release. Publication
is deliberately maintainer-controlled and is not part of merging the implementation pull request.

## Contract audit

| Area                   | Audited result                                                                                                                                                                                                                                                                               | Regression evidence                                                                   |
| ---------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------- |
| Public Rust API        | BoxFerry can acquire, discover, consume every typed observation state, construct deployment intent, plan, and render using public items only. Protocol response DTOs and observation constructors remain private. Public extension enums are non-exhaustive.                                 | `tests/public_api.rs`, `tests/boxferry_adapter.rs`, rustdoc with warnings denied      |
| Native field coverage  | 142 input-observation rows and 50 output-intent rows, 192 total. Every accepted row names decoder/contract/diagnostic/test ownership; unmodeled fields remain explicit.                                                                                                                      | Strict `catalogue/v1/native-field-coverage.json` validation                           |
| Snapshot schema        | `snapshot::v1` is serialization-only, always redacted, strict Draft 2020-12, and has exact inventory/graph goldens. There is no deserializer.                                                                                                                                                | `tests/snapshot_v1.rs`, schema and golden checks                                      |
| Deployment schema      | `artifact::deployment_v1` is serialization-only, strict Draft 2020-12, retains external prerequisites without material, and matches the shell rendering's semantic plan.                                                                                                                     | Deployment, render, schema, and exact artifact tests                                  |
| Diagnostics            | Acquisition, discovery, planning, and rendering return bounded structured findings with stable codes and field/resource context. Malformed and partial input never becomes silent absence; plans/renderings are all-or-nothing on errors.                                                    | Negative inventory/corpus/discovery/deployment/render suites                          |
| Redaction              | Debug, diagnostics, snapshots, fixtures, and artifacts exclude connection secrets, environment values, health-command arguments, secret payloads, secret driver option names/values, host-specific unknown values, and protected label values. Secret payload endpoints are never requested. | Redaction tests, sentinel scans, corpus policy tests                                  |
| Compatibility          | Podman 5.4 through 6.1 uses explicit half-open ranges and revision-pinned evidence. All six kinds have pinned 5.7, 6.0, and bounded 6.1 offline corpora.                                                                                                                                     | Capability, rendering, corpus-hash, all-six-kind, and current-patch conformance tests |
| Downstream integration | The exact mapping policy handles field state and origin before neutral intent. The public-only scenario covers acquisition through both renderers with a committed expected result.                                                                                                          | `docs/boxferry-integration.md`, `tests/boxferry_adapter.rs`                           |
| Release mechanics      | Release-plz prepares release PRs. The protected release workflow alone publishes, tags, attests, and creates the GitHub release.                                                                                                                                                             | Repository-policy tests and workflow validation                                       |

## Redaction guarantees

The following are first-release invariants:

1. Secret payload endpoints are never called and secret material has no observation type.
2. Environment and health-command values are accessible only through explicit caller
   authorization; their `Debug` and snapshot forms remain redacted.
3. Snapshot v1 never serializes environment values, secret payloads, connection data, raw unknown
   JSON, label values, driver-option values, Compose ownership values, or local unknown-field
   values.
4. Secret driver options retain only field state, provenance, and count. Security options retain
   count-only evidence where their values cannot be represented safely.
5. Diagnostics contain stable codes and bounded safe identities, never protected values.
6. Synthetic fixtures contain no real endpoint, user, container, image, environment, secret,
   connection, or credential data and are hash-pinned.

Any relaxation requires an explicit public authorization type, positive and sentinel-negative
tests, documentation, and a compatibility review.

## Schema and API change policy

The first release establishes crate version `0.1.0` as the semver baseline. PodmanLens is pre-1.0:

- compatible fixes and additive bounded APIs may ship in `0.1.x`;
- a source-breaking public Rust API change requires the next pre-1.0 minor version and a breaking
  Conventional Commit;
- private Libpod DTO changes do not affect semver;
- an incompatible serialized snapshot or deployment shape requires a new versioned module and
  schema after the first release; v1 is not silently rewritten;
- native-field ledger and compatibility-catalogue changes must accompany behavior changes;
- release-plz determines the release proposal, but only a maintainer merges its release PR and
  authorizes publication.

The ordinary offline gate validates the package and API. Once a published baseline exists, hosted
CI also runs semver checks against it. Publication credentials remain confined to the protected
release workflow.

## M0-M7 acceptance

| Milestone             | Acceptance evidence                                                                                                                                                 |
| --------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| M0 Foundation         | Pinned Rust 2024/MSRV workspace, governance, dependency/release policy, complete local gate                                                                         |
| M1 Version/transport  | Explicit connections, read-only transport, version probe, reviewed 5.4-6.1 catalogue                                                                                |
| M2 Inventory          | Non-atomic six-kind acquisition, inspect-once behavior, protected environment policy, partial/malformed findings                                                    |
| M3 Discovery          | Deterministic roots/groups/dependencies, ownership evidence, network boundaries, explanations                                                                       |
| M4 Stable input       | Public typed inventory/graph, private DTOs, redacted snapshot v1, fixed malformed/rootless/rootful/graph corpora                                                    |
| M5 Planning           | Fully resolved target intent, deterministic operations, external prerequisites, no execution                                                                        |
| M6 Output             | Strict-ledger-backed B1-B4 CLI and Libpod descriptions, target gates, exact artifacts, no silent lossy rendering                                                    |
| M7 BoxFerry readiness | Typed native observations, exact mapping contract, compatibility matrices, all-six-kind 5.7/6.0/6.1 corpora, public downstream scenario, audits and semver baseline |

Deferred execution, broader native fields, Docker/Kubernetes protocols, persistent grouping
configuration, arbitrary mutation, and built-in secret encryption are explicitly outside this
release. They do not weaken any accepted contract and must enter through later reviewed batches.

The final acceptance command is `./scripts/check-all.sh`. Any edit after a successful final run
invalidates that result and requires the complete gate again.
