use httpmock::prelude::*;

#[test]
fn url_param_matching_test() {
    // Arrange
    let server = MockServer::start();

    let m = server.mock(|when, then| {
        when.query_param("query", "Metallica").query_param_exists("query");
        then.status(200);
    });

    // Act: Send the request using the fully qualified path
    reqwest::blocking::get(server.url("/search?query=Metallica")).unwrap();

    // Assert
    m.assert();
}

#[test]
fn url_param_urlencoded_matching_test() {
    // Arrange
    let server = MockServer::start();

    let m = server.mock(|when, then| {
        when.query_param("query", "Motörhead").query_param_exists("query");
        then.status(200);
    });

    // Act: Send the request using the fully qualified path
    reqwest::blocking::get(server.url("/search?query=Mot%C3%B6rhead")).unwrap();

    // Assert
    m.assert();
}

#[test]
fn url_param_unencoded_matching_test() {
    // Arrange
    let server = MockServer::start();

    let m = server.mock(|when, then| {
        when.query_param("query", "Motörhead").query_param_exists("query");
        then.status(200);
    });

    // Act: Send the request using the fully qualified path
    reqwest::blocking::get(server.url("/search?query=Motörhead")).unwrap();

    // Assert
    m.assert();
}

#[test]
fn url_param_encoding_issue_56() {
    // Arrange
    let server = MockServer::start();

    let m = server.mock(|when, then| {
        when.query_param("query", "Metallica is cool");
        then.status(200);
    });

    // Act: Send the request using the fully qualified path
    reqwest::blocking::get(server.url("/search?query=Metallica+is+cool")).unwrap();

    // Assert
    m.assert();
}
