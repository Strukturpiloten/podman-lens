# 0005: Sensitive values require explicit authorization

- Status: Accepted
- Date: 2026-08-19

## Context

Podman can expose secret payloads and configured environment values. Runtime values may be
sensitive even when their names do not identify them as secrets.

## Decision

Secret inspection returns metadata by default. Payload acquisition requires explicit caller
authorization and produces an opaque sensitive value type. Runtime environment values are redacted
by default; callers must explicitly request their inclusion.

Sensitive values never appear in diagnostics, logs, observational snapshots, deployment artifacts,
`Debug` or `Display` output, or serialized deployment plans. Plans refer to external sensitive
inputs. Any future file export is an explicit unsafe mode and writes restricted files. Base64 is not
treated as protection. A value in a deployment artifact must have been explicitly declared public
by the caller; no observed sensitive value can be converted into that public contract.

## Consequences

- Default inspection is safe to report and serialize.
- A redacted conversion retains names and emits a structured incompleteness outcome.
- Faithful sensitive migration remains possible through an explicit trust boundary.
- CLI and API renderers apply the same redaction policy.
