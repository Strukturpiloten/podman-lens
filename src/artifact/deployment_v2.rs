//! Version 2 serialization-only deployment artifact with an explicit sensitivity indicator.

use serde::{Serialize, Serializer, ser::SerializeStruct};

use super::wire::{ArtifactData, status};
use crate::DeploymentRendering;

/// Schema version of deployment artifacts that can carry authorized protected inline values.
pub const SCHEMA_VERSION: u8 = 2;

/// Returns a version 2 deployment artifact.
///
/// The serialized bytes may contain protected inline environment values when the source
/// rendering includes them. The explicit sensitivity indicator is derived from actual rendered
/// values, not simply from the caller's authorization grant.
#[must_use]
pub fn deployment(source: &DeploymentRendering) -> DeploymentArtifact {
    DeploymentArtifact::from_rendering(source)
}

/// Version 2 artifact, with a consumer-visible protected-value indicator.
///
/// Serialized bytes are inert but can be sensitive. `Debug` deliberately omits their content.
pub struct DeploymentArtifact {
    status: &'static str,
    contains_protected_inline_environment: bool,
    data: ArtifactData,
}

impl std::fmt::Debug for DeploymentArtifact {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("DeploymentArtifact")
            .field("schema_version", &SCHEMA_VERSION)
            .field(
                "contains_protected_inline_environment",
                &self.contains_protected_inline_environment,
            )
            .field("status", &self.status)
            .field("operation_count", &self.data.operations.len())
            .finish()
    }
}

impl DeploymentArtifact {
    /// Builds a v2 artifact from a complete inert rendering.
    #[must_use]
    pub fn from_rendering(source: &DeploymentRendering) -> Self {
        Self {
            status: status(source.status()),
            contains_protected_inline_environment: source.contains_protected_inline_environment(),
            data: ArtifactData::from_rendering(source),
        }
    }

    /// Returns the v2 schema version.
    #[must_use]
    pub const fn schema_version(&self) -> u8 {
        SCHEMA_VERSION
    }

    /// Reports whether this artifact contains protected inline environment values.
    #[must_use]
    pub const fn contains_protected_inline_environment(&self) -> bool {
        self.contains_protected_inline_environment
    }
}

impl Serialize for DeploymentArtifact {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut artifact = serializer.serialize_struct("DeploymentArtifact", 6)?;
        artifact.serialize_field("schema_version", &SCHEMA_VERSION)?;
        artifact.serialize_field(
            "contains_protected_inline_environment",
            &self.contains_protected_inline_environment,
        )?;
        artifact.serialize_field("status", &self.status)?;
        artifact.serialize_field("connection", &self.data.connection)?;
        artifact.serialize_field("external_preconditions", &self.data.external_preconditions)?;
        artifact.serialize_field("operations", &self.data.operations)?;
        artifact.end()
    }
}
