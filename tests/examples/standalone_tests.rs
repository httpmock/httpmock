#[test]
#[cfg(feature = "remote")]
fn standalone_test() {
    use httpmock::MockServer;
    use reqwest::blocking::Client;

    use crate::with_standalone_server;

    // Arrange

    // This starts up a standalone server in the background running on port 5050
    with_standalone_server();

    // Instead of creating a new MockServer using new(), we connect to an existing remote instance.
    let server = MockServer::connect("localhost:5050");

    let search_mock = server.mock(|when, then| {
        when.path("/search").body("wow so large".repeat(1000000));
        then.status(202);
    });

    // Act: Send the HTTP request
    let client = Client::builder().build().unwrap();
    let response = client
        .post(server.url("/search"))
        .body("wow so large".repeat(1000000))
        .send()
        .unwrap();

    // Assert
    search_mock.assert();
    assert_eq!(response.status(), 202);
}

#[cfg(feature = "remote")]
#[tokio::test]
async fn async_standalone_test() {
    use httpmock::MockServer;
    use reqwest::Client;

    use crate::with_standalone_server;

    // Arrange

    // This starts up a standalone server in the background running on port 5050
    with_standalone_server();

    // Instead of creating a new MockServer using connect_from_env_async(), we connect by
    // reading the host and port from the environment (HTTPMOCK_HOST / HTTPMOCK_PORT) or
    // falling back to defaults (localhost on port 5050)
    let server = MockServer::connect_from_env_async().await;

    let search_mock = server
        .mock_async(|when, then| {
            when.path_includes("/search")
                .query_param("query", "metallica");
            then.status(202);
        })
        .await;

    // Act: Send the HTTP request
    let client = Client::new();
    let response = client
        .get(format!(
            "http://{}/search?query=metallica",
            server.address()
        ))
        .send()
        .await
        .unwrap();

    // Assert 1
    assert_eq!(response.status(), 202);
    assert_eq!(search_mock.calls_async().await, 1);

    // Act 2: Delete the mock and send a request to show that it is not present on the server anymore
    search_mock.delete_async().await;
    let response = client
        .get(format!(
            "http://{}:{}/search?query=metallica",
            server.host(),
            server.port()
        ))
        .send()
        .await
        .unwrap();

    // Assert: The mock was not found
    assert_eq!(response.status(), 404);
}

#[cfg(feature = "remote")]
#[test]
#[should_panic]
fn unsupported_features() {
    use httpmock::MockServer;

    use crate::with_standalone_server;

    // Arrange

    // This starts up a standalone server in the background running on port 5050
    with_standalone_server();

    // Instead of creating a new MockServer using connect_from_env(), we connect by reading the
    // host and port from the environment (HTTPMOCK_HOST / HTTPMOCK_PORT) or falling back to defaults
    let server = MockServer::connect_from_env();

    // Creating this mock will panic because expect_match is not supported when using
    // a remote mock server.
    let _ = server.mock(|when, _then| {
        when.is_true(|_| true);
    });
}

#[cfg(feature = "remote")]
#[test]
fn binary_body_standalone_test() {
    use httpmock::MockServer;
    use reqwest::blocking::get;

    use crate::with_standalone_server;

    // Arrange

    // This starts up a standalone server in the background running on port 5050
    with_standalone_server();

    let binary_content = b"\x80\x02\x03\xF0\x90\x80";

    let server = MockServer::connect_from_env();
    let m = server.mock(|when, then| {
        when.path("/hello");
        then.status(200).body(binary_content);
    });

    // Act
    let mut response = get(server.url("/hello")).unwrap();

    // Assert
    m.assert();
    assert_eq!(response.status(), 200);

    let mut buf: Vec<u8> = Vec::new();
    response.copy_to(&mut buf).expect("Cannot read from body");

    assert_eq!(buf, binary_content.to_vec());
}
