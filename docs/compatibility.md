# Compatibility matrices

Compatibility is explicit evidence, not a claim that every Podman field is modeled. The capability
catalogue selects reviewed native wire meaning; the native-field ledger records every accepted
input and output field; rendering has its own revision-pinned evidence catalogue.

## Podman versions

All reviewed lines require Libpod API 4.0.0 or newer. A target outside the half-open range is
rejected even when its JSON happens to decode.

| Podman line | Accepted range  | Pinned evidence      | Offline input evidence                                                     | Output status                                                              |
| ----------- | --------------- | -------------------- | -------------------------------------------------------------------------- | -------------------------------------------------------------------------- |
| 5.4         | >=5.4.0, <5.5.0 | v5.4.0 / `f9f7d48b…` | Rootless full inventory boundary                                           | Bounded M6 B1-B3 fields; B4 UID/GID and image-policy features target-gated |
| 5.5         | >=5.5.0, <5.6.0 | v5.5.0 / `0dbcb514…` | Capability and rendering catalogue                                         | Same 5.x bounded rendering gates                                           |
| 5.6         | >=5.6.0, <5.7.0 | v5.6.0 / `da671ef6…` | Capability and rendering catalogue                                         | Adds reviewed UID/GID, unlimited-rlimit, and image-policy support          |
| 5.7         | >=5.7.0, <5.8.0 | v5.7.0 / `0370128f…` | Pinned all-six-kind BoxFerry corpus                                        | Bounded B1-B4 rendering; route type remains input-version-inapplicable     |
| 5.8         | >=5.8.0, <6.0.0 | v5.8.6 / `a859fc66…` | Opt-in current-patch conformance plus catalogue                            | Bounded B1-B4 rendering; non-unicast route/order features remain gated     |
| 6.0         | >=6.0.0, <6.1.0 | v6.0.0 / `a8ed4b6d…` | Pinned all-six-kind and IPAM/route corpora                                 | Adds reviewed route types, network order, and journald-label support       |
| 6.1         | >=6.1.0, <6.2.0 | v6.1.0 / `cade97a5…` | Pinned bounded all-six-kind adapter, rootful, malformed, and graph corpora | Complete current bounded B1-B4 rendering                                   |

The complete revisions and source URLs live in `catalogue/v1/podman-capabilities.json`,
`catalogue/v1/podman-deployment-rendering.json`, and `fixtures/corpus/manifest.json`. The
ignored current-patch conformance test is an explicit network/runtime check and is not part of the
offline gate.

## Resource kinds

“Output” means typed authored intent can be planned and rendered. It never means an observation is
automatically replayed.

| Kind      | Acquire and type                                                                                         | Discovery                                        | Snapshot v1             | Neutral mapping                                                                     | Bounded output                                                               |
| --------- | -------------------------------------------------------------------------------------------------------- | ------------------------------------------------ | ----------------------- | ----------------------------------------------------------------------------------- | ---------------------------------------------------------------------------- |
| Container | Yes: configuration, topology, mounts/grants, networking, health, logging, security, namespaces, controls | Exact/label roots, pod/dependency closure        | Yes, always redacted    | `Service`; origin-gated                                                             | `ContainerIntent`, settings/runtime, mounts, grants, networks                |
| Pod       | Yes: membership, labels, infra/networking                                                                | Exact/label roots and strong grouping            | Yes                     | `ServiceGroup` and `ServiceGroupRuntime`                                            | `PodIntent`; one start per pod                                               |
| Network   | Yes: labels, internal, option keys, IPAM, routes                                                         | Exact/label roots and explicit boundary crossing | Yes                     | `Network`; effective IPAM policy-gated, routes unsupported in current neutral model | `NetworkIntent` with bounded subnets/routes                                  |
| Volume    | Yes: labels, UID/GID, driver, timestamp, anonymous                                                       | Exact/label roots and mount prerequisites        | Yes                     | `Volume` with uncertain ownership                                                   | `VolumeIntent`, optional UID/GID                                             |
| Image     | Yes: tags/digests, digest, timestamp, author, architecture, OS, manifest type, protected environment     | Exact roots and configured-image prerequisites   | Yes                     | Configured container image maps; cache metadata remains observation-only            | `ImageIntent` with explicit source/pull policy                               |
| Secret    | Metadata only: labels, driver state/count, timestamps; no payload endpoint                               | Exact/label roots and grant prerequisites        | Yes, payload impossible | `Secret` metadata plus external/uncertain ownership                                 | External precondition or `SecretIntent` with caller-owned material reference |

Unmodeled native data stays visible through bounded semantic metadata and completeness state. Exact
field coverage is machine-readable in `catalogue/v1/native-field-coverage.json`.

## Transports and execution

| Capability                        | Built-in Unix transport           | Caller `LibpodTransport`                                | CLI rendering                               | Libpod rendering                         |
| --------------------------------- | --------------------------------- | ------------------------------------------------------- | ------------------------------------------- | ---------------------------------------- |
| Explicit connection selection     | `ConnectionSpec::Unix`            | Caller resolves Unix, SSH, or mutual-TLS specs          | Optional non-sensitive connection reference | Caller chooses endpoint outside renderer |
| Version probe                     | Yes                               | Yes                                                     | Not applicable                              | Not applicable                           |
| Six-kind list/inspect acquisition | Yes, GET only                     | Yes, contract requires the supplied read-only responses | Not applicable                              | Not applicable                           |
| Redirects, decompression, retries | No                                | Caller policy outside PodmanLens                        | No execution                                | No execution                             |
| Built-in SSH client               | No                                | Caller supplied                                         | Podman connection name may be rendered      | Caller supplied                          |
| Built-in mutual-TLS client        | No                                | Caller supplied                                         | Not opened                                  | Caller supplied                          |
| Mutation/execution                | Rejected before socket connection | Not used by acquisition APIs                            | Description only                            | Typed method/path/body description only  |
| Secret material transfer          | Never acquired                    | Never requested by acquisition                          | External input reference only               | External sensitive body reference only   |

`ReadOnlyUnixTransport` supports bounded HTTP/1.1 over an explicitly selected Unix socket. It has
no ambient endpoint discovery, background tasks, redirects, compression, retry loop, SSH, TLS, or
mutation facility. `plan_deployment` is transport-neutral; `render_deployment` creates
non-executing descriptions. Applying a plan remains outside the first-release contract.
