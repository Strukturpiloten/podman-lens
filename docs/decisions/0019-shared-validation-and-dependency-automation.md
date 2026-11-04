# 0019: Shared validation and dependency automation

- Status: Accepted
- Date: 2026-11-03

## Decision

PodmanLens consumes the immutable BoxFerry automation interface as development tooling only.
The product library remains independent under ADR 0001. Repository-owned task argv preserve
Rust, package, coverage, API, MSRV, file and native contracts. Policies stay schema 2; shared
plans use schema 3 and bind the exact tested candidate separately from trusted base policy.
Unknown changes and main, manual, local full and release validation retain the complete gate.
Public prose uses file and offline-link checks; executable examples retain the guide contract.

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
