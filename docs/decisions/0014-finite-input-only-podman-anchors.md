# 0014: Finite input-only Podman anchors remain separate from output targets

- Status: Accepted
- Date: 2026-08-24

## Context

BoxFerry must migrate installations from distribution-patched Podman releases that predate
Quadlet. Treating every version that parses as an output target would invent rendering support and
would make an observed legacy runtime look like a deployment destination.

## Decision

PodmanLens keeps finite, source-pinned input anchors separate from reviewed `TargetProfile`
versions. Podman 3.0.1, 3.4.4, 4.3.1, 4.9.3, and 4.9.4 are input-only anchors. The service probe
records their immutable input capability; `TargetProfile` continues to accept only the reviewed
5.4 through 6.1 output range.

Input-only wire differences are decoded only when backed by the pinned anchor evidence. A known
absent resource endpoint is reported as version-inapplicable and is never requested. Unknown CNI
network configuration stays bounded unmodelled evidence; only reviewed bridge IPAM CIDRs and
routes are typed.

## Consequences

- A caller imports an old runtime then explicitly chooses a modern output target.
- New distribution package versions need a distinct evidence row and tests; no broad 3.x or 4.x
  compatibility claim is implied.
- `ServiceObservation` distinguishes input evidence from an optional output-compatible profile.
- Snapshot v1 remains observational. Its historical `target_podman` and `target_api` fields mirror
  the service profile observed during acquisition; they never describe a caller-selected migration
  target.
