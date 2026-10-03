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

fn request(server: &MockServer, version: &'static SupportedProtocolVersion) -> std::io::Result<String> {
    let mut roots = RootCertStore::empty();
    roots
        .add(CertificateDer::from_pem_slice(httpmock::server::DEFAULT_CA_CERTIFICATE.as_bytes()).unwrap())
        .unwrap();
    let config = ClientConfig::builder_with_provider(Arc::new(rustls::crypto::ring::default_provider()))
        .with_protocol_versions(&[version])
        .unwrap()
        .with_root_certificates(roots)
        .with_no_client_auth();
    let connection = ClientConnection::new(Arc::new(config), ServerName::try_from("localhost").unwrap()).unwrap();
    let socket = TcpStream::connect(server.address())?;
    socket.set_read_timeout(Some(Duration::from_secs(5)))?;
    socket.set_write_timeout(Some(Duration::from_secs(5)))?;
    let mut stream = rustls::StreamOwned::new(connection, socket);
    stream.write_all(b"GET /tls HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n")?;
    let mut response = String::new();
    stream.read_to_string(&mut response)?;
    Ok(response)
}

#[test]
fn application_tls_provider_is_respected() {
    // This test has its own integration-test binary: the global provider is not shared
    // with the main test suite or its clients.
    let mut provider = rustls::crypto::ring::default_provider();
    provider
        .cipher_suites
        .retain(|suite| suite.version().version == rustls::ProtocolVersion::TLSv1_2);
    provider.install_default().unwrap();

    let server = MockServer::start();
    let mock = server.mock(|when, then| {
        when.path("/tls");
        then.status(201);
    });

    assert!(
        request(&server, &rustls::version::TLS12)
            .unwrap()
            .starts_with("HTTP/1.1 201")
    );
    assert!(request(&server, &rustls::version::TLS13).is_err());
    mock.assert();
}
