# 0001: Independent native Podman library

- Status: Accepted
- Date: 2026-08-19

## Context

Native Podman acquisition, version behavior, and deployment planning are too large and specialized
to implement inside BoxFerry. Parsing command output would also couple correctness to human-facing
CLI formatting.

## Decision

PodmanLens is an independent Rust library with no dependency on BoxFerry. It uses the native Libpod
REST API through a replaceable transport for local sockets and explicit remote connections.

The Docker-compatible API is not canonical because it lacks complete Podman-native semantics.
Podman CLI commands are generated output representations, not the input protocol. The initial
library creates plans but never executes them.

## Consequences

- BoxFerry maps between its neutral model and PodmanLens public types.
- Tests can replace the transport with reviewed offline responses.
- Native version differences remain isolated from BoxFerry.
- Plan execution requires a separate future decision and is never implicit.
