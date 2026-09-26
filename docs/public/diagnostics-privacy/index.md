# Handle diagnostics and protected data

PodmanLens reports bounded structured findings throughout acquisition, discovery, planning, and
rendering. A diagnostic code identifies a stable condition; resource identity, field path,
occurrence, count, and version evidence add safe context where available.

Representative boundaries include:

- `PLN0018`: an unexpected secret payload field was discarded;
- `PLN0023`: native evidence is retained as unmodelled rather than silently treated as supported;
- `PLN0033`: a requested network-boundary override was unused;
- `PLN0046`: populated target intent has no exact renderer for the selected target.

Malformed input stays local when possible. An invalid member does not become ordinary absence, and
an unavailable section does not hide unrelated resource kinds. Planning and rendering are
all-or-nothing when findings block a complete result.

## Privacy boundaries

PodmanLens excludes these observed or protected values from diagnostics, `Debug`, `Display`,
and observational snapshots. Deployment artifacts exclude them by default:

- actual connection endpoints, credentials, certificates, host keys, and opaque authentication
  values;
- runtime environment values unless held behind an explicitly authorized non-serializing wrapper;
- protected health-command arguments;
- secret payloads and secret driver option names or values;
- raw unknown JSON, observed native label values, Compose ownership values, and host-local unknown
  values.

Container-mount `Debug` output reports field state and option counts rather than source,
destination, backing-path, propagation, or subpath values. SELinux relabel decoding retains only
the closed `Shared` (`z`) or `Private` (`Z`) choice; raw `HostConfig.Binds` strings remain outside
the public API and snapshots.

A caller-selected non-sensitive connection name may remain as provenance. Public target labels
authored explicitly by the caller are serialized into deployment artifacts by design. Secret
payload endpoints are never requested. Base64 is not protection. A caller must explicitly construct
a public target value before it can appear in a deployment artifact by the ordinary renderer.

Never cast a protected value through `PublicEnvironmentValue`: use
`render_deployment_with_authorization` and `SensitiveInlineRenderAuthorization` at the render call.
Its output bytes can contain protected values, so treat the CLI arguments, Libpod JSON, review
script, and serialized v2 artifact as sensitive. The original v1 artifact fails serialization
before writing protected bytes. V2 exposes `contains_protected_inline_environment` so consumers
can identify sensitive output. Their `Debug` views remain redacted. External
environment values remain unresolved and block the complete artifact.

## Snapshot versus deployment artifact

`snapshot::v1` is serialization-only, always redacted, and represents observed inventory or graph
evidence. It can still expose resource names, IDs, native field paths, and evidence URLs, so it is
redacted rather than anonymous.

`artifact::deployment_v1` retains the public-only desired-output contract. V2 represents
caller-authorized protected inline output with an explicit indicator. Neither deserializes as an
inventory or contains external sensitive input references. Review both forms before sharing them.

Bounded creation evidence exposes only closed consistency results and typed mount indices, never
the command or its arguments. Its image and mount-relabel states are independent; unavailable
correlation is reported as state rather than by retaining the compared native value. `PLN0050`
reports a conflict only when the creation evidence and typed inspect evidence are both observed
and disagree; missing, unavailable, or malformed typed evidence is not a contradiction.
