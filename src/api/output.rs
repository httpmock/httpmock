use std::io::Write;

use crate::common::{
    data::{
        ClosestMatch, Diff, DiffResult, FunctionComparison, KeyValueComparison,
        KeyValueComparisonKeyValuePair, Mismatch, SingleValueComparison,
    },
    util::title_case,
};

use tabwriter::TabWriter;

use crate::server::matchers::generic::MatchingStrategy;
#[cfg(feature = "color")]
use colored::Colorize;

const QUOTED_TEXT: &str = "quoted for better readability";

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
        let (mm_output, fail_text_pair) = create_mismatch_output(idx, &mm);

        if fail_text.is_none() {
            if let Some(text) = fail_text_pair {
                fail_text = Some(text)
            }
        }

        output.push_str(&mm_output);
    }

    if let Some((left, right)) = fail_text {
        assert_eq!(left, right, "{}", output)
    }

    panic!("{}", output)
}

pub fn create_mismatch_output(
    idx: usize,
    mismatch: &Mismatch,
) -> (String, Option<(String, String)>) {
    let mut tw = TabWriter::new(vec![]);
    let mut ide_diff_left = String::new();
    let mut ide_diff_right = String::new();

    write_header(&mut tw, idx, mismatch);

    if let Some(comparison) = &mismatch.comparison {
        let (left, right) = handle_single_value_comparison(&mut tw, mismatch, comparison);

        ide_diff_left.push_str(&left);
        ide_diff_right.push_str(&right);
    } else if let Some(comparison) = &mismatch.key_value_comparison {
        let (left, right) = handle_key_value_comparison(&mut tw, mismatch, comparison);

        ide_diff_left.push_str(&left);
        ide_diff_right.push_str(&right);
    } else if let Some(comparison) = &mismatch.function_comparison {
        handle_function_comparison(&mut tw, mismatch, comparison);
    }

    write_footer(&mut tw, mismatch);

    tw.flush().unwrap();

    let output = String::from_utf8(tw.into_inner().unwrap()).unwrap();

    if ide_diff_left.len() > 0 && ide_diff_right.len() > 0 {
        return (output, Some((ide_diff_left, ide_diff_right)));
    }

    (output, None)
}

fn write_header(tw: &mut TabWriter<Vec<u8>>, idx: usize, mismatch: &Mismatch) {
    writeln!(tw, "{}", &"-".repeat(60)).unwrap();
    writeln!(
        tw,
        "{}",
        &format!("{} : {} Mismatch ", idx + 1, title_case(&mismatch.entity),)
    )
    .unwrap();
    writeln!(tw, "{}", &"-".repeat(60)).unwrap();
}

fn handle_single_value_comparison(
    tw: &mut TabWriter<Vec<u8>>,
    mismatch: &Mismatch,
    comparison: &SingleValueComparison,
) -> (String, String) {
    writeln!(
        tw,
        "Expected {} {}:\n{}",
        mismatch.entity, comparison.operator, comparison.expected
    )
    .unwrap();

    writeln!(tw, "\nReceived:\n{}", comparison.actual).unwrap();

    (
        comparison.expected.to_string(),
        comparison.actual.to_string(),
    )
}

fn handle_key_value_comparison(
    tw: &mut TabWriter<Vec<u8>>,
    mismatch: &Mismatch,
    comparison: &KeyValueComparison,
) -> (String, String) {
    let most_similar = match mismatch.best_match {
        true => format!(" (most similar {})", mismatch.entity),
        false => String::from(" "),
    };

    writeln!(tw, "Expected:").unwrap();

    if let Some(key) = &comparison.key {
        let expected = match quote_if_whitespace(&key.expected) {
            (actual, true) => format!("{} ({})", actual, &QUOTED_TEXT),
            (actual, false) => actual.to_string(),
        };
        writeln!(tw, "\tkey\t[{}]\t{}", key.operator, expected).unwrap();
    }

    if let Some(value) = &comparison.value {
        let expected = match quote_if_whitespace(&value.expected) {
            (expected, true) => format!("{} ({})", expected, &QUOTED_TEXT),
            (expected, false) => expected.to_string(),
        };
        writeln!(tw, "\tvalue\t[{}]\t{}", value.operator, expected).unwrap();
    }

    if let (Some(expected_count), Some(actual_count)) =
        (comparison.expected_count, comparison.actual_count)
    {
        if comparison.key.is_none() && comparison.value.is_none() {
            writeln!(
                tw,
                "\n{} to appear {} {} but appeared {}",
                mismatch.entity,
                expected_count,
                times_str(expected_count),
                actual_count
            )
            .unwrap();
        } else {
            writeln!(
                tw,
                "\nto appear {} {} but appeared {}",
                expected_count,
                times_str(expected_count),
                actual_count
            )
            .unwrap();
        }

        print_all_request_values(tw, &mismatch.entity, &comparison.all);

        return (expected_count.to_string(), actual_count.to_string());
    }

    if let (Some(key_attr), Some(value_attr)) = (&comparison.key, &comparison.value) {
        let result = match (&key_attr.actual, &value_attr.actual) {
            (Some(key), Some(value)) => {
                writeln!(tw, "\nReceived{}:\n\t{}={}", most_similar, key, value).unwrap();
                (format!("{}\n{}", key, value), format!("{}\n{}", key, value))
            }
            (None, Some(value)) => {
                writeln!(
                    tw,
                    "\nbut{}{} value was\n\t{}",
                    most_similar, mismatch.entity, value
                )
                .unwrap();
                (value.to_string(), value.to_string())
            }
            (Some(key), None) => {
                writeln!(
                    tw,
                    "\nbut{}{} key was\n\t{}",
                    most_similar, mismatch.entity, key
                )
                .unwrap();
                (key.to_string(), key.to_string())
            }
            (None, None) => {
                let msg = match &mismatch.matching_strategy {
                    None => "but none was provided",
                    Some(v) => match v {
                        MatchingStrategy::Presence => {
                            "to be in the request, but none was provided."
                        }
                        MatchingStrategy::Absence => {
                            "not to be present, but the request contained it."
                        }
                    },
                };

                writeln!(tw, "\n{}", msg).unwrap();
                (String::new(), String::new())
            }
        };

        // print_value_not_in_request(tw, &mismatch.matching_strategy);
        print_all_request_values(tw, &mismatch.entity, &comparison.all);

        return result;
    }

    print_value_not_in_request(tw, &mismatch.matching_strategy);
    print_all_request_values(tw, &mismatch.entity, &comparison.all);

    (String::new(), String::new())
}

fn print_all_request_values(
    tw: &mut TabWriter<Vec<u8>>,
    entity: &str,
    all: &[KeyValueComparisonKeyValuePair],
) {
    if all.is_empty() {
        return;
    }

    writeln!(tw, "\nAll received {} values:", entity).unwrap();

    for (index, pair) in all.iter().enumerate() {
        let value = if pair.value.is_some() {
            format!("={}", pair.value.clone().unwrap())
        } else {
            String::new()
        };

        let text = format!("{}{}", pair.key, value);
        writeln!(tw, "\t{}. {}", index + 1, text).unwrap();
    }
}

fn print_value_not_in_request(
    tw: &mut TabWriter<Vec<u8>>,
    matching_strategy: &Option<MatchingStrategy>,
) {
    writeln!(
        tw,
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

fn handle_function_comparison(
    tw: &mut TabWriter<Vec<u8>>,
    mismatch: &Mismatch,
    comparison: &FunctionComparison,
) {
    writeln!(
        tw,
        "Custom matcher function {} with index {} did not match the request",
        mismatch.matcher_method, comparison.index
    )
    .unwrap();
}

fn write_footer(tw: &mut TabWriter<Vec<u8>>, mismatch: &Mismatch) {
    let mut version = env!("CARGO_PKG_VERSION");
    if version.trim().is_empty() {
        version = "latest";
    }

    let link = format!(
        "https://docs.rs/httpmock/{}/httpmock/struct.When.html#method.{}",
        version, mismatch.matcher_method
    );

    writeln!(tw).unwrap();

    if let Some(diff_result) = &mismatch.diff {
        writeln!(tw, "{}", &create_diff_result_output(diff_result)).unwrap();
        writeln!(tw).unwrap();
    }

    writeln!(tw, "Matcher:\t{}", mismatch.matcher_method).unwrap();
    writeln!(tw, "Docs:\t{}", link).unwrap();
    writeln!(tw, " ").unwrap();
}

fn create_diff_result_output(dd: &DiffResult) -> String {
    let mut output = String::new();
    output.push_str("Diff:");
    if dd.differences.is_empty() {
        output.push_str("<empty>");
    }
    output.push_str("\n");

    dd.differences.iter().enumerate().for_each(|(idx, d)| {
        if idx > 0 {
            output.push_str("\n")
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
    if v == 1 {
        return "time";
    }

    return "times";
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
