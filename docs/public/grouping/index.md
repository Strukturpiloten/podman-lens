# Review groups, pods, and shared boundaries

PodmanLens keeps four concepts separate because they carry different evidence:

| Concept             | Meaning                                                                   |
| ------------------- | ------------------------------------------------------------------------- |
| Dependency          | A directed dependent-to-prerequisite ordering fact                        |
| Resource group      | Resources joined by strong ownership or lifecycle evidence                |
| Shared prerequisite | A network, volume, image, or secret used across otherwise separate groups |
| Pod membership      | Native evidence that selected containers already belong to one Podman pod |

Pod membership is grouping evidence without becoming a dependency cycle. Conversely, graph
connectivity does not authorize inventing a pod. Pod layout for new target intent remains an
explicit caller decision.

## Evidence that can merge groups

Groups may merge through observed pod membership, native container dependencies, or complete and
consistent Docker/Podman Compose ownership labels. Compose labels are advisory only when project
and service pairs agree and any present configuration hashes agree. Empty, incomplete, orphaned,
or conflicting labels produce findings and no merge.

Merely sharing a network, volume, image, or secret never merges groups. `network.internal`
describes connectivity, not ownership.

## Network borders

The offline `graph-boundaries-6.1.responses.json` fixture covers stopped shared-network traversal,
an explicitly selected shared root, exact name and ID crossings, shared prerequisites, ownership
evidence, and unused overrides. Its contract protects both sides of the boundary:

- without authorization, another group's consumers remain outside the closure;
- with one exact network authorization, direct consumers may be traversed and the explanation
  records the crossing;
- unrelated shared prerequisites remain prerequisites rather than accidental group glue.

Inspect `ResourceGraph::groups`, dependency edges, findings, and explanations together. A list of
members alone is not enough evidence to reconstruct portable lifecycle or namespace intent.
