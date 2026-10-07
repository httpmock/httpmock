use std::{
    collections::HashMap,
    fmt::Debug,
    io::Cursor,
    sync::{Arc, Mutex, RwLock},
};

use rcgen::{CertificateParams, Issuer, KeyPair, SanType};
use rustls::{
    ContentType,
    crypto::ring::sign::any_supported_type,
    pki_types::{CertificateDer, PrivateKeyDer},
    server::{ClientHello, ResolvesServerCert},
    sign::CertifiedKey,
};
use rustls_pki_types::pem::PemObject;
use thiserror::Error;

use crate::server::tls::Error::{CaCertificateError, GenerateCertificateError};

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
    certificates: RwLock<HashMap<String, Arc<CertifiedKey>>>,
    locks: RwLock<HashMap<String, Arc<Mutex<()>>>>,
    ca_cert_str: String,
    ca_key_str: String,
}

impl std::fmt::Debug for SharedState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SharedState")
            .field("certificates", &self.certificates.read().unwrap().keys())
            .field("locks", &self.locks.read().unwrap().keys())
            .field("ca_cert_str", &self.ca_cert_str)
            .field("ca_key_str", &self.ca_key_str)
            .finish()
    }
}

#[derive(Debug)]
pub struct GeneratingCertificateResolverFactory {
    state: Arc<SharedState>,
}

impl GeneratingCertificateResolverFactory {
    pub fn new<IntoString: Into<String>>(ca_cert: IntoString, ca_key: IntoString) -> Result<Self, Error> {
        Ok(Self {
            state: Arc::new(SharedState {
                certificates: RwLock::new(HashMap::new()),
                locks: RwLock::new(HashMap::new()),
                ca_cert_str: ca_cert.into(),
                ca_key_str: ca_key.into(),
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

impl<'a> GeneratingCertificateResolver {
    fn load_certificates(cert_pem: String) -> Result<Vec<CertificateDer<'a>>, Error> {
        let cert_pem_reader = Cursor::new(cert_pem.into_bytes());
        let mut certificates = Vec::new();
        let certs_iterator = CertificateDer::pem_reader_iter(cert_pem_reader);
        for cert_result in certs_iterator {
            let cert = cert_result
                .map_err(|err| GenerateCertificateError(format!("cannot use generated certificate: {:?}", err)))?; // Propagate error if any
            certificates.push(cert);
        }

        Ok(certificates)
    }

    fn load_private_key(key_pem: String) -> Result<PrivateKeyDer<'a>, Error> {
        let cert_pem_reader = Cursor::new(key_pem.into_bytes());
        let private_key = PrivateKeyDer::from_pem_reader(cert_pem_reader)
            .map_err(|err| GenerateCertificateError(format!("cannot use generated private key: {:?}", err)))?;
        Ok(private_key)
    }

    fn authority_ip(&self) -> Option<std::net::IpAddr> {
        let auth = self.authority.as_deref()?;

        // 1) Full socket address like "127.0.0.1:8080" or "[::1]:443"
        if let Ok(sa) = auth.parse::<std::net::SocketAddr>() {
            return Some(sa.ip());
        }

        // 2) Bracketed IPv6 without port: "[::1]"
        if let Some(inner) = auth.strip_prefix('[').and_then(|s| s.strip_suffix(']'))
            && let Ok(ip) = inner.parse::<std::net::IpAddr>()
        {
            return Some(ip);
        }

        // 3) Parse as HTTP authority and take the host (handles bracketed IPv6 and ports)
        if let Ok(a) = auth.parse::<http::uri::Authority>()
            && let Ok(ip) = a.host().parse::<std::net::IpAddr>()
        {
            return Some(ip);
        }

        // 4) Plain IP literal (v4 or v6)
        if let Ok(ip) = auth.parse::<std::net::IpAddr>() {
            return Some(ip);
        }

        // 5) Conservative host:port split only if there's exactly one ':' (avoids mangling IPv6)
        if auth.matches(':').count() == 1
            && let Some((host, _)) = auth.rsplit_once(':')
            && let Ok(ip) = host.parse::<std::net::IpAddr>()
        {
            return Some(ip);
        }

        None
    }

    pub fn generate_host_certificate(&'a self, hostname: &str) -> Result<Arc<CertifiedKey>, Error> {
        // Create a key pair for the CA from the provided PEM
        let ca_key = KeyPair::from_pem(&self.state.ca_key_str).map_err(|err| {
            CaCertificateError(format!(
                "Expected CA key to be provided in PEM format but failed to parse it (host: {}: error: {:?})",
                hostname, err
            ))
        })?;

        // Set up certificate parameters for the new certificate
        let params = if let Ok(ip) = hostname.parse::<std::net::IpAddr>() {
            // If the hostname is an IP address, place it into IP SANs unless it's unspecified (0.0.0.0 / ::)
            let mut p = CertificateParams::default();
            if !ip.is_unspecified() {
                p.subject_alt_names.push(SanType::IpAddress(ip));
            }

            // If this call originated from a no-SNI fallback, hostname equals the
            // local TCP address of this resolver. In that case, enrich the cert
            // with local-friendly SANs and any extras from HTTPMOCK_EXTRA_SANS.
            if self.authority.is_none() || self.authority_ip().map(|a| a == ip).unwrap_or(false) {
                // No-SNI fallback or no authority: add localhost variants, all local IPs, and extras from env
                if let Ok(localhost_dns) =
                    <rcgen::string::Ia5String as std::convert::TryFrom<&str>>::try_from("localhost")
                {
                    p.subject_alt_names.push(SanType::DnsName(localhost_dns));
                }
                if let Ok(loopback_v4) = "127.0.0.1".parse::<std::net::IpAddr>() {
                    p.subject_alt_names.push(SanType::IpAddress(loopback_v4));
                }
                if let Ok(loopback_v6) = "::1".parse::<std::net::IpAddr>() {
                    p.subject_alt_names.push(SanType::IpAddress(loopback_v6));
                }
                // Add all local interface IPs
                let locals = collect_local_ips();
                let local_sans: Vec<SanType> = locals.into_iter().map(SanType::IpAddress).collect();
                push_unique_sans(&mut p.subject_alt_names, local_sans);
                // Merge extras from env, avoiding duplicates
                let extra_sans = parse_extra_sans_from_env();
                push_unique_sans(&mut p.subject_alt_names, extra_sans);
            }
            p
        } else {
            // Otherwise, treat it as a DNS name
            let p = CertificateParams::new(vec![hostname.to_owned()]).map_err(|err| {
                GenerateCertificateError(format!(
                    "Cannot generate Certificate (host: {}: error: {:?})",
                    hostname, err
                ))
            })?;

            // When SNI is provided (DNS case), we intentionally do NOT add extra
            // SANs like localhost/loopbacks by default. If users need more SANs
            // here, they can still set HTTPMOCK_EXTRA_SANS explicitly; however the
            // requirement is to only add extras when there is no hostname.
            p
        };

        let key_pair = KeyPair::generate_for(&rcgen::PKCS_ECDSA_P256_SHA256).map_err(|err| {
            GenerateCertificateError(format!(
                "Cannot generate new key pair (host: {}: error: {:?})",
                hostname, err
            ))
        })?;

        let serialized_key_pair = key_pair.serialize_pem();

        // Build an issuer from the CA certificate, signing new certificates with the CA's private key
        let issuer = Issuer::from_ca_cert_pem(&self.state.ca_cert_str, &ca_key).map_err(|err| {
            GenerateCertificateError(format!(
                "Cannot create issuer from CA certificate (host: {}: error: {:?})",
                hostname, err
            ))
        })?;

        let new_host_cert = params.signed_by(&key_pair, &issuer).map_err(|err| {
            GenerateCertificateError(format!(
                "Cannot generate new host certificate (host: {}: error: {:?})",
                hostname, err
            ))
        })?;

        let cert_pem = new_host_cert.pem();

        // Convert the generated key and certificate into rustls-compatible formats
        let private_key = Self::load_private_key(serialized_key_pair).map_err(|err| {
            GenerateCertificateError(format!(
                "Cannot convert generated key pair to private key for host (host: {}: error: {:?})",
                hostname, err
            ))
        })?;

        let certificates = Self::load_certificates(cert_pem).map_err(|err| {
            GenerateCertificateError(format!(
                "Cannot convert generated generated cert PEN to list of certificates (host: {}: error: {:?})",
                hostname, err
            ))
        })?;

        let signing_key = any_supported_type(&private_key).map_err(|err| {
            GenerateCertificateError(format!(
                "Cannot convert generated private key to signing key (host: {}: error: {:?})",
                hostname, err
            ))
        })?;

        Ok(Arc::new(CertifiedKey::new(certificates, signing_key)))
    }

    fn get_lock_for_hostname(&self, hostname: &str) -> Arc<Mutex<()>> {
        let mut locks = self.state.locks.write().unwrap();
        locks
            .entry(hostname.to_string())
            .or_insert_with(|| Arc::new(Mutex::new(())))
            .clone()
    }

    fn generate(&self, hostname: &str) -> Result<Arc<CertifiedKey>, Error> {
        {
            let configs = self.state.certificates.read().unwrap();
            if let Some(config) = configs.get(hostname) {
                return Ok(config.clone());
            }
        }

        let lock = self.get_lock_for_hostname(hostname);
        let _guard = lock.lock();
        {
            let certs = self.state.certificates.read().unwrap();
            if let Some(bundle) = certs.get(hostname) {
                return Ok(bundle.clone());
            }
        }

        let key = self.generate_host_certificate(hostname).unwrap();
        {
            let mut certs = self.state.certificates.write().unwrap();
            certs.insert(hostname.to_string(), key.clone());
        }

        Ok(key)
    }
}

// Parses HTTPMOCK_EXTRA_SANS (comma-separated) into SAN entries. Non-IP tokens are treated as DNS names.
fn parse_extra_sans_from_env() -> Vec<SanType> {
    let raw = match std::env::var("HTTPMOCK_EXTRA_SANS") {
        Ok(v) => v,
        Err(_) => return Vec::new(),
    };
    let mut out = Vec::new();
    for item in raw.split(',').map(|s| s.trim()).filter(|s| !s.is_empty()) {
        if let Ok(ip) = item.parse::<std::net::IpAddr>() {
            out.push(SanType::IpAddress(ip));
        } else if let Ok(dns) = <rcgen::string::Ia5String as std::convert::TryFrom<&str>>::try_from(item) {
            out.push(SanType::DnsName(dns));
        }
    }
    out
}

// Deduplicate SANs before pushing extras.
fn push_unique_sans(target: &mut Vec<SanType>, extras: Vec<SanType>) {
    for e in extras {
        let exists = target.iter().any(|t| match (t, &e) {
            (SanType::DnsName(a), SanType::DnsName(b)) => a == b,
            (SanType::IpAddress(a), SanType::IpAddress(b)) => a == b,
            _ => false,
        });
        if !exists {
            target.push(e);
        }
    }
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

            return Some(
                self.generate(hostname)
                    .unwrap_or_else(|_| panic!("Cannot generate certificate for host {}", hostname)),
            );
        }

        // According to https://datatracker.ietf.org/doc/html/rfc6066#section-3
        // clients may choose to not include a server name (SNI extension) in TLS ClientHello
        // messages. If there is no SNI extension, we assume the client used an IP address instead
        // of a hostname.
        let hostname = self
            .authority_ip()
            .map(|ip| ip.to_string())
            .unwrap_or_else(|| "0.0.0.0".to_string());
        tracing::debug!("no hostname using: {}", hostname);
        Some(self.generate(&hostname).expect("Cannot generate fallback certificate"))
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

// Collect all local interface IP addresses (IPv4 and IPv6), excluding unspecified.
fn collect_local_ips() -> Vec<std::net::IpAddr> {
    let mut out = Vec::new();
    if let Ok(ifaces) = if_addrs::get_if_addrs() {
        for iface in ifaces {
            let ip = iface.ip();
            if !ip.is_unspecified() && !out.contains(&ip) {
                out.push(ip);
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use std::{sync::Arc, time::Duration};

    use rustls::{
        ClientConfig, ClientConnection, RootCertStore, crypto::ring::default_provider, pki_types::ServerName,
    };
    use tokio::{
        io::{AsyncReadExt, AsyncWriteExt},
        net::{TcpListener, TcpStream},
        time::timeout,
    };

    use super::starts_with_tls_handshake;

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
}
