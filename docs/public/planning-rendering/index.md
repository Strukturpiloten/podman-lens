# Plan and render without executing

Output begins with caller-authored `DeploymentIntent`, not an acquired inventory snapshot. The
caller selects one exact reviewed `TargetProfile`, an explicit rootful, rootless, or unknown
execution context, fully resolved resource identities, and every external precondition.

Run the deterministic offline example:

```console
cargo run --example offline_plan_and_render
```

It declares one external network, one managed image, and one unpodded container. It then exposes:

1. the ordered transport-neutral semantic operations;
2. retained external preconditions;
3. exact CLI program and argument arrays;
4. exact Libpod method, versioned path, and typed body descriptions;
5. serialization-only deployment-v1 JSON; and
6. a POSIX review script generated from the same CLI arrays.

The example opens no socket and starts no process.

## All-or-nothing contracts

`plan_deployment` returns a complete `DeploymentPlan` only when its sorted planning findings are
empty. It validates identities, prerequisites, pod membership, dependencies, cycles, target gates,
and sensitive-input boundaries without producing a partial plan.

`render_deployment` likewise returns a complete `DeploymentRendering` only when every populated
field has exact evidence for the selected target in both CLI and Libpod planes. Unsupported or
manual fields produce structured findings and no partial artifact for the affected rendering.

Protected inline environment values are rejected by this default path. A caller who explicitly
authorizes all protected inline environment values in one plan can call
`render_deployment_with_authorization(&plan, SensitiveInlineRenderAuthorization::new())`.
Unresolved external environment values still block the complete rendering. Authorized output
bytes may contain protected values in CLI arguments, Libpod JSON, and the review script.
`artifact::deployment_v1` refuses such a rendering before writing any bytes. Use
`artifact::deployment_v2` to serialize it: v2 includes a
`contains_protected_inline_environment` indicator derived from rendered values. Handle the bytes
as sensitive when that indicator is true; `Debug` remains redacted.

## Artifacts are inert

`artifact::deployment_v1::deployment` serializes desired deployment output. It is not an observed
inventory snapshot and is not accepted as Podman input. `DeploymentRendering::shell_script`
produces a reviewable script; PodmanLens never runs it. External secret material remains an opaque
reference and becomes a deferred file-input requirement, never artifact content.

Callers decide whether to publish artifacts. Any later execution, authorization, rollback, or
runtime mutation belongs outside PodmanLens.
