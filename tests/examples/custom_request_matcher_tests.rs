#[test]
fn my_custom_request_matcher_test() {
    use httpmock::prelude::*;

    // Arrange
    let server = MockServer::start();

    let mock = server.mock(|when, then| {
        when.is_true(|req| req.uri().path().ends_with("Test"));
        then.status(201);
    });

    // Act: Send the HTTP request using reqwest
    let response = reqwest::blocking::get(server.url("/thisIsMyTest")).unwrap();

    // Assert
    mock.assert();
    assert_eq!(response.status(), 201);
}

#[test]
fn dynamic_responder_test() {
    use httpmock::prelude::*;
    use reqwest::blocking::Client;
    use std::sync::Mutex;

    // Arrange
    let server = MockServer::start();

    // This is our counter that will determine the response later.
    // It needs to be protected by a mutex the custom respond method
    // is called from the HTTP server thread.
    let call_count = Mutex::new(0);

    let mock = server.mock(|when, then| {
        when.method("GET").is_true(|r| {
            return r.uri().path().ends_with("/hello");
        });
        then.respond_with(move |_req: &HttpMockRequest| {
            let mut count = call_count.lock().unwrap();
            *count += 1;

            return HttpMockResponse::builder().status(200 + *count).build();
        });
    });

    // Act
    let client = Client::new();

    let response1 = client.get(server.url("/hello")).send().unwrap();
    let response2 = client.get(server.url("/hello")).send().unwrap();
    let response3 = client.get(server.url("/hello")).send().unwrap();

    // Assert
    mock.assert_calls(3);

    assert_eq!(response1.status(), 201);
    assert_eq!(response2.status(), 202);
    assert_eq!(response3.status(), 203);
}

#[test]
fn dynamic_responder_http_crate_test() {
    use httpmock::prelude::*;
    use reqwest::blocking::Client;
    use std::sync::Mutex;

    // Arrange
    let server = MockServer::start();

    // This is our counter that will determine the response later.
    // It needs to be protected by a mutex the custom respond method
    // is called from the HTTP server thread.
    let call_count = Mutex::new(0);

    let mock = server.mock(|when, then| {
        when.method("GET");
        then.respond_with(move |req: &HttpMockRequest| {
            // Convert the HttpMockRequest to a http creates Request object
            let req: http::Request<()> = req.into();

            let mut count = call_count.lock().unwrap();
            *count += 1;

            // Return a http crate Response object which will automatically be converted into
            // a HttpMockResponse internally
            http::Response::builder()
                .status(200 + *count)
                .body(req.uri().path().to_string())
                .unwrap()
                .into()
        });
    });

    // Act
    let client = Client::new();

    let response1 = client.get(server.base_url()).send().unwrap();
    let response2 = client.get(server.base_url()).send().unwrap();
    let response3 = client.get(server.base_url()).send().unwrap();

    // Assert
    mock.assert_calls(3);

    assert_eq!(response1.status(), 201);
    assert_eq!(response2.status(), 202);
    assert_eq!(response3.status(), 203);
}
