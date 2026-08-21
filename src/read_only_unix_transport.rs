//! Built-in read-only Unix-socket HTTP/1.1 transport for Libpod acquisition.

use std::{fmt, time::Duration};

use bytes::Bytes;
use http_body_util::{BodyExt, Full, LengthLimitError, Limited};
use hyper::{Request, client::conn::http1};
use hyper_util::rt::TokioIo;
use tokio::{net::UnixStream, time::timeout};

use crate::{
    Diagnostic, DiagnosticCode, LibpodHeader, LibpodHeaders, LibpodMethod, LibpodRequest, LibpodResponse,
    LibpodTransport, LibpodTransportFuture, PodmanLensResult, TransportError, TransportLimits, UnixConnection,
};

/// Hyper's documented minimum HTTP/1 parser buffer size.
pub const MIN_HTTP1_HEADER_BYTES: usize = 8_192;

/// Explicit non-zero deadlines for one read-only Unix-socket exchange.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ReadOnlyUnixTransportTimeouts {
    connect: Duration,
    headers: Duration,
    body: Duration,
    total: Duration,
}

impl ReadOnlyUnixTransportTimeouts {
    /// Creates explicit connection, response-header, response-body, and total deadlines.
    ///
    /// # Errors
    ///
    /// Returns `PLN0003` when any deadline is zero.
    pub fn new(connect: Duration, headers: Duration, body: Duration, total: Duration) -> PodmanLensResult<Self> {
        if connect.is_zero() || headers.is_zero() || body.is_zero() || total.is_zero() {
            return Err(Diagnostic::new(DiagnosticCode::InvalidTransportMessage));
        }
        Ok(Self {
            connect,
            headers,
            body,
            total,
        })
    }

    /// Returns the connection deadline.
    #[must_use]
    pub const fn connect(self) -> Duration {
        self.connect
    }

    /// Returns the response-header deadline.
    #[must_use]
    pub const fn headers(self) -> Duration {
        self.headers
    }

    /// Returns the response-body deadline.
    #[must_use]
    pub const fn body(self) -> Duration {
        self.body
    }

    /// Returns the complete-exchange deadline.
    #[must_use]
    pub const fn total(self) -> Duration {
        self.total
    }
}

impl Default for ReadOnlyUnixTransportTimeouts {
    fn default() -> Self {
        Self {
            connect: Duration::from_secs(5),
            headers: Duration::from_secs(10),
            body: Duration::from_secs(30),
            total: Duration::from_secs(45),
        }
    }
}

/// An explicit Unix-socket HTTP/1.1 transport that can acquire data only.
///
/// It rejects every method except `GET` before opening the selected Unix socket. It does not
/// discover endpoints, follow redirects, decompress content, retry requests, execute plans, or
/// start detached tasks.
#[derive(Clone)]
pub struct ReadOnlyUnixTransport {
    connection: UnixConnection,
    limits: TransportLimits,
    timeouts: ReadOnlyUnixTransportTimeouts,
}

impl ReadOnlyUnixTransport {
    /// Creates a read-only transport for one explicit Unix socket.
    ///
    /// # Errors
    ///
    /// Returns `PLN0003` when the configured header-byte limit is smaller than the HTTP/1
    /// parser's minimum allocation ceiling.
    pub fn new(
        connection: UnixConnection,
        limits: TransportLimits,
        timeouts: ReadOnlyUnixTransportTimeouts,
    ) -> PodmanLensResult<Self> {
        if limits.max_header_bytes() < MIN_HTTP1_HEADER_BYTES {
            return Err(Diagnostic::new(DiagnosticCode::InvalidTransportMessage));
        }
        Ok(Self {
            connection,
            limits,
            timeouts,
        })
    }

    /// Returns the selected explicit Unix connection.
    #[must_use]
    pub const fn connection(&self) -> &UnixConnection {
        &self.connection
    }

    /// Returns the enforced response bounds.
    #[must_use]
    pub const fn limits(&self) -> TransportLimits {
        self.limits
    }

    /// Returns the enforced exchange deadlines.
    #[must_use]
    pub const fn timeouts(&self) -> ReadOnlyUnixTransportTimeouts {
        self.timeouts
    }

    async fn send_get(&self, request: &LibpodRequest) -> Result<LibpodResponse, TransportError> {
        validate_acquisition_request(request, self.limits)?;
        let request = build_http_request(request)?;
        let stream = timeout(
            self.timeouts.connect(),
            UnixStream::connect(self.connection.socket_path()),
        )
        .await
        .map_err(|_| TransportError::unavailable())?
        .map_err(|_| TransportError::unavailable())?;
        let mut builder = http1::Builder::new();
        builder.max_headers(self.limits.max_header_count());
        builder.max_buf_size(self.limits.max_header_bytes());
        let (mut sender, connection) = builder
            .handshake(TokioIo::new(stream))
            .await
            .map_err(|_| TransportError::unavailable())?;
        let mut connection = Box::pin(connection);

        let response_future = sender.send_request(request);
        tokio::pin!(response_future);
        let response = timeout(self.timeouts.headers(), async {
            let response = tokio::select! {
                biased;
                response = &mut response_future => response,
                result = &mut connection => {
                    // Keep driving the connection, but the phase future owns classification. A
                    // completed connection can race a still-ready response on macOS.
                    let _connection_result = result;
                    response_future.await
                }
            };
            response.map_err(|error| classify_hyper_error(&error))
        })
        .await
        .map_err(|_| TransportError::unavailable())??;

        let status = response.status().as_u16();
        let headers = convert_headers(response.headers())?;
        let body = Limited::new(response.into_body(), self.limits.max_body_bytes());
        let body_future = body.collect();
        tokio::pin!(body_future);
        let collected = timeout(self.timeouts.body(), async {
            let body = tokio::select! {
                biased;
                body = &mut body_future => body,
                result = &mut connection => {
                    // As above, body framing is authoritative after headers were accepted.
                    let _connection_result = result;
                    body_future.await
                }
            };
            body.map_err(|error| classify_body_error(error.as_ref()))
        })
        .await
        .map_err(|_| TransportError::unavailable())??;
        LibpodResponse::with_limits(self.limits, status, headers, collected.to_bytes().to_vec())
            .map_err(|_| TransportError::invalid_message())
    }
}

impl LibpodTransport for ReadOnlyUnixTransport {
    fn send<'a>(&'a self, request: &'a LibpodRequest) -> LibpodTransportFuture<'a> {
        if let Err(error) = validate_acquisition_request(request, self.limits) {
            return Box::pin(async move { Err(error) });
        }
        Box::pin(async move {
            timeout(self.timeouts.total(), self.send_get(request))
                .await
                .map_err(|_| TransportError::unavailable())?
        })
    }
}

fn validate_acquisition_request(request: &LibpodRequest, limits: TransportLimits) -> Result<(), TransportError> {
    if request.method() != LibpodMethod::Get {
        return Err(TransportError::read_only_rejected());
    }
    if !request.body().is_empty() || request.headers().len() > limits.max_header_count() {
        return Err(TransportError::invalid_message());
    }
    let header_bytes = request.headers().iter().try_fold(0_usize, |total, header| {
        total.checked_add(header.name().len() + header.value().len())
    });
    if header_bytes.is_none_or(|total| total > limits.max_header_bytes()) {
        return Err(TransportError::invalid_message());
    }
    Ok(())
}

impl fmt::Debug for ReadOnlyUnixTransport {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("ReadOnlyUnixTransport([redacted])")
    }
}

fn build_http_request(request: &LibpodRequest) -> Result<Request<Full<Bytes>>, TransportError> {
    let mut builder = Request::builder()
        .method(request.method().as_str())
        .uri(request.path().as_str());
    let headers = builder.headers_mut().ok_or_else(TransportError::unavailable)?;
    for header in request.headers().iter() {
        if header.name().eq_ignore_ascii_case("host") {
            return Err(TransportError::invalid_message());
        }
        let name = hyper::http::header::HeaderName::from_bytes(header.name().as_bytes())
            .map_err(|_| TransportError::invalid_message())?;
        let value =
            hyper::http::HeaderValue::from_str(header.value()).map_err(|_| TransportError::invalid_message())?;
        headers.append(name, value);
    }
    headers.insert(
        hyper::http::header::HOST,
        hyper::http::HeaderValue::from_static("localhost"),
    );
    builder
        .body(Full::new(Bytes::copy_from_slice(request.body())))
        .map_err(|_| TransportError::invalid_message())
}

fn convert_headers(headers: &hyper::http::HeaderMap) -> Result<LibpodHeaders, TransportError> {
    let mut converted = Vec::with_capacity(headers.len());
    for (name, value) in headers {
        let value = value.to_str().map_err(|_| TransportError::invalid_message())?;
        converted.push(LibpodHeader::new(name.as_str(), value).map_err(|_| TransportError::invalid_message())?);
    }
    Ok(LibpodHeaders::new(converted))
}

fn classify_hyper_error(error: &hyper::Error) -> TransportError {
    if error.is_parse() {
        TransportError::invalid_message()
    } else {
        TransportError::unavailable()
    }
}

fn classify_body_hyper_error(_error: &hyper::Error) -> TransportError {
    // Once response headers have been accepted, a Hyper connection error means the
    // advertised response framing could not be completed. Pre-response connection
    // failures are classified separately as transport unavailability.
    TransportError::invalid_message()
}

fn classify_body_error(error: &(dyn std::error::Error + Send + Sync + 'static)) -> TransportError {
    if error.downcast_ref::<LengthLimitError>().is_some() {
        return TransportError::invalid_message();
    }
    error
        .downcast_ref::<hyper::Error>()
        .map_or_else(TransportError::unavailable, classify_body_hyper_error)
}
