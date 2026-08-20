# 0008: Versioned snapshots are serialization-only and always redacted

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
needed to diagnose acquisition and grouping decisions.

An incompatible shape change requires a new versioned module and schema. A schema version never
silently changes meaning.

## Consequences

- Callers can create deterministic reports and support bundles without depending on private Libpod
  response types.
- Snapshot redaction is independent of acquisition policy and covered by distinctive leak tests.
- Snapshots still contain operational metadata such as resource identities, image aliases,
  environment variable names, subnets, and evidence URLs. They are redacted, not anonymous.
- Replaying or importing snapshots is outside the initial contract and can be designed separately
  only if a concrete need appears.
