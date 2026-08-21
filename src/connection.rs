//! Explicit, transport-neutral Podman service connection specifications.

use std::{
    fmt,
    num::NonZeroU16,
    path::{Component, Path, PathBuf},
};

use url::Url;

use crate::{Diagnostic, DiagnosticCode, PodmanLensResult};

const SOCKADDR_UN_PATH_MAX_BYTES: usize = 107;

/// An opaque external reference such as a host key, certificate, or authentication material.
///
/// The reference identifies material owned by the caller; it never contains the material itself.
/// Its textual value is intentionally redacted from formatting implementations.
#[derive(Clone, Eq, PartialEq, Hash)]
pub struct OpaqueReference(String);

impl OpaqueReference {
    /// Validates and retains a non-empty external reference.
    ///
    /// # Errors
    ///
    /// Returns `PLN0001` when the reference is empty or contains a control character.
    pub fn new(reference: impl Into<String>) -> PodmanLensResult<Self> {
        let reference = reference.into();
        if reference.trim().is_empty() || contains_control_character(&reference) {
            return Err(Diagnostic::new(DiagnosticCode::InvalidConnection));
        }
        Ok(Self(reference))
    }

    /// Returns the caller-owned reference for a transport implementation.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for OpaqueReference {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("OpaqueReference([redacted])")
    }
}

impl fmt::Display for OpaqueReference {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("[redacted]")
    }
}

/// A validated local Unix-domain socket connection.
#[derive(Clone, Eq, PartialEq)]
pub struct UnixConnection {
    socket_path: PathBuf,
}

impl UnixConnection {
    /// Creates a Unix connection with an absolute socket path.
    ///
    /// # Errors
    ///
    /// Returns `PLN0001` when the path is not a safe absolute Unix socket path.
    pub fn new(socket_path: impl Into<PathBuf>) -> PodmanLensResult<Self> {
        let socket_path = socket_path.into();
        if !is_valid_unix_socket_path(&socket_path) {
            return Err(Diagnostic::new(DiagnosticCode::InvalidConnection));
        }
        Ok(Self { socket_path })
    }

    /// Parses an explicit `unix:///absolute/socket` endpoint spelling.
    ///
    /// # Errors
    ///
    /// Returns `PLN0001` when the endpoint is not an explicit safe Unix socket URI.
    pub fn parse(endpoint: &str) -> PodmanLensResult<Self> {
        let raw_path = raw_uri_path(endpoint, "unix")?;
        if !is_safe_raw_socket_path(raw_path) {
            return Err(Diagnostic::new(DiagnosticCode::InvalidConnection));
        }
        let parsed = Url::parse(endpoint).map_err(|_| Diagnostic::new(DiagnosticCode::InvalidConnection))?;
        if parsed.scheme() != "unix"
            || parsed.host_str().is_some()
            || parsed.query().is_some()
            || parsed.fragment().is_some()
            || parsed.path().contains('%')
        {
            return Err(Diagnostic::new(DiagnosticCode::InvalidConnection));
        }
        Self::new(parsed.path())
    }

    /// Returns the selected local socket path for a caller-provided Unix transport.
    #[must_use]
    pub fn socket_path(&self) -> &Path {
        &self.socket_path
    }
}

impl fmt::Debug for UnixConnection {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("UnixConnection { socket_path: [redacted] }")
    }
}

impl fmt::Display for UnixConnection {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("unix socket [redacted]")
    }
}

/// A validated SSH endpoint with explicit host verification and authentication references.
#[derive(Clone, Eq, PartialEq)]
pub struct SshConnection {
    host: String,
    port: NonZeroU16,
    user: String,
    remote_socket_path: PathBuf,
    host_key_reference: OpaqueReference,
    authentication_reference: OpaqueReference,
}

impl SshConnection {
    /// Creates an SSH endpoint with an absolute remote socket path and required verification.
    ///
    /// # Errors
    ///
    /// Returns `PLN0001` when an endpoint component, port, or remote socket path is invalid.
    pub fn new(
        host: impl Into<String>,
        port: u16,
        user: impl Into<String>,
        remote_socket_path: impl Into<PathBuf>,
        host_key_reference: OpaqueReference,
        authentication_reference: OpaqueReference,
    ) -> PodmanLensResult<Self> {
        let host = validate_endpoint_name(host.into())?;
        let user = validate_endpoint_name(user.into())?;
        let remote_socket_path = remote_socket_path.into();
        let port = NonZeroU16::new(port).ok_or_else(|| Diagnostic::new(DiagnosticCode::InvalidConnection))?;
        if !is_valid_unix_socket_path(&remote_socket_path) {
            return Err(Diagnostic::new(DiagnosticCode::InvalidConnection));
        }
        Ok(Self {
            host,
            port,
            user,
            remote_socket_path,
            host_key_reference,
            authentication_reference,
        })
    }

    /// Parses `ssh://user@host:port/absolute/socket` with separately supplied security material.
    ///
    /// # Errors
    ///
    /// Returns `PLN0001` when the endpoint omits an explicit secure SSH component.
    pub fn parse(
        endpoint: &str,
        host_key_reference: OpaqueReference,
        authentication_reference: OpaqueReference,
    ) -> PodmanLensResult<Self> {
        let raw_path = raw_uri_path(endpoint, "ssh")?;
        if !is_safe_raw_socket_path(raw_path) {
            return Err(Diagnostic::new(DiagnosticCode::InvalidConnection));
        }
        let parsed = Url::parse(endpoint).map_err(|_| Diagnostic::new(DiagnosticCode::InvalidConnection))?;
        if parsed.scheme() != "ssh"
            || parsed.username().is_empty()
            || parsed.password().is_some()
            || parsed.query().is_some()
            || parsed.fragment().is_some()
            || parsed.path().contains('%')
        {
            return Err(Diagnostic::new(DiagnosticCode::InvalidConnection));
        }
        let host = parsed
            .host_str()
            .ok_or_else(|| Diagnostic::new(DiagnosticCode::InvalidConnection))?;
        let port = parsed
            .port()
            .ok_or_else(|| Diagnostic::new(DiagnosticCode::InvalidConnection))?;
        Self::new(
            host,
            port,
            parsed.username(),
            parsed.path(),
            host_key_reference,
            authentication_reference,
        )
    }

    /// Returns the explicit host for a caller-provided SSH transport.
    #[must_use]
    pub fn host(&self) -> &str {
        &self.host
    }

    /// Returns the explicit non-zero SSH port.
    #[must_use]
    pub const fn port(&self) -> u16 {
        self.port.get()
    }

    /// Returns the selected remote user.
    #[must_use]
    pub fn user(&self) -> &str {
        &self.user
    }

    /// Returns the absolute remote socket path.
    #[must_use]
    pub fn remote_socket_path(&self) -> &Path {
        &self.remote_socket_path
    }

    /// Returns the required verified-host-key reference.
    #[must_use]
    pub fn host_key_reference(&self) -> &OpaqueReference {
        &self.host_key_reference
    }

    /// Returns the caller-owned authentication reference.
    #[must_use]
    pub fn authentication_reference(&self) -> &OpaqueReference {
        &self.authentication_reference
    }
}

impl fmt::Debug for SshConnection {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("SshConnection([redacted])")
    }
}

impl fmt::Display for SshConnection {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("SSH Podman connection [redacted]")
    }
}

/// Mandatory mutual-TLS material references for a TCP Podman endpoint.
#[derive(Clone, Eq, PartialEq)]
pub struct MutualTlsPolicy {
    server_name: String,
    certificate_authority_reference: OpaqueReference,
    client_certificate_reference: OpaqueReference,
    client_private_key_reference: OpaqueReference,
}

impl MutualTlsPolicy {
    /// Creates a mandatory mutual-TLS policy with hostname verification and client credentials.
    ///
    /// # Errors
    ///
    /// Returns `PLN0001` when the TLS server name is invalid.
    pub fn new(
        server_name: impl Into<String>,
        certificate_authority_reference: OpaqueReference,
        client_certificate_reference: OpaqueReference,
        client_private_key_reference: OpaqueReference,
    ) -> PodmanLensResult<Self> {
        Ok(Self {
            server_name: validate_endpoint_name(server_name.into())?,
            certificate_authority_reference,
            client_certificate_reference,
            client_private_key_reference,
        })
    }

    /// Returns the hostname that a caller-provided TLS transport must verify.
    #[must_use]
    pub fn server_name(&self) -> &str {
        &self.server_name
    }

    /// Returns the required certificate-authority reference.
    #[must_use]
    pub fn certificate_authority_reference(&self) -> &OpaqueReference {
        &self.certificate_authority_reference
    }

    /// Returns the required client-certificate reference.
    #[must_use]
    pub fn client_certificate_reference(&self) -> &OpaqueReference {
        &self.client_certificate_reference
    }

    /// Returns the required client-private-key reference.
    #[must_use]
    pub fn client_private_key_reference(&self) -> &OpaqueReference {
        &self.client_private_key_reference
    }
}

impl fmt::Debug for MutualTlsPolicy {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("MutualTlsPolicy([redacted])")
    }
}

impl fmt::Display for MutualTlsPolicy {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("mutual TLS [redacted]")
    }
}

/// A Podman-style TCP endpoint that is protected by mandatory mutual TLS.
#[derive(Clone, Eq, PartialEq)]
pub struct TcpMutualTlsConnection {
    host: String,
    port: NonZeroU16,
    policy: MutualTlsPolicy,
}

impl TcpMutualTlsConnection {
    /// Parses an explicit `tcp://host:port` endpoint and attaches mandatory mutual TLS policy.
    ///
    /// Plaintext TCP is intentionally not representable by this API.
    ///
    /// # Errors
    ///
    /// Returns `PLN0001` when the endpoint is not a host-and-port-only `tcp` URI.
    pub fn parse(endpoint: &str, policy: MutualTlsPolicy) -> PodmanLensResult<Self> {
        let parsed = Url::parse(endpoint).map_err(|_| Diagnostic::new(DiagnosticCode::InvalidConnection))?;
        if parsed.scheme() != "tcp"
            || !parsed.username().is_empty()
            || parsed.password().is_some()
            || parsed.query().is_some()
            || parsed.fragment().is_some()
            || !(parsed.path().is_empty() || parsed.path() == "/")
        {
            return Err(Diagnostic::new(DiagnosticCode::InvalidConnection));
        }
        let host = parsed
            .host_str()
            .ok_or_else(|| Diagnostic::new(DiagnosticCode::InvalidConnection))?;
        let host = validate_endpoint_name(host.to_owned())?;
        let port = parsed
            .port()
            .and_then(NonZeroU16::new)
            .ok_or_else(|| Diagnostic::new(DiagnosticCode::InvalidConnection))?;
        Ok(Self { host, port, policy })
    }

    /// Returns the explicit TCP host for a caller-provided mutual-TLS transport.
    #[must_use]
    pub fn host(&self) -> &str {
        &self.host
    }

    /// Returns the explicit non-zero TCP port.
    #[must_use]
    pub const fn port(&self) -> u16 {
        self.port.get()
    }

    /// Returns the mandatory mutual-TLS policy.
    #[must_use]
    pub fn policy(&self) -> &MutualTlsPolicy {
        &self.policy
    }
}

impl fmt::Debug for TcpMutualTlsConnection {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("TcpMutualTlsConnection([redacted])")
    }
}

impl fmt::Display for TcpMutualTlsConnection {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("mutual-TLS TCP Podman connection [redacted]")
    }
}

/// The transport category selected by a [`ConnectionSpec`].
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ConnectionKind {
    /// A local Unix-domain socket.
    Unix,
    /// An SSH tunnel to an explicitly named remote socket.
    Ssh,
    /// A TCP endpoint with mandatory mutual TLS.
    TcpMutualTls,
}

/// An explicit Podman service endpoint without ambient environment or config discovery.
#[derive(Clone, Eq, PartialEq)]
pub enum ConnectionSpec {
    /// A local Unix-domain socket endpoint.
    Unix(UnixConnection),
    /// An SSH endpoint with explicit authentication and host-verification references.
    Ssh(SshConnection),
    /// A TCP endpoint that always requires mutual TLS.
    TcpMutualTls(TcpMutualTlsConnection),
}

impl ConnectionSpec {
    /// Returns the category of this explicit connection.
    #[must_use]
    pub const fn kind(&self) -> ConnectionKind {
        match self {
            Self::Unix(_) => ConnectionKind::Unix,
            Self::Ssh(_) => ConnectionKind::Ssh,
            Self::TcpMutualTls(_) => ConnectionKind::TcpMutualTls,
        }
    }
}

impl fmt::Debug for ConnectionSpec {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unix(connection) => connection.fmt(formatter),
            Self::Ssh(connection) => connection.fmt(formatter),
            Self::TcpMutualTls(connection) => connection.fmt(formatter),
        }
    }
}

impl fmt::Display for ConnectionSpec {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unix(connection) => connection.fmt(formatter),
            Self::Ssh(connection) => connection.fmt(formatter),
            Self::TcpMutualTls(connection) => connection.fmt(formatter),
        }
    }
}

fn validate_endpoint_name(value: String) -> PodmanLensResult<String> {
    if value.trim().is_empty()
        || value != value.trim()
        || contains_control_character(&value)
        || value.chars().any(char::is_whitespace)
        || value.contains(['/', '@', '?', '#'])
    {
        return Err(Diagnostic::new(DiagnosticCode::InvalidConnection));
    }
    Ok(value)
}

fn contains_control_character(value: &str) -> bool {
    value.chars().any(char::is_control)
}

fn raw_uri_path<'a>(endpoint: &'a str, scheme: &str) -> PodmanLensResult<&'a str> {
    let authority_and_path = endpoint
        .strip_prefix(&format!("{scheme}://"))
        .ok_or_else(|| Diagnostic::new(DiagnosticCode::InvalidConnection))?;
    authority_and_path
        .find('/')
        .map(|index| &authority_and_path[index..])
        .ok_or_else(|| Diagnostic::new(DiagnosticCode::InvalidConnection))
}

fn is_safe_raw_socket_path(path: &str) -> bool {
    path.is_ascii()
        && path.starts_with('/')
        && !path.contains('%')
        && !path
            .chars()
            .any(|character| character.is_control() || character.is_whitespace())
        && path.split('/').all(|segment| !matches!(segment, "." | ".."))
}

fn is_valid_unix_socket_path(path: &Path) -> bool {
    let Some(path_text) = path.to_str() else {
        return false;
    };
    path.is_absolute()
        && path.file_name().is_some()
        && !path_text.contains('\0')
        && path_text.len() <= SOCKADDR_UN_PATH_MAX_BYTES
        && path_text.split('/').all(|segment| !matches!(segment, "." | ".."))
        && path.components().all(|component| {
            !matches!(
                component,
                Component::ParentDir | Component::CurDir | Component::Prefix(_)
            )
        })
}
