# Repository guidance for coding agents

This file applies to the whole PodmanLens repository.

## Start with the document that owns the task

| Task                                   | Read first                                                          |
| -------------------------------------- | ------------------------------------------------------------------- |
| Any behavior or boundary change        | `README.md`, `docs/architecture.md`, and `docs/decisions/README.md` |
| Public Rust API or serialized artifact | `docs/api-stability.md` and the relevant Rustdoc or schema          |
| Podman version or native-field support | `docs/public/compatibility/index.md` and `catalogue/v1/`            |
| Tests or fixtures                      | `docs/testing.md`                                                   |
| File ownership                         | `docs/project-structure.md`                                         |
| Dependency change                      | `docs/dependency-policy.md`                                         |
| Release work                           | `docs/releasing.md` and `CHANGELOG.md`                              |
| Public guide                           | `docs/public/index.md` and `tests/public_guides.rs`                 |

Read the accepted decision records that affect the task. An architectural change must update or
supersede the relevant decision in the same change.

## Scope

PodmanLens owns native Libpod API handling, version evidence, typed native observations, resource
discovery, ordered deployment semantics, and deterministic non-executing renderings. It does not
depend on BoxFerry, choose cross-format mappings, parse Compose or Quadlet, shell out to `podman`
for input, or execute a plan.

## Non-negotiable behavior

- Treat runtime input as fallible; malformed data produces structured failures, never panics.
- Preserve native evidence and unknown data long enough for callers to explain outcomes.
- Keep target versions explicit and evidence-backed.
- Keep transport replaceable and free from ambient connection discovery.
- Never request secret payloads.
- Redact protected and runtime-sensitive values from diagnostics, logs, snapshots, and artifacts.
- Keep observation, caller-authored intent, planning, and rendering as separate stages.
- Start every repository-owned complete YAML document with `---`.

## Development rules

- Add positive and negative tests for each supported field and capability boundary.
- Record Podman version, source, and fixture provenance for native behavior claims.
- Keep public APIs independent from private protocol response types.
- Update capability data and documentation with every version-boundary change.
- Keep current-state prose concise; history belongs in the changelog and decision records.
- Pin every GitHub Action to a full commit SHA with its exact release tag in a comment.

## Validation

Run the complete gate after the final edit:

```shell
./scripts/check-all.sh
```

Focused commands are listed in `docs/testing.md`. The complete gate is required before a commit
or pull request.

## Git and GitHub workflow

The primary Sol agent owns issue creation, branches, final integration, the complete validation
gate, staging, commit, push, pull request creation, and GitHub readback. Workers may research,
implement bounded changes, or verify, but never commit, push, publish, tag, or create GitHub
objects.

Opening and reading back a ready pull request is the default stopping point. Authorization to run
the Git workflow or perform GitHub writes does not authorize a merge.

Merge only when the user explicitly authorizes merging the specific pull request or the scoped set
of pull requests in the current request. Immediately before merging, read back the exact head
commit and verify that the pull request is ready, mergeable, and has every required check
successful. Never bypass branch protection, use an administrator override, or infer authority for
an out-of-scope release, publication, or deployment pull request.

Use the repository's normal merge method with an exact-head safeguard, then read back and report
the merged state and merge commit.

Use `feat`, `fix`, `perf`, `refactor`, or `revert` only for release-worthy code. Use
`docs`, `test`, `ci`, `build`, `style`, or `chore` for maintenance so release-plz does
not propose an unnecessary crate release.
