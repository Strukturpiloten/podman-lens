# Release process

release-plz proposes crate versions and changelog updates. The protected `Release` workflow is
the only publisher, tagger, attestation producer, and GitHub release creator.

## Prepare a release-worthy change

Only merged `feat`, `fix`, `perf`, `refactor`, or `revert` commits create a release
proposal. Documentation, tests, CI, build tooling, formatting, and other maintenance use
`docs`, `test`, `ci`, `build`, `style`, or `chore`.

Use a breaking `!` only for an intentional public break that follows
[the API stability policy](api-stability.md).

## Review the release pull request

Before merging the generated `release-plz-*` pull request, verify that:

- the proposed version matches the change type;
- `CHANGELOG.md` contains a usable release section for that exact version;
- breaking changes include migration guidance;
- the package and compatibility checks pass; and
- no publication credential is present in repository secrets.

The changelog section is a release gate, not optional metadata.

## Publish

1. Merge the reviewed release-plz pull request.
2. Let it dispatch the protected `Release` workflow.
3. Review the complete verification result.
4. Approve the protected release environment.

The workflow verifies the package, API compatibility, MSRV, dependencies, coverage, checksum,
attestation, release tag, and draft GitHub release before publishing. crates.io authentication uses
trusted publishing; do not bootstrap publication with a stored crates.io token.
