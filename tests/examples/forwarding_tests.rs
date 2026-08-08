use httpmock::prelude::*;
use reqwest::blocking::Client;

// @example-start: forwarding
#[test]
fn forwarding_test() {
    // We will create this mock server to simulate a real service (e.g., GitHub, AWS, etc.).
    let target_server = MockServer::start();
    target_server.mock(|when, then| {
        when.any_request();
        then.status(200).body("Hi from fake GitHub!");
    });

    // Let's create our mock server for the test
    let server = MockServer::start();

    // We configure our server to forward the request to the target host instead of
    // answering with a mocked response. The 'when' variable lets you configure
    // rules under which forwarding should take place.
    server
        .forward_to(target_server.base_url(), |rule| {
            rule.filter(|when| {
                when.any_request(); // We want all requests to be forwarded.
            });
        })
        .unwrap();

    // Now let's send an HTTP request to the mock server. The request will be forwarded
    // to the target host, as we configured before.
    let client = Client::new();

    // Since the request was forwarded, we should see the target host's response.
    let response = client.get(server.url("/get")).send().unwrap();
    assert_eq!(response.status().as_u16(), 200);
    assert_eq!(response.text().unwrap(), "Hi from fake GitHub!");
}
// @example-end

#[test]
fn invalid_forwarding_target_is_rejected() {
    let server = MockServer::start();

    let result = server.forward_to("/relative", |_| {});

    assert!(matches!(
        result,
        Err(httpmock::ServerAdapterError::UpstreamError(message))
            if message.contains("forwarding target has no scheme")
    ));
}

#[test]
fn invalid_forwarding_header_is_rejected() {
    let server = MockServer::start();

    let result = server.forward_to("http://example.com", |rule| {
        rule.add_request_header("invalid header", "value");
    });

    assert!(matches!(
        result,
        Err(httpmock::ServerAdapterError::UpstreamError(message))
            if message.contains("invalid forwarding header name")
    ));
}

#[test]
fn forward_to_website() {
    // Let's create our mock server for the test
    let server = MockServer::start();

    // We configure our server to forward the request to the target
    // host instead of answering with a mocked response. The 'when'
    // variable lets you configure rules under which forwarding
    // should take place.
    server
        .forward_to("https://httpmock.rs", |rule| {
            rule.filter(|when| {
                when.any_request(); // Ensure all requests are forwarded.
            });
        })
        .unwrap();

    // Now let's send an HTTP request to the mock server. The request
    // will be forwarded to the GitHub API, as we configured before.
    let client = Client::new();

    let response = client.get(server.base_url()).send().unwrap();

    // Since the request was forwarded, we should see a GitHub API response.
    assert_eq!(response.status().as_u16(), 200);
    assert!(
        response
            .text()
            .unwrap()
            .contains("Simple yet powerful HTTP mocking library for Rust")
    );
}
