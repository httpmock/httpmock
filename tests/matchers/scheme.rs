use httpmock::MockServer;
use reqwest::blocking::get;

use crate::matchers::expect_fails_with;

#[test]
fn scheme_test() {
    // Arrange
    let server = MockServer::start();

    #[cfg(feature = "https")]
    let scheme = "https";
    #[cfg(not(feature = "https"))]
    let scheme = "http";

    let m = server.mock(|when, then| {
        when.scheme(scheme);
        then.status(200);
    });

    // Act
    let response = get(server.base_url()).unwrap();

    // Assert
    m.assert();
    assert_eq!(response.status(), 200);
}

#[test]
fn scheme_failure() {
    #[cfg(feature = "https")]
    let expected_scheme = "http";
    #[cfg(feature = "https")]
    let actual_scheme = "https";
    #[cfg(not(feature = "https"))]
    let expected_scheme = "https";
    #[cfg(not(feature = "https"))]
    let actual_scheme = "http";

    expect_fails_with(
        || {
            // Arrange
            let server = MockServer::start();
            let m = server.mock(|when, then| {
                when.scheme(expected_scheme);
                then.status(200);
            });

            // Act
            get(server.base_url()).unwrap();

            m.assert()
        },
        vec![
            "Scheme Mismatch",
            "Expected",
            "scheme equals",
            expected_scheme,
            "Received",
            actual_scheme,
        ],
    )
}

#[test]
fn scheme_not_test() {
    // Arrange
    let server = MockServer::start();

    #[cfg(feature = "https")]
    let scheme = "http";
    #[cfg(not(feature = "https"))]
    let scheme = "https";

    let m = server.mock(|when, then| {
        when.scheme_not(scheme);
        then.status(200);
    });

    // Act
    let response = get(server.base_url()).unwrap();

    // Assert
    m.assert();
    assert_eq!(response.status(), 200);
}

#[test]
fn scheme_not_failure() {
    #[cfg(feature = "https")]
    let expected_scheme = "https";
    #[cfg(feature = "https")]
    let actual_scheme = "http";
    #[cfg(not(feature = "https"))]
    let expected_scheme = "http";
    #[cfg(not(feature = "https"))]
    let actual_scheme = "https";

    expect_fails_with(
        || {
            // Arrange
            let server = MockServer::start();

            let m = server.mock(|when, then| {
                when.scheme_not(expected_scheme);
                then.status(200);
            });

            // Act
            get(server.base_url()).unwrap();

            m.assert()
        },
        vec![
            "Scheme Mismatch",
            "Expected",
            "scheme not equal to",
            expected_scheme,
            "Received",
            actual_scheme,
        ],
    )
}
