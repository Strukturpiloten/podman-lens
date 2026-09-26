# Release process

release-plz proposes crate versions and changelog updates. The protected `Release` workflow is
the only publisher, tagger, attestation producer, and GitHub release creator. Its
`validation_only` dispatch runs the identical validation path but cannot enter the publication job
or receive its publication permissions.

The reusable CI caller always runs the complete candidate validation plan for Release, including
the offline native-evidence contract. The separate live native conformance worker remains an
additional fail-closed release gate; a focused pull-request plan never substitutes for either.
Release calls the reusable native worker with `release_validation=true`. Its separate admission
job checks out the exact SHA and verifies that the candidate still heads the current default
branch before the privileged matrix starts. The calling dispatch event cannot select the manual
issue-branch path. A reviewed issue-branch manual native run is useful pre-merge diagnostics but
is validation-only; its artifacts never replace fresh SHA/run/attempt-bound Release evidence.

## Prepare a release-worthy change

Only merged `feat`, `fix`, `perf`, `refactor`, or `revert` commits create a release
proposal. Documentation, tests, CI, build tooling, formatting, and other maintenance use
`docs`, `test`, `ci`, `build`, `style`, or `chore`.

Use a breaking `!` only for an intentional public break that follows
[the API stability policy](api-stability.md).

Record user-visible changes in the single `Unreleased` section. Ordinary product pull requests
must not create a future numbered section, set its date, or bump the crate version. Release-plz
inserts only the dated version heading, with blank lines on both sides; the curated notes below
`Unreleased` thereby become that release's notes exactly once, without duplicate `Added`,
`Changed`, or `Fixed` headings.

The API check reads all Conventional Commits since the latest release tag. A `!` subject marker or
`BREAKING CHANGE:` footer authorizes a break in the next pre-1.0 minor release. The wrapper passes
cargo-semver-checks its `major` category because that is the tool's name for an API break; this does
not change PodmanLens's pre-1.0 versioning policy.

## Review the release pull request

Before merging the generated `release-plz-*` pull request, verify that:

- the proposed version matches the change type;
- the newest numbered changelog section matches the crate version;
- `Unreleased` is empty when the proposed crate version is newer than the latest release tag;
- `CHANGELOG.md` contains a usable release section for that exact version;
- breaking changes include migration guidance;
- the package and compatibility checks pass; and
- `scripts/check-native-release-contract.sh` passes its offline privacy, provenance, replay,
  and semantic contracts;
- the reusable `Native Podman conformance` worker passed for the exact candidate SHA, current
  run ID, and current run attempt, with its SHA/run/attempt/task evidence artifact, pinned RPM and
  Fedora compose provenance, built image ID, installed package closure, and both root modes;
- the release gate checked out that exact candidate, with credentials disabled, before invoking
  the candidate-owned native-evidence validator; and
- no publication credential is present in repository secrets.

The changelog section is a release gate, not optional metadata.

## Publish

1. Merge the reviewed release-plz pull request.
2. Let it dispatch the protected `Release` workflow.
3. Review the complete verification result.
4. Approve the protected release environment.

The workflow verifies captured-native release evidence, the package, API compatibility, MSRV, dependencies, coverage, checksum,
attestation, release tag, and draft GitHub release before publishing. crates.io authentication uses
trusted publishing; do not bootstrap publication with a stored crates.io token.
