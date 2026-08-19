# Testing

Every implementation change needs focused positive and negative coverage. Keep protocol fixtures
offline, minimal, and provenance-bearing. Never embed live connection endpoints, environment
values, secret payloads, or credentials.

Use unit tests for pure typed conversion and graph rules. Use integration tests for public APIs,
resource discovery, version boundaries, and rendered deployment plans. Tests that require a live
Podman service must be explicitly opt-in and must never be part of the ordinary deterministic gate.

Run the complete local gate with `./scripts/check-all.sh`. It formats tracked files, runs Rust and
repository-policy tests, measures coverage without inventing an initial threshold, validates the
MSRV, checks dependencies, and checks local documentation links offline.
