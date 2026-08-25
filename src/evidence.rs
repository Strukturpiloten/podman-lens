//! Immutable provenance for reviewed Podman compatibility lines.

use semver::Version;
use serde::Deserialize;

use crate::{Diagnostic, DiagnosticCode, PodmanLensResult};

const CATALOGUE_JSON: &str = include_str!("../catalogue/v1/podman-capabilities.json");
const REVIEWED_RHEL_ALIAS: (&str, &str, &str, &str, &str, &str, &str) = (
    "4.9.4-rhel",
    "4.9.4-rhel",
    "ghcr.io/strukturpiloten/podman-ubi-8-rootful:v1.0.0@sha256:51dc92fd2165112131a3f070021f7fe382f3fcd541a4f39f7e01fdb8326483fc",
    "4.9.4-34.module+el8.10.0+24510+6ea3880e.x86_64",
    "https://github.com/Strukturpiloten/containers/tree/f0259a080e8a49be43358fc00c4cac89528d4954/images/podman/podman-ubi-8-rootful",
    "f0259a080e8a49be43358fc00c4cac89528d4954",
    "v1.0.0",
);
const LEGACY_INPUT_ANCHORS: [(&str, &str, &str, &str, &str, &str); 5] = [
    (
        "3.0.1",
        "3.0.2",
        "3.0.0",
        "3.0.0",
        "c640670e85c4aaaff92741691d6a854a90229d8d",
        "version/version.go",
    ),
    (
        "3.4.4",
        "3.4.5",
        "3.1.0",
        "3.4.4",
        "f6526ada1025c2e3f88745ba83b8b461ca659933",
        "version/version.go",
    ),
    (
        "4.3.1",
        "4.3.2",
        "4.0.0",
        "4.3.1",
        "814b7b003cc630bf6ab188274706c383f9fb9915",
        "version/version.go",
    ),
    (
        "4.9.3",
        "4.9.4",
        "4.0.0",
        "4.9.3",
        "8d2b55ddde1bc81f43d018dfc1ac027c06b26a7f",
        "version/rawversion/rawversion.go",
    ),
    (
        "4.9.4",
        "4.9.5",
        "4.0.0",
        "4.9.4",
        "3aceae8ace3c7e3c5591900db32d188cf60be535",
        "version/rawversion/rawversion.go",
    ),
];

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
    #[serde(default = "default_output_supported")]
    output_supported: bool,
    #[serde(default)]
    reported_version_aliases: Vec<ReportedVersionAlias>,
    evidence: EvidenceReference,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
struct ReportedVersionAlias {
    podman_version: String,
    libpod_api_version: String,
    image: String,
    package_revision: String,
    root_mode: String,
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

    /// Returns whether this line has reviewed deployment-output evidence.
    ///
    /// Input-only anchors may be acquired and discovered so an old host can be migrated to a
    /// newer explicit target, but they can never be selected as a rendering target.
    #[must_use]
    pub const fn output_supported(&self) -> bool {
        self.output_supported
    }

    pub(crate) fn matches_reported_podman_version(&self, value: &str) -> bool {
        value == self.observed_podman_version
            || self
                .reported_version_aliases
                .iter()
                .any(|alias| alias.podman_version == value)
    }

    pub(crate) fn matches_reported_version_pair(&self, podman: &str, api: &str) -> bool {
        (podman == self.observed_podman_version && api == self.observed_libpod_api_version)
            || self
                .reported_version_aliases
                .iter()
                .any(|alias| alias.podman_version == podman && alias.libpod_api_version == api)
    }

    /// Returns the immutable source evidence.
    #[must_use]
    pub fn evidence(&self) -> &EvidenceReference {
        &self.evidence
    }
}

const fn default_output_supported() -> bool {
    true
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

pub(crate) fn normalized_reported_version(value: &str) -> PodmanLensResult<Option<String>> {
    for entry in capability_catalogue()? {
        for alias in entry.reported_version_aliases {
            if alias.podman_version == value {
                return Ok(Some(entry.observed_podman_version));
            }
            if alias.libpod_api_version == value {
                return Ok(Some(entry.observed_libpod_api_version));
            }
        }
    }
    Ok(None)
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
    if !valid_legacy_catalogue(entries) {
        return false;
    }
    valid_output_catalogue(entries)
}

fn valid_legacy_catalogue(entries: &[CapabilityCatalogueEntry]) -> bool {
    let legacy = entries
        .iter()
        .filter(|entry| !entry.output_supported)
        .collect::<Vec<_>>();
    if legacy.len() != LEGACY_INPUT_ANCHORS.len() {
        return false;
    }
    for (entry, (minimum, maximum, minimum_api, expected_api, revision, source_path)) in
        legacy.into_iter().zip(LEGACY_INPUT_ANCHORS)
    {
        let Some((parsed_minimum, parsed_maximum, _parsed_minimum_api, observed_engine, observed_api)) =
            parsed_versions(entry)
        else {
            return false;
        };
        let Ok(expected_observed_api) = Version::parse(expected_api) else {
            return false;
        };
        let expected_source = format!("https://github.com/containers/podman/blob/{revision}/{source_path}");
        let aliases_are_valid = if entry.observed_podman_version == "4.9.4" {
            valid_rhel_aliases(&entry.reported_version_aliases)
        } else {
            entry.reported_version_aliases.is_empty()
        };
        if entry.minimum_podman_version != minimum
            || entry.maximum_exclusive_podman_version != maximum
            || entry.minimum_libpod_api_version != minimum_api
            || entry.observed_podman_version != minimum
            || entry.observed_libpod_api_version != expected_api
            || entry.evidence.revision != revision
            || entry.evidence.source != expected_source
            || parsed_minimum != observed_engine
            || observed_api != expected_observed_api
            || parsed_maximum <= parsed_minimum
            || entry.podman_minor_line != format!("{}.{}", parsed_minimum.major, parsed_minimum.minor)
            || entry.evidence.release_tag != format!("v{observed_engine}")
            || !is_lowercase_sha40(&entry.evidence.revision)
            || !aliases_are_valid
        {
            return false;
        }
    }

    true
}

fn valid_rhel_aliases(aliases: &[ReportedVersionAlias]) -> bool {
    let [alias] = aliases else {
        return false;
    };
    let (podman, api, image, package, source, revision, release_tag) = REVIEWED_RHEL_ALIAS;
    alias.podman_version == podman
        && alias.libpod_api_version == api
        && alias.image == image
        && alias.package_revision == package
        && alias.root_mode == "rootful"
        && alias.evidence.source == source
        && alias.evidence.revision == revision
        && alias.evidence.release_tag == release_tag
        && is_lowercase_sha40(&alias.evidence.revision)
}

fn valid_output_catalogue(entries: &[CapabilityCatalogueEntry]) -> bool {
    let mut expected_minimum = Version::new(5, 4, 0);
    for entry in entries.iter().filter(|entry| entry.output_supported) {
        if !entry.reported_version_aliases.is_empty() {
            return false;
        }
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
        catalogue.reviewed_lines[5].evidence.revision.replace_range(0..1, "F");
        assert!(!valid_catalogue(&catalogue.reviewed_lines));

        let mut catalogue = decoded_catalogue()?;
        catalogue.reviewed_lines[5].evidence.source = "https://example.invalid/mutable".to_owned();
        assert!(!valid_catalogue(&catalogue.reviewed_lines));

        let mut catalogue = decoded_catalogue()?;
        catalogue.reviewed_lines[5].observed_podman_version = "5.4.1".to_owned();
        assert!(!valid_catalogue(&catalogue.reviewed_lines));

        let mut catalogue = decoded_catalogue()?;
        catalogue.reviewed_lines[0].evidence.source = "https://example.invalid/mutable".to_owned();
        assert!(!valid_catalogue(&catalogue.reviewed_lines));

        let mut catalogue = decoded_catalogue()?;
        catalogue.reviewed_lines[1].observed_libpod_api_version = "3.1.0".to_owned();
        assert!(!valid_catalogue(&catalogue.reviewed_lines));

        let mut catalogue = decoded_catalogue()?;
        catalogue.reviewed_lines[4].reported_version_aliases[0].image =
            "ghcr.io/strukturpiloten/podman-ubi-8-rootful:mutable".to_owned();
        assert!(!valid_catalogue(&catalogue.reviewed_lines));

        let mut catalogue = decoded_catalogue()?;
        catalogue.reviewed_lines[4].reported_version_aliases[0].podman_version = "4.9.4-vendor".to_owned();
        assert!(!valid_catalogue(&catalogue.reviewed_lines));
        Ok(())
    }
}
