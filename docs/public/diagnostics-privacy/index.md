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
observational snapshots, and deployment artifacts:

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
a public target value before it can appear in a deployment artifact.

## Snapshot versus deployment artifact

`snapshot::v1` is serialization-only, always redacted, and represents observed inventory or graph
evidence. It can still expose resource names, IDs, native field paths, and evidence URLs, so it is
redacted rather than anonymous.

`artifact::deployment_v1` represents caller-authorized desired output. It never deserializes as an
inventory and never contains sensitive input references. Review both forms before sharing them.
