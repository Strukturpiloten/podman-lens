# 0003: Evidence-backed resource discovery

- Status: Accepted
- Date: 2026-08-19

## Context

A selected Podman resource normally belongs to a larger application graph. Blind traversal across
every shared dependency can merge unrelated applications, while requiring a grouping file makes the
common case unnecessarily difficult.

## Decision

Every explicit container, pod, network, volume, image, secret, or label selector is a discovery
root. Exact resource selectors use an ID, name, or image alias. A distinct prefix selector matches
only native resource names that start with the literal prefix, in sorted identity order; it never
matches IDs or image aliases. Label selectors use either exact key presence or an exact key-value
pair. Glob, regular-expression, and wildcard syntax are rejected, so a caller cannot accidentally
select a wider topology. The result retains requested selectors and resolved roots; unresolved and
ambiguous selectors become structured findings. The default closure includes the root's complete
evidenced resource group.

Pod membership, native container dependencies, and reviewed ownership labels are strong grouping
evidence. Closures that overlap through that evidence merge. Overlap only through a shared
prerequisite does not merge groups.
Shared, externally owned, or ambiguous resources are included as prerequisites but stop reverse
traversal. Explicitly selecting a shared resource or authorizing an exact network name-or-ID
boundary crossing permits reverse traversal through it. No separate grouping file is part of the
initial design.

Disjoint closures remain ordered groups. `all` discovery uses the same rules and emits every shared
prerequisite once. The graph explains every included resource, stopped boundary, authorized
crossing, strong-evidence merge, and group-ordering decision. Ambiguous relationships and unused
boundary authorizations remain structured findings rather than guesses.

Compose ownership evidence is accepted when one complete, non-empty Docker or Podman
project/service namespace exists. If both namespaces exist, their project and service values must
agree. A configuration hash must belong to a complete namespace; when both hashes are present they
must agree. Incomplete, orphaned, empty, or conflicting aliases produce no grouping edge. An exact
network authorization must resolve to one network name or ID; unused, unresolved, and ambiguous
authorizations remain findings. Boundary authorization is intentionally exact because it permits
reverse traversal from a shared prerequisite to direct consumers.

Podman's `network.internal` property is connectivity evidence, not ownership evidence.

## Consequences

- The normal selector produces a useful complete graph without extra configuration.
- Ambiguity is visible as a diagnostic instead of being guessed.
- Exact command-line overrides cover exceptional shared-network topologies.
- Images and other widely shared prerequisites do not automatically connect unrelated groups.
