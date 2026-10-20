//! HTTP mocking library that allows you to simulate responses from HTTP-based services.
//!
//! # Features
//! * Mocks responses from HTTP services
//! * Simple, expressive, fluent API.
//! * Many built-in helpers for easy request matching ([Regex](https://docs.rs/regex/), JSON, [serde](https://crates.io/crates/serde), cookies, and more).
//! * Record and Playback
//! * Forward and Proxy Mode
//! * HTTPS support
//! * Fault and network delay simulation.
//! * Custom request matchers.
//! * Standalone mode with an accompanying [Docker image](https://hub.docker.com/r/httpmock/httpmock).
//! * Helpful error messages
//! * [Advanced verification and debugging support](https://httpmock.rs/server/debugging/) (including diff generation between actual and expected HTTP request values)
//! * Parallel test execution.
//! * Fully asynchronous core with synchronous and asynchronous APIs.
//! * Support for [mock configuration using YAML files](https://httpmock.rs/server/standalone/).
//!
//! # Getting Started
//! Add `httpmock` to `Cargo.toml`:
//!
//! ```toml
//! [dev-dependencies]
//! httpmock = "0.9.0"
//! ```
//!
//! You can then use `httpmock` as follows:
//! ```
//! use httpmock::prelude::*;
//! use reqwest::blocking::get;
//!
//! // Start a lightweight mock server.
//! let server = MockServer::start();
//!
//! // Create a mock on the server.
//! let hello_mock = server.mock(|when, then| {
//!     when.method(GET)
//!         .path("/translate")
//!         .query_param("word", "hello");
//!     then.status(200)
//!         .header("content-type", "text/html")
//!         .body("ohi");
//! });
//!
//! // Send an HTTP request to the mock server. This simulates your code.
//! let response = get(&server.url("/translate?word=hello")).unwrap();
//!
//! // Ensure the specified mock was called exactly one time (or fail with a detailed error description).
//! hello_mock.assert();
//! // Ensure the mock server did respond as specified.
//! assert_eq!(response.status(), 200);
//! ```
//!
//! When the specified expectations do not match the received request, `mock.assert()` fails the test with a detailed error description,
//! including a diff that shows the differences between the expected and actual HTTP requests. Example:
//!
//! ```bash
//! 0 of 1 expected requests matched the mock specification.
//! Here is a comparison with the most similar unmatched request (request number 1):
//!
//! ------------------------------------------------------------
//! 1 : Query Parameter Mismatch
//! ------------------------------------------------------------
//! Expected:
//!     key    [equals]  word
//!     value  [equals]  hello-rustaceans
//!
//! Received (most similar query parameter):
//!     word=hello
//!
//! All received query parameter values:
//!     1. word=hello
//!
//! Matcher:  query_param
//! Docs:     https://docs.rs/httpmock/latest/httpmock/struct.When.html#method.query_param
//! ```
//!
//! # Usage
//!
//! These pages are the API reference: every [`When`] and [`Then`] method documents itself with an
//! example, and [`MockServer`], [`Mock`], [`HttpMockRequest`] and [`HttpMockResponse`] cover the
//! rest. The guide lives on the [official website](https://httpmock.rs), which explains how the
//! pieces fit together:
//!
//! * [Fundamentals](https://httpmock.rs/getting_started/fundamentals/): how mocks, matchers and
//!   the server pool work.
//! * [Standalone server and Docker](https://httpmock.rs/server/standalone/): running the server
//!   outside your tests, including YAML mock files.
//! * [HTTPS](https://httpmock.rs/server/https/): TLS support and certificates.
//! * [Recording](https://httpmock.rs/record-and-playback/recording/) and
//!   [playback](https://httpmock.rs/record-and-playback/playback/): capturing real traffic and
//!   replaying it.
//! * [Debugging](https://httpmock.rs/server/debugging/): reading assertion output and logs.
//! * [FAQ](https://httpmock.rs/miscellaneous/faq/) and
//!   [upgrading](https://httpmock.rs/miscellaneous/upgrading/).
//!
//! ## Examples
//!
//! You can find examples in the
//! [`httpmock` test directory](https://github.com/httpmock/httpmock/blob/master/tests/).
//! The [official website](https://httpmock.rs) and the item docs here also contain a lot of examples.
//!
//! # Cargo features
//!
//! Only `cookies` is enabled by default. Items that need a feature carry a badge on docs.rs.
//!
//! * `cookies` (default): cookie matching on the server side for [`When::cookie`] and its
//!   siblings. The methods exist without it, but the server only parses and matches cookies
//!   with the feature on. Pulls in the `headers` crate.
//! * `color`: colors the added and removed lines in the request diff that failed assertions print.
//!   Pulls in the `colored` crate.
//! * `remote`: connecting to a mock server running in standalone mode with
//!   [`MockServer::connect`] and [`MockServer::connect_from_env`]. Enables the `hyper-util` client.
//! * `remote-https`: the same connection over HTTPS, using `rustls` with the `ring` provider and
//!   `hyper-rustls`. Implies `remote`.
//! * `proxy`: forwarding and proxy rules, see [`MockServer::forward_to`] and
//!   [`MockServer::proxy`]. Implies `remote-https`.
//! * `record`: recording traffic with [`MockServer::record`] and reading mock specifications
//!   from YAML files (`serde_yaml`). Implies `proxy`.
//! * `https`: the mock server terminates TLS on the same port it serves plain HTTP on, generating
//!   certificates with `rcgen`. Pulls in `rustls` with the `ring` provider, `tokio-rustls`,
//!   `hyper-rustls`, `rustls-pki-types` and `if-addrs`.
//! * `http2`: HTTP/2 support for the mock server (`hyper/http2`, `hyper-util/server-auto`) and,
//!   when `hyper-rustls` is present, for the remote client.
//! * `standalone`: builds the `httpmock` binary. Implies `record`, `http2`, `cookies`, `remote`
//!   and `remote-https` and adds `clap`, `tokio/signal` and `tracing-subscriber`. `color` is not
//!   implied, so enable it separately or use `--all-features`.
//!
//! # Environment variables
//!
//! The library reads these variables at runtime; each one falls back to the default when unset.
//! Setting a variable to something that does not parse panics when the value is first used.
//!
//! * `HTTPMOCK_MAX_SERVERS` (default `25`): the size of the pool of local mock servers that
//!   [`MockServer::start`] hands out. A test that asks for a server while all of them are in use
//!   waits until one is returned.
//! * `HTTPMOCK_HOST` (default `127.0.0.1`) and `HTTPMOCK_PORT` (default `5050`): the standalone
//!   server that [`MockServer::connect_from_env`] connects to. Requires the `remote` feature.
//! * `HTTPMOCK_HTTP_CLIENT_WORKER_THREADS` (default `1`) and
//!   `HTTPMOCK_HTTP_CLIENT_MAX_BLOCKING_THREADS` (default `10`): the runtime threads of the HTTP
//!   client that talks to a remote mock server. Requires the `remote` feature.
//! * `HTTPMOCK_EXTRA_SANS`: a comma-separated list of extra DNS names or IP addresses added as
//!   subject alternative names to the certificate the server generates when a client connects
//!   without SNI. Unset by default. Requires the `https` feature.
//!
//! The variables the standalone binary reads (`HTTPMOCK_PORT`, `HTTPMOCK_EXPOSE`,
//! `HTTPMOCK_MOCK_FILES_DIR`, `HTTPMOCK_DISABLE_ACCESS_LOG` and `HTTPMOCK_REQUEST_HISTORY_LIMIT`)
//! are documented on the [standalone server page](https://httpmock.rs/server/standalone/).
//!
//! ## License
//!
//! `httpmock` is free software: you can redistribute it and/or modify it under the terms of the MIT Public License.
//!
//! This program is distributed in the hope that it will be useful, but WITHOUT ANY WARRANTY; without even the implied
//! warranty of MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the MIT Public License for more details.
#![cfg_attr(docsrs, feature(doc_cfg))]

mod common;

pub use api::{Method, Mock, MockServer, Regex, Then, When};
pub use common::data::{HttpMockRequest, HttpMockResponse};
mod api;
pub mod server;

#[cfg(feature = "proxy")]
pub use api::{ForwardingRule, ForwardingRuleBuilder, ProxyRule, ProxyRuleBuilder};
#[cfg(feature = "record")]
pub use api::{Recording, RecordingRuleBuilder};

pub mod prelude {
    #[doc(no_inline)]
    pub use crate::{
        Method, Method::DELETE, Method::GET, Method::OPTIONS, Method::PATCH, Method::POST, Method::PUT, Regex,
        api::MockServer, common::data::HttpMockRequest, common::data::HttpMockResponse,
        common::data::MockServerHttpResponse,
    };
}
