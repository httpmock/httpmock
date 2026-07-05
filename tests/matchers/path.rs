use httpmock::{MockServer, When};

use crate::matchers::{expect_fails_with2, SingleValueMatcherDataSet};

#[test]
fn path() {
    for (idx, data) in generate_data().attribute.iter().enumerate() {
        run_test(
            format!("Running test case with index '{idx}' and test data: {data:?}"),
            |when| when.path(format!("/{}", data.expect)),
            data.actual,
            data.failure_msg.clone(),
        )
    }
}

#[test]
fn path_not() {
    for (idx, data) in generate_data().attribute_not.iter().enumerate() {
        run_test(
            format!("Running test case with index '{idx}' and test data: {data:?}"),
            |when| when.path_not(format!("/{}", data.expect)),
            data.actual,
            data.failure_msg.clone(),
        )
    }
}

#[test]
fn path_includes() {
    for (idx, data) in generate_data().attribute_includes.iter().enumerate() {
        run_test(
            format!("Running test case with index '{idx}' and test data: {data:?}"),
            |when| when.path_includes(data.expect),
            data.actual,
            data.failure_msg.clone(),
        )
    }
}

#[test]
fn path_excludes() {
    for (idx, data) in generate_data().attribute_excludes.iter().enumerate() {
        run_test(
            format!("Running test case with index '{idx}' and test data: {data:?}"),
            |when| when.path_excludes(data.expect),
            data.actual,
            data.failure_msg.clone(),
        )
    }
}

#[test]
fn path_prefix() {
    for (idx, data) in generate_data().attribute_prefix.iter().enumerate() {
        run_test(
            format!("Running test case with index '{idx}' and test data: {data:?}"),
            |when| when.path_prefix(format!("/{}", data.expect)),
            data.actual,
            data.failure_msg.clone(),
        )
    }
}

#[test]
fn path_prefix_not() {
    for (idx, data) in generate_data().attribute_prefix_not.iter().enumerate() {
        run_test(
            format!("Running test case with index '{idx}' and test data: {data:?}"),
            |when| when.path_prefix_not(format!("/{}", data.expect)),
            data.actual,
            data.failure_msg.clone(),
        )
    }
}

#[test]
fn path_suffix() {
    for (idx, data) in generate_data().attribute_suffix.iter().enumerate() {
        run_test(
            format!("Running test case with index '{idx}' and test data: {data:?}"),
            |when| when.path_suffix(data.expect),
            data.actual,
            data.failure_msg.clone(),
        )
    }
}

#[test]
fn path_suffix_not() {
    for (idx, data) in generate_data().attribute_suffix_not.iter().enumerate() {
        run_test(
            format!("Running test case with index '{idx}' and test data: {data:?}"),
            |when| when.path_suffix_not(data.expect),
            data.actual,
            data.failure_msg.clone(),
        )
    }
}

#[test]
fn path_matches() {
    for (idx, data) in generate_data().attribute_matches.iter().enumerate() {
        run_test(
            format!("Running test case with index '{idx}' and test data: {data:?}"),
            |when| when.path_matches(data.expect),
            data.actual,
            data.failure_msg.clone(),
        )
    }
}

fn generate_data() -> SingleValueMatcherDataSet<&'static str, &'static str> {
    SingleValueMatcherDataSet::generate("path", "Path Mismatch")
}

fn run_test<F, S>(
    name: S,
    set_expectation: F,
    actual: &'static str,
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
        let response = reqwest::blocking::Client::new()
            .get(server.url(format!("/{actual}")))
            .send()
            .unwrap();

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
