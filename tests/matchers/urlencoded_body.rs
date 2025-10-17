use crate::matchers::{expect_fails_with2, MultiValueMatcherTestSet};
use httpmock::{MockServer, When};

#[test]
fn form_urlencoded_tuple() {
    for (idx, data) in generate_data().attribute.iter().enumerate() {
        run_test(
            format!("Running test case with index '{idx}' and test data: {data:?}"),
            |when| when.form_urlencoded_tuple(data.expect.0, data.expect.1),
            data.actual.clone(),
            data.failure_msg.clone(),
        )
    }
}

#[test]
fn form_urlencoded_tuple_not() {
    for (idx, data) in generate_data().attribute_not.iter().enumerate() {
        run_test(
            format!("Running test case with index '{idx}' and test data: {data:?}"),
            |when| when.form_urlencoded_tuple_not(data.expect.0, data.expect.1),
            data.actual.clone(),
            data.failure_msg.clone(),
        )
    }
}

#[test]

fn form_urlencoded_tuple_exists() {
    for (idx, data) in generate_data().attribute_exists.iter().enumerate() {
        run_test(
            format!("Running test case with index '{idx}' and test data: {data:?}"),
            |when| when.form_urlencoded_tuple_exists(data.expect),
            data.actual.clone(),
            data.failure_msg.clone(),
        )
    }
}

#[test]

fn form_urlencoded_tuple_missing() {
    for (idx, data) in generate_data().attribute_missing.iter().enumerate() {
        run_test(
            format!("Running test case with index '{idx}' and test data: {data:?}"),
            |when| when.form_urlencoded_tuple_missing(data.expect),
            data.actual.clone(),
            data.failure_msg.clone(),
        )
    }
}

#[test]

fn form_urlencoded_tuple_includes() {
    for (idx, data) in generate_data().attribute_includes.iter().enumerate() {
        run_test(
            format!("Running test case with index '{idx}' and test data: {data:?}"),
            |when| when.form_urlencoded_tuple_includes(data.expect.0, data.expect.1),
            data.actual.clone(),
            data.failure_msg.clone(),
        )
    }
}

#[test]

fn form_urlencoded_tuple_excludes() {
    for (idx, data) in generate_data().attribute_excludes.iter().enumerate() {
        run_test(
            format!("Running test case with index '{idx}' and test data: {data:?}"),
            |when| when.form_urlencoded_tuple_excludes(data.expect.0, data.expect.1),
            data.actual.clone(),
            data.failure_msg.clone(),
        )
    }
}

#[test]

fn form_urlencoded_tuple_prefix() {
    for (idx, data) in generate_data().attribute_prefix.iter().enumerate() {
        run_test(
            format!("Running test case with index '{idx}' and test data: {data:?}"),
            |when| when.form_urlencoded_tuple_prefix(data.expect.0, data.expect.1),
            data.actual.clone(),
            data.failure_msg.clone(),
        )
    }
}

#[test]

fn form_urlencoded_tuple_suffix() {
    for (idx, data) in generate_data().attribute_suffix.iter().enumerate() {
        run_test(
            format!("Running test case with index '{idx}' and test data: {data:?}"),
            |when| when.form_urlencoded_tuple_suffix(data.expect.0, data.expect.1),
            data.actual.clone(),
            data.failure_msg.clone(),
        )
    }
}

#[test]

fn form_urlencoded_tuple_prefix_not() {
    for (idx, data) in generate_data().attribute_prefix_not.iter().enumerate() {
        run_test(
            format!("Running test case with index '{idx}' and test data: {data:?}"),
            |when| when.form_urlencoded_tuple_prefix_not(data.expect.0, data.expect.1),
            data.actual.clone(),
            data.failure_msg.clone(),
        )
    }
}

#[test]

fn form_urlencoded_tuple_suffix_not() {
    for (idx, data) in generate_data().attribute_suffix_not.iter().enumerate() {
        run_test(
            format!("Running test case with index '{idx}' and test data: {data:?}"),
            |when| when.form_urlencoded_tuple_suffix_not(data.expect.0, data.expect.1),
            data.actual.clone(),
            data.failure_msg.clone(),
        )
    }
}

#[test]

fn form_urlencoded_tuple_matches() {
    for (idx, data) in generate_data().attribute_matches.iter().enumerate() {
        run_test(
            format!("Running test case with index '{idx}' and test data: {data:?}"),
            |when| when.form_urlencoded_tuple_matches(data.expect.0, data.expect.1),
            data.actual.clone(),
            data.failure_msg.clone(),
        )
    }
}

#[test]

fn form_urlencoded_tuple_count() {
    for (idx, data) in generate_data().attribute_count.iter().enumerate() {
        run_test(
            format!("Running test case with index '{idx}' and test data: {data:?}"),
            |when| when.form_urlencoded_tuple_count(data.expect.0, data.expect.1, data.expect.2),
            data.actual.clone(),
            data.failure_msg.clone(),
        )
    }
}

fn generate_data() -> MultiValueMatcherTestSet<&'static str, &'static str, usize, &'static str> {
    MultiValueMatcherTestSet::generate("form_urlencoded_tuple", "Form-urlencoded Body Mismatch")
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
        let mut params = form_urlencoded::Serializer::new(String::new());
        for (key, value) in actual {
            params.append_pair(key, value);
        }

        let response = reqwest::blocking::Client::new()
            .post(server.url("/test"))
            .body(params.finish())
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
