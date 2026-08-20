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

The opt-in current-patch probe test is deliberately not part of that gate. It only permits the
fixed read-only acquisition probe. Run it explicitly against a selected local socket with:

```shell
PODMAN_LENS_CONFORMANCE_UNIX_SOCKET=/absolute/podman.sock \
PODMAN_LENS_CONFORMANCE_EXPECTED_VERSION=6.1.0 \
cargo test --test current_patch_conformance -- --ignored
```

The expected version must be exactly `5.8.6` or `6.1.0`; the test never discovers a connection,
retries, or sends a mutating Libpod request.

The built-in Unix transport tests prove that mutation methods, body-bearing acquisition requests,
caller-supplied `Host`, and over-limit requests fail before a socket is opened. The Unix-only module
is conditionally compiled so platform-neutral connection and caller-provided transport contracts
remain usable on non-Unix targets.

M2 inventory tests use fixture transports only. They cover the Podman 5.4 and 6.1 evidence
boundaries; probe-before-list ordering; canonical list sorting; exact list queries; escaped image
inspect paths; partial races; malformed and duplicate list records; secret-metadata-only requests;
unknown-field metadata; and both environment retention policies. No ordinary test contacts a live
Podman service or contains a real secret or environment value.

The native-field coverage ledger is parsed as a strict public catalogue. Its unit tests reject
schema, identifier, diagnostic, decoder-reference, planner/renderer applicability, and expected-row
mutations. Inventory tests prove that unmodeled `HostConfig` members become bounded unsupported
metadata, `Secret.Spec.Driver` does not become unknown metadata, and overflow or partial inspection
makes `ResourceRecord::unknown_fields_complete()` false.

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
