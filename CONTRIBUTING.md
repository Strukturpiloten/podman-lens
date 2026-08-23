# Contributing to PodmanLens

PodmanLens accepts focused changes that preserve its read-only input and non-executing output
boundaries. Start with the task routing in [AGENTS.md](AGENTS.md), then read the relevant
architecture decision and public contract.

## Development environment

The recommended environment is the repository's VS Code Dev Container. It provides the pinned Rust
toolchain and all validation tools while keeping Cargo state in writable container volumes.

Outside VS Code, start the same environment with:

```console
npx --yes @devcontainers/cli@0.83.0 up --workspace-folder .
```

## Development loop

1. Make one focused change.
2. Add positive and negative tests.
3. Update native evidence, catalogues, schemas, and documentation when their contracts change.
4. Run the complete gate:

   ```console
   ./scripts/check-all.sh
   ```

The gate formats tracked files, checks Rust and repository policies, runs tests and doctests,
builds Rustdoc, verifies the package, audits dependencies, and checks local documentation links.
Any edit after a successful final run invalidates that result.

Use `scripts/check-files.sh --fix` for Markdown, JSON, YAML, TOML, and shell formatting while
working.

## Evidence and fixture rules

Do not add undocumented Libpod behavior. Record the upstream source, version, and fixture
provenance before supporting a field or endpoint. Fixtures must be deterministic, synthetic or
sanitized, and free of live endpoints, environment values, secret payloads, and credentials.

Current behavior belongs in architecture, Rustdoc, tests, and machine catalogues. Historical
decisions belong in ADRs; completed work does not need to remain as a prose implementation ledger.

Contributions use the Mozilla Public License 2.0. By submitting a contribution, you license it
under the repository's [LICENSE](LICENSE).
