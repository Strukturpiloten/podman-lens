//! Observed and target Podman versions backed by reviewed catalogue evidence.

use std::fmt;

use semver::Version;

use crate::{
    Diagnostic, DiagnosticCode, PodmanLensResult,
    evidence::{CapabilityCatalogueEntry, capability_catalogue},
};

/// A validated Podman version reported by a Libpod service.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct ObservedPodmanVersion {
    original: String,
    normalized: Version,
}

impl ObservedPodmanVersion {
    /// Parses a complete semantic Podman version.
    ///
    /// # Errors
    ///
    /// Returns `PLN0004` when the version is malformed or a prerelease.
    pub fn parse(value: &str) -> PodmanLensResult<Self> {
        let normalized = Version::parse(value).map_err(|_| Diagnostic::new(DiagnosticCode::InvalidVersion))?;
        if !normalized.pre.is_empty() {
            return Err(Diagnostic::new(DiagnosticCode::InvalidVersion));
        }
        Ok(Self {
            original: value.to_owned(),
            normalized,
        })
    }

    /// Returns the semantic version.
    #[must_use]
    pub const fn as_semver(&self) -> &Version {
        &self.normalized
    }

    /// Returns the exact spelling reported by the service.
    #[must_use]
    pub fn original(&self) -> &str {
        &self.original
    }
}

impl fmt::Display for ObservedPodmanVersion {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.original.fmt(formatter)
    }
}

/// A validated Libpod API version reported or selected for a service.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct ObservedApiVersion {
    original: String,
    normalized: Version,
}

impl ObservedApiVersion {
    /// Parses a complete semantic Libpod API version.
    ///
    /// # Errors
    ///
    /// Returns `PLN0004` when the version is malformed or a prerelease.
    pub fn parse(value: &str) -> PodmanLensResult<Self> {
        let normalized = Version::parse(value).map_err(|_| Diagnostic::new(DiagnosticCode::InvalidVersion))?;
        if !normalized.pre.is_empty() {
            return Err(Diagnostic::new(DiagnosticCode::InvalidVersion));
        }
        Ok(Self {
            original: value.to_owned(),
            normalized,
        })
    }

    /// Returns the semantic API version.
    #[must_use]
    pub const fn as_semver(&self) -> &Version {
        &self.normalized
    }

    /// Returns the exact spelling reported or selected by the caller.
    #[must_use]
    pub fn original(&self) -> &str {
        &self.original
    }
}

impl fmt::Display for ObservedApiVersion {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.original.fmt(formatter)
    }
}

/// The reviewed, fail-closed Podman target range.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct SupportedPodmanRange;

impl SupportedPodmanRange {
    /// The minimum reviewed Podman version.
    pub const MINIMUM: &'static str = "5.4.0";
    /// The exclusive upper bound of the reviewed Podman range.
    pub const MAXIMUM_EXCLUSIVE: &'static str = "6.2.0";

    /// Returns whether a version is inside the reviewed target range.
    #[must_use]
    pub fn contains(self, version: &ObservedPodmanVersion) -> bool {
        let version = version.as_semver();
        version >= &Version::new(5, 4, 0) && version < &Version::new(6, 2, 0)
    }
}

/// An explicit, evidence-backed target used to create Libpod operations.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TargetProfile {
    podman_version: ObservedPodmanVersion,
    api_version: ObservedApiVersion,
    execution_context: TargetExecutionContext,
}

/// Explicit privilege context of the selected deployment target.
///
/// Podman accepts some native settings, including static network addresses and MAC addresses,
/// only for rootful targets. The context is caller-supplied evidence; `PodmanLens` never probes the
/// development machine to infer it.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
#[non_exhaustive]
pub enum TargetExecutionContext {
    /// The target privilege context has not been explicitly established.
    #[default]
    Unknown,
    /// The target is explicitly rootless.
    Rootless,
    /// The target is explicitly rootful.
    Rootful,
}

impl TargetProfile {
    /// Validates an explicit Podman and Libpod API target pair against the embedded catalogue.
    ///
    /// # Errors
    ///
    /// Returns `PLN0005` when the engine is outside reviewed evidence or the selected Libpod API
    /// is below the reviewed minimum or newer than that engine.
    pub fn new(podman_version: ObservedPodmanVersion, api_version: ObservedApiVersion) -> PodmanLensResult<Self> {
        if !SupportedPodmanRange.contains(&podman_version) {
            return Err(Diagnostic::new(DiagnosticCode::IncompatibleTargetProfile));
        }
        let entry = matching_catalogue_entry(&podman_version)?;
        if !api_version_is_supported(&entry, &podman_version, &api_version) {
            return Err(Diagnostic::new(DiagnosticCode::IncompatibleTargetProfile));
        }
        Ok(Self {
            podman_version,
            api_version,
            execution_context: TargetExecutionContext::Unknown,
        })
    }

    /// Records the caller-proven privilege context of the deployment target.
    pub fn set_execution_context(&mut self, context: TargetExecutionContext) {
        self.execution_context = context;
    }

    /// Returns the explicit Podman target version.
    #[must_use]
    pub fn podman_version(&self) -> &ObservedPodmanVersion {
        &self.podman_version
    }

    /// Returns the explicit Libpod API target version.
    #[must_use]
    pub fn api_version(&self) -> &ObservedApiVersion {
        &self.api_version
    }

    /// Returns the caller-proven privilege context of the deployment target.
    #[must_use]
    pub const fn execution_context(&self) -> TargetExecutionContext {
        self.execution_context
    }
}

fn matching_catalogue_entry(version: &ObservedPodmanVersion) -> PodmanLensResult<CapabilityCatalogueEntry> {
    capability_catalogue()?
        .into_iter()
        .find(|entry| version_in_entry(version.as_semver(), entry))
        .ok_or_else(|| Diagnostic::new(DiagnosticCode::IncompatibleTargetProfile))
}

fn version_in_entry(version: &Version, entry: &CapabilityCatalogueEntry) -> bool {
    let Ok(minimum) = Version::parse(entry.minimum_podman_version()) else {
        return false;
    };
    let Ok(maximum_exclusive) = Version::parse(entry.maximum_exclusive_podman_version()) else {
        return false;
    };
    version >= &minimum && version < &maximum_exclusive
}

fn api_version_is_supported(
    entry: &CapabilityCatalogueEntry,
    podman_version: &ObservedPodmanVersion,
    api_version: &ObservedApiVersion,
) -> bool {
    let Ok(minimum_api) = Version::parse(entry.minimum_libpod_api_version()) else {
        return false;
    };
    semantic_core_inclusive(api_version.as_semver(), &minimum_api, podman_version.as_semver())
}

fn semantic_core_inclusive(version: &Version, minimum: &Version, maximum: &Version) -> bool {
    version >= minimum && version <= maximum
}
