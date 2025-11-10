#![cfg(feature = "https")]

use crate::with_standalone_server;

#[tokio::test]
async fn test_http_get_request() {
    use httpmock::MockServer;

    // Arrange
    let server = MockServer::start_async().await;

    server
        .mock_async(|when, then| {
            when.any_request();
            then.header("X-Hello", "test").status(200);
        })
        .await;

    let base_url = format!("https://{}", server.address());

    let client = reqwest::Client::new();
    let res = client.get(&base_url).send().await.unwrap();

    assert_eq!(res.status(), 200, "HTTP status should be 200 OK");
}

#[cfg(feature = "remote")]
#[tokio::test]
async fn https_test_reqwest() {
    use httpmock::MockServer;
    use reqwest::{tls::Certificate, Client};
    use std::{fs::read, path::PathBuf};

    // This starts up a standalone server in the background running on port 5050
    with_standalone_server();

    // Arrange
    let server = MockServer::connect_async("localhost:5050").await;

    server
        .mock_async(|when, then| {
            when.any_request();
            then.header("X-Hello", "test").status(200);
        })
        .await;

    let base_url = format!("https://localhost:{}", server.address().port());

    // Load the CA certificate from the project path
    let project_dir = env!("CARGO_MANIFEST_DIR");
    let cert_path = PathBuf::from(project_dir).join("certs/ca.pem");
    let cert = Certificate::from_pem(&read(cert_path).unwrap()).unwrap();

    // Build the client with the CA certificate
    let client = Client::builder()
        .add_root_certificate(cert)
        .build()
        .unwrap();

    let res = client.get(&base_url).send().await.unwrap();

    assert_eq!(res.status(), 200);
    assert_eq!(
        res.headers().get("X-Hello").unwrap().to_str().unwrap(),
        "test"
    );
    assert!(base_url.starts_with("https://"));
}
