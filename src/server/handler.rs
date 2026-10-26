//! The mock server's sans-IO core.
//!
//! [`Handler::handle`] takes a buffered request in absolute-form and returns an [`Action`]
//! without doing any I/O; the transport carries it out. What happens to a request:
//!
//! | Request | Action | State change |
//! | --- | --- | --- |
//! | management route with a supported method | `Respond` | per route: mocks, rules, recordings, history |
//! | a forwarding rule matches | `Serve` with `Reply::Forward` | none |
//! | a proxy rule matches and no forwarding rule does | `Serve` with `Reply::Proxy` | none |
//! | a mock matches and is not exhausted | `Serve` with `Reply::Mock` and the mock's delay | request appended to the history, call counter incremented |
//! | nothing matches, including a management path with another method | `Serve` with a 404 `Reply::Mock` | request appended to the history |
//!
//! After an `Action::Serve` the transport passes the response and the time it took to
//! [`Handler::finish`], which turns them into mock definitions for every recording whose rule
//! matches the request.

use std::{
    fmt::{Debug, Display},
    str::FromStr,
    sync::Arc,
    time::Duration,
};

use http::StatusCode;
#[cfg(feature = "proxy")]
use http::{HeaderValue, Uri, uri::Scheme};
use hyper::{Method, Request, Response, body::Bytes};
use path_tree::{Path, PathTree};
use serde::{Serialize, de::DeserializeOwned};
use thiserror::Error;

#[cfg(feature = "record")]
use crate::common::data::RecordingRuleConfig;
use crate::{
    common::{
        data,
        data::{Error as DataError, ErrorResponse, MockDefinition, RequestRequirements, UpToNTimesConfig},
    },
    prelude::{HttpMockRequest, HttpMockResponse},
    server::{
        handler::Error::{
            InvalidParamFormat, MissingParam, RequestBodyDeserialization, RequestConversion, ResponseBodyConstruction,
            ResponseBodySerialization, ResponseDataConversion,
        },
        state,
    },
};
#[cfg(feature = "proxy")]
use crate::{
    common::{
        data::{ActiveForwardingRule, ActiveProxyRule, ForwardingRuleConfig, ProxyRuleConfig},
        http::Error as HttpClientError,
    },
    server::handler::Error::InvalidHeader,
};

#[derive(Error, Debug)]
pub enum Error {
    #[error("cannot deserialize request body: {0}")]
    RequestBodyDeserialization(#[source] serde_json::Error),
    #[error("cannot serialize response body: {0}")]
    ResponseBodySerialization(#[source] serde_json::Error),
    #[error("cannot construct response: {0}")]
    ResponseBodyConstruction(#[source] http::Error),
    #[error("cannot convert response body: {0}")]
    ResponseDataConversion(#[source] data::Error),
    #[error("expected URL parameters not found")]
    MissingParam,
    #[error("URL parameter format is invalid: {0}")]
    InvalidParamFormat(String),
    #[error("state operation failed: {0}")]
    State(#[from] state::Error),
    #[error("cannot convert request to internal data structure: {0}")]
    RequestConversion(String),
    // The transport sends forwarded and proxied requests, but a failed one stays a handler error
    // as in 0.9.0, since a new variant in the public `server::Error` would be a breaking change.
    #[cfg(feature = "proxy")]
    #[error("failed to send HTTP request: {0}")]
    HttpClient(#[from] HttpClientError),
    #[error("invalid header: {0}")]
    InvalidHeader(String),
}

/// What the transport has to do for a request. The [`Handler`] decides this without doing any
/// I/O, so the transport is the only place that sleeps or talks to upstream servers.
///
/// `Serve` is several times the size of `Respond` because of the request kept for recording. An
/// action is built and consumed within one request, never stored, so that stack space is cheaper
/// than boxing the variant on every served request.
#[allow(clippy::large_enum_variant)]
pub(crate) enum Action {
    /// Send the response to a management API request. Management responses are not recorded.
    Respond(Finished),
    /// Serve a request that is not for the management API: mock it, forward it or proxy it, then
    /// give `pending` back to [`Handler::finish`] together with the response that was sent.
    Serve { reply: Reply, pending: Pending },
}

/// A served request whose outcome has not been reported yet. Its fields are private and
/// [`Handler::finish`] is the only consumer.
#[must_use = "pass it to Handler::finish with the response that was sent, or recordings miss it"]
pub(crate) struct Pending {
    /// The request as the matchers saw it.
    #[cfg(feature = "record")]
    request: HttpMockRequest,
    #[cfg(feature = "record")]
    is_proxied: bool,
}

/// A response that is ready to go out. The only ways to get one are [`Action::Respond`] and
/// [`Handler::finish`], so a driver that returns `Finished` cannot send a served request's
/// response without letting the recordings see it first. A driver that fails to produce a
/// response, because the upstream request failed, answers with its own error response instead
/// and drops the [`Pending`]; such failures are not recorded.
pub(crate) struct Finished(Response<Bytes>);

impl Finished {
    pub(crate) fn into_response(self) -> Response<Bytes> {
        self.0
    }
}

/// How a served request is answered.
pub(crate) enum Reply {
    /// A mocked response, or a 404 when no mock matched, to be sent after `delay` has passed.
    Mock {
        delay: Option<Duration>,
        response: Response<Bytes>,
    },
    /// The request rewritten for the target of a forwarding rule; its response is the reply.
    #[cfg(feature = "proxy")]
    Forward(Request<Bytes>),
    /// The request rewritten for its own origin, as a proxy rule matched; its response is the
    /// reply.
    #[cfg(feature = "proxy")]
    Proxy(Request<Bytes>),
}

enum RoutePath {
    Ping,
    Reset,
    MockCollection,
    SingleMock,
    SingleMockUpToNTimes,
    History,
    Verify,
    #[cfg(feature = "proxy")]
    SingleForwardingRule,
    #[cfg(feature = "proxy")]
    ForwardingRuleCollection,
    #[cfg(feature = "proxy")]
    ProxyRuleCollection,
    #[cfg(feature = "proxy")]
    SingleProxyRule,
    #[cfg(feature = "record")]
    RecordingCollection,
    #[cfg(feature = "record")]
    SingleRecording,
}

/// Routes incoming requests either to the mock server's management API or to the
/// mocking, forwarding and proxying logic.
///
/// The handler is the server's sans-IO core: it takes fully buffered requests in absolute-form
/// and returns an [`Action`] for the transport to carry out. It never sleeps, keeps time or sends
/// requests itself.
pub(crate) struct Handler {
    path_tree: PathTree<RoutePath>,
    state: Arc<state::HttpMockStateManager>,
}

impl Handler {
    pub(crate) fn new(state: Arc<state::HttpMockStateManager>) -> Self {
        let mut path_tree: PathTree<RoutePath> = PathTree::new();
        #[allow(unused_must_use)]
        {
            path_tree.insert("/__httpmock__/ping", RoutePath::Ping);
            path_tree.insert("/__httpmock__/state", RoutePath::Reset);
            path_tree.insert("/__httpmock__/mocks", RoutePath::MockCollection);
            path_tree.insert("/__httpmock__/mocks/:id", RoutePath::SingleMock);
            path_tree.insert("/__httpmock__/mocks/:id/up_to_n_times", RoutePath::SingleMockUpToNTimes);
            path_tree.insert("/__httpmock__/verify", RoutePath::Verify);
            path_tree.insert("/__httpmock__/history", RoutePath::History);

            #[cfg(feature = "proxy")]
            {
                path_tree.insert("/__httpmock__/forwarding_rules", RoutePath::ForwardingRuleCollection);
                path_tree.insert("/__httpmock__/forwarding_rules/:id", RoutePath::SingleForwardingRule);
                path_tree.insert("/__httpmock__/proxy_rules", RoutePath::ProxyRuleCollection);
                path_tree.insert("/__httpmock__/proxy_rules/:id", RoutePath::SingleProxyRule);
            }

            #[cfg(feature = "record")]
            {
                path_tree.insert("/__httpmock__/recordings", RoutePath::RecordingCollection);
                path_tree.insert("/__httpmock__/recordings/:id", RoutePath::SingleRecording);
            }
        }

        Self { path_tree, state }
    }

    /// Decides what to do with a request. The request's URI must be in absolute-form, see
    /// `transport::to_absolute_form_uri`.
    pub(crate) fn handle(&self, req: Request<Bytes>) -> Result<Action, Error> {
        tracing::trace!("Routing incoming request: {:?}", req);

        if let Some((route, params)) = self.path_tree.find(req.uri().path())
            && let Some(response) = self.manage(route, params, &req)
        {
            return response.map(|response| Action::Respond(Finished(response)));
        }

        self.catch_all(req)
    }

    /// Answers a management API request, or returns `None` when the route does not support the
    /// request's method, in which case the request is served like any other.
    fn manage(
        &self,
        route: &RoutePath,
        params: Path<'_, '_>,
        req: &Request<Bytes>,
    ) -> Option<Result<Response<Bytes>, Error>> {
        let response = match (route, req.method()) {
            (RoutePath::Ping, &Method::GET) => self.handle_ping(),
            (RoutePath::Reset, &Method::DELETE) => self.handle_reset(),
            (RoutePath::MockCollection, &Method::POST) => self.handle_add_mock(req),
            (RoutePath::MockCollection, &Method::DELETE) => self.handle_delete_all_mocks(),
            (RoutePath::SingleMock, &Method::GET) => self.handle_read_mock(params),
            (RoutePath::SingleMock, &Method::DELETE) => self.handle_delete_mock(params),
            (RoutePath::SingleMockUpToNTimes, &Method::PUT) => self.handle_set_up_to_n_times(params, req),
            (RoutePath::History, &Method::DELETE) => self.handle_delete_history(),
            (RoutePath::Verify, &Method::POST) => self.handle_verify(req),
            #[cfg(feature = "proxy")]
            (RoutePath::ForwardingRuleCollection, &Method::POST) => self.handle_add_forwarding_rule(req),
            #[cfg(feature = "proxy")]
            (RoutePath::ForwardingRuleCollection, &Method::DELETE) => self.handle_delete_all_forwarding_rules(),
            #[cfg(feature = "proxy")]
            (RoutePath::SingleForwardingRule, &Method::DELETE) => self.handle_delete_forwarding_rule(params),
            #[cfg(feature = "proxy")]
            (RoutePath::ProxyRuleCollection, &Method::POST) => self.handle_add_proxy_rule(req),
            #[cfg(feature = "proxy")]
            (RoutePath::ProxyRuleCollection, &Method::DELETE) => self.handle_delete_all_proxy_rules(),
            #[cfg(feature = "proxy")]
            (RoutePath::SingleProxyRule, &Method::DELETE) => self.handle_delete_proxy_rule(params),
            #[cfg(feature = "record")]
            (RoutePath::RecordingCollection, &Method::POST) => self.handle_add_recording_matcher(req),
            #[cfg(feature = "record")]
            (RoutePath::RecordingCollection, &Method::DELETE) => self.handle_delete_all_recording_matchers(),
            #[cfg(feature = "record")]
            (RoutePath::SingleRecording, &Method::GET) => self.handle_read_recording(params),
            #[cfg(feature = "record")]
            (RoutePath::SingleRecording, &Method::DELETE) => self.handle_delete_recording(params),
            #[cfg(feature = "record")]
            (RoutePath::SingleRecording, &Method::POST) => self.handle_load_recording(req),
            _ => return None,
        };

        Some(response)
    }

    /// Completes an [`Action::Serve`]: records the response for every recording whose rule
    /// matches the request and releases it for sending. `elapsed` is the time the driver took to
    /// produce the response.
    pub(crate) fn finish(
        &self,
        pending: Pending,
        elapsed: Duration,
        response: Response<Bytes>,
    ) -> Result<Finished, Error> {
        #[cfg(feature = "record")]
        self.state
            .record(pending.is_proxied, elapsed, pending.request, &response)?;
        #[cfg(not(feature = "record"))]
        let _ = (pending, elapsed);
        Ok(Finished(response))
    }

    fn handle_ping(&self) -> Result<Response<Bytes>, Error> {
        response::<()>(StatusCode::OK, None)
    }

    fn handle_reset(&self) -> Result<Response<Bytes>, Error> {
        self.state.reset();
        response::<()>(StatusCode::NO_CONTENT, None)
    }

    fn handle_add_mock(&self, req: &Request<Bytes>) -> Result<Response<Bytes>, Error> {
        let definition: MockDefinition = parse_json_body(req)?;
        let active_mock = self.state.add_mock(definition)?;
        response(StatusCode::CREATED, Some(active_mock))
    }

    fn handle_read_mock(&self, params: Path<'_, '_>) -> Result<Response<Bytes>, Error> {
        let active_mock = self.state.read_mock(param("id", params)?)?;
        let status_code = active_mock.as_ref().map_or(StatusCode::NOT_FOUND, |_| StatusCode::OK);
        response(status_code, active_mock)
    }

    fn handle_delete_mock(&self, params: Path<'_, '_>) -> Result<Response<Bytes>, Error> {
        let deleted = self.state.delete_mock(param("id", params)?)?;
        let status_code = if deleted {
            StatusCode::NO_CONTENT
        } else {
            StatusCode::NOT_FOUND
        };
        response::<()>(status_code, None)
    }

    fn handle_set_up_to_n_times(&self, params: Path<'_, '_>, req: &Request<Bytes>) -> Result<Response<Bytes>, Error> {
        let config: UpToNTimesConfig = parse_json_body(req)?;
        let updated = self
            .state
            .set_up_to_n_times(param("id", params)?, config.up_to_n_times)?;
        let status_code = if updated {
            StatusCode::NO_CONTENT
        } else {
            StatusCode::NOT_FOUND
        };
        response::<()>(status_code, None)
    }

    fn handle_delete_all_mocks(&self) -> Result<Response<Bytes>, Error> {
        self.state.delete_all_mocks();
        response::<()>(StatusCode::NO_CONTENT, None)
    }

    fn handle_delete_history(&self) -> Result<Response<Bytes>, Error> {
        self.state.delete_history();
        response::<()>(StatusCode::NO_CONTENT, None)
    }

    fn handle_verify(&self, req: &Request<Bytes>) -> Result<Response<Bytes>, Error> {
        let requirements: RequestRequirements = parse_json_body(req)?;
        let closest_match = self.state.verify(&requirements)?;
        let status_code = closest_match.as_ref().map_or(StatusCode::NOT_FOUND, |_| StatusCode::OK);
        response(status_code, closest_match)
    }

    fn catch_all(&self, req: Request<Bytes>) -> Result<Action, Error> {
        let request: HttpMockRequest = (&req)
            .try_into()
            .map_err(|err: DataError| RequestConversion(err.to_string()))?;

        #[cfg(feature = "proxy")]
        let reply = if let Some(rule) = self.state.find_forward_rule(&request) {
            Reply::Forward(forward_request(rule, req)?)
        } else if let Some(rule) = self.state.find_proxy_rule(&request) {
            Reply::Proxy(proxy_request(rule, req)?)
        } else {
            self.mock_reply(&request)?
        };

        #[cfg(not(feature = "proxy"))]
        let reply = self.mock_reply(&request)?;

        let pending = Pending {
            #[cfg(feature = "record")]
            is_proxied: matches!(reply, Reply::Proxy(_)),
            #[cfg(feature = "record")]
            request,
        };

        Ok(Action::Serve { reply, pending })
    }

    /// Matches the request against the mocks and builds the reply, a 404 when none matched.
    fn mock_reply(&self, req: &HttpMockRequest) -> Result<Reply, Error> {
        let Some(definition) = self.state.serve_mock(req)? else {
            return Ok(Reply::Mock {
                delay: None,
                response: response(
                    StatusCode::NOT_FOUND,
                    Some(ErrorResponse::new(&"Request did not match any route or mock")),
                )?,
            });
        };

        let delay = definition.delay.map(Duration::from_millis);

        // Resolve dynamic vs. static response into HttpMockResponse
        let resp_def: HttpMockResponse = definition
            .respond_with
            .map(|f| f(req))
            .unwrap_or_else(|| HttpMockResponse {
                status: definition.status.or(Some(StatusCode::OK.as_u16())),
                headers: definition.headers,
                body: definition.body,
            });

        let response: Response<Bytes> = resp_def.try_into().map_err(ResponseDataConversion)?;

        Ok(Reply::Mock { delay, response })
    }
}

#[cfg(feature = "proxy")]
impl Handler {
    fn handle_add_forwarding_rule(&self, req: &Request<Bytes>) -> Result<Response<Bytes>, Error> {
        let config: ForwardingRuleConfig = parse_json_body(req)?;
        let active_forwarding_rule = self.state.create_forwarding_rule(config);
        response(StatusCode::CREATED, Some(active_forwarding_rule))
    }

    fn handle_delete_forwarding_rule(&self, params: Path<'_, '_>) -> Result<Response<Bytes>, Error> {
        let deleted = self.state.delete_forwarding_rule(param("id", params)?);
        let status_code = if deleted.is_some() {
            StatusCode::NO_CONTENT
        } else {
            StatusCode::NOT_FOUND
        };
        response::<()>(status_code, None)
    }

    fn handle_delete_all_forwarding_rules(&self) -> Result<Response<Bytes>, Error> {
        self.state.delete_all_forwarding_rules();
        response::<()>(StatusCode::NO_CONTENT, None)
    }

    fn handle_add_proxy_rule(&self, req: &Request<Bytes>) -> Result<Response<Bytes>, Error> {
        let config: ProxyRuleConfig = parse_json_body(req)?;
        let active_proxy_rule = self.state.create_proxy_rule(config);
        response(StatusCode::CREATED, Some(active_proxy_rule))
    }

    fn handle_delete_proxy_rule(&self, params: Path<'_, '_>) -> Result<Response<Bytes>, Error> {
        let deleted = self.state.delete_proxy_rule(param("id", params)?);
        let status_code = if deleted.is_some() {
            StatusCode::NO_CONTENT
        } else {
            StatusCode::NOT_FOUND
        };
        response::<()>(status_code, None)
    }

    fn handle_delete_all_proxy_rules(&self) -> Result<Response<Bytes>, Error> {
        self.state.delete_all_proxy_rules();
        response::<()>(StatusCode::NO_CONTENT, None)
    }
}

/// Rewrites a request for the target of the forwarding rule that matched it.
#[cfg(feature = "proxy")]
fn forward_request(rule: ActiveForwardingRule, req: Request<Bytes>) -> Result<Request<Bytes>, Error> {
    let to_base_uri: Uri = rule.config.target_base_url.parse().unwrap();

    let (mut req_parts, body) = req.into_parts();

    // We need to remove the host header, because it contains the host of this mock server.
    req_parts.headers.remove(http::header::HOST);

    let mut uri_parts = req_parts.uri.into_parts();
    uri_parts.authority = Some(to_base_uri.authority().unwrap().clone());
    uri_parts.scheme = Some(to_base_uri.scheme().cloned().unwrap_or(Scheme::HTTP));
    req_parts.uri = Uri::from_parts(uri_parts).unwrap();

    if !rule.config.request_header.is_empty() {
        for (key, value) in &rule.config.request_header {
            let key =
                http::HeaderName::from_str(key).map_err(|err| InvalidHeader(format!("invalid header key: {err}")))?;

            let value =
                HeaderValue::from_str(value).map_err(|err| InvalidHeader(format!("invalid header value: {err}")))?;

            req_parts.headers.append(key, value);
        }
    }

    // The HTTP client writes the absolute URI in origin-form, with a Host header, as origin
    // servers expect.
    Ok(Request::from_parts(req_parts, body))
}

/// Rewrites a request for its own origin, as a proxy rule matched it.
#[cfg(feature = "proxy")]
fn proxy_request(rule: ActiveProxyRule, mut req: Request<Bytes>) -> Result<Request<Bytes>, Error> {
    if !rule.config.request_header.is_empty() {
        let headers = req.headers_mut();

        for (key, value) in &rule.config.request_header {
            let key =
                http::HeaderName::from_str(key).map_err(|err| InvalidHeader(format!("invalid header key: {err}")))?;

            let value =
                HeaderValue::from_str(value).map_err(|err| InvalidHeader(format!("invalid header value: {err}")))?;

            headers.append(key, value);
        }
    }

    // The HTTP client writes the absolute URI in origin-form, with a Host header, as origin
    // servers expect.
    Ok(req)
}

#[cfg(feature = "record")]
impl Handler {
    fn handle_add_recording_matcher(&self, req: &Request<Bytes>) -> Result<Response<Bytes>, Error> {
        let req_req: RecordingRuleConfig = parse_json_body(req)?;
        let active_recording = self.state.create_recording(req_req);
        response(StatusCode::CREATED, Some(active_recording))
    }

    fn handle_delete_recording(&self, params: Path<'_, '_>) -> Result<Response<Bytes>, Error> {
        let deleted = self.state.delete_recording(param("id", params)?);
        let status_code = if deleted.is_some() {
            StatusCode::NO_CONTENT
        } else {
            StatusCode::NOT_FOUND
        };
        response::<()>(status_code, None)
    }

    fn handle_delete_all_recording_matchers(&self) -> Result<Response<Bytes>, Error> {
        self.state.delete_all_recordings();
        response::<()>(StatusCode::NO_CONTENT, None)
    }

    fn handle_read_recording(&self, params: Path<'_, '_>) -> Result<Response<Bytes>, Error> {
        let rec = self.state.export_recording(param("id", params)?)?;
        let status_code = rec.as_ref().map_or(StatusCode::NOT_FOUND, |_| StatusCode::OK);
        response(status_code, rec)
    }

    fn handle_load_recording(&self, req: &Request<Bytes>) -> Result<Response<Bytes>, Error> {
        let recording_file_content =
            std::str::from_utf8(req.body()).map_err(|err| RequestConversion(err.to_string()))?;

        let rec = self.state.load_mocks_from_recording(recording_file_content)?;
        response(StatusCode::OK, Some(rec))
    }
}

fn param<T>(name: &str, tree_path: Path<'_, '_>) -> Result<T, Error>
where
    T: FromStr,
    T::Err: Debug + Display,
{
    for (n, v) in tree_path.params() {
        if n.eq(name) {
            let parse_result: Result<T, T::Err> = v.parse::<T>();
            let parsed_value = parse_result.map_err(|e| InvalidParamFormat(format!("{e:?}")))?;
            return Ok(parsed_value);
        }
    }

    Err(MissingParam)
}

fn response<T>(status: StatusCode, body: Option<T>) -> Result<Response<Bytes>, Error>
where
    T: Serialize,
{
    let mut builder = Response::builder().status(status);

    if let Some(body_obj) = body {
        builder = builder.header("content-type", "application/json");

        let body_bytes = serde_json::to_vec(&body_obj).map_err(ResponseBodySerialization)?;

        return builder.body(Bytes::from(body_bytes)).map_err(ResponseBodyConstruction);
    }

    builder.body(Bytes::new()).map_err(ResponseBodyConstruction)
}

fn parse_json_body<T>(req: &Request<Bytes>) -> Result<T, Error>
where
    T: DeserializeOwned,
{
    let body: T = serde_json::from_slice(req.body().as_ref()).map_err(RequestBodyDeserialization)?;
    Ok(body)
}

#[cfg(test)]
mod tests {
    use std::{sync::Arc, time::Duration};

    use hyper::{Request, Response, body::Bytes};

    use super::{Action, Handler, Reply};
    use crate::{
        common::data::{MockDefinition, MockServerHttpResponse, RequestRequirements},
        server::{RequestMetadata, state::HttpMockStateManager},
    };

    fn handler() -> Handler {
        Handler::new(Arc::new(HttpMockStateManager::default()))
    }

    fn request(method: &str, uri: &str) -> Request<Bytes> {
        let mut req = Request::builder().method(method).uri(uri).body(Bytes::new()).unwrap();
        req.extensions_mut().insert(RequestMetadata::new("http"));
        req
    }

    fn mock(path: &str, status: u16, delay: Option<u64>) -> MockDefinition {
        MockDefinition {
            request: RequestRequirements {
                path: Some(path.to_string()),
                ..Default::default()
            },
            response: MockServerHttpResponse {
                status: Some(status),
                delay,
                ..Default::default()
            },
            up_to_n_times: None,
        }
    }

    fn expect_mock_reply(action: Action) -> (Option<Duration>, Response<Bytes>) {
        match action {
            Action::Serve { reply, .. } => match reply {
                Reply::Mock { delay, response } => (delay, response),
                #[cfg(feature = "proxy")]
                _ => panic!("expected a mock reply"),
            },
            Action::Respond(_) => panic!("expected a served request, got a management response"),
        }
    }

    #[test]
    fn management_route_is_answered_directly() {
        let Action::Respond(finished) = handler()
            .handle(request("GET", "http://localhost/__httpmock__/ping"))
            .unwrap()
        else {
            panic!("expected a management response");
        };

        assert_eq!(finished.into_response().status(), 200);
    }

    #[test]
    fn management_path_with_other_method_falls_through_to_mocks() {
        let (_, response) = expect_mock_reply(
            handler()
                .handle(request("POST", "http://localhost/__httpmock__/ping"))
                .unwrap(),
        );

        assert_eq!(response.status(), 404);
    }

    #[test]
    fn unmatched_request_is_a_404_without_delay() {
        let (delay, response) = expect_mock_reply(handler().handle(request("GET", "http://localhost/nope")).unwrap());

        assert_eq!(delay, None);
        assert_eq!(response.status(), 404);
    }

    #[test]
    fn matched_mock_is_served_with_its_delay() {
        let handler = handler();
        handler.state.add_mock(mock("/hello", 202, Some(150))).unwrap();

        let (delay, response) = expect_mock_reply(handler.handle(request("GET", "http://localhost/hello")).unwrap());

        assert_eq!(delay, Some(Duration::from_millis(150)));
        assert_eq!(response.status(), 202);
    }

    #[test]
    fn serving_a_mock_counts_the_call_and_appends_the_request_to_the_history() {
        let handler = handler();
        let id = handler.state.add_mock(mock("/hello", 200, None)).unwrap().id;

        handler.handle(request("GET", "http://localhost/hello")).unwrap();

        assert_eq!(handler.state.read_mock(id).unwrap().unwrap().call_counter, 1);
        let unmatched = RequestRequirements {
            path: Some("/other".to_string()),
            ..Default::default()
        };
        let closest = handler
            .state
            .verify(&unmatched)
            .unwrap()
            .expect("the history holds the request");
        assert_eq!(closest.request.uri().path(), "/hello");
    }

    #[test]
    fn exhausted_mock_falls_through_to_404() {
        let handler = handler();
        let mut definition = mock("/once", 200, None);
        definition.up_to_n_times = Some(1);
        handler.state.add_mock(definition).unwrap();

        let first = expect_mock_reply(handler.handle(request("GET", "http://localhost/once")).unwrap());
        let second = expect_mock_reply(handler.handle(request("GET", "http://localhost/once")).unwrap());

        assert_eq!(first.1.status(), 200);
        assert_eq!(second.1.status(), 404);
    }

    #[cfg(feature = "proxy")]
    mod proxy {
        use super::*;
        use crate::common::data::{ForwardingRuleConfig, ProxyRuleConfig};

        fn upstream_request(action: Action) -> (bool, Request<Bytes>) {
            match action {
                Action::Serve { reply, .. } => match reply {
                    Reply::Forward(req) => (false, req),
                    Reply::Proxy(req) => (true, req),
                    Reply::Mock { .. } => panic!("expected an upstream request"),
                },
                Action::Respond(_) => panic!("expected a served request"),
            }
        }

        #[test]
        fn forwarding_rule_rewrites_the_request_for_its_target() {
            let handler = handler();
            handler.state.create_forwarding_rule(ForwardingRuleConfig {
                target_base_url: "https://upstream.test:8443".to_string(),
                request_header: vec![("x-forwarded-by".to_string(), "httpmock".to_string())],
                ..Default::default()
            });

            let (proxied, upstream) =
                upstream_request(handler.handle(request("GET", "http://localhost:1234/api?x=1")).unwrap());

            assert!(!proxied);
            assert_eq!(upstream.uri(), "https://upstream.test:8443/api?x=1");
            assert_eq!(upstream.headers()["x-forwarded-by"], "httpmock");
        }

        #[test]
        fn forwarding_target_without_scheme_gets_http() {
            let handler = handler();
            handler.state.create_forwarding_rule(ForwardingRuleConfig {
                target_base_url: "upstream.test:8080".to_string(),
                ..Default::default()
            });

            let (_, upstream) = upstream_request(handler.handle(request("GET", "https://localhost:1234/api")).unwrap());

            assert_eq!(upstream.uri(), "http://upstream.test:8080/api");
        }

        #[test]
        fn proxy_rule_keeps_the_origin_and_adds_headers() {
            let handler = handler();
            handler.state.create_proxy_rule(ProxyRuleConfig {
                request_header: vec![("x-added".to_string(), "1".to_string())],
                ..Default::default()
            });

            let (proxied, upstream) =
                upstream_request(handler.handle(request("GET", "http://origin.test/path")).unwrap());

            assert!(proxied);
            assert_eq!(upstream.uri(), "http://origin.test/path");
            assert_eq!(upstream.headers()["x-added"], "1");
        }

        #[test]
        fn forwarding_rule_takes_precedence_over_proxy_rule() {
            let handler = handler();
            handler.state.create_proxy_rule(ProxyRuleConfig::default());
            handler.state.create_forwarding_rule(ForwardingRuleConfig {
                target_base_url: "http://upstream.test".to_string(),
                ..Default::default()
            });

            let (proxied, _) = upstream_request(handler.handle(request("GET", "http://origin.test/")).unwrap());

            assert!(!proxied);
        }

        #[test]
        fn invalid_rule_header_is_an_error() {
            let handler = handler();
            handler.state.create_proxy_rule(ProxyRuleConfig {
                request_header: vec![("not a header name".to_string(), "1".to_string())],
                ..Default::default()
            });

            let result = handler.handle(request("GET", "http://origin.test/path"));

            assert!(matches!(result, Err(super::super::Error::InvalidHeader(_))));
        }
    }

    #[cfg(feature = "record")]
    mod record {
        use super::*;
        use crate::common::data::RecordingRuleConfig;

        #[test]
        fn recorded_outcome_becomes_a_mock_with_the_measured_delay() {
            let handler = handler();
            let recording = handler.state.create_recording(RecordingRuleConfig {
                request_requirements: RequestRequirements::default(),
                record_headers: Vec::new(),
                record_response_delays: true,
                record_response_headers: Vec::new(),
            });
            handler.state.add_mock(mock("/recorded", 201, None)).unwrap();

            let Action::Serve { reply, pending } = handler.handle(request("GET", "http://localhost/recorded")).unwrap()
            else {
                panic!("expected a served request");
            };
            let Reply::Mock { response, .. } = reply else {
                panic!("expected a mock reply");
            };
            let finished = handler.finish(pending, Duration::from_millis(40), response).unwrap();
            assert_eq!(finished.into_response().status(), 201);

            let yaml = handler.state.export_recording(recording.id).unwrap().unwrap();
            let yaml = std::str::from_utf8(&yaml).unwrap();
            assert!(yaml.contains("path: /recorded"), "{yaml}");
            assert!(yaml.contains("status: 201"), "{yaml}");
            assert!(yaml.contains("delay: 40"), "{yaml}");
        }

        #[test]
        fn recorded_response_keeps_only_listed_headers() {
            let handler = handler();
            let recording = handler.state.create_recording(RecordingRuleConfig {
                request_requirements: RequestRequirements::default(),
                record_headers: Vec::new(),
                record_response_delays: false,
                record_response_headers: vec!["Content-Type".to_string()],
            });
            let mut mock = mock("/recorded", 200, None);
            mock.response.headers = Some(vec![
                ("content-type".to_string(), "text/plain".to_string()),
                ("set-cookie".to_string(), "session=secret".to_string()),
            ]);
            handler.state.add_mock(mock).unwrap();

            let Action::Serve { reply, pending } = handler.handle(request("GET", "http://localhost/recorded")).unwrap()
            else {
                panic!("expected a served request");
            };
            let Reply::Mock { response, .. } = reply else {
                panic!("expected a mock reply");
            };
            handler.finish(pending, Duration::ZERO, response).unwrap();

            let yaml = handler.state.export_recording(recording.id).unwrap().unwrap();
            let yaml = std::str::from_utf8(&yaml).unwrap();
            assert!(yaml.contains("text/plain"), "{yaml}");
            assert!(!yaml.contains("set-cookie"), "{yaml}");
        }

        #[test]
        fn outcome_is_not_recorded_when_no_recording_rule_matches() {
            let handler = handler();
            let recording = handler.state.create_recording(RecordingRuleConfig {
                request_requirements: RequestRequirements {
                    path: Some("/other".to_string()),
                    ..Default::default()
                },
                record_headers: Vec::new(),
                record_response_delays: false,
                record_response_headers: Vec::new(),
            });

            let Action::Serve { reply, pending } =
                handler.handle(request("GET", "http://localhost/unrecorded")).unwrap()
            else {
                panic!("expected a served request");
            };
            let Reply::Mock { response, .. } = reply else {
                panic!("expected a mock reply");
            };
            handler.finish(pending, Duration::ZERO, response).unwrap();

            let yaml = handler.state.export_recording(recording.id).unwrap().unwrap();
            assert!(yaml.is_empty(), "{}", std::str::from_utf8(&yaml).unwrap());
        }
    }
}
