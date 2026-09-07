# 0015: Bounded creation-command evidence

- Status: Accepted
- Date: 2026-09-07

## Context

`Config.CreateCommand` can contain useful authored image and SELinux-relabel
evidence, as well as protected environment assignments, secret references,
paths, and post-image payloads.

## Decision

PodmanLens decodes a closed `podman create`/`podman run` grammar transiently.
It retains only configured-origin closed image consistency outcomes and typed
inspect-mount indices for `z`/`Z`. Image and mount-relabel projections have
independent observation states: missing typed image evidence cannot erase a
correlated relabel hint, and unavailable mount correlation cannot erase a valid
image hint. Image evidence is contradictory only when both the configured image
spelling and local resolved image identifier are available and the transient
operand differs from both. Unknown option arity, malformed values, and ambiguous
correlations remain explicit observation states. Conflict findings are value-free.

No raw command component, image spelling, path, environment assignment, secret
reference, or post-image payload reaches the public API, debug output, or
snapshot. This evidence does not prove pull, build, runtime, or lifecycle
history.
