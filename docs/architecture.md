# Architecture

PodmanLens turns explicit Libpod API evidence into typed native observations and turns
caller-authored Podman intent into deterministic, non-executing output. Observation and intent are
separate contracts: acquired runtime state is never treated as portable desired state
automatically.

## Boundary

| PodmanLens owns                          | The caller owns                             |
| ---------------------------------------- | ------------------------------------------- |
| Explicit connection specifications       | User-facing CLI and configuration           |
| Replaceable Libpod transport messages    | SSH or mutual-TLS client implementation     |
| Built-in read-only Unix acquisition      | Endpoint selection and credentials          |
| Version and capability evidence          | Cross-format and neutral-model mappings     |
| Typed native inventory and discovery     | Loss-policy authorization                   |
| Podman deployment intent and planning    | Target layout decisions                     |
| CLI, Libpod, JSON, and script renderings | Publication or execution of rendered output |

PodmanLens does not depend on BoxFerry, parse Compose or Quadlet, discover ambient Podman
connections, or execute generated operations.

## Data flow

```text
explicit connection + acquisition options
                    |
                    v
          replaceable transport
                    |
                    v
       versioned Libpod decoding
                    |
                    v
       typed resource inventory
                    |
                    v
     explicit discovery request
                    |
                    v
       evidence-backed graph

caller-authored Podman intent
                    |
                    v
       compatibility validation
                    |
                    v
    ordered semantic operations
                    |
                    v
 CLI + Libpod + JSON + review script
```

The Docker-compatible API is not the native input contract because it cannot represent Podman
features such as pods.

## Acquisition and observations

`acquire_inventory` probes one explicitly selected Libpod service, then lists and inspects
containers, pods, networks, volumes, images, and secret metadata with bodyless `GET` requests.
The built-in `ReadOnlyUnixTransport` rejects mutation, caller-supplied `Host`, and over-limit
messages before opening its Unix socket. Callers may provide another `LibpodTransport`.

Acquisition is deliberately non-atomic. A failed list marks only that resource section
unavailable. A disappeared or malformed inspection remains an incomplete observation while
unrelated resources stay usable.

Every modeled native field is an `ObservationField<T>`. Its state distinguishes absence,
unavailable or malformed evidence, version inapplicability, unsupported data, and a usable
observation. Usable values also retain whether they are configured, effective, runtime-assigned,
or locally resolved. Downstream policy must inspect both state and origin before creating desired
state.

Private Libpod response types do not cross the public API. Unsupported input is retained as
bounded semantic metadata without retaining arbitrary raw JSON. The inventory-wide bound is
allocated fairly across listed resources so an early resource kind cannot consume every retained
path descriptor. Closed runtime projections that duplicate typed or host-local state are recorded
in the strict coverage ledger and discarded instead of being reported as unknown authored intent.

Container mount decoding retains a case-sensitive SELinux `z` or `Z` choice as a typed configured
observation. It reads the normalized `Mounts[].Mode` evidence first and may correlate the same
closed choice from `HostConfig.Binds`; raw bind strings, host paths, and other creation arguments do
not cross the decoder boundary.

## Discovery

`discover` is pure after acquisition. A `DiscoveryRequest` selects exact resource identities,
literal name prefixes, exact label predicates, or all eligible application roots. Prefixes match
names only and resolve in sorted identity order; IDs and image aliases remain exact-only. Glob,
regular-expression, and wildcard syntax are not accepted.

The resulting graph keeps directed dependency edges separate from grouping evidence. Pod
membership, native container dependencies, and one complete internally consistent Compose
ownership-label namespace may join resource groups. Merely sharing a network, volume, image, or secret does not. Reverse
traversal across a shared network requires that network to be an explicit root or an exact boundary
authorization.

Graph ordering and explanations are deterministic. Findings preserve unresolved selectors,
ambiguous relationships, stopped boundaries, and unused authorizations instead of guessing.

## Planning and rendering

Output begins with a caller-created `DeploymentIntent` and explicit `TargetProfile`. Managed
resources use target-side identities that are distinct from acquired native identities. Every
unmanaged network, volume, image, or secret dependency is an explicit
`ExternalPrecondition`.

`plan_deployment` validates the complete intent and returns ordered semantic operations or sorted
findings, never a partial success. Image acquisition is explicit. Pod members are created before a
single pod start; unpodded containers receive their own starts. Dependency order is authoritative
and may also be inspected through operation dependencies.

`render_deployment` accepts only fields with reviewed semantics for the selected target. It
produces CLI argument arrays, typed Libpod request descriptions, a serialization-only deployment
artifact, and a POSIX review script from the same plan. It opens no connection and starts no
process.

## Versions and evidence

Observed input versions and selected output versions are explicit values. A version that parses is
not automatically supported. Capability, native-field, and renderer claims live in
`catalogue/v1/` with upstream source and test ownership.

Human-readable support information is in the
[compatibility guide](public/compatibility/index.md). Version-bound behavior changes must update
the relevant catalogue, boundary tests, and guide in one change.

## Protected data

Secret payload endpoints are never requested. Environment values, protected health commands,
credentials, connection details, raw unknown JSON, and secret driver options are excluded from
diagnostics, debug output, snapshots, and deployment artifacts unless a public output value is
explicitly constructed by the caller.

`snapshot::v1` is an always-redacted observational export. It is still operational data because
resource names, identifiers, and evidence paths may remain. `artifact::deployment_v1` represents
caller-authorized desired output and never accepts an observational snapshot as input.

## Decisions

The durable reasons behind these boundaries are recorded in
[the accepted decisions](decisions/README.md). Change or supersede a decision explicitly when the
architecture changes.
