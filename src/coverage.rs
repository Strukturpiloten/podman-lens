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

const ALL_REVIEWED_TARGETS: &[&str] = &["5.4.0", "5.5.0", "5.6.0", "5.7.0", "5.8.6", "6.0.0", "6.1.0"];
const UNLIMITED_RLIMIT_TARGETS: &[&str] = &["5.6.0", "5.7.0", "5.8.6", "6.0.0", "6.1.0"];
const JOURNALD_LABEL_TARGETS: &[&str] = &["6.0.0", "6.1.0"];

struct ExpectedOutputEntry {
    id: &'static str,
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
    if catalogue.schema_version != 2
        || catalogue.scope != "m2-input-observation-and-m6-b3-output-intent"
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
                    && entry.resource_kind == "container"
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
                    && valid_field_path(&entry.field_path, "runtime.")
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
            ("\"schema_version\": 2", "\"schema_version\": 1"),
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

    #[test]
    fn plausible_target_availability_swaps_are_rejected() -> Result<(), serde_json::Error> {
        let mut catalogue: serde_json::Value = serde_json::from_str(COVERAGE_CATALOGUE_JSON)?;
        let first_output = 38;
        let journald = first_output + 14;
        let unlimited_rlimit = first_output + 31;
        let journald_versions = catalogue["entries"][journald]["target_versions"].clone();
        catalogue["entries"][journald]["target_versions"] =
            catalogue["entries"][unlimited_rlimit]["target_versions"].clone();
        catalogue["entries"][unlimited_rlimit]["target_versions"] = journald_versions;
        assert!(parse_native_field_coverage_catalogue(&serde_json::to_string(&catalogue)?).is_err());
        Ok(())
    }
}
