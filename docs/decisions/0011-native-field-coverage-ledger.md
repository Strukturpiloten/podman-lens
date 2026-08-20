# 0011: Native field coverage is an explicit, strict ledger

- Status: Accepted
- Date: 2026-08-20

## Context

Podman inspect responses contain a large and evolving native surface. Treating a containing object
as known while decoding only one member silently loses configuration. Retaining arbitrary unknown
metadata also has a bounded memory budget, so a retained list cannot imply an exhaustive account
after overflow.

## Decision

`catalogue/v1/native-field-coverage.json` is a strict, packaged ledger for every M2 native input
field currently accepted by the inventory decoder. It is not an M6 output coverage claim. Each deterministic row states its native path,
coverage classification, decoder owner, planner and renderer applicability, public API contract,
diagnostic, and focused positive and negative test identifiers. The embedded parser rejects an
unknown JSON key, altered semantic link, reordered or missing expected row, duplicate identifier,
or an unsupported coverage classification. Every row is compared to complete compiled expected
metadata; syntactically plausible substitutions are not accepted.

An accepted object is never a blanket acceptance for its descendants. A typed member is modeled;
every other direct member is retained as `UnknownNativeField` metadata and receives `PLN0023`.
`HostConfig.MemorySwappiness` is the initial enforced example. `Secret.Spec.Driver` is an explicit
typed metadata field. Secret payload material remains a manual, redacted boundary and is never
retained.

Unknown metadata remains bounded. `ResourceRecord::unknown_fields_complete()` is false for a
partial inspection or whenever `PLN0021` reports overflow. Consumers must not mistake the retained
slice for complete native configuration in either case.

## Consequences

- A decoder change must update the strict ledger and its focused test references in the same pull
  request.
- Inventory fields with no output mapping are visible as observation-only rather than implied
  conversion support.
- Future output work can replace `not_applicable` planner or renderer references only when exact
  semantic and rendering evidence exists.
- M6-B3 semantic intent does not create ledger output rows. Those rows are added only with the
  B3b exact per-version renderer evidence.
- The bounded unknown-field policy stays explicit without retaining raw values or secret material.
