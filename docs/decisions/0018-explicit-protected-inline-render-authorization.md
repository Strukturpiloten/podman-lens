# 0018: Explicit protected inline render authorization

- Status: Accepted
- Date: 2026-09-26

## Context

The native plan distinguishes public, protected inline, and unresolved external environment
values. The ordinary renderer rejects protected inline values. A caller may need to place a
caller-supplied protected value in generated Podman output, while the public-value constructor
exposes its string in `Debug` and cannot safely serve as a declassification cast.

## Decision

Keep `render_deployment` fail-closed. A second rendering API requires an explicit
`SensitiveInlineRenderAuthorization` value at the render call. The grant covers every protected
inline environment value in that plan and only that call; it does not alter intent, observation,
planning, or a later render call. It does not resolve `External` environment values. Such values
continue to block the complete output with redacted, actionable findings.

Authorized protected inline values may appear only in inert CLI argument arrays, Libpod JSON
request descriptions, the review script, and version 2 serialized deployment artifacts. The
original `deployment_v1` contract remains public-only: serialization of a protected rendering
fails before writing bytes. `deployment_v2` uses `schema_version: 2` and a required
`contains_protected_inline_environment` boolean derived from values actually rendered, not from
authorization alone. These bytes are sensitive when the indicator is true, and callers must
control their distribution. `Debug`, diagnostics, findings, and validation output omit the values.
No renderer opens a connection, writes a file, executes a command, or deploys a resource.
Observation snapshots and serialized plans stay redacted.

This amends the deployment-output restrictions in decisions
[0005](0005-sensitive-values-require-authorization.md) and
[0010](0010-versioned-nonexecuting-output-rendering.md). It does not authorize constructing a
`PublicEnvironmentValue` from a protected or observed value.

## Consequences

- Existing callers retain rejection by default.
- Explicitly authorized renderings and v2 serialized artifacts require sensitive handling when
  protected values are present, even though their `Debug` output is redacted.
- Version and execution-mode evidence remains the same; this is a value-handling boundary, not a
  new native capability claim.
