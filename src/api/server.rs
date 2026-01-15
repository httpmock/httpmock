#[cfg(feature = "record")]
use std::path::PathBuf;
use std::{
    cell::Cell,
    future::pending,
    net::SocketAddr,
    rc::Rc,
    sync::{Arc, LazyLock},
    thread,
};

use async_object_pool::Pool;
use tokio::sync::oneshot::channel;

#[cfg(feature = "remote")]
use crate::api::RemoteMockServerAdapter;
#[cfg(feature = "record")]
use crate::api::{
    common::data::RecordingRuleConfig,
    mock::MockSet,
    proxy::{Recording, RecordingRuleBuilder},
};
#[cfg(feature = "remote")]
use crate::common::http::HttpMockHttpClient;
#[cfg(feature = "proxy")]
use crate::{
    api::proxy::{ForwardingRule, ForwardingRuleBuilder, ProxyRule, ProxyRuleBuilder},
    common::data::{ForwardingRuleConfig, ProxyRuleConfig},
};
use crate::{
    api::{
        spec::{Then, When},
        LocalMockServerAdapter, MockServerAdapter,
    },
    common::{
        data::{MockDefinition, MockServerHttpResponse, RequestRequirements},
        runtime,
        util::{read_env, with_retry, Join},
    },
    server::{state::HttpMockStateManager, HttpMockServerBuilder},
    Mock,
};

/// Represents a mock server designed to simulate HTTP server behaviors for testing purposes.
/// This server intercepts HTTP requests and can be configured to return predetermined responses.
/// It is used extensively in automated tests to validate client behavior without the need for a live server,
/// ensuring that applications behave as expected in controlled environments.
///
/// The mock server allows developers to:
/// - Specify expected HTTP requests using a variety of matching criteria such as path, method, headers, and body content.
/// - Define corresponding HTTP responses including status codes, headers, and body data.
/// - Monitor and verify that the expected requests are made by the client under test.
/// - Simulate various network conditions and server responses, including errors and latencies.
pub struct MockServer {
    pub(crate) server_adapter: Option<Arc<dyn MockServerAdapter + Send + Sync>>,
    pool: Arc<Pool<Arc<dyn MockServerAdapter + Send + Sync>>>,
}

impl MockServer {
    async fn from(
        server_adapter: Arc<dyn MockServerAdapter + Send + Sync>,
        pool: Arc<Pool<Arc<dyn MockServerAdapter + Send + Sync>>>,
    ) -> Self {
        let server = Self {
            server_adapter: Some(server_adapter),
            pool,
        };

        server.reset_async().await;

        return server;
    }

    /// Asynchronously connects to a remote mock server running in standalone mode.
    ///
    /// # Arguments
    /// * `address` - A string slice representing the address in the format "<host>:<port>", e.g., "127.0.0.1:8080".
    ///
    /// # Returns
    /// An instance of `Self` representing the connected mock server.
    ///
    /// # Panics
    /// This method will panic if the address cannot be parsed, resolved to an IPv4 address, or if the mock server is unreachable.
    ///
    /// # Note
    /// This method requires the `remote` feature to be enabled.
    #[cfg(feature = "remote")]
    pub async fn connect_async(address: &str) -> Self {
        use std::net::ToSocketAddrs;
        let addr = address
            .to_socket_addrs()
            .expect("Cannot parse address")
            .find(|addr| addr.is_ipv4())
            .expect("Not able to resolve the provided host name to an IPv4 address");

        let adapter = REMOTE_SERVER_POOL_REF
            .take_or_create(|| {
                Arc::new(RemoteMockServerAdapter::new(
                    addr,
                    REMOTE_SERVER_CLIENT.clone(),
                ))
            })
            .await;
        Self::from(adapter, REMOTE_SERVER_POOL_REF.clone()).await
    }

    /// Synchronously connects to a remote mock server running in standalone mode.
    ///
    /// # Arguments
    /// * `address` - A string slice representing the address in the format "<host>:<port>", e.g., "127.0.0.1:8080".
    ///
    /// # Returns
    /// An instance of `Self` representing the connected mock server.
    ///
    /// # Panics
    /// This method will panic if the address cannot be parsed, resolved to an IPv4 address, or if the mock server is unreachable.
    ///
    /// # Note
    /// This method requires the `remote` feature to be enabled.
    #[cfg(feature = "remote")]
    pub fn connect(address: &str) -> Self {
        Self::connect_async(address).join()
    }

    /// Asynchronously connects to a remote mock server running in standalone mode
    /// using connection parameters stored in the `HTTPMOCK_HOST` and `HTTPMOCK_PORT`
    /// environment variables.
    ///
    /// # Returns
    /// An instance of `Self` representing the connected mock server.
    ///
    /// # Panics
    /// This method will panic if the `HTTPMOCK_PORT` environment variable cannot be
    /// parsed to an integer or if the connection fails.
    ///
    /// # Note
    /// This method requires the `remote` feature to be enabled.
    ///
    /// # Environment Variables
    /// * `HTTPMOCK_HOST` - The hostname or IP address of the mock server (default: "127.0.0.1").
    /// * `HTTPMOCK_PORT` - The port number of the mock server (default: "5050").
    #[cfg(feature = "remote")]
    pub async fn connect_from_env_async() -> Self {
        let host = read_env("HTTPMOCK_HOST", "127.0.0.1");
        let port = read_env("HTTPMOCK_PORT", "5050")
            .parse::<u16>()
            .expect("Cannot parse environment variable HTTPMOCK_PORT to an integer");
        Self::connect_async(&format!("{}:{}", host, port)).await
    }

    /// Synchronously connects to a remote mock server running in standalone mode
    /// using connection parameters stored in the `HTTPMOCK_HOST` and `HTTPMOCK_PORT`
    /// environment variables.
    ///
    /// # Returns
    /// An instance of `Self` representing the connected mock server.
    ///
    /// # Panics
    /// This method will panic if the `HTTPMOCK_PORT` environment variable cannot be
    /// parsed to an integer or if the connection fails.
    ///
    /// # Note
    /// This method requires the `remote` feature to be enabled.
    #[cfg(feature = "remote")]
    pub fn connect_from_env() -> Self {
        Self::connect_from_env_async().join()
    }

    /// Starts a new `MockServer` asynchronously.
    ///
    /// # Attention
    /// This library manages a pool of `MockServer` instances in the background.
    /// Instead of always starting a new mock server, a `MockServer` instance is
    /// only created on demand if there is no free `MockServer` instance in the pool
    /// and the pool has not reached its maximum size yet. Otherwise, **THIS METHOD WILL BLOCK**
    /// the executing function until a free mock server is available.
    ///
    /// This approach allows running many tests in parallel without exhausting
    /// the executing machine by creating too many mock servers.
    ///
    /// A `MockServer` instance is automatically taken from the pool whenever this method is called.
    /// The instance is put back into the pool automatically when the corresponding
    /// `MockServer` variable goes out of scope.
    ///
    /// # Returns
    /// An instance of `Self` representing the started mock server.
    /// ```
    pub async fn start_async() -> Self {
        let adapter = LOCAL_SERVER_POOL_REF
            .take_or_create(LOCAL_SERVER_ADAPTER_GENERATOR)
            .await;
        Self::from(adapter, LOCAL_SERVER_POOL_REF.clone()).await
    }

    /// Starts a new `MockServer` synchronously.
    ///
    /// Attention: This library manages a pool of `MockServer` instances in the background.
    /// Instead of always starting a new mock server, a `MockServer` instance is only created
    /// on demand if there is no free `MockServer` instance in the pool and the pool has not
    /// reached a maximum size yet. Otherwise, *THIS METHOD WILL BLOCK* the executing function
    /// until a free mock server is available.
    ///
    /// This allows to run many tests in parallel, but will prevent exhaust the executing
    /// machine by creating too many mock servers.
    ///
    /// A `MockServer` instance is automatically taken from the pool whenever this method is called.
    /// The instance is put back into the pool automatically when the corresponding
    /// 'MockServer' variable gets out of scope.
    pub fn start() -> MockServer {
        Self::start_async().join()
    }

    /// Returns the hostname of the `MockServer`.
    ///
    /// By default, this is `127.0.0.1`. In standalone mode, the hostname will be
    /// the host where the standalone mock server is running.
    ///
    /// # Returns
    /// A `String` representing the hostname of the `MockServer`.
    ///
    /// # Example
    /// ```rust
    /// use httpmock::MockServer;
    ///
    /// let server = MockServer::start();
    /// let host = server.host();
    ///
    /// assert_eq!(host, "127.0.0.1");
    /// ```
    pub fn host(&self) -> String {
        self.server_adapter.as_ref().unwrap().host()
    }

    /// Returns the TCP port that the mock server is listening on.
    ///
    /// # Returns
    /// A `u16` representing the port number of the `MockServer`.
    ///
    /// # Example
    /// ```rust
    /// use httpmock::MockServer;
    ///
    /// let server = MockServer::start();
    /// let port = server.port();
    ///
    /// assert!(port > 0);
    /// ```
    pub fn port(&self) -> u16 {
        self.server_adapter.as_ref().unwrap().port()
    }

    /// Builds the address for a specific path on the mock server.
    ///
    /// # Returns
    /// A reference to the `SocketAddr` representing the address of the `MockServer`.
    ///
    /// # Example
    /// ```rust
    /// // Start a local mock server for exclusive use by this test function.
    /// let server = httpmock::MockServer::start();
    ///
    /// let expected_addr_str = format!("127.0.0.1:{}", server.port());
    ///
    /// // Get the address of the MockServer.
    /// let addr = server.address();
    ///
    /// // Ensure the returned URL is as expected.
    /// assert_eq!(expected_addr_str, addr.to_string());
    /// ```
    pub fn address(&self) -> &SocketAddr {
        self.server_adapter.as_ref().unwrap().address()
    }

    /// Builds the URL for a specific path on the mock server.
    ///
    /// # Arguments
    /// * `path` - A string slice representing the specific path on the mock server.
    ///
    /// # Returns
    /// A `String` representing the full URL for the given path on the `MockServer`.
    ///
    /// # Example
    /// ```rust
    /// // Start a local mock server for exclusive use by this test function.
    /// let server = httpmock::MockServer::start();
    ///
    /// let expected_url = format!("https://127.0.0.1:{}/hello", server.port());
    ///
    /// // Get the URL for path "/hello".
    /// let url = server.url("/hello");
    ///
    /// // Ensure the returned URL is as expected.
    /// assert_eq!(expected_url, url);
    /// ```
    #[cfg(feature = "https")]
    pub fn url<S: Into<String>>(&self, path: S) -> String {
        return format!("https://{}{}", self.address(), path.into());
    }

    /// Builds the URL for a specific path on the mock server.
    ///
    /// # Arguments
    /// * `path` - A string slice representing the specific path on the mock server.
    ///
    /// # Returns
    /// A `String` representing the full URL for the given path on the `MockServer`.
    ///
    /// # Example
    /// ```rust
    /// use httpmock::MockServer;
    ///
    /// // Start a local mock server for exclusive use by this test function.
    /// let server = httpmock::MockServer::start();
    ///
    /// let expected_url = format!("http://127.0.0.1:{}/hello", server.port());
    ///
    /// // Get the URL for path "/hello".
    /// let url = server.url("/hello");
    ///
    /// // Ensure the returned URL is as expected.
    /// assert_eq!(expected_url, url);
    /// ```
    #[cfg(not(feature = "https"))]
    pub fn url<S: Into<String>>(&self, path: S) -> String {
        return format!("http://{}{}", self.address(), path.into());
    }

    /// Builds the base URL for the mock server.
    ///
    /// # Returns
    /// A `String` representing the base URL of the `MockServer`.
    ///
    /// # Example
    /// ```rust
    /// use httpmock::MockServer;
    ///
    /// // Start a local mock server for exclusive use by this test function.
    /// let server = httpmock::MockServer::start();
    ///
    /// // If the "https" feature is enabled, `server.base_url` below will generate a URL
    /// // using the "https" scheme (e.g., https://127.0.0.1:34567). Otherwise, it will
    /// // use "http" (e.g., http://127.0.0.1:34567).
    /// let expected_scheme = if cfg!(feature = "https") { "https" } else { "http" };
    ///
    /// let expected_url = format!("{}://127.0.0.1:{}", expected_scheme, server.port());
    ///
    /// // Get the base URL of the MockServer.
    /// let base_url = server.base_url();
    ///
    /// // Ensure the returned URL is as expected.
    /// assert_eq!(expected_url, base_url);
    /// ```
    pub fn base_url(&self) -> String {
        self.url("")
    }

    /// Creates a [Mock](struct.Mock.html) object on the mock server.
    ///
    /// # Arguments
    /// * `config_fn` - A closure that takes a `When` and `Then` to configure the mock.
    ///
    /// # Returns
    /// A `Mock` object representing the created mock on the server.
    ///
    /// # Example
    /// ```rust
    /// use reqwest::blocking::get;
    /// use httpmock::MockServer;
    ///
    /// // Start a local mock server for exclusive use by this test function.
    /// let server = MockServer::start();
    ///
    /// // Create a mock on the server.
    /// let mock = server.mock(|when, then| {
    ///     when.path("/hello");
    ///     then.status(200);
    /// });
    ///
    /// // Send an HTTP request to the mock server. This simulates your code.
    /// get(&server.url("/hello")).unwrap();
    ///
    /// // Ensure the mock was called as expected.
    /// mock.assert();
    /// ```
    pub fn mock<F>(&self, config_fn: F) -> Mock
    where
        F: FnOnce(When, Then),
    {
        self.mock_async(config_fn).join()
    }

    /// Creates a [Mock](struct.Mock.html) object on the mock server asynchronously.
    ///
    /// # Arguments
    /// * `spec_fn` - A closure that takes a `When` and `Then` to configure the mock.
    ///
    /// # Returns
    /// A `Mock` object representing the created mock on the server.
    ///
    /// # Example
    /// ```rust
    /// use reqwest::get;
    /// use httpmock::MockServer;
    ///
    /// let rt = tokio::runtime::Runtime::new().unwrap();
    /// rt.block_on(async {
    ///     let server = MockServer::start();
    ///
    ///     let mock = server
    ///         .mock_async(|when, then| {
    ///             when.path("/hello");
    ///             then.status(200);
    ///         })
    ///         .await;
    ///
    ///     get(&server.url("/hello")).await.unwrap();
    ///
    ///     mock.assert_async().await;
    /// });
    /// ```
    pub async fn mock_async<'a, SpecFn>(&'a self, spec_fn: SpecFn) -> Mock<'a>
    where
        SpecFn: FnOnce(When, Then),
    {
        let req = Rc::new(Cell::new(RequestRequirements::new()));
        let res = Rc::new(Cell::new(MockServerHttpResponse::new()));

        spec_fn(
            When {
                expectations: req.clone(),
            },
            Then {
                response_template: res.clone(),
            },
        );

        let response = self
            .server_adapter
            .as_ref()
            .unwrap()
            .create_mock(&MockDefinition {
                request: req.take(),
                response: res.take(),
            })
            .await
            .expect("Cannot deserialize mock server response");

        Mock {
            id: response.id,
            server: self,
        }
    }

    /// Resets the mock server. More specifically, it deletes all [Mock](struct.Mock.html) objects
    /// from the mock server and clears its request history.
    ///
    /// # Example
    /// ```rust
    /// use reqwest::blocking::get;
    /// use httpmock::MockServer;
    ///
    /// let server = MockServer::start();
    ///
    /// let mock = server.mock(|when, then| {
    ///     when.path("/hello");
    ///     then.status(200);
    /// });
    ///
    /// let response = get(&server.url("/hello")).unwrap();
    /// assert_eq!(response.status(), 200);
    ///
    /// server.reset();
    ///
    /// let response = get(&server.url("/hello")).unwrap();
    /// assert_eq!(response.status(), 404);
    /// ```
    pub fn reset(&self) {
        self.reset_async().join()
    }

    /// Resets the mock server. More specifically, it deletes all [Mock](struct.Mock.html) objects
    /// from the mock server and clears its request history.
    ///
    /// # Example
    /// ```rust
    /// use reqwest::get;
    /// use httpmock::MockServer;
    ///
    /// let rt = tokio::runtime::Runtime::new().unwrap();
    /// rt.block_on(async {
    ///     let server = MockServer::start_async().await;
    ///
    ///     let mock = server.mock_async(|when, then| {
    ///         when.path("/hello");
    ///         then.status(200);
    ///     }).await;
    ///
    ///     let response = get(&server.url("/hello")).await.unwrap();
    ///     assert_eq!(response.status(), 200);
    ///
    ///     server.reset_async().await;
    ///
    ///     let response = get(&server.url("/hello")).await.unwrap();
    ///     assert_eq!(response.status(), 404);
    /// });
    /// ```
    pub async fn reset_async(&self) {
        if let Some(server_adapter) = &self.server_adapter {
            with_retry(3, || server_adapter.reset())
                .await
                .expect("Cannot reset mock server (task: delete mocks).");
        }
    }

    /// Configures the mock server to forward the request to the target host by replacing the host name,
    /// but only if the request expectations are met. If the request is recorded, the recording will
    /// **NOT** contain the host name as an expectation to allow the recording to be reused.
    ///
    /// # Arguments
    /// * `to_base_url` - A string that represents the base URL to which the request should be forwarded.
    /// * `rule` - A closure that takes a `ForwardingRuleBuilder` to configure the forwarding rule.
    ///
    /// # Returns
    /// A `ForwardingRule` object representing the configured forwarding rule.
    ///
    /// # Example
    /// ```rust
    /// use httpmock::prelude::*;
    /// use reqwest::blocking::Client;
    ///
    /// // We will create this mock server to simulate a real service (e.g., GitHub, AWS, etc.).
    /// let target_server = MockServer::start();
    /// target_server.mock(|when, then| {
    ///     when.any_request();
    ///     then.status(200).body("Hi from fake GitHub!");
    /// });
    ///
    /// // Let's create our mock server for the test
    /// let server = MockServer::start();
    ///
    /// // We configure our server to forward the request to the target host instead of
    /// // answering with a mocked response. The 'rule' variable lets you configure
    /// // rules under which forwarding should take place.
    /// server.forward_to(target_server.base_url(), |rule| {
    ///     rule.filter(|when| {
    ///         when.any_request(); // We want all requests to be forwarded.
    ///     });
    /// });
    ///
    /// // Now let's send an HTTP request to the mock server. The request will be forwarded
    /// // to the target host, as we configured before.
    /// let client = Client::new();
    ///
    /// // Since the request was forwarded, we should see the target host's response.
    /// let response = client.get(&server.url("/get")).send().unwrap();
    /// let status = response.status();
    ///
    /// assert_eq!("Hi from fake GitHub!", response.text().unwrap());
    /// assert_eq!(status, 200);
    /// ```
    ///
    /// # Feature
    /// This method is only available when the `proxy` feature is enabled.
    #[cfg(feature = "proxy")]
    pub fn forward_to<IntoString, ForwardingRuleBuilderFn>(
        &self,
        to_base_url: IntoString,
        rule: ForwardingRuleBuilderFn,
    ) -> ForwardingRule
    where
        ForwardingRuleBuilderFn: FnOnce(ForwardingRuleBuilder),
        IntoString: Into<String>,
    {
        self.forward_to_async(to_base_url, rule).join()
    }

    /// Asynchronously configures the mock server to forward the request to the target host by replacing the host name,
    /// but only if the request expectations are met. If the request is recorded, the recording will
    /// contain the host name as an expectation to allow the recording to be reused.
    ///
    /// # Arguments
    /// * `target_base_url` - A string that represents the base URL to which the request should be forwarded.
    /// * `rule` - A closure that takes a `ForwardingRuleBuilder` to configure the forwarding rule.
    ///
    /// # Returns
    /// A `ForwardingRule` object representing the configured forwarding rule.
    ///
    /// # Example
    /// ```rust
    /// use httpmock::prelude::*;
    /// use reqwest::Client;
    ///
    /// let rt = tokio::runtime::Runtime::new().unwrap();
    /// rt.block_on(async {
    ///     // We will create this mock server to simulate a real service (e.g., GitHub, AWS, etc.).
    ///     let target_server = MockServer::start_async().await;
    ///     target_server.mock_async(|when, then| {
    ///         when.any_request();
    ///         then.status(200).body("Hi from fake GitHub!");
    ///     }).await;
    ///
    ///     // Let's create our mock server for the test
    ///     let server = MockServer::start_async().await;
    ///
    ///     // We configure our server to forward the request to the target host instead of
    ///     // answering with a mocked response. The 'rule' variable lets you configure
    ///     // rules under which forwarding should take place.
    ///     server.forward_to_async(target_server.base_url(), |rule| {
    ///         rule.filter(|when| {
    ///             when.any_request(); // We want all requests to be forwarded.
    ///         });
    ///     }).await;
    ///
    ///     // Now let's send an HTTP request to the mock server. The request will be forwarded
    ///     // to the target host, as we configured before.
    ///     let client = Client::new();
    ///
    ///     // Since the request was forwarded, we should see the target host's response.
    ///     let response = client.get(&server.url("/get")).send().await.unwrap();
    ///     let status = response.status();
    ///     assert_eq!(status, 200);
    ///     assert_eq!("Hi from fake GitHub!", response.text().await.unwrap());
    /// });
    /// ```
    ///
    /// # Feature
    /// This method is only available when the `proxy` feature is enabled.
    #[cfg(feature = "proxy")]
    pub async fn forward_to_async<'a, IntoString, ForwardingRuleBuilderFn>(
        &'a self,
        target_base_url: IntoString,
        rule: ForwardingRuleBuilderFn,
    ) -> ForwardingRule<'a>
    where
        ForwardingRuleBuilderFn: FnOnce(ForwardingRuleBuilder),
        IntoString: Into<String>,
    {
        let mut headers = Rc::new(Cell::new(Vec::new()));
        let mut req = Rc::new(Cell::new(RequestRequirements::new()));

        rule(ForwardingRuleBuilder {
            headers: headers.clone(),
            request_requirements: req.clone(),
        });

        let response = self
            .server_adapter
            .as_ref()
            .unwrap()
            .create_forwarding_rule(ForwardingRuleConfig {
                target_base_url: target_base_url.into(),
                request_requirements: req.take(),
                request_header: headers.take(),
            })
            .await
            .expect("Cannot deserialize mock server response");

        ForwardingRule {
            id: response.id,
            server: self,
        }
    }

    /// Configures the mock server to proxy HTTP requests based on specified criteria.
    ///
    /// This method configures the mock server to forward incoming requests to the target host
    /// when the requests meet the defined criteria. If a request matches the criteria, it will be
    /// proxied to the target host.
    ///
    /// When a recording is active (which records requests and responses), the host name of the request
    /// will be stored with the recording as a request expectation.
    ///
    /// # Arguments
    /// * `rule` - A closure that takes a `ProxyRuleBuilder` to configure the proxy rule.
    ///
    /// # Returns
    /// A `ProxyRule` object representing the configured proxy rule that is stored on the mock server.
    ///
    /// # Example
    /// ```rust
    /// use httpmock::prelude::*;
    /// use reqwest::blocking::Client;
    ///
    /// // Create a mock server to simulate a real service (e.g., GitHub, AWS, etc.).
    /// let target_server = MockServer::start();
    /// target_server.mock(|when, then| {
    ///     when.any_request();
    ///     then.status(200).body("Hi from fake GitHub!");
    /// });
    ///
    /// // Create a proxy mock server for the test.
    /// let proxy_server = MockServer::start();
    ///
    /// // Configure the proxy server to forward requests to the target server.
    /// // The `rule` closure allows specifying criteria for requests that should be proxied.
    /// proxy_server.proxy(|rule| {
    ///     rule.filter(|when| {
    ///         // Only allow requests to the target server to be proxied.
    ///         when.host(target_server.host()).port(target_server.port());
    ///     });
    /// });
    ///
    /// // Create an HTTP client configured to use the proxy server.
    /// let client = Client::builder()
    ///     .proxy(reqwest::Proxy::all(proxy_server.base_url()).unwrap()) // Set the proxy server
    ///     .build()
    ///     .unwrap();
    ///
    /// // Send a request to the target server through the proxy server.
    /// // The request will be forwarded to the target server as configured.
    /// let response = client.get(&target_server.url("/get")).send().unwrap();
    /// let status = response.status();
    ///
    /// // Verify that the response comes from the target server.
    /// assert_eq!(status, 200);
    /// assert_eq!("Hi from fake GitHub!", response.text().unwrap());
    /// ```
    ///
    /// # Feature
    /// This method is only available when the `proxy` feature is enabled.
    #[cfg(feature = "proxy")]
    pub fn proxy<ProxyRuleBuilderFn>(&self, rule: ProxyRuleBuilderFn) -> ProxyRule
    where
        ProxyRuleBuilderFn: FnOnce(ProxyRuleBuilder),
    {
        self.proxy_async(rule).join()
    }

    /// Asynchronously configures the mock server to proxy HTTP requests based on specified criteria.
    ///
    /// This method configures the mock server to forward incoming requests to the target host
    /// when the requests meet the defined criteria. If a request matches the criteria, it will be
    /// proxied to the target host.
    ///
    /// When a recording is active (which records requests and responses), the host name of the request
    /// will be stored with the recording to allow the recording to be reused.
    ///
    /// # Arguments
    /// * `rule` - A closure that takes a `ProxyRuleBuilder` to configure the proxy rule.
    ///
    /// # Returns
    /// A `ProxyRule` object representing the configured proxy rule.
    ///
    /// # Example
    /// ```rust
    /// use httpmock::prelude::*;
    /// use reqwest::Client;
    ///
    /// let rt = tokio::runtime::Runtime::new().unwrap();
    /// rt.block_on(async {
    ///     // We will create this mock server to simulate a real service (e.g., GitHub, AWS, etc.).
    ///     let target_server = MockServer::start_async().await;
    ///     target_server.mock_async(|when, then| {
    ///         when.any_request();
    ///         then.status(200).body("Hi from fake GitHub!");
    ///     }).await;
    ///
    ///     // Let's create our proxy mock server for the test
    ///     let proxy_server = MockServer::start_async().await;
    ///
    ///     // We configure our proxy server to forward requests to the target server
    ///     // The 'rule' closure allows specifying criteria for requests that should be proxied
    ///     proxy_server.proxy_async(|rule| {
    ///         rule.filter(|when| {
    ///             // Only allow requests to the target server to be proxied
    ///             when.host(target_server.host()).port(target_server.port());
    ///         });
    ///     }).await;
    ///
    ///     // Create an HTTP client configured to use the proxy server
    ///     let client = Client::builder()
    ///         .proxy(reqwest::Proxy::all(proxy_server.base_url()).unwrap())
    ///         .build()
    ///         .unwrap();
    ///
    ///     // Send a request to the target server through the proxy server
    ///     // The request will be forwarded to the target server as configured
    ///     let response = client.get(&target_server.url("/get")).send().await.unwrap();
    ///     let status = response.status();
    ///
    ///     // Verify that the response comes from the target server
    ///     assert_eq!(status, 200);
    ///     assert_eq!("Hi from fake GitHub!", response.text().await.unwrap());
    /// });
    /// ```
    ///
    /// # Feature
    /// This method is only available when the `proxy` feature is enabled.
    #[cfg(feature = "proxy")]
    pub async fn proxy_async<'a, ProxyRuleBuilderFn>(
        &'a self,
        rule: ProxyRuleBuilderFn,
    ) -> ProxyRule<'a>
    where
        ProxyRuleBuilderFn: FnOnce(ProxyRuleBuilder),
    {
        let mut headers = Rc::new(Cell::new(Vec::new()));
        let mut req = Rc::new(Cell::new(RequestRequirements::new()));

        rule(ProxyRuleBuilder {
            headers: headers.clone(),
            request_requirements: req.clone(),
        });

        let response = self
            .server_adapter
            .as_ref()
            .unwrap()
            .create_proxy_rule(ProxyRuleConfig {
                request_requirements: req.take(),
                request_header: headers.take(),
            })
            .await
            .expect("Cannot deserialize mock server response");

        ProxyRule {
            id: response.id,
            server: self,
        }
    }

    /// Records all requests matching a given rule and the corresponding responses
    /// sent back by the mock server. If requests are forwarded or proxied to another
    /// host, the original responses from those target hosts will also be recorded.
    ///
    /// # Parameters
    ///
    /// * `rule`: A closure that takes a `RecordingRuleBuilder` as an argument,
    ///           which defines the conditions under which HTTP requests and
    ///           their corresponding responses will be recorded.
    ///
    /// # Returns
    ///
    /// * `Recording`: A reference to the recording object stored on the mock server,
    ///                which can be used to manage the recording, such as downloading
    ///                or deleting it. The `Recording` object provides functionality
    ///                to download the recording and store it under a file. Users can
    ///                use these files for later playback by calling the `playback`
    ///                method of the mock server.
    ///
    /// # Example
    ///
    /// ```rust
    /// // Create a mock server to simulate a real service (e.g., GitHub, AWS, etc.).
    /// use reqwest::blocking::Client;
    /// use httpmock::MockServer;
    ///
    /// let target_server = MockServer::start();
    /// target_server.mock(|when, then| {
    ///     when.any_request();
    ///     then.status(200).body("Hi from fake GitHub!");
    /// });
    ///
    /// // Create the recording server for the test.
    /// let recording_server = MockServer::start();
    ///
    /// // Configure the recording server to forward requests to the target host.
    /// recording_server.forward_to(target_server.base_url(), |rule| {
    ///     rule.filter(|when| {
    ///         when.path("/hello"); // Forward all requests with path "/hello".
    ///     });
    /// });
    ///
    /// // Record the target server's response.
    /// let recording = recording_server.record(|rule| {
    ///     rule.record_response_delays(true)
    ///         .record_request_headers(vec!["Accept", "Content-Type"]) // Record specific headers.
    ///         .filter(|when| {
    ///             when.path("/hello"); // Only record requests with path "/hello".
    ///         });
    /// });
    ///
    /// // Use httpmock as a proxy server.
    /// let github_client = Client::new();
    ///
    /// let response = github_client
    ///     .get(&format!("{}/hello", recording_server.base_url()))
    ///     .send()
    ///     .unwrap();
    /// assert_eq!(response.text().unwrap(), "Hi from fake GitHub!");
    ///
    /// // Store the recording to a file and create a new mock server to playback the recording.
    /// let target_path = recording.save("my_test_scenario").unwrap();
    ///
    /// let playback_server = MockServer::start();
    ///
    /// playback_server.playback(target_path);
    ///
    /// let response = github_client
    ///     .get(&format!("{}/hello", playback_server.base_url()))
    ///     .send()
    ///     .unwrap();
    /// assert_eq!(response.text().unwrap(), "Hi from fake GitHub!");
    /// ```
    ///
    /// # Feature
    ///
    /// This method is only available when the `record` feature is enabled.
    #[cfg(feature = "record")]
    pub fn record<RecordingRuleBuilderFn>(&self, rule: RecordingRuleBuilderFn) -> Recording
    where
        RecordingRuleBuilderFn: FnOnce(RecordingRuleBuilder),
    {
        self.record_async(rule).join()
    }

    /// Asynchronously records all requests matching a given rule and the corresponding responses
    /// sent back by the mock server. If requests are forwarded or proxied to another
    /// host, the original responses from those target hosts will also be recorded.
    ///
    /// # Parameters
    ///
    /// * `rule`: A closure that takes a `RecordingRuleBuilder` as an argument,
    ///           which defines the conditions under which requests will be recorded.
    ///
    /// # Returns
    ///
    /// * `Recording`: A reference to the recording object stored on the mock server,
    ///                which can be used to manage the recording, such as downloading
    ///                or deleting it. The `Recording` object provides functionality
    ///                to download the recording and store it under a file. Users can
    ///                use these files for later playback by calling the `playback`
    ///                method of the mock server.
    ///
    /// # Example
    ///
    /// ```rust
    /// use httpmock::MockServer;
    /// use reqwest::Client;
    ///
    /// let rt = tokio::runtime::Runtime::new().unwrap();
    /// rt.block_on(async {
    ///     // Create a mock server to simulate a real service (e.g., GitHub, AWS, etc.).
    ///     let target_server = MockServer::start_async().await;
    ///     target_server.mock_async(|when, then| {
    ///         when.any_request();
    ///         then.status(200).body("Hi from fake GitHub!");
    ///     }).await;
    ///
    ///     // Create the recording server for the test.
    ///     let recording_server = MockServer::start_async().await;
    ///
    ///     // Configure the recording server to forward requests to the target host.
    ///     recording_server.forward_to_async(target_server.base_url(), |rule| {
    ///         rule.filter(|when| {
    ///             when.path("/hello"); // Forward all requests with path "/hello".
    ///         });
    ///     }).await;
    ///
    ///     // Record the target server's response.
    ///     let recording = recording_server.record_async(|rule| {
    ///         rule.record_response_delays(true)
    ///             .record_request_headers(vec!["Accept", "Content-Type"]) // Record specific headers.
    ///             .filter(|when| {
    ///                 when.path("/hello"); // Only record requests with path "/hello".
    ///             });
    ///     }).await;
    ///
    ///     // Use httpmock as a proxy server.
    ///     let client = Client::new();
    ///
    ///     let response = client
    ///         .get(&format!("{}/hello", recording_server.base_url()))
    ///         .send()
    ///         .await
    ///         .unwrap();
    ///     assert_eq!(response.text().await.unwrap(), "Hi from fake GitHub!");
    ///
    ///     // Store the recording to a file and create a new mock server to playback the recording.
    ///     let target_path = recording.save_async("my_test_scenario").await.unwrap();
    ///
    ///     let playback_server = MockServer::start_async().await;
    ///
    ///     playback_server.playback_async(target_path).await;
    ///
    ///     let response = client
    ///         .get(&format!("{}/hello", playback_server.base_url()))
    ///         .send()
    ///         .await
    ///         .unwrap();
    ///     assert_eq!(response.text().await.unwrap(), "Hi from fake GitHub!");
    /// });
    /// ```
    ///
    /// # Feature
    ///
    /// This method is only available when the `record` feature is enabled.
    #[cfg(feature = "record")]
    pub async fn record_async<'a, RecordingRuleBuilderFn>(
        &'a self,
        rule: RecordingRuleBuilderFn,
    ) -> Recording<'a>
    where
        RecordingRuleBuilderFn: FnOnce(RecordingRuleBuilder),
    {
        let mut config = Rc::new(Cell::new(RecordingRuleConfig {
            request_requirements: RequestRequirements::new(),
            record_headers: Vec::new(),
            record_response_delays: false,
        }));

        rule(RecordingRuleBuilder {
            config: config.clone(),
        });

        let response = self
            .server_adapter
            .as_ref()
            .unwrap()
            .create_recording(config.take())
            .await
            .expect("Cannot deserialize mock server response");

        Recording {
            id: response.id,
            server: self,
        }
    }

    /// Reads a recording file and configures the mock server to respond with the
    /// recorded responses when an incoming request matches the corresponding recorded HTTP request.
    /// This allows users to record responses from a real service and use these recordings for testing later,
    /// without needing to be online or having access to the real service during subsequent tests.
    ///
    /// # Parameters
    ///
    /// * `path`: A path to the file containing the recording. This can be any type
    ///           that implements `Into<PathBuf>`, such as a `&str` or `String`.
    ///
    /// # Returns
    ///
    /// * `MockSet`: An object representing the set of mocks that were loaded from the recording file.
    ///
    /// # Example
    ///
    /// ```rust
    /// // Create a mock server to simulate a real service (e.g., GitHub, AWS, etc.).
    /// use reqwest::blocking::Client;
    /// use httpmock::MockServer;
    ///
    /// let target_server = MockServer::start();
    /// target_server.mock(|when, then| {
    ///     when.any_request();
    ///     then.status(200).body("Hi from fake GitHub!");
    /// });
    ///
    /// // Create the recording server for the test.
    /// let recording_server = MockServer::start();
    ///
    /// // Configure the recording server to forward requests to the target host.
    /// recording_server.forward_to(target_server.base_url(), |rule| {
    ///     rule.filter(|when| {
    ///         when.path("/hello"); // Forward all requests with path "/hello".
    ///     });
    /// });
    ///
    /// // Record the target server's response.
    /// let recording = recording_server.record(|rule| {
    ///     rule.record_response_delays(true)
    ///         .record_request_headers(vec!["Accept", "Content-Type"]) // Record specific headers.
    ///         .filter(|when| {
    ///             when.path("/hello"); // Only record requests with path "/hello".
    ///         });
    /// });
    ///
    /// // Use httpmock as a proxy server.
    /// let client = Client::new();
    ///
    /// let response = client
    ///     .get(&format!("{}/hello", recording_server.base_url()))
    ///     .send()
    ///     .unwrap();
    /// assert_eq!(response.text().unwrap(), "Hi from fake GitHub!");
    ///
    /// // Store the recording to a file and create a new mock server to play back the recording.
    /// let target_path = recording.save("my_test_scenario").unwrap();
    ///
    /// let playback_server = MockServer::start();
    ///
    /// // Play back the recorded interactions from the file.
    /// playback_server.playback(target_path);
    ///
    /// let response = client
    ///     .get(&format!("{}/hello", playback_server.base_url()))
    ///     .send()
    ///     .unwrap();
    /// assert_eq!(response.text().unwrap(), "Hi from fake GitHub!");
    /// ```
    ///
    /// # Feature
    ///
    /// This method is only available when the `record` feature is enabled.
    #[cfg(feature = "record")]
    pub fn playback<IntoPathBuf: Into<PathBuf>>(&self, path: IntoPathBuf) -> MockSet {
        self.playback_async(path).join()
    }

    /// Asynchronously reads a recording file and configures the mock server to respond with the
    /// recorded responses when an incoming request matches the corresponding recorded HTTP request.
    /// This allows users to record responses from a real service and use these recordings for testing later,
    /// without needing to be online or having access to the real service during subsequent tests.
    ///
    /// # Parameters
    ///
    /// * `path`: A path to the file containing the recorded interactions. This can be any type
    ///           that implements `Into<PathBuf>`, such as a `&str` or `String`.
    ///
    /// # Returns
    ///
    /// * `MockSet`: An object representing the set of mocks that were loaded from the recording file.
    ///
    /// # Example
    ///
    /// ```rust
    /// use httpmock::MockServer;
    /// use reqwest::Client;
    ///
    /// let rt = tokio::runtime::Runtime::new().unwrap();
    /// rt.block_on(async {
    ///     // Create a mock server to simulate a real service (e.g., GitHub, AWS, etc.).
    ///     let target_server = MockServer::start_async().await;
    ///     target_server.mock_async(|when, then| {
    ///         when.any_request();
    ///         then.status(200).body("Hi from fake GitHub!");
    ///     }).await;
    ///
    ///     // Create the recording server for the test.
    ///     let recording_server = MockServer::start_async().await;
    ///
    ///     // Configure the recording server to forward requests to the target host.
    ///     recording_server.forward_to_async(target_server.base_url(), |rule| {
    ///         rule.filter(|when| {
    ///             when.path("/hello"); // Forward all requests with path "/hello".
    ///         });
    ///     }).await;
    ///
    ///     // Record the target server's response.
    ///     let recording = recording_server.record_async(|rule| {
    ///         rule.record_response_delays(true)
    ///             .record_request_headers(vec!["Accept", "Content-Type"]) // Record specific headers.
    ///             .filter(|when| {
    ///                 when.path("/hello"); // Only record requests with path "/hello".
    ///             });
    ///     }).await;
    ///
    ///     // Use httpmock as a proxy server.
    ///     let client = Client::new();
    ///
    ///     let response = client
    ///         .get(&format!("{}/hello", recording_server.base_url()))
    ///         .send()
    ///         .await
    ///         .unwrap();
    ///     assert_eq!(response.text().await.unwrap(), "Hi from fake GitHub!");
    ///
    ///     // Store the recording to a file and create a new mock server to play back the recording.
    ///     let target_path = recording.save("my_test_scenario").unwrap();
    ///
    ///     let playback_server = MockServer::start_async().await;
    ///
    ///     playback_server.playback_async(target_path).await;
    ///
    ///     let response = client
    ///         .get(&format!("{}/hello", playback_server.base_url()))
    ///         .send()
    ///         .await
    ///         .unwrap();
    ///     assert_eq!(response.text().await.unwrap(), "Hi from fake GitHub!");
    /// });
    /// ```
    ///
    /// # Feature
    ///
    /// This method is only available when the `record` feature is enabled.
    #[cfg(feature = "record")]
    pub async fn playback_async<IntoPathBuf: Into<PathBuf>>(&self, path: IntoPathBuf) -> MockSet {
        use std::fs;

        let path = path.into();
        let content = fs::read_to_string(&path).expect(&format!(
            "could not read from file {}",
            path.as_os_str()
                .to_str()
                .map_or(String::new(), |p| p.to_string())
        ));

        return self.playback_from_yaml_async(content).await;
    }

    /// Configures the mock server to respond with the recorded responses based on a provided recording
    /// in the form of a YAML string.  This allows users to directly use a YAML string representing
    /// the recorded interactions, which can be useful for testing and debugging without needing a physical file.
    ///
    /// # Parameters
    ///
    /// * `content`: A YAML string that represents the contents of the recording file.
    ///              This can be any type that implements `AsRef<str>`, such as a `&str` or `String`.
    ///
    /// # Returns
    ///
    /// * `MockSet`: An object representing the set of mocks that were loaded from the YAML string.
    ///
    /// # Example
    ///
    /// ```rust
    /// use httpmock::MockServer;
    /// use reqwest::blocking::Client;
    ///
    /// // Example YAML content representing recorded interactions.
    /// let yaml_content = r#"
    /// when:
    ///   method: GET
    ///   path: /recorded-mock
    /// then:
    ///   status: 200
    ///   header:
    ///     - name: Content-Type
    ///       value: application/json
    ///   body: '{ "response" : "hello" }'
    /// "#;
    ///
    /// // Create the mock server.
    /// let mock_server = MockServer::start();
    ///
    /// // Play back the recorded interactions from the YAML string.
    /// mock_server.playback_from_yaml(yaml_content);
    ///
    /// // Use the reqwest HTTP client to send a request to the mock server.
    /// let client = Client::new();
    ///
    /// let response = client
    ///     .get(&format!("{}/recorded-mock", mock_server.base_url())) // Build the full URL using the mock server's base URL
    ///     .send() // Send the GET request
    ///     .unwrap(); // Unwrap the result, assuming the request is successful
    ///
    /// assert_eq!(response.headers().get("Content-Type").unwrap(), "application/json");
    /// assert_eq!(response.text().unwrap(), r#"{ "response" : "hello" }"#);
    /// ```
    ///
    /// # Feature
    ///
    /// This method is only available when the `record` feature is enabled.
    #[cfg(feature = "record")]
    pub fn playback_from_yaml<AsStrRef: AsRef<str>>(&self, content: AsStrRef) -> MockSet {
        self.playback_from_yaml_async(content).join()
    }

    /// Asynchronously configures the mock server to respond with the recorded responses based on a provided recording
    /// in the form of a YAML string.  This allows users to directly use a YAML string representing
    /// the recorded interactions, which can be useful for testing and debugging without needing a physical file.
    ///
    /// # Parameters
    ///
    /// * `content`: A YAML string that represents the contents of the recording file.
    ///              This can be any type that implements `AsRef<str>`, such as a `&str` or `String`.
    ///
    /// # Returns
    ///
    /// * `MockSet`: An object representing the set of mocks that were loaded from the YAML string.
    ///
    /// # Example
    ///
    /// ```rust
    /// use tokio::runtime::Runtime; // Import tokio for asynchronous runtime
    /// use httpmock::MockServer;
    /// use reqwest::Client;
    ///
    /// // Example YAML content representing a recording.
    /// let yaml_content = r#"
    /// when:
    ///   method: GET
    ///   path: /recorded-mock
    /// then:
    ///   status: 200
    ///   body: '{ "response" : "hello" }'
    /// "#;
    ///
    /// let rt = Runtime::new().unwrap();
    /// rt.block_on(async {
    ///     // Create the mock server.
    ///     let mock_server = MockServer::start_async().await;
    ///
    ///     // Play back the recorded interactions from the YAML string.
    ///     mock_server.playback_from_yaml_async(yaml_content).await;
    ///
    ///     // Use reqwest to send an asynchronous request to the mock server.
    ///     let client = Client::new();
    ///
    ///     let response = client
    ///         .get(&format!("{}/recorded-mock", mock_server.base_url()))
    ///         .send()
    ///         .await
    ///         .unwrap();
    ///
    ///     assert_eq!(response.text().await.unwrap(), r#"{ "response" : "hello" }"#);
    /// });
    /// ```
    ///
    /// # Feature
    ///
    /// This method is only available when the `record` feature is enabled.
    #[cfg(feature = "record")]
    pub async fn playback_from_yaml_async<AsStrRef: AsRef<str>>(
        &self,
        content: AsStrRef,
    ) -> MockSet {
        let response = self
            .server_adapter
            .as_ref()
            .unwrap()
            .create_mocks_from_recording(content.as_ref())
            .await
            .expect("Cannot deserialize mock server response");

        MockSet {
            ids: response,
            server: self,
        }
    }
}

/// Implements the `Drop` trait for `MockServer`.
/// When a `MockServer` instance goes out of scope, this method is called automatically to manage resources.
impl Drop for MockServer {
    /// This method will returns the mock server to the pool of mock servers. The mock server is not cleaned immediately.
    /// Instead, it will be reset and cleaned when `MockServer::start()` is called again, preparing it for reuse by another test.
    ///
    /// # Important Considerations
    ///
    /// Users should be aware that when a `MockServer` instance is dropped, the server is not immediately cleaned.
    /// The actual reset and cleaning of the server happen when `MockServer::start()` is called again, making it ready for reuse.
    ///
    /// # Feature
    ///
    /// This behavior is part of the `MockServer` struct and does not require any additional features to be enabled.
    fn drop(&mut self) {
        let adapter = self.server_adapter.take().unwrap();
        self.pool.put(adapter).join();
    }
}

const LOCAL_SERVER_ADAPTER_GENERATOR: fn() -> Arc<dyn MockServerAdapter + Send + Sync> = || {
    let (addr_sender, addr_receiver) = channel::<SocketAddr>();
    let state_manager = Arc::new(HttpMockStateManager::default());
    let srv = HttpMockServerBuilder::new()
        .build_with_state(state_manager.clone())
        .expect("cannot build mock server");

    // TODO: Check how we can improve here to not create a Tokio runtime on the current thread per MockServer.
    //  Can we create one runtime and use it for all servers?
    thread::spawn(move || {
        let server_fn = srv.start_with_signals(Some(addr_sender), pending());
        runtime::block_on_current_thread(server_fn).expect("Server execution failed");
    });

    let addr = addr_receiver.join().expect("Cannot get server address");
    Arc::new(LocalMockServerAdapter::new(addr, state_manager))
};

static LOCAL_SERVER_POOL_REF: LazyLock<Arc<Pool<Arc<dyn MockServerAdapter + Send + Sync>>>> =
    LazyLock::new(|| {
        let max_servers = read_env("HTTPMOCK_MAX_SERVERS", "25")
            .parse::<usize>()
            .expect("Cannot parse environment variable HTTPMOCK_MAX_SERVERS as an integer");
        Arc::new(Pool::new(max_servers))
    });

static REMOTE_SERVER_POOL_REF: LazyLock<Arc<Pool<Arc<dyn MockServerAdapter + Send + Sync>>>> =
    LazyLock::new(|| Arc::new(Pool::new(1)));

#[cfg(feature = "remote")]
// TODO: REFACTOR to use a runtime agnostic HTTP client for remote access.
//  This solution does not require OpenSSL and less dependencies compared to
//  other HTTP clients (tested: isahc, surf). Curl seems to use OpenSSL by default,
//  so this is not an option. Optimally, the HTTP client uses rustls to avoid the
//  dependency on OpenSSL installed on the OS.
static REMOTE_SERVER_CLIENT: LazyLock<Arc<HttpMockHttpClient>> = LazyLock::new(|| {
    let max_workers = read_env("HTTPMOCK_HTTP_CLIENT_WORKER_THREADS", "1")
        .parse::<usize>()
        .expect(
            "Cannot parse environment variable HTTPMOCK_HTTP_CLIENT_WORKER_THREADS as an integer",
        );
    let max_blocking_threads = read_env("HTTPMOCK_HTTP_CLIENT_MAX_BLOCKING_THREADS", "10")
        .parse::<usize>()
        .expect("Cannot parse environment variable HTTPMOCK_HTTP_CLIENT_MAX_BLOCKING_THREADS to an integer");
    Arc::new(HttpMockHttpClient::new(Some(Arc::new(
        runtime::new(max_workers, max_blocking_threads).unwrap(),
    ))))
});
