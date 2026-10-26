#[cfg(feature = "proxy")]
mod proxy;
#[cfg(feature = "record")]
mod record;

use std::{
    collections::BTreeMap,
    num::NonZero,
    sync::{Arc, Mutex},
};

use thiserror::Error;

use crate::{
    common::data::{ActiveMock, ClosestMatch, Mismatch, MockDefinition, MockServerHttpResponse, RequestRequirements},
    prelude::HttpMockRequest,
    server::{
        matchers,
        state::Error::{BodyMethodInvalid, StaticMockError, ValidationError},
    },
};

#[derive(Error, Debug)]
pub enum Error {
    #[error("The mock is static and cannot be modified or deleted")]
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
pub(crate) struct MockServerState {
    next_mock_id: usize,
    history_limit: usize,
    pub mocks: BTreeMap<usize, ActiveMock>,
    pub history: Vec<Arc<HttpMockRequest>>,
    #[cfg(feature = "proxy")]
    proxy: proxy::State,
    #[cfg(feature = "record")]
    recording: record::State,
}

impl MockServerState {
    pub fn new(history_limit: usize) -> Self {
        MockServerState {
            mocks: BTreeMap::new(),
            #[cfg(feature = "proxy")]
            proxy: Default::default(),
            #[cfg(feature = "record")]
            recording: Default::default(),
            history: Vec::new(),
            history_limit,
            next_mock_id: 0,
        }
    }
}

/// Owns the mock server's state and serialises access to it.
pub(crate) struct HttpMockStateManager {
    state: Mutex<MockServerState>,
}

impl HttpMockStateManager {
    pub(crate) fn new(history_limit: usize) -> Self {
        Self {
            state: Mutex::new(MockServerState::new(history_limit)),
        }
    }

    pub(crate) fn reset(&self) {
        self.delete_all_mocks();
        self.reset_static_mocks();
        self.delete_history();
        #[cfg(feature = "proxy")]
        {
            self.delete_all_forwarding_rules();
            self.delete_all_proxy_rules();
        }
        #[cfg(feature = "record")]
        self.delete_all_recordings();
    }

    pub(crate) fn add_mock(&self, definition: MockDefinition) -> Result<ActiveMock, Error> {
        self.insert_mock(definition, false)
    }

    /// Adds a mock that survives resets and can't be deleted or limited, like the ones read
    /// from the static mock directory.
    #[cfg(any(test, feature = "record"))]
    pub(crate) fn add_static_mock(&self, definition: MockDefinition) -> Result<ActiveMock, Error> {
        self.insert_mock(definition, true)
    }

    fn insert_mock(&self, mut definition: MockDefinition, is_static: bool) -> Result<ActiveMock, Error> {
        validate_request_requirements(&definition.request)?;
        let up_to_n_times = definition
            .up_to_n_times
            .take()
            .map(validate_up_to_n_times)
            .transpose()?;

        let mut state = self.state.lock().unwrap();

        let id = state.next_mock_id;
        let active_mock = ActiveMock {
            id,
            call_counter: 0,
            definition,
            is_static,
            up_to_n_times,
        };

        tracing::debug!("Adding new mock with ID={}", id);

        state.mocks.insert(id, active_mock.clone());

        state.next_mock_id += 1;

        Ok(active_mock)
    }

    pub(crate) fn read_mock(&self, id: usize) -> Result<Option<ActiveMock>, Error> {
        let state = self.state.lock().unwrap();

        Ok(state.mocks.get(&id).cloned())
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

    /// Limits the mock with the given ID to `count` calls in total, including the calls it has
    /// already served. Returns `false` if no such mock exists.
    pub(crate) fn set_up_to_n_times(&self, id: usize, count: usize) -> Result<bool, Error> {
        let count = validate_up_to_n_times(count)?;

        let mut state = self.state.lock().unwrap();

        let Some(mock) = state.mocks.get_mut(&id) else {
            return Ok(false);
        };

        if mock.is_static {
            return Err(StaticMockError);
        }

        tracing::debug!("Limiting mock with id={} to {} calls", id, count);

        mock.up_to_n_times = Some(count);

        Ok(true)
    }

    pub(crate) fn delete_all_mocks(&self) {
        let mut state = self.state.lock().unwrap();

        state.mocks.retain(|_, mock| mock.is_static);

        tracing::trace!("Deleted all mocks");
    }

    /// Clears the call counters of static mocks, which survive a reset, letting exhausted ones
    /// match again.
    fn reset_static_mocks(&self) {
        let mut state = self.state.lock().unwrap();

        state
            .mocks
            .values_mut()
            .filter(|mock| mock.is_static)
            .for_each(|mock| mock.call_counter = 0);

        tracing::trace!("Reset static mocks");
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
            .filter(|req| !request_matches(req, requirements))
            .collect();

        let request_distances = get_distances(&non_matching_requests, requirements);
        let best_matches = get_min_distance_requests(&request_distances);

        let closes_match_request_idx = match best_matches.first() {
            None => return Ok(None),
            Some(idx) => *idx,
        };

        let req = non_matching_requests.get(closes_match_request_idx).unwrap();
        let mismatches = get_request_mismatches(req, requirements);

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
            .find(|&mock| !mock.is_exhausted() && request_matches(&req, &mock.definition.request));

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

impl Default for HttpMockStateManager {
    fn default() -> Self {
        HttpMockStateManager::new(DEFAULT_HISTORY_LIMIT)
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

fn validate_up_to_n_times(count: usize) -> Result<NonZero<usize>, Error> {
    NonZero::new(count).ok_or_else(|| ValidationError("up_to_n_times must be greater than 0".to_string()))
}

fn request_matches(req: &HttpMockRequest, request_requirements: &RequestRequirements) -> bool {
    tracing::trace!("Matching incoming HTTP request");
    matchers::all().all(|x| x.matches(req, request_requirements))
}

fn get_distances(history: &[&Arc<HttpMockRequest>], mock_rr: &RequestRequirements) -> BTreeMap<usize, usize> {
    history
        .iter()
        .enumerate()
        .map(|(idx, req)| (idx, get_request_distance(req, mock_rr)))
        .collect()
}

fn get_request_distance(req: &Arc<HttpMockRequest>, mock_request_requirements: &RequestRequirements) -> usize {
    matchers::all()
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

fn get_request_mismatches(req: &Arc<HttpMockRequest>, mock_rr: &RequestRequirements) -> Vec<Mismatch> {
    matchers::all().flat_map(|mat| mat.mismatches(req, mock_rr)).collect()
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
        let manager = HttpMockStateManager::new(history_limit);

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

    fn limited_mock(up_to_n_times: Option<usize>) -> MockDefinition {
        MockDefinition {
            request: RequestRequirements::default(),
            response: MockServerHttpResponse::default(),
            up_to_n_times,
        }
    }

    #[test]
    fn exhausted_mock_stops_matching_but_keeps_its_call_counter() {
        let manager = HttpMockStateManager::default();
        let id = manager.add_mock(limited_mock(Some(2))).unwrap().id;

        let served: Vec<bool> = (0..3)
            .map(|_| manager.serve_mock(&dummy_request()).unwrap().is_some())
            .collect();

        assert_eq!(served, vec![true, true, false]);
        assert_eq!(manager.read_mock(id).unwrap().unwrap().call_counter, 2);
    }

    #[test]
    fn up_to_n_times_counts_calls_served_before_the_limit_was_set() {
        let manager = HttpMockStateManager::default();
        let id = manager.add_mock(limited_mock(None)).unwrap().id;
        for _ in 0..3 {
            assert!(manager.serve_mock(&dummy_request()).unwrap().is_some());
        }

        assert!(manager.set_up_to_n_times(id, 2).unwrap());

        assert!(manager.serve_mock(&dummy_request()).unwrap().is_none());
        assert_eq!(manager.read_mock(id).unwrap().unwrap().call_counter, 3);
    }

    #[test]
    fn raising_up_to_n_times_lets_an_exhausted_mock_match_again() {
        let manager = HttpMockStateManager::default();
        let id = manager.add_mock(limited_mock(Some(1))).unwrap().id;
        assert!(manager.serve_mock(&dummy_request()).unwrap().is_some());
        assert!(manager.serve_mock(&dummy_request()).unwrap().is_none());

        assert!(manager.set_up_to_n_times(id, 2).unwrap());

        let served: Vec<bool> = (0..2)
            .map(|_| manager.serve_mock(&dummy_request()).unwrap().is_some())
            .collect();
        assert_eq!(served, vec![true, false]);
        assert_eq!(manager.read_mock(id).unwrap().unwrap().call_counter, 2);
    }

    #[test]
    fn up_to_n_times_on_unknown_mock_reports_not_found() {
        let manager = HttpMockStateManager::default();

        assert!(!manager.set_up_to_n_times(42, 1).unwrap());
    }

    #[test]
    fn up_to_n_times_on_static_mock_is_rejected() {
        let manager = HttpMockStateManager::default();
        let id = manager.add_static_mock(limited_mock(None)).unwrap().id;

        assert!(matches!(manager.set_up_to_n_times(id, 1), Err(StaticMockError)));
    }

    #[test]
    fn reset_lets_exhausted_static_mocks_match_again() {
        let manager = HttpMockStateManager::default();
        let id = manager.add_static_mock(limited_mock(Some(1))).unwrap().id;
        assert!(manager.serve_mock(&dummy_request()).unwrap().is_some());
        assert!(manager.serve_mock(&dummy_request()).unwrap().is_none());

        manager.reset();

        assert_eq!(manager.read_mock(id).unwrap().unwrap().call_counter, 0);
        assert!(manager.serve_mock(&dummy_request()).unwrap().is_some());
    }

    #[test]
    fn zero_up_to_n_times_is_rejected() {
        let manager = HttpMockStateManager::default();

        assert!(matches!(
            manager.add_mock(limited_mock(Some(0))),
            Err(ValidationError(_))
        ));

        let id = manager.add_mock(limited_mock(None)).unwrap().id;
        assert!(matches!(manager.set_up_to_n_times(id, 0), Err(ValidationError(_))));
        assert!(manager.read_mock(id).unwrap().unwrap().up_to_n_times.is_none());
    }

    #[test]
    fn exhausted_mock_round_trips_through_serialization() {
        let manager = HttpMockStateManager::default();
        let id = manager.add_mock(limited_mock(Some(1))).unwrap().id;
        assert!(manager.serve_mock(&dummy_request()).unwrap().is_some());

        let json = serde_json::to_string(&manager.read_mock(id).unwrap().unwrap()).unwrap();
        let mock: ActiveMock = serde_json::from_str(&json).unwrap();

        assert_eq!(mock.up_to_n_times.map(NonZero::get), Some(1));
        assert_eq!(mock.call_counter, 1);
        assert!(mock.is_exhausted());
    }

    #[test]
    fn default_history_limit_is_preserved() {
        let manager = HttpMockStateManager::default();
        assert_eq!(manager.state.lock().unwrap().history_limit, DEFAULT_HISTORY_LIMIT);
    }
}
