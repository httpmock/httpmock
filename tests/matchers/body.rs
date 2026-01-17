use httpmock::{MockServer, When};

use crate::matchers::{expect_fails_with2, SingleValueMatcherDataSet};

#[test]
fn body() {
    for (idx, data) in generate_data().attribute.iter().enumerate() {
        run_test(
            format!("Running test case with index '{idx}' and test data: {data:?}"),
            |when| when.body(data.expect),
            data.actual,
            data.failure_msg.clone(),
        )
    }
}

#[test]
fn body_fail_message() {
    run_test(
        "fail message format",
        |when| when.body("test"),
        "not-test",
        Some(vec![
            "Expected body equals:",
            "test",
            "",
            "Received:",
            "not-test",
            "",
            "Diff:",
            "---| test",
            "+++| not-test",
        ]),
    )
}

#[test]
fn body_not() {
    for (idx, data) in generate_data().attribute_not.iter().enumerate() {
        run_test(
            format!("Running test case with index '{idx}' and test data: {data:?}"),
            |when| when.body_not(data.expect),
            data.actual,
            data.failure_msg.clone(),
        )
    }
}

#[test]
fn body_not_fail_message() {
    run_test(
        "fail message format",
        |when| when.body_not("test"),
        "test",
        Some(vec![
            "Expected body not equal to:",
            "test",
            "",
            "Received:",
            "test",
            "",
            "Diff:",
            "   | test",
            "",
            "Matcher:  body_not",
        ]),
    )
}

#[test]
fn body_includes() {
    for (idx, data) in generate_data().attribute_includes.iter().enumerate() {
        run_test(
            format!("Running test case with index '{idx}' and test data: {data:?}"),
            |when| when.body_includes(data.expect),
            data.actual,
            data.failure_msg.clone(),
        )
    }
}

#[test]
fn body_includes_fail_message() {
    run_test(
        "fail message format",
        |when| when.body_includes("x"),
        "a\n. \n test \n abc \nline",
        Some(vec![
            "Expected body includes:",
            "x",
            "",
            "Received:",
            "a",
            ".",
            " test",
            " abc",
            "line",
            "",
            "Diff:",
            "---| x",
            "+++| a",
            "+++| .",
            "+++|  test",
            "+++|  abc",
            "+++| line",
        ]),
    )
}

#[test]
fn body_includes_multiline() {
    let expect = "\"onclick\": \"CreateDoc()\",\n                    \"value\": \"New\"";
    let actual = r#"
{
    "menu": {
        "id": "file",
        "popup": {
            "menuitem": [
                {
                    "onclick": "CreateDoc()",
                    "value": "New"
                },
                {
                    "onclick": "OpenDoc()",
                    "value": "Open"
                },
                {
                    "onclick": "SaveDoc()",
                    "value": "Save"
                }
            ]
        },
        "value": "File"
    }
}
"#;

    run_test(
        "multi-line body",
        |when| when.body_includes(expect),
        actual,
        None,
    );
}

#[test]
fn body_excludes() {
    for (idx, data) in generate_data().attribute_excludes.iter().enumerate() {
        run_test(
            format!("Running test case with index '{idx}' and test data: {data:?}"),
            |when| when.body_excludes(data.expect),
            data.actual,
            data.failure_msg.clone(),
        )
    }
}

#[test]
fn body_excludes_fail_message() {
    run_test(
        "fail message format",
        |when| when.body_excludes("test"),
        "a\n. \n test \n abc \nline",
        Some(vec![
            "Expected body excludes:",
            "test",
            "",
            "Received:",
            "a",
            ".",
            " test",
            " abc",
            "line",
            "",
            "Diff:",
            "---| test",
            "+++| a",
            "+++| .",
            "+++|  test",
            "+++|  abc",
            "+++| line",
        ]),
    )
}

#[test]
fn body_prefix() {
    for (idx, data) in generate_data().attribute_prefix.iter().enumerate() {
        run_test(
            format!("Running test case with index '{idx}' and test data: {data:?}"),
            |when| when.body_prefix(data.expect),
            data.actual,
            data.failure_msg.clone(),
        )
    }
}

#[test]
fn body_prefix_fail_message() {
    run_test(
        "fail message format",
        |when| when.body_prefix("test"),
        "a test",
        Some(vec![
            "Expected body has prefix:",
            "test",
            "",
            "Received:",
            "a test",
            "",
            "Diff:",
            "---| test",
            "+++| a test",
            "",
            "Matcher:  body_prefix",
        ]),
    )
}

#[test]
fn body_prefix_not() {
    for (idx, data) in generate_data().attribute_prefix_not.iter().enumerate() {
        run_test(
            format!("Running test case with index '{idx}' and test data: {data:?}"),
            |when| when.body_prefix_not(data.expect),
            data.actual,
            data.failure_msg.clone(),
        )
    }
}

#[test]
fn body_prefix_not_fail_message() {
    run_test(
        "fail message format",
        |when| when.body_prefix_not("test"),
        "test it is",
        Some(vec![
            "Expected body prefix not:",
            "test",
            "",
            "Received:",
            "test it is",
            "",
            "Diff:",
            "---| test",
            "+++| test it is",
            "",
            "Matcher:  body_prefix_not",
        ]),
    )
}

#[test]
fn body_suffix() {
    for (idx, data) in generate_data().attribute_suffix.iter().enumerate() {
        run_test(
            format!("Running test case with index '{idx}' and test data: {data:?}"),
            |when| when.body_suffix(data.expect),
            data.actual,
            data.failure_msg.clone(),
        )
    }
}

#[test]
fn body_suffix_fail_message() {
    run_test(
        "fail message format",
        |when| when.body_suffix("test"),
        "it is test not",
        Some(vec![
            "Expected body has suffix:",
            "test",
            "",
            "Received:",
            "it is test not",
            "",
            "Diff:",
            "---| test",
            "+++| it is test not",
        ]),
    )
}

#[test]
fn body_suffix_not() {
    for (idx, data) in generate_data().attribute_suffix_not.iter().enumerate() {
        run_test(
            format!("Running test case with index '{idx}' and test data: {data:?}"),
            |when| when.body_suffix_not(data.expect),
            data.actual,
            data.failure_msg.clone(),
        )
    }
}

#[test]
fn body_suffix_not_fail_message() {
    run_test(
        "fail message format",
        |when| when.body_suffix_not("test"),
        "it is test",
        Some(vec![
            "Expected body suffix not:",
            "test",
            "",
            "Received:",
            "it is test",
            "",
            "Diff:",
            "---| test",
            "+++| it is test",
        ]),
    )
}

#[test]
fn body_matches() {
    for (idx, data) in generate_data().attribute_matches.iter().enumerate() {
        run_test(
            format!("Running test case with index '{idx}' and test data: {data:?}"),
            |when| when.body_matches(data.expect),
            data.actual,
            data.failure_msg.clone(),
        )
    }
}

#[test]
fn body_matches_fail_message() {
    run_test(
        "fail message format",
        |when| when.body_matches("def"),
        "abcghijklmn",
        Some(vec![
            "Expected body matches regex:",
            "def",
            "",
            "Received:",
            "abcghijklmn",
            "",
            "Diff:",
            "---| def",
            "+++| abcghijklmn",
            "",
            "Matcher:  body_matches",
        ]),
    )
}

fn generate_data() -> SingleValueMatcherDataSet<&'static str, &'static str> {
    SingleValueMatcherDataSet::generate("body", "Body Mismatch")
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
            .get(server.url("/test"))
            .body(actual)
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
