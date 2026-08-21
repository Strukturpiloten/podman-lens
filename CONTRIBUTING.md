# Contributing to PodmanLens

Read [the architecture](docs/architecture.md), [accepted decisions](docs/decisions/README.md), and
[testing guidance](docs/testing.md) before proposing behavior changes.

## Development loop

Use the Dev Container, make one focused change, add focused positive and negative tests, then run:

```shell
./scripts/check-all.sh
```

Do not add undocumented Libpod behavior. Record source and version evidence before supporting a
field or endpoint. Never include secret payloads or real credentials in fixtures, snapshots, logs,
or pull requests.

Contributions use the Mozilla Public License 2.0. By submitting a contribution, you license it
under the repository's [LICENSE](LICENSE).
