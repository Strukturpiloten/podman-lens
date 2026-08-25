# 0011: Native field coverage is an explicit, strict ledger

- Status: Accepted
- Date: 2026-08-20

## Context

Podman inspect responses contain a large and evolving native surface. Treating a containing object
as known while decoding only one member silently loses configuration. Retaining arbitrary unknown
metadata also has a bounded memory budget, so a retained list cannot imply an exhaustive account
after overflow.

## Decision

`catalogue/v1/native-field-coverage.json` is a strict, packaged two-plane ledger. Its input-
observation rows cover every M2 native field accepted by the inventory decoder. Its output-intent
rows cover B3b runtime fields and the complete M6-B4 mount, secret-grant, volume-ownership, and
image-acquisition surface with exact planner, CLI, and Libpod rendering evidence. Each deterministic
row states its field path, coverage classification, observation owner, planner owner, separate CLI
and Libpod renderer owners, reviewed target applicability, public API contract, diagnostic, and
focused positive and negative test identifiers. The embedded parser rejects an unknown JSON key,
altered semantic link, reordered or missing expected row, duplicate identifier, fabricated target
availability, or an unsupported coverage classification. Every row is compared to complete compiled
expected metadata; syntactically plausible substitutions are not accepted.

An accepted object is never a blanket acceptance for its descendants. A typed member is modeled;
every other direct member is retained as `UnmodelledField` metadata and receives `PLN0023`.
`HostConfig.MemorySwappiness` is the initial enforced example. `Secret.Spec.Driver` is an explicit
typed metadata object. Its option names and values are discarded immediately; only option state,
provenance, and count survive. Secret payload material remains a manual, redacted boundary and is
never retained.

Unknown metadata remains bounded. `ObservationHeader::unmodelled_completeness()` is incomplete for
an unavailable or malformed inspection or whenever `PLN0021` reports overflow. Consumers must not
mistake the retained slice for complete native configuration in either case.

## Consequences

- A decoder change must update the strict ledger and its focused test references in the same pull
  request.
- Inventory fields with no output mapping are visible as observation-only rather than implied
  conversion support.
- Future output work adds output-intent rows only when exact semantic, CLI, Libpod, and
  per-target rendering evidence exists.
- M6-B3b adds 32 exact public runtime rows plus two manual, redacted health-command boundaries.
  Sensitive or externally supplied health commands have no payload rendering claim and block the
  complete resource artifact with `PLN0046` on every reviewed target.
- M6-B4 adds 16 output rows: named-volume copy, subpath-copy, and no-copy mounts; bind and tmpfs
  mounts; mount and environment secret grants; 5.6+ UID/GID and all four explicit pull policies;
  plus manual source-portability, no-copy-subpath, and pod-infra-mount boundaries. M7-B2a adds ten
  input-only native network IPAM and route rows. M7-B2b adds 22 pod-infra and unpodded-container
  networking rows. M7-B3a adds 20 observation-only restart, health, and logging rows, including
  parent objects. M7-B3b adds 18 security, namespace, and resource-control rows. M7-B4 adds 16
  input rows for volume UID/GID, driver, creation time, and anonymous state; image repository
  digests, digest, creation time, author, architecture, operating system, and manifest type; and
  secret driver name/options plus creation/update times. It also corrects the existing image
  reference row to the native `RepoTags` field. Repository tags and digests remain separate
  local-resolution evidence, native timestamps retain their exact validated RFC 3339 spelling,
  and secret driver options are count-only. This avoids silently accepting nested descendants.
  The ledger separately records every reviewed input-observation and output-intent field. Every reviewed
  line records
  immutable B4 evidence with mutually exclusive exact, target-gated, manual, and blocked sets.
- The bounded unknown-field policy stays explicit without retaining raw values or secret material.
