# 0016: Captured native evidence is a test-only release contract

- Status: Accepted
- Date: 2026-09-07

## Context

Source-derived synthetic cassettes prove broad version and context behavior, but they cannot alone
show that current production acquisition decodes responses emitted by a native Podman service.
Running a live container matrix in every PodmanLens pull request or release would add privileged,
networked and nondeterministic release dependencies. BoxFerry already owns the maintained,
digest-pinned live version and root-mode matrix.

Native captures can also contain host paths, endpoint details, identifiers, timestamps, addresses,
environment values and other operational data. A raw capture is therefore unsuitable as a committed
test fixture.

## Decision

PodmanLens admits selected native responses only as bounded, privacy-reviewed, deterministic test
fixtures. Each captured cassette:

- records exact Podman source or package evidence, the digest-pinned BoxFerry runtime matrix cell,
  capture-manifest digest and capture-tool digests;
- distinguishes a CLI-only session from a mixed-origin session containing both CLI-created and
  Compose-provider-created resources;
- replaces identifiers, names, paths, environment values, timestamps, request references and
  network addresses consistently while retaining JSON shape, null-versus-absent distinctions,
  array order, response status and request order;
- rejects secret payload material and replays only bodyless `GET` requests through the production
  `acquire_inventory` path;
- is registered by exact SHA-256 in `fixtures/native-regressions/manifest.json`.

`podman-lens-cassette-v1` and the capture manifest are repository test formats. They are not public
Rust APIs, trusted-input deserialization APIs, migration interchange formats or compatibility
promises. They may be packaged for auditability, but production library modules cannot import or
parse them. Test support owns schema validation and strict request consumption.

`scripts/check-native-release-contract.sh` is an offline local, CI and release gate. An ignored
integration-test entry point accepts only an explicitly supplied Unix socket and an exact reviewed
version so BoxFerry can exercise this release contract from its maintained live harness. The
ignored test never discovers an ambient endpoint and the ordinary gate never starts Podman.

## Consequences

A release is blocked when captured fixture hashes, privacy invariants, schema strictness, production
replay expectations or workflow wiring drift. Capturing an additional runtime requires a new
privacy review and immutable provenance; editing a captured artifact in place requires updating its
manifest entry and focused semantic assertions.

PodmanLens gains a native regression anchor without duplicating BoxFerry's live matrix. The
committed evidence remains finite and cannot prove every distribution patch, host configuration or
future Podman response.
