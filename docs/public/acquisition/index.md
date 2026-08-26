# Acquire one explicit Podman inventory

PodmanLens reads native Podman state only through a caller-selected `LibpodTransport`. Its built-in
`ReadOnlyUnixTransport` accepts one absolute Unix socket, permits only bodyless `GET` requests, and
rejects mutation methods before opening the socket.

Run the packaged example with the socket you chose:

```console
cargo run --example read_only_discovery -- /run/user/1000/podman/podman.sock
```

The example does not read Podman connection configuration, search conventional socket locations,
or invoke the `podman` executable. It uses `AcquisitionOptions::redacted()`, discovers all eligible
roots, and writes a redacted graph snapshot to standard output. A snapshot can still contain
operational resource names and identifiers, so review it before sharing.

## Acquisition sequence

`acquire_inventory` performs one fixed read-only sequence:

1. Probe `GET /libpod/_ping` and the version endpoint.
2. List containers, pods, networks, volumes, images, and secret metadata.
3. Inspect each stable listed identity once in deterministic order.
4. Reconcile typed relationships without requesting secret payloads.

A list failure marks only that resource section unavailable. A disappeared or malformed inspect
response remains a partial `ResourceObservation`; unrelated observations stay available. Every
modeled field is an `ObservationField` whose state and, when observed, origin distinguish
configured, effective, runtime-assigned, and local-resolution evidence.

Do not promote effective, runtime-assigned, or local-resolution evidence into portable desired
state automatically. That decision belongs to the caller's mapping policy.

## Bounded native evidence

PodmanLens retains field paths and JSON value kinds for genuinely unmodelled native members, never
their raw values. While reading one resource kind, the inventory-wide descriptor budget reserves
one slot for every later non-empty kind. Unused reservations carry forward, so later kinds still
receive diagnostic evidence without reducing ordinary per-resource coverage.

Closed runtime projections such as process state, local storage paths, and effective capability
summaries are listed in the strict native-field ledger and discarded. They neither consume the
unmodelled budget nor masquerade as authored intent. Configured fields that still need a typed
contract—including network driver/IPAM settings and volume options—remain explicit unmodelled
evidence.

For mounts, the case-sensitive SELinux relabel choices `z` and `Z` are typed configured evidence.
The decoder prefers `Mounts[].Mode` and uses only a correlated relabel token from
`HostConfig.Binds` as a fallback. It does not retain a raw bind string or creation command.

## Protected values

The default acquisition policy retains environment names and order but redacts values. Explicit
in-memory inclusion uses opaque protected wrappers; it does not make values serializable or safe
for logs. Secret acquisition is metadata-only, and PodmanLens never requests a secret-payload
endpoint.

SSH and mutual-TLS connection specifications are public, but callers supply those transports.
PodmanLens does not contain an SSH client, TLS client, ambient endpoint resolver, or executor.
