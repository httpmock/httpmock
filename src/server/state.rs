use crate::{
    common::{
        data,
        data::{
            ActiveForwardingRule, ActiveMock, ActiveProxyRule, ActiveRecording, ClosestMatch,
            Mismatch, MockDefinition, MockServerHttpResponse, RequestRequirements,
        },
    },
    prelude::HttpMockRequest,
    server::{
        matchers,
        matchers::{all, Matcher},
        state::Error::{BodyMethodInvalid, DataConversionError, StaticMockError, ValidationError},
    },
};

#[cfg(feature = "record")]
use crate::server::persistence::{deserialize_mock_defs_from_yaml, serialize_mock_defs_to_yaml};

use crate::common::data::{ForwardingRuleConfig, ProxyRuleConfig, RecordingRuleConfig};
use bytes::Bytes;
use std::{
    collections::BTreeMap,
    convert::{TryFrom, TryInto},
    sync::{Arc, Mutex},
    time::Duration,
};
use thiserror::Error;

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

pub struct MockServerState {
    history_limit: usize,
    next_mock_id: usize,
    next_forwarding_rule_id: usize,
    next_proxy_rule_id: usize,
    next_recording_id: usize,
    pub mocks: BTreeMap<usize, ActiveMock>,
    pub history: Vec<Arc<HttpMockRequest>>,
    pub matchers: Vec<Box<dyn Matcher + Sync + Send>>,
    pub forwarding_rules: BTreeMap<usize, ActiveForwardingRule>,
    pub proxy_rules: BTreeMap<usize, ActiveProxyRule>,
    pub recordings: BTreeMap<usize, ActiveRecording>,
}

impl MockServerState {
    pub fn new(history_limit: usize) -> Self {
        MockServerState {
            mocks: BTreeMap::new(),
            forwarding_rules: BTreeMap::new(),
            proxy_rules: BTreeMap::new(),
            recordings: BTreeMap::new(),
            history_limit,
            history: Vec::new(),
            next_mock_id: 0,
            next_forwarding_rule_id: 0,
            next_proxy_rule_id: 0,
            next_recording_id: 0,
            matchers: matchers::all(),
        }
    }
}

pub(crate) trait StateManager {
    fn reset(&self);
    fn add_mock(&self, definition: MockDefinition, is_static: bool) -> Result<ActiveMock, Error>;
    fn read_mock(&self, id: usize) -> Result<Option<ActiveMock>, Error>;
    fn delete_mock(&self, id: usize) -> Result<bool, Error>;
    fn delete_mock_after_calls(&self, id: usize, count: usize) -> Result<bool, Error>;
    fn delete_all_mocks(&self);

    fn delete_history(&self);

    fn verify(&self, requirements: &RequestRequirements) -> Result<Option<ClosestMatch>, Error>;

    fn serve_mock(&self, req: &HttpMockRequest) -> Result<Option<MockServerHttpResponse>, Error>;

    fn create_forwarding_rule(&self, config: ForwardingRuleConfig) -> ActiveForwardingRule;
    fn delete_forwarding_rule(&self, id: usize) -> Option<ActiveForwardingRule>;
    fn delete_all_forwarding_rules(&self);

    fn create_proxy_rule(&self, constraints: ProxyRuleConfig) -> ActiveProxyRule;
    fn delete_proxy_rule(&self, id: usize) -> Option<ActiveProxyRule>;
    fn delete_all_proxy_rules(&self);

    fn create_recording(&self, config: RecordingRuleConfig) -> ActiveRecording;
    fn delete_recording(&self, recording_id: usize) -> Option<ActiveRecording>;
    fn delete_all_recordings(&self);

    #[cfg(feature = "record")]
    fn export_recording(&self, id: usize) -> Result<Option<Bytes>, Error>;

    #[cfg(feature = "record")]
    fn load_mocks_from_recording(&self, recording_file_content: &str) -> Result<Vec<usize>, Error>;

    fn find_forward_rule<'a>(
        &'a self,
        req: &'a HttpMockRequest,
    ) -> Result<Option<ActiveForwardingRule>, Error>;
    fn find_proxy_rule<'a>(
        &'a self,
        req: &'a HttpMockRequest,
    ) -> Result<Option<ActiveProxyRule>, Error>;
    fn record<
        IntoResponse: TryInto<MockServerHttpResponse, Error = impl std::fmt::Display + std::fmt::Debug + 'static>,
    >(
        &self,
        is_proxied: bool,
        time_taken: Duration,
        req: HttpMockRequest,
        res: IntoResponse,
    ) -> Result<(), Error>;
}

pub struct HttpMockStateManager {
    state: Mutex<MockServerState>,
}

impl HttpMockStateManager {
    pub fn new(history_limit: usize) -> Self {
        Self {
            state: Mutex::new(MockServerState::new(history_limit)),
        }
    }
}

impl Default for HttpMockStateManager {
    fn default() -> Self {
        HttpMockStateManager::new(usize::MAX)
    }
}

impl StateManager for HttpMockStateManager {
    fn reset(&self) {
        self.delete_all_mocks();
        self.delete_history();
        self.delete_all_forwarding_rules();
        self.delete_all_proxy_rules();
        self.delete_all_recordings();
    }

    fn add_mock(&self, definition: MockDefinition, is_static: bool) -> Result<ActiveMock, Error> {
        validate_request_requirements(&definition.request)?;

        let mut state = self.state.lock().unwrap();

        let id = state.next_mock_id;
        let active_mock = ActiveMock::new(id, definition, 0, None, is_static);

        tracing::debug!("Adding new mock with ID={}", id);

        state.mocks.insert(id, active_mock.clone());

        state.next_mock_id += 1;

        Ok(active_mock)
    }

    fn read_mock(&self, id: usize) -> Result<Option<ActiveMock>, Error> {
        let mut state = self.state.lock().unwrap();

        let result = state.mocks.get(&id);
        match result {
            Some(found) => Ok(Some(found.clone())),
            None => Ok(None),
        }
    }

    fn delete_mock(&self, id: usize) -> Result<bool, Error> {
        let mut state = self.state.lock().unwrap();

        if let Some(m) = state.mocks.get(&id) {
            if m.is_static {
                return Err(StaticMockError);
            }
        }

        tracing::debug!("Deleting mock with id={}", id);

        Ok(state.mocks.remove(&id).is_some())
    }

    fn delete_mock_after_calls(&self, id: usize, count: usize) -> Result<bool, Error> {
        let mut state = self.state.lock().unwrap();

        if let Some(mock) = state.mocks.get_mut(&id) {
            if mock.is_static {
                return Err(StaticMockError);
            }
            mock.delete_after = Some(count);

            return Ok(true);
        };

        Ok(false)
    }

    fn delete_all_mocks(&self) {
        let mut state = self.state.lock().unwrap();

        let ids: Vec<usize> = state
            .mocks
            .iter()
            .filter(|(k, v)| !v.is_static)
            .map(|(k, v)| *k)
            .collect();

        ids.iter().for_each(|k| {
            state.mocks.remove(k);
        });

        tracing::trace!("Deleted all mocks");
    }

    fn delete_history(&self) {
        let mut state = self.state.lock().unwrap();
        state.history.clear();
        tracing::trace!("Deleted request history");
    }

    fn verify(&self, requirements: &RequestRequirements) -> Result<Option<ClosestMatch>, Error> {
        let mut state = self.state.lock().unwrap();

        let non_matching_requests: Vec<&Arc<HttpMockRequest>> = state
            .history
            .iter()
            .filter(|req| !request_matches(&state.matchers, req, requirements))
            .collect();

        let request_distances =
            get_distances(&non_matching_requests, &state.matchers, requirements);
        let best_matches = get_min_distance_requests(&request_distances);

        let closes_match_request_idx = match best_matches.get(0) {
            None => return Ok(None),
            Some(idx) => *idx,
        };

        let req = non_matching_requests.get(closes_match_request_idx).unwrap();
        let mismatches = get_request_mismatches(req, &requirements, &state.matchers);

        Ok(Some(ClosestMatch {
            request: HttpMockRequest::clone(&req),
            request_index: closes_match_request_idx,
            mismatches,
        }))
    }

    fn serve_mock(&self, req: &HttpMockRequest) -> Result<Option<MockServerHttpResponse>, Error> {
        let mut state = self.state.lock().unwrap();

        let req = Arc::new(req.clone());

        if state.history.len() > 100 {
            // TODO: Make max history configurable
            state.history.remove(0);
        }
        state.history.push(req.clone());

        let result = state
            .mocks
            .values()
            .find(|&mock| request_matches(&state.matchers, &req, &mock.definition.request));

        let found_mock_id = match result {
            Some(mock) => Some(mock.id),
            None => None,
        };

        if let Some(found_id) = found_mock_id {
            tracing::debug!(
                "Matched mock with id={} to the following request: {:#?}",
                found_id,
                req
            );

            let mock = state.mocks.get_mut(&found_id).unwrap();
            mock.call_counter += 1;
            let resp = mock.definition.response.clone();

            if let Some(delete_after) = mock.delete_after {
                if mock.call_counter >= delete_after {
                    state.mocks.remove(&found_id);
                }
            }

            return Ok(Some(resp));
        }

        tracing::debug!(
            "Could not match any mock to the following request: {:#?}",
            req
        );

        Ok(None)
    }

    fn create_forwarding_rule(&self, config: ForwardingRuleConfig) -> ActiveForwardingRule {
        let mut state = self.state.lock().unwrap();

        let rule = ActiveForwardingRule {
            id: state.next_forwarding_rule_id,
            config,
        };

        state.forwarding_rules.insert(rule.id, rule.clone());

        state.next_forwarding_rule_id += 1;

        rule
    }

    fn delete_forwarding_rule(&self, id: usize) -> Option<ActiveForwardingRule> {
        let mut state = self.state.lock().unwrap();

        let result = state.forwarding_rules.remove(&id);

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

    fn delete_all_forwarding_rules(&self) {
        let mut state = self.state.lock().unwrap();
        state.forwarding_rules.clear();

        tracing::debug!("Deleted all forwarding rules");
    }

    fn create_proxy_rule(&self, config: ProxyRuleConfig) -> ActiveProxyRule {
        let mut state = self.state.lock().unwrap();

        let rule = ActiveProxyRule {
            id: state.next_proxy_rule_id,
            config,
        };

        state.proxy_rules.insert(rule.id, rule.clone());

        state.next_proxy_rule_id += 1;

        rule
    }

    fn delete_proxy_rule(&self, id: usize) -> Option<ActiveProxyRule> {
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

    fn delete_all_proxy_rules(&self) {
        let mut state = self.state.lock().unwrap();
        state.proxy_rules.clear();

        tracing::debug!("Deleted all proxy rules");
    }

    fn create_recording(&self, config: RecordingRuleConfig) -> ActiveRecording {
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

    fn delete_recording(&self, id: usize) -> Option<ActiveRecording> {
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

    fn delete_all_recordings(&self) {
        let mut state = self.state.lock().unwrap();
        state.recordings.clear();

        tracing::debug!("Deleted all recorders");
    }

    #[cfg(feature = "record")]
    fn export_recording(&self, id: usize) -> Result<Option<Bytes>, Error> {
        let mut state = self.state.lock().unwrap();

        if let Some(rec) = state.recordings.get(&id) {
            return Ok(Some(
                serialize_mock_defs_to_yaml(&rec.mocks)
                    .map_err(|err| DataConversionError(err.to_string()))?,
            ));
        }

        Ok(None)
    }

    #[cfg(feature = "record")]
    fn load_mocks_from_recording(&self, recording_file_content: &str) -> Result<Vec<usize>, Error> {
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

    fn find_forward_rule<'a>(
        &'a self,
        req: &'a HttpMockRequest,
    ) -> Result<(Option<ActiveForwardingRule>), Error> {
        let mut state = self.state.lock().unwrap();

        let result = state
            .forwarding_rules
            .values()
            .find(|&rule| request_matches(&state.matchers, req, &rule.config.request_requirements))
            .cloned();

        Ok(result)
    }

    fn find_proxy_rule<'a>(
        &'a self,
        req: &'a HttpMockRequest,
    ) -> Result<Option<ActiveProxyRule>, Error> {
        let mut state = self.state.lock().unwrap();

        let result = state
            .proxy_rules
            .values()
            .find(|&rule| request_matches(&state.matchers, req, &rule.config.request_requirements))
            .cloned();

        Ok(result)
    }

    fn record<
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

        let res = res
            .try_into()
            .map_err(|err| DataConversionError(err.to_string()))?;

        for id in recording_ids {
            let rec = state.recordings.get_mut(&id).unwrap();
            let definition =
                build_mock_definition(is_proxied, time_taken, &req, &res, &rec.config)?;
            rec.mocks.push(definition);
        }

        Ok(())
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
            if let Some(key) = key {
                if header_name_lowercase == key.to_string().to_lowercase() {
                    let value = value
                        .to_str()
                        .map_err(|err| DataConversionError(err.to_string()))?;
                    headers.push((header_name.to_string(), value.to_string()))
                }
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
        host_not: None,
        host_contains: None,
        host_excludes: None,
        host_prefix: None,
        host_suffix: None,
        host_prefix_not: None,
        host_suffix_not: None,
        host_matches: None,
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
        path_not: None,
        path_includes: None,
        path_excludes: None,
        path_prefix: None,
        path_suffix: None,
        path_prefix_not: None,
        path_suffix_not: None,
        path_matches: None,
        method: Some(request.method().to_string()),
        header: if !headers.is_empty() {
            Some(headers)
        } else {
            None
        },
        header_not: None,
        header_exists: None,
        header_missing: None,
        header_includes: None,
        header_excludes: None,
        header_prefix: None,
        header_suffix: None,
        header_prefix_not: None,
        header_suffix_not: None,
        header_matches: None,
        header_count: None,
        cookie: None,
        cookie_not: None,
        cookie_exists: None,
        cookie_missing: None,
        cookie_includes: None,
        cookie_excludes: None,
        cookie_prefix: None,
        cookie_suffix: None,
        cookie_prefix_not: None,
        cookie_suffix_not: None,
        cookie_matches: None,
        cookie_count: None,
        body: if request.body().is_empty() {
            None
        } else {
            Some(request.body().clone())
        },
        json_body: None,
        json_body_not: None,
        json_body_includes: None,
        body_includes: None,
        body_excludes: None,
        body_prefix: None,
        body_suffix: None,
        body_prefix_not: None,
        body_suffix_not: None,
        body_matches: None,
        query_param_exists: None,
        query_param_missing: None,
        query_param_includes: None,
        query_param_excludes: None,
        query_param_prefix: None,
        query_param_suffix: None,
        query_param_prefix_not: None,
        query_param_suffix_not: None,
        query_param_matches: None,
        query_param_count: None,
        query_param: if request.query_param_length() == 0 {
            None
        } else {
            Some(request.query_params())
        },
        form_urlencoded_tuple_exists: None,
        form_urlencoded_tuple_missing: None,
        form_urlencoded_tuple_includes: None,
        form_urlencoded_tuple_excludes: None,
        form_urlencoded_tuple_prefix: None,
        form_urlencoded_tuple_suffix: None,
        form_urlencoded_tuple_prefix_not: None,
        form_urlencoded_tuple_suffix_not: None,
        form_urlencoded_tuple_matches: None,
        form_urlencoded_tuple_count: None,
        form_urlencoded_tuple: None,
        is_true: None,
        scheme_not: None,
        port_not: None,
        method_not: None,
        query_param_not: None,
        body_not: None,
        json_body_excludes: None,
        form_urlencoded_tuple_not: None,
        is_false: None,
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

    if let Some(_body) = &req.body {
        if let Some(method) = &req.method {
            if NON_BODY_METHODS.contains(&method.as_str()) {
                return Err(BodyMethodInvalid);
            }
        }
    }
    Ok(())
}

fn request_matches(
    matchers: &Vec<Box<dyn Matcher + Sync + Send>>,
    req: &HttpMockRequest,
    request_requirements: &RequestRequirements,
) -> bool {
    tracing::trace!("Matching incoming HTTP request");
    matchers
        .iter()
        .enumerate()
        .all(|(i, x)| x.matches(req, request_requirements))
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
        .min_by(|(idx1, d1), (idx2, d2)| (**d1).cmp(d2));

    let max = match min_elem {
        None => return Vec::new(),
        Some((_, n)) => *n,
    };

    request_distances
        .into_iter()
        .filter(|(idx, distance)| **distance == max)
        .map(|(idx, _)| *idx)
        .collect()
}

fn get_request_mismatches(
    req: &Arc<HttpMockRequest>,
    mock_rr: &RequestRequirements,
    matchers: &Vec<Box<dyn Matcher + Sync + Send>>,
) -> Vec<Mismatch> {
    matchers
        .iter()
        .map(|mat| mat.mismatches(req, mock_rr))
        .flatten()
        .into_iter()
        .collect()
}
