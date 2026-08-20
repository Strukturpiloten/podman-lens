# Library API

The native input API is stable from M4. It is exercised as an external consumer and by fixed
rootless, rootful, malformed, and graph-boundary corpora.

```text
acquire_inventory → DiscoveryRequest → discover → ResourceGraph
```

## Acquire an inventory

`acquire_inventory` reads an explicitly selected `LibpodTransport`. It probes the service, lists
containers, pods, networks, volumes, images, and secret metadata, then inspects stable resource IDs.
The result preserves partial and malformed observations as evidence or structured findings.

## Select roots

Add exact resource roots with `ResourceSelector::exact`. A reference is one exact resource name,
ID, or image alias; patterns are rejected. Add label roots with `LabelSelector::presence` or
`LabelSelector::exact`. An empty exact label value is valid and differs from presence-only matching.
`DiscoveryRequest::select_all` selects only eligible application roots, not every cached image.

Discovery follows dependency closure by default. Selecting a shared network, volume, image, or
secret explicitly crosses to its direct consumers. A network boundary can also be crossed by one
exact name-or-ID authorization. There is no grouping configuration file and no wildcard crossing.

## Read the graph

`ResourceGraph` exposes:

- requested resource and label roots;
- the `all` choice and resolved root identities with redacted selector-origin positions;
- deterministic resource groups and their prerequisites;
- shared prerequisites;
- directed dependent-to-prerequisite edges;
- separate non-directed grouping evidence;
- structured findings; and
- explanations for roots, inclusions, prerequisites, stopped boundaries, authorized crossings,
  strong-evidence merges, and group ordering.

Groups merge through pod membership, native container dependencies, or complete matching
Docker/Podman Compose ownership evidence. Merely sharing a network, volume, image, or secret never
merges groups. `network.internal` describes connectivity only.

## Determinism and sensitive values

All collections use deterministic ordering. `ResourceKind::canonical_rank` fixes resource and
group ordering as container, pod, network, volume, image, then secret; a group ID is its smallest
`(kind, id)` member under that order. Debug output redacts label values and Compose ownership
values. Environment values and secret payloads must not appear in diagnostics, debug output, or
versioned snapshots.

## Export a redacted snapshot

`snapshot::v1::inventory` and `snapshot::v1::graph` create serialization-only reports with
`schema_version: 1`. They implement `Serialize`, not `Deserialize`, and conform to the committed
Draft 2020-12 schema. Snapshot creation always removes environment values, secret payloads,
connection data, raw unknown JSON, label values, driver-option values, and Compose ownership
values, regardless of in-memory acquisition policy.

Snapshots still contain operational metadata such as resource IDs and names, image aliases,
environment variable names, network subnets, evidence URLs, and source field paths. Always-redacted
does not mean anonymous; callers must still handle reports as operational data.

## Non-goals

The input API does not discover an ambient connection, parse `podman` command output, execute
commands, choose BoxFerry mappings, or decide a target pod layout.
