#[test]
fn all_runtimes_test() {
    use crate::with_standalone_server;

    with_standalone_server();

    // Tokio
    assert_eq!(
        tokio::runtime::Runtime::new().unwrap().block_on(test_fn()),
        202
    );

    // Actix
    assert_eq!(actix_rt::Runtime::new().unwrap().block_on(test_fn()), 202);

    // async_std
    assert_eq!(smol::block_on(test_fn()), 202);
}

#[cfg(all(feature = "proxy", feature = "https"))]
async fn test_fn() -> u16 {
    use httpmock::prelude::*;

    use crate::utils::http::get_request;

    // Proxy forwarder
    let server2 = MockServer::connect_async("localhost:5050").await;
    server2
        .mock_async(|when, then| {
            when.any_request();
            then.status(202).body("Hi from fake GitHub!");
        })
        .await;

    // Outer proxy
    let server1 = MockServer::start_async().await;
    server1
        .proxy_async(|rule| {
            rule.filter(|when| {
                when.any_request();
            });
        })
        .await;

    // Through proxy to server2
    let (status_code, body) = get_request(
        &server2.url("/get"),
        None,
        Some(server1.base_url().as_str()),
    )
    .await
    .expect("proxy to server2 failed");

    assert_eq!("Hi from fake GitHub!", body);
    assert_eq!(202, status_code);

    status_code
}

#[cfg(all(feature = "proxy", not(any(feature = "https", feature = "standalone"))))]
async fn test_fn() -> u16 {
    use httpmock::prelude::*;

    use crate::utils::http::get_request;

    // Proxy forwarder
    let server2 = MockServer::connect_async("localhost:5050").await;
    server2
        .forward_to_async("http://httpbingo.org", |rule| {
            rule.filter(|when| {
                when.any_request();
            });
        })
        .await;

    // Outer proxy
    let server1 = MockServer::start_async().await;
    server1
        .proxy_async(|rule| {
            rule.filter(|when| {
                when.any_request();
            });
        })
        .await;

    // Through proxy to server2
    let (status_code, body) = get_request(
        &server2.url("/get"),
        // httpbingo requires a User-Agent header
        Some(
            [("User-Agent".into(), "MyTestClient/1.0".into())]
                .into_iter()
                .collect(),
        ),
        Some(server1.base_url().as_str()),
    )
    .await
    .expect("proxy to server2 failed");

    assert_eq!(200, status_code);
    assert!(body.contains("httpbingo.org"));

    202 // fake 202 since the parent test expects it
}

#[cfg(all(feature = "proxy", feature = "standalone", not(feature = "https")))]
async fn test_fn() -> u16 {
    use httpmock::prelude::*;

    use crate::utils::http::get_request;

    // Fake GitHub target
    let target_server = MockServer::connect_async("localhost:5050").await;
    target_server
        .mock_async(|when, then| {
            when.any_request();
            then.status(202).body("Hi from fake GitHub!");
        })
        .await;

    // Our proxy proxy
    let proxy_server = MockServer::start_async().await;
    proxy_server
        .proxy_async(|rule| {
            rule.filter(|when| {
                when.any_request();
            });
        })
        .await;

    // Through proxy to server2
    let (status_code, body) = get_request(
        &target_server.url("/get"),
        None,
        Some(proxy_server.base_url().as_str()),
    )
    .await
    .expect("proxy to server2 failed");

    assert_eq!("Hi from fake GitHub!", body);
    assert_eq!(202, status_code);

    status_code
}

#[cfg(not(any(feature = "proxy")))]
async fn test_fn() -> u16 {
    use httpmock::prelude::*;

    use crate::utils::http::get_request;

    let server = MockServer::start_async().await;
    let mock = server
        .mock_async(|when, then| {
            when.path("/get");
            then.status(202);
        })
        .await;

    let (status, _body) = get_request(&server.url("/get"), None, None)
        .await
        .expect("request failed");

    mock.assert_async().await;

    status
}
