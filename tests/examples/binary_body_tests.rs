use std::io::Read;

use httpmock::prelude::*;

#[test]
fn binary_body_test() {
    // Arrange
    let binary_content = b"\x80\x02\x03";

    let server = MockServer::start();

    let m = server.mock(|when, then| {
        when.method("GET").path("/hello");
        then.status(200).body(binary_content);
    });

    // Act
    let mut response = reqwest::blocking::get(server.url("/hello")).unwrap();

    // Assert
    m.assert();
    assert_eq!(response.status(), 200);
    assert_eq!(body_to_vec(&mut response), binary_content.to_vec());
}

fn body_to_vec(response: &mut reqwest::blocking::Response) -> Vec<u8> {
    let mut buf: Vec<u8> = Vec::new();
    response
        .read_to_end(&mut buf)
        .expect("Cannot read from body");
    buf
}
