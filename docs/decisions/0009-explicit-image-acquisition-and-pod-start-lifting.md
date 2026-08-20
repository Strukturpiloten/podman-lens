# 0009: Explicit image acquisition and pod-start lifting

- Status: Accepted
- Date: 2026-08-20

## Context

Image acquisition and container startup can look implicit in a command-oriented plan. That would
hide mutable image behavior and make a requested startup order inside a Podman pod impossible to
represent faithfully.

## Decision

Managed images use one bounded portable pull reference—host-qualified lower-case registry and
repository with a tag or `sha256` digest—and an explicit `ImagePullPolicy`. The initial policy is
`Missing`: image acquisition is a separate semantic operation and container creation does not imply
a pull. A network, volume, image, or secret needed but deliberately managed elsewhere is represented
by an explicit `ExternalPrecondition`; a missing declaration is always a finding. Pods and containers
remain managed in M5 because their lifecycle and membership cannot be represented as a precondition.

A pod with declared members receives exactly one `StartPod` after its member create operations.
An unpodded container receives `StartContainer`. An intent-level container startup edge is lifted
to its start-operation anchors. An edge between two members of the same pod is rejected, because a
Podman pod starts those members as one native operation. Cycles are rejected before M6 rendering.
Each managed operation retains its typed source intent and the plan retains deterministically
ordered external preconditions. A secret operation contains only its redacted external material
reference, never secret bytes.

## Consequences

- M5 plans explain image and startup ordering without containing CLI or HTTP syntax.
- M6 can render `podman` and Libpod forms from one validated source of truth.
- An M6 renderer does not need to reconstruct semantic input or guess external requirements from
  operation identifiers.
- A plan never guesses an externally created resource or silently turns container creation into a
  mutable image pull.
