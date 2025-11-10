use httpmock::prelude::*;

#[test]
fn explicit_delete_mock_test() {
    // Arrange
    let server = MockServer::start();

    let mut m = server.mock(|when, then| {
        when.method("GET").path("/health");
        then.status(205);
    });

    // Act: Send the HTTP request using reqwest
    let response =
        reqwest::blocking::get(format!("http://{}:{}/health", server.host(), server.port()))
            .unwrap();

    // Assert
    m.assert();
    assert_eq!(response.status(), 205);

    // Delete the mock and send the request again
    m.delete();

    let response = reqwest::blocking::get(format!("http://{}/health", server.address())).unwrap();

    // Assert that the request failed because the mock has been deleted
    assert_eq!(response.status(), 404);
}

#[test]
fn delete_mock_after_calls_test() {
    // Arrange
    let server = MockServer::start();

    let mut first_mock = server.mock(|when, then| {
        when.method("GET").path("/health");
        then.status(205);
    });
    first_mock.delete_after_calls(2);

    let mut second_mock = server.mock(|when, then| {
        when.method("GET").path("/health");
        then.status(500);
    });
    second_mock.delete_after_calls(1);

    // Act: Send the HTTP request using reqwest
    let url = format!("http://{}:{}/health", server.host(), server.port());

    let response_1 = reqwest::blocking::get(&url).unwrap();
    let response_2 = reqwest::blocking::get(&url).unwrap();

    // Assert
    assert_eq!(response_1.status(), 205);
    assert_eq!(response_2.status(), 205);

    // The first mock is now deleted, second mock is responding
    let response_3 = reqwest::blocking::get(&url).unwrap();
    assert_eq!(response_3.status(), 500);

    let response_4 = reqwest::blocking::get(&url).unwrap();

    // Assert that the request failed because both mocks has been deleted
    assert_eq!(response_4.status(), 404);
}

#[test]
#[should_panic]
fn delete_mock_after_calls_panic_with_zero_test() {
    // Arrange
    let server = MockServer::start();

    let mut m = server.mock(|when, then| {
        when.method("GET").path("/health");
        then.status(205);
    });

    // Act: Set the invalid count on delete_after_calls

    // Will panic
    m.delete_after_calls(0);
}

#[test]
#[cfg(feature = "remote")]
fn remote_delete_mock_after_calls_test() {
    use crate::with_standalone_server;
    use httpmock::MockServer;
    use reqwest::blocking::Client;

    // Arrange

    // This starts up a standalone server in the background running on port 5050
    with_standalone_server();

    // Instead of creating a new MockServer using new(), we connect to an existing remote instance.
    let server = MockServer::connect("localhost:5050");

    let mut search_mock = server.mock(|when, then| {
        when.path("/search").body("hello");
        then.status(202);
    });

    search_mock.delete_after_calls(2);

    // Act: Send the HTTP requests
    let client = Client::new();
    let response_1 = client
        .post(&server.url("/search"))
        .body("hello")
        .send()
        .unwrap();
    let response_2 = client
        .post(&server.url("/search"))
        .body("hello")
        .send()
        .unwrap();

    // Assert
    assert_eq!(response_1.status(), 202);
    assert_eq!(response_2.status(), 202);

    let response_3 = client
        .post(&server.url("/search"))
        .body("hello")
        .send()
        .unwrap();

    assert_eq!(response_3.status(), 404);
}
