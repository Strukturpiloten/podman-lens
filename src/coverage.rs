//! Strict, versioned coverage ledger for native observations and output intent.

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

/// The contract plane represented by one strict coverage-ledger row.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "kebab-case")]
#[non_exhaustive]
pub enum NativeFieldCoveragePlane {
    /// A native Libpod inspection observation accepted by the M2 decoder.
    InputObservation,
    /// A caller-declared deployment field classified by planning and rendering contracts.
    OutputIntent,
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

struct ExpectedInputEntry {
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
        ExpectedInputEntry {
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

const EXPECTED_INPUT_ENTRIES: &[ExpectedInputEntry] = &[
    expected!(
        "PLN-FLD-0001",
        "container",
        "$.Id",
        "supported-exact",
        "inventory::decode_container",
        "not_applicable",
        "not_applicable",
        "ObservationHeader::identity",
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
        "ObservationHeader::identity",
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
        "ContainerObservation::labels",
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
        "ContainerObservation::environment",
        "PLN0019",
        "tests::inventory::explicit_environment_inclusion_is_opaque_and_preserves_duplicate_order",
        "tests::inventory::environment_boundaries_preserve_valid_entries_and_report_every_bad_occurrence"
    ),
    expected!(
        "PLN-FLD-0005",
        "container",
        "$.Config.Secrets",
        "observation-only",
        "inventory::decode_container_secret_grants",
        "not_applicable",
        "not_applicable",
        "ContainerObservation::secret_grants",
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
        "ContainerObservation::local_image_id",
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
        "ContainerObservation::configured_image",
        "PLN0017",
        "tests::inventory::acquisition_probes_lists_every_kind_then_inspects_canonical_stable_ids",
        "tests::inventory::modeled_nested_boundaries_report_the_precise_path_without_hiding_the_record"
    ),
    expected!(
        "PLN-FLD-0008",
        "container",
        "$.Pod",
        "observation-only",
        "inventory::decode_native_reference",
        "not_applicable",
        "not_applicable",
        "ContainerObservation::pod_membership",
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
        "ResourceGraph::dependencies",
        "PLN0017",
        "tests::inventory::acquisition_probes_lists_every_kind_then_inspects_canonical_stable_ids",
        "tests::inventory::modeled_nested_boundaries_report_the_precise_path_without_hiding_the_record"
    ),
    expected!(
        "PLN-FLD-0010",
        "container",
        "$.Mounts",
        "observation-only",
        "inventory::decode_container_mounts",
        "not_applicable",
        "not_applicable",
        "ContainerObservation::mounts",
        "PLN0017",
        "tests::inventory::acquisition_probes_lists_every_kind_then_inspects_canonical_stable_ids",
        "tests::inventory::modeled_nested_boundaries_report_the_precise_path_without_hiding_the_record"
    ),
    expected!(
        "PLN-FLD-0011",
        "container",
        "$.Dependencies",
        "observation-only",
        "inventory::decode_native_dependencies",
        "not_applicable",
        "not_applicable",
        "ContainerObservation::native_dependencies",
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
        "ContainerObservation::memory_swappiness",
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
        "ObservationHeader::unmodelled_fields",
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
        "ContainerObservation::infra",
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
        "ObservationHeader::identity",
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
        "ObservationHeader::identity",
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
        "PodObservation::labels",
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
        "ResourceGraph::dependencies",
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
        "ResourceGraph::dependencies",
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
        "ObservationHeader::identity",
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
        "ObservationHeader::identity",
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
        "NetworkObservation::labels",
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
        "NetworkObservation::internal",
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
        "NetworkObservation::options",
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
        "NetworkObservation::subnets",
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
        "ObservationHeader::identity",
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
        "VolumeObservation::labels",
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
        "ObservationHeader::identity",
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
        "ImageObservation::aliases",
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
        "ImageObservation::labels",
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
        "ImageObservation::environment",
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
        "ObservationHeader::identity",
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
        "ObservationHeader::identity",
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
        "SecretObservation::labels",
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
        "SecretObservation::driver",
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
        "ObservationHeader::findings",
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
        "ObservationHeader::findings",
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
        "ObservationHeader::unmodelled_completeness",
        "PLN0021",
        "tests::inventory::unknown_fields_are_bounded_per_record_and_across_the_inventory",
        "tests::input_corpus::malformed_corpus_is_structured_and_bounded_never_panics"
    ),
    expected!(
        "PLN-FLD-0039",
        "container",
        "$.Config.Cmd",
        "observation-only",
        "inventory::decode_container_configuration",
        "not_applicable",
        "not_applicable",
        "ContainerObservation::command",
        "PLN0017",
        "tests::inventory::container_core_mount_and_secret_observations_are_typed_and_redacted",
        "tests::inventory::malformed_enclosing_config_marks_every_modeled_child_malformed"
    ),
    expected!(
        "PLN-FLD-0040",
        "container",
        "$.Config.Entrypoint",
        "observation-only",
        "inventory::decode_container_configuration",
        "not_applicable",
        "not_applicable",
        "ContainerObservation::entrypoint",
        "PLN0017",
        "tests::inventory::container_core_mount_and_secret_observations_are_typed_and_redacted",
        "tests::inventory::malformed_enclosing_config_marks_every_modeled_child_malformed"
    ),
    expected!(
        "PLN-FLD-0041",
        "container",
        "$.Config.User",
        "observation-only",
        "inventory::decode_container_configuration",
        "not_applicable",
        "not_applicable",
        "ContainerObservation::user",
        "PLN0017",
        "tests::inventory::container_core_mount_and_secret_observations_are_typed_and_redacted",
        "tests::inventory::malformed_enclosing_config_marks_every_modeled_child_malformed"
    ),
    expected!(
        "PLN-FLD-0042",
        "container",
        "$.Config.WorkingDir",
        "observation-only",
        "inventory::decode_container_configuration",
        "not_applicable",
        "not_applicable",
        "ContainerObservation::working_directory",
        "PLN0017",
        "tests::inventory::container_core_mount_and_secret_observations_are_typed_and_redacted",
        "tests::inventory::malformed_enclosing_config_marks_every_modeled_child_malformed"
    ),
    expected!(
        "PLN-FLD-0043",
        "container",
        "$.Config.Hostname",
        "observation-only",
        "inventory::decode_container_configuration",
        "not_applicable",
        "not_applicable",
        "ContainerObservation::hostname",
        "PLN0017",
        "tests::inventory::container_core_mount_and_secret_observations_are_typed_and_redacted",
        "tests::inventory::malformed_enclosing_config_marks_every_modeled_child_malformed"
    ),
    expected!(
        "PLN-FLD-0046",
        "container",
        "$.Mounts.Type",
        "observation-only",
        "inventory::decode_container_mounts",
        "not_applicable",
        "not_applicable",
        "ContainerObservation::mounts",
        "PLN0023",
        "tests::inventory::container_core_mount_and_secret_observations_are_typed_and_redacted",
        "tests::inventory::malformed_secret_aliases_and_unsupported_mounts_remain_non_deployable_evidence"
    ),
    expected!(
        "PLN-FLD-0047",
        "container",
        "$.Mounts.Name",
        "observation-only",
        "inventory::decode_container_mounts",
        "not_applicable",
        "not_applicable",
        "ContainerMountObservation::source",
        "PLN0017",
        "tests::inventory::container_core_mount_and_secret_observations_are_typed_and_redacted",
        "tests::inventory::modeled_nested_boundaries_report_the_precise_path_without_hiding_the_record"
    ),
    expected!(
        "PLN-FLD-0048",
        "container",
        "$.Mounts.Source",
        "observation-only",
        "inventory::decode_container_mounts",
        "not_applicable",
        "not_applicable",
        "ContainerMountObservation::source",
        "PLN0017",
        "tests::inventory::container_core_mount_and_secret_observations_are_typed_and_redacted",
        "tests::inventory::malformed_mount_destination_or_local_backing_path_invalidates_the_complete_mount_family"
    ),
    expected!(
        "PLN-FLD-0049",
        "container",
        "$.Mounts.Destination",
        "observation-only",
        "inventory::decode_container_mounts",
        "not_applicable",
        "not_applicable",
        "ContainerMountObservation::destination",
        "PLN0017",
        "tests::inventory::container_core_mount_and_secret_observations_are_typed_and_redacted",
        "tests::inventory::malformed_mount_destination_or_local_backing_path_invalidates_the_complete_mount_family"
    ),
    expected!(
        "PLN-FLD-0050",
        "container",
        "$.Mounts.RW",
        "observation-only",
        "inventory::decode_container_mounts",
        "not_applicable",
        "not_applicable",
        "ContainerMountObservation::writable",
        "PLN0017",
        "tests::inventory::container_core_mount_and_secret_observations_are_typed_and_redacted",
        "tests::inventory::modeled_nested_boundaries_report_the_precise_path_without_hiding_the_record"
    ),
    expected!(
        "PLN-FLD-0051",
        "container",
        "$.Mounts.Options",
        "observation-only",
        "inventory::decode_container_mounts",
        "not_applicable",
        "not_applicable",
        "ContainerMountObservation::options",
        "PLN0017",
        "tests::inventory::container_core_mount_and_secret_observations_are_typed_and_redacted",
        "tests::inventory::modeled_nested_boundaries_report_the_precise_path_without_hiding_the_record"
    ),
    expected!(
        "PLN-FLD-0052",
        "container",
        "$.Mounts.Propagation",
        "observation-only",
        "inventory::decode_container_mounts",
        "not_applicable",
        "not_applicable",
        "ContainerMountObservation::propagation",
        "PLN0017",
        "tests::inventory::container_core_mount_and_secret_observations_are_typed_and_redacted",
        "tests::inventory::modeled_nested_boundaries_report_the_precise_path_without_hiding_the_record"
    ),
    expected!(
        "PLN-FLD-0053",
        "container",
        "$.Mounts.SubPath",
        "observation-only",
        "inventory::decode_container_mounts",
        "not_applicable",
        "not_applicable",
        "ContainerMountObservation::subpath",
        "PLN0017",
        "tests::inventory::container_core_mount_and_secret_observations_are_typed_and_redacted",
        "tests::inventory::modeled_nested_boundaries_report_the_precise_path_without_hiding_the_record"
    ),
    expected!(
        "PLN-FLD-0054",
        "container",
        "$.Config.Secrets.ID",
        "observation-only",
        "inventory::decode_container_secret_grants",
        "not_applicable",
        "not_applicable",
        "ContainerSecretGrantObservation::reference",
        "PLN0017",
        "tests::inventory::container_core_mount_and_secret_observations_are_typed_and_redacted",
        "tests::inventory::malformed_secret_aliases_and_unsupported_mounts_remain_non_deployable_evidence"
    ),
    expected!(
        "PLN-FLD-0055",
        "container",
        "$.Config.Secrets.Name",
        "observation-only",
        "inventory::decode_container_secret_grants",
        "not_applicable",
        "not_applicable",
        "ContainerSecretGrantObservation::reference",
        "PLN0017",
        "tests::inventory::container_core_mount_and_secret_observations_are_typed_and_redacted",
        "tests::inventory::malformed_secret_aliases_and_unsupported_mounts_remain_non_deployable_evidence"
    ),
    expected!(
        "PLN-FLD-0056",
        "container",
        "$.Config.Secrets.UID",
        "observation-only",
        "inventory::decode_container_secret_grants",
        "not_applicable",
        "not_applicable",
        "ContainerSecretGrantObservation::uid",
        "PLN0017",
        "tests::inventory::canonical_direct_secret_metadata_preserves_effective_zero_and_configured_aliases",
        "tests::inventory::malformed_direct_secret_effective_metadata_invalidates_the_grant_family"
    ),
    expected!(
        "PLN-FLD-0057",
        "container",
        "$.Config.Secrets.GID",
        "observation-only",
        "inventory::decode_container_secret_grants",
        "not_applicable",
        "not_applicable",
        "ContainerSecretGrantObservation::gid",
        "PLN0017",
        "tests::inventory::canonical_direct_secret_metadata_preserves_effective_zero_and_configured_aliases",
        "tests::inventory::malformed_direct_secret_effective_metadata_invalidates_the_grant_family"
    ),
    expected!(
        "PLN-FLD-0058",
        "container",
        "$.Config.Secrets.Mode",
        "observation-only",
        "inventory::decode_container_secret_grants",
        "not_applicable",
        "not_applicable",
        "ContainerSecretGrantObservation::mode",
        "PLN0017",
        "tests::inventory::canonical_direct_secret_metadata_preserves_effective_zero_and_configured_aliases",
        "tests::inventory::malformed_direct_secret_effective_metadata_invalidates_the_grant_family"
    ),
];

const ALL_REVIEWED_TARGETS: &[&str] = &["5.4.0", "5.5.0", "5.6.0", "5.7.0", "5.8.6", "6.0.0", "6.1.0"];
const UNLIMITED_RLIMIT_TARGETS: &[&str] = &["5.6.0", "5.7.0", "5.8.6", "6.0.0", "6.1.0"];
const JOURNALD_LABEL_TARGETS: &[&str] = &["6.0.0", "6.1.0"];
const B4_VERSIONED_TARGETS: &[&str] = &["5.6.0", "5.7.0", "5.8.6", "6.0.0", "6.1.0"];

struct ExpectedOutputEntry {
    id: &'static str,
    resource_kind: &'static str,
    field_path: &'static str,
    classification: &'static str,
    target_versions: &'static [&'static str],
    planner: &'static str,
    cli_renderer: &'static str,
    libpod_renderer: &'static str,
    public_contract: &'static str,
    finding: &'static str,
    positive_test: &'static str,
    negative_test: &'static str,
}

macro_rules! output {
    ($id:literal, $field_path:literal, $classification:literal, $target_versions:expr, $public_contract:literal, $positive_test:expr, $negative_test:expr) => {
        ExpectedOutputEntry {
            id: $id,
            resource_kind: "container",
            field_path: $field_path,
            classification: $classification,
            target_versions: $target_versions,
            planner: "deployment::validate_runtime_settings",
            cli_renderer: "render::append_container_runtime_arguments",
            libpod_renderer: "render::append_container_runtime_json",
            public_contract: $public_contract,
            finding: "PLN0046",
            positive_test: $positive_test,
            negative_test: $negative_test,
        }
    };
    (manual $id:literal, $field_path:literal, $public_contract:literal) => {
        ExpectedOutputEntry {
            id: $id,
            resource_kind: "container",
            field_path: $field_path,
            classification: "manual",
            target_versions: ALL_REVIEWED_TARGETS,
            planner: "deployment::validate_runtime_settings",
            cli_renderer: "render::unsupported_runtime_fields",
            libpod_renderer: "render::health_command_json",
            public_contract: $public_contract,
            finding: "PLN0046",
            positive_test: "tests::runtime::sensitive_health_command_blocks_the_resource_without_leaking_an_artifact",
            negative_test: "tests::runtime::sensitive_health_commands_never_leak_from_runtime_debug",
        }
    };
}

macro_rules! b4_output {
    ($id:literal, $resource_kind:literal, $field_path:literal, $classification:literal, $target_versions:expr, $planner:literal, $cli_renderer:literal, $libpod_renderer:literal, $public_contract:literal, $positive_test:literal, $negative_test:literal) => {
        ExpectedOutputEntry {
            id: $id,
            resource_kind: $resource_kind,
            field_path: $field_path,
            classification: $classification,
            target_versions: $target_versions,
            planner: $planner,
            cli_renderer: $cli_renderer,
            libpod_renderer: $libpod_renderer,
            public_contract: $public_contract,
            finding: "PLN0046",
            positive_test: $positive_test,
            negative_test: $negative_test,
        }
    };
    (manual $id:literal, $resource_kind:literal, $field_path:literal, $planner:literal, $cli_renderer:literal, $libpod_renderer:literal, $public_contract:literal, $positive_test:literal, $negative_test:literal) => {
        b4_output!(
            $id,
            $resource_kind,
            $field_path,
            "manual",
            ALL_REVIEWED_TARGETS,
            $planner,
            $cli_renderer,
            $libpod_renderer,
            $public_contract,
            $positive_test,
            $negative_test
        )
    };
}

const RUNTIME_RENDER_POSITIVE: &str = "tests::runtime::bounded_runtime_intent_plans_and_renders_exactly";
const RUNTIME_RENDER_NEGATIVE: &str =
    "tests::runtime::bounded_runtime_values_reject_invalid_inputs_and_preserve_explicit_false";

const EXPECTED_OUTPUT_ENTRIES: &[ExpectedOutputEntry] = &[
    output!(
        "PLN-OUT-0001",
        "runtime.health.disabled",
        "supported-exact",
        ALL_REVIEWED_TARGETS,
        "ContainerRuntimeSettings::health",
        RUNTIME_RENDER_POSITIVE,
        RUNTIME_RENDER_NEGATIVE
    ),
    output!(
        "PLN-OUT-0002",
        "runtime.health.command.public",
        "supported-exact",
        ALL_REVIEWED_TARGETS,
        "ConfiguredHealthCheck::command",
        RUNTIME_RENDER_POSITIVE,
        RUNTIME_RENDER_NEGATIVE
    ),
    output!(
        "PLN-OUT-0003",
        "runtime.health.interval",
        "supported-exact",
        ALL_REVIEWED_TARGETS,
        "ConfiguredHealthCheck::interval",
        RUNTIME_RENDER_POSITIVE,
        RUNTIME_RENDER_NEGATIVE
    ),
    output!(
        "PLN-OUT-0004",
        "runtime.health.timeout",
        "supported-exact",
        ALL_REVIEWED_TARGETS,
        "ConfiguredHealthCheck::timeout",
        RUNTIME_RENDER_POSITIVE,
        RUNTIME_RENDER_NEGATIVE
    ),
    output!(
        "PLN-OUT-0005",
        "runtime.health.retries",
        "supported-exact",
        ALL_REVIEWED_TARGETS,
        "ConfiguredHealthCheck::retries",
        RUNTIME_RENDER_POSITIVE,
        RUNTIME_RENDER_NEGATIVE
    ),
    output!(
        "PLN-OUT-0006",
        "runtime.health.start_period",
        "supported-exact",
        ALL_REVIEWED_TARGETS,
        "ConfiguredHealthCheck::start_period",
        RUNTIME_RENDER_POSITIVE,
        RUNTIME_RENDER_NEGATIVE
    ),
    output!(
        "PLN-OUT-0007",
        "runtime.health.on_failure",
        "supported-exact",
        ALL_REVIEWED_TARGETS,
        "ConfiguredHealthCheck::on_failure",
        RUNTIME_RENDER_POSITIVE,
        RUNTIME_RENDER_NEGATIVE
    ),
    output!(
        "PLN-OUT-0008",
        "runtime.startup_health.command.public",
        "supported-exact",
        ALL_REVIEWED_TARGETS,
        "StartupHealthCheck::command",
        RUNTIME_RENDER_POSITIVE,
        RUNTIME_RENDER_NEGATIVE
    ),
    output!(
        "PLN-OUT-0009",
        "runtime.startup_health.interval",
        "supported-exact",
        ALL_REVIEWED_TARGETS,
        "StartupHealthCheck::interval",
        RUNTIME_RENDER_POSITIVE,
        RUNTIME_RENDER_NEGATIVE
    ),
    output!(
        "PLN-OUT-0010",
        "runtime.startup_health.timeout",
        "supported-exact",
        ALL_REVIEWED_TARGETS,
        "StartupHealthCheck::timeout",
        RUNTIME_RENDER_POSITIVE,
        RUNTIME_RENDER_NEGATIVE
    ),
    output!(
        "PLN-OUT-0011",
        "runtime.startup_health.retries",
        "supported-exact",
        ALL_REVIEWED_TARGETS,
        "StartupHealthCheck::retries",
        RUNTIME_RENDER_POSITIVE,
        RUNTIME_RENDER_NEGATIVE
    ),
    output!(
        "PLN-OUT-0012",
        "runtime.startup_health.successes",
        "supported-exact",
        ALL_REVIEWED_TARGETS,
        "StartupHealthCheck::successes",
        RUNTIME_RENDER_POSITIVE,
        RUNTIME_RENDER_NEGATIVE
    ),
    output!(
        "PLN-OUT-0013",
        "runtime.logging.driver",
        "supported-exact",
        ALL_REVIEWED_TARGETS,
        "LoggingSettings::driver",
        RUNTIME_RENDER_POSITIVE,
        RUNTIME_RENDER_NEGATIVE
    ),
    output!(
        "PLN-OUT-0014",
        "runtime.logging.max_size",
        "supported-exact",
        ALL_REVIEWED_TARGETS,
        "LoggingSettings::max_size",
        RUNTIME_RENDER_POSITIVE,
        RUNTIME_RENDER_NEGATIVE
    ),
    output!(
        "PLN-OUT-0015",
        "runtime.logging.journald_labels",
        "target-gated",
        JOURNALD_LABEL_TARGETS,
        "LoggingSettings::journald_labels",
        "tests::runtime::journald_labels_are_supported_from_podman_six_in_every_reviewed_target",
        "tests::runtime::journald_labels_are_supported_from_podman_six_in_every_reviewed_target"
    ),
    output!(
        "PLN-OUT-0016",
        "runtime.security.privileged",
        "supported-exact",
        ALL_REVIEWED_TARGETS,
        "SecuritySettings::privileged",
        RUNTIME_RENDER_POSITIVE,
        RUNTIME_RENDER_NEGATIVE
    ),
    output!(
        "PLN-OUT-0017",
        "runtime.security.cap_add",
        "supported-exact",
        ALL_REVIEWED_TARGETS,
        "SecuritySettings::cap_add",
        RUNTIME_RENDER_POSITIVE,
        RUNTIME_RENDER_NEGATIVE
    ),
    output!(
        "PLN-OUT-0018",
        "runtime.security.cap_drop",
        "supported-exact",
        ALL_REVIEWED_TARGETS,
        "SecuritySettings::cap_drop",
        RUNTIME_RENDER_POSITIVE,
        RUNTIME_RENDER_NEGATIVE
    ),
    output!(
        "PLN-OUT-0019",
        "runtime.security.no_new_privileges",
        "supported-exact",
        ALL_REVIEWED_TARGETS,
        "SecuritySettings::no_new_privileges",
        RUNTIME_RENDER_POSITIVE,
        RUNTIME_RENDER_NEGATIVE
    ),
    output!(
        "PLN-OUT-0020",
        "runtime.security.read_only_filesystem",
        "supported-exact",
        ALL_REVIEWED_TARGETS,
        "SecuritySettings::read_only_filesystem",
        RUNTIME_RENDER_POSITIVE,
        RUNTIME_RENDER_NEGATIVE
    ),
    output!(
        "PLN-OUT-0021",
        "runtime.security.read_write_tmpfs",
        "supported-exact",
        ALL_REVIEWED_TARGETS,
        "SecuritySettings::read_write_tmpfs",
        RUNTIME_RENDER_POSITIVE,
        RUNTIME_RENDER_NEGATIVE
    ),
    output!(
        "PLN-OUT-0022",
        "runtime.namespaces.pid",
        "supported-exact",
        ALL_REVIEWED_TARGETS,
        "ContainerNamespaceSettings::pid",
        RUNTIME_RENDER_POSITIVE,
        RUNTIME_RENDER_NEGATIVE
    ),
    output!(
        "PLN-OUT-0023",
        "runtime.namespaces.ipc",
        "supported-exact",
        ALL_REVIEWED_TARGETS,
        "ContainerNamespaceSettings::ipc",
        RUNTIME_RENDER_POSITIVE,
        RUNTIME_RENDER_NEGATIVE
    ),
    output!(
        "PLN-OUT-0024",
        "runtime.namespaces.uts",
        "supported-exact",
        ALL_REVIEWED_TARGETS,
        "ContainerNamespaceSettings::uts",
        RUNTIME_RENDER_POSITIVE,
        RUNTIME_RENDER_NEGATIVE
    ),
    output!(
        "PLN-OUT-0025",
        "runtime.namespaces.cgroup",
        "supported-exact",
        ALL_REVIEWED_TARGETS,
        "ContainerNamespaceSettings::cgroup",
        RUNTIME_RENDER_POSITIVE,
        RUNTIME_RENDER_NEGATIVE
    ),
    output!(
        "PLN-OUT-0026",
        "runtime.resources.cpu_shares",
        "supported-exact",
        ALL_REVIEWED_TARGETS,
        "ContainerResourceControls::cpu_shares",
        RUNTIME_RENDER_POSITIVE,
        RUNTIME_RENDER_NEGATIVE
    ),
    output!(
        "PLN-OUT-0027",
        "runtime.resources.cpu_period",
        "supported-exact",
        ALL_REVIEWED_TARGETS,
        "ContainerResourceControls::cpu_period",
        RUNTIME_RENDER_POSITIVE,
        RUNTIME_RENDER_NEGATIVE
    ),
    output!(
        "PLN-OUT-0028",
        "runtime.resources.cpu_quota",
        "supported-exact",
        ALL_REVIEWED_TARGETS,
        "ContainerResourceControls::cpu_quota",
        "tests::runtime::finite_bounded_runtime_fields_render_for_every_reviewed_target",
        "tests::runtime::cpu_quota_accepts_only_exact_positive_millisecond_values"
    ),
    output!(
        "PLN-OUT-0029",
        "runtime.resources.memory_bytes",
        "supported-exact",
        ALL_REVIEWED_TARGETS,
        "ContainerResourceControls::memory_bytes",
        RUNTIME_RENDER_POSITIVE,
        RUNTIME_RENDER_NEGATIVE
    ),
    output!(
        "PLN-OUT-0030",
        "runtime.resources.pids",
        "supported-exact",
        ALL_REVIEWED_TARGETS,
        "ContainerResourceControls::pids",
        RUNTIME_RENDER_POSITIVE,
        RUNTIME_RENDER_NEGATIVE
    ),
    output!(
        "PLN-OUT-0031",
        "runtime.resources.rlimits.finite",
        "supported-exact",
        ALL_REVIEWED_TARGETS,
        "ContainerResourceControls::rlimits",
        RUNTIME_RENDER_POSITIVE,
        RUNTIME_RENDER_NEGATIVE
    ),
    output!(
        "PLN-OUT-0032",
        "runtime.resources.rlimits.unlimited",
        "target-gated",
        UNLIMITED_RLIMIT_TARGETS,
        "ContainerResourceControls::rlimits",
        "tests::runtime::unlimited_rlimits_are_supported_from_podman_five_six_in_every_reviewed_target",
        "tests::runtime::unlimited_rlimits_are_supported_from_podman_five_six_in_every_reviewed_target"
    ),
    output!(manual "PLN-OUT-0033", "runtime.health.command.sensitive", "HealthCommand"),
    output!(manual "PLN-OUT-0034", "runtime.startup_health.command.sensitive", "HealthCommand"),
    b4_output!(
        "PLN-OUT-0035",
        "container",
        "mount.named_volume.copy",
        "supported-exact",
        ALL_REVIEWED_TARGETS,
        "deployment::validate_mounts",
        "render::append_mount_arguments",
        "render::mount_json",
        "MountIntent::NamedVolume",
        "tests::render::b4_bind_tmpfs_ordinary_volume_and_secret_grants_are_exact_on_all_reviewed_targets",
        "tests::render::b4_version_and_portability_boundaries_block_the_complete_resource_artifact"
    ),
    b4_output!(
        "PLN-OUT-0036",
        "container",
        "mount.named_volume.copy.subpath",
        "supported-exact",
        ALL_REVIEWED_TARGETS,
        "deployment::validate_mounts",
        "render::append_mount_arguments",
        "render::mount_json",
        "NamedVolumeMount::set_subpath",
        "tests::render::b4_mounts_secrets_and_volume_ownership_are_exact_on_every_supported_target",
        "tests::deployment::b4_typed_mounts_volume_ownership_and_secret_grants_preserve_all_optional_states"
    ),
    b4_output!(
        "PLN-OUT-0037",
        "container",
        "mount.named_volume.nocopy",
        "supported-exact",
        ALL_REVIEWED_TARGETS,
        "deployment::validate_mounts",
        "render::append_mount_arguments",
        "render::mount_json",
        "MountIntent::NamedVolume",
        "tests::render::b4_bind_tmpfs_ordinary_volume_and_secret_grants_are_exact_on_all_reviewed_targets",
        "tests::deployment::b4_typed_mounts_volume_ownership_and_secret_grants_preserve_all_optional_states"
    ),
    b4_output!(manual
        "PLN-OUT-0038", "container", "mount.named_volume.nocopy.subpath", "deployment::validate_mounts",
        "render::unsupported_fields", "render::mount_json", "NamedVolumeMount::set_subpath",
        "tests::render::b4_version_and_portability_boundaries_block_the_complete_resource_artifact",
        "tests::deployment::b4_typed_mounts_volume_ownership_and_secret_grants_preserve_all_optional_states"
    ),
    b4_output!(
        "PLN-OUT-0039",
        "container",
        "mount.bind",
        "supported-exact",
        ALL_REVIEWED_TARGETS,
        "deployment::validate_mounts",
        "render::append_mount_arguments",
        "render::native_mount_json",
        "MountIntent::Bind",
        "tests::render::b4_bind_tmpfs_ordinary_volume_and_secret_grants_are_exact_on_all_reviewed_targets",
        "tests::render::b4_version_and_portability_boundaries_block_the_complete_resource_artifact"
    ),
    b4_output!(
        "PLN-OUT-0040",
        "container",
        "mount.tmpfs",
        "supported-exact",
        ALL_REVIEWED_TARGETS,
        "deployment::validate_mounts",
        "render::append_mount_arguments",
        "render::native_mount_json",
        "MountIntent::Tmpfs",
        "tests::render::b4_bind_tmpfs_ordinary_volume_and_secret_grants_are_exact_on_all_reviewed_targets",
        "tests::render::b4_version_and_portability_boundaries_block_the_complete_resource_artifact"
    ),
    b4_output!(
        "PLN-OUT-0041",
        "container",
        "secret_grant.mount",
        "supported-exact",
        ALL_REVIEWED_TARGETS,
        "deployment::validate_secret_grants",
        "render::append_secret_grants_arguments",
        "render::append_secret_grants_json",
        "SecretGrant::Mount",
        "tests::render::b4_secret_mount_default_and_explicit_modes_are_exact_on_all_reviewed_targets",
        "tests::render::b4_version_and_portability_boundaries_block_the_complete_resource_artifact"
    ),
    b4_output!(
        "PLN-OUT-0042",
        "container",
        "secret_grant.environment",
        "supported-exact",
        ALL_REVIEWED_TARGETS,
        "deployment::validate_secret_grants",
        "render::append_secret_grants_arguments",
        "render::append_secret_grants_json",
        "SecretGrant::Environment",
        "tests::render::renderer_renders_typed_secret_grants_without_exposing_secret_material",
        "tests::render::b4_version_and_portability_boundaries_block_the_complete_resource_artifact"
    ),
    b4_output!(
        "PLN-OUT-0043",
        "volume",
        "ownership.uid",
        "target-gated",
        B4_VERSIONED_TARGETS,
        "deployment::validate_volume_ownership",
        "render::volume_create_cli_arguments",
        "render::volume_create_json",
        "VolumeIntent::uid",
        "tests::render::b4_mounts_secrets_and_volume_ownership_are_exact_on_every_supported_target",
        "tests::render::b4_version_and_portability_boundaries_block_the_complete_resource_artifact"
    ),
    b4_output!(
        "PLN-OUT-0044",
        "volume",
        "ownership.gid",
        "target-gated",
        B4_VERSIONED_TARGETS,
        "deployment::validate_volume_ownership",
        "render::volume_create_cli_arguments",
        "render::volume_create_json",
        "VolumeIntent::gid",
        "tests::render::b4_mounts_secrets_and_volume_ownership_are_exact_on_every_supported_target",
        "tests::render::b4_version_and_portability_boundaries_block_the_complete_resource_artifact"
    ),
    b4_output!(
        "PLN-OUT-0045",
        "image",
        "pull_policy.always",
        "target-gated",
        B4_VERSIONED_TARGETS,
        "deployment::validate_image_policy",
        "render::render_operation",
        "render::render_operation",
        "ImagePullPolicy::Always",
        "tests::render::b4_image_pull_policies_are_exact_on_every_supported_target",
        "tests::render::b4_image_pull_policies_are_exact_on_every_supported_target"
    ),
    b4_output!(
        "PLN-OUT-0046",
        "image",
        "pull_policy.missing",
        "target-gated",
        B4_VERSIONED_TARGETS,
        "deployment::validate_image_policy",
        "render::render_operation",
        "render::render_operation",
        "ImagePullPolicy::Missing",
        "tests::render::b4_image_pull_policies_are_exact_on_every_supported_target",
        "tests::render::b4_image_pull_policies_are_exact_on_every_supported_target"
    ),
    b4_output!(
        "PLN-OUT-0047",
        "image",
        "pull_policy.never",
        "target-gated",
        B4_VERSIONED_TARGETS,
        "deployment::validate_image_policy",
        "render::render_operation",
        "render::render_operation",
        "ImagePullPolicy::Never",
        "tests::render::b4_image_pull_policies_are_exact_on_every_supported_target",
        "tests::render::b4_image_pull_policies_are_exact_on_every_supported_target"
    ),
    b4_output!(
        "PLN-OUT-0048",
        "image",
        "pull_policy.newer",
        "target-gated",
        B4_VERSIONED_TARGETS,
        "deployment::validate_image_policy",
        "render::render_operation",
        "render::render_operation",
        "ImagePullPolicy::Newer",
        "tests::render::b4_image_pull_policies_are_exact_on_every_supported_target",
        "tests::render::b4_image_pull_policies_are_exact_on_every_supported_target"
    ),
    b4_output!(manual
        "PLN-OUT-0049", "image", "source.portability", "deployment::validate_image_source", "render::unsupported_fields",
        "render::render_operation", "ImageSource::classification",
        "tests::render::b4_image_portability_manual_boundaries_block_the_complete_artifact",
        "tests::deployment::image_source_classification_requires_explicit_policy_and_preserves_manual_boundaries"
    ),
    b4_output!(manual
        "PLN-OUT-0050", "pod", "infra_mounts", "deployment::validate_mounts", "render::unsupported_fields",
        "render::render_operation", "PodIntent::infra_mounts",
        "tests::render::b4_pod_infra_mounts_block_without_a_partial_artifact",
        "tests::deployment::infra_container_mounts_support_managed_external_and_duplicate_boundaries"
    ),
];

/// One strict coverage row linking an observation or declared output field to its contract.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct NativeFieldCoverageEntry {
    id: String,
    plane: NativeFieldCoveragePlane,
    resource_kind: String,
    field_path: String,
    classification: NativeFieldCoverageClassification,
    observation: String,
    planner: String,
    cli_renderer: String,
    libpod_renderer: String,
    target_versions: Vec<String>,
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

    /// Returns whether this row covers decoded input or caller-declared output intent.
    #[must_use]
    pub const fn plane(&self) -> NativeFieldCoveragePlane {
        self.plane
    }

    /// Returns the native resource kind covered by this row, or `all` for a global boundary.
    #[must_use]
    pub fn resource_kind(&self) -> &str {
        &self.resource_kind
    }

    /// Returns the native or semantic field path, including a documented wildcard where appropriate.
    #[must_use]
    pub fn field_path(&self) -> &str {
        &self.field_path
    }

    /// Returns the declared coverage outcome.
    #[must_use]
    pub const fn classification(&self) -> NativeFieldCoverageClassification {
        self.classification
    }

    /// Returns the input-observation owner, or `not_applicable` for output-only intent.
    #[must_use]
    pub fn observation(&self) -> &str {
        &self.observation
    }

    /// Returns the planner ownership reference, or `not_applicable` for observation-only input.
    #[must_use]
    pub fn planner(&self) -> &str {
        &self.planner
    }

    /// Returns the exact CLI-renderer owner, or `not_applicable` for observation-only input.
    #[must_use]
    pub fn cli_renderer(&self) -> &str {
        &self.cli_renderer
    }

    /// Returns the exact Libpod-renderer owner, or `not_applicable` for observation-only input.
    #[must_use]
    pub fn libpod_renderer(&self) -> &str {
        &self.libpod_renderer
    }

    /// Returns reviewed targets to which this row applies.
    #[must_use]
    pub fn target_versions(&self) -> &[String] {
        &self.target_versions
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

/// Returns the strict, embedded native-observation and output-intent coverage ledger.
///
/// # Errors
///
/// Returns `PLN0047` when the packaged catalogue is malformed, incomplete, or internally
/// inconsistent with the input or output boundary it claims to cover.
pub fn native_field_coverage_catalogue() -> PodmanLensResult<Vec<NativeFieldCoverageEntry>> {
    parse_native_field_coverage_catalogue(COVERAGE_CATALOGUE_JSON)
}

fn parse_native_field_coverage_catalogue(source: &str) -> PodmanLensResult<Vec<NativeFieldCoverageEntry>> {
    let catalogue: CoverageCatalogue =
        serde_json::from_str(source).map_err(|_| Diagnostic::new(DiagnosticCode::NativeFieldCoverageUnavailable))?;
    if catalogue.schema_version != 3
        || catalogue.scope != "m2-input-observation-and-m6-b3-b4-output-intent"
        || !valid_entries(&catalogue.entries)
    {
        return Err(Diagnostic::new(DiagnosticCode::NativeFieldCoverageUnavailable));
    }
    Ok(catalogue.entries)
}

fn valid_entries(entries: &[NativeFieldCoverageEntry]) -> bool {
    entries.len() == EXPECTED_INPUT_ENTRIES.len() + EXPECTED_OUTPUT_ENTRIES.len()
        && entries
            .iter()
            .take(EXPECTED_INPUT_ENTRIES.len())
            .zip(EXPECTED_INPUT_ENTRIES)
            .all(|(entry, expected)| {
                entry.plane == NativeFieldCoveragePlane::InputObservation
                    && entry.id == expected.id
                    && entry.resource_kind == expected.resource_kind
                    && entry.field_path == expected.native_path
                    && entry.classification.as_str() == expected.classification
                    && entry.observation == expected.decoder
                    && entry.planner == expected.planner
                    && entry.cli_renderer == expected.renderer
                    && entry.libpod_renderer == expected.renderer
                    && entry.target_versions.is_empty()
                    && entry.public_contract == expected.public_contract
                    && entry.finding == expected.finding
                    && entry.positive_test == expected.positive_test
                    && entry.negative_test == expected.negative_test
                    && valid_reference(&entry.observation, "inventory::")
                    && valid_reference(&entry.planner, "")
                    && valid_reference(&entry.cli_renderer, "")
                    && valid_reference(&entry.libpod_renderer, "")
                    && valid_field_path(&entry.field_path, "$")
                    && valid_semantic_links(entry)
            })
        && entries
            .iter()
            .skip(EXPECTED_INPUT_ENTRIES.len())
            .zip(EXPECTED_OUTPUT_ENTRIES)
            .all(|(entry, expected)| {
                entry.plane == NativeFieldCoveragePlane::OutputIntent
                    && entry.id == expected.id
                    && entry.resource_kind == expected.resource_kind
                    && entry.field_path == expected.field_path
                    && entry.classification.as_str() == expected.classification
                    && entry.observation == "not_applicable"
                    && entry.planner == expected.planner
                    && entry.cli_renderer == expected.cli_renderer
                    && entry.libpod_renderer == expected.libpod_renderer
                    && entry
                        .target_versions
                        .iter()
                        .map(String::as_str)
                        .eq(expected.target_versions.iter().copied())
                    && entry.public_contract == expected.public_contract
                    && entry.finding == expected.finding
                    && entry.positive_test == expected.positive_test
                    && entry.negative_test == expected.negative_test
                    && valid_field_path(
                        &entry.field_path,
                        match expected.resource_kind {
                            "container" => {
                                if expected.field_path.starts_with("runtime.") {
                                    "runtime."
                                } else if expected.field_path.starts_with("mount.") {
                                    "mount."
                                } else {
                                    "secret_grant."
                                }
                            }
                            "volume" => "ownership.",
                            "image" => {
                                if expected.field_path.starts_with("pull_policy.") {
                                    "pull_policy."
                                } else {
                                    "source."
                                }
                            }
                            "pod" => "infra_mounts",
                            _ => return false,
                        },
                    )
                    && valid_semantic_links(entry)
            })
}

fn valid_semantic_links(entry: &NativeFieldCoverageEntry) -> bool {
    valid_reference(&entry.public_contract, "")
        && valid_diagnostic(&entry.finding)
        && valid_reference(&entry.positive_test, "tests::")
        && valid_reference(&entry.negative_test, "tests::")
        && entry
            .target_versions
            .iter()
            .all(|version| ALL_REVIEWED_TARGETS.contains(&version.as_str()))
}

fn valid_field_path(value: &str, required_prefix: &str) -> bool {
    value.starts_with(required_prefix)
        && value.len() <= 160
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'$' | b'.' | b'_' | b'<' | b'>' | b'*'))
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
            ("\"schema_version\": 3", "\"schema_version\": 1"),
            ("\"PLN-FLD-0037\"", "\"PLN-FLD-9999\""),
            ("\"observation\"", "\"unknown_observation\""),
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
            ("observation", "inventory::decode_pod"),
            ("planner", "deployment::plan"),
            ("cli_renderer", "render::deployment"),
            ("libpod_renderer", "render::deployment"),
            ("public_contract", "ObservationHeader::findings"),
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

    #[test]
    fn plausible_target_availability_swaps_are_rejected() -> Result<(), Box<dyn std::error::Error>> {
        let mut catalogue: serde_json::Value = serde_json::from_str(COVERAGE_CATALOGUE_JSON)?;
        let entry_index = |id| -> Result<usize, String> {
            catalogue["entries"]
                .as_array()
                .and_then(|entries| entries.iter().position(|entry| entry["id"] == id))
                .ok_or_else(|| format!("the embedded ledger must contain {id}"))
        };
        let journald = entry_index("PLN-OUT-0015")?;
        let unlimited_rlimit = entry_index("PLN-OUT-0032")?;
        let journald_versions = catalogue["entries"][journald]["target_versions"].clone();
        catalogue["entries"][journald]["target_versions"] =
            catalogue["entries"][unlimited_rlimit]["target_versions"].clone();
        catalogue["entries"][unlimited_rlimit]["target_versions"] = journald_versions;
        assert!(parse_native_field_coverage_catalogue(&serde_json::to_string(&catalogue)?).is_err());
        Ok(())
    }
}
