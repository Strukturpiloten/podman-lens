# Architecture

PodmanLens owns Podman-native protocol handling. It turns reviewed Libpod API evidence into typed
input observations and ordered deployment plans. Callers such as BoxFerry own orchestration and
cross-format semantics.

## System boundary

| PodmanLens owns                              | The caller owns                                  |
| -------------------------------------------- | ------------------------------------------------ |
| Connection and transport abstraction         | User-facing command-line structure               |
| Libpod API request and response types        | Compose, Quadlet, Docker, and Kubernetes parsing |
| Podman version and capability evidence       | Mapping to and from a neutral application model  |
| Native resource inspection and relationships | Loss-policy authorization                        |
| Native resource discovery and boundaries     | Output-directory lifecycle and reports           |
| Ordered Podman deployment operations         | Deciding pod layout for cross-format input       |
| CLI and Libpod API operation renderings      | Whether and how a plan is executed               |

PodmanLens is a library. It must not shell out to `podman` to acquire input, depend on BoxFerry, or
execute generated operations.

## Data flow

```text
Connection + discovery request
             |
             v
      replaceable transport
             |
             v
    versioned Libpod protocol
             |
             v
 typed native resource inventory
             |
             v
 evidence-backed resource graph
             |
             +----> caller maps input semantics

explicit Podman deployment intent
             |
             v
  version-aware operation planner
             |
             v
 ordered semantic operations
       |                 |
       v                 v
  CLI `argv`       Libpod HTTP request
```

The Docker-compatible API is not the canonical protocol because it cannot represent every
Podman-native feature, including pods. A replaceable transport carries versioned Libpod requests
over a local socket or an explicitly selected remote connection.

## Core contracts

Names are conceptual until the public Rust API is implemented.

- `ConnectionSpec` identifies a local or remote Podman service without ambient discovery.
- `TargetProfile` records the exact Podman and API compatibility range.
- `DiscoveryRequest` contains explicit roots, label selectors, closure policy, and authorized
  boundary crossings.
- `ResourceInventory` retains typed Podman resources, identifiers, relationships, and native
  evidence.
- `ResourceGraph` contains discovered groups, shared prerequisites, boundaries, and their evidence.
- `DeploymentIntent` contains fully resolved Podman-native resources supplied by a caller.
- `DeploymentPlan` contains an ordered list of semantic operations.
- `Operation` contains identity, dependencies, resource action, and all exact supported transport
  representations.

## Ordered deployment plans

The order of `operations` is authoritative. `depends_on` records why the order exists and permits a
consumer to validate or safely parallelize independent work. The semantic resource action is the
source of truth; CLI and HTTP forms are generated representations.

Executing every operation sequentially in array order must always be valid. Parallel execution is
an optional optimization derived from `depends_on`, not a requirement for consuming the plan.

```json
{
  "schema_version": 1,
  "target": {
    "connection": "production",
    "podman_version": "6.1.0",
    "api": "libpod"
  },
  "operations": [
    {
      "id": "pod.immich",
      "action": "create",
      "resource": {
        "kind": "pod",
        "name": "immich"
      },
      "depends_on": ["network.immich"],
      "representations": {
        "cli": {
          "argv": ["podman", "--connection", "production", "pod", "create", "--name", "immich"]
        },
        "api": {
          "method": "POST",
          "path": "/v6.1.0/libpod/pods/create",
          "body": {
            "name": "immich"
          }
        }
      }
    }
  ]
}
```

A caller may render `deployment.sh` from the same CLI representations. PodmanLens never executes
that script or the HTTP requests.

## Expected BoxFerry integration

PodmanLens does not own BoxFerry's CLI, but these option groups map directly to its public
contracts:

- `--input-connection` selects the `ConnectionSpec` used for read-only acquisition.
- Container, pod, network, volume, image, secret, label, and `all` selectors build a
  `DiscoveryRequest`.
- Complete resource-group closure is the default; an exact network-boundary override authorizes an
  exceptional crossing.
- `--pod-layout` is a BoxFerry mapping policy applied before Podman output intent reaches
  PodmanLens.
- `--output-connection` is recorded in the plan and rendered into CLI operations. Planning does not
  connect to that destination.
- `--output-directory` receives `deployment-plan.json` and `deployment.sh`; generated artifacts do
  not replace the human or JSON conversion report on standard output.

PodmanLens returns structured data and rendered artifacts. BoxFerry remains responsible for
output-directory safety, diagnostic presentation, and loss-policy authorization.

## Discovery and resource groups

Every resource selector is a root. Default discovery follows the complete, evidenced dependency
closure. Overlapping closures form one resource group; disjoint closures remain separate groups.
`all` discovery finds every group using the same rules.

Pod membership and reviewed ownership labels are strong evidence. A resource used only inside the
current closure may also remain in that group. Shared or source-declared external resources are
prerequisites and boundaries, not automatic bridges to unrelated consumers.

Podman's `internal` network property controls connectivity, not ownership. It cannot by itself
decide whether traversal should cross a network. Exact user-authorized boundary crossings handle
exceptions; no separate grouping file is required.

## Pod layout

Resource groups describe dependency topology. Pod layout describes target runtime placement. They
must remain separate.

PodmanLens preserves observed pod membership on input and renders explicit pod membership on
output. The caller decides whether previously unpodded containers are preserved, wrapped one per
pod, combined per resource group, or left without pods. Combining a whole resource group in one pod
can change namespace and lifecycle semantics and must not be inferred silently.

## Sensitive values

Normal secret inspection retrieves metadata only. Secret payload access requires explicit caller
authorization. Environment values obtained from a runtime are redacted by default because Podman
cannot prove which values are sensitive.

Sensitive bytes must not appear in diagnostics, `Debug` or `Display` output, snapshots, or the
serialized deployment plan. A plan refers to external sensitive inputs. An explicit unsafe-file
renderer may materialize restricted files later. Base64 is an encoding, not protection.

## Compatibility

Input records the observed Podman version. Output requires an explicit target profile. Capability
data governs both CLI flags and Libpod request fields; the development machine never supplies an
implicit target version.

The initial evidence programme will review Podman 5.4 through 6.1, including current patch-level
conformance for 5.8.6 and 6.1.0. A version is not supported until its fixtures, boundaries, and
positive and negative tests are committed.

## Primary references

- [Podman system service](https://docs.podman.io/en/latest/markdown/podman-system-service.1.html)
- [Libpod REST API](https://docs.podman.io/en/latest/_static/api.html)
- [Podman remote connections](https://docs.podman.io/en/latest/markdown/podman-system-connection.1.html)
