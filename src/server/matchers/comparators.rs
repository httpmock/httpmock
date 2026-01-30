use assert_json_diff::{assert_json_matches_no_panic, CompareMode, Config};
use bytes::Bytes;
use serde_json::Value;
use std::{borrow::Cow, convert::TryInto, ops::Deref, sync::Arc};

use crate::{
    common::{
        data::{HttpMockRegex, HttpMockRequest},
        util::HttpMockBytes,
    },
    server::matchers::comparison::{
        distance_for, distance_for_prefix, distance_for_substring, distance_for_suffix,
        equal_weight_distance_for, hostname_equals, regex_unmatched_length, string_contains,
        string_distance, string_equals, string_has_prefix, string_has_suffix,
    },
};

use crate::server::matchers::comparison;

pub trait ValueComparator<S: ?Sized, T: ?Sized> {
    fn matches(&self, mock_value: &Option<&S>, req_value: &Option<&T>) -> bool;
    fn name(&self) -> &str;
    fn distance(&self, mock_value: &Option<&S>, req_value: &Option<&T>) -> usize;
}

// ************************************************************************************************
// JSONExactMatchComparator
// ************************************************************************************************
pub struct JSONExactMatchComparator {}

impl JSONExactMatchComparator {
    pub fn new() -> Self {
        Self {}
    }
}

impl ValueComparator<Value, Value> for JSONExactMatchComparator {
    fn matches(&self, mock_value: &Option<&Value>, req_value: &Option<&Value>) -> bool {
        match (mock_value, req_value) {
            (None, _) => true,
            (Some(_), None) => false,
            (Some(mv), Some(rv)) => {
                let config = Config::new(CompareMode::Strict);
                assert_json_matches_no_panic(rv, mv, config).is_ok()
            }
        }
    }

    fn name(&self) -> &str {
        "equals"
    }

    fn distance(&self, mock_value: &Option<&Value>, req_value: &Option<&Value>) -> usize {
        let mv_bytes = mock_value.map_or(Vec::new(), |v| v.to_string().into_bytes());
        let rv_bytes = req_value.map_or(Vec::new(), |v| v.to_string().into_bytes());
        distance_for(&mv_bytes, &rv_bytes)
    }
}

// ************************************************************************************************
// JSONContainsMatchComparator
// ************************************************************************************************
pub struct JSONContainsMatchComparator {
    pub negated: bool,
}

impl JSONContainsMatchComparator {
    pub fn new(negated: bool) -> Self {
        Self { negated }
    }
}

impl ValueComparator<Value, Value> for JSONContainsMatchComparator {
    fn matches(&self, mock_value: &Option<&Value>, req_value: &Option<&Value>) -> bool {
        match (mock_value, req_value) {
            (None, _) => true,
            (Some(_), None) => self.negated,
            (Some(mv), Some(rv)) => {
                let config = Config::new(CompareMode::Inclusive);
                let matches = assert_json_matches_no_panic(rv, mv, config).is_ok();
                if self.negated {
                    !matches
                } else {
                    matches
                }
            }
        }
    }

    fn name(&self) -> &str {
        if self.negated {
            "excludes"
        } else {
            "includes"
        }
    }

    fn distance(&self, mock_value: &Option<&Value>, req_value: &Option<&Value>) -> usize {
        let mv_bytes = mock_value.map_or(Vec::new(), |v| v.to_string().into_bytes());
        let rv_bytes = req_value.map_or(Vec::new(), |v| v.to_string().into_bytes());
        let distance = equal_weight_distance_for(&mv_bytes, &rv_bytes);

        if self.negated {
            std::cmp::max(mv_bytes.len(), rv_bytes.len()) - distance
        } else {
            distance
        }
    }
}

// ************************************************************************************************
// StringExactMatchComparator
// ************************************************************************************************
pub struct HostEqualsComparator {
    negated: bool,
}

impl HostEqualsComparator {
    pub fn new(negated: bool) -> Self {
        Self { negated }
    }
}

impl ValueComparator<String, String> for HostEqualsComparator {
    fn matches(&self, mock_value: &Option<&String>, req_value: &Option<&String>) -> bool {
        hostname_equals(self.negated, mock_value, req_value)
    }

    fn name(&self) -> &str {
        if self.negated {
            "not equal to"
        } else {
            "equals"
        }
    }

    fn distance(&self, mock_value: &Option<&String>, req_value: &Option<&String>) -> usize {
        // negation is taken care of in matches!
        if self.matches(mock_value, req_value) {
            return 0;
        }

        string_distance(false, self.negated, mock_value, req_value)
    }
}

// ************************************************************************************************
// StringExactMatchComparator
// ************************************************************************************************
pub struct StringEqualsComparator {
    case_sensitive: bool,
    negated: bool,
}

impl StringEqualsComparator {
    pub fn new(case_sensitive: bool, negated: bool) -> Self {
        Self {
            case_sensitive,
            negated,
        }
    }
}

impl ValueComparator<String, String> for StringEqualsComparator {
    fn matches(&self, mock_value: &Option<&String>, req_value: &Option<&String>) -> bool {
        string_equals(self.case_sensitive, self.negated, mock_value, req_value)
    }

    fn name(&self) -> &str {
        if self.negated {
            "not equal to"
        } else {
            "equals"
        }
    }

    fn distance(&self, mock_value: &Option<&String>, req_value: &Option<&String>) -> usize {
        string_distance(self.case_sensitive, self.negated, mock_value, req_value)
    }
}

// ************************************************************************************************
// StringIncludesMatchComparator
// ************************************************************************************************
pub struct StringContainsComparator {
    case_sensitive: bool,
    negated: bool,
}

impl StringContainsComparator {
    pub fn new(case_sensitive: bool, negated: bool) -> Self {
        Self {
            case_sensitive,
            negated,
        }
    }
}

impl ValueComparator<String, String> for StringContainsComparator {
    fn matches(&self, mock_value: &Option<&String>, req_value: &Option<&String>) -> bool {
        string_contains(self.case_sensitive, self.negated, mock_value, req_value)
    }

    fn name(&self) -> &str {
        if self.negated {
            "excludes"
        } else {
            "includes"
        }
    }

    fn distance(&self, mock_value: &Option<&String>, req_value: &Option<&String>) -> usize {
        let mock_slice = mock_value.as_ref().map(|s| s.as_str());
        let req_slice = req_value.as_ref().map(|s| s.as_str());

        distance_for_substring(self.case_sensitive, self.negated, &mock_slice, &req_slice)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_distance_mock_shorter_than_req() {
        let binding1 = "hello".to_string();
        let mock_value = Some(&binding1);
        let binding2 = "hello world".to_string();
        let req_value = Some(&binding2);

        let comparator = StringContainsComparator {
            case_sensitive: true,
            negated: false,
        };
        assert_eq!(comparator.distance(&mock_value, &req_value), 6); // Assuming "hello" vs "hello world"
    }

    #[test]
    fn test_distance_mock_equal_size_different() {
        let binding1 = "hello".to_string();
        let mock_value = Some(&binding1);
        let binding2 = "world".to_string();
        let req_value = Some(&binding2);

        let comparator = StringContainsComparator {
            case_sensitive: true,
            negated: false,
        };
        assert_eq!(comparator.distance(&mock_value, &req_value), 4); // Assuming "hello" vs "world"
    }

    #[test]
    fn test_distance_exact_match() {
        let binding1 = "hello".to_string();
        let mock_value = Some(&binding1);
        let binding2 = "hello".to_string();
        let req_value = Some(&binding2);

        let comparator = StringContainsComparator {
            case_sensitive: true,
            negated: false,
        };
        assert_eq!(comparator.distance(&mock_value, &req_value), 0); // Exact match
    }
}

// ************************************************************************************************
// StringContainsMatchComparator
// ************************************************************************************************
pub struct StringPrefixMatchComparator {
    case_sensitive: bool,
    negated: bool,
}

impl StringPrefixMatchComparator {
    pub fn new(case_sensitive: bool, negated: bool) -> Self {
        Self {
            case_sensitive,
            negated,
        }
    }
}

impl ValueComparator<String, String> for StringPrefixMatchComparator {
    fn matches(&self, mock_value: &Option<&String>, req_value: &Option<&String>) -> bool {
        string_has_prefix(self.case_sensitive, self.negated, mock_value, req_value)
    }

    fn name(&self) -> &str {
        if self.negated {
            "prefix not"
        } else {
            "has prefix"
        }
    }

    fn distance(&self, mock_value: &Option<&String>, req_value: &Option<&String>) -> usize {
        distance_for_prefix(self.case_sensitive, self.negated, mock_value, req_value)
    }
}

// ************************************************************************************************
// StringContainsMatchComparator
// ************************************************************************************************
pub struct StringSuffixMatchComparator {
    case_sensitive: bool,
    negated: bool,
}

impl StringSuffixMatchComparator {
    pub fn new(case_sensitive: bool, negated: bool) -> Self {
        Self {
            case_sensitive,
            negated,
        }
    }
}

impl ValueComparator<String, String> for StringSuffixMatchComparator {
    fn matches(&self, mock_value: &Option<&String>, req_value: &Option<&String>) -> bool {
        string_has_suffix(self.case_sensitive, self.negated, mock_value, req_value)
    }

    fn name(&self) -> &str {
        if self.negated {
            "suffix not"
        } else {
            "has suffix"
        }
    }

    fn distance(&self, mock_value: &Option<&String>, req_value: &Option<&String>) -> usize {
        distance_for_suffix(self.case_sensitive, self.negated, mock_value, req_value)
    }
}

// ************************************************************************************************
// StringPatternMatchComparator
// ************************************************************************************************
pub struct StringPatternMatchComparator {
    case_sensitive: bool,
    negated: bool,
}

impl StringPatternMatchComparator {
    pub fn new(negated: bool, case_sensitive: bool) -> Self {
        Self {
            negated,
            case_sensitive,
        }
    }
}

impl ValueComparator<HttpMockRegex, String> for StringPatternMatchComparator {
    fn matches(&self, mock_value: &Option<&HttpMockRegex>, req_value: &Option<&String>) -> bool {
        comparison::string_matches_regex(self.negated, self.case_sensitive, mock_value, req_value)
    }

    fn name(&self) -> &str {
        if self.negated {
            "does not match regex"
        } else {
            "matches regex"
        }
    }

    fn distance(&self, mock_value: &Option<&HttpMockRegex>, req_value: &Option<&String>) -> usize {
        comparison::regex_string_distance(self.negated, self.case_sensitive, mock_value, req_value)
    }
}

// ************************************************************************************************
// StringExactMatchComparator
// ************************************************************************************************
pub struct HttpMockBytesPatternComparator {}

impl HttpMockBytesPatternComparator {
    pub fn new() -> Self {
        Self {}
    }
}

impl ValueComparator<HttpMockRegex, HttpMockBytes> for HttpMockBytesPatternComparator {
    fn matches(
        &self,
        mock_value: &Option<&HttpMockRegex>,
        req_value: &Option<&HttpMockBytes>,
    ) -> bool {
        match (mock_value, req_value) {
            (None, Some(_)) => true,
            (Some(_), None) => false,
            (Some(mv), Some(rv)) => mv.0.is_match(&rv.to_maybe_lossy_str()),
            (None, None) => true,
        }
    }

    fn name(&self) -> &str {
        "matches regex"
    }

    fn distance(
        &self,
        mock_value: &Option<&HttpMockRegex>,
        req_value: &Option<&HttpMockBytes>,
    ) -> usize {
        let rv = match req_value {
            Some(s) => s.to_maybe_lossy_str(),
            None => Cow::Borrowed(""),
        };

        let default_pattern = HttpMockRegex(regex::Regex::new(".*").unwrap());

        let mv = mock_value.unwrap_or(&default_pattern);

        regex_unmatched_length(&rv, mv)
    }
}

// ************************************************************************************************
// StringExactMatchComparator
// ************************************************************************************************
pub struct StringRegexMatchComparator {}

impl StringRegexMatchComparator {
    pub fn new() -> Self {
        Self {}
    }
}

impl ValueComparator<HttpMockRegex, String> for StringRegexMatchComparator {
    fn matches(&self, mock_value: &Option<&HttpMockRegex>, req_value: &Option<&String>) -> bool {
        match (mock_value, req_value) {
            (None, Some(_)) => true,
            (Some(_), None) => false,
            (Some(mv), Some(rv)) => mv.0.is_match(rv),
            (None, None) => true,
        }
    }

    fn name(&self) -> &str {
        "matches regex"
    }

    fn distance(&self, mock_value: &Option<&HttpMockRegex>, req_value: &Option<&String>) -> usize {
        let rv = req_value.map_or("", |s| s.as_str());
        let mut mv = &HttpMockRegex(regex::Regex::new(".*").unwrap());
        if mock_value.is_some() {
            mv = mock_value.unwrap()
        };
        regex_unmatched_length(rv, mv)
    }
}

// ************************************************************************************************
// IntegerExactMatchComparator
// ************************************************************************************************
pub struct U16ExactMatchComparator {
    negated: bool,
}

impl U16ExactMatchComparator {
    pub fn new(negated: bool) -> Self {
        Self { negated }
    }
}

impl ValueComparator<u16, u16> for U16ExactMatchComparator {
    fn matches(&self, mock_value: &Option<&u16>, req_value: &Option<&u16>) -> bool {
        comparison::integer_equals(self.negated, mock_value, req_value)
    }

    fn name(&self) -> &str {
        if self.negated {
            "not equal to"
        } else {
            "equals"
        }
    }

    fn distance(&self, mock_value: &Option<&u16>, req_value: &Option<&u16>) -> usize {
        comparison::distance_for_usize(mock_value, req_value)
    }
}

// ************************************************************************************************
// BytesExactMatchComparator
// ************************************************************************************************
pub struct BytesExactMatchComparator {
    negated: bool,
}

impl BytesExactMatchComparator {
    pub fn new(negated: bool) -> Self {
        Self { negated }
    }
}

impl ValueComparator<HttpMockBytes, HttpMockBytes> for BytesExactMatchComparator {
    fn matches(
        &self,
        mock_value: &Option<&HttpMockBytes>,
        req_value: &Option<&HttpMockBytes>,
    ) -> bool {
        comparison::bytes_equal(self.negated, mock_value, req_value)
    }

    fn name(&self) -> &str {
        if self.negated {
            "not equal to"
        } else {
            "equals"
        }
    }

    fn distance(
        &self,
        mock_value: &Option<&HttpMockBytes>,
        req_value: &Option<&HttpMockBytes>,
    ) -> usize {
        let mock_slice = mock_value
            .as_ref()
            .map(|mv| mv.to_bytes().clone())
            .unwrap_or_default();

        let req_slice = req_value
            .as_ref()
            .map(|rv| rv.to_bytes().clone())
            .unwrap_or_default();

        distance_for(mock_slice.as_ref(), req_slice.as_ref())
    }
}

// ************************************************************************************************
// BytesExactMatchComparator
// ************************************************************************************************
pub struct BytesIncludesComparator {
    negated: bool,
}

impl BytesIncludesComparator {
    pub fn new(negated: bool) -> Self {
        Self { negated }
    }
}

impl ValueComparator<HttpMockBytes, HttpMockBytes> for BytesIncludesComparator {
    fn matches(
        &self,
        mock_value: &Option<&HttpMockBytes>,
        req_value: &Option<&HttpMockBytes>,
    ) -> bool {
        comparison::bytes_includes(self.negated, mock_value, req_value)
    }

    fn name(&self) -> &str {
        if self.negated {
            "excludes"
        } else {
            "includes"
        }
    }

    fn distance(
        &self,
        mock_value: &Option<&HttpMockBytes>,
        req_value: &Option<&HttpMockBytes>,
    ) -> usize {
        let mock_slice = mock_value
            .as_ref()
            .map(|mv| mv.to_bytes().clone())
            .unwrap_or_default();

        let req_slice = req_value
            .as_ref()
            .map(|rv| rv.to_bytes().clone())
            .unwrap_or_default();

        distance_for(mock_slice.as_ref(), req_slice.as_ref())
    }
}

// ************************************************************************************************
// BytesPrefixComparator
// ************************************************************************************************
pub struct BytesPrefixComparator {
    negated: bool,
}

impl BytesPrefixComparator {
    pub fn new(negated: bool) -> Self {
        Self { negated }
    }
}

impl ValueComparator<HttpMockBytes, HttpMockBytes> for BytesPrefixComparator {
    fn matches(
        &self,
        mock_value: &Option<&HttpMockBytes>,
        req_value: &Option<&HttpMockBytes>,
    ) -> bool {
        comparison::bytes_prefix(self.negated, mock_value, req_value)
    }

    fn name(&self) -> &str {
        if self.negated {
            return "prefix not";
        }

        "has prefix"
    }

    fn distance(
        &self,
        mock_value: &Option<&HttpMockBytes>,
        req_value: &Option<&HttpMockBytes>,
    ) -> usize {
        let mock_slice = mock_value
            .as_ref()
            .map(|mv| mv.to_bytes().clone())
            .unwrap_or_default();

        let req_slice = req_value
            .as_ref()
            .map(|rv| rv.to_bytes().clone())
            .unwrap_or_default();

        // If mock has no requirement, distance is always 0
        if mock_value.is_none() || mock_slice.is_empty() {
            return 0;
        }

        // If request does not contain any data
        if req_value.is_none() || req_slice.is_empty() {
            return mock_slice.len();
        }

        // Compare only up to the length of the mock_slice
        let compared_window = std::cmp::min(mock_slice.len(), req_slice.len());
        let distance = equal_weight_distance_for(
            &mock_slice[..compared_window],
            &req_slice[..compared_window],
        );

        // if negated, we want to find out how many
        if self.negated {
            // This is why we need the equal_weight_distance_for function:
            // to calculate the distance as the number of differing characters.
            compared_window - distance
        } else {
            distance
        }
    }
}

// ************************************************************************************************
// BytesSuffixComparator
// ************************************************************************************************
pub struct BytesSuffixComparator {
    negated: bool,
}

impl BytesSuffixComparator {
    pub fn new(negated: bool) -> Self {
        Self { negated }
    }
}

impl ValueComparator<HttpMockBytes, HttpMockBytes> for BytesSuffixComparator {
    fn matches(
        &self,
        mock_value: &Option<&HttpMockBytes>,
        req_value: &Option<&HttpMockBytes>,
    ) -> bool {
        comparison::bytes_suffix(self.negated, mock_value, req_value)
    }

    fn name(&self) -> &str {
        if self.negated {
            "suffix not"
        } else {
            "has suffix"
        }
    }

    fn distance(
        &self,
        mock_value: &Option<&HttpMockBytes>,
        req_value: &Option<&HttpMockBytes>,
    ) -> usize {
        let mock_slice = mock_value
            .as_ref()
            .map(|mv| mv.to_bytes().clone())
            .unwrap_or_default();

        let req_slice = req_value
            .as_ref()
            .map(|rv| rv.to_bytes().clone())
            .unwrap_or_default();

        // If mock has no requirement, distance is always 0
        if mock_value.is_none() || mock_slice.is_empty() {
            return 0;
        }

        // If request does not contain any data
        if req_value.is_none() || req_slice.is_empty() {
            return mock_slice.len();
        }

        // Compare only up to the length of the mock_slice
        let compared_window = std::cmp::min(mock_slice.len(), req_slice.len());
        let distance = equal_weight_distance_for(
            &mock_slice[..compared_window],
            &req_slice[req_slice.len() - compared_window..],
        );

        // if negated, we want to find out how many
        if self.negated {
            // This is why we need the equal_weight_distance_for function:
            // to calculate the distance as the number of differing characters.
            compared_window - distance
        } else {
            distance
        }
    }
}

// ************************************************************************************************
// AnyValueComparator
// ************************************************************************************************
pub struct AnyValueComparator {}

impl AnyValueComparator {
    pub fn new() -> Self {
        Self {}
    }
}

impl<T, U> ValueComparator<T, U> for AnyValueComparator {
    fn matches(&self, _: &Option<&T>, _: &Option<&U>) -> bool {
        true
    }
    fn name(&self) -> &str {
        "any"
    }

    fn distance(&self, _: &Option<&T>, _: &Option<&U>) -> usize {
        0
    }
}

// ************************************************************************************************
// FunctionMatchComparator
// ************************************************************************************************
pub struct FunctionMatchesRequestComparator {
    negated: bool,
}

impl FunctionMatchesRequestComparator {
    pub fn new(negated: bool) -> Self {
        Self { negated }
    }
}

impl ValueComparator<Arc<dyn Fn(&HttpMockRequest) -> bool + 'static + Sync + Send>, HttpMockRequest>
    for FunctionMatchesRequestComparator
{
    fn matches(
        &self,
        mock_value: &Option<&Arc<dyn Fn(&HttpMockRequest) -> bool + 'static + Sync + Send>>,
        req_value: &Option<&HttpMockRequest>,
    ) -> bool {
        let result = match (mock_value, req_value) {
            (None, _) => true,
            (Some(_), None) => self.negated,
            (Some(mv), Some(rv)) => mv(rv),
        };

        if self.negated {
            !result
        } else {
            result
        }
    }

    fn name(&self) -> &str {
        "matches"
    }

    fn distance(
        &self,
        mock_value: &Option<&Arc<dyn Fn(&HttpMockRequest) -> bool + 'static + Sync + Send>>,
        req_value: &Option<&HttpMockRequest>,
    ) -> usize {
        let result = match self.matches(mock_value, req_value) {
            true => 0,
            false => 1,
        };

        if self.negated {
            return match result {
                0 => 1,
                _ => 0,
            };
        }

        result
    }
}

#[cfg(test)]
mod test {
    use crate::{
        common::data::HttpMockRegex,
        server::matchers::comparators::{
            AnyValueComparator, JSONContainsMatchComparator, JSONExactMatchComparator,
            StringContainsComparator, StringEqualsComparator, StringRegexMatchComparator,
            ValueComparator,
        },
    };
    use regex::Regex;
    use serde_json::json;

    fn run_test<S, T>(
        comparator: &dyn ValueComparator<S, T>,
        v1: &S,
        v2: &T,
        expected_match: bool,
        expected_distance: usize,
        expected_name: &str,
    ) {
        // Act
        let match_result = comparator.matches(&Some(v1), &Some(v2));
        let distance_result = comparator.distance(&Some(v1), &Some(v2));
        let name_result = comparator.name();

        // Assert
        assert_eq!(match_result, expected_match);
        assert_eq!(distance_result, expected_distance);
        assert_eq!(name_result, expected_name);
    }

    #[test]
    fn json_exact_match_comparator_match() {
        run_test(
            &JSONExactMatchComparator::new(),
            &json!({"name" : "Peter", "surname" : "Griffin"}),
            &json!({"name" : "Peter", "surname" : "Griffin"}),
            true,
            0,
            "equals",
        );
    }

    #[test]
    fn json_exact_match_comparator_no_match() {
        run_test(
            &JSONExactMatchComparator::new(),
            &json!({"name" : "Peter", "surname" : "Griffin"}),
            &json!({"name" : "Walter", "surname" : "White"}),
            false,
            9,
            "equals",
        );
    }

    #[test]
    fn json_contains_comparator_match() {
        run_test(
            &JSONContainsMatchComparator::new(false),
            &json!({ "other" : { "human" : { "surname" : "Griffin" }}}),
            &json!({ "name" : "Peter", "other" : { "human" : { "surname" : "Griffin" }}}),
            true,
            15, // compute distance even if values match!
            "includes",
        );
    }

    #[test]
    fn json_contains_comparator_no_match() {
        run_test(
            &JSONContainsMatchComparator::new(false),
            &json!({ "surname" : "Griffin" }),
            &json!({ "name" : "Peter", "other" : { "human" : { "surname" : "Griffin" }}}),
            false,
            35, // compute distance even if values match!
            "includes",
        );
    }

    #[test]
    fn string_exact_comparator_match() {
        run_test(
            &StringEqualsComparator::new(true, false),
            &"test string".to_string(),
            &"test string".to_string(),
            true,
            0, // compute distance even if values match!
            "equals",
        );
    }

    #[test]
    fn string_exact_comparator_no_match() {
        run_test(
            &StringEqualsComparator::new(true, false),
            &"test string".to_string(),
            &"not a test string".to_string(),
            false,
            6, // compute distance even if values match!
            "equals",
        );
    }

    #[test]
    fn string_exact_comparator_case_sensitive_match() {
        run_test(
            &StringEqualsComparator::new(false, false),
            &"TEST string".to_string(),
            &"test STRING".to_string(),
            true,
            0,
            "equals",
        );
    }

    #[test]
    fn string_contains_comparator_match() {
        run_test(
            &StringContainsComparator::new(true, false),
            &"st st".to_string(),
            &"test string".to_string(),
            true,
            6, // compute distance even if values match!
            "includes",
        );
    }

    #[test]
    fn string_contains_comparator_no_match() {
        run_test(
            &StringContainsComparator::new(true, false),
            &"xxx".to_string(),
            &"yyy".to_string(),
            false,
            3, // compute distance even if values match!
            "includes",
        );
    }

    #[test]
    fn string_contains_comparator_case_sensitive_match() {
        run_test(
            &StringContainsComparator::new(false, false),
            &"ST st".to_string(),
            &"test STRING".to_string(),
            true,
            6,
            "includes",
        );
    }

    #[test]
    fn regex_comparator_match() {
        run_test(
            &StringRegexMatchComparator::new(),
            &HttpMockRegex(Regex::new(r"^\d{4}-\d{2}-\d{2}$").unwrap()),
            &"2014-01-01".to_string(),
            true,
            0, // compute distance even if values match!
            "matches regex",
        );
    }

    #[test]
    fn regex_comparator_no_match() {
        run_test(
            &StringRegexMatchComparator::new(),
            &HttpMockRegex(Regex::new(r"^\d{4}-\d{2}-\d{2}$").unwrap()),
            &"xxx".to_string(),
            false,
            3,
            "matches regex",
        );
    }

    #[test]
    fn any_comparator_match() {
        run_test(
            &AnyValueComparator::new(),
            &"00000000".to_string(),
            &"xxx".to_string(),
            true,
            0, // compute distance even if values match!
            "any",
        );
    }
}
