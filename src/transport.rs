//! Object-safe, caller-implemented Libpod transport contracts.

use std::{future::Future, pin::Pin};

use crate::{Diagnostic, DiagnosticCode, PodmanLensResult};

/// Maximum accepted encoded path and query length for one Libpod request.
pub const MAX_PATH_AND_QUERY_BYTES: usize = 8_192;

/// Bounded allocation limits for one Libpod request or response.
///
/// Limits belong to the caller because an inventory transport may legitimately need larger
/// responses than a narrow health probe. The default is deliberately large enough for ordinary
/// resource inventories while still preventing accidental unbounded allocation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TransportLimits {
    body_bytes: usize,
    header_count: usize,
    header_bytes: usize,
}

impl TransportLimits {
    /// Creates explicit message limits. Every limit must be non-zero.
    ///
    /// # Errors
    ///
    /// Returns `PLN0003` when any supplied limit is zero.
    pub fn new(max_body_bytes: usize, max_header_count: usize, max_header_bytes: usize) -> PodmanLensResult<Self> {
        if max_body_bytes == 0 || max_header_count == 0 || max_header_bytes == 0 {
            return Err(Diagnostic::new(DiagnosticCode::InvalidTransportMessage));
        }
        Ok(Self {
            body_bytes: max_body_bytes,
            header_count: max_header_count,
            header_bytes: max_header_bytes,
        })
    }

    /// Returns the maximum accepted request or response body size.
    #[must_use]
    pub const fn max_body_bytes(self) -> usize {
        self.body_bytes
    }

    /// Returns the maximum accepted header count, including duplicate names.
    #[must_use]
    pub const fn max_header_count(self) -> usize {
        self.header_count
    }

    /// Returns the maximum aggregate header size.
    #[must_use]
    pub const fn max_header_bytes(self) -> usize {
        self.header_bytes
    }
}

impl Default for TransportLimits {
    fn default() -> Self {
        Self {
            body_bytes: 64 * 1024 * 1024,
            header_count: 256,
            header_bytes: 64 * 1024,
        }
    }
}

/// A bounded Libpod HTTP method.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LibpodMethod {
    /// Read a Libpod resource.
    Get,
    /// Create or otherwise submit a Libpod resource.
    Post,
    /// Remove a Libpod resource.
    Delete,
}

impl LibpodMethod {
    /// Returns the HTTP method token used on the wire.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Get => "GET",
            Self::Post => "POST",
            Self::Delete => "DELETE",
        }
    }
}

/// An ordered, validated Libpod header.
///
/// Headers remain ordered and duplicate names are retained so a protocol decoder can identify
/// conflicting singleton headers such as `Libpod-API-Version` and `Content-Length`.
#[derive(Clone, Eq, PartialEq)]
pub struct LibpodHeader {
    name: String,
    value: String,
}

impl LibpodHeader {
    /// Creates a syntactically safe header without exposing it through formatting output.
    ///
    /// # Errors
    ///
    /// Returns `PLN0003` when the name or value contains unsafe syntax.
    pub fn new(name: impl Into<String>, value: impl Into<String>) -> PodmanLensResult<Self> {
        let name = name.into();
        let value = value.into();
        let valid_name = !name.is_empty()
            && name
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'));
        if !valid_name || value.chars().any(char::is_control) {
            return Err(Diagnostic::new(DiagnosticCode::InvalidTransportMessage));
        }
        Ok(Self { name, value })
    }

    /// Returns the validated header name.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns the validated header value for a protocol decoder.
    #[must_use]
    pub fn value(&self) -> &str {
        &self.value
    }
}

impl std::fmt::Debug for LibpodHeader {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("LibpodHeader([redacted])")
    }
}

/// An ordered collection of headers that preserves duplicate names.
#[derive(Clone, Default, Eq, PartialEq)]
pub struct LibpodHeaders(Vec<LibpodHeader>);

impl LibpodHeaders {
    /// Creates a header collection without discarding duplicate names.
    #[must_use]
    pub fn new(headers: impl Into<Vec<LibpodHeader>>) -> Self {
        Self(headers.into())
    }

    /// Returns every header in input order.
    #[must_use]
    pub fn iter(&self) -> impl ExactSizeIterator<Item = &LibpodHeader> {
        self.0.iter()
    }

    /// Finds all case-insensitive matches for one header name in input order.
    pub fn values<'a>(&'a self, name: &'a str) -> impl Iterator<Item = &'a str> + 'a {
        self.0
            .iter()
            .filter(move |header| header.name.eq_ignore_ascii_case(name))
            .map(|header| header.value.as_str())
    }

    /// Returns the number of retained headers, including duplicates.
    #[must_use]
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Returns whether the collection is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl std::fmt::Debug for LibpodHeaders {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("LibpodHeaders([redacted])")
    }
}

/// A validated Libpod path and encoded query.
#[derive(Clone, Eq, PartialEq)]
pub struct LibpodPath(String);

impl LibpodPath {
    /// Parses a Libpod path with a narrowly validated encoded query.
    ///
    /// Versioned endpoints must use `/v<semver>/libpod/...`. The unversioned
    /// `/libpod/_ping` probe is also accepted because the service exposes it before version observation.
    /// Query parameter order and duplicates are preserved for the native protocol.
    ///
    /// # Errors
    ///
    /// Returns `PLN0003` when the path, version, or encoded query is outside this contract.
    pub fn parse(path_and_query: impl Into<String>) -> PodmanLensResult<Self> {
        let path_and_query = path_and_query.into();
        if path_and_query.len() > MAX_PATH_AND_QUERY_BYTES {
            return Err(Diagnostic::new(DiagnosticCode::InvalidTransportMessage));
        }
        let (path, query) = path_and_query
            .split_once('?')
            .map_or((path_and_query.as_str(), None), |(path, query)| (path, Some(query)));
        let valid_path = (path == "/libpod/_ping" && query.is_none())
            || path
                .strip_prefix("/v")
                .and_then(|remainder| remainder.split_once("/libpod/"))
                .is_some_and(|(version, resource)| {
                    !resource.is_empty()
                        && semver::Version::parse(version).is_ok_and(|version| version.pre.is_empty())
                        && valid_raw_path(path)
                });
        let valid_query = query.is_none_or(valid_encoded_query);
        if !valid_path
            || !valid_query
            || path.contains(['#', '?'])
            || path.contains("://")
            || !path.is_ascii()
            || path
                .chars()
                .any(|character| character.is_control() || character.is_whitespace())
        {
            return Err(Diagnostic::new(DiagnosticCode::InvalidTransportMessage));
        }
        Ok(Self(path_and_query))
    }

    /// Constructs one versioned resource path from a raw, trusted identifier.
    ///
    /// This is deliberately narrower than [`Self::parse`]: callers provide an identifier as it
    /// was returned by Podman, never a pre-escaped URI segment. Reserved bytes such as `/`, `:`,
    /// and `@` are encoded exactly once. A raw percent sign is rejected so `%2F` cannot be
    /// silently treated as a pre-escaped slash. It is used for image names as well as stable IDs,
    /// whose spelling cannot safely be interpolated into an HTTP path.
    ///
    /// The collection and suffix are internal protocol constants. Keeping them private prevents
    /// an arbitrary-path escape hatch from becoming part of the transport API before M4.
    pub(crate) fn resource(
        api_version: &crate::ObservedApiVersion,
        collection: &'static str,
        identifier: &str,
        suffix: &'static str,
    ) -> PodmanLensResult<Self> {
        if !matches!(
            collection,
            "containers" | "pods" | "networks" | "volumes" | "images" | "secrets"
        ) || !matches!(suffix, "json")
            || !valid_unescaped_identifier(identifier)
        {
            return Err(Diagnostic::new(DiagnosticCode::InvalidTransportMessage));
        }
        let path = format!(
            "/v{}/libpod/{collection}/{}/{}",
            api_version.original(),
            percent_encode_identifier(identifier),
            suffix
        );
        if path.len() > MAX_PATH_AND_QUERY_BYTES {
            return Err(Diagnostic::new(DiagnosticCode::InvalidTransportMessage));
        }
        Ok(Self(path))
    }

    /// Returns the validated relative Libpod path and its encoded query.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Debug for LibpodPath {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("LibpodPath([redacted])")
    }
}

/// A bounded request submitted through a [`LibpodTransport`].
#[derive(Clone, Eq, PartialEq)]
pub struct LibpodRequest {
    method: LibpodMethod,
    path: LibpodPath,
    headers: LibpodHeaders,
    body: Vec<u8>,
}

impl LibpodRequest {
    /// Creates a request with the default safe transport limits.
    ///
    /// # Errors
    ///
    /// Returns `PLN0003` when the body is larger than the default limit.
    pub fn new(method: LibpodMethod, path: LibpodPath, body: impl Into<Vec<u8>>) -> PodmanLensResult<Self> {
        Self::with_limits(TransportLimits::default(), method, path, LibpodHeaders::default(), body)
    }

    /// Creates a request with custom headers and the default safe transport limits.
    ///
    /// # Errors
    ///
    /// Returns `PLN0003` when the message exceeds the default limits.
    pub fn with_headers(
        method: LibpodMethod,
        path: LibpodPath,
        headers: LibpodHeaders,
        body: impl Into<Vec<u8>>,
    ) -> PodmanLensResult<Self> {
        Self::with_limits(TransportLimits::default(), method, path, headers, body)
    }

    /// Creates a request with caller-specified safety limits.
    ///
    /// # Errors
    ///
    /// Returns `PLN0003` when the message exceeds the supplied limits.
    pub fn with_limits(
        limits: TransportLimits,
        method: LibpodMethod,
        path: LibpodPath,
        headers: LibpodHeaders,
        body: impl Into<Vec<u8>>,
    ) -> PodmanLensResult<Self> {
        let body = body.into();
        validate_message(&headers, &body, limits)?;
        Ok(Self {
            method,
            path,
            headers,
            body,
        })
    }

    /// Serializes a JSON body before applying the default request bounds.
    ///
    /// # Errors
    ///
    /// Returns `PLN0003` when JSON serialization fails or the message exceeds default limits.
    pub fn json(method: LibpodMethod, path: LibpodPath, value: &serde_json::Value) -> PodmanLensResult<Self> {
        let body = serde_json::to_vec(value).map_err(|_| Diagnostic::new(DiagnosticCode::InvalidTransportMessage))?;
        Self::with_headers(
            method,
            path,
            LibpodHeaders::new(vec![LibpodHeader::new("content-type", "application/json")?]),
            body,
        )
    }

    /// Returns the bounded request method.
    #[must_use]
    pub const fn method(&self) -> LibpodMethod {
        self.method
    }

    /// Returns the validated Libpod path.
    #[must_use]
    pub fn path(&self) -> &LibpodPath {
        &self.path
    }

    /// Returns ordered, duplicate-preserving request headers.
    #[must_use]
    pub fn headers(&self) -> &LibpodHeaders {
        &self.headers
    }

    /// Returns the bounded request body for the selected transport.
    #[must_use]
    pub fn body(&self) -> &[u8] {
        &self.body
    }
}

impl std::fmt::Debug for LibpodRequest {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "LibpodRequest {{ method: {:?}, path: [redacted], headers: [redacted], body: [redacted] }}",
            self.method
        )
    }
}

/// A bounded response returned by a [`LibpodTransport`].
#[derive(Clone, Eq, PartialEq)]
pub struct LibpodResponse {
    status: u16,
    headers: LibpodHeaders,
    body: Vec<u8>,
}

impl LibpodResponse {
    /// Creates a response with default safe transport limits.
    ///
    /// # Errors
    ///
    /// Returns `PLN0003` when the status or message exceeds the default limits.
    pub fn new(status: u16, headers: LibpodHeaders, body: impl Into<Vec<u8>>) -> PodmanLensResult<Self> {
        Self::with_limits(TransportLimits::default(), status, headers, body)
    }

    /// Creates a response with caller-specified safety limits.
    ///
    /// # Errors
    ///
    /// Returns `PLN0003` when the status or message exceeds the supplied limits.
    pub fn with_limits(
        limits: TransportLimits,
        status: u16,
        headers: LibpodHeaders,
        body: impl Into<Vec<u8>>,
    ) -> PodmanLensResult<Self> {
        let body = body.into();
        if !(100..=599).contains(&status) {
            return Err(Diagnostic::new(DiagnosticCode::InvalidTransportMessage));
        }
        validate_message(&headers, &body, limits)?;
        Ok(Self { status, headers, body })
    }

    /// Returns the HTTP status code.
    #[must_use]
    pub const fn status(&self) -> u16 {
        self.status
    }

    /// Returns ordered, duplicate-preserving response headers.
    #[must_use]
    pub fn headers(&self) -> &LibpodHeaders {
        &self.headers
    }

    /// Returns the bounded response body for the protocol decoder.
    #[must_use]
    pub fn body(&self) -> &[u8] {
        &self.body
    }
}

impl std::fmt::Debug for LibpodResponse {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "LibpodResponse {{ status: {}, headers: [redacted], body: [redacted] }}",
            self.status
        )
    }
}

/// A redacted error returned by a caller-provided transport.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TransportError(Diagnostic);

impl TransportError {
    /// Creates a redacted unavailable-transport error.
    #[must_use]
    pub const fn unavailable() -> Self {
        Self(Diagnostic::new(DiagnosticCode::TransportUnavailable))
    }

    /// Creates a redacted error for a prohibited mutation request.
    #[must_use]
    pub const fn read_only_rejected() -> Self {
        Self::invalid_message()
    }

    /// Creates a redacted invalid-transport-message error for internal boundary enforcement.
    #[must_use]
    pub(crate) const fn invalid_message() -> Self {
        Self(Diagnostic::new(DiagnosticCode::InvalidTransportMessage))
    }

    /// Returns the stable diagnostic for this transport failure.
    #[must_use]
    pub const fn diagnostic(&self) -> &Diagnostic {
        &self.0
    }
}

impl std::fmt::Display for TransportError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(formatter)
    }
}

impl std::error::Error for TransportError {}

/// The standard-library boxed future returned by [`LibpodTransport`].
pub type LibpodTransportFuture<'a> = Pin<Box<dyn Future<Output = Result<LibpodResponse, TransportError>> + Send + 'a>>;

/// A replaceable, object-safe asynchronous Libpod transport.
///
/// [`crate::ReadOnlyUnixTransport`] provides bounded local acquisition and rejects mutation before
/// connecting. A caller can implement this trait for reviewed SSH or mutual-TLS transport without
/// coupling `PodmanLens` to an SSH or TLS client.
pub trait LibpodTransport: Send + Sync {
    /// Sends one bounded request and resolves to a bounded response or redacted failure.
    fn send<'a>(&'a self, request: &'a LibpodRequest) -> LibpodTransportFuture<'a>;
}

fn valid_encoded_query(query: &str) -> bool {
    query.is_ascii()
        && !query.is_empty()
        && query.split('&').all(|parameter| {
            let (name, value) = parameter.split_once('=').unwrap_or((parameter, ""));
            !name.is_empty() && valid_encoded_component(name) && valid_encoded_component(value)
        })
}

fn valid_unescaped_identifier(value: &str) -> bool {
    !value.is_empty()
        && value.is_ascii()
        && !matches!(value, "." | "..")
        && !value.contains(['?', '#', '%', '\\'])
        && !value
            .chars()
            .any(|character| character.is_control() || character.is_whitespace())
}

fn percent_encode_identifier(value: &str) -> String {
    let mut encoded = String::with_capacity(value.len());
    for byte in value.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~') {
            encoded.push(char::from(byte));
        } else {
            use std::fmt::Write as _;
            let _ = write!(encoded, "%{byte:02X}");
        }
    }
    encoded
}

fn valid_raw_path(path: &str) -> bool {
    path.split('/')
        .all(|segment| !matches!(segment, "." | "..") && valid_encoded_component(segment))
}

fn valid_encoded_component(value: &str) -> bool {
    let mut index = 0;
    let bytes = value.as_bytes();
    while index < bytes.len() {
        match bytes[index] {
            b'%' => {
                if index + 2 >= bytes.len()
                    || !bytes[index + 1].is_ascii_hexdigit()
                    || !bytes[index + 2].is_ascii_hexdigit()
                {
                    return false;
                }
                let decoded = decode_percent_byte(bytes[index + 1], bytes[index + 2]);
                if !is_safe_percent_decoded_byte(decoded) {
                    return false;
                }
                index += 3;
            }
            byte if is_uri_component_byte(byte) => {
                index += 1;
            }
            _ => return false,
        }
    }
    true
}

fn decode_percent_byte(high: u8, low: u8) -> u8 {
    let nibble = |value| match value {
        b'0'..=b'9' => value - b'0',
        b'a'..=b'f' => value - b'a' + 10,
        b'A'..=b'F' => value - b'A' + 10,
        _ => 0,
    };
    (nibble(high) << 4) | nibble(low)
}

fn is_safe_percent_decoded_byte(byte: u8) -> bool {
    is_uri_component_byte(byte) && byte != b'.'
}

fn is_uri_component_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric()
        || matches!(
            byte,
            b'-' | b'.'
                | b'_'
                | b'~'
                | b'!'
                | b'$'
                | b'&'
                | b'\''
                | b'('
                | b')'
                | b'*'
                | b'+'
                | b','
                | b';'
                | b'='
                | b':'
                | b'@'
        )
}

fn validate_message(headers: &LibpodHeaders, body: &[u8], limits: TransportLimits) -> PodmanLensResult<()> {
    if body.len() > limits.max_body_bytes() || headers.len() > limits.max_header_count() {
        return Err(Diagnostic::new(DiagnosticCode::InvalidTransportMessage));
    }
    let header_bytes = headers.iter().try_fold(0_usize, |total, header| {
        total.checked_add(header.name.len() + header.value.len())
    });
    if header_bytes.is_none_or(|total| total > limits.max_header_bytes()) {
        return Err(Diagnostic::new(DiagnosticCode::InvalidTransportMessage));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::LibpodPath;
    use crate::ObservedApiVersion;

    #[test]
    fn trusted_resource_paths_encode_raw_image_identifiers_exactly_once() -> Result<(), Box<dyn std::error::Error>> {
        let api = ObservedApiVersion::parse("6.1.0")?;
        let path = LibpodPath::resource(
            &api,
            "images",
            "registry.example.invalid/team/image:1@sha256:abcdef",
            "json",
        )?;
        assert_eq!(
            path.as_str(),
            "/v6.1.0/libpod/images/registry.example.invalid%2Fteam%2Fimage%3A1%40sha256%3Aabcdef/json"
        );
        Ok(())
    }

    #[test]
    fn trusted_resource_paths_reject_preescaped_and_ambiguous_identifier_spelling()
    -> Result<(), Box<dyn std::error::Error>> {
        let api = ObservedApiVersion::parse("6.1.0")?;
        for identifier in [
            "registry.example.invalid%2Fteam%2Fimage",
            "image?all=true",
            "image#fragment",
            "image\\name",
            "image name",
            "image\nname",
            "",
        ] {
            assert!(LibpodPath::resource(&api, "images", identifier, "json").is_err());
        }
        assert!(LibpodPath::resource(&api, "unknown", "image", "json").is_err());
        assert!(LibpodPath::resource(&api, "images", "image", "delete").is_err());
        Ok(())
    }
}
