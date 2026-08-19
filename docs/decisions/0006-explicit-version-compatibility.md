# 0006: Compatibility is explicit and evidence-backed

- Status: Accepted
- Date: 2026-08-19

## Context

Podman CLI flags, Libpod fields, and resource behavior change across releases. The installed
development version is not a valid target-selection policy.

## Decision

Input observations record the reported Podman and API versions. Output planning requires an
explicit target profile. Versioned capability evidence governs both CLI and Libpod representations.

A version is supported only after reviewed specifications or runtime responses and positive and
negative boundary tests are committed. Unknown versions fail closed or require an explicit caller
policy; they are never treated as the development machine's version.

## Consequences

- Compatibility claims are auditable.
- CLI and API plans cannot silently use newer fields.
- Current-patch runtime conformance supplements deterministic offline fixtures.
- New Podman releases require evidence updates rather than only a number change.
