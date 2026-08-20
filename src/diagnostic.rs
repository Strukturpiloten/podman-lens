//! Structured, non-sensitive diagnostics produced by `PodmanLens` contracts.

use std::{error::Error, fmt};

/// A stable identifier for a `PodmanLens` diagnostic class.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
#[non_exhaustive]
pub enum DiagnosticCode {
    /// An explicit connection specification is malformed or incomplete.
    InvalidConnection,
    /// A supplied transport cannot currently reach the selected service.
    TransportUnavailable,
    /// A Libpod request or response exceeds this library's contract boundaries.
    InvalidTransportMessage,
    /// A reported or requested version is malformed.
    InvalidVersion,
    /// A requested target profile is outside reviewed compatibility evidence.
    IncompatibleTargetProfile,
    /// The embedded evidence catalogue cannot be decoded.
    CatalogueUnavailable,
    /// A Libpod probe received an unexpected HTTP status.
    ProbeHttpStatus,
    /// A Libpod probe response header is missing, duplicated, or malformed.
    ProbeHeader,
    /// A Libpod probe response body is not valid bounded JSON.
    ProbeJson,
    /// A Libpod probe JSON document has an unsupported shape.
    ProbeShape,
    /// A Libpod probe does not contain one valid Podman Engine component.
    ProbeComponent,
    /// Observed engine and API versions are outside reviewed compatibility evidence.
    ObservedCompatibility,
    /// A Libpod inventory request received an unexpected HTTP status.
    InventoryHttpStatus,
    /// A Libpod inventory response is not valid bounded JSON.
    InventoryJson,
    /// A Libpod inventory response has an unsupported JSON shape.
    InventoryShape,
    /// A listed resource could no longer be inspected.
    ResourceUnavailable,
    /// An individual resource response is malformed.
    ResourceMalformed,
    /// A secret metadata response unexpectedly included payload material.
    SecretPayloadDiscarded,
    /// A runtime environment entry has an unsupported spelling.
    EnvironmentMalformed,
    /// Native fields contain contradictory relationship evidence.
    RelationshipConflict,
    /// Unknown-field metadata reached a configured safe retention limit.
    UnknownFieldOverflow,
    /// A field is not representable for the observed Libpod API version.
    VersionInapplicableField,
    /// A retained unknown native field has no typed M2 representation.
    NativeFieldUnsupported,
    /// A relationship cannot be resolved in an available target section.
    UnresolvedRelationship,
    /// Pod and container membership evidence disagrees.
    PodMembershipConflict,
    /// The observed version lacks matching immutable capability evidence.
    InventoryEvidenceUnavailable,
    /// A resource-discovery selector or boundary override is invalid.
    InvalidDiscoveryRequest,
    /// A requested discovery selector did not match an available native resource.
    SelectorUnresolved,
    /// A requested discovery selector matched more than one native resource.
    SelectorAmbiguous,
    /// Compose ownership labels are incomplete or empty and cannot group resources.
    AdvisoryLabelIncomplete,
    /// Docker and Podman Compose ownership labels disagree and cannot group resources.
    AdvisoryLabelConflict,
    /// A native relationship reference matched more than one resource.
    RelationshipAmbiguous,
    /// A valid network-boundary override did not cross any selected dependency boundary.
    BoundaryOverrideUnused,
    /// A typed deployment intent is malformed or violates a required resource-kind boundary.
    InvalidDeploymentIntent,
    /// A deployment intent repeats a resource identity or one exact prerequisite.
    DeploymentDuplicateResource,
    /// Two target-side declarations conflict for the same stable identity.
    DeploymentConflictingResource,
    /// A deployment intent refers to a resource declaration that is not present.
    DeploymentUnresolvedPrerequisite,
    /// A deployment intent combines otherwise supported concepts in an unsupported way.
    DeploymentUnsupportedCombination,
    /// Semantic deployment operations contain a dependency cycle.
    DeploymentCycle,
    /// A deployment intent attempts to embed sensitive bytes rather than an external reference.
    SensitivePayloadEmbedded,
    /// An image source cannot be represented as one strict native image reference.
    InvalidImageReference,
    /// An external precondition is malformed or conflicts with a managed resource.
    InvalidExternalPrecondition,
    /// Explicit target-side pod membership is incomplete or disagrees with a container declaration.
    DeploymentPodMembership,
    /// A requested startup order cannot be represented inside one Podman pod.
    SamePodStartupDependency,
}

impl DiagnosticCode {
    /// Returns the stable machine-readable rule code.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::InvalidConnection => "PLN0001",
            Self::TransportUnavailable => "PLN0002",
            Self::InvalidTransportMessage => "PLN0003",
            Self::InvalidVersion => "PLN0004",
            Self::IncompatibleTargetProfile => "PLN0005",
            Self::CatalogueUnavailable => "PLN0006",
            Self::ProbeHttpStatus => "PLN0007",
            Self::ProbeHeader => "PLN0008",
            Self::ProbeJson => "PLN0009",
            Self::ProbeShape => "PLN0010",
            Self::ProbeComponent => "PLN0011",
            Self::ObservedCompatibility => "PLN0012",
            Self::InventoryHttpStatus => "PLN0013",
            Self::InventoryJson => "PLN0014",
            Self::InventoryShape => "PLN0015",
            Self::ResourceUnavailable => "PLN0016",
            Self::ResourceMalformed => "PLN0017",
            Self::SecretPayloadDiscarded => "PLN0018",
            Self::EnvironmentMalformed => "PLN0019",
            Self::RelationshipConflict => "PLN0020",
            Self::UnknownFieldOverflow => "PLN0021",
            Self::VersionInapplicableField => "PLN0022",
            Self::NativeFieldUnsupported => "PLN0023",
            Self::UnresolvedRelationship => "PLN0024",
            Self::PodMembershipConflict => "PLN0025",
            Self::InventoryEvidenceUnavailable => "PLN0026",
            Self::InvalidDiscoveryRequest => "PLN0027",
            Self::SelectorUnresolved => "PLN0028",
            Self::SelectorAmbiguous => "PLN0029",
            Self::AdvisoryLabelIncomplete => "PLN0030",
            Self::AdvisoryLabelConflict => "PLN0031",
            Self::RelationshipAmbiguous => "PLN0032",
            Self::BoundaryOverrideUnused => "PLN0033",
            Self::InvalidDeploymentIntent => "PLN0034",
            Self::DeploymentDuplicateResource => "PLN0035",
            Self::DeploymentConflictingResource => "PLN0036",
            Self::DeploymentUnresolvedPrerequisite => "PLN0037",
            Self::DeploymentUnsupportedCombination => "PLN0038",
            Self::DeploymentCycle => "PLN0039",
            Self::SensitivePayloadEmbedded => "PLN0040",
            Self::InvalidImageReference => "PLN0041",
            Self::InvalidExternalPrecondition => "PLN0042",
            Self::DeploymentPodMembership => "PLN0043",
            Self::SamePodStartupDependency => "PLN0044",
        }
    }
}

impl fmt::Display for DiagnosticCode {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// A redacted diagnostic with a stable code and human-facing explanation.
///
/// It deliberately does not retain rejected endpoint strings, credentials, request bodies, or
/// response data. Callers can safely display, log, or serialize the code and message.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Diagnostic {
    code: DiagnosticCode,
    message: &'static str,
}

impl Diagnostic {
    /// Creates a diagnostic from one of the library's stable codes.
    #[must_use]
    pub const fn new(code: DiagnosticCode) -> Self {
        Self {
            code,
            message: match code {
                DiagnosticCode::InvalidConnection => "the explicit Podman connection is invalid",
                DiagnosticCode::TransportUnavailable => "the selected Podman transport is unavailable",
                DiagnosticCode::InvalidTransportMessage => {
                    "the Libpod transport message is outside supported safety bounds"
                }
                DiagnosticCode::InvalidVersion => "the Podman or Libpod API version is invalid",
                DiagnosticCode::IncompatibleTargetProfile => {
                    "the requested Podman target profile is not supported by reviewed evidence"
                }
                DiagnosticCode::CatalogueUnavailable => "the embedded Podman compatibility catalogue is unavailable",
                DiagnosticCode::ProbeHttpStatus => "the Libpod probe received an unexpected HTTP status",
                DiagnosticCode::ProbeHeader => "the Libpod probe response header is invalid",
                DiagnosticCode::ProbeJson => "the Libpod probe response JSON is invalid or exceeds its safety bound",
                DiagnosticCode::ProbeShape => "the Libpod probe response has an unsupported JSON shape",
                DiagnosticCode::ProbeComponent => {
                    "the Libpod probe response does not identify one valid Podman Engine component"
                }
                DiagnosticCode::ObservedCompatibility => {
                    "the observed Podman engine and Libpod API versions are outside reviewed evidence"
                }
                DiagnosticCode::InventoryHttpStatus => "a Libpod inventory request received an unexpected HTTP status",
                DiagnosticCode::InventoryJson => "a Libpod inventory response is invalid or exceeds its safety bound",
                DiagnosticCode::InventoryShape => "a Libpod inventory response has an unsupported JSON shape",
                DiagnosticCode::ResourceUnavailable => {
                    "a listed Podman resource was unavailable during non-atomic inspection"
                }
                DiagnosticCode::ResourceMalformed => "a Podman resource response is malformed",
                DiagnosticCode::SecretPayloadDiscarded => {
                    "unexpected secret payload material was discarded from metadata inspection"
                }
                DiagnosticCode::EnvironmentMalformed => "a Podman runtime environment entry is malformed",
                DiagnosticCode::RelationshipConflict => "a Podman resource contains conflicting relationship evidence",
                DiagnosticCode::UnknownFieldOverflow => "unknown Podman field metadata exceeded a safe retention limit",
                DiagnosticCode::VersionInapplicableField => {
                    "a Podman field is inapplicable for the observed Libpod API version"
                }
                DiagnosticCode::NativeFieldUnsupported => "a Podman native field is retained as unsupported metadata",
                DiagnosticCode::UnresolvedRelationship => "a Podman native relationship could not be resolved",
                DiagnosticCode::PodMembershipConflict => "Podman pod and container membership evidence disagrees",
                DiagnosticCode::InventoryEvidenceUnavailable => {
                    "the observed Podman version has no matching immutable inventory evidence"
                }
                DiagnosticCode::InvalidDiscoveryRequest => "the resource-discovery request is invalid",
                DiagnosticCode::SelectorUnresolved => "a requested resource selector did not match inventory",
                DiagnosticCode::SelectorAmbiguous => "a requested resource selector matched multiple inventory records",
                DiagnosticCode::AdvisoryLabelIncomplete => {
                    "Compose ownership labels are incomplete or empty and cannot group resources"
                }
                DiagnosticCode::AdvisoryLabelConflict => {
                    "Docker and Podman Compose ownership labels disagree and cannot group resources"
                }
                DiagnosticCode::RelationshipAmbiguous => {
                    "a Podman native relationship reference matched more than one resource"
                }
                DiagnosticCode::BoundaryOverrideUnused => {
                    "a network-boundary override did not cross a selected dependency boundary"
                }
                DiagnosticCode::InvalidDeploymentIntent => "the typed Podman deployment intent is invalid",
                DiagnosticCode::DeploymentDuplicateResource => {
                    "a deployment resource identity or prerequisite is duplicated"
                }
                DiagnosticCode::DeploymentConflictingResource => {
                    "target-side declarations conflict for one resource identity"
                }
                DiagnosticCode::DeploymentUnresolvedPrerequisite => {
                    "a deployment prerequisite is not declared by the intent"
                }
                DiagnosticCode::DeploymentUnsupportedCombination => {
                    "a deployment intent uses an unsupported semantic combination"
                }
                DiagnosticCode::DeploymentCycle => "deployment operations contain a dependency cycle",
                DiagnosticCode::SensitivePayloadEmbedded => "a deployment plan cannot embed sensitive payload material",
                DiagnosticCode::InvalidImageReference => {
                    "an image source is not a strict supported Podman image reference"
                }
                DiagnosticCode::InvalidExternalPrecondition => "an external deployment precondition is invalid",
                DiagnosticCode::DeploymentPodMembership => {
                    "explicit target-side pod membership is incomplete or conflicting"
                }
                DiagnosticCode::SamePodStartupDependency => {
                    "a startup dependency cannot order containers within one Podman pod"
                }
            },
        }
    }

    /// Returns the stable rule code.
    #[must_use]
    pub const fn code(&self) -> DiagnosticCode {
        self.code
    }

    /// Returns the redacted explanation.
    #[must_use]
    pub const fn message(&self) -> &'static str {
        self.message
    }
}

impl fmt::Display for Diagnostic {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}: {}", self.code, self.message)
    }
}

impl Error for Diagnostic {}

/// The result type returned by fallible `PodmanLens` public contracts.
pub type PodmanLensResult<T> = Result<T, Diagnostic>;
