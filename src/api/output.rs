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

/// A writer that aligns `\t`-separated columns of consecutive tab-containing lines
/// (elastic tab stops) when the buffered text is retrieved via [`TabWriter::into_string`].
///
/// This is a simplified replacement for the `tabwriter` crate that produces identical
/// output for the patterns this module generates. It measures cell width in `char`s
/// rather than display columns, so cells containing wide characters (e.g. CJK) may
/// align differently in terminals.
struct TabWriter {
    buf: String,
}

impl TabWriter {
    fn new() -> Self {
        Self { buf: String::new() }
    }

    fn into_string(self) -> String {
        let mut out_lines: Vec<String> = Vec::new();
        let mut block: Vec<&str> = Vec::new();

        for line in self.buf.split('\n') {
            if line.contains('\t') {
                block.push(line);
            } else {
                Self::align_block(&mut block, &mut out_lines);
                out_lines.push(line.to_string());
            }
        }
        Self::align_block(&mut block, &mut out_lines);

        out_lines.join("\n")
    }

    /// Aligns the columns of a block of consecutive tab-containing lines and clears the block.
    fn align_block(block: &mut Vec<&str>, out_lines: &mut Vec<String>) {
        let rows: Vec<Vec<&str>> = block.drain(..).map(|line| line.split('\t').collect()).collect();

        // Compute the width of each column. The last cell of a row is never padded,
        // so it does not contribute to its column's width. The minimum column width of 2
        // matches the `tabwriter` crate's default, which this replaces: most visibly, it
        // renders the empty cell of lines starting with `\t` as a 4-space indent.
        let column_count = rows.iter().map(|cells| cells.len()).max().unwrap_or(0);
        let mut widths = vec![2; column_count];
        for cells in &rows {
            for (idx, cell) in cells.iter().enumerate() {
                if idx + 1 < cells.len() {
                    widths[idx] = widths[idx].max(cell.chars().count());
                }
            }
        }

        for cells in &rows {
            let mut line = String::new();
            for (idx, cell) in cells.iter().enumerate() {
                line.push_str(cell);
                if idx + 1 < cells.len() {
                    let padding = widths[idx] - cell.chars().count() + 2;
                    line.push_str(&" ".repeat(padding));
                }
            }
            out_lines.push(line);
        }
    }
}

impl Write for TabWriter {
    fn write_str(&mut self, s: &str) -> std::fmt::Result {
        self.buf.push_str(s);
        Ok(())
    }
}

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
    let mut tw = TabWriter::new();
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

    let output = tw.into_string();

    if !ide_diff_left.is_empty() && !ide_diff_right.is_empty() {
        return (output, Some((ide_diff_left, ide_diff_right)));
    }

    (output, None)
}

fn write_header(tw: &mut TabWriter, idx: usize, mismatch: &Mismatch) {
    writeln!(tw, "{}", "-".repeat(60)).unwrap();
    writeln!(tw, "{} : {} Mismatch ", idx + 1, title_case(&mismatch.entity)).unwrap();
    writeln!(tw, "{}", "-".repeat(60)).unwrap();
}

fn handle_single_value_comparison(
    tw: &mut TabWriter,
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

    (comparison.expected.to_string(), comparison.actual.to_string())
}

fn handle_key_value_comparison(
    tw: &mut TabWriter,
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
            (actual, true) => format!("{} ({})", actual, QUOTED_TEXT),
            (actual, false) => actual.to_string(),
        };
        writeln!(tw, "\tkey\t[{}]\t{}", key.operator, expected).unwrap();
    }

    if let Some(value) = &comparison.value {
        let expected = match quote_if_whitespace(&value.expected) {
            (expected, true) => format!("{} ({})", expected, QUOTED_TEXT),
            (expected, false) => expected.to_string(),
        };
        writeln!(tw, "\tvalue\t[{}]\t{}", value.operator, expected).unwrap();
    }

    if let (Some(expected_count), Some(actual_count)) = (comparison.expected_count, comparison.actual_count) {
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
                writeln!(tw, "\nbut{}{} value was\n\t{}", most_similar, mismatch.entity, value).unwrap();
                (value.to_string(), value.to_string())
            }
            (Some(key), None) => {
                writeln!(tw, "\nbut{}{} key was\n\t{}", most_similar, mismatch.entity, key).unwrap();
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

fn print_all_request_values(tw: &mut TabWriter, entity: &str, all: &[KeyValueComparisonKeyValuePair]) {
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

fn print_value_not_in_request(tw: &mut TabWriter, matching_strategy: &Option<MatchingStrategy>) {
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

fn handle_function_comparison(tw: &mut TabWriter, mismatch: &Mismatch, comparison: &FunctionComparison) {
    writeln!(
        tw,
        "Custom matcher function {} with index {} did not match the request",
        mismatch.matcher_method, comparison.index
    )
    .unwrap();
}

fn write_footer(tw: &mut TabWriter, mismatch: &Mismatch) {
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
        writeln!(tw, "{}", create_diff_result_output(diff_result)).unwrap();
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
