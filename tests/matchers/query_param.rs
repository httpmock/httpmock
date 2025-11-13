use crate::matchers::{expect_fails_with2, to_urlencoded_query_string, MultiValueMatcherTestSet};
use httpmock::{MockServer, When};

#[test]
fn query_param() {
    for (idx, data) in generate_data().attribute.iter().enumerate() {
        run_test(
            format!("Running test case with index '{idx}' and test data: {data:?}"),
            |when| when.query_param(data.expect.0, data.expect.1),
            data.actual.clone(),
            data.failure_msg.clone(),
        )
    }
}

#[test]
fn query_param_not() {
    for (idx, data) in generate_data().attribute_not.iter().enumerate() {
        run_test(
            format!("Running test case with index '{idx}' and test data: {data:?}"),
            |when| when.query_param_not(data.expect.0, data.expect.1),
            data.actual.clone(),
            data.failure_msg.clone(),
        )
    }
}

#[test]
fn query_param_exists() {
    for (idx, data) in generate_data().attribute_exists.iter().enumerate() {
        run_test(
            format!("Running test case with index '{idx}' and test data: {data:?}"),
            |when| when.query_param_exists(data.expect),
            data.actual.clone(),
            data.failure_msg.clone(),
        )
    }
}

#[test]
fn query_param_missing() {
    for (idx, data) in generate_data().attribute_missing.iter().enumerate() {
        run_test(
            format!("Running test case with index '{idx}' and test data: {data:?}"),
            |when| when.query_param_missing(data.expect),
            data.actual.clone(),
            data.failure_msg.clone(),
        )
    }
}

#[test]
fn query_param_includes() {
    for (idx, data) in generate_data().attribute_includes.iter().enumerate() {
        run_test(
            format!("Running test case with index '{idx}' and test data: {data:?}"),
            |when| when.query_param_includes(data.expect.0, data.expect.1),
            data.actual.clone(),
            data.failure_msg.clone(),
        )
    }
}

#[test]
fn query_param_excludes() {
    for (idx, data) in generate_data().attribute_excludes.iter().enumerate() {
        run_test(
            format!("Running test case with index '{idx}' and test data: {data:?}"),
            |when| when.query_param_excludes(data.expect.0, data.expect.1),
            data.actual.clone(),
            data.failure_msg.clone(),
        )
    }
}

#[test]
fn query_param_prefix() {
    for (idx, data) in generate_data().attribute_prefix.iter().enumerate() {
        run_test(
            format!("Running test case with index '{idx}' and test data: {data:?}"),
            |when| when.query_param_prefix(data.expect.0, data.expect.1),
            data.actual.clone(),
            data.failure_msg.clone(),
        )
    }
}

#[test]
fn query_param_suffix() {
    for (idx, data) in generate_data().attribute_suffix.iter().enumerate() {
        run_test(
            format!("Running test case with index '{idx}' and test data: {data:?}"),
            |when| when.query_param_suffix(data.expect.0, data.expect.1),
            data.actual.clone(),
            data.failure_msg.clone(),
        )
    }
}

#[test]
fn query_param_prefix_not() {
    for (idx, data) in generate_data().attribute_prefix_not.iter().enumerate() {
        run_test(
            format!("Running test case with index '{idx}' and test data: {data:?}"),
            |when| when.query_param_prefix_not(data.expect.0, data.expect.1),
            data.actual.clone(),
            data.failure_msg.clone(),
        )
    }
}

#[test]
fn query_param_suffix_not() {
    for (idx, data) in generate_data().attribute_suffix_not.iter().enumerate() {
        run_test(
            format!("Running test case with index '{idx}' and test data: {data:?}"),
            |when| when.query_param_suffix_not(data.expect.0, data.expect.1),
            data.actual.clone(),
            data.failure_msg.clone(),
        )
    }
}

#[test]
fn query_param_matches() {
    for (idx, data) in generate_data().attribute_matches.iter().enumerate() {
        run_test(
            format!("Running test case with index '{idx}' and test data: {data:?}"),
            |when| when.query_param_matches(data.expect.0, data.expect.1),
            data.actual.clone(),
            data.failure_msg.clone(),
        )
    }
}

#[test]
fn query_param_count() {
    for (idx, data) in generate_data().attribute_count.iter().enumerate() {
        run_test(
            format!("Running test case with index '{idx}' and test data: {data:?}"),
            |when| when.query_param_count(data.expect.0, data.expect.1, data.expect.2),
            data.actual.clone(),
            data.failure_msg.clone(),
        )
    }
}

fn generate_data() -> MultiValueMatcherTestSet<&'static str, &'static str, usize, &'static str> {
    MultiValueMatcherTestSet::generate("query_param", "Query Parameter Mismatch")
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
        let url = server.url(format!("/test?{}", to_urlencoded_query_string(actual)));
        let response = reqwest::blocking::get(&url).unwrap();

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
