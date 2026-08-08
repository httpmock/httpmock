use httpmock::prelude::*;

#[tokio::test]
async fn reset_server_test() {
    let server = MockServer::start();

    server.mock(|when, then| {
        when.method("GET").path("/translate").query_param("word", "hello");
        then.status(500)
            .header("content-type", "text/html; charset=UTF-8")
            .body("hola");
    });

    server.reset_async().await;

    let hello_mock = server.mock(|when, then| {
        when.method("GET").path("/translate").query_param("word", "hello");
        then.status(200)
            .header("content-type", "text/html; charset=UTF-8")
            .body("hola");
    });

    let response = reqwest::get(&server.url("/translate?word=hello")).await.unwrap();

    hello_mock.assert();
    assert_eq!(response.status(), 200);
}
