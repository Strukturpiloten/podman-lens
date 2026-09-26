# 0008: Versioned observational snapshots are serialization-only and always redacted

- Status: Accepted
- Date: 2026-08-20

## Context

BoxFerry, support tools, and offline tests need a deterministic JSON representation of acquired
inventory and discovered graphs. Serializing native Rust types directly would expose implementation
details and could reveal protected runtime values. Accepting snapshots as input would create a
second untrusted parsing and compatibility boundary that the first release does not need.

## Decision

PodmanLens exposes `snapshot::v1` inventory and graph data-transfer objects with
`schema_version: 1`. They implement serialization only. PodmanLens does not deserialize snapshots.
The exact shape is governed by a committed Draft 2020-12 JSON Schema and golden fixtures.

Snapshots are always redacted, even when acquisition retained environment values in memory. They
exclude environment values, secret payloads, connection details, raw unknown JSON, label values,
driver-option values, and Compose ownership values. They retain safe structural and evidence data
needed to diagnose acquisition and grouping decisions. Deployment rendering is a distinct output
contract in `artifact::deployment_v1`, not a snapshot. Decision
[0018](0018-explicit-protected-inline-render-authorization.md) retains v1 as public-only and
permits protected inline environment values only in v2 after a distinct render-time grant;
external sensitive-input references remain excluded.
Its optional connection field is only `null` or a validated 1–64-byte ASCII Podman connection name:
an ASCII alphanumeric first character followed only by ASCII alphanumeric characters, dots,
underscores, or hyphens. It can never represent a URI, endpoint, socket path, credential, token,
whitespace, colon, slash, backslash, or `@` detail.

After the first PodmanLens release, an incompatible shape change requires a new versioned module
and schema. A published schema version never silently changes meaning. In particular, the
historical v1 service `target_*` fields mirror the engine/API profile observed during acquisition;
they never describe a caller-selected deployment or migration target.

## Consequences

- Callers can create deterministic reports and support bundles without depending on private Libpod
  response types.
- Snapshot redaction is independent of acquisition policy and covered by distinctive leak tests.
- Deployment artifacts preserve only a safe connection selector, never a connection endpoint,
  credential detail, or sensitive-input reference. Protected inline environment values require
  the separate render-time grant from decision 0018.
- Snapshots retain resource identities, environment variable names, evidence URLs, and source
  field paths. Image aliases and network subnets are intentionally represented only by counts:
  their spellings can reveal private registry, topology, or addressing information. This
  deliberately supersedes the earlier retain-values wording; snapshots are redacted, not
  anonymous.
- Replaying or importing snapshots is outside the initial contract and can be designed separately
  only if a concrete need appears.
