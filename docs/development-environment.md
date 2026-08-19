# Development environment

The Dev Container provides the pinned Rust toolchain and all quality tools. It keeps Cargo registry
and build artifacts in per-container writable volumes so tool locks never use the read-only image
cache.

Open the repository in VS Code and choose **Reopen in Container**, then run:

```shell
./scripts/check-all.sh
```

Outside VS Code, rebuild with the current Dev Container CLI:

```shell
npx --yes @devcontainers/cli@0.83.0 up --workspace-folder .
```

The local helper `scripts/check-files.sh --fix` formats Markdown, JSON, YAML, TOML, and shell
files. Complete YAML documents must start with `---`.
