# 0019: Shared validation and dependency automation

- Status: Accepted
- Date: 2026-11-03

## Decision

PodmanLens consumes the immutable BoxFerry automation interface as development tooling only.
The product library remains independent under ADR 0001. Repository-owned task argv preserve
Rust, package, coverage, API, MSRV, file and native contracts. Policies stay schema 2; shared
plans use schema 3 and bind the exact tested candidate separately from trusted base policy.
Unknown changes and main, manual, local full and release validation retain the complete gate.
Maintenance prose in the exact `README.md` and `docs/README.md` allowlist uses file and offline-link
checks only when complete before/after evidence proves an unambiguous, unchanged code projection.
Added or deleted allowlisted files, changed or ambiguous code, other maintenance paths and mixed
source changes retain complete validation. Public guides retain the targeted guide contract;
`docs/public/development/` retains complete validation. This changes iteration and trusted PR/queue
selection only: main, manual, release and the final complete local gate remain complete.

API validation explicitly separates purpose. Ordinary CI and local development compare with a
trusted Git base selected by shared automation; Release and validation-only compare against the
canonical published crate baseline, inferring the release allowance from the candidate manifest.
Only published-purpose success exposes API proof. The CI workflow exposes its existing checked
proof as `deterministic_published_api_candidate_sha`; its internal gate retains
`published_api_candidate_sha`. The release aggregate receives exactly one prerequisite declaring
`published_api_candidate_sha`, from the published-purpose SemVer job. The local release gate still
requires both the deterministic and SemVer published proofs to match the exact candidate before
exposing its candidate receipt. All five original release prerequisites retain their success and
exact-candidate checks. Neither proof is derived from a generic candidate output. Local wrappers
use the same selected shared module through declared policy tasks, without another baseline
selector. Development uses the current event's intent and the whole proposal's trusted Git base;
default-branch pushes compare the nonzero event `before` across the entire push. Local checks use
the fixed minor allowance and never infer permission from historical breaking commits. Published
checks accept no intent override; the chosen manifest version owns the release allowance.
This refines the API task contract above without changing native qualification.

A default-branch dependency eligibility service runs on CI workflow events and schedule and reads
authoritative PR and queue metadata,
persists first-seen evidence, and writes receipts. Candidate CI has read-only permissions and
waits for the exact tested SHA. Admission resolves authority A from the manifest and policy Git
objects at the trusted consumer base, even when a candidate proposes setup B. Third-party updates retain 72-hour admission; isolated first-party
release updates include necessary transitive resolution without an age hold after trusted checks.
Unavailable, malformed, stale and mismatched evidence fails closed. Dependency updates need green
integrity and compatibility evidence, with no routine human approval. Software releases still
require the protected approved release environment.

The maintainer keeps the qualified Podman native runtime fixed for this overhaul. The complete
coupled tuple in `scripts/native-runtime-pins.sh` covers the engine, binary/source RPMs, source
revision/archive, base image and fixed compose metadata. Renovate records its existing extractors
for policy validation but disables their exact package/datasource/file owners with `enabled:false`;
it creates no routine update or manual-review proposal for this tuple. Changing the target requires
a separate qualification change with verified provenance and both root modes. This supersedes
ADR 0017's routine Renovate signal/manual-review policy only. Its native admission boundary,
read-only acquisition, budgets, privacy, cleanup and mandatory Release qualification remain.
Historical captures and provenance retain their original versions and hashes. Ordinary Rust,
compiler and development-tool updates continue through automatic shared policy.

Ordinary adoption preserves every existing main check and uses the strict generic Required CI
gate. It adds no Dev Container job, seed receipt requirement, or local image leaf. Direct image
inputs, feature declarations, lifecycle scripts and the image-only legacy tool installer retain
their current-main bytes. Their existing extraction owners remain unique, with a late
`enabled:false` hold until separate CI image qualification under BoxFerry #563; no manual
approval or dependency-dashboard route replaces that hold. The coupled native tuple above remains
fixed independently. Ordinary shared automation, package locks and compiler declarations retain
one automatic owner even where the existing Dockerfile copies them. Such updates do not qualify
the resulting image closure.

Ordinary host and CI callers use `scripts/install-host-tools.sh` and the selected shared tool
inventory. `scripts/install-file-tools.sh` serves only the retained image Dockerfile. Contributors
use VS Code Dev Containers with their existing Podman storage, preferably rootless; this adoption
adds no private store, host sudo requirement or image lifecycle. Fresh GitHub CI runners will
qualify images with standard engine storage in the separate image work.

API validation uses explicit purposes. Development compares against the trusted canonical Git
base; published purpose compares against the canonical released crate and derives allowance from
the candidate manifest. Release and validation-only require successful published-purpose evidence
bound to the exact candidate from both CI and SemVer. Generic candidate outputs cannot substitute
for published API proof. Shared release mechanics preserve PodmanLens first-publication policy.

Image validation lanes and the feature updater remain deferred to separate qualified changes.
This adoption does not build, run, publish or qualify an image, or activate the updater.

Coverage tool and compiler selection now use the candidate's own annotated Dockerfile ARG and
exact Rust channel in local, shared CI and reused release validation. Local preflight verifies
existing dispatch in the measurement Cargo home, without installing or using a producer fallback;
cleanup and measurement retain that home, the selected compiler and the original argv. Shared
CI installs and verifies that same candidate selection. The one existing coverage ARG extractor
remains automatic under the shared age and approval policy through an exact crate/path/manager
exception to the image hold. Other image/native holds and qualification requirements remain.

Shared Rust quality, coverage and MSRV callers opt into explicit trusted tasks. Strict
preflight rejects a missing or malformed task before Rust setup, and execution repeats that
selection. All three tasks retain their configured command order and 1800-second budgets;
MSRV already has an explicit policy definition. This changes no native or image qualification
caller, coverage behavior, dependency-age policy or release prerequisite.

## Migration and rollback

Land the trusted service, automation manifest and additive task policy first, preserving the legacy planner and every
complete job. Its parser accepts only the additional `tasks` key and never executes those tasks.
Bootstrap merge queue events select every existing job and reject every missing or failed result.
Then adopt the shared planner, read-only receipt waiter, Rust mechanics and shared Renovate presets
at one verified immutable foundation revision. Never require a service absent from trusted main.
Rollback the caller adoption as one change while retaining the trusted service and full legacy gate;
do not weaken age, integrity or Required CI checks. No publication runs as part of migration.

Native version inputs, bodyless-GET acquisition, both root modes, candidate/run/attempt binding,
privacy, resource budgets and cleanup remain repository-owned under ADRs 0016 and 0017.
