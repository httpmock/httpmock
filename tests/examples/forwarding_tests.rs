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
    server.forward_to(target_server.base_url(), |rule| {
        rule.filter(|when| {
            when.any_request(); // We want all requests to be forwarded.
        });
    });

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
fn forward_to_website() {
    // Let's create our mock server for the test
    let server = MockServer::start();

    // We configure our server to forward the request to the target
    // host instead of answering with a mocked response. The 'when'
    // variable lets you configure rules under which forwarding
    // should take place.
    server.forward_to("https://httpmock.rs", |rule| {
        rule.filter(|when| {
            when.any_request(); // Ensure all requests are forwarded.
        });
    });

    // Now let's send an HTTP request to the mock server. The request
    // will be forwarded to the GitHub API, as we configured before.
    let client = Client::new();

    let response = client.get(server.base_url()).send().unwrap();

    // Since the request was forwarded, we should see a GitHub API response.
    assert_eq!(response.status().as_u16(), 200);
    assert!(response
        .text()
        .unwrap()
        .contains("Simple yet powerful HTTP mocking library for Rust"));
}

/// Deletes a forwarding rule through the admin API of a remote server and
/// verifies that deleting an unknown rule id returns 404.
#[cfg(feature = "remote")]
#[test]
fn delete_forwarding_rule_on_remote_server_test() {
    use crate::with_standalone_server;

    // Arrange: connect to a standalone server so the deletion goes through
    // the HTTP admin API rather than the in-process state manager.
    with_standalone_server();
    let server = MockServer::connect("localhost:5050");

    let mut rule = server.forward_to("http://localhost:12345", |rule| {
        rule.filter(|when| {
            when.any_request();
        });
    });

    // Act + Assert: panics if the server does not respond with 204.
    rule.delete();

    // Deleting an unknown rule id must return 404.
    let response = Client::new()
        .delete(format!(
            "{}/__httpmock__/forwarding_rules/4242",
            server.base_url()
        ))
        .send()
        .unwrap();
    assert_eq!(response.status().as_u16(), 404);
}
