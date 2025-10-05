use crate::common::data::{
    ActiveForwardingRule, ActiveProxyRule, Error as DataError, ErrorResponse, MockDefinition,
    RequestRequirements,
};

use crate::{
    common::runtime,
    server::{
        handler::Error::{
            InvalidHeader, ParamError, ParamFormatError, RequestBodyDeserializeError,
            RequestConversionError, ResponseBodyConversionError, ResponseBodySerializeError,
        },
        state,
        state::StateManager,
    },
};
use std::convert::TryInto;

#[cfg(any(feature = "remote", feature = "proxy"))]
use crate::common::http::{Error as HttpClientError, HttpClient};

use crate::common::data::{ForwardingRuleConfig, ProxyRuleConfig, RecordingRuleConfig};

use crate::prelude::HttpMockRequest;
use async_trait::async_trait;
use http::{HeaderMap, HeaderName, HeaderValue, StatusCode, Uri};
use http_body_util::BodyExt;
use hyper::{body::Bytes, Method, Request, Response};
use path_tree::{Path, PathTree};
use serde::{de::DeserializeOwned, Serialize};
use std::{
    fmt::{Debug, Display},
    str::FromStr,
    sync::Arc,
    thread,
    time::Duration,
};
use thiserror::Error;
use tokio::time::Instant;

#[derive(Error, Debug)]
pub enum Error {
    #[error("cannot parse regex: {0}")]
    RegexError(#[from] regex::Error),
    #[error("cannot deserialize request body: {0}")]
    RequestBodyDeserializeError(serde_json::Error),
    #[error("cannot process request body: {0}")]
    RequestBodyError(String),
    #[error("cannot serialize response body: {0}")]
    ResponseBodySerializeError(serde_json::Error),
    #[error("cannot convert response body: {0}")]
    ResponseBodyConversionError(http::Error),
    #[error("expected URL parameters not found")]
    ParamError,
    #[error("URL parameter format is invalid: {0}")]
    ParamFormatError(String),
    #[error("cannot modify state: {0}")]
    StateManagerError(#[from] state::Error),
    #[error("invalid status code: {0}")]
    InvalidStatusCode(#[from] http::status::InvalidStatusCode),
    #[error("cannot convert request to internal data structure: {0}")]
    RequestConversionError(String),
    #[cfg(any(feature = "remote", feature = "proxy"))]
    #[error("failed to send HTTP request: {0}")]
    HttpClientError(#[from] HttpClientError),
    #[error("invalid header: {0}")]
    InvalidHeader(String),
    #[error("unknown error")]
    Unknown,
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

#[async_trait]
pub(crate) trait Handler {
    async fn handle(&self, req: Request<Bytes>) -> Result<Response<Bytes>, Error>;
}

pub struct HttpMockHandler<S>
where
    S: StateManager + Send + Sync + 'static,
{
    path_tree: PathTree<RoutePath>,
    state: Arc<S>,
    #[cfg(feature = "proxy")]
    http_client: Arc<dyn HttpClient + Send + Sync + 'static>,
}

#[async_trait]
impl<H> Handler for HttpMockHandler<H>
where
    H: StateManager + Send + Sync + 'static,
{
    async fn handle(&self, req: Request<Bytes>) -> Result<Response<Bytes>, Error> {
        tracing::trace!("Routing incoming request: {:?}", req);

        let method = req.method().clone();
        let path = req.uri().path().to_string();

        if let Some((matched_path, params)) = self.path_tree.find(&path) {
            match matched_path {
                RoutePath::Ping => match method {
                    Method::GET => return self.handle_ping(),
                    _ => {}
                },
                RoutePath::Reset => match method {
                    Method::DELETE => return self.handle_reset(),
                    _ => {}
                },
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
                RoutePath::History => match method {
                    Method::DELETE => return self.handle_delete_history(),
                    _ => {}
                },
                RoutePath::Verify => match method {
                    Method::POST => return self.handle_verify(req),
                    _ => {}
                },
                #[cfg(feature = "proxy")]
                RoutePath::ForwardingRuleCollection => match method {
                    Method::POST => return self.handle_add_forwarding_rule(req),
                    Method::DELETE => return self.handle_delete_all_forwarding_rules(),
                    _ => {}
                },
                #[cfg(feature = "proxy")]
                RoutePath::SingleForwardingRule => match method {
                    Method::DELETE => return self.handle_delete_forwarding_rule(params),
                    _ => {}
                },
                #[cfg(feature = "proxy")]
                RoutePath::ProxyRuleCollection => match method {
                    Method::POST => return self.handle_add_proxy_rule(req),
                    Method::DELETE => return self.handle_delete_all_proxy_rules(),
                    _ => {}
                },
                #[cfg(feature = "proxy")]
                RoutePath::SingleProxyRule => match method {
                    Method::DELETE => return self.handle_delete_proxy_rule(params),
                    _ => {}
                },
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
}

impl<H> HttpMockHandler<H>
where
    H: StateManager + Send + Sync + 'static,
{
    pub fn new(
        state: Arc<H>,
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
                path_tree.insert(
                    "/__httpmock__/forwarding_rules",
                    RoutePath::ForwardingRuleCollection,
                );
                path_tree.insert(
                    "/__httpmock__/forwarding_rules/:id",
                    RoutePath::SingleForwardingRule,
                );
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

    fn handle_ping(&self) -> Result<Response<Bytes>, Error> {
        return response::<()>(StatusCode::OK, None);
    }

    fn handle_reset(&self) -> Result<Response<Bytes>, Error> {
        self.state.reset();
        return response::<()>(StatusCode::NO_CONTENT, None);
    }

    fn handle_add_mock(&self, req: Request<Bytes>) -> Result<Response<Bytes>, Error> {
        let definition: MockDefinition = parse_json_body(req)?;
        let active_mock = self.state.add_mock(definition, false)?;
        return response(StatusCode::CREATED, Some(active_mock));
    }

    fn handle_read_mock(&self, params: Path) -> Result<Response<Bytes>, Error> {
        let active_mock = self.state.read_mock(param("id", params)?)?;
        let status_code = active_mock
            .as_ref()
            .map_or(StatusCode::NOT_FOUND, |_| StatusCode::OK);
        return response(status_code, active_mock);
    }

    fn handle_delete_mock(&self, params: Path) -> Result<Response<Bytes>, Error> {
        let deleted = self.state.delete_mock(param("id", params)?)?;
        let status_code = if deleted {
            StatusCode::NO_CONTENT
        } else {
            StatusCode::NOT_FOUND
        };
        return response::<()>(status_code, None);
    }

    fn handle_delete_all_mocks(&self) -> Result<Response<Bytes>, Error> {
        self.state.delete_all_mocks();
        return response::<()>(StatusCode::NO_CONTENT, None);
    }

    fn handle_delete_history(&self) -> Result<Response<Bytes>, Error> {
        self.state.delete_history();
        return response::<()>(StatusCode::NO_CONTENT, None);
    }

    fn handle_verify(&self, req: Request<Bytes>) -> Result<Response<Bytes>, Error> {
        let requirements: RequestRequirements = parse_json_body(req)?;
        let closest_match = self.state.verify(&requirements)?;
        let status_code = closest_match
            .as_ref()
            .map_or(StatusCode::NOT_FOUND, |_| StatusCode::OK);
        return response(status_code, closest_match);
    }

    fn handle_add_forwarding_rule(&self, req: Request<Bytes>) -> Result<Response<Bytes>, Error> {
        let config: ForwardingRuleConfig = parse_json_body(req)?;
        let active_forwarding_rule = self.state.create_forwarding_rule(config);
        return response(StatusCode::CREATED, Some(active_forwarding_rule));
    }

    fn handle_delete_forwarding_rule(&self, params: Path) -> Result<Response<Bytes>, Error> {
        let deleted = self.state.delete_forwarding_rule(param("id", params)?);
        let status_code = if deleted.is_some() {
            StatusCode::NO_CONTENT
        } else {
            StatusCode::NOT_FOUND
        };
        return response::<()>(status_code, None);
    }

    fn handle_delete_all_forwarding_rules(&self) -> Result<Response<Bytes>, Error> {
        self.state.delete_all_forwarding_rules();
        return response::<()>(StatusCode::NO_CONTENT, None);
    }

    fn handle_add_proxy_rule(&self, req: Request<Bytes>) -> Result<Response<Bytes>, Error> {
        let config: ProxyRuleConfig = parse_json_body(req)?;
        let active_proxy_rule = self.state.create_proxy_rule(config);
        return response(StatusCode::CREATED, Some(active_proxy_rule));
    }

    fn handle_delete_proxy_rule(&self, params: Path) -> Result<Response<Bytes>, Error> {
        let deleted = self.state.delete_proxy_rule(param("id", params)?);
        let status_code = if deleted.is_some() {
            StatusCode::NO_CONTENT
        } else {
            StatusCode::NOT_FOUND
        };
        return response::<()>(status_code, None);
    }

    fn handle_delete_all_proxy_rules(&self) -> Result<Response<Bytes>, Error> {
        self.state.delete_all_proxy_rules();
        return response::<()>(StatusCode::NO_CONTENT, None);
    }

    #[cfg(feature = "record")]
    fn handle_add_recording_matcher(&self, req: Request<Bytes>) -> Result<Response<Bytes>, Error> {
        let req_req: RecordingRuleConfig = parse_json_body(req)?;
        let active_recording = self.state.create_recording(req_req);
        return response(StatusCode::CREATED, Some(active_recording));
    }

    #[cfg(feature = "record")]
    fn handle_delete_recording(&self, params: Path) -> Result<Response<Bytes>, Error> {
        let deleted = self.state.delete_proxy_rule(param("id", params)?);
        let status_code = if deleted.is_some() {
            StatusCode::NO_CONTENT
        } else {
            StatusCode::NOT_FOUND
        };
        return response::<()>(status_code, None);
    }

    #[cfg(feature = "record")]
    fn handle_delete_all_recording_matchers(&self) -> Result<Response<Bytes>, Error> {
        self.state.delete_all_recordings();
        return response::<()>(StatusCode::NO_CONTENT, None);
    }

    #[cfg(feature = "record")]
    fn handle_read_recording(&self, params: Path) -> Result<Response<Bytes>, Error> {
        let rec = self.state.export_recording(param("id", params)?)?;
        let status_code = rec
            .as_ref()
            .map_or(StatusCode::NOT_FOUND, |_| StatusCode::OK);
        return response(status_code, rec);
    }

    #[cfg(feature = "record")]
    fn handle_load_recording(&self, req: Request<Bytes>) -> Result<Response<Bytes>, Error> {
        let recording_file_content = std::str::from_utf8(&req.body())
            .map_err(|err| RequestConversionError(err.to_string()))?;

        let rec = self
            .state
            .load_mocks_from_recording(recording_file_content)?;
        return response(StatusCode::OK, Some(rec));
    }

    async fn catch_all(&self, req: Request<Bytes>) -> Result<Response<Bytes>, Error> {
        let internal_request: HttpMockRequest = (&req)
            .try_into()
            .map_err(|err: DataError| RequestConversionError(err.to_string()))?;

        let mut is_proxied = false;
        let start = Instant::now();

        #[cfg(feature = "proxy")]
        let res = if let Some(rule) = self.state.find_forward_rule(&internal_request)? {
            self.forward(rule, req).await?
        } else if let Some(rule) = self.state.find_proxy_rule(&internal_request)? {
            is_proxied = true;
            self.proxy(rule, req).await?
        } else {
            self.serve_mock(&internal_request).await?
        };

        #[cfg(not(feature = "proxy"))]
        let res = self.serve_mock(&internal_request).await?;

        #[cfg(feature = "record")]
        self.state
            .record(is_proxied, start.elapsed(), internal_request, &res)?;

        Ok(res)
    }

    #[cfg(feature = "proxy")]
    async fn forward(
        &self,
        rule: ActiveForwardingRule,
        req: Request<Bytes>,
    ) -> Result<Response<Bytes>, Error> {
        let to_base_uri: Uri = rule.config.target_base_url.parse().unwrap();

        let (mut req_parts, body) = req.into_parts();

        // We need to remove the host header, because it contains the host of this mock server.
        req_parts.headers.remove(http::header::HOST);

        let mut uri_parts = req_parts.uri.into_parts();
        uri_parts.authority = Some(to_base_uri.authority().unwrap().clone());
        uri_parts.scheme = to_base_uri.scheme().map(|s| s.clone()).or(uri_parts.scheme);
        req_parts.uri = Uri::from_parts(uri_parts).unwrap();

        // Record the upstream scheme (http/https) so the HttpClient can reconstruct
        // an absolute target URI after converting to origin-form.
        let upstream_scheme: &'static str = match to_base_uri.scheme_str() {
            Some("https") => "https",
            _ => "http",
        };
        req_parts
            .extensions
            .insert(crate::server::RequestMetadata::new(upstream_scheme));

        if !rule.config.request_header.is_empty() {
            for (key, value) in &rule.config.request_header {
                let key = HeaderName::from_str(key).map_err(|err| {
                    InvalidHeader(format!("invalid header key: {}", err.to_string()))
                })?;

                let value = HeaderValue::from_str(value).map_err(|err| {
                    InvalidHeader(format!("invalid header value: {}", err.to_string()))
                })?;

                req_parts.headers.insert(key, value);
            }
        }

        let req = Request::from_parts(req_parts, body);
        // Requests are normalized to absolute-form inside this server for internal uniformity
        // (matchers/recorders can read scheme/host/port from req.uri()). Before talking to an
        // upstream origin server we MUST convert to origin-form (path + query only) and provide
        // the authority via the Host header, as expected by HTTP/1.1 and HTTP/2 origin servers.
        let req = to_origin_form(req)?;
        Ok(self.http_client.send(req).await?)
    }

    #[cfg(feature = "proxy")]
    async fn proxy(
        &self,
        rule: ActiveProxyRule,
        mut req: Request<Bytes>,
    ) -> Result<Response<Bytes>, Error> {
        if !rule.config.request_header.is_empty() {
            let headers = req.headers_mut();

            for (key, value) in &rule.config.request_header {
                let key = HeaderName::from_str(key).map_err(|err| {
                    InvalidHeader(format!("invalid header key: {}", err.to_string()))
                })?;

                let value = HeaderValue::from_str(value).map_err(|err| {
                    InvalidHeader(format!("invalid header value: {}", err.to_string()))
                })?;

                headers.insert(key, value);
            }
        }

        // Requests are normalized to absolute-form inside this server for internal uniformity
        // (matchers/recorders can read scheme/host/port from req.uri()). Before talking to an
        // upstream origin server we MUST convert to origin-form (path + query only) and provide
        // the authority via the Host header, as expected by HTTP/1.1 and HTTP/2 origin servers.
        let req = to_origin_form(req)?;
        Ok(self.http_client.send(req).await?)
    }

    async fn serve_mock(&self, req: &HttpMockRequest) -> Result<Response<Bytes>, Error> {
        let mock_response = self.state.serve_mock(req)?;

        if let Some(mock_response) = mock_response {
            let status_code = match mock_response.status.as_ref() {
                None => StatusCode::OK,
                Some(c) => StatusCode::from_u16(c.clone())?,
            };

            let mut builder = Response::builder().status(status_code);

            if let Some(headers) = mock_response.headers {
                for (name, value) in headers {
                    builder = builder.header(name, value);
                }
            }

            let response = builder
                .body(
                    mock_response
                        .body
                        .map_or(Bytes::new(), |bytes| bytes.to_bytes()),
                )
                .map_err(|e| ResponseBodyConversionError(e))?;

            if let Some(duration) = mock_response.delay {
                runtime::sleep(Duration::from_millis(duration)).await;
            }

            return Ok(response);
        }

        let status_code = mock_response.map_or(StatusCode::NOT_FOUND, |_| StatusCode::OK);

        return response(
            status_code,
            Some(ErrorResponse::new(
                &"Request did not match any route or mock",
            )),
        );
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
            let parsed_value = parse_result.map_err(|e| ParamFormatError(format!("{:?}", e)))?;
            return Ok(parsed_value);
        }
    }

    Err(ParamError)
}

fn response<T>(status: StatusCode, body: Option<T>) -> Result<Response<Bytes>, Error>
where
    T: Serialize,
{
    let mut builder = Response::builder().status(status);

    if let Some(body_obj) = body {
        builder = builder.header("content-type", "application/json");

        let body_bytes =
            serde_json::to_vec(&body_obj).map_err(|e| ResponseBodySerializeError(e))?;

        return Ok(builder
            .body(Bytes::from(body_bytes))
            .map_err(|e| ResponseBodyConversionError(e))?);
    }

    return Ok(builder
        .body(Bytes::new())
        .map_err(|e| ResponseBodyConversionError(e))?);
}

fn parse_json_body<T>(req: Request<Bytes>) -> Result<T, Error>
where
    T: DeserializeOwned,
{
    let body: T =
        serde_json::from_slice(req.body().as_ref()).map_err(|e| RequestBodyDeserializeError(e))?;
    Ok(body)
}

fn extract_query_params(req: &Request<Bytes>) -> Result<Vec<(String, String)>, Error> {
    // There doesn't seem to be a way to just parse Query string with the `url` crate, so we're
    // prefixing a dummy URL for parsing.
    let url = format!("http://dummy?{}", req.uri().query().unwrap_or(""));
    let url = url::Url::parse(&url).map_err(|e| RequestConversionError(e.to_string()))?;

    let query_params = url
        .query_pairs()
        .map(|(k, v)| (k.into(), v.into()))
        .collect();

    Ok(query_params)
}

fn headers_to_vec<T>(req: &Request<T>) -> Result<Vec<(String, String)>, Error> {
    req.headers()
        .iter()
        .map(|(name, value)| {
            // Attempt to convert the HeaderValue to a &str, returning an error if it fails.
            let value_str = value
                .to_str()
                .map_err(|e| RequestConversionError(e.to_string()))?;
            Ok((name.as_str().to_string(), value_str.to_string()))
        })
        .collect()
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
            let host_val = HeaderValue::from_str(auth.as_str()).map_err(|err| {
                InvalidHeader(format!("invalid header value: {}", err.to_string()))
            })?;
            req.headers_mut().insert(http::header::HOST, host_val);
        }

        // Set path-and-query only (origin-form)
        let path_and_query = uri.path_and_query().map(|pq| pq.as_str()).unwrap_or("/");
        let new_uri = Uri::builder()
            .path_and_query(path_and_query)
            .build()
            .map_err(|e| RequestConversionError(e.to_string()))?;
        *req.uri_mut() = new_uri;
    }

    Ok(req)
}
