use httpmock::MockServer;
use http::StatusCode;
use wasi_http_client::Client;

#[tokio::test(flavor = "current_thread")]
async fn wasi_test() {
    // Connect to your already running httpmock server (Docker etc.)
    let server = MockServer::connect_async("127.0.0.1:5050").await;

    // Arrange: create mock on that remote instance (async API!)
    let search_mock = server
        .mock_async(|when, then| {
            when.method("POST").path("/test");
            then.status(202);
        })
        .await;

    // Act: `wasi_http_client` is synchronous — no `.await` on send()
    let resp = Client::new()
        .post(&format!("{}/test", server.base_url()))
        .body(b"hi") // needs &[u8]
        .send()
        .expect("HTTP request failed");

    // Assert
    search_mock.assert_async().await;
    assert_eq!(resp.status(), StatusCode::ACCEPTED);
}
