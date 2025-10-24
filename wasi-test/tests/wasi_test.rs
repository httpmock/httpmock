use httpmock::MockServer;
use http::StatusCode;
use wasi_http_client::Client;

#[test]
fn wasi_http_post_works() {
    // Connect synchronously to the already running httpmock server
    let server = MockServer::connect("127.0.0.1:5050");

    // Arrange: synchronous mock
    let search_mock = server.mock(|when, then| {
        when.method("POST").path("/test");
        then.status(202);
    });

    // Act: wasi_http_client is also synchronous
    let resp = Client::new()
        .post(&format!("{}/test", server.base_url()))
        .body(b"hi")
        .send()
        .expect("HTTP request failed");

    // Assert
    search_mock.assert();
    assert_eq!(resp.status(), StatusCode::ACCEPTED);
}