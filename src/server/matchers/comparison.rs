use crate::common::{data::HttpMockRegex, util::HttpMockBytes};
use regex::Regex;
use std::{convert::TryInto, ops::Deref};
use stringmetrics::LevWeights;

pub fn string_has_prefix(
    case_sensitive: bool,
    negated: bool,
    mock_value: &Option<&String>,
    req_value: &Option<&String>,
) -> bool {
    let result = match (mock_value, req_value) {
        (None, _) => return true,
        (Some(_), None) => return negated,
        (Some(mv), Some(rv)) => {
            if rv.len() < mv.len() {
                return false;
            }

            match case_sensitive {
                true => rv.starts_with(mv.as_str()),
                false => rv.to_lowercase().starts_with(&mv.to_lowercase()),
            }
        }
    };

    if negated {
        !result
    } else {
        result
    }
}

#[cfg(test)]
mod string_has_prefix_tests {
    use super::*;

    #[test]
    fn test_case_sensitive_prefix_match() {
        let mock_value = "Hello".to_string();
        let req_value = "Hello World".to_string();
        assert!(string_has_prefix(
            true,
            false,
            &Some(&mock_value),
            &Some(&req_value),
        ));

        let mock_value = "hello".to_string();
        let req_value = "Hello World".to_string();
        assert!(!string_has_prefix(
            true,
            false,
            &Some(&mock_value),
            &Some(&req_value),
        ));
    }

    #[test]
    fn test_case_insensitive_prefix_match() {
        let mock_value = "hello".to_string();
        let req_value = "Hello World".to_string();
        assert!(string_has_prefix(
            false,
            false,
            &Some(&mock_value),
            &Some(&req_value),
        ));

        let mock_value = "HELLO".to_string();
        let req_value = "Hello World".to_string();
        assert!(string_has_prefix(
            false,
            false,
            &Some(&mock_value),
            &Some(&req_value),
        ));
    }

    #[test]
    fn test_negated_prefix_match() {
        let mock_value = "Hello".to_string();
        let req_value = "Hello World".to_string();
        assert!(!string_has_prefix(
            true,
            true,
            &Some(&mock_value),
            &Some(&req_value),
        ));

        let mock_value = "hello".to_string();
        let req_value = "Hello World".to_string();
        assert!(string_has_prefix(
            true,
            true,
            &Some(&mock_value),
            &Some(&req_value),
        ));
    }

    #[test]
    fn test_prefix_too_short() {
        let mock_value = "Hello World".to_string();
        let req_value = "Hello".to_string();
        assert!(!string_has_prefix(
            true,
            false,
            &Some(&mock_value),
            &Some(&req_value),
        ));
    }

    #[test]
    fn test_exact_match() {
        let mock_value = "Hello".to_string();
        let req_value = "Hello".to_string();
        assert!(string_has_prefix(
            true,
            false,
            &Some(&mock_value),
            &Some(&req_value),
        ));
    }

    #[test]
    fn test_empty_mock_value() {
        let mock_value = "".to_string();
        let req_value = "Hello World".to_string();
        assert!(string_has_prefix(
            true,
            false,
            &Some(&mock_value),
            &Some(&req_value),
        ));
    }

    #[test]
    fn test_empty_req_value() {
        let mock_value = "Hello".to_string();
        let req_value = "".to_string();
        assert!(!string_has_prefix(
            true,
            false,
            &Some(&mock_value),
            &Some(&req_value),
        ));
    }

    #[test]
    fn test_both_empty() {
        let mock_value = "".to_string();
        let req_value = "".to_string();
        assert!(string_has_prefix(
            true,
            false,
            &Some(&mock_value),
            &Some(&req_value),
        ));
    }
}

pub fn distance_for_prefix(
    case_sensitive: bool,
    negated: bool,
    mock_value: &Option<&String>,
    req_value: &Option<&String>,
) -> usize {
    if mock_value.map_or(0, |v| v.len()) == 0 {
        return 0;
    }

    let mock_slice = mock_value.as_deref();
    let mock_slice_len = mock_slice.map_or(0, |v| v.len());

    let req_slice = req_value
        .as_deref()
        .map(|s| &s[..mock_slice_len.min(s.len())]);

    distance_for_substring(
        case_sensitive,
        negated,
        &mock_slice.map(|v| v.as_str()),
        &req_slice,
    )
}

#[cfg(test)]
mod distance_for_prefix_tests {
    use super::*;

    #[test]
    fn test_distance_for_prefix_case_sensitive() {
        let mock_value_str = "hello".to_string();
        let req_value_str = "hello world".to_string();
        let mock_value = Some(&mock_value_str);
        let req_value = Some(&req_value_str);
        assert_eq!(distance_for_prefix(true, false, &mock_value, &req_value), 0);
    }

    #[test]
    fn test_distance_for_prefix_case_insensitive() {
        let mock_value_str = "Hello".to_string();
        let req_value_str = "hello world".to_string();
        let mock_value = Some(&mock_value_str);
        let req_value = Some(&req_value_str);
        assert_eq!(
            distance_for_prefix(false, false, &mock_value, &req_value),
            0
        );
    }

    #[test]
    fn test_distance_for_prefix_negated() {
        let mock_value_str = "hello".to_string();
        let req_value_str = "hello world".to_string();
        let mock_value = Some(&mock_value_str);
        let req_value = Some(&req_value_str);
        assert_eq!(distance_for_prefix(true, true, &mock_value, &req_value), 5);
    }

    #[test]
    fn test_distance_for_prefix_no_match() {
        let mock_value_str = "world".to_string();
        let req_value_str = "hello world".to_string();
        let mock_value = Some(&mock_value_str);
        let req_value = Some(&req_value_str);
        assert!(distance_for_prefix(true, false, &mock_value, &req_value) > 0);
    }

    #[test]
    fn test_distance_for_prefix_partial_match() {
        let mock_value_str = "hell".to_string();
        let req_value_str = "hello world".to_string();
        let mock_value = Some(&mock_value_str);
        let req_value = Some(&req_value_str);
        assert_eq!(distance_for_prefix(true, false, &mock_value, &req_value), 0);
    }

    #[test]
    fn test_distance_for_prefix_empty_mock_value() {
        let mock_value_str = "".to_string();
        let req_value_str = "hello world".to_string();
        let mock_value = Some(&mock_value_str);
        let req_value = Some(&req_value_str);
        assert_eq!(distance_for_prefix(true, false, &mock_value, &req_value), 0);
    }

    #[test]
    fn test_distance_for_prefix_empty_req_value() {
        let mock_value_str = "hello".to_string();
        let req_value_str = "".to_string();
        let mock_value = Some(&mock_value_str);
        let req_value = Some(&req_value_str);
        assert_eq!(distance_for_prefix(true, false, &mock_value, &req_value), 5);
    }

    #[test]
    fn test_distance_for_prefix_both_empty() {
        let mock_value_str = "".to_string();
        let req_value_str = "".to_string();
        let mock_value = Some(&mock_value_str);
        let req_value = Some(&req_value_str);
        assert_eq!(distance_for_prefix(true, false, &mock_value, &req_value), 0);
    }

    #[test]
    fn test_distance_for_prefix_none_mock_value() {
        let req_value_str = "hello world".to_string();
        let mock_value: Option<&String> = None;
        let req_value = Some(&req_value_str);
        assert_eq!(distance_for_prefix(true, false, &mock_value, &req_value), 0);
    }

    #[test]
    fn test_distance_for_prefix_none_req_value() {
        let mock_value_str = "hello".to_string();
        let mock_value = Some(&mock_value_str);
        let req_value: Option<&String> = None;
        assert_eq!(distance_for_prefix(true, false, &mock_value, &req_value), 5);
    }

    #[test]
    fn test_distance_for_prefix_none_both() {
        let mock_value: Option<&String> = None;
        let req_value: Option<&String> = None;
        assert_eq!(distance_for_prefix(true, false, &mock_value, &req_value), 0);
    }
}

pub fn string_has_suffix(
    case_sensitive: bool,
    negated: bool,
    mock_value: &Option<&String>,
    req_value: &Option<&String>,
) -> bool {
    let result = match (mock_value, req_value) {
        (None, _) => return true,
        (Some(_), None) => return negated,
        (Some(mv), Some(rv)) => {
            if rv.len() < mv.len() {
                return false;
            }

            match case_sensitive {
                true => rv.ends_with(mv.as_str()),
                false => rv.to_lowercase().ends_with(&mv.to_lowercase()),
            }
        }
    };

    if negated {
        !result
    } else {
        result
    }
}

#[cfg(test)]
mod string_has_suffix_tests {
    use super::*;

    #[test]
    fn test_case_sensitive_suffix_match() {
        let mock_value = "world".to_string();
        let req_value = "hello world".to_string();
        assert!(string_has_suffix(
            true,
            false,
            &Some(&mock_value),
            &Some(&req_value),
        ));
        let mock_value = "World".to_string();
        assert!(!string_has_suffix(
            true,
            false,
            &Some(&mock_value),
            &Some(&req_value),
        ));
    }

    #[test]
    fn test_case_insensitive_suffix_match() {
        let mock_value = "world".to_string();
        let req_value = "hello world".to_string();
        assert!(string_has_suffix(
            false,
            false,
            &Some(&mock_value),
            &Some(&req_value),
        ));
        let mock_value = "World".to_string();
        assert!(string_has_suffix(
            false,
            false,
            &Some(&mock_value),
            &Some(&req_value),
        ));
    }

    #[test]
    fn test_negated_suffix_match() {
        let mock_value = "world".to_string();
        let req_value = "hello world".to_string();
        assert!(!string_has_suffix(
            true,
            true,
            &Some(&mock_value),
            &Some(&req_value),
        ));
        let mock_value = "World".to_string();
        assert!(string_has_suffix(
            true,
            true,
            &Some(&mock_value),
            &Some(&req_value),
        ));
    }

    #[test]
    fn test_suffix_too_short() {
        let mock_value = "hello world".to_string();
        let req_value = "world".to_string();
        assert!(!string_has_suffix(
            true,
            false,
            &Some(&mock_value),
            &Some(&req_value),
        ));
    }

    #[test]
    fn test_exact_match_case_sensitive() {
        let mock_value = "hello".to_string();
        let req_value = "hello".to_string();
        assert!(string_has_suffix(
            true,
            false,
            &Some(&mock_value),
            &Some(&req_value),
        ));
    }

    #[test]
    fn test_exact_match_case_insensitive() {
        let mock_value = "Hello".to_string();
        let req_value = "hello".to_string();
        assert!(string_has_suffix(
            false,
            false,
            &Some(&mock_value),
            &Some(&req_value),
        ));
    }

    #[test]
    fn test_no_match_case_sensitive() {
        let mock_value = "world".to_string();
        let req_value = "hello".to_string();
        assert!(!string_has_suffix(
            true,
            false,
            &Some(&mock_value),
            &Some(&req_value),
        ));
    }

    #[test]
    fn test_no_match_case_insensitive() {
        let mock_value = "World".to_string();
        let req_value = "hello".to_string();
        assert!(!string_has_suffix(
            false,
            false,
            &Some(&mock_value),
            &Some(&req_value),
        ));
    }

    #[test]
    fn test_empty_mock_value() {
        let mock_value = "".to_string();
        let req_value = "hello world".to_string();
        assert!(string_has_suffix(
            true,
            false,
            &Some(&mock_value),
            &Some(&req_value),
        ));
    }

    #[test]
    fn test_empty_req_value() {
        let mock_value = "hello".to_string();
        let req_value = "".to_string();
        assert!(!string_has_suffix(
            true,
            false,
            &Some(&mock_value),
            &Some(&req_value),
        ));
    }

    #[test]
    fn test_both_empty() {
        let mock_value = "".to_string();
        let req_value = "".to_string();
        assert!(string_has_suffix(
            true,
            false,
            &Some(&mock_value),
            &Some(&req_value),
        ));
    }
}

/// Calculates the distance between the suffix of a given string (`req_value`) and the expected suffix (`mock_value`).
///
/// # Arguments
///
/// * `case_sensitive` - A boolean indicating if the comparison should be case-sensitive.
/// * `negated` - A boolean indicating if the result should be negated.
/// * `mock_value` - An optional reference to the expected suffix.
/// * `req_value` - An optional reference to the string to be checked.
///
/// # Returns
///
/// A usize representing the distance between the suffix of `req_value` and `mock_value`.
/// If `negated` is true, the result will be the length of `mock_value` minus the calculated distance.
pub fn distance_for_suffix(
    case_sensitive: bool,
    negated: bool,
    mock_value: &Option<&String>,
    req_value: &Option<&String>,
) -> usize {
    if mock_value.map_or(0, |v| v.len()) == 0 {
        return 0;
    }

    let mock_slice = mock_value.as_deref();
    let mock_slice_len = mock_slice.map_or(0, |v| v.len());

    let req_slice = req_value
        .as_deref()
        .map(|s| &s[..mock_slice_len.min(s.len())]);

    distance_for_substring(
        case_sensitive,
        negated,
        &mock_slice.map(|v| v.as_str()),
        &req_slice,
    )
}

pub fn string_contains(
    case_sensitive: bool,
    negated: bool,
    mock_value: &Option<&String>,
    req_value: &Option<&String>,
) -> bool {
    let result = match (mock_value, req_value) {
        (None, _) => return true,
        (Some(_), None) => return negated,
        (Some(mv), Some(rv)) => match case_sensitive {
            true => rv.contains(mv.as_str()),
            false => rv.to_lowercase().contains(&mv.to_lowercase()),
        },
    };

    if negated {
        !result
    } else {
        result
    }
}

#[cfg(test)]
mod string_contains_tests {
    use super::*;

    #[test]
    fn test_case_sensitive_contains() {
        let mock_value = "world".to_string();
        let req_value = "hello world".to_string();
        assert!(string_contains(
            true,
            false,
            &Some(&mock_value),
            &Some(&req_value),
        ));

        let mock_value = "World".to_string();
        assert!(!string_contains(
            true,
            false,
            &Some(&mock_value),
            &Some(&req_value),
        ));
    }

    #[test]
    fn test_case_insensitive_contains() {
        let mock_value = "world".to_string();
        let req_value = "hello world".to_string();
        assert!(string_contains(
            false,
            false,
            &Some(&mock_value),
            &Some(&req_value),
        ));

        let mock_value = "World".to_string();
        assert!(string_contains(
            false,
            false,
            &Some(&mock_value),
            &Some(&req_value),
        ));
    }

    #[test]
    fn test_negated_contains() {
        let mock_value = "world".to_string();
        let req_value = "hello world".to_string();
        assert!(!string_contains(
            true,
            true,
            &Some(&mock_value),
            &Some(&req_value),
        ));

        let mock_value = "World".to_string();
        assert!(string_contains(
            true,
            true,
            &Some(&mock_value),
            &Some(&req_value),
        ));
    }

    #[test]
    fn test_contains_substring() {
        let mock_value = "lo wo".to_string();
        let req_value = "hello world".to_string();
        assert!(string_contains(
            true,
            false,
            &Some(&mock_value),
            &Some(&req_value),
        ));
    }

    #[test]
    fn test_no_match_contains() {
        let mock_value = "test".to_string();
        let req_value = "hello world".to_string();
        assert!(!string_contains(
            true,
            false,
            &Some(&mock_value),
            &Some(&req_value),
        ));
    }

    #[test]
    fn test_empty_mock_value() {
        let mock_value = "".to_string();
        let req_value = "hello world".to_string();
        assert!(string_contains(
            true,
            false,
            &Some(&mock_value),
            &Some(&req_value),
        ));
    }

    #[test]
    fn test_empty_req_value() {
        let mock_value = "hello".to_string();
        let req_value = "".to_string();
        assert!(!string_contains(
            true,
            false,
            &Some(&mock_value),
            &Some(&req_value),
        ));
    }

    #[test]
    fn test_both_empty() {
        let mock_value = "".to_string();
        let req_value = "".to_string();
        assert!(string_contains(
            true,
            false,
            &Some(&mock_value),
            &Some(&req_value),
        ));
    }
}

pub fn distance_for_substring<T>(
    case_sensitive: bool,
    negated: bool,
    mock_value: &Option<T>,
    req_value: &Option<T>,
) -> usize
where
    T: Deref<Target = str> + AsRef<str>,
{
    if mock_value.is_none() {
        return 0;
    }

    let mock_slice = mock_value.as_deref().unwrap_or("");
    let req_slice = req_value.as_deref().unwrap_or("");

    let lcs_length = longest_common_substring(case_sensitive, mock_slice, req_slice);

    if negated {
        lcs_length
    } else {
        std::cmp::max(mock_slice.len(), req_slice.len()) - lcs_length
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_distance_for_substring_case_sensitive_match() {
        let mock_value = Some("Hello");
        let req_value = Some("Hello World");
        assert_eq!(
            distance_for_substring(true, false, &mock_value, &req_value),
            6
        );
    }

    #[test]
    fn test_distance_for_substring_case_sensitive_no_match() {
        let mock_value = Some("Hello");
        let req_value = Some("World");
        assert_eq!(
            distance_for_substring(true, false, &mock_value, &req_value),
            4
        );
    }

    #[test]
    fn test_distance_for_substring_case_insensitive_match() {
        let mock_value = Some("hello");
        let req_value = Some("Hello World");
        assert_eq!(
            distance_for_substring(false, false, &mock_value, &req_value),
            6
        );
    }

    #[test]
    fn test_distance_for_substring_case_insensitive_no_match() {
        let mock_value = Some("hello");
        let req_value = Some("WORLD");
        assert_eq!(
            distance_for_substring(false, false, &mock_value, &req_value),
            4
        );
    }

    #[test]
    fn test_distance_for_substring_negated_match() {
        let mock_value = Some("Hello");
        let req_value = Some("Hello World");
        assert_eq!(
            distance_for_substring(true, true, &mock_value, &req_value),
            5
        );
    }

    #[test]
    fn test_distance_for_substring_negated_no_match() {
        let mock_value = Some("Hello");
        let req_value = Some("World");
        assert_eq!(
            distance_for_substring(true, true, &mock_value, &req_value),
            1
        );
    }

    #[test]
    fn test_distance_for_substring_empty_mock_value() {
        let mock_value = Some("");
        let req_value = Some("Hello");
        assert_eq!(
            distance_for_substring(true, false, &mock_value, &req_value),
            5
        );
    }

    #[test]
    fn test_distance_for_substring_empty_req_value() {
        let mock_value = Some("Hello");
        let req_value = Some("");
        assert_eq!(
            distance_for_substring(true, false, &mock_value, &req_value),
            5
        );
    }

    #[test]
    fn test_distance_for_substring_both_empty() {
        let mock_value = Some("");
        let req_value = Some("");
        assert_eq!(
            distance_for_substring(true, false, &mock_value, &req_value),
            0
        );
    }

    #[test]
    fn test_distance_for_substring_none_mock_value() {
        let req_value = Some("hello");
        let mock_value: Option<&str> = None;
        assert_eq!(
            distance_for_substring(true, false, &mock_value, &req_value),
            0
        );
    }

    #[test]
    fn test_distance_for_substring_none_req_value() {
        let mock_value = Some("hello");
        let req_value: Option<&str> = None;
        assert_eq!(
            distance_for_substring(true, false, &mock_value, &req_value),
            5
        );
    }

    #[test]
    fn test_distance_for_substring_none_both() {
        let mock_value: Option<&str> = None;
        let req_value: Option<&str> = None;
        assert_eq!(
            distance_for_substring(true, false, &mock_value, &req_value),
            0
        );
    }
}

pub fn longest_common_substring(case_sensitive: bool, s1: &str, s2: &str) -> usize {
    let (long_s, short_s) = if s1.len() < s2.len() {
        (s2, s1)
    } else {
        (s1, s2)
    };

    let mut previous = vec![0; short_s.chars().count() + 1];
    let mut current = vec![0; short_s.chars().count() + 1];
    let mut longest = 0;

    for (i, long_char) in long_s.chars().enumerate() {
        for (j, short_char) in short_s.chars().enumerate() {
            let long_char = if case_sensitive {
                long_char
            } else {
                long_char.to_lowercase().next().unwrap()
            };

            let short_char = if case_sensitive {
                short_char
            } else {
                short_char.to_lowercase().next().unwrap()
            };

            if long_char == short_char {
                current[j + 1] = previous[j] + 1;
                if current[j + 1] > longest {
                    longest = current[j + 1];
                }
            } else {
                current[j + 1] = 0;
            }
        }
        std::mem::swap(&mut previous, &mut current);
    }
    longest
}

#[cfg(test)]
mod longest_common_substring_tests {
    use super::*;

    #[test]
    fn test_case_sensitive_match() {
        let s1 = "abcdef";
        let s2 = "zabcy";
        assert_eq!(longest_common_substring(true, s1, s2), 3);

        let s1 = "abcdef";
        let s2 = "zabCY";
        assert_eq!(longest_common_substring(true, s1, s2), 2);

        let s1 = "abcDEF";
        let s2 = "zabcy";
        assert_eq!(longest_common_substring(true, s1, s2), 3);
    }

    #[test]
    fn test_case_insensitive_match() {
        let s1 = "abcdef";
        let s2 = "zabcy";
        assert_eq!(longest_common_substring(false, s1, s2), 3);

        let s1 = "abcDEF";
        let s2 = "zabcY";
        assert_eq!(longest_common_substring(false, s1, s2), 3);

        let s1 = "ABCdef";
        let s2 = "ZABcy";
        assert_eq!(longest_common_substring(false, s1, s2), 3);
    }

    #[test]
    fn test_no_common_substring() {
        let s1 = "abc";
        let s2 = "xyz";
        assert_eq!(longest_common_substring(true, s1, s2), 0);
        assert_eq!(longest_common_substring(false, s1, s2), 0);
    }

    #[test]
    fn test_empty_strings() {
        let s1 = "";
        let s2 = "abcdef";
        assert_eq!(longest_common_substring(true, s1, s2), 0);
        assert_eq!(longest_common_substring(false, s1, s2), 0);

        let s1 = "abcdef";
        let s2 = "";
        assert_eq!(longest_common_substring(true, s1, s2), 0);
        assert_eq!(longest_common_substring(false, s1, s2), 0);

        let s1 = "";
        let s2 = "";
        assert_eq!(longest_common_substring(true, s1, s2), 0);
        assert_eq!(longest_common_substring(false, s1, s2), 0);
    }

    #[test]
    fn test_full_string_match() {
        let s1 = "abcdef";
        let s2 = "abcdef";
        assert_eq!(longest_common_substring(true, s1, s2), 6);
        assert_eq!(longest_common_substring(false, s1, s2), 6);
    }

    #[test]
    fn test_utf8_support() {
        let s1 = "你好世界";
        let s2 = "世界你好";
        assert_eq!(longest_common_substring(true, s1, s2), 2);

        let s1 = "你好世界";
        let s2 = "世界HELLO";
        assert_eq!(longest_common_substring(false, s1, s2), 2);
    }
}

pub fn string_equals(
    case_sensitive: bool,
    negated: bool,
    mock_value: &Option<&String>,
    req_value: &Option<&String>,
) -> bool {
    let result = match (mock_value, req_value) {
        (None, _) => return true,
        (Some(_), None) => return negated,
        (Some(mv), Some(rv)) => match case_sensitive {
            true => mv.eq(rv),
            false => mv.to_lowercase().eq(&rv.to_lowercase()),
        },
    };

    if negated {
        !result
    } else {
        result
    }
}

pub fn hostname_equals(
    negated: bool,
    mock_value: &Option<&String>,
    req_value: &Option<&String>,
) -> bool {
    if let (Some(mv), Some(rv)) = (mock_value, req_value) {
        let mv_is = mv.eq_ignore_ascii_case("localhost") || mv.eq_ignore_ascii_case("127.0.0.1");
        let rv_is = rv.eq_ignore_ascii_case("localhost") || rv.eq_ignore_ascii_case("127.0.0.1");

        if mv_is && rv_is {
            return !negated;
        }
    }

    string_equals(false, negated, mock_value, req_value)
}

#[cfg(test)]
mod hostname_equals_tests {
    use super::*;

    #[test]
    fn test_hostname_is_not_equal() {
        let mock_value_str = "github.com".to_string();
        let req_value_str = "not_github.com".to_string();

        assert_eq!(
            hostname_equals(false, &Some(&mock_value_str), &Some(&req_value_str)),
            false
        );

        assert_eq!(
            hostname_equals(true, &Some(&mock_value_str), &Some(&req_value_str)),
            true
        );
    }

    #[test]
    fn test_hostname_is_equal() {
        let mock_value_str = "github.com".to_string();
        let req_value_str = "github.com".to_string();

        assert_eq!(
            hostname_equals(false, &Some(&mock_value_str), &Some(&req_value_str)),
            true
        );

        assert_eq!(
            hostname_equals(true, &Some(&mock_value_str), &Some(&req_value_str)),
            false
        );
    }

    #[test]
    fn test_hostname_localhost_equals() {
        let localhost_str = "localhost".to_string();
        let ip_str = "127.0.0.1".to_string();

        assert_eq!(
            hostname_equals(false, &Some(&localhost_str), &Some(&localhost_str)),
            true
        );

        assert_eq!(
            hostname_equals(true, &Some(&localhost_str), &Some(&localhost_str)),
            false
        );

        assert_eq!(
            hostname_equals(false, &Some(&localhost_str), &Some(&ip_str)),
            true
        );

        assert_eq!(
            hostname_equals(true, &Some(&localhost_str), &Some(&ip_str)),
            false
        );

        assert_eq!(
            hostname_equals(false, &Some(&ip_str), &Some(&localhost_str)),
            true
        );

        assert_eq!(
            hostname_equals(true, &Some(&ip_str), &Some(&localhost_str)),
            false
        );

        assert_eq!(hostname_equals(false, &Some(&ip_str), &Some(&ip_str)), true);

        assert_eq!(hostname_equals(true, &Some(&ip_str), &Some(&ip_str)), false);
    }

    #[test]
    fn test_hostname_is_not_equal_to_localhost() {
        let github_str = "github.com".to_string();
        let localhost_str = "localhost".to_string();

        assert_eq!(
            hostname_equals(false, &Some(&github_str), &Some(&localhost_str)),
            false
        );

        assert_eq!(
            hostname_equals(true, &Some(&github_str), &Some(&localhost_str)),
            true
        );

        assert_eq!(
            hostname_equals(false, &Some(&localhost_str), &Some(&github_str)),
            false
        );

        assert_eq!(
            hostname_equals(true, &Some(&localhost_str), &Some(&github_str)),
            true
        );
    }
}

/// Computes the distance between two optional strings (`mock_value` and `req_value`),
/// with optional case sensitivity and negation.
///
/// # Arguments
///
/// * `case_sensitive` - A boolean indicating if the comparison should be case-sensitive.
/// * `negated` - A boolean indicating if the result should be negated.
/// * `mock_value` - An optional reference to the first string to compare.
/// * `req_value` - An optional reference to the second string to compare.
///
/// # Returns
///
/// A `usize` representing the distance between `mock_value` and `req_value`,
/// taking into account case sensitivity and negation.
pub fn string_distance(
    case_sensitive: bool,
    negated: bool,
    mock_value: &Option<&String>,
    req_value: &Option<&String>,
) -> usize {
    if mock_value.is_none() {
        return 0;
    }

    let mock_slice = mock_value.as_deref().map_or("", |s| s.as_str());
    let req_slice = req_value.as_deref().map_or("", |s| s.as_str());

    let (mock_slice, req_slice) = if case_sensitive {
        (mock_slice.to_string(), req_slice.to_string())
    } else {
        (mock_slice.to_lowercase(), req_slice.to_lowercase())
    };

    let distance = equal_weight_distance_for(mock_slice.as_bytes(), req_slice.as_bytes());

    if negated {
        std::cmp::max(mock_slice.len(), req_slice.len()) - distance
    } else {
        distance
    }
}

#[cfg(test)]
mod string_distance_tests {
    use super::*;

    #[test]
    fn test_string_distance_case_sensitive() {
        let mock_value_str = "Hello".to_string();
        let req_value_str = "Hello".to_string();
        let mock_value = Some(&mock_value_str);
        let req_value = Some(&req_value_str);
        assert_eq!(string_distance(true, false, &mock_value, &req_value), 0);

        let mock_value_str = "Hello".to_string();
        let req_value_str = "hello".to_string();
        let mock_value = Some(&mock_value_str);
        let req_value = Some(&req_value_str);
        assert_eq!(string_distance(true, false, &mock_value, &req_value), 1);
    }

    #[test]
    fn test_string_distance_case_insensitive() {
        let mock_value_str = "Hello".to_string();
        let req_value_str = "hello".to_string();
        let mock_value = Some(&mock_value_str);
        let req_value = Some(&req_value_str);
        assert_eq!(string_distance(false, false, &mock_value, &req_value), 0);

        let mock_value_str = "HELLO".to_string();
        let req_value_str = "hello".to_string();
        let mock_value = Some(&mock_value_str);
        let req_value = Some(&req_value_str);
        assert_eq!(string_distance(false, false, &mock_value, &req_value), 0);
    }

    #[test]
    fn test_string_distance_negated() {
        let mock_value_str = "Hello".to_string();
        let req_value_str = "Hello".to_string();
        let mock_value = Some(&mock_value_str);
        let req_value = Some(&req_value_str);
        assert_eq!(string_distance(true, true, &mock_value, &req_value), 5);

        let mock_value_str = "Hello".to_string();
        let req_value_str = "hello".to_string();
        let mock_value = Some(&mock_value_str);
        let req_value = Some(&req_value_str);
        assert_eq!(string_distance(true, true, &mock_value, &req_value), 4);
    }

    #[test]
    fn test_string_distance_no_match() {
        let mock_value_str = "Hello".to_string();
        let req_value_str = "World".to_string();
        let mock_value = Some(&mock_value_str);
        let req_value = Some(&req_value_str);
        assert_eq!(string_distance(true, false, &mock_value, &req_value), 4);
    }

    #[test]
    fn test_string_distance_empty_mock_value() {
        let mock_value_str = "".to_string();
        let req_value_str = "hello".to_string();
        let mock_value = Some(&mock_value_str);
        let req_value = Some(&req_value_str);
        assert_eq!(string_distance(true, false, &mock_value, &req_value), 5);
    }

    #[test]
    fn test_string_distance_empty_req_value() {
        let mock_value_str = "hello".to_string();
        let req_value_str = "".to_string();
        let mock_value = Some(&mock_value_str);
        let req_value = Some(&req_value_str);
        assert_eq!(string_distance(true, false, &mock_value, &req_value), 5);
    }

    #[test]
    fn test_string_distance_both_empty() {
        let mock_value_str = "".to_string();
        let req_value_str = "".to_string();
        let mock_value = Some(&mock_value_str);
        let req_value = Some(&req_value_str);
        assert_eq!(string_distance(true, false, &mock_value, &req_value), 0);
    }

    #[test]
    fn test_string_distance_none_mock_value() {
        let req_value_str = "hello".to_string();
        let mock_value: Option<&String> = None;
        let req_value = Some(&req_value_str);
        assert_eq!(string_distance(true, false, &mock_value, &req_value), 0);
    }
}

// *************************************************************************************************
// Helper functions
// *************************************************************************************************
pub fn distance_for<T>(expected: &[T], actual: &[T]) -> usize
where
    T: PartialEq + Sized,
{
    stringmetrics::levenshtein_limit_iter(expected.iter(), actual.iter(), u32::MAX) as usize
}

pub fn equal_weight_distance_for<T>(expected: &[T], actual: &[T]) -> usize
where
    T: PartialEq + Sized,
{
    stringmetrics::try_levenshtein_weight_iter(
        expected.iter(),
        actual.iter(),
        u32::MAX,
        &LevWeights {
            insertion: 1,
            deletion: 1,
            substitution: 1,
        },
    )
    // Option=None is only returned in case limit is maxed out here it realistically can't
    .expect("character limit exceeded") as usize
}

pub fn regex_unmatched_length(text: &str, re: &HttpMockRegex) -> usize {
    let mut last_end = 0;
    let mut total_unmatched_length = 0;

    // Iterate through all matches and sum the lengths of unmatched parts
    for mat in re.0.find_iter(text) {
        if last_end != mat.start() {
            total_unmatched_length += mat.start() - last_end;
        }
        last_end = mat.end();
    }

    // Add any characters after the last match to the total
    if last_end < text.len() {
        total_unmatched_length += text.len() - last_end;
    }

    total_unmatched_length
}

pub fn integer_equals<T: PartialEq>(
    negated: bool,
    mock_value: &Option<&T>,
    req_value: &Option<&T>,
) -> bool {
    let result = match (mock_value, req_value) {
        (None, _) => return true,
        (Some(_), None) => return negated,
        (Some(mv), Some(rv)) => mv == rv,
    };

    if negated {
        !result
    } else {
        result
    }
}
#[cfg(test)]
mod usize_equals_tests {
    use super::*;

    #[test]
    fn test_usize_equals_equal_values_not_negated() {
        assert_eq!(true, integer_equals(false, &Some(&10), &Some(&10)));
    }

    #[test]
    fn test_usize_equals_unequal_values_not_negated() {
        assert_eq!(false, integer_equals(false, &Some(&10), &Some(&20)));
    }

    #[test]
    fn test_usize_equals_equal_values_negated() {
        assert_eq!(false, integer_equals(true, &Some(&10), &Some(&10)));
    }

    #[test]
    fn test_usize_equals_unequal_values_negated() {
        assert_eq!(true, integer_equals(true, &Some(&10), &Some(&20)));
    }

    #[test]
    fn test_usize_equals_mock_value_none_not_negated() {
        assert_eq!(true, integer_equals(false, &None, &Some(&10)));
    }

    #[test]
    fn test_usize_equals_req_value_none_not_negated() {
        assert_eq!(false, integer_equals(false, &Some(&10), &None));
    }

    #[test]
    fn test_usize_equals_both_none_not_negated() {
        assert_eq!(true, integer_equals::<i32>(false, &None, &None));
    }

    #[test]
    fn test_usize_equals_mock_value_none_negated() {
        assert_eq!(true, integer_equals(true, &None, &Some(&10)));
    }

    #[test]
    fn test_usize_equals_req_value_none_negated() {
        assert_eq!(true, integer_equals(true, &Some(&10), &None));
    }

    #[test]
    fn test_usize_equals_both_none_negated() {
        assert_eq!(true, integer_equals::<i32>(true, &None, &None));
    }
}

pub fn bytes_equal(
    negated: bool,
    mock_value: &Option<&HttpMockBytes>,
    req_value: &Option<&HttpMockBytes>,
) -> bool {
    let result = match (mock_value, req_value) {
        (None, _) => return true,
        (Some(_), None) => return negated,
        (Some(mv), Some(rv)) => mv == rv,
    };

    if negated {
        !result
    } else {
        result
    }
}

pub fn bytes_includes(
    negated: bool,
    mock_value: &Option<&HttpMockBytes>,
    req_value: &Option<&HttpMockBytes>,
) -> bool {
    let result = match (mock_value, req_value) {
        (None, _) => return true,
        (Some(_), None) => return negated,
        (Some(mv), Some(rv)) => {
            // Convert both Bytes into slices for comparison
            let mock_slice = mv.to_bytes();
            let req_slice = rv.to_bytes();

            if mock_slice.is_empty() {
                return true;
            }

            // Check if the request slice contains the mock slice
            req_slice
                .windows(mock_slice.len())
                .any(|window| window == mock_slice)
        }
    };

    if negated {
        !result
    } else {
        result
    }
}

#[cfg(test)]
mod bytes_includes_test {
    use crate::{common::util::HttpMockBytes, server::matchers::comparison::bytes_includes};

    #[test]
    fn test_bytes_includes() {
        assert_eq!(
            bytes_includes(
                false,
                &Some(&HttpMockBytes::from(bytes::Bytes::from("   b\n c"))),
                &Some(&HttpMockBytes::from(bytes::Bytes::from(
                    "a   b\n c  \ncd ef"
                ))),
            ),
            true
        );
    }
}

pub fn bytes_prefix(
    negated: bool,
    mock_value: &Option<&HttpMockBytes>,
    req_value: &Option<&HttpMockBytes>,
) -> bool {
    let result = match (mock_value, req_value) {
        (None, _) => return true,
        (Some(_), None) => return negated,
        (Some(mv), Some(rv)) => {
            // Convert both Bytes into slices for comparison
            let mock_slice = mv.to_bytes();
            let req_slice = rv.to_bytes();

            // Check if the request slice starts with the mock slice
            req_slice.starts_with(&mock_slice)
        }
    };

    if negated {
        !result
    } else {
        result
    }
}

pub fn bytes_suffix(
    negated: bool,
    mock_value: &Option<&HttpMockBytes>,
    req_value: &Option<&HttpMockBytes>,
) -> bool {
    let result = match (mock_value, req_value) {
        (None, _) => return true,
        (Some(_), None) => return negated,
        (Some(mv), Some(rv)) => {
            // Convert both Bytes into slices for comparison
            let mock_slice = mv.to_bytes();
            let req_slice = rv.to_bytes();

            // Check if the request slice ends with the mock slice
            req_slice.ends_with(&mock_slice)
        }
    };

    if negated {
        !result
    } else {
        result
    }
}

/// Calculates the "distance" between two optional numeric values.
///
/// The distance is defined as the absolute difference between the two values
/// if both are present. If one is present and the other is not, the distance
/// is the present value, unless it is zero, in which case the distance is 1.
/// If both values are `None`, the distance is 0.
///
/// # Arguments
///
/// * `mock_value` - An optional reference to a u16 that may or may not be present.
/// * `req_value` - An optional reference to a u16 that may or may not be present.
///
/// # Returns
///
/// Returns a usize representing the distance as defined above.
///
pub fn distance_for_usize<T>(expected: &Option<&T>, actual: &Option<&T>) -> usize
where
    T: TryInto<usize> + Copy,
{
    let mock_size = expected.map_or(0, |&v| v.try_into().unwrap_or(0));
    let req_size = actual.map_or(0, |&v| v.try_into().unwrap_or(0));

    match (expected, actual) {
        (Some(_), Some(_)) => {
            if mock_size > req_size {
                mock_size - req_size
            } else {
                req_size - mock_size
            }
        }
        (Some(_), None) | (None, Some(_)) => {
            if mock_size == 0 {
                1
            } else {
                mock_size
            }
        }
        (None, None) => 0,
        // Redundant pattern, logically unnecessary but included for completeness
    }
}

#[cfg(test)]
mod distance_for_usize_test {
    use crate::server::matchers::comparison::distance_for_usize;

    #[test]
    fn tree_map_fully_contains_other() {
        assert_eq!(distance_for_usize::<usize>(&Some(&4), &None), 4);
        assert_eq!(distance_for_usize::<usize>(&Some(&0), &None), 1);
        assert_eq!(distance_for_usize::<usize>(&Some(&5), &Some(&3)), 2);
        assert_eq!(distance_for_usize::<usize>(&None, &None), 0);
    }
}

pub fn string_matches_regex(
    negated: bool,
    case_sensitive: bool,
    mock_value: &Option<&HttpMockRegex>,
    req_value: &Option<&String>,
) -> bool {
    let result = match (mock_value, req_value) {
        (None, _) => return true,
        (Some(_), None) => return negated,
        (Some(mv), Some(rv)) => {
            if case_sensitive {
                mv.0.is_match(rv)
            } else {
                let case_insensitive_str = mv.0.as_str().to_lowercase();
                let case_insensitive_regex = Regex::new(&case_insensitive_str).unwrap();
                case_insensitive_regex.is_match(&rv.to_lowercase())
            }
        }
    };

    match negated {
        true => !result,
        false => result,
    }
}

#[cfg(test)]
mod string_matches_regex_tests {
    use super::*;
    use crate::common::data::HttpMockRegex;

    #[test]
    fn test_string_matches_regex() {
        let pattern = HttpMockRegex(Regex::new(r"^Hello.*").unwrap());

        let req_value = "Hello, world!".to_string();
        assert_eq!(
            string_matches_regex(false, true, &Some(&pattern), &Some(&req_value)),
            true
        );

        let req_value = "Goodbye, world!".to_string();
        assert_eq!(
            string_matches_regex(false, true, &Some(&pattern), &Some(&req_value)),
            false
        );
    }

    #[test]
    fn test_string_matches_regex_negated() {
        let pattern = HttpMockRegex(Regex::new(r"^Hello.*").unwrap());

        let req_value = "Hello, world!".to_string();
        assert_eq!(
            string_matches_regex(true, true, &Some(&pattern), &Some(&req_value)),
            false
        );

        let req_value = "Goodbye, world!".to_string();
        assert_eq!(
            string_matches_regex(true, true, &Some(&pattern), &Some(&req_value)),
            true
        );
    }

    #[test]
    fn test_string_matches_regex_empty_pattern() {
        let pattern = HttpMockRegex(Regex::new(r"").unwrap());

        let req_value = "Anything".to_string();
        assert_eq!(
            string_matches_regex(false, true, &Some(&pattern), &Some(&req_value)),
            true
        );
    }

    #[test]
    fn test_string_matches_regex_empty_request_value() {
        let pattern = HttpMockRegex(Regex::new(r"^Hello.*").unwrap());

        let req_value = "".to_string();
        assert_eq!(
            string_matches_regex(false, true, &Some(&pattern), &Some(&req_value)),
            false
        );
    }

    #[test]
    fn test_string_matches_regex_empty_pattern_and_request_value() {
        let pattern = HttpMockRegex(Regex::new(r"").unwrap());

        let req_value = "".to_string();
        assert_eq!(
            string_matches_regex(false, true, &Some(&pattern), &Some(&req_value)),
            true
        );
    }

    #[test]
    fn test_string_matches_regex_none_pattern() {
        let req_value = "Hello, world!".to_string();
        assert_eq!(
            string_matches_regex(false, true, &None, &Some(&req_value)),
            true
        );
    }

    #[test]
    fn test_string_matches_regex_none_request_value() {
        let pattern = HttpMockRegex(Regex::new(r"^Hello.*").unwrap());

        assert_eq!(
            string_matches_regex(false, true, &Some(&pattern), &None),
            false
        );
    }

    #[test]
    fn test_string_matches_regex_none_pattern_and_request_value() {
        assert_eq!(string_matches_regex(false, true, &None, &None), true);
    }

    #[test]
    fn test_string_matches_regex_none_pattern_negated() {
        let req_value = "Hello, world!".to_string();
        assert_eq!(
            string_matches_regex(true, true, &None, &Some(&req_value)),
            true
        );
    }

    #[test]
    fn test_string_matches_regex_none_request_value_negated() {
        let pattern = HttpMockRegex(Regex::new(r"^Hello.*").unwrap());

        assert_eq!(
            string_matches_regex(true, true, &Some(&pattern), &None),
            true
        );
    }

    #[test]
    fn test_string_matches_regex_none_pattern_and_request_value_negated() {
        assert_eq!(string_matches_regex(true, true, &None, &None), true);
    }

    #[test]
    fn test_string_matches_regex_pattern_with_special_chars() {
        let pattern = HttpMockRegex(Regex::new(r"^\d{3}-\d{2}-\d{4}$").unwrap());

        let req_value = "123-45-6789".to_string();
        assert_eq!(
            string_matches_regex(false, true, &Some(&pattern), &Some(&req_value)),
            true
        );

        let req_value = "123-45-678".to_string();
        assert_eq!(
            string_matches_regex(false, true, &Some(&pattern), &Some(&req_value)),
            false
        );
    }

    #[test]
    fn test_string_matches_regex_case_insensitive() {
        let pattern = HttpMockRegex(Regex::new(r"(?i)^hello.*").unwrap());

        let req_value = "Hello, world!".to_string();
        assert_eq!(
            string_matches_regex(false, false, &Some(&pattern), &Some(&req_value)),
            true
        );

        let req_value = "hello, world!".to_string();
        assert_eq!(
            string_matches_regex(false, false, &Some(&pattern), &Some(&req_value)),
            true
        );

        let req_value = "HELLO, WORLD!".to_string();
        assert_eq!(
            string_matches_regex(false, false, &Some(&pattern), &Some(&req_value)),
            true
        );
    }
}
/// Computes the distance between a given string and a regex pattern based on whether the match is negated or not.
///
/// # Arguments
/// * `negated` - A boolean indicating whether the match should be negated.
/// * `mock_value` - An optional reference to a `Pattern` containing the regex to match against.
/// * `req_value` - An optional reference to a `String` representing the input string to be matched.
///
/// # Returns
/// * `usize` - The computed distance. In the negated case, it returns the number of characters that did match the regex.
///             In the non-negated case, it returns the number of characters that did not match the regex.
pub fn regex_string_distance(
    negated: bool,
    case_sensitive: bool,
    mock_value: &Option<&HttpMockRegex>,
    req_value: &Option<&String>,
) -> usize {
    if mock_value.is_none() {
        return 0;
    }

    if req_value.is_none() || req_value.unwrap().is_empty() {
        let matches = string_matches_regex(negated, case_sensitive, mock_value, req_value);
        return match matches {
            true => 0,
            false => mock_value.unwrap().0.as_str().len(),
        };
    }

    let rv = req_value.map_or("", |s| s.as_str());
    let unmatched_len = regex_unmatched_length(rv, mock_value.unwrap());

    match negated {
        true => rv.len() - unmatched_len,
        false => unmatched_len,
    }
}

#[cfg(test)]
mod regex_string_distance_tests {
    use super::*;
    use regex::Regex;

    #[test]
    fn test_non_negated_full_match() {
        let pattern = HttpMockRegex(Regex::new("a+").unwrap());
        let mock_value = Some(&pattern);
        let req_value_str = String::from("aaaa");
        let req_value = Some(&req_value_str);
        assert_eq!(
            regex_string_distance(false, true, &mock_value, &req_value),
            0
        );
    }

    #[test]
    fn test_negated_full_match() {
        let pattern = HttpMockRegex(Regex::new("a+").unwrap());
        let mock_value = Some(&pattern);
        let req_value_str = String::from("aaaa");
        let req_value = Some(&req_value_str);
        assert_eq!(
            regex_string_distance(true, true, &mock_value, &req_value),
            4
        );
    }

    #[test]
    fn test_non_negated_partial_match() {
        let pattern = HttpMockRegex(Regex::new("a+").unwrap());
        let mock_value = Some(&pattern);
        let req_value_str = String::from("aaabbb");
        let req_value = Some(&req_value_str);
        assert_eq!(
            regex_string_distance(false, true, &mock_value, &req_value),
            3
        );
    }

    #[test]
    fn test_negated_partial_match() {
        let pattern = HttpMockRegex(Regex::new("a+").unwrap());
        let mock_value = Some(&pattern);
        let req_value_str = String::from("aaaabbb");
        let req_value = Some(&req_value_str);
        assert_eq!(
            regex_string_distance(true, true, &mock_value, &req_value),
            4
        );
    }

    #[test]
    fn test_non_negated_no_match() {
        let pattern = HttpMockRegex(Regex::new("c+").unwrap());
        let mock_value = Some(&pattern);
        let req_value_str = String::from("aaabbb");
        let req_value = Some(&req_value_str);
        assert_eq!(
            regex_string_distance(false, true, &mock_value, &req_value),
            6
        );
    }

    #[test]
    fn test_negated_no_match() {
        let pattern = HttpMockRegex(Regex::new("c+").unwrap());
        let mock_value = Some(&pattern);
        let req_value_str = String::from("aaabbb");
        let req_value = Some(&req_value_str);
        assert_eq!(
            regex_string_distance(true, true, &mock_value, &req_value),
            0
        );
    }

    #[test]
    fn test_no_req_value() {
        let pattern = HttpMockRegex(Regex::new("a+").unwrap());
        let mock_value = Some(&pattern);
        let req_value: Option<&String> = None;
        assert_eq!(
            regex_string_distance(false, true, &mock_value, &req_value),
            2
        );
    }

    #[test]
    fn test_no_mock_value() {
        let mock_value: Option<&HttpMockRegex> = None;
        let req_value_str = String::from("aaabbb");
        let req_value = Some(&req_value_str);
        assert_eq!(
            regex_string_distance(false, true, &mock_value, &req_value),
            0
        );
    }

    #[test]
    fn test_empty_rv_non_negated_match() {
        let pattern = HttpMockRegex(Regex::new(".*").unwrap());
        let mock_value = Some(&pattern);
        let req_value: Option<&String> = None;
        assert_eq!(
            regex_string_distance(false, true, &mock_value, &req_value),
            2
        );
    }

    #[test]
    fn test_empty_rv_non_negated_no_match() {
        let pattern = HttpMockRegex(Regex::new(".+").unwrap());
        let mock_value = Some(&pattern);
        let req_value: Option<&String> = None; // This will make rv empty
        assert_eq!(
            regex_string_distance(false, true, &mock_value, &req_value),
            pattern.0.as_str().len()
        );
    }

    #[test]
    fn test_empty_rv_negated_match() {
        let pattern = HttpMockRegex(Regex::new(".*").unwrap());
        let mock_value = Some(&pattern);
        let req_value: Option<&String> = None;

        assert_eq!(
            // When request does not have any value, it is never a match. Since we have "negate=true",
            // it is a match. Hence, distance is 0.
            regex_string_distance(true, true, &mock_value, &req_value),
            0
        );
    }

    #[test]
    fn test_empty_rv_negated_no_match() {
        let pattern = HttpMockRegex(Regex::new(".+").unwrap());
        let mock_value = Some(&pattern);
        let req_value: Option<&String> = None;
        // Body does not match, but negated = true, so its a match, hence distance is a 0.
        assert_eq!(
            regex_string_distance(true, true, &mock_value, &req_value),
            0
        );
    }
}
