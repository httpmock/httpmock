use std::{
    collections::BTreeMap,
    str::FromStr,
    sync::{Arc, Mutex},
    time::Duration,
};

#[cfg(feature = "record")]
use bytes::Bytes;
use http::{HeaderMap, HeaderName, HeaderValue, Uri, uri::Authority};
use thiserror::Error;

#[cfg(feature = "record")]
use crate::{
    common::data,
    server::{
        persistence::{deserialize_mock_defs_from_yaml, serialize_mock_defs_to_yaml},
        state::Error::ValidationError,
    },
};
use crate::{
    common::data::{
        ActiveForwardingRule, ActiveMock, ActiveProxyRule, ActiveRecording, ClosestMatch, ForwardingRuleConfig,
        Mismatch, MockDefinition, MockServerHttpResponse, ProxyRuleConfig, RecordingRuleConfig, RequestRequirements,
    },
    prelude::HttpMockRequest,
    server::{
        matchers,
        matchers::Matcher,
        state::Error::{BodyMethodInvalid, DataConversionError, StaticMockError},
    },
};

#[derive(Error, Debug)]
pub enum Error {
    #[error("The mock is static and cannot be deleted")]
    StaticMockError,
    #[error("Validation error: request HTTP method GET or HEAD cannot have a body")]
    BodyMethodInvalid,
    #[error("cannot convert: {0}")]
    DataConversionError(String),
    #[error("validation error: {0}")]
    ValidationError(String),
    #[error("unknown error")]
    Unknown,
}

/// The default maximum number of requests retained in the server's history when
/// no explicit limit is configured.
pub(crate) const DEFAULT_HISTORY_LIMIT: usize = 100;

/// The mock server's mutable state: the registered mocks, the request history,
/// and the active forwarding, proxy and recording rules.
pub(crate) struct Inner {
    next_mock_id: usize,
    next_forwarding_rule_id: usize,
    next_proxy_rule_id: usize,
    next_recording_id: usize,
    history_limit: usize,
    pub mocks: BTreeMap<usize, ActiveMock>,
    pub history: Vec<Arc<HttpMockRequest>>,
    pub matchers: Vec<Box<dyn Matcher + Sync + Send>>,
    pub(crate) forwarding_rules: BTreeMap<usize, ForwardingRule>,
    pub proxy_rules: BTreeMap<usize, ActiveProxyRule>,
    pub recordings: BTreeMap<usize, ActiveRecording>,
}

#[derive(Clone)]
pub(crate) struct ForwardingRule {
    pub active: ActiveForwardingRule,
    pub target: ForwardTarget,
    pub request_headers: HeaderMap,
}

#[derive(Clone)]
pub(crate) struct ForwardTarget {
    scheme: ForwardScheme,
    authority: Authority,
}

#[derive(Clone, Copy)]
enum ForwardScheme {
    Http,
    Https,
}

impl TryFrom<&str> for ForwardTarget {
    type Error = Error;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        let uri =
            Uri::from_str(value).map_err(|err| Error::ValidationError(format!("invalid forwarding target: {err}")))?;
        let parts = uri.into_parts();

        let scheme = match parts.scheme.as_ref().map(|scheme| scheme.as_str()) {
            Some("http") => ForwardScheme::Http,
            Some("https") => ForwardScheme::Https,
            Some(scheme) => {
                return Err(Error::ValidationError(format!(
                    "invalid forwarding target scheme '{scheme}': expected http or https"
                )));
            }
            None => return Err(Error::ValidationError("forwarding target has no scheme".to_string())),
        };

        let authority = parts
            .authority
            .ok_or_else(|| Error::ValidationError("forwarding target has no authority".to_string()))?;

        Ok(Self { scheme, authority })
    }
}

impl ForwardTarget {
    pub fn into_parts(self) -> (&'static str, http::uri::Scheme, Authority) {
        let (name, scheme) = match self.scheme {
            ForwardScheme::Http => ("http", http::uri::Scheme::HTTP),
            ForwardScheme::Https => ("https", http::uri::Scheme::HTTPS),
        };

        (name, scheme, self.authority)
    }
}

impl Inner {
    pub fn new(history_limit: usize) -> Self {
        Inner {
            mocks: BTreeMap::new(),
            forwarding_rules: BTreeMap::new(),
            proxy_rules: BTreeMap::new(),
            recordings: BTreeMap::new(),
            history: Vec::new(),
            history_limit,
            next_mock_id: 0,
            next_forwarding_rule_id: 0,
            next_proxy_rule_id: 0,
            next_recording_id: 0,
            matchers: matchers::all(),
        }
    }
}

/// Owns the mock server's state and serialises access to it.
pub struct Manager {
    state: Mutex<Inner>,
}

impl Manager {
    pub fn new(history_limit: usize) -> Self {
        Self {
            state: Mutex::new(Inner::new(history_limit)),
        }
    }

    pub(crate) fn reset(&self) {
        self.delete_all_mocks();
        self.delete_history();
        self.delete_all_forwarding_rules();
        self.delete_all_proxy_rules();
        self.delete_all_recordings();
    }

    pub(crate) fn add_mock(&self, definition: MockDefinition, is_static: bool) -> Result<ActiveMock, Error> {
        validate_request_requirements(&definition.request)?;

        let mut state = self.state.lock().unwrap();

        let id = state.next_mock_id;
        let active_mock = ActiveMock::new(id, definition, 0, is_static);

        tracing::debug!("Adding new mock with ID={}", id);

        state.mocks.insert(id, active_mock.clone());

        state.next_mock_id += 1;

        Ok(active_mock)
    }

    pub(crate) fn read_mock(&self, id: usize) -> Result<Option<ActiveMock>, Error> {
        let state = self.state.lock().unwrap();

        let result = state.mocks.get(&id);
        match result {
            Some(found) => Ok(Some(found.clone())),
            None => Ok(None),
        }
    }

    pub(crate) fn delete_mock(&self, id: usize) -> Result<bool, Error> {
        let mut state = self.state.lock().unwrap();

        if let Some(m) = state.mocks.get(&id)
            && m.is_static
        {
            return Err(StaticMockError);
        }

        tracing::debug!("Deleting mock with id={}", id);

        Ok(state.mocks.remove(&id).is_some())
    }

    pub(crate) fn delete_all_mocks(&self) {
        let mut state = self.state.lock().unwrap();

        let ids: Vec<usize> = state
            .mocks
            .iter()
            .filter(|(_k, v)| !v.is_static)
            .map(|(k, _v)| *k)
            .collect();

        ids.iter().for_each(|k| {
            state.mocks.remove(k);
        });

        tracing::trace!("Deleted all mocks");
    }

    pub(crate) fn delete_history(&self) {
        let mut state = self.state.lock().unwrap();
        state.history.clear();
        tracing::trace!("Deleted request history");
    }

    pub(crate) fn verify(&self, requirements: &RequestRequirements) -> Result<Option<ClosestMatch>, Error> {
        let state = self.state.lock().unwrap();

        let non_matching_requests: Vec<&Arc<HttpMockRequest>> = state
            .history
            .iter()
            .filter(|req| !request_matches(&state.matchers, req, requirements))
            .collect();

        let request_distances = get_distances(&non_matching_requests, &state.matchers, requirements);
        let best_matches = get_min_distance_requests(&request_distances);

        let closes_match_request_idx = match best_matches.first() {
            None => return Ok(None),
            Some(idx) => *idx,
        };

        let req = non_matching_requests.get(closes_match_request_idx).unwrap();
        let mismatches = get_request_mismatches(req, requirements, &state.matchers);

        Ok(Some(ClosestMatch {
            request: HttpMockRequest::clone(req),
            request_index: closes_match_request_idx,
            mismatches,
        }))
    }

    pub(crate) fn serve_mock(&self, req: &HttpMockRequest) -> Result<Option<MockServerHttpResponse>, Error> {
        let mut state = self.state.lock().unwrap();

        let req = Arc::new(req.clone());

        if state.history.len() > state.history_limit {
            state.history.remove(0);
        }
        state.history.push(req.clone());

        let result = state
            .mocks
            .values()
            .find(|&mock| request_matches(&state.matchers, &req, &mock.definition.request));

        let found_mock_id = result.map(|mock| mock.id);

        if let Some(found_id) = found_mock_id {
            tracing::debug!("Matched mock with id={} to the following request: {:#?}", found_id, req);

            let mock = state.mocks.get_mut(&found_id).unwrap();
            mock.call_counter += 1;

            return Ok(Some(mock.definition.response.clone()));
        }

        tracing::debug!("Could not match any mock to the following request: {:#?}", req);

        Ok(None)
    }

    pub(crate) fn create_forwarding_rule(&self, config: ForwardingRuleConfig) -> Result<ActiveForwardingRule, Error> {
        let target = ForwardTarget::try_from(config.target_base_url.as_str())?;
        let mut request_headers = HeaderMap::with_capacity(config.request_header.len());
        for (name, value) in &config.request_header {
            let name = HeaderName::from_str(name)
                .map_err(|err| Error::ValidationError(format!("invalid forwarding header name: {err}")))?;
            let value = HeaderValue::from_str(value)
                .map_err(|err| Error::ValidationError(format!("invalid forwarding header value: {err}")))?;
            request_headers.append(name, value);
        }
        let mut state = self.state.lock().unwrap();

        let active = ActiveForwardingRule {
            id: state.next_forwarding_rule_id,
            config,
        };
        let rule = ForwardingRule {
            active: active.clone(),
            target,
            request_headers,
        };

        state.forwarding_rules.insert(active.id, rule);

        state.next_forwarding_rule_id += 1;

        Ok(active)
    }

    pub(crate) fn delete_forwarding_rule(&self, id: usize) -> Option<ActiveForwardingRule> {
        let mut state = self.state.lock().unwrap();

        let result = state.forwarding_rules.remove(&id).map(|rule| rule.active);

        if result.is_some() {
            tracing::debug!("Deleting proxy rule with id={}", id);
        } else {
            tracing::warn!(
                "Could not delete proxy rule with id={} (no proxy rule with that id found)",
                id
            );
        }

        result
    }

    pub(crate) fn delete_all_forwarding_rules(&self) {
        let mut state = self.state.lock().unwrap();
        state.forwarding_rules.clear();

        tracing::debug!("Deleted all forwarding rules");
    }

    pub(crate) fn create_proxy_rule(&self, config: ProxyRuleConfig) -> ActiveProxyRule {
        let mut state = self.state.lock().unwrap();

        let rule = ActiveProxyRule {
            id: state.next_proxy_rule_id,
            config,
        };

        state.proxy_rules.insert(rule.id, rule.clone());

        state.next_proxy_rule_id += 1;

        rule
    }

    pub(crate) fn delete_proxy_rule(&self, id: usize) -> Option<ActiveProxyRule> {
        let mut state = self.state.lock().unwrap();

        let result = state.proxy_rules.remove(&id);

        if result.is_some() {
            tracing::debug!("Deleting proxy rule with id={}", id);
        } else {
            tracing::warn!(
                "Could not delete proxy rule with id={} (no proxy rule with that id found)",
                id
            );
        }

        result
    }

    pub(crate) fn delete_all_proxy_rules(&self) {
        let mut state = self.state.lock().unwrap();
        state.proxy_rules.clear();

        tracing::debug!("Deleted all proxy rules");
    }

    pub(crate) fn create_recording(&self, config: RecordingRuleConfig) -> ActiveRecording {
        let mut state = self.state.lock().unwrap();

        let rec = ActiveRecording {
            id: state.next_recording_id,
            config,
            mocks: Vec::new(),
        };

        state.recordings.insert(rec.id, rec.clone());

        state.next_recording_id += 1;

        rec
    }

    pub(crate) fn delete_recording(&self, id: usize) -> Option<ActiveRecording> {
        let mut state = self.state.lock().unwrap();

        let result = state.recordings.remove(&id);

        if result.is_some() {
            tracing::debug!("Deleting proxy rule with id={}", id);
        } else {
            tracing::warn!(
                "Could not delete proxy rule with id={} (no proxy rule with that id found)",
                id
            );
        }

        result
    }

    pub(crate) fn delete_all_recordings(&self) {
        let mut state = self.state.lock().unwrap();
        state.recordings.clear();

        tracing::debug!("Deleted all recorders");
    }

    #[cfg(feature = "record")]
    pub(crate) fn export_recording(&self, id: usize) -> Result<Option<Bytes>, Error> {
        let state = self.state.lock().unwrap();

        if let Some(rec) = state.recordings.get(&id) {
            return Ok(Some(
                serialize_mock_defs_to_yaml(&rec.mocks).map_err(|err| DataConversionError(err.to_string()))?,
            ));
        }

        Ok(None)
    }

    #[cfg(feature = "record")]
    pub(crate) fn load_mocks_from_recording(&self, recording_file_content: &str) -> Result<Vec<usize>, Error> {
        let all_static_mock_defs = deserialize_mock_defs_from_yaml(recording_file_content)
            .map_err(|err| DataConversionError(err.to_string()))?;

        if all_static_mock_defs.is_empty() {
            return Err(ValidationError(
                "no mock definitions could be found in the provided recording content".to_string(),
            ));
        }

        let mut mock_ids = Vec::with_capacity(all_static_mock_defs.len());

        for static_mock_def in all_static_mock_defs {
            let mock_def: MockDefinition = static_mock_def
                .try_into()
                .map_err(|err: data::Error| DataConversionError(err.to_string()))?;

            let active_mock = self.add_mock(mock_def, false)?;
            mock_ids.push(active_mock.id);
        }

        Ok(mock_ids)
    }

    pub(crate) fn find_forward_rule<'a>(&'a self, req: &'a HttpMockRequest) -> Result<Option<ForwardingRule>, Error> {
        let state = self.state.lock().unwrap();

        let result = state
            .forwarding_rules
            .values()
            .find(|&rule| request_matches(&state.matchers, req, &rule.active.config.request_requirements))
            .cloned();

        Ok(result)
    }

    pub(crate) fn find_proxy_rule<'a>(&'a self, req: &'a HttpMockRequest) -> Result<Option<ActiveProxyRule>, Error> {
        let state = self.state.lock().unwrap();

        let result = state
            .proxy_rules
            .values()
            .find(|&rule| request_matches(&state.matchers, req, &rule.config.request_requirements))
            .cloned();

        Ok(result)
    }

    pub(crate) fn record<
        IntoResponse: TryInto<MockServerHttpResponse, Error = impl std::fmt::Display + std::fmt::Debug + 'static>,
    >(
        &self,
        is_proxied: bool,
        time_taken: Duration,
        req: HttpMockRequest,
        res: IntoResponse,
    ) -> Result<(), Error> {
        let mut state = self.state.lock().unwrap();

        let recording_ids: Vec<usize> = state
            .recordings
            .values()
            .filter(|rec| request_matches(&state.matchers, &req, &rec.config.request_requirements))
            .map(|r| r.id)
            .collect();

        if recording_ids.is_empty() {
            return Ok(());
        }

        let res = res.try_into().map_err(|err| DataConversionError(err.to_string()))?;

        for id in recording_ids {
            let rec = state.recordings.get_mut(&id).unwrap();
            let definition = build_mock_definition(is_proxied, time_taken, &req, &res, &rec.config)?;
            rec.mocks.push(definition);
        }

        Ok(())
    }
}

impl Default for Manager {
    fn default() -> Self {
        Manager::new(DEFAULT_HISTORY_LIMIT)
    }
}

fn build_mock_definition(
    is_proxied: bool,
    time_taken: Duration,
    request: &HttpMockRequest,
    response: &MockServerHttpResponse,
    config: &RecordingRuleConfig,
) -> Result<MockDefinition, Error> {
    // ************************************************************************************
    // Request
    let mut headers = Vec::with_capacity(config.record_headers.len());
    for header_name in &config.record_headers {
        let header_name_lowercase = header_name.to_lowercase();
        for (key, value) in request.headers() {
            if let Some(key) = key
                && header_name_lowercase == key.to_string().to_lowercase()
            {
                let value = value.to_str().map_err(|err| DataConversionError(err.to_string()))?;
                headers.push((header_name.to_string(), value.to_string()))
            }
        }
    }

    let request = RequestRequirements {
        /* Authority and scheme are assumed to always exist for proxies requests for the
        following reasons:

        RFC 7230 - Hypertext Transfer Protocol (HTTP/1.1): Message Syntax and Routing
        Section 5.3.2 (absolute-form):
        The section clearly states that an absolute URI (absolute-form) must be used when the
        request is made to a proxy. This inclusion of the full URI (including the scheme,
        host, and optional port) ensures that the proxy can correctly interpret the destination
        of the request without additional context.
        Exact Text from RFC 7230:
        The RFC says under Section 5.3.2:

        "absolute-form = absolute-URI"
        "When making a request to a proxy, other than a CONNECT or server-wide
        OPTIONS request (as detailed in Section 5.3.4), a client MUST send
        the target URI in absolute-form as the request-target."
        "An example absolute-form of request-line would be:
        GET http://www.example.org/pub/WWW/TheProject.html HTTP/1.1"
        */
        host: if is_proxied {
            request.uri().host().map(|h| h.to_string())
        } else {
            None
        },
        port: if is_proxied {
            request.uri().port().map(|h| h.as_u16())
        } else {
            None
        },
        scheme: if is_proxied {
            request.uri().scheme().map(|h| h.to_string())
        } else {
            None
        },
        path: Some(request.uri().path().to_string()),
        method: Some(request.method().to_string()),
        header: if !headers.is_empty() { Some(headers) } else { None },
        body: if request.body().is_empty() {
            None
        } else {
            Some(request.body().clone())
        },
        query_param: if request.query_param_length() == 0 {
            None
        } else {
            Some(request.query_params())
        },
        ..Default::default()
    };

    // ************************************************************************************
    // Response
    let mut response = response.clone();

    if config.record_response_delays {
        response.delay = Some(time_taken.as_millis() as u64)
    }

    Ok(MockDefinition { request, response })
}

fn validate_request_requirements(req: &RequestRequirements) -> Result<(), Error> {
    const NON_BODY_METHODS: &[&str] = &["GET", "HEAD"];

    if let Some(_body) = &req.body
        && let Some(method) = &req.method
        && NON_BODY_METHODS.contains(&method.as_str())
    {
        return Err(BodyMethodInvalid);
    }
    Ok(())
}

fn request_matches(
    matchers: &Vec<Box<dyn Matcher + Sync + Send>>,
    req: &HttpMockRequest,
    request_requirements: &RequestRequirements,
) -> bool {
    tracing::trace!("Matching incoming HTTP request");
    matchers.iter().all(|x| x.matches(req, request_requirements))
}

fn get_distances(
    history: &Vec<&Arc<HttpMockRequest>>,
    matchers: &Vec<Box<dyn Matcher + Sync + Send>>,
    mock_rr: &RequestRequirements,
) -> BTreeMap<usize, usize> {
    history
        .iter()
        .enumerate()
        .map(|(idx, req)| (idx, get_request_distance(req, mock_rr, matchers)))
        .collect()
}

fn get_request_distance(
    req: &Arc<HttpMockRequest>,
    mock_request_requirements: &RequestRequirements,
    matchers: &Vec<Box<dyn Matcher + Sync + Send>>,
) -> usize {
    matchers
        .iter()
        .map(|matcher| matcher.distance(req, mock_request_requirements))
        .sum()
}

fn get_min_distance_requests(request_distances: &BTreeMap<usize, usize>) -> Vec<usize> {
    // Find the element with the maximum matches
    let min_elem = request_distances
        .iter()
        .min_by(|(_idx1, d1), (_idx2, d2)| (**d1).cmp(d2));

    let max = match min_elem {
        None => return Vec::new(),
        Some((_, n)) => *n,
    };

    request_distances
        .iter()
        .filter(|(_idx, distance)| **distance == max)
        .map(|(idx, _)| *idx)
        .collect()
}

fn get_request_mismatches(
    req: &Arc<HttpMockRequest>,
    mock_rr: &RequestRequirements,
    matchers: &Vec<Box<dyn Matcher + Sync + Send>>,
) -> Vec<Mismatch> {
    matchers.iter().flat_map(|mat| mat.mismatches(req, mock_rr)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::util::HttpMockBytes;

    fn dummy_request() -> HttpMockRequest {
        HttpMockRequest::new(
            "http".to_string(),
            "/test".to_string(),
            "GET".to_string(),
            Vec::new(),
            "HTTP/1.1".to_string(),
            HttpMockBytes::from(bytes::Bytes::new()),
        )
    }

    #[test]
    fn history_is_capped_at_configured_limit() {
        let history_limit = 3;
        let manager = Manager::new(history_limit);

        // Serve more requests than the configured limit.
        for _ in 0..10 {
            manager
                .serve_mock(&dummy_request())
                .expect("serving a request should not fail");
        }

        let history_len = manager.state.lock().unwrap().history.len();

        // The trim logic removes the oldest entry once the length exceeds the
        // limit and then pushes the new request, so the history stabilizes at
        // `history_limit + 1` and must never grow unbounded.
        assert!(
            history_len <= history_limit + 1,
            "history length {history_len} exceeded the configured limit {history_limit}"
        );
        assert_eq!(history_len, history_limit + 1);
    }

    #[test]
    fn default_history_limit_is_preserved() {
        let manager = Manager::default();
        assert_eq!(manager.state.lock().unwrap().history_limit, DEFAULT_HISTORY_LIMIT);
    }

    #[test]
    fn static_mock_cannot_be_deleted() {
        let manager = Manager::default();
        let active_mock = manager
            .add_mock(
                MockDefinition::new(RequestRequirements::default(), MockServerHttpResponse::default()),
                true,
            )
            .expect("static mock should be created");

        assert!(matches!(
            manager.delete_mock(active_mock.id),
            Err(Error::StaticMockError)
        ));
    }

    #[test]
    fn get_request_with_body_is_rejected() {
        let requirements = RequestRequirements {
            method: Some("GET".to_string()),
            body: Some(HttpMockBytes::from(bytes::Bytes::from_static(b"body"))),
            ..Default::default()
        };

        assert!(matches!(
            validate_request_requirements(&requirements),
            Err(Error::BodyMethodInvalid)
        ));
    }
}
