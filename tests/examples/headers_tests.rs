use httpmock::prelude::*;
use reqwest::blocking::Client;

#[test]
fn headers_test() {
    // Arrange
    let server = MockServer::start();

    let m = server.mock(|when, then| {
        when.path("/test")
            .header("Authorization", "token 123456789")
            .header_exists("Authorization");
        then.status(201).header("Content-Length", "0");
    });

    // Act: Send the request using reqwest
    let client = Client::new();
    let response = client
        .post(format!("http://{}/test", server.address()))
        .header("Authorization", "token 123456789")
        .send()
        .unwrap();

    // Assert
    m.assert();
    assert_eq!(response.status(), 201);
    assert_eq!(response.headers().get("Content-Length").unwrap().to_str().unwrap(), "0");
}

#[test]
fn headers_test_header_count_regex() {
    // Arrange
    let server = MockServer::start();

    // Create a mock that expects at least 2 headers whose keys match the regex "^X-Custom-Header.*"
    // and values match the regex "value.*"
    let mock = server.mock(|when, then| {
        when.header_count("^X-Custom-Header.*", "value.*", 2);
        then.status(200); // Respond with a 200 status code if the condition is met
    });

    // Act: Make a request that includes the required headers using reqwest
    let client = Client::new();
    client
        .post(format!("http://{}/test", server.address()))
        .header("x-custom-header-1", "value1")
        .header("X-Custom-Header-2", "value2")
        .send()
        .unwrap();

    // Assert: Verify that the mock was called at least once
    mock.assert();
}
