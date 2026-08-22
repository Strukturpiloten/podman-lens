# Testing

Every implementation change needs focused positive and negative coverage. Keep protocol fixtures
offline, minimal, and provenance-bearing. Never embed live connection endpoints, environment
values, secret payloads, or credentials.

Use unit tests for pure typed conversion and graph rules. Use integration tests for public APIs,
resource discovery, version boundaries, and rendered deployment plans. Tests that require a live
Podman service must be explicitly opt-in and must never be part of the ordinary deterministic gate.

Run the complete local gate with `./scripts/check-all.sh`. It formats tracked files, runs Rust and
repository-policy tests, measures coverage without inventing an initial threshold, validates the
MSRV, checks dependencies, and checks local documentation links offline.

`tests/public_guides.rs` protects the website-ready guide inventory and navigation, the explicit
Unix-socket/no-mutation example boundary, the deterministic offline plan and both rendering planes,
the exact version catalogue and Podman 5.6 renderer gate, graph-boundary fixture claims, diagnostic
codes, and protected-reference redaction. `cargo ci-check` compiles every packaged example target;
`cargo ci-doctest` compiles and runs the corresponding crate-level Rustdoc snippets, while
`tests/public_guides.rs` executes the packaged offline planning example directly.

Run the focused public-documentation checks with:

```shell
cargo test --test public_guides
cargo test --doc
cargo check --examples
```

The opt-in current-patch probe test is deliberately not part of that gate. It only permits the
fixed read-only acquisition probe. Run it explicitly against a selected local socket with:

```shell
PODMAN_LENS_CONFORMANCE_UNIX_SOCKET=/absolute/podman.sock \
PODMAN_LENS_CONFORMANCE_EXPECTED_VERSION=6.1.0 \
cargo test --test current_patch_conformance -- --ignored
```

The expected version must be exactly `5.8.6` or `6.1.0`; the test never discovers a connection,
retries, or sends a mutating Libpod request.

## Complex cassette conformance

The complex cassette suite is governed by
`docs/schemas/podman-lens-cassette-v1.schema.json` and replayed by
`tests/support/cassette.rs`. Its 14 fixtures cover all seven reviewed versions in simulated
rootless and rootful contexts. They are source-derived, synthetic, sanitized fixtures, not exports
of live Podman environments.

Replay matches the exact HTTP method and complete path plus query, requires every interaction to
be consumed, and rejects reordered, repeated, missing, unconsumed, or duplicate expected request
method/path keys. The manifest additionally validates each fixture's immutable provenance and
SHA-256 digest.

The main matrix exercises typed inventories and origins for all six resource kinds; exact and label
roots; dependency closure and shared prerequisites; stopped and authorized network crossings;
malformed, unavailable, and ambiguous overlays; deterministic permutations; redaction;
version-specific `StaticIP` and `route_type`; and matching-context planning with nonexecuting
rendering. The matching-context case preserves both pods, four unpodded containers, four network
boundaries, three shared volumes, three images, and two external secrets. It promotes configured
image, command, membership, and dependency facts only; network attachments and mounts are
explicitly test-authored policy, while effective IPAM, ownership, runtime settings, local image
resolution, bind sources, and unreconstructable secret grants stay out of target intent. It asserts
the complete 23-operation pre-5.6 or 26-operation 5.6+ plan dependency structure and matching
CLI/Libpod operation semantics. Podman 6.0 and 6.1 cases also prove that typed `route_type` evidence
does not remain as stale unmodelled route metadata in either simulated execution context.

Run the focused cassette gates with:

```shell
cargo test --test cassette_contract
cargo test --test complex_corpus
cargo test --test input_corpus corpus_manifest_verifies_fixed_provenance_and_hashes
```

## Deferred live version matrix

The committed offline response fixtures are source-derived, synthetic, sanitized test inputs. They
are not exports of running Podman environments and do not constitute live rootful or rootless
conformance evidence. [GitHub issue #3](https://github.com/Strukturpiloten/podman-lens/issues/3)
tracks a separate live workflow after reproducible environments exist for all 14 required cells:
Podman 5.4.0, 5.5.0, 5.6.0, 5.7.0, 5.8.6, 6.0.0, and 6.1.0, each in rootless and rootful mode.

No partial placeholder workflow is committed now. The future workflow is `workflow_dispatch` only:
it has no nightly schedule, pull-request trigger, excluded matrix cell, tolerated failure, or silent
skip. Every cell must use a reproducible digest-pinned environment, verify its exact engine and API
version, select an isolated cell-local Unix socket explicitly, and destroy its private runtime and
storage after the run. Privileged rootful execution must not expose the host Podman socket or run
untrusted pull-request code; rootless execution must use a dedicated user, subordinate IDs, and
isolated writable runtime and storage paths.

Live acquisition must continue through the production read-only Unix transport. A separate test
harness may provision a disposable source engine and apply rendered operations to a disposable
target engine, but that does not add mutation or execution to PodmanLens. Comparisons bind every
expected request and evaluate normalized, sanitized observations and semantic results rather than
volatile raw IDs, timestamps, addresses, or response bytes. Raw responses, connection details,
environment values, credentials, and secret material must never enter logs or uploaded artifacts.

The built-in Unix transport tests prove that mutation methods, body-bearing acquisition requests,
caller-supplied `Host`, and over-limit requests fail before a socket is opened. The Unix-only module
is conditionally compiled so platform-neutral connection and caller-provided transport contracts
remain usable on non-Unix targets.

M2 inventory tests use fixture transports only. They cover the Podman 5.4 and 6.1 evidence
boundaries; probe-before-list ordering; canonical list sorting; exact list queries; escaped image
inspect paths; partial races; malformed and duplicate list records; secret-metadata-only requests;
unknown-field metadata; and both environment retention policies. No ordinary test contacts a live
Podman service or contains a real secret or environment value.

Raw response-boundary tables exercise lists and inspections independently with truncated and
oversized JSON, case-insensitive parameterized JSON content types, and missing, duplicate, or
non-JSON content types. A malformed list remains section-local and prevents inspect requests; a
malformed inspection retains only its safe list identity while sibling observations remain
complete. Distinctive private sentinels prove raw bodies never enter debug output.

The coverage ledger is parsed as a strict public two-plane catalogue with 142 input-observation and
50 output-intent rows (192 total). Its unit tests reject schema,
identifier, diagnostic, observation/planner/CLI/Libpod link, expected-row, and plausible
target-availability swap mutations. Inventory tests prove that unmodeled `HostConfig` members
become bounded unsupported metadata, `Secret.Spec.Driver` does not become unknown metadata, and
overflow or an incomplete observation makes `ObservationHeader::unmodelled_completeness()`
incomplete. M7-A coverage exercises every `ObservationField` state through public acquisition,
kind-safe detail access, malformed label localization for all resource kinds, typed section/resource
availability, coalesced secret ID/name grants with dual graph evidence, configured-versus-local
image conflicts, and protected-value redaction in debug and snapshots.

M7-B1 adds table-driven container core configuration, mount, and secret-grant decoding coverage,
including absent, malformed-member, local-resolution, and snapshot-redaction boundaries. Volume owner coverage
distinguishes omitted wire IDs from literal zero and rejects null, negative, non-numeric, and
out-of-range IDs.

M7-B2a covers typed effective network IPAM subnet, gateway, lease-range, route, metric, and
route-type observations across the reviewed version boundary. Lease-range endpoints are
independently optional; tests cover start-only, end-only, empty, outside-CIDR, malformed, and
complete reversed ranges. Network-subnet CIDR tests retain reviewed host-bit spelling while
validating containment against normalized network bits. Generic decoding preserves CIDR wire syntax
defensively, but does not assert host-bit route validity: pinned Podman 5.4 evidence rejects such
route destinations. The pinned Podman 6.0 corpus covers IPv4 and IPv6 values, route-type defaults
and every 6.0 route type. Podman 5.x route-type evidence is version-inapplicable but static
unicast routes still require a gateway; unknown 6.x route types remain bounded unmodelled metadata.
Malformed members make only their complete subnet or route family malformed. Snapshot tests prove
topology values remain redacted.

M6-B3a tests cover public and redacted shell/direct health-command forms, timing and startup-health
dependency, logging driver/label rules, explicit security false values and conflicts, bounded CPU,
memory, PID, and rlimit controls, cgroup/root-context boundaries, pod-member runtime acceptance,
the namespace-only member boundary, all-seven reviewed-target matrices for journald labels and
unlimited rlimits, CPU-quota boundaries, and renderer blocking.
M6-B3b adds exact CLI/Libpod assertions for public health and runtime resource fields across every
reviewed target, every health failure action's distinct CLI/native-integer form, and all sensitive
or external health command variants in both normal and startup positions. The strict renderer
catalogue records and mutation-tests the Libpod member value shape, not only its spelling.

M6-B4 tests cover every typed mount form and access mode, normalized and rejected subpaths,
subpath-with-`NoCopy` rejection, duplicate destinations, individually absent/zero/maximum volume
UID/GID, all four image pull policies, five-version exact output matrices, 5.4/5.5 policy blocks,
and manual local/unqualified/tagless image portability findings. Secret grants cover mount and
environment targets, optional UID/GID/mode, duplicate target rejection, wrong/missing source
identity, and distinctive payload/reference redaction in debug and rendered artifacts. Pod infra
mounts always produce sorted `PLN0046` findings and no partial artifact.

M3 discovery tests use fixture transports only. They cover all six exact resource-root kinds;
label-presence, label-value, empty-value, and rejected selectors; dependent-to-prerequisite closure;
cycle-free pod membership; shared prerequisites; explicit shared roots; exact network name-or-ID
crossings; unused and ambiguous overrides; deterministic `all` seeds and selector permutations;
accepted, incomplete, conflicting, and config-hash Compose ownership evidence; relationship
ambiguity; unrelated-evidence filtering; debug redaction; and complete explanation coverage.
Fixtures also prove that `network.internal` never creates grouping evidence or authorizes reverse
traversal.

M4 snapshot tests compare exact inventory and graph goldens, validate both against the strict Draft
2020-12 schema, reject missing/extra/wrong-type mutations, and inject distinctive environment,
secret, label, Compose, and driver-option values to prove none serialize. The fixed corpus covers
sanitized rootless 5.4 and rootful 6.1 inventories, all six resource list/inspect failure families,
non-atomic races, malformed environments, unexpected secret payload fields, unknown-field bounds,
ambiguous aliases, shared boundaries, exact name/ID crossings, ownership conflicts, `all` seeding,
and dependency cycles. Manifest tests verify immutable source provenance and SHA-256 for every
artifact. Fixed list and selector permutations must produce byte-identical snapshots. No corpus
test discovers or contacts a live Podman service, and no fuzzing infrastructure is required.

M7 retains the revision-pinned bounded all-six-resource-kind Podman 6.1 stream used by the
downstream golden. M8's request-aware matrix proves every section is available and complete for
Podman 5.4.0 through 6.1.0 in both simulated contexts. The manifest contains those 14 cassettes and
seven focused regression or golden artifacts; superseded 5.7 and 6.0 BoxFerry streams are removed.
`tests/boxferry_adapter.rs` is a public-only downstream consumer. Its 6.1 scenario covers
acquisition, discovery, typed observation/origin decisions, a neutral application projection,
deployment-intent construction, planning, and deterministic CLI/Libpod rendering against
`fixtures/corpus/boxferry-adapter-6.1.expected.json`.

M5 deployment tests use no transport. They cover every typed resource kind, explicit external
preconditions, all reviewed target versions, strict managed image sources, migration-safe image
policy, deterministic resource permutations, shared-prerequisite deduplication, pod membership,
pod versus unpodded starts, cross-pod startup lifting, same-pod ordering rejection, duplicate and
conflicting declarations, missing prerequisites, cycles, and protected secret references. The
public API test compiles this contract as an external consumer. It also proves that managed
operation intent and all deterministic external preconditions survive planning, while plan `Debug`
output never exposes an external secret-material reference.

M6 renderer tests cover every evidence-listed operation over every reviewed Podman release. The v5
catalogue records immutable CLI, model, and handler sources for every rendered setting as well as
the operation route/body evidence. Parser mutation tests reject omitted, duplicate, unknown,
wrong-claim, mutable-source, and substituted-release evidence. Byte-exact CLI/API and artifact goldens prove the emitted argv
semantics that a JSON Schema cannot express; the local Draft 2020-12 schema validates operation
kind/action/method/path/body/input-flag pairings and the explicit output-connection field. Tests
reject unlisted, build-metadata, and non-identical API/engine targets; prove that external network,
volume, image, and secret prerequisites are safely disclosed in the review script; and prove the
sensitive external-input reference sentinel never reaches an artifact. They also prove pod and
unpodded network topology, typed named-volume mounts, and bounded public settings are emitted
exactly. A table-driven matrix proves the CLI and Libpod JSON spelling of all four restart policies
for every reviewed target. Tests exercise valid and invalid boundaries,
ordering, duplicate and conflicting settings, every restart policy, public-consumer compilation,
and sentinel redaction for inline and external environment values. They use no live connection and
never execute generated commands.

M6-B2 tests cover typed aliases, static addresses, MAC addresses, port mappings, DNS, host aliases,
IPAM subnets, routes, address-family boundaries, CIDR containment, duplicate setter state
preservation, and explicit rootful/rootless/unknown planning. Byte-exact CLI and Libpod tests cover
all reviewed releases, IPv4/IPv6, TCP/UDP/SCTP, deterministic order, pod versus unpodded ownership,
6.0 target boundaries, and strict per-release Podman/common-module catalogue mutations. These tests
use no live connection and never run generated commands.

M7-B2b tests use only pinned or synthesized offline inspect responses. They prove that a pod's
`InfraConfig` is authoritative over disagreeing member runtime values; `CreateInfra` and
`InfraConfig` inconsistencies fail closed; unpodded `HostConfig` values retain configured origin;
and resolver/hosts gates suppress their dependent fields. `CreateNetNS` is preserved separately,
while port bindings remain validated when that gate is absent or false. Explicit 5.8.0, 5.8.6,
5.8.7, and 6.0.0 cases prove deprecated `StaticIP` covers every supported 5.x patch while
`StaticMAC` remains permanently inapplicable.
Snapshot tests assert state,
origin, and counts only, never addresses, host entries, DNS strings, port values, network names,
or opaque option data.

M7-B3a tests use pinned or synthesized offline container inspect responses. They cover effective
restart, normal/startup health (including startup `StartPeriod`), failure action, and logging
observations; protected-command callback access; debug and snapshot redaction; all malformed
health command table shapes; and negative restart/retry/success count values. Unknown restart,
logging, action, and health enum spellings must remain field-local unmodelled metadata with
`PLN0023`. The external-consumer test compiles every public accessor and handles the
non-exhaustive health-command enum.

M7-B3b offline tests cover effective security, namespace, and resource controls on a pod member;
capability order and duplicates; opaque security-option redaction; zero and `-1` preservation; and
native ulimit order. Table-driven negative cases cover every ledger-linked scalar, collection, and
ulimit member. Any malformed ulimit member poisons the collection. Unknown capabilities and future
namespace modes remain bounded `PLN0023` metadata. The private-default case covers empty PID, IPC,
and UTS modes while keeping empty cgroup mode malformed. Snapshot and external-consumer tests cover
the new state/origin/count-only contract without reusing deployment runtime types.

M7-B4 offline tests cover every new image, volume, secret, and nested driver field with positive
provenance and malformed-wire cases. Timestamp tests cover leap dates, leap seconds, fractional
precision, offsets, and invalid calendar/time boundaries. Snapshots retain image reference counts,
field state/origin, and secret option count only; distinctive option names and values must never
reach debug output or serialized snapshots.
