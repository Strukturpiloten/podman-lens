# 0010: Versioned non-executing output rendering

- Status: Accepted
- Date: 2026-08-20

## Decision

M6-A renders the evidence-backed basic deployment topology into deterministic CLI argument arrays,
versioned Libpod request descriptions, a redacted serialization-only JSON snapshot, and a POSIX
review script. A target is accepted only when its normalized, identical engine/API version appears
in the committed renderer evidence. That evidence records the immutable CLI and Libpod route source
for each operation at every reviewed release, plus the body-decoding source when a body exists. The
optional non-sensitive output connection survives into the rendering and JSON export. The review
script safely names every external prerequisite in deterministic comments but never exposes secret
material or its external reference. Neither the renderer nor the script generator opens a connection
or executes Podman.

Pod networks, pod-member container assignment, and unpodded-container networks are exact. The
current semantic model does not retain mount targets, modes, secret targets, or related options, so
pod volumes and container volumes or secrets produce `PLN0046` findings instead of an `Exact`
rendering. Secret bytes remain an explicit external input requirement.

## Consequences

- Committed renderer evidence covers eight semantic operation categories across every reviewed line
  with per-operation immutable source provenance.
- M6-B must add the typed data and evidence needed for mounts and secret attachment rather than
  guessing their target spelling.
- M6-B adds broader settings and per-field historical evidence.
