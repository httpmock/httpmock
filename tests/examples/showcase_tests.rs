use httpmock::prelude::*;
use regex::Regex;
use reqwest::blocking::Client;
use serde_json::json;

#[test]
fn showcase_test() {
    #[derive(serde::Serialize, serde::Deserialize)]
    struct TransferItem {
        number: usize,
    }

    let server = MockServer::start();

    let m = server.mock(|when, then| {
        when.method(POST)
            .path("/test")
            .path_includes("test")
            .query_param("myQueryParam", "überschall")
            .query_param_exists("myQueryParam")
            .path_matches(Regex::new(r#"test"#).unwrap())
            .header("content-type", "application/json")
            .header_exists("User-Agent")
            .body("{\"number\":5}")
            .body_includes("number")
            .body_matches(Regex::new(r#"(\d+)"#).unwrap())
            .json_body(json!({ "number": 5 }))
            .is_true(|req: &HttpMockRequest| req.uri().path().contains("es"));
        then.status(200);
    });

    let uri = format!("http://{}/test?myQueryParam=%C3%BCberschall", server.address());
    let client = Client::new();
    let response = client
        .post(&uri)
        .header("content-type", "application/json")
        .header("User-Agent", "rust-test")
        .body(serde_json::to_string(&TransferItem { number: 5 }).unwrap())
        .send()
        .unwrap();

    m.assert();
    assert_eq!(response.status(), 200);
}
