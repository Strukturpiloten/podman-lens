# 0003: Evidence-backed resource discovery

- Status: Accepted
- Date: 2026-08-19

## Context

A selected Podman resource normally belongs to a larger application graph. Blind traversal across
every shared dependency can merge unrelated applications, while requiring a grouping file makes the
common case unnecessarily difficult.

## Decision

Every explicit container, pod, network, volume, image, secret, or label selector is a discovery
root. The default closure includes its complete evidenced resource group.

Pod membership and reviewed ownership labels are strong grouping evidence. Shared, externally
owned, or ambiguous resources are included as prerequisites but stop reverse traversal. Explicitly
selecting a boundary resource or authorizing an exact boundary crossing permits traversal through
it. No separate grouping file is part of the initial design.

Overlapping closures merge. Disjoint closures remain ordered groups. `all` discovery uses the same
rules and emits every shared prerequisite once.

Podman's internal-network property is connectivity evidence, not ownership evidence.

## Consequences

- The normal selector produces a useful complete graph without extra configuration.
- Ambiguity is visible as a diagnostic instead of being guessed.
- Exact command-line overrides cover exceptional shared-network topologies.
- Images and other widely shared prerequisites do not automatically connect unrelated groups.
