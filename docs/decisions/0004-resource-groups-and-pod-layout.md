# 0004: Resource groups and pod layout are separate

- Status: Accepted
- Date: 2026-08-19

## Context

Dependency connectivity does not prove that containers should share a pod. Pods change namespaces,
lifecycle, scheduling, and scaling behavior.

## Decision

PodmanLens represents discovered resource groups and observed pod membership independently. It does
not infer new pod membership from graph connectivity.

A caller may request one of these output policies for containers without source pod membership:

- preserve existing membership and leave unpodded containers unpodded;
- create one pod per unpodded container;
- create one pod per resource group after explicit loss authorization; or
- emit no new pods where the target permits it.

BoxFerry owns the user-facing pod-layout choice and cross-format semantics. PodmanLens validates and
renders the resulting explicit Podman deployment intent.

## Consequences

- Podman and Quadlet input can preserve existing topology.
- Compose and Docker input require a conscious target pod-layout decision.
- Kubernetes output can reject a layout that leaves containers outside pods.
- Group-wide pods are never a silent default.
