//! Boundary tests for the built-in Unix acquisition transport.

#![cfg(unix)]

use std::{
    io::{Read, Write},
    os::unix::net::UnixListener,
    path::PathBuf,
    sync::atomic::{AtomicUsize, Ordering},
    thread,
    time::Duration,
};

use podman_lens::{
    DiagnosticCode, LibpodHeader, LibpodHeaders, LibpodMethod, LibpodPath, LibpodRequest, LibpodTransport,
    MIN_HTTP1_HEADER_BYTES, ReadOnlyUnixTransport, ReadOnlyUnixTransportTimeouts, TransportLimits, UnixConnection,
};

static SOCKET_ID: AtomicUsize = AtomicUsize::new(0);

fn socket_path() -> PathBuf {
    std::env::temp_dir().join(format!(
        "podman-lens-read-only-{}-{}.sock",
        std::process::id(),
        SOCKET_ID.fetch_add(1, Ordering::Relaxed)
    ))
}

fn transport(path: PathBuf) -> Result<ReadOnlyUnixTransport, Box<dyn std::error::Error>> {
    transport_with_limits(path, TransportLimits::new(1_024, 16, MIN_HTTP1_HEADER_BYTES)?)
}

fn transport_with_limits(
    path: PathBuf,
    limits: TransportLimits,
) -> Result<ReadOnlyUnixTransport, Box<dyn std::error::Error>> {
    Ok(ReadOnlyUnixTransport::new(
        UnixConnection::new(path)?,
        limits,
        ReadOnlyUnixTransportTimeouts::new(
            Duration::from_secs(1),
            Duration::from_secs(1),
            Duration::from_secs(1),
            Duration::from_secs(2),
        )?,
    )?)
}

#[tokio::test]
async fn transport_rejects_post_and_delete_before_opening_a_socket() -> Result<(), Box<dyn std::error::Error>> {
    let path = socket_path();
    let transport = transport(path.clone())?;
    for method in [LibpodMethod::Post, LibpodMethod::Delete] {
        let request = LibpodRequest::new(
            method,
            LibpodPath::parse("/v6.1.0/libpod/containers/create")?,
            Vec::new(),
        )?;
        let error = transport
            .send(&request)
            .await
            .err()
            .ok_or_else(|| std::io::Error::other("read-only transport accepted a mutation"))?;
        assert_eq!(error.diagnostic().code(), DiagnosticCode::InvalidTransportMessage);
        assert!(!path.exists());
    }
    assert!(!format!("{transport:?}").contains("podman-lens-read-only"));
    Ok(())
}

#[tokio::test]
async fn transport_rejects_caller_supplied_host_before_opening_a_socket() -> Result<(), Box<dyn std::error::Error>> {
    let path = socket_path();
    let transport = transport(path.clone())?;
    for name in ["Host", "host"] {
        let request = LibpodRequest::with_headers(
            LibpodMethod::Get,
            LibpodPath::parse("/libpod/_ping")?,
            LibpodHeaders::new(vec![LibpodHeader::new(name, "private.example.invalid")?]),
            Vec::new(),
        )?;
        let error = transport
            .send(&request)
            .await
            .err()
            .ok_or_else(|| std::io::Error::other("transport accepted caller Host"))?;
        assert_eq!(error.diagnostic().code(), DiagnosticCode::InvalidTransportMessage);
        assert!(!path.exists());
    }
    Ok(())
}

#[tokio::test]
async fn transport_rejects_bodied_and_over_limit_get_requests_before_opening_a_socket()
-> Result<(), Box<dyn std::error::Error>> {
    let cases = [
        LibpodRequest::new(
            LibpodMethod::Get,
            LibpodPath::parse("/libpod/_ping")?,
            b"unexpected".to_vec(),
        )?,
        LibpodRequest::with_headers(
            LibpodMethod::Get,
            LibpodPath::parse("/libpod/_ping")?,
            LibpodHeaders::new(vec![LibpodHeader::new("x-one", "1")?, LibpodHeader::new("x-two", "2")?]),
            Vec::new(),
        )?,
        LibpodRequest::with_headers(
            LibpodMethod::Get,
            LibpodPath::parse("/libpod/_ping")?,
            LibpodHeaders::new(vec![LibpodHeader::new("x-large", "a".repeat(MIN_HTTP1_HEADER_BYTES))?]),
            Vec::new(),
        )?,
    ];

    for (index, request) in cases.iter().enumerate() {
        let path = socket_path();
        let limits = if index == 1 {
            TransportLimits::new(1_024, 1, MIN_HTTP1_HEADER_BYTES)?
        } else {
            TransportLimits::new(1_024, 16, MIN_HTTP1_HEADER_BYTES)?
        };
        let transport = transport_with_limits(path.clone(), limits)?;
        let error = transport
            .send(request)
            .await
            .err()
            .ok_or_else(|| std::io::Error::other("transport accepted invalid acquisition request"))?;
        assert_eq!(error.diagnostic().code(), DiagnosticCode::InvalidTransportMessage);
        assert!(!path.exists());
    }
    Ok(())
}

#[test]
fn transport_requires_the_http1_parser_minimum_header_ceiling() -> Result<(), Box<dyn std::error::Error>> {
    let error = ReadOnlyUnixTransport::new(
        UnixConnection::new(socket_path())?,
        TransportLimits::new(1_024, 16, MIN_HTTP1_HEADER_BYTES - 1)?,
        ReadOnlyUnixTransportTimeouts::default(),
    )
    .err()
    .ok_or_else(|| std::io::Error::other("transport accepted a header ceiling below Hyper's minimum"))?;
    assert_eq!(error.code(), DiagnosticCode::InvalidTransportMessage);
    Ok(())
}

#[test]
fn transport_deadlines_must_be_nonzero() {
    for timeouts in [
        ReadOnlyUnixTransportTimeouts::new(
            Duration::ZERO,
            Duration::from_secs(1),
            Duration::from_secs(1),
            Duration::from_secs(1),
        ),
        ReadOnlyUnixTransportTimeouts::new(
            Duration::from_secs(1),
            Duration::ZERO,
            Duration::from_secs(1),
            Duration::from_secs(1),
        ),
        ReadOnlyUnixTransportTimeouts::new(
            Duration::from_secs(1),
            Duration::from_secs(1),
            Duration::ZERO,
            Duration::from_secs(1),
        ),
        ReadOnlyUnixTransportTimeouts::new(
            Duration::from_secs(1),
            Duration::from_secs(1),
            Duration::from_secs(1),
            Duration::ZERO,
        ),
    ] {
        assert_eq!(
            timeouts.err().map(|error| error.code()),
            Some(DiagnosticCode::InvalidTransportMessage)
        );
    }
}

#[tokio::test]
async fn transport_uses_hyper_for_content_length_chunked_and_close_delimited_bodies()
-> Result<(), Box<dyn std::error::Error>> {
    for response in [
        b"HTTP/1.1 200 OK\r\nContent-Length: 5\r\nX-Mode: content-length\r\nX-Duplicate: first\r\nX-Duplicate: second\r\n\r\nhello".as_slice(),
        b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\nX-Mode: chunked\r\n\r\n5\r\nhello\r\n0\r\n\r\n".as_slice(),
        b"HTTP/1.0 200 OK\r\nX-Mode: close\r\n\r\nhello".as_slice(),
    ] {
        let path = socket_path();
        let listener = UnixListener::bind(&path)?;
        let server = thread::spawn(move || -> Result<Vec<u8>, std::io::Error> {
            let (mut stream, _) = listener.accept()?;
            let mut request = vec![0; 1_024];
            let bytes = stream.read(&mut request)?;
            stream.write_all(response)?;
            Ok(request[..bytes].to_vec())
        });

        let transport = transport(path.clone())?;
        let request = LibpodRequest::new(LibpodMethod::Get, LibpodPath::parse("/libpod/_ping")?, Vec::new())?;
        let actual = transport.send(&request).await?;
        assert_eq!(actual.status(), 200);
        assert_eq!(actual.body(), b"hello");
        if actual.headers().values("x-mode").any(|mode| mode == "content-length") {
            assert_eq!(actual.headers().values("x-duplicate").collect::<Vec<_>>(), vec!["first", "second"]);
        }
        let received = server
            .join()
            .map_err(|_| std::io::Error::other("fixture server panicked"))??;
        assert!(received.starts_with(b"GET /libpod/_ping HTTP/1.1\r\n"));
        assert_eq!(
            String::from_utf8_lossy(&received)
                .split("\r\n")
                .filter(|line| line.eq_ignore_ascii_case("host: localhost"))
                .count(),
            1
        );
        std::fs::remove_file(path)?;
    }
    Ok(())
}

#[tokio::test]
async fn response_header_count_and_bytes_are_bounded_as_invalid_messages() -> Result<(), Box<dyn std::error::Error>> {
    let cases = [
        (
            TransportLimits::new(1_024, 2, MIN_HTTP1_HEADER_BYTES)?,
            b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\nX-One: 1\r\nX-Two: 2\r\n\r\n".to_vec(),
        ),
        (
            TransportLimits::new(1_024, 16, MIN_HTTP1_HEADER_BYTES)?,
            format!(
                "HTTP/1.1 200 OK\r\nContent-Length: 0\r\nX-Large: {}\r\n\r\n",
                "a".repeat(MIN_HTTP1_HEADER_BYTES)
            )
            .into_bytes(),
        ),
    ];

    for (limits, response) in cases {
        let path = socket_path();
        let listener = UnixListener::bind(&path)?;
        let server = thread::spawn(move || -> Result<(), std::io::Error> {
            let (mut stream, _) = listener.accept()?;
            let mut request = [0; 1_024];
            let _ = stream.read(&mut request)?;
            stream.write_all(&response)
        });

        let transport = transport_with_limits(path.clone(), limits)?;
        let request = LibpodRequest::new(LibpodMethod::Get, LibpodPath::parse("/libpod/_ping")?, Vec::new())?;
        let error = transport
            .send(&request)
            .await
            .err()
            .ok_or_else(|| std::io::Error::other("over-limit response headers were accepted"))?;
        assert_eq!(error.diagnostic().code(), DiagnosticCode::InvalidTransportMessage);
        server
            .join()
            .map_err(|_| std::io::Error::other("fixture server panicked"))??;
        std::fs::remove_file(path)?;
    }
    Ok(())
}

#[tokio::test]
async fn peer_close_is_reported_as_transport_unavailable() -> Result<(), Box<dyn std::error::Error>> {
    let path = socket_path();
    let listener = UnixListener::bind(&path)?;
    let server = thread::spawn(move || -> Result<(), std::io::Error> {
        let (mut stream, _) = listener.accept()?;
        let mut request = [0; 1_024];
        let _ = stream.read(&mut request)?;
        Ok(())
    });

    let transport = transport(path.clone())?;
    let request = LibpodRequest::new(LibpodMethod::Get, LibpodPath::parse("/libpod/_ping")?, Vec::new())?;
    let error = transport
        .send(&request)
        .await
        .err()
        .ok_or_else(|| std::io::Error::other("closed peer produced a response"))?;
    assert_eq!(error.diagnostic().code(), DiagnosticCode::TransportUnavailable);
    server
        .join()
        .map_err(|_| std::io::Error::other("fixture server panicked"))??;
    std::fs::remove_file(path)?;
    Ok(())
}

#[tokio::test]
async fn truncated_framed_bodies_are_invalid_messages() -> Result<(), Box<dyn std::error::Error>> {
    for response in [
        b"HTTP/1.1 200 OK\r\nContent-Length: 5\r\n\r\nabc".to_vec(),
        b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n5\r\nabc".to_vec(),
    ] {
        let path = socket_path();
        let listener = UnixListener::bind(&path)?;
        let server = thread::spawn(move || -> Result<(), std::io::Error> {
            let (mut stream, _) = listener.accept()?;
            let mut request = [0; 1_024];
            let _ = stream.read(&mut request)?;
            stream.write_all(&response)
        });

        let transport = transport(path.clone())?;
        let request = LibpodRequest::new(LibpodMethod::Get, LibpodPath::parse("/libpod/_ping")?, Vec::new())?;
        let error = transport
            .send(&request)
            .await
            .err()
            .ok_or_else(|| std::io::Error::other("truncated framed body was accepted"))?;
        assert_eq!(error.diagnostic().code(), DiagnosticCode::InvalidTransportMessage);
        server
            .join()
            .map_err(|_| std::io::Error::other("fixture server panicked"))??;
        std::fs::remove_file(path)?;
    }
    Ok(())
}
