use std::{
    convert::TryInto,
    fmt::{Debug, Display},
    str::FromStr,
    sync::Arc,
};

use http::{HeaderValue, StatusCode, Uri};
use hyper::{Method, Request, Response, body::Bytes};
use path_tree::{Path, PathTree};
use serde::{Serialize, de::DeserializeOwned};
use thiserror::Error;
#[cfg(feature = "record")]
use tokio::time::Instant;

#[cfg(feature = "record")]
use crate::common::data::RecordingRuleConfig;
#[cfg(any(feature = "remote", feature = "proxy"))]
use crate::common::http::Error as HttpClientError;
#[cfg(feature = "proxy")]
use crate::common::http::HttpClient;
#[cfg(feature = "proxy")]
use crate::{common::data::ActiveProxyRule, server::state::ForwardingRule};
use crate::{
    common::{
        data,
        data::{
            Error as DataError, ErrorResponse, ForwardingRuleConfig, MockDefinition, ProxyRuleConfig,
            RequestRequirements,
        },
        runtime,
    },
    prelude::{HttpMockRequest, HttpMockResponse},
    server::{
        handler::Error::{
            InvalidHeader, InvalidParamFormat, MissingParam, RequestBodyDeserialization, RequestConversion,
            ResponseBodyConstruction, ResponseBodySerialization, ResponseDataConversion,
        },
        state,
    },
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
    #[cfg(any(feature = "remote", feature = "proxy"))]
    #[error("failed to send HTTP request: {0}")]
    HttpClient(#[from] HttpClientError),
    #[error("invalid header: {0}")]
    InvalidHeader(String),
}

enum RoutePath {
    Ping,
    Reset,
    MockCollection,
    SingleMock,
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
pub(crate) struct Handler {
    path_tree: PathTree<RoutePath>,
    state: Arc<state::Manager>,
    #[cfg(feature = "proxy")]
    http_client: Arc<dyn HttpClient + Send + Sync + 'static>,
}

impl Handler {
    pub(crate) fn new(
        state: Arc<state::Manager>,
        #[cfg(feature = "proxy")] http_client: Arc<dyn HttpClient + Send + Sync + 'static>,
    ) -> Self {
        let mut path_tree: PathTree<RoutePath> = PathTree::new();
        #[allow(unused_must_use)]
        {
            path_tree.insert("/__httpmock__/ping", RoutePath::Ping);
            path_tree.insert("/__httpmock__/state", RoutePath::Reset);
            path_tree.insert("/__httpmock__/mocks", RoutePath::MockCollection);
            path_tree.insert("/__httpmock__/mocks/:id", RoutePath::SingleMock);
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

        Self {
            path_tree,
            state,
            #[cfg(feature = "proxy")]
            http_client,
        }
    }

    pub(crate) async fn handle(&self, req: Request<Bytes>) -> Result<Response<Bytes>, Error> {
        tracing::trace!("Routing incoming request: {:?}", req);

        let method = req.method().clone();
        let path = req.uri().path().to_string();

        if let Some((matched_path, params)) = self.path_tree.find(&path) {
            match matched_path {
                RoutePath::Ping => {
                    if method == Method::GET {
                        return self.handle_ping();
                    }
                }
                RoutePath::Reset => {
                    if method == Method::DELETE {
                        return self.handle_reset();
                    }
                }
                RoutePath::SingleMock => match method {
                    Method::GET => return self.handle_read_mock(params),
                    Method::DELETE => return self.handle_delete_mock(params),
                    _ => {}
                },
                RoutePath::MockCollection => match method {
                    Method::POST => return self.handle_add_mock(req),
                    Method::DELETE => return self.handle_delete_all_mocks(),
                    _ => {}
                },
                RoutePath::History => {
                    if method == Method::DELETE {
                        return self.handle_delete_history();
                    }
                }
                RoutePath::Verify => {
                    if method == Method::POST {
                        return self.handle_verify(req);
                    }
                }
                #[cfg(feature = "proxy")]
                RoutePath::ForwardingRuleCollection => match method {
                    Method::POST => return self.handle_add_forwarding_rule(req),
                    Method::DELETE => return self.handle_delete_all_forwarding_rules(),
                    _ => {}
                },
                #[cfg(feature = "proxy")]
                RoutePath::SingleForwardingRule => {
                    if method == Method::DELETE {
                        return self.handle_delete_forwarding_rule(params);
                    }
                }
                #[cfg(feature = "proxy")]
                RoutePath::ProxyRuleCollection => match method {
                    Method::POST => return self.handle_add_proxy_rule(req),
                    Method::DELETE => return self.handle_delete_all_proxy_rules(),
                    _ => {}
                },
                #[cfg(feature = "proxy")]
                RoutePath::SingleProxyRule => {
                    if method == Method::DELETE {
                        return self.handle_delete_proxy_rule(params);
                    }
                }
                #[cfg(feature = "record")]
                RoutePath::RecordingCollection => match method {
                    Method::POST => return self.handle_add_recording_matcher(req),
                    Method::DELETE => return self.handle_delete_all_recording_matchers(),
                    _ => {}
                },
                #[cfg(feature = "record")]
                RoutePath::SingleRecording => match method {
                    Method::GET => return self.handle_read_recording(params),
                    Method::DELETE => return self.handle_delete_recording(params),
                    Method::POST => return self.handle_load_recording(req),
                    _ => {}
                },
            }
        }

        return self.catch_all(req).await;
    }

    fn handle_ping(&self) -> Result<Response<Bytes>, Error> {
        response::<()>(StatusCode::OK, None)
    }

    fn handle_reset(&self) -> Result<Response<Bytes>, Error> {
        self.state.reset();
        response::<()>(StatusCode::NO_CONTENT, None)
    }

    fn handle_add_mock(&self, req: Request<Bytes>) -> Result<Response<Bytes>, Error> {
        let definition: MockDefinition = parse_json_body(req)?;
        let active_mock = self.state.add_mock(definition, false)?;
        response(StatusCode::CREATED, Some(active_mock))
    }

    fn handle_read_mock(&self, params: Path) -> Result<Response<Bytes>, Error> {
        let active_mock = self.state.read_mock(param("id", params)?)?;
        let status_code = active_mock.as_ref().map_or(StatusCode::NOT_FOUND, |_| StatusCode::OK);
        response(status_code, active_mock)
    }

    fn handle_delete_mock(&self, params: Path) -> Result<Response<Bytes>, Error> {
        let deleted = self.state.delete_mock(param("id", params)?)?;
        let status_code = if deleted {
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

    fn handle_verify(&self, req: Request<Bytes>) -> Result<Response<Bytes>, Error> {
        let requirements: RequestRequirements = parse_json_body(req)?;
        let closest_match = self.state.verify(&requirements)?;
        let status_code = closest_match.as_ref().map_or(StatusCode::NOT_FOUND, |_| StatusCode::OK);
        response(status_code, closest_match)
    }

    fn handle_add_forwarding_rule(&self, req: Request<Bytes>) -> Result<Response<Bytes>, Error> {
        let config: ForwardingRuleConfig = parse_json_body(req)?;
        let active_forwarding_rule = self.state.create_forwarding_rule(config)?;
        response(StatusCode::CREATED, Some(active_forwarding_rule))
    }

    fn handle_delete_forwarding_rule(&self, params: Path) -> Result<Response<Bytes>, Error> {
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

    fn handle_add_proxy_rule(&self, req: Request<Bytes>) -> Result<Response<Bytes>, Error> {
        let config: ProxyRuleConfig = parse_json_body(req)?;
        let active_proxy_rule = self.state.create_proxy_rule(config);
        response(StatusCode::CREATED, Some(active_proxy_rule))
    }

    fn handle_delete_proxy_rule(&self, params: Path) -> Result<Response<Bytes>, Error> {
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

    #[cfg(feature = "record")]
    fn handle_add_recording_matcher(&self, req: Request<Bytes>) -> Result<Response<Bytes>, Error> {
        let req_req: RecordingRuleConfig = parse_json_body(req)?;
        let active_recording = self.state.create_recording(req_req);
        response(StatusCode::CREATED, Some(active_recording))
    }

    #[cfg(feature = "record")]
    fn handle_delete_recording(&self, params: Path) -> Result<Response<Bytes>, Error> {
        let deleted = self.state.delete_proxy_rule(param("id", params)?);
        let status_code = if deleted.is_some() {
            StatusCode::NO_CONTENT
        } else {
            StatusCode::NOT_FOUND
        };
        response::<()>(status_code, None)
    }

    #[cfg(feature = "record")]
    fn handle_delete_all_recording_matchers(&self) -> Result<Response<Bytes>, Error> {
        self.state.delete_all_recordings();
        response::<()>(StatusCode::NO_CONTENT, None)
    }

    #[cfg(feature = "record")]
    fn handle_read_recording(&self, params: Path) -> Result<Response<Bytes>, Error> {
        let rec = self.state.export_recording(param("id", params)?)?;
        let status_code = rec.as_ref().map_or(StatusCode::NOT_FOUND, |_| StatusCode::OK);
        response(status_code, rec)
    }

    #[cfg(feature = "record")]
    fn handle_load_recording(&self, req: Request<Bytes>) -> Result<Response<Bytes>, Error> {
        let recording_file_content =
            std::str::from_utf8(req.body()).map_err(|err| RequestConversion(err.to_string()))?;

        let rec = self.state.load_mocks_from_recording(recording_file_content)?;
        response(StatusCode::OK, Some(rec))
    }

    async fn catch_all(&self, req: Request<Bytes>) -> Result<Response<Bytes>, Error> {
        let internal_request: HttpMockRequest = (&req)
            .try_into()
            .map_err(|err: DataError| RequestConversion(err.to_string()))?;

        #[cfg(feature = "record")]
        let start = Instant::now();

        #[cfg(feature = "proxy")]
        let (res, is_proxied) = if let Some(rule) = self.state.find_forward_rule(&internal_request)? {
            (self.forward(rule, req).await?, false)
        } else if let Some(rule) = self.state.find_proxy_rule(&internal_request)? {
            (self.proxy(rule, req).await?, true)
        } else {
            (self.serve_mock(&internal_request).await?, false)
        };

        #[cfg(not(feature = "proxy"))]
        let (res, is_proxied) = (self.serve_mock(&internal_request).await?, false);

        #[cfg(feature = "record")]
        self.state.record(is_proxied, start.elapsed(), internal_request, &res)?;

        Ok(res)
    }

    #[cfg(feature = "proxy")]
    async fn forward(&self, rule: ForwardingRule, req: Request<Bytes>) -> Result<Response<Bytes>, Error> {
        let state::ForwardingRule {
            active: _,
            target,
            request_headers,
        } = rule;
        let (target_scheme_name, target_scheme, target_authority) = target.into_parts();

        let (mut req_parts, body) = req.into_parts();

        // The forwarding target replaces the mock server host.
        req_parts.headers.remove(http::header::HOST);

        let mut uri_parts = req_parts.uri.into_parts();
        uri_parts.authority = Some(target_authority);
        uri_parts.scheme = Some(target_scheme);
        req_parts.uri = Uri::from_parts(uri_parts).map_err(|err| RequestConversion(err.to_string()))?;

        // The client uses this scheme when reconstructing the absolute upstream URI.
        req_parts
            .extensions
            .insert(crate::server::RequestMetadata::new(target_scheme_name));

        req_parts.headers.extend(request_headers);

        let req = Request::from_parts(req_parts, body);
        // Origin servers receive the path and query in the request target and the authority in `Host`.
        let req = to_origin_form(req)?;
        Ok(self.http_client.send(req).await?)
    }

    #[cfg(feature = "proxy")]
    async fn proxy(&self, rule: ActiveProxyRule, mut req: Request<Bytes>) -> Result<Response<Bytes>, Error> {
        if !rule.config.request_header.is_empty() {
            let headers = req.headers_mut();

            for (key, value) in &rule.config.request_header {
                let key = http::HeaderName::from_str(key)
                    .map_err(|err| InvalidHeader(format!("invalid header key: {}", err)))?;

                let value = HeaderValue::from_str(value)
                    .map_err(|err| InvalidHeader(format!("invalid header value: {}", err)))?;

                headers.append(key, value);
            }
        }

        // Requests are normalized to absolute-form inside this server for internal uniformity
        // (matchers/recorders can read scheme/host/port from req.uri()). Before talking to an
        // upstream origin server we MUST convert to origin-form (path + query only) and provide
        // the authority via the Host header, as expected by HTTP/1.1 and HTTP/2 origin servers.
        let req = to_origin_form(req)?;
        Ok(self.http_client.send(req).await?)
    }

    async fn serve_mock(&self, req: &HttpMockRequest) -> Result<http::Response<bytes::Bytes>, Error> {
        let Some(definition) = self.state.serve_mock(req)? else {
            return response(
                http::StatusCode::NOT_FOUND,
                Some(ErrorResponse::new(&"Request did not match any route or mock")),
            );
        };

        if let Some(duration) = definition.delay {
            runtime::sleep(std::time::Duration::from_millis(duration)).await;
        }

        // Resolve dynamic vs. static response into HttpMockResponse
        let resp_def: HttpMockResponse = definition
            .respond_with
            .map(|f| f(req))
            .unwrap_or_else(|| HttpMockResponse {
                status: definition.status.or(Some(StatusCode::OK.as_u16())),
                headers: definition.headers,
                body: definition.body,
            });

        // Convert via your TryFrom<HttpMockResponse> impl
        let http_resp: http::Response<bytes::Bytes> = resp_def.try_into().map_err(ResponseDataConversion)?;

        Ok(http_resp)
    }
}

fn param<T>(name: &str, tree_path: Path) -> Result<T, Error>
where
    T: FromStr,
    T::Err: Debug + Display,
{
    for (n, v) in tree_path.params() {
        if n.eq(name) {
            let parse_result: Result<T, T::Err> = v.parse::<T>();
            let parsed_value = parse_result.map_err(|e| InvalidParamFormat(format!("{:?}", e)))?;
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

fn parse_json_body<T>(req: Request<Bytes>) -> Result<T, Error>
where
    T: DeserializeOwned,
{
    let body: T = serde_json::from_slice(req.body().as_ref()).map_err(RequestBodyDeserialization)?;
    Ok(body)
}

/// Convert an absolute-form request URI into origin-form prior to dispatching upstream.
///
/// Rationale:
/// - The server normalizes inbound requests to absolute-form (`scheme://authority/path?query`) so
///   matchers and recorders can read scheme/host/port directly from `req.uri()`.
/// - Upstream origin servers typically expect origin-form on the wire (path-and-query only), with
///   the `Host` header carrying the authority; absolute-form is mainly used by proxies.
/// - Consequently, if `req.uri()` has both `scheme` and `authority`, we treat it as absolute-form,
///   set `Host` to that authority, and strip scheme/authority from the URI to yield origin-form.
/// - Requests lacking either part are already in origin- or asterisk-form and are left untouched.
///   CONNECT (authority-form) is handled separately.
pub fn to_origin_form(mut req: Request<Bytes>) -> Result<Request<Bytes>, Error> {
    let uri = req.uri().clone();

    if uri.scheme().is_some() && uri.authority().is_some() {
        // Ensure Host header matches the authority
        if let Some(auth) = uri.authority() {
            let host_val = HeaderValue::from_str(auth.as_str())
                .map_err(|err| InvalidHeader(format!("invalid header value: {err}")))?;
            req.headers_mut().insert(http::header::HOST, host_val);
        }

        // Set path-and-query only (origin-form)
        let path_and_query = uri.path_and_query().map(|pq| pq.as_str()).unwrap_or("/");
        let new_uri = Uri::builder()
            .path_and_query(path_and_query)
            .build()
            .map_err(|e| RequestConversion(e.to_string()))?;
        *req.uri_mut() = new_uri;
    }

    Ok(req)
}

#[cfg(all(test, feature = "proxy"))]
mod tests {
    use super::*;
    use crate::common::http::HttpMockHttpClient;

    #[test]
    fn valid_forwarding_rule_can_be_added() {
        let handler = Handler::new(
            Arc::new(state::Manager::default()),
            Arc::new(HttpMockHttpClient::new(None)),
        );
        let config = ForwardingRuleConfig {
            target_base_url: "http://example.com".to_string(),
            ..Default::default()
        };
        let request = Request::post("/__httpmock__/forwarding_rules")
            .body(Bytes::from(serde_json::to_vec(&config).unwrap()))
            .unwrap();

        let response = handler.handle_add_forwarding_rule(request).unwrap();

        assert_eq!(response.status(), StatusCode::CREATED);
    }
}
