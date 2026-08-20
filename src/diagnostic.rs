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
