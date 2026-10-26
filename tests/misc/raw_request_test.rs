use std::{
    io::{Read, Write},
    net::TcpStream,
};

use httpmock::MockServer;

#[test]
fn request_without_host_header_is_a_bad_request() {
    let server = MockServer::start();

    let mut stream = TcpStream::connect(server.address()).unwrap();
    stream.write_all(b"GET /hello HTTP/1.0\r\n\r\n").unwrap();
    let mut response = String::new();
    stream.read_to_string(&mut response).unwrap();

    let status_line = response.lines().next().unwrap_or_default();
    assert!(status_line.contains(" 400 "), "{response}");
}
