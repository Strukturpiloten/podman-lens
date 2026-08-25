# Changelog

All notable changes to PodmanLens are documented here. The project follows the pre-1.0 policy in
[API stability](docs/api-stability.md).

## [Unreleased]

## [0.2.1](https://github.com/Strukturpiloten/podman-lens/compare/v0.2.0...v0.2.1) - 2026-08-25

### Fixed

- accept valid runtime network and swappiness evidence ([#15](https://github.com/Strukturpiloten/podman-lens/pull/15))

### Fixed

- Normalize the valid `HostConfig.MemorySwappiness=-1` Podman inspect sentinel as absent
  system-default intent instead of reporting malformed input.
- Preserve named effective attachments from a standalone container's
  `NetworkSettings.Networks` map while continuing to leave pod-member runtime networking
  inapplicable.

## [0.2.0](https://github.com/Strukturpiloten/podman-lens/compare/v0.1.1...v0.2.0) - 2026-08-25

### Added

- [**breaking**] support finite legacy Podman input anchors ([#12](https://github.com/Strukturpiloten/podman-lens/pull/12))

### Changed

- Separate current architecture, API, testing, security, release, and roadmap guidance from
  milestone history; remove duplicate compatibility and BoxFerry mapping pages while retaining
  public guides and machine-readable evidence
  ([#10](https://github.com/Strukturpiloten/podman-lens/pull/10)).

- distinguish finite legacy input-only Podman anchors from reviewed rendering targets; acquire
  Podman 3.0.1, 3.4.4, 4.3.1, 4.9.3, and 4.9.4 without allowing them as output targets.

  `ServiceObservation::target_profile` is replaced with `input_capability` and
  `output_target_profile`. Callers importing legacy runtime state must select their own modern
  `TargetProfile` before creating a deployment plan.

### Fixed

- preserve single complete Compose-label namespaces as advisory grouping evidence and retain
  bounded structural metadata for runtime network attachments.
- decode Debian 11 / Podman 3.0.1 API-3 volume and CNI-network responses, including its absent
  secret endpoint and empty default container fields.
- apply the repository's pre-1.0 breaking-change policy consistently in pull-request and protected
  release API checks.

## [0.1.1](https://github.com/Strukturpiloten/podman-lens/compare/v0.1.0...v0.1.1) - 2026-08-21

### Fixed

- strengthen versioned Podman conformance ([#5](https://github.com/Strukturpiloten/podman-lens/pull/5))
