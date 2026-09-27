use httpmock::prelude::*;
use reqwest::blocking::get;

#[cfg(feature = "remote")]
use crate::with_standalone_server;

#[cfg(feature = "remote")]
#[test]
fn loop_with_standalone_test() {
    // Arrange

    // This starts up a standalone server in the background running on port 5050
    with_standalone_server();

    // Instead of creating a new MockServer using new(), we connect to an existing remote instance.
    let server = MockServer::connect("localhost:5050");

    for x in 0..1000 {
        let search_mock = server.mock(|when, then| {
            when.path(format!("/test/{x}"));
            then.status(202);
        });

        // Act: Send the HTTP request
        let response = get(server.url(format!("/test/{x}"))).unwrap();

        // Assert
        search_mock.assert();
        assert_eq!(response.status(), 202);
    }
}

#[test]
fn loop_with_local_test() {
    // Arrange

    // Create a new local MockServer instance.
    let server = MockServer::start();

    let _mock = server.mock(|when, then| {
        when.path("/test")
            .path_includes("test")
            .query_param("myQueryParam", "überschall");
        then.status(202);
    });

    for x in 0..1000 {
        let search_mock = server.mock(|when, then| {
            when.path(format!("/test/{x}"));
            then.status(202);
        });

        // Act: Send the HTTP request
        let response = get(server.url(format!("/test/{x}"))).unwrap();

        // Assert
        search_mock.assert();
        assert_eq!(response.status(), 202);
    }
}
