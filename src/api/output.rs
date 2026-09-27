use std::fmt::Write;

#[cfg(feature = "color")]
use colored::Colorize;

use crate::{
    common::{
        data::{
            ClosestMatch, Diff, DiffResult, FunctionComparison, KeyValueComparison, KeyValueComparisonKeyValuePair,
            Mismatch, SingleValueComparison,
        },
        util::title_case,
    },
    server::matchers::generic::MatchingStrategy,
};

const QUOTED_TEXT: &str = "quoted for better readability";

/// Indentation of the lines listed under a heading.
const INDENT: &str = "    ";

pub fn fail_with(actual_hits: usize, expected_hits: usize, closest_match: Option<ClosestMatch>) {
    let closest_match = closest_match.expect("No request has been received by the mock server.");
    let mut output = String::new();
    output.push_str(&format!(
        "{} of {} expected requests matched the mock specification.\n",
        actual_hits, expected_hits
    ));
    output.push_str(&format!(
        "Here is a comparison with the most similar unmatched request (request number {}): \n\n",
        closest_match.request_index + 1
    ));

    let mut fail_text = None;

    for (idx, mm) in closest_match.mismatches.iter().enumerate() {
        let (mm_output, fail_text_pair) = create_mismatch_output(idx, mm);

        if fail_text.is_none()
            && let Some(text) = fail_text_pair
        {
            fail_text = Some(text)
        }

        output.push_str(&mm_output);
    }

    if let Some((left, right)) = fail_text {
        assert_eq!(left, right, "{}", output)
    }

    panic!("{}", output)
}

pub fn create_mismatch_output(idx: usize, mismatch: &Mismatch) -> (String, Option<(String, String)>) {
    let mut output = String::new();
    let mut ide_diff_left = String::new();
    let mut ide_diff_right = String::new();

    write_header(&mut output, idx, mismatch);

    if let Some(comparison) = &mismatch.comparison {
        let (left, right) = handle_single_value_comparison(&mut output, mismatch, comparison);

        ide_diff_left.push_str(&left);
        ide_diff_right.push_str(&right);
    } else if let Some(comparison) = &mismatch.key_value_comparison {
        let (left, right) = handle_key_value_comparison(&mut output, mismatch, comparison);

        ide_diff_left.push_str(&left);
        ide_diff_right.push_str(&right);
    } else if let Some(comparison) = &mismatch.function_comparison {
        handle_function_comparison(&mut output, mismatch, comparison);
    }

    write_footer(&mut output, mismatch);

    if !ide_diff_left.is_empty() && !ide_diff_right.is_empty() {
        return (output, Some((ide_diff_left, ide_diff_right)));
    }

    (output, None)
}

fn write_header(out: &mut String, idx: usize, mismatch: &Mismatch) {
    writeln!(out, "{}", "-".repeat(60)).unwrap();
    writeln!(out, "{} : {} Mismatch ", idx + 1, title_case(&mismatch.entity)).unwrap();
    writeln!(out, "{}", "-".repeat(60)).unwrap();
}

fn handle_single_value_comparison(
    out: &mut String,
    mismatch: &Mismatch,
    comparison: &SingleValueComparison,
) -> (String, String) {
    writeln!(
        out,
        "Expected {} {}:\n{}",
        mismatch.entity, comparison.operator, comparison.expected
    )
    .unwrap();

    writeln!(out, "\nReceived:\n{}", comparison.actual).unwrap();

    (comparison.expected.to_string(), comparison.actual.to_string())
}

fn handle_key_value_comparison(
    out: &mut String,
    mismatch: &Mismatch,
    comparison: &KeyValueComparison,
) -> (String, String) {
    let most_similar = match mismatch.best_match {
        true => format!(" (most similar {})", mismatch.entity),
        false => String::from(" "),
    };

    writeln!(out, "Expected:").unwrap();

    let expected_rows: Vec<(&str, String, String)> = [("key", &comparison.key), ("value", &comparison.value)]
        .into_iter()
        .filter_map(|(label, attribute)| {
            let attribute = attribute.as_ref()?;
            let expected = match quote_if_whitespace(&attribute.expected) {
                (expected, true) => format!("{} ({})", expected, QUOTED_TEXT),
                (expected, false) => expected,
            };
            Some((label, format!("[{}]", attribute.operator), expected))
        })
        .collect();

    // Align the label and operator columns of the expected rows.
    let label_width = expected_rows.iter().map(|(label, _, _)| label.len()).max().unwrap_or(0);
    let operator_width = expected_rows
        .iter()
        .map(|(_, operator, _)| operator.chars().count())
        .max()
        .unwrap_or(0);
    for (label, operator, expected) in &expected_rows {
        writeln!(
            out,
            "{INDENT}{label:<label_width$}  {operator:<operator_width$}  {expected}"
        )
        .unwrap();
    }

    if let (Some(expected_count), Some(actual_count)) = (comparison.expected_count, comparison.actual_count) {
        if comparison.key.is_none() && comparison.value.is_none() {
            writeln!(
                out,
                "\n{} to appear {} {} but appeared {}",
                mismatch.entity,
                expected_count,
                times_str(expected_count),
                actual_count
            )
            .unwrap();
        } else {
            writeln!(
                out,
                "\nto appear {} {} but appeared {}",
                expected_count,
                times_str(expected_count),
                actual_count
            )
            .unwrap();
        }

        print_all_request_values(out, &mismatch.entity, &comparison.all);

        return (expected_count.to_string(), actual_count.to_string());
    }

    if let (Some(key_attr), Some(value_attr)) = (&comparison.key, &comparison.value) {
        let result = match (&key_attr.actual, &value_attr.actual) {
            (Some(key), Some(value)) => {
                writeln!(out, "\nReceived{}:\n{INDENT}{}={}", most_similar, key, value).unwrap();
                (format!("{}\n{}", key, value), format!("{}\n{}", key, value))
            }
            (None, Some(value)) => {
                writeln!(
                    out,
                    "\nbut{}{} value was\n{INDENT}{}",
                    most_similar, mismatch.entity, value
                )
                .unwrap();
                (value.to_string(), value.to_string())
            }
            (Some(key), None) => {
                writeln!(out, "\nbut{}{} key was\n{INDENT}{}", most_similar, mismatch.entity, key).unwrap();
                (key.to_string(), key.to_string())
            }
            (None, None) => {
                let msg = match &mismatch.matching_strategy {
                    None => "but none was provided",
                    Some(v) => match v {
                        MatchingStrategy::Presence => "to be in the request, but none was provided.",
                        MatchingStrategy::Absence => "not to be present, but the request contained it.",
                    },
                };

                writeln!(out, "\n{}", msg).unwrap();
                (String::new(), String::new())
            }
        };

        // print_value_not_in_request(out, &mismatch.matching_strategy);
        print_all_request_values(out, &mismatch.entity, &comparison.all);

        return result;
    }

    print_value_not_in_request(out, &mismatch.matching_strategy);
    print_all_request_values(out, &mismatch.entity, &comparison.all);

    (String::new(), String::new())
}

fn print_all_request_values(out: &mut String, entity: &str, all: &[KeyValueComparisonKeyValuePair]) {
    if all.is_empty() {
        return;
    }

    writeln!(out, "\nAll received {} values:", entity).unwrap();

    for (index, pair) in all.iter().enumerate() {
        let value = if pair.value.is_some() {
            format!("={}", pair.value.clone().unwrap())
        } else {
            String::new()
        };

        let text = format!("{}{}", pair.key, value);
        writeln!(out, "{INDENT}{}. {}", index + 1, text).unwrap();
    }
}

fn print_value_not_in_request(out: &mut String, matching_strategy: &Option<MatchingStrategy>) {
    writeln!(
        out,
        "\n{}",
        match matching_strategy {
            None => "but none was provided",
            Some(v) => match v {
                MatchingStrategy::Presence => "to be in the request, but none was provided.",
                MatchingStrategy::Absence => "not to be present, but the request contained it.",
            },
        }
    )
    .unwrap();
}

fn handle_function_comparison(out: &mut String, mismatch: &Mismatch, comparison: &FunctionComparison) {
    writeln!(
        out,
        "Custom matcher function {} with index {} did not match the request",
        mismatch.matcher_method, comparison.index
    )
    .unwrap();
}

fn write_footer(out: &mut String, mismatch: &Mismatch) {
    let mut version = env!("CARGO_PKG_VERSION");
    if version.trim().is_empty() {
        version = "latest";
    }

    let link = format!(
        "https://docs.rs/httpmock/{}/httpmock/struct.When.html#method.{}",
        version, mismatch.matcher_method
    );

    writeln!(out).unwrap();

    if let Some(diff_result) = &mismatch.diff {
        writeln!(out, "{}", create_diff_result_output(diff_result)).unwrap();
        writeln!(out).unwrap();
    }

    writeln!(out, "{:<10}{}", "Matcher:", mismatch.matcher_method).unwrap();
    writeln!(out, "{:<10}{}", "Docs:", link).unwrap();
    writeln!(out, "\u{2002}").unwrap();
}

fn create_diff_result_output(dd: &DiffResult) -> String {
    let mut output = String::new();
    output.push_str("Diff:");
    if dd.differences.is_empty() {
        output.push_str("<empty>");
    }
    output.push('\n');

    dd.differences.iter().enumerate().for_each(|(idx, d)| {
        if idx > 0 {
            output.push('\n')
        }

        match d {
            Diff::Same(edit) => {
                for line in remove_trailing_linebreak(edit).split("\n") {
                    output.push_str(&format!("   | {}", line));
                }
            }
            Diff::Add(edit) => {
                for line in remove_trailing_linebreak(edit).split("\n") {
                    #[cfg(feature = "color")]
                    output.push_str(&format!("+++| {}", line).green().to_string());
                    #[cfg(not(feature = "color"))]
                    output.push_str(&format!("+++| {}", line));
                }
            }
            Diff::Rem(edit) => {
                for line in remove_trailing_linebreak(edit).split("\n") {
                    #[cfg(feature = "color")]
                    output.push_str(&format!("---| {}", line).red().to_string());
                    #[cfg(not(feature = "color"))]
                    output.push_str(&format!("---| {}", line));
                }
            }
        }
    });
    output
}

#[inline]
fn times_str<'a>(v: usize) -> &'a str {
    if v == 1 { "time" } else { "times" }
}

fn quote_if_whitespace(s: &str) -> (String, bool) {
    if s.is_empty() || s.starts_with(char::is_whitespace) || s.ends_with(char::is_whitespace) {
        (format!("\"{}\"", s), true)
    } else {
        (s.to_string(), false)
    }
}

fn remove_trailing_linebreak(s: &str) -> String {
    let mut result = s.to_string();
    if result.ends_with('\n') {
        result.pop();
        if result.ends_with('\r') {
            result.pop();
        }
    }
    result
}

#[cfg(test)]
mod test {
    use super::create_mismatch_output;
    use crate::{
        common::data::{
            Diff, DiffResult, KeyValueComparison, KeyValueComparisonAttribute, KeyValueComparisonKeyValuePair,
            Mismatch, SingleValueComparison, Tokenizer,
        },
        server::matchers::generic::MatchingStrategy,
    };

    fn docs_link(method: &str) -> String {
        format!(
            "https://docs.rs/httpmock/{}/httpmock/struct.When.html#method.{}",
            env!("CARGO_PKG_VERSION"),
            method
        )
    }

    fn attribute(operator: &str, expected: &str, actual: Option<&str>) -> KeyValueComparisonAttribute {
        KeyValueComparisonAttribute {
            operator: operator.to_string(),
            expected: expected.to_string(),
            actual: actual.map(str::to_string),
        }
    }

    fn pair(key: &str, value: &str) -> KeyValueComparisonKeyValuePair {
        KeyValueComparisonKeyValuePair {
            key: key.to_string(),
            value: Some(value.to_string()),
        }
    }

    fn key_value_mismatch(comparison: KeyValueComparison) -> Mismatch {
        Mismatch {
            entity: "header".to_string(),
            matcher_method: "header".to_string(),
            comparison: None,
            key_value_comparison: Some(comparison),
            function_comparison: None,
            matching_strategy: Some(MatchingStrategy::Presence),
            best_match: true,
            diff: None,
        }
    }

    #[test]
    fn key_value_mismatch_aligns_expected_rows() {
        let mismatch = key_value_mismatch(KeyValueComparison {
            key: Some(attribute("equals", "content-type", Some("content-type"))),
            value: Some(attribute(
                "equals_case_insensitive",
                "application/json",
                Some("text/plain"),
            )),
            expected_count: None,
            actual_count: None,
            all: vec![pair("content-type", "text/plain"), pair("accept", "*/*")],
        });

        let (output, _) = create_mismatch_output(0, &mismatch);

        let expected = [
            "-".repeat(60),
            "1 : Header Mismatch ".to_string(),
            "-".repeat(60),
            "Expected:".to_string(),
            "    key    [equals]                   content-type".to_string(),
            "    value  [equals_case_insensitive]  application/json".to_string(),
            String::new(),
            "Received (most similar header):".to_string(),
            "    content-type=text/plain".to_string(),
            String::new(),
            "All received header values:".to_string(),
            "    1. content-type=text/plain".to_string(),
            "    2. accept=*/*".to_string(),
            String::new(),
            "Matcher:  header".to_string(),
            format!("Docs:     {}", docs_link("header")),
            "\u{2002}".to_string(),
            String::new(),
        ]
        .join("\n");
        assert_eq!(output, expected);
    }

    #[test]
    fn key_only_count_mismatch_aligns_single_row() {
        let mismatch = key_value_mismatch(KeyValueComparison {
            key: Some(attribute("equals", "x-id", None)),
            value: None,
            expected_count: Some(2),
            actual_count: Some(1),
            all: vec![pair("x-id", "1")],
        });

        let (output, diff) = create_mismatch_output(1, &mismatch);

        let expected = [
            "-".repeat(60),
            "2 : Header Mismatch ".to_string(),
            "-".repeat(60),
            "Expected:".to_string(),
            "    key  [equals]  x-id".to_string(),
            String::new(),
            "to appear 2 times but appeared 1".to_string(),
            String::new(),
            "All received header values:".to_string(),
            "    1. x-id=1".to_string(),
            String::new(),
            "Matcher:  header".to_string(),
            format!("Docs:     {}", docs_link("header")),
            "\u{2002}".to_string(),
            String::new(),
        ]
        .join("\n");
        assert_eq!(output, expected);
        assert_eq!(diff, Some(("2".to_string(), "1".to_string())));
    }

    #[test]
    fn single_value_mismatch_prints_diff() {
        let mismatch = Mismatch {
            entity: "body".to_string(),
            matcher_method: "body".to_string(),
            comparison: Some(SingleValueComparison {
                operator: "equals".to_string(),
                expected: "hello".to_string(),
                actual: "hello".to_string(),
            }),
            key_value_comparison: None,
            function_comparison: None,
            matching_strategy: None,
            best_match: false,
            diff: Some(DiffResult {
                differences: vec![Diff::Same("hello".to_string())],
                distance: 0.0,
                tokenizer: Tokenizer::Line,
            }),
        };

        let (output, _) = create_mismatch_output(0, &mismatch);

        let expected = [
            "-".repeat(60),
            "1 : Body Mismatch ".to_string(),
            "-".repeat(60),
            "Expected body equals:".to_string(),
            "hello".to_string(),
            String::new(),
            "Received:".to_string(),
            "hello".to_string(),
            String::new(),
            "Diff:".to_string(),
            "   | hello".to_string(),
            String::new(),
            "Matcher:  body".to_string(),
            format!("Docs:     {}", docs_link("body")),
            "\u{2002}".to_string(),
            String::new(),
        ]
        .join("\n");
        assert_eq!(output, expected);
    }

    #[test]
    fn request_data_containing_tabs_is_printed_verbatim() {
        let mismatch = Mismatch {
            entity: "body".to_string(),
            matcher_method: "body".to_string(),
            comparison: Some(SingleValueComparison {
                operator: "equals".to_string(),
                expected: "id\tname\n1\tAlice".to_string(),
                actual: "id\tname\n1\tBob".to_string(),
            }),
            key_value_comparison: None,
            function_comparison: None,
            matching_strategy: None,
            best_match: false,
            diff: Some(DiffResult {
                differences: vec![Diff::Same("id\tname\n".to_string())],
                distance: 0.0,
                tokenizer: Tokenizer::Line,
            }),
        };

        let (output, _) = create_mismatch_output(0, &mismatch);

        assert!(output.contains("id\tname\n1\tAlice\n"), "{output}");
        assert!(output.contains("id\tname\n1\tBob\n"), "{output}");
        assert!(output.contains("   | id\tname\n"), "{output}");
    }
}
