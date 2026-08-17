//! The YAML representation of a mock definition.
//!
//! Recordings and file-based mock files are stored as `StaticMockDefinition` documents.

use std::{convert::TryInto, str::FromStr};

use base64::{Engine, engine::general_purpose::STANDARD as BASE64};
use bytes::Bytes;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::common::{
    data::{
        Error, Error::StaticMockConversion, HttpMockRegex, Method, MockDefinition, MockServerHttpResponse,
        RequestRequirements,
    },
    util::HttpMockBytes,
};

#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub struct NameValueStringPair {
    name: String,
    value: String,
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub struct NameValuePatternPair {
    name: HttpMockRegex,
    value: HttpMockRegex,
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub struct KeyValuePatternCountTriple {
    name: HttpMockRegex,
    value: HttpMockRegex,
    count: usize,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct StaticRequestRequirements {
    // Scheme-related fields
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scheme: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scheme_not: Option<String>,

    // Host-related fields
    #[serde(skip_serializing_if = "Option::is_none")]
    pub host: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub host_not: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub host_contains: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub host_excludes: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub host_prefix: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub host_suffix: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub host_prefix_not: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub host_suffix_not: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub host_matches: Option<Vec<HttpMockRegex>>,

    // Port-related fields
    #[serde(skip_serializing_if = "Option::is_none")]
    pub port: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub port_not: Option<Vec<u16>>,

    // Path-related fields
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path_not: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path_contains: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path_excludes: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path_prefix: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path_suffix: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path_prefix_not: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path_suffix_not: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path_matches: Option<Vec<HttpMockRegex>>,

    // Method-related fields
    #[serde(skip_serializing_if = "Option::is_none")]
    pub method: Option<Method>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub method_not: Option<Vec<Method>>,

    // Query Parameter-related fields
    #[serde(skip_serializing_if = "Option::is_none")]
    pub query_param: Option<Vec<NameValueStringPair>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub query_param_not: Option<Vec<NameValueStringPair>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub query_param_exists: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub query_param_missing: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub query_param_contains: Option<Vec<NameValueStringPair>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub query_param_excludes: Option<Vec<NameValueStringPair>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub query_param_prefix: Option<Vec<NameValueStringPair>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub query_param_suffix: Option<Vec<NameValueStringPair>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub query_param_prefix_not: Option<Vec<NameValueStringPair>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub query_param_suffix_not: Option<Vec<NameValueStringPair>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub query_param_matches: Option<Vec<NameValuePatternPair>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub query_param_count: Option<Vec<KeyValuePatternCountTriple>>,

    // Header-related fields
    #[serde(skip_serializing_if = "Option::is_none")]
    pub header: Option<Vec<NameValueStringPair>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub header_not: Option<Vec<NameValueStringPair>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub header_exists: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub header_missing: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub header_contains: Option<Vec<NameValueStringPair>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub header_excludes: Option<Vec<NameValueStringPair>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub header_prefix: Option<Vec<NameValueStringPair>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub header_suffix: Option<Vec<NameValueStringPair>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub header_prefix_not: Option<Vec<NameValueStringPair>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub header_suffix_not: Option<Vec<NameValueStringPair>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub header_matches: Option<Vec<NameValuePatternPair>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub header_count: Option<Vec<KeyValuePatternCountTriple>>,

    // Cookie-related fields
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cookie: Option<Vec<NameValueStringPair>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cookie_not: Option<Vec<NameValueStringPair>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cookie_exists: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cookie_missing: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cookie_contains: Option<Vec<NameValueStringPair>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cookie_excludes: Option<Vec<NameValueStringPair>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cookie_prefix: Option<Vec<NameValueStringPair>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cookie_suffix: Option<Vec<NameValueStringPair>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cookie_prefix_not: Option<Vec<NameValueStringPair>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cookie_suffix_not: Option<Vec<NameValueStringPair>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cookie_matches: Option<Vec<NameValuePatternPair>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cookie_count: Option<Vec<KeyValuePatternCountTriple>>,

    // Body-related fields
    #[serde(skip_serializing_if = "Option::is_none")]
    pub body: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub body_base64: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub body_not: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub body_not_base64: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub body_contains: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub body_contains_base64: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub body_excludes: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub body_excludes_base64: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub body_prefix: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub body_prefix_base64: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub body_suffix: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub body_suffix_base64: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub body_prefix_not: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub body_prefix_not_base64: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub body_suffix_not: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub body_suffix_not_base64: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub body_matches: Option<Vec<HttpMockRegex>>,

    // JSON Body-related fields
    #[serde(skip_serializing_if = "Option::is_none")]
    pub json_body: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub json_body_not: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub json_body_includes: Option<Vec<Value>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub json_body_excludes: Option<Vec<Value>>,

    // x-www-form-urlencoded fields
    #[serde(skip_serializing_if = "Option::is_none")]
    pub form_urlencoded_tuple: Option<Vec<NameValueStringPair>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub form_urlencoded_tuple_not: Option<Vec<NameValueStringPair>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub form_urlencoded_key_exists: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub form_urlencoded_key_missing: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub form_urlencoded_contains: Option<Vec<NameValueStringPair>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub form_urlencoded_excludes: Option<Vec<NameValueStringPair>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub form_urlencoded_prefix: Option<Vec<NameValueStringPair>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub form_urlencoded_suffix: Option<Vec<NameValueStringPair>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub form_urlencoded_prefix_not: Option<Vec<NameValueStringPair>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub form_urlencoded_suffix_not: Option<Vec<NameValueStringPair>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub form_urlencoded_matches: Option<Vec<NameValuePatternPair>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub form_urlencoded_count: Option<Vec<KeyValuePatternCountTriple>>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct StaticHTTPResponse {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub header: Option<Vec<NameValueStringPair>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub body: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub body_base64: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub delay: Option<u64>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct StaticMockDefinition {
    when: StaticRequestRequirements,
    then: StaticHTTPResponse,
}

impl TryInto<MockDefinition> for StaticMockDefinition {
    type Error = Error;

    fn try_into(self) -> Result<MockDefinition, Self::Error> {
        Ok(MockDefinition {
            request: RequestRequirements {
                // Scheme-related fields
                scheme: self.when.scheme,
                scheme_not: self.when.scheme_not,

                // Host-related fields
                host: self.when.host,
                host_not: self.when.host_not,
                host_contains: self.when.host_contains,
                host_excludes: self.when.host_excludes,
                host_prefix: self.when.host_prefix,
                host_suffix: self.when.host_suffix,
                host_prefix_not: self.when.host_prefix_not,
                host_suffix_not: self.when.host_suffix_not,
                host_matches: self.when.host_matches,

                // Port-related fields
                port: self.when.port,
                port_not: self.when.port_not,

                // Path-related fields
                path: self.when.path,
                path_not: self.when.path_not,
                path_includes: self.when.path_contains,
                path_excludes: self.when.path_excludes,
                path_prefix: self.when.path_prefix,
                path_suffix: self.when.path_suffix,
                path_prefix_not: self.when.path_prefix_not,
                path_suffix_not: self.when.path_suffix_not,
                path_matches: self.when.path_matches,

                // Method-related fields
                method: self.when.method.map(|m| m.to_string()),
                method_not: from_method_vec(self.when.method_not),
                // Query Parameter-related fields
                query_param: from_name_value_string_pair_vec(self.when.query_param),
                query_param_not: from_name_value_string_pair_vec(self.when.query_param_not),
                query_param_exists: self.when.query_param_exists,
                query_param_missing: self.when.query_param_missing,
                query_param_includes: from_name_value_string_pair_vec(self.when.query_param_contains),
                query_param_excludes: from_name_value_string_pair_vec(self.when.query_param_excludes),
                query_param_prefix: from_name_value_string_pair_vec(self.when.query_param_prefix),
                query_param_suffix: from_name_value_string_pair_vec(self.when.query_param_suffix),
                query_param_prefix_not: from_name_value_string_pair_vec(self.when.query_param_prefix_not),
                query_param_suffix_not: from_name_value_string_pair_vec(self.when.query_param_suffix_not),
                query_param_matches: from_name_value_pattern_pair_vec(self.when.query_param_matches),
                query_param_count: from_key_value_pattern_count_triple_vec(self.when.query_param_count),

                // Header-related fields
                header: from_name_value_string_pair_vec(self.when.header),
                header_not: from_name_value_string_pair_vec(self.when.header_not),
                header_exists: self.when.header_exists,
                header_missing: self.when.header_missing,
                header_includes: from_name_value_string_pair_vec(self.when.header_contains),
                header_excludes: from_name_value_string_pair_vec(self.when.header_excludes),
                header_prefix: from_name_value_string_pair_vec(self.when.header_prefix),
                header_suffix: from_name_value_string_pair_vec(self.when.header_suffix),
                header_prefix_not: from_name_value_string_pair_vec(self.when.header_prefix_not),
                header_suffix_not: from_name_value_string_pair_vec(self.when.header_suffix_not),
                header_matches: from_name_value_pattern_pair_vec(self.when.header_matches),
                header_count: from_key_value_pattern_count_triple_vec(self.when.header_count),
                // Cookie-related fields
                cookie: from_name_value_string_pair_vec(self.when.cookie),
                cookie_not: from_name_value_string_pair_vec(self.when.cookie_not),
                cookie_exists: self.when.cookie_exists,
                cookie_missing: self.when.cookie_missing,
                cookie_includes: from_name_value_string_pair_vec(self.when.cookie_contains),
                cookie_excludes: from_name_value_string_pair_vec(self.when.cookie_excludes),
                cookie_prefix: from_name_value_string_pair_vec(self.when.cookie_prefix),
                cookie_suffix: from_name_value_string_pair_vec(self.when.cookie_suffix),
                cookie_prefix_not: from_name_value_string_pair_vec(self.when.cookie_prefix_not),
                cookie_suffix_not: from_name_value_string_pair_vec(self.when.cookie_suffix_not),
                cookie_matches: from_name_value_pattern_pair_vec(self.when.cookie_matches),
                cookie_count: from_key_value_pattern_count_triple_vec(self.when.cookie_count),

                // Body-related fields
                body: from_string_to_bytes_choose(self.when.body, self.when.body_base64),
                body_not: to_bytes_vec(self.when.body_not, self.when.body_not_base64),
                body_includes: to_bytes_vec(self.when.body_contains, self.when.body_contains_base64),
                body_excludes: to_bytes_vec(self.when.body_excludes, self.when.body_excludes_base64),
                body_prefix: to_bytes_vec(self.when.body_prefix, self.when.body_prefix_base64),
                body_suffix: to_bytes_vec(self.when.body_suffix, self.when.body_suffix_base64),
                body_prefix_not: to_bytes_vec(self.when.body_prefix_not, self.when.body_prefix_not_base64),
                body_suffix_not: to_bytes_vec(self.when.body_suffix_not, self.when.body_suffix_not_base64),
                body_matches: self.when.body_matches,

                // JSON Body-related fields
                json_body: self.when.json_body,
                json_body_not: self.when.json_body_not,
                json_body_includes: self.when.json_body_includes,
                json_body_excludes: self.when.json_body_excludes,

                // x-www-form-urlencoded fields
                form_urlencoded_tuple: from_name_value_string_pair_vec(self.when.form_urlencoded_tuple),
                form_urlencoded_tuple_not: from_name_value_string_pair_vec(self.when.form_urlencoded_tuple_not),
                form_urlencoded_tuple_exists: self.when.form_urlencoded_key_exists,
                form_urlencoded_tuple_missing: self.when.form_urlencoded_key_missing,
                form_urlencoded_tuple_includes: from_name_value_string_pair_vec(self.when.form_urlencoded_contains),
                form_urlencoded_tuple_excludes: from_name_value_string_pair_vec(self.when.form_urlencoded_excludes),
                form_urlencoded_tuple_prefix: from_name_value_string_pair_vec(self.when.form_urlencoded_prefix),
                form_urlencoded_tuple_suffix: from_name_value_string_pair_vec(self.when.form_urlencoded_suffix),
                form_urlencoded_tuple_prefix_not: from_name_value_string_pair_vec(self.when.form_urlencoded_prefix_not),
                form_urlencoded_tuple_suffix_not: from_name_value_string_pair_vec(self.when.form_urlencoded_suffix_not),
                form_urlencoded_tuple_matches: from_name_value_pattern_pair_vec(self.when.form_urlencoded_matches),

                form_urlencoded_tuple_count: from_key_value_pattern_count_triple_vec(self.when.form_urlencoded_count),

                // Boolean dynamic checks
                is_true: None,
                is_false: None,
            },
            response: MockServerHttpResponse {
                status: self.then.status,
                headers: from_name_value_string_pair_vec(self.then.header),
                body: from_string_to_bytes_choose(self.then.body, self.then.body_base64),
                delay: self.then.delay,
                respond_with: None,
            },
        })
    }
}

fn to_method_vec(vec: Option<Vec<String>>) -> Option<Vec<Method>> {
    vec.map(|vec| vec.iter().map(|val| Method::from(val.as_str())).collect())
}

fn from_method_vec(value: Option<Vec<Method>>) -> Option<Vec<String>> {
    value.map(|vec| vec.iter().map(|m| m.to_string()).collect())
}

fn from_name_value_string_pair_vec(kvp: Option<Vec<NameValueStringPair>>) -> Option<Vec<(String, String)>> {
    kvp.map(|vec| vec.into_iter().map(|nvp| (nvp.name, nvp.value)).collect())
}

fn from_name_value_pattern_pair_vec(
    kvp: Option<Vec<NameValuePatternPair>>,
) -> Option<Vec<(HttpMockRegex, HttpMockRegex)>> {
    kvp.map(|vec| vec.into_iter().map(|pair| (pair.name, pair.value)).collect())
}

fn from_key_value_pattern_count_triple_vec(
    input: Option<Vec<KeyValuePatternCountTriple>>,
) -> Option<Vec<(HttpMockRegex, HttpMockRegex, usize)>> {
    input.map(|vec| {
        vec.into_iter()
            .map(|triple| (triple.name, triple.value, triple.count))
            .collect()
    })
}

fn to_name_value_string_pair_vec(vec: Option<Vec<(String, String)>>) -> Option<Vec<NameValueStringPair>> {
    vec.map(|vec| {
        vec.into_iter()
            .map(|(name, value)| NameValueStringPair { name, value })
            .collect()
    })
}

fn to_name_value_pattern_pair_vec(
    vec: Option<Vec<(HttpMockRegex, HttpMockRegex)>>,
) -> Option<Vec<NameValuePatternPair>> {
    vec.map(|vec| {
        vec.into_iter()
            .map(|(name, value)| NameValuePatternPair { name, value })
            .collect()
    })
}

fn to_key_value_pattern_count_triple_vec(
    vec: Option<Vec<(HttpMockRegex, HttpMockRegex, usize)>>,
) -> Option<Vec<KeyValuePatternCountTriple>> {
    vec.map(|vec| {
        vec.into_iter()
            .map(|(name, value, count)| KeyValuePatternCountTriple { name, value, count })
            .collect()
    })
}

fn from_bytes_to_string(data: Option<HttpMockBytes>) -> (Option<String>, Option<String>) {
    let mut text_representation = None;
    let mut base64_representation = None;

    if let Some(bytes_container) = data {
        if let Ok(text_str) = std::str::from_utf8(&bytes_container.to_bytes()) {
            text_representation = Some(text_str.to_string());
        } else {
            base64_representation = Some(BASE64.encode(bytes_container.to_bytes()));
        }
    }

    (text_representation, base64_representation)
}

fn bytes_to_string_vec(data: Option<Vec<HttpMockBytes>>) -> (Option<Vec<String>>, Option<Vec<String>>) {
    let mut text_representations = Vec::new();
    let mut base64_representations = Vec::new();

    if let Some(bytes_vec) = data {
        for bytes_container in bytes_vec {
            let bytes = bytes_container.to_bytes();
            if let Ok(text) = std::str::from_utf8(&bytes) {
                text_representations.push(text.to_owned());
            } else {
                base64_representations.push(BASE64.encode(&bytes));
            }
        }
    }

    let text_opt_vec = if !text_representations.is_empty() {
        Some(text_representations)
    } else {
        None
    };

    let base64_opt_vec = if !base64_representations.is_empty() {
        Some(base64_representations)
    } else {
        None
    };

    (text_opt_vec, base64_opt_vec)
}

fn to_bytes_vec(option_string: Option<Vec<String>>, option_base64: Option<Vec<String>>) -> Option<Vec<HttpMockBytes>> {
    let mut result = Vec::new();

    if let Some(strings) = option_string {
        result.extend(strings.into_iter().map(|s| HttpMockBytes::from(Bytes::from(s))));
    }

    if let Some(base64_strings) = option_base64 {
        result.extend(base64_strings.into_iter().filter_map(|s| {
            BASE64
                .decode(&s)
                .ok()
                .map(|decoded_bytes| HttpMockBytes::from(Bytes::from(decoded_bytes)))
        }));
    }

    if result.is_empty() { None } else { Some(result) }
}

fn from_string_to_bytes_choose(option_string: Option<String>, option_base64: Option<String>) -> Option<HttpMockBytes> {
    let request_body = match (option_string, option_base64) {
        (Some(body), None) => Some(body.into_bytes()),
        (None, Some(base64_body)) => BASE64.decode(base64_body).ok(),
        _ => None, // Handle unexpected combinations or both None
    };

    request_body.map(|s| HttpMockBytes::from(Bytes::from(s)))
}

impl TryFrom<&MockDefinition> for StaticMockDefinition {
    type Error = Error;

    fn try_from(value: &MockDefinition) -> Result<Self, Self::Error> {
        let value = value.clone();

        let (response_body, response_body_base64) = from_bytes_to_string(value.response.body);

        let (request_body, request_body_base64) = from_bytes_to_string(value.request.body);
        let (request_body_not, request_body_not_base64) = bytes_to_string_vec(value.request.body_not);
        let (request_body_includes, request_body_includes_base64) = bytes_to_string_vec(value.request.body_includes);
        let (request_body_excludes, request_body_excludes_base64) = bytes_to_string_vec(value.request.body_excludes);
        let (request_body_prefix, request_body_prefix_base64) = bytes_to_string_vec(value.request.body_prefix);
        let (request_body_suffix, request_body_suffix_base64) = bytes_to_string_vec(value.request.body_suffix);
        let (request_body_prefix_not, request_body_prefix_not_base64) =
            bytes_to_string_vec(value.request.body_prefix_not);
        let (request_body_suffix_not, request_body_suffix_not_base64) =
            bytes_to_string_vec(value.request.body_suffix_not);

        let mut method = None;
        if let Some(method_str) = value.request.method {
            method = Some(Method::from_str(&method_str).map_err(|err| StaticMockConversion(err.to_string()))?);
        }

        Ok(StaticMockDefinition {
            when: StaticRequestRequirements {
                // Scheme-related fields
                scheme: value.request.scheme,
                scheme_not: value.request.scheme_not,

                // Method-related fields
                method,
                method_not: to_method_vec(value.request.method_not),
                // Host-related fields
                host: value.request.host,
                host_not: value.request.host_not,
                host_contains: value.request.host_contains,
                host_excludes: value.request.host_excludes,
                host_prefix: value.request.host_prefix,
                host_suffix: value.request.host_suffix,
                host_prefix_not: value.request.host_prefix_not,
                host_suffix_not: value.request.host_suffix_not,
                host_matches: value.request.host_matches,

                // Port-related fields
                port: value.request.port,
                port_not: value.request.port_not,

                // Path-related fields
                path: value.request.path,
                path_not: value.request.path_not,
                path_contains: value.request.path_includes,
                path_excludes: value.request.path_excludes,
                path_prefix: value.request.path_prefix,
                path_suffix: value.request.path_suffix,
                path_prefix_not: value.request.path_prefix_not,
                path_suffix_not: value.request.path_suffix_not,
                path_matches: value.request.path_matches,

                // Header-related fields
                header: to_name_value_string_pair_vec(value.request.header),
                header_not: to_name_value_string_pair_vec(value.request.header_not),
                header_exists: value.request.header_exists,
                header_missing: value.request.header_missing,
                header_contains: to_name_value_string_pair_vec(value.request.header_includes),
                header_excludes: to_name_value_string_pair_vec(value.request.header_excludes),
                header_prefix: to_name_value_string_pair_vec(value.request.header_prefix),
                header_suffix: to_name_value_string_pair_vec(value.request.header_suffix),
                header_prefix_not: to_name_value_string_pair_vec(value.request.header_prefix_not),
                header_suffix_not: to_name_value_string_pair_vec(value.request.header_suffix_not),
                header_matches: to_name_value_pattern_pair_vec(value.request.header_matches),
                header_count: to_key_value_pattern_count_triple_vec(value.request.header_count),

                // Cookie-related fields
                cookie: to_name_value_string_pair_vec(value.request.cookie),
                cookie_not: to_name_value_string_pair_vec(value.request.cookie_not),
                cookie_exists: value.request.cookie_exists,
                cookie_missing: value.request.cookie_missing,
                cookie_contains: to_name_value_string_pair_vec(value.request.cookie_includes),
                cookie_excludes: to_name_value_string_pair_vec(value.request.cookie_excludes),
                cookie_prefix: to_name_value_string_pair_vec(value.request.cookie_prefix),
                cookie_suffix: to_name_value_string_pair_vec(value.request.cookie_suffix),
                cookie_prefix_not: to_name_value_string_pair_vec(value.request.cookie_prefix_not),
                cookie_suffix_not: to_name_value_string_pair_vec(value.request.cookie_suffix_not),
                cookie_matches: to_name_value_pattern_pair_vec(value.request.cookie_matches),

                cookie_count: to_key_value_pattern_count_triple_vec(value.request.cookie_count),

                // Query Parameter-related fields
                query_param: to_name_value_string_pair_vec(value.request.query_param),
                query_param_not: to_name_value_string_pair_vec(value.request.query_param_not),
                query_param_exists: value.request.query_param_exists,
                query_param_missing: value.request.query_param_missing,
                query_param_contains: to_name_value_string_pair_vec(value.request.query_param_includes),
                query_param_excludes: to_name_value_string_pair_vec(value.request.query_param_excludes),
                query_param_prefix: to_name_value_string_pair_vec(value.request.query_param_prefix),
                query_param_suffix: to_name_value_string_pair_vec(value.request.query_param_suffix),
                query_param_prefix_not: to_name_value_string_pair_vec(value.request.query_param_prefix_not),
                query_param_suffix_not: to_name_value_string_pair_vec(value.request.query_param_suffix_not),
                query_param_matches: to_name_value_pattern_pair_vec(value.request.query_param_matches),
                query_param_count: to_key_value_pattern_count_triple_vec(value.request.query_param_count),

                // Body-related fields
                body: request_body,
                body_base64: request_body_base64,
                body_not: request_body_not,
                body_not_base64: request_body_not_base64,
                body_contains: request_body_includes,
                body_contains_base64: request_body_includes_base64,
                body_excludes: request_body_excludes,
                body_excludes_base64: request_body_excludes_base64,
                body_prefix: request_body_prefix,
                body_prefix_base64: request_body_prefix_base64,
                body_suffix: request_body_suffix,
                body_suffix_base64: request_body_suffix_base64,
                body_prefix_not: request_body_prefix_not,
                body_prefix_not_base64: request_body_prefix_not_base64,
                body_suffix_not: request_body_suffix_not,
                body_suffix_not_base64: request_body_suffix_not_base64,
                body_matches: value.request.body_matches,

                // JSON Body-related fields
                json_body: value.request.json_body,
                json_body_not: value.request.json_body_not,
                json_body_includes: value.request.json_body_includes,
                json_body_excludes: value.request.json_body_excludes,

                // Form URL-encoded fields
                form_urlencoded_tuple: to_name_value_string_pair_vec(value.request.form_urlencoded_tuple),
                form_urlencoded_tuple_not: to_name_value_string_pair_vec(value.request.form_urlencoded_tuple_not),
                form_urlencoded_key_exists: value.request.form_urlencoded_tuple_exists,
                form_urlencoded_key_missing: value.request.form_urlencoded_tuple_missing,
                form_urlencoded_contains: to_name_value_string_pair_vec(value.request.form_urlencoded_tuple_includes),
                form_urlencoded_excludes: to_name_value_string_pair_vec(value.request.form_urlencoded_tuple_excludes),
                form_urlencoded_prefix: to_name_value_string_pair_vec(value.request.form_urlencoded_tuple_prefix),
                form_urlencoded_suffix: to_name_value_string_pair_vec(value.request.form_urlencoded_tuple_suffix),
                form_urlencoded_prefix_not: to_name_value_string_pair_vec(
                    value.request.form_urlencoded_tuple_prefix_not,
                ),
                form_urlencoded_suffix_not: to_name_value_string_pair_vec(
                    value.request.form_urlencoded_tuple_suffix_not,
                ),
                form_urlencoded_matches: to_name_value_pattern_pair_vec(value.request.form_urlencoded_tuple_matches),

                form_urlencoded_count: to_key_value_pattern_count_triple_vec(value.request.form_urlencoded_tuple_count),
            },
            then: StaticHTTPResponse {
                status: value.response.status,
                header: to_name_value_string_pair_vec(value.response.headers),
                body: response_body,
                body_base64: response_body_base64,
                // Reason for the cast to u64: The Duration::as_millis method returns the total
                // number of milliseconds contained within the Duration as a u128. This is
                // because Duration::as_millis needs to handle larger values that
                // can result from multiplying the seconds (stored internally as a u64)
                // by 1000 and adding the milliseconds (also a u64), potentially
                // exceeding the u64 limit.
                delay: value.response.delay,
            },
        })
    }
}
