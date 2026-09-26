# 0005: Sensitive values require explicit authorization

- Status: Accepted; rendered-output restriction amended by [0018](0018-explicit-protected-inline-render-authorization.md)
- Date: 2026-08-19

## Context

Podman can expose secret payloads and configured environment values. Runtime values may be
sensitive even when their names do not identify them as secrets.

## Decision

Secret inspection returns metadata by default. Payload acquisition requires explicit caller
authorization and produces an opaque sensitive value type. Runtime environment values are redacted
by default; callers must explicitly request their inclusion.

Sensitive values never appear in diagnostics, logs, observational snapshots, `Debug` or `Display`
output, or serialized deployment plans. Plans refer to external sensitive inputs. Base64 is not
treated as protection. A caller-declared protected inline environment value can enter inert
deployment output only through the separate render-time authorization in decision 0018; the
ordinary public-value constructor must never be used as a cast for observed sensitive values.

## Consequences

- Default inspection is safe to report and serialize.
- A redacted conversion retains names and emits a structured incompleteness outcome.
- Faithful sensitive migration remains possible through an explicit trust boundary.
- CLI and API renderers apply the same redaction policy.
