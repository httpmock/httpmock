use std::panic::{AssertUnwindSafe, catch_unwind};

use httpmock::{HttpMockRequest, MockServer};

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
fn request_conversion_preserves_uri_headers_and_binary_body() {
    let request = make_request(vec![0, 255, 128]);
    let converted = request.to_http_request();
    assert_eq!(converted.uri(), &request.uri());
    assert_eq!(converted.headers()["x-test"], "retained");
    assert_eq!(converted.body().as_ref(), &[0, 255, 128]);
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
fn released_state_manager_name_remains_accessible() {
    let _manager = httpmock::server::state::HttpMockStateManager::new(100);
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
