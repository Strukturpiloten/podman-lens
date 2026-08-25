# Architecture decisions

Accepted decisions define the implementation boundary. Supersede an accepted decision explicitly;
do not let code silently change it.

| Decision                                                                                                                   | Status   |
| -------------------------------------------------------------------------------------------------------------------------- | -------- |
| [0001: Independent native Podman library](0001-independent-native-podman-library.md)                                       | Accepted |
| [0002: Ordered transport-neutral deployment plans](0002-ordered-transport-neutral-deployment-plans.md)                     | Accepted |
| [0003: Evidence-backed resource discovery](0003-evidence-backed-resource-discovery.md)                                     | Accepted |
| [0004: Resource groups and pod layout are separate](0004-resource-groups-and-pod-layout.md)                                | Accepted |
| [0005: Sensitive values require explicit authorization](0005-sensitive-values-require-authorization.md)                    | Accepted |
| [0006: Compatibility is explicit and evidence-backed](0006-explicit-version-compatibility.md)                              | Accepted |
| [0007: Explicit endpoints and read-only acquisition transport](0007-explicit-endpoints-and-caller-owned-transport.md)      | Accepted |
| [0008: Versioned observational snapshots are serialization-only and always redacted](0008-versioned-redacted-snapshots.md) | Accepted |
| [0009: Explicit image acquisition and pod-start lifting](0009-explicit-image-acquisition-and-pod-start-lifting.md)         | Accepted |
| [0010: Versioned non-executing output rendering](0010-versioned-nonexecuting-output-rendering.md)                          | Accepted |
| [0011: Native field coverage is an explicit, strict ledger](0011-native-field-coverage-ledger.md)                          | Accepted |
| [0012: Bounded runtime intent before rendering](0012-bounded-runtime-intent-before-rendering.md)                           | Accepted |
| [0013: Typed native observations preserve state and provenance](0013-typed-native-observations.md)                         | Accepted |
| [0014: Finite input-only Podman anchors remain separate from output targets](0014-finite-input-only-podman-anchors.md)     | Accepted |
