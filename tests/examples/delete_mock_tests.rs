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
    let response = reqwest::blocking::get(format!("http://{}:{}/health", server.host(), server.port())).unwrap();

    // Assert
    m.assert();
    assert_eq!(response.status(), 205);

    // Delete the mock and send the request again
    m.delete();

    let response = reqwest::blocking::get(format!("http://{}/health", server.address())).unwrap();

    // Assert that the request failed because the mock has been deleted
    assert_eq!(response.status(), 404);
}
