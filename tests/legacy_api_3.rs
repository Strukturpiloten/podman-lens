//! Ordered acquisition coverage for the Debian 11 / Podman 3.0.1 Libpod wire shape.

#![allow(clippy::expect_used, clippy::panic)] // Test fixture access reports concise failures.

use std::{collections::VecDeque, sync::Mutex};

use podman_lens::{
    AcquisitionOptions, DiagnosticCode, DiscoveryRequest, LibpodHeader, LibpodHeaders, LibpodMethod, LibpodRequest,
    LibpodResponse, LibpodTransport, LibpodTransportFuture, NativeRestartPolicyName, ObservationField, ResourceDetails,
    ResourceKind, ResourceSelector, TransportError, acquire_inventory, discover,
};

struct Transport {
    responses: Mutex<VecDeque<LibpodResponse>>,
    requests: Mutex<Vec<String>>,
}

impl Transport {
    fn new(responses: Vec<LibpodResponse>) -> Self {
        Self {
            responses: Mutex::new(responses.into()),
            requests: Mutex::new(Vec::new()),
        }
    }

    fn requests(&self) -> Vec<String> {
        self.requests.lock().expect("test lock").clone()
    }
}

impl LibpodTransport for Transport {
    fn send<'a>(&'a self, request: &'a LibpodRequest) -> LibpodTransportFuture<'a> {
        let response = self
            .requests
            .lock()
            .map_err(|_| TransportError::unavailable())
            .and_then(|mut requests| {
                assert_eq!(request.method(), LibpodMethod::Get);
                requests.push(request.path().as_str().to_owned());
                self.responses
                    .lock()
                    .map_err(|_| TransportError::unavailable())
                    .and_then(|mut responses| responses.pop_front().ok_or_else(TransportError::unavailable))
            });
        Box::pin(async move { response })
    }
}

fn json(body: &str) -> Result<LibpodResponse, Box<dyn std::error::Error>> {
    Ok(LibpodResponse::new(
        200,
        LibpodHeaders::new(vec![LibpodHeader::new("content-type", "application/json")?]),
        body.as_bytes(),
    )?)
}

fn responses() -> Result<Vec<LibpodResponse>, Box<dyn std::error::Error>> {
    Ok(vec![
        LibpodResponse::new(
            200,
            LibpodHeaders::new(vec![LibpodHeader::new("Libpod-API-Version", "3.0.0")?]),
            [],
        )?,
        json(r#"{"Components":[{"Name":"Podman Engine","Version":"3.0.1"}]}"#)?,
        json(r#"[{"Id":"c-web","Names":["web"]}]"#)?,
        json("[]")?,
        json(r#"[{"Name":"legacy-net"}]"#)?,
        json(r#"[{"Name":"legacy-data"}]"#)?,
        json(r#"[{"Id":"sha256:legacy","Names":["example.invalid/legacy:1"]}]"#)?,
        json(
            r#"{"Id":"c-web","Name":"web","ImageName":"example.invalid/legacy:1","Pod":"","Config":{"Entrypoint":""},"HostConfig":{"RestartPolicy":{"Name":""}},"NetworkSettings":{"Networks":{"legacy-net":{}}},"Mounts":[{"Type":"volume","Name":"legacy-data","Destination":"/data","RW":true}]}"#,
        )?,
        json(
            r#"[{"cniVersion":"0.4.0","name":"legacy-net","plugins":[{"type":"bridge","ipam":{"type":"host-local","ranges":[[{"subnet":"10.88.0.0/16","gateway":"10.88.0.1"}]],"routes":[{"dst":"10.89.0.0/16","gw":"10.88.0.1"}]}}]}]"#,
        )?,
        json(r#"{"Name":"legacy-data"}"#)?,
        json(r#"{"Id":"sha256:legacy","RepoTags":["example.invalid/legacy:1"]}"#)?,
    ])
}

#[tokio::test]
async fn api_three_acquisition_uses_legacy_shapes_without_a_secret_request() -> Result<(), Box<dyn std::error::Error>> {
    let transport = Transport::new(responses()?);
    let inventory = acquire_inventory(&transport, AcquisitionOptions::redacted()).await?;
    assert_eq!(inventory.service().engine_version().original(), "3.0.1");
    assert_eq!(inventory.service().api_version().original(), "3.0.0");
    let requests = transport.requests();
    assert_eq!(
        requests,
        [
            "/libpod/_ping",
            "/v3.0.0/libpod/version",
            "/v3.0.0/libpod/containers/json?all=true&sync=true",
            "/v3.0.0/libpod/pods/json",
            "/v3.0.0/libpod/networks/json",
            "/v3.0.0/libpod/volumes/json",
            "/v3.0.0/libpod/images/json?all=true",
            "/v3.0.0/libpod/containers/c-web/json",
            "/v3.0.0/libpod/networks/legacy-net/json",
            "/v3.0.0/libpod/volumes/legacy-data/json",
            "/v3.0.0/libpod/images/sha256%3Alegacy/json",
        ]
    );
    assert!(requests.iter().all(|path| !path.contains("/secrets")));
    assert!(
        inventory
            .section(ResourceKind::Secret)
            .expect("secret section")
            .findings()
            .iter()
            .any(|finding| finding.code() == DiagnosticCode::VersionInapplicableField)
    );

    let container = inventory
        .observations()
        .find(|observation| observation.header().identity().id() == "c-web")
        .expect("container observation");
    let ResourceDetails::Container(container) = container.details() else {
        panic!("c-web must be a container");
    };
    assert!(matches!(container.pod_membership(), ObservationField::Absent));
    assert!(matches!(container.entrypoint(), ObservationField::Absent));
    assert!(matches!(
        container.restart_policy().observed().and_then(|policy| policy.value().name().observed()),
        Some(name) if *name.value() == NativeRestartPolicyName::No
    ));

    let network = inventory
        .observations()
        .find(|observation| observation.header().identity().kind() == ResourceKind::Network)
        .expect("network observation");
    assert_eq!(network.header().identity().id(), "legacy-net");
    assert!(
        network
            .header()
            .unmodelled_fields()
            .iter()
            .any(|field| field.path() == "$.CniConfig")
    );

    let mut request = DiscoveryRequest::new();
    request.add_root(ResourceSelector::exact(ResourceKind::Container, "web")?);
    let graph = discover(&inventory, &request)?;
    let members = graph
        .groups()
        .iter()
        .flat_map(podman_lens::ResourceGroup::members)
        .map(podman_lens::ResourceIdentity::id)
        .collect::<Vec<_>>();
    assert_eq!(members, ["c-web"]);
    let prerequisites = graph
        .groups()
        .iter()
        .flat_map(podman_lens::ResourceGroup::prerequisites)
        .map(podman_lens::ResourceIdentity::id)
        .collect::<Vec<_>>();
    assert_eq!(prerequisites, ["legacy-net", "legacy-data", "sha256:legacy"]);
    Ok(())
}
