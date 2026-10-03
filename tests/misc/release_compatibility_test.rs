use std::panic::{AssertUnwindSafe, catch_unwind};

use httpmock::{HttpMockRequest, HttpMockResponse, Mock, MockExt, MockServer};

fn make_request(body: Vec<u8>) -> HttpMockRequest {
    http::Request::builder()
        .uri("/users?name=first&name=last+value&encoded=%26")
        .header("x-test", "retained")
        .extension(httpmock::server::RequestMetadata::new("http"))
        .body(body)
        .unwrap()
        .into()
}

#[test]
fn body_helpers_remain_available_on_requests_and_responses() {
    let request = make_request(b"\xffready\0".to_vec());
    let body = request.body();
    assert!(body.contains_str("ready"));
    assert!(body.contains_str(""));
    assert!(body.contains_slice(b"\xffready"));
    assert!(body.contains_vec(&b"ready\0".to_vec()));
    assert!(!body.contains_str("missing"));
    assert!(!body.is_blank());

    let response = HttpMockResponse::builder().body(" \r\n\t").build();
    assert!(response.body.as_ref().unwrap().is_blank());
    assert!(make_request(Vec::new()).body().is_blank());
}

#[test]
fn request_helpers_preserve_query_and_binary_body() {
    let request = make_request(vec![0, 255, 128]);
    let query = request.query_params_map();
    assert_eq!(query["name"], "last value");
    assert_eq!(query["encoded"], "&");
    let converted = request.to_http_request();
    assert_eq!(converted.uri(), &request.uri());
    assert_eq!(converted.headers()["x-test"], "retained");
    assert_eq!(converted.body().as_ref(), &[0, 255, 128]);
}

fn legacy_id<'a, T: MockExt<'a>>(mock: &T) -> usize {
    mock.id()
}

#[test]
fn mock_extension_trait_and_mutable_id_remain_usable() {
    let server = MockServer::start();
    let mut first = server.mock(|when, then| {
        when.path("/first");
        then.status(201);
    });
    let second = server.mock(|when, then| {
        when.path("/second");
        then.status(202);
    });
    let recreated = <Mock<'_> as MockExt<'_>>::new(legacy_id(&first), &server);
    assert_eq!(recreated.id(), first.id);

    first.id = second.id;
    assert_eq!(reqwest::blocking::get(server.url("/second")).unwrap().status(), 202);
    first.assert();
    second.assert();
    recreated.assert_calls(0);
    let Mock { id, .. } = first;
    assert_eq!(id, second.id);
}

type ConversionError = <HttpMockRequest as TryFrom<&'static http::Request<String>>>::Error;

#[test]
fn conversion_error_variants_and_json_conversion_remain_usable() {
    let json_error = serde_json::from_str::<serde_json::Value>("{").unwrap_err();
    let errors = [
        ConversionError::HeaderDeserializationError("header".into()),
        ConversionError::CookieParserError("cookie".into()),
        ConversionError::StaticMockConversionError("static".into()),
        ConversionError::from(json_error),
        ConversionError::InvalidRequestData("data".into()),
        ConversionError::RequestConversionError("request".into()),
        ConversionError::ResponseConversionError("response".into()),
    ];
    for error in errors {
        // Exhaustive matching was possible against the associated error type in 0.8.3.
        match error {
            ConversionError::HeaderDeserializationError(message)
            | ConversionError::CookieParserError(message)
            | ConversionError::StaticMockConversionError(message)
            | ConversionError::InvalidRequestData(message)
            | ConversionError::RequestConversionError(message)
            | ConversionError::ResponseConversionError(message) => assert!(!message.is_empty()),
            ConversionError::JSONConversionError(source) => assert!(source.is_eof()),
        }
    }
}

#[test]
fn router_error_payload_accepts_regex_and_status_errors() {
    let invalid_pattern = String::from("[");
    let regex_error = regex::Regex::new(&invalid_pattern).unwrap_err();
    let router_error = httpmock::server::Error::RouterError(regex_error.into());
    assert!(router_error.to_string().contains("cannot parse regex"));

    let status_error = http::StatusCode::from_u16(99).unwrap_err();
    let router_error = httpmock::server::Error::RouterError(status_error.into());
    assert!(router_error.to_string().contains("invalid status code"));
}

#[test]
fn released_state_names_collections_and_constructor_remain_accessible() {
    let _manager = httpmock::server::state::HttpMockStateManager::new(100);
    let mut state = httpmock::server::state::MockServerState::new(100);
    assert!(!state.matchers.is_empty());
    state.matchers.clear();
    state.mocks.clear();
    state.history.clear();
    state.forwarding_rules.clear();
    state.proxy_rules.clear();
    state.recordings.clear();
    let _constructor = httpmock::server::HttpMockServer::new;
}

fn panic_message(action: impl FnOnce()) -> String {
    let panic = catch_unwind(AssertUnwindSafe(action)).unwrap_err();
    panic.downcast::<String>().map(|message| *message).unwrap()
}

#[test]
fn file_and_method_panics_retain_their_causes() {
    let missing = std::env::temp_dir().join(format!("httpmock-missing-{}/body.txt", std::process::id()));
    let cause = std::fs::read(&missing).unwrap_err().to_string();
    let server = MockServer::start();
    let message = panic_message(|| {
        server.mock(|when, then| {
            when.any_request();
            then.body_from_file(missing.to_str().unwrap());
        });
    });
    assert!(message.contains(&cause), "{message}");

    let message = panic_message(|| {
        let _ = httpmock::Method::from("BOGUS");
    });
    assert!(message.contains("Invalid HTTP method BOGUS"), "{message}");
}

#[cfg(feature = "record")]
#[test]
fn playback_panics_retain_the_io_error() {
    let missing = std::env::temp_dir().join(format!("httpmock-missing-{}/recording.yaml", std::process::id()));
    let cause = format!("{:?}", std::fs::read_to_string(&missing).unwrap_err());
    let server = MockServer::start();
    let message = panic_message(|| {
        server.playback(&missing);
    });
    assert!(message.contains(&cause), "{message}");
}
