//! Version 1 serialization-only deployment artifact. Protected inline values are never emitted.

use serde::{Serialize, Serializer, ser::SerializeStruct};

use super::wire::{ArtifactData, status};
use crate::DeploymentRendering;

/// Schema version of the original public-only deployment artifact.
pub const SCHEMA_VERSION: u8 = 1;

/// Returns the original serialization-only deployment artifact.
///
/// The signature is retained for existing callers. If the rendering contains protected inline
/// environment values, serialization fails before writing any bytes. Use
/// [`super::deployment_v2::deployment`] when the caller has authorized sensitive artifact bytes.
#[must_use]
pub fn deployment(source: &DeploymentRendering) -> DeploymentArtifact {
    DeploymentArtifact::from_rendering(source)
}

/// The original public-only deployment artifact. It deliberately does not deserialize.
///
/// `Debug` is redacted. A protected rendering cannot be serialized as v1, so consumers of the
/// original v1 schema cannot mistake an authorized value for ordinary public output.
pub struct DeploymentArtifact {
    status: &'static str,
    data: Option<ArtifactData>,
}

impl std::fmt::Debug for DeploymentArtifact {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("DeploymentArtifact")
            .field("schema_version", &SCHEMA_VERSION)
            .field("status", &self.status)
            .field("serialization_allowed", &self.data.is_some())
            .finish()
    }
}

impl DeploymentArtifact {
    /// Builds a v1 artifact wrapper. Protected rendering bytes are not copied into this wrapper.
    #[must_use]
    pub fn from_rendering(source: &DeploymentRendering) -> Self {
        Self {
            status: status(source.status()),
            data: (!source.contains_protected_inline_environment()).then(|| ArtifactData::from_rendering(source)),
        }
    }

    /// Returns the v1 schema version. Serialization can still fail for protected renderings.
    #[must_use]
    pub const fn schema_version(&self) -> u8 {
        SCHEMA_VERSION
    }
}

impl Serialize for DeploymentArtifact {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let Some(data) = &self.data else {
            return Err(serde::ser::Error::custom(
                "deployment v1 refuses protected inline values; use deployment v2",
            ));
        };
        let mut artifact = serializer.serialize_struct("DeploymentArtifact", 5)?;
        artifact.serialize_field("schema_version", &SCHEMA_VERSION)?;
        artifact.serialize_field("status", &self.status)?;
        artifact.serialize_field("connection", &data.connection)?;
        artifact.serialize_field("external_preconditions", &data.external_preconditions)?;
        artifact.serialize_field("operations", &data.operations)?;
        artifact.end()
    }
}
