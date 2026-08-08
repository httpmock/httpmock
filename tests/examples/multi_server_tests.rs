use httpmock::prelude::*;
use reqwest::{blocking::Client, redirect::Policy};

#[test]
fn multi_server_test() {
    let server1 = MockServer::start();
    let server2 = MockServer::start();

    let redirect_mock = server1.mock(|when, then| {
        when.path("/redirectTest");
        then.status(302)
            .body("Found")
            .header("Location", server2.url("/finalTarget"));
    });

    let target_mock = server2.mock(|when, then| {
        when.path("/finalTarget");
        then.status(200);
    });

    let client = Client::builder().redirect(Policy::limited(10)).build().unwrap();

    let response = client.get(server1.url("/redirectTest")).send().unwrap();

    redirect_mock.assert();
    target_mock.assert();
    assert_eq!(response.status(), 200);
}
