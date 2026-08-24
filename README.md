# PodmanLens

PodmanLens is a Rust library for inspecting native Podman state and producing version-aware,
non-executing deployment plans. It is the Podman boundary used by
[BoxFerry](https://github.com/Strukturpiloten/boxferry), but it has no BoxFerry dependency.

PodmanLens can:

- inspect containers, pods, networks, volumes, images, and secret metadata through the Libpod API;
- preserve field state, provenance, and unsupported native evidence;
- discover deterministic resource groups and dependencies from explicit roots;
- validate caller-authored Podman deployment intent; and
- render deterministic CLI descriptions, Libpod request descriptions, JSON, and a review script.

It never discovers an ambient endpoint, invokes `podman` to acquire input, requests secret
payloads, executes a plan, or chooses a cross-format mapping.

## Try it

Use the repository's explicit-socket example for read-only acquisition:

```console
cargo run --example read_only_discovery -- /run/user/1000/podman/podman.sock
```

Use the offline example to build intent, plan it, and inspect both rendering planes:

```console
cargo run --example offline_plan_and_render
```

The first example requires a Unix Podman socket selected by the caller. The second opens no socket
and starts no process.

## Compatibility

PodmanLens has reviewed evidence for Podman 5.4 through 6.1. Input records the observed engine and
Libpod API versions. Output requires an explicit `TargetProfile`; support for individual fields
is version-specific.

See the [compatibility guide](docs/public/compatibility/index.md) for the human-readable summary
and `catalogue/v1/` for the machine-readable capability and rendering evidence.

## Documentation

- [Task-oriented guides](docs/public/index.md)
- [Rust API](https://docs.rs/podman-lens)
- [Architecture](docs/architecture.md)
- [API and schema stability](docs/api-stability.md)
- [Testing](docs/testing.md)
- [Roadmap](docs/roadmap.md)
- [Documentation index](docs/README.md)
- [Accepted decisions](docs/decisions/README.md)

Cross-format conversion policy belongs to BoxFerry. PodmanLens exposes typed native observations
and accepts explicit Podman output intent so a downstream adapter can keep that policy visible and
testable.

## Contributing

PodmanLens requires Rust 1.85.0 or newer. The Dev Container supplies the pinned development
toolchain. See [CONTRIBUTING.md](CONTRIBUTING.md) for the development loop and evidence rules.
