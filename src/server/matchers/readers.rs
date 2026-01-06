pub mod expectations {
    use crate::{
        common::{
            data::{HttpMockRegex, RequestRequirements},
            util::HttpMockBytes,
        },
        prelude::HttpMockRequest,
    };
    use serde_json::Value;
    use std::sync::Arc;

    #[inline]
    pub fn scheme_equal_to(mock: &RequestRequirements) -> Option<Vec<&String>> {
        mock.scheme.as_ref().map(|b| vec![b])
    }

    #[inline]
    pub fn scheme_not_equal_to(mock: &RequestRequirements) -> Option<Vec<&String>> {
        mock.scheme_not.as_ref().map(|b| vec![b])
    }

    #[inline]
    pub fn method_equal_to(mock: &RequestRequirements) -> Option<Vec<&String>> {
        mock.method.as_ref().map(|b| vec![b])
    }

    #[inline]
    pub fn method_not_equal_to(mock: &RequestRequirements) -> Option<Vec<&String>> {
        mock.method_not
            .as_ref()
            .map(|b| b.into_iter().map(|v| v).collect())
    }

    #[inline]
    pub fn host_equal_to(mock: &RequestRequirements) -> Option<Vec<&String>> {
        mock.host.as_ref().map(|b| vec![b])
    }

    #[inline]
    pub fn host_not_equal_to(mock: &RequestRequirements) -> Option<Vec<&String>> {
        mock.host_not.as_ref().map(|v| v.iter().collect())
    }

    #[inline]
    pub fn host_includes(mock: &RequestRequirements) -> Option<Vec<&String>> {
        mock.host_contains
            .as_ref()
            .map(|b| b.into_iter().map(|v| v).collect())
    }

    #[inline]
    pub fn host_excludes(mock: &RequestRequirements) -> Option<Vec<&String>> {
        mock.host_excludes
            .as_ref()
            .map(|b| b.into_iter().map(|v| v).collect())
    }

    #[inline]
    pub fn host_prefix(mock: &RequestRequirements) -> Option<Vec<&String>> {
        mock.host_prefix
            .as_ref()
            .map(|b| b.into_iter().map(|v| v).collect())
    }

    #[inline]
    pub fn host_prefix_not(mock: &RequestRequirements) -> Option<Vec<&String>> {
        mock.host_prefix_not
            .as_ref()
            .map(|b| b.into_iter().map(|v| v).collect())
    }

    #[inline]
    pub fn host_has_suffix(mock: &RequestRequirements) -> Option<Vec<&String>> {
        mock.host_suffix
            .as_ref()
            .map(|b| b.into_iter().map(|v| v).collect())
    }

    #[inline]
    pub fn host_has_no_suffix(mock: &RequestRequirements) -> Option<Vec<&String>> {
        mock.host_suffix_not
            .as_ref()
            .map(|b| b.into_iter().map(|v| v).collect())
    }

    #[inline]
    pub fn host_matches_regex(mock: &RequestRequirements) -> Option<Vec<&HttpMockRegex>> {
        mock.host_matches
            .as_ref()
            .map(|b| b.into_iter().map(|v| v).collect())
    }

    #[inline]
    pub fn port_equal_to(mock: &RequestRequirements) -> Option<Vec<&u16>> {
        mock.port.as_ref().map(|b| vec![b])
    }

    #[inline]
    pub fn port_not_equal_to(mock: &RequestRequirements) -> Option<Vec<&u16>> {
        mock.port_not.as_ref().map(|v| v.iter().collect())
    }

    #[inline]
    pub fn path_equal_to(mock: &RequestRequirements) -> Option<Vec<&String>> {
        mock.path.as_ref().map(|b| vec![b])
    }

    #[inline]
    pub fn path_not_equal_to(mock: &RequestRequirements) -> Option<Vec<&String>> {
        mock.path_not.as_ref().map(|v| v.iter().collect())
    }

    #[inline]
    pub fn path_includes(mock: &RequestRequirements) -> Option<Vec<&String>> {
        mock.path_includes.as_ref().map(|v| v.iter().collect())
    }

    #[inline]
    pub fn path_excludes(mock: &RequestRequirements) -> Option<Vec<&String>> {
        mock.path_excludes.as_ref().map(|v| v.iter().collect())
    }

    #[inline]
    pub fn path_prefix(mock: &RequestRequirements) -> Option<Vec<&String>> {
        mock.path_prefix.as_ref().map(|v| v.iter().collect())
    }

    #[inline]
    pub fn path_prefix_not(mock: &RequestRequirements) -> Option<Vec<&String>> {
        mock.path_prefix_not.as_ref().map(|v| v.iter().collect())
    }

    #[inline]
    pub fn path_suffix(mock: &RequestRequirements) -> Option<Vec<&String>> {
        mock.path_suffix.as_ref().map(|v| v.iter().collect())
    }

    #[inline]
    pub fn path_suffix_not(mock: &RequestRequirements) -> Option<Vec<&String>> {
        mock.path_suffix_not.as_ref().map(|v| v.iter().collect())
    }

    #[inline]
    pub fn path_matches(mock: &RequestRequirements) -> Option<Vec<&HttpMockRegex>> {
        mock.path_matches
            .as_ref()
            .map(|b| b.into_iter().map(|v| v).collect())
    }

    #[inline]
    pub fn query_param(mock: &RequestRequirements) -> Option<Vec<(&String, Option<&String>)>> {
        mock.query_param
            .as_ref()
            .map(|v| v.into_iter().map(|(k, v)| (k, Some(v))).collect())
    }

    #[inline]
    pub fn query_param_not(mock: &RequestRequirements) -> Option<Vec<(&String, Option<&String>)>> {
        mock.query_param_not
            .as_ref()
            .map(|v| v.into_iter().map(|(k, v)| (k, Some(v))).collect())
    }

    #[inline]
    pub fn query_param_exists(
        mock: &RequestRequirements,
    ) -> Option<Vec<(&String, Option<&String>)>> {
        mock.query_param_exists
            .as_ref()
            .map(|v| v.into_iter().map(|v| (v, None)).collect())
    }

    #[inline]
    pub fn query_param_missing(
        mock: &RequestRequirements,
    ) -> Option<Vec<(&String, Option<&String>)>> {
        mock.query_param_missing
            .as_ref()
            .map(|v| v.into_iter().map(|v| (v, None)).collect())
    }

    #[inline]
    pub fn query_param_includes(
        mock: &RequestRequirements,
    ) -> Option<Vec<(&String, Option<&String>)>> {
        mock.query_param_includes
            .as_ref()
            .map(|v| v.into_iter().map(|(k, v)| (k, Some(v))).collect())
    }

    #[inline]
    pub fn query_param_excludes(
        mock: &RequestRequirements,
    ) -> Option<Vec<(&String, Option<&String>)>> {
        mock.query_param_excludes
            .as_ref()
            .map(|v| v.into_iter().map(|(k, v)| (k, Some(v))).collect())
    }

    #[inline]
    pub fn query_param_prefix(
        mock: &RequestRequirements,
    ) -> Option<Vec<(&String, Option<&String>)>> {
        mock.query_param_prefix
            .as_ref()
            .map(|v| v.into_iter().map(|(k, v)| (k, Some(v))).collect())
    }

    #[inline]
    pub fn query_param_prefix_not(
        mock: &RequestRequirements,
    ) -> Option<Vec<(&String, Option<&String>)>> {
        mock.query_param_prefix_not
            .as_ref()
            .map(|v| v.into_iter().map(|(k, v)| (k, Some(v))).collect())
    }

    #[inline]
    pub fn query_param_suffix(
        mock: &RequestRequirements,
    ) -> Option<Vec<(&String, Option<&String>)>> {
        mock.query_param_suffix
            .as_ref()
            .map(|v| v.into_iter().map(|(k, v)| (k, Some(v))).collect())
    }

    #[inline]
    pub fn query_param_suffix_not(
        mock: &RequestRequirements,
    ) -> Option<Vec<(&String, Option<&String>)>> {
        mock.query_param_suffix_not
            .as_ref()
            .map(|v| v.into_iter().map(|(k, v)| (k, Some(v))).collect())
    }

    #[inline]
    pub fn query_param_matches(
        mock: &RequestRequirements,
    ) -> Option<Vec<(&HttpMockRegex, Option<&HttpMockRegex>)>> {
        mock.query_param_matches
            .as_ref()
            .map(|v| v.into_iter().map(|(k, v)| (k, Some(v))).collect())
    }

    #[inline]
    pub fn query_param_count(
        mock: &RequestRequirements,
    ) -> Option<Vec<(Option<&HttpMockRegex>, Option<&HttpMockRegex>, usize)>> {
        mock.query_param_count
            .as_ref()
            .map(|v| v.iter().map(|(k, v, c)| (Some(k), Some(v), *c)).collect())
    }

    #[inline]
    pub fn header(mock: &RequestRequirements) -> Option<Vec<(&String, Option<&String>)>> {
        mock.header
            .as_ref()
            .map(|v| v.into_iter().map(|(k, v)| (k, Some(v))).collect())
    }

    #[inline]
    pub fn header_not(mock: &RequestRequirements) -> Option<Vec<(&String, Option<&String>)>> {
        mock.header_not
            .as_ref()
            .map(|v| v.into_iter().map(|(k, v)| (k, Some(v))).collect())
    }

    #[inline]
    pub fn header_exists(mock: &RequestRequirements) -> Option<Vec<(&String, Option<&String>)>> {
        mock.header_exists
            .as_ref()
            .map(|v| v.into_iter().map(|v| (v, None)).collect())
    }

    #[inline]
    pub fn header_missing(mock: &RequestRequirements) -> Option<Vec<(&String, Option<&String>)>> {
        mock.header_missing
            .as_ref()
            .map(|v| v.into_iter().map(|v| (v, None)).collect())
    }

    #[inline]
    pub fn header_includes(mock: &RequestRequirements) -> Option<Vec<(&String, Option<&String>)>> {
        mock.header_includes
            .as_ref()
            .map(|v| v.into_iter().map(|(k, v)| (k, Some(v))).collect())
    }

    #[inline]
    pub fn header_excludes(mock: &RequestRequirements) -> Option<Vec<(&String, Option<&String>)>> {
        mock.header_excludes
            .as_ref()
            .map(|v| v.into_iter().map(|(k, v)| (k, Some(v))).collect())
    }

    #[inline]
    pub fn header_prefix(mock: &RequestRequirements) -> Option<Vec<(&String, Option<&String>)>> {
        mock.header_prefix
            .as_ref()
            .map(|v| v.into_iter().map(|(k, v)| (k, Some(v))).collect())
    }

    #[inline]
    pub fn header_prefix_not(
        mock: &RequestRequirements,
    ) -> Option<Vec<(&String, Option<&String>)>> {
        mock.header_prefix_not
            .as_ref()
            .map(|v| v.into_iter().map(|(k, v)| (k, Some(v))).collect())
    }

    #[inline]
    pub fn header_suffix(mock: &RequestRequirements) -> Option<Vec<(&String, Option<&String>)>> {
        mock.header_suffix
            .as_ref()
            .map(|v| v.into_iter().map(|(k, v)| (k, Some(v))).collect())
    }

    #[inline]
    pub fn header_suffix_not(
        mock: &RequestRequirements,
    ) -> Option<Vec<(&String, Option<&String>)>> {
        mock.header_suffix_not
            .as_ref()
            .map(|v| v.into_iter().map(|(k, v)| (k, Some(v))).collect())
    }

    #[inline]
    pub fn header_matches(
        mock: &RequestRequirements,
    ) -> Option<Vec<(&HttpMockRegex, Option<&HttpMockRegex>)>> {
        mock.header_matches
            .as_ref()
            .map(|v| v.into_iter().map(|(k, v)| (k, Some(v))).collect())
    }

    #[inline]
    pub fn header_count(
        mock: &RequestRequirements,
    ) -> Option<Vec<(Option<&HttpMockRegex>, Option<&HttpMockRegex>, usize)>> {
        mock.header_count
            .as_ref()
            .map(|v| v.iter().map(|(k, v, c)| (Some(k), Some(v), *c)).collect())
    }

    #[inline]
    pub fn cookie(mock: &RequestRequirements) -> Option<Vec<(&String, Option<&String>)>> {
        mock.cookie
            .as_ref()
            .map(|v| v.into_iter().map(|(k, v)| (k, Some(v))).collect())
    }

    #[inline]
    pub fn cookie_not(mock: &RequestRequirements) -> Option<Vec<(&String, Option<&String>)>> {
        mock.cookie_not
            .as_ref()
            .map(|v| v.into_iter().map(|(k, v)| (k, Some(v))).collect())
    }

    #[inline]
    pub fn cookie_exists(mock: &RequestRequirements) -> Option<Vec<(&String, Option<&String>)>> {
        mock.cookie_exists
            .as_ref()
            .map(|v| v.into_iter().map(|v| (v, None)).collect())
    }

    #[inline]
    pub fn cookie_missing(mock: &RequestRequirements) -> Option<Vec<(&String, Option<&String>)>> {
        mock.cookie_missing
            .as_ref()
            .map(|v| v.into_iter().map(|v| (v, None)).collect())
    }

    #[inline]
    pub fn cookie_includes(mock: &RequestRequirements) -> Option<Vec<(&String, Option<&String>)>> {
        mock.cookie_includes
            .as_ref()
            .map(|v| v.into_iter().map(|(k, v)| (k, Some(v))).collect())
    }

    #[inline]
    pub fn cookie_excludes(mock: &RequestRequirements) -> Option<Vec<(&String, Option<&String>)>> {
        mock.cookie_excludes
            .as_ref()
            .map(|v| v.into_iter().map(|(k, v)| (k, Some(v))).collect())
    }

    #[inline]
    pub fn cookie_prefix(mock: &RequestRequirements) -> Option<Vec<(&String, Option<&String>)>> {
        mock.cookie_prefix
            .as_ref()
            .map(|v| v.into_iter().map(|(k, v)| (k, Some(v))).collect())
    }

    #[inline]
    pub fn cookie_prefix_not(
        mock: &RequestRequirements,
    ) -> Option<Vec<(&String, Option<&String>)>> {
        mock.cookie_prefix_not
            .as_ref()
            .map(|v| v.into_iter().map(|(k, v)| (k, Some(v))).collect())
    }

    #[inline]
    pub fn cookie_suffix(mock: &RequestRequirements) -> Option<Vec<(&String, Option<&String>)>> {
        mock.cookie_suffix
            .as_ref()
            .map(|v| v.into_iter().map(|(k, v)| (k, Some(v))).collect())
    }

    #[inline]
    pub fn cookie_suffix_not(
        mock: &RequestRequirements,
    ) -> Option<Vec<(&String, Option<&String>)>> {
        mock.cookie_suffix_not
            .as_ref()
            .map(|v| v.into_iter().map(|(k, v)| (k, Some(v))).collect())
    }

    #[inline]
    pub fn cookie_matches(
        mock: &RequestRequirements,
    ) -> Option<Vec<(&HttpMockRegex, Option<&HttpMockRegex>)>> {
        mock.cookie_matches
            .as_ref()
            .map(|v| v.into_iter().map(|(k, v)| (k, Some(v))).collect())
    }

    #[inline]
    pub fn cookie_count(
        mock: &RequestRequirements,
    ) -> Option<Vec<(Option<&HttpMockRegex>, Option<&HttpMockRegex>, usize)>> {
        mock.cookie_count
            .as_ref()
            .map(|v| v.iter().map(|(k, v, c)| (Some(k), Some(v), *c)).collect())
    }

    #[inline]
    pub fn body(mock: &RequestRequirements) -> Option<Vec<&HttpMockBytes>> {
        mock.body.as_ref().map(|b| vec![b])
    }

    #[inline]
    pub fn body_not(mock: &RequestRequirements) -> Option<Vec<&HttpMockBytes>> {
        mock.body_not.as_ref().map(|v| v.iter().collect())
    }

    #[inline]
    pub fn body_includes(mock: &RequestRequirements) -> Option<Vec<&HttpMockBytes>> {
        mock.body_includes.as_ref().map(|v| v.iter().collect())
    }

    #[inline]
    pub fn body_excludes(mock: &RequestRequirements) -> Option<Vec<&HttpMockBytes>> {
        mock.body_excludes.as_ref().map(|v| v.iter().collect())
    }

    #[inline]
    pub fn body_prefix(mock: &RequestRequirements) -> Option<Vec<&HttpMockBytes>> {
        mock.body_prefix.as_ref().map(|v| v.iter().collect())
    }

    #[inline]
    pub fn body_prefix_not(mock: &RequestRequirements) -> Option<Vec<&HttpMockBytes>> {
        mock.body_prefix_not.as_ref().map(|v| v.iter().collect())
    }

    #[inline]
    pub fn body_suffix(mock: &RequestRequirements) -> Option<Vec<&HttpMockBytes>> {
        mock.body_suffix.as_ref().map(|v| v.iter().collect())
    }

    #[inline]
    pub fn body_suffix_not(mock: &RequestRequirements) -> Option<Vec<&HttpMockBytes>> {
        mock.body_suffix_not.as_ref().map(|v| v.iter().collect())
    }

    #[inline]
    pub fn body_matches(mock: &RequestRequirements) -> Option<Vec<&HttpMockRegex>> {
        mock.body_matches
            .as_ref()
            .map(|b| b.into_iter().map(|v| v).collect())
    }

    #[inline]
    pub fn json_body(mock: &RequestRequirements) -> Option<Vec<&Value>> {
        mock.json_body.as_ref().map(|b| vec![b])
    }

    #[inline]
    pub fn json_body_includes(mock: &RequestRequirements) -> Option<Vec<&serde_json::Value>> {
        mock.json_body_includes
            .as_ref()
            .map(|b| b.into_iter().collect())
    }

    #[inline]
    pub fn json_body_excludes(mock: &RequestRequirements) -> Option<Vec<&serde_json::Value>> {
        mock.json_body_excludes
            .as_ref()
            .map(|b| b.into_iter().collect())
    }

    #[inline]
    pub fn is_true(
        mock: &RequestRequirements,
    ) -> Option<Vec<&Arc<dyn Fn(&HttpMockRequest) -> bool + 'static + Sync + Send>>> {
        mock.is_true.as_ref().map(|b| b.iter().map(|f| f).collect())
    }

    #[inline]
    pub fn is_false(
        mock: &RequestRequirements,
    ) -> Option<Vec<&Arc<dyn Fn(&HttpMockRequest) -> bool + 'static + Sync + Send>>> {
        mock.is_false.as_ref().map(|b| b.iter().map(|f| f).collect())
    }

    pub fn form_urlencoded_tuple(
        mock: &RequestRequirements,
    ) -> Option<Vec<(&String, Option<&String>)>> {
        mock.form_urlencoded_tuple
            .as_ref()
            .map(|v| v.into_iter().map(|(k, v)| (k, Some(v))).collect())
    }

    pub fn form_urlencoded_tuple_not(
        mock: &RequestRequirements,
    ) -> Option<Vec<(&String, Option<&String>)>> {
        mock.form_urlencoded_tuple_not
            .as_ref()
            .map(|v| v.into_iter().map(|(k, v)| (k, Some(v))).collect())
    }

    pub fn form_urlencoded_key_exists(
        mock: &RequestRequirements,
    ) -> Option<Vec<(&String, Option<&String>)>> {
        mock.form_urlencoded_tuple_exists
            .as_ref()
            .map(|v| v.into_iter().map(|v| (v, None)).collect())
    }

    pub fn form_urlencoded_key_missing(
        mock: &RequestRequirements,
    ) -> Option<Vec<(&String, Option<&String>)>> {
        mock.form_urlencoded_tuple_missing
            .as_ref()
            .map(|v| v.into_iter().map(|v| (v, None)).collect())
    }

    #[inline]
    pub fn form_urlencoded_includes(
        mock: &RequestRequirements,
    ) -> Option<Vec<(&String, Option<&String>)>> {
        mock.form_urlencoded_tuple_includes
            .as_ref()
            .map(|v| v.into_iter().map(|(k, v)| (k, Some(v))).collect())
    }

    #[inline]
    pub fn form_urlencoded_excludes(
        mock: &RequestRequirements,
    ) -> Option<Vec<(&String, Option<&String>)>> {
        mock.form_urlencoded_tuple_excludes
            .as_ref()
            .map(|v| v.into_iter().map(|(k, v)| (k, Some(v))).collect())
    }

    #[inline]
    pub fn form_urlencoded_prefix(
        mock: &RequestRequirements,
    ) -> Option<Vec<(&String, Option<&String>)>> {
        mock.form_urlencoded_tuple_prefix
            .as_ref()
            .map(|v| v.into_iter().map(|(k, v)| (k, Some(v))).collect())
    }

    #[inline]
    pub fn form_urlencoded_prefix_not(
        mock: &RequestRequirements,
    ) -> Option<Vec<(&String, Option<&String>)>> {
        mock.form_urlencoded_tuple_prefix_not
            .as_ref()
            .map(|v| v.into_iter().map(|(k, v)| (k, Some(v))).collect())
    }

    #[inline]
    pub fn form_urlencoded_suffix(
        mock: &RequestRequirements,
    ) -> Option<Vec<(&String, Option<&String>)>> {
        mock.form_urlencoded_tuple_suffix
            .as_ref()
            .map(|v| v.into_iter().map(|(k, v)| (k, Some(v))).collect())
    }

    #[inline]
    pub fn form_urlencoded_suffix_not(
        mock: &RequestRequirements,
    ) -> Option<Vec<(&String, Option<&String>)>> {
        mock.form_urlencoded_tuple_suffix_not
            .as_ref()
            .map(|v| v.into_iter().map(|(k, v)| (k, Some(v))).collect())
    }

    #[inline]
    pub fn form_urlencoded_matches(
        mock: &RequestRequirements,
    ) -> Option<Vec<(&HttpMockRegex, Option<&HttpMockRegex>)>> {
        mock.form_urlencoded_tuple_matches
            .as_ref()
            .map(|v| v.into_iter().map(|(k, v)| (k, Some(v))).collect())
    }

    #[inline]
    pub fn form_urlencoded_key_value_count(
        mock: &RequestRequirements,
    ) -> Option<Vec<(Option<&HttpMockRegex>, Option<&HttpMockRegex>, usize)>> {
        mock.form_urlencoded_tuple_count
            .as_ref()
            .map(|v| v.iter().map(|(k, v, c)| (Some(k), Some(v), *c)).collect())
    }
}

pub mod request_value {
    use crate::{common::util::HttpMockBytes, prelude::HttpMockRequest};
    use serde_json::Value;

    #[inline]
    pub fn scheme(req: &HttpMockRequest) -> Option<String> {
        Some(req.scheme())
    }

    #[inline]
    pub fn method(req: &HttpMockRequest) -> Option<String> {
        Some(req.method().to_string())
    }

    #[inline]
    pub fn host(req: &HttpMockRequest) -> Option<String> {
        req.host().map(|h| h.to_string())
    }

    #[inline]
    pub fn port(req: &HttpMockRequest) -> Option<u16> {
        Some(req.port())
    }

    #[inline]
    pub fn path(req: &HttpMockRequest) -> Option<String> {
        Some(req.uri().path().to_string())
    }

    #[inline]
    pub fn query_params(req: &HttpMockRequest) -> Option<Vec<(String, Option<String>)>> {
        Some(
            req.query_params()
                .iter()
                .map(|(k, v)| (k.into(), Some(v.into())))
                .collect(),
        )
    }

    #[inline]
    pub fn headers(req: &HttpMockRequest) -> Option<Vec<(String, Option<String>)>> {
        Some(
            req.headers_vec()
                .iter()
                .map(|(k, v)| (k.into(), Some(v.into())))
                .collect(),
        )
    }

    #[cfg(feature = "cookies")]
    #[inline]
    pub fn cookies(req: &HttpMockRequest) -> Option<Vec<(String, Option<String>)>> {
        Some(
            req.cookies()
                .expect("cannot parse cookies")
                .iter()
                .map(|(k, v)| (k.into(), Some(v.into())))
                .collect(),
        )
    }

    #[inline]
    pub fn body(req: &HttpMockRequest) -> Option<HttpMockBytes> {
        Some(req.body().clone())
    }

    #[inline]
    pub fn json_body(req: &HttpMockRequest) -> Option<serde_json::Value> {
        let body = req.body_ref();
        if body.len() == 0 {
            ()
        }

        match serde_json::from_slice(body) {
            Err(e) => {
                tracing::trace!("Cannot parse json value: {}", e);
                None
            }
            Ok(v) => Some(v),
        }
    }

    pub fn form_urlencoded_body(req: &HttpMockRequest) -> Option<Vec<(String, Option<String>)>> {
        Some(
            form_urlencoded::parse(req.body_ref())
                .into_owned()
                .map(|(k, v)| (k, Some(v)))
                .collect(),
        )
    }

    #[inline]
    pub fn full_request(req: &HttpMockRequest) -> Option<&HttpMockRequest> {
        Some(req)
    }
}
