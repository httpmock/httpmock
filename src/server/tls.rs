use std::{
    collections::HashMap,
    fmt::Debug,
    io::Cursor,
    net::SocketAddr,
    sync::{Arc, Mutex, RwLock},
};

use async_trait::async_trait;
use rcgen::{Certificate, CertificateParams, KeyPair, SanType};
use rustls::{
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

impl<'a> GeneratingCertificateResolverFactory {
    pub fn new<IntoString: Into<String>>(
        ca_cert: IntoString,
        ca_key: IntoString,
    ) -> Result<Self, Error> {
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
        let mut cert_pem_reader = Cursor::new(cert_pem.into_bytes());
        let mut certificates = Vec::new();
        let certs_iterator = CertificateDer::pem_reader_iter(cert_pem_reader);
        for cert_result in certs_iterator {
            let cert = cert_result.map_err(|err| {
                GenerateCertificateError(format!("cannot use generated certificate: {:?}", err))
            })?; // Propagate error if any
            certificates.push(cert);
        }

        Ok(certificates)
    }

    fn load_private_key(key_pem: String) -> Result<PrivateKeyDer<'a>, Error> {
        let mut cert_pem_reader = Cursor::new(key_pem.into_bytes());
        let private_key = PrivateKeyDer::from_pem_reader(cert_pem_reader).map_err(|err| {
            GenerateCertificateError(format!("cannot use generated private key: {:?}", err))
        })?;
        Ok(private_key)
    }

    fn authority_ip(&self) -> Option<std::net::IpAddr> {
        let auth = self.authority.as_deref()?;

        // 1) Full socket address like "127.0.0.1:8080" or "[::1]:443"
        if let Ok(sa) = auth.parse::<std::net::SocketAddr>() {
            return Some(sa.ip());
        }

        // 2) Bracketed IPv6 without port: "[::1]"
        if let Some(inner) = auth.strip_prefix('[').and_then(|s| s.strip_suffix(']')) {
            if let Ok(ip) = inner.parse::<std::net::IpAddr>() {
                return Some(ip);
            }
        }

        // 3) Parse as HTTP authority and take the host (handles bracketed IPv6 and ports)
        if let Ok(a) = auth.parse::<http::uri::Authority>() {
            if let Ok(ip) = a.host().parse::<std::net::IpAddr>() {
                return Some(ip);
            }
        }

        // 4) Plain IP literal (v4 or v6)
        if let Ok(ip) = auth.parse::<std::net::IpAddr>() {
            return Some(ip);
        }

        // 5) Conservative host:port split only if there's exactly one ':' (avoids mangling IPv6)
        if auth.matches(':').count() == 1 {
            if let Some((host, _)) = auth.rsplit_once(':') {
                if let Ok(ip) = host.parse::<std::net::IpAddr>() {
                    return Some(ip);
                }
            }
        }

        None
    }

    pub fn generate_host_certificate(&'a self, hostname: &str) -> Result<Arc<CertifiedKey>, Error> {
        // Create a key pair for the CA from the provided PEM
        let ca_key = KeyPair::from_pem(&self.state.ca_key_str).map_err(|err| {
            CaCertificateError(format!("Expected CA key to be provided in PEM format but failed to parse it (host: {}: error: {:?})", hostname, err))
        })?;

        // Set up certificate parameters for the new certificate
        let mut params = if let Ok(ip) = hostname.parse::<std::net::IpAddr>() {
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
                    <rcgen::Ia5String as std::convert::TryFrom<&str>>::try_from("localhost")
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
            let mut p = CertificateParams::new(vec![hostname.to_owned()]).map_err(|err| {
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

        // Serialize the new certificate, signing it with the CA's private key
        let new_host_cert_params = CertificateParams::from_ca_cert_pem(&self.state.ca_cert_str).map_err(|err| {
            GenerateCertificateError(format!("Cannot create new host certificate parameters from CA certificate (host: {}: error: {:?})", hostname, err))
        })?;

        let ca_cert = new_host_cert_params.self_signed(&ca_key).map_err(|err| {
            GenerateCertificateError(format!("Cannot create new host certificate parameters from CA certificate (host: {}: error: {:?})", hostname, err))
        })?;

        let new_host_cert = params
            .signed_by(&key_pair, &ca_cert, &ca_key)
            .map_err(|err| {
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
            GenerateCertificateError(format!("Cannot convert generated generated cert PEN to list of certificates (host: {}: error: {:?})", hostname, err))
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
        } else if let Ok(dns) = <rcgen::Ia5String as std::convert::TryFrom<&str>>::try_from(item) {
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

            return Some(self.generate(hostname).expect(&format!(
                "Cannot generate certificate for host {}",
                hostname
            )));
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
        return Some(
            self.generate(&hostname)
                .expect(&format!("Cannot generate fallback certificate")),
        );
    }
}

pub struct TcpStreamPeekBuffer<'a> {
    stream: &'a tokio::net::TcpStream,
    buffer: Vec<u8>,
}

impl<'a> TcpStreamPeekBuffer<'a> {
    pub fn new(stream: &'a tokio::net::TcpStream) -> Self {
        TcpStreamPeekBuffer {
            stream,
            buffer: Vec::new(),
        }
    }

    pub fn buffer(&self) -> &[u8] {
        return &self.buffer;
    }

    pub async fn advance(&mut self, offset: usize) -> std::io::Result<()> {
        if self.buffer.len() > offset {
            return Ok(());
        }

        let required_size = offset + 1;
        if required_size > self.buffer.len() {
            self.buffer.resize(required_size, 0);
        }

        let mut total_peeked = 0;
        while total_peeked < required_size {
            let peeked_now = self.stream.peek(&mut self.buffer[total_peeked..]).await?;
            if peeked_now == 0 {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::UnexpectedEof,
                    "EOF reached before offset",
                ));
            }
            total_peeked += peeked_now;
        }

        Ok(())
    }
}

#[async_trait]
impl<'a> tls_detect::Read<'a> for TcpStreamPeekBuffer<'a> {
    async fn read_byte(&mut self, from_offset: usize) -> std::io::Result<u8> {
        self.advance(from_offset).await?;
        Ok(self.buffer[from_offset])
    }

    async fn read_bytes(
        &mut self,
        from_offset: usize,
        to_offset: usize,
    ) -> std::io::Result<Vec<u8>> {
        self.advance(to_offset).await?;
        Ok(self.buffer[from_offset..to_offset].to_vec())
    }

    async fn read_u16_from_be(&mut self, offset: usize) -> std::io::Result<u16> {
        let u16_bytes = self.read_bytes(offset, offset + 2).await?;
        Ok(u16::from_be_bytes([u16_bytes[0], u16_bytes[1]]))
    }

    async fn buffer_to(&mut self, limit: usize) -> std::io::Result<()> {
        self.advance(limit).await
    }
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
