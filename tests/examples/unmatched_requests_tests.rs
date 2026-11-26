use std::panic::{AssertUnwindSafe, catch_unwind};

use httpmock::prelude::*;
use reqwest::blocking::Client;

#[test]
fn assert_no_unmatched_requests_accepts_matched_requests() {
    let server = MockServer::start();

    // Nothing has arrived yet.
    server.assert_no_unmatched_requests();

    let mock = server.mock(|when, then| {
        when.method(GET).path("/hello");
        then.status(200);
    });

    let response = Client::new().get(server.url("/hello")).send().unwrap();

    assert_eq!(response.status(), 200);
    mock.assert();
    server.assert_no_unmatched_requests();
}

#[test]
fn unmatched_request_fails_with_its_method_path_and_headers() {
    let server = MockServer::start();
    server.mock(|when, then| {
        when.method(GET).path("/hello");
        then.status(200);
    });

    let client = Client::new();
    client.get(server.url("/hello")).send().unwrap();
    client
        .get(server.url("/missing?x=1"))
        .header("x-trace", "unmatched")
        .send()
        .unwrap();

    let previous_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(|_| {}));
    let result = catch_unwind(AssertUnwindSafe(|| server.assert_no_unmatched_requests()));
    std::panic::set_hook(previous_hook);

    let message = match result {
        Ok(()) => panic!("expected unmatched requests to fail the assertion"),
        Err(payload) => payload.downcast::<String>().expect("panic message"),
    };

    assert!(message.contains("1 request did not match any mock"), "{message}");
    assert!(message.contains("GET /missing?x=1"), "{message}");
    assert!(message.contains("x-trace: unmatched"), "{message}");
    assert!(!message.contains("GET /hello"), "{message}");
}

#[tokio::test]
async fn assert_no_unmatched_requests_async_accepts_a_matched_request() {
    let server = MockServer::start_async().await;
    server
        .mock_async(|when, then| {
            when.path("/hello");
            then.status(200);
        })
        .await;

    let response = reqwest::get(server.url("/hello")).await.unwrap();

    assert_eq!(response.status(), 200);
    server.assert_no_unmatched_requests_async().await;
}

#[test]
fn reset_clears_unmatched_requests() {
    let server = MockServer::start();
    Client::new().get(server.url("/gone")).send().unwrap();

    server.reset();

    server.assert_no_unmatched_requests();
}

#[cfg(feature = "remote")]
#[test]
#[should_panic(expected = "GET /standalone-unmatched")]
fn standalone_server_reports_unmatched_requests() {
    crate::with_standalone_server();

    let server = MockServer::connect("localhost:5050");
    Client::new().get(server.url("/standalone-unmatched")).send().unwrap();
    server.assert_no_unmatched_requests();
}
