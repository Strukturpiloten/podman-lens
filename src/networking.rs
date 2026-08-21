//! Typed declared Podman networking output intent.
//!
//! These types describe caller-declared target configuration. They never represent runtime-assigned
//! addresses observed from an existing container or pod.

use std::net::IpAddr;

use crate::{DeploymentResourceId, Diagnostic, DiagnosticCode, PodmanLensResult, ResourceKind};

const MAX_ATTACHMENTS: usize = 32;
const MAX_ALIASES: usize = 64;
const MAX_DNS_VALUES: usize = 16;
const MAX_PORT_MAPPINGS: usize = 128;
const MAX_HOST_ALIASES: usize = 128;
const MAX_IPAM_SUBNETS: usize = 32;
const MAX_ROUTES: usize = 64;

/// A declared CIDR network, validated without consulting a runtime.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NetworkCidr(String);

impl NetworkCidr {
    /// Creates a validated canonical IPv4 or IPv6 CIDR.
    ///
    /// # Errors
    ///
    /// Returns `PLN0034` when the spelling is not a bounded IPv4 or IPv6 CIDR.
    pub fn new(value: impl Into<String>) -> PodmanLensResult<Self> {
        let value = value.into();
        let Some((address, prefix)) = value.split_once('/') else {
            return Err(Diagnostic::new(DiagnosticCode::InvalidDeploymentIntent));
        };
        let Ok(address) = address.parse::<IpAddr>() else {
            return Err(Diagnostic::new(DiagnosticCode::InvalidDeploymentIntent));
        };
        let Ok(prefix) = prefix.parse::<u8>() else {
            return Err(Diagnostic::new(DiagnosticCode::InvalidDeploymentIntent));
        };
        if value.len() > 128 || prefix > if address.is_ipv4() { 32 } else { 128 } {
            return Err(Diagnostic::new(DiagnosticCode::InvalidDeploymentIntent));
        }
        let network = masked_address(address, prefix);
        Ok(Self(format!("{network}/{prefix}")))
    }

    /// Returns the canonical network-address and numeric-prefix spelling.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Returns whether `address` has the same address family as this CIDR.
    #[must_use]
    pub fn has_address_family(&self, address: IpAddr) -> bool {
        self.0.split_once('/').is_some_and(|(network, _)| {
            network
                .parse::<IpAddr>()
                .is_ok_and(|network| network.is_ipv4() == address.is_ipv4())
        })
    }

    /// Returns whether an address of the same family lies within this CIDR.
    ///
    /// This is lexical address arithmetic only. It does not inspect a host network or make any
    /// assumption about whether a particular address is available for allocation.
    #[must_use]
    pub fn contains(&self, address: IpAddr) -> bool {
        let Some((network, prefix)) = self.0.split_once('/') else {
            return false;
        };
        let Ok(prefix) = prefix.parse::<u8>() else {
            return false;
        };
        let Ok(network) = network.parse::<IpAddr>() else {
            return false;
        };
        if network.is_ipv4() != address.is_ipv4() {
            return false;
        }
        network == masked_address(address, prefix)
    }
}

/// A declared static MAC address for one network attachment.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StaticMacAddress(String);

impl StaticMacAddress {
    /// Creates a normalized lower-case six-octet MAC address.
    ///
    /// # Errors
    ///
    /// Returns `PLN0034` for an invalid MAC address.
    pub fn new(value: impl AsRef<str>) -> PodmanLensResult<Self> {
        let value = value.as_ref();
        if value.len() != 17
            || value.split(':').count() != 6
            || value
                .split(':')
                .any(|part| part.len() != 2 || u8::from_str_radix(part, 16).is_err())
        {
            return Err(Diagnostic::new(DiagnosticCode::InvalidDeploymentIntent));
        }
        Ok(Self(value.to_ascii_lowercase()))
    }

    /// Returns the normalized MAC address.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// One ordered attachment to a named Podman network.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NetworkAttachment {
    network: DeploymentResourceId,
    aliases: Vec<String>,
    static_ipv4: Option<IpAddr>,
    static_ipv6: Option<IpAddr>,
    static_mac: Option<StaticMacAddress>,
}

impl NetworkAttachment {
    /// Creates an attachment for one exact network resource.
    ///
    /// # Errors
    ///
    /// Returns `PLN0034` when `network` is not a network identity.
    pub fn new(network: DeploymentResourceId) -> PodmanLensResult<Self> {
        if network.kind() != ResourceKind::Network {
            return Err(Diagnostic::new(DiagnosticCode::InvalidDeploymentIntent));
        }
        Ok(Self {
            network,
            aliases: Vec::new(),
            static_ipv4: None,
            static_ipv6: None,
            static_mac: None,
        })
    }

    /// Adds one declared network alias in attachment order.
    ///
    /// # Errors
    ///
    /// Returns `PLN0034` for invalid aliases and `PLN0035` for duplicates or capacity exhaustion.
    pub fn add_alias(&mut self, alias: impl Into<String>) -> PodmanLensResult<()> {
        let alias = alias.into();
        if alias.is_empty()
            || alias.len() > 253
            || alias.chars().any(char::is_control)
            || alias.contains([',', ':', '='])
        {
            return Err(Diagnostic::new(DiagnosticCode::InvalidDeploymentIntent));
        }
        if self.aliases.len() == MAX_ALIASES || self.aliases.contains(&alias) {
            return Err(Diagnostic::new(DiagnosticCode::DeploymentDuplicateResource));
        }
        self.aliases.push(alias);
        Ok(())
    }

    /// Sets a declared static IPv4 address; it is never inferred from runtime inspection.
    ///
    /// # Errors
    ///
    /// Returns `PLN0038` when `address` is not IPv4 or the value is already set.
    pub fn set_static_ipv4(&mut self, address: IpAddr) -> PodmanLensResult<()> {
        if !address.is_ipv4() || self.static_ipv4.is_some() {
            return Err(Diagnostic::new(DiagnosticCode::DeploymentUnsupportedCombination));
        }
        self.static_ipv4 = Some(address);
        Ok(())
    }

    /// Sets a declared static IPv6 address; it is never inferred from runtime inspection.
    ///
    /// # Errors
    ///
    /// Returns `PLN0038` when `address` is not IPv6 or the value is already set.
    pub fn set_static_ipv6(&mut self, address: IpAddr) -> PodmanLensResult<()> {
        if !address.is_ipv6() || self.static_ipv6.is_some() {
            return Err(Diagnostic::new(DiagnosticCode::DeploymentUnsupportedCombination));
        }
        self.static_ipv6 = Some(address);
        Ok(())
    }

    /// Sets one declared static MAC address.
    ///
    /// # Errors
    ///
    /// Returns `PLN0038` when the value is already set.
    pub fn set_static_mac(&mut self, address: StaticMacAddress) -> PodmanLensResult<()> {
        if self.static_mac.is_some() {
            return Err(Diagnostic::new(DiagnosticCode::DeploymentUnsupportedCombination));
        }
        self.static_mac = Some(address);
        Ok(())
    }

    /// Returns the exact network identity.
    #[must_use]
    pub fn network(&self) -> &DeploymentResourceId {
        &self.network
    }

    /// Returns aliases in declaration order.
    #[must_use]
    pub fn aliases(&self) -> &[String] {
        &self.aliases
    }

    /// Returns the optional caller-declared static IPv4 address.
    #[must_use]
    pub const fn static_ipv4(&self) -> Option<IpAddr> {
        self.static_ipv4
    }

    /// Returns the optional caller-declared static IPv6 address.
    #[must_use]
    pub const fn static_ipv6(&self) -> Option<IpAddr> {
        self.static_ipv6
    }

    /// Returns the optional caller-declared static MAC address.
    #[must_use]
    pub fn static_mac(&self) -> Option<&StaticMacAddress> {
        self.static_mac.as_ref()
    }
}

/// IP protocol used by a declared host-to-container port mapping.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum PortProtocol {
    /// TCP transport.
    Tcp,
    /// UDP transport.
    Udp,
    /// SCTP transport.
    Sctp,
}

/// One explicit host-to-container port mapping.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PortMapping {
    host_ip: Option<IpAddr>,
    host_port: u16,
    container_port: u16,
    protocol: PortProtocol,
}

impl PortMapping {
    /// Creates one nonzero port mapping.
    ///
    /// # Errors
    ///
    /// Returns `PLN0034` when either port is zero.
    pub fn new(
        host_ip: Option<IpAddr>,
        host_port: u16,
        container_port: u16,
        protocol: PortProtocol,
    ) -> PodmanLensResult<Self> {
        if host_port == 0 || container_port == 0 {
            return Err(Diagnostic::new(DiagnosticCode::InvalidDeploymentIntent));
        }
        Ok(Self {
            host_ip,
            host_port,
            container_port,
            protocol,
        })
    }

    /// Returns the optional declared host bind address.
    #[must_use]
    pub const fn host_ip(&self) -> Option<IpAddr> {
        self.host_ip
    }
    /// Returns the host port.
    #[must_use]
    pub const fn host_port(&self) -> u16 {
        self.host_port
    }
    /// Returns the container port.
    #[must_use]
    pub const fn container_port(&self) -> u16 {
        self.container_port
    }
    /// Returns the protocol.
    #[must_use]
    pub const fn protocol(&self) -> PortProtocol {
        self.protocol
    }
}

/// Declared DNS configuration, preserving source declaration order.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct DnsConfiguration {
    servers: Vec<IpAddr>,
    search: Vec<String>,
    options: Vec<String>,
}

impl DnsConfiguration {
    /// Adds one DNS server.
    ///
    /// # Errors
    ///
    /// Returns `PLN0035` for a duplicate server or capacity exhaustion.
    pub fn add_server(&mut self, server: IpAddr) -> PodmanLensResult<()> {
        add_distinct(&mut self.servers, server, MAX_DNS_VALUES)
    }
    /// Adds one DNS search domain.
    ///
    /// # Errors
    ///
    /// Returns `PLN0034` for an invalid domain and `PLN0035` for a duplicate or capacity exhaustion.
    pub fn add_search(&mut self, domain: impl Into<String>) -> PodmanLensResult<()> {
        add_text(&mut self.search, domain.into(), MAX_DNS_VALUES)
    }
    /// Adds one DNS resolver option.
    ///
    /// # Errors
    ///
    /// Returns `PLN0034` for an invalid option and `PLN0035` for a duplicate or capacity exhaustion.
    pub fn add_option(&mut self, option: impl Into<String>) -> PodmanLensResult<()> {
        add_text(&mut self.options, option.into(), MAX_DNS_VALUES)
    }
    /// Returns servers in declaration order.
    #[must_use]
    pub fn servers(&self) -> &[IpAddr] {
        &self.servers
    }
    /// Returns search domains in declaration order.
    #[must_use]
    pub fn search(&self) -> &[String] {
        &self.search
    }
    /// Returns resolver options in declaration order.
    #[must_use]
    pub fn options(&self) -> &[String] {
        &self.options
    }
}

/// One explicit `/etc/hosts` alias.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HostAlias {
    address: IpAddr,
    hostname: String,
}

impl HostAlias {
    /// Creates a bounded declared host alias.
    ///
    /// # Errors
    ///
    /// Returns `PLN0034` for an invalid host name.
    pub fn new(address: IpAddr, hostname: impl Into<String>) -> PodmanLensResult<Self> {
        let hostname = hostname.into();
        if !is_hostname(&hostname) {
            return Err(Diagnostic::new(DiagnosticCode::InvalidDeploymentIntent));
        }
        Ok(Self { address, hostname })
    }
    /// Returns the declared address.
    #[must_use]
    pub const fn address(&self) -> IpAddr {
        self.address
    }
    /// Returns the declared host name.
    #[must_use]
    pub fn hostname(&self) -> &str {
        &self.hostname
    }
}

/// IPAM subnet declaration for one managed network.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NetworkSubnet {
    subnet: NetworkCidr,
    gateway: Option<IpAddr>,
    range: Option<(IpAddr, IpAddr)>,
}

impl NetworkSubnet {
    /// Creates one subnet declaration.
    #[must_use]
    pub const fn new(subnet: NetworkCidr) -> Self {
        Self {
            subnet,
            gateway: None,
            range: None,
        }
    }
    /// Sets one gateway address.
    ///
    /// # Errors
    ///
    /// Returns `PLN0038` when the family does not match the subnet or a gateway is already set.
    pub fn set_gateway(&mut self, gateway: IpAddr) -> PodmanLensResult<()> {
        if !self.subnet.contains(gateway) || self.gateway.is_some() {
            return Err(Diagnostic::new(DiagnosticCode::DeploymentUnsupportedCombination));
        }
        self.gateway = Some(gateway);
        Ok(())
    }
    /// Sets an inclusive declared allocation range.
    ///
    /// # Errors
    ///
    /// Returns `PLN0038` when either address family differs from the subnet or a range is already set.
    pub fn set_range(&mut self, start: IpAddr, end: IpAddr) -> PodmanLensResult<()> {
        if !self.subnet.contains(start)
            || !self.subnet.contains(end)
            || !address_precedes_or_equals(start, end)
            || self.range.is_some()
        {
            return Err(Diagnostic::new(DiagnosticCode::DeploymentUnsupportedCombination));
        }
        self.range = Some((start, end));
        Ok(())
    }
    /// Returns the subnet.
    #[must_use]
    pub fn subnet(&self) -> &NetworkCidr {
        &self.subnet
    }
    /// Returns the optional gateway.
    #[must_use]
    pub const fn gateway(&self) -> Option<IpAddr> {
        self.gateway
    }
    /// Returns the optional allocation range.
    #[must_use]
    pub const fn range(&self) -> Option<(IpAddr, IpAddr)> {
        self.range
    }
}

/// Route kind for a managed network.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum RouteType {
    /// A route that forwards traffic through a declared gateway.
    Unicast,
    /// A route that drops matching traffic.
    Blackhole,
    /// A route that reports the destination as unreachable.
    Unreachable,
    /// A route that reports the destination as administratively prohibited.
    Prohibit,
}

/// One static network route declaration.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NetworkRoute {
    destination: NetworkCidr,
    gateway: Option<IpAddr>,
    route_type: RouteType,
    metric: Option<u32>,
}

impl NetworkRoute {
    /// Creates a route with the gateway required by a unicast route.
    ///
    /// # Errors
    ///
    /// Returns `PLN0034` unless a unicast route has a same-family gateway and a non-unicast route has none.
    pub fn new(destination: NetworkCidr, gateway: Option<IpAddr>, route_type: RouteType) -> PodmanLensResult<Self> {
        let valid_gateway = match route_type {
            RouteType::Unicast => gateway.is_some_and(|gateway| destination.has_address_family(gateway)),
            RouteType::Blackhole | RouteType::Unreachable | RouteType::Prohibit => gateway.is_none(),
        };
        if !valid_gateway {
            return Err(Diagnostic::new(DiagnosticCode::InvalidDeploymentIntent));
        }
        Ok(Self {
            destination,
            gateway,
            route_type,
            metric: None,
        })
    }

    /// Sets an explicit native route metric.
    ///
    /// # Errors
    ///
    /// Returns `PLN0038` when a metric is already set.
    pub fn set_metric(&mut self, metric: u32) -> PodmanLensResult<()> {
        if self.metric.is_some() {
            return Err(Diagnostic::new(DiagnosticCode::DeploymentUnsupportedCombination));
        }
        self.metric = Some(metric);
        Ok(())
    }
    /// Returns the route destination.
    #[must_use]
    pub fn destination(&self) -> &NetworkCidr {
        &self.destination
    }
    /// Returns the optional gateway.
    #[must_use]
    pub const fn gateway(&self) -> Option<IpAddr> {
        self.gateway
    }
    /// Returns the route type.
    #[must_use]
    pub const fn route_type(&self) -> RouteType {
        self.route_type
    }

    /// Returns the caller-declared native route metric, if any.
    #[must_use]
    pub const fn metric(&self) -> Option<u32> {
        self.metric
    }
}

pub(crate) fn add_attachment(
    values: &mut Vec<NetworkAttachment>,
    attachment: NetworkAttachment,
) -> PodmanLensResult<()> {
    if values.len() == MAX_ATTACHMENTS || values.iter().any(|existing| existing.network == attachment.network) {
        return Err(Diagnostic::new(DiagnosticCode::DeploymentDuplicateResource));
    }
    values.push(attachment);
    Ok(())
}
pub(crate) fn add_port(values: &mut Vec<PortMapping>, mapping: PortMapping) -> PodmanLensResult<()> {
    if values.len() == MAX_PORT_MAPPINGS
        || values.iter().any(|existing| {
            existing.host_ip == mapping.host_ip
                && existing.host_port == mapping.host_port
                && existing.protocol == mapping.protocol
        })
    {
        return Err(Diagnostic::new(DiagnosticCode::DeploymentDuplicateResource));
    }
    values.push(mapping);
    Ok(())
}

pub(crate) fn add_host(values: &mut Vec<HostAlias>, alias: HostAlias) -> PodmanLensResult<()> {
    add_distinct(values, alias, MAX_HOST_ALIASES)
}

pub(crate) fn add_subnet(values: &mut Vec<NetworkSubnet>, subnet: NetworkSubnet) -> PodmanLensResult<()> {
    if values.len() == MAX_IPAM_SUBNETS || values.iter().any(|existing| existing.subnet == subnet.subnet) {
        return Err(Diagnostic::new(DiagnosticCode::DeploymentDuplicateResource));
    }
    values.push(subnet);
    Ok(())
}

pub(crate) fn add_route(values: &mut Vec<NetworkRoute>, route: NetworkRoute) -> PodmanLensResult<()> {
    add_distinct(values, route, MAX_ROUTES)
}

fn add_distinct<T: Eq>(values: &mut Vec<T>, value: T, maximum: usize) -> PodmanLensResult<()> {
    if values.len() == maximum || values.contains(&value) {
        return Err(Diagnostic::new(DiagnosticCode::DeploymentDuplicateResource));
    }
    values.push(value);
    Ok(())
}

fn add_text(values: &mut Vec<String>, value: String, maximum: usize) -> PodmanLensResult<()> {
    if value.is_empty() || value.len() > 253 || value.chars().any(char::is_control) {
        return Err(Diagnostic::new(DiagnosticCode::InvalidDeploymentIntent));
    }
    add_distinct(values, value, maximum)
}

fn is_hostname(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 253
        && value.split('.').all(|label| {
            !label.is_empty()
                && label.len() <= 63
                && !label.starts_with('-')
                && !label.ends_with('-')
                && label.bytes().all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
        })
}

fn address_precedes_or_equals(start: IpAddr, end: IpAddr) -> bool {
    match (start, end) {
        (IpAddr::V4(start), IpAddr::V4(end)) => u32::from(start) <= u32::from(end),
        (IpAddr::V6(start), IpAddr::V6(end)) => u128::from(start) <= u128::from(end),
        _ => false,
    }
}

fn masked_address(address: IpAddr, prefix: u8) -> IpAddr {
    match address {
        IpAddr::V4(address) => {
            let mask = if prefix == 0 { 0 } else { u32::MAX << (32 - prefix) };
            IpAddr::V4(std::net::Ipv4Addr::from(u32::from(address) & mask))
        }
        IpAddr::V6(address) => {
            let mask = if prefix == 0 { 0 } else { u128::MAX << (128 - prefix) };
            IpAddr::V6(std::net::Ipv6Addr::from(u128::from(address) & mask))
        }
    }
}
