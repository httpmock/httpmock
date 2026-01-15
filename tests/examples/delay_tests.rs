use std::time::{Duration, SystemTime};

use httpmock::prelude::*;

#[test]
fn delay_test() {
    // Arrange
    let start_time = SystemTime::now();
    let delay = Duration::from_secs(3);

    let server = MockServer::start();

    let mock = server.mock(|when, then| {
        when.path("/delay");
        then.status(200).delay(delay);
    });

    // Act: Send the HTTP request using reqwest
    let response = reqwest::blocking::get(server.url("/delay")).unwrap();

    // Assert
    mock.assert();
    assert_eq!(response.status(), 200);
    assert!(start_time.elapsed().unwrap() > delay);
}
