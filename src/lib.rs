//! Version-aware native Podman inspection and deployment planning.
//!
//! This first milestone establishes explicit, redacted connection configuration; validated
//! Libpod request and response contracts; and evidence-backed Podman target profiles. It does
//! not open a connection or decode runtime responses. Applications provide a
//! [`LibpodTransport`] implementation for their selected transport.

#![forbid(unsafe_code)]

pub mod connection;
pub mod diagnostic;
pub mod evidence;
pub mod transport;
pub mod version;

pub use connection::{
    ConnectionKind, ConnectionSpec, MutualTlsPolicy, OpaqueReference, SshConnection, TcpMutualTlsConnection,
    UnixConnection,
};
pub use diagnostic::{Diagnostic, DiagnosticCode, PodmanLensResult};
pub use evidence::{CapabilityCatalogueEntry, EvidenceReference, capability_catalogue};
pub use transport::{
    LibpodHeader, LibpodHeaders, LibpodMethod, LibpodPath, LibpodRequest, LibpodResponse, LibpodTransport,
    LibpodTransportFuture, MAX_PATH_AND_QUERY_BYTES, TransportError, TransportLimits,
};
pub use version::{ObservedApiVersion, ObservedPodmanVersion, SupportedPodmanRange, TargetProfile};
