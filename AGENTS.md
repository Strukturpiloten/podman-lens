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
| Release or workflow work               | `docs/releasing.md`, `docs/dependency-policy.md`, `CHANGELOG.md`    |
| Public guide                           | `docs/public/index.md` and `tests/public_guides.rs`                 |

Read the accepted decision records that affect the task. An architectural change must update or
supersede the relevant decision in the same change.

## Workflow and version ownership

- When shared CI or release behavior changes, audit reusable callers and equivalent tasks in
  BoxFerry, ComposeLens, and QuadletLens so the repositories keep the same contract intentionally.
- Every fixed action, runner, tool, runtime image, version, and digest must have Renovate ownership
  or an explicit documented manual owner. Update `.github/renovate.json` and executable repository
  policy tests in the same change.
- Keep one canonical current value. Tests and prose validate its structure and coupling instead of
  duplicating a version or digest that Renovate would leave stale.
- Release validation must bind evidence to the exact candidate, current run, and current run attempt, fail closed, and
  keep validation-only runs unable to publish.

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

The primary agent owns issue creation, branches, final integration, the complete validation
gate, staging, commit, push, pull request creation, and GitHub readback. Workers may research,
implement bounded changes, or verify, but never commit, push, publish, tag, or create GitHub
objects.

Use `feat`, `fix`, `perf`, `refactor`, or `revert` only for release-worthy code. Use
`docs`, `test`, `ci`, `build`, `style`, or `chore` for maintenance so release-plz does
not propose an unnecessary crate release.

## Workspace scope and standing GitHub authorization

The maintainer grants standing authorization for task-related Git and GitHub work only in these
workspace repositories:

- `Strukturpiloten/boxferry`
- `Strukturpiloten/compose-lens`
- `Strukturpiloten/podman-lens`
- `Strukturpiloten/quadlet-lens`
- `Strukturpiloten/boxferry-website`
- `Strukturpiloten/docker-lens`

Do not work on or modify any repository outside this explicit allowlist, including its issues,
pull requests, branches, settings, or workflows. An upstream documentation reference is not
permission to operate on that upstream repository. A newly discovered checkout is not implicitly
in scope.

For user-requested work within this scope, the primary agent may create issues, branches, commits,
pushes, and pull requests and merge verified task-related pull requests without asking for renewed
approval. This permission does not authorize unrelated backlog work, implementation of
discussion-only proposals, or expansion of the requested product scope. A later user instruction
may narrow or revoke this permission.

Immediately before merging, read back the exact head commit and verify that the pull request is
ready, mergeable, independently reviewed, and has every required check successful. Use the normal
merge method with an exact-head safeguard; never bypass branch protection or use an administrator
override. Read back the merged state and merge commit, synchronize local `main` with `origin/main`,
and remove the task's recorded worktrees and verified merged local branches while preserving
unrelated work.

This standing permission does not authorize releases, publication, deployment operations, or
merging release/publication/deployment pull requests; those require a separate explicit request.
The primary agent owns all Git and GitHub writes. Subagents remain within their assigned task and
checkout and must not perform those writes.

## Agent roles and verification

Model defaults belong in [`.codex/config.toml`](.codex/config.toml); task-specific models and
reasoning belong in [`.codex/agents/`](.codex/agents/). The primary manager always uses
`gpt-6-astra` with `xhigh` reasoning. Implementation, specification research, and independent
review use `gpt-6-sol` with `high` reasoning; check-only verification uses `gpt-6-luna` with
`high` reasoning. Use Luna for bounded read-only exploration and Sol for difficult failure
diagnosis. These model settings do not expand workspace scope or grant additional permissions.

- Delegate bounded tasks when independent work can usefully proceed in parallel. Define the shared
  contract and explicit repository, checkout, and file ownership before delegation.
- Use up to nine concurrent subagents plus the primary manager, subject to the session's actual
  runtime limit. Nine is a ceiling, not a target or nine distinct roles: subagents may use the same
  role for independent tasks. Do not create nested agents to evade the limit.
- Never run two writers in one checkout. Use separate assigned repositories or worktrees for
  concurrent implementation. Research and review remain read-only.
- The reviewer checks the original requirements and independent expected results, not just agreement
  between the implementation and its tests.
- After writing finishes, the verifier runs `./scripts/check-all.sh --check`. It reports failures
  without formatting or editing tracked files; ignored build artifacts and caches are allowed.
- Run at most one complete gate or heavy runtime suite at a time across the workspace. Agent
  concurrency does not permit competing builds. The primary agent owns integration, the final
  complete gate, and every authorized Git or GitHub write.

The default `./scripts/check-all.sh` still formats before checking. `--check` runs the same
complete gate without source formatting; it is not a reduced test tier. A later edit invalidates
either result. Neither mode grants release, publication, or deployment authority.

## Code discovery

For code discovery, use an available codebase-memory graph first; otherwise use CodeGraph only if
the repository already has a usable index. Do not create an index without user authorization.
If neither graph is available or a query cannot answer the question, use `rg` and targeted reads.
For string literals, configuration, scripts, and documentation, start with `rg` directly.

## After an authorized merge

Read back the merged state and exact merge commit, then synchronize the primary checkout with
`origin/main`. Preserve unrelated files. Remove only the recorded task worktree with
`git worktree remove <recorded-path>`, delete the verified merged local issue branch with
`git branch --delete --force TheRealBecks/issue<NUMBER>`, and run
`git worktree prune --verbose`. Read back `git worktree list --porcelain` and
`git status --short --branch`; do not leave stale task worktree registrations.
