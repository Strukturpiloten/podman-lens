# 0002: Ordered transport-neutral deployment plans

- Status: Accepted
- Date: 2026-08-19

## Context

Users need copyable Podman commands, while applications need a structured API representation.
Making shell arguments the canonical plan would prevent safe native API use.

## Decision

A deployment plan is a versioned, ordered list of semantic resource operations. Each operation has a
stable identifier, action, complete typed managed-resource intent, and dependencies. The plan also
retains its exact deterministic external preconditions. M5 establishes that semantic source of
truth. M6 adds its exact representations for the selected target:

- CLI representation as a program and argument array;
- native Libpod representation as an HTTP method, versioned path, and typed body.

Array order is authoritative. Dependencies explain and validate that order. A shell script is
rendered from the same CLI representation and cannot diverge from the JSON plan.

Sequentially processing the complete array must always be valid. Dependency metadata may enable
safe parallel execution, but a consumer does not need a graph scheduler.

## Consequences

- Consumers do not parse shell strings.
- CLI and API equivalence can be tested per semantic operation and Podman version once M6 exists.
- A missing or approximate representation produces a structured outcome.
- Sensitive payloads remain external references rather than serialized request bodies.
