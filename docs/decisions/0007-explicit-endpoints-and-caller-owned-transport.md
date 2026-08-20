# 0007: Explicit endpoints and read-only acquisition transport

- Status: Accepted
- Date: 2026-08-20

## Context

Podman connections can contain host names, Unix paths, host-key locations, authentication
references, certificate references, response headers, and payloads. Ambient Podman configuration
would make the selected source invisible to callers and tests. Selecting an unbounded or mutating
client would make an acquisition library capable of changing a runtime merely by probing it.
Callers still need a replaceable contract for SSH and mutual-TLS connections.

## Decision

PodmanLens accepts only explicit connection specifications:

- absolute, lexically safe Unix socket paths;
- SSH host, port, user, absolute remote socket path, verified host-key reference, and opaque
  authentication reference; or
- `tcp://host:port` with an explicit mandatory mutual-TLS policy containing CA, client
  certificate, and private-key references.

Plaintext TCP has no representable type. Connection-sensitive data is redacted from `Debug`,
`Display`, and structured errors.

`LibpodTransport` is an object-safe boxed-future trait. PodmanLens defines bounded request and
response messages, caller-visible limits, and duplicate-preserving headers. Callers may implement
their own Unix, SSH, or mutual-TLS transport.

PodmanLens also supplies `ReadOnlyUnixTransport` for explicit local Unix acquisition. It uses
Tokio and Hyper's HTTP/1.1 framing, accepts only `GET`, and rejects every other represented method
before opening the socket. Requests must be bodyless and remain within the transport's header
bounds; callers cannot override `Host`. The configurable parser ceiling has Hyper's documented
8 KiB minimum. The implementation is available only on Unix targets. It has explicit non-zero
connect, response-header, response-body, and total deadlines; does not discover endpoints, follow
redirects, decompress, retry, spawn detached tasks, execute plans, or implement SSH/TLS.
PodmanLens supplies no mutating executor.

## Consequences

- Tests and integrations name their endpoint and security-material references explicitly.
- Fixed M1–M4 acquisition can probe an explicitly selected local socket without permitting a
  library-supplied mutation path.
- Future remote clients remain caller-provided without replacing the public transport contract.
- Probe decoding can detect conflicting response headers instead of losing evidence in a map.
- Formatting, diagnostics, and snapshots do not expose endpoint details or payloads by default.
