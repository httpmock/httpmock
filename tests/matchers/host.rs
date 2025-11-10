#![cfg(feature = "proxy")]

use crate::matchers::expect_fails_with2;
use httpmock::{MockServer, When};

#[test]
fn path_success_table_test() {
    struct TestData {
        expectation: fn(when: When) -> When,
    }

    let tests = [
        TestData {
            expectation: |when| when.host("127.0.0.1"),
        },
        TestData {
            expectation: |when| when.host("localhost"),
        },
        TestData {
            expectation: |when| when.host("LOCALHOST"),
        },
        TestData {
            expectation: |when| when.host_not("127.0.0.2"),
        },
        TestData {
            expectation: |when| when.host_includes("7.0.0"),
        },
        TestData {
            expectation: |when| when.host_excludes("28.0.0"),
        },
        TestData {
            expectation: |when| when.host_prefix("127"),
        },
        TestData {
            expectation: |when| when.host_prefix_not("128"),
        },
        TestData {
            expectation: |when| when.host_suffix(".0.1"),
        },
        TestData {
            expectation: |when| when.host_suffix_not("0.0.2"),
        },
        TestData {
            expectation: |when| when.host_matches(".*27.*"),
        },
    ];

    for (idx, test_data) in tests.iter().enumerate() {
        println!("Running test case with index '{idx}'");

        let target_server = MockServer::start();
        target_server.mock(|when, then| {
            when.any_request();
            then.status(200);
        });

        let proxy_server = MockServer::start();

        proxy_server.proxy(|rule| {
            rule.filter(|when| {
                (test_data.expectation)(when).port(target_server.port());
            });
        });

        let client = reqwest::blocking::Client::builder()
            .proxy(reqwest::Proxy::all(proxy_server.base_url()).unwrap())
            .build()
            .unwrap();

        let response = client.get(target_server.url("/get")).send().unwrap();
        assert_eq!(response.status(), 200);
    }
}

#[test]
fn path_failure_table_test() {
    pub struct TestData {
        expectation: fn(when: When) -> When,
        failure_message: Vec<&'static str>,
    }

    let tests = vec![
        TestData {
            expectation: |when| when.host("127.0.0.2"),
            failure_message: vec!["No request has been received by the mock server"],
        },
        TestData {
            expectation: |when| when.host_not("127.0.0.1"),
            failure_message: vec!["No request has been received by the mock server"],
        },
        TestData {
            expectation: |when| when.host_includes("192"),
            failure_message: vec!["No request has been received by the mock server"],
        },
        TestData {
            expectation: |when| when.host_excludes("127"),
            failure_message: vec!["No request has been received by the mock server"],
        },
        TestData {
            expectation: |when| when.host_prefix("192"),
            failure_message: vec!["No request has been received by the mock server"],
        },
        TestData {
            expectation: |when| when.host_prefix_not("127"),
            failure_message: vec!["No request has been received by the mock server"],
        },
        TestData {
            expectation: |when| when.host_suffix("2"),
            failure_message: vec!["No request has been received by the mock server"],
        },
        TestData {
            expectation: |when| when.host_suffix_not("1"),
            failure_message: vec!["No request has been received by the mock server"],
        },
    ];

    for (idx, test_data) in tests.iter().enumerate() {
        println!("Running test case with index '{idx}'");

        let err_msg = test_data.failure_message.clone();

        expect_fails_with2(err_msg, || {
            let target_server = MockServer::start();
            let m = target_server.mock(|when, then| {
                when.any_request();
                then.status(200);
            });

            let proxy_server = MockServer::start();
            proxy_server.proxy(|rule| {
                rule.filter(|when| {
                    (test_data.expectation)(when).port(target_server.port());
                });
            });

            let client = reqwest::blocking::Client::builder()
                .proxy(reqwest::Proxy::all(proxy_server.base_url()).unwrap())
                .build()
                .unwrap();

            let response = client.get(target_server.url("/get")).send().unwrap();
            assert_eq!(404, response.status());

            m.assert();
        });
    }
}
