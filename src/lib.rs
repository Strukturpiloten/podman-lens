//! Version-aware native Podman inspection and deployment planning.
//!
//! This first milestone establishes explicit, redacted connection configuration; validated
//! Libpod request and response contracts; evidence-backed Podman target profiles; and a fixed
//! read-only service probe. On Unix, `ReadOnlyUnixTransport` can acquire from one explicit socket
//! and rejects every method except `GET` before opening it. Applications provide any SSH or TLS
//! [`LibpodTransport`] implementation themselves.

#![forbid(unsafe_code)]

/// Versioned, serialization-only deployment artifacts.
pub mod artifact;
pub mod connection;
pub mod coverage;
pub mod deployment;
pub mod diagnostic;
pub mod discovery;
pub mod evidence;
pub mod inventory;
pub mod networking;
pub mod probe;
#[cfg(unix)]
pub mod read_only_unix_transport;
pub mod render;
pub mod settings;
pub mod snapshot;
pub mod transport;
pub mod version;

pub use connection::{
    ConnectionKind, ConnectionSpec, MutualTlsPolicy, OpaqueReference, SshConnection, TcpMutualTlsConnection,
    UnixConnection,
};
pub use coverage::{NativeFieldCoverageClassification, NativeFieldCoverageEntry, native_field_coverage_catalogue};
pub use deployment::{
    ContainerIntent, DeploymentConnectionReference, DeploymentIntent, DeploymentOperation, DeploymentOperationId,
    DeploymentPlan, DeploymentResource, DeploymentResourceId, ExternalPrecondition, ImageIntent, ImagePullPolicy,
    NetworkIntent, PlanningFinding, PlanningOutcome, PodIntent, SecretIntent, SemanticOperationAction,
    SensitiveInputReference, StartupDependency, VolumeIntent, plan_deployment,
};
pub use diagnostic::{Diagnostic, DiagnosticCode, PodmanLensResult};
pub use discovery::{
    DependencyEvidence, DiscoveryExplanation, DiscoveryExplanationKind, DiscoveryFinding, DiscoveryRequest,
    DiscoveryRootOrigin, GroupingEdge, GroupingEvidence, LabelSelector, ResourceDependency, ResourceGraph,
    ResourceGroup, ResourceSelector, discover,
};
pub use evidence::{CapabilityCatalogueEntry, EvidenceReference, capability_catalogue};
pub use inventory::{
    AcquisitionOptions, EnvironmentEntry, EnvironmentValue, EnvironmentValuePolicy, InventoryFinding, InventorySection,
    JsonValueKind, MAX_INVENTORY_JSON_BYTES, MAX_UNKNOWN_FIELDS_PER_INVENTORY, MAX_UNKNOWN_FIELDS_PER_RECORD,
    NetworkDetails, ObservationState, ResourceEvidence, ResourceIdentity, ResourceInventory, ResourceKind,
    ResourceRecord, ResourceRelationship, SensitiveEnvironmentValue, UnknownNativeField, acquire_inventory,
};
pub use networking::{
    DnsConfiguration, HostAlias, NetworkAttachment, NetworkCidr, NetworkRoute, NetworkSubnet, PortMapping,
    PortProtocol, RouteType, StaticMacAddress,
};
pub use probe::{MAX_PROBE_JSON_BYTES, ServiceObservation, probe_libpod_service};
#[cfg(unix)]
pub use read_only_unix_transport::{MIN_HTTP1_HEADER_BYTES, ReadOnlyUnixTransport, ReadOnlyUnixTransportTimeouts};
pub use render::{
    CliInvocation, DeploymentRendering, LibpodInvocation, RenderStatus, RenderedHttpBody, RenderedHttpMethod,
    RenderedOperation, RenderingFinding, RenderingOutcome, render_deployment,
};
pub use settings::{
    AbsoluteContainerPath, ArgumentArray, ContainerHostname, ContainerSettings, ContainerUser, ContainerWorkdir,
    DeploymentEnvironmentValue, EnvironmentAssignment, EnvironmentName, Label, LabelKey, NamedVolumeCopyMode,
    NamedVolumeMount, PublicEnvironmentValue, PublicLabelValue, RestartPolicy, SensitiveInlineEnvironmentValue,
};
pub use transport::{
    LibpodHeader, LibpodHeaders, LibpodMethod, LibpodPath, LibpodRequest, LibpodResponse, LibpodTransport,
    LibpodTransportFuture, MAX_PATH_AND_QUERY_BYTES, TransportError, TransportLimits,
};
pub use version::{
    ObservedApiVersion, ObservedPodmanVersion, SupportedPodmanRange, TargetExecutionContext, TargetProfile,
};
