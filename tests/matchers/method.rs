use httpmock::{
    Method::{GET, POST},
    MockServer,
};
use reqwest::blocking::get;

use crate::matchers::expect_fails_with;

#[test]
fn success_method() {
    // Arrange
    let server = MockServer::start();

    let m = server.mock(|when, then| {
        when.method(GET);
        then.status(200);
    });

    // Act
    let response = get(server.base_url()).unwrap();

    // Assert
    m.assert();
    assert_eq!(response.status(), 200);
}

#[test]
fn failure_method() {
    expect_fails_with(
        || {
            // Arrange
            let server = MockServer::start();

            let m = server.mock(|when, then| {
                when.method(POST);
                then.status(200);
            });

            // Act
            get(server.base_url()).unwrap();

            m.assert()
        },
        vec![
            "Method Mismatch",
            "Expected method equals",
            "POST",
            "Received",
            "GET",
        ],
    )
}

#[test]
fn success_method_not() {
    // Arrange
    let server = MockServer::start();

    let m = server.mock(|when, then| {
        when.method_not(POST);
        then.status(200);
    });

    // Act
    let response = get(server.base_url()).unwrap();

    // Assert
    m.assert();
    assert_eq!(response.status(), 200);
}

#[test]
fn failure_method_not() {
    expect_fails_with(
        || {
            // Arrange
            let server = MockServer::start();

            let m = server.mock(|when, then| {
                when.method_not(GET);
                then.status(200);
            });

            // Act
            get(server.base_url()).unwrap();

            m.assert()
        },
        vec![
            "Method Mismatch",
            "Expected method not equal to",
            "GET",
            "Received",
            "GET",
        ],
    )
}
