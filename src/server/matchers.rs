use crate::{
    common::data::{HttpMockRequest, Mismatch, RequestRequirements, Tokenizer},
    server::matchers::{
        comparators::{
            AnyValueComparator, BytesEqualsComparator, BytesIncludesComparator, BytesMatchesComparator,
            BytesPrefixComparator, BytesSuffixComparator, FunctionMatchesRequestComparator, HostEqualsComparator,
            JsonEqualsComparator, JsonIncludesComparator, StringEqualsComparator, StringIncludesComparator,
            StringMatchesComparator, StringPrefixComparator, StringSuffixComparator, U16EqualsComparator,
        },
        generic::{
            FunctionValueMatcher, KeyValueOperator, MatchingStrategy, MultiValueCountMatcher, MultiValueMatcher,
            SingleValueMatcher,
        },
    },
};

mod comparators;
mod comparison;
pub(crate) mod generic;
mod readers;

// Builds the matchers of a request part with a single value, like the path or the body. Each
// variant names the `When` method, which is also the name of its expectation reader, and its
// comparator. The negated variants match by the value's absence.
macro_rules! single_value_matchers {
    (@strategy equals) => { MatchingStrategy::Presence };
    (@strategy not) => { MatchingStrategy::Absence };
    (@strategy includes) => { MatchingStrategy::Presence };
    (@strategy excludes) => { MatchingStrategy::Absence };
    (@strategy prefix) => { MatchingStrategy::Presence };
    (@strategy prefix_not) => { MatchingStrategy::Absence };
    (@strategy suffix) => { MatchingStrategy::Presence };
    (@strategy suffix_not) => { MatchingStrategy::Absence };
    (@strategy matches) => { MatchingStrategy::Presence };
    (
        $entity:literal, $request_value:ident, weight: $weight:literal, diff_with: $diff_with:expr,
        $($variant:ident: $method:ident => $comparator:expr),+ $(,)?
    ) => {
        vec![$(
            Box::new(SingleValueMatcher {
                entity_name: $entity,
                matcher_method: stringify!($method),
                matching_strategy: single_value_matchers!(@strategy $variant),
                comparator: Box::new($comparator),
                expectation: readers::expectations::$method,
                request_value: readers::request_value::$request_value,
                diff_with: $diff_with,
                weight: $weight,
            }) as Box<dyn Matcher + Sync + Send>
        ),+]
    };
}

// Builds the twelve matchers of a request part made of key-value pairs, like query parameters or
// headers. Only the entity, the request reader and whether keys are case-sensitive differ between
// them. `not`, `excludes`, `prefix_not` and `suffix_not` still require the key to be present and
// only negate the value comparison, so their key comparator isn't negated.
macro_rules! key_value_matchers {
    (
        @pair $entity:literal, $request_value:ident, $method:ident, $strategy:ident, $operator:ident,
        key_required: $key_required:literal, key: $key:expr, value: $value:expr
    ) => {
        Box::new(MultiValueMatcher {
            entity_name: $entity,
            matcher_method: stringify!($method),
            matching_strategy: MatchingStrategy::$strategy,
            operator: KeyValueOperator::$operator,
            expectation: readers::expectations::$method,
            request_value: readers::request_value::$request_value,
            key_required: $key_required,
            key_comparator: Box::new($key),
            value_comparator: Box::new($value),
            weight: 1,
        }) as Box<dyn Matcher + Sync + Send>
    };
    (
        $entity:literal, $request_value:ident, case_sensitive_keys: $keys:literal,
        equals: $equals:ident, not: $not:ident, exists: $exists:ident, missing: $missing:ident,
        includes: $includes:ident, excludes: $excludes:ident, prefix: $prefix:ident,
        prefix_not: $prefix_not:ident, suffix: $suffix:ident, suffix_not: $suffix_not:ident,
        matches: $matches:ident, count: $count:ident $(,)?
    ) => {
        vec![
            key_value_matchers!(@pair $entity, $request_value, $equals, Presence, And,
                key_required: true, key: StringEqualsComparator::new($keys, false),
                value: StringEqualsComparator::new(true, false)),
            key_value_matchers!(@pair $entity, $request_value, $not, Absence, Implication,
                key_required: true, key: StringEqualsComparator::new($keys, false),
                value: StringEqualsComparator::new(true, true)),
            key_value_matchers!(@pair $entity, $request_value, $exists, Presence, And,
                key_required: true, key: StringEqualsComparator::new($keys, false),
                value: AnyValueComparator::new()),
            key_value_matchers!(@pair $entity, $request_value, $missing, Absence, And,
                key_required: false, key: StringEqualsComparator::new($keys, true),
                value: AnyValueComparator::new()),
            key_value_matchers!(@pair $entity, $request_value, $includes, Presence, And,
                key_required: true, key: StringEqualsComparator::new($keys, false),
                value: StringIncludesComparator::new(true, false)),
            key_value_matchers!(@pair $entity, $request_value, $excludes, Absence, Implication,
                key_required: true, key: StringEqualsComparator::new($keys, false),
                value: StringIncludesComparator::new(true, true)),
            key_value_matchers!(@pair $entity, $request_value, $prefix, Presence, And,
                key_required: true, key: StringEqualsComparator::new($keys, false),
                value: StringPrefixComparator::new(true, false)),
            key_value_matchers!(@pair $entity, $request_value, $prefix_not, Absence, Implication,
                key_required: true, key: StringEqualsComparator::new($keys, false),
                value: StringPrefixComparator::new(true, true)),
            key_value_matchers!(@pair $entity, $request_value, $suffix, Presence, And,
                key_required: true, key: StringEqualsComparator::new($keys, false),
                value: StringSuffixComparator::new(true, false)),
            key_value_matchers!(@pair $entity, $request_value, $suffix_not, Absence, Implication,
                key_required: true, key: StringEqualsComparator::new($keys, false),
                value: StringSuffixComparator::new(true, true)),
            key_value_matchers!(@pair $entity, $request_value, $matches, Presence, And,
                key_required: true, key: StringMatchesComparator::new($keys, false),
                value: StringMatchesComparator::new(true, false)),
            Box::new(MultiValueCountMatcher {
                entity_name: $entity,
                matcher_method: stringify!($count),
                expectation: readers::expectations::$count,
                request_value: readers::request_value::$request_value,
                key_comparator: Box::new(StringMatchesComparator::new($keys, false)),
                value_comparator: Box::new(StringMatchesComparator::new(true, false)),
                weight: 1,
            }),
        ]
    };
}

pub(crate) fn all() -> Vec<Box<dyn Matcher + Sync + Send>> {
    [
        single_value_matchers!("scheme", scheme, weight: 3, diff_with: None,
            equals: scheme => StringEqualsComparator::new(false, false),
            not: scheme_not => StringEqualsComparator::new(false, true),
        ),
        single_value_matchers!("method", method, weight: 3, diff_with: None,
            equals: method => StringEqualsComparator::new(false, false),
            not: method_not => StringEqualsComparator::new(false, true),
        ),
        single_value_matchers!("host", host, weight: 3, diff_with: None,
            equals: host => HostEqualsComparator::new(false),
            not: host_not => HostEqualsComparator::new(true),
            includes: host_includes => StringIncludesComparator::new(false, false),
            excludes: host_excludes => StringIncludesComparator::new(false, true),
            prefix: host_prefix => StringPrefixComparator::new(false, false),
            prefix_not: host_prefix_not => StringPrefixComparator::new(false, true),
            suffix: host_suffix => StringSuffixComparator::new(false, false),
            suffix_not: host_suffix_not => StringSuffixComparator::new(false, true),
            matches: host_matches => StringMatchesComparator::new(true, false),
        ),
        single_value_matchers!("port", port, weight: 2, diff_with: None,
            equals: port => U16EqualsComparator::new(false),
            not: port_not => U16EqualsComparator::new(true),
        ),
        single_value_matchers!("path", path, weight: 10, diff_with: None,
            equals: path => StringEqualsComparator::new(true, false),
            not: path_not => StringEqualsComparator::new(true, true),
            includes: path_includes => StringIncludesComparator::new(true, false),
            excludes: path_excludes => StringIncludesComparator::new(true, true),
            prefix: path_prefix => StringPrefixComparator::new(true, false),
            prefix_not: path_prefix_not => StringPrefixComparator::new(true, true),
            suffix: path_suffix => StringSuffixComparator::new(true, false),
            suffix_not: path_suffix_not => StringSuffixComparator::new(true, true),
            matches: path_matches => StringMatchesComparator::new(true, false),
        ),
        key_value_matchers!("query parameter", query_params, case_sensitive_keys: true,
            equals: query_param, not: query_param_not, exists: query_param_exists,
            missing: query_param_missing, includes: query_param_includes,
            excludes: query_param_excludes, prefix: query_param_prefix,
            prefix_not: query_param_prefix_not, suffix: query_param_suffix,
            suffix_not: query_param_suffix_not, matches: query_param_matches,
            count: query_param_count,
        ),
        key_value_matchers!("header", headers, case_sensitive_keys: false,
            equals: header, not: header_not, exists: header_exists, missing: header_missing,
            includes: header_includes, excludes: header_excludes, prefix: header_prefix,
            prefix_not: header_prefix_not, suffix: header_suffix, suffix_not: header_suffix_not,
            matches: header_matches, count: header_count,
        ),
        #[cfg(feature = "cookies")]
        key_value_matchers!("cookie", cookies, case_sensitive_keys: true,
            equals: cookie, not: cookie_not, exists: cookie_exists, missing: cookie_missing,
            includes: cookie_includes, excludes: cookie_excludes, prefix: cookie_prefix,
            prefix_not: cookie_prefix_not, suffix: cookie_suffix, suffix_not: cookie_suffix_not,
            matches: cookie_matches, count: cookie_count,
        ),
        single_value_matchers!("body", body, weight: 1, diff_with: Some(Tokenizer::Line),
            equals: body => BytesEqualsComparator::new(false),
            not: body_not => BytesEqualsComparator::new(true),
            includes: body_includes => BytesIncludesComparator::new(false),
            excludes: body_excludes => BytesIncludesComparator::new(true),
            prefix: body_prefix => BytesPrefixComparator::new(false),
            prefix_not: body_prefix_not => BytesPrefixComparator::new(true),
            suffix: body_suffix => BytesSuffixComparator::new(false),
            suffix_not: body_suffix_not => BytesSuffixComparator::new(true),
            matches: body_matches => BytesMatchesComparator::new(),
        ),
        single_value_matchers!("body", json_body, weight: 1, diff_with: Some(Tokenizer::Line),
            equals: json_body => JsonEqualsComparator::new(),
        ),
        single_value_matchers!("JSON body", json_body, weight: 1, diff_with: Some(Tokenizer::Line),
            includes: json_body_includes => JsonIncludesComparator::new(false),
            excludes: json_body_excludes => JsonIncludesComparator::new(true),
        ),
        vec![
            Box::new(FunctionValueMatcher {
                entity_name: "custom matcher function",
                matcher_function: "is_true",
                comparator: Box::new(FunctionMatchesRequestComparator::new(false)),
                expectation: readers::expectations::is_true,
                request_value: readers::request_value::full_request,
                weight: 1,
            }) as Box<dyn Matcher + Sync + Send>,
            Box::new(FunctionValueMatcher {
                entity_name: "custom matcher function",
                matcher_function: "is_false",
                comparator: Box::new(FunctionMatchesRequestComparator::new(true)),
                expectation: readers::expectations::is_false,
                request_value: readers::request_value::full_request,
                weight: 1,
            }),
        ],
        key_value_matchers!("form-urlencoded body", form_urlencoded_body, case_sensitive_keys: true,
            equals: form_urlencoded_tuple, not: form_urlencoded_tuple_not,
            exists: form_urlencoded_tuple_exists, missing: form_urlencoded_tuple_missing,
            includes: form_urlencoded_tuple_includes, excludes: form_urlencoded_tuple_excludes,
            prefix: form_urlencoded_tuple_prefix, prefix_not: form_urlencoded_tuple_prefix_not,
            suffix: form_urlencoded_tuple_suffix, suffix_not: form_urlencoded_tuple_suffix_not,
            matches: form_urlencoded_tuple_matches, count: form_urlencoded_tuple_count,
        ),
    ]
    .into_iter()
    .flatten()
    .collect()
}

pub(crate) trait Matcher {
    fn matches(&self, req: &HttpMockRequest, mock: &RequestRequirements) -> bool;
    fn distance(&self, req: &HttpMockRequest, mock: &RequestRequirements) -> usize;
    fn mismatches(&self, req: &HttpMockRequest, mock: &RequestRequirements) -> Vec<Mismatch>;
}
