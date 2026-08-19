//! Immutable provenance for reviewed Podman compatibility lines.

use semver::Version;
use serde::Deserialize;

use crate::{Diagnostic, DiagnosticCode, PodmanLensResult};

const CATALOGUE_JSON: &str = include_str!("../catalogue/v1/podman-capabilities.json");

/// Immutable upstream evidence retained for one reviewed Podman line.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
pub struct EvidenceReference {
    source: String,
    revision: String,
    release_tag: String,
}

impl EvidenceReference {
    /// Returns the immutable upstream source URL.
    #[must_use]
    pub fn source(&self) -> &str {
        &self.source
    }

    /// Returns the exact upstream Git revision.
    #[must_use]
    pub fn revision(&self) -> &str {
        &self.revision
    }

    /// Returns the reviewed upstream release tag.
    #[must_use]
    pub fn release_tag(&self) -> &str {
        &self.release_tag
    }
}

/// One reviewed Podman minor line and its Libpod API target version.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
pub struct CapabilityCatalogueEntry {
    podman_minor_line: String,
    minimum_podman_version: String,
    maximum_exclusive_podman_version: String,
    minimum_libpod_api_version: String,
    observed_podman_version: String,
    observed_libpod_api_version: String,
    evidence: EvidenceReference,
}

impl CapabilityCatalogueEntry {
    /// Returns the reviewed Podman `major.minor` line.
    #[must_use]
    pub fn podman_minor_line(&self) -> &str {
        &self.podman_minor_line
    }

    /// Returns the first supported Podman patch for the line.
    #[must_use]
    pub fn minimum_podman_version(&self) -> &str {
        &self.minimum_podman_version
    }

    /// Returns the exclusive upper Podman bound for the line.
    #[must_use]
    pub fn maximum_exclusive_podman_version(&self) -> &str {
        &self.maximum_exclusive_podman_version
    }

    /// Returns the reviewed minimum Libpod API version for the line.
    #[must_use]
    pub fn minimum_libpod_api_version(&self) -> &str {
        &self.minimum_libpod_api_version
    }

    /// Returns the Podman engine version observed in the immutable evidence release.
    #[must_use]
    pub fn observed_podman_version(&self) -> &str {
        &self.observed_podman_version
    }

    /// Returns the Libpod API version observed in the immutable evidence release.
    #[must_use]
    pub fn observed_libpod_api_version(&self) -> &str {
        &self.observed_libpod_api_version
    }

    /// Returns the immutable source evidence.
    #[must_use]
    pub fn evidence(&self) -> &EvidenceReference {
        &self.evidence
    }
}

#[derive(Deserialize)]
struct Catalogue {
    schema_version: u8,
    reviewed_lines: Vec<CapabilityCatalogueEntry>,
}

/// Decodes the embedded, offline compatibility catalogue.
///
/// The JSON file is part of the published crate and is deliberately parsed on demand so future
/// catalogue schema upgrades remain explicit rather than relying on hidden build-time code.
///
/// # Errors
///
/// Returns `PLN0006` when the published catalogue fails its schema validation.
pub fn capability_catalogue() -> PodmanLensResult<Vec<CapabilityCatalogueEntry>> {
    parse_catalogue(CATALOGUE_JSON)
}

fn parse_catalogue(source: &str) -> PodmanLensResult<Vec<CapabilityCatalogueEntry>> {
    let catalogue: Catalogue =
        serde_json::from_str(source).map_err(|_| Diagnostic::new(DiagnosticCode::CatalogueUnavailable))?;
    if catalogue.schema_version != 1 || !valid_catalogue(&catalogue.reviewed_lines) {
        return Err(Diagnostic::new(DiagnosticCode::CatalogueUnavailable));
    }
    Ok(catalogue.reviewed_lines)
}

fn valid_catalogue(entries: &[CapabilityCatalogueEntry]) -> bool {
    let mut expected_minimum = Version::new(5, 4, 0);
    for entry in entries {
        let Some((minimum, maximum_exclusive, minimum_api, observed_engine, observed_api)) = parsed_versions(entry)
        else {
            return false;
        };
        let expected_label = format!("{}.{}", minimum.major, minimum.minor);
        let expected_source = format!(
            "https://github.com/containers/podman/blob/{}/version/rawversion/rawversion.go",
            entry.evidence.revision
        );
        let expected_tag = format!("v{observed_engine}");
        if minimum != expected_minimum
            || maximum_exclusive <= minimum
            || entry.podman_minor_line != expected_label
            || minimum_api != Version::new(4, 0, 0)
            || observed_engine < minimum
            || observed_engine >= maximum_exclusive
            || observed_api < minimum_api
            || observed_api > observed_engine
            || observed_engine != observed_api
            || entry.evidence.release_tag != expected_tag
            || entry.evidence.source != expected_source
            || !is_lowercase_sha40(&entry.evidence.revision)
        {
            return false;
        }
        expected_minimum = maximum_exclusive;
    }
    expected_minimum == Version::new(6, 2, 0)
}

fn parsed_versions(entry: &CapabilityCatalogueEntry) -> Option<(Version, Version, Version, Version, Version)> {
    let minimum = Version::parse(&entry.minimum_podman_version).ok()?;
    let maximum_exclusive = Version::parse(&entry.maximum_exclusive_podman_version).ok()?;
    let minimum_api = Version::parse(&entry.minimum_libpod_api_version).ok()?;
    let observed_engine = Version::parse(&entry.observed_podman_version).ok()?;
    let observed_api = Version::parse(&entry.observed_libpod_api_version).ok()?;
    [
        &minimum,
        &maximum_exclusive,
        &minimum_api,
        &observed_engine,
        &observed_api,
    ]
    .iter()
    .all(|version| version.pre.is_empty() && version.build.is_empty())
    .then_some((minimum, maximum_exclusive, minimum_api, observed_engine, observed_api))
}

fn is_lowercase_sha40(value: &str) -> bool {
    value.len() == 40
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
}

#[cfg(test)]
mod tests {
    use super::{CATALOGUE_JSON, Catalogue, parse_catalogue, valid_catalogue};

    fn decoded_catalogue() -> Result<Catalogue, Box<dyn std::error::Error>> {
        Ok(serde_json::from_str(CATALOGUE_JSON)?)
    }

    #[test]
    fn embedded_catalogue_obeys_the_complete_policy() -> Result<(), Box<dyn std::error::Error>> {
        let catalogue = decoded_catalogue()?;
        assert!(valid_catalogue(&catalogue.reviewed_lines));
        Ok(())
    }

    #[test]
    fn malformed_catalogue_policy_is_rejected() {
        let cases = [
            (
                "\"maximum_exclusive_podman_version\": \"5.5.0\"",
                "\"maximum_exclusive_podman_version\": \"5.4.9\"",
            ),
            (
                "\"minimum_podman_version\": \"5.5.0\"",
                "\"minimum_podman_version\": \"5.4.0\"",
            ),
            (
                "f9f7d48b24b1ca4403f189caaeab1cb8ff4a9aa2",
                "F9f7d48b24b1ca4403f189caaeab1cb8ff4a9aa2",
            ),
            ("\"release_tag\": \"v5.4.0\"", "\"release_tag\": \"v5.4.1\""),
            (
                "\"observed_libpod_api_version\": \"5.4.0\"",
                "\"observed_libpod_api_version\": \"3.9.9\"",
            ),
        ];
        for (from, to) in cases {
            assert!(parse_catalogue(&CATALOGUE_JSON.replacen(from, to, 1)).is_err());
        }
    }

    #[test]
    fn reordered_catalogue_policy_is_rejected() -> Result<(), Box<dyn std::error::Error>> {
        let mut catalogue = decoded_catalogue()?;
        catalogue.reviewed_lines.swap(0, 1);
        assert!(!valid_catalogue(&catalogue.reviewed_lines));
        Ok(())
    }

    #[test]
    fn individual_evidence_policy_violations_are_rejected() -> Result<(), Box<dyn std::error::Error>> {
        let mut catalogue = decoded_catalogue()?;
        catalogue.reviewed_lines[0].evidence.revision.replace_range(0..1, "F");
        assert!(!valid_catalogue(&catalogue.reviewed_lines));

        let mut catalogue = decoded_catalogue()?;
        catalogue.reviewed_lines[0].evidence.source = "https://example.invalid/mutable".to_owned();
        assert!(!valid_catalogue(&catalogue.reviewed_lines));

        let mut catalogue = decoded_catalogue()?;
        catalogue.reviewed_lines[0].observed_podman_version = "5.4.1".to_owned();
        assert!(!valid_catalogue(&catalogue.reviewed_lines));
        Ok(())
    }
}
