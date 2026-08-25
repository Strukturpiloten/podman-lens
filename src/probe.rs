//! Read-only Libpod service probing and fixed version observation.

use crate::version::observed_input_is_supported;
use crate::{
    CapabilityCatalogueEntry, Diagnostic, DiagnosticCode, LibpodHeaders, LibpodMethod, LibpodPath, LibpodRequest,
    LibpodTransport, ObservedApiVersion, ObservedPodmanVersion, PodmanLensResult, TargetProfile,
};

/// Maximum accepted byte length of the small Libpod version JSON response.
pub const MAX_PROBE_JSON_BYTES: usize = 65_536;

/// Independently observed Podman engine and Libpod API versions.
///
/// The values are intentionally retained separately: a compatible API version need not have the
/// same spelling as the engine version. [`Self::input_capability`] records reviewed input
/// compatibility after the probe validates both observations.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ServiceObservation {
    engine_version: ObservedPodmanVersion,
    api_version: ObservedApiVersion,
    input_capability: CapabilityCatalogueEntry,
    output_target_profile: Option<TargetProfile>,
}

impl ServiceObservation {
    /// Returns the reported Podman Engine version.
    #[must_use]
    pub fn engine_version(&self) -> &ObservedPodmanVersion {
        &self.engine_version
    }

    /// Returns the API version from the `Libpod-API-Version` ping header.
    #[must_use]
    pub fn api_version(&self) -> &ObservedApiVersion {
        &self.api_version
    }

    /// Returns immutable evidence for the reviewed input runtime.
    #[must_use]
    pub fn input_capability(&self) -> &CapabilityCatalogueEntry {
        &self.input_capability
    }

    /// Returns a reviewed output profile only when the observed runtime is also a supported
    /// deployment target.
    ///
    /// Older input-only anchors intentionally return `None`: they can be acquired for migration,
    /// but callers must choose a newer explicit [`TargetProfile`] before rendering.
    #[must_use]
    pub fn output_target_profile(&self) -> Option<&TargetProfile> {
        self.output_target_profile.as_ref()
    }
}

/// Performs the fixed, read-only Libpod version probe.
///
/// The probe sends exactly these requests, in this order, and never retries:
///
/// 1. `GET /libpod/_ping`
/// 2. `GET /v4.0.0/libpod/version` for API 4+ or a versioned Podman 3 Libpod endpoint matching
///    the service's advertised API version.
///
/// # Errors
///
/// Returns a redacted diagnostic when either response is malformed, outside reviewed
/// compatibility evidence, or cannot be acquired by the caller-provided transport.
pub async fn probe_libpod_service(transport: &dyn LibpodTransport) -> PodmanLensResult<ServiceObservation> {
    let ping = request(LibpodPath::parse("/libpod/_ping")?)?;
    let ping_response = transport
        .send(&ping)
        .await
        .map_err(|error| error.diagnostic().clone())?;
    require_ok_status(ping_response.status())?;
    let api_version = ping_api_version(ping_response.headers())?;

    let version = request(version_probe_path(&api_version)?)?;
    let version_response = transport
        .send(&version)
        .await
        .map_err(|error| error.diagnostic().clone())?;
    require_ok_status(version_response.status())?;
    require_json_content_type(version_response.headers())?;
    let engine_version = engine_version(version_response.body())?;
    let input_capability = observed_input_is_supported(&engine_version, &api_version)?;
    let output_target_profile = TargetProfile::new(engine_version.clone(), api_version.clone()).ok();

    Ok(ServiceObservation {
        engine_version,
        api_version,
        input_capability,
        output_target_profile,
    })
}

fn version_probe_path(api_version: &ObservedApiVersion) -> PodmanLensResult<LibpodPath> {
    let version = if api_version.as_semver().major == 3 {
        api_version.original()
    } else {
        "4.0.0"
    };
    LibpodPath::parse(format!("/v{version}/libpod/version"))
}

fn request(path: LibpodPath) -> PodmanLensResult<LibpodRequest> {
    LibpodRequest::new(LibpodMethod::Get, path, Vec::new())
}

fn require_ok_status(status: u16) -> PodmanLensResult<()> {
    if status == 200 {
        Ok(())
    } else {
        Err(Diagnostic::new(DiagnosticCode::ProbeHttpStatus))
    }
}

fn ping_api_version(headers: &LibpodHeaders) -> PodmanLensResult<ObservedApiVersion> {
    let values = headers.values("libpod-api-version").collect::<Vec<_>>();
    let [value] = values.as_slice() else {
        return Err(Diagnostic::new(DiagnosticCode::ProbeHeader));
    };
    ObservedApiVersion::parse_reported(value).map_err(|_| Diagnostic::new(DiagnosticCode::ProbeHeader))
}

fn require_json_content_type(headers: &LibpodHeaders) -> PodmanLensResult<()> {
    let values = headers.values("content-type").collect::<Vec<_>>();
    let [value] = values.as_slice() else {
        return Err(Diagnostic::new(DiagnosticCode::ProbeHeader));
    };
    let Some(media_type) = value.split(';').next() else {
        return Err(Diagnostic::new(DiagnosticCode::ProbeHeader));
    };
    if media_type.trim().eq_ignore_ascii_case("application/json") {
        Ok(())
    } else {
        Err(Diagnostic::new(DiagnosticCode::ProbeHeader))
    }
}

fn engine_version(body: &[u8]) -> PodmanLensResult<ObservedPodmanVersion> {
    if body.len() > MAX_PROBE_JSON_BYTES {
        return Err(Diagnostic::new(DiagnosticCode::ProbeJson));
    }
    let value: serde_json::Value =
        serde_json::from_slice(body).map_err(|_| Diagnostic::new(DiagnosticCode::ProbeJson))?;
    let Some(root) = value.as_object() else {
        return Err(Diagnostic::new(DiagnosticCode::ProbeShape));
    };
    let Some(components) = root.get("Components") else {
        return Err(Diagnostic::new(DiagnosticCode::ProbeShape));
    };
    let Some(components) = components.as_array() else {
        return Err(Diagnostic::new(DiagnosticCode::ProbeShape));
    };

    let mut engine = None;
    for component in components {
        let Some(component) = component.as_object() else {
            continue;
        };
        if component.get("Name").and_then(serde_json::Value::as_str) != Some("Podman Engine") {
            continue;
        }
        let Some(version) = component.get("Version").and_then(serde_json::Value::as_str) else {
            return Err(Diagnostic::new(DiagnosticCode::ProbeComponent));
        };
        if engine.replace(version).is_some() {
            return Err(Diagnostic::new(DiagnosticCode::ProbeComponent));
        }
    }
    let Some(engine) = engine else {
        return Err(Diagnostic::new(DiagnosticCode::ProbeComponent));
    };
    ObservedPodmanVersion::parse_reported(engine).map_err(|_| Diagnostic::new(DiagnosticCode::ProbeComponent))
}

#[cfg(test)]
mod tests {
    use super::{MAX_PROBE_JSON_BYTES, engine_version, require_json_content_type, version_probe_path};
    use crate::{DiagnosticCode, LibpodHeader, LibpodHeaders, ObservedApiVersion};

    #[test]
    fn probe_json_is_bounded_before_decode() -> Result<(), Box<dyn std::error::Error>> {
        let body = vec![b' '; MAX_PROBE_JSON_BYTES + 1];
        let error = engine_version(&body)
            .err()
            .ok_or_else(|| std::io::Error::other("oversized probe JSON was unexpectedly accepted"))?;
        assert_eq!(error.code(), DiagnosticCode::ProbeJson);
        Ok(())
    }

    #[test]
    fn content_type_is_case_insensitive_and_allows_parameters() -> Result<(), Box<dyn std::error::Error>> {
        let headers = LibpodHeaders::new(vec![LibpodHeader::new(
            "Content-Type",
            "Application/Json; charset=utf-8",
        )?]);
        assert!(require_json_content_type(&headers).is_ok());
        Ok(())
    }

    #[test]
    fn legacy_probe_uses_the_advertised_podman_three_api_path() -> Result<(), Box<dyn std::error::Error>> {
        assert_eq!(
            version_probe_path(&ObservedApiVersion::parse("3.0.0")?)?.as_str(),
            "/v3.0.0/libpod/version"
        );
        assert_eq!(
            version_probe_path(&ObservedApiVersion::parse("3.4.4")?)?.as_str(),
            "/v3.4.4/libpod/version"
        );
        assert_eq!(
            version_probe_path(&ObservedApiVersion::parse("4.9.4")?)?.as_str(),
            "/v4.0.0/libpod/version"
        );
        Ok(())
    }
}
