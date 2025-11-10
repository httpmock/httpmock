use httpmock::prelude::*;

use reqwest::blocking::Client;

#[cfg(feature = "record")]
#[test]
fn record_with_forwarding_test() {
    let target_server = MockServer::start();
    target_server.mock(|when, then| {
        when.any_request();
        then.status(200).body("Hi from fake GitHub!");
    });

    let recording_server = MockServer::start();
    recording_server.forward_to(target_server.base_url(), |rule| {
        rule.filter(|when| {
            when.path("/hello");
        });
    });

    let recording = recording_server.record(|rule| {
        rule.record_response_delays(true)
            .record_request_headers(vec!["Accept", "Content-Type"])
            .filter(|when| {
                when.path("/hello");
            });
    });

    let github_client = Client::builder().build().unwrap();

    let response = github_client
        .get(format!("{}/hello", recording_server.base_url()))
        .send()
        .unwrap();
    assert_eq!(response.text().unwrap(), "Hi from fake GitHub!");

    let target_path = recording.save("my_test_scenario").unwrap();

    let playback_server = MockServer::start();

    playback_server.playback(target_path);

    let response = github_client
        .get(format!("{}/hello", playback_server.base_url()))
        .send()
        .unwrap();
    assert_eq!(response.text().unwrap(), "Hi from fake GitHub!");
}

// @example-start: record-proxy-website
#[cfg(all(feature = "proxy", feature = "https", feature = "record"))]
#[test]
fn record_with_proxy_example_test() {
    use httpmock::RecordingRuleBuilder;

    // Start a mock server to act as a proxy for the HTTP client
    let recording_proxy_server = MockServer::start();

    // Configure the mock server to proxy all incoming requests
    recording_proxy_server.proxy(|rule| {
        rule.filter(|when| {
            when.any_request(); // Intercept all requests
        });
    });

    // Set up recording on the mock server to capture all proxied
    // requests and responses
    let recording = recording_proxy_server.record(|rule: RecordingRuleBuilder| {
        rule.filter(|when| {
            when.any_request(); // Record all requests
        });
    });

    // Create an HTTP client configured to route requests
    // through the mock proxy server
    let client = Client::builder()
        // Set the proxy URL to the mock server's URL
        .proxy(reqwest::Proxy::all(recording_proxy_server.base_url()).unwrap())
        .build()
        .unwrap();

    // Send a GET request using the client, which will be proxied by the mock server
    let response = client.get("https://httpmock.rs").send().unwrap();

    // Since the request was forwarded, we should see a GitHub API response.
    assert_eq!(response.status().as_u16(), 200);
    assert!(response
        .text()
        .unwrap()
        .contains("Simple yet powerful HTTP mocking library for Rust"));

    // Save the recording to
    // "target/httpmock/recordings/website-via-proxy_<timestamp>.yaml".
    let recording_file_path = recording
        .save("website-via-proxy")
        .expect("could not save the recording");

    // **********************************************************************
    // Playback

    // Start a new mock server instance for playback
    let playback_server = MockServer::start();

    // Load the recorded interactions into the new mock server
    playback_server.playback(recording_file_path);

    // Create an HTTP client configured to route requests through the playback mock proxy server
    let client = Client::builder()
        // Set the proxy URL to the mock server's URL
        .proxy(reqwest::Proxy::all(playback_server.base_url()).unwrap())
        .build()
        .unwrap();

    // Send a request to the httpmock website which will be responded this time with the proxy s
    // matches the recorded data
    let response = client.get("https://httpmock.rs").send().unwrap();
    assert_eq!(response.status().as_u16(), 200);
    assert!(response
        .text()
        .unwrap()
        .contains("Simple yet powerful HTTP mocking library for Rust"));
}
// @example-end

// @example-start: record-forwarding-website
#[cfg(all(feature = "record"))]
#[test]
fn record_with_forwarding_example_test() {
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

    let recording = server.record(|rule| {
        rule.filter(|when| {
            when.any_request(); // Ensure all requests are recorded.
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

    // Save the recording to
    // "target/httpmock/recordings/website-via-forwarding_<timestamp>.yaml".
    let recording_file_path = recording
        .save("website-via-forwarding")
        .expect("cannot store recording on disk");

    // Start a new mock server instance for playback
    let playback_server = MockServer::start();

    // Load the recorded interactions into the new mock server
    playback_server.playback(recording_file_path);

    // Send a request to the playback server and verify the response
    // matches the recorded data
    let response = client.get(playback_server.base_url()).send().unwrap();
    assert_eq!(response.status().as_u16(), 200);
    assert!(response
        .text()
        .unwrap()
        .contains("Simple yet powerful HTTP mocking library for Rust"));
}
// @example-end

// @example-start: playback-forwarding-website
#[cfg(all(feature = "record"))]
#[test]
fn playback_github_api() {
    // Start a mock server for the test
    let server = MockServer::start();

    // Configure the mock server to forward requests to the target
    // host (GitHub API) instead of responding with a mock. The 'rule'
    // parameter allows you to define conditions under which forwarding
    // should occur.
    server.forward_to("https://httpmock.rs", |rule| {
        rule.filter(|when| {
            when.any_request(); // Forward all requests.
        });
    });

    // Set up recording to capture all forwarded requests and responses
    let recording = server.record(|rule| {
        rule.filter(|when| {
            when.any_request(); // Record all requests and responses.
        });
    });

    // Send an HTTP request to the mock server, which will be forwarded
    // to the GitHub API
    let client = Client::new();
    let response = client.get(server.base_url()).send().unwrap();

    // Assert that the response from the forwarded request is as expected
    assert_eq!(response.status().as_u16(), 200);
    assert!(response
        .text()
        .unwrap()
        .contains("Simple yet powerful HTTP mocking library for Rust"));

    // Save the recorded interactions to a file
    let target_path = recording
        .save("github-torvalds-scenario")
        .expect("Failed to save the recording to disk");

    // Start a new mock server instance for playback
    let playback_server = MockServer::start();

    // Load the recorded interactions into the new mock server

    playback_server.playback(target_path);

    // Send a request to the playback server and verify the response
    // matches the recorded data
    let response = client.get(playback_server.base_url()).send().unwrap();
    assert_eq!(response.status().as_u16(), 200);
    assert!(response
        .text()
        .unwrap()
        .contains("Simple yet powerful HTTP mocking library for Rust"));
}
// @example-end

#[cfg(all(feature = "record"))]
#[test]
fn record_with_forwarding_all_request_parts_test() {
    let server = MockServer::start();

    server.forward_to("https://httpmock.rs", |rule| {
        rule.filter(|when| {
            when.any_request(); // Ensure all requests are forwarded.
        });
    });

    let recording = server.record(|rule| {
        rule.record_request_headers(vec![
            String::from("X-Auth-Token"),
            String::from("Accept-Language"),
        ])
        .filter(|when| {
            when.any_request(); // Ensure all requests are recorded.
        });
    });

    let client = Client::new();

    // Example headers
    let mut headers = reqwest::header::HeaderMap::new();
    headers.insert("X-Auth-Token", "secret123".parse().unwrap());
    headers.insert("Accept-Language", "en-US".parse().unwrap());

    // Query Parameters
    let mut query_params = std::collections::HashMap::new();
    query_params.insert("this", "is");
    query_params.insert("a", "test");
    query_params.insert("empty", "");

    let response = client
        .get(server.base_url())
        .headers(headers.clone())
        .query(&query_params)
        .send()
        .unwrap();

    // Send "original" request
    assert_eq!(response.status().as_u16(), 200);
    assert!(response
        .text()
        .unwrap()
        .contains("Simple yet powerful HTTP mocking library for Rust"));

    let recording_file_path = recording
        .save("website-via-forwarding")
        .expect("cannot store recording on disk");

    // Start a new mock server instance for playback
    let playback_server = MockServer::start();
    playback_server.playback(recording_file_path);

    // Send a request to playback server
    let response = client
        .get(playback_server.base_url())
        .headers(headers)
        .query(&query_params)
        .send()
        .unwrap();

    assert_eq!(response.status().as_u16(), 200);
    assert!(response
        .text()
        .unwrap()
        .contains("Simple yet powerful HTTP mocking library for Rust"));
}
