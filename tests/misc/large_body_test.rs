use httpmock::prelude::*;
use reqwest::blocking::Client;

use crate::with_standalone_server;

#[test]
fn large_body_test() {
    // Arrange

    // This starts up a standalone server in the background running on port 5050
    with_standalone_server();

    // Instead of creating a new MockServer using new(), we connect to an existing remote instance.
    let server = MockServer::connect("localhost:5050");

    let search_mock = server.mock(|when, then| {
        when.path("/search").body("wow so large".repeat(1024 * 1024 * 10)); // 10 MB body
        then.status(202);
    });

    // Act: Send the HTTP request
    let client = Client::new();
    let response = client
        .post(server.url("/search"))
        .body("wow so large".repeat(1024 * 1024 * 10)) // 10 MB body
        .send()
        .unwrap();

    // Assert
    search_mock.assert();
    assert_eq!(response.status(), 202);
}
