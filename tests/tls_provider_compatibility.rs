//! Ensures that the HTTPS mock server respects the application's installed rustls crypto provider.
//!
//! A provider supplies TLS algorithms and cipher suites. Replacing an application's installed
//! provider with a fresh ring default would silently discard its configuration.
//!
//! httpmock supports TLS 1.3. This test deliberately removes TLS 1.3 cipher suites from its test
//! provider to make the server's choice observable:
//!
//! - A TLS 1.2 request must reach the `/tls` mock and return HTTP 201.
//! - A TLS 1.3-only request must fail before reaching HTTP request matching.
//! - The mock must record exactly one request, from the successful TLS 1.2 connection.
//!
//! The client uses a separate provider that supports both versions, so only the server is restricted.
//!
//! This covers an already-installed provider; it does not test the ring fallback when none is installed.
//!
//! Keep this in its own integration-test binary: rustls's default provider can only be installed once
//! per process, so sharing it with other TLS tests would make the result depend on test order.

#![cfg(feature = "https")]

use std::{
    io::{Read, Write},
    net::TcpStream,
    sync::Arc,
    time::Duration,
};

use httpmock::MockServer;
use rustls::{ClientConfig, ClientConnection, RootCertStore, SupportedProtocolVersion};
use rustls_pki_types::{CertificateDer, ServerName, pem::PemObject};

/// Make a real HTTPS request with exactly the requested TLS version and return its HTTP response.
///
/// Using rustls directly controls the client's provider and protocol while retaining certificate
/// verification. Handshake and I/O failures are returned to the caller.
fn request(server: &MockServer, version: &'static SupportedProtocolVersion) -> std::io::Result<String> {
    // Trust httpmock's test CA while retaining normal server-certificate verification.
    let mut roots = RootCertStore::empty();
    roots
        .add(CertificateDer::from_pem_slice(httpmock::server::DEFAULT_CA_CERTIFICATE.as_bytes()).unwrap())
        .unwrap();

    // Use an independent, unrestricted ring provider for the client. Using the global provider
    // would prevent the client itself from attempting TLS 1.3 and would not test the server's choice.
    let config = ClientConfig::builder_with_provider(Arc::new(rustls::crypto::ring::default_provider()))
        .with_protocol_versions(&[version])
        .unwrap()
        .with_root_certificates(roots)
        .with_no_client_auth();

    // Verify the certificate for localhost while connecting to the mock server's allocated port.
    let connection = ClientConnection::new(Arc::new(config), ServerName::try_from("localhost").unwrap()).unwrap();
    let socket = TcpStream::connect(server.address())?;

    // Bound network waits so a TLS regression does not leave the test waiting indefinitely on I/O.
    socket.set_read_timeout(Some(Duration::from_secs(5)))?;
    socket.set_write_timeout(Some(Duration::from_secs(5)))?;

    // Writing drives the TLS handshake and sends GET /tls; handshake failures return an I/O error.
    // Connection: close lets us read through EOF without an HTTP response parser.
    let mut stream = rustls::StreamOwned::new(connection, socket);
    stream.write_all(b"GET /tls HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n")?;

    let mut response = String::new();
    stream.read_to_string(&mut response)?;

    Ok(response)
}

/// Preserve an application's TLS settings when constructing the mock server's TLS configuration.
/// The two client attempts expose whether httpmock uses the installed provider or replaces it.
#[test]
fn application_tls_provider_is_respected() {
    // Create the application's provider; the client creates its own independent ring provider.
    let mut provider = rustls::crypto::ring::default_provider();

    // Deliberately retain only TLS 1.2 cipher suites. rustls cannot negotiate TLS 1.3 without a
    // TLS 1.3 cipher suite, even if the server's configured protocol list includes that version.
    // This restriction belongs to this test's provider, not to httpmock's normal TLS configuration.
    provider
        .cipher_suites
        .retain(|suite| suite.version().version == rustls::ProtocolVersion::TLSv1_2);

    // Installation must succeed before httpmock can create a TLS configuration. A previously
    // installed provider would invalidate this setup, so an installation error must fail the test.
    provider.install_default().unwrap();

    // Start a local mock server after installing the provider, as an application would do.
    let server = MockServer::start();

    // A 201 response proves that TLS completed and the request reached the /tls mock.
    let mock = server.mock(|when, then| {
        when.path("/tls");
        then.status(201);
    });

    // First confirm TLS 1.2, certificate trust and HTTP matching work before checking rejection.
    assert!(
        request(&server, &rustls::version::TLS12)
            .unwrap()
            .starts_with("HTTP/1.1 201")
    );

    // The TLS 1.3-only client cannot fall back to TLS 1.2. If httpmock replaces the application's
    // provider with unrestricted ring defaults, this request succeeds and the assertion fails.
    assert!(request(&server, &rustls::version::TLS13).is_err());

    // Only the TLS 1.2 request should reach HTTP matching; the rejected handshake adds no mock call.
    mock.assert();
}
