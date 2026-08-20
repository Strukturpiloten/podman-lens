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
