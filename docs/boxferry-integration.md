# BoxFerry integration contract

This document is the normative first-release mapping contract between PodmanLens native
observations and the BoxFerry neutral application model. PodmanLens remains independent of
BoxFerry: the downstream adapter depends on both libraries, while neither format library depends
on the neutral model.

The contract covers the public BoxFerry model as reviewed for Milestone 7: `Application`,
`Service`, `ServiceGroup`, `ServiceGroupRuntime`, `Network`, `NetworkAttachment`,
`NetworkIpamConfig`, `Volume`, `Secret`, `ResourceGrant`, `Mount`, `ImageReference`,
`ImageAcquisition`, `Command`, `Entrypoint`, `RestartPolicy`, `Healthcheck`, `Logging`,
`Port`, `HostMapping`, `ResourceLimit`, `SecurityOption`, `ResourceOwnership`, and
`Sourced<T>` with `Provenance`.

## Mandatory adapter algorithm

An adapter must process every selected `ResourceObservation` and every modeled
`ObservationField<T>`. It must not use a value until it has handled both the field state and,
for `Observed`, its `ObservationOrigin`.

| PodmanLens state            | Required adapter outcome                                                                                                                                                                                  |
| --------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `Absent`                    | Leave the neutral field absent. This is source omission, not a default.                                                                                                                                   |
| `Observed(Configured)`      | Promote only mappings marked **exact** below. Attach `Provenance::runtime_observation` because the native definition was obtained through runtime inspection.                                             |
| `Observed(Effective)`       | Keep observation-only unless an explicit conversion policy accepts the native value. A promoted value uses `Provenance::conversion_decision`, retaining the runtime observation as a contributing origin. |
| `Observed(RuntimeAssigned)` | Never create desired state automatically. Retain only explanatory observation evidence.                                                                                                                   |
| `Observed(LocalResolution)` | Never create portable desired state automatically. Require a user override or explicit target-local policy.                                                                                               |
| `Unavailable`               | Emit a structured incomplete-input outcome for the affected neutral field or resource.                                                                                                                    |
| `Malformed`                 | Emit a structured invalid-native-input outcome. Do not substitute absence or a default.                                                                                                                   |
| `VersionInapplicable`       | Record that the field has no usable meaning on the observed Podman line.                                                                                                                                  |
| `NotApplicable`             | Do not create a neutral value; no warning is needed unless the adapter expected the field for that kind.                                                                                                  |
| `Unmodelled(id)`            | Emit an unsupported-field outcome containing the stable semantic ID and use the observation header's completeness state.                                                                                  |

The source identity used for BoxFerry provenance must identify the selected Podman connection
without embedding endpoint, credential, environment value, label value, host path, or secret
material. PodmanLens evidence supplies the engine/API versions and reviewed capability entry.

Resource names are chosen from the public `ResourceIdentity`: prefer its native name when present
and keep its stable ID in adapter correlation state. A collision after neutral-name normalization is
an error, never a last-write-wins merge. Runtime-inspected resources begin as
`ResourceOwnership::Uncertain`. Only caller policy may change them to `Application`,
`External`, or `Implicit`.

## Container and pod mapping

| PodmanLens source                                               | BoxFerry target                                                        | Contract                                                                                                                                                                  |
| --------------------------------------------------------------- | ---------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Container identity/name                                         | `Service::name`, optionally `Service::runtime_name`                    | Exact identity mapping; preserve the stable native ID separately.                                                                                                         |
| `configured_image`                                              | `Service::image` / `ImageReference`; optional `ImageAcquisition`       | Exact only when configured. The adapter must not replace it with the local image ID or repository cache evidence.                                                         |
| `local_image_id`                                                | No desired-state field                                                 | Local-resolution evidence only.                                                                                                                                           |
| `labels`                                                        | `Service::labels` / `MetadataLabel`                                    | Exact when configured. Preserve protection classification and provenance for each value.                                                                                  |
| `environment`                                                   | `Service::environment` / `EnvironmentVariable`                         | Names may be mapped. Values require the caller-authorized callback and must remain protected; redacted values never become empty strings.                                 |
| `command`                                                       | `Service::command` / `Command::Exec`                                   | Exact ordered argument array when configured.                                                                                                                             |
| `entrypoint`                                                    | `Service::entrypoint` / `Entrypoint`                                   | Exact ordered argument array when configured.                                                                                                                             |
| `user`                                                          | `Service::user` and, when separable without guessing, `Service::group` | Exact spelling; do not invent a group from an absent component.                                                                                                           |
| `working_directory`                                             | `Service::working_directory`                                           | Exact configured path.                                                                                                                                                    |
| `hostname`                                                      | `Service::hostname`                                                    | Exact for an unpodded service. A pod member needs group-ownership review before promotion.                                                                                |
| `pod_membership`                                                | `ServiceGroup::members`                                                | Exact after resolving the ID/name reference to one pod. Missing or ambiguous resolution is an error.                                                                      |
| Pod identity/name                                               | `ServiceGroup`                                                         | Exact structural group with initially uncertain ownership.                                                                                                                |
| Pod `networking`                                                | `ServiceGroupRuntime` ports, networks, DNS, host mappings              | Configured values may map exactly where the neutral type has the same semantics. Effective values require policy.                                                         |
| Container `networking`                                          | `Service` networks, ports, DNS, host mappings                          | Only for unpodded containers. Runtime-assigned addresses are never promoted.                                                                                              |
| `native_dependencies`                                           | `ServiceDependency`                                                    | Exact dependent-to-prerequisite edge after unique resolution. No health condition is inferred.                                                                            |
| Named-volume mount                                              | `Service::mounts` / `MountSource::Volume`                              | Source and target map exactly. Effective access, options, propagation, and subpath require explicit semantic checks.                                                      |
| Bind mount                                                      | `Service::mounts` / `MountSource::HostPath`                            | Container target can map; the local-resolution host source requires an explicit user/target-local decision.                                                               |
| `secret_grants`                                                 | `Service::secret_grants` / `ResourceGrant`                             | Reference, target, UID, GID, and mode map only when their individual field states are usable. Secret bytes remain absent.                                                 |
| `restart_policy`                                                | `Service::restart_policy`                                              | Effective evidence; policy-gated because inspect can include normalization/defaults.                                                                                      |
| `health_check`, `startup_health_check`, `health_failure_action` | `Healthcheck` where semantics overlap                                  | Policy-gated. Protected command arguments require explicit authorization; startup/failure-action data without a neutral equivalent produces an unsupported-field outcome. |
| `logging`                                                       | `Service::logging`                                                     | Policy-gated effective evidence. Unknown drivers/options remain explicit unsupported data.                                                                                |
| `security`                                                      | capabilities and `SecurityOption` collections                          | Policy-gated effective evidence. Preserve order and duplicates; count-only security-option evidence cannot reconstruct values.                                            |
| `namespaces`                                                    | neutral namespace fields where exact                                   | Policy-gated effective evidence. Pod membership never hides member namespace evidence.                                                                                    |
| `resource_controls` and `memory_swappiness`                     | `ResourceLimit` and matching service limits                            | Policy-gated effective/configured evidence. Preserve native zero and `-1`; do not apply output validation during input mapping.                                           |

The adapter must preserve pod ownership: pod-owned networking is mapped to
`ServiceGroupRuntime`, while unpodded-container networking belongs to `Service`. A pod member's
runtime `NetworkSettings` cannot overwrite the pod's configured network namespace intent.

## Network, volume, image, and secret mapping

| PodmanLens source                                            | BoxFerry target                                                                  | Contract                                                                                                                                                             |
| ------------------------------------------------------------ | -------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Network identity/name                                        | `Network`                                                                        | Exact identity with uncertain ownership.                                                                                                                             |
| Network `labels`, `internal`                                 | matching `Network` fields                                                        | Labels configured; `internal` effective and therefore policy-gated.                                                                                                  |
| Network option keys                                          | No `NetworkDriverOption` values                                                  | Observation-only. PodmanLens deliberately has no option values, so the adapter cannot manufacture pairs.                                                             |
| Network `subnets`                                            | `NetworkIpamConfig`                                                              | Policy-gated effective evidence. Keep subnet/gateway/range association; never zip independent arrays. Host-bit spelling is evidence, not normalized authored intent. |
| Network `routes`                                             | No current BoxFerry neutral route field                                          | Unsupported-field outcome. Preserve route type version applicability in diagnostics.                                                                                 |
| Volume identity/name                                         | `Volume`                                                                         | Exact identity with uncertain ownership.                                                                                                                             |
| Volume labels                                                | `Volume::labels`                                                                 | Exact when configured.                                                                                                                                               |
| Volume UID/GID, driver, anonymous flag, creation time        | Matching field only when the neutral model has exact authored semantics          | Otherwise observation-only. Wire-absent UID/GID is not explicit zero. Creation time never becomes deployment intent.                                                 |
| Container configured image                                   | `Service::image` and optionally `ImageAcquisition`                               | Canonical source of desired image spelling.                                                                                                                          |
| Image repository tags/digests and digest                     | No automatic image intent                                                        | Local-resolution/effective evidence. They may verify a user choice but cannot replace configured intent.                                                             |
| Image author, architecture, OS, manifest type, creation time | No current desired-state mapping                                                 | Observation-only metadata.                                                                                                                                           |
| Image environment                                            | No automatic service environment                                                 | Protected image defaults remain observation-only.                                                                                                                    |
| Secret identity/name                                         | `Secret`                                                                         | Exact metadata identity with `ResourceOwnership::External` or `Uncertain`; never application-managed without an external material declaration.                       |
| Secret labels                                                | No material semantics; metadata may be retained if the neutral model supports it | Never treat labels as payload.                                                                                                                                       |
| Secret driver name, option count, timestamps                 | No `SecretMaterial`                                                              | Observation-only. Option names/values and payload are unavailable by design.                                                                                         |

## Neutral model to PodmanLens output

Input observations do not implement a conversion into deployment intent. The BoxFerry adapter owns
the policy boundary, then constructs PodmanLens output types explicitly:

| BoxFerry intent                                     | PodmanLens output                                                              |
| --------------------------------------------------- | ------------------------------------------------------------------------------ |
| `ImageAcquisition` / `ImageReference` / pull policy | `ImageIntent`, `ImageSource`, `ImagePullPolicy`                                |
| `Network` and reviewed IPAM/routes                  | `NetworkIntent`, `NetworkSubnet`, `NetworkRoute`                               |
| `Volume` and explicit ownership                     | `VolumeIntent`                                                                 |
| External secret                                     | `ExternalPrecondition`; managed material reference becomes `SecretIntent`      |
| `ServiceGroup` / `ServiceGroupRuntime`              | `PodIntent`                                                                    |
| `Service`                                           | `ContainerIntent`, settings, runtime settings, mounts, grants, and attachments |
| `ServiceDependency`                                 | `StartupDependency` only for an exact start-order dependency                   |

The adapter calls `plan_deployment` and must return every `PlanningFinding`; it calls
`render_deployment` only for a complete plan and must return every `RenderingFinding`. It must
not execute CLI or Libpod descriptions. The selected `TargetProfile` comes from explicit
destination evidence, never from the inspected source or development host by accident.

The external-consumer scenario in `tests/boxferry_adapter.rs` exercises acquisition, discovery,
typed observation consumption, these mapping decisions, neutral intent, deployment intent,
planning, CLI rendering, and Libpod rendering. Its fixed input and expected result are the bounded
Podman 6.1 corpus artifacts.
