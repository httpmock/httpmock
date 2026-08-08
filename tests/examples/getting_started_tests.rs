#[test]
fn getting_started_test() {
    use httpmock::prelude::*;

    // Start a lightweight mock server.
    let server = MockServer::start();

    // Create a mock on the server.
    let hello_mock = server.mock(|when, then| {
        when.method("GET").path("/translate").query_param("word", "hello");
        then.status(200)
            .header("content-type", "text/html; charset=UTF-8")
            .body("hola");
    });

    // Send an HTTP request to the mock server. This simulates your code.
    let response = reqwest::blocking::get(server.url("/translate?word=hello")).unwrap();

    // Ensure the specified mock was called.
    hello_mock.assert();

    // Ensure the mock server did respond as specified.
    assert_eq!(response.status(), 200);
}

#[tokio::test]
async fn async_getting_started_test() {
    use httpmock::prelude::*;

    // Start a lightweight mock server.
    let server = MockServer::start_async().await;

    // Create a mock on the server.
    let mock = server
        .mock_async(|when, then| {
            when.method(GET).path("/translate").query_param("word", "hello");
            then.status(200)
                .header("content-type", "text/html; charset=UTF-8")
                .body("hola");
        })
        .await;

    // Send an HTTP request to the mock server. This simulates your code.
    let client = reqwest::Client::new();
    let response = client.get(server.url("/translate?word=hello")).send().await.unwrap();

    // Ensure the specified mock was called exactly one time (or fail with a
    // detailed error description).
    mock.assert();

    // Ensure the mock server did respond as specified.
    assert_eq!(response.status(), 200);
}
