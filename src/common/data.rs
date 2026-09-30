use std::{cmp::Ordering, fmt, fmt::Debug, str::FromStr, sync::Arc};

use bytes::Bytes;
#[cfg(feature = "cookies")]
use headers::{Cookie, HeaderMapExt};
use http::{
    HeaderMap, HeaderValue, Method as HttpMethod, Uri, Version,
    uri::{Authority, Scheme},
};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::Value;

pub type ResponseCallback = Arc<dyn Fn(&HttpMockRequest) -> HttpMockResponse + Send + Sync>;
pub type RequestPredicate = Arc<dyn Fn(&HttpMockRequest) -> bool + Send + Sync>;

use crate::{
    common::{data::Error::HeaderDeserialization, util::HttpMockBytes},
    server::{RequestMetadata, matchers::generic::MatchingStrategy},
};

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("Cannot deserialize header: {0}")]
    HeaderDeserialization(String),
    #[error("cannot convert to/from static mock: {0}")]
    StaticMockConversion(String),
    #[error("cannot convert JSON: {0}")]
    JSONConversion(#[from] serde_json::Error),
    #[error("Invalid request data: {0}")]
    InvalidRequestData(String),
    #[error("Response conversion error: {0}")]
    ResponseConversion(String),
}

/// An error converting an HTTP request into [`HttpMockRequest`].
#[derive(thiserror::Error, Debug)]
#[non_exhaustive]
pub enum HttpMockRequestConversionError {
    #[error("request has no URI scheme or transport metadata")]
    MissingScheme,
    #[error("invalid request authority: {0}")]
    InvalidAuthority(#[source] http::uri::InvalidUri),
}

#[derive(thiserror::Error, Debug)]
enum HttpMockRequestWireError {
    #[error(transparent)]
    Request(#[from] HttpMockRequestConversionError),
    #[error("request scheme {request} does not match URI scheme {uri}")]
    SchemeMismatch { request: Scheme, uri: Scheme },
    #[error("invalid request scheme: {0}")]
    InvalidScheme(#[source] http::uri::InvalidUri),
    #[error("invalid request URI: {0}")]
    InvalidUri(#[source] http::uri::InvalidUri),
    #[error("invalid request method: {0}")]
    InvalidMethod(#[source] http::method::InvalidMethod),
    #[error("invalid request header name: {0}")]
    InvalidHeaderName(#[source] http::header::InvalidHeaderName),
    #[error("invalid request header value: {0}")]
    InvalidHeaderValue(#[source] http::header::InvalidHeaderValue),
    #[error("unsupported HTTP version: {0}")]
    UnsupportedVersion(String),
}

/// A validated HTTP request received by `httpmock`.
#[derive(Debug, Clone)]
pub struct HttpMockRequest {
    scheme: Scheme,
    uri: Uri,
    authority: Option<Authority>,
    method: HttpMethod,
    headers: HeaderMap,
    version: Version,
    body: HttpMockBytes,
}

impl HttpMockRequest {
    fn from_parts(
        scheme: Scheme,
        uri: Uri,
        method: HttpMethod,
        headers: HeaderMap,
        version: Version,
        body: HttpMockBytes,
    ) -> Result<Self, HttpMockRequestConversionError> {
        let authority = match uri.authority() {
            Some(authority) => Some(authority.clone()),
            None => headers
                .get(http::header::HOST)
                .map(HeaderValue::as_bytes)
                .map(Authority::try_from)
                .transpose()
                .map_err(HttpMockRequestConversionError::InvalidAuthority)?,
        };

        Ok(Self {
            scheme,
            uri,
            authority,
            method,
            headers,
            version,
            body,
        })
    }

    /// Returns the request URI.
    pub fn uri(&self) -> &Uri {
        &self.uri
    }

    /// Returns the request scheme, including transport-derived fallback data for origin-form URIs.
    pub fn scheme(&self) -> &Scheme {
        &self.scheme
    }

    /// Returns the request authority from the URI or `Host` header.
    pub fn authority(&self) -> Option<&Authority> {
        self.authority.as_ref()
    }

    /// Returns the request host without its port.
    pub fn host(&self) -> Option<&str> {
        self.authority().map(Authority::host)
    }

    /// Returns the explicit request port, 443 for HTTPS, or 80 for other schemes.
    pub fn port(&self) -> u16 {
        self.authority()
            .and_then(|authority| authority.port_u16())
            .unwrap_or_else(|| if self.scheme() == &Scheme::HTTPS { 443 } else { 80 })
    }

    /// Returns the request method.
    pub fn method(&self) -> &HttpMethod {
        &self.method
    }

    /// Returns the request headers.
    pub fn headers(&self) -> &HeaderMap {
        &self.headers
    }

    /// Returns the decoded query parameter pairs in URI order.
    pub fn query_params(&self) -> impl Iterator<Item = (std::borrow::Cow<'_, str>, std::borrow::Cow<'_, str>)> + '_ {
        form_urlencoded::parse(self.uri().query().unwrap_or("").as_bytes())
    }

    /// Returns the request body.
    pub fn body(&self) -> &HttpMockBytes {
        &self.body
    }

    /// Returns the HTTP version.
    pub fn version(&self) -> Version {
        self.version
    }

    #[cfg(feature = "cookies")]
    pub(crate) fn cookies(&self) -> Vec<(String, String)> {
        let mut result = Vec::new();

        if let Some(cookie) = self.headers.typed_get::<Cookie>() {
            for (key, value) in cookie.iter() {
                result.push((key.to_string(), value.to_string()));
            }
        }

        result
    }
}

#[derive(Deserialize)]
#[serde(untagged)]
enum HttpHeaderValueWire {
    Text(String),
    Bytes(Vec<u8>),
}

#[derive(Serialize)]
#[serde(untagged)]
enum HttpHeaderValueWireRef<'a> {
    Text(&'a str),
    Bytes(&'a [u8]),
}

#[derive(Deserialize)]
struct HttpMockRequestWire {
    scheme: String,
    uri: String,
    method: String,
    headers: Vec<(String, HttpHeaderValueWire)>,
    version: String,
    body: HttpMockBytes,
}

#[derive(Serialize)]
struct HttpMockRequestWireRef<'a> {
    scheme: &'a str,
    uri: String,
    method: &'a str,
    headers: Vec<(&'a str, HttpHeaderValueWireRef<'a>)>,
    version: &'static str,
    body: &'a HttpMockBytes,
}

impl TryFrom<HttpMockRequestWire> for HttpMockRequest {
    type Error = HttpMockRequestWireError;

    fn try_from(request: HttpMockRequestWire) -> Result<Self, Self::Error> {
        let HttpMockRequestWire {
            scheme,
            uri,
            method,
            headers,
            version,
            body,
        } = request;
        let scheme = Scheme::from_str(&scheme).map_err(HttpMockRequestWireError::InvalidScheme)?;
        let uri = Uri::try_from(uri).map_err(HttpMockRequestWireError::InvalidUri)?;
        if let Some(uri_scheme) = uri.scheme()
            && uri_scheme != &scheme
        {
            return Err(HttpMockRequestWireError::SchemeMismatch {
                request: scheme,
                uri: uri_scheme.clone(),
            });
        }
        let method = HttpMethod::from_bytes(method.as_bytes()).map_err(HttpMockRequestWireError::InvalidMethod)?;
        let version = parse_http_version(&version)?;

        let mut header_map = HeaderMap::with_capacity(headers.len());
        for (name, value) in headers {
            let name = http::HeaderName::try_from(name).map_err(HttpMockRequestWireError::InvalidHeaderName)?;
            let value = match value {
                HttpHeaderValueWire::Text(value) => HeaderValue::try_from(value),
                HttpHeaderValueWire::Bytes(value) => HeaderValue::try_from(value),
            }
            .map_err(HttpMockRequestWireError::InvalidHeaderValue)?;
            header_map.append(name, value);
        }

        Ok(Self::from_parts(scheme, uri, method, header_map, version, body)?)
    }
}

impl Serialize for HttpMockRequest {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let headers = self
            .headers
            .iter()
            .map(|(name, value)| {
                let value = match value.to_str() {
                    Ok(value) => HttpHeaderValueWireRef::Text(value),
                    Err(_) => HttpHeaderValueWireRef::Bytes(value.as_bytes()),
                };
                (name.as_str(), value)
            })
            .collect();
        HttpMockRequestWireRef {
            scheme: self.scheme().as_str(),
            uri: self.uri.to_string(),
            method: self.method.as_str(),
            headers,
            version: http_version_str(self.version).map_err(serde::ser::Error::custom)?,
            body: &self.body,
        }
        .serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for HttpMockRequest {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        HttpMockRequestWire::deserialize(deserializer)?
            .try_into()
            .map_err(serde::de::Error::custom)
    }
}

fn http_version_str(version: Version) -> Result<&'static str, HttpMockRequestWireError> {
    match version {
        Version::HTTP_09 => Ok("HTTP/0.9"),
        Version::HTTP_10 => Ok("HTTP/1.0"),
        Version::HTTP_11 => Ok("HTTP/1.1"),
        Version::HTTP_2 => Ok("HTTP/2.0"),
        Version::HTTP_3 => Ok("HTTP/3.0"),
        _ => Err(HttpMockRequestWireError::UnsupportedVersion(format!("{version:?}"))),
    }
}

fn parse_http_version(version: &str) -> Result<Version, HttpMockRequestWireError> {
    match version {
        "HTTP/0.9" => Ok(Version::HTTP_09),
        "HTTP/1.0" => Ok(Version::HTTP_10),
        "HTTP/1.1" => Ok(Version::HTTP_11),
        "HTTP/2.0" | "HTTP/2" => Ok(Version::HTTP_2),
        "HTTP/3.0" | "HTTP/3" => Ok(Version::HTTP_3),
        _ => Err(HttpMockRequestWireError::UnsupportedVersion(version.to_string())),
    }
}

fn request_scheme<B>(request: &http::Request<B>) -> Result<Scheme, HttpMockRequestConversionError> {
    if let Some(scheme) = request.uri().scheme() {
        return Ok(scheme.clone());
    }

    let metadata = request
        .extensions()
        .get::<RequestMetadata>()
        .ok_or(HttpMockRequestConversionError::MissingScheme)?;

    Ok(metadata.scheme.clone())
}

impl<B> TryFrom<http::Request<B>> for HttpMockRequest
where
    B: Into<HttpMockBytes>,
{
    type Error = HttpMockRequestConversionError;

    fn try_from(request: http::Request<B>) -> Result<Self, Self::Error> {
        let scheme = request_scheme(&request)?;
        let (parts, body) = request.into_parts();

        Self::from_parts(
            scheme,
            parts.uri,
            parts.method,
            parts.headers,
            parts.version,
            body.into(),
        )
    }
}

impl<B> TryFrom<&http::Request<B>> for HttpMockRequest
where
    B: Clone + Into<HttpMockBytes>,
{
    type Error = HttpMockRequestConversionError;

    fn try_from(request: &http::Request<B>) -> Result<Self, Self::Error> {
        Self::from_parts(
            request_scheme(request)?,
            request.uri().clone(),
            request.method().clone(),
            request.headers().clone(),
            request.version(),
            request.body().clone().into(),
        )
    }
}

impl From<HttpMockRequest> for http::Request<Bytes> {
    fn from(req: HttpMockRequest) -> Self {
        let scheme = req.scheme.clone();
        let mut request = http::Request::new(req.body.into());
        *request.method_mut() = req.method;
        *request.uri_mut() = req.uri;
        *request.version_mut() = req.version;
        *request.headers_mut() = req.headers;
        request.extensions_mut().insert(RequestMetadata::new(scheme));
        request
    }
}

impl From<&HttpMockRequest> for http::Request<Bytes> {
    fn from(req: &HttpMockRequest) -> Self {
        req.clone().into()
    }
}

#[cfg(test)]
mod http_message_tests {
    use super::*;

    #[test]
    fn request_keeps_typed_http_parts() {
        let mut request = http::Request::builder()
            .method(http::Method::PATCH)
            .uri("https://[::1]:8443/search?q=rust")
            .version(http::Version::HTTP_2)
            .body(Bytes::from_static(b"body"))
            .unwrap();
        request
            .headers_mut()
            .append(http::header::ACCEPT, HeaderValue::from_static("text/plain"));
        request
            .headers_mut()
            .append(http::header::ACCEPT, HeaderValue::from_static("application/json"));
        request
            .headers_mut()
            .insert("x-binary", HeaderValue::from_bytes(&[0x80]).unwrap());

        let request = HttpMockRequest::try_from(request).unwrap();

        assert_eq!(request.scheme(), &Scheme::HTTPS);
        assert_eq!(
            request.uri(),
            &"https://[::1]:8443/search?q=rust".parse::<Uri>().unwrap()
        );
        assert_eq!(request.method(), http::Method::PATCH);
        assert_eq!(request.version(), Version::HTTP_2);
        assert_eq!(request.authority().map(Authority::as_str), Some("[::1]:8443"));
        assert_eq!(request.host(), Some("[::1]"));
        assert_eq!(request.port(), 8443);
        assert_eq!(request.headers().get_all(http::header::ACCEPT).iter().count(), 2);
        assert_eq!(request.headers()["x-binary"].as_bytes(), &[0x80]);
        assert_eq!(request.body().as_ref(), b"body");
        assert_eq!(
            request
                .query_params()
                .map(|(key, value)| (key.into_owned(), value.into_owned()))
                .collect::<Vec<_>>(),
            vec![("q".to_string(), "rust".to_string())]
        );
    }

    #[test]
    fn request_uses_ipv6_host_header_for_origin_form_uri() {
        let mut request = http::Request::builder()
            .uri("/resource")
            .header(http::header::HOST, "[::1]:8080")
            .body(Bytes::new())
            .unwrap();
        request.extensions_mut().insert(RequestMetadata::new(Scheme::HTTP));

        let request = HttpMockRequest::try_from(request).unwrap();

        assert_eq!(request.authority().map(Authority::as_str), Some("[::1]:8080"));
        assert_eq!(request.host(), Some("[::1]"));
        assert_eq!(request.port(), 8080);

        let mut request = http::Request::builder()
            .uri("/resource")
            .header(http::header::HOST, "[::1]")
            .body(Bytes::new())
            .unwrap();
        request.extensions_mut().insert(RequestMetadata::new(Scheme::HTTPS));

        let request = HttpMockRequest::try_from(request).unwrap();

        assert_eq!(request.host(), Some("[::1]"));
        assert_eq!(request.port(), 443);
    }

    #[test]
    fn request_rejects_invalid_host_authority() {
        let mut request = http::Request::builder()
            .uri("/resource")
            .header(http::header::HOST, "not an authority")
            .body(Bytes::new())
            .unwrap();
        request.extensions_mut().insert(RequestMetadata::new(Scheme::HTTP));

        assert!(HttpMockRequest::try_from(request).is_err());
    }

    #[test]
    fn uri_authority_takes_precedence_over_host_header() {
        let request = http::Request::builder()
            .uri("http://uri.example:8080/resource")
            .header(http::header::HOST, "header.example:9090")
            .body(Bytes::new())
            .unwrap();

        let request = HttpMockRequest::try_from(request).unwrap();

        assert_eq!(request.authority().map(Authority::as_str), Some("uri.example:8080"));
    }

    #[test]
    fn request_wire_format_matches_existing_schema() {
        let fixture = serde_json::json!({
            "scheme": "https",
            "uri": "https://example.com/resource",
            "method": "POST",
            "headers": [["x-test", "one"], ["x-test", "two"]],
            "version": "HTTP/1.1",
            "body": [98, 111, 100, 121]
        });

        let request: HttpMockRequest = serde_json::from_value(fixture.clone()).unwrap();

        assert_eq!(request.headers().get_all("x-test").iter().count(), 2);
        assert_eq!(serde_json::to_value(request).unwrap(), fixture);
    }

    #[test]
    fn request_wire_format_rejects_conflicting_schemes() {
        let fixture = serde_json::json!({
            "scheme": "http",
            "uri": "https://example.com/resource",
            "method": "GET",
            "headers": [],
            "version": "HTTP/1.1",
            "body": []
        });

        assert!(serde_json::from_value::<HttpMockRequest>(fixture).is_err());
    }

    #[test]
    fn request_wire_format_preserves_non_utf8_headers() {
        let mut request = http::Request::builder()
            .uri("http://example.com/")
            .body(Bytes::new())
            .unwrap();
        request
            .headers_mut()
            .insert("x-binary", HeaderValue::from_bytes(&[0x80]).unwrap());
        let request = HttpMockRequest::try_from(request).unwrap();

        let wire = serde_json::to_value(&request).unwrap();
        assert_eq!(wire["headers"], serde_json::json!([["x-binary", [128]]]));

        let request: HttpMockRequest = serde_json::from_value(wire).unwrap();
        assert_eq!(request.headers()["x-binary"].as_bytes(), &[0x80]);
    }

    #[test]
    fn wire_format_maps_every_supported_http_version_explicitly() {
        for (version, wire) in [
            (Version::HTTP_09, "HTTP/0.9"),
            (Version::HTTP_10, "HTTP/1.0"),
            (Version::HTTP_11, "HTTP/1.1"),
            (Version::HTTP_2, "HTTP/2.0"),
            (Version::HTTP_3, "HTTP/3.0"),
        ] {
            let request = http::Request::builder()
                .uri("http://example.com/")
                .version(version)
                .body(Bytes::new())
                .unwrap();
            let request = HttpMockRequest::try_from(request).unwrap();
            let encoded = serde_json::to_value(&request).unwrap();

            assert_eq!(encoded["version"], wire);
            assert_eq!(
                serde_json::from_value::<HttpMockRequest>(encoded).unwrap().version(),
                version
            );
        }
    }

    #[test]
    fn borrowed_request_conversion_preserves_parts_and_transport_metadata() {
        let mut request = http::Request::builder()
            .method(http::Method::PUT)
            .uri("/resource")
            .header("x-test", "value")
            .body(Bytes::from_static(b"request"))
            .unwrap();
        request.extensions_mut().insert(RequestMetadata::new(Scheme::HTTPS));

        let request = HttpMockRequest::try_from(&request).unwrap();
        let request: http::Request<Bytes> = (&request).into();

        assert_eq!(request.method(), http::Method::PUT);
        assert_eq!(request.headers()["x-test"], "value");
        assert_eq!(request.body(), &Bytes::from_static(b"request"));
        assert_eq!(
            request.extensions().get::<RequestMetadata>().unwrap().scheme,
            Scheme::HTTPS
        );

        let request = HttpMockRequest::try_from(request).unwrap();
        assert_eq!(request.scheme(), &Scheme::HTTPS);
    }

    #[test]
    fn unit_is_an_empty_request_body() {
        let mut request = http::Request::new(());
        request.extensions_mut().insert(RequestMetadata::new(Scheme::HTTP));

        let request = HttpMockRequest::try_from(request).unwrap();

        assert!(request.body().is_empty());
    }
}

/// A general abstraction of an HTTP response for all handlers.
#[derive(Serialize, Deserialize, Clone)]
pub struct HttpMockResponse {
    pub status: Option<u16>,
    pub headers: Option<Vec<(String, String)>>,
    #[serde(default, with = "opt_vector_serde_base64")]
    pub body: Option<HttpMockBytes>,
}

impl HttpMockResponse {
    pub fn builder() -> HttpMockResponseBuilder {
        HttpMockResponseBuilder::new()
    }
}

/// Converts an `HttpMockResponse` into a real `http::Response<Bytes>`.
impl TryFrom<HttpMockResponse> for http::Response<bytes::Bytes> {
    type Error = Error;

    fn try_from(res: HttpMockResponse) -> Result<Self, Self::Error> {
        (&res).try_into() // reuse the by-ref impl
    }
}

impl TryFrom<&HttpMockResponse> for http::Response<bytes::Bytes> {
    type Error = Error;

    fn try_from(res: &HttpMockResponse) -> Result<Self, Self::Error> {
        let raw_status = res
            .status
            .ok_or_else(|| Error::ResponseConversion("missing status".into()))?;

        let status = http::StatusCode::from_u16(raw_status)
            .map_err(|_| Error::ResponseConversion(format!("invalid status: {}", raw_status)))?;

        let mut builder = http::Response::builder().status(status);

        if let Some(headers) = &res.headers {
            for (name, value) in headers {
                let header_name = http::header::HeaderName::try_from(name.clone())
                    .map_err(|_| Error::ResponseConversion(format!("invalid header name: {}", name)))?;

                let header_value = http::header::HeaderValue::try_from(value.clone()).map_err(|_| {
                    Error::ResponseConversion(format!("invalid header value for '{}': {}", name, value))
                })?;

                builder = builder.header(header_name, header_value);
            }
        }

        let body = res.body.as_ref().map_or(bytes::Bytes::new(), |b| b.0.clone());

        builder
            .body(body)
            .map_err(|e| Error::ResponseConversion(format!("http build error: {}", e)))
    }
}

impl<B> TryFrom<&http::Response<B>> for HttpMockResponse
where
    B: Clone + Into<HttpMockBytes>,
{
    type Error = Error;

    fn try_from(resp: &http::Response<B>) -> Result<Self, Self::Error> {
        // headers -> Vec<(String, String)> (UTF-8 strict)
        let mut headers = Vec::with_capacity(resp.headers().len());
        for (name, value) in resp.headers() {
            let name = name.as_str().to_string();
            let val = value
                .to_str()
                .map_err(|_| Error::ResponseConversion(format!("non-utf8 header value for '{}'", name)))?;
            headers.push((name, val.to_string()));
        }

        let body = resp.body().clone().into();

        Ok(HttpMockResponse {
            status: Some(resp.status().as_u16()),
            headers: Some(headers),
            body: Some(body),
        })
    }
}

impl<B> From<http::Response<B>> for HttpMockResponse
where
    B: Into<HttpMockBytes>,
{
    fn from(resp: http::Response<B>) -> Self {
        let (parts, body) = resp.into_parts();
        let headers = parts
            .headers
            .iter()
            .map(|(name, value)| {
                (
                    name.as_str().to_string(),
                    value
                        .to_str()
                        .unwrap_or_else(|_| panic!("non-UTF-8 value for response header '{name}'"))
                        .to_string(),
                )
            })
            .collect();

        Self {
            status: Some(parts.status.as_u16()),
            headers: Some(headers),
            body: Some(body.into()),
        }
    }
}

#[cfg(test)]
mod http_body_conversion_tests {
    use super::*;

    struct NonCloneBody(&'static [u8]);

    impl From<NonCloneBody> for HttpMockBytes {
        fn from(body: NonCloneBody) -> Self {
            body.0.into()
        }
    }

    #[test]
    fn owned_message_conversions_accept_non_clone_bodies() {
        let mut request = http::Request::new(NonCloneBody(b"request"));
        request.extensions_mut().insert(RequestMetadata::new(Scheme::HTTP));
        let request = HttpMockRequest::try_from(request).unwrap();

        let response = http::Response::new(NonCloneBody(b"response"));
        let response: HttpMockResponse = response.into();

        assert_eq!(request.body().as_ref(), b"request");
        assert_eq!(response.body.as_ref().unwrap().as_ref(), b"response");
    }

    #[test]
    fn borrowed_message_conversions_still_accept_clone_bodies() {
        let mut request = http::Request::new(Vec::from(b"request"));
        request.extensions_mut().insert(RequestMetadata::new(Scheme::HTTP));
        let request = HttpMockRequest::try_from(&request).unwrap();

        let response = http::Response::new(Vec::from(b"response"));
        let response = HttpMockResponse::try_from(&response).unwrap();

        assert_eq!(request.body().as_ref(), b"request");
        assert_eq!(response.body.as_ref().unwrap().as_ref(), b"response");
    }

    #[test]
    fn unit_is_an_empty_http_body() {
        let response: HttpMockResponse = http::Response::new(()).into();

        assert!(response.body.unwrap().is_empty());
    }
}

#[derive(Default, Debug, Clone)]
pub struct HttpMockResponseBuilder {
    status: Option<u16>,
    headers: Vec<(String, String)>,
    body: Option<HttpMockBytes>,
}

impl HttpMockResponseBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    /// Set an HTTP status (e.g., 200, 404).
    pub fn status(mut self, status: u16) -> Self {
        self.status = Some(status);
        self
    }

    /// Add a single header (appends; duplicates are allowed).
    pub fn header<K, V>(mut self, key: K, val: V) -> Self
    where
        K: Into<String>,
        V: Into<String>,
    {
        self.headers.push((key.into(), val.into()));
        self
    }

    /// Replace all headers at once.
    pub fn headers<I, K, V>(mut self, headers: I) -> Self
    where
        I: IntoIterator<Item = (K, V)>,
        K: Into<String>,
        V: Into<String>,
    {
        self.headers = headers.into_iter().map(|(k, v)| (k.into(), v.into())).collect();
        self
    }

    /// Set a body from anything convertible into `HttpMockBytes`.
    pub fn body<B>(mut self, body: B) -> Self
    where
        B: Into<HttpMockBytes>,
    {
        self.body = Some(body.into());
        self
    }

    /// Explicitly clear the body.
    pub fn no_body(mut self) -> Self {
        self.body = None;
        self
    }

    /// Finalize into `HttpMockResponse`.
    pub fn build(self) -> HttpMockResponse {
        HttpMockResponse {
            status: self.status,
            headers: if self.headers.is_empty() {
                None
            } else {
                Some(self.headers)
            },
            body: self.body,
        }
    }
}

/// A general abstraction of an HTTP response for all handlers.
#[derive(Serialize, Deserialize, Clone, Default)]
pub struct MockServerHttpResponse {
    pub status: Option<u16>,
    pub headers: Option<Vec<(String, String)>>,
    #[serde(default, with = "opt_vector_serde_base64")]
    pub body: Option<HttpMockBytes>,
    pub delay: Option<u64>,
    #[serde(skip)]
    pub respond_with: Option<ResponseCallback>,
}

impl TryFrom<&http::Response<Bytes>> for MockServerHttpResponse {
    type Error = Error;

    fn try_from(value: &http::Response<Bytes>) -> Result<Self, Self::Error> {
        let mut headers = Vec::with_capacity(value.headers().len());

        for (key, value) in value.headers() {
            let value = value.to_str().map_err(|err| HeaderDeserialization(err.to_string()))?;

            headers.push((key.as_str().to_string(), value.to_string()))
        }

        Ok(Self {
            status: Some(value.status().as_u16()),
            headers: if !headers.is_empty() { Some(headers) } else { None },
            body: if !value.body().is_empty() {
                Some(HttpMockBytes::from(value.body().clone()))
            } else {
                None
            },
            delay: None,
            respond_with: None,
        })
    }
}

/// Serializes and deserializes the response body to/from a Base64 string.
mod opt_vector_serde_base64 {
    use base64::{Engine, engine::general_purpose::STANDARD as BASE64};
    use bytes::Bytes;
    use serde::{Deserialize, Deserializer, Serializer};

    use crate::common::util::HttpMockBytes;

    // See the following references:
    // https://github.com/serde-rs/serde/blob/master/serde/src/ser/impls.rs#L99
    // https://github.com/serde-rs/serde/issues/661
    pub fn serialize<T, S>(bytes: &Option<T>, serializer: S) -> Result<S::Ok, S::Error>
    where
        T: AsRef<[u8]>,
        S: Serializer,
    {
        match bytes {
            Some(value) => serializer.serialize_bytes(BASE64.encode(value).as_bytes()),
            None => serializer.serialize_none(),
        }
    }

    // See the following references:
    // https://github.com/serde-rs/serde/issues/1444
    pub fn deserialize<'de, D>(deserializer: D) -> Result<Option<HttpMockBytes>, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct Wrapper(#[serde(deserialize_with = "from_base64")] HttpMockBytes);

        let v = Option::deserialize(deserializer)?;
        Ok(v.map(|Wrapper(a)| a))
    }

    fn from_base64<'de, D>(deserializer: D) -> Result<HttpMockBytes, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = Vec::deserialize(deserializer)?;
        let decoded = BASE64.decode(value).map_err(serde::de::Error::custom)?;
        Ok(HttpMockBytes::from(Bytes::from(decoded)))
    }
}

/// Prints the response body as UTF8 string
impl fmt::Debug for MockServerHttpResponse {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("MockServerHttpResponse")
            .field("status", &self.status)
            .field("headers", &self.headers)
            .field(
                "body",
                &self
                    .body
                    .as_ref()
                    .map(|x| String::from_utf8_lossy(x.as_ref()).to_string()),
            )
            .field("delay", &self.delay)
            .finish()
    }
}

/// (De)serializes a [`regex::Regex`] as its pattern string.
mod regex_string {
    use serde::{Deserialize, Deserializer, Serializer, de::Error};

    pub fn serialize<S: Serializer>(value: &regex::Regex, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(value.as_str())
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<regex::Regex, D::Error> {
        let pattern = std::borrow::Cow::<str>::deserialize(deserializer)?;
        pattern.parse().map_err(Error::custom)
    }
}

/// A general abstraction of an HTTP request for all handlers.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct HttpMockRegex(#[serde(with = "regex_string")] pub regex::Regex);

impl Ord for HttpMockRegex {
    fn cmp(&self, other: &Self) -> Ordering {
        self.0.as_str().cmp(other.0.as_str())
    }
}

impl PartialOrd for HttpMockRegex {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl PartialEq for HttpMockRegex {
    fn eq(&self, other: &Self) -> bool {
        self.0.as_str() == other.0.as_str()
    }
}

impl Eq for HttpMockRegex {}

impl From<regex::Regex> for HttpMockRegex {
    fn from(value: regex::Regex) -> Self {
        HttpMockRegex(value)
    }
}

impl From<&str> for HttpMockRegex {
    fn from(value: &str) -> Self {
        let re = regex::Regex::from_str(value).expect("cannot parse value as regex");
        HttpMockRegex::from(re)
    }
}

impl From<String> for HttpMockRegex {
    fn from(value: String) -> Self {
        HttpMockRegex::from(value.as_str())
    }
}

impl fmt::Display for HttpMockRegex {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// A general abstraction of an HTTP request for all handlers.
#[derive(Serialize, Deserialize, Clone, Default)]
pub struct RequestRequirements {
    pub scheme: Option<String>,
    pub scheme_not: Option<String>, // NEW
    pub host: Option<String>,
    pub host_not: Option<Vec<String>>,        // NEW
    pub host_contains: Option<Vec<String>>,   // NEW
    pub host_excludes: Option<Vec<String>>,   // NEW
    pub host_prefix: Option<Vec<String>>,     // NEW
    pub host_suffix: Option<Vec<String>>,     // NEW
    pub host_prefix_not: Option<Vec<String>>, // NEW
    pub host_suffix_not: Option<Vec<String>>, // NEW
    pub host_matches: Option<Vec<HttpMockRegex>>,
    pub port: Option<u16>,
    pub port_not: Option<Vec<u16>>, // NEW
    pub method: Option<String>,
    pub method_not: Option<Vec<String>>, // NEW
    pub path: Option<String>,
    pub path_not: Option<Vec<String>>,        // NEW
    pub path_includes: Option<Vec<String>>,   // NEW
    pub path_excludes: Option<Vec<String>>,   // NEW
    pub path_prefix: Option<Vec<String>>,     // NEW
    pub path_suffix: Option<Vec<String>>,     // NEW
    pub path_prefix_not: Option<Vec<String>>, // NEW
    pub path_suffix_not: Option<Vec<String>>, // NEW
    pub path_matches: Option<Vec<HttpMockRegex>>,
    pub query_param: Option<Vec<(String, String)>>,
    pub query_param_not: Option<Vec<(String, String)>>, // NEW
    pub query_param_exists: Option<Vec<String>>,
    pub query_param_missing: Option<Vec<String>>,              // NEW
    pub query_param_includes: Option<Vec<(String, String)>>,   // NEW
    pub query_param_excludes: Option<Vec<(String, String)>>,   // NEW
    pub query_param_prefix: Option<Vec<(String, String)>>,     // NEW
    pub query_param_suffix: Option<Vec<(String, String)>>,     // NEW
    pub query_param_prefix_not: Option<Vec<(String, String)>>, // NEW
    pub query_param_suffix_not: Option<Vec<(String, String)>>, // NEW
    pub query_param_matches: Option<Vec<(HttpMockRegex, HttpMockRegex)>>, // NEW
    pub query_param_count: Option<Vec<(HttpMockRegex, HttpMockRegex, usize)>>, // NEW
    pub header: Option<Vec<(String, String)>>,                 // CHANGED from headers to header
    pub header_not: Option<Vec<(String, String)>>,             // NEW
    pub header_exists: Option<Vec<String>>,
    pub header_missing: Option<Vec<String>>,                              // NEW
    pub header_includes: Option<Vec<(String, String)>>,                   // NEW
    pub header_excludes: Option<Vec<(String, String)>>,                   // NEW
    pub header_prefix: Option<Vec<(String, String)>>,                     // NEW
    pub header_suffix: Option<Vec<(String, String)>>,                     // NEW
    pub header_prefix_not: Option<Vec<(String, String)>>,                 // NEW
    pub header_suffix_not: Option<Vec<(String, String)>>,                 // NEW
    pub header_matches: Option<Vec<(HttpMockRegex, HttpMockRegex)>>,      // NEW
    pub header_count: Option<Vec<(HttpMockRegex, HttpMockRegex, usize)>>, // NEW
    pub cookie: Option<Vec<(String, String)>>,                            // CHANGED from cookies to cookie
    pub cookie_not: Option<Vec<(String, String)>>,                        // NEW
    pub cookie_exists: Option<Vec<String>>,
    pub cookie_missing: Option<Vec<String>>,                              // NEW
    pub cookie_includes: Option<Vec<(String, String)>>,                   // NEW
    pub cookie_excludes: Option<Vec<(String, String)>>,                   // NEW
    pub cookie_prefix: Option<Vec<(String, String)>>,                     // NEW
    pub cookie_suffix: Option<Vec<(String, String)>>,                     // NEW
    pub cookie_prefix_not: Option<Vec<(String, String)>>,                 // NEW
    pub cookie_suffix_not: Option<Vec<(String, String)>>,                 // NEW
    pub cookie_matches: Option<Vec<(HttpMockRegex, HttpMockRegex)>>,      // NEW
    pub cookie_count: Option<Vec<(HttpMockRegex, HttpMockRegex, usize)>>, // NEW          // NEW
    pub body: Option<HttpMockBytes>,
    pub body_not: Option<Vec<HttpMockBytes>>,        // NEW
    pub body_includes: Option<Vec<HttpMockBytes>>,   // CHANG
    pub body_excludes: Option<Vec<HttpMockBytes>>,   // NEW
    pub body_prefix: Option<Vec<HttpMockBytes>>,     // NEW
    pub body_suffix: Option<Vec<HttpMockBytes>>,     // NEW
    pub body_prefix_not: Option<Vec<HttpMockBytes>>, //
    pub body_suffix_not: Option<Vec<HttpMockBytes>>, //
    pub body_matches: Option<Vec<HttpMockRegex>>,    // NEW
    pub json_body: Option<Value>,
    pub json_body_not: Option<Value>, // NEW
    pub json_body_includes: Option<Vec<Value>>,
    pub json_body_excludes: Option<Vec<Value>>, // NEW
    pub form_urlencoded_tuple: Option<Vec<(String, String)>>,
    pub form_urlencoded_tuple_not: Option<Vec<(String, String)>>, // NEW
    pub form_urlencoded_tuple_exists: Option<Vec<String>>,
    pub form_urlencoded_tuple_missing: Option<Vec<String>>, // NEW
    pub form_urlencoded_tuple_includes: Option<Vec<(String, String)>>, // NEW
    pub form_urlencoded_tuple_excludes: Option<Vec<(String, String)>>, // NEW
    pub form_urlencoded_tuple_prefix: Option<Vec<(String, String)>>, // NEW
    pub form_urlencoded_tuple_suffix: Option<Vec<(String, String)>>, // NEW
    pub form_urlencoded_tuple_prefix_not: Option<Vec<(String, String)>>, // NEW
    pub form_urlencoded_tuple_suffix_not: Option<Vec<(String, String)>>, // NEW
    pub form_urlencoded_tuple_matches: Option<Vec<(HttpMockRegex, HttpMockRegex)>>, // NEW
    pub form_urlencoded_tuple_count: Option<Vec<(HttpMockRegex, HttpMockRegex, usize)>>, // NEW
    #[serde(skip)]
    pub is_true: Option<Vec<RequestPredicate>>, // NEW + DEPRECATE matches() -> point to using "is_true" instead
    #[serde(skip)]
    pub is_false: Option<Vec<RequestPredicate>>, // NEW
}

/// A Request that is made to set a new mock.
#[derive(Serialize, Deserialize, Clone)]
pub struct MockDefinition {
    pub request: RequestRequirements,
    pub response: MockServerHttpResponse,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct ActiveMock {
    pub id: usize,
    pub call_counter: usize,
    pub definition: MockDefinition,
    pub is_static: bool,
}

#[cfg(feature = "proxy")]
#[derive(Serialize, Deserialize, Clone)]
pub struct ActiveForwardingRule {
    pub id: usize,
    pub config: ForwardingRuleConfig,
}

#[cfg(feature = "proxy")]
#[derive(Serialize, Deserialize, Clone)]
pub struct ActiveProxyRule {
    pub id: usize,
    pub config: ProxyRuleConfig,
}

#[cfg(feature = "record")]
#[derive(Serialize, Deserialize, Clone)]
pub struct ActiveRecording {
    pub id: usize,
    pub config: RecordingRuleConfig,
    pub mocks: Vec<MockDefinition>,
}

#[derive(Serialize, Deserialize)]
pub struct ClosestMatch {
    pub request: HttpMockRequest,
    pub request_index: usize,
    pub mismatches: Vec<Mismatch>,
}

#[derive(Serialize, Deserialize)]
pub struct ErrorResponse {
    pub message: String,
}

impl ErrorResponse {
    pub fn new<T>(message: &T) -> ErrorResponse
    where
        T: ToString,
    {
        ErrorResponse {
            message: message.to_string(),
        }
    }
}

// *************************************************************************************************
// Diff and Change correspond to difference::Changeset and Difference structs. They are duplicated
// here only for the reason to make them serializable/deserializable using serde.
// *************************************************************************************************
#[derive(PartialEq, Debug, Serialize, Deserialize)]
pub enum Diff {
    Same(String),
    Add(String),
    Rem(String),
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DiffResult {
    pub differences: Vec<Diff>,
    pub distance: f32,
    pub tokenizer: Tokenizer,
}

#[derive(PartialEq, Debug, Serialize, Deserialize, Clone, Copy)]
pub enum Tokenizer {
    Line,
    Word,
    Character,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct KeyValueComparisonKeyValuePair {
    pub key: String,
    pub value: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct KeyValueComparisonAttribute {
    pub operator: String,
    pub expected: String,
    pub actual: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct KeyValueComparison {
    pub key: Option<KeyValueComparisonAttribute>,
    pub value: Option<KeyValueComparisonAttribute>,
    pub expected_count: Option<usize>,
    pub actual_count: Option<usize>,
    pub all: Vec<KeyValueComparisonKeyValuePair>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct FunctionComparison {
    pub index: usize,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SingleValueComparison {
    pub operator: String,
    pub expected: String,
    pub actual: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Mismatch {
    pub entity: String,
    pub matcher_method: String,
    pub comparison: Option<SingleValueComparison>,
    pub key_value_comparison: Option<KeyValueComparison>,
    pub function_comparison: Option<FunctionComparison>,
    pub matching_strategy: Option<MatchingStrategy>,
    pub best_match: bool,
    pub diff: Option<DiffResult>,
}

// *************************************************************************************************
// Configs and Builders
// *************************************************************************************************

#[cfg(feature = "record")]
#[derive(Serialize, Deserialize, Clone, Default)]
pub struct RecordingRuleConfig {
    pub request_requirements: RequestRequirements,
    pub record_headers: Vec<String>,
    pub record_response_delays: bool,
}

#[cfg(feature = "proxy")]
#[derive(Serialize, Deserialize, Clone, Default)]
pub struct ProxyRuleConfig {
    pub request_requirements: RequestRequirements,
    pub request_header: Vec<(String, String)>,
}

#[cfg(feature = "proxy")]
#[derive(Serialize, Deserialize, Clone, Default)]
pub struct ForwardingRuleConfig {
    pub target_base_url: String,
    pub request_requirements: RequestRequirements,
    pub request_header: Vec<(String, String)>,
}

/// Represents an HTTP method.
#[derive(Serialize, Deserialize, Debug)]
pub enum Method {
    GET,
    HEAD,
    POST,
    PUT,
    DELETE,
    CONNECT,
    OPTIONS,
    TRACE,
    PATCH,
}

impl PartialEq<Method> for http::method::Method {
    fn eq(&self, other: &Method) -> bool {
        self.to_string().to_uppercase() == other.to_string().to_uppercase()
    }
}

impl FromStr for Method {
    type Err = String;

    fn from_str(input: &str) -> Result<Self, Self::Err> {
        match input.to_uppercase().as_str() {
            "GET" => Ok(Method::GET),
            "HEAD" => Ok(Method::HEAD),
            "POST" => Ok(Method::POST),
            "PUT" => Ok(Method::PUT),
            "DELETE" => Ok(Method::DELETE),
            "CONNECT" => Ok(Method::CONNECT),
            "OPTIONS" => Ok(Method::OPTIONS),
            "TRACE" => Ok(Method::TRACE),
            "PATCH" => Ok(Method::PATCH),
            _ => Err(format!("Invalid HTTP method {}", input)),
        }
    }
}

impl From<&str> for Method {
    fn from(value: &str) -> Self {
        value
            .parse()
            .unwrap_or_else(|_| panic!("Cannot parse HTTP method from string {:?}", value))
    }
}

impl std::fmt::Display for Method {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        std::fmt::Debug::fmt(self, f)
    }
}
