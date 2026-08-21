# Repository guidance for coding agents

This file applies to the whole PodmanLens repository.

## Read before changing code

1. `README.md`
2. `docs/architecture.md`
3. `docs/roadmap.md`
4. `docs/project-structure.md`
5. `docs/api-stability.md`
6. `docs/testing.md`
7. `docs/development-environment.md`
8. `docs/dependency-policy.md`
9. `docs/decisions/README.md` and every accepted decision

Architectural changes require documentation and an ADR update in the same change.

## Scope

PodmanLens owns native Libpod API handling, version evidence, typed native observations, resource
discovery, and ordered deployment plans. It does not depend on BoxFerry, parse Compose or Quadlet,
choose cross-format mappings, shell out to `podman` for input, or execute a plan.

## Non-negotiable behavior

- Treat runtime input as fallible; malformed data must produce structured failures, never panics.
- Preserve native evidence and unknown data long enough for callers to explain outcomes.
- Do not infer target behavior from the development machine.
- Keep transport replaceable and free from ambient connection discovery.
- Redact secrets and runtime environment values by default in diagnostics, logs, snapshots, and
  serialized plans.
- Start every repository-owned complete YAML document with `---`.

## Development rules

- Keep versioned protocol decoding, native models, discovery, and deployment planning separate.
- Add positive and negative tests for every supported field and capability boundary.
- Record Podman version, source, and fixture provenance for behavior claims.
- Keep public APIs independent from private protocol response types.
- Update capability data and documentation with every version-boundary change.
- Pin every GitHub Action to a full commit SHA and an exact release-tag comment.

## Canonical development commands

```shell
./scripts/check-all.sh
./scripts/check-files.sh --check
cargo fmt --all -- --check
cargo ci-check
cargo ci-policy
cargo ci-clippy
cargo ci-test
cargo ci-doctest
RUSTDOCFLAGS="-D warnings" cargo ci-doc
cargo +1.85.0 ci-check
cargo +1.85.0 ci-policy
cargo deny check
```

## Git and GitHub workflow

The primary Sol agent owns issue creation, branches, full validation, staging, commit, push, pull
request creation, and GitHub readback. Terra workers may research, implement bounded changes, or
verify, but never commit, push, publish, tag, or create GitHub objects. Run `./scripts/check-all.sh`
after the final edit; its success is a hard gate for a normal pull request.

Use `feat`, `fix`, `perf`, `refactor`, or `revert` only for release-worthy code. Use `docs`, `test`,
`ci`, `build`, `style`, or `chore` for maintenance so release-plz ignores it.
