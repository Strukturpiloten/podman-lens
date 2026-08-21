//! Deterministic, evidence-backed grouping over a read-only native inventory.

use std::collections::{BTreeMap, BTreeSet};

use crate::{
    Diagnostic, DiagnosticCode, InventorySection, ObservationField, ResourceIdentity, ResourceInventory, ResourceKind,
    ResourceObservation,
};

const DOCKER_PROJECT: &str = "com.docker.compose.project";
const DOCKER_SERVICE: &str = "com.docker.compose.service";
const PODMAN_PROJECT: &str = "io.podman.compose.project";
const PODMAN_SERVICE: &str = "io.podman.compose.service";
const DOCKER_CONFIG_HASH: &str = "com.docker.compose.config-hash";
const PODMAN_CONFIG_HASH: &str = "io.podman.compose.config-hash";

/// One exact resource selector for discovery.
#[derive(Clone, Eq, Ord, PartialEq, PartialOrd)]
pub struct ResourceSelector {
    kind: ResourceKind,
    reference: String,
}
impl std::fmt::Debug for ResourceSelector {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ResourceSelector")
            .field("kind", &self.kind)
            .field("reference", &self.reference)
            .finish()
    }
}

impl ResourceSelector {
    /// Creates an exact, non-empty native resource selector.
    ///
    /// Wildcards are deliberately unsupported: callers must name one concrete resource.
    ///
    /// # Errors
    ///
    /// Returns `PLN0027` when the reference is empty, contains whitespace, or has wildcard syntax.
    pub fn exact(kind: ResourceKind, reference: impl Into<String>) -> Result<Self, Diagnostic> {
        let reference = reference.into();
        if !is_exact_reference(&reference) {
            return Err(Diagnostic::new(DiagnosticCode::InvalidDiscoveryRequest));
        }
        Ok(Self { kind, reference })
    }

    /// Returns the selected resource kind.
    #[must_use]
    pub const fn kind(&self) -> ResourceKind {
        self.kind
    }

    /// Returns the exact native ID, name, or image alias requested by the caller.
    #[must_use]
    pub fn reference(&self) -> &str {
        &self.reference
    }
}

/// Explicit inputs controlling resource discovery.
#[derive(Clone, Default, Eq, PartialEq)]
pub struct DiscoveryRequest {
    roots: BTreeSet<ResourceSelector>,
    all: bool,
    network_boundary_overrides: BTreeSet<String>,
    label_selectors: BTreeSet<LabelSelector>,
}
impl std::fmt::Debug for DiscoveryRequest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DiscoveryRequest")
            .field("root_count", &self.roots.len())
            .field("all", &self.all)
            .field(
                "network_boundary_override_count",
                &self.network_boundary_overrides.len(),
            )
            .field("label_selector_count", &self.label_selectors.len())
            .finish()
    }
}

impl DiscoveryRequest {
    /// Creates a request with no roots. A caller must add a root or select all eligible roots.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            roots: BTreeSet::new(),
            all: false,
            network_boundary_overrides: BTreeSet::new(),
            label_selectors: BTreeSet::new(),
        }
    }

    /// Adds one exact selector root.
    pub fn add_root(&mut self, selector: ResourceSelector) {
        self.roots.insert(selector);
    }

    /// Selects every eligible application root in the inventory.
    pub fn select_all(&mut self) {
        self.all = true;
    }

    /// Authorizes crossing one exact named-or-ID network boundary to its direct consumers.
    ///
    /// Only a concrete network name or ID is accepted. Wildcards and label expressions are not
    /// boundary overrides because they would make the selected crossing ambiguous.
    ///
    /// # Errors
    ///
    /// Returns `PLN0027` when the identifier is not exact.
    pub fn add_network_boundary_override(&mut self, identifier: impl Into<String>) -> Result<(), Diagnostic> {
        let identifier = identifier.into();
        if !is_exact_reference(&identifier) {
            return Err(Diagnostic::new(DiagnosticCode::InvalidDiscoveryRequest));
        }
        self.network_boundary_overrides.insert(identifier);
        Ok(())
    }

    /// Adds one validated label selector as a discovery root.
    pub fn add_label_root(&mut self, selector: LabelSelector) {
        self.label_selectors.insert(selector);
    }

    /// Returns explicit selector roots in deterministic order.
    #[must_use]
    pub fn roots(&self) -> impl ExactSizeIterator<Item = &ResourceSelector> {
        self.roots.iter()
    }

    /// Returns label selector roots in deterministic order.
    #[must_use]
    pub fn label_roots(&self) -> impl ExactSizeIterator<Item = &LabelSelector> {
        self.label_selectors.iter()
    }

    /// Returns whether all eligible roots are selected.
    #[must_use]
    pub const fn all(&self) -> bool {
        self.all
    }

    /// Returns exact named network-boundary overrides in deterministic order.
    #[must_use]
    pub fn network_boundary_overrides(&self) -> impl ExactSizeIterator<Item = &str> {
        self.network_boundary_overrides.iter().map(String::as_str)
    }
}

/// A validated label-root selector with presence or exact-value semantics.
#[derive(Clone, Eq, Ord, PartialEq, PartialOrd)]
pub struct LabelSelector {
    name: String,
    value: Option<String>,
}
impl std::fmt::Debug for LabelSelector {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LabelSelector")
            .field("has_exact_value", &self.value.is_some())
            .finish_non_exhaustive()
    }
}
impl LabelSelector {
    /// Selects every resource carrying the exact label name, regardless of its value.
    ///
    /// # Errors
    ///
    /// Returns `PLN0027` when the name is empty, contains whitespace, or uses wildcard syntax.
    pub fn presence(name: impl Into<String>) -> Result<Self, Diagnostic> {
        Self::new(name.into(), None)
    }

    /// Selects every resource whose exact label name has the exact supplied value.
    ///
    /// Empty values are valid label values and remain distinct from presence-only selection.
    ///
    /// # Errors
    ///
    /// Returns `PLN0027` when the name is empty, contains whitespace, or uses wildcard syntax.
    pub fn exact(name: impl Into<String>, value: impl Into<String>) -> Result<Self, Diagnostic> {
        Self::new(name.into(), Some(value.into()))
    }

    fn new(name: String, value: Option<String>) -> Result<Self, Diagnostic> {
        if !is_exact_reference(&name) {
            return Err(Diagnostic::new(DiagnosticCode::InvalidDiscoveryRequest));
        }
        Ok(Self { name, value })
    }
    /// Returns the exact label key.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }
    /// Returns the requested exact value, or `None` for presence.
    #[must_use]
    pub fn value(&self) -> Option<&str> {
        self.value.as_deref()
    }
}

/// The source of an edge in the dependency graph.
#[derive(Clone, Eq, PartialEq)]
#[non_exhaustive]
pub enum DependencyEvidence {
    /// A concrete native relationship from an inspect response.
    NativeRelationship {
        /// Every source field that jointly asserted this relationship.
        field_paths: Vec<String>,
    },
}
impl std::fmt::Debug for DependencyEvidence {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NativeRelationship { field_paths } => f
                .debug_struct("NativeRelationship")
                .field("field_paths", field_paths)
                .finish(),
        }
    }
}

/// A directed dependent-to-prerequisite edge.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResourceDependency {
    dependent: ResourceIdentity,
    prerequisite: ResourceIdentity,
    evidence: DependencyEvidence,
}

impl ResourceDependency {
    /// Returns the dependent resource.
    #[must_use]
    pub fn dependent(&self) -> &ResourceIdentity {
        &self.dependent
    }

    /// Returns the prerequisite resource.
    #[must_use]
    pub fn prerequisite(&self) -> &ResourceIdentity {
        &self.prerequisite
    }

    /// Returns native evidence for the directed edge.
    #[must_use]
    pub fn evidence(&self) -> &DependencyEvidence {
        &self.evidence
    }
}

/// Evidence that joins two non-shared resources into one group.
#[derive(Clone, Eq, Ord, PartialEq, PartialOrd)]
#[non_exhaustive]
pub enum GroupingEvidence {
    /// Pod/container membership is native, strong grouping evidence.
    PodMembership,
    /// A container's native dependency keeps the dependent and prerequisite together.
    ContainerDependency,
    /// Matching complete Docker and Podman Compose labels are advisory evidence.
    ComposeOwnership {
        /// Agreed non-empty Compose project name.
        project: String,
    },
}
impl std::fmt::Debug for GroupingEvidence {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::PodMembership => f.write_str("PodMembership"),
            Self::ContainerDependency => f.write_str("ContainerDependency"),
            Self::ComposeOwnership { .. } => f.write_str("ComposeOwnership([redacted])"),
        }
    }
}

/// A non-directed grouping edge, kept separate from deployment dependency edges.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GroupingEdge {
    left: ResourceIdentity,
    right: ResourceIdentity,
    evidence: GroupingEvidence,
    field_paths: Vec<String>,
}

impl GroupingEdge {
    /// Returns one grouped resource.
    #[must_use]
    pub fn left(&self) -> &ResourceIdentity {
        &self.left
    }

    /// Returns the other grouped resource.
    #[must_use]
    pub fn right(&self) -> &ResourceIdentity {
        &self.right
    }

    /// Returns evidence for grouping, distinct from a dependency ordering edge.
    #[must_use]
    pub fn evidence(&self) -> &GroupingEvidence {
        &self.evidence
    }

    /// Returns every native source path that supports this grouping edge.
    #[must_use]
    pub fn field_paths(&self) -> &[String] {
        &self.field_paths
    }
}

/// A structured, non-fatal discovery outcome.
#[derive(Clone, Eq, PartialEq)]
pub struct DiscoveryFinding {
    code: DiagnosticCode,
    resource: Option<ResourceIdentity>,
    selector: Option<ResourceSelector>,
    label_selector: Option<LabelSelector>,
    field_path: Option<String>,
}

impl std::fmt::Debug for DiscoveryFinding {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("DiscoveryFinding")
            .field("code", &self.code)
            .field("resource", &self.resource)
            .field("has_resource_selector", &self.selector.is_some())
            .field("has_label_selector", &self.label_selector.is_some())
            .field("field_path", &self.field_path)
            .finish()
    }
}

impl DiscoveryFinding {
    fn resource(code: DiagnosticCode, resource: ResourceIdentity) -> Self {
        Self {
            code,
            resource: Some(resource),
            selector: None,
            label_selector: None,
            field_path: None,
        }
    }

    fn for_selector(code: DiagnosticCode, selector: ResourceSelector) -> Self {
        Self {
            code,
            resource: None,
            selector: Some(selector),
            label_selector: None,
            field_path: None,
        }
    }

    fn for_label_selector(code: DiagnosticCode, selector: LabelSelector) -> Self {
        Self {
            code,
            resource: None,
            selector: None,
            label_selector: Some(selector),
            field_path: None,
        }
    }

    /// Returns the stable finding code.
    #[must_use]
    pub const fn code(&self) -> DiagnosticCode {
        self.code
    }

    /// Returns the implicated resource, where one was resolved.
    #[must_use]
    pub fn resource_identity(&self) -> Option<&ResourceIdentity> {
        self.resource.as_ref()
    }

    /// Returns the unresolved or ambiguous selector, where applicable.
    #[must_use]
    pub fn selector(&self) -> Option<&ResourceSelector> {
        self.selector.as_ref()
    }

    /// Returns the unresolved label selector, where applicable.
    #[must_use]
    pub fn label_selector(&self) -> Option<&LabelSelector> {
        self.label_selector.as_ref()
    }
    /// Returns the source relationship path where applicable.
    #[must_use]
    pub fn field_path(&self) -> Option<&str> {
        self.field_path.as_deref()
    }
}

/// A deterministic explanation for an inclusion, boundary, merge, or ordering decision.
#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum DiscoveryExplanationKind {
    /// Resource selected as a root.
    Root,
    /// Resource included in a group.
    IncludedMember,
    /// Resource retained as prerequisite.
    Prerequisite,
    /// Shared resource did not merge groups.
    StoppedSharedBoundary,
    /// Exact network override crossed to consumers.
    AuthorizedNetworkCrossing,
    /// An explicitly selected shared resource crossed to its consumers.
    AuthorizedSharedCrossing,
    /// Strong evidence merged resources.
    StrongEvidenceMerge,
    /// Stable group ordering decision.
    GroupOrdering,
}

/// Redacted provenance for one resolved discovery root.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
#[non_exhaustive]
pub enum DiscoveryRootOrigin {
    /// Position in [`ResourceGraph::requested_roots`].
    ResourceSelector {
        /// Zero-based selector position.
        position: usize,
    },
    /// Position in [`ResourceGraph::requested_label_roots`].
    LabelSelector {
        /// Zero-based selector position. The selected label value is never copied here.
        position: usize,
    },
    /// Root selected by [`DiscoveryRequest::select_all`].
    All,
}
/// One explanation associated with a resource.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DiscoveryExplanation {
    kind: DiscoveryExplanationKind,
    resource: ResourceIdentity,
    related: Option<ResourceIdentity>,
    position: Option<usize>,
    root_origin: Option<DiscoveryRootOrigin>,
}
impl DiscoveryExplanation {
    /// Returns the explanation category.
    #[must_use]
    pub const fn kind(&self) -> &DiscoveryExplanationKind {
        &self.kind
    }
    /// Returns the explained resource.
    #[must_use]
    pub fn resource(&self) -> &ResourceIdentity {
        &self.resource
    }
    /// Returns the related resource when relevant.
    #[must_use]
    pub fn related(&self) -> Option<&ResourceIdentity> {
        self.related.as_ref()
    }

    /// Returns the zero-based position for a group-ordering explanation.
    #[must_use]
    pub const fn position(&self) -> Option<usize> {
        self.position
    }

    /// Returns redacted selector provenance for a root explanation.
    #[must_use]
    pub const fn root_origin(&self) -> Option<&DiscoveryRootOrigin> {
        self.root_origin.as_ref()
    }
}

/// A deterministic resource group and its non-grouping prerequisites.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResourceGroup {
    id: ResourceIdentity,
    members: Vec<ResourceIdentity>,
    prerequisites: Vec<ResourceIdentity>,
}

impl ResourceGroup {
    /// Returns the smallest immutable `(kind, id)` member used as the stable group ID.
    #[must_use]
    pub fn id(&self) -> &ResourceIdentity {
        &self.id
    }

    /// Returns grouped application resources in deterministic order.
    #[must_use]
    pub fn members(&self) -> &[ResourceIdentity] {
        &self.members
    }

    /// Returns direct non-grouping prerequisites in deterministic order.
    #[must_use]
    pub fn prerequisites(&self) -> &[ResourceIdentity] {
        &self.prerequisites
    }
}

/// A deterministic native graph suitable for later planning without executing Podman.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResourceGraph {
    requested_roots: Vec<ResourceSelector>,
    requested_label_roots: Vec<LabelSelector>,
    all_requested: bool,
    resolved_roots: Vec<ResourceIdentity>,
    groups: Vec<ResourceGroup>,
    shared_prerequisites: Vec<ResourceIdentity>,
    dependencies: Vec<ResourceDependency>,
    grouping_edges: Vec<GroupingEdge>,
    findings: Vec<DiscoveryFinding>,
    explanations: Vec<DiscoveryExplanation>,
}

impl ResourceGraph {
    /// Returns exact requested resource roots.
    #[must_use]
    pub fn requested_roots(&self) -> &[ResourceSelector] {
        &self.requested_roots
    }
    /// Returns requested label roots.
    #[must_use]
    pub fn requested_label_roots(&self) -> &[LabelSelector] {
        &self.requested_label_roots
    }
    /// Returns whether `select_all` participated in root selection.
    #[must_use]
    pub const fn all_requested(&self) -> bool {
        self.all_requested
    }
    /// Returns resolved roots, including roots selected by `all`.
    #[must_use]
    pub fn resolved_roots(&self) -> &[ResourceIdentity] {
        &self.resolved_roots
    }
    /// Returns disjoint groups ordered by stable group ID.
    #[must_use]
    pub fn groups(&self) -> &[ResourceGroup] {
        &self.groups
    }

    /// Returns prerequisite resources referenced by more than one group.
    #[must_use]
    pub fn shared_prerequisites(&self) -> &[ResourceIdentity] {
        &self.shared_prerequisites
    }

    /// Returns directed dependent-to-prerequisite edges in deterministic order.
    #[must_use]
    pub fn dependencies(&self) -> &[ResourceDependency] {
        &self.dependencies
    }

    /// Returns non-directed grouping evidence separately from dependency ordering.
    #[must_use]
    pub fn grouping_edges(&self) -> &[GroupingEdge] {
        &self.grouping_edges
    }

    /// Returns structured selector and advisory-label findings.
    #[must_use]
    pub fn findings(&self) -> &[DiscoveryFinding] {
        &self.findings
    }
    /// Returns deterministic explanations for graph decisions.
    #[must_use]
    pub fn explanations(&self) -> &[DiscoveryExplanation] {
        &self.explanations
    }
}

/// Builds deterministic resource groups from an already acquired native inventory.
///
/// A dependency edge always points from dependent to prerequisite. Networks, volumes, images,
/// and secrets remain prerequisites and never join consumers merely because they are shared.
/// Reverse consumer traversal occurs only for an explicitly selected shared resource or an exact
/// network name-or-ID boundary override.
///
/// # Errors
///
/// Returns `PLN0027` when neither explicit roots nor `select_all` are present in the request.
#[allow(clippy::too_many_lines)] // The deterministic discovery phases intentionally remain adjacent.
pub fn discover(inventory: &ResourceInventory, request: &DiscoveryRequest) -> Result<ResourceGraph, Diagnostic> {
    if !request.all && request.roots.is_empty() && request.label_selectors.is_empty() {
        return Err(Diagnostic::new(DiagnosticCode::InvalidDiscoveryRequest));
    }

    let records = inventory
        .sections()
        .iter()
        .flat_map(InventorySection::observations)
        .map(|observation| (observation.header().identity().clone(), observation))
        .collect::<BTreeMap<_, _>>();
    let mut findings = advisory_label_findings(&records);
    let ownership = compose_ownership_index(&records);
    let dependencies = collect_dependencies(&records, &mut findings);
    let reverse = reverse_dependencies(&dependencies);
    let grouping_edges = grouping_edges(&records, &ownership, &dependencies, &mut findings);
    let grouping_adjacency = grouping_adjacency(&grouping_edges);

    let mut members = BTreeSet::new();
    let mut explicitly_selected_shared = BTreeSet::new();
    let mut resolved_roots = BTreeSet::new();
    let mut root_origins = BTreeSet::new();
    for (position, selector) in request.roots.iter().enumerate() {
        match resolve_selector(&records, selector) {
            SelectorResolution::One(identity) if is_shared_kind(identity.kind()) => {
                root_origins.insert((identity.clone(), DiscoveryRootOrigin::ResourceSelector { position }));
                resolved_roots.insert(identity.clone());
                explicitly_selected_shared.insert(identity);
            }
            SelectorResolution::One(identity) => {
                root_origins.insert((identity.clone(), DiscoveryRootOrigin::ResourceSelector { position }));
                resolved_roots.insert(identity.clone());
                members.insert(identity);
            }
            SelectorResolution::None => findings.push(DiscoveryFinding::for_selector(
                DiagnosticCode::SelectorUnresolved,
                selector.clone(),
            )),
            SelectorResolution::Many => findings.push(DiscoveryFinding::for_selector(
                DiagnosticCode::SelectorAmbiguous,
                selector.clone(),
            )),
        }
    }
    for (position, selector) in request.label_selectors.iter().enumerate() {
        let matched = records
            .iter()
            .filter(|(_, record)| {
                observed_labels(record)
                    .and_then(|labels| labels.get(&selector.name))
                    .is_some_and(|actual| selector.value.as_ref().is_none_or(|expected| actual == expected))
            })
            .map(|(identity, _)| identity.clone())
            .collect::<Vec<_>>();
        if matched.is_empty() {
            findings.push(DiscoveryFinding::for_label_selector(
                DiagnosticCode::SelectorUnresolved,
                selector.clone(),
            ));
        }
        for identity in matched {
            root_origins.insert((identity.clone(), DiscoveryRootOrigin::LabelSelector { position }));
            resolved_roots.insert(identity.clone());
            if is_shared_kind(identity.kind()) {
                explicitly_selected_shared.insert(identity);
            } else {
                members.insert(identity);
            }
        }
    }
    if request.all {
        for (identity, record) in &records {
            if eligible_all_root(identity, record, &reverse, ownership.get(identity)) {
                root_origins.insert((identity.clone(), DiscoveryRootOrigin::All));
                resolved_roots.insert(identity.clone());
                if is_shared_kind(identity.kind()) {
                    explicitly_selected_shared.insert(identity.clone());
                } else {
                    members.insert(identity.clone());
                }
            }
        }
    }

    let mut authorized_shared_crossings = BTreeSet::new();
    for shared in &explicitly_selected_shared {
        let consumers = reverse.get(shared).cloned().unwrap_or_default();
        if consumers.is_empty() {
            members.insert(shared.clone());
        } else {
            authorized_shared_crossings.insert(shared.clone());
            members.extend(consumers);
        }
    }

    let resolved_network_boundaries = resolve_network_boundaries(&records, request, &mut findings);
    let network_boundaries = resolved_network_boundaries.values().cloned().collect::<BTreeSet<_>>();
    let mut crossed_network_boundaries = BTreeSet::new();
    loop {
        let before = members.len();
        expand_group_members(&mut members, &grouping_adjacency);
        let prerequisite_networks = dependencies
            .iter()
            .filter(|edge| members.contains(&edge.dependent))
            .filter(|edge| edge.prerequisite.kind() == ResourceKind::Network)
            .map(|edge| edge.prerequisite.clone())
            .filter(|network| network_boundaries.contains(network))
            .collect::<BTreeSet<_>>();
        for network in prerequisite_networks {
            let before_crossing = members.len();
            members.extend(reverse.get(&network).cloned().unwrap_or_default());
            if members.len() > before_crossing {
                crossed_network_boundaries.insert(network);
            }
        }
        if members.len() == before {
            break;
        }
    }

    let mut groups = grouped_members(&members, &grouping_adjacency);
    for group in &mut groups {
        group.prerequisites = dependencies
            .iter()
            .filter(|edge| group.members.contains(&edge.dependent) && !group.members.contains(&edge.prerequisite))
            .map(|edge| edge.prerequisite.clone())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();
    }
    groups.sort_by(|left, right| left.id.cmp(&right.id));

    let mut prerequisite_count = BTreeMap::<ResourceIdentity, usize>::new();
    for group in &groups {
        for prerequisite in &group.prerequisites {
            *prerequisite_count.entry(prerequisite.clone()).or_default() += 1;
        }
    }
    let shared_prerequisites = prerequisite_count
        .into_iter()
        .filter_map(|(identity, count)| (count > 1).then_some(identity))
        .collect();
    let grouped = groups
        .iter()
        .flat_map(|group| group.members.iter())
        .collect::<BTreeSet<_>>();
    let dependencies = dependencies
        .into_iter()
        .filter(|edge| grouped.contains(&edge.dependent))
        .collect();
    let grouping_edges = grouping_edges
        .into_iter()
        .filter(|edge| grouped.contains(&edge.left) && grouped.contains(&edge.right))
        .collect::<Vec<_>>();
    let mut explanations = Vec::new();
    for (root, origin) in &root_origins {
        explanations.push(DiscoveryExplanation {
            kind: DiscoveryExplanationKind::Root,
            resource: root.clone(),
            related: None,
            position: None,
            root_origin: Some(origin.clone()),
        });
    }
    for (position, group) in groups.iter().enumerate() {
        explanations.push(DiscoveryExplanation {
            kind: DiscoveryExplanationKind::GroupOrdering,
            resource: group.id.clone(),
            related: None,
            position: Some(position),
            root_origin: None,
        });
        for member in &group.members {
            explanations.push(DiscoveryExplanation {
                kind: DiscoveryExplanationKind::IncludedMember,
                resource: member.clone(),
                related: Some(group.id.clone()),
                position: None,
                root_origin: None,
            });
        }
        for prerequisite in &group.prerequisites {
            explanations.push(DiscoveryExplanation {
                kind: DiscoveryExplanationKind::Prerequisite,
                resource: prerequisite.clone(),
                related: Some(group.id.clone()),
                position: None,
                root_origin: None,
            });
        }
    }
    for edge in &grouping_edges {
        explanations.push(DiscoveryExplanation {
            kind: DiscoveryExplanationKind::StrongEvidenceMerge,
            resource: edge.left.clone(),
            related: Some(edge.right.clone()),
            position: None,
            root_origin: None,
        });
    }
    for boundary in &authorized_shared_crossings {
        explanations.push(DiscoveryExplanation {
            kind: DiscoveryExplanationKind::AuthorizedSharedCrossing,
            resource: boundary.clone(),
            related: None,
            position: None,
            root_origin: None,
        });
    }
    for boundary in &crossed_network_boundaries {
        explanations.push(DiscoveryExplanation {
            kind: DiscoveryExplanationKind::AuthorizedNetworkCrossing,
            resource: boundary.clone(),
            related: None,
            position: None,
            root_origin: None,
        });
    }
    for group in &groups {
        for prerequisite in &group.prerequisites {
            let stops_other_consumers = is_shared_kind(prerequisite.kind())
                && reverse
                    .get(prerequisite)
                    .is_some_and(|consumers| consumers.iter().any(|consumer| !group.members.contains(consumer)));
            let authorized =
                explicitly_selected_shared.contains(prerequisite) || crossed_network_boundaries.contains(prerequisite);
            if stops_other_consumers && !authorized {
                explanations.push(DiscoveryExplanation {
                    kind: DiscoveryExplanationKind::StoppedSharedBoundary,
                    resource: prerequisite.clone(),
                    related: Some(group.id.clone()),
                    position: None,
                    root_origin: None,
                });
            }
        }
    }
    for (identifier, boundary) in &resolved_network_boundaries {
        if !crossed_network_boundaries.contains(boundary) {
            findings.push(DiscoveryFinding::for_selector(
                DiagnosticCode::BoundaryOverrideUnused,
                ResourceSelector {
                    kind: ResourceKind::Network,
                    reference: identifier.clone(),
                },
            ));
        }
    }
    findings.sort_by(finding_order);
    Ok(ResourceGraph {
        requested_roots: request.roots.iter().cloned().collect(),
        requested_label_roots: request.label_selectors.iter().cloned().collect(),
        all_requested: request.all,
        resolved_roots: resolved_roots.into_iter().collect(),
        groups,
        shared_prerequisites,
        dependencies,
        grouping_edges,
        findings,
        explanations,
    })
}

fn is_exact_reference(value: &str) -> bool {
    !value.is_empty() && !value.chars().any(char::is_whitespace) && !value.contains(['*', '?', '[', ']'])
}

enum SelectorResolution {
    None,
    One(ResourceIdentity),
    Many,
}

fn relationship_state_blocks_discovery(
    relationships: &ObservationField<Vec<crate::observation::NativeRelationship>>,
) -> bool {
    matches!(
        relationships,
        ObservationField::Unavailable
            | ObservationField::Malformed
            | ObservationField::VersionInapplicable
            | ObservationField::NotApplicable
            | ObservationField::Unmodelled(_)
    )
}

fn observed_relationships(record: &ResourceObservation) -> Option<&[crate::observation::NativeRelationship]> {
    match record.relationships()? {
        ObservationField::Observed(value) => Some(value.value()),
        _ => None,
    }
}

fn observed_image_alias_matches(record: &ResourceObservation, reference: &str) -> bool {
    [record.image_repo_tags(), record.image_repo_digests()]
        .into_iter()
        .flatten()
        .any(|field| {
            field
                .observed()
                .is_some_and(|aliases| aliases.value().iter().any(|alias| alias == reference))
        })
}

fn observed_labels(record: &ResourceObservation) -> Option<&crate::Labels> {
    match record.labels() {
        ObservationField::Observed(value) => Some(value.value()),
        _ => None,
    }
}

fn is_eligible_unpodded_container(record: &ResourceObservation) -> bool {
    match record.details() {
        crate::ResourceDetails::Container(container) => match container.infra() {
            ObservationField::Absent => true,
            ObservationField::Observed(value) => !*value.value(),
            ObservationField::Unavailable
            | ObservationField::Malformed
            | ObservationField::VersionInapplicable
            | ObservationField::NotApplicable
            | ObservationField::Unmodelled(_) => false,
        },
        _ => false,
    }
}

fn resolve_selector(
    records: &BTreeMap<ResourceIdentity, &ResourceObservation>,
    selector: &ResourceSelector,
) -> SelectorResolution {
    let matched = records
        .iter()
        .filter(|(identity, record)| {
            identity.kind() == selector.kind
                && (identity.id() == selector.reference
                    || identity.name() == Some(selector.reference.as_str())
                    || (selector.kind == ResourceKind::Image
                        && observed_image_alias_matches(record, &selector.reference)))
        })
        .map(|(identity, _)| identity.clone())
        .collect::<Vec<_>>();
    match matched.as_slice() {
        [] => SelectorResolution::None,
        [identity] => SelectorResolution::One(identity.clone()),
        _ => SelectorResolution::Many,
    }
}

fn collect_dependencies(
    records: &BTreeMap<ResourceIdentity, &ResourceObservation>,
    findings: &mut Vec<DiscoveryFinding>,
) -> Vec<ResourceDependency> {
    let mut collected = BTreeMap::<(ResourceIdentity, ResourceIdentity), BTreeSet<String>>::new();
    for (identity, record) in records {
        let Some(relationships) = observed_relationships(record) else {
            if record.relationships().is_some_and(relationship_state_blocks_discovery) {
                findings.push(DiscoveryFinding::resource(
                    DiagnosticCode::RelationshipConflict,
                    identity.clone(),
                ));
            }
            continue;
        };
        for relationship in relationships {
            if identity.kind() == ResourceKind::Pod && relationship.kind == ResourceKind::Container {
                continue;
            }
            match resolve_relationship_reference(records, relationship) {
                RelationshipReferenceResolution::One(prerequisite) => {
                    collected
                        .entry((identity.clone(), prerequisite))
                        .or_default()
                        .extend(relationship.field_paths.iter().cloned());
                }
                RelationshipReferenceResolution::None => findings.extend(relationship.field_paths.iter().cloned().map(
                    |field_path| DiscoveryFinding {
                        code: DiagnosticCode::UnresolvedRelationship,
                        resource: Some(identity.clone()),
                        selector: None,
                        label_selector: None,
                        field_path: Some(field_path),
                    },
                )),
                RelationshipReferenceResolution::Many => findings.extend(relationship.field_paths.iter().cloned().map(
                    |field_path| DiscoveryFinding {
                        code: DiagnosticCode::RelationshipAmbiguous,
                        resource: Some(identity.clone()),
                        selector: None,
                        label_selector: None,
                        field_path: Some(field_path),
                    },
                )),
                RelationshipReferenceResolution::Conflict => findings.extend(
                    relationship
                        .field_paths
                        .iter()
                        .cloned()
                        .map(|field_path| DiscoveryFinding {
                            code: DiagnosticCode::RelationshipConflict,
                            resource: Some(identity.clone()),
                            selector: None,
                            label_selector: None,
                            field_path: Some(field_path),
                        }),
                ),
            }
        }
    }
    collected
        .into_iter()
        .map(|((dependent, prerequisite), field_paths)| ResourceDependency {
            dependent,
            prerequisite,
            evidence: DependencyEvidence::NativeRelationship {
                field_paths: field_paths.into_iter().collect(),
            },
        })
        .collect()
}

enum RelationshipReferenceResolution {
    One(ResourceIdentity),
    None,
    Many,
    Conflict,
}

fn resolve_relationship_reference(
    records: &BTreeMap<ResourceIdentity, &ResourceObservation>,
    relationship: &crate::observation::NativeRelationship,
) -> RelationshipReferenceResolution {
    let mut resolved = BTreeSet::new();
    for reference in &relationship.references {
        match resolve_reference_count(records, relationship.kind, reference) {
            SelectorResolution::One(identity) => {
                resolved.insert(identity);
            }
            SelectorResolution::None => return RelationshipReferenceResolution::None,
            SelectorResolution::Many => return RelationshipReferenceResolution::Many,
        }
    }
    match resolved.into_iter().collect::<Vec<_>>().as_slice() {
        [identity] => RelationshipReferenceResolution::One(identity.clone()),
        [] => RelationshipReferenceResolution::None,
        _ => RelationshipReferenceResolution::Conflict,
    }
}

fn resolve_reference_count(
    records: &BTreeMap<ResourceIdentity, &ResourceObservation>,
    kind: ResourceKind,
    reference: &str,
) -> SelectorResolution {
    let matched = records
        .iter()
        .filter(|(identity, record)| {
            identity.kind() == kind
                && (identity.id() == reference
                    || identity.name() == Some(reference)
                    || (kind == ResourceKind::Image && observed_image_alias_matches(record, reference)))
        })
        .map(|(identity, _)| identity.clone())
        .collect::<Vec<_>>();
    match matched.as_slice() {
        [] => SelectorResolution::None,
        [identity] => SelectorResolution::One(identity.clone()),
        _ => SelectorResolution::Many,
    }
}

fn reverse_dependencies(dependencies: &[ResourceDependency]) -> BTreeMap<ResourceIdentity, BTreeSet<ResourceIdentity>> {
    let mut reverse = BTreeMap::new();
    for edge in dependencies {
        reverse
            .entry(edge.prerequisite.clone())
            .or_insert_with(BTreeSet::new)
            .insert(edge.dependent.clone());
    }
    reverse
}

fn is_shared_kind(kind: ResourceKind) -> bool {
    matches!(
        kind,
        ResourceKind::Network | ResourceKind::Volume | ResourceKind::Image | ResourceKind::Secret
    )
}

fn eligible_all_root(
    identity: &ResourceIdentity,
    record: &ResourceObservation,
    reverse: &BTreeMap<ResourceIdentity, BTreeSet<ResourceIdentity>>,
    ownership: Option<&String>,
) -> bool {
    match identity.kind() {
        ResourceKind::Pod => true,
        ResourceKind::Container => {
            is_eligible_unpodded_container(record)
                && !observed_relationships(record).is_some_and(|relationships| {
                    relationships
                        .iter()
                        .any(|relationship| relationship.kind == ResourceKind::Pod)
                })
        }
        ResourceKind::Network | ResourceKind::Volume | ResourceKind::Secret => !reverse.contains_key(identity),
        ResourceKind::Image => ownership.is_some(),
    }
}

fn resolve_network_boundaries(
    records: &BTreeMap<ResourceIdentity, &ResourceObservation>,
    request: &DiscoveryRequest,
    findings: &mut Vec<DiscoveryFinding>,
) -> BTreeMap<String, ResourceIdentity> {
    let mut resolved = BTreeMap::new();
    for identifier in &request.network_boundary_overrides {
        let selector = ResourceSelector {
            kind: ResourceKind::Network,
            reference: identifier.clone(),
        };
        match resolve_selector(records, &selector) {
            SelectorResolution::One(identity) => {
                resolved.insert(identifier.clone(), identity);
            }
            SelectorResolution::None => findings.push(DiscoveryFinding::for_selector(
                DiagnosticCode::SelectorUnresolved,
                selector,
            )),
            SelectorResolution::Many => findings.push(DiscoveryFinding::for_selector(
                DiagnosticCode::SelectorAmbiguous,
                selector,
            )),
        }
    }
    resolved
}

fn grouping_edges(
    records: &BTreeMap<ResourceIdentity, &ResourceObservation>,
    ownership: &BTreeMap<ResourceIdentity, String>,
    dependencies: &[ResourceDependency],
    findings: &mut Vec<DiscoveryFinding>,
) -> Vec<GroupingEdge> {
    let mut edges = BTreeMap::<(ResourceIdentity, ResourceIdentity, GroupingEvidence), BTreeSet<String>>::new();
    for (identity, record) in records {
        let Some(relationships) = observed_relationships(record) else {
            continue;
        };
        for relationship in relationships {
            let membership = matches!(
                (identity.kind(), relationship.kind),
                (ResourceKind::Pod, ResourceKind::Container) | (ResourceKind::Container, ResourceKind::Pod)
            );
            if membership {
                match resolve_relationship_reference(records, relationship) {
                    RelationshipReferenceResolution::One(member) => insert_grouping_edge(
                        &mut edges,
                        identity,
                        &member,
                        GroupingEvidence::PodMembership,
                        relationship.field_paths.first().map(String::as_str),
                    ),
                    RelationshipReferenceResolution::None if identity.kind() == ResourceKind::Pod => {
                        findings.push(DiscoveryFinding {
                            code: DiagnosticCode::UnresolvedRelationship,
                            resource: Some(identity.clone()),
                            selector: None,
                            label_selector: None,
                            field_path: relationship.field_paths.first().cloned(),
                        });
                    }
                    RelationshipReferenceResolution::Many if identity.kind() == ResourceKind::Pod => {
                        findings.push(DiscoveryFinding {
                            code: DiagnosticCode::RelationshipAmbiguous,
                            resource: Some(identity.clone()),
                            selector: None,
                            label_selector: None,
                            field_path: relationship.field_paths.first().cloned(),
                        });
                    }
                    RelationshipReferenceResolution::Conflict if identity.kind() == ResourceKind::Pod => {
                        findings.push(DiscoveryFinding {
                            code: DiagnosticCode::RelationshipConflict,
                            resource: Some(identity.clone()),
                            selector: None,
                            label_selector: None,
                            field_path: relationship.field_paths.first().cloned(),
                        });
                    }
                    RelationshipReferenceResolution::None
                    | RelationshipReferenceResolution::Many
                    | RelationshipReferenceResolution::Conflict => {}
                }
            }
        }
    }
    for edge in dependencies {
        if matches!(
            (edge.dependent.kind(), edge.prerequisite.kind()),
            (ResourceKind::Container, ResourceKind::Container)
        ) {
            let DependencyEvidence::NativeRelationship { field_paths } = &edge.evidence;
            for field_path in field_paths {
                insert_grouping_edge(
                    &mut edges,
                    &edge.dependent,
                    &edge.prerequisite,
                    GroupingEvidence::ContainerDependency,
                    Some(field_path),
                );
            }
        }
    }
    let mut by_project = BTreeMap::<String, Vec<ResourceIdentity>>::new();
    for (identity, project) in ownership {
        if matches!(identity.kind(), ResourceKind::Container | ResourceKind::Pod) && records.contains_key(identity) {
            by_project.entry(project.clone()).or_default().push(identity.clone());
        }
    }
    for (project, members) in by_project {
        for (index, left) in members.iter().enumerate() {
            for right in &members[index + 1..] {
                insert_grouping_edge(
                    &mut edges,
                    left,
                    right,
                    GroupingEvidence::ComposeOwnership {
                        project: project.clone(),
                    },
                    None,
                );
            }
        }
    }
    edges
        .into_iter()
        .map(|((left, right, evidence), field_paths)| GroupingEdge {
            left,
            right,
            evidence,
            field_paths: field_paths.into_iter().collect(),
        })
        .collect()
}

fn insert_grouping_edge(
    edges: &mut BTreeMap<(ResourceIdentity, ResourceIdentity, GroupingEvidence), BTreeSet<String>>,
    left: &ResourceIdentity,
    right: &ResourceIdentity,
    evidence: GroupingEvidence,
    field_path: Option<&str>,
) {
    if left == right {
        return;
    }
    let (left, right) = if left < right {
        (left.clone(), right.clone())
    } else {
        (right.clone(), left.clone())
    };
    let paths = edges.entry((left, right, evidence)).or_default();
    if let Some(field_path) = field_path {
        paths.insert(field_path.to_owned());
    }
}

fn grouping_adjacency(edges: &[GroupingEdge]) -> BTreeMap<ResourceIdentity, BTreeSet<ResourceIdentity>> {
    let mut adjacency = BTreeMap::new();
    for edge in edges {
        adjacency
            .entry(edge.left.clone())
            .or_insert_with(BTreeSet::new)
            .insert(edge.right.clone());
        adjacency
            .entry(edge.right.clone())
            .or_insert_with(BTreeSet::new)
            .insert(edge.left.clone());
    }
    adjacency
}

fn expand_group_members(
    members: &mut BTreeSet<ResourceIdentity>,
    adjacency: &BTreeMap<ResourceIdentity, BTreeSet<ResourceIdentity>>,
) {
    let mut pending = members.iter().cloned().collect::<Vec<_>>();
    while let Some(identity) = pending.pop() {
        for neighbor in adjacency.get(&identity).into_iter().flatten() {
            if members.insert(neighbor.clone()) {
                pending.push(neighbor.clone());
            }
        }
    }
}

fn grouped_members(
    members: &BTreeSet<ResourceIdentity>,
    adjacency: &BTreeMap<ResourceIdentity, BTreeSet<ResourceIdentity>>,
) -> Vec<ResourceGroup> {
    let mut remaining = members.clone();
    let mut groups = Vec::new();
    while let Some(start) = remaining.iter().next().cloned() {
        let mut component = BTreeSet::from([start.clone()]);
        let mut pending = vec![start];
        while let Some(identity) = pending.pop() {
            for neighbor in adjacency.get(&identity).into_iter().flatten() {
                if remaining.contains(neighbor) && component.insert(neighbor.clone()) {
                    pending.push(neighbor.clone());
                }
            }
        }
        remaining.retain(|identity| !component.contains(identity));
        let members = component.into_iter().collect::<Vec<_>>();
        groups.push(ResourceGroup {
            id: members[0].clone(),
            members,
            prerequisites: Vec::new(),
        });
    }
    groups
}

fn advisory_label_findings(records: &BTreeMap<ResourceIdentity, &ResourceObservation>) -> Vec<DiscoveryFinding> {
    records
        .iter()
        .filter_map(|(identity, record)| {
            compose_label_status(record)
                .err()
                .map(|code| DiscoveryFinding::resource(code, identity.clone()))
        })
        .collect()
}

fn compose_ownership_index(
    records: &BTreeMap<ResourceIdentity, &ResourceObservation>,
) -> BTreeMap<ResourceIdentity, String> {
    records
        .iter()
        .filter_map(|(identity, record)| {
            compose_label_status(record)
                .ok()
                .flatten()
                .map(|project| (identity.clone(), project))
        })
        .collect()
}

fn compose_label_status(record: &ResourceObservation) -> Result<Option<String>, DiagnosticCode> {
    let docker = label_pair(record, DOCKER_PROJECT, DOCKER_SERVICE)?;
    let podman = label_pair(record, PODMAN_PROJECT, PODMAN_SERVICE)?;
    let docker_hash = optional_label(record, DOCKER_CONFIG_HASH)?;
    let podman_hash = optional_label(record, PODMAN_CONFIG_HASH)?;
    if docker.is_none() && podman.is_none() {
        return if docker_hash.is_none() && podman_hash.is_none() {
            Ok(None)
        } else {
            Err(DiagnosticCode::AdvisoryLabelIncomplete)
        };
    }
    if docker_hash.is_some() != podman_hash.is_some() {
        return Err(DiagnosticCode::AdvisoryLabelIncomplete);
    }
    if docker_hash.is_some() && docker_hash != podman_hash {
        return Err(DiagnosticCode::AdvisoryLabelConflict);
    }
    match (docker, podman) {
        (None, None) => Ok(None),
        (Some((docker_project, docker_service)), Some((podman_project, podman_service)))
            if docker_project == podman_project && docker_service == podman_service =>
        {
            Ok(Some(docker_project))
        }
        (Some(_), Some(_)) => Err(DiagnosticCode::AdvisoryLabelConflict),
        (Some(_), None) | (None, Some(_)) => Err(DiagnosticCode::AdvisoryLabelIncomplete),
    }
}

fn optional_label(record: &ResourceObservation, key: &str) -> Result<Option<String>, DiagnosticCode> {
    match observed_labels(record).and_then(|labels| labels.get(key)) {
        None => Ok(None),
        Some(value) if value.is_empty() => Err(DiagnosticCode::AdvisoryLabelIncomplete),
        Some(value) => Ok(Some(value.clone())),
    }
}

fn label_pair(
    record: &ResourceObservation,
    project: &str,
    service: &str,
) -> Result<Option<(String, String)>, DiagnosticCode> {
    let labels = match record.labels() {
        ObservationField::Absent => return Ok(None),
        ObservationField::Observed(value) => value.value(),
        _ => return Err(DiagnosticCode::AdvisoryLabelIncomplete),
    };
    match (labels.get(project), labels.get(service)) {
        (None, None) => Ok(None),
        (Some(project), Some(service)) if !project.is_empty() && !service.is_empty() => {
            Ok(Some((project.clone(), service.clone())))
        }
        _ => Err(DiagnosticCode::AdvisoryLabelIncomplete),
    }
}

fn finding_order(left: &DiscoveryFinding, right: &DiscoveryFinding) -> std::cmp::Ordering {
    left.code
        .as_str()
        .cmp(right.code.as_str())
        .then_with(|| left.resource.cmp(&right.resource))
        .then_with(|| left.selector.cmp(&right.selector))
        .then_with(|| left.label_selector.cmp(&right.label_selector))
        .then_with(|| left.field_path.cmp(&right.field_path))
}
