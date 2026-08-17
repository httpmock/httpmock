#[cfg(feature = "proxy")]
mod proxy;
#[cfg(feature = "record")]
mod record;

use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex},
};

use thiserror::Error;

use crate::{
    common::data::{ActiveMock, ClosestMatch, Mismatch, MockDefinition, MockServerHttpResponse, RequestRequirements},
    prelude::HttpMockRequest,
    server::{
        matchers,
        matchers::Matcher,
        state::Error::{BodyMethodInvalid, StaticMockError},
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
    history_limit: usize,
    pub mocks: BTreeMap<usize, ActiveMock>,
    pub history: Vec<Arc<HttpMockRequest>>,
    pub matchers: Vec<Box<dyn Matcher + Sync + Send>>,
    #[cfg(feature = "proxy")]
    proxy: proxy::State,
    #[cfg(feature = "record")]
    recording: record::State,
}

impl Inner {
    pub fn new(history_limit: usize) -> Self {
        Inner {
            mocks: BTreeMap::new(),
            #[cfg(feature = "proxy")]
            proxy: Default::default(),
            #[cfg(feature = "record")]
            recording: Default::default(),
            history: Vec::new(),
            history_limit,
            next_mock_id: 0,
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
        #[cfg(feature = "proxy")]
        {
            self.delete_all_forwarding_rules();
            self.delete_all_proxy_rules();
        }
        #[cfg(feature = "record")]
        self.delete_all_recordings();
    }

    pub(crate) fn add_mock(&self, definition: MockDefinition, is_static: bool) -> Result<ActiveMock, Error> {
        validate_request_requirements(&definition.request)?;

        let mut state = self.state.lock().unwrap();

        let id = state.next_mock_id;
        let active_mock = ActiveMock {
            id,
            call_counter: 0,
            definition,
            is_static,
        };

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
}

impl Default for Manager {
    fn default() -> Self {
        Manager::new(DEFAULT_HISTORY_LIMIT)
    }
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
}
