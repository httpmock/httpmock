use std::{
    collections::{HashMap, HashSet},
    fmt::Debug,
    net::{IpAddr, SocketAddr},
    sync::{Arc, Mutex, OnceLock},
};

use rcgen::{CertificateParams, Issuer, KeyPair};
use rustls::{
    ContentType,
    server::{ClientHello, ResolvesServerCert},
    sign::CertifiedKey,
};
use thiserror::Error;

use crate::{
    common::util::crypto_provider,
    server::tls::Error::{CaCertificateError, GenerateCertificateError},
};

#[derive(Error, Debug)]
pub enum Error {
    #[error("CA certificate error: {0}")]
    CaCertificateError(String),
    #[error("cannot generate certificate: {0}")]
    GenerateCertificateError(String),
}

pub trait CertificateResolverFactory {
    fn build(&self, authority: Option<String>) -> Arc<dyn ResolvesServerCert>;
}

struct SharedState {
    issuer: Issuer<'static, KeyPair>,
    // One cell per hostname, so each certificate is generated once without other hosts waiting.
    certificates: Mutex<HashMap<String, Arc<OnceLock<Arc<CertifiedKey>>>>>,
}

impl Debug for SharedState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SharedState")
            .field("issuer", &self.issuer)
            .field("certificates", &self.certificates.lock().unwrap().keys())
            .finish()
    }
}

#[derive(Debug)]
pub struct GeneratingCertificateResolverFactory {
    state: Arc<SharedState>,
}

impl GeneratingCertificateResolverFactory {
    pub fn new<IntoString: Into<String>>(ca_cert: IntoString, ca_key: IntoString) -> Result<Self, Error> {
        let ca_key = KeyPair::from_pem(&ca_key.into())
            .map_err(|err| CaCertificateError(format!("cannot parse the CA key as PEM: {err:?}")))?;
        let issuer = Issuer::from_ca_cert_pem(&ca_cert.into(), ca_key)
            .map_err(|err| CaCertificateError(format!("cannot parse the CA certificate as PEM: {err:?}")))?;
        Ok(Self {
            state: Arc::new(SharedState {
                issuer,
                certificates: Mutex::default(),
            }),
        })
    }
}

impl CertificateResolverFactory for GeneratingCertificateResolverFactory {
    fn build(&self, authority: Option<String>) -> Arc<dyn ResolvesServerCert> {
        Arc::new(GeneratingCertificateResolver {
            state: self.state.clone(),
            authority,
        })
    }
}

#[derive(Debug)]
pub struct GeneratingCertificateResolver {
    state: Arc<SharedState>,
    authority: Option<String>,
}

impl GeneratingCertificateResolver {
    pub fn generate_host_certificate(&self, hostname: &str) -> Result<Arc<CertifiedKey>, Error> {
        let params = CertificateParams::new(self.subject_alt_names(hostname))
            .map_err(failed("cannot create the certificate parameters", hostname))?;
        let key_pair = KeyPair::generate_for(&rcgen::PKCS_ECDSA_P256_SHA256)
            .map_err(failed("cannot generate a key pair", hostname))?;
        let certificate = params
            .signed_by(&key_pair, &self.state.issuer)
            .map_err(failed("cannot sign the certificate", hostname))?;
        let certified_key =
            CertifiedKey::from_der(vec![certificate.der().clone()], key_pair.into(), &crypto_provider())
                .map_err(failed("cannot load the generated key", hostname))?;
        Ok(Arc::new(certified_key))
    }

    // An SNI name gets only itself. An IP address gets itself unless it's unspecified, and when it's
    // the no-SNI fallback for this connection, also localhost, the loopback and local interface
    // addresses and the names from HTTPMOCK_EXTRA_SANS.
    fn subject_alt_names(&self, hostname: &str) -> Vec<String> {
        let Ok(ip) = hostname.parse::<IpAddr>() else {
            return vec![hostname.to_owned()];
        };

        let mut names = Vec::new();
        if !ip.is_unspecified() {
            names.push(ip.to_string());
        }
        if self
            .authority
            .as_deref()
            .is_none_or(|authority| authority_ip(authority) == Some(ip))
        {
            names.extend(["localhost", "127.0.0.1", "::1"].map(String::from));
            names.extend(collect_local_ips().map(|ip| ip.to_string()));
            names.extend(extra_sans(&std::env::var("HTTPMOCK_EXTRA_SANS").unwrap_or_default()));
        }

        let mut seen = HashSet::new();
        names.retain(|name| seen.insert(name.clone()));
        names
    }

    fn generate(&self, hostname: &str) -> Arc<CertifiedKey> {
        let certificate = Arc::clone(
            self.state
                .certificates
                .lock()
                .unwrap()
                .entry(hostname.to_owned())
                .or_default(),
        );
        certificate
            .get_or_init(|| {
                self.generate_host_certificate(hostname)
                    .unwrap_or_else(|error| panic!("Cannot generate certificate for host {hostname}: {error:?}"))
            })
            .clone()
    }
}

// Names the failed generation step and the host in the error.
fn failed<E: Debug>(step: &str, hostname: &str) -> impl FnOnce(E) -> Error {
    move |err| GenerateCertificateError(format!("{step} (host: {hostname}, error: {err:?})"))
}

// The IP address in an authority like "10.0.0.1:443", "[::1]:443", "[::1]", "::1" or "10.0.0.1".
// User info is ignored, like `http::uri::Authority::host` does.
fn authority_ip(authority: &str) -> Option<IpAddr> {
    let host_port = authority.rsplit_once('@').map_or(authority, |(_, host_port)| host_port);
    if let Ok(address) = host_port.parse::<SocketAddr>() {
        return Some(address.ip());
    }

    // `SocketAddr` rejects ports like "abc" or "99999". A bare IPv6 address has colons of its own,
    // so only a single colon separates a port.
    let host = match host_port.split_once(':') {
        Some((host, port)) if !port.contains(':') => host,
        _ => host_port,
    };
    host.strip_prefix('[')
        .and_then(|host| host.strip_suffix(']'))
        .unwrap_or(host)
        .parse()
        .ok()
}

// The comma separated IPs and DNS names of HTTPMOCK_EXTRA_SANS. IPs are normalised so duplicates are
// found, and names that can't go into a certificate (non-ASCII) are skipped.
fn extra_sans(raw: &str) -> impl Iterator<Item = String> {
    raw.split(',')
        .map(str::trim)
        .filter(|name| !name.is_empty() && name.is_ascii())
        .map(|name| {
            name.parse::<IpAddr>()
                .map_or_else(|_| name.to_owned(), |ip| ip.to_string())
        })
}

// TODO: Change ResolvesServerCert to acceptor so that async operations are supported
impl ResolvesServerCert for GeneratingCertificateResolver {
    // TODO: This implementation is synchronous, which will cause synchronous locking to
    //  enable certificate caching (lock protected hash map, sync implementation).
    //  If you look at ResolvesServerCert, it suggests that for async IO, the Acceptor interface
    //  is recommended for usage. However, it seems to require a significantly larger implementation
    //  overhead than a ResolvesServerCert. For now, this is an accepted performance loss, but should
    //  definitely be looked into and improved later!
    fn resolve(&self, client_hello: ClientHello) -> Option<Arc<CertifiedKey>> {
        if let Some(hostname) = client_hello.server_name() {
            tracing::info!("have hostname: {}", hostname);

            return Some(self.generate(hostname));
        }

        // According to https://datatracker.ietf.org/doc/html/rfc6066#section-3
        // clients may choose to not include a server name (SNI extension) in TLS ClientHello
        // messages. If there is no SNI extension, we assume the client used an IP address instead
        // of a hostname.
        let hostname = self
            .authority
            .as_deref()
            .and_then(authority_ip)
            .map(|ip| ip.to_string())
            .unwrap_or_else(|| "0.0.0.0".to_string());
        tracing::debug!("no hostname using: {}", hostname);
        Some(self.generate(&hostname))
    }
}

/// Tells whether the client opened `stream` with a TLS handshake, without consuming anything.
///
/// A TLS client always starts with a `ClientHello` in a handshake record, while HTTP/1 requests
/// and the HTTP/2 connection preface start with ASCII text, so the first byte is enough to tell
/// them apart; rustls validates the rest during the handshake. Peeking a single byte also can't
/// spin on a partially received prefix: `peek` only returns once data is available, and then it
/// either fills the one-byte buffer or reports EOF.
pub async fn starts_with_tls_handshake(stream: &tokio::net::TcpStream) -> std::io::Result<bool> {
    let mut first_byte = [0];
    stream
        .peek(&mut first_byte)
        .await
        .map(|peeked| peeked == 1 && ContentType::from(first_byte[0]) == ContentType::Handshake)
}

// The addresses of all local network interfaces, IPv4 and IPv6, without unspecified ones.
fn collect_local_ips() -> impl Iterator<Item = IpAddr> {
    if_addrs::get_if_addrs()
        .into_iter()
        .flatten()
        .map(|interface| interface.ip())
        .filter(|ip| !ip.is_unspecified())
}

#[cfg(test)]
mod tests {
    use std::{net::IpAddr, sync::Arc, time::Duration};

    use rustls::{
        ClientConfig, ClientConnection, RootCertStore,
        client::{WebPkiServerVerifier, danger::ServerCertVerifier},
        crypto::aws_lc_rs::default_provider,
        pki_types::{CertificateDer, ServerName, UnixTime, pem::PemObject},
    };
    use tokio::{
        io::{AsyncReadExt, AsyncWriteExt},
        net::{TcpListener, TcpStream},
        time::timeout,
    };

    use super::{
        GeneratingCertificateResolver, GeneratingCertificateResolverFactory, authority_ip, collect_local_ips,
        extra_sans, starts_with_tls_handshake,
    };
    use crate::server::{DEFAULT_CA_CERTIFICATE, DEFAULT_CA_PRIVATE_KEY};

    // The first flight a real rustls client sends, i.e. a handshake record carrying the ClientHello.
    fn client_hello() -> Vec<u8> {
        let config = ClientConfig::builder_with_provider(Arc::new(default_provider()))
            .with_safe_default_protocol_versions()
            .unwrap()
            .with_root_certificates(RootCertStore::empty())
            .with_no_client_auth();
        let mut connection =
            ClientConnection::new(Arc::new(config), ServerName::try_from("localhost").unwrap()).unwrap();
        let mut hello = Vec::new();
        connection.write_tls(&mut hello).unwrap();
        hello
    }

    async fn connected_pair() -> (TcpStream, TcpStream) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let client = TcpStream::connect(listener.local_addr().unwrap()).await.unwrap();
        client.set_nodelay(true).unwrap();
        let (server, _) = listener.accept().await.unwrap();
        (client, server)
    }

    // Sends `first`, runs the detection, then sends `rest` and checks that the server still reads
    // everything the client sent, which proves that the detection didn't consume any bytes.
    async fn detect(first: &[u8], rest: &[u8]) -> bool {
        let (mut client, mut server) = connected_pair().await;
        client.write_all(first).await.unwrap();
        let is_tls = timeout(Duration::from_secs(5), starts_with_tls_handshake(&server))
            .await
            .expect("detection should not wait for more than the first byte")
            .unwrap();

        client.write_all(rest).await.unwrap();
        let mut received = vec![0; first.len() + rest.len()];
        timeout(Duration::from_secs(5), server.read_exact(&mut received))
            .await
            .expect("all bytes sent by the client should still be readable")
            .unwrap();
        assert_eq!(received, [first, rest].concat());

        is_tls
    }

    #[tokio::test]
    async fn detects_rustls_client_hello() {
        assert!(detect(&client_hello(), &[]).await);
    }

    #[tokio::test]
    async fn detects_client_hello_from_first_byte() {
        let hello = client_hello();
        assert!(detect(&hello[..1], &hello[1..]).await);
    }

    #[tokio::test]
    async fn http1_request_is_not_tls() {
        assert!(!detect(b"GET / HTTP/1.1\r\nHost: localhost\r\n\r\n", b"").await);
        assert!(!detect(b"G", b"ET / HTTP/1.1\r\nHost: localhost\r\n\r\n").await);
    }

    #[tokio::test]
    async fn http1_request_with_leading_crlf_is_not_tls() {
        assert!(!detect(b"\r\nGET / HTTP/1.1\r\nHost: localhost\r\n\r\n", b"").await);
    }

    #[tokio::test]
    async fn h2_preface_is_not_tls() {
        assert!(!detect(b"PRI * HTTP/2.0\r\n\r\nSM\r\n\r\n", b"").await);
    }

    #[tokio::test]
    async fn closed_connection_is_not_tls() {
        let (client, server) = connected_pair().await;
        drop(client);
        let is_tls = timeout(Duration::from_secs(5), starts_with_tls_handshake(&server))
            .await
            .expect("detection should finish once the client closed the connection")
            .unwrap();
        assert!(!is_tls);
    }

    #[tokio::test]
    async fn waits_for_first_byte() {
        let (_client, server) = connected_pair().await;
        assert!(
            timeout(Duration::from_millis(50), starts_with_tls_handshake(&server))
                .await
                .is_err()
        );
    }

    fn certificate_for(hostname: &str, authority: Option<&str>) -> CertificateDer<'static> {
        let factory =
            GeneratingCertificateResolverFactory::new(DEFAULT_CA_CERTIFICATE, DEFAULT_CA_PRIVATE_KEY).unwrap();
        let resolver = GeneratingCertificateResolver {
            state: factory.state,
            authority: authority.map(str::to_owned),
        };
        resolver.generate_host_certificate(hostname).unwrap().cert[0].clone()
    }

    fn is_valid_for(certificate: &CertificateDer<'_>, name: &str) -> bool {
        let mut roots = RootCertStore::empty();
        roots
            .add(CertificateDer::from_pem_slice(DEFAULT_CA_CERTIFICATE.as_bytes()).unwrap())
            .unwrap();
        WebPkiServerVerifier::builder_with_provider(Arc::new(roots), Arc::new(default_provider()))
            .build()
            .unwrap()
            .verify_server_cert(
                certificate,
                &[],
                &ServerName::try_from(name.to_owned()).unwrap(),
                &[],
                UnixTime::now(),
            )
            .is_ok()
    }

    #[test]
    fn sni_certificate_is_only_valid_for_the_requested_name() {
        let certificate = certificate_for("example.test", Some("example.test:443"));
        assert!(is_valid_for(&certificate, "example.test"));
        assert!(!is_valid_for(&certificate, "localhost"));
        assert!(!is_valid_for(&certificate, "127.0.0.1"));
    }

    #[test]
    fn fallback_certificate_is_valid_for_local_names() {
        let certificate = certificate_for("127.0.0.1", Some("127.0.0.1:443"));
        for name in ["127.0.0.1", "localhost", "::1"] {
            assert!(is_valid_for(&certificate, name), "{name}");
        }
        for ip in collect_local_ips() {
            assert!(is_valid_for(&certificate, &ip.to_string()), "{ip}");
        }
    }

    #[test]
    fn certificate_without_authority_is_valid_for_local_names() {
        let certificate = certificate_for("192.0.2.1", None);
        assert!(is_valid_for(&certificate, "192.0.2.1"));
        assert!(is_valid_for(&certificate, "localhost"));
    }

    #[test]
    fn unspecified_address_is_left_out() {
        let certificate = certificate_for("0.0.0.0", None);
        assert!(is_valid_for(&certificate, "localhost"));
        assert!(!is_valid_for(&certificate, "0.0.0.0"));
    }

    #[test]
    fn ip_of_another_authority_gets_no_local_names() {
        let certificate = certificate_for("192.0.2.1", Some("198.51.100.1:443"));
        assert!(is_valid_for(&certificate, "192.0.2.1"));
        assert!(!is_valid_for(&certificate, "localhost"));
        assert!(!is_valid_for(&certificate, "198.51.100.1"));
    }

    #[test]
    fn invalid_ca_is_rejected_when_building_the_factory() {
        assert!(GeneratingCertificateResolverFactory::new(DEFAULT_CA_CERTIFICATE, "not a key").is_err());
        assert!(GeneratingCertificateResolverFactory::new("not a certificate", DEFAULT_CA_PRIVATE_KEY).is_err());
    }

    #[test]
    fn extra_sans_are_trimmed_normalised_and_filtered() {
        let names: Vec<_> = extra_sans(" 10.0.0.1, example.test,,0:0:0:0:0:0:0:1, bücher.example ").collect();
        assert_eq!(names, ["10.0.0.1", "example.test", "::1"]);
    }

    #[test]
    fn authority_ip_accepts_common_authority_forms() {
        let ip = |ip: &str| Some(ip.parse::<IpAddr>().unwrap());
        let cases = [
            ("127.0.0.1:8080", ip("127.0.0.1")),
            ("[::1]:443", ip("::1")),
            ("[::1]", ip("::1")),
            ("::1", ip("::1")),
            ("10.0.0.1", ip("10.0.0.1")),
            ("10.0.0.1:abc", ip("10.0.0.1")),
            ("10.0.0.1:99999", ip("10.0.0.1")),
            ("user@10.0.0.1:80", ip("10.0.0.1")),
            ("[::1]:abc", None),
            ("localhost:443", None),
            ("example.com", None),
            ("", None),
        ];
        for (authority, expected) in cases {
            assert_eq!(authority_ip(authority), expected, "{authority}");
        }
    }
}
