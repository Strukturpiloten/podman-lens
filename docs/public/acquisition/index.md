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

## Protected values

The default acquisition policy retains environment names and order but redacts values. Explicit
in-memory inclusion uses opaque protected wrappers; it does not make values serializable or safe
for logs. Secret acquisition is metadata-only, and PodmanLens never requests a secret-payload
endpoint.

SSH and mutual-TLS connection specifications are public, but callers supply those transports.
PodmanLens does not contain an SSH client, TLS client, ambient endpoint resolver, or executor.
