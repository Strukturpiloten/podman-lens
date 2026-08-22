# Discover an application graph

Discovery starts from the acquired `ResourceInventory` and an explicit `DiscoveryRequest`. It is
pure and deterministic: no transport is contacted after acquisition.

Choose roots in one or more ways:

- `ResourceSelector::exact` selects one container, pod, network, volume, image, or secret by exact
  name, ID, or supported image alias.
- `LabelSelector::presence` selects resources carrying one exact label key.
- `LabelSelector::exact` additionally requires one exact value; an empty value remains distinct
  from presence-only matching.
- `select_all` selects every eligible application root, not every cached image or shared resource.

Wildcards are rejected. Unresolved and ambiguous selectors remain structured findings instead of
guessing which resource the caller meant.

## Follow evidenced relationships

The default closure follows dependent-to-prerequisite relationships. It includes native container
dependencies, pod membership, networks, volumes, configured image evidence, and metadata-only
secret references when the evidence resolves without conflict.

Configured image spelling may create an image dependency. A locally resolved image identity never
becomes desired-image evidence. A contradictory ID/name secret reference never becomes a graph
edge.

The returned `ResourceGraph` keeps requested and resolved roots, resource groups, shared
prerequisites, dependency edges, grouping evidence, findings, and explanations. Explanations state
why a resource was included, why traversal stopped, when a crossing was authorized, and why groups
merged or remained separate.

## Cross one shared network deliberately

Sharing a network does not prove two applications are one group. Reverse traversal stops at that
boundary unless the network itself is an explicit root or the request authorizes its exact name or
ID with `add_network_boundary_override`.

Use the [grouping guide](../grouping/) before authorizing a crossing. A broad or unused override is
not ignored silently; it is rejected or reported as a finding.
