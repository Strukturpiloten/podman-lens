# Release process

release-plz prepares version and changelog pull requests only for merged release-worthy code
commits. The protected `Release` workflow remains the only publisher, tagger, and GitHub-release
creator. Merging the `release-plz-*` pull request automatically dispatches that protected workflow;
it must prove package, API, MSRV, dependency, coverage, checksum, attestation, tag, and draft
release provenance before publishing.

1. Merge a reviewed `feat`, `fix`, `perf`, `refactor`, or `revert` pull request.
2. Review and merge the generated `release-plz-*` pull request.
3. Approve the protected release environment after its complete verification succeeds.

Use `docs`, `test`, `ci`, `build`, `style`, or `chore` for changes that must not trigger a release.
The organization GitHub App needs **Contents** and **Pull requests** read/write permissions for the
release-preparation workflow. Trusted publishing belongs only to the protected release workflow;
never store a crates.io token in GitHub secrets.
