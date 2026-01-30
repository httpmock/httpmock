#[cfg(feature = "record")]
use std::path::PathBuf;
use std::{error::Error, sync::Arc};

#[cfg(feature = "proxy")]
use crate::common::http::{HttpClient, HttpMockHttpClient};
#[cfg(feature = "record")]
use crate::server::persistence::read_static_mock_definitions;
use crate::server::{
    handler::HttpMockHandler,
    server::{MockServer, MockServerConfig},
    state::{HttpMockStateManager, StateManager},
    HttpMockServer,
};
#[cfg(feature = "https")]
use crate::server::{
    server::MockServerHttpsConfig,
    tls::{CertificateResolverFactory, GeneratingCertificateResolverFactory},
};

const DEFAULT_CA_PRIVATE_KEY: &'static str = include_str!("../../certs/ca.key");
const DEFAULT_CA_CERTIFICATE: &'static str = include_str!("../../certs/ca.pem");

/// The Builder streamlines the configuration process, automatically setting up defaults and
/// handling dependency injection for the mock server. It consolidates configuration parameters,
/// fallback mechanisms, and default settings into a single point of management.
#[cfg(feature = "https")]
pub struct HttpsConfigBuilder {
    ca_cert: Option<String>,
    ca_key: Option<String>,
    ca_cert_path: Option<PathBuf>,
    ca_key_path: Option<PathBuf>,
    enable_https: Option<bool>,
    cert_resolver_factory: Option<Arc<dyn CertificateResolverFactory + Send + Sync>>,
}

#[cfg(feature = "https")]
impl HttpsConfigBuilder {
    fn new() -> Self {
        Self {
            ca_cert: None,
            ca_key: None,
            ca_cert_path: None,
            ca_key_path: None,
            cert_resolver_factory: None,
            enable_https: None,
        }
    }

    /// Validates the HTTPS configuration to ensure no conflicting settings are present.
    fn validate(&self) -> Result<(), Box<dyn Error>> {
        if self.enable_https.unwrap_or(true) {
            let has_ca_cert = self.ca_cert.is_some() || self.ca_key.is_some();
            let has_ca_cert_path = self.ca_cert_path.is_some() || self.ca_key_path.is_some();
            let has_cert_generator = self.cert_resolver_factory.is_some();

            if has_ca_cert && has_ca_cert_path {
                return Err("A CA certificate and a CA certificate path have both been configured. Please choose only one method.".into());
            }

            if (has_ca_cert || has_ca_cert_path) && has_cert_generator {
                return Err("Both a CA certificate and a certificate generator were configured. Please use only one of them.".into());
            }
        }

        Ok(())
    }

    /// Sets the CA certificate for HTTPS.
    ///
    /// # Parameters
    /// - `ca_cert`: An optional CA certificate as a string in PEM format.
    ///
    /// # Returns
    /// A modified `HttpsConfigBuilder` instance for method chaining.
    pub fn ca_cert<IntoString>(mut self, ca_cert: Option<IntoString>) -> Self
    where
        IntoString: Into<String>,
    {
        self.ca_cert = ca_cert.map(|b| b.into());
        self
    }

    /// Sets the CA private key for HTTPS.
    ///
    /// # Parameters
    /// - `ca_key`: An optional CA private key as a string in PEM format.
    ///
    /// # Returns
    /// A modified `HttpsConfigBuilder` instance for method chaining.
    pub fn ca_key<IntoString>(mut self, ca_key: Option<IntoString>) -> Self
    where
        IntoString: Into<String>,
    {
        self.ca_key = ca_key.map(|b| b.into());
        self
    }

    /// Sets the path to the CA certificate for HTTPS.
    ///
    /// # Parameters
    /// - `ca_cert_path`: An optional path to the CA certificate in PEM format.
    ///
    /// # Returns
    /// A modified `HttpsConfigBuilder` instance for method chaining.
    pub fn ca_cert_path(mut self, ca_cert_path: Option<PathBuf>) -> Self {
        self.ca_cert_path = ca_cert_path;
        self
    }

    /// Sets the path to the CA private key for HTTPS.
    ///
    /// # Parameters
    /// - `ca_key_path`: An optional path to the CA private key.
    ///
    /// # Returns
    /// A modified `HttpsConfigBuilder` instance for method chaining.
    pub fn ca_key_path(mut self, ca_key_path: Option<PathBuf>) -> Self {
        self.ca_key_path = ca_key_path;
        self
    }

    /// Sets the certificate resolver factory for generating certificates.
    ///
    /// # Parameters
    /// - `generator`: An optional certificate resolver factory.
    ///
    /// # Returns
    /// A modified `HttpsConfigBuilder` instance for method chaining.
    pub(crate) fn cert_resolver(
        mut self,
        generator: Option<Arc<dyn CertificateResolverFactory + Send + Sync>>,
    ) -> Self {
        self.cert_resolver_factory = generator;
        self
    }

    /// Enables or disables HTTPS.
    ///
    /// # Parameters
    /// - `enable`: An optional boolean to enable or disable HTTPS.
    ///
    /// # Returns
    /// A modified `HttpsConfigBuilder` instance for method chaining.

    /// Builds the `MockServerHttpsConfig` with the current settings.
    ///
    /// # Returns
    /// A `MockServerHttpsConfig` instance or an error if validation fails.
    pub fn build(mut self) -> Result<MockServerHttpsConfig, Box<dyn Error>> {
        self.validate()?;

        let cert_resolver_factory = match (
            self.cert_resolver_factory,
            self.ca_cert_path,
            self.ca_key_path,
            self.ca_cert,
            self.ca_key,
        ) {
            // If a direct resolver was provided, use it.
            (Some(cert_resolver), _, _, _, _) => cert_resolver,
            // If paths are provided, read the certificates and create a default resolver
            // with these certs.
            (_, Some(ca_cert_path), Some(ca_key_path), _, _) => {
                let ca_cert = std::fs::read_to_string(ca_cert_path)?;
                let ca_key = std::fs::read_to_string(ca_key_path)?;
                Arc::new(GeneratingCertificateResolverFactory::new(ca_cert, ca_key)?)
            }
            // If certificate data is directly provided, use it to create the resolver.
            (_, _, _, Some(ca_cert), Some(ca_key)) => Arc::new(
                GeneratingCertificateResolverFactory::new(ca_cert.clone(), ca_key.clone())?,
            ),
            // If no CA certificate information was configured, use the default.
            _ => Arc::new(GeneratingCertificateResolverFactory::new(
                DEFAULT_CA_CERTIFICATE,
                DEFAULT_CA_PRIVATE_KEY,
            )?),
        };

        Ok(MockServerHttpsConfig {
            cert_resolver_factory,
        })
    }
}

/// The `HttpMockServerBuilder` struct is used to configure the HTTP mock server.
/// It provides methods to set various configuration options such as port, logging, history limit, and HTTPS settings.
pub struct HttpMockServerBuilder {
    port: Option<u16>,
    expose: Option<bool>,
    print_access_log: Option<bool>,
    history_limit: Option<usize>,
    #[cfg(feature = "record")]
    static_mock_dir: Option<PathBuf>,
    #[cfg(feature = "https")]
    https_config_builder: HttpsConfigBuilder,
    #[cfg(feature = "proxy")]
    http_client: Option<Arc<dyn HttpClient + Send + Sync + 'static>>,
}

impl HttpMockServerBuilder {
    /// Creates a new instance of `HttpMockServerBuilder` with default settings.
    ///
    /// # Returns
    /// A new `HttpMockServerBuilder` instance.
    pub fn new() -> Self {
        HttpMockServerBuilder {
            print_access_log: None,
            port: None,
            expose: None,
            history_limit: None,
            #[cfg(feature = "record")]
            static_mock_dir: None,
            #[cfg(feature = "proxy")]
            http_client: None,
            #[cfg(feature = "https")]
            https_config_builder: HttpsConfigBuilder::new(),
        }
    }

    /// Sets the port for the HTTP mock server.
    ///
    /// # Parameters
    /// - `port`: The port number.
    ///
    /// # Returns
    /// A modified `HttpMockServerBuilder` instance for method chaining.
    pub fn port(mut self, port: u16) -> Self {
        self.port = Some(port);
        self
    }

    /// Sets the port for the HTTP mock server as an optional value.
    ///
    /// # Parameters
    /// - `port`: An optional port number.
    ///
    /// # Returns
    /// A modified `HttpMockServerBuilder` instance for method chaining.
    pub fn port_option(mut self, port: Option<u16>) -> Self {
        self.port = port;
        self
    }

    /// Sets whether the server should be exposed to external access.
    ///
    /// # Parameters
    /// - `expose`: A boolean indicating whether to expose the server.
    ///
    /// # Returns
    /// A modified `HttpMockServerBuilder` instance for method chaining.
    pub fn expose(mut self, expose: bool) -> Self {
        self.expose = Some(expose);
        self
    }

    /// Sets whether the server should be exposed to external access as an optional value.
    ///
    /// # Parameters
    /// - `expose`: An optional boolean indicating whether to expose the server.
    ///
    /// # Returns
    /// A modified `HttpMockServerBuilder` instance for method chaining.
    pub fn expose_option(mut self, expose: Option<bool>) -> Self {
        self.expose = expose;
        self
    }

    /// Sets whether to print access logs.
    ///
    /// # Parameters
    /// - `enabled`: A boolean indicating whether to print access logs.
    ///
    /// # Returns
    /// A modified `HttpMockServerBuilder` instance for method chaining.
    pub fn print_access_log(mut self, enabled: bool) -> Self {
        self.print_access_log = Some(enabled);
        self
    }

    /// Sets whether to print access logs as an optional value.
    ///
    /// # Parameters
    /// - `enabled`: An optional boolean indicating whether to print access logs.
    ///
    /// # Returns
    /// A modified `HttpMockServerBuilder` instance for method chaining.
    pub fn print_access_log_option(mut self, enabled: Option<bool>) -> Self {
        self.print_access_log = enabled;
        self
    }

    /// Sets the history limit for the server.
    ///
    /// # Parameters
    /// - `limit`: The maximum number of history entries to keep.
    ///
    /// # Returns
    /// A modified `HttpMockServerBuilder` instance for method chaining.
    pub fn history_limit(mut self, limit: usize) -> Self {
        self.history_limit = Some(limit);
        self
    }

    /// Sets the history limit for the server as an optional value.
    ///
    /// # Parameters
    /// - `limit`: An optional maximum number of history entries to keep.
    ///
    /// # Returns
    /// A modified `HttpMockServerBuilder` instance for method chaining.
    pub fn history_limit_option(mut self, limit: Option<usize>) -> Self {
        self.history_limit = limit;
        self
    }

    /// Sets the directory for static mock files.
    ///
    /// # Parameters
    /// - `path`: The path to the static mock directory.
    ///
    /// # Returns
    /// A modified `HttpMockServerBuilder` instance for method chaining.
    #[cfg(feature = "record")]
    pub fn static_mock_dir(mut self, path: PathBuf) -> Self {
        self.static_mock_dir = Some(path);
        self
    }

    /// Sets the directory for static mock files as an optional value.
    ///
    /// # Parameters
    /// - `path`: An optional path to the static mock directory.
    ///
    /// # Returns
    /// A modified `HttpMockServerBuilder` instance for method chaining.
    #[cfg(feature = "record")]
    pub fn static_mock_dir_option(mut self, path: Option<PathBuf>) -> Self {
        self.static_mock_dir = path;
        self
    }

    /// Sets the certificate resolver factory for generating certificates.
    ///
    /// # Parameters
    /// - `factory`: A certificate resolver factory.
    ///
    /// # Returns
    /// A modified `HttpMockServerBuilder` instance for method chaining.
    #[cfg(feature = "https")]
    pub fn server_config_factory(
        mut self,
        factory: Arc<GeneratingCertificateResolverFactory>,
    ) -> Self {
        self.https_config_builder = self.https_config_builder.cert_resolver(Some(factory));
        self
    }

    /// Sets the certificate resolver factory as an optional value.
    ///
    /// # Parameters
    /// - `factory`: An optional certificate resolver factory.
    ///
    /// # Returns
    /// A modified `HttpMockServerBuilder` instance for method chaining.
    #[cfg(feature = "https")]
    pub fn cert_resolver_option(
        mut self,
        factory: Option<Arc<dyn CertificateResolverFactory + Send + Sync>>,
    ) -> Self {
        self.https_config_builder = self.https_config_builder.cert_resolver(factory);
        self
    }

    /// Sets the CA certificate and private key for HTTPS.
    ///
    /// # Parameters
    /// - `cert`: The CA certificate.
    /// - `private_key`: The CA private key.
    ///
    /// # Returns
    /// A modified `HttpMockServerBuilder` instance for method chaining.
    #[cfg(feature = "https")]
    pub fn https_ca_key_pair<IntoString: Into<String>>(
        mut self,
        cert: IntoString,
        private_key: IntoString,
    ) -> Self {
        self.https_config_builder = self.https_config_builder.ca_cert(Some(cert));
        self.https_config_builder = self.https_config_builder.ca_key(Some(private_key));
        self
    }

    /// Sets the CA certificate and private key for HTTPS as optional values.
    ///
    /// # Parameters
    /// - `cert`: An optional CA certificate.
    /// - `private_key`: An optional CA private key.
    ///
    /// # Returns
    /// A modified `HttpMockServerBuilder` instance for method chaining.
    #[cfg(feature = "https")]
    pub fn https_ca_key_pair_option<IntoString: Into<String>>(
        mut self,
        cert: Option<IntoString>,
        private_key: Option<IntoString>,
    ) -> Self {
        self.https_config_builder = self.https_config_builder.ca_cert(cert);
        self.https_config_builder = self.https_config_builder.ca_key(private_key);
        self
    }

    /// Sets the paths to the CA certificate and private key files for HTTPS.
    ///
    /// # Parameters
    /// - `cert_path`: The path to the CA certificate file.
    /// - `private_key_path`: The path to the CA private key file.
    ///
    /// # Returns
    /// A modified `HttpMockServerBuilder` instance for method chaining.
    #[cfg(feature = "https")]
    pub fn https_ca_key_pair_files<Path: Into<PathBuf>>(
        mut self,
        cert_path: Path,
        private_key_path: Path,
    ) -> Self {
        self.https_config_builder = self
            .https_config_builder
            .ca_cert_path(Some(cert_path.into()));
        self.https_config_builder = self
            .https_config_builder
            .ca_key_path(Some(private_key_path.into()));
        self
    }

    /// Sets the paths to the CA certificate and private key files for HTTPS as optional values.
    ///
    /// # Parameters
    /// - `cert_path`: An optional path to the CA certificate file.
    /// - `private_key_path`: An optional path to the CA private key file.
    ///
    /// # Returns
    /// A modified `HttpMockServerBuilder` instance for method chaining.
    #[cfg(feature = "https")]
    pub fn https_ca_key_pair_files_option<Path: Into<PathBuf>>(
        mut self,
        cert_path: Option<Path>,
        private_key_path: Option<Path>,
    ) -> Self {
        let cert_path = cert_path.map(|b| b.into());
        let private_key_path = private_key_path.map(|b| b.into());

        self.https_config_builder = self.https_config_builder.ca_cert_path(cert_path);
        self.https_config_builder = self.https_config_builder.ca_key_path(private_key_path);
        self
    }

    /// Builds the `HttpMockServer` with the current settings.
    ///
    /// # Returns
    /// A `HttpMockServer` instance or an error if the build process fails.
    pub fn build(self) -> Result<HttpMockServer, Box<dyn Error>> {
        self.build_with_state(Arc::new(HttpMockStateManager::default()))
    }

    /// Builds the `MockServer` with the current settings and provided state manager.
    ///
    /// # Parameters
    /// - `state`: The state manager to use.
    ///
    /// # Returns
    /// A `MockServer` instance or an error if the build process fails.
    pub(crate) fn build_with_state<S>(
        self,
        state: Arc<S>,
    ) -> Result<MockServer<HttpMockHandler<S>>, Box<dyn Error>>
    where
        S: StateManager + Send + Sync + 'static,
    {
        #[cfg(feature = "proxy")]
        let http_client = self
            .http_client
            .unwrap_or_else(|| Arc::new(HttpMockHttpClient::new(None)));

        #[cfg(feature = "record")]
        if let Some(dir) = self.static_mock_dir {
            read_static_mock_definitions(dir, state.as_ref())?;
        }

        let handler = HttpMockHandler::new(
            state,
            #[cfg(feature = "proxy")]
            http_client,
        );

        Ok(MockServer::new(
            Box::new(handler),
            MockServerConfig {
                static_port: self.port,
                expose: self.expose.unwrap_or(false),
                print_access_log: self.print_access_log.unwrap_or(false),
                #[cfg(feature = "https")]
                https: self.https_config_builder.build()?,
            },
        )?)
    }
}
