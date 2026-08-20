//! Strict, versioned ledger for native-field coverage in the public inventory contract.

use serde::Deserialize;

use crate::{Diagnostic, DiagnosticCode, PodmanLensResult};

const COVERAGE_CATALOGUE_JSON: &str = include_str!("../catalogue/v1/native-field-coverage.json");

/// The outcome currently declared for one native Podman field.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "kebab-case")]
#[non_exhaustive]
pub enum NativeFieldCoverageClassification {
    /// The field has a complete, exact typed contract.
    SupportedExact,
    /// The field is represented only for reviewed target-version conditions.
    TargetGated,
    /// The field requires a manual caller action rather than being retained as data.
    Manual,
    /// The field is intentionally retained only for observation, not semantic output.
    ObservationOnly,
    /// Retained metadata is deliberately bounded and therefore not exhaustive.
    UnknownIncomplete,
}

impl NativeFieldCoverageClassification {
    const fn as_str(self) -> &'static str {
        match self {
            Self::SupportedExact => "supported-exact",
            Self::TargetGated => "target-gated",
            Self::Manual => "manual",
            Self::ObservationOnly => "observation-only",
            Self::UnknownIncomplete => "unknown-incomplete",
        }
    }
}

struct ExpectedEntry {
    id: &'static str,
    resource_kind: &'static str,
    native_path: &'static str,
    classification: &'static str,
    decoder: &'static str,
    planner: &'static str,
    renderer: &'static str,
    public_contract: &'static str,
    finding: &'static str,
    positive_test: &'static str,
    negative_test: &'static str,
}

macro_rules! expected {
    ($id:literal, $resource_kind:literal, $native_path:literal, $classification:literal, $decoder:literal, $planner:literal, $renderer:literal, $public_contract:literal, $finding:literal, $positive_test:literal, $negative_test:literal) => {
        ExpectedEntry {
            id: $id,
            resource_kind: $resource_kind,
            native_path: $native_path,
            classification: $classification,
            decoder: $decoder,
            planner: $planner,
            renderer: $renderer,
            public_contract: $public_contract,
            finding: $finding,
            positive_test: $positive_test,
            negative_test: $negative_test,
        }
    };
}

const EXPECTED_ENTRIES: &[ExpectedEntry] = &[
    expected!(
        "PLN-FLD-0001",
        "container",
        "$.Id",
        "supported-exact",
        "inventory::decode_container",
        "not_applicable",
        "not_applicable",
        "ResourceRecord::identity",
        "PLN0017",
        "tests::inventory::acquisition_probes_lists_every_kind_then_inspects_canonical_stable_ids",
        "tests::inventory::every_inspect_status_and_shape_failure_retains_a_partial_stable_identity"
    ),
    expected!(
        "PLN-FLD-0002",
        "container",
        "$.Name",
        "supported-exact",
        "inventory::decode_container",
        "not_applicable",
        "not_applicable",
        "ResourceRecord::identity",
        "PLN0017",
        "tests::inventory::acquisition_probes_lists_every_kind_then_inspects_canonical_stable_ids",
        "tests::inventory::every_inspect_status_and_shape_failure_retains_a_partial_stable_identity"
    ),
    expected!(
        "PLN-FLD-0003",
        "container",
        "$.Config.Labels",
        "observation-only",
        "inventory::decode_container",
        "not_applicable",
        "not_applicable",
        "ResourceRecord::labels",
        "PLN0017",
        "tests::inventory::acquisition_probes_lists_every_kind_then_inspects_canonical_stable_ids",
        "tests::inventory::modeled_nested_boundaries_report_the_precise_path_without_hiding_the_record"
    ),
    expected!(
        "PLN-FLD-0004",
        "container",
        "$.Config.Env",
        "observation-only",
        "inventory::decode_container",
        "not_applicable",
        "not_applicable",
        "ResourceRecord::environment",
        "PLN0019",
        "tests::inventory::explicit_environment_inclusion_is_opaque_and_preserves_duplicate_order",
        "tests::inventory::environment_boundaries_preserve_valid_entries_and_report_every_bad_occurrence"
    ),
    expected!(
        "PLN-FLD-0005",
        "container",
        "$.Config.Secrets",
        "observation-only",
        "inventory::decode_container_secrets",
        "not_applicable",
        "not_applicable",
        "ResourceRecord::relationships",
        "PLN0017",
        "tests::inventory::acquisition_probes_lists_every_kind_then_inspects_canonical_stable_ids",
        "tests::inventory::modeled_nested_boundaries_report_the_precise_path_without_hiding_the_record"
    ),
    expected!(
        "PLN-FLD-0006",
        "container",
        "$.Image",
        "observation-only",
        "inventory::decode_container",
        "not_applicable",
        "not_applicable",
        "ResourceRecord::relationships",
        "PLN0017",
        "tests::inventory::acquisition_probes_lists_every_kind_then_inspects_canonical_stable_ids",
        "tests::inventory::modeled_nested_boundaries_report_the_precise_path_without_hiding_the_record"
    ),
    expected!(
        "PLN-FLD-0007",
        "container",
        "$.ImageName",
        "observation-only",
        "inventory::decode_container",
        "not_applicable",
        "not_applicable",
        "ResourceRecord::relationships",
        "PLN0017",
        "tests::inventory::acquisition_probes_lists_every_kind_then_inspects_canonical_stable_ids",
        "tests::inventory::modeled_nested_boundaries_report_the_precise_path_without_hiding_the_record"
    ),
    expected!(
        "PLN-FLD-0008",
        "container",
        "$.Pod",
        "observation-only",
        "inventory::decode_container",
        "not_applicable",
        "not_applicable",
        "ResourceRecord::relationships",
        "PLN0017",
        "tests::inventory::acquisition_probes_lists_every_kind_then_inspects_canonical_stable_ids",
        "tests::inventory::modeled_nested_boundaries_report_the_precise_path_without_hiding_the_record"
    ),
    expected!(
        "PLN-FLD-0009",
        "container",
        "$.NetworkSettings.Networks",
        "observation-only",
        "inventory::decode_container_networks",
        "not_applicable",
        "not_applicable",
        "ResourceRecord::relationships",
        "PLN0017",
        "tests::inventory::acquisition_probes_lists_every_kind_then_inspects_canonical_stable_ids",
        "tests::inventory::modeled_nested_boundaries_report_the_precise_path_without_hiding_the_record"
    ),
    expected!(
        "PLN-FLD-0010",
        "container",
        "$.Mounts",
        "observation-only",
        "inventory::decode_mounts",
        "not_applicable",
        "not_applicable",
        "ResourceRecord::relationships",
        "PLN0017",
        "tests::inventory::acquisition_probes_lists_every_kind_then_inspects_canonical_stable_ids",
        "tests::inventory::modeled_nested_boundaries_report_the_precise_path_without_hiding_the_record"
    ),
    expected!(
        "PLN-FLD-0011",
        "container",
        "$.Dependencies",
        "observation-only",
        "inventory::decode_dependencies",
        "not_applicable",
        "not_applicable",
        "ResourceRecord::relationships",
        "PLN0017",
        "tests::inventory::acquisition_probes_lists_every_kind_then_inspects_canonical_stable_ids",
        "tests::inventory::modeled_nested_boundaries_report_the_precise_path_without_hiding_the_record"
    ),
    expected!(
        "PLN-FLD-0012",
        "container",
        "$.HostConfig.MemorySwappiness",
        "target-gated",
        "inventory::decode_memory_swappiness",
        "not_applicable",
        "not_applicable",
        "ResourceRecord::memory_swappiness",
        "PLN0022",
        "tests::inventory::memory_swappiness_distinguishes_reviewed_null_boundary_and_invalid_values",
        "tests::inventory::memory_swappiness_distinguishes_reviewed_null_boundary_and_invalid_values"
    ),
    expected!(
        "PLN-FLD-0013",
        "container",
        "$.HostConfig.*",
        "unknown-incomplete",
        "inventory::unknown_nested_fields",
        "not_applicable",
        "not_applicable",
        "ResourceRecord::unknown_fields",
        "PLN0023",
        "tests::inventory::host_config_members_not_yet_modeled_are_retained_as_unknown_metadata",
        "tests::inventory::unknown_fields_are_bounded_per_record_and_across_the_inventory"
    ),
    expected!(
        "PLN-FLD-0014",
        "container",
        "$.IsInfra",
        "observation-only",
        "inventory::decode_is_infra",
        "not_applicable",
        "not_applicable",
        "ResourceRecord::is_infra",
        "PLN0017",
        "tests::inventory::acquisition_probes_lists_every_kind_then_inspects_canonical_stable_ids",
        "tests::inventory::modeled_nested_boundaries_report_the_precise_path_without_hiding_the_record"
    ),
    expected!(
        "PLN-FLD-0015",
        "pod",
        "$.Id",
        "supported-exact",
        "inventory::decode_pod",
        "not_applicable",
        "not_applicable",
        "ResourceRecord::identity",
        "PLN0017",
        "tests::inventory::acquisition_probes_lists_every_kind_then_inspects_canonical_stable_ids",
        "tests::inventory::every_inspect_status_and_shape_failure_retains_a_partial_stable_identity"
    ),
    expected!(
        "PLN-FLD-0016",
        "pod",
        "$.Name",
        "supported-exact",
        "inventory::decode_pod",
        "not_applicable",
        "not_applicable",
        "ResourceRecord::identity",
        "PLN0017",
        "tests::inventory::acquisition_probes_lists_every_kind_then_inspects_canonical_stable_ids",
        "tests::inventory::every_inspect_status_and_shape_failure_retains_a_partial_stable_identity"
    ),
    expected!(
        "PLN-FLD-0017",
        "pod",
        "$.Labels",
        "observation-only",
        "inventory::decode_pod",
        "not_applicable",
        "not_applicable",
        "ResourceRecord::labels",
        "PLN0017",
        "tests::inventory::acquisition_probes_lists_every_kind_then_inspects_canonical_stable_ids",
        "tests::inventory::modeled_nested_boundaries_report_the_precise_path_without_hiding_the_record"
    ),
    expected!(
        "PLN-FLD-0018",
        "pod",
        "$.Containers",
        "observation-only",
        "inventory::decode_pod_containers",
        "not_applicable",
        "not_applicable",
        "ResourceRecord::relationships",
        "PLN0017",
        "tests::inventory::acquisition_probes_lists_every_kind_then_inspects_canonical_stable_ids",
        "tests::inventory::modeled_nested_boundaries_report_the_precise_path_without_hiding_the_record"
    ),
    expected!(
        "PLN-FLD-0019",
        "pod",
        "$.Networks",
        "observation-only",
        "inventory::decode_pod_networks",
        "not_applicable",
        "not_applicable",
        "ResourceRecord::relationships",
        "PLN0017",
        "tests::inventory::acquisition_probes_lists_every_kind_then_inspects_canonical_stable_ids",
        "tests::inventory::modeled_nested_boundaries_report_the_precise_path_without_hiding_the_record"
    ),
    expected!(
        "PLN-FLD-0020",
        "network",
        "$.id",
        "supported-exact",
        "inventory::decode_network",
        "not_applicable",
        "not_applicable",
        "ResourceRecord::identity",
        "PLN0017",
        "tests::inventory::acquisition_probes_lists_every_kind_then_inspects_canonical_stable_ids",
        "tests::inventory::every_inspect_status_and_shape_failure_retains_a_partial_stable_identity"
    ),
    expected!(
        "PLN-FLD-0021",
        "network",
        "$.name",
        "supported-exact",
        "inventory::decode_network",
        "not_applicable",
        "not_applicable",
        "ResourceRecord::identity",
        "PLN0017",
        "tests::inventory::acquisition_probes_lists_every_kind_then_inspects_canonical_stable_ids",
        "tests::inventory::every_inspect_status_and_shape_failure_retains_a_partial_stable_identity"
    ),
    expected!(
        "PLN-FLD-0022",
        "network",
        "$.labels",
        "observation-only",
        "inventory::decode_network",
        "not_applicable",
        "not_applicable",
        "ResourceRecord::labels",
        "PLN0017",
        "tests::inventory::acquisition_probes_lists_every_kind_then_inspects_canonical_stable_ids",
        "tests::inventory::modeled_nested_boundaries_report_the_precise_path_without_hiding_the_record"
    ),
    expected!(
        "PLN-FLD-0023",
        "network",
        "$.internal",
        "observation-only",
        "inventory::decode_network_details",
        "not_applicable",
        "not_applicable",
        "NetworkDetails::internal",
        "PLN0017",
        "tests::inventory::acquisition_probes_lists_every_kind_then_inspects_canonical_stable_ids",
        "tests::inventory::modeled_nested_boundaries_report_the_precise_path_without_hiding_the_record"
    ),
    expected!(
        "PLN-FLD-0024",
        "network",
        "$.options",
        "observation-only",
        "inventory::decode_network_details",
        "not_applicable",
        "not_applicable",
        "NetworkDetails::options",
        "PLN0017",
        "tests::inventory::acquisition_probes_lists_every_kind_then_inspects_canonical_stable_ids",
        "tests::inventory::modeled_nested_boundaries_report_the_precise_path_without_hiding_the_record"
    ),
    expected!(
        "PLN-FLD-0025",
        "network",
        "$.subnets",
        "observation-only",
        "inventory::decode_network_details",
        "not_applicable",
        "not_applicable",
        "NetworkDetails::subnets",
        "PLN0017",
        "tests::inventory::acquisition_probes_lists_every_kind_then_inspects_canonical_stable_ids",
        "tests::inventory::modeled_nested_boundaries_report_the_precise_path_without_hiding_the_record"
    ),
    expected!(
        "PLN-FLD-0026",
        "volume",
        "$.Name",
        "supported-exact",
        "inventory::decode_volume",
        "not_applicable",
        "not_applicable",
        "ResourceRecord::identity",
        "PLN0017",
        "tests::inventory::acquisition_probes_lists_every_kind_then_inspects_canonical_stable_ids",
        "tests::inventory::every_inspect_status_and_shape_failure_retains_a_partial_stable_identity"
    ),
    expected!(
        "PLN-FLD-0027",
        "volume",
        "$.Labels",
        "observation-only",
        "inventory::decode_volume",
        "not_applicable",
        "not_applicable",
        "ResourceRecord::labels",
        "PLN0017",
        "tests::inventory::acquisition_probes_lists_every_kind_then_inspects_canonical_stable_ids",
        "tests::inventory::modeled_nested_boundaries_report_the_precise_path_without_hiding_the_record"
    ),
    expected!(
        "PLN-FLD-0028",
        "image",
        "$.Id",
        "supported-exact",
        "inventory::decode_image",
        "not_applicable",
        "not_applicable",
        "ResourceRecord::identity",
        "PLN0017",
        "tests::inventory::acquisition_probes_lists_every_kind_then_inspects_canonical_stable_ids",
        "tests::inventory::every_inspect_status_and_shape_failure_retains_a_partial_stable_identity"
    ),
    expected!(
        "PLN-FLD-0029",
        "image",
        "$.Names",
        "observation-only",
        "inventory::decode_image",
        "not_applicable",
        "not_applicable",
        "ResourceRecord::image_aliases",
        "PLN0017",
        "tests::inventory::acquisition_probes_lists_every_kind_then_inspects_canonical_stable_ids",
        "tests::inventory::modeled_nested_boundaries_report_the_precise_path_without_hiding_the_record"
    ),
    expected!(
        "PLN-FLD-0030",
        "image",
        "$.Labels",
        "observation-only",
        "inventory::decode_image",
        "not_applicable",
        "not_applicable",
        "ResourceRecord::labels",
        "PLN0017",
        "tests::inventory::acquisition_probes_lists_every_kind_then_inspects_canonical_stable_ids",
        "tests::inventory::modeled_nested_boundaries_report_the_precise_path_without_hiding_the_record"
    ),
    expected!(
        "PLN-FLD-0031",
        "image",
        "$.Config.Env",
        "observation-only",
        "inventory::decode_image",
        "not_applicable",
        "not_applicable",
        "ResourceRecord::environment",
        "PLN0019",
        "tests::inventory::explicit_environment_inclusion_is_opaque_and_preserves_duplicate_order",
        "tests::inventory::environment_boundaries_preserve_valid_entries_and_report_every_bad_occurrence"
    ),
    expected!(
        "PLN-FLD-0032",
        "secret",
        "$.ID",
        "supported-exact",
        "inventory::decode_secret",
        "not_applicable",
        "not_applicable",
        "ResourceRecord::identity",
        "PLN0017",
        "tests::inventory::acquisition_probes_lists_every_kind_then_inspects_canonical_stable_ids",
        "tests::inventory::every_inspect_status_and_shape_failure_retains_a_partial_stable_identity"
    ),
    expected!(
        "PLN-FLD-0033",
        "secret",
        "$.Spec.Name",
        "supported-exact",
        "inventory::decode_secret",
        "not_applicable",
        "not_applicable",
        "ResourceRecord::identity",
        "PLN0017",
        "tests::inventory::acquisition_probes_lists_every_kind_then_inspects_canonical_stable_ids",
        "tests::inventory::every_inspect_status_and_shape_failure_retains_a_partial_stable_identity"
    ),
    expected!(
        "PLN-FLD-0034",
        "secret",
        "$.Spec.Labels",
        "observation-only",
        "inventory::decode_secret",
        "not_applicable",
        "not_applicable",
        "ResourceRecord::labels",
        "PLN0017",
        "tests::inventory::acquisition_probes_lists_every_kind_then_inspects_canonical_stable_ids",
        "tests::inventory::modeled_nested_boundaries_report_the_precise_path_without_hiding_the_record"
    ),
    expected!(
        "PLN-FLD-0035",
        "secret",
        "$.Spec.Driver",
        "observation-only",
        "inventory::decode_secret",
        "not_applicable",
        "not_applicable",
        "ResourceRecord::secret_driver",
        "PLN0017",
        "tests::inventory::secret_driver_is_modeled_without_unsupported_metadata",
        "tests::inventory::modeled_nested_boundaries_report_the_precise_path_without_hiding_the_record"
    ),
    expected!(
        "PLN-FLD-0036",
        "secret",
        "$.SecretData",
        "manual",
        "inventory::decode_secret",
        "not_applicable",
        "not_applicable",
        "ResourceRecord::findings",
        "PLN0018",
        "tests::inventory::secret_payload_is_discarded_from_metadata_inspection",
        "tests::inventory::secret_payload_is_discarded_from_metadata_inspection"
    ),
    expected!(
        "PLN-FLD-0037",
        "secret",
        "$.Spec.SecretData",
        "manual",
        "inventory::decode_secret",
        "not_applicable",
        "not_applicable",
        "ResourceRecord::findings",
        "PLN0018",
        "tests::inventory::secret_payload_is_discarded_from_metadata_inspection",
        "tests::inventory::secret_payload_is_discarded_from_metadata_inspection"
    ),
    expected!(
        "PLN-FLD-0038",
        "all",
        "$.<unknown>",
        "unknown-incomplete",
        "inventory::unknown_top_level",
        "not_applicable",
        "not_applicable",
        "ResourceRecord::unknown_fields_complete",
        "PLN0021",
        "tests::inventory::unknown_fields_are_bounded_per_record_and_across_the_inventory",
        "tests::input_corpus::malformed_corpus_is_structured_and_bounded_never_panics"
    ),
];

/// One ledger row linking a native field to implementation, public API, diagnostics, and tests.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct NativeFieldCoverageEntry {
    id: String,
    resource_kind: String,
    native_path: String,
    classification: NativeFieldCoverageClassification,
    decoder: String,
    planner: String,
    renderer: String,
    public_contract: String,
    finding: String,
    positive_test: String,
    negative_test: String,
}

impl NativeFieldCoverageEntry {
    /// Returns the stable ledger identifier.
    #[must_use]
    pub fn id(&self) -> &str {
        &self.id
    }

    /// Returns the native resource kind covered by this row, or `all` for a global boundary.
    #[must_use]
    pub fn resource_kind(&self) -> &str {
        &self.resource_kind
    }

    /// Returns the native JSON path, including a documented wildcard where appropriate.
    #[must_use]
    pub fn native_path(&self) -> &str {
        &self.native_path
    }

    /// Returns the declared coverage outcome.
    #[must_use]
    pub const fn classification(&self) -> NativeFieldCoverageClassification {
        self.classification
    }

    /// Returns the private decoder ownership reference.
    #[must_use]
    pub fn decoder(&self) -> &str {
        &self.decoder
    }

    /// Returns the planner ownership reference, or `not_applicable` for observation-only input.
    #[must_use]
    pub fn planner(&self) -> &str {
        &self.planner
    }

    /// Returns the renderer ownership reference, or `not_applicable` for observation-only input.
    #[must_use]
    pub fn renderer(&self) -> &str {
        &self.renderer
    }

    /// Returns the stable public API access point for the field outcome.
    #[must_use]
    pub fn public_contract(&self) -> &str {
        &self.public_contract
    }

    /// Returns the stable diagnostic rule associated with malformed, unsupported, or manual input.
    #[must_use]
    pub fn finding(&self) -> &str {
        &self.finding
    }

    /// Returns the focused positive test identifier.
    #[must_use]
    pub fn positive_test(&self) -> &str {
        &self.positive_test
    }

    /// Returns the focused negative test identifier.
    #[must_use]
    pub fn negative_test(&self) -> &str {
        &self.negative_test
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CoverageCatalogue {
    schema_version: u8,
    scope: String,
    entries: Vec<NativeFieldCoverageEntry>,
}

/// Returns the strict, embedded native-field coverage ledger.
///
/// # Errors
///
/// Returns `PLN0047` when the packaged catalogue is malformed, incomplete, or internally
/// inconsistent with the decoder boundary it claims to cover.
pub fn native_field_coverage_catalogue() -> PodmanLensResult<Vec<NativeFieldCoverageEntry>> {
    parse_native_field_coverage_catalogue(COVERAGE_CATALOGUE_JSON)
}

fn parse_native_field_coverage_catalogue(source: &str) -> PodmanLensResult<Vec<NativeFieldCoverageEntry>> {
    let catalogue: CoverageCatalogue =
        serde_json::from_str(source).map_err(|_| Diagnostic::new(DiagnosticCode::NativeFieldCoverageUnavailable))?;
    if catalogue.schema_version != 1 || catalogue.scope != "m2-native-inventory" || !valid_entries(&catalogue.entries) {
        return Err(Diagnostic::new(DiagnosticCode::NativeFieldCoverageUnavailable));
    }
    Ok(catalogue.entries)
}

fn valid_entries(entries: &[NativeFieldCoverageEntry]) -> bool {
    entries.len() == EXPECTED_ENTRIES.len()
        && entries.iter().zip(EXPECTED_ENTRIES).all(|(entry, expected)| {
            entry.id == expected.id
                && entry.resource_kind == expected.resource_kind
                && entry.native_path == expected.native_path
                && entry.classification.as_str() == expected.classification
                && entry.decoder == expected.decoder
                && entry.planner == expected.planner
                && entry.renderer == expected.renderer
                && entry.public_contract == expected.public_contract
                && entry.finding == expected.finding
                && entry.positive_test == expected.positive_test
                && entry.negative_test == expected.negative_test
                && valid_reference(&entry.decoder, "inventory::")
                && valid_reference(&entry.public_contract, "")
                && valid_diagnostic(&entry.finding)
                && valid_reference(&entry.positive_test, "tests::")
                && valid_reference(&entry.negative_test, "tests::")
        })
}

fn valid_reference(value: &str, required_prefix: &str) -> bool {
    !value.is_empty()
        && value.len() <= 160
        && value.starts_with(required_prefix)
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b':' | b'_' | b'-'))
}

fn valid_diagnostic(value: &str) -> bool {
    value.len() == 7 && value.starts_with("PLN") && value.as_bytes()[3..].iter().all(u8::is_ascii_digit)
}

#[cfg(test)]
mod tests {
    use super::{COVERAGE_CATALOGUE_JSON, parse_native_field_coverage_catalogue};

    fn mutate_first_entry(field: &str, value: &str) -> Result<String, serde_json::Error> {
        let mut catalogue: serde_json::Value = serde_json::from_str(COVERAGE_CATALOGUE_JSON)?;
        catalogue["entries"][0][field] = serde_json::Value::String(value.to_owned());
        serde_json::to_string(&catalogue)
    }

    #[test]
    fn embedded_coverage_catalogue_is_strict_and_complete() {
        assert!(parse_native_field_coverage_catalogue(COVERAGE_CATALOGUE_JSON).is_ok());
    }

    #[test]
    fn malformed_or_incomplete_coverage_catalogue_is_rejected() {
        for (from, to) in [
            ("\"schema_version\": 1", "\"schema_version\": 2"),
            ("\"PLN-FLD-0037\"", "\"PLN-FLD-9999\""),
            ("\"decoder\"", "\"unknown_decoder\""),
        ] {
            assert!(
                parse_native_field_coverage_catalogue(&COVERAGE_CATALOGUE_JSON.replacen(from, to, 1)).is_err(),
                "mutation {from} -> {to} must fail"
            );
        }
    }

    #[test]
    fn every_semantic_ledger_link_is_pinned_to_the_expected_row() -> Result<(), serde_json::Error> {
        for (field, value) in [
            ("classification", "manual"),
            ("decoder", "inventory::decode_pod"),
            ("planner", "deployment::plan"),
            ("renderer", "render::deployment"),
            ("public_contract", "ResourceRecord::findings"),
            ("finding", "PLN0046"),
            (
                "positive_test",
                "tests::inventory::memory_swappiness_distinguishes_reviewed_null_boundary_and_invalid_values",
            ),
            (
                "negative_test",
                "tests::inventory::modeled_nested_boundaries_report_the_precise_path_without_hiding_the_record",
            ),
        ] {
            assert!(
                parse_native_field_coverage_catalogue(&mutate_first_entry(field, value)?).is_err(),
                "altered {field} must fail"
            );
        }
        Ok(())
    }
}
