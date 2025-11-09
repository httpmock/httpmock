<div align="center">
    <picture>
      <source media="(prefers-color-scheme: dark)" srcset="https://raw.githubusercontent.com/httpmock/httpmock/master/assets/logo-dark.svg" />
      <source media="(prefers-color-scheme: light)" srcset="https://raw.githubusercontent.com/httpmock/httpmock/master/assets/logo-light.svg" />
      <img width="275" src="https://raw.githubusercontent.com/httpmock/httpmock/master/assets/logo-pink.svg" alt="httpmock" />
    </picture>
</div>

<p align="center">Simple yet powerful HTTP mocking library for Rust</p>

<div align="center">

[![Build](https://github.com/httpmock/httpmock/actions/workflows/build.yml/badge.svg)](https://github.com/httpmock/httpmock/actions/workflows/build.yml)
[![crates.io](https://img.shields.io/crates/d/httpmock.svg)](https://crates.io/crates/httpmock)
[![Mentioned in Awesome](https://raw.githubusercontent.com/alexliesenfeld/docs-assets/refs/heads/main/ab.svg)](https://github.com/rust-unofficial/awesome-rust#testing)
[![Rust](https://img.shields.io/badge/rust-1.83%2B-blue.svg?maxAge=3600)](https://github.com/rust-lang/rust/blob/master/RELEASES.md#version-1830-2024-11-28)

</div>

<p align="center">
    <a href="https://httpmock.rs">Website</a>
    ·
    <a href="https://docs.rs/httpmock/">API Reference</a>
    ·
    <a href="https://crates.io/crates/httpmock">Crate</a>
    ·
    <a href="https://discord.com/invite/7QzTfBUe">Chat on Discord</a>
    ·
 <a href="https://github.com/httpmock/httpmock/discussions">Forum</a>
    ·
    <a href="https://github.com/httpmock/httpmock/issues">Report Bug</a>
    ·
    <a href="https://github.com/httpmock/httpmock/issues">Request Feature</a>
    ·
    <a href="https://github.com/sponsors/httpmock">Support this Project</a>
</p>


## Features

* Mocks responses from HTTP services
* Simple, expressive, fluent API.
* Many built-in helpers for easy request matching ([Regex](https://docs.rs/regex/), JSON, [serde](https://crates.io/crates/serde), cookies, and more).
* Record and Playback third-party services
* Forward and Proxy Mode
* HTTPS support
* Fault and network delay simulation.
* Custom request matchers.
* Standalone mode with an accompanying [Docker image](https://hub.docker.com/r/httpmock/httpmock).
* Helpful error messages
* [Advanced verification and debugging support](https://alexliesenfeld.github.io/posts/mocking-http--services-in-rust/#creating-mocks) (including diff generation between actual and expected HTTP request values)
* Parallel test execution.
* Fully asynchronous core with synchronous and asynchronous APIs.
* Support for [mock configuration using YAML files](https://github.com/httpmock/httpmock/tree/master#file-based-mock-specification).

## Getting Started

Add `httpmock` to `Cargo.toml`:

```toml
[dev-dependencies]
httpmock = "0.8.2"
```

You can then use `httpmock` as follows:

```rust
use httpmock::prelude::*;

// Start a lightweight mock server.
let server = MockServer::start();

// Create a mock on the server.
let mock = server.mock(|when, then| {
    when.method(GET)
        .path("/translate")
        .query_param("word", "hello");
    then.status(200)
        .header("content-type", "text/html; charset=UTF-8")
        .body("hola");
});

// Send an HTTP request to the mock server. This simulates your code.
let response = isahc::get(server.url("/translate?word=hello")).unwrap();

// Ensure the specified mock was called exactly one time (or fail with a
// detailed error description).
mock.assert();

// Ensure the mock server did respond as specified.
assert_eq!(response.status(), 200);
```

The above example will spin up a lightweight HTTP mock server and configure it to respond to all `GET` requests
to path `/translate` with query parameter `word=hello`. The corresponding HTTP response will contain the text body
`hola`.

When the specified expectations do not match the received request, `mock.assert()` fails the test with a detailed error description, 
including a diff that shows the differences between the expected and actual HTTP requests. Example:

```bash
0 of 1 expected requests matched the mock specification.
Here is a comparison with the most similar unmatched request (request number 1):

------------------------------------------------------------
1 : Query Parameter Mismatch
------------------------------------------------------------
Expected:
    key    [equals]  word
    value  [equals]  hello-rustaceans

Received (most similar query parameter):
    word=hello

All received query parameter values:
    1. word=hello

Matcher:  query_param
Docs:     https://docs.rs/httpmock/0.8.2/httpmock/struct.When.html#method.query_param
```

# Usage

See the [official website](http://httpmock.rs) for detailed API documentation.

## Examples

You can find examples in the
[`httpmock` test directory](https://github.com/httpmock/httpmock/blob/master/tests/).
The [official website](http://httpmock.rs) and [reference docs](https://docs.rs/httpmock/) also contain _**a lot**_ of examples. 

## License

`httpmock` is free software: you can redistribute it and/or modify it under the terms of the MIT Public License.

This program is distributed in the hope that it will be useful, but WITHOUT ANY WARRANTY; without even the implied
warranty of MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the MIT Public License for more details.
