use crate::prelude::HttpMockResponse;
use crate::{
    common::{
        data::{MockServerHttpResponse, RequestRequirements},
        util::{get_test_resource_file_path, update_cell, HttpMockBytes},
    },
    prelude::HttpMockRequest,
    Method, Regex,
};
use bytes::Bytes;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    cell::Cell, convert::TryInto, fs::read_to_string, path::Path, rc::Rc, str::FromStr, sync::Arc,
    time::Duration,
};

/// Represents the conditions that an incoming HTTP request must satisfy to be handled by the mock server.
///
/// The `When` structure is used exclusively to define the expectations for HTTP requests. It allows
/// the configuration of various aspects of the request such as paths, headers, methods, and more.
/// These specifications determine whether a request matches the mock setup and should be handled accordingly.
/// This structure is part of the setup process in creating a mock server, typically used before defining the response
/// behavior with a `Then` structure.
pub struct When {
    pub(crate) expectations: Rc<Cell<RequestRequirements>>,
}

impl When {
    /// Configures the mock server to respond to any incoming request, regardless of the URL path,
    /// query parameters, headers, or method.
    ///
    /// This method doesn't directly alter the behavior of the mock server, as it already responds to any
    /// request by default. However, it serves as an explicit indication in your code that the
    /// server will respond to any request.
    ///
    /// # Example
    ///
    /// ```rust
    /// use httpmock::prelude::*;
    ///
    /// // Start a new mock server
    /// let server = MockServer::start();
    ///
    /// // Configure the mock server to respond to any request
    /// let mock = server.mock(|when, then| {
    ///     when.any_request();  // Explicitly specify that any request should match
    ///     then.status(200);    // Respond with status code 200 for all matched requests
    /// });
    ///
    /// // Make a request to the server's URL and ensure the mock is triggered
    /// let response = reqwest::blocking::get(server.url("/anyPath")).unwrap();
    ///
    /// // Ensure the request was successful
    /// assert_eq!(response.status(), 200);
    ///
    /// // Assert that the mock was called at least once
    /// mock.assert();
    /// ```
    ///
    /// # Note
    /// This is the default behavior as of now, but it may change in future versions.
    ///
    /// # Returns
    /// The updated `When` instance to enable method chaining.
    ///
    pub fn any_request(self) -> Self {
        // This method does nothing. It only exists to make it very explicit that
        // the mock server will respond to any request. This is the default at this time, but
        // may change in the future.
        self
    }
    // @docs-group: Miscellaneous

    /// Specifies the scheme (e.g., "http" or "https") that requests must match for the mock server to respond.
    ///
    /// This method sets the scheme to filter requests and ensures that the mock server only matches
    /// requests with the specified scheme. This allows for more precise testing in environments where
    /// multiple protocols are used.
    ///
    /// **Note**: Scheme matching is case-insensitive, conforming to
    /// [RFC 3986, Section 3.2.2](https://datatracker.ietf.org/doc/html/rfc3986#section-3.2.2).
    ///
    /// # Example
    ///
    /// ```rust
    /// use httpmock::prelude::*;
    ///
    /// // Start a new mock server
    /// let server = MockServer::start();
    ///
    /// // If the "https" feature is enabled, `server.url` below will generate a URL using
    /// // the "https" scheme (e.g., https://localhost:34567/test). Otherwise, it will
    /// // use "http" (e.g., http://localhost:34567/test).
    /// let expected_scheme = if cfg!(feature = "https") { "https" } else { "http" };
    ///
    /// // Create a mock that only matches requests with the "http" scheme
    /// let mock = server.mock(|when, then| {
    ///     when.scheme(expected_scheme);  // Restrict to the "http" scheme
    ///     then.status(200);              // Respond with status code 200 for all matched requests
    /// });
    ///
    /// // Make an "http" request to the server's URL to trigger the mock
    /// let response = reqwest::blocking::get(server.url("/test")).unwrap();
    ///
    /// // Ensure the request was successful
    /// assert_eq!(response.status(), 200);
    ///
    /// // Verify that the mock was called at least once
    /// mock.assert();
    /// ```
    ///
    /// # Parameters
    /// - `scheme`: A string specifying the scheme that requests should match. Common values include "http" and "https".
    ///
    /// # Returns
    /// The modified `When` instance to allow for method chaining.
    ///
    pub fn scheme<TryIntoString: TryInto<String>>(self, scheme: TryIntoString) -> Self
    where
        <TryIntoString as TryInto<String>>::Error: std::fmt::Debug,
    {
        let scheme = scheme
            .try_into()
            .expect("cannot convert scheme into a string");
        update_cell(&self.expectations, |e| {
            e.scheme = Some(scheme);
        });
        self
    }
    // @docs-group: Scheme

    /// Specifies a scheme (e.g., "https") that requests must not match for the mock server to respond.
    ///
    /// This method allows you to exclude specific schemes from matching, ensuring that the mock server
    /// won't respond to requests using those protocols. This is useful when you want to mock server
    /// behavior based on protocol security requirements or other criteria.
    ///
    /// **Note**: Scheme matching is case-insensitive, conforming to
    /// [RFC 3986, Section 3.2.2](https://datatracker.ietf.org/doc/html/rfc3986#section-3.2.2).
    ///
    /// # Example
    ///
    /// ```rust
    /// use httpmock::prelude::*;
    ///
    /// // Start a new mock server
    /// let server = MockServer::start();
    ///
    /// // Create a mock that will only match requests that do not use the "https" scheme
    /// let mock = server.mock(|when, then| {
    ///     when.scheme_not("ftp");  // Exclude the "ftp" scheme from matching
    ///     then.status(200);        // Respond with status code 200 for all matched requests
    /// });
    ///
    /// // Make a request to the server's URL with the "http" scheme to trigger the mock
    /// let response = reqwest::blocking::get(server.url("/test")).unwrap();
    ///
    /// // Ensure the request was successful
    /// assert_eq!(response.status(), 200);
    ///
    /// // Ensure that the mock was called at least once
    /// mock.assert();
    /// ```
    ///
    /// # Parameters
    /// - `scheme`: A string specifying the scheme that requests should not match. Common values include "http" and "https".
    ///
    /// # Returns
    /// The modified `When` instance to allow for method chaining.
    ///
    pub fn scheme_not<TryIntoString: TryInto<String>>(self, scheme: TryIntoString) -> Self
    where
        <TryIntoString as TryInto<String>>::Error: std::fmt::Debug,
    {
        let scheme = scheme
            .try_into()
            .expect("cannot convert scheme into a string");
        update_cell(&self.expectations, |e| {
            e.scheme_not = Some(scheme);
        });
        self
    }
    // @docs-group: Scheme

    /// Sets the expected HTTP method for which the mock server should respond.
    ///
    /// This method ensures that the mock server only matches requests that use the specified HTTP method,
    /// such as `GET`, `POST`, or any other valid method. This allows testing behavior that's specific
    /// to different types of HTTP requests.
    ///
    /// **Note**: Method matching is case-insensitive.
    ///
    /// # Example
    ///
    /// ```rust
    /// use httpmock::prelude::*;
    ///
    /// // Start a new mock server
    /// let server = MockServer::start();
    ///
    /// // Create a mock that matches only `GET` requests
    /// let mock = server.mock(|when, then| {
    ///     when.method(GET);    // Match only `GET` HTTP method
    ///     then.status(200);    // Respond with status code 200 for all matched requests
    /// });
    ///
    /// // Make a GET request to the server's URL to trigger the mock
    /// let response = reqwest::blocking::get(server.url("/")).unwrap();
    ///
    /// // Ensure the request was successful
    /// assert_eq!(response.status(), 200);
    ///
    /// // Verify that the mock was called at least once
    /// mock.assert();
    /// ```
    ///
    /// # Parameters
    /// - `method`: An HTTP method as either a `Method` enum or a `String` value, specifying the expected method type for matching.
    ///
    /// # Returns
    /// The updated `When` instance to allow for method chaining.
    ///
    pub fn method<TryIntoMethod: TryInto<Method>>(self, method: TryIntoMethod) -> Self
    where
        <TryIntoMethod as TryInto<Method>>::Error: std::fmt::Debug,
    {
        let method = method
            .try_into()
            .expect("cannot convert method into httpmock::Method");

        update_cell(&self.expectations, |e| e.method = Some(method.to_string()));
        self
    }
    // @docs-group: Method

    /// Excludes the specified HTTP method from the requests the mock server will respond to.
    ///
    /// This method ensures that the mock server does not respond to requests using the given HTTP method,
    /// like `GET`, `POST`, etc. This allows testing scenarios where a particular method should not
    /// trigger a response, and thus testing behaviors like method-based security.
    ///
    /// **Note**: Method matching is case-insensitive.
    ///
    /// # Example
    ///
    /// ```rust
    /// use httpmock::prelude::*;
    ///
    /// // Start a new mock server
    /// let server = MockServer::start();
    ///
    /// // Create a mock that matches any request except those using the `POST` method
    /// let mock = server.mock(|when, then| {
    ///     when.method_not(POST);  // Exclude the `POST` HTTP method from matching
    ///     then.status(200);       // Respond with status code 200 for all other matched requests
    /// });
    ///
    /// // Make a GET request to the server's URL, which will trigger the mock
    /// let response = reqwest::blocking::get(server.url("/")).unwrap();
    ///
    /// // Ensure the request was successful
    /// assert_eq!(response.status(), 200);
    ///
    /// // Ensure that the mock was called at least once
    /// mock.assert();
    /// ```
    ///
    /// # Parameters
    /// - `method`: An HTTP method as either a `Method` enum or a `String` value, specifying the method type to exclude from matching.
    ///
    /// # Returns
    /// The updated `When` instance to allow for method chaining.
    ///
    pub fn method_not<IntoMethod: Into<Method>>(self, method: IntoMethod) -> Self {
        update_cell(&self.expectations, |e| {
            if e.method_not.is_none() {
                e.method_not = Some(Vec::new());
            }
            e.method_not
                .as_mut()
                .unwrap()
                .push(method.into().to_string());
        });

        self
    }
    // @docs-group: Method

    /// Sets the expected host name. This constraint is especially useful when working with
    /// proxy or forwarding rules, but it can also be used to serve mocks (e.g., when using a mock
    /// server as a proxy).
    ///
    /// **Note**: Host matching is case-insensitive, conforming to
    /// [RFC 3986, Section 3.2.2](https://datatracker.ietf.org/doc/html/rfc3986#section-3.2.2).
    /// This standard dictates that all host names are treated equivalently, regardless of character case.
    ///
    /// **Note**: Both `localhost` and `127.0.0.1` are treated equally.
    /// If the provided host is set to either `localhost` or `127.0.0.1`, it will match
    /// requests containing either `localhost` or `127.0.0.1`.
    ///
    /// * `host` - The host name (should not include a port).
    ///
    /// # Example
    /// ```rust
    /// use httpmock::prelude::*;
    /// use reqwest::blocking::Client;
    ///
    /// let server = MockServer::start();
    ///
    /// server.mock(|when, then| {
    ///     when.host("github.com");
    ///     then.body("This is a mock response");
    /// });
    ///
    /// let client = Client::builder()
    ///     .proxy(reqwest::Proxy::all(&server.base_url()).unwrap())
    ///     .build()
    ///     .unwrap();
    ///
    /// let response = client.get("http://github.com").send().unwrap();
    ///
    /// assert_eq!(response.text().unwrap(), "This is a mock response");
    /// ```
    ///
    /// # Returns
    /// The updated `When` instance to enable method chaining.
    ///
    pub fn host<IntoString: Into<String>>(self, host: IntoString) -> Self {
        update_cell(&self.expectations, |e| e.host = Some(host.into()));
        self
    }
    // @docs-group: Host

    /// Sets the host name that should **NOT** be responded for.
    ///
    /// This constraint is especially useful when working with proxy or forwarding rules, but it
    /// can also be used to serve mocks (e.g., when using a mock server as a proxy).
    ///
    /// To add multiple suffixes, invoke this function multiple times.
    ///
    /// **Note**: Host matching is case-insensitive, conforming to
    /// [RFC 3986, Section 3.2.2](https://datatracker.ietf.org/doc/html/rfc3986#section-3.2.2).
    /// This standard dictates that all host names are treated equivalently, regardless of character case.
    ///
    /// * `host` - The host name (should not include a port).
    ///
    /// # Example
    /// ```rust
    /// use httpmock::prelude::*;
    /// use reqwest::blocking::Client;
    ///
    /// let server = MockServer::start();
    ///
    /// server.mock(|when, then| {
    ///     when.host("github.com");
    ///     then.body("This is a mock response");
    /// });
    ///
    /// let client = Client::builder()
    ///     .proxy(reqwest::Proxy::all(&server.base_url()).unwrap())
    ///     .build()
    ///     .unwrap();
    ///
    /// let response = client.get("http://github.com").send().unwrap();
    ///
    /// assert_eq!(response.text().unwrap(), "This is a mock response");
    /// ```
    ///
    /// # Returns
    /// The updated `When` instance to enable method chaining.
    ///
    pub fn host_not<IntoString: Into<String>>(self, host: IntoString) -> Self {
        update_cell(&self.expectations, |e| {
            if e.host_not.is_none() {
                e.host_not = Some(Vec::new());
            }
            e.host_not.as_mut().unwrap().push(host.into());
        });
        self
    }
    // @docs-group: Host

    /// Adds a substring to match within the request's host name.
    ///
    /// This method ensures that the mock server only matches requests whose host name contains the specified substring.
    ///
    /// This constraint is especially useful when working with proxy or forwarding rules, but it
    /// can also be used to serve mocks (e.g., when using a mock server as a proxy).
    ///
    /// To add multiple substrings, invoke this function multiple times.
    ///
    /// **Note**: Host matching is case-insensitive, conforming to
    /// [RFC 3986, Section 3.2.2](https://datatracker.ietf.org/doc/html/rfc3986#section-3.2.2).
    /// This standard dictates that all host names are treated equivalently, regardless of character case.
    ///
    /// **Note**: This function does not automatically compare with pseudo names, like "localhost".
    ///
    /// # Attention
    /// This function does not automatically treat 127.0.0.1 like localhost.
    ///
    /// # Example
    ///
    /// ```rust
    /// use httpmock::prelude::*;
    ///
    /// // Start a new mock server
    /// let server = MockServer::start();
    ///
    /// // Create a mock that matches any request where the host name contains "localhost"
    /// let mock = server.mock(|when, then| {
    ///     when.host_includes("0.0");  // Only match hosts containing "0.0" (e.g., 127.0.0.1)
    ///     then.status(200);           // Respond with status code 200 for all matched requests
    /// });
    ///
    /// // Make a request to a URL whose host name is "localhost" to trigger the mock
    /// let response = reqwest::blocking::get(server.url("/test")).unwrap();
    ///
    /// // Ensure the request was successful
    /// assert_eq!(response.status(), 200);
    ///
    /// // Ensure that the mock was called at least once
    /// mock.assert();
    /// ```
    ///
    /// # Parameters
    /// - `host`: A string or other type convertible to `String` that will be added as a substring to match against the request's host name.
    ///
    /// # Returns
    /// The updated `When` instance to enable method chaining.
    ///
    pub fn host_includes<IntoString: Into<String>>(self, host: IntoString) -> Self {
        update_cell(&self.expectations, |e| {
            if e.host_contains.is_none() {
                e.host_contains = Some(Vec::new());
            }
            e.host_contains.as_mut().unwrap().push(host.into());
        });
        self
    }
    // @docs-group: Host

    /// Adds a substring that must not be present within the request's host name for the mock server to respond.
    ///
    /// This method ensures that the mock server does not respond to requests if the host name contains the specified substring.
    ///
    /// This constraint is especially useful when working with proxy or forwarding rules, but it
    /// can also be used to serve mocks (e.g., when using a mock server as a proxy).
    ///
    /// To add multiple excluded substrings, invoke this function multiple times.
    ///
    /// **Note**: Host matching is case-insensitive, conforming to
    /// [RFC 3986, Section 3.2.2](https://datatracker.ietf.org/doc/html/rfc3986#section-3.2.2).
    /// This standard dictates that all host names are treated equivalently, regardless of character case.
    ///
    /// **Note**: This function does not automatically compare with pseudo names, like "localhost".
    ///
    /// # Example
    ///
    /// ```rust
    /// use httpmock::prelude::*;
    ///
    /// // Start a new mock server
    /// let server = MockServer::start();
    ///
    /// // Create a mock that excludes any request where the host name contains "www.google.com"
    /// let mock = server.mock(|when, then| {
    ///     when.host_excludes("www.google.com");  // Exclude hosts containing "www.google.com"
    ///     then.status(200);                      // Respond with status code 200 for other matched requests
    /// });
    ///
    /// // Make a request to a URL whose host name will be "localhost" and trigger the mock
    /// let response = reqwest::blocking::get(server.url("/test")).unwrap();
    ///
    /// // Ensure the request was successful
    /// assert_eq!(response.status(), 200);
    ///
    /// // Ensure that the mock was called at least once
    /// mock.assert();
    /// ```
    ///
    /// # Parameters
    /// - `host`: A string or other type convertible to `String` that will be added as a substring to exclude from matching.
    ///
    /// # Returns
    /// The updated `When` instance to enable method chaining.
    ///
    pub fn host_excludes<IntoString: Into<String>>(self, host: IntoString) -> Self {
        update_cell(&self.expectations, |e| {
            if e.host_excludes.is_none() {
                e.host_excludes = Some(Vec::new());
            }
            e.host_excludes.as_mut().unwrap().push(host.into());
        });
        self
    }
    // @docs-group: Host

    /// Adds a prefix that the request's host name must start with for the mock server to respond.
    ///
    /// This constraint is especially useful when working with proxy or forwarding rules, but it
    /// can also be used to serve mocks (e.g., when using a mock server as a proxy).
    ///
    /// To add multiple prefixes, invoke this function multiple times.
    ///
    /// **Note**: Host matching is case-insensitive, conforming to
    /// [RFC 3986, Section 3.2.2](https://datatracker.ietf.org/doc/html/rfc3986#section-3.2.2).
    /// This standard dictates that all host names are treated equivalently, regardless of character case.
    ///
    /// **Note**: This function does not automatically compare with pseudo names, like "localhost".
    ///
    /// # Example
    ///
    /// ```rust
    /// use httpmock::prelude::*;
    ///
    /// // Start a new mock server
    /// let server = MockServer::start();
    ///
    /// // Create a mock that matches any request where the host name starts with "local"
    /// let mock = server.mock(|when, then| {
    ///     when.host_prefix("127.0");      // Only match hosts starting with "127.0"
    ///     then.status(200);               // Respond with status code 200 for all matched requests
    /// });
    ///
    /// // Make a request to the mock server with a host name of "127.0.0.1" to trigger the mock response.
    /// let response = reqwest::blocking::get(server.url("/test")).unwrap();
    ///
    /// // Ensure the request was successful
    /// assert_eq!(response.status(), 200);
    ///
    /// // Ensure that the mock was called at least once
    /// mock.assert();
    /// ```
    ///
    /// # Parameters
    /// - `prefix`: A string or other type convertible to `String` specifying the prefix that the host name should start with.
    ///
    /// # Returns
    /// The updated `When` instance to enable method chaining.
    ///
    pub fn host_prefix<IntoString: Into<String>>(self, host: IntoString) -> Self {
        update_cell(&self.expectations, |e| {
            if e.host_prefix.is_none() {
                e.host_prefix = Some(Vec::new());
            }
            e.host_prefix.as_mut().unwrap().push(host.into());
        });
        self
    }
    // @docs-group: Host

    /// Adds a suffix that the request's host name must end with for the mock server to respond.
    ///
    /// This constraint is especially useful when working with proxy or forwarding rules, but it
    /// can also be used to serve mocks (e.g., when using a mock server as a proxy).
    ///
    /// To add multiple suffixes, invoke this function multiple times.
    ///
    /// **Note**: Host matching is case-insensitive, conforming to
    /// [RFC 3986, Section 3.2.2](https://datatracker.ietf.org/doc/html/rfc3986#section-3.2.2).
    /// This standard dictates that all host names are treated equivalently, regardless of character case.
    ///
    /// **Note**: This function does not automatically compare with pseudo names, like "localhost".
    ///
    /// # Example
    ///
    /// ```rust
    /// use httpmock::prelude::*;
    ///
    /// // Start a new mock server
    /// let server = MockServer::start();
    ///
    /// // Create a mock that matches any request where the host name ends with "host" (e.g., "localhost").
    /// let mock = server.mock(|when, then| {
    ///     when.host_suffix("0.1");    // Only match hosts ending with "0.1"
    ///     then.status(200);           // Respond with status code 200 for all matched requests
    /// });
    ///
    /// // Make a request to the mock server with a host name of "127.0.0.1" to trigger the mock response.
    /// let response = reqwest::blocking::get(server.url("/test")).unwrap();
    ///
    /// // Ensure the request was successful
    /// assert_eq!(response.status(), 200);
    ///
    /// // Ensure that the mock was called at least once
    /// mock.assert();
    /// ```
    ///
    /// # Parameters
    /// - `host`: A string or other type convertible to `String` specifying the suffix that the host name should end with.
    ///
    /// # Returns
    /// The updated `When` instance to enable method chaining.
    ///
    pub fn host_suffix<IntoString: Into<String>>(self, host: IntoString) -> Self {
        update_cell(&self.expectations, |e| {
            if e.host_suffix.is_none() {
                e.host_suffix = Some(Vec::new());
            }
            e.host_suffix.as_mut().unwrap().push(host.into());
        });
        self
    }
    // @docs-group: Host

    /// Adds a prefix that the request's host name must *not* start with for the mock server to respond.
    ///
    /// This constraint is especially useful when working with proxy or forwarding rules, but it
    /// can also be used to serve mocks (e.g., when using a mock server as a proxy).
    ///
    /// To add multiple excluded prefixes, invoke this function multiple times.
    ///
    /// **Note**: Host matching is case-insensitive, conforming to
    /// [RFC 3986, Section 3.2.2](https://datatracker.ietf.org/doc/html/rfc3986#section-3.2.2).
    /// This standard dictates that all host names are treated equivalently, regardless of character case.
    ///
    /// **Note**: This function does not automatically compare with pseudo names, like "localhost".
    ///
    /// # Example
    /// ```rust
    /// use httpmock::prelude::*;
    ///
    /// // Start a new mock server
    /// let server = MockServer::start();
    ///
    /// // Create a mock that matches any request where the host name does not start with "www."
    /// let mock = server.mock(|when, then| {
    ///     when.host_prefix_not("www.");      // Exclude hosts starting with "www"
    ///     then.status(200);                  // Respond with status code 200 for all other requests
    /// });
    ///
    /// // Make a request with host name "localhost" that does not start with "www" and therefore
    /// // triggers the mock response.
    /// let response = reqwest::blocking::get(server.url("/example")).unwrap();
    ///
    /// // Ensure the request was successful
    /// assert_eq!(response.status(), 200);
    ///
    /// // Ensure that the mock was called at least once
    /// mock.assert();
    /// ```
    ///
    /// # Parameters
    /// - `prefix`: A string or other type convertible to `String` specifying the prefix that the host name should *not* start with.
    ///
    /// # Returns
    /// The updated `When` instance to enable method chaining.
    ///
    pub fn host_prefix_not<IntoString: Into<String>>(self, prefix: IntoString) -> Self {
        update_cell(&self.expectations, |e| {
            if e.host_prefix_not.is_none() {
                e.host_prefix_not = Some(Vec::new());
            }
            e.host_prefix_not.as_mut().unwrap().push(prefix.into());
        });
        self
    }
    // @docs-group: Host

    /// Adds a suffix that the request's host name must *not* end with for the mock server to respond.
    ///
    /// This constraint is especially useful when working with proxy or forwarding rules, but it
    /// can also be used to serve mocks (e.g., when using a mock server as a proxy).
    ///
    /// To add multiple excluded suffixes, invoke this function multiple times.
    ///
    /// **Note**: Host matching is case-insensitive, conforming to
    /// [RFC 3986, Section 3.2.2](https://datatracker.ietf.org/doc/html/rfc3986#section-3.2.2).
    /// This standard dictates that all host names are treated equivalently, regardless of character case.
    ///
    /// **Note**: This function does not automatically compare with pseudo names, like "localhost".
    ///
    /// # Example
    /// ```rust
    /// use httpmock::prelude::*;
    ///
    /// // Start a new mock server
    /// let server = MockServer::start();
    ///
    /// // Create a mock that matches any request where the host name does not end with "host".
    /// let mock = server.mock(|when, then| {
    ///     when.host_suffix_not("host");   // Exclude hosts ending with "host"
    ///     then.status(200);               // Respond with status code 200 for all other requests
    /// });
    ///
    /// // Make a request with a host name that does not end with "host" to trigger the mock response.
    /// let response = reqwest::blocking::get(server.url("/example")).unwrap();
    ///
    /// // Ensure the request was successful
    /// assert_eq!(response.status(), 200);
    ///
    /// // Ensure that the mock was called at least once
    /// mock.assert();
    /// ```
    ///
    /// # Parameters
    /// - `host`: A string or other type convertible to `String` specifying the suffix that the host name should *not* end with.
    ///
    /// # Returns
    /// The updated `When` instance to enable method chaining.
    ///
    pub fn host_suffix_not<IntoString: Into<String>>(self, host: IntoString) -> Self {
        update_cell(&self.expectations, |e| {
            if e.host_suffix_not.is_none() {
                e.host_suffix_not = Some(Vec::new());
            }
            e.host_suffix_not.as_mut().unwrap().push(host.into());
        });
        self
    }
    // @docs-group: Host

    /// Sets a regular expression pattern that the request's host name must match for the mock server to respond.
    ///
    /// This constraint is especially useful when working with proxy or forwarding rules, but it
    /// can also be used to serve mocks (e.g., when using a mock server as a proxy).
    ///
    /// To add multiple patterns, invoke this function multiple times.
    ///
    /// **Note**: Host matching is case-insensitive, conforming to
    /// [RFC 3986, Section 3.2.2](https://datatracker.ietf.org/doc/html/rfc3986#section-3.2.2).
    /// This standard dictates that all host names are treated equivalently, regardless of character case.
    ///
    /// **Note**: This function does not automatically compare with pseudo names, like "localhost".
    ///
    /// # Parameters
    /// - `regex`: A regular expression pattern to match against the host name. Should be a valid regex string.
    ///
    /// # Example
    /// ```rust
    /// use httpmock::prelude::*;
    ///
    /// // Start a new mock server
    /// let server = MockServer::start();
    ///
    /// // Create a mock that matches requests where the host name is exactly "localhost"
    /// let mock = server.mock(|when, then| {
    ///     when.host_matches(r"^127.0.0.1$");
    ///     then.status(200);
    /// });
    ///
    /// // Make a request with "127.0.0.1" as the host name to trigger the mock response.
    /// let response = reqwest::blocking::get(server.url("/")).unwrap();
    ///
    /// // Ensure the request was successful
    /// assert_eq!(response.status(), 200);
    ///
    /// // Verify that the mock was called at least once
    /// mock.assert();
    /// ```
    ///
    /// # Returns
    /// The updated `When` instance to enable method chaining.
    ///
    pub fn host_matches<IntoRegex: Into<Regex>>(self, regex: IntoRegex) -> Self {
        update_cell(&self.expectations, |e| {
            if e.host_matches.is_none() {
                e.host_matches = Some(Vec::new());
            }
            e.host_matches.as_mut().unwrap().push(regex.into());
        });
        self
    }
    // @docs-group: Host

    /// Specifies the expected port number for incoming requests to match.
    ///
    /// This constraint is especially useful when working with proxy or forwarding rules, but it
    /// can also be used to serve mocks (e.g., when using a mock server as a proxy).
    ///
    /// # Parameters
    /// - `port`: A value convertible to `u16`, representing the expected port number.
    ///
    /// # Example
    /// ```rust
    /// use httpmock::prelude::*;
    /// use reqwest::blocking::Client;
    ///
    /// // Start a new mock server
    /// let server = MockServer::start();
    ///
    /// // Configure a mock to respond to requests made to `github.com`
    /// // with a specific port
    /// server.mock(|when, then| {
    ///     when.port(80);  // Specify the expected port
    ///     then.body("This is a mock response");
    /// });
    ///
    /// // Set up an HTTP client to use the mock server as a proxy
    /// let client = Client::builder()
    ///     // Proxy all requests to the mock server
    ///     .proxy(reqwest::Proxy::all(&server.base_url()).unwrap())
    ///     .build()
    ///     .unwrap();
    ///
    /// // Send a GET request to `github.com` on port 80.
    /// // The request will be sent to our mock server due to the HTTP client proxy settings.
    /// let response = client.get("http://github.com:80").send().unwrap();
    ///
    /// // Validate that the mock server returned the expected response
    /// assert_eq!(response.text().unwrap(), "This is a mock response");
    /// ```
    ///
    /// # Errors
    /// - This function will panic if the port number cannot be converted to a valid `u16` value.
    ///
    /// # Returns
    /// The updated `When` instance to allow method chaining.
    ///
    pub fn port<U16: TryInto<u16>>(self, port: U16) -> Self
    where
        <U16 as TryInto<u16>>::Error: std::fmt::Debug,
    {
        let port: u16 = port.try_into().expect("Port value is out of range for u16");

        update_cell(&self.expectations, |e| e.port = Some(port));
        self
    }
    // @docs-group: Port

    /// Specifies the port number that incoming requests must *not* match.
    ///
    /// This constraint is especially useful when working with proxy or forwarding rules, but it
    /// can also be used to serve mocks (e.g., when using a mock server as a proxy).
    ///
    /// To add multiple excluded ports, invoke this function multiple times.
    ///
    /// # Parameters
    /// - `port`: A value convertible to `u16`, representing the port number to be excluded.
    ///
    /// # Example
    /// ```rust
    /// use httpmock::prelude::*;
    /// use reqwest::blocking::Client;
    ///
    /// // Start a new mock server
    /// let server = MockServer::start();
    ///
    /// // Configure a mock to respond to requests not using port 81
    /// server.mock(|when, then| {
    ///     when.port_not(81);  // Exclude requests on port 81
    ///     then.body("This is a mock response");
    /// });
    ///
    /// // Set up an HTTP client to use the mock server as a proxy
    /// let client = Client::builder()
    ///     .proxy(reqwest::Proxy::all(&server.base_url()).unwrap())
    ///     .build()
    ///     .unwrap();
    ///
    /// // Make a request to `github.com` on port 80, which will trigger
    /// // the mock response
    /// let response = client.get("http://github.com:80").send().unwrap();
    ///
    /// // Validate that the mock server returned the expected response
    /// assert_eq!(response.text().unwrap(), "This is a mock response");
    /// ```
    ///
    /// # Errors
    /// - This function will panic if the port number cannot be converted to a valid `u16` value.
    ///
    /// # Returns
    /// The updated `When` instance to enable method chaining.
    ///
    pub fn port_not<U16: TryInto<u16>>(self, port: U16) -> Self
    where
        <U16 as TryInto<u16>>::Error: std::fmt::Debug,
    {
        let port: u16 = port.try_into().expect("Port value is out of range for u16");

        update_cell(&self.expectations, |e| {
            if e.port_not.is_none() {
                e.port_not = Some(Vec::new());
            }
            e.port_not.as_mut().unwrap().push(port);
        });
        self
    }
    // @docs-group: Port

    /// Specifies the expected URL path that incoming requests must match for the mock server to respond.
    /// This is useful for targeting specific endpoints, such as API routes, to ensure only relevant requests trigger the mock response.
    ///
    /// # Parameters
    /// - `path`: A string or other value convertible to `String` that represents the expected URL path.
    ///
    /// # Example
    /// ```rust
    /// use httpmock::prelude::*;
    ///
    /// // Start a new mock server
    /// let server = MockServer::start();
    ///
    /// // Create a mock that matches requests to `/test`
    /// let mock = server.mock(|when, then| {
    ///     when.path("/test");
    ///     then.status(200);  // Respond with a 200 status code
    /// });
    ///
    /// // Make a request to the mock server using the specified path
    /// let response = reqwest::blocking::get(server.url("/test")).unwrap();
    ///
    /// // Ensure the request was successful
    /// assert_eq!(response.status(), 200);
    ///
    /// // Verify that the mock was called at least once
    /// mock.assert();
    /// ```
    ///
    /// # Returns
    /// The updated `When` instance, allowing method chaining for additional configuration.
    ///
    pub fn path<TryIntoString: TryInto<String>>(self, path: TryIntoString) -> Self
    where
        <TryIntoString as TryInto<String>>::Error: std::fmt::Debug,
    {
        let path = path.try_into().expect("cannot convert path into a string");
        update_cell(&self.expectations, |e| {
            e.path = Some(path);
        });
        self
    }
    // @docs-group: Path

    /// Specifies the URL path that incoming requests must *not* match for the mock server to respond.
    /// This is helpful when you need to exclude specific endpoints while allowing others through.
    ///
    /// To add multiple excluded paths, invoke this function multiple times.
    ///
    /// # Parameters
    /// - `path`: A string or other value convertible to `String` that represents the URL path to exclude.
    ///
    /// # Example
    /// ```rust
    /// use httpmock::prelude::*;
    ///
    /// // Start a new mock server
    /// let server = MockServer::start();
    ///
    /// // Create a mock that will not match requests to `/exclude`
    /// let mock = server.mock(|when, then| {
    ///     when.path_not("/exclude");
    ///     then.status(200);  // Respond with status 200 for all other paths
    /// });
    ///
    /// // Make a request to a path that does not match the exclusion
    /// let response = reqwest::blocking::get(server.url("/include")).unwrap();
    ///
    /// // Ensure the request was successful
    /// assert_eq!(response.status(), 200);
    ///
    /// // Verify that the mock was called at least once
    /// mock.assert();
    /// ```
    ///
    /// # Returns
    /// The updated `When` instance, allowing method chaining for further configuration.
    ///
    pub fn path_not<TryIntoString: TryInto<String>>(self, path: TryIntoString) -> Self
    where
        <TryIntoString as TryInto<String>>::Error: std::fmt::Debug,
    {
        let path = path.try_into().expect("cannot convert path into string");
        update_cell(&self.expectations, |e| {
            if e.path_not.is_none() {
                e.path_not = Some(Vec::new());
            }
            e.path_not.as_mut().unwrap().push(path);
        });
        self
    }
    // @docs-group: Path

    /// Specifies a substring that the URL path must contain for the mock server to respond.
    /// This constraint is useful for matching URLs based on partial segments, especially when exact path matching isn't required.
    ///
    /// # Parameters
    /// - `substring`: A string or any value convertible to `String` representing the substring that must be present in the URL path.
    ///
    /// # Example
    /// ```rust
    /// use httpmock::prelude::*;
    ///
    /// // Start a new mock server
    /// let server = MockServer::start();
    ///
    /// // Create a mock that matches any path containing the substring "es"
    /// let mock = server.mock(|when, then| {
    ///     when.path_includes("es");
    ///     then.status(200);  // Respond with a 200 status code for matched requests
    /// });
    ///
    /// // Make a request to a path containing "es" to trigger the mock response
    /// let response = reqwest::blocking::get(server.url("/test")).unwrap();
    ///
    /// // Ensure the request was successful
    /// assert_eq!(response.status(), 200);
    ///
    /// // Ensure that the mock was called at least once
    /// mock.assert();
    /// ```
    ///
    /// # Returns
    /// The updated `When` instance to allow method chaining for further configuration.
    ///
    pub fn path_includes<TryIntoString: TryInto<String>>(self, substring: TryIntoString) -> Self
    where
        <TryIntoString as TryInto<String>>::Error: std::fmt::Debug,
    {
        let substring = substring
            .try_into()
            .expect("cannot convert substring into string");
        update_cell(&self.expectations, |e| {
            if e.path_includes.is_none() {
                e.path_includes = Some(Vec::new());
            }
            e.path_includes.as_mut().unwrap().push(substring);
        });
        self
    }
    // @docs-group: Path

    /// Specifies a substring that the URL path must *not* contain for the mock server to respond.
    /// This constraint is useful for excluding requests to paths containing particular segments or patterns.
    ///
    /// # Parameters
    /// - `substring`: A string or other value convertible to `String` representing the substring that should not appear in the URL path.
    ///
    /// # Example
    /// ```rust
    /// use httpmock::prelude::*;
    ///
    /// // Start a new mock server
    /// let server = MockServer::start();
    ///
    /// // Create a mock that matches any path not containing the substring "xyz"
    /// let mock = server.mock(|when, then| {
    ///     when.path_excludes("xyz");
    ///     then.status(200);  // Respond with status 200 for paths excluding "xyz"
    /// });
    ///
    /// // Make a request to a path that does not contain "xyz"
    /// let response = reqwest::blocking::get(server.url("/testpath")).unwrap();
    ///
    /// // Ensure the request was successful
    /// assert_eq!(response.status(), 200);
    ///
    /// // Ensure the mock server returned the expected response
    /// mock.assert();
    /// ```
    ///
    /// # Returns
    /// The updated `When` instance to enable method chaining for additional configuration.
    ///
    pub fn path_excludes<TryIntoString: TryInto<String>>(self, substring: TryIntoString) -> Self
    where
        <TryIntoString as TryInto<String>>::Error: std::fmt::Debug,
    {
        let substring = substring
            .try_into()
            .expect("cannot convert substring into string");
        update_cell(&self.expectations, |e| {
            if e.path_excludes.is_none() {
                e.path_excludes = Some(Vec::new());
            }
            e.path_excludes.as_mut().unwrap().push(substring);
        });
        self
    }
    // @docs-group: Path

    /// Specifies a prefix that the URL path must start with for the mock server to respond.
    /// This is useful when only the initial segments of a path need to be validated, such as checking specific API routes.
    ///
    /// # Parameters
    /// - `prefix`: A string or other value convertible to `String` representing the prefix that the URL path should start with.
    ///
    /// # Example
    /// ```rust
    /// use httpmock::prelude::*;
    ///
    /// // Start a new mock server
    /// let server = MockServer::start();
    ///
    /// // Create a mock that matches any path starting with the prefix "/api"
    /// let mock = server.mock(|when, then| {
    ///     when.path_prefix("/api");
    ///     then.status(200);  // Respond with a 200 status code for matched requests
    /// });
    ///
    /// // Make a request to a path starting with "/api"
    /// let response = reqwest::blocking::get(server.url("/api/v1/resource")).unwrap();
    ///
    /// // Ensure the request was successful
    /// assert_eq!(response.status(), 200);
    ///
    /// // Verify that the mock was called at least once
    /// mock.assert();
    /// ```
    ///
    /// # Returns
    /// The updated `When` instance to allow method chaining for further configuration.
    ///
    pub fn path_prefix<TryIntoString: TryInto<String>>(self, prefix: TryIntoString) -> Self
    where
        <TryIntoString as TryInto<String>>::Error: std::fmt::Debug,
    {
        let prefix = prefix
            .try_into()
            .expect("cannot convert prefix into string");
        update_cell(&self.expectations, |e| {
            if e.path_prefix.is_none() {
                e.path_prefix = Some(Vec::new());
            }
            e.path_prefix.as_mut().unwrap().push(prefix);
        });
        self
    }
    // @docs-group: Path

    /// Specifies a suffix that the URL path must end with for the mock server to respond.
    /// This is useful when the final segments of a path need to be validated, such as file extensions or specific patterns.
    ///
    /// # Parameters
    /// - `suffix`: A string or other value convertible to `String` representing the suffix that the URL path should end with.
    ///
    /// # Example
    /// ```rust
    /// use httpmock::prelude::*;
    ///
    /// // Start a new mock server
    /// let server = MockServer::start();
    ///
    /// // Create a mock that matches any path ending with the suffix ".html"
    /// let mock = server.mock(|when, then| {
    ///     when.path_suffix(".html");
    ///     then.status(200);  // Respond with a 200 status code for matched requests
    /// });
    ///
    /// // Make a request to a path ending with ".html"
    /// let response = reqwest::blocking::get(server.url("/about/index.html")).unwrap();
    ///
    /// // Ensure the request was successful
    /// assert_eq!(response.status(), 200);
    ///
    /// // Verify that the mock was called at least once
    /// mock.assert();
    /// ```
    ///
    /// # Returns
    /// The updated `When` instance to allow method chaining for further configuration.
    ///
    pub fn path_suffix<TryIntoString: TryInto<String>>(self, suffix: TryIntoString) -> Self
    where
        <TryIntoString as TryInto<String>>::Error: std::fmt::Debug,
    {
        let suffix = suffix
            .try_into()
            .expect("cannot convert suffix into string");
        update_cell(&self.expectations, |e| {
            if e.path_suffix.is_none() {
                e.path_suffix = Some(Vec::new());
            }
            e.path_suffix.as_mut().unwrap().push(suffix);
        });
        self
    }
    // @docs-group: Path

    /// Specifies a prefix that the URL path must not start with for the mock server to respond.
    /// This constraint is useful for excluding paths that begin with particular segments or patterns.
    ///
    /// # Parameters
    /// - `prefix`: A string or other value convertible to `String` representing the prefix that the URL path should not start with.
    ///
    /// # Example
    /// ```rust
    /// use httpmock::prelude::*;
    ///
    /// // Start a new mock server
    /// let server = MockServer::start();
    ///
    /// // Create a mock that matches any path not starting with the prefix "/admin"
    /// let mock = server.mock(|when, then| {
    ///     when.path_prefix_not("/admin");
    ///     then.status(200);  // Respond with status 200 for paths excluding "/admin"
    /// });
    ///
    /// // Make a request to a path that does not start with "/admin"
    /// let response = reqwest::blocking::get(server.url("/public/home")).unwrap();
    ///
    /// // Ensure the request was successful
    /// assert_eq!(response.status(), 200);
    ///
    /// // Verify that the mock server returned the expected response
    /// mock.assert();
    /// ```
    ///
    /// # Returns
    /// The updated `When` instance to allow method chaining for additional configuration.
    ///
    pub fn path_prefix_not<TryIntoString: TryInto<String>>(self, prefix: TryIntoString) -> Self
    where
        <TryIntoString as TryInto<String>>::Error: std::fmt::Debug,
    {
        let prefix = prefix
            .try_into()
            .expect("cannot convert prefix into string");
        update_cell(&self.expectations, |e| {
            if e.path_prefix_not.is_none() {
                e.path_prefix_not = Some(Vec::new());
            }
            e.path_prefix_not.as_mut().unwrap().push(prefix);
        });
        self
    }
    // @docs-group: Path

    /// Specifies a suffix that the URL path must not end with for the mock server to respond.
    /// This constraint is useful for excluding paths with specific file extensions or patterns.
    ///
    /// # Parameters
    /// - `suffix`: A string or other value convertible to `String` representing the suffix that the URL path should not end with.
    ///
    /// # Example
    /// ```rust
    /// use httpmock::prelude::*;
    ///
    /// // Start a new mock server
    /// let server = MockServer::start();
    ///
    /// // Create a mock that matches any path not ending with the suffix ".json"
    /// let mock = server.mock(|when, then| {
    ///     when.path_suffix_not(".json");
    ///     then.status(200);  // Respond with a 200 status code for paths excluding ".json"
    /// });
    ///
    /// // Make a request to a path that does not end with ".json"
    /// let response = reqwest::blocking::get(server.url("/about/index.html")).unwrap();
    ///
    /// // Ensure the request was successful
    /// assert_eq!(response.status(), 200);
    ///
    /// // Verify that the mock was called at least once
    /// mock.assert();
    /// ```
    ///
    /// # Returns
    /// The updated `When` instance to allow method chaining for further configuration.
    ///
    pub fn path_suffix_not<TryIntoString: TryInto<String>>(self, suffix: TryIntoString) -> Self
    where
        <TryIntoString as TryInto<String>>::Error: std::fmt::Debug,
    {
        let suffix = suffix
            .try_into()
            .expect("cannot convert suffix into string");
        update_cell(&self.expectations, |e| {
            if e.path_suffix_not.is_none() {
                e.path_suffix_not = Some(Vec::new());
            }
            e.path_suffix_not.as_mut().unwrap().push(suffix);
        });
        self
    }
    // @docs-group: Path

    /// Specifies a regular expression that the URL path must match for the mock server to respond.
    /// This method allows flexible matching using regex patterns, making it useful for various matching scenarios.
    ///
    /// # Parameters
    /// - `regex`: An expression that implements `Into<Regex>`, representing the regex pattern to match against the URL path.
    ///
    /// # Example
    /// ```rust
    /// use httpmock::prelude::*;
    ///
    /// // Start a new mock server
    /// let server = MockServer::start();
    ///
    /// // Create a mock that matches paths ending with the suffix "le"
    /// let mock = server.mock(|when, then| {
    ///     when.path_matches(r"le$");
    ///     then.status(200);  // Respond with a 200 status code for paths matching the pattern
    /// });
    ///
    /// // Make a request to a path ending with "le"
    /// let response = reqwest::blocking::get(server.url("/example")).unwrap();
    ///
    /// // Ensure the request was successful
    /// assert_eq!(response.status(), 200);
    ///
    /// // Verify that the mock server returned the expected response
    /// mock.assert();
    /// ```
    ///
    /// # Returns
    /// The updated `When` instance to allow method chaining for additional configuration.
    ///
    /// # Errors
    /// This function will panic if the provided regex pattern is invalid.
    ///
    pub fn path_matches<TryIntoRegex: TryInto<Regex>>(self, regex: TryIntoRegex) -> Self
    where
        <TryIntoRegex as TryInto<Regex>>::Error: std::fmt::Debug,
    {
        let regex = regex
            .try_into()
            .expect("cannot convert provided value into regex");
        update_cell(&self.expectations, |e| {
            if e.path_matches.is_none() {
                e.path_matches = Some(Vec::new());
            }
            e.path_matches.as_mut().unwrap().push(regex)
        });
        self
    }
    // @docs-group: Path

    /// Specifies a required query parameter for the request.
    /// This function ensures that the specified query parameter (key-value pair) must be included
    /// in the request URL for the mock server to respond.
    ///
    /// **Note**: The request query keys and values are implicitly *allowed but not required* to be URL-encoded.
    /// However, the value passed to this method should always be in plain text (i.e., not encoded).
    ///
    /// # Parameters
    /// - `name`: The name of the query parameter to match against.
    /// - `value`: The expected value of the query parameter.
    ///
    /// # Example
    /// ```rust
    /// // Arrange
    /// use reqwest::blocking::get;
    /// use httpmock::prelude::*;
    ///
    /// // Start a new mock server
    /// let server = MockServer::start();
    ///
    /// // Create a mock that expects the query parameter `query` to have the value "This is cool"
    /// let m = server.mock(|when, then| {
    ///     when.query_param("query", "This is cool");
    ///     then.status(200);  // Respond with a 200 status code if the condition is met
    /// });
    ///
    /// // Act: Make a request that includes the specified query parameter and value
    /// get(&server.url("/search?query=This+is+cool")).unwrap();
    ///
    /// // Assert: Verify that the mock was called at least once
    /// m.assert();
    /// ```
    ///
    /// # Returns
    /// The updated `When` instance to allow method chaining for additional configuration.
    pub fn query_param<KeyString: Into<String>, ValueString: Into<String>>(
        self,
        name: KeyString,
        value: ValueString,
    ) -> Self {
        update_cell(&self.expectations, |e| {
            if e.query_param.is_none() {
                e.query_param = Some(Vec::new());
            }
            e.query_param
                .as_mut()
                .unwrap()
                .push((name.into(), value.into()));
        });
        self
    }
    // @docs-group: Query Parameters

    /// This function ensures that the specified query parameter (key) does exist in the request URL,
    /// and its value is not equal to the specified value.
    ///
    /// **Note**: Query keys and values are implicitly *allowed but not required* to be URL-encoded
    /// in the HTTP request. However, values passed to this method should always be in plain text
    /// (i.e., not encoded).
    ///
    /// # Parameters
    /// - `name`: The name of the query parameter to ensure is not present.
    /// - `value`: The value of the query parameter to ensure is not present.
    ///
    /// # Example
    /// ```rust
    /// // Arrange
    /// use httpmock::prelude::*;
    ///
    /// // Start a new mock server
    /// let server = MockServer::start();
    ///
    /// // Create a mock that expects the query parameter `query` to NOT have the value "This is cool"
    /// let m = server.mock(|when, then| {
    ///     when.query_param_not("query", "This is cool");
    ///     then.status(200);  // Respond with a 200 status code if the condition is met
    /// });
    ///
    /// // Act: Make a request that does not include the specified query parameter and value
    /// let response = reqwest::blocking::get(&server.url("/search?query=awesome")).unwrap();
    ///
    /// // Assert: Verify that the mock was called
    /// assert_eq!(response.status(), 200);
    /// m.assert();
    /// ```
    ///
    /// # Returns
    /// The updated `When` instance to allow method chaining for additional configuration.
    pub fn query_param_not<KeyString: Into<String>, ValueString: Into<String>>(
        self,
        name: KeyString,
        value: ValueString,
    ) -> Self {
        update_cell(&self.expectations, |e| {
            if e.query_param_not.is_none() {
                e.query_param_not = Some(Vec::new());
            }
            e.query_param_not
                .as_mut()
                .unwrap()
                .push((name.into(), value.into()));
        });
        self
    }
    // @docs-group: Query Parameters

    /// Specifies that a query parameter must be present in an HTTP request.
    /// This function ensures that the specified query parameter key exists in the request URL
    /// for the mock server to respond, regardless of the parameter's value.
    ///
    /// **Note**: The query key in the request is implicitly *allowed but not required* to be URL-encoded.
    /// However, provide the key in plain text here (i.e., not encoded).
    ///
    /// # Parameters
    /// - `name`: The name of the query parameter that must exist in the request.
    ///
    /// # Example
    /// ```rust
    /// // Arrange
    /// use httpmock::prelude::*;
    ///
    /// // Start a new mock server
    /// let server = MockServer::start();
    ///
    /// // Create a mock that expects the query parameter `query` to exist, regardless of its value
    /// let m = server.mock(|when, then| {
    ///     when.query_param_exists("query");
    ///     then.status(200);  // Respond with a 200 status code if the parameter exists
    /// });
    ///
    /// // Act: Make a request with the specified query parameter
    /// reqwest::blocking::get(&server.url("/search?query=restaurants+near+me")).unwrap();
    ///
    /// // Assert: Verify that the mock was called at least once
    /// m.assert();
    /// ```
    ///
    /// # Returns
    /// The updated `When` instance to allow method chaining for additional configuration.
    ///
    pub fn query_param_exists<IntoString: Into<String>>(self, name: IntoString) -> Self {
        update_cell(&self.expectations, |e| {
            if e.query_param_exists.is_none() {
                e.query_param_exists = Some(Vec::new());
            }
            e.query_param_exists.as_mut().unwrap().push(name.into());
        });
        self
    }
    // @docs-group: Query Parameters

    /// Specifies that a query parameter must *not* be present in an HTTP request.
    /// This function ensures that the specified query parameter key is absent in the request URL
    /// for the mock server to respond, regardless of the parameter's value.
    ///
    /// **Note**: The request query key is implicitly *allowed but not required* to be URL-encoded.
    /// However, provide the key in plain text (i.e., not encoded).
    ///
    /// # Parameters
    /// - `name`: The name of the query parameter that should be missing from the request.
    ///
    /// # Example
    /// ```rust
    /// // Arrange
    /// use httpmock::prelude::*;
    ///
    /// // Start a new mock server
    /// let server = MockServer::start();
    ///
    /// // Create a mock that expects the query parameter `query` to be missing
    /// let m = server.mock(|when, then| {
    ///     when.query_param_missing("query");
    ///     then.status(200);  // Respond with a 200 status code if the parameter is absent
    /// });
    ///
    /// // Act: Make a request without the specified query parameter
    /// reqwest::blocking::get(&server.url("/search")).unwrap();
    ///
    /// // Assert: Verify that the mock was called at least once
    /// m.assert();
    /// ```
    ///
    /// # Returns
    /// The updated `When` instance to allow method chaining for additional configuration.
    ///
    pub fn query_param_missing<IntoString: Into<String>>(self, name: IntoString) -> Self {
        update_cell(&self.expectations, |e| {
            if e.query_param_missing.is_none() {
                e.query_param_missing = Some(Vec::new());
            }
            e.query_param_missing.as_mut().unwrap().push(name.into());
        });
        self
    }
    // @docs-group: Query Parameters

    /// Specifies that a query parameter's value (**not** the key) must contain a specific substring for the request to match.
    /// This function ensures that the specified query parameter (key) does exist in the request URL, and
    /// it does have a value containing the given substring for the mock server to respond.
    ///
    /// **Note**: The request query key-value pairs are implicitly *allowed but not required* to be URL-encoded.
    /// However, provide the substring in plain text (i.e., not encoded).
    ///
    /// # Parameters
    /// - `name`: The name of the query parameter to match against.
    /// - `substring`: The substring that must appear within the value of the query parameter.
    ///
    /// # Example
    /// ```rust
    /// // Arrange
    /// use reqwest::blocking::get;
    /// use httpmock::prelude::*;
    ///
    /// // Start a new mock server
    /// let server = MockServer::start();
    ///
    /// // Create a mock that expects the query parameter `query`
    /// // to have a value containing "cool"
    /// let m = server.mock(|when, then| {
    ///     when.query_param_includes("query", "cool");
    ///     then.status(200);  // Respond with a 200 status code if the condition is met
    /// });
    ///
    /// // Act: Make a request that includes a value containing the substring "cool"
    /// get(server.url("/search?query=Something+cool")).unwrap();
    ///
    /// // Assert: Verify that the mock was called at least once
    /// m.assert();
    /// ```
    ///
    /// # Returns
    /// The updated `When` instance to allow method chaining for additional configuration.
    ///
    pub fn query_param_includes<KeyString: Into<String>, ValueString: Into<String>>(
        self,
        name: KeyString,
        substring: ValueString,
    ) -> Self {
        update_cell(&self.expectations, |e| {
            if e.query_param_includes.is_none() {
                e.query_param_includes = Some(Vec::new());
            }
            e.query_param_includes
                .as_mut()
                .unwrap()
                .push((name.into(), substring.into()));
        });
        self
    }
    // @docs-group: Query Parameters

    /// Specifies that a query parameter's value (**not** the key) must not contain a specific substring for the request to match.
    ///
    /// This function ensures that the specified query parameter (key) does exist in the request URL, and
    /// it does not have a value containing the given substring for the mock server to respond.
    ///
    /// **Note**: The request query key-value pairs are implicitly *allowed but not required* to be URL-encoded.
    /// However, provide the substring in plain text here (i.e., not encoded).
    ///
    /// # Parameters
    /// - `name`: The name of the query parameter to match against.
    /// - `substring`: The substring that must not appear within the value of the query parameter.
    ///
    /// # Example
    /// ```rust
    /// // Arrange
    /// use httpmock::prelude::*;
    ///
    /// // Start a new mock server
    /// let server = MockServer::start();
    ///
    /// // Create a mock that expects the query parameter `query`
    /// // to have a value that does not contain "uncool"
    /// let m = server.mock(|when, then| {
    ///     when.query_param_excludes("query", "uncool");
    ///     then.status(200);  // Respond with a 200 status code if the condition is met
    /// });
    ///
    /// // Act: Make a request that includes a value not containing the substring "uncool"
    /// reqwest::blocking::get(&server.url("/search?query=Something+cool")).unwrap();
    ///
    /// // Assert: Verify that the mock was called at least once
    /// m.assert();
    /// ```
    ///
    /// # Returns
    /// The updated `When` instance to allow method chaining for additional configuration.
    ///
    pub fn query_param_excludes<KeyString: Into<String>, ValueString: Into<String>>(
        self,
        name: KeyString,
        substring: ValueString,
    ) -> Self {
        update_cell(&self.expectations, |e| {
            if e.query_param_excludes.is_none() {
                e.query_param_excludes = Some(Vec::new());
            }
            e.query_param_excludes
                .as_mut()
                .unwrap()
                .push((name.into(), substring.into()));
        });

        self
    }
    // @docs-group: Query Parameters

    /// Specifies that a query parameter's value (**not** the key) must start with a specific prefix for the request to match.
    /// This function ensures that the specified query parameter (key) has a value starting with the given prefix
    /// in the request URL for the mock server to respond.
    ///
    /// **Note**: The request query key-value pairs are implicitly *allowed but not required* to be URL-encoded.
    /// Provide the prefix in plain text here (i.e., not encoded).
    ///
    /// # Parameters
    /// - `name`: The name of the query parameter to match against.
    /// - `prefix`: The prefix that the query parameter value should start with.
    ///
    /// # Example
    /// ```rust
    /// // Arrange
    /// use httpmock::prelude::*;
    ///
    /// // Start a new mock server
    /// let server = MockServer::start();
    ///
    /// // Create a mock that expects the query parameter `query`
    /// // to have a value starting with "cool"
    /// let m = server.mock(|when, then| {
    ///     when.query_param_prefix("query", "cool");
    ///     then.status(200);  // Respond with a 200 status code if the condition is met
    /// });
    ///
    /// // Act: Make a request that includes a value starting with the prefix "cool"
    /// reqwest::blocking::get(&server.url("/search?query=cool+stuff")).unwrap();
    ///
    /// // Assert: Verify that the mock was called at least once
    /// m.assert();
    /// ```
    ///
    /// # Returns
    /// The updated `When` instance to allow method chaining for additional configuration.
    ///
    pub fn query_param_prefix<KeyString: Into<String>, ValueString: Into<String>>(
        self,
        name: KeyString,
        prefix: ValueString,
    ) -> Self {
        update_cell(&self.expectations, |e| {
            if e.query_param_prefix.is_none() {
                e.query_param_prefix = Some(Vec::new());
            }
            e.query_param_prefix
                .as_mut()
                .unwrap()
                .push((name.into(), prefix.into()));
        });
        self
    }
    // @docs-group: Query Parameters

    /// Specifies that a query parameter's value (**not** the key) must end with a specific suffix for the request to match.
    /// This function ensures that the specified query parameter (key) has a value ending with the given suffix
    /// in the request URL for the mock server to respond.
    ///
    /// **Note**: The request query key-value pairs are implicitly *allowed but not required* to be URL-encoded.
    /// Provide the suffix in plain text here (i.e., not encoded).
    ///
    /// # Parameters
    /// - `name`: The name of the query parameter to match against.
    /// - `suffix`: The suffix that the query parameter value should end with.
    ///
    /// # Example
    /// ```rust
    /// // Arrange
    /// use httpmock::prelude::*;
    ///
    /// // Start a new mock server
    /// let server = MockServer::start();
    ///
    /// // Create a mock that expects the query parameter `query`
    /// // to have a value ending with "cool"
    /// let m = server.mock(|when, then| {
    ///     when.query_param_suffix("query", "cool");
    ///     then.status(200);  // Respond with a 200 status code if the condition is met
    /// });
    ///
    /// // Act: Make a request that includes a value ending with the suffix "cool"
    /// reqwest::blocking::get(&server.url("/search?query=really_cool")).unwrap();
    ///
    /// // Assert: Verify that the mock was called at least once
    /// m.assert();
    /// ```
    ///
    /// # Returns
    /// The updated `When` instance to allow method chaining for additional configuration.
    ///
    pub fn query_param_suffix<KeyString: Into<String>, ValueString: Into<String>>(
        self,
        name: KeyString,
        suffix: ValueString,
    ) -> Self {
        update_cell(&self.expectations, |e| {
            if e.query_param_suffix.is_none() {
                e.query_param_suffix = Some(Vec::new());
            }
            e.query_param_suffix
                .as_mut()
                .unwrap()
                .push((name.into(), suffix.into()));
        });
        self
    }
    // @docs-group: Query Parameters

    /// Specifies that a query parameter's value (**not** the key) must not start with a specific prefix for the request to match.
    /// This function ensures that the specified query parameter (key) has a value not starting with the given prefix
    /// in the request URL for the mock server to respond.
    ///
    /// **Note**: The request query key-value pairs are implicitly *allowed but not required* to be URL-encoded.
    /// Provide the prefix in plain text here (i.e., not encoded).
    ///
    /// # Parameters
    /// - `name`: The name of the query parameter to match against.
    /// - `prefix`: The prefix that the query parameter value should not start with.
    ///
    /// # Example
    /// ```rust
    /// // Arrange
    /// use httpmock::prelude::*;
    ///
    /// // Start a new mock server
    /// let server = MockServer::start();
    ///
    /// // Create a mock that expects the query parameter `query`
    /// // to have a value not starting with "cool"
    /// let m = server.mock(|when, then| {
    ///     when.query_param_prefix_not("query", "cool");
    ///     then.status(200);  // Respond with a 200 status code if the condition is met
    /// });
    ///
    /// // Act: Make a request that does not start with the prefix "cool"
    /// reqwest::blocking::get(&server.url("/search?query=warm_stuff")).unwrap();
    ///
    /// // Assert: Verify that the mock was called at least once
    /// m.assert();
    /// ```
    ///
    /// # Returns
    /// The updated `When` instance to allow method chaining for additional configuration.
    ///
    pub fn query_param_prefix_not<KeyString: Into<String>, ValueString: Into<String>>(
        self,
        name: KeyString,
        prefix: ValueString,
    ) -> Self {
        update_cell(&self.expectations, |e| {
            if e.query_param_prefix_not.is_none() {
                e.query_param_prefix_not = Some(Vec::new());
            }
            e.query_param_prefix_not
                .as_mut()
                .unwrap()
                .push((name.into(), prefix.into()));
        });
        self
    }
    // @docs-group: Query Parameters

    /// Specifies that a query parameter's value (**not** the key) must not end with a specific suffix for the request to match.
    /// This function ensures that the specified query parameter (key) has a value not ending with the given suffix
    /// in the request URL for the mock server to respond.
    ///
    /// **Note**: The request query key-value pairs are implicitly *allowed but not required* to be URL-encoded.
    /// Provide the suffix in plain text here (i.e., not encoded).
    ///
    /// # Parameters
    /// - `name`: The name of the query parameter to match against.
    /// - `suffix`: The suffix that the query parameter value should not end with.
    ///
    /// # Example
    /// ```rust
    /// // Arrange
    /// use httpmock::prelude::*;
    ///
    /// // Start a new mock server
    /// let server = MockServer::start();
    ///
    /// // Create a mock that expects the query parameter `query`
    /// // to have a value not ending with "cool"
    /// let m = server.mock(|when, then| {
    ///     when.query_param_suffix_not("query", "cool");
    ///     then.status(200);  // Respond with a 200 status code if the condition is met
    /// });
    ///
    /// // Act: Make a request that doesn't end with the suffix "cool"
    /// reqwest::blocking::get(&server.url("/search?query=uncool_stuff")).unwrap();
    ///
    /// // Assert: Verify that the mock was called at least once
    /// m.assert();
    /// ```
    ///
    /// # Returns
    /// The updated `When` instance to allow method chaining for additional configuration.
    ///
    pub fn query_param_suffix_not<KeyString: Into<String>, ValueString: Into<String>>(
        self,
        name: KeyString,
        suffix: ValueString,
    ) -> Self {
        update_cell(&self.expectations, |e| {
            if e.query_param_suffix_not.is_none() {
                e.query_param_suffix_not = Some(Vec::new());
            }
            e.query_param_suffix_not
                .as_mut()
                .unwrap()
                .push((name.into(), suffix.into()));
        });
        self
    }
    // @docs-group: Query Parameters

    /// Specifies that a query parameter must match a specific regular expression pattern for the key and another pattern for the value.
    /// This function ensures that the specified query parameter key-value pair matches the given patterns
    /// in the request URL for the mock server to respond.
    ///
    /// # Parameters
    /// - `key_regex`: A regular expression pattern for the query parameter's key to match against.
    /// - `value_regex`: A regular expression pattern for the query parameter's value to match against.
    ///
    /// # Example
    /// ```rust
    /// // Arrange
    /// use httpmock::prelude::*;
    ///
    /// // Start a new mock server
    /// let server = MockServer::start();
    ///
    /// // Create a mock that expects the query parameter key to match the regex "user.*"
    /// // and the value to match the regex "admin.*"
    /// let m = server.mock(|when, then| {
    ///     when.query_param_matches(r"user.*", r"admin.*");
    ///     then.status(200);  // Respond with a 200 status code if the condition is met
    /// });
    ///
    /// // Act: Make a request that matches the regex patterns for both key and value
    /// reqwest::blocking::get(&server.url("/search?user=admin_user")).unwrap();
    ///
    /// // Assert: Verify that the mock was called at least once
    /// m.assert();
    /// ```
    ///
    /// # Returns
    /// The updated `When` instance to allow method chaining for additional configuration.
    ///
    pub fn query_param_matches<KeyRegex: Into<Regex>, ValueRegex: Into<Regex>>(
        self,
        key_regex: KeyRegex,
        value_regex: ValueRegex,
    ) -> Self {
        let key_regex = key_regex.into();
        let value_regex = value_regex.into();

        update_cell(&self.expectations, |e| {
            if e.query_param_matches.is_none() {
                e.query_param_matches = Some(Vec::new());
            }
            e.query_param_matches
                .as_mut()
                .unwrap()
                .push((key_regex, value_regex));
        });
        self
    }
    // @docs-group: Query Parameters

    /// Specifies that the count of query parameters with keys and values matching specific regular
    /// expression patterns must equal a specified number for the request to match.
    /// This function ensures that the number of query parameters whose keys and values match the
    /// given regex patterns is equal to the specified count in the request URL for the mock
    /// server to respond.
    ///
    /// # Parameters
    /// - `key_regex`: A regular expression pattern for the query parameter's key to match against.
    /// - `value_regex`: A regular expression pattern for the query parameter's value to match against.
    /// - `expected_count`: The expected number of query parameters whose keys and values match the regex patterns.
    ///
    /// # Example
    /// ```rust
    /// // Arrange
    /// use httpmock::prelude::*;
    ///
    /// // Start a new mock server
    /// let server = MockServer::start();
    ///
    /// // Create a mock that expects exactly two query parameters with keys matching the regex "user.*"
    /// // and values matching the regex "admin.*"
    /// let m = server.mock(|when, then| {
    ///     when.query_param_count(r"user.*", r"admin.*", 2);
    ///     then.status(200);  // Respond with a 200 status code if the condition is met
    /// });
    ///
    /// // Act: Make a request that matches the conditions
    /// reqwest::blocking::get(&server.url("/search?user1=admin1&user2=admin2")).unwrap();
    ///
    /// // Assert: Verify that the mock was called at least once
    /// m.assert();
    /// ```
    ///
    /// # Returns
    /// The updated `When` instance to allow method chaining for additional configuration.
    ///
    pub fn query_param_count<KeyRegex: Into<Regex>, ValueRegex: Into<Regex>>(
        self,
        key_regex: KeyRegex,
        value_regex: ValueRegex,
        expected_count: usize,
    ) -> Self {
        let key_regex = key_regex.into();
        let value_regex = value_regex.into();

        update_cell(&self.expectations, |e| {
            if e.query_param_count.is_none() {
                e.query_param_count = Some(Vec::new());
            }
            e.query_param_count
                .as_mut()
                .unwrap()
                .push((key_regex, value_regex, expected_count));
        });
        self
    }
    // @docs-group: Query Parameters

    /// Sets the expected HTTP header and its value for the request to match.
    /// This function ensures that the specified header with the given value is present in the request.
    /// Header names are case-insensitive, as per RFC 2616.
    ///
    /// # Parameters
    /// - `name`: The HTTP header name. Header names are case-insensitive.
    /// - `value`: The expected value of the HTTP header.
    ///
    /// # Example
    /// ```rust
    /// use httpmock::prelude::*;
    /// use reqwest::blocking::Client;
    ///
    /// // Start a new mock server
    /// let server = MockServer::start();
    ///
    /// // Create a mock that expects the "Authorization" header with a specific value
    /// let mock = server.mock(|when, then| {
    ///     when.header("Authorization", "token 1234567890");
    ///     then.status(200);  // Respond with a 200 status code if the header and value are present
    /// });
    ///
    /// // Make a request that includes the "Authorization" header with the specified value
    /// Client::new()
    ///     .post(&format!("http://{}/test", server.address()))
    ///     .header("Authorization", "token 1234567890")
    ///     .send()
    ///     .unwrap();
    ///
    /// // Verify that the mock was called at least once
    /// mock.assert();
    /// ```
    ///
    /// # Returns
    /// The updated `When` instance to allow method chaining for additional configuration.
    ///
    pub fn header<KeyString: Into<String>, ValueString: Into<String>>(
        self,
        name: KeyString,
        value: ValueString,
    ) -> Self {
        update_cell(&self.expectations, |e| {
            if e.header.is_none() {
                e.header = Some(Vec::new());
            }
            e.header.as_mut().unwrap().push((name.into(), value.into()));
        });
        self
    }
    // @docs-group: Headers

    /// Sets the requirement that the HTTP request must not contain a specific header with the specified value.
    /// This function ensures that the specified header with the given value is absent in the request.
    /// Header names are case-insensitive, as per RFC 2616.
    ///
    /// This function may be called multiple times to add multiple excluded headers.
    ///
    /// # Parameters
    /// - `name`: The HTTP header name. Header names are case-insensitive.
    /// - `value`: The value of the HTTP header that must not be present.
    ///
    /// # Example
    /// ```rust
    /// use httpmock::prelude::*;
    /// use reqwest::blocking::Client;
    ///
    /// // Start a new mock server
    /// let server = MockServer::start();
    ///
    /// // Create a mock that expects the "Authorization" header with a specific value to be absent
    /// let mock = server.mock(|when, then| {
    ///     when.header_not("Authorization", "token 1234567890");
    ///     then.status(200);  // Respond with a 200 status code if the header and value are absent
    /// });
    ///
    /// // Make a request that includes the "Authorization" header with a different value
    /// Client::new()
    ///     .post(&format!("http://{}/test", server.address()))
    ///     .header("Authorization", "token abcdefg")
    ///     .send()
    ///     .unwrap();
    ///
    /// // Verify that the mock was called at least once
    /// mock.assert();
    /// ```
    ///
    /// # Returns
    /// The updated `When` instance to allow method chaining for additional configuration.
    ///
    pub fn header_not<KeyString: Into<String>, ValueString: Into<String>>(
        self,
        name: KeyString,
        value: ValueString,
    ) -> Self {
        update_cell(&self.expectations, |e| {
            if e.header_not.is_none() {
                e.header_not = Some(Vec::new());
            }
            e.header_not
                .as_mut()
                .unwrap()
                .push((name.into(), value.into()));
        });
        self
    }
    // @docs-group: Headers

    /// Sets the requirement that the HTTP request must contain a specific header.
    /// The presence of the header is checked, but its value is not validated.
    /// For value validation, refer to [Mock::expect_header](struct.Mock.html#method.expect_header).
    ///
    /// # Parameters
    /// - `name`: The HTTP header name. Header names are case-insensitive, as per RFC 2616.
    ///
    /// # Example
    /// ```rust
    /// use httpmock::prelude::*;
    /// use reqwest::blocking::Client;
    ///
    /// // Start a new mock server
    /// let server = MockServer::start();
    ///
    /// // Create a mock that expects the "Authorization" header to be present in the request
    /// let mock = server.mock(|when, then| {
    ///     when.header_exists("Authorization");
    ///     then.status(200);  // Respond with a 200 status code if the header is present
    /// });
    ///
    /// // Make a request that includes the "Authorization" header
    /// Client::new()
    ///     .post(&format!("http://{}/test", server.address()))
    ///     .header("Authorization", "token 1234567890")
    ///     .send()
    ///     .unwrap();
    ///
    /// // Verify that the mock was called at least once
    /// mock.assert();
    /// ```
    ///
    /// # Returns
    /// The updated `When` instance to allow method chaining for additional configuration.
    ///
    pub fn header_exists<IntoString: Into<String>>(self, name: IntoString) -> Self {
        update_cell(&self.expectations, |e| {
            if e.header_exists.is_none() {
                e.header_exists = Some(Vec::new());
            }
            e.header_exists.as_mut().unwrap().push(name.into());
        });
        self
    }
    // @docs-group: Headers

    /// Sets the requirement that the HTTP request must not contain a specific header.
    /// This function ensures that the specified header is absent in the request.
    /// Header names are case-insensitive, as per RFC 2616.
    ///
    /// This function may be called multiple times to add multiple excluded headers.
    ///
    /// # Parameters
    /// - `name`: The HTTP header name. Header names are case-insensitive.
    ///
    /// # Example
    /// ```rust
    /// use httpmock::prelude::*;
    /// use reqwest::blocking::Client;
    ///
    /// // Start a new mock server
    /// let server = MockServer::start();
    ///
    /// // Create a mock that expects the "Authorization" header to be absent in the request
    /// let mock = server.mock(|when, then| {
    ///     when.header_missing("Authorization");
    ///     then.status(200);  // Respond with a 200 status code if the header is absent
    /// });
    ///
    /// // Make a request that does not include the "Authorization" header
    /// Client::new()
    ///     .post(&format!("http://{}/test", server.address()))
    ///     .send()
    ///     .unwrap();
    ///
    /// // Verify that the mock was called at least once
    /// mock.assert();
    /// ```
    ///
    /// # Returns
    /// The updated `When` instance to allow method chaining for additional configuration.
    ///
    pub fn header_missing<IntoString: Into<String>>(self, name: IntoString) -> Self {
        update_cell(&self.expectations, |e| {
            if e.header_missing.is_none() {
                e.header_missing = Some(Vec::new());
            }
            e.header_missing.as_mut().unwrap().push(name.into());
        });
        self
    }
    // @docs-group: Headers

    /// Sets the requirement that the HTTP request must contain a specific header whose value contains a specified substring.
    /// This function ensures that the specified header is present and its value contains the given substring.
    /// Header names are case-insensitive, as per RFC 2616.
    ///
    /// This function may be called multiple times to check multiple headers and substrings.
    ///
    /// # Parameters
    /// - `name`: The HTTP header name. Header names are case-insensitive.
    /// - `substring`: The substring that the header value must contain.
    ///
    /// # Example
    /// ```rust
    /// use httpmock::prelude::*;
    /// use reqwest::blocking::Client;
    ///
    /// // Start a new mock server
    /// let server = MockServer::start();
    ///
    /// // Create a mock that expects the "Authorization" header's value to contain "token"
    /// let mock = server.mock(|when, then| {
    ///     when.header_includes("Authorization", "token");
    ///     then.status(200);  // Respond with a 200 status code if the header value contains the substring
    /// });
    ///
    /// // Make a request that includes the "Authorization" header with the specified substring in its value
    /// Client::new()
    ///     .post(&format!("http://{}/test", server.address()))
    ///     .header("Authorization", "token 1234567890")
    ///     .send()
    ///     .unwrap();
    ///
    /// // Verify that the mock was called at least once
    /// mock.assert();
    /// ```
    ///
    /// # Returns
    /// The updated `When` instance to allow method chaining for additional configuration.
    ///
    pub fn header_includes<KeyString: Into<String>, ValueString: Into<String>>(
        self,
        name: KeyString,
        substring: ValueString,
    ) -> Self {
        update_cell(&self.expectations, |e| {
            if e.header_includes.is_none() {
                e.header_includes = Some(Vec::new());
            }
            e.header_includes
                .as_mut()
                .unwrap()
                .push((name.into(), substring.into()));
        });
        self
    }
    // @docs-group: Headers

    /// Sets the requirement that the HTTP request must contain a specific header whose value does not contain a specified substring.
    /// This function ensures that the specified header is present and its value does not contain the given substring.
    /// Header names are case-insensitive, as per RFC 2616.
    ///
    /// This function may be called multiple times to check multiple headers and substrings.
    ///
    /// # Parameters
    /// - `name`: The HTTP header name. Header names are case-insensitive.
    /// - `substring`: The substring that the header value must not contain.
    ///
    /// # Example
    /// ```rust
    /// use httpmock::prelude::*;
    /// use reqwest::blocking::Client;
    ///
    /// // Start a new mock server
    /// let server = MockServer::start();
    ///
    /// // Create a mock that expects the "Authorization" header's value to not contain "Bearer"
    /// let mock = server.mock(|when, then| {
    ///     when.header_excludes("Authorization", "Bearer");
    ///     then.status(200);  // Respond with a 200 status code if the header value does not contain the substring
    /// });
    ///
    /// // Make a request that includes the "Authorization" header without the forbidden substring in its value
    /// Client::new()
    ///     .post(&format!("http://{}/test", server.address()))
    ///     .header("Authorization", "token 1234567890")
    ///     .send()
    ///     .unwrap();
    ///
    /// // Verify that the mock was called at least once
    /// mock.assert();
    /// ```
    ///
    /// # Returns
    /// The updated `When` instance to allow method chaining for additional configuration.
    ///
    pub fn header_excludes<KeyString: Into<String>, ValueString: Into<String>>(
        self,
        name: KeyString,
        substring: ValueString,
    ) -> Self {
        update_cell(&self.expectations, |e| {
            if e.header_excludes.is_none() {
                e.header_excludes = Some(Vec::new());
            }
            e.header_excludes
                .as_mut()
                .unwrap()
                .push((name.into(), substring.into()));
        });
        self
    }
    // @docs-group: Headers

    /// Sets the requirement that the HTTP request must contain a specific header whose value starts with a specified prefix.
    /// This function ensures that the specified header is present and its value starts with the given prefix.
    /// Header names are case-insensitive, as per RFC 2616.
    ///
    /// This function may be called multiple times to check multiple headers and prefixes.
    ///
    /// # Parameters
    /// - `name`: The HTTP header name. Header names are case-insensitive.
    /// - `prefix`: The prefix that the header value must start with.
    ///
    /// # Example
    /// ```rust
    /// use httpmock::prelude::*;
    /// use reqwest::blocking::Client;
    ///
    /// // Start a new mock server
    /// let server = MockServer::start();
    ///
    /// // Create a mock that expects the "Authorization" header's value to start with "token"
    /// let mock = server.mock(|when, then| {
    ///     when.header_prefix("Authorization", "token");
    ///     then.status(200);  // Respond with a 200 status code if the header value starts with the prefix
    /// });
    ///
    /// // Make a request that includes the "Authorization" header with the specified prefix in its value
    /// Client::new()
    ///     .post(&format!("http://{}/test", server.address()))
    ///     .header("Authorization", "token 1234567890")
    ///     .send()
    ///     .unwrap();
    ///
    /// // Verify that the mock was called at least once
    /// mock.assert();
    /// ```
    ///
    /// # Returns
    /// The updated `When` instance to allow method chaining for additional configuration.
    ///
    pub fn header_prefix<KeyString: Into<String>, ValueString: Into<String>>(
        self,
        name: KeyString,
        prefix: ValueString,
    ) -> Self {
        update_cell(&self.expectations, |e| {
            if e.header_prefix.is_none() {
                e.header_prefix = Some(Vec::new());
            }
            e.header_prefix
                .as_mut()
                .unwrap()
                .push((name.into(), prefix.into()));
        });
        self
    }
    // @docs-group: Headers

    /// Sets the requirement that the HTTP request must contain a specific header whose value ends with a specified suffix.
    /// This function ensures that the specified header is present and its value ends with the given suffix.
    /// Header names are case-insensitive, as per RFC 2616.
    ///
    /// This function may be called multiple times to check multiple headers and suffixes.
    ///
    /// # Parameters
    /// - `name`: The HTTP header name. Header names are case-insensitive.
    /// - `suffix`: The suffix that the header value must end with.
    ///
    /// # Example
    /// ```rust
    /// use httpmock::prelude::*;
    /// use reqwest::blocking::Client;
    ///
    /// // Start a new mock server
    /// let server = MockServer::start();
    ///
    /// // Create a mock that expects the "Authorization" header's value to end with "7890"
    /// let mock = server.mock(|when, then| {
    ///     when.header_suffix("Authorization", "7890");
    ///     then.status(200);  // Respond with a 200 status code if the header value ends with the suffix
    /// });
    ///
    /// // Make a request that includes the "Authorization" header with the specified suffix in its value
    /// Client::new()
    ///     .post(&format!("http://{}/test", server.address()))
    ///     .header("Authorization", "token 1234567890")
    ///     .send()
    ///     .unwrap();
    ///
    /// // Verify that the mock was called at least once
    /// mock.assert();
    /// ```
    ///
    /// # Returns
    /// The updated `When` instance to allow method chaining for additional configuration.
    ///
    pub fn header_suffix<KeyString: Into<String>, ValueString: Into<String>>(
        self,
        name: KeyString,
        suffix: ValueString,
    ) -> Self {
        update_cell(&self.expectations, |e| {
            if e.header_suffix.is_none() {
                e.header_suffix = Some(Vec::new());
            }
            e.header_suffix
                .as_mut()
                .unwrap()
                .push((name.into(), suffix.into()));
        });
        self
    }
    // @docs-group: Headers

    /// Sets the requirement that the HTTP request must contain a specific header whose value does not start with a specified prefix.
    /// This function ensures that the specified header is present and its value does not start with the given prefix.
    /// Header names are case-insensitive, as per RFC 2616.
    ///
    /// This function may be called multiple times to check multiple headers and prefixes.
    ///
    /// # Parameters
    /// - `name`: The HTTP header name. Header names are case-insensitive.
    /// - `prefix`: The prefix that the header value must not start with.
    ///
    /// # Example
    /// ```rust
    /// use httpmock::prelude::*;
    /// use reqwest::blocking::Client;
    ///
    /// // Start a new mock server
    /// let server = MockServer::start();
    ///
    /// // Create a mock that expects the "Authorization" header's value to not start with "Bearer"
    /// let mock = server.mock(|when, then| {
    ///     when.header_prefix_not("Authorization", "Bearer");
    ///     then.status(200);  // Respond with a 200 status code if the header value does not start with the prefix
    /// });
    ///
    /// // Make a request that includes the "Authorization" header without the "Bearer" prefix in its value
    /// Client::new()
    ///     .post(&format!("http://{}/test", server.address()))
    ///     .header("Authorization", "token 1234567890")
    ///     .send()
    ///     .unwrap();
    ///
    /// // Verify that the mock was called at least once
    /// mock.assert();
    /// ```
    ///
    /// # Returns
    /// The updated `When` instance to allow method chaining for additional configuration.
    ///
    pub fn header_prefix_not<KeyString: Into<String>, ValueString: Into<String>>(
        self,
        name: KeyString,
        prefix: ValueString,
    ) -> Self {
        update_cell(&self.expectations, |e| {
            if e.header_prefix_not.is_none() {
                e.header_prefix_not = Some(Vec::new());
            }
            e.header_prefix_not
                .as_mut()
                .unwrap()
                .push((name.into(), prefix.into()));
        });
        self
    }
    // @docs-group: Headers

    /// Sets the requirement that the HTTP request must contain a specific header whose value does not end with a specified suffix.
    /// This function ensures that the specified header is present and its value does not end with the given suffix.
    /// Header names are case-insensitive, as per RFC 2616.
    ///
    /// This function may be called multiple times to check multiple headers and suffixes.
    ///
    /// # Parameters
    /// - `name`: The HTTP header name. Header names are case-insensitive.
    /// - `suffix`: The suffix that the header value must not end with.
    ///
    /// # Example
    /// ```rust
    /// use httpmock::prelude::*;
    /// use reqwest::blocking::Client;
    ///
    /// // Start a new mock server
    /// let server = MockServer::start();
    ///
    /// // Create a mock that expects the "Authorization" header's value to not end with "abc"
    /// let mock = server.mock(|when, then| {
    ///     when.header_suffix_not("Authorization", "abc");
    ///     then.status(200);  // Respond with a 200 status code if the header value does not end with the suffix
    /// });
    ///
    /// // Make a request that includes the "Authorization" header without the "abc" suffix in its value
    /// Client::new()
    ///     .post(&format!("http://{}/test", server.address()))
    ///     .header("Authorization", "token 1234567890")
    ///     .send()
    ///     .unwrap();
    ///
    /// // Verify that the mock was called at least once
    /// mock.assert();
    /// ```
    ///
    /// # Returns
    /// The updated `When` instance to allow method chaining for additional configuration.
    ///
    pub fn header_suffix_not<KeyString: Into<String>, ValueString: Into<String>>(
        self,
        name: KeyString,
        suffix: ValueString,
    ) -> Self {
        update_cell(&self.expectations, |e| {
            if e.header_suffix_not.is_none() {
                e.header_suffix_not = Some(Vec::new());
            }
            e.header_suffix_not
                .as_mut()
                .unwrap()
                .push((name.into(), suffix.into()));
        });
        self
    }
    // @docs-group: Headers

    /// Sets the requirement that the HTTP request must contain a specific header whose key and value match the specified regular expressions.
    /// This function ensures that the specified header is present and both its key and value match the given regular expressions.
    /// Header names are case-insensitive, as per RFC 2616.
    ///
    /// This function may be called multiple times to check multiple headers and patterns.
    ///
    /// # Parameters
    /// - `key_regex`: The regular expression that the header key must match.
    /// - `value_regex`: The regular expression that the header value must match.
    ///
    /// # Example
    /// ```rust
    /// use httpmock::prelude::*;
    /// use reqwest::blocking::Client;
    ///
    /// // Start a new mock server
    /// let server = MockServer::start();
    ///
    /// // Create a mock that expects the "Authorization" header's key to match the regex "^Auth.*"
    /// // and its value to match the regex "token .*"
    /// let mock = server.mock(|when, then| {
    ///     when.header_matches("^Auth.*", "token .*");
    ///     then.status(200);  // Respond with a 200 status code if the header key and value match the patterns
    /// });
    ///
    /// // Make a request that includes the "Authorization" header with a value matching the regex
    /// Client::new()
    ///     .post(&format!("http://{}/test", server.address()))
    ///     .header("Authorization", "token 1234567890")
    ///     .send()
    ///     .unwrap();
    ///
    /// // Verify that the mock was called at least once
    /// mock.assert();
    /// ```
    ///
    /// # Returns
    /// The updated `When` instance to allow method chaining for additional configuration.
    ///
    pub fn header_matches<KeyString: Into<Regex>, ValueString: Into<Regex>>(
        self,
        key_regex: KeyString,
        value_regex: ValueString,
    ) -> Self {
        update_cell(&self.expectations, |e| {
            if e.header_matches.is_none() {
                e.header_matches = Some(Vec::new());
            }
            e.header_matches
                .as_mut()
                .unwrap()
                .push((key_regex.into(), value_regex.into()));
        });
        self
    }
    // @docs-group: Headers

    /// Sets the requirement that the HTTP request must contain a specific number of headers whose keys and values match specified patterns.
    /// This function ensures that the specified number of headers with keys and values matching the given patterns are present in the request.
    /// Header names are case-insensitive, as per RFC 2616.
    ///
    /// This function may be called multiple times to check multiple patterns and counts.
    ///
    /// # Parameters
    /// - `key_pattern`: The pattern that the header keys must match.
    /// - `value_pattern`: The pattern that the header values must match.
    /// - `count`: The number of headers with keys and values matching the patterns that must be present.
    ///
    /// # Example
    /// ```rust
    /// use httpmock::prelude::*;
    /// use reqwest::blocking::Client;
    ///
    /// // Start a new mock server
    /// let server = MockServer::start();
    ///
    /// // Create a mock that expects at least 2 headers whose keys match the regex "^X-Custom-Header.*"
    /// // and values match the regex "value.*"
    /// let mock = server.mock(|when, then| {
    ///     when.header_count("^X-Custom-Header.*", "value.*", 2);
    ///     then.status(200);  // Respond with a 200 status code if the condition is met
    /// });
    ///
    /// // Make a request that includes the required headers
    /// Client::new()
    ///     .post(&format!("http://{}/test", server.address()))
    ///     .header("x-custom-header-1", "value1")
    ///     .header("X-Custom-Header-2", "value2")
    ///     .send()
    ///     .unwrap();
    ///
    /// // Verify that the mock was called at least once
    /// mock.assert();
    /// ```
    ///
    /// # Returns
    /// The updated `When` instance to allow method chaining for additional configuration.
    ///
    pub fn header_count<
        KeyRegex: TryInto<Regex>,
        ValueRegex: TryInto<Regex>,
        IntoUsize: TryInto<usize>,
    >(
        self,
        key_pattern: KeyRegex,
        value_pattern: ValueRegex,
        count: IntoUsize,
    ) -> Self
    where
        <KeyRegex as TryInto<Regex>>::Error: std::fmt::Debug,
        <ValueRegex as TryInto<Regex>>::Error: std::fmt::Debug,
        <IntoUsize as TryInto<usize>>::Error: std::fmt::Debug,
    {
        let count = match count.try_into() {
            Ok(c) => c,
            Err(_) => panic!("parameter count must be a positive integer that fits into a usize"),
        };

        let key_pattern = key_pattern.try_into().expect("cannot convert key to regex");
        let value_pattern = value_pattern
            .try_into()
            .expect("cannot convert key to regex");

        update_cell(&self.expectations, |e| {
            if e.header_count.is_none() {
                e.header_count = Some(Vec::new());
            }
            e.header_count
                .as_mut()
                .unwrap()
                .push((key_pattern, value_pattern, count));
        });
        self
    }
    // @docs-group: Headers

    /// Sets the cookie that needs to exist in the HTTP request.
    /// Cookie parsing follows [RFC-6265](https://tools.ietf.org/html/rfc6265.html).
    /// **Attention**: Cookie names are **case-sensitive**.
    ///
    /// # Parameters
    /// - `name`: The name of the cookie. Must be a case-sensitive match.
    /// - `value`: The expected value of the cookie.
    ///
    /// > Note: This function is only available when the `cookies` feature is enabled. This feature is enabled by default.
    ///
    /// # Example
    /// ```rust
    /// use httpmock::prelude::*;
    /// use reqwest::blocking::Client;
    ///
    /// // Start a new mock server
    /// let server = MockServer::start();
    ///
    /// // Create a mock that expects a cookie named "SESSIONID" with the value "1234567890"
    /// let mock = server.mock(|when, then| {
    ///     when.cookie("SESSIONID", "1234567890");
    ///     then.status(200);  // Respond with a 200 status code if the condition is met
    /// });
    ///
    /// // Make a request that includes the required cookie
    /// Client::new()
    ///     .post(&format!("http://{}/test", server.address()))
    ///     .header("Cookie", "TRACK=12345; SESSIONID=1234567890; CONSENT=1")
    ///     .send()
    ///     .unwrap();
    ///
    /// // Verify that the mock was called at least once
    /// mock.assert();
    /// ```
    ///
    /// # Returns
    /// The updated `When` instance to allow method chaining for additional configuration.
    pub fn cookie<KeyString: Into<String>, ValueString: Into<String>>(
        self,
        name: KeyString,
        value: ValueString,
    ) -> Self {
        update_cell(&self.expectations, |e| {
            if e.cookie.is_none() {
                e.cookie = Some(Vec::new());
            }
            e.cookie.as_mut().unwrap().push((name.into(), value.into()));
        });
        self
    }
    // @docs-group: Cookies

    /// Sets the cookie that should not exist or should not have a specific value in the HTTP request.
    /// Cookie parsing follows [RFC-6265](https://tools.ietf.org/html/rfc6265.html).
    /// **Attention**: Cookie names are **case-sensitive**.
    ///
    /// # Parameters
    /// - `name`: The name of the cookie. Must be a case-sensitive match.
    /// - `value`: The value that the cookie should not have.
    ///
    /// > Note: This function is only available when the `cookies` feature is enabled. This feature is enabled by default.
    ///
    /// # Example
    /// ```rust
    /// use httpmock::prelude::*;
    /// use reqwest::blocking::Client;
    ///
    /// // Start a new mock server
    /// let server = MockServer::start();
    ///
    /// // Create a mock that expects a cookie named "SESSIONID" to not have the value "1234567890"
    /// let mock = server.mock(|when, then| {
    ///     when.cookie_not("SESSIONID", "1234567890");
    ///     then.status(200);  // Respond with a 200 status code if the condition is met
    /// });
    ///
    /// // Make a request that includes the required cookie
    /// Client::new()
    ///     .post(&format!("http://{}/test", server.address()))
    ///     .header("Cookie", "TRACK=12345; SESSIONID=0987654321; CONSENT=1")
    ///     .send()
    ///     .unwrap();
    ///
    /// // Verify that the mock was called at least once
    /// mock.assert();
    /// ```
    ///
    /// # Returns
    /// The updated `When` instance to allow method chaining for additional configuration.
    pub fn cookie_not<KeyString: Into<String>, ValueString: Into<String>>(
        self,
        name: KeyString,
        value: ValueString,
    ) -> Self {
        update_cell(&self.expectations, |e| {
            if e.cookie_not.is_none() {
                e.cookie_not = Some(Vec::new());
            }
            e.cookie_not
                .as_mut()
                .unwrap()
                .push((name.into(), value.into()));
        });
        self
    }
    // @docs-group: Cookies

    /// Sets the requirement that a cookie with the specified name must exist in the HTTP request.
    /// Cookie parsing follows [RFC-6265](https://tools.ietf.org/html/rfc6265.html).
    /// **Attention**: Cookie names are **case-sensitive**.
    ///
    /// # Parameters
    /// - `name`: The name of the cookie that must exist.
    ///
    /// > Note: This function is only available when the `cookies` feature is enabled. This feature is enabled by default.
    ///
    /// # Example
    /// ```rust
    /// use httpmock::prelude::*;
    /// use reqwest::blocking::Client;
    ///
    /// // Start a new mock server
    /// let server = MockServer::start();
    ///
    /// // Create a mock that expects a cookie named "SESSIONID"
    /// let mock = server.mock(|when, then| {
    ///     when.cookie_exists("SESSIONID");
    ///     then.status(200);  // Respond with a 200 status code if the condition is met
    /// });
    ///
    /// // Make a request that includes the required cookie
    /// Client::new()
    ///     .post(&format!("http://{}/test", server.address()))
    ///     .header("Cookie", "TRACK=12345; SESSIONID=1234567890; CONSENT=1")
    ///     .send()
    ///     .unwrap();
    ///
    /// // Verify that the mock was called at least once
    /// mock.assert();
    /// ```
    ///
    /// # Returns
    /// The updated `When` instance to allow method chaining for additional configuration.
    pub fn cookie_exists<IntoString: Into<String>>(self, name: IntoString) -> Self {
        update_cell(&self.expectations, |e| {
            if e.cookie_exists.is_none() {
                e.cookie_exists = Some(Vec::new());
            }
            e.cookie_exists.as_mut().unwrap().push(name.into());
        });
        self
    }
    // @docs-group: Cookies

    /// Sets the requirement that a cookie with the specified name must not exist in the HTTP request.
    /// Cookie parsing follows [RFC-6265](https://tools.ietf.org/html/rfc6265.html).
    /// **Attention**: Cookie names are **case-sensitive**.
    ///
    /// # Parameters
    /// - `name`: The name of the cookie that must not exist.
    ///
    /// > Note: This function is only available when the `cookies` feature is enabled. This feature is enabled by default.
    ///
    /// # Example
    /// ```rust
    /// use httpmock::prelude::*;
    /// use reqwest::blocking::Client;
    ///
    /// // Start a new mock server
    /// let server = MockServer::start();
    ///
    /// // Create a mock that expects a cookie named "SESSIONID" not to exist
    /// let mock = server.mock(|when, then| {
    ///     when.cookie_missing("SESSIONID");
    ///     then.status(200);  // Respond with a 200 status code if the condition is met
    /// });
    ///
    /// // Make a request that does not include the excluded cookie
    /// Client::new()
    ///     .post(&format!("http://{}/test", server.address()))
    ///     .header("Cookie", "TRACK=12345; CONSENT=1")
    ///     .send()
    ///     .unwrap();
    ///
    /// // Verify that the mock was called at least once
    /// mock.assert();
    /// ```
    ///
    /// # Returns
    /// The updated `When` instance to allow method chaining for additional configuration.
    pub fn cookie_missing<IntoString: Into<String>>(self, name: IntoString) -> Self {
        update_cell(&self.expectations, |e| {
            if e.cookie_missing.is_none() {
                e.cookie_missing = Some(Vec::new());
            }
            e.cookie_missing.as_mut().unwrap().push(name.into());
        });
        self
    }
    // @docs-group: Cookies

    /// Sets the requirement that a cookie with the specified name must exist and its value must contain the specified substring.
    /// Cookie parsing follows [RFC-6265](https://tools.ietf.org/html/rfc6265.html).
    /// **Attention**: Cookie names are **case-sensitive**.
    ///
    /// # Parameters
    /// - `name`: The name of the cookie that must exist.
    /// - `value_substring`: The substring that must be present in the cookie value.
    ///
    /// > Note: This function is only available when the `cookies` feature is enabled. This feature is enabled by default.
    ///
    /// # Example
    /// ```rust
    /// use httpmock::prelude::*;
    /// use reqwest::blocking::Client;
    ///
    /// // Start a new mock server
    /// let server = MockServer::start();
    ///
    /// // Create a mock that expects a cookie named "SESSIONID" with a value containing "1234"
    /// let mock = server.mock(|when, then| {
    ///     when.cookie_includes("SESSIONID", "1234");
    ///     then.status(200);  // Respond with a 200 status code if the condition is met
    /// });
    ///
    /// // Make a request that includes the required cookie
    /// Client::new()
    ///     .post(&format!("http://{}/test", server.address()))
    ///     .header("Cookie", "TRACK=12345; SESSIONID=abc1234def; CONSENT=1")
    ///     .send()
    ///     .unwrap();
    ///
    /// // Verify that the mock was called at least once
    /// mock.assert();
    /// ```
    ///
    /// # Returns
    /// The updated `When` instance to allow method chaining for additional configuration.
    pub fn cookie_includes<KeyString: Into<String>, ValueString: Into<String>>(
        self,
        name: KeyString,
        value_substring: ValueString,
    ) -> Self {
        update_cell(&self.expectations, |e| {
            if e.cookie_includes.is_none() {
                e.cookie_includes = Some(Vec::new());
            }
            e.cookie_includes
                .as_mut()
                .unwrap()
                .push((name.into(), value_substring.into()));
        });
        self
    }
    // @docs-group: Cookies

    /// Sets the requirement that a cookie with the specified name must exist and its value must not contain the specified substring.
    /// Cookie parsing follows [RFC-6265](https://tools.ietf.org/html/rfc6265.html).
    /// **Attention**: Cookie names are **case-sensitive**.
    ///
    /// # Parameters
    /// - `name`: The name of the cookie that must exist.
    /// - `value_substring`: The substring that must not be present in the cookie value.
    ///
    /// > Note: This function is only available when the `cookies` feature is enabled. This feature is enabled by default.
    ///
    /// # Example
    /// ```rust
    /// use httpmock::prelude::*;
    /// use reqwest::blocking::Client;
    ///
    /// // Start a new mock server
    /// let server = MockServer::start();
    ///
    /// // Create a mock that expects a cookie named "SESSIONID" with a value not containing "1234"
    /// let mock = server.mock(|when, then| {
    ///     when.cookie_excludes("SESSIONID", "1234");
    ///     then.status(200);  // Respond with a 200 status code if the condition is met
    /// });
    ///
    /// // Make a request that includes the required cookie
    /// Client::new()
    ///     .post(&format!("http://{}/test", server.address()))
    ///     .header("Cookie", "TRACK=12345; SESSIONID=abcdef; CONSENT=1")
    ///     .send()
    ///     .unwrap();
    ///
    /// // Verify that the mock was called at least once
    /// mock.assert();
    /// ```
    ///
    /// # Returns
    /// The updated `When` instance to allow method chaining for additional configuration.
    pub fn cookie_excludes<KeyString: Into<String>, ValueString: Into<String>>(
        self,
        name: KeyString,
        value_substring: ValueString,
    ) -> Self {
        update_cell(&self.expectations, |e| {
            if e.cookie_excludes.is_none() {
                e.cookie_excludes = Some(Vec::new());
            }
            e.cookie_excludes
                .as_mut()
                .unwrap()
                .push((name.into(), value_substring.into()));
        });
        self
    }
    // @docs-group: Cookies

    /// Sets the requirement that a cookie with the specified name must exist and its value must start with the specified substring.
    /// Cookie parsing follows [RFC-6265](https://tools.ietf.org/html/rfc6265.html).
    /// **Attention**: Cookie names are **case-sensitive**.
    ///
    /// # Parameters
    /// - `name`: The name of the cookie that must exist.
    /// - `value_prefix`: The substring that must be at the start of the cookie value.
    ///
    /// > Note: This function is only available when the `cookies` feature is enabled. This feature is enabled by default.
    ///
    /// # Example
    /// ```rust
    /// use httpmock::prelude::*;
    /// use reqwest::blocking::Client;
    ///
    /// // Start a new mock server
    /// let server = MockServer::start();
    ///
    /// // Create a mock that expects a cookie named "SESSIONID" with a value starting with "1234"
    /// let mock = server.mock(|when, then| {
    ///     when.cookie_prefix("SESSIONID", "1234");
    ///     then.status(200);  // Respond with a 200 status code if the condition is met
    /// });
    ///
    /// // Make a request that includes the required cookie
    /// Client::new()
    ///     .post(&format!("http://{}/test", server.address()))
    ///     .header("Cookie", "TRACK=12345; SESSIONID=1234abcdef; CONSENT=1")
    ///     .send()
    ///     .unwrap();
    ///
    /// // Verify that the mock was called at least once
    /// mock.assert();
    /// ```
    ///
    /// # Returns
    /// The updated `When` instance to allow method chaining for additional configuration.
    pub fn cookie_prefix<KeyString: Into<String>, ValueString: Into<String>>(
        self,
        name: KeyString,
        value_prefix: ValueString,
    ) -> Self {
        update_cell(&self.expectations, |e| {
            if e.cookie_prefix.is_none() {
                e.cookie_prefix = Some(Vec::new());
            }
            e.cookie_prefix
                .as_mut()
                .unwrap()
                .push((name.into(), value_prefix.into()));
        });
        self
    }
    // @docs-group: Cookies

    /// Sets the requirement that a cookie with the specified name must exist and its value must end with the specified substring.
    /// Cookie parsing follows [RFC-6265](https://tools.ietf.org/html/rfc6265.html).
    /// **Attention**: Cookie names are **case-sensitive**.
    ///
    /// # Parameters
    /// - `name`: The name of the cookie that must exist.
    /// - `value_suffix`: The substring that must be at the end of the cookie value.
    ///
    /// > Note: This function is only available when the `cookies` feature is enabled. This feature is enabled by default.
    ///
    /// # Example
    /// ```rust
    /// use httpmock::prelude::*;
    /// use reqwest::blocking::Client;
    ///
    /// // Start a new mock server
    /// let server = MockServer::start();
    ///
    /// // Create a mock that expects a cookie named "SESSIONID" with a value ending with "7890"
    /// let mock = server.mock(|when, then| {
    ///     when.cookie_suffix("SESSIONID", "7890");
    ///     then.status(200);  // Respond with a 200 status code if the condition is met
    /// });
    ///
    /// // Make a request that includes the required cookie
    /// Client::new()
    ///     .post(&format!("http://{}/test", server.address()))
    ///     .header("Cookie", "TRACK=12345; SESSIONID=abcdef7890; CONSENT=1")
    ///     .send()
    ///     .unwrap();
    ///
    /// // Verify that the mock was called at least once
    /// mock.assert();
    /// ```
    ///
    /// # Returns
    /// The updated `When` instance to allow method chaining for additional configuration.
    pub fn cookie_suffix<KeyString: Into<String>, ValueString: Into<String>>(
        self,
        name: KeyString,
        value_suffix: ValueString,
    ) -> Self {
        update_cell(&self.expectations, |e| {
            if e.cookie_suffix.is_none() {
                e.cookie_suffix = Some(Vec::new());
            }
            e.cookie_suffix
                .as_mut()
                .unwrap()
                .push((name.into(), value_suffix.into()));
        });
        self
    }
    // @docs-group: Cookies

    /// Sets the requirement that a cookie with the specified name must exist and its value must not start with the specified substring.
    /// Cookie parsing follows [RFC-6265](https://tools.ietf.org/html/rfc6265.html).
    /// **Attention**: Cookie names are **case-sensitive**.
    ///
    /// # Parameters
    /// - `name`: The name of the cookie that must exist.
    /// - `value_prefix`: The substring that must not be at the start of the cookie value.
    ///
    /// > Note: This function is only available when the `cookies` feature is enabled. This feature is enabled by default.
    ///
    /// # Example
    /// ```rust
    /// use httpmock::prelude::*;
    /// use reqwest::blocking::Client;
    ///
    /// // Start a new mock server
    /// let server = MockServer::start();
    ///
    /// // Create a mock that expects a cookie named "SESSIONID" with a value not starting with "1234"
    /// let mock = server.mock(|when, then| {
    ///     when.cookie_prefix_not("SESSIONID", "1234");
    ///     then.status(200);  // Respond with a 200 status code if the condition is met
    /// });
    ///
    /// // Make a request that includes the required cookie
    /// Client::new()
    ///     .post(&format!("http://{}/test", server.address()))
    ///     .header("Cookie", "TRACK=12345; SESSIONID=abcd1234; CONSENT=1")
    ///     .send()
    ///     .unwrap();
    ///
    /// // Verify that the mock was called at least once
    /// mock.assert();
    /// ```
    ///
    /// # Returns
    /// The updated `When` instance to allow method chaining for additional configuration.
    pub fn cookie_prefix_not<KeyString: Into<String>, ValueString: Into<String>>(
        self,
        name: KeyString,
        value_prefix: ValueString,
    ) -> Self {
        update_cell(&self.expectations, |e| {
            if e.cookie_prefix_not.is_none() {
                e.cookie_prefix_not = Some(Vec::new());
            }
            e.cookie_prefix_not
                .as_mut()
                .unwrap()
                .push((name.into(), value_prefix.into()));
        });
        self
    }
    // @docs-group: Cookies

    /// Sets the requirement that a cookie with the specified name must exist and its value must not end with the specified substring.
    /// Cookie parsing follows [RFC-6265](https://tools.ietf.org/html/rfc6265.html).
    /// **Attention**: Cookie names are **case-sensitive**.
    ///
    /// # Parameters
    /// - `name`: The name of the cookie that must exist.
    /// - `value_suffix`: The substring that must not be at the end of the cookie value.
    ///
    /// > Note: This function is only available when the `cookies` feature is enabled. This feature is enabled by default.
    ///
    /// # Example
    /// ```rust
    /// use httpmock::prelude::*;
    /// use reqwest::blocking::Client;
    ///
    /// // Start a new mock server
    /// let server = MockServer::start();
    ///
    /// // Create a mock that expects a cookie named "SESSIONID" with a value not ending with "7890"
    /// let mock = server.mock(|when, then| {
    ///     when.cookie_suffix_not("SESSIONID", "7890");
    ///     then.status(200);  // Respond with a 200 status code if the condition is met
    /// });
    ///
    /// // Make a request that includes the required cookie
    ///  Client::new()
    ///     .post(&format!("http://{}/test", server.address()))
    ///     .header("Cookie", "TRACK=12345; SESSIONID=abcdef1234; CONSENT=1")
    ///     .send()
    ///     .unwrap();
    ///
    /// // Verify that the mock was called at least once
    /// mock.assert();
    /// ```
    ///
    /// # Returns
    /// The updated `When` instance to allow method chaining for additional configuration.
    pub fn cookie_suffix_not<KeyString: Into<String>, ValueString: Into<String>>(
        self,
        name: KeyString,
        value_suffix: ValueString,
    ) -> Self {
        update_cell(&self.expectations, |e| {
            if e.cookie_suffix_not.is_none() {
                e.cookie_suffix_not = Some(Vec::new());
            }
            e.cookie_suffix_not
                .as_mut()
                .unwrap()
                .push((name.into(), value_suffix.into()));
        });
        self
    }
    // @docs-group: Cookies

    /// Sets the requirement that a cookie with a name matching the specified regex must exist and its value must match the specified regex.
    /// Cookie parsing follows [RFC-6265](https://tools.ietf.org/html/rfc6265.html).
    /// **Attention**: Cookie names are **case-sensitive**.
    ///
    /// # Parameters
    /// - `key_regex`: The regex pattern that the cookie name must match.
    /// - `value_regex`: The regex pattern that the cookie value must match.
    ///
    /// > Note: This function is only available when the `cookies` feature is enabled. This feature is enabled by default.
    ///
    /// # Example
    /// ```rust
    /// use httpmock::prelude::*;
    /// use reqwest::blocking::Client;
    ///
    /// // Start a new mock server
    /// let server = MockServer::start();
    ///
    /// // Create a mock that expects a cookie with a name matching the regex "^SESSION"
    /// // and a value matching the regex "^[0-9]{10}$"
    /// let mock = server.mock(|when, then| {
    ///     when.cookie_matches(r"^SESSION", r"^[0-9]{10}$");
    ///     then.status(200);  // Respond with a 200 status code if the condition is met
    /// });
    ///
    /// // Make a request that includes the required cookie
    /// Client::new()
    ///     .post(&format!("http://{}/test", server.address()))
    ///     .header("Cookie", "TRACK=12345; SESSIONID=1234567890; CONSENT=1")
    ///     .send()
    ///     .unwrap();
    ///
    /// // Verify that the mock was called at least once
    /// mock.assert();
    /// ```
    ///
    /// # Returns
    /// The updated `When` instance to allow method chaining for additional configuration.
    pub fn cookie_matches<KeyRegex: Into<Regex>, ValueRegex: Into<Regex>>(
        self,
        key_regex: KeyRegex,
        value_regex: ValueRegex,
    ) -> Self {
        update_cell(&self.expectations, |e| {
            if e.cookie_matches.is_none() {
                e.cookie_matches = Some(Vec::new());
            }
            e.cookie_matches
                .as_mut()
                .unwrap()
                .push((key_regex.into(), value_regex.into()));
        });
        self
    }
    // @docs-group: Cookies

    /// Sets the requirement that a cookie with a name and value matching the specified regexes must appear a specified number of times in the HTTP request.
    /// Cookie parsing follows [RFC-6265](https://tools.ietf.org/html/rfc6265.html).
    /// **Attention**: Cookie names are **case-sensitive**.
    ///
    /// # Parameters
    /// - `key_regex`: The regex pattern that the cookie name must match.
    /// - `value_regex`: The regex pattern that the cookie value must match.
    /// - `count`: The number of times a cookie with a matching name and value must appear.
    ///
    /// > Note: This function is only available when the `cookies` feature is enabled. This feature is enabled by default.
    ///
    /// # Example
    /// ```rust
    /// use httpmock::prelude::*;
    /// use reqwest::blocking::Client;
    ///
    /// // Start a new mock server
    /// let server = MockServer::start();
    ///
    /// // Create a mock that expects a cookie with a name matching the regex "^SESSION"
    /// // and a value matching the regex "^[0-9]{10}$" to appear exactly twice
    /// let mock = server.mock(|when, then| {
    ///     when.cookie_count(r"^SESSION", r"^[0-9]{10}$", 2);
    ///     then.status(200);  // Respond with a 200 status code if the condition is met
    /// });
    ///
    /// // Make a request that includes the required cookies
    /// Client::new()
    ///     .post(&format!("http://{}/test", server.address()))
    ///     .header("Cookie", "SESSIONID=1234567890; TRACK=12345; SESSIONTOKEN=0987654321; CONSENT=1")
    ///     .send()
    ///     .unwrap();
    ///
    /// // Verify that the mock was called at least once
    /// mock.assert();
    /// ```
    ///
    /// # Returns
    /// The updated `When` instance to allow method chaining for additional configuration.
    pub fn cookie_count<KeyRegex: Into<Regex>, ValueRegex: Into<Regex>>(
        self,
        key_regex: KeyRegex,
        value_regex: ValueRegex,
        count: usize,
    ) -> Self {
        update_cell(&self.expectations, |e| {
            if e.cookie_count.is_none() {
                e.cookie_count = Some(Vec::new());
            }
            e.cookie_count
                .as_mut()
                .unwrap()
                .push((key_regex.into(), value_regex.into(), count));
        });
        self
    }
    // @docs-group: Cookies

    /// Sets the required HTTP request body content.
    /// This method specifies that the HTTP request body must match the provided content exactly.
    ///
    /// **Note**: The body content is case-sensitive and must be an exact match.
    ///
    /// # Parameters
    /// - `body`: The required HTTP request body content. This parameter accepts any type that can be converted into a `String`.
    ///
    /// # Example
    /// ```rust
    /// use httpmock::prelude::*;
    /// use reqwest::blocking::Client;
    ///
    /// // Start a new mock server
    /// let server = MockServer::start();
    ///
    /// // Create a mock that expects the request body to be "The Great Gatsby"
    /// let mock = server.mock(|when, then| {
    ///     when.body("The Great Gatsby");
    ///     then.status(200);  // Respond with a 200 status code if the condition is met
    /// });
    ///
    /// // Make a request with the required body content
    /// Client::new()
    ///     .post(&format!("http://{}/test", server.address()))
    ///     .body("The Great Gatsby")
    ///     .send()
    ///     .unwrap();
    ///
    /// // Verify that the mock was called at least once
    /// mock.assert();
    /// ```
    ///
    /// # Returns
    /// The updated `When` instance to allow method chaining for additional configuration.
    pub fn body<IntoString: Into<String>>(self, body: IntoString) -> Self {
        update_cell(&self.expectations, |e| {
            e.body = Some(HttpMockBytes::from(Bytes::from(body.into())));
        });
        self
    }
    // @docs-group: Body

    /// Sets the condition that the HTTP request body content must not match the specified value.
    /// This method ensures that the request body does not contain the provided content exactly.
    ///
    /// **Note**: The body content is case-sensitive and must be an exact mismatch.
    ///
    /// # Parameters
    /// - `body`: The body content that the HTTP request must not contain. This parameter accepts any type that can be converted into a `String`.
    ///
    /// # Example
    /// ```rust
    /// use httpmock::prelude::*;
    /// use reqwest::blocking::Client;
    ///
    /// // Start a new mock server
    /// let server = MockServer::start();
    ///
    /// // Create a mock that expects the request body to not be "The Great Gatsby"
    /// let mock = server.mock(|when, then| {
    ///     when.body_not("The Great Gatsby");
    ///     then.status(200);  // Respond with a 200 status code if the condition is met
    /// });
    ///
    /// // Make a request with a different body content
    /// Client::new()
    ///     .post(&format!("http://{}/test", server.address()))
    ///     .body("A Tale of Two Cities")
    ///     .send()
    ///     .unwrap();
    ///
    /// // Verify that the mock was called at least once
    /// mock.assert();
    /// ```
    ///
    /// # Returns
    /// The updated `When` instance to allow method chaining for additional configuration.
    pub fn body_not<IntoString: Into<String>>(self, body: IntoString) -> Self {
        update_cell(&self.expectations, |e| {
            if e.body_not.is_none() {
                e.body_not = Some(Vec::new());
            }
            e.body_not
                .as_mut()
                .unwrap()
                .push(HttpMockBytes::from(Bytes::from(body.into())));
        });
        self
    }
    // @docs-group: Body

    /// Sets the condition that the HTTP request body content must contain the specified substring.
    /// This method ensures that the request body includes the provided content as a substring.
    ///
    /// **Note**: The body content is case-sensitive.
    ///
    /// # Parameters
    /// - `substring`: The substring that the HTTP request body must contain. This parameter accepts any type that can be converted into a `String`.
    ///
    /// # Example
    /// ```rust
    /// use httpmock::prelude::*;
    /// use reqwest::blocking::Client;
    ///
    /// // Start a new mock server
    /// let server = MockServer::start();
    ///
    /// // Create a mock that expects the request body to contain the substring "Gatsby"
    /// let mock = server.mock(|when, then| {
    ///     when.body_includes("Gatsby");
    ///     then.status(200);  // Respond with a 200 status code if the condition is met
    /// });
    ///
    /// // Make a request with the required substring in the body content
    /// Client::new()
    ///     .post(&format!("http://{}/test", server.address()))
    ///     .body("The Great Gatsby is a novel.")
    ///     .send()
    ///     .unwrap();
    ///
    /// // Verify that the mock was called at least once
    /// mock.assert();
    /// ```
    ///
    /// # Returns
    /// The updated `When` instance to allow method chaining for additional configuration.
    pub fn body_includes<IntoString: Into<String>>(self, substring: IntoString) -> Self {
        update_cell(&self.expectations, |e| {
            if e.body_includes.is_none() {
                e.body_includes = Some(Vec::new());
            }
            e.body_includes
                .as_mut()
                .unwrap()
                .push(HttpMockBytes::from(Bytes::from(substring.into())));
        });
        self
    }
    // @docs-group: Body

    /// Sets the condition that the HTTP request body content must not contain the specified substring.
    /// This method ensures that the request body does not include the provided content as a substring.
    ///
    /// **Note**: The body content is case-sensitive.
    ///
    /// # Parameters
    /// - `substring`: The substring that the HTTP request body must not contain. This parameter accepts any type that can be converted into a `String`.
    ///
    /// # Example
    /// ```rust
    /// use httpmock::prelude::*;
    /// use reqwest::blocking::Client;
    ///
    /// // Start a new mock server
    /// let server = MockServer::start();
    ///
    /// // Create a mock that expects the request body to not contain the substring "Gatsby"
    /// let mock = server.mock(|when, then| {
    ///     when.body_excludes("Gatsby");
    ///     then.status(200);  // Respond with a 200 status code if the condition is met
    /// });
    ///
    /// // Make a request with a different body content
    /// Client::new()
    ///     .post(&format!("http://{}/test", server.address()))
    ///     .body("A Tale of Two Cities is a novel.")
    ///     .send()
    ///     .unwrap();
    ///
    /// // Verify that the mock was called at least once
    /// mock.assert();
    /// ```
    ///
    /// # Returns
    /// The updated `When` instance to allow method chaining for additional configuration.
    pub fn body_excludes<IntoString: Into<String>>(self, substring: IntoString) -> Self {
        update_cell(&self.expectations, |e| {
            if e.body_excludes.is_none() {
                e.body_excludes = Some(Vec::new());
            }
            e.body_excludes
                .as_mut()
                .unwrap()
                .push(HttpMockBytes::from(Bytes::from(substring.into())));
        });
        self
    }
    // @docs-group: Body

    /// Sets the condition that the HTTP request body content must begin with the specified substring.
    /// This method ensures that the request body starts with the provided content as a substring.
    ///
    /// **Note**: The body content is case-sensitive.
    ///
    /// # Parameters
    /// - `prefix`: The substring that the HTTP request body must begin with. This parameter accepts any type that can be converted into a `String`.
    ///
    /// # Example
    /// ```rust
    /// use httpmock::prelude::*;
    /// use reqwest::blocking::Client;
    ///
    /// // Start a new mock server
    /// let server = MockServer::start();
    ///
    /// // Create a mock that expects the request body to begin with the substring "The Great"
    /// let mock = server.mock(|when, then| {
    ///     when.body_prefix("The Great");
    ///     then.status(200);  // Respond with a 200 status code if the condition is met
    /// });
    ///
    /// // Make a request with the required prefix in the body content
    /// Client::new()
    ///     .post(&format!("http://{}/test", server.address()))
    ///     .body("The Great Gatsby is a novel.")
    ///     .send()
    ///     .unwrap();
    ///
    /// // Verify that the mock was called at least once
    /// mock.assert();
    /// ```
    ///
    /// # Returns
    /// The updated `When` instance to allow method chaining for additional configuration.
    pub fn body_prefix<IntoString: Into<String>>(self, prefix: IntoString) -> Self {
        update_cell(&self.expectations, |e| {
            if e.body_prefix.is_none() {
                e.body_prefix = Some(Vec::new());
            }
            e.body_prefix
                .as_mut()
                .unwrap()
                .push(HttpMockBytes::from(Bytes::from(prefix.into())));
        });
        self
    }
    // @docs-group: Body

    /// Sets the condition that the HTTP request body content must end with the specified substring.
    /// This method ensures that the request body concludes with the provided content as a substring.
    ///
    /// **Note**: The body content is case-sensitive.
    ///
    /// # Parameters
    /// - `suffix`: The substring that the HTTP request body must end with. This parameter accepts any type that can be converted into a `String`.
    ///
    /// # Example
    /// ```rust
    /// use httpmock::prelude::*;
    /// use reqwest::blocking::Client;
    ///
    /// // Start a new mock server
    /// let server = MockServer::start();
    ///
    /// // Create a mock that expects the request body to end with the substring "a novel."
    /// let mock = server.mock(|when, then| {
    ///     when.body_suffix("a novel.");
    ///     then.status(200);  // Respond with a 200 status code if the condition is met
    /// });
    ///
    /// // Make a request with the required suffix in the body content
    /// Client::new()
    ///     .post(&format!("http://{}/test", server.address()))
    ///     .body("The Great Gatsby is a novel.")
    ///     .send()
    ///     .unwrap();
    ///
    /// // Verify that the mock was called at least once
    /// mock.assert();
    /// ```
    ///
    /// # Returns
    /// The updated `When’ instance to allow method chaining for additional configuration.
    pub fn body_suffix<IntoString: Into<String>>(self, suffix: IntoString) -> Self {
        update_cell(&self.expectations, |e| {
            if e.body_suffix.is_none() {
                e.body_suffix = Some(Vec::new());
            }
            e.body_suffix
                .as_mut()
                .unwrap()
                .push(HttpMockBytes::from(Bytes::from(suffix.into())));
        });
        self
    }
    // @docs-group: Body

    /// Sets the condition that the HTTP request body content must not begin with the specified substring.
    /// This method ensures that the request body does not start with the provided content as a substring.
    ///
    /// **Note**: The body content is case-sensitive.
    ///
    /// # Parameters
    /// - `prefix`: The substring that the HTTP request body must not begin with. This parameter accepts any type that can be converted into a `String`.
    ///
    /// # Example
    /// ```rust
    /// use httpmock::prelude::*;
    /// use reqwest::blocking::Client;
    ///
    /// // Start a new mock server
    /// let server = MockServer::start();
    ///
    /// // Create a mock that expects the request body to not begin with the substring "Error:"
    /// let mock = server.mock(|when, then| {
    ///     when.body_prefix_not("Error:");
    ///     then.status(200);  // Respond with a 200 status code if the condition is met
    /// });
    ///
    /// // Make a request with a different body content
    /// Client::new()
    ///     .post(&format!("http://{}/test", server.address()))
    ///     .body("Success: Operation completed.")
    ///     .send()
    ///     .unwrap();
    ///
    /// // Verify that the mock was called at least once
    /// mock.assert();
    /// ```
    ///
    /// # Returns
    /// The updated `When’ instance to allow method chaining for additional configuration.
    pub fn body_prefix_not<IntoString: Into<String>>(self, prefix: IntoString) -> Self {
        update_cell(&self.expectations, |e| {
            if e.body_prefix_not.is_none() {
                e.body_prefix_not = Some(Vec::new());
            }
            e.body_prefix_not
                .as_mut()
                .unwrap()
                .push(HttpMockBytes::from(Bytes::from(prefix.into())));
        });
        self
    }
    // @docs-group: Body

    /// Sets the condition that the HTTP request body content must not end with the specified substring.
    /// This method ensures that the request body does not conclude with the provided content as a substring.
    ///
    /// **Note**: The body content is case-sensitive.
    ///
    /// # Parameters
    /// - `suffix`: The substring that the HTTP request body must not end with. This parameter accepts any type that can be converted into a `String`.
    ///
    /// # Example
    /// ```rust
    /// use httpmock::prelude::*;
    /// use reqwest::blocking::Client;
    ///
    /// // Start a new mock server
    /// let server = MockServer::start();
    ///
    /// // Create a mock that expects the request body to not end with the substring "a novel."
    /// let mock = server.mock(|when, then| {
    ///     when.body_suffix_not("a novel.");
    ///     then.status(200);  // Respond with a 200 status code if the condition is met
    /// });
    ///
    /// // Make a request with a different body content
    /// Client::new()
    ///     .post(&format!("http://{}/test", server.address()))
    ///     .body("The Great Gatsby is a story.")
    ///     .send()
    ///     .unwrap();
    ///
    /// // Verify that the mock was called at least once
    /// mock.assert();
    /// ```
    ///
    /// # Returns
    /// The updated `When’ instance to allow method chaining for additional configuration.
    pub fn body_suffix_not<IntoString: Into<String>>(self, suffix: IntoString) -> Self {
        update_cell(&self.expectations, |e| {
            if e.body_suffix_not.is_none() {
                e.body_suffix_not = Some(Vec::new());
            }
            e.body_suffix_not
                .as_mut()
                .unwrap()
                .push(HttpMockBytes::from(Bytes::from(suffix.into())));
        });
        self
    }
    // @docs-group: Body

    /// Sets the condition that the HTTP request body content must match the specified regular expression.
    /// This method ensures that the request body fully conforms to the provided regex pattern.
    ///
    /// **Note**: The regex matching is case-sensitive unless the regex is explicitly defined to be case-insensitive.
    ///
    /// # Parameters
    /// - `pattern`: The regular expression pattern that the HTTP request body must match. This parameter accepts any type that can be converted into a `Regex`.
    ///
    /// # Example
    /// ```rust
    /// use httpmock::prelude::*;
    /// use reqwest::blocking::Client;
    ///
    /// // Start a new mock server
    /// let server = MockServer::start();
    ///
    /// // Create a mock that expects the request body to match the regex pattern "^The Great Gatsby.*"
    /// let mock = server.mock(|when, then| {
    ///     when.body_matches("^The Great Gatsby.*");
    ///     then.status(200);  // Respond with a 200 status code if the condition is met
    /// });
    ///
    /// // Make a request with a body that matches the regex pattern
    /// Client::new()
    ///     .post(&format!("http://{}/test", server.address()))
    ///     .body("The Great Gatsby is a novel by F. Scott Fitzgerald.")
    ///     .send()
    ///     .unwrap();
    ///
    /// // Verify that the mock was called at least once
    /// mock.assert();
    /// ```
    ///
    /// # Returns
    /// The updated `When’ instance to allow method chaining for additional configuration.
    pub fn body_matches<IntoRegex: Into<Regex>>(self, pattern: IntoRegex) -> Self {
        update_cell(&self.expectations, |e| {
            if e.body_matches.is_none() {
                e.body_matches = Some(Vec::new());
            }
            e.body_matches.as_mut().unwrap().push(pattern.into());
        });
        self
    }
    // @docs-group: Body

    /// Sets the condition that the HTTP request body content must match the specified JSON structure.
    /// This method ensures that the request body exactly matches the JSON value provided.
    ///
    /// **Note**: The body content is case-sensitive.
    ///
    /// **Note**: This method does not automatically verify the `Content-Type` header.
    /// If specific content type verification is required (e.g., `application/json`),
    /// you must add this expectation manually.
    ///
    /// # Parameters
    /// - `json_value`: The JSON structure that the HTTP request body must match. This parameter accepts any type that can be converted into a `serde_json::Value`.
    ///
    /// # Example
    /// ```rust
    /// use httpmock::prelude::*;
    /// use reqwest::blocking::Client;
    /// use serde_json::json;
    ///
    /// // Start a new mock server
    /// let server = MockServer::start();
    ///
    /// // Create a mock that expects the request body to match a specific JSON structure
    /// let mock = server.mock(|when, then| {
    ///     when.json_body(json!({
    ///         "title": "The Great Gatsby",
    ///         "author": "F. Scott Fitzgerald"
    ///     }));
    ///     then.status(200);  // Respond with a 200 status code if the condition is met
    /// });
    ///
    /// // Make a request with a JSON body that matches the expected structure
    /// Client::new()
    ///     .post(&format!("http://{}/test", server.address()))
    ///     .header("Content-Type", "application/json") // It's important to set the Content-Type header manually
    ///     .body(r#"{"title":"The Great Gatsby","author":"F. Scott Fitzgerald"}"#)
    ///     .send()
    ///     .unwrap();
    ///
    /// // Verify that the mock was called at least once
    /// mock.assert();
    /// ```
    ///
    /// # Returns
    /// The updated `When’ instance to allow method chaining for additional configuration.
    pub fn json_body<JsonValue: Into<Value>>(self, json_value: JsonValue) -> Self {
        update_cell(&self.expectations, |e| {
            e.json_body = Some(json_value.into());
        });
        self
    }
    // @docs-group: Body

    /// Sets the expected JSON body using a serializable serde object.
    /// This function automatically serializes the given object into a JSON string using serde.
    ///
    /// **Note**: This method does not automatically verify the `Content-Type` header.
    /// If specific content type verification is required (e.g., `application/json`),
    /// you must add this expectation manually.
    ///
    /// # Parameters
    /// - `body`: The HTTP body object to be serialized to JSON. This object should implement both `serde::Serialize` and `serde::Deserialize`.
    ///
    /// # Example
    /// ```rust
    /// use httpmock::prelude::*;
    /// use reqwest::blocking::Client;
    /// use serde_json::json;
    /// use serde::{Serialize, Deserialize};
    ///
    /// #[derive(Serialize, Deserialize)]
    /// struct TestUser {
    ///     name: String,
    /// }
    ///
    /// // Initialize logging (optional, for debugging purposes)
    /// tracing_subscriber::fmt::init();
    ///
    /// // Start the mock server
    /// let server = MockServer::start();
    ///
    /// // Set up a mock endpoint
    /// let m = server.mock(|when, then| {
    ///     when.path("/user")
    ///         .header("content-type", "application/json")
    ///         .json_body_obj(&TestUser { name: String::from("Fred") });
    ///     then.status(200);
    /// });
    ///
    /// // Send a POST request with a JSON body
    /// let response = Client::new()
    ///     .post(&format!("http://{}/user", server.address()))
    ///     .header("content-type", "application/json")
    ///     .body(json!(&TestUser { name: "Fred".to_string() }).to_string())
    ///     .send()
    ///     .unwrap();
    ///
    /// // Assert the mock was called and the response status is as expected
    /// m.assert();
    /// assert_eq!(response.status(), 200);
    /// ```
    ///
    /// This method is particularly useful when you need to test server responses to structured JSON data. It helps
    /// ensure that the JSON serialization and deserialization processes are correctly implemented in your API handling logic.
    pub fn json_body_obj<T>(self, body: &T) -> Self
    where
        T: Serialize + ?Sized,
    {
        let json_value = serde_json::to_value(body).expect("Cannot serialize json body to JSON");
        self.json_body(json_value)
    }
    // @docs-group: Body

    /// Sets the expected partial JSON body to check for specific content within a larger JSON structure.
    ///
    /// **Attention:** The partial JSON string must be a valid JSON string and should represent a substructure
    /// of the full JSON object. It can omit irrelevant attributes but must maintain any necessary object hierarchy.
    ///
    /// **Note:** This method does not automatically set the `Content-Type` header to `application/json`.
    /// You must explicitly set this header in your requests.
    ///
    /// # Parameters
    /// - `partial_body`: The partial JSON content to check for. This must be a valid JSON string.
    ///
    /// # Example
    /// Suppose your application sends the following JSON request body:
    /// ```json
    /// {
    ///     "parent_attribute": "Some parent data goes here",
    ///     "child": {
    ///         "target_attribute": "Example",
    ///         "other_attribute": "Another value"
    ///     }
    /// }
    /// ```
    /// To verify the presence of `target_attribute` with the value `Example` without needing the entire JSON object:
    /// ```rust
    /// use httpmock::prelude::*;
    /// use reqwest::blocking::Client;
    ///
    /// let server = MockServer::start();
    ///
    /// let mock = server.mock(|when, then| {
    ///     when.json_body_includes(r#"
    ///         {
    ///             "child": {
    ///                 "target_attribute": "Example"
    ///             }
    ///         }
    ///     "#);
    ///     then.status(200);
    /// });
    ///
    /// // Send a POST request with a JSON body
    /// let response = Client::new()
    ///     .post(&format!("http://{}/some/path", server.address()))
    ///     .header("content-type", "application/json")
    ///     .body(r#"
    ///         {
    ///             "parent_attribute": "Some parent data goes here",
    ///             "child": {
    ///                 "target_attribute": "Example",
    ///                 "other_attribute": "Another value"
    ///             }
    ///         }
    ///     "#)
    ///     .send()
    ///     .unwrap();
    ///
    /// // Assert the mock was called and the response status is as expected
    /// mock.assert();
    /// assert_eq!(response.status(), 200);
    /// ```
    /// It's important that the partial JSON contains the full object hierarchy necessary to reach the target attribute.
    /// Irrelevant attributes such as `parent_attribute` and `child.other_attribute` can be omitted.
    pub fn json_body_includes<IntoString: Into<String>>(self, partial: IntoString) -> Self {
        update_cell(&self.expectations, |e| {
            if e.json_body_includes.is_none() {
                e.json_body_includes = Some(Vec::new());
            }
            let value = Value::from_str(&partial.into())
                .expect("cannot convert JSON string to serde value");
            e.json_body_includes.as_mut().unwrap().push(value);
        });
        self
    }
    // @docs-group: Body

    /// Sets the expected partial JSON body to ensure that specific content is not present within a larger JSON structure.
    ///
    /// **Attention:** The partial JSON string must be a valid JSON string and should represent a substructure
    /// of the full JSON object. It can omit irrelevant attributes but must maintain any necessary object hierarchy.
    ///
    /// **Note:** This method does not automatically set the `Content-Type` header to `application/json`.
    /// You must explicitly set this header in your requests.
    ///
    /// # Parameters
    /// - `partial_body`: The partial JSON content to check for exclusion. This must be a valid JSON string.
    ///
    /// # Example
    /// Suppose your application sends the following JSON request body:
    /// ```json
    /// {
    ///     "parent_attribute": "Some parent data goes here",
    ///     "child": {
    ///         "target_attribute": "Example",
    ///         "other_attribute": "Another value"
    ///     }
    /// }
    /// ```
    /// To verify the absence of `target_attribute` with the value `Example`:
    /// ```rust
    /// use httpmock::prelude::*;
    /// use reqwest::blocking::Client;
    ///
    /// let server = MockServer::start();
    ///
    /// let mock = server.mock(|when, then| {
    ///     when.json_body_excludes(r#"
    ///         {
    ///             "child": {
    ///                 "target_attribute": "Example"
    ///             }
    ///         }
    ///     "#);
    ///     then.status(200);
    /// });
    ///
    /// // Send a POST request with a JSON body
    /// let response = Client::new()
    ///     .post(&format!("http://{}/some/path", server.address()))
    ///     .header("content-type", "application/json")
    ///     .body(r#"
    ///         {
    ///             "parent_attribute": "Some parent data goes here",
    ///             "child": {
    ///                 "other_attribute": "Another value"
    ///             }
    ///         }
    ///     "#)
    ///     .send()
    ///     .unwrap();
    ///
    /// // Assert the mock was called and the response status is as expected
    /// mock.assert();
    /// assert_eq!(response.status(), 200);
    /// ```
    /// It's important that the partial JSON contains the full object hierarchy necessary to reach the target attribute.
    /// Irrelevant attributes such as `parent_attribute` and `child.other_attribute` in the example can be omitted.
    pub fn json_body_excludes<IntoString: Into<String>>(self, partial: IntoString) -> Self {
        update_cell(&self.expectations, |e| {
            if e.json_body_excludes.is_none() {
                e.json_body_excludes = Some(Vec::new());
            }
            let value = Value::from_str(&partial.into())
                .expect("cannot convert JSON string to serde value");
            e.json_body_excludes.as_mut().unwrap().push(value);
        });
        self
    }
    // @docs-group: Body

    /// Adds a key-value pair to the requirements for an `application/x-www-form-urlencoded` request body.
    ///
    /// This method sets an expectation for a specific key-value pair to be included in the request body
    /// of an `application/x-www-form-urlencoded` POST request. Each key and value are URL-encoded as specified
    /// by the [URL Standard](https://url.spec.whatwg.org/#application/x-www-form-urlencoded).
    ///
    /// **Note**: The mock server does not automatically verify that the HTTP method is POST as per spec.
    /// If you want to verify that the request method is POST, you must explicitly set it in your mock configuration.
    ///
    /// # Parameters
    /// - `key`: The key of the key-value pair to set as a requirement.
    /// - `value`: The value of the key-value pair to set as a requirement.
    ///
    /// # Example
    /// ```rust
    /// use httpmock::prelude::*;
    /// use reqwest::blocking::Client;
    ///
    /// // Arrange
    /// let server = MockServer::start();
    ///
    /// let m = server.mock(|when, then| {
    ///    when.method(POST)
    ///        .path("/example")
    ///        .header("content-type", "application/x-www-form-urlencoded")
    ///        .form_urlencoded_tuple("name", "Peter Griffin")
    ///        .form_urlencoded_tuple("town", "Quahog");
    ///    then.status(202);
    /// });
    ///
    /// let response = Client::new()
    ///    .post(server.url("/example"))
    ///    .header("content-type", "application/x-www-form-urlencoded")
    ///    .body("name=Peter%20Griffin&town=Quahog")
    ///    .send()
    ///    .unwrap();
    ///
    /// // Assert
    /// m.assert();
    /// assert_eq!(response.status(), 202);
    /// ```
    ///
    /// # Returns
    /// `When`: Returns the modified `When` object with the new key-value pair added to the `application/x-www-form-urlencoded` expectations.
    pub fn form_urlencoded_tuple<KeyString: Into<String>, ValueString: Into<String>>(
        self,
        key: KeyString,
        value: ValueString,
    ) -> Self {
        update_cell(&self.expectations, |e| {
            if e.form_urlencoded_tuple.is_none() {
                e.form_urlencoded_tuple = Some(Vec::new());
            }
            e.form_urlencoded_tuple
                .as_mut()
                .unwrap()
                .push((key.into(), value.into()));
        });
        self
    }
    // @docs-group: Body

    /// Adds a key-value pair to the negative requirements for an `application/x-www-form-urlencoded` request body.
    ///
    /// This method sets an expectation for a specific key-value pair to be excluded from the request body
    /// of an `application/x-www-form-urlencoded` POST request. Each key and value are URL-encoded as specified
    /// by the [URL Standard](https://url.spec.whatwg.org/#application/x-www-form-urlencoded).
    ///
    /// **Note**: The mock server does not automatically verify that the HTTP method is POST as per spec.
    /// If you want to verify that the request method is POST, you must explicitly set it in your mock configuration.
    ///
    /// # Parameters
    /// - `key`: The key of the key-value pair to set as a requirement.
    /// - `value`: The value of the key-value pair to set as a requirement.
    ///
    /// # Example
    /// ```rust
    /// use httpmock::prelude::*;
    /// use reqwest::blocking::Client;
    ///
    /// // Arrange
    /// let server = MockServer::start();
    ///
    /// let m = server.mock(|when, then| {
    ///    when.method(POST)
    ///        .path("/example")
    ///        .header("content-type", "application/x-www-form-urlencoded")
    ///        .form_urlencoded_tuple_not("name", "Peter Griffin");
    ///    then.status(202);
    /// });
    ///
    /// let response = Client::new()
    ///    .post(server.url("/example"))
    ///    .header("content-type", "application/x-www-form-urlencoded")
    ///    .body("name=Lois%20Griffin&town=Quahog")
    ///    .send()
    ///    .unwrap();
    ///
    /// // Assert
    /// m.assert();
    /// assert_eq!(response.status(), 202);
    /// ```
    ///
    /// # Returns
    /// `When`: Returns the modified `When` object with the new key-value pair added to the negative `application/x-www-form-urlencoded` expectations.
    pub fn form_urlencoded_tuple_not<KeyString: Into<String>, ValueString: Into<String>>(
        self,
        key: KeyString,
        value: ValueString,
    ) -> Self {
        update_cell(&self.expectations, |e| {
            if e.form_urlencoded_tuple_not.is_none() {
                e.form_urlencoded_tuple_not = Some(Vec::new());
            }
            e.form_urlencoded_tuple_not
                .as_mut()
                .unwrap()
                .push((key.into(), value.into()));
        });
        self
    }
    // @docs-group: Body

    /// Sets a requirement for the existence of a key in an `application/x-www-form-urlencoded` request body.
    ///
    /// This method sets an expectation that a specific key must be present in the request body of an
    /// `application/x-www-form-urlencoded` POST request, regardless of its value. The key is URL-encoded
    /// as specified by the [URL Standard](https://url.spec.whatwg.org/#application/x-www-form-urlencoded).
    ///
    /// **Note**: The mock server does not automatically verify that the HTTP method is POST as per spec.
    /// If you want to verify that the request method is POST, you must explicitly set it in your mock configuration.
    ///
    /// # Parameters
    /// - `key`: The key that must exist in the `application/x-www-form-urlencoded` request body.
    ///
    /// # Example
    /// ```rust
    /// use httpmock::prelude::*;
    /// use reqwest::blocking::Client;
    ///
    /// // Arrange
    /// let server = MockServer::start();
    ///
    /// let m = server.mock(|when, then| {
    ///    when.method(POST)
    ///        .path("/example")
    ///        .header("content-type", "application/x-www-form-urlencoded")
    ///        .form_urlencoded_tuple_exists("name")
    ///        .form_urlencoded_tuple_exists("town");
    ///    then.status(202);
    /// });
    ///
    /// let response = Client::new()
    ///    .post(server.url("/example"))
    ///    .header("content-type", "application/x-www-form-urlencoded")
    ///    .body("name=Peter%20Griffin&town=Quahog")
    ///    .send()
    ///    .unwrap();
    ///
    /// // Assert
    /// m.assert();
    /// assert_eq!(response.status(), 202);
    /// ```
    ///
    /// # Returns
    /// `When`: Returns the modified `When` object with the new key existence requirement added to the
    /// `application/x-www-form-urlencoded` expectations.
    pub fn form_urlencoded_tuple_exists<IntoString: Into<String>>(self, key: IntoString) -> Self {
        update_cell(&self.expectations, |e| {
            if e.form_urlencoded_tuple_exists.is_none() {
                e.form_urlencoded_tuple_exists = Some(Vec::new());
            }
            e.form_urlencoded_tuple_exists
                .as_mut()
                .unwrap()
                .push(key.into());
        });
        self
    }
    // @docs-group: Body

    /// Sets a requirement that a key must be absent in an `application/x-www-form-urlencoded` request body.
    ///
    /// This method sets an expectation that a specific key must not be present in the request body of an
    /// `application/x-www-form-urlencoded` POST request. The key is URL-encoded as specified by the
    /// [URL Standard](https://url.spec.whatwg.org/#application/x-www-form-urlencoded).
    ///
    /// **Note**: The mock server does not automatically verify that the HTTP method is POST as per spec.
    /// If you want to verify that the request method is POST, you must explicitly set it in your mock configuration.
    ///
    /// # Parameters
    /// - `key`: The key that must be absent in the `application/x-www-form-urlencoded` request body.
    ///
    /// # Example
    /// ```rust
    /// use httpmock::prelude::*;
    /// use reqwest::blocking::Client;
    ///
    /// // Arrange
    /// let server = MockServer::start();
    ///
    /// let m = server.mock(|when, then| {
    ///    when.method(POST)
    ///        .path("/example")
    ///        .header("content-type", "application/x-www-form-urlencoded")
    ///        .form_urlencoded_tuple_missing("name")
    ///        .form_urlencoded_tuple_missing("town");
    ///    then.status(202);
    /// });
    ///
    /// let response = Client::new()
    ///    .post(server.url("/example"))
    ///    .header("content-type", "application/x-www-form-urlencoded")
    ///    .body("city=Quahog&occupation=Cartoonist")
    ///    .send()
    ///    .unwrap();
    ///
    /// // Assert
    /// m.assert();
    /// assert_eq!(response.status(), 202);
    /// ```
    ///
    /// # Returns
    /// `When`: Returns the modified `When` object with the new key absence requirement added to the
    /// `application/x-www-form-urlencoded` expectations.
    pub fn form_urlencoded_tuple_missing<IntoString: Into<String>>(self, key: IntoString) -> Self {
        update_cell(&self.expectations, |e| {
            if e.form_urlencoded_tuple_missing.is_none() {
                e.form_urlencoded_tuple_missing = Some(Vec::new());
            }
            e.form_urlencoded_tuple_missing
                .as_mut()
                .unwrap()
                .push(key.into());
        });
        self
    }
    // @docs-group: Body

    /// Sets a requirement that a key's value in an `application/x-www-form-urlencoded` request body must contain a specific substring.
    ///
    /// This method sets an expectation that the value associated with a specific key must contain a specified substring
    /// in the request body of an `application/x-www-form-urlencoded` POST request. The key and the substring are URL-encoded
    /// as specified by the [URL Standard](https://url.spec.whatwg.org/#application/x-www-form-urlencoded).
    ///
    /// **Note**: The mock server does not automatically verify that the HTTP method is POST as per spec.
    /// If you want to verify that the request method is POST, you must explicitly set it in your mock configuration.
    ///
    /// # Parameters
    /// - `key`: The key in the `application/x-www-form-urlencoded` request body.
    /// - `substring`: The substring that must be present in the value associated with the key.
    ///
    /// # Example
    /// ```rust
    /// use httpmock::prelude::*;
    /// use reqwest::blocking::Client;
    ///
    /// // Arrange
    /// let server = MockServer::start();
    ///
    /// let m = server.mock(|when, then| {
    ///    when.method(POST)
    ///        .path("/example")
    ///        .header("content-type", "application/x-www-form-urlencoded")
    ///        .form_urlencoded_tuple_includes("name", "Griffin")
    ///        .form_urlencoded_tuple_includes("town", "Quahog");
    ///    then.status(202);
    /// });
    ///
    /// let response = Client::new()
    ///    .post(server.url("/example"))
    ///    .header("content-type", "application/x-www-form-urlencoded")
    ///    .body("name=Peter%20Griffin&town=Quahog")
    ///    .send()
    ///    .unwrap();
    ///
    /// // Assert
    /// m.assert();
    /// assert_eq!(response.status(), 202);
    /// ```
    ///
    /// # Returns
    /// `When`: Returns the modified `When` object with the new key-value substring requirement added to the
    /// `application/x-www-form-urlencoded` expectations.
    pub fn form_urlencoded_tuple_includes<KeyString: Into<String>, ValueString: Into<String>>(
        self,
        key: KeyString,
        substring: ValueString,
    ) -> Self {
        update_cell(&self.expectations, |e| {
            if e.form_urlencoded_tuple_includes.is_none() {
                e.form_urlencoded_tuple_includes = Some(Vec::new());
            }
            e.form_urlencoded_tuple_includes
                .as_mut()
                .unwrap()
                .push((key.into(), substring.into()));
        });
        self
    }
    // @docs-group: Body

    /// Sets a requirement that a key's value in an `application/x-www-form-urlencoded` request body must not contain a specific substring.
    ///
    /// This method sets an expectation that the value associated with a specific key must not contain a specified substring
    /// in the request body of an `application/x-www-form-urlencoded` POST request. The key and the substring are URL-encoded
    /// as specified by the [URL Standard](https://url.spec.whatwg.org/#application/x-www-form-urlencoded).
    ///
    /// **Note**: The mock server does not automatically verify that the HTTP method is POST as per spec.
    /// If you want to verify that the request method is POST, you must explicitly set it in your mock configuration.
    ///
    /// # Parameters
    /// - `key`: The key in the `application/x-www-form-urlencoded` request body.
    /// - `substring`: The substring that must not be present in the value associated with the key.
    ///
    /// # Example
    /// ```rust
    /// use httpmock::prelude::*;
    /// use reqwest::blocking::Client;
    ///
    /// // Arrange
    /// let server = MockServer::start();
    ///
    /// let m = server.mock(|when, then| {
    ///    when.method(POST)
    ///        .path("/example")
    ///        .header("content-type", "application/x-www-form-urlencoded")
    ///        .form_urlencoded_tuple_excludes("name", "Griffin");
    ///    then.status(202);
    /// });
    ///
    /// let response = Client::new()
    ///    .post(server.url("/example"))
    ///    .header("content-type", "application/x-www-form-urlencoded")
    ///    .body("name=Lois%20Smith&city=Quahog")
    ///    .send()
    ///    .unwrap();
    ///
    /// // Assert
    /// m.assert();
    /// assert_eq!(response.status(), 202);
    /// ```
    ///
    /// # Returns
    /// `When`: Returns the modified `When` object with the new key-value substring exclusion requirement added to the
    /// `application/x-www-form-urlencoded` expectations.
    pub fn form_urlencoded_tuple_excludes<KeyString: Into<String>, ValueString: Into<String>>(
        self,
        key: KeyString,
        substring: ValueString,
    ) -> Self {
        update_cell(&self.expectations, |e| {
            if e.form_urlencoded_tuple_excludes.is_none() {
                e.form_urlencoded_tuple_excludes = Some(Vec::new());
            }
            e.form_urlencoded_tuple_excludes
                .as_mut()
                .unwrap()
                .push((key.into(), substring.into()));
        });
        self
    }
    // @docs-group: Body

    /// Sets a requirement that a key's value in an `application/x-www-form-urlencoded` request body must start with a specific prefix.
    ///
    /// This method sets an expectation that the value associated with a specific key must start with a specified prefix
    /// in the request body of an `application/x-www-form-urlencoded` POST request. The key and the prefix are URL-encoded
    /// as specified by the [URL Standard](https://url.spec.whatwg.org/#application/x-www-form-urlencoded).
    ///
    /// **Note**: The mock server does not automatically verify that the HTTP method is POST as per spec.
    /// If you want to verify that the request method is POST, you must explicitly set it in your mock configuration.
    ///
    /// # Parameters
    /// - `key`: The key in the `application/x-www-form-urlencoded` request body.
    /// - `prefix`: The prefix that must appear at the start of the value associated with the key.
    ///
    /// # Example
    /// ```rust
    /// use httpmock::prelude::*;
    /// use reqwest::blocking::Client;
    ///
    /// // Arrange
    /// let server = MockServer::start();
    ///
    /// let m = server.mock(|when, then| {
    ///    when.method(POST)
    ///        .path("/example")
    ///        .header("content-type", "application/x-www-form-urlencoded")
    ///        .form_urlencoded_tuple_prefix("name", "Pete")
    ///        .form_urlencoded_tuple_prefix("town", "Qua");
    ///    then.status(202);
    /// });
    ///
    /// let response = Client::new()
    ///    .post(server.url("/example"))
    ///    .header("content-type", "application/x-www-form-urlencoded")
    ///    .body("name=Peter%20Griffin&town=Quahog")
    ///    .send()
    ///    .unwrap();
    ///
    /// // Assert
    /// m.assert();
    /// assert_eq!(response.status(), 202);
    /// ```
    ///
    /// # Returns
    /// `When`: Returns the modified `When` object with the new key-value prefix requirement added to the
    /// `application/x-www-form-urlencoded` expectations.
    pub fn form_urlencoded_tuple_prefix<KeyString: Into<String>, ValueString: Into<String>>(
        self,
        key: KeyString,
        prefix: ValueString,
    ) -> Self {
        update_cell(&self.expectations, |e| {
            if e.form_urlencoded_tuple_prefix.is_none() {
                e.form_urlencoded_tuple_prefix = Some(Vec::new());
            }
            e.form_urlencoded_tuple_prefix
                .as_mut()
                .unwrap()
                .push((key.into(), prefix.into()));
        });
        self
    }
    // @docs-group: Body

    /// Sets a requirement that a key's value in an `application/x-www-form-urlencoded` request body must not start with a specific prefix.
    ///
    /// This method sets an expectation that the value associated with a specific key must not start with a specified prefix
    /// in the request body of an `application/x-www-form-urlencoded` POST request. The key and the prefix are URL-encoded
    /// as specified by the [URL Standard](https://url.spec.whatwg.org/#application/x-www-form-urlencoded).
    ///
    /// **Note**: The mock server does not automatically verify that the HTTP method is POST as per spec.
    /// If you want to verify that the request method is POST, you must explicitly set it in your mock configuration.
    ///
    /// # Parameters
    /// - `key`: The key in the `application/x-www-form-urlencoded` request body.
    /// - `prefix`: The prefix that must not appear at the start of the value associated with the key.
    ///
    /// # Example
    /// ```rust
    /// use httpmock::prelude::*;
    /// use reqwest::blocking::Client;
    ///
    /// // Arrange
    /// let server = MockServer::start();
    ///
    /// let m = server.mock(|when, then| {
    ///    when.method(POST)
    ///        .path("/example")
    ///        .header("content-type", "application/x-www-form-urlencoded")
    ///        .form_urlencoded_tuple_prefix_not("name", "Lois")
    ///        .form_urlencoded_tuple_prefix_not("town", "Hog");
    ///    then.status(202);
    /// });
    ///
    /// let response = Client::new()
    ///    .post(server.url("/example"))
    ///    .header("content-type", "application/x-www-form-urlencoded")
    ///    .body("name=Peter%20Griffin&town=Quahog")
    ///    .send()
    ///    .unwrap();
    ///
    /// // Assert
    /// m.assert();
    /// assert_eq!(response.status(), 202);
    /// ```
    ///
    /// # Returns
    /// `When`: Returns the modified `When` object with the new key-value prefix exclusion requirement added to the
    /// `application/x-www-form-urlencoded` expectations.
    pub fn form_urlencoded_tuple_prefix_not<KeyString: Into<String>, ValueString: Into<String>>(
        self,
        key: KeyString,
        prefix: ValueString,
    ) -> Self {
        update_cell(&self.expectations, |e| {
            if e.form_urlencoded_tuple_prefix_not.is_none() {
                e.form_urlencoded_tuple_prefix_not = Some(Vec::new());
            }
            e.form_urlencoded_tuple_prefix_not
                .as_mut()
                .unwrap()
                .push((key.into(), prefix.into()));
        });
        self
    }
    // @docs-group: Body

    /// Sets a requirement that a key's value in an `application/x-www-form-urlencoded` request body must end with a specific suffix.
    ///
    /// This method sets an expectation that the value associated with a specific key must end with a specified suffix
    /// in the request body of an `application/x-www-form-urlencoded` POST request. The key and the suffix are URL-encoded
    /// as specified by the [URL Standard](https://url.spec.whatwg.org/#application/x-www-form-urlencoded).
    ///
    /// **Note**: The mock server does not automatically verify that the HTTP method is POST as per spec.
    /// If you want to verify that the request method is POST, you must explicitly set it in your mock configuration.
    ///
    /// # Parameters
    /// - `key`: The key in the `application/x-www-form-urlencoded` request body.
    /// - `suffix`: The suffix that must appear at the end of the value associated with the key.
    ///
    /// # Example
    /// ```rust
    /// use httpmock::prelude::*;
    /// use reqwest::blocking::Client;
    ///
    /// // Arrange
    /// let server = MockServer::start();
    ///
    /// let m = server.mock(|when, then| {
    ///    when.method(POST)
    ///        .path("/example")
    ///        .header("content-type", "application/x-www-form-urlencoded")
    ///        .form_urlencoded_tuple_suffix("name", "Griffin")
    ///        .form_urlencoded_tuple_suffix("town", "hog");
    ///    then.status(202);
    /// });
    ///
    /// let response = Client::new()
    ///    .post(server.url("/example"))
    ///    .header("content-type", "application/x-www-form-urlencoded")
    ///    .body("name=Peter%20Griffin&town=Quahog")
    ///    .send()
    ///    .unwrap();
    ///
    /// // Assert
    /// m.assert();
    /// assert_eq!(response.status(), 202);
    /// ```
    ///
    /// # Returns
    /// `When`: Returns the modified `When` object with the new key-value suffix requirement added to the
    /// `application/x-www-form-urlencoded` expectations.
    pub fn form_urlencoded_tuple_suffix<KeyString: Into<String>, ValueString: Into<String>>(
        self,
        key: KeyString,
        suffix: ValueString,
    ) -> Self {
        update_cell(&self.expectations, |e| {
            if e.form_urlencoded_tuple_suffix.is_none() {
                e.form_urlencoded_tuple_suffix = Some(Vec::new());
            }
            e.form_urlencoded_tuple_suffix
                .as_mut()
                .unwrap()
                .push((key.into(), suffix.into()));
        });
        self
    }
    // @docs-group: Body

    /// Sets a requirement that a key's value in an `application/x-www-form-urlencoded` request body must not end with a specific suffix.
    ///
    /// This method sets an expectation that the value associated with a specific key must not end with a specified suffix
    /// in the request body of an `application/x-www-form-urlencoded` POST request. The key and the suffix are URL-encoded
    /// as specified by the [URL Standard](https://url.spec.whatwg.org/#application/x-www-form-urlencoded).
    ///
    /// **Note**: The mock server does not automatically verify that the HTTP method is POST as per spec.
    /// If you want to verify that the request method is POST, you must explicitly set it in your mock configuration.
    ///
    /// # Parameters
    /// - `key`: The key in the `application/x-www-form-urlencoded` request body.
    /// - `suffix`: The suffix that must not appear at the end of the value associated with the key.
    ///
    /// # Example
    /// ```rust
    /// use httpmock::prelude::*;
    /// use reqwest::blocking::Client;
    ///
    /// // Arrange
    /// let server = MockServer::start();
    ///
    /// let m = server.mock(|when, then| {
    ///    when.method(POST)
    ///        .path("/example")
    ///        .header("content-type", "application/x-www-form-urlencoded")
    ///        .form_urlencoded_tuple_suffix_not("name", "Smith")
    ///        .form_urlencoded_tuple_suffix_not("town", "ville");
    ///    then.status(202);
    /// });
    ///
    /// let response = Client::new()
    ///    .post(server.url("/example"))
    ///    .header("content-type", "application/x-www-form-urlencoded")
    ///    .body("name=Peter%20Griffin&town=Quahog")
    ///    .send()
    ///    .unwrap();
    ///
    /// // Assert
    /// m.assert();
    /// assert_eq!(response.status(), 202);
    /// ```
    ///
    /// # Returns
    /// `When`: Returns the modified `When` object with the new key-value suffix exclusion requirement added to the
    /// `application/x-www-form-urlencoded` expectations.
    pub fn form_urlencoded_tuple_suffix_not<KeyString: Into<String>, ValueString: Into<String>>(
        self,
        key: KeyString,
        suffix: ValueString,
    ) -> Self {
        update_cell(&self.expectations, |e| {
            if e.form_urlencoded_tuple_suffix_not.is_none() {
                e.form_urlencoded_tuple_suffix_not = Some(Vec::new());
            }
            e.form_urlencoded_tuple_suffix_not
                .as_mut()
                .unwrap()
                .push((key.into(), suffix.into()));
        });
        self
    }
    // @docs-group: Body

    /// Sets a requirement that a key-value pair in an `application/x-www-form-urlencoded` request body must match specific regular expressions.
    ///
    /// This method sets an expectation that the key and the value in a key-value pair must match the specified regular expressions
    /// in the request body of an `application/x-www-form-urlencoded` POST request. The key and value regular expressions are URL-encoded
    /// as specified by the [URL Standard](https://url.spec.whatwg.org/#application/x-www-form-urlencoded).
    ///
    /// **Note**: The mock server does not automatically verify that the HTTP method is POST as per spec.
    /// If you want to verify that the request method is POST, you must explicitly set it in your mock configuration.
    ///
    /// # Parameters
    /// - `key_regex`: The regular expression that the key must match in the `application/x-www-form-urlencoded` request body.
    /// - `value_regex`: The regular expression that the value must match in the `application/x-www-form-urlencoded` request body.
    ///
    /// # Example
    /// ```rust
    /// use httpmock::prelude::*;
    /// use reqwest::blocking::Client;
    /// use regex::Regex;
    ///
    /// // Arrange
    /// let server = MockServer::start();
    ///
    /// let key_regex = Regex::new(r"^name$").unwrap();
    /// let value_regex = Regex::new(r"^Peter\sGriffin$").unwrap();
    ///
    /// let m = server.mock(|when, then| {
    ///    when.method(POST)
    ///        .path("/example")
    ///        .header("content-type", "application/x-www-form-urlencoded")
    ///        .form_urlencoded_tuple_matches(key_regex, value_regex);
    ///    then.status(202);
    /// });
    ///
    /// let response = Client::new()
    ///    .post(server.url("/example"))
    ///    .header("content-type", "application/x-www-form-urlencoded")
    ///    .body("name=Peter%20Griffin&town=Quahog")
    ///    .send()
    ///    .unwrap();
    ///
    /// // Assert
    /// m.assert();
    /// assert_eq!(response.status(), 202);
    /// ```
    ///
    /// # Returns
    /// `When`: Returns the modified `When` object with the new key-value regex matching requirement added to the
    /// `application/x-www-form-urlencoded` expectations.
    pub fn form_urlencoded_tuple_matches<KeyRegex: Into<Regex>, ValueRegex: Into<Regex>>(
        self,
        key_regex: KeyRegex,
        value_regex: ValueRegex,
    ) -> Self {
        update_cell(&self.expectations, |e| {
            if e.form_urlencoded_tuple_matches.is_none() {
                e.form_urlencoded_tuple_matches = Some(Vec::new());
            }
            e.form_urlencoded_tuple_matches
                .as_mut()
                .unwrap()
                .push((key_regex.into(), value_regex.into()));
        });
        self
    }
    // @docs-group: Body

    /// Sets a requirement for the number of times a key-value pair matching specific regular expressions appears in an `application/x-www-form-urlencoded` request body.
    ///
    /// This method sets an expectation that the key-value pair must appear a specific number of times in the request body of an
    /// `application/x-www-form-urlencoded` POST request. The key and value regular expressions are URL-encoded as specified by the
    /// [URL Standard](https://url.spec.whatwg.org/#application/x-www-form-urlencoded).
    ///
    /// **Note**: The mock server does not automatically verify that the HTTP method is POST as per spec.
    /// If you want to verify that the request method is POST, you must explicitly set it in your mock configuration.
    ///
    /// # Parameters
    /// - `key_regex`: The regular expression that the key must match in the `application/x-www-form-urlencoded` request body.
    /// - `value_regex`: The regular expression that the value must match in the `application/x-www-form-urlencoded` request body.
    /// - `count`: The number of times the key-value pair matching the regular expressions must appear.
    ///
    /// # Example
    /// ```rust
    /// use httpmock::prelude::*;
    /// use reqwest::blocking::Client;
    /// use regex::Regex;
    ///
    /// // Arrange
    /// let server = MockServer::start();
    ///
    /// let m = server.mock(|when, then| {
    ///    when.method(POST)
    ///        .path("/example")
    ///        .header("content-type", "application/x-www-form-urlencoded")
    ///        .form_urlencoded_tuple_count(
    ///            Regex::new(r"^name$").unwrap(),
    ///            Regex::new(r".*Griffin$").unwrap(),
    ///            2
    ///        );
    ///    then.status(202);
    /// });
    ///
    /// // Act
    /// let response = Client::new()
    ///    .post(server.url("/example"))
    ///    .header("content-type", "application/x-www-form-urlencoded")
    ///    .body("name=Peter%20Griffin&name=Lois%20Griffin&town=Quahog")
    ///    .send()
    ///    .unwrap();
    ///
    /// // Assert
    /// m.assert();
    /// assert_eq!(response.status(), 202);
    /// ```
    ///
    /// # Returns
    /// `When`: Returns the modified `When` object with the new key-value count requirement added to the
    /// `application/x-www-form-urlencoded` expectations.
    pub fn form_urlencoded_tuple_count<KeyRegex: Into<Regex>, ValueRegex: Into<Regex>>(
        self,
        key_regex: KeyRegex,
        value_regex: ValueRegex,
        count: usize,
    ) -> Self {
        update_cell(&self.expectations, |e| {
            if e.form_urlencoded_tuple_count.is_none() {
                e.form_urlencoded_tuple_count = Some(Vec::new());
            }
            e.form_urlencoded_tuple_count.as_mut().unwrap().push((
                key_regex.into(),
                value_regex.into(),
                count,
            ));
        });
        self
    }
    // @docs-group: Body

    /// Adds a custom matcher for expected HTTP requests. If this function returns true, the request
    /// is considered a match, and the mock server will respond to the request
    /// (given all other criteria are also met).
    ///
    /// You can use this function to create custom expectations for your mock server based on any aspect
    /// of the `HttpMockRequest` object.
    ///
    /// # Parameters
    /// - `matcher`: A function that takes a reference to an `HttpMockRequest` and returns a boolean indicating whether the request matches.
    ///
    /// ## Example
    /// ```rust
    /// use httpmock::prelude::*;
    ///
    /// // Arrange
    /// let server = MockServer::start();
    ///
    /// let m = server.mock(|when, then| {
    ///    when.matches(|req: &HttpMockRequest| {
    ///         req.uri().path().contains("es")
    ///    });
    ///    then.status(200);
    /// });
    ///
    /// // Act: Send the HTTP request
    /// let response = reqwest::blocking::get(server.url("/test")).unwrap();
    ///
    /// // Assert
    /// m.assert();
    /// assert_eq!(response.status(), 200);
    /// ```
    ///
    /// # Returns
    /// `When`: Returns the modified `When` object with the new custom matcher added to the expectations.
    #[deprecated(
        since = "0.8.0",
        note = "Please use the `is_true` and `is_false` function instead"
    )]
    pub fn matches(
        self,
        matcher: impl Fn(&HttpMockRequest) -> bool + Sync + Send + 'static,
    ) -> Self {
        self.is_true(matcher)
    }
    // @docs-group: Custom

    /// Adds a custom matcher for expected HTTP requests. If this function returns true, the request
    /// is considered a match, and the mock server will respond to the request
    /// (given all other criteria are also met).
    ///
    /// You can use this function to create custom expectations for your mock server based on any aspect
    /// of the `HttpMockRequest` object.
    ///
    /// # Parameters
    /// - `matcher`: A function that takes a reference to an `HttpMockRequest` and returns a boolean indicating whether the request matches.
    ///
    /// ## Example
    /// ```rust
    /// use httpmock::prelude::*;
    ///
    /// // Arrange
    /// let server = MockServer::start();
    ///
    /// let m = server.mock(|when, then| {
    ///    when.is_true(|req: &HttpMockRequest| {
    ///         req.uri().path().contains("es")
    ///    });
    ///    then.status(200);
    /// });
    ///
    /// // Act: Send the HTTP request
    /// let response = reqwest::blocking::get(server.url("/test")).unwrap();
    ///
    /// // Assert
    /// m.assert();
    /// assert_eq!(response.status(), 200);
    /// ```
    ///
    /// # Returns
    /// `When`: Returns the modified `When` object with the new custom matcher added to the expectations.
    pub fn is_true(
        self,
        matcher: impl Fn(&HttpMockRequest) -> bool + Sync + Send + 'static,
    ) -> Self {
        update_cell(&self.expectations, |e| {
            if e.is_true.is_none() {
                e.is_true = Some(Vec::new());
            }
            e.is_true.as_mut().unwrap().push(Arc::new(matcher));
        });
        self
    }
    // @docs-group: Custom

    /// Adds a custom matcher for expected HTTP requests. If this function returns false, the request
    /// is considered a match, and the mock server will respond to the request
    /// (given all other criteria are also met).
    ///
    /// You can use this function to create custom expectations for your mock server based on any aspect
    /// of the `HttpMockRequest` object.
    ///
    /// # Parameters
    /// - `matcher`: A function that takes a reference to an `HttpMockRequest` and returns a boolean indicating whether the request matches.
    ///
    /// ## Example
    /// ```rust
    /// use httpmock::prelude::*;
    ///
    /// // Arrange
    /// let server = MockServer::start();
    ///
    /// let m = server.mock(|when, then| {
    ///    when.is_false(|req: &HttpMockRequest| {
    ///         req.uri().path().contains("hello")
    ///    });
    ///    then.status(200);
    /// });
    ///
    /// // Act: Send the HTTP request
    /// let response = reqwest::blocking::get(server.url("/test")).unwrap();
    ///
    /// // Assert
    /// m.assert();
    /// assert_eq!(response.status(), 200);
    /// ```
    ///
    /// # Returns
    /// `When`: Returns the modified `When` object with the new custom matcher added to the expectations.
    pub fn is_false(
        self,
        matcher: impl Fn(&HttpMockRequest) -> bool + Sync + Send + 'static,
    ) -> Self {
        update_cell(&self.expectations, |e| {
            if e.is_false.is_none() {
                e.is_false = Some(Vec::new());
            }
            e.is_false.as_mut().unwrap().push(Arc::new(matcher));
        });
        self
    }
    // @docs-group: Custom

    /// Applies a specified function to enhance or modify the `When` instance. This method allows for the
    /// encapsulation of multiple matching conditions into a single function, maintaining a clear and fluent
    /// interface for setting up HTTP request expectations.
    ///
    /// This method is particularly useful for reusing common setup patterns across multiple test scenarios,
    /// promoting cleaner and more maintainable test code.
    ///
    /// # Parameters
    /// - `func`: A function that takes a `When` instance and returns it after applying some conditions.
    ///
    /// ## Example
    /// ```rust
    /// use httpmock::{prelude::*, When};
    /// use httpmock::Method::POST;
    ///
    /// // Function to apply a standard authorization and content type setup for JSON POST requests
    /// fn is_authorized_json_post_request(when: When) -> When {
    ///     when.method(POST)
    ///         .header("Authorization", "SOME API KEY")
    ///         .header("Content-Type", "application/json")
    /// }
    ///
    /// // Usage example demonstrating how to maintain fluent interface style with complex setups.
    /// // This approach keeps the chain of conditions clear and readable, enhancing test legibility
    /// let server = MockServer::start();
    /// let m = server.mock(|when, then| {
    ///     when.query_param("user_id", "12345")
    ///         .and(is_authorized_json_post_request) // apply the function to include common setup
    ///         .json_body_includes(r#"{"key": "value"}"#); // additional specific condition
    ///     then.status(200);
    /// });
    /// ```
    ///
    /// # Returns
    /// `When`: The modified `When` instance with additional conditions applied, suitable for further chaining.
    pub fn and(self, func: impl FnOnce(When) -> When) -> Self {
        func(self)
    }
    // @docs-group: Miscellaneous
}

/// Represents the configuration of HTTP responses in a mock server environment.
///
/// The `Then` structure is used to define the details of the HTTP response that will be sent if
/// an incoming request meets the conditions specified by a corresponding `When` structure. It
/// allows for detailed customization of response aspects such as status codes, headers, body
/// content, and delays. This structure is integral to defining how the mock server behaves when
/// it receives a request that matches the defined expectations.
pub struct Then {
    pub(crate) response_template: Rc<Cell<MockServerHttpResponse>>,
}

impl Then {
    /// Configures the HTTP response status code that the mock server will return.
    ///
    /// # Parameters
    /// - `status`: A `u16` HTTP status code that the mock server should return for the configured request.
    ///
    /// # Returns
    /// Returns `self` to allow chaining of method calls on the `Mock` object.
    ///
    /// # Example
    /// Demonstrates setting a 200 OK status for a request to the path `/hello`.
    ///
    /// ```rust
    /// use httpmock::prelude::*;
    ///
    /// // Initialize the mock server
    /// let server = MockServer::start();
    ///
    /// // Configure the mock
    /// let m = server.mock(|when, then| {
    ///     when.path("/hello");
    ///     then.status(200);
    /// });
    ///
    /// // Send a request and verify the response
    /// let response = reqwest::blocking::get(server.url("/hello")).unwrap();
    ///
    /// // Check that the mock was called as expected and the response status is as configured
    /// m.assert();
    /// assert_eq!(response.status(), 200);
    /// ```
    pub fn status<U16: TryInto<u16>>(self, status: U16) -> Self
    where
        <U16 as TryInto<u16>>::Error: std::fmt::Debug,
    {
        update_cell(&self.response_template, |r| {
            r.status = Some(
                status
                    .try_into()
                    .expect("cannot parse status code to usize"),
            );
        });
        self
    }
    // @docs-group: Status

    /// Configures the HTTP response body that the mock server will return.
    ///
    /// # Parameters
    /// - `body`: The content of the response body, provided as a type that can be referenced as a byte slice.
    ///
    /// # Returns
    /// Returns `self` to allow chaining of method calls on the `Mock` object.
    ///
    /// # Example
    /// Demonstrates setting a response body for a request to the path `/hello` with a 200 OK status.
    ///
    /// ```rust
    /// use httpmock::prelude::*;
    /// use reqwest::blocking::Client;
    ///
    /// // Initialize the mock server
    /// let server = MockServer::start();
    ///
    /// // Configure the mock
    /// let m = server.mock(|when, then| {
    ///     when.path("/hello");
    ///     then.status(200)
    ///         .body("ohi!");
    /// });
    ///
    /// // Send a request and verify the response
    /// let response = Client::new()
    ///     .get(server.url("/hello"))
    ///     .send()
    ///     .unwrap();
    ///
    /// // Check that the mock was called as expected and the response body is as configured
    /// m.assert();
    /// assert_eq!(response.status(), 200);
    /// assert_eq!(response.text().unwrap(), "ohi!");
    /// ```
    pub fn body<SliceRef: AsRef<[u8]>>(self, body: SliceRef) -> Self {
        update_cell(&self.response_template, |r| {
            r.body = Some(HttpMockBytes::from(Bytes::copy_from_slice(body.as_ref())));
        });
        self
    }
    // @docs-group: Body

    /// Configures the HTTP response body with content loaded from a specified file on the mock server.
    ///
    /// # Parameters
    /// - `resource_file_path`: A string representing the path to the file whose contents will be used as the response body. The path can be absolute or relative to the server's running directory.
    ///
    /// # Returns
    /// Returns `self` to allow chaining of method calls on the `Mock` object.
    ///
    /// # Panics
    /// Panics if the specified file cannot be read, or if the path provided cannot be resolved to an absolute path.
    ///
    /// # Example
    /// Demonstrates setting the response body from a file for a request to the path `/hello` with a 200 OK status.
    ///
    /// ```rust
    /// use httpmock::prelude::*;
    /// use reqwest::blocking::Client;
    ///
    /// // Initialize the mock server
    /// let server = MockServer::start();
    ///
    /// // Configure the mock
    /// let m = server.mock(|when, then| {
    ///     when.path("/hello");
    ///     then.status(200)
    ///         .body_from_file("tests/resources/simple_body.txt");
    /// });
    ///
    /// // Send a request and verify the response
    /// let response = Client::new()
    ///     .get(server.url("/hello"))
    ///     .send()
    ///     .unwrap();
    ///
    /// // Check that the mock was called as expected and the response body matches the file contents
    /// m.assert();
    /// assert_eq!(response.status(), 200);
    /// assert_eq!(response.text().unwrap(), "ohi!");
    /// ```
    pub fn body_from_file<IntoString: Into<String>>(self, resource_file_path: IntoString) -> Self {
        let resource_file_path = resource_file_path.into();
        let path = Path::new(&resource_file_path);
        let absolute_path = match path.is_absolute() {
            true => path.to_path_buf(),
            false => get_test_resource_file_path(&resource_file_path).expect(&format!(
                "Cannot create absolute path from string '{}'",
                &resource_file_path
            )),
        };
        let content = crate::common::util::read_file(&absolute_path).expect(&format!(
            "Cannot read from file {}",
            absolute_path.to_str().expect("Invalid OS path")
        ));
        self.body(content)
    }
    // @docs-group: Body

    /// Sets the JSON body for the HTTP response that will be returned by the mock server.
    ///
    /// This function accepts a JSON object that must be serializable and deserializable by serde.
    /// Note that this method does not automatically set the "Content-Type" header to "application/json".
    /// You will need to set this header manually if required.
    ///
    /// # Parameters
    /// - `body`: The HTTP response body in the form of a `serde_json::Value` object.
    ///
    /// # Returns
    /// Returns `self` to allow chaining of method calls on the `Mock` object.
    ///
    /// # Example
    /// Demonstrates how to set a JSON body and a matching "Content-Type" header for a mock response.
    ///
    /// ```rust
    /// use httpmock::prelude::*;
    /// use serde_json::{Value, json};
    /// use reqwest::blocking::Client;
    ///
    /// // Arrange
    /// tracing_subscriber::fmt::init();
    /// let server = MockServer::start();
    ///
    /// // Configure the mock
    /// let m = server.mock(|when, then| {
    ///     when.path("/user");
    ///     then.status(200)
    ///         .header("content-type", "application/json")
    ///         .json_body(json!({ "name": "Hans" }));
    /// });
    ///
    /// // Act
    /// let response = Client::new()
    ///     .get(server.url("/user"))
    ///     .send()
    ///     .unwrap();
    ///
    /// // Get the status code first
    /// let status = response.status();
    ///
    /// // Extract the text from the response
    /// let response_text = response.text().unwrap();
    ///
    /// // Deserialize the JSON response
    /// let user: Value =
    ///     serde_json::from_str(&response_text).expect("cannot deserialize JSON");
    ///
    /// // Assert
    /// m.assert();
    /// assert_eq!(status, 200);
    /// assert_eq!(user["name"], "Hans");
    /// ```
    pub fn json_body<V: Into<Value>>(self, body: V) -> Self {
        update_cell(&self.response_template, |r| {
            r.body = Some(HttpMockBytes::from(Bytes::from(body.into().to_string())));
        });
        self
    }
    // @docs-group: Body

    /// Sets the JSON body that will be returned by the mock server using a serializable serde object.
    ///
    /// This method converts the provided object into a JSON string. It does not automatically set
    /// the "Content-Type" header to "application/json", so you must set this header manually if it's
    /// needed.
    ///
    /// # Parameters
    /// - `body`: A reference to an object that implements the `serde::Serialize` trait.
    ///
    /// # Returns
    /// Returns `self` to allow chaining of method calls on the `Mock` object.
    ///
    /// # Panics
    /// Panics if the object cannot be serialized into a JSON string.
    ///
    /// # Example
    /// Demonstrates setting a JSON body and the corresponding "Content-Type" header for a user object.
    ///
    /// ```rust
    /// use httpmock::prelude::*;
    /// use reqwest::blocking::Client;
    /// use serde::{Serialize, Deserialize};
    ///
    /// #[derive(Serialize, Deserialize)]
    /// struct TestUser {
    ///     name: String,
    /// }
    ///
    /// // Arrange
    /// tracing_subscriber::fmt::init();
    /// let server = MockServer::start();
    ///
    /// // Configure the mock
    /// let m = server.mock(|when, then| {
    ///     when.path("/user");
    ///     then.status(200)
    ///         .header("content-type", "application/json")
    ///         .json_body_obj(&TestUser {
    ///             name: String::from("Hans"),
    ///         });
    /// });
    ///
    /// // Act
    /// let response = Client::new()
    ///     .get(server.url("/user"))
    ///     .send()
    ///     .unwrap();
    ///
    /// // Get the status code first
    /// let status = response.status();
    ///
    /// // Extract the text from the response
    /// let response_text = response.text().unwrap();
    ///
    /// // Deserialize the JSON response into a TestUser object
    /// let user: TestUser =
    ///     serde_json::from_str(&response_text).unwrap();
    ///
    /// // Assert
    /// m.assert();
    /// assert_eq!(status, 200);
    /// assert_eq!(user.name, "Hans");
    /// ```
    pub fn json_body_obj<T>(self, body: &T) -> Self
    where
        T: Serialize + ?Sized,
    {
        let json_body =
            serde_json::to_value(body).expect("Failed to serialize object to JSON string");
        self.json_body(json_body)
    }
    // @docs-group: Body

    /// Sets an HTTP header that the mock server will return in the response.
    ///
    /// This method configures a response header to be included when the mock server handles a request.
    ///
    /// # Parameters
    /// - `name`: The name of the header to set.
    /// - `value`: The value of the header.
    ///
    /// # Returns
    /// Returns `self` to allow chaining of method calls on the `Mock` object.
    ///
    /// # Example
    /// Demonstrates setting the "Expires" header for a response to a request to the root path.
    ///
    /// ```rust
    /// use httpmock::prelude::*;
    /// use reqwest::blocking::Client;
    ///
    /// // Arrange
    /// tracing_subscriber::fmt::init();
    /// let server = MockServer::start();
    ///
    /// // Configure the mock
    /// let m = server.mock(|when, then| {
    ///     when.path("/");
    ///     then.status(200)
    ///         .header("Expires", "Wed, 21 Oct 2050 07:28:00 GMT");
    /// });
    ///
    /// // Act
    /// let response = Client::new()
    ///     .get(server.url("/"))
    ///     .send()
    ///     .unwrap();
    ///
    /// // Assert
    /// m.assert();
    /// assert_eq!(response.status(), 200);
    /// assert_eq!(
    ///     response.headers().get("Expires").unwrap().to_str().unwrap(),
    ///     "Wed, 21 Oct 2050 07:28:00 GMT"
    /// );
    /// ```
    pub fn header<KeyString: Into<String>, ValueString: Into<String>>(
        self,
        name: KeyString,
        value: ValueString,
    ) -> Self {
        update_cell(&self.response_template, |r| {
            if r.headers.is_none() {
                r.headers = Some(Vec::new());
            }
            r.headers
                .as_mut()
                .unwrap()
                .push((name.into(), value.into()));
        });
        self
    }
    // @docs-group: Headers

    /// Sets a delay for the mock server response.
    ///
    /// This method configures the server to wait for a specified duration before sending a response,
    /// which can be useful for testing timeout scenarios or asynchronous operations.
    ///
    /// # Parameters
    /// - `duration`: The length of the delay as a `std::time::Duration`.
    ///
    /// # Returns
    /// Returns `self` to allow chaining of method calls on the `Mock` object.
    ///
    /// # Panics
    /// Panics if the specified duration results in a delay that cannot be represented as a 64-bit
    /// unsigned integer of milliseconds (more than approximately 584 million years).
    ///
    /// # Example
    /// Demonstrates setting a 3-second delay for a request to the path `/delay`.
    ///
    /// ```rust
    /// use std::time::{SystemTime, Duration};
    /// use httpmock::prelude::*;
    /// use reqwest::blocking::Client;
    ///
    /// // Arrange
    /// tracing_subscriber::fmt::init();
    /// let start_time = SystemTime::now();
    /// let three_seconds = Duration::from_secs(3);
    /// let server = MockServer::start();
    ///
    /// // Configure the mock
    /// let mock = server.mock(|when, then| {
    ///     when.path("/delay");
    ///     then.status(200)
    ///         .delay(three_seconds);
    /// });
    ///
    /// // Act
    /// let response = Client::new()
    ///     .get(server.url("/delay"))
    ///     .send()
    ///     .unwrap();
    ///
    /// // Assert
    /// mock.assert();
    /// assert!(start_time.elapsed().unwrap() >= three_seconds);
    /// ```
    pub fn delay<D: Into<Duration>>(self, duration: D) -> Self {
        let duration = duration.into();

        // Ensure the delay duration does not exceed the maximum u64 milliseconds limit
        let millis = duration.as_millis();
        let max = u64::MAX as u128;
        if millis >= max {
            panic!("A delay higher than {} milliseconds is not supported.", max)
        }

        update_cell(&self.response_template, |r| {
            r.delay = Some(duration.as_millis() as u64);
        });
        self
    }
    // @docs-group: Network

    /// Applies a custom function to modify a `Then` instance, enhancing flexibility and readability
    /// in setting up mock server responses.
    ///
    /// This method allows you to encapsulate complex configurations into reusable functions,
    /// and apply them without breaking the chain of method calls on a `Then` object.
    ///
    /// # Parameters
    /// - `func`: A function that takes a `Then` instance and returns it after applying some modifications.
    ///
    /// # Returns
    /// Returns `self` to allow chaining of method calls on the `Then` object.
    ///
    /// # Example
    /// Demonstrates how to use the `and` method to maintain readability while applying multiple
    /// modifications from an external function.
    ///
    /// ```rust
    /// use std::time::Duration;
    /// use http::{StatusCode, header::HeaderValue};
    /// use httpmock::{Then, MockServer};
    ///
    /// // Function that configures a response with JSON content and a delay
    /// fn ok_json_with_delay(then: Then) -> Then {
    ///     then.status(StatusCode::OK.as_u16())
    ///         .header("content-type", "application/json")
    ///         .delay(Duration::from_secs_f32(0.5))
    /// }
    ///
    /// // Usage within a method chain
    /// let server = MockServer::start();
    /// let then = server.mock(|when, then| {
    ///     when.path("/example");
    ///     then.header("general-vibe", "much better")
    ///         .and(ok_json_with_delay);
    /// });
    ///
    /// // The `and` method keeps the setup intuitively readable as a continuous chain
    /// ```
    pub fn and(self, func: impl FnOnce(Then) -> Then) -> Self {
        func(self)
    }
    // @docs-group: Miscellaneous

    /// Sets a **dynamic responder** that is invoked for every request matching the `when`
    /// conditions. The provided closure receives the full `HttpMockRequest` and must
    /// **fully determine** the `HttpMockResponse` (status, headers, and body).
    ///
    /// This is ideal when the reply depends on request details (path, headers, body),
    /// or when you need stateful behavior across calls (e.g., counters, cycling codes).
    ///
    /// > **Important:** Dynamic responders are only supported by the local server.
    /// > They are **not** supported by remote/standalone servers and this method **will panic**
    /// > when used against them.
    ///
    /// # Parameters
    /// - `f`: A response generator closure that is executed on the mock server’s request
    ///   handling thread for each match.
    ///
    /// # Behavior
    /// - When set, this responder **overrides** any static configuration on `Then`
    ///   (e.g., previously set status/body/headers) for matching requests other than the
    ///   configured delay using the `delay` method.
    /// - The closure may capture shared state. If you mutate shared data, use a
    ///   synchronization primitive (`Mutex`, `RwLock`, or atomics). This is required
    ///   because the responder runs on the mock server’s request-handling thread,
    ///   which is distinct from the test thread.
    ///
    /// # Returns
    /// Returns `self` for continued method chaining on `Then`.
    ///
    /// # Example
    /// A minimal dynamic responder that mirrors the request body value back to the client:
    ///
    /// ```rust
    /// use httpmock::{MockServer, HttpMockRequest, HttpMockResponse};
    /// use reqwest::blocking::Client;
    ///
    /// let server = MockServer::start();
    ///
    /// let mock = server.mock(|when, then| {
    ///     when.path("/echo");
    ///     then.respond_with(|req: &HttpMockRequest| {
    ///         // Echo the received request body back in the response body.
    ///         let echoed_body = req.body().to_string();
    ///
    ///         HttpMockResponse::builder()
    ///             .status(200)
    ///             .body(echoed_body)
    ///             .build()
    ///     });
    /// });
    ///
    /// let res = Client::new()
    ///     .post(format!("{}/echo", server.base_url()))
    ///     .body("Hello, world!")
    ///     .send()
    ///     .unwrap();
    ///
    /// mock.assert();
    ///
    /// assert_eq!(res.status(), 200);
    /// assert_eq!(res.text().unwrap(), "Hello, world!");
    /// ```
    ///
    /// # Example
    /// Stateful dynamic responder that increments the status code on each call:
    ///
    /// ```rust
    /// use httpmock::{MockServer, HttpMockRequest, HttpMockResponse};
    /// use reqwest::blocking::Client;
    /// use std::sync::Mutex;
    ///
    /// // Arrange
    /// let server = MockServer::start();
    ///
    /// // Shared counter used inside the responder closure.
    /// // Use a Mutex/RwLock/atomic because the closure runs on the server thread.
    /// let call_count = Mutex::new(0);
    ///
    /// let mock = server.mock(|when, then| {
    ///     when.path("/hello");
    ///     then.respond_with(move |_req: &HttpMockRequest| {
    ///         let mut count = call_count.lock().unwrap();
    ///         *count += 1;
    ///
    ///         HttpMockResponse::builder()
    ///             .status(200 + *count) // 201, 202, 203, ...
    ///             .build()
    ///     });
    /// });
    ///
    /// // Act
    /// let client = Client::new();
    /// let response1 = client.get(format!("{}/hello", server.base_url())).send().unwrap();
    /// let response2 = client.get(format!("{}/hello", server.base_url())).send().unwrap();
    /// let response3 = client.get(format!("{}/hello", server.base_url())).send().unwrap();
    ///
    /// // Assert
    /// mock.assert_calls(3);
    /// assert_eq!(response1.status(), 201);
    /// assert_eq!(response2.status(), 202);
    /// assert_eq!(response3.status(), 203);
    /// ```
    ///
    /// # HTTP Crate Integration
    /// Both `HttpMockRequest` and `HttpMockResponse` implement conversion traits
    /// to and from types of the [`http`](https://crates.io/crates/http) crate.
    ///
    /// This means you can seamlessly integrate with other HTTP utilities or middleware
    /// that operate on `http::Request` and `http::Response`.
    ///
    /// ```rust
    /// use httpmock::{MockServer, HttpMockRequest, HttpMockResponse};
    /// use reqwest::blocking::Client;
    ///
    /// let server = MockServer::start();
    ///
    /// let mock = server.mock(|when, then| {
    ///     when.method("POST").path("/echo");
    ///     then.respond_with(|req: &HttpMockRequest| {
    ///         // Convert the HttpMockRequest to an `http` crate Request (with body)
    ///         let http_req: http::Request<String> = req.into();
    ///
    ///         // Build an `http` crate Response (auto-converted to HttpMockResponse)
    ///         http::Response::builder()
    ///             .status(200)
    ///             .header("content-type", "text/plain")
    ///             .body(format!("Echo from {}: {}", http_req.uri().path(), http_req.body()))
    ///             .unwrap()
    ///             .into()
    ///     });
    /// });
    ///
    /// let res = Client::new()
    ///     .post(format!("{}/echo", server.base_url()))
    ///     .body("Hello from client!")
    ///     .send()
    ///     .unwrap();
    ///
    /// mock.assert();
    /// assert_eq!(res.status(), 200);
    /// assert_eq!(res.text().unwrap(), "Echo from /echo: Hello from client!");
    /// ```
    ///
    /// # Notes
    /// - Avoid long-running or blocking work inside the responder; it runs on the request
    ///   handling path and will delay responses.
    /// - If you need to combine static defaults with dynamic tweaks, compute them inside
    ///   the closure (e.g., start from `HttpMockResponse::builder()` and adjust as needed).
    pub fn respond_with<F>(self, f: F) -> Self
    where
        F: Fn(&HttpMockRequest) -> HttpMockResponse + Send + Sync + 'static,
    {
        update_cell(&self.response_template, |r| {
            r.respond_with = Some(std::sync::Arc::new(f));
        });
        self
    }
}
