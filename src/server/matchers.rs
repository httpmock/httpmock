use std::fmt::Display;

use serde_json::Value;

use crate::{
    common::{
        data::{HttpMockRegex, HttpMockRequest, Mismatch, RequestRequirements, Tokenizer},
        util::HttpMockBytes,
    },
    server::matchers::{
        comparators::{
            AnyValueComparator, BytesEqualsComparator, BytesIncludesComparator, BytesMatchesComparator,
            BytesPrefixComparator, BytesSuffixComparator, FunctionMatchesRequestComparator, HostEqualsComparator,
            JsonEqualsComparator, JsonIncludesComparator, StringEqualsComparator, StringIncludesComparator,
            StringMatchesComparator, StringPrefixComparator, StringSuffixComparator, U16EqualsComparator,
            ValueComparator,
        },
        generic::{
            FunctionValueMatcher, KeyValueRule, MatchingStrategy, MultiValueCountExpectation, MultiValueCountMatcher,
            MultiValueExpectation, MultiValueMatcher, MultiValueRequest, SingleValueExpectation, SingleValueMatcher,
        },
        readers::{expectations, request_value},
    },
};

mod comparators;
mod comparison;
pub(crate) mod generic;
mod readers;

/// A request part with a single value, like the path or the body.
struct Part<T> {
    entity: &'static str,
    request_value: fn(&HttpMockRequest) -> Option<T>,
    weight: usize,
    diff_with: Option<Tokenizer>,
}

impl<T: Display> Part<T> {
    /// A matcher for a `When` method like `path_prefix`, which requires a matching value.
    const fn present<S: Display, C: ValueComparator<S, T>>(
        &self,
        method: &'static str,
        expectation: SingleValueExpectation<S>,
        comparator: C,
    ) -> SingleValueMatcher<S, T, C> {
        self.matcher(method, MatchingStrategy::Presence, expectation, comparator)
    }

    /// A matcher for a negated `When` method like `path_prefix_not`, which requires the
    /// value's absence.
    const fn absent<S: Display, C: ValueComparator<S, T>>(
        &self,
        method: &'static str,
        expectation: SingleValueExpectation<S>,
        comparator: C,
    ) -> SingleValueMatcher<S, T, C> {
        self.matcher(method, MatchingStrategy::Absence, expectation, comparator)
    }

    const fn matcher<S: Display, C: ValueComparator<S, T>>(
        &self,
        method: &'static str,
        matching_strategy: MatchingStrategy,
        expectation: SingleValueExpectation<S>,
        comparator: C,
    ) -> SingleValueMatcher<S, T, C> {
        SingleValueMatcher {
            entity_name: self.entity,
            matcher_method: method,
            matching_strategy,
            expectation,
            request_value: self.request_value,
            comparator,
            diff_with: self.diff_with,
            weight: self.weight,
        }
    }
}

/// A request part made of key-value pairs, like query parameters or headers. Its twelve `When`
/// methods only differ in how they compare, so each one gets a constructor here.
struct KeyValuePart {
    entity: &'static str,
    request_value: MultiValueRequest<String, String>,
    case_sensitive_keys: bool,
}

type KeyValueExpectation = MultiValueExpectation<String, String>;
type KeyValueMatcher<C> = MultiValueMatcher<String, String, String, String, StringEqualsComparator, C>;
type KeyValueRegexMatcher =
    MultiValueMatcher<HttpMockRegex, HttpMockRegex, String, String, StringMatchesComparator, StringMatchesComparator>;
type KeyValueCountMatcher = MultiValueCountMatcher<
    HttpMockRegex,
    HttpMockRegex,
    String,
    String,
    StringMatchesComparator,
    StringMatchesComparator,
>;

// `not`, `excludes`, `prefix_not` and `suffix_not` still require the key to be present and only
// negate the value comparison, so their key comparator isn't negated. Only `missing` negates
// the key.
impl KeyValuePart {
    const fn key_equals(&self) -> StringEqualsComparator {
        if self.case_sensitive_keys {
            StringEqualsComparator::new()
        } else {
            StringEqualsComparator::new().ignore_case()
        }
    }

    const fn key_matches(&self) -> StringMatchesComparator {
        if self.case_sensitive_keys {
            StringMatchesComparator::new()
        } else {
            StringMatchesComparator::new().ignore_case()
        }
    }

    const fn equals(
        &self,
        method: &'static str,
        expectation: KeyValueExpectation,
    ) -> KeyValueMatcher<StringEqualsComparator> {
        let value = StringEqualsComparator::new();
        self.with_key(method, KeyValueRule::Present, expectation, value)
    }

    const fn not(
        &self,
        method: &'static str,
        expectation: KeyValueExpectation,
    ) -> KeyValueMatcher<StringEqualsComparator> {
        let value = StringEqualsComparator::new().negated();
        self.with_key(method, KeyValueRule::ValueAbsent, expectation, value)
    }

    const fn exists(
        &self,
        method: &'static str,
        expectation: KeyValueExpectation,
    ) -> KeyValueMatcher<AnyValueComparator> {
        let value = AnyValueComparator::new();
        self.with_key(method, KeyValueRule::Present, expectation, value)
    }

    const fn missing(
        &self,
        method: &'static str,
        expectation: KeyValueExpectation,
    ) -> KeyValueMatcher<AnyValueComparator> {
        MultiValueMatcher {
            entity_name: self.entity,
            matcher_method: method,
            rule: KeyValueRule::KeyAbsent,
            expectation,
            request_value: self.request_value,
            key_comparator: self.key_equals().negated(),
            value_comparator: AnyValueComparator::new(),
            weight: 1,
        }
    }

    const fn includes(
        &self,
        method: &'static str,
        expectation: KeyValueExpectation,
    ) -> KeyValueMatcher<StringIncludesComparator> {
        let value = StringIncludesComparator::new();
        self.with_key(method, KeyValueRule::Present, expectation, value)
    }

    const fn excludes(
        &self,
        method: &'static str,
        expectation: KeyValueExpectation,
    ) -> KeyValueMatcher<StringIncludesComparator> {
        let value = StringIncludesComparator::new().negated();
        self.with_key(method, KeyValueRule::ValueAbsent, expectation, value)
    }

    const fn prefix(
        &self,
        method: &'static str,
        expectation: KeyValueExpectation,
    ) -> KeyValueMatcher<StringPrefixComparator> {
        let value = StringPrefixComparator::new();
        self.with_key(method, KeyValueRule::Present, expectation, value)
    }

    const fn prefix_not(
        &self,
        method: &'static str,
        expectation: KeyValueExpectation,
    ) -> KeyValueMatcher<StringPrefixComparator> {
        let value = StringPrefixComparator::new().negated();
        self.with_key(method, KeyValueRule::ValueAbsent, expectation, value)
    }

    const fn suffix(
        &self,
        method: &'static str,
        expectation: KeyValueExpectation,
    ) -> KeyValueMatcher<StringSuffixComparator> {
        let value = StringSuffixComparator::new();
        self.with_key(method, KeyValueRule::Present, expectation, value)
    }

    const fn suffix_not(
        &self,
        method: &'static str,
        expectation: KeyValueExpectation,
    ) -> KeyValueMatcher<StringSuffixComparator> {
        let value = StringSuffixComparator::new().negated();
        self.with_key(method, KeyValueRule::ValueAbsent, expectation, value)
    }

    const fn matches(
        &self,
        method: &'static str,
        expectation: MultiValueExpectation<HttpMockRegex, HttpMockRegex>,
    ) -> KeyValueRegexMatcher {
        MultiValueMatcher {
            entity_name: self.entity,
            matcher_method: method,
            rule: KeyValueRule::Present,
            expectation,
            request_value: self.request_value,
            key_comparator: self.key_matches(),
            value_comparator: StringMatchesComparator::new(),
            weight: 1,
        }
    }

    const fn count(
        &self,
        method: &'static str,
        expectation: MultiValueCountExpectation<HttpMockRegex, HttpMockRegex>,
    ) -> KeyValueCountMatcher {
        MultiValueCountMatcher {
            entity_name: self.entity,
            matcher_method: method,
            expectation,
            request_value: self.request_value,
            key_comparator: self.key_matches(),
            value_comparator: StringMatchesComparator::new(),
            weight: 1,
        }
    }

    /// A matcher that compares the key exactly.
    const fn with_key<C: ValueComparator<String, String>>(
        &self,
        method: &'static str,
        rule: KeyValueRule,
        expectation: KeyValueExpectation,
        value_comparator: C,
    ) -> KeyValueMatcher<C> {
        MultiValueMatcher {
            entity_name: self.entity,
            matcher_method: method,
            rule,
            expectation,
            request_value: self.request_value,
            key_comparator: self.key_equals(),
            value_comparator,
            weight: 1,
        }
    }
}

const SCHEME: Part<String> = Part {
    entity: "scheme",
    request_value: request_value::scheme,
    weight: 3,
    diff_with: None,
};
const METHOD: Part<String> = Part {
    entity: "method",
    request_value: request_value::method,
    ..SCHEME
};
const HOST: Part<String> = Part {
    entity: "host",
    request_value: request_value::host,
    ..SCHEME
};
const PORT: Part<u16> = Part {
    entity: "port",
    request_value: request_value::port,
    weight: 2,
    diff_with: None,
};
const PATH: Part<String> = Part {
    entity: "path",
    request_value: request_value::path,
    weight: 10,
    diff_with: None,
};
const BODY: Part<HttpMockBytes> = Part {
    entity: "body",
    request_value: request_value::body,
    weight: 1,
    diff_with: Some(Tokenizer::Line),
};
const JSON_BODY: Part<Value> = Part {
    entity: "body",
    request_value: request_value::json_body,
    weight: 1,
    diff_with: Some(Tokenizer::Line),
};
// `json_body_includes` and `json_body_excludes` report their mismatches under a name of their own.
const PARTIAL_JSON_BODY: Part<Value> = Part {
    entity: "JSON body",
    ..JSON_BODY
};

const QUERY_PARAMS: KeyValuePart = KeyValuePart {
    entity: "query parameter",
    request_value: request_value::query_params,
    case_sensitive_keys: true,
};
const HEADERS: KeyValuePart = KeyValuePart {
    entity: "header",
    request_value: request_value::headers,
    case_sensitive_keys: false,
};
#[cfg(feature = "cookies")]
const COOKIES: KeyValuePart = KeyValuePart {
    entity: "cookie",
    request_value: request_value::cookies,
    case_sensitive_keys: true,
};
const FORM: KeyValuePart = KeyValuePart {
    entity: "form-urlencoded body",
    request_value: request_value::form_urlencoded_body,
    case_sensitive_keys: true,
};

// Grouped by request part, so the cookie matchers can be left out as a whole. One matcher per
// line reads better than what rustfmt makes of these calls.
#[rustfmt::skip]
static MATCHERS: &[&[&(dyn Matcher + Sync)]] = &[
    &[
        &SCHEME.present("scheme", expectations::scheme, StringEqualsComparator::new().ignore_case()),
        &SCHEME.absent("scheme_not", expectations::scheme_not, StringEqualsComparator::new().ignore_case().negated()),
    ],
    &[
        &METHOD.present("method", expectations::method, StringEqualsComparator::new().ignore_case()),
        &METHOD.absent("method_not", expectations::method_not, StringEqualsComparator::new().ignore_case().negated()),
    ],
    &[
        &HOST.present("host", expectations::host, HostEqualsComparator::new()),
        &HOST.absent("host_not", expectations::host_not, HostEqualsComparator::new().negated()),
        &HOST.present("host_includes", expectations::host_includes, StringIncludesComparator::new().ignore_case()),
        &HOST.absent(
            "host_excludes", expectations::host_excludes, StringIncludesComparator::new().ignore_case().negated(),
        ),
        &HOST.present("host_prefix", expectations::host_prefix, StringPrefixComparator::new().ignore_case()),
        &HOST.absent(
            "host_prefix_not", expectations::host_prefix_not, StringPrefixComparator::new().ignore_case().negated(),
        ),
        &HOST.present("host_suffix", expectations::host_suffix, StringSuffixComparator::new().ignore_case()),
        &HOST.absent(
            "host_suffix_not", expectations::host_suffix_not, StringSuffixComparator::new().ignore_case().negated(),
        ),
        &HOST.present("host_matches", expectations::host_matches, StringMatchesComparator::new()),
    ],
    &[
        &PORT.present("port", expectations::port, U16EqualsComparator::new()),
        &PORT.absent("port_not", expectations::port_not, U16EqualsComparator::new().negated()),
    ],
    &[
        &PATH.present("path", expectations::path, StringEqualsComparator::new()),
        &PATH.absent("path_not", expectations::path_not, StringEqualsComparator::new().negated()),
        &PATH.present("path_includes", expectations::path_includes, StringIncludesComparator::new()),
        &PATH.absent("path_excludes", expectations::path_excludes, StringIncludesComparator::new().negated()),
        &PATH.present("path_prefix", expectations::path_prefix, StringPrefixComparator::new()),
        &PATH.absent("path_prefix_not", expectations::path_prefix_not, StringPrefixComparator::new().negated()),
        &PATH.present("path_suffix", expectations::path_suffix, StringSuffixComparator::new()),
        &PATH.absent("path_suffix_not", expectations::path_suffix_not, StringSuffixComparator::new().negated()),
        &PATH.present("path_matches", expectations::path_matches, StringMatchesComparator::new()),
    ],
    &[
        &QUERY_PARAMS.equals("query_param", expectations::query_param),
        &QUERY_PARAMS.not("query_param_not", expectations::query_param_not),
        &QUERY_PARAMS.exists("query_param_exists", expectations::query_param_exists),
        &QUERY_PARAMS.missing("query_param_missing", expectations::query_param_missing),
        &QUERY_PARAMS.includes("query_param_includes", expectations::query_param_includes),
        &QUERY_PARAMS.excludes("query_param_excludes", expectations::query_param_excludes),
        &QUERY_PARAMS.prefix("query_param_prefix", expectations::query_param_prefix),
        &QUERY_PARAMS.prefix_not("query_param_prefix_not", expectations::query_param_prefix_not),
        &QUERY_PARAMS.suffix("query_param_suffix", expectations::query_param_suffix),
        &QUERY_PARAMS.suffix_not("query_param_suffix_not", expectations::query_param_suffix_not),
        &QUERY_PARAMS.matches("query_param_matches", expectations::query_param_matches),
        &QUERY_PARAMS.count("query_param_count", expectations::query_param_count),
    ],
    &[
        &HEADERS.equals("header", expectations::header),
        &HEADERS.not("header_not", expectations::header_not),
        &HEADERS.exists("header_exists", expectations::header_exists),
        &HEADERS.missing("header_missing", expectations::header_missing),
        &HEADERS.includes("header_includes", expectations::header_includes),
        &HEADERS.excludes("header_excludes", expectations::header_excludes),
        &HEADERS.prefix("header_prefix", expectations::header_prefix),
        &HEADERS.prefix_not("header_prefix_not", expectations::header_prefix_not),
        &HEADERS.suffix("header_suffix", expectations::header_suffix),
        &HEADERS.suffix_not("header_suffix_not", expectations::header_suffix_not),
        &HEADERS.matches("header_matches", expectations::header_matches),
        &HEADERS.count("header_count", expectations::header_count),
    ],
    #[cfg(feature = "cookies")]
    &[
        &COOKIES.equals("cookie", expectations::cookie),
        &COOKIES.not("cookie_not", expectations::cookie_not),
        &COOKIES.exists("cookie_exists", expectations::cookie_exists),
        &COOKIES.missing("cookie_missing", expectations::cookie_missing),
        &COOKIES.includes("cookie_includes", expectations::cookie_includes),
        &COOKIES.excludes("cookie_excludes", expectations::cookie_excludes),
        &COOKIES.prefix("cookie_prefix", expectations::cookie_prefix),
        &COOKIES.prefix_not("cookie_prefix_not", expectations::cookie_prefix_not),
        &COOKIES.suffix("cookie_suffix", expectations::cookie_suffix),
        &COOKIES.suffix_not("cookie_suffix_not", expectations::cookie_suffix_not),
        &COOKIES.matches("cookie_matches", expectations::cookie_matches),
        &COOKIES.count("cookie_count", expectations::cookie_count),
    ],
    &[
        &BODY.present("body", expectations::body, BytesEqualsComparator::new()),
        &BODY.absent("body_not", expectations::body_not, BytesEqualsComparator::new().negated()),
        &BODY.present("body_includes", expectations::body_includes, BytesIncludesComparator::new()),
        &BODY.absent("body_excludes", expectations::body_excludes, BytesIncludesComparator::new().negated()),
        &BODY.present("body_prefix", expectations::body_prefix, BytesPrefixComparator::new()),
        &BODY.absent("body_prefix_not", expectations::body_prefix_not, BytesPrefixComparator::new().negated()),
        &BODY.present("body_suffix", expectations::body_suffix, BytesSuffixComparator::new()),
        &BODY.absent("body_suffix_not", expectations::body_suffix_not, BytesSuffixComparator::new().negated()),
        &BODY.present("body_matches", expectations::body_matches, BytesMatchesComparator::new()),
    ],
    &[
        &JSON_BODY.present("json_body", expectations::json_body, JsonEqualsComparator::new()),
        &PARTIAL_JSON_BODY
            .present("json_body_includes", expectations::json_body_includes, JsonIncludesComparator::new()),
        &PARTIAL_JSON_BODY
            .absent("json_body_excludes", expectations::json_body_excludes, JsonIncludesComparator::new().negated()),
    ],
    &[
        &FunctionValueMatcher {
            entity_name: "custom matcher function",
            matcher_function: "is_true",
            comparator: FunctionMatchesRequestComparator::new(),
            expectation: expectations::is_true,
            request_value: request_value::full_request,
            weight: 1,
        },
        &FunctionValueMatcher {
            entity_name: "custom matcher function",
            matcher_function: "is_false",
            comparator: FunctionMatchesRequestComparator::new().negated(),
            expectation: expectations::is_false,
            request_value: request_value::full_request,
            weight: 1,
        },
    ],
    &[
        &FORM.equals("form_urlencoded_tuple", expectations::form_urlencoded_tuple),
        &FORM.not("form_urlencoded_tuple_not", expectations::form_urlencoded_tuple_not),
        &FORM.exists("form_urlencoded_tuple_exists", expectations::form_urlencoded_tuple_exists),
        &FORM.missing("form_urlencoded_tuple_missing", expectations::form_urlencoded_tuple_missing),
        &FORM.includes("form_urlencoded_tuple_includes", expectations::form_urlencoded_tuple_includes),
        &FORM.excludes("form_urlencoded_tuple_excludes", expectations::form_urlencoded_tuple_excludes),
        &FORM.prefix("form_urlencoded_tuple_prefix", expectations::form_urlencoded_tuple_prefix),
        &FORM.prefix_not("form_urlencoded_tuple_prefix_not", expectations::form_urlencoded_tuple_prefix_not),
        &FORM.suffix("form_urlencoded_tuple_suffix", expectations::form_urlencoded_tuple_suffix),
        &FORM.suffix_not("form_urlencoded_tuple_suffix_not", expectations::form_urlencoded_tuple_suffix_not),
        &FORM.matches("form_urlencoded_tuple_matches", expectations::form_urlencoded_tuple_matches),
        &FORM.count("form_urlencoded_tuple_count", expectations::form_urlencoded_tuple_count),
    ],
];

pub(crate) fn all() -> impl Iterator<Item = &'static (dyn Matcher + Sync)> {
    MATCHERS.iter().flat_map(|part| part.iter().copied())
}

pub(crate) trait Matcher {
    fn matches(&self, req: &HttpMockRequest, mock: &RequestRequirements) -> bool;
    fn distance(&self, req: &HttpMockRequest, mock: &RequestRequirements) -> usize;
    fn mismatches(&self, req: &HttpMockRequest, mock: &RequestRequirements) -> Vec<Mismatch>;
}
