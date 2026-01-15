use httpmock::{MockServer, When};

use crate::matchers::{expect_fails_with2, MultiValueMatcherTestSet};

#[test]
fn header() {
    for (idx, data) in generate_data().attribute.iter().enumerate() {
        run_test(
            format!("Running test case with index '{idx}' and test data: {data:?}"),
            |when| when.header(data.expect.0, data.expect.1),
            data.actual.clone(),
            data.failure_msg.clone(),
        )
    }
}

#[test]
fn header_not() {
    for (idx, data) in generate_data().attribute_not.iter().enumerate() {
        run_test(
            format!("Running test case with index '{idx}' and test data: {data:?}"),
            |when| when.header_not(data.expect.0, data.expect.1),
            data.actual.clone(),
            data.failure_msg.clone(),
        )
    }
}

#[test]
fn header_exists() {
    for (idx, data) in generate_data().attribute_exists.iter().enumerate() {
        run_test(
            format!("Running test case with index '{idx}' and test data: {data:?}"),
            |when| when.header_exists(data.expect),
            data.actual.clone(),
            data.failure_msg.clone(),
        )
    }
}

#[test]
fn header_missing() {
    for (idx, data) in generate_data().attribute_missing.iter().enumerate() {
        run_test(
            format!("Running test case with index '{idx}' and test data: {data:?}"),
            |when| when.header_missing(data.expect),
            data.actual.clone(),
            data.failure_msg.clone(),
        )
    }
}

#[test]
fn header_includes() {
    for (idx, data) in generate_data().attribute_includes.iter().enumerate() {
        run_test(
            format!("Running test case with index '{idx}' and test data: {data:?}"),
            |when| when.header_includes(data.expect.0, data.expect.1),
            data.actual.clone(),
            data.failure_msg.clone(),
        )
    }
}

#[test]
fn header_excludes() {
    for (idx, data) in generate_data().attribute_excludes.iter().enumerate() {
        run_test(
            format!("Running test case with index '{idx}' and test data: {data:?}"),
            |when| when.header_excludes(data.expect.0, data.expect.1),
            data.actual.clone(),
            data.failure_msg.clone(),
        )
    }
}

#[test]
fn header_prefix() {
    for (idx, data) in generate_data().attribute_prefix.iter().enumerate() {
        run_test(
            format!("Running test case with index '{idx}' and test data: {data:?}"),
            |when| when.header_prefix(data.expect.0, data.expect.1),
            data.actual.clone(),
            data.failure_msg.clone(),
        )
    }
}

#[test]
fn header_suffix() {
    for (idx, data) in generate_data().attribute_suffix.iter().enumerate() {
        run_test(
            format!("Running test case with index '{idx}' and test data: {data:?}"),
            |when| when.header_suffix(data.expect.0, data.expect.1),
            data.actual.clone(),
            data.failure_msg.clone(),
        )
    }
}

#[test]
fn header_prefix_not() {
    for (idx, data) in generate_data().attribute_prefix_not.iter().enumerate() {
        run_test(
            format!("Running test case with index '{idx}' and test data: {data:?}"),
            |when| when.header_prefix_not(data.expect.0, data.expect.1),
            data.actual.clone(),
            data.failure_msg.clone(),
        )
    }
}

#[test]
fn header_suffix_not() {
    for (idx, data) in generate_data().attribute_suffix_not.iter().enumerate() {
        run_test(
            format!("Running test case with index '{idx}' and test data: {data:?}"),
            |when| when.header_suffix_not(data.expect.0, data.expect.1),
            data.actual.clone(),
            data.failure_msg.clone(),
        )
    }
}

#[test]
fn header_matches() {
    for (idx, data) in generate_data().attribute_matches.iter().enumerate() {
        run_test(
            format!("Running test case with index '{idx}' and test data: {data:?}"),
            |when| when.header_matches(data.expect.0, data.expect.1),
            data.actual.clone(),
            data.failure_msg.clone(),
        )
    }
}

#[test]
fn header_count() {
    for (idx, data) in generate_data().attribute_count.iter().enumerate() {
        run_test(
            format!("Running test case with index '{idx}' and test data: {data:?}"),
            |when| when.header_count(data.expect.0, data.expect.1, data.expect.2),
            data.actual.clone(),
            data.failure_msg.clone(),
        )
    }
}

fn generate_data() -> MultiValueMatcherTestSet<&'static str, &'static str, usize, &'static str> {
    MultiValueMatcherTestSet::generate("header", "Header Mismatch")
}

fn run_test<F, S>(
    name: S,
    set_expectation: F,
    actual: Vec<(&'static str, &'static str)>,
    error_msg: Option<Vec<&'static str>>,
) where
    F: Fn(When) -> When + std::panic::UnwindSafe + std::panic::RefUnwindSafe,
    S: Into<String>,
{
    println!("{}", name.into());

    let run = || {
        // Arrange
        let server = MockServer::start();

        let m = server.mock(|when, then| {
            set_expectation(when);
            then.status(200);
        });

        // Act
        // Build the request with custom headers
        let client = reqwest::blocking::Client::new();
        let mut request_builder = client.get(server.url("/test"));

        for (key, value) in actual {
            request_builder = request_builder.header(key, value);
        }

        let response = request_builder.send().unwrap();

        // Assert
        m.assert();
        assert_eq!(response.status(), 200);
    };

    if let Some(err_msg) = error_msg {
        expect_fails_with2(err_msg, run);
    } else {
        run();
    }
}
