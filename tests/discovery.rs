//! M3 deterministic discovery coverage over fixture-only transports.

#![allow(clippy::expect_used)]

use std::{collections::VecDeque, sync::Mutex};

use podman_lens::{
    AcquisitionOptions, DiagnosticCode, DiscoveryExplanationKind, DiscoveryRequest, DiscoveryRootOrigin,
    GroupingEvidence, LabelSelector, LibpodHeader, LibpodHeaders, LibpodRequest, LibpodResponse, LibpodTransport,
    LibpodTransportFuture, ResourceKind, ResourceSelector, TransportError, acquire_inventory, discover,
};

struct Transport {
    responses: Mutex<VecDeque<LibpodResponse>>,
}

impl Transport {
    fn new(responses: Vec<LibpodResponse>) -> Self {
        Self {
            responses: Mutex::new(responses.into()),
        }
    }
}

impl LibpodTransport for Transport {
    fn send<'a>(&'a self, _request: &'a LibpodRequest) -> LibpodTransportFuture<'a> {
        let response = self
            .responses
            .lock()
            .map_err(|_| TransportError::unavailable())
            .and_then(|mut responses| responses.pop_front().ok_or_else(TransportError::unavailable));
        Box::pin(async move { response })
    }
}

fn json(body: &str) -> Result<LibpodResponse, Box<dyn std::error::Error>> {
    Ok(LibpodResponse::new(
        200,
        LibpodHeaders::new(vec![LibpodHeader::new("content-type", "application/json")?]),
        body.as_bytes().to_vec(),
    )?)
}

fn responses() -> Result<Vec<LibpodResponse>, Box<dyn std::error::Error>> {
    Ok(vec![
        LibpodResponse::new(
            200,
            LibpodHeaders::new(vec![LibpodHeader::new("libpod-api-version", "6.1.0")?]),
            Vec::new(),
        )?,
        json(r#"{"Components":[{"Name":"Podman Engine","Version":"6.1.0"}]}"#)?,
        json(
            r#"[{"Id":"container-a","Names":["a"]},{"Id":"container-b","Names":["b"]},{"Id":"container-c","Names":["c"]},{"Id":"infra","Names":["infra"]}]"#,
        )?,
        json(r#"[{"Id":"pod-1","Name":"pod"}]"#)?,
        json(r#"[{"id":"network-1","name":"app"},{"id":"network-2","name":"internal"}]"#)?,
        json(r#"{"Volumes":[{"Name":"data"},{"Name":"standalone-data"}]}"#)?,
        json(
            r#"[{"Id":"sha256:one","Names":["example.invalid/one:1"]},{"Id":"sha256:cache","Names":["example.invalid/cache:1"]}]"#,
        )?,
        json(
            r#"[{"ID":"secret-1","Spec":{"Name":"credential"}},{"ID":"secret-2","Spec":{"Name":"standalone-secret"}}]"#,
        )?,
        json(
            r#"{"Id":"container-a","Name":"a","Pod":"pod-1","Image":"sha256:one","ImageName":"sha256:one","NetworkSettings":{"Networks":{"app":{}}},"Mounts":[{"Type":"volume","Name":"data"}],"Dependencies":["container-b"],"Config":{"Secrets":[{"ID":"secret-1"}]}}"#,
        )?,
        json(
            r#"{"Id":"container-b","Name":"b","Image":"sha256:one","ImageName":"sha256:one","NetworkSettings":{"Networks":{"app":{}}}}"#,
        )?,
        json(
            r#"{"Id":"container-c","Name":"c","Image":"sha256:one","ImageName":"sha256:one","NetworkSettings":{"Networks":{"app":{}}}}"#,
        )?,
        json(r#"{"Id":"infra","Name":"infra","Pod":"pod-1","IsInfra":true}"#)?,
        json(r#"{"Id":"pod-1","Name":"pod","Containers":[{"Id":"container-a"},{"Id":"infra"}]}"#)?,
        json(r#"{"id":"network-1","name":"app","internal":true}"#)?,
        json(r#"{"id":"network-2","name":"internal","internal":true}"#)?,
        json(r#"{"Name":"data"}"#)?,
        json(r#"{"Name":"standalone-data"}"#)?,
        json(r#"{"Id":"sha256:cache","Names":["example.invalid/cache:1"]}"#)?,
        json(r#"{"Id":"sha256:one","Names":["example.invalid/one:1"]}"#)?,
        json(r#"{"ID":"secret-1","Spec":{"Name":"credential"}}"#)?,
        json(r#"{"ID":"secret-2","Spec":{"Name":"standalone-secret"}}"#)?,
    ])
}

async fn inventory() -> Result<podman_lens::ResourceInventory, Box<dyn std::error::Error>> {
    Ok(acquire_inventory(&Transport::new(responses()?), AcquisitionOptions::redacted()).await?)
}

fn root(kind: ResourceKind, reference: &str) -> Result<DiscoveryRequest, Box<dyn std::error::Error>> {
    let mut request = DiscoveryRequest::new();
    request.add_root(ResourceSelector::exact(kind, reference)?);
    Ok(request)
}

#[tokio::test]
async fn container_closure_keeps_dependencies_and_pods_together_but_not_shared_prerequisites()
-> Result<(), Box<dyn std::error::Error>> {
    let graph = discover(&inventory().await?, &root(ResourceKind::Container, "a")?)?;
    assert_eq!(graph.groups().len(), 1);
    let group = &graph.groups()[0];
    assert_eq!(group.id().id(), "container-a");
    assert_eq!(
        group
            .members()
            .iter()
            .map(podman_lens::ResourceIdentity::id)
            .collect::<Vec<_>>(),
        ["container-a", "container-b", "infra", "pod-1"]
    );
    assert_eq!(
        group
            .prerequisites()
            .iter()
            .map(podman_lens::ResourceIdentity::id)
            .collect::<Vec<_>>(),
        ["network-1", "data", "sha256:one", "secret-1"]
    );
    assert!(
        graph
            .dependencies()
            .iter()
            .all(|edge| edge.dependent().id() != "container-c")
    );
    assert!(
        graph
            .grouping_edges()
            .iter()
            .any(|edge| edge.left().id() == "container-a")
    );
    Ok(())
}

#[tokio::test]
async fn shared_networks_never_merge_groups_but_explicit_and_named_crossings_add_consumers()
-> Result<(), Box<dyn std::error::Error>> {
    let mut request = root(ResourceKind::Container, "a")?;
    request.add_root(ResourceSelector::exact(ResourceKind::Container, "c")?);
    let graph = discover(&inventory().await?, &request)?;
    assert_eq!(graph.groups().len(), 2);
    assert_eq!(
        graph
            .shared_prerequisites()
            .iter()
            .map(podman_lens::ResourceIdentity::id)
            .collect::<Vec<_>>(),
        ["network-1", "sha256:one"]
    );

    let graph = discover(&inventory().await?, &root(ResourceKind::Network, "app")?)?;
    assert_eq!(graph.groups().len(), 2);
    assert!(
        graph
            .groups()
            .iter()
            .any(|group| group.members().iter().any(|identity| identity.id() == "container-c"))
    );

    let mut request = root(ResourceKind::Container, "a")?;
    request.add_network_boundary_override("app")?;
    let graph = discover(&inventory().await?, &request)?;
    assert_eq!(graph.groups().len(), 2);
    assert!(
        graph
            .groups()
            .iter()
            .any(|group| group.members().iter().any(|identity| identity.id() == "container-c"))
    );
    assert!(request.add_network_boundary_override("app*").is_err());
    assert!(request.add_network_boundary_override("network-1").is_ok());
    Ok(())
}

#[tokio::test]
async fn all_seeds_only_pods_unpodded_non_infra_containers_standalone_prerequisites_and_owned_images()
-> Result<(), Box<dyn std::error::Error>> {
    let mut fixture = responses()?;
    fixture[18] = json(
        r#"{"Id":"sha256:one","Names":["example.invalid/one:1"],"Labels":{"com.docker.compose.project":"demo","com.docker.compose.service":"web","io.podman.compose.project":"demo","io.podman.compose.service":"web"}}"#,
    )?;
    let inventory = acquire_inventory(&Transport::new(fixture), AcquisitionOptions::redacted()).await?;
    let mut request = DiscoveryRequest::new();
    request.select_all();
    let graph = discover(&inventory, &request)?;
    assert!(graph.all_requested());
    let roots = graph.groups().iter().map(|group| group.id().id()).collect::<Vec<_>>();
    assert!(roots.contains(&"container-c"));
    assert!(
        graph
            .groups()
            .iter()
            .any(|group| group.members().iter().any(|identity| identity.id() == "pod-1"))
    );
    assert!(roots.contains(&"network-2"));
    assert!(roots.contains(&"standalone-data"));
    assert!(roots.contains(&"secret-2"));
    assert!(
        graph
            .resolved_roots()
            .iter()
            .any(|identity| identity.id() == "sha256:one")
    );
    assert!(!roots.contains(&"infra"));
    assert!(!roots.contains(&"sha256:cache"));
    Ok(())
}

#[tokio::test]
async fn selector_and_compose_label_failures_are_structured_and_do_not_create_grouping_edges()
-> Result<(), Box<dyn std::error::Error>> {
    assert!(ResourceSelector::exact(ResourceKind::Container, "").is_err());
    assert!(ResourceSelector::exact(ResourceKind::Container, "web*").is_err());
    let request = root(ResourceKind::Container, "absent")?;
    let graph = discover(&inventory().await?, &request)?;
    assert!(graph.groups().is_empty());
    assert_eq!(graph.findings()[0].code(), DiagnosticCode::SelectorUnresolved);

    let mut fixture = responses()?;
    fixture[8] = json(
        r#"{"Id":"container-a","Name":"a","Config":{"Labels":{"com.docker.compose.project":"demo","com.docker.compose.service":"web"}}}"#,
    )?;
    fixture[9] = json(
        r#"{"Id":"container-b","Name":"b","Config":{"Labels":{"io.podman.compose.project":"other","io.podman.compose.service":"web"}}}"#,
    )?;
    let inventory = acquire_inventory(&Transport::new(fixture), AcquisitionOptions::redacted()).await?;
    let mut request = root(ResourceKind::Container, "a")?;
    request.add_root(ResourceSelector::exact(ResourceKind::Container, "b")?);
    let graph = discover(&inventory, &request)?;
    assert!(
        graph
            .findings()
            .iter()
            .any(|finding| finding.code() == DiagnosticCode::AdvisoryLabelIncomplete)
    );
    assert!(
        graph
            .findings()
            .iter()
            .all(|finding| finding.code() != DiagnosticCode::AdvisoryLabelConflict)
    );
    assert!(
        graph
            .grouping_edges()
            .iter()
            .all(|edge| !matches!(edge.evidence(), GroupingEvidence::ComposeOwnership { .. }))
    );

    let mut fixture = responses()?;
    fixture[8] = json(
        r#"{"Id":"container-a","Name":"a","Config":{"Labels":{"com.docker.compose.project":"demo","com.docker.compose.service":"web","io.podman.compose.project":"other","io.podman.compose.service":"web"}}}"#,
    )?;
    let inventory = acquire_inventory(&Transport::new(fixture), AcquisitionOptions::redacted()).await?;
    let graph = discover(&inventory, &root(ResourceKind::Container, "a")?)?;
    assert!(
        graph
            .findings()
            .iter()
            .any(|finding| finding.code() == DiagnosticCode::AdvisoryLabelConflict)
    );
    Ok(())
}

#[tokio::test]
async fn matching_complete_compose_alias_pairs_are_advisory_grouping_evidence() -> Result<(), Box<dyn std::error::Error>>
{
    let mut fixture = responses()?;
    let labels = r#""Labels":{"com.docker.compose.project":"demo","com.docker.compose.service":"web","io.podman.compose.project":"demo","io.podman.compose.service":"web"}"#;
    fixture[8] = json(&format!(r#"{{"Id":"container-a","Name":"a","Config":{{{labels}}}}}"#))?;
    fixture[10] = json(&format!(r#"{{"Id":"container-c","Name":"c","Config":{{{labels}}}}}"#))?;
    let inventory = acquire_inventory(&Transport::new(fixture), AcquisitionOptions::redacted()).await?;
    let mut request = root(ResourceKind::Container, "a")?;
    request.add_root(ResourceSelector::exact(ResourceKind::Container, "c")?);
    let graph = discover(&inventory, &request)?;
    assert_eq!(graph.groups().len(), 1);
    assert!(
        graph
            .grouping_edges()
            .iter()
            .any(|edge| matches!(edge.evidence(), GroupingEvidence::ComposeOwnership { project } if project == "demo"))
    );
    Ok(())
}

#[tokio::test]
async fn label_roots_support_presence_exact_and_empty_values_without_debug_leaks()
-> Result<(), Box<dyn std::error::Error>> {
    assert!(LabelSelector::presence("").is_err());
    assert!(LabelSelector::exact("team*", "alpha").is_err());
    let empty = LabelSelector::exact("empty", "")?;
    assert_eq!(empty.value(), Some(""));

    let mut fixture = responses()?;
    fixture[8] =
        json(r#"{"Id":"container-a","Name":"a","Config":{"Labels":{"private.team":"private-alpha","empty":""}}}"#)?;
    fixture[10] = json(r#"{"Id":"container-c","Name":"c","Config":{"Labels":{"private.team":"private-beta"}}}"#)?;
    let inventory = acquire_inventory(&Transport::new(fixture), AcquisitionOptions::redacted()).await?;

    let mut presence = DiscoveryRequest::new();
    presence.add_label_root(LabelSelector::presence("private.team")?);
    let graph = discover(&inventory, &presence)?;
    assert_eq!(presence.label_roots().len(), 1);
    assert_eq!(graph.requested_label_roots()[0].name(), "private.team");
    assert_eq!(
        graph
            .resolved_roots()
            .iter()
            .map(podman_lens::ResourceIdentity::id)
            .collect::<Vec<_>>(),
        ["container-a", "container-c"]
    );

    let mut exact = DiscoveryRequest::new();
    exact.add_label_root(LabelSelector::exact("private.team", "private-alpha")?);
    let graph = discover(&inventory, &exact)?;
    assert_eq!(graph.resolved_roots().len(), 1);
    assert_eq!(graph.resolved_roots()[0].id(), "container-a");
    let rendered = format!(
        "{exact:?} {graph:?} {:?}",
        LabelSelector::exact("private.team", "private-alpha")?
    );
    assert!(!rendered.contains("private.team"));
    assert!(!rendered.contains("private-alpha"));

    let mut missing = DiscoveryRequest::new();
    missing.add_label_root(LabelSelector::exact("private.team", "missing")?);
    let graph = discover(&inventory, &missing)?;
    let finding = graph
        .findings()
        .iter()
        .find(|finding| finding.code() == DiagnosticCode::SelectorUnresolved)
        .expect("unresolved label root");
    assert!(finding.label_selector().is_some());
    assert!(!format!("{finding:?}").contains("private.team"));
    Ok(())
}

#[tokio::test]
async fn compose_config_hash_aliases_cover_absent_equal_incomplete_empty_conflicting_and_orphan_values()
-> Result<(), Box<dyn std::error::Error>> {
    let cases = [
        (None, None, true, None),
        (Some("same"), Some("same"), true, None),
        (Some("same"), None, true, Some(DiagnosticCode::AdvisoryLabelIncomplete)),
        (Some(""), Some(""), true, Some(DiagnosticCode::AdvisoryLabelIncomplete)),
        (
            Some("left"),
            Some("right"),
            true,
            Some(DiagnosticCode::AdvisoryLabelConflict),
        ),
        (
            Some("same"),
            Some("same"),
            false,
            Some(DiagnosticCode::AdvisoryLabelIncomplete),
        ),
    ];
    for (docker_hash, podman_hash, include_pairs, expected) in cases {
        let mut labels = serde_json::Map::new();
        if include_pairs {
            for (key, value) in [
                ("com.docker.compose.project", "demo"),
                ("com.docker.compose.service", "web"),
                ("io.podman.compose.project", "demo"),
                ("io.podman.compose.service", "web"),
            ] {
                labels.insert(key.to_owned(), serde_json::Value::String(value.to_owned()));
            }
        }
        if let Some(value) = docker_hash {
            labels.insert(
                "com.docker.compose.config-hash".to_owned(),
                serde_json::Value::String(value.to_owned()),
            );
        }
        if let Some(value) = podman_hash {
            labels.insert(
                "io.podman.compose.config-hash".to_owned(),
                serde_json::Value::String(value.to_owned()),
            );
        }
        let mut fixture = responses()?;
        fixture[8] =
            json(&serde_json::json!({"Id": "container-a", "Name": "a", "Config": {"Labels": labels}}).to_string())?;
        let inventory = acquire_inventory(&Transport::new(fixture), AcquisitionOptions::redacted()).await?;
        let graph = discover(&inventory, &root(ResourceKind::Container, "a")?)?;
        let observed = graph
            .findings()
            .iter()
            .find(|finding| {
                matches!(
                    finding.code(),
                    DiagnosticCode::AdvisoryLabelIncomplete | DiagnosticCode::AdvisoryLabelConflict
                )
            })
            .map(podman_lens::DiscoveryFinding::code);
        assert_eq!(observed, expected);
    }
    Ok(())
}

#[tokio::test]
async fn relationship_ambiguity_is_reported_and_pod_membership_never_creates_a_directed_cycle()
-> Result<(), Box<dyn std::error::Error>> {
    let mut fixture = responses()?;
    fixture[8] = json(r#"{"Id":"container-a","Name":"a","Pod":"pod-1","Image":"sha256:one","ImageName":"shared:1"}"#)?;
    fixture[17] = json(r#"{"Id":"sha256:cache","Names":["shared:1"]}"#)?;
    fixture[18] = json(r#"{"Id":"sha256:one","Names":["shared:1"]}"#)?;
    let inventory = acquire_inventory(&Transport::new(fixture), AcquisitionOptions::redacted()).await?;
    let graph = discover(&inventory, &root(ResourceKind::Container, "a")?)?;
    assert!(graph.findings().iter().any(|finding| {
        finding.code() == DiagnosticCode::RelationshipAmbiguous && finding.field_path() == Some("$.ImageName")
    }));
    assert!(graph.dependencies().iter().any(|edge| {
        edge.dependent().kind() == ResourceKind::Container && edge.prerequisite().kind() == ResourceKind::Pod
    }));
    assert!(!graph.dependencies().iter().any(|edge| {
        edge.dependent().kind() == ResourceKind::Pod && edge.prerequisite().kind() == ResourceKind::Container
    }));
    let membership = graph
        .grouping_edges()
        .iter()
        .find(|edge| {
            matches!(edge.evidence(), GroupingEvidence::PodMembership)
                && [edge.left().id(), edge.right().id()].contains(&"container-a")
        })
        .expect("pod membership grouping evidence");
    assert_eq!(membership.field_paths(), ["$.Containers[0].Id", "$.Pod"]);

    let mut fixture = responses()?;
    fixture[8] = json(r#"{"Id":"container-a","Name":"a"}"#)?;
    let inventory = acquire_inventory(&Transport::new(fixture), AcquisitionOptions::redacted()).await?;
    let graph = discover(&inventory, &root(ResourceKind::Pod, "pod")?)?;
    assert!(
        graph.groups()[0]
            .members()
            .iter()
            .any(|identity| identity.id() == "container-a")
    );

    let mut fixture = responses()?;
    fixture[12] = json(r#"{"Id":"pod-1","Name":"pod","Containers":[{"Id":"missing"}]}"#)?;
    let inventory = acquire_inventory(&Transport::new(fixture), AcquisitionOptions::redacted()).await?;
    let graph = discover(&inventory, &root(ResourceKind::Pod, "pod")?)?;
    assert!(graph.findings().iter().any(|finding| {
        finding.code() == DiagnosticCode::UnresolvedRelationship
            && finding
                .resource_identity()
                .is_some_and(|resource| resource.id() == "pod-1")
            && finding.field_path() == Some("$.Containers[0].Id")
    }));

    let mut fixture = responses()?;
    fixture[8] = json(r#"{"Id":"container-a","Name":"duplicate"}"#)?;
    fixture[9] = json(r#"{"Id":"container-b","Name":"duplicate"}"#)?;
    fixture[12] = json(r#"{"Id":"pod-1","Name":"pod","Containers":[{"Id":"duplicate"}]}"#)?;
    let inventory = acquire_inventory(&Transport::new(fixture), AcquisitionOptions::redacted()).await?;
    let graph = discover(&inventory, &root(ResourceKind::Pod, "pod")?)?;
    assert!(graph.findings().iter().any(|finding| {
        finding.code() == DiagnosticCode::RelationshipAmbiguous
            && finding
                .resource_identity()
                .is_some_and(|resource| resource.id() == "pod-1")
            && finding.field_path() == Some("$.Containers[0].Id")
    }));
    Ok(())
}

#[tokio::test]
async fn native_secret_id_and_name_grants_are_coalesced_and_never_select_one_reference_silently()
-> Result<(), Box<dyn std::error::Error>> {
    let cases = [
        (
            r#"{"Id":"container-a","Name":"a","Config":{"Secrets":[{"ID":"secret-1","Name":"standalone-secret"}]}}"#,
            Some(DiagnosticCode::RelationshipConflict),
            0,
        ),
        (
            r#"{"Id":"container-a","Name":"a","Config":{"Secrets":[{"ID":"secret-1","Name":"missing"}]}}"#,
            Some(DiagnosticCode::UnresolvedRelationship),
            0,
        ),
    ];
    for (container, expected, edge_count) in cases {
        let mut fixture = responses()?;
        fixture[8] = json(container)?;
        let inventory = acquire_inventory(&Transport::new(fixture), AcquisitionOptions::redacted()).await?;
        let graph = discover(&inventory, &root(ResourceKind::Container, "a")?)?;
        assert_eq!(
            graph
                .dependencies()
                .iter()
                .filter(
                    |edge| edge.dependent().id() == "container-a" && edge.prerequisite().kind() == ResourceKind::Secret
                )
                .count(),
            edge_count
        );
        assert!(
            graph
                .findings()
                .iter()
                .any(|finding| finding.code() == expected.expect("expected finding"))
        );
    }

    let mut fixture = responses()?;
    fixture[8] =
        json(r#"{"Id":"container-a","Name":"a","Config":{"Secrets":[{"ID":"secret-1","Name":"credential"}]}}"#)?;
    let inventory = acquire_inventory(&Transport::new(fixture), AcquisitionOptions::redacted()).await?;
    let graph = discover(&inventory, &root(ResourceKind::Container, "a")?)?;
    let secret_edges = graph
        .dependencies()
        .iter()
        .filter(|edge| edge.dependent().id() == "container-a" && edge.prerequisite().kind() == ResourceKind::Secret)
        .collect::<Vec<_>>();
    assert_eq!(secret_edges.len(), 1);
    let podman_lens::DependencyEvidence::NativeRelationship { field_paths } = secret_edges[0].evidence() else {
        return Err("secret dependency must retain native evidence".into());
    };
    assert_eq!(
        field_paths.iter().map(String::as_str).collect::<Vec<_>>(),
        ["$.Config.Secrets[0].ID", "$.Config.Secrets[0].Name"]
    );

    let mut fixture = responses()?;
    fixture[7] = json(r#"[{"ID":"secret-1","Spec":{"Name":"same"}},{"ID":"secret-2","Spec":{"Name":"same"}}]"#)?;
    fixture[8] = json(r#"{"Id":"container-a","Name":"a","Config":{"Secrets":[{"ID":"secret-1","Name":"same"}]}}"#)?;
    fixture[19] = json(r#"{"ID":"secret-1","Spec":{"Name":"same"}}"#)?;
    fixture[20] = json(r#"{"ID":"secret-2","Spec":{"Name":"same"}}"#)?;
    let inventory = acquire_inventory(&Transport::new(fixture), AcquisitionOptions::redacted()).await?;
    let graph = discover(&inventory, &root(ResourceKind::Container, "a")?)?;
    assert!(
        graph
            .findings()
            .iter()
            .any(|finding| finding.code() == DiagnosticCode::RelationshipAmbiguous)
    );
    assert!(
        !graph
            .dependencies()
            .iter()
            .any(|edge| edge.dependent().id() == "container-a" && edge.prerequisite().kind() == ResourceKind::Secret)
    );
    Ok(())
}

#[tokio::test]
async fn configured_and_locally_resolved_container_images_remain_separate() -> Result<(), Box<dyn std::error::Error>> {
    let inventory = inventory().await?;
    let container = &inventory
        .section(ResourceKind::Container)
        .expect("containers")
        .observations()[0];
    assert!(
        !container
            .header()
            .findings()
            .iter()
            .any(|finding| finding.code() == DiagnosticCode::RelationshipConflict)
    );

    let mut fixture = responses()?;
    fixture[8] = json(r#"{"Id":"container-a","Name":"a","Image":"sha256:cache","ImageName":"sha256:one"}"#)?;
    let inventory = acquire_inventory(&Transport::new(fixture), AcquisitionOptions::redacted()).await?;
    let container = &inventory
        .section(ResourceKind::Container)
        .expect("containers")
        .observations()[0];
    assert!(container.header().findings().iter().any(|finding| {
        finding.code() == DiagnosticCode::RelationshipConflict && finding.field_path() == Some("$.ImageName")
    }));
    let graph = discover(&inventory, &root(ResourceKind::Container, "a")?)?;
    let image_dependencies = graph
        .dependencies()
        .iter()
        .filter(|edge| edge.dependent().id() == "container-a" && edge.prerequisite().kind() == ResourceKind::Image)
        .collect::<Vec<_>>();
    assert_eq!(image_dependencies.len(), 1);
    assert_eq!(image_dependencies[0].prerequisite().id(), "sha256:one");

    let mut fixture = responses()?;
    fixture[8] = json(r#"{"Id":"container-a","Name":"a","Image":"sha256:one","ImageName":"missing"}"#)?;
    let inventory = acquire_inventory(&Transport::new(fixture), AcquisitionOptions::redacted()).await?;
    let graph = discover(&inventory, &root(ResourceKind::Container, "a")?)?;
    assert!(graph.findings().iter().any(|finding| {
        finding.code() == DiagnosticCode::UnresolvedRelationship && finding.field_path() == Some("$.ImageName")
    }));
    assert!(
        !graph
            .dependencies()
            .iter()
            .any(|edge| edge.dependent().id() == "container-a" && edge.prerequisite().kind() == ResourceKind::Image)
    );

    let mut fixture = responses()?;
    fixture[8] = json(r#"{"Id":"container-a","Name":"a","Image":"sha256:one","ImageName":"shared:1"}"#)?;
    fixture[17] = json(r#"{"Id":"sha256:cache","Names":["shared:1"]}"#)?;
    fixture[18] = json(r#"{"Id":"sha256:one","Names":["shared:1"]}"#)?;
    let inventory = acquire_inventory(&Transport::new(fixture), AcquisitionOptions::redacted()).await?;
    let graph = discover(&inventory, &root(ResourceKind::Container, "a")?)?;
    assert!(graph.findings().iter().any(|finding| {
        finding.code() == DiagnosticCode::RelationshipAmbiguous && finding.field_path() == Some("$.ImageName")
    }));
    assert!(
        !graph
            .dependencies()
            .iter()
            .any(|edge| edge.dependent().id() == "container-a" && edge.prerequisite().kind() == ResourceKind::Image)
    );
    Ok(())
}

#[tokio::test]
async fn malformed_relationship_fields_are_visible_but_never_traversed() -> Result<(), Box<dyn std::error::Error>> {
    let mut fixture = responses()?;
    fixture[8] = json("false")?;
    let inventory = acquire_inventory(&Transport::new(fixture), AcquisitionOptions::redacted()).await?;
    let graph = discover(&inventory, &root(ResourceKind::Container, "a")?)?;
    assert!(
        graph
            .findings()
            .iter()
            .any(|finding| finding.code() == DiagnosticCode::RelationshipConflict)
    );
    assert!(
        !graph
            .dependencies()
            .iter()
            .any(|edge| edge.dependent().id() == "container-a")
    );
    Ok(())
}

#[tokio::test]
async fn absent_relationship_collections_create_no_edges_or_conflict_findings() -> Result<(), Box<dyn std::error::Error>>
{
    let cases = [
        (8, r#"{"Id":"container-a","Name":"a"}"#, ResourceKind::Container, "a"),
        (12, r#"{"Id":"pod-1","Name":"pod"}"#, ResourceKind::Pod, "pod"),
    ];
    for (response_index, body, kind, reference) in cases {
        let mut fixture = responses()?;
        fixture[response_index] = json(body)?;
        let inventory = acquire_inventory(&Transport::new(fixture), AcquisitionOptions::redacted()).await?;
        let graph = discover(&inventory, &root(kind, reference)?)?;
        assert!(
            !graph
                .dependencies()
                .iter()
                .any(|edge| edge.dependent().kind() == kind && edge.dependent().name() == Some(reference)),
            "absent {kind:?} relationships must not create edges"
        );
        assert!(
            !graph.findings().iter().any(|finding| {
                finding.code() == DiagnosticCode::RelationshipConflict
                    && finding
                        .resource_identity()
                        .is_some_and(|identity| identity.kind() == kind)
            }),
            "absent {kind:?} relationships must not be diagnosed as a conflict"
        );
    }
    Ok(())
}

#[tokio::test]
async fn one_malformed_relationship_member_blocks_its_entire_collection() -> Result<(), Box<dyn std::error::Error>> {
    let cases = [
        (
            8,
            r#"{"Id":"container-a","Name":"a","ImageName":"sha256:one","NetworkSettings":{"Networks":{"app":{},"broken":false}}}"#,
            ResourceKind::Container,
            "a",
        ),
        (
            8,
            r#"{"Id":"container-a","Name":"a","ImageName":"sha256:one","Mounts":[{"Type":"volume","Name":"data"},false]}"#,
            ResourceKind::Container,
            "a",
        ),
        (
            8,
            r#"{"Id":"container-a","Name":"a","ImageName":"sha256:one","Dependencies":["container-b",false]}"#,
            ResourceKind::Container,
            "a",
        ),
        (
            8,
            r#"{"Id":"container-a","Name":"a","ImageName":"sha256:one","Config":{"Secrets":[{"ID":"secret-1"},false]}}"#,
            ResourceKind::Container,
            "a",
        ),
        (
            12,
            r#"{"Id":"pod-1","Name":"pod","Containers":[{"Id":"container-a"},false]}"#,
            ResourceKind::Pod,
            "pod",
        ),
        (
            12,
            r#"{"Id":"pod-1","Name":"pod","Networks":["app",false]}"#,
            ResourceKind::Pod,
            "pod",
        ),
    ];

    for (response_index, body, kind, reference) in cases {
        let mut fixture = responses()?;
        fixture[response_index] = json(body)?;
        let inventory = acquire_inventory(&Transport::new(fixture), AcquisitionOptions::redacted()).await?;
        let graph = discover(&inventory, &root(kind, reference)?)?;
        assert!(graph.findings().iter().any(|finding| {
            finding.code() == DiagnosticCode::RelationshipConflict
                && finding
                    .resource_identity()
                    .is_some_and(|identity| identity.kind() == kind)
        }));
        assert!(
            !graph
                .dependencies()
                .iter()
                .any(|edge| edge.dependent().kind() == kind && edge.dependent().name() == Some(reference)),
            "malformed {kind:?} relationship collection must not contribute an edge"
        );
    }
    Ok(())
}

#[tokio::test]
async fn malformed_infra_marker_never_promotes_a_container_to_an_all_root() -> Result<(), Box<dyn std::error::Error>> {
    let mut fixture = responses()?;
    fixture[11] = json(r#"{"Id":"infra","Name":"infra","Pod":"pod-1","IsInfra":"not-a-boolean"}"#)?;
    let inventory = acquire_inventory(&Transport::new(fixture), AcquisitionOptions::redacted()).await?;
    let mut request = DiscoveryRequest::new();
    request.select_all();
    let graph = discover(&inventory, &request)?;
    assert!(graph.resolved_roots().iter().all(|identity| identity.id() != "infra"));
    Ok(())
}

#[tokio::test]
async fn root_explanations_preserve_redacted_selector_origin_and_select_all() -> Result<(), Box<dyn std::error::Error>>
{
    let mut fixture = responses()?;
    fixture[8] =
        json(r#"{"Id":"container-a","Name":"a","Pod":"pod-1","Config":{"Labels":{"private.team":"private-alpha"}}}"#)?;
    let inventory = acquire_inventory(&Transport::new(fixture), AcquisitionOptions::redacted()).await?;
    let mut request = root(ResourceKind::Container, "a")?;
    request.add_label_root(LabelSelector::exact("private.team", "private-alpha")?);
    request.select_all();
    let graph = discover(&inventory, &request)?;
    assert!(graph.all_requested());
    assert!(graph.explanations().iter().any(|explanation| {
        matches!(explanation.kind(), DiscoveryExplanationKind::Root)
            && explanation.resource().id() == "container-a"
            && matches!(
                explanation.root_origin(),
                Some(DiscoveryRootOrigin::ResourceSelector { position: 0 })
            )
    }));
    assert!(graph.explanations().iter().any(|explanation| {
        matches!(explanation.kind(), DiscoveryExplanationKind::Root)
            && explanation.resource().id() == "container-a"
            && matches!(
                explanation.root_origin(),
                Some(DiscoveryRootOrigin::LabelSelector { position: 0 })
            )
    }));
    assert!(graph.explanations().iter().any(|explanation| {
        matches!(explanation.kind(), DiscoveryExplanationKind::Root)
            && matches!(explanation.root_origin(), Some(DiscoveryRootOrigin::All))
    }));
    let rendered = format!("{graph:?}");
    assert!(!rendered.contains("private-alpha"));
    Ok(())
}

#[tokio::test]
async fn boundary_and_group_explanations_cover_every_included_resource_and_ordering_decision()
-> Result<(), Box<dyn std::error::Error>> {
    let inventory = inventory().await?;
    let mut request = root(ResourceKind::Container, "a")?;
    request.add_root(ResourceSelector::exact(ResourceKind::Container, "c")?);
    let graph = discover(&inventory, &request)?;
    for (position, group) in graph.groups().iter().enumerate() {
        assert!(graph.explanations().iter().any(|explanation| {
            matches!(explanation.kind(), DiscoveryExplanationKind::GroupOrdering)
                && explanation.resource() == group.id()
                && explanation.position() == Some(position)
        }));
        for member in group.members() {
            assert!(graph.explanations().iter().any(|explanation| {
                matches!(explanation.kind(), DiscoveryExplanationKind::IncludedMember)
                    && explanation.resource() == member
                    && explanation.related() == Some(group.id())
            }));
        }
        for prerequisite in group.prerequisites() {
            assert!(graph.explanations().iter().any(|explanation| {
                matches!(explanation.kind(), DiscoveryExplanationKind::Prerequisite)
                    && explanation.resource() == prerequisite
                    && explanation.related() == Some(group.id())
            }));
        }
    }
    assert!(graph.explanations().iter().any(|explanation| {
        matches!(explanation.kind(), DiscoveryExplanationKind::StoppedSharedBoundary)
            && explanation.resource().id() == "network-1"
    }));
    assert!(
        graph
            .explanations()
            .iter()
            .any(|explanation| { matches!(explanation.kind(), DiscoveryExplanationKind::StrongEvidenceMerge) })
    );

    let mut request = root(ResourceKind::Container, "a")?;
    request.add_network_boundary_override("network-1")?;
    let graph = discover(&inventory, &request)?;
    assert!(graph.explanations().iter().any(|explanation| {
        matches!(explanation.kind(), DiscoveryExplanationKind::AuthorizedNetworkCrossing)
            && explanation.resource().id() == "network-1"
    }));

    let graph = discover(&inventory, &root(ResourceKind::Image, "sha256:one")?)?;
    assert!(graph.explanations().iter().any(|explanation| {
        matches!(explanation.kind(), DiscoveryExplanationKind::AuthorizedSharedCrossing)
            && explanation.resource().id() == "sha256:one"
    }));
    Ok(())
}

#[tokio::test]
async fn boundary_failures_root_kinds_filtering_and_selector_order_are_deterministic()
-> Result<(), Box<dyn std::error::Error>> {
    let inventory = inventory().await?;
    for (kind, reference) in [
        (ResourceKind::Pod, "pod"),
        (ResourceKind::Network, "internal"),
        (ResourceKind::Volume, "standalone-data"),
        (ResourceKind::Image, "sha256:cache"),
        (ResourceKind::Secret, "standalone-secret"),
    ] {
        let graph = discover(&inventory, &root(kind, reference)?)?;
        assert_eq!(graph.resolved_roots().len(), 1, "{kind:?}");
        assert!(!graph.groups().is_empty(), "{kind:?}");
    }
    let graph = discover(&inventory, &root(ResourceKind::Network, "internal")?)?;
    assert!(graph.grouping_edges().is_empty());

    let mut unused = root(ResourceKind::Container, "a")?;
    unused.add_network_boundary_override("network-2")?;
    unused.add_network_boundary_override("internal")?;
    let graph = discover(&inventory, &unused)?;
    assert_eq!(
        graph
            .findings()
            .iter()
            .filter(|finding| finding.code() == DiagnosticCode::BoundaryOverrideUnused)
            .filter_map(podman_lens::DiscoveryFinding::selector)
            .map(ResourceSelector::reference)
            .collect::<Vec<_>>(),
        ["internal", "network-2"]
    );

    let mut unresolved = root(ResourceKind::Container, "a")?;
    unresolved.add_network_boundary_override("absent")?;
    let graph = discover(&inventory, &unresolved)?;
    assert!(
        graph
            .findings()
            .iter()
            .any(|finding| finding.code() == DiagnosticCode::SelectorUnresolved)
    );

    let mut fixture = responses()?;
    fixture[4] = json(r#"[{"id":"network-1","name":"app"},{"id":"network-2","name":"app"}]"#)?;
    fixture[14] = json(r#"{"id":"network-2","name":"app","internal":true}"#)?;
    let ambiguous_inventory = acquire_inventory(&Transport::new(fixture), AcquisitionOptions::redacted()).await?;
    let mut ambiguous = root(ResourceKind::Container, "a")?;
    ambiguous.add_network_boundary_override("app")?;
    let graph = discover(&ambiguous_inventory, &ambiguous)?;
    assert!(
        graph
            .findings()
            .iter()
            .any(|finding| finding.code() == DiagnosticCode::SelectorAmbiguous)
    );

    let mut first = root(ResourceKind::Container, "a")?;
    first.add_root(ResourceSelector::exact(ResourceKind::Container, "c")?);
    let mut second = root(ResourceKind::Container, "c")?;
    second.add_root(ResourceSelector::exact(ResourceKind::Container, "a")?);
    assert_eq!(discover(&inventory, &first)?, discover(&inventory, &second)?);
    Ok(())
}

#[test]
fn resource_kind_canonical_rank_is_the_explicit_identity_order() {
    let mut kinds = [
        ResourceKind::Secret,
        ResourceKind::Image,
        ResourceKind::Volume,
        ResourceKind::Network,
        ResourceKind::Pod,
        ResourceKind::Container,
    ];
    kinds.sort();
    assert_eq!(
        kinds,
        [
            ResourceKind::Container,
            ResourceKind::Pod,
            ResourceKind::Network,
            ResourceKind::Volume,
            ResourceKind::Image,
            ResourceKind::Secret,
        ]
    );
    assert_eq!(kinds.map(ResourceKind::canonical_rank), [0, 1, 2, 3, 4, 5],);
}

#[tokio::test]
async fn group_order_uses_canonical_resource_kind_ranks_under_selector_permutations()
-> Result<(), Box<dyn std::error::Error>> {
    let inventory = inventory().await?;
    let selectors = [
        (ResourceKind::Container, "a"),
        (ResourceKind::Network, "internal"),
        (ResourceKind::Volume, "standalone-data"),
        (ResourceKind::Secret, "standalone-secret"),
    ];
    let mut forward = DiscoveryRequest::new();
    let mut reverse = DiscoveryRequest::new();
    for (kind, reference) in selectors {
        forward.add_root(ResourceSelector::exact(kind, reference)?);
    }
    for (kind, reference) in selectors.into_iter().rev() {
        reverse.add_root(ResourceSelector::exact(kind, reference)?);
    }
    let forward_graph = discover(&inventory, &forward)?;
    let reverse_graph = discover(&inventory, &reverse)?;
    assert_eq!(forward_graph, reverse_graph);
    assert_eq!(
        forward_graph
            .groups()
            .iter()
            .map(|group| group.id().kind())
            .collect::<Vec<_>>(),
        [
            ResourceKind::Container,
            ResourceKind::Network,
            ResourceKind::Volume,
            ResourceKind::Secret,
        ]
    );
    Ok(())
}
