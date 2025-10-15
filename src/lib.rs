#![allow(warnings)]
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
//! * [Advanced verification and debugging support](https://httpmock.github.io/posts/mocking-http--services-in-rust/#creating-mocks) (including diff generation between actual and expected HTTP request values)
//! * Parallel test execution.
//! * Fully asynchronous core with synchronous and asynchronous APIs.
//! * Support for [mock configuration using YAML files](https://github.com/httpmock/httpmock/tree/master#file-based-mock-specification).
//!
//! # Getting Started
//! Add `httpmock` to `Cargo.toml`:
//!
//! ```toml
//! [dev-dependencies]
//! httpmock = "0.8.1"
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
//! Docs:     https://docs.rs/httpmock/0.8.1/httpmock/struct.When.html#method.query_param
//! ```
//!
//! # Usage
//!
//! See the [official website](http://httpmock.rs) a for detailed
//! API documentation.
//!
//! ## Examples
//!
//! You can find examples in the
//! [`httpmock` test directory](https://github.com/httpmock/httpmock/blob/master/tests/).
//! The [official website](http://httpmock.rs) and [reference docs](https://docs.rs/httpmock/)
//! also contain _**a lot**_ of examples.
//!
//! ## License
//!
//! `httpmock` is free software: you can redistribute it and/or modify it under the terms of the MIT Public License.
//!
//! This program is distributed in the hope that it will be useful, but WITHOUT ANY WARRANTY; without even the implied
//! warranty of MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the MIT Public License for more details.

use std::{borrow::BorrowMut, net::ToSocketAddrs};

use std::str::FromStr;

use serde::{Deserialize, Serialize};

use api::MockServerAdapter;

mod common;
use common::util::Join;

pub use api::{Method, Mock, MockExt, MockServer, Regex, Then, When};
pub use common::data::{HttpMockRequest, HttpMockResponse};

mod api;
pub mod server;

#[cfg(feature = "record")]
pub use api::{Recording, RecordingRuleBuilder};

#[cfg(feature = "proxy")]
pub use api::{ForwardingRule, ForwardingRuleBuilder, ProxyRule, ProxyRuleBuilder};

pub mod prelude {
    #[doc(no_inline)]
    pub use crate::{
        api::MockServer, common::data::HttpMockRequest, common::data::HttpMockResponse,
        common::data::MockServerHttpResponse, Method, Method::DELETE, Method::GET, Method::OPTIONS,
        Method::PATCH, Method::POST, Method::PUT, Regex,
    };
}
