# Release process

Shared release preparation runs release-plz with the repository-scoped App token to propose crate
versions and changelog updates. The protected `Release` workflow remains the only publisher,
tagger, attestation producer and GitHub release creator. Shared automation is development tooling;
PodmanLens has no BoxFerry product dependency.

`validation_only` executes the same validation path and read-only publication inventory without
publication permissions or credentials. Ordinary dependency updates require no routine human
approval; software publication still requires approval of the protected release environment.

## Validate the exact candidate

Release invokes complete reusable CI with published API purpose, release metadata checks, native
conformance and published SemVer validation. The shared final gate directly requires all five
repository prerequisites: deterministic CI, metadata, native conformance, SemVer and native evidence
validation. Each successful result and evidence receipt binds the exact candidate. Native evidence
also binds the current run and attempt; cleanup must succeed before native candidate output exists.

Development CI compares against the trusted canonical Git base, without historical breaking-marker
allowances. Release and validation-only compare against the canonical published crate. Only distinct
successful published-purpose API evidence for the exact candidate satisfies the release guard.
Missing published evidence never becomes proof through a generic candidate output.

Native release admission checks out the exact SHA and verifies the current default-branch head
before privileged cells start. `release_validation=true` cannot select the manual issue-branch path.
A diagnostic issue-branch run never replaces fresh Release evidence. Native qualification retains
both root modes, pinned RPM/compose provenance, built image identity, installed package closure,
privacy checks, budgets and cleanup.

After a failure, dispatch a complete new Release run on current `main`; rerunning failed jobs can
omit native cells whose earlier-attempt evidence is inadmissible. Never reuse those artifacts or
weaken validation. The ordinary `scripts/check-native-release-contract.sh` also remains mandatory.

## Prepare and review a release

Only merged `feat`, `fix`, `perf`, `refactor` and `revert` changes create release proposals.
Documentation, tests, CI, build tooling and maintenance use non-release commit types.
Intentional API breaks follow [API stability](api-stability.md), including the next pre-1.0 minor
version and migration notes.

Keep user-visible notes in one `Unreleased` section. Product PRs neither bump versions nor create
future dated sections. release-plz inserts the dated heading once above the curated notes.
`check-release-metadata.sh --development` permits pending notes in ordinary CI;
`--release` requires an empty `Unreleased` for release-plz and Release/validation-only.

Review the proposed version, matching changelog section, package, API compatibility, MSRV,
dependencies, coverage and native evidence. The candidate is checked out with credentials disabled
before its native-evidence validator runs. No publication credential belongs in repository secrets.

## Publish

Merge the reviewed release PR, dispatch the protected workflow, review complete verification, then
approve the release environment. Shared idempotent crate mechanics retain first-publication policy
and exact-candidate checks. PodmanLens retains tags, checksum assets, attestations and GitHub release
creation. crates.io uses trusted publishing; never bootstrap it with a stored token.
