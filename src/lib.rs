//! Version-aware native Podman inspection and non-executing deployment planning.
//!
//! `PodmanLens` acquires one explicitly selected Libpod service through a replaceable transport,
//! preserves typed native observation state and provenance, discovers an evidence-backed resource
//! graph, plans caller-authored target intent, and renders deterministic CLI and Libpod
//! descriptions. It never discovers an ambient endpoint, shells out to `podman` for input, sends a
//! mutating acquisition request, or executes a rendered plan.
//!
//! # Explicit read-only acquisition
//!
//! The built-in Unix transport accepts one caller-supplied socket and rejects every method except
//! `GET` before opening it. Environment values are redacted by default and secret payload endpoints
//! are never requested.
//!
//! ```no_run
//! # #[cfg(unix)]
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! use podman_lens::{
//!     AcquisitionOptions, DiscoveryRequest, ReadOnlyUnixTransport,
//!     ReadOnlyUnixTransportTimeouts, TransportLimits, UnixConnection, acquire_inventory,
//!     discover,
//! };
//!
//! let transport = ReadOnlyUnixTransport::new(
//!     UnixConnection::new("/run/user/1000/podman/podman.sock")?,
//!     TransportLimits::default(),
//!     ReadOnlyUnixTransportTimeouts::default(),
//! )?;
//! let runtime = tokio::runtime::Builder::new_current_thread().enable_all().build()?;
//! let graph = runtime.block_on(async {
//!     let inventory = acquire_inventory(&transport, AcquisitionOptions::redacted()).await?;
//!     let mut request = DiscoveryRequest::new();
//!     request.select_all();
//!     discover(&inventory, &request)
//! })?;
//! assert!(graph.requested_roots().is_empty());
//! # Ok(())
//! # }
//! ```
//!
//! `select_all` is retained separately on the graph; exact and label selectors are available when
//! the caller needs a narrower application boundary. See the task-oriented public guides for
//! selector, grouping, network-crossing, and privacy contracts.
//!
//! # Deterministic offline planning and rendering
//!
//! Planning uses explicit target-side intent and opens no connection. Rendering produces data and
//! review text only.
//!
//! ```
//! use podman_lens::{
//!     DeploymentIntent, DeploymentResource, DeploymentResourceId, ImageIntent, ImagePullPolicy,
//!     ImageSource, ObservedApiVersion, ObservedPodmanVersion, ResourceKind, TargetProfile,
//!     artifact::deployment_v1, plan_deployment, render_deployment,
//! };
//!
//! let target = TargetProfile::new(
//!     ObservedPodmanVersion::parse("6.1.0")?,
//!     ObservedApiVersion::parse("6.1.0")?,
//! )?;
//! let image = DeploymentResourceId::new(ResourceKind::Image, "application-image")?;
//! let mut intent = DeploymentIntent::new(target);
//! intent.add_resource(DeploymentResource::Image(ImageIntent::new(
//!     image,
//!     ImageSource::new("registry.example.invalid/team/application:1")?,
//!     ImagePullPolicy::Missing,
//! )?));
//! let planned = plan_deployment(&intent);
//! let plan = planned.plan().expect("reviewed intent produces a complete plan");
//! let rendered = render_deployment(plan);
//! let rendering = rendered.rendering().expect("reviewed target renders exactly");
//! assert_eq!(plan.operations().len(), 1);
//! assert_eq!(rendering.operations()[0].cli().program(), "podman");
//! assert_eq!(deployment_v1::deployment(rendering).schema_version(), 1);
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```

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
pub mod observation;
pub mod probe;
#[cfg(unix)]
pub mod read_only_unix_transport;
pub mod render;
pub mod runtime;
pub mod settings;
pub mod snapshot;
pub mod transport;
pub mod version;

pub use connection::{
    ConnectionKind, ConnectionSpec, MutualTlsPolicy, OpaqueReference, SshConnection, TcpMutualTlsConnection,
    UnixConnection,
};
pub use coverage::{
    NativeFieldCoverageClassification, NativeFieldCoverageEntry, NativeFieldCoveragePlane,
    native_field_coverage_catalogue,
};
pub use deployment::{
    ContainerIntent, DeploymentConnectionReference, DeploymentIntent, DeploymentOperation, DeploymentOperationId,
    DeploymentPlan, DeploymentResource, DeploymentResourceId, ExternalPrecondition, ImageIntent, ImagePullPolicy,
    ImageSource, ImageSourceClassification, NetworkIntent, PlanningFinding, PlanningOutcome, PodIntent, SecretIntent,
    SemanticOperationAction, SensitiveInputReference, StartupDependency, VolumeIntent, plan_deployment,
};
pub use diagnostic::{Diagnostic, DiagnosticCode, PodmanLensResult};
pub use discovery::{
    DependencyEvidence, DiscoveryExplanation, DiscoveryExplanationKind, DiscoveryFinding, DiscoveryRequest,
    DiscoveryRootOrigin, GroupingEdge, GroupingEvidence, LabelSelector, ResourceDependency, ResourceGraph,
    ResourceGroup, ResourceSelector, ResourceSelectorMatch, discover,
};
pub use evidence::{CapabilityCatalogueEntry, EvidenceReference, capability_catalogue};
pub use inventory::{
    AcquisitionOptions, EnvironmentValuePolicy, InventoryFinding, InventorySection, InventorySectionAvailability,
    JsonValueKind, MAX_INVENTORY_JSON_BYTES, MAX_UNKNOWN_FIELDS_PER_INVENTORY, MAX_UNKNOWN_FIELDS_PER_RECORD,
    ResourceEvidence, ResourceIdentity, ResourceInventory, ResourceKind, SensitiveEnvironmentValue, acquire_inventory,
};
pub use networking::{
    DnsConfiguration, HostAlias, NetworkAttachment, NetworkCidr, NetworkRoute, NetworkSubnet, PortMapping,
    PortProtocol, RouteType, StaticMacAddress,
};
pub use observation::{
    ConfiguredContainerCommand, ConfiguredContainerEntrypoint, ConfiguredContainerHostname, ConfiguredContainerUser,
    ConfiguredContainerWorkdir, ContainerMountKind, ContainerMountObservation, ContainerMountSelinuxRelabel,
    ContainerMountSource, ContainerObservation, ContainerSecretGrantObservation, ContainerSecretReference,
    ImageObservation, Labels, NativeCapability, NativeHealthCheckObservation, NativeHealthCommand,
    NativeHealthFailureAction, NativeIpcNamespaceMode, NativeLogDriver, NativeLoggingObservation, NativeNamespaceMode,
    NativeNamespaceObservation, NativeNetworkCidr, NativeNetworkLeaseRange, NativeNetworkRouteObservation,
    NativeNetworkRouteType, NativeNetworkSubnetObservation, NativeNetworkingObservation, NativeOpaqueNetworkOptions,
    NativeOpaqueSecurityOptions, NativePortBindingObservation, NativePortProtocol, NativeResourceControlObservation,
    NativeResourceReference, NativeRestartPolicyName, NativeRestartPolicyObservation, NativeSecretDriverObservation,
    NativeSecretDriverOptions, NativeSecurityObservation, NativeStartupHealthCheckObservation, NativeTimestamp,
    NativeUlimitObservation, NetworkObservation, NetworkOptionKeys, ObservationField, ObservationHeader,
    ObservationOrigin, ObservedValue, PodObservation, ProtectedEnvironment, ProtectedEnvironmentEntry,
    ProtectedEnvironmentValue, ProtectedHealthCommand, ResourceDetails, ResourceObservation, ResourceObservationState,
    SecretObservation, UnixId as ObservedUnixId, UnmodelledCompleteness, UnmodelledField, UnmodelledFieldId,
    VolumeObservation, VolumeOwnerIdWireValue,
};
pub use probe::{MAX_PROBE_JSON_BYTES, ServiceObservation, probe_libpod_service};
#[cfg(unix)]
pub use read_only_unix_transport::{MIN_HTTP1_HEADER_BYTES, ReadOnlyUnixTransport, ReadOnlyUnixTransportTimeouts};
pub use render::{
    CliInvocation, DeploymentRendering, LibpodInvocation, RenderStatus, RenderedHttpBody, RenderedHttpMethod,
    RenderedOperation, RenderingFinding, RenderingOutcome, render_deployment,
};
pub use runtime::{
    ConfiguredHealthCheck, ContainerNamespaceSettings, ContainerResourceControls, ContainerRuntimeSettings,
    HealthCheck, HealthCommand, HealthDuration, HealthInterval, HealthOnFailure, HealthRetries, HealthStartPeriod,
    HealthTimeout, IpcNamespaceMode, LinuxCapability, LogDriver, LogSize, LoggingSettings, NamespaceMode,
    PublicHealthArgumentArray, PublicHealthCommand, Rlimit, RlimitKind, RlimitValue, SecuritySettings,
    SensitiveInlineHealthArgumentArray, SensitiveInlineHealthCommand, StartupHealthCheck, StartupHealthRetries,
    StartupHealthSuccesses,
};
pub use settings::{
    AbsoluteContainerPath, ArgumentArray, BindMount, ContainerHostname, ContainerSettings, ContainerUser,
    ContainerWorkdir, DeploymentEnvironmentValue, EnvironmentAssignment, EnvironmentName, Label, LabelKey, MountAccess,
    MountIntent, NamedVolumeCopyMode, NamedVolumeMount, PublicEnvironmentValue, PublicLabelValue, RestartPolicy,
    SecretGrant, SecretMode, SensitiveInlineEnvironmentValue, TmpfsMount, UnixId, VolumeSubpath,
};
pub use transport::{
    LibpodHeader, LibpodHeaders, LibpodMethod, LibpodPath, LibpodRequest, LibpodResponse, LibpodTransport,
    LibpodTransportFuture, MAX_PATH_AND_QUERY_BYTES, TransportError, TransportLimits,
};
pub use version::{
    CgroupCapabilityEvidence, CgroupController, CgroupVersion, ObservedApiVersion, ObservedPodmanVersion,
    SupportedPodmanRange, TargetExecutionContext, TargetProfile,
};
