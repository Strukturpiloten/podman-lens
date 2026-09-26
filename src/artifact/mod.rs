//! Versioned, serialization-only deployment artifacts.

/// Version 1 deployment-rendering artifact.
pub mod deployment_v1;
/// Version 2 deployment-rendering artifact with a protected-value indicator.
pub mod deployment_v2;

mod wire;
