use wasm_bindgen_test::*;
use httpmock::MockServer;
use reqwest::Client;

wasm_bindgen_test_configure!(run_in_browser);

#[wasm_bindgen_test]
async fn connect_to_remote_httpmock_and_post_big_body() {
    // Connect to your already running httpmock server (Docker etc.)
    let server = MockServer::connect_async("127.0.0.1:5050").await;


    // Arrange: create mock on that remote instance
    // TODO / FIX: It seems the test currently also runs without this, which means we clould be
    //  too aggressive setting CORS headers in responses
    server
        .mock_async(|when, then| {
            when.method("OPTIONS");
            then.status(204) // preflights usually return 204 No Content
                .header("Access-Control-Allow-Origin", "*")
                .header("Access-Control-Allow-Methods", "GET, POST, PUT, PATCH, DELETE, OPTIONS")
                .header("Access-Control-Allow-Headers", "Content-Type, Authorization, X-Requested-With, Accept")
                .header("Access-Control-Max-Age", "600");
        })
        .await;


    // Arrange: create mock on that remote instance
    let search_mock = server.mock_async(|when, then| {
        when.method("POST");
        then.status(202);
    }).await;

    // Act: send the HTTP request to the mock endpoint
    let client = Client::builder().build().unwrap();
    let response = client
        .post(server.url("/search"))
        .body("wow so large".repeat(1))
        .send()
        .await
        .unwrap();

    // Assert: mock called and correct status code
    search_mock.assert_async().await;
    assert_eq!(response.status(), 202);
}
