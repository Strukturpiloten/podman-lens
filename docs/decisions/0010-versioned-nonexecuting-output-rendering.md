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
the semantic plan. M6-B1b records revision-pinned CLI, model, and Libpod handler evidence for every
exact emitted setting on every reviewed line before rendering becomes exact. The current v5 catalogue
retains that B1 evidence and adds M6-B2 availability plus cross-repository common-module claims. It
renders only caller-declared public label and environment values; sensitive inline and external
environment variants block the entire artifact with static, redacted findings. Secret attachment
targets and related options remain unmodelled; secret bytes remain an explicit external input
requirement.

M6-B2 extends the same non-executing rendering boundary with typed declared networking. Pod
attachments, ports, DNS, and host aliases render on the infra container; unpodded containers render
them directly. Pod members cannot declare network namespace configuration. Static IPv4, IPv6, and
MAC declarations require a caller-proven rootful target during planning. Exact attachment options,
IPAM subnets, and routes have per-release Podman and common-module evidence. Explicit container
network order and non-unicast routes are gated to Podman 6.0 and newer. This contract deliberately
does not infer runtime addresses, invent a pod network ordering API, or model multi-IP attachments,
port ranges, interface names, arbitrary network drivers/options, or unmanaged namespace modes.

M6-B3b renders bounded public health, logging, security, CPU/memory/PID, and rlimit settings for
containers, including pod members; the namespace subset is limited to unpodded containers. Health
command arrays are compact canonical JSON for the CLI and `healthconfig`/`startupHealthConfig`
bodies for Libpod; disabled health uses `--no-healthcheck` and
`Test: [NONE]`. Health failure actions use CLI names and Podman's native integer Libpod enum.
Sensitive or externally supplied health commands block the affected resource with a redacted
finding and no partial artifact. Journald labels are exact only from 6.0 and unlimited
rlimits only from 5.6; rendering repeats those planner gates. Pod-member namespace settings remain
rejected rather than moved to a pod.

## Consequences

- Committed renderer evidence covers eight semantic operation categories and the M6-B1b field matrix
  across every reviewed line with immutable source provenance.
- M6-B1a establishes the typed settings foundation; M6-B1b renders its bounded public subset
  exactly while preserving redacted all-or-nothing sensitive handling.
- Later M6-B work adds per-field historical evidence and exact renderings, plus secret attachment
  targets and other still-unmodelled native settings.
