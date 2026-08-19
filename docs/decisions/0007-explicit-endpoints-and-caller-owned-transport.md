# 0007: Explicit endpoints and caller-owned transport

- Status: Accepted
- Date: 2026-08-20

## Context

Podman connections can contain host names, Unix paths, host-key locations, authentication
references, certificate references, response headers, and payloads. Ambient Podman configuration
would make the selected source invisible to callers and tests. Selecting an HTTP, SSH, TLS, or
async-runtime stack in the first public contract would make a later transport choice needlessly
breaking.

## Decision

PodmanLens accepts only explicit connection specifications:

- absolute, lexically safe Unix socket paths;
- SSH host, port, user, absolute remote socket path, verified host-key reference, and opaque
  authentication reference; or
- `tcp://host:port` with an explicit mandatory mutual-TLS policy containing CA, client
  certificate, and private-key references.

Plaintext TCP has no representable type. Connection-sensitive data is redacted from `Debug`,
`Display`, and structured errors.

`LibpodTransport` is an object-safe boxed-future trait implemented by the caller. PodmanLens
defines bounded request and response messages, caller-visible limits, and duplicate-preserving
headers, but provides no HTTP, Unix-socket, SSH, TLS, process, or async-runtime implementation.
The M1 catalogue uses `semver`, `serde`, `serde_json`, and `url`; no client dependency is selected.

## Consequences

- Tests and integrations name their endpoint and security-material references explicitly.
- Future Unix and remote clients can be added without replacing the public transport contract.
- Probe decoding can detect conflicting response headers instead of losing evidence in a map.
- Formatting, diagnostics, and snapshots do not expose endpoint details or payloads by default.
