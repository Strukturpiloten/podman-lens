# 0010: Versioned non-executing output rendering

- Status: Accepted
- Date: 2026-08-20

## Decision

M6-A renders the evidence-backed basic deployment topology into deterministic CLI argument arrays,
versioned Libpod request descriptions, a serialization-only JSON deployment artifact, and a POSIX
review script. A target is accepted only when its normalized, identical engine/API version appears
in the committed renderer evidence. That evidence records the immutable CLI and Libpod route source
for each operation at every reviewed release, plus the body-decoding source when a body exists. The
optional non-sensitive output connection survives into the rendering and JSON export. The review
script safely names every external prerequisite in deterministic comments but never exposes secret
material or its external reference. A deployment artifact may contain only explicitly
caller-authorized public declared values; it never contains a sensitive value or sensitive-input
reference. Neither the renderer nor the script generator opens a connection
or executes Podman.

Pod networks, pod-member container assignment, and unpodded-container networks are exact. M6-B1a
adds bounded, typed named-volume mounts (source, normalized destination, read-only, copy mode) for
containers and explicitly named infra-container mounts for pods, plus
container command, entrypoint, user, workdir, hostname, labels, environment, and restart policy to
the semantic plan. They remain deliberately unrendered until per-field CLI/Libpod evidence proves
their exact target spelling. Any populated field produces `PLN0046`, never a partially configured
rendering. Secret attachment targets and related options remain unmodelled; secret bytes remain an
explicit external input requirement.

## Consequences

- Committed renderer evidence covers eight semantic operation categories across every reviewed line
  with per-operation immutable source provenance.
- M6-B1a establishes the typed settings foundation and rejects all unrendered values uniformly.
- Later M6-B work adds per-field historical evidence and exact renderings, plus secret attachment
  targets and other still-unmodelled native settings.
