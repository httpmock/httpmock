use httpmock::prelude::*;
use reqwest::blocking::Client;

#[test]
fn dynamic_response_test() {
    // Arrange
    let server = MockServer::start();

    let mock = server.mock(|when, then| {
        when.method("GET");
        then.reply_with(|_req| {
            http::Response::builder()
                .status(201)
                .body("ok") // bytes::Bytes
                .unwrap()
                .try_into()
                .unwrap()
        });
    });

    // Act: Send the request with cookies
    let client = Client::new();
    let response = client.get(server.base_url()).send().unwrap();

    // Assert
    mock.assert();
    assert_eq!(response.status(), 201);
}
