//! M5 public deployment-planning contracts.

#![allow(clippy::expect_used)] // Test-only construction keeps each semantic scenario legible.

use podman_lens::{
    AbsoluteContainerPath, ArgumentArray, ContainerHostname, ContainerIntent, ContainerUser, ContainerWorkdir,
    DeploymentConnectionReference, DeploymentEnvironmentValue, DeploymentIntent, DeploymentResource,
    DeploymentResourceId, DnsConfiguration, EnvironmentAssignment, EnvironmentName, ExternalPrecondition, HostAlias,
    ImageIntent, ImagePullPolicy, ImageSource, Label, LabelKey, MountAccess, MountIntent, NamedVolumeCopyMode,
    NamedVolumeMount, NetworkAttachment, NetworkCidr, NetworkIntent, NetworkRoute, NetworkSubnet, ObservedApiVersion,
    ObservedPodmanVersion, PodIntent, PortMapping, PortProtocol, PublicEnvironmentValue, PublicLabelValue,
    ResourceKind, RestartPolicy, RouteType, SecretGrant, SecretIntent, SemanticOperationAction,
    SensitiveInlineEnvironmentValue, SensitiveInputReference, StartupDependency, StaticMacAddress,
    TargetExecutionContext, TargetProfile, VolumeIntent, plan_deployment,
};

fn target(version: &str) -> TargetProfile {
    TargetProfile::new(
        ObservedPodmanVersion::parse(version).expect("reviewed Podman version"),
        ObservedApiVersion::parse("4.0.0").expect("reviewed Libpod API version"),
    )
    .expect("compatible target profile")
}

#[test]
fn pod_members_cannot_own_network_namespace_configuration() {
    let image = id(ResourceKind::Image, "registry.example.invalid/member:1");
    let network = id(ResourceKind::Network, "member-network");
    let pod = id(ResourceKind::Pod, "member-pod");
    let container = id(ResourceKind::Container, "member");
    let mut member = ContainerIntent::new(container.clone(), image.clone()).expect("member");
    member.set_pod(pod.clone()).expect("pod");
    member
        .add_network(NetworkAttachment::new(network.clone()).expect("network attachment"))
        .expect("network");
    member
        .add_port(PortMapping::new(None, 8080, 8080, PortProtocol::Tcp).expect("port"))
        .expect("port");
    member
        .dns_mut()
        .add_server("192.0.2.53".parse().expect("address"))
        .expect("dns");
    member
        .add_host_alias(HostAlias::new("192.0.2.10".parse().expect("address"), "member.test").expect("host"))
        .expect("host");
    member.set_network_order(vec![network.clone()]).expect("order");

    let mut pod_intent = PodIntent::new(pod).expect("pod");
    pod_intent.add_member(container).expect("member");
    let mut intent = DeploymentIntent::new(target("6.1.0"));
    intent.add_resource(DeploymentResource::Image(
        ImageIntent::new(
            image,
            ImageSource::new("registry.example.invalid/team/member:1").expect("image source"),
            ImagePullPolicy::Missing,
        )
        .expect("image"),
    ));
    intent.add_resource(DeploymentResource::Network(
        NetworkIntent::new(network).expect("network"),
    ));
    intent.add_resource(DeploymentResource::Pod(pod_intent));
    intent.add_resource(DeploymentResource::Container(member));

    let outcome = plan_deployment(&intent);
    assert!(!outcome.is_success());
    assert_eq!(
        outcome
            .findings()
            .iter()
            .filter_map(podman_lens::PlanningFinding::field)
            .collect::<Vec<_>>(),
        ["dns", "host_aliases", "network_order", "networks", "ports"]
    );
}

#[test]
fn typed_networking_values_reject_wrong_families_duplicates_and_invalid_values_without_mutation() {
    let network = id(ResourceKind::Network, "network");
    let mut attachment = NetworkAttachment::new(network).expect("network attachment");
    assert!(
        attachment
            .set_static_ipv4("2001:db8::10".parse().expect("address"))
            .is_err()
    );
    attachment
        .set_static_ipv4("192.0.2.10".parse().expect("address"))
        .expect("ipv4");
    assert!(
        attachment
            .set_static_ipv4("192.0.2.11".parse().expect("address"))
            .is_err()
    );
    assert_eq!(attachment.static_ipv4(), Some("192.0.2.10".parse().expect("address")));
    attachment
        .set_static_ipv6("2001:db8::10".parse().expect("address"))
        .expect("ipv6");
    assert!(
        attachment
            .set_static_ipv6("2001:db8::11".parse().expect("address"))
            .is_err()
    );
    assert_eq!(attachment.static_ipv6(), Some("2001:db8::10".parse().expect("address")));
    attachment
        .set_static_mac(StaticMacAddress::new("02:42:ac:11:00:02").expect("mac"))
        .expect("mac");
    assert!(
        attachment
            .set_static_mac(StaticMacAddress::new("02:42:ac:11:00:03").expect("mac"))
            .is_err()
    );
    assert_eq!(
        attachment.static_mac().map(StaticMacAddress::as_str),
        Some("02:42:ac:11:00:02")
    );
    attachment.add_alias("web").expect("alias");
    assert_eq!(
        attachment
            .add_alias("web")
            .expect_err("duplicate alias")
            .code()
            .as_str(),
        "PLN0035"
    );
    assert_eq!(
        attachment
            .add_alias("bad:alias")
            .expect_err("invalid alias")
            .code()
            .as_str(),
        "PLN0034"
    );

    let mut dns = DnsConfiguration::default();
    dns.add_search("example.test").expect("search");
    assert!(dns.add_search("example.test").is_err());
    for hostname in [
        "bad\nname",
        "bad host",
        "bad:host",
        "bad,host",
        "bad=host",
        "-bad",
        "bad-",
        "a..b",
    ] {
        assert!(
            HostAlias::new("192.0.2.1".parse().expect("address"), hostname).is_err(),
            "{hostname}"
        );
    }
    assert_eq!(
        HostAlias::new("2001:db8::53".parse().expect("address"), "dns.example")
            .expect("IPv6 host alias")
            .hostname(),
        "dns.example"
    );
    assert!(PortMapping::new(None, 0, 80, PortProtocol::Tcp).is_err());

    let cidr = NetworkCidr::new("192.0.2.0/24").expect("cidr");
    assert!(NetworkRoute::new(cidr.clone(), None, RouteType::Unicast).is_err());
    assert!(NetworkRoute::new(cidr, Some("192.0.2.1".parse().expect("address")), RouteType::Blackhole,).is_err());
}

#[test]
fn network_alias_capacity_returns_the_duplicate_or_capacity_rule() {
    let mut attachment = NetworkAttachment::new(id(ResourceKind::Network, "network")).expect("attachment");
    for index in 0..64 {
        attachment
            .add_alias(format!("alias-{index}"))
            .expect("alias within capacity");
    }
    assert_eq!(
        attachment
            .add_alias("one-too-many")
            .expect_err("alias capacity")
            .code()
            .as_str(),
        "PLN0035"
    );
}

#[test]
fn port_mappings_reject_duplicate_or_conflicting_host_sockets_only() {
    let image = id(ResourceKind::Image, "registry.example.invalid/application:1");
    let base = PortMapping::new(
        Some("192.0.2.10".parse().expect("host address")),
        8080,
        80,
        PortProtocol::Tcp,
    )
    .expect("port mapping");

    let mut duplicate =
        ContainerIntent::new(id(ResourceKind::Container, "duplicate"), image.clone()).expect("container");
    duplicate.add_port(base.clone()).expect("port mapping");
    assert_eq!(
        duplicate
            .add_port(base.clone())
            .expect_err("duplicate host socket")
            .code()
            .as_str(),
        "PLN0035"
    );

    let mut conflict = ContainerIntent::new(id(ResourceKind::Container, "conflict"), image.clone()).expect("container");
    conflict.add_port(base.clone()).expect("port mapping");
    let conflicting_container_port = PortMapping::new(
        Some("192.0.2.10".parse().expect("host address")),
        8080,
        81,
        PortProtocol::Tcp,
    )
    .expect("port mapping");
    assert_eq!(
        conflict
            .add_port(conflicting_container_port)
            .expect_err("conflicting host socket")
            .code()
            .as_str(),
        "PLN0035"
    );

    let mut distinct = ContainerIntent::new(id(ResourceKind::Container, "distinct"), image).expect("container");
    distinct.add_port(base).expect("port mapping");
    for mapping in [
        PortMapping::new(
            Some("192.0.2.11".parse().expect("host address")),
            8080,
            80,
            PortProtocol::Tcp,
        )
        .expect("different host address"),
        PortMapping::new(
            Some("192.0.2.10".parse().expect("host address")),
            8081,
            80,
            PortProtocol::Tcp,
        )
        .expect("different host port"),
        PortMapping::new(
            Some("192.0.2.10".parse().expect("host address")),
            8080,
            80,
            PortProtocol::Udp,
        )
        .expect("different protocol"),
    ] {
        distinct.add_port(mapping).expect("distinct host socket");
    }
    assert_eq!(distinct.ports().len(), 4);
}

#[test]
fn network_subnets_reject_duplicate_cidrs_even_when_settings_differ() {
    let cidr = NetworkCidr::new("192.0.2.0/24").expect("CIDR");
    let mut exact = NetworkIntent::new(id(ResourceKind::Network, "exact")).expect("network");
    exact.add_subnet(NetworkSubnet::new(cidr.clone())).expect("subnet");
    assert_eq!(
        exact
            .add_subnet(NetworkSubnet::new(
                NetworkCidr::new("192.0.2.1/24").expect("equivalent CIDR")
            ))
            .expect_err("textually different duplicate CIDR")
            .code()
            .as_str(),
        "PLN0035"
    );

    let mut gateway = NetworkSubnet::new(cidr.clone());
    gateway
        .set_gateway("192.0.2.1".parse().expect("gateway"))
        .expect("gateway");
    let mut range = NetworkSubnet::new(cidr);
    range
        .set_range("192.0.2.10".parse().expect("start"), "192.0.2.20".parse().expect("end"))
        .expect("range");
    let mut conflicting = NetworkIntent::new(id(ResourceKind::Network, "conflicting")).expect("network");
    conflicting.add_subnet(gateway).expect("subnet");
    assert_eq!(
        conflicting
            .add_subnet(range)
            .expect_err("same CIDR with different IPAM settings")
            .code()
            .as_str(),
        "PLN0035"
    );
    conflicting
        .add_subnet(NetworkSubnet::new(NetworkCidr::new("198.51.100.0/24").expect("CIDR")))
        .expect("distinct CIDR");
    assert_eq!(conflicting.subnets().len(), 2);
}

#[test]
fn duplicate_network_order_rejection_preserves_the_first_declared_order() {
    let first = id(ResourceKind::Network, "first");
    let second = id(ResourceKind::Network, "second");
    let expected = vec![first.clone(), second.clone()];
    let mut container = ContainerIntent::new(
        id(ResourceKind::Container, "application"),
        id(ResourceKind::Image, "registry.example.invalid/application:1"),
    )
    .expect("container");
    container.set_network_order(expected.clone()).expect("network order");
    assert!(container.set_network_order(vec![second]).is_err());
    assert_eq!(container.network_order(), Some(expected.as_slice()));
}

#[test]
fn network_cidrs_validate_containment_ranges_and_duplicate_setters_without_mutation() {
    let ipv4_zero = NetworkCidr::new("0.0.0.0/0").expect("CIDR");
    assert!(ipv4_zero.contains("203.0.113.1".parse().expect("address")));
    assert!(!ipv4_zero.contains("2001:db8::1".parse().expect("address")));
    assert_eq!(NetworkCidr::new("192.0.2.1/24").expect("CIDR").as_str(), "192.0.2.0/24");
    let ipv4_host = NetworkCidr::new("192.0.2.10/32").expect("CIDR");
    assert!(ipv4_host.contains("192.0.2.10".parse().expect("address")));
    assert!(!ipv4_host.contains("192.0.2.11".parse().expect("address")));
    let ipv6_zero = NetworkCidr::new("::/0").expect("CIDR");
    assert!(ipv6_zero.contains("2001:db8::1".parse().expect("address")));
    assert_eq!(
        NetworkCidr::new("2001:db8::feed/64").expect("CIDR").as_str(),
        "2001:db8::/64"
    );
    let ipv6_host = NetworkCidr::new("2001:db8::1/128").expect("CIDR");
    assert!(ipv6_host.contains("2001:db8::1".parse().expect("address")));
    assert!(!ipv6_host.contains("2001:db8::2".parse().expect("address")));
    assert!(!ipv6_host.contains("192.0.2.1".parse().expect("address")));

    let mut subnet = NetworkSubnet::new(NetworkCidr::new("192.0.2.0/24").expect("CIDR"));
    subnet
        .set_gateway("192.0.2.1".parse().expect("gateway"))
        .expect("gateway");
    assert!(subnet.set_gateway("192.0.2.2".parse().expect("gateway")).is_err());
    assert_eq!(subnet.gateway(), Some("192.0.2.1".parse().expect("gateway")));
    assert!(
        subnet
            .set_range("192.0.1.10".parse().expect("start"), "192.0.2.20".parse().expect("end"))
            .is_err()
    );
    subnet
        .set_range("192.0.2.10".parse().expect("start"), "192.0.2.20".parse().expect("end"))
        .expect("range");
    assert!(
        subnet
            .set_range("192.0.2.21".parse().expect("start"), "192.0.2.22".parse().expect("end"))
            .is_err()
    );
    assert_eq!(
        subnet.range(),
        Some(("192.0.2.10".parse().expect("start"), "192.0.2.20".parse().expect("end")))
    );

    let mut reversed = NetworkSubnet::new(NetworkCidr::new("2001:db8::/64").expect("CIDR"));
    assert!(
        reversed
            .set_range(
                "2001:db8::20".parse().expect("start"),
                "2001:db8::10".parse().expect("end")
            )
            .is_err()
    );
    assert!(
        reversed
            .set_range(
                "2001:db9::10".parse().expect("start"),
                "2001:db8::20".parse().expect("end")
            )
            .is_err()
    );
    reversed
        .set_range(
            "2001:db8::10".parse().expect("start"),
            "2001:db8::20".parse().expect("end"),
        )
        .expect("IPv6 range");
    assert_eq!(
        reversed.range(),
        Some((
            "2001:db8::10".parse().expect("start"),
            "2001:db8::20".parse().expect("end")
        ))
    );

    let mut route = NetworkRoute::new(
        NetworkCidr::new("198.51.100.0/24").expect("CIDR"),
        Some("198.51.100.1".parse().expect("gateway")),
        RouteType::Unicast,
    )
    .expect("route");
    route.set_metric(42).expect("metric");
    assert!(route.set_metric(43).is_err());
    assert_eq!(route.metric(), Some(42));
}

#[test]
fn static_network_addresses_require_an_explicit_rootful_target_context() {
    for (context, expected_fields) in [
        (
            TargetExecutionContext::Unknown,
            vec![
                "networks.static_ipv4_requires_rootful",
                "networks.static_ipv6_requires_rootful",
                "networks.static_mac_requires_rootful",
            ],
        ),
        (
            TargetExecutionContext::Rootless,
            vec![
                "networks.static_ipv4_requires_rootful",
                "networks.static_ipv6_requires_rootful",
                "networks.static_mac_requires_rootful",
            ],
        ),
    ] {
        let outcome = plan_with_all_static_network_values(context);
        assert!(outcome.plan().is_none(), "{context:?}");
        assert_eq!(
            outcome
                .findings()
                .iter()
                .filter_map(podman_lens::PlanningFinding::field)
                .collect::<Vec<_>>(),
            expected_fields,
            "{context:?}"
        );
    }
    assert!(plan_with_all_static_network_values(TargetExecutionContext::Rootful).is_success());
}

#[test]
fn port_mapping_accepts_each_protocol_and_ipv6_bind_address() {
    for protocol in [PortProtocol::Tcp, PortProtocol::Udp, PortProtocol::Sctp] {
        let port =
            PortMapping::new(Some("2001:db8::1".parse().expect("address")), 8443, 443, protocol).expect("port mapping");
        assert_eq!(port.host_ip(), Some("2001:db8::1".parse().expect("address")));
        assert_eq!(port.protocol(), protocol);
    }
}

fn plan_with_all_static_network_values(context: TargetExecutionContext) -> podman_lens::PlanningOutcome {
    let network = id(ResourceKind::Network, "network");
    let image = id(ResourceKind::Image, "registry.example.invalid/application:1");
    let container = id(ResourceKind::Container, "application");
    let mut attachment = NetworkAttachment::new(network.clone()).expect("attachment");
    attachment
        .set_static_ipv4("192.0.2.10".parse().expect("IPv4"))
        .expect("IPv4");
    attachment
        .set_static_ipv6("2001:db8::10".parse().expect("IPv6"))
        .expect("IPv6");
    attachment
        .set_static_mac(StaticMacAddress::new("02:42:ac:11:00:02").expect("MAC"))
        .expect("MAC");
    let mut container = ContainerIntent::new(container, image.clone()).expect("container");
    container.add_network(attachment).expect("network");
    let mut target = target("6.1.0");
    target.set_execution_context(context);
    let mut intent = DeploymentIntent::new(target);
    intent.add_resource(DeploymentResource::Network(
        NetworkIntent::new(network).expect("network"),
    ));
    intent.add_resource(DeploymentResource::Image(
        ImageIntent::new(
            image,
            ImageSource::new("registry.example.invalid/team/application:1").expect("image source"),
            ImagePullPolicy::Missing,
        )
        .expect("image"),
    ));
    intent.add_resource(DeploymentResource::Container(container));
    plan_deployment(&intent)
}

fn id(kind: ResourceKind, name: &str) -> DeploymentResourceId {
    DeploymentResourceId::new(kind, name).expect("valid resource identity")
}

fn mount(volume: DeploymentResourceId, destination: &str) -> NamedVolumeMount {
    NamedVolumeMount::new(
        volume,
        AbsoluteContainerPath::new(destination).expect("destination"),
        MountAccess::ReadWrite,
        NamedVolumeCopyMode::Copy,
    )
    .expect("mount")
}

#[test]
fn typed_setting_scalars_validate_values_and_reject_conflicts() {
    assert_eq!(
        ArgumentArray::new(["program", ""])
            .expect("empty argument is valid")
            .values(),
        ["program", ""]
    );
    for arguments in [
        Vec::new(),
        vec!["bad\nargument".to_owned()],
        vec!["a".repeat(4097)],
        vec!["argument".to_owned(); 129],
    ] {
        assert_eq!(
            ArgumentArray::new(arguments)
                .expect_err("invalid arguments")
                .code()
                .as_str(),
            "PLN0034"
        );
    }
    for path in ["/", "/srv/application", "/var/lib/app-data"] {
        assert_eq!(AbsoluteContainerPath::new(path).expect("path").as_str(), path);
    }
    for path in [
        "",
        "relative",
        "/double//slash",
        "/a/./b",
        "/a/../b",
        r"C:\\data",
        "/bad\npath",
    ] {
        assert!(AbsoluteContainerPath::new(path).is_err(), "{path}");
    }
    assert_eq!(ContainerUser::new("1000:1000").expect("user").as_str(), "1000:1000");
    assert!(ContainerUser::new("user name").is_err());
    assert_eq!(
        ContainerHostname::new("web-1.example").expect("hostname").as_str(),
        "web-1.example"
    );
    assert!(ContainerHostname::new("-web.example").is_err());

    let mut container = ContainerIntent::new(
        id(ResourceKind::Container, "web"),
        id(ResourceKind::Image, "registry.example.invalid/web:1"),
    )
    .expect("container");
    {
        let settings = container.settings_mut();
        settings
            .set_command(ArgumentArray::new(["serve", "--foreground"]).expect("command"))
            .expect("command");
        assert_eq!(
            settings
                .set_command(ArgumentArray::new(["serve", "--foreground"]).expect("command"))
                .expect_err("duplicate command")
                .code()
                .as_str(),
            "PLN0035"
        );
        assert_eq!(
            settings
                .set_command(ArgumentArray::new(["worker"]).expect("command"))
                .expect_err("conflicting command")
                .code()
                .as_str(),
            "PLN0038"
        );
        settings
            .set_entrypoint(ArgumentArray::new(["/entrypoint"]).expect("entrypoint"))
            .expect("entrypoint");
        settings
            .set_user(ContainerUser::new("1000:1000").expect("user"))
            .expect("user");
        settings
            .set_workdir(ContainerWorkdir::new(
                AbsoluteContainerPath::new("/srv/application").expect("workdir"),
            ))
            .expect("workdir");
        settings
            .set_hostname(ContainerHostname::new("web-1.example").expect("hostname"))
            .expect("hostname");
        for policy in [
            RestartPolicy::No,
            RestartPolicy::OnFailure,
            RestartPolicy::Always,
            RestartPolicy::UnlessStopped,
        ] {
            let mut settings = podman_lens::ContainerSettings::default();
            settings.set_restart_policy(policy).expect("policy");
            assert_eq!(settings.restart_policy(), Some(policy));
        }
    }
}

#[test]
fn pod_members_reject_explicit_hostname_before_rendering() {
    let image = id(ResourceKind::Image, "registry.example.invalid/web:1");
    let pod = id(ResourceKind::Pod, "application");
    let container = id(ResourceKind::Container, "web");
    let mut pod_intent = PodIntent::new(pod.clone()).expect("pod");
    pod_intent.add_member(container.clone()).expect("member");
    let mut container_intent = ContainerIntent::new(container.clone(), image.clone()).expect("container");
    container_intent.set_pod(pod).expect("pod assignment");
    container_intent
        .settings_mut()
        .set_hostname(ContainerHostname::new("web.example").expect("hostname"))
        .expect("hostname");
    let mut intent = DeploymentIntent::new(target("6.1.0"));
    intent.add_resource(DeploymentResource::Image(
        ImageIntent::new(
            image,
            ImageSource::new("registry.example.invalid/web:1").expect("image source"),
            ImagePullPolicy::Missing,
        )
        .expect("image"),
    ));
    intent.add_resource(DeploymentResource::Pod(pod_intent));
    intent.add_resource(DeploymentResource::Container(container_intent));
    let outcome = plan_deployment(&intent);
    assert!(outcome.plan().is_none());
    assert!(outcome.findings().iter().any(|finding| {
        finding.code().as_str() == "PLN0038"
            && finding.subject() == Some(&container)
            && finding.field() == Some("hostname")
    }));
}

#[test]
fn scalar_user_path_and_hostname_grammars_cover_exact_boundaries() {
    for user in ["root", "1000", "app:1000", "app:group", "a.b_c-1:2._-9"] {
        assert_eq!(ContainerUser::new(user).expect("valid user").as_str(), user);
    }
    for user in ["", ":group", "user:", "user::group", "user:group:extra", "user/name"] {
        assert!(ContainerUser::new(user).is_err(), "{user}");
    }
    assert!(ContainerUser::new("a".repeat(4097)).is_err());

    let longest_path = format!("/{}", "a".repeat(4095));
    assert_eq!(
        AbsoluteContainerPath::new(longest_path.clone())
            .expect("4096-byte path")
            .as_str(),
        longest_path
    );
    assert!(AbsoluteContainerPath::new(format!("/{}", "a".repeat(4096))).is_err());

    let hostname = ["a".repeat(63), "b".repeat(63), "c".repeat(63), "d".repeat(61)].join(".");
    assert_eq!(hostname.len(), 253);
    assert_eq!(
        ContainerHostname::new(hostname.clone()).expect("hostname").as_str(),
        hostname
    );
    for hostname in [
        "a".repeat(64),
        "web-".to_owned(),
        "web..example".to_owned(),
        "web_1".to_owned(),
    ] {
        assert!(ContainerHostname::new(hostname).is_err());
    }
}

#[test]
fn scalar_label_and_environment_grammars_cover_positive_and_negative_boundaries() {
    let key = "k".repeat(4096);
    assert_eq!(LabelKey::new(key.clone()).expect("label key").as_str(), key);
    for key in ["", "has=equals", "bad\nkey", &"k".repeat(4097)] {
        assert!(LabelKey::new(key).is_err(), "{key:?}");
    }

    let value = "v".repeat(4096);
    assert_eq!(
        PublicLabelValue::new(value.clone())
            .expect("public label value")
            .as_str(),
        value
    );
    for value in ["bad\nvalue", &"v".repeat(4097)] {
        assert!(PublicLabelValue::new(value).is_err(), "{value:?}");
    }
    assert_eq!(
        PublicLabelValue::new("").expect("empty public label value").as_str(),
        ""
    );

    let name = format!("A{}", "_".repeat(255));
    assert_eq!(EnvironmentName::new(name.clone()).expect("name").as_str(), name);
    for name in [
        "",
        "1VALUE",
        "VALUE-NAME",
        "VALUE.NAME",
        &format!("A{}", "_".repeat(256)),
    ] {
        assert!(EnvironmentName::new(name).is_err(), "{name:?}");
    }

    let value = "v".repeat(4096);
    assert_eq!(
        PublicEnvironmentValue::new(value).expect("public value").as_str().len(),
        4096
    );
    for value in ["bad\nvalue", &"v".repeat(4097)] {
        assert!(PublicEnvironmentValue::new(value).is_err(), "{value:?}");
        assert!(SensitiveInlineEnvironmentValue::new(value).is_err(), "{value:?}");
    }
    assert!(SensitiveInlineEnvironmentValue::new("").is_ok());
}

#[test]
fn named_volume_mounts_keep_both_mode_values_and_reject_non_volume_sources() {
    let volume = id(ResourceKind::Volume, "application-data");
    let copy = NamedVolumeMount::new(
        volume.clone(),
        AbsoluteContainerPath::new("/data").expect("path"),
        MountAccess::ReadWrite,
        NamedVolumeCopyMode::Copy,
    )
    .expect("copy mount");
    assert_eq!(copy.source(), &volume);
    assert_eq!(copy.destination().as_str(), "/data");
    assert!(!copy.is_read_only());
    assert_eq!(copy.copy_mode(), NamedVolumeCopyMode::Copy);

    let no_copy = NamedVolumeMount::new(
        volume,
        AbsoluteContainerPath::new("/readonly").expect("path"),
        MountAccess::ReadOnly,
        NamedVolumeCopyMode::NoCopy,
    )
    .expect("no-copy mount");
    assert!(no_copy.is_read_only());
    assert_eq!(no_copy.copy_mode(), NamedVolumeCopyMode::NoCopy);
    assert!(
        NamedVolumeMount::new(
            id(ResourceKind::Network, "not-a-volume"),
            AbsoluteContainerPath::new("/data").expect("path"),
            MountAccess::ReadWrite,
            NamedVolumeCopyMode::Copy,
        )
        .is_err()
    );
}

#[test]
fn typed_settings_preserve_collection_order_and_redact_sensitive_environment_values() {
    let mut container = ContainerIntent::new(
        id(ResourceKind::Container, "web"),
        id(ResourceKind::Image, "registry.example.invalid/web:1"),
    )
    .expect("container");
    let sensitive_sentinel = "sensitive-inline-value";
    {
        let settings = container.settings_mut();
        settings
            .add_label(Label::new(
                LabelKey::new("org.example.first").expect("key"),
                PublicLabelValue::new("").expect("empty value"),
            ))
            .expect("label");
        settings
            .add_label(Label::new(
                LabelKey::new("org.example.second").expect("key"),
                PublicLabelValue::new("two").expect("value"),
            ))
            .expect("label");
        assert_eq!(
            settings
                .labels()
                .iter()
                .map(|label| label.key().as_str())
                .collect::<Vec<_>>(),
            ["org.example.first", "org.example.second"]
        );
        assert_eq!(
            settings
                .add_label(Label::new(
                    LabelKey::new("org.example.first").expect("key"),
                    PublicLabelValue::new("replacement").expect("value"),
                ))
                .expect_err("duplicate label")
                .code()
                .as_str(),
            "PLN0035"
        );
        settings
            .add_environment(EnvironmentAssignment::new(
                EnvironmentName::new("EMPTY").expect("name"),
                DeploymentEnvironmentValue::Public(PublicEnvironmentValue::new("").expect("value")),
            ))
            .expect("plain environment");
        settings
            .add_environment(EnvironmentAssignment::new(
                EnvironmentName::new("PASSWORD").expect("name"),
                DeploymentEnvironmentValue::SensitiveInline(
                    SensitiveInlineEnvironmentValue::new(sensitive_sentinel).expect("sensitive value"),
                ),
            ))
            .expect("sensitive environment");
        settings
            .add_environment(EnvironmentAssignment::new(
                EnvironmentName::new("TOKEN_FILE").expect("name"),
                DeploymentEnvironmentValue::External(
                    SensitiveInputReference::new("vault/token-file").expect("external value"),
                ),
            ))
            .expect("external environment");
        assert_eq!(
            settings
                .add_environment(EnvironmentAssignment::new(
                    EnvironmentName::new("EMPTY").expect("name"),
                    DeploymentEnvironmentValue::Public(PublicEnvironmentValue::new("replacement").expect("value")),
                ))
                .expect_err("duplicate environment")
                .code()
                .as_str(),
            "PLN0035"
        );
        assert_eq!(settings.environment().len(), 3);
    }
    let debug = format!("{container:?}");
    assert!(!debug.contains(sensitive_sentinel));
    assert!(!debug.contains("vault/token-file"));
}

#[test]
fn typed_setting_collections_accept_exact_capacity_and_reject_one_more_value() {
    let mut labels = podman_lens::ContainerSettings::default();
    for index in 0..128 {
        labels
            .add_label(Label::new(
                LabelKey::new(format!("org.example.{index}")).expect("label key"),
                PublicLabelValue::new("value").expect("label value"),
            ))
            .expect("bounded label");
    }
    assert_eq!(labels.labels().len(), 128);
    assert_eq!(
        labels
            .add_label(Label::new(
                LabelKey::new("org.example.overflow").expect("label key"),
                PublicLabelValue::new("value").expect("label value"),
            ))
            .expect_err("label capacity")
            .code()
            .as_str(),
        "PLN0034"
    );

    let mut environment = podman_lens::ContainerSettings::default();
    for index in 0..128 {
        environment
            .add_environment(EnvironmentAssignment::new(
                EnvironmentName::new(format!("VALUE_{index}")).expect("environment name"),
                DeploymentEnvironmentValue::Public(PublicEnvironmentValue::new("value").expect("value")),
            ))
            .expect("bounded environment");
    }
    assert_eq!(environment.environment().len(), 128);
    assert_eq!(
        environment
            .add_environment(EnvironmentAssignment::new(
                EnvironmentName::new("VALUE_OVERFLOW").expect("environment name"),
                DeploymentEnvironmentValue::Public(PublicEnvironmentValue::new("value").expect("value")),
            ))
            .expect_err("environment capacity")
            .code()
            .as_str(),
        "PLN0034"
    );
}

#[test]
fn mounts_resolve_volume_dependencies_and_reject_duplicate_destinations() {
    let volume = id(ResourceKind::Volume, "data");
    let image = id(ResourceKind::Image, "registry.example.invalid/web:1");
    let container = id(ResourceKind::Container, "web");
    let mut managed_container = ContainerIntent::new(container, image.clone()).expect("container");
    managed_container.add_mount(mount(volume.clone(), "/data"));
    let mut managed = DeploymentIntent::new(target("6.1.0"));
    managed.add_resource(DeploymentResource::Volume(
        VolumeIntent::new(volume.clone()).expect("volume"),
    ));
    managed.add_resource(DeploymentResource::Image(
        ImageIntent::new(
            image,
            ImageSource::new("registry.example.invalid/web:1").expect("image source"),
            ImagePullPolicy::Missing,
        )
        .expect("image"),
    ));
    managed.add_resource(DeploymentResource::Container(managed_container));
    let managed_outcome = plan_deployment(&managed);
    let plan = managed_outcome.plan().expect("plan");
    assert!(
        plan.operations()[2]
            .depends_on()
            .iter()
            .any(|dependency| dependency.resource() == &volume)
    );

    let mut duplicate = PodIntent::new(id(ResourceKind::Pod, "duplicate")).expect("pod");
    duplicate.add_infra_mount(mount(volume.clone(), "/data"));
    duplicate.add_infra_mount(
        NamedVolumeMount::new(
            volume.clone(),
            AbsoluteContainerPath::new("/data").expect("destination"),
            MountAccess::ReadOnly,
            NamedVolumeCopyMode::NoCopy,
        )
        .expect("mount"),
    );
    let mut invalid = DeploymentIntent::new(target("6.1.0"));
    invalid.add_resource(DeploymentResource::Pod(duplicate));
    invalid.add_resource(DeploymentResource::ExternalPrecondition(
        ExternalPrecondition::new(volume).expect("external volume"),
    ));
    let invalid_outcome = plan_deployment(&invalid);
    let finding = invalid_outcome
        .findings()
        .iter()
        .find(|finding| finding.field() == Some("infra_mounts"))
        .expect("duplicate mount finding");
    assert_eq!(finding.code().as_str(), "PLN0035");
    assert_eq!(finding.occurrence(), Some(2));
}

fn complete_pod_intent(version: &str) -> DeploymentIntent {
    let network = id(ResourceKind::Network, "application-network");
    let volume = id(ResourceKind::Volume, "application-data");
    let secret = id(ResourceKind::Secret, "application-password");
    let image = id(ResourceKind::Image, "registry.example.invalid/application:1.0");
    let pod = id(ResourceKind::Pod, "application");
    let container = id(ResourceKind::Container, "application-web");

    let mut pod_intent = PodIntent::new(pod).expect("pod identity");
    pod_intent
        .add_network(NetworkAttachment::new(network.clone()).expect("network attachment"))
        .expect("network identity");
    pod_intent.add_member(container.clone()).expect("container identity");

    let mut container_intent = ContainerIntent::new(container, image.clone()).expect("container identity");
    container_intent
        .set_pod(pod_intent.identity().clone())
        .expect("pod identity");
    container_intent.add_mount(mount(volume.clone(), "/var/lib/application"));
    container_intent.add_secret_grant(SecretGrant::mount(secret.clone()).expect("secret identity"));

    let mut intent = DeploymentIntent::new(target(version));
    intent.add_resource(DeploymentResource::Container(container_intent));
    intent.add_resource(DeploymentResource::Pod(pod_intent));
    intent.add_resource(DeploymentResource::Image(
        ImageIntent::new(
            image,
            ImageSource::new("registry.example.invalid/application:1.0").expect("image source"),
            ImagePullPolicy::Missing,
        )
        .expect("strict image reference"),
    ));
    intent.add_resource(DeploymentResource::Secret(
        SecretIntent::new(
            secret,
            SensitiveInputReference::new("vault/application-password").expect("external reference"),
        )
        .expect("secret identity"),
    ));
    intent.add_resource(DeploymentResource::Volume(
        VolumeIntent::new(volume).expect("volume identity"),
    ));
    intent.add_resource(DeploymentResource::Network(
        NetworkIntent::new(network).expect("network identity"),
    ));
    intent
}

#[test]
fn supported_target_profiles_produce_the_same_semantic_plan() {
    for version in ["5.4.0", "5.5.0", "5.6.0", "5.7.0", "5.8.6", "6.0.0", "6.1.0"] {
        let outcome = plan_deployment(&complete_pod_intent(version));
        assert!(outcome.is_success(), "{version}: {:?}", outcome.findings());
        let plan = outcome.plan().expect("plan");
        assert_eq!(plan.target().podman_version().original(), version);
        assert_eq!(plan.operations().len(), 7);
        assert_eq!(
            plan.operations()
                .iter()
                .map(|operation| operation.id().action())
                .collect::<Vec<_>>(),
            vec![
                SemanticOperationAction::Create,
                SemanticOperationAction::Create,
                SemanticOperationAction::Create,
                SemanticOperationAction::EnsureImage,
                SemanticOperationAction::Create,
                SemanticOperationAction::Create,
                SemanticOperationAction::StartPod,
            ]
        );
        assert_eq!(plan.operations()[3].image_pull_policy(), Some(ImagePullPolicy::Missing));
        assert_eq!(plan.operations()[6].depends_on().len(), 2);
    }
}

#[test]
fn independent_resource_kinds_have_stable_order_and_shared_prerequisites_are_emitted_once() {
    let first = complete_pod_intent("6.1.0");
    let mut second = DeploymentIntent::new(target("6.1.0"));
    for resource in first.resources().iter().rev().cloned() {
        second.add_resource(resource);
    }
    let first_plan = plan_deployment(&first).plan().cloned().expect("first plan");
    let second_plan = plan_deployment(&second).plan().cloned().expect("second plan");
    assert_eq!(first_plan, second_plan);
    assert_eq!(
        first_plan
            .operations()
            .iter()
            .filter(|operation| operation.id().resource().kind() == ResourceKind::Network)
            .count(),
        1
    );
}

#[test]
fn external_preconditions_are_explicit_and_emit_no_operation() {
    let image = id(ResourceKind::Image, "registry.example.invalid/web:1");
    let container = id(ResourceKind::Container, "web");
    let mut intent = DeploymentIntent::new(target("6.1.0"));
    intent.add_resource(DeploymentResource::ExternalPrecondition(
        ExternalPrecondition::new(image.clone()).expect("external image"),
    ));
    intent.add_resource(DeploymentResource::Container(
        ContainerIntent::new(container, image).expect("container"),
    ));
    let plan = plan_deployment(&intent).plan().cloned().expect("plan");
    assert_eq!(plan.operations().len(), 2);
    assert!(
        plan.operations()
            .iter()
            .all(|operation| operation.id().resource().kind() != ResourceKind::Image)
    );
    assert_eq!(
        plan.operations()[1].id().action(),
        SemanticOperationAction::StartContainer
    );
}

#[test]
fn semantic_plan_retains_managed_intent_and_every_external_precondition_without_secret_leaks() {
    let managed_image = id(ResourceKind::Image, "managed-web");
    let source = "registry.example.invalid/team/web:1.2.3";
    let secret = id(ResourceKind::Secret, "managed-secret");
    let mut intent = DeploymentIntent::new(target("6.1.0"));
    intent.add_resource(DeploymentResource::Image(
        ImageIntent::new(
            managed_image,
            ImageSource::new(source).expect("image source"),
            ImagePullPolicy::Missing,
        )
        .expect("managed image"),
    ));
    intent.add_resource(DeploymentResource::Secret(
        SecretIntent::new(
            secret,
            SensitiveInputReference::new("vault/production/web-password").expect("external material"),
        )
        .expect("managed secret"),
    ));
    for (kind, name) in [
        (ResourceKind::Network, "external-network"),
        (ResourceKind::Volume, "external-volume"),
        (ResourceKind::Image, "external-image"),
        (ResourceKind::Secret, "external-secret"),
    ] {
        intent.add_resource(DeploymentResource::ExternalPrecondition(
            ExternalPrecondition::new(id(kind, name)).expect("allowed external prerequisite"),
        ));
    }

    let plan = plan_deployment(&intent).plan().cloned().expect("plan");
    assert_eq!(
        plan.external_preconditions()
            .iter()
            .map(|precondition| precondition.identity().name())
            .collect::<Vec<_>>(),
        vec![
            "external-network",
            "external-volume",
            "external-image",
            "external-secret"
        ]
    );
    let image = plan
        .operations()
        .iter()
        .find_map(|operation| match operation.resource_intent() {
            DeploymentResource::Image(image) => Some(image),
            _ => None,
        })
        .expect("managed image operation");
    assert_eq!(image.source().as_str(), source);
    let secret = plan
        .operations()
        .iter()
        .find_map(|operation| match operation.resource_intent() {
            DeploymentResource::Secret(secret) => Some(secret),
            _ => None,
        })
        .expect("managed secret operation");
    assert_eq!(secret.material().as_str(), "vault/production/web-password");
    let debug = format!("{plan:?}");
    assert!(!debug.contains("vault/production/web-password"));
    assert!(debug.contains("SensitiveInputReference([redacted])"));
}

#[test]
fn missing_and_duplicate_or_conflicting_resources_are_structured_findings() {
    let image = id(ResourceKind::Image, "registry.example.invalid/web:1");
    let container = id(ResourceKind::Container, "web");
    let mut missing = DeploymentIntent::new(target("6.1.0"));
    missing.add_resource(DeploymentResource::Container(
        ContainerIntent::new(container, image).expect("container"),
    ));
    let missing = plan_deployment(&missing);
    assert_eq!(missing.findings()[0].code().as_str(), "PLN0037");

    let network = NetworkIntent::new(id(ResourceKind::Network, "network")).expect("network");
    let mut duplicate = DeploymentIntent::new(target("6.1.0"));
    duplicate.add_resource(DeploymentResource::Network(network.clone()));
    duplicate.add_resource(DeploymentResource::Network(network));
    let duplicate = plan_deployment(&duplicate);
    assert_eq!(duplicate.findings()[0].code().as_str(), "PLN0035");
    assert_eq!(duplicate.findings()[0].occurrence(), None);
    assert_eq!(duplicate.findings()[0].count(), Some(2));

    let same = id(ResourceKind::Network, "conflict");
    let mut conflict = DeploymentIntent::new(target("6.1.0"));
    conflict.add_resource(DeploymentResource::Network(
        NetworkIntent::new(same.clone()).expect("network"),
    ));
    conflict.add_resource(DeploymentResource::ExternalPrecondition(
        ExternalPrecondition::new(same).expect("external network"),
    ));
    let conflict = plan_deployment(&conflict);
    assert_eq!(conflict.findings()[0].code().as_str(), "PLN0036");
}

#[test]
fn pod_members_start_once_and_cross_pod_dependencies_are_lifted() {
    let image = id(ResourceKind::Image, "registry.example.invalid/app:1");
    let first_pod = id(ResourceKind::Pod, "first");
    let second_pod = id(ResourceKind::Pod, "second");
    let first = id(ResourceKind::Container, "first-member");
    let second = id(ResourceKind::Container, "second-member");
    let mut first_pod_intent = PodIntent::new(first_pod.clone()).expect("pod");
    first_pod_intent.add_member(first.clone()).expect("member");
    let mut second_pod_intent = PodIntent::new(second_pod.clone()).expect("pod");
    second_pod_intent.add_member(second.clone()).expect("member");
    let mut first_container = ContainerIntent::new(first.clone(), image.clone()).expect("container");
    first_container.set_pod(first_pod.clone()).expect("pod");
    let mut second_container = ContainerIntent::new(second.clone(), image.clone()).expect("container");
    second_container.set_pod(second_pod.clone()).expect("pod");
    let mut intent = DeploymentIntent::new(target("6.1.0"));
    intent.add_resource(DeploymentResource::Image(
        ImageIntent::new(
            image,
            ImageSource::new("registry.example.invalid/app:1").expect("image source"),
            ImagePullPolicy::Missing,
        )
        .expect("image"),
    ));
    intent.add_resource(DeploymentResource::Pod(first_pod_intent));
    intent.add_resource(DeploymentResource::Pod(second_pod_intent));
    intent.add_resource(DeploymentResource::Container(first_container));
    intent.add_resource(DeploymentResource::Container(second_container));
    intent.add_startup_dependency(StartupDependency::new(first.clone(), second.clone()).expect("dependency"));
    let plan = plan_deployment(&intent).plan().cloned().expect("plan");
    assert_eq!(
        plan.operations()
            .iter()
            .filter(|operation| operation.id().action() == SemanticOperationAction::StartPod)
            .count(),
        2
    );
    assert!(
        plan.operations()
            .iter()
            .all(|operation| operation.id().action() != SemanticOperationAction::StartContainer)
    );
    let second_start = plan
        .operations()
        .iter()
        .find(|operation| {
            operation.id().resource() == &second_pod && operation.id().action() == SemanticOperationAction::StartPod
        })
        .expect("second pod start");
    assert!(
        second_start
            .depends_on()
            .iter()
            .any(|dependency| dependency.resource() == &first_pod
                && dependency.action() == SemanticOperationAction::StartPod)
    );
    assert!(matches!(second_start.resource_intent(), DeploymentResource::Pod(_)));
}

#[test]
fn same_pod_order_and_start_cycles_are_rejected() {
    let mut same_pod = complete_pod_intent("6.1.0");
    let member = id(ResourceKind::Container, "application-web");
    same_pod.add_startup_dependency(StartupDependency::new(member.clone(), member).expect("dependency"));
    let same_pod = plan_deployment(&same_pod);
    assert_eq!(same_pod.findings()[0].code().as_str(), "PLN0044");

    let image = id(ResourceKind::Image, "registry.example.invalid/app:1");
    let left = id(ResourceKind::Container, "left");
    let right = id(ResourceKind::Container, "right");
    let mut cycle = DeploymentIntent::new(target("6.1.0"));
    cycle.add_resource(DeploymentResource::Image(
        ImageIntent::new(
            image.clone(),
            ImageSource::new("registry.example.invalid/app:1").expect("image source"),
            ImagePullPolicy::Missing,
        )
        .expect("image"),
    ));
    cycle.add_resource(DeploymentResource::Container(
        ContainerIntent::new(left.clone(), image.clone()).expect("left"),
    ));
    cycle.add_resource(DeploymentResource::Container(
        ContainerIntent::new(right.clone(), image).expect("right"),
    ));
    cycle.add_startup_dependency(StartupDependency::new(left.clone(), right.clone()).expect("edge"));
    cycle.add_startup_dependency(StartupDependency::new(right, left).expect("edge"));
    let cycle = plan_deployment(&cycle);
    assert_eq!(cycle.findings()[0].code().as_str(), "PLN0039");
}

#[test]
fn incomplete_pod_membership_is_rejected_before_operation_ordering() {
    let image = id(ResourceKind::Image, "registry.example.invalid/app:1");
    let pod = id(ResourceKind::Pod, "application");
    let container = id(ResourceKind::Container, "application-member");
    let mut member = ContainerIntent::new(container.clone(), image.clone()).expect("container");
    member.set_pod(pod.clone()).expect("pod");
    let mut intent = DeploymentIntent::new(target("6.1.0"));
    intent.add_resource(DeploymentResource::Image(
        ImageIntent::new(
            image,
            ImageSource::new("registry.example.invalid/app:1").expect("image source"),
            ImagePullPolicy::Missing,
        )
        .expect("image"),
    ));
    intent.add_resource(DeploymentResource::Pod(PodIntent::new(pod.clone()).expect("pod")));
    intent.add_resource(DeploymentResource::Container(member));
    let outcome = plan_deployment(&intent);
    assert!(!outcome.is_success());
    let finding = outcome
        .findings()
        .iter()
        .find(|finding| finding.code().as_str() == "PLN0043")
        .expect("pod membership finding");
    assert_eq!(finding.subject(), Some(&container));
    assert_eq!(finding.related(), &[pod]);
    assert_eq!(finding.field(), Some("pod"));
}

#[test]
fn constructors_reject_wrong_kinds_invalid_images_and_sensitive_payload_spelling() {
    assert_eq!(
        DeploymentResourceId::new(ResourceKind::Network, "")
            .expect_err("empty name")
            .code()
            .as_str(),
        "PLN0034"
    );
    assert_eq!(
        NetworkIntent::new(id(ResourceKind::Volume, "not-a-network"))
            .expect_err("wrong kind")
            .code()
            .as_str(),
        "PLN0034"
    );
    assert_eq!(
        ContainerIntent::new(
            id(ResourceKind::Container, "wrong-image"),
            id(ResourceKind::Network, "not-an-image"),
        )
        .expect_err("wrong prerequisite kind")
        .code()
        .as_str(),
        "PLN0034"
    );
    assert_eq!(
        ExternalPrecondition::new(id(ResourceKind::Pod, "not-external"))
            .expect_err("pods need managed membership")
            .code()
            .as_str(),
        "PLN0042"
    );
    assert_eq!(
        PodIntent::new(id(ResourceKind::Network, "not-a-pod"))
            .expect_err("wrong pod kind")
            .code()
            .as_str(),
        "PLN0034"
    );
    assert_eq!(
        VolumeIntent::new(id(ResourceKind::Network, "not-a-volume"))
            .expect_err("wrong volume kind")
            .code()
            .as_str(),
        "PLN0034"
    );
    assert_eq!(
        SecretIntent::new(
            id(ResourceKind::Network, "not-a-secret"),
            SensitiveInputReference::new("vault/test").expect("reference"),
        )
        .expect_err("wrong secret kind")
        .code()
        .as_str(),
        "PLN0034"
    );
    assert_eq!(
        ImageIntent::new(
            id(ResourceKind::Network, "not-an-image"),
            ImageSource::new("registry.example.invalid/team/image:1").expect("source"),
            ImagePullPolicy::Missing,
        )
        .expect_err("wrong image kind")
        .code()
        .as_str(),
        "PLN0034"
    );
    assert_eq!(
        SensitiveInputReference::new("literal:not-safe")
            .expect_err("literal payload")
            .code()
            .as_str(),
        "PLN0040"
    );
    assert_eq!(
        ImageSource::new("registry.example.invalid/app:1 $(whoami)")
            .expect_err("shell syntax is not an image reference")
            .code()
            .as_str(),
        "PLN0041"
    );
    let sensitive = SensitiveInputReference::new("vault/test-value").expect("reference");
    assert!(!format!("{sensitive:?}").contains("vault/test-value"));
}

#[test]
fn validation_collects_independent_missing_resources_and_startup_errors() {
    let container = id(ResourceKind::Container, "broken");
    let image = id(ResourceKind::Image, "registry.example.invalid/broken:1");
    let pod = id(ResourceKind::Pod, "missing-pod");
    let network = id(ResourceKind::Network, "missing-network");
    let volume = id(ResourceKind::Volume, "missing-volume");
    let secret = id(ResourceKind::Secret, "missing-secret");
    let missing_container = id(ResourceKind::Container, "missing-container");
    let mut broken = ContainerIntent::new(container.clone(), image).expect("container");
    broken.set_pod(pod.clone()).expect("pod identity");
    broken
        .add_network(NetworkAttachment::new(network).expect("network attachment"))
        .expect("network identity");
    broken.add_mount(mount(volume, "/var/lib/application"));
    broken.add_secret_grant(SecretGrant::mount(secret).expect("secret identity"));
    let mut intent = DeploymentIntent::new(target("6.1.0"));
    intent.add_resource(DeploymentResource::Container(broken));
    intent.add_startup_dependency(StartupDependency::new(container.clone(), missing_container).expect("edge"));
    let outcome = plan_deployment(&intent);
    assert!(!outcome.is_success());
    assert!(
        outcome
            .findings()
            .iter()
            .any(|finding| finding.code().as_str() == "PLN0038" && finding.field() == Some("networks"))
    );
    assert!(outcome.findings().len() >= 6, "{:?}", outcome.findings());
    assert!(
        outcome
            .findings()
            .iter()
            .any(|finding| finding.field() == Some("networks") && !finding.related().is_empty())
    );
    assert!(outcome.findings().iter().any(|finding| {
        finding.field() == Some("pod") && finding.subject() == Some(&container) && finding.related() == [pod.clone()]
    }));
    assert!(
        outcome
            .findings()
            .iter()
            .any(|finding| finding.field() == Some("startup_dependencies") && finding.occurrence() == Some(1))
    );
    assert!(outcome.findings().iter().all(|finding| !finding.message().is_empty()));
}

#[test]
fn pod_assignment_rejects_duplicate_and_conflicting_second_values_without_overwrite() {
    let container = id(ResourceKind::Container, "web");
    let image = id(ResourceKind::Image, "registry.example.invalid/web:1");
    let first = id(ResourceKind::Pod, "first");
    let second = id(ResourceKind::Pod, "second");
    let mut intent = ContainerIntent::new(container, image).expect("container");
    intent.set_pod(first.clone()).expect("first pod");
    assert_eq!(
        intent.set_pod(first.clone()).expect_err("duplicate").code().as_str(),
        "PLN0035"
    );
    assert_eq!(intent.set_pod(second).expect_err("conflict").code().as_str(), "PLN0038");
    assert_eq!(intent.pod(), Some(&first));
}

#[test]
fn invalid_declaration_permutations_produce_identical_sorted_findings() {
    let network = id(ResourceKind::Network, "conflict");
    let image = id(ResourceKind::Image, "registry.example.invalid/missing:1");
    let container = id(ResourceKind::Container, "broken");
    let resources = vec![
        DeploymentResource::Network(NetworkIntent::new(network.clone()).expect("network")),
        DeploymentResource::ExternalPrecondition(ExternalPrecondition::new(network).expect("external network")),
        DeploymentResource::Container(ContainerIntent::new(container, image).expect("container")),
    ];
    let mut first = DeploymentIntent::new(target("6.1.0"));
    let mut second = DeploymentIntent::new(target("6.1.0"));
    for resource in &resources {
        first.add_resource(resource.clone());
    }
    for resource in resources.into_iter().rev() {
        second.add_resource(resource);
    }
    assert_eq!(plan_deployment(&first).findings(), plan_deployment(&second).findings());
}

#[test]
fn infra_container_mounts_support_managed_external_and_duplicate_boundaries() {
    let pod = id(ResourceKind::Pod, "application");
    let volume = id(ResourceKind::Volume, "application-data");
    let mut managed_pod = PodIntent::new(pod.clone()).expect("pod");
    managed_pod.add_infra_mount(mount(volume.clone(), "/data"));
    assert_eq!(managed_pod.infra_mounts().len(), 1);
    assert_eq!(managed_pod.infra_mounts()[0].destination().as_str(), "/data");
    let mut managed = DeploymentIntent::new(target("6.1.0"));
    managed.add_resource(DeploymentResource::Pod(managed_pod));
    managed.add_resource(DeploymentResource::Volume(
        VolumeIntent::new(volume.clone()).expect("volume"),
    ));
    let managed_plan = plan_deployment(&managed).plan().cloned().expect("managed plan");
    assert_eq!(managed_plan.operations()[0].id().resource(), &volume);

    let mut external_pod = PodIntent::new(pod).expect("pod");
    external_pod.add_infra_mount(mount(volume.clone(), "/data"));
    let mut external = DeploymentIntent::new(target("6.1.0"));
    external.add_resource(DeploymentResource::Pod(external_pod));
    external.add_resource(DeploymentResource::ExternalPrecondition(
        ExternalPrecondition::new(volume.clone()).expect("external volume"),
    ));
    assert_eq!(
        plan_deployment(&external)
            .plan()
            .expect("external plan")
            .operations()
            .len(),
        1
    );

    let mut duplicate_pod = PodIntent::new(id(ResourceKind::Pod, "duplicate")).expect("pod");
    duplicate_pod.add_infra_mount(mount(volume.clone(), "/data"));
    duplicate_pod.add_infra_mount(mount(volume, "/data"));
    let mut duplicate = DeploymentIntent::new(target("6.1.0"));
    duplicate.add_resource(DeploymentResource::Pod(duplicate_pod));
    assert_eq!(plan_deployment(&duplicate).findings()[0].code().as_str(), "PLN0035");
}

#[test]
fn two_distinct_members_of_one_pod_cannot_have_a_startup_order() {
    let image = id(ResourceKind::Image, "registry.example.invalid/app:1");
    let pod = id(ResourceKind::Pod, "application");
    let left = id(ResourceKind::Container, "left");
    let right = id(ResourceKind::Container, "right");
    let mut pod_intent = PodIntent::new(pod.clone()).expect("pod");
    pod_intent.add_member(left.clone()).expect("left member");
    pod_intent.add_member(right.clone()).expect("right member");
    let mut left_intent = ContainerIntent::new(left.clone(), image.clone()).expect("left");
    left_intent.set_pod(pod.clone()).expect("pod");
    let mut right_intent = ContainerIntent::new(right.clone(), image.clone()).expect("right");
    right_intent.set_pod(pod).expect("pod");
    let mut intent = DeploymentIntent::new(target("6.1.0"));
    intent.add_resource(DeploymentResource::Image(
        ImageIntent::new(
            image,
            ImageSource::new("registry.example.invalid/app:1").expect("image source"),
            ImagePullPolicy::Missing,
        )
        .expect("image"),
    ));
    intent.add_resource(DeploymentResource::Pod(pod_intent));
    intent.add_resource(DeploymentResource::Container(left_intent));
    intent.add_resource(DeploymentResource::Container(right_intent));
    intent.add_startup_dependency(StartupDependency::new(left.clone(), right.clone()).expect("edge"));
    let outcome = plan_deployment(&intent);
    let finding = outcome
        .findings()
        .iter()
        .find(|finding| finding.code().as_str() == "PLN0044")
        .expect("same-pod finding");
    assert_eq!(finding.subject(), Some(&right));
    assert_eq!(finding.related(), &[left]);
    assert_eq!(finding.occurrence(), Some(1));
}

#[test]
fn image_source_grammar_classifies_safe_spellings_without_rewriting() {
    let digest = format!("registry.example.invalid/team/app@sha256:{}", "a".repeat(64));
    for source in [
        "registry.example.invalid/team/app:1.2.3".to_owned(),
        "registry.example.invalid:5000/team/app:stable".to_owned(),
        digest,
    ] {
        let image = ImageIntent::new(
            id(ResourceKind::Image, &source),
            ImageSource::new(&source).expect("portable source"),
            ImagePullPolicy::Missing,
        )
        .expect("image");
        assert_eq!(image.source().as_str(), source);
    }
    for source in [
        "registry.example.invalid/team/app:-tag",
        "Registry.example.invalid/team/app:1",
        "registry_example.invalid/team/app:1",
        "registry.example.invalid/Team/app:1",
        "registry.example.invalid/team/app@sha256:abc",
        "registry.example.invalid/team/app:tag@sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
    ] {
        assert_eq!(ImageSource::new(source).expect_err(source).code().as_str(), "PLN0041");
    }
}

#[test]
fn b4_typed_mounts_volume_ownership_and_secret_grants_preserve_all_optional_states() {
    use podman_lens::{BindMount, EnvironmentName, SecretGrant, SecretMode, TmpfsMount, UnixId, VolumeSubpath};

    let volume = id(ResourceKind::Volume, "data");
    let secret = id(ResourceKind::Secret, "credential");
    let image = id(ResourceKind::Image, "registry.example.invalid/app:1");
    let container = id(ResourceKind::Container, "app");
    let mut volume_intent = VolumeIntent::new(volume.clone()).expect("volume");
    volume_intent.set_uid(UnixId::new(0).expect("zero uid")).expect("uid");
    volume_intent
        .set_gid(UnixId::new(i32::MAX as u32).expect("max gid"))
        .expect("gid");
    assert_eq!(volume_intent.uid().expect("uid").get(), 0);
    assert_eq!(volume_intent.gid().expect("gid").get(), i32::MAX as u32);
    assert!(UnixId::new(i32::MAX as u32 + 1).is_err());

    let mut named = NamedVolumeMount::new(
        volume.clone(),
        AbsoluteContainerPath::new("/data").expect("destination"),
        MountAccess::ReadOnly,
        NamedVolumeCopyMode::Copy,
    )
    .expect("named mount");
    named
        .set_subpath(VolumeSubpath::new("/application/state").expect("subpath"))
        .expect("subpath");
    assert!(VolumeSubpath::new("relative").is_err());
    assert!(VolumeSubpath::new("/state/../escape").is_err());
    let mut no_copy = NamedVolumeMount::new(
        volume.clone(),
        AbsoluteContainerPath::new("/other").expect("destination"),
        MountAccess::ReadWrite,
        NamedVolumeCopyMode::NoCopy,
    )
    .expect("named mount");
    assert!(
        no_copy
            .set_subpath(VolumeSubpath::new("/state").expect("subpath"))
            .is_err()
    );

    let mut mount_grant = SecretGrant::mount(secret.clone()).expect("secret mount");
    mount_grant
        .set_mount_target(AbsoluteContainerPath::new("/run/secrets/credential").expect("target"))
        .expect("target");
    mount_grant.set_mount_uid(UnixId::new(0).expect("uid")).expect("uid");
    mount_grant.set_mount_gid(UnixId::new(1).expect("gid")).expect("gid");
    mount_grant
        .set_mount_mode(SecretMode::new(0o440).expect("mode"))
        .expect("mode");
    assert!(SecretMode::new(0o1000).is_err());
    let env_grant = SecretGrant::environment(secret.clone(), EnvironmentName::new("DATABASE_PASSWORD").expect("name"))
        .expect("secret environment");
    assert!(env_grant.mount_target().is_none());

    let mut intent = ContainerIntent::new(container.clone(), image).expect("container");
    intent.add_mount(MountIntent::NamedVolume(named));
    intent.add_mount(BindMount::new(
        AbsoluteContainerPath::new("/srv/application").expect("source"),
        AbsoluteContainerPath::new("/application").expect("destination"),
        MountAccess::ReadWrite,
    ));
    intent.add_mount(TmpfsMount::new(
        AbsoluteContainerPath::new("/run/cache").expect("destination"),
        MountAccess::ReadOnly,
    ));
    intent.add_secret_grant(mount_grant);
    intent.add_secret_grant(env_grant);
    assert_eq!(intent.mounts().len(), 3);
    assert_eq!(intent.secret_grants().len(), 2);
}

#[test]
fn image_source_classification_requires_explicit_policy_and_preserves_manual_boundaries() {
    use podman_lens::ImageSourceClassification;

    assert_eq!(
        ImageSource::new("registry.example.invalid/app:1")
            .expect("portable")
            .classification(),
        ImageSourceClassification::Portable
    );
    assert_eq!(
        ImageSource::new("localhost/app:1").expect("local").classification(),
        ImageSourceClassification::Local
    );
    assert_eq!(
        ImageSource::new("app:1").expect("unqualified").classification(),
        ImageSourceClassification::Unqualified
    );
    assert_eq!(
        ImageSource::new("registry.example.invalid/app")
            .expect("tagless")
            .classification(),
        ImageSourceClassification::Tagless
    );
    for policy in [
        ImagePullPolicy::Always,
        ImagePullPolicy::Missing,
        ImagePullPolicy::Never,
        ImagePullPolicy::Newer,
    ] {
        assert!(
            ImageIntent::new(
                id(ResourceKind::Image, "registry.example.invalid/app:1"),
                ImageSource::new("registry.example.invalid/app:1").expect("source"),
                policy,
            )
            .is_ok()
        );
    }
}

#[test]
fn public_constructors_reject_wrong_kinds_and_invalid_non_sensitive_references() {
    for name in ["production", "remote-one", "edge_2.1", "7"] {
        assert_eq!(
            DeploymentConnectionReference::new(name)
                .expect("safe connection name")
                .as_str(),
            name
        );
    }
    for name in [
        "",
        "-leading-hyphen",
        "remote one",
        "bad\nconnection",
        "ssh://user@example.invalid/run/user/1000/podman/podman.sock",
        "unix:///run/user/1000/podman/podman.sock",
        "tcp://example.invalid:8080",
        "remote:5000",
        "/run/user/1000/podman/podman.sock",
        r"C:\\podman\\podman.sock",
        "user@example.invalid",
        "credential=secret-token",
        &"a".repeat(65),
    ] {
        assert_eq!(
            DeploymentConnectionReference::new(name)
                .expect_err("unsafe connection detail")
                .code()
                .as_str(),
            "PLN0034",
            "{name}"
        );
    }
    for reference in [
        "",
        "bad\nreference",
        "literal:value",
        "plaintext:value",
        "base64:value",
        "LITERAL:value",
        "PlainText:value",
        "BASE64:value",
    ] {
        assert!(SensitiveInputReference::new(reference).is_err(), "{reference}");
    }
    assert_eq!(
        ExternalPrecondition::new(id(ResourceKind::Pod, "pod"))
            .expect_err("pod")
            .code()
            .as_str(),
        "PLN0042"
    );
    assert_eq!(
        ExternalPrecondition::new(id(ResourceKind::Container, "container"))
            .expect_err("container")
            .code()
            .as_str(),
        "PLN0042"
    );
    let mut pod = PodIntent::new(id(ResourceKind::Pod, "pod")).expect("pod");
    assert!(NetworkAttachment::new(id(ResourceKind::Volume, "wrong")).is_err());
    assert!(
        NamedVolumeMount::new(
            id(ResourceKind::Network, "wrong"),
            AbsoluteContainerPath::new("/data").expect("path"),
            MountAccess::ReadWrite,
            NamedVolumeCopyMode::Copy,
        )
        .is_err()
    );
    assert!(pod.add_member(id(ResourceKind::Pod, "wrong")).is_err());
    assert!(NetworkAttachment::new(id(ResourceKind::Volume, "wrong")).is_err());
    assert!(SecretGrant::mount(id(ResourceKind::Volume, "wrong")).is_err());
    assert!(StartupDependency::new(id(ResourceKind::Pod, "wrong"), id(ResourceKind::Container, "container")).is_err());
}

#[test]
fn duplicate_startup_dependencies_report_the_second_edge_position() {
    let image = id(ResourceKind::Image, "registry.example.invalid/team/app:1");
    let first = id(ResourceKind::Container, "first");
    let second = id(ResourceKind::Container, "second");
    let dependency = StartupDependency::new(first.clone(), second.clone()).expect("dependency");
    let mut intent = DeploymentIntent::new(target("6.1.0"));
    intent.add_resource(DeploymentResource::Image(
        ImageIntent::new(
            image.clone(),
            ImageSource::new("registry.example.invalid/team/app:1").expect("image source"),
            ImagePullPolicy::Missing,
        )
        .expect("image"),
    ));
    intent.add_resource(DeploymentResource::Container(
        ContainerIntent::new(first, image.clone()).expect("first container"),
    ));
    intent.add_resource(DeploymentResource::Container(
        ContainerIntent::new(second.clone(), image).expect("second container"),
    ));
    intent.add_startup_dependency(dependency.clone());
    intent.add_startup_dependency(dependency);
    let outcome = plan_deployment(&intent);
    let finding = outcome
        .findings()
        .iter()
        .find(|finding| finding.code().as_str() == "PLN0035" && finding.field() == Some("startup_dependencies"))
        .expect("duplicate startup finding");
    assert_eq!(finding.subject(), Some(&second));
    assert_eq!(finding.occurrence(), Some(2));
    assert_eq!(finding.count(), None);
}
