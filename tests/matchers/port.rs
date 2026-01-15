use httpmock::MockServer;
use reqwest::blocking::get;

use crate::matchers::expect_fails_with;

#[test]
fn host_tests() {
    // Arrange
    let server = MockServer::start();
    let port = server.port();

    let m = server.mock(|when, then| {
        when.port(port);
        then.status(200);
    });

    // Act
    let response = get(server.base_url()).unwrap();

    // Assert
    m.assert();
    assert_eq!(response.status(), 200);
}

#[test]
fn host_failure() {
    expect_fails_with(
        || {
            // Arrange
            let server = MockServer::start();

            let m = server.mock(|when, then| {
                when.port(0); // explicitly matching against port 0 will always fail
                then.status(200);
            });

            // Act
            get(server.base_url()).unwrap();

            m.assert()
        },
        vec!["Port Mismatch", "Expected port equals", "0", "Received"],
    )
}

#[test]
fn host_not_success_name() {
    // Arrange
    let server = MockServer::start();

    let m = server.mock(|when, then| {
        when.port_not(0); // Port is never 0, so this will always match.
        then.status(200);
    });

    // Act
    let response = get(server.base_url()).unwrap();

    // Assert
    m.assert();
    assert_eq!(response.status(), 200);
}

#[test]
fn host_not_failure() {
    expect_fails_with(
        || {
            // Arrange
            let server = MockServer::start();

            let m = server.mock(|when, then| {
                when.host_not("127.0.0.1");
                then.status(200);
            });

            // Act
            get(server.base_url()).unwrap();

            m.assert()
        },
        vec![
            "Host Mismatch",
            "Expected host not equal to",
            "127.0.0.1",
            "Received",
            "127.0.0.1",
        ],
    );

    expect_fails_with(
        || {
            // Arrange
            let server = MockServer::start();

            let m = server.mock(|when, then| {
                when.host_not("localhost");
                then.status(200);
            });

            // Act
            get(server.base_url()).unwrap();

            m.assert()
        },
        vec![
            "Host Mismatch",
            "Expected host not equal to",
            "localhost",
            "Received",
            "127.0.0.1",
        ],
    );
}
