extern crate serde_regex;

use std::{
    cmp::Ordering,
    convert::{TryFrom, TryInto},
    fmt,
    fmt::Debug,
    str::FromStr,
    sync::Arc,
};

use base64::{Engine, engine::general_purpose::STANDARD as BASE64};
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
    common::{
        data::Error::{HeaderDeserializationError, RequestConversionError, StaticMockConversionError},
        util::HttpMockBytes,
    },
    server::{RequestMetadata, matchers::generic::MatchingStrategy},
};

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("Cannot deserialize header: {0}")]
    HeaderDeserializationError(String),
    #[error("Cookie parser error: {0}")]
    CookieParserError(String),
    #[error("cannot convert to/from static mock: {0}")]
    StaticMockConversionError(String),
    #[error("JSONConversionError: {0}")]
    JSONConversionError(#[from] serde_json::Error),
    #[error("Invalid request data: {0}")]
    InvalidRequestData(String),
    #[error("Cannot convert request to/from internal structure: {0}")]
    RequestConversionError(String),
    #[error("Response conversion error: {0}")]
    ResponseConversionError(String),
}

/// A validated HTTP request received by `httpmock`.
#[derive(Debug, Clone)]
pub struct HttpMockRequest {
    scheme: Scheme,
    uri: Uri,
    authority: RequestAuthority,
    method: HttpMethod,
    headers: HeaderMap,
    version: Version,
    body: HttpMockBytes,
}

#[derive(Debug, Clone)]
enum RequestAuthority {
    Uri,
    Host(Authority),
    None,
}

impl HttpMockRequest {
    fn from_parts(
        scheme: Scheme,
        uri: Uri,
        method: HttpMethod,
        headers: HeaderMap,
        version: Version,
        body: HttpMockBytes,
    ) -> Result<Self, Error> {
        if let Some(uri_scheme) = uri.scheme()
            && uri_scheme != &scheme
        {
            return Err(RequestConversionError(format!(
                "request scheme {scheme} does not match URI scheme {uri_scheme}"
            )));
        }

        let authority = match (uri.authority(), headers.get(http::header::HOST)) {
            (Some(_), _) => RequestAuthority::Uri,
            (None, Some(value)) => RequestAuthority::Host(
                Authority::try_from(value.as_bytes())
                    .map_err(|err| RequestConversionError(format!("invalid request authority: {err}")))?,
            ),
            (None, None) => RequestAuthority::None,
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
        self.uri.scheme().unwrap_or(&self.scheme)
    }

    /// Returns the request authority from the URI or `Host` header.
    pub fn authority(&self) -> Option<&Authority> {
        match &self.authority {
            RequestAuthority::Uri => self.uri.authority(),
            RequestAuthority::Host(authority) => Some(authority),
            RequestAuthority::None => None,
        }
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

#[derive(Serialize, Deserialize)]
struct HttpMockRequestWire {
    scheme: String,
    uri: String,
    method: String,
    headers: Vec<(String, String)>,
    version: String,
    body: HttpMockBytes,
}

impl TryFrom<&HttpMockRequest> for HttpMockRequestWire {
    type Error = Error;

    fn try_from(request: &HttpMockRequest) -> Result<Self, Self::Error> {
        let headers = request
            .headers
            .iter()
            .map(|(name, value)| {
                let value = value.to_str().map_err(|err| RequestConversionError(err.to_string()))?;
                Ok((name.as_str().to_string(), value.to_string()))
            })
            .collect::<Result<_, Error>>()?;

        Ok(Self {
            scheme: request.scheme().as_str().to_string(),
            uri: request.uri.to_string(),
            method: request.method.as_str().to_string(),
            headers,
            version: format!("{:?}", request.version),
            body: request.body.clone(),
        })
    }
}

impl TryFrom<HttpMockRequestWire> for HttpMockRequest {
    type Error = Error;

    fn try_from(request: HttpMockRequestWire) -> Result<Self, Self::Error> {
        let scheme = Scheme::from_str(&request.scheme)
            .map_err(|err| RequestConversionError(format!("invalid scheme: {err}")))?;
        let uri = Uri::from_str(&request.uri).map_err(|err| RequestConversionError(format!("invalid URI: {err}")))?;
        let method = HttpMethod::from_bytes(request.method.as_bytes())
            .map_err(|err| RequestConversionError(format!("invalid method: {err}")))?;
        let version = parse_http_version(&request.version)?;

        let mut headers = HeaderMap::with_capacity(request.headers.len());
        for (name, value) in request.headers {
            let name = http::HeaderName::from_bytes(name.as_bytes())
                .map_err(|err| RequestConversionError(format!("invalid header name: {err}")))?;
            let value = HeaderValue::from_str(&value)
                .map_err(|err| RequestConversionError(format!("invalid header value: {err}")))?;
            headers.append(name, value);
        }

        Self::from_parts(scheme, uri, method, headers, version, request.body)
    }
}

impl Serialize for HttpMockRequest {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        HttpMockRequestWire::try_from(self)
            .map_err(serde::ser::Error::custom)?
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

fn parse_http_version(version: &str) -> Result<Version, Error> {
    match version {
        "HTTP/0.9" => Ok(Version::HTTP_09),
        "HTTP/1.0" => Ok(Version::HTTP_10),
        "HTTP/1.1" => Ok(Version::HTTP_11),
        "HTTP/2.0" | "HTTP/2" => Ok(Version::HTTP_2),
        "HTTP/3.0" | "HTTP/3" => Ok(Version::HTTP_3),
        _ => Err(RequestConversionError(format!("unknown HTTP version: {version}"))),
    }
}

fn request_scheme<B>(request: &http::Request<B>) -> Result<Scheme, Error> {
    if let Some(scheme) = request.uri().scheme() {
        return Ok(scheme.clone());
    }

    let metadata = request
        .extensions()
        .get::<RequestMetadata>()
        .ok_or_else(|| RequestConversionError("request has no URI scheme or transport metadata".to_string()))?;

    Ok(metadata.scheme.clone())
}

impl<B> TryFrom<http::Request<B>> for HttpMockRequest
where
    B: Into<HttpMockBytes>,
{
    type Error = Error;

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
    type Error = Error;

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
    fn request_wire_format_rejects_non_utf8_headers() {
        let mut request = http::Request::builder()
            .uri("http://example.com/")
            .body(Bytes::new())
            .unwrap();
        request
            .headers_mut()
            .insert("x-binary", HeaderValue::from_bytes(&[0x80]).unwrap());
        let request = HttpMockRequest::try_from(request).unwrap();

        assert!(serde_json::to_string(&request).is_err());
    }

    #[test]
    fn typed_message_conversions_preserve_parts() {
        let request = http::Request::builder()
            .method(http::Method::PUT)
            .uri("http://example.com/resource")
            .header("x-test", "value")
            .body(HttpMockBytes::from(Bytes::from_static(b"request")))
            .unwrap();
        let request = HttpMockRequest::try_from(request).unwrap();
        let request: http::Request<Bytes> = request.into();

        assert_eq!(request.method(), http::Method::PUT);
        assert_eq!(request.headers()["x-test"], "value");
        assert_eq!(request.body(), &Bytes::from_static(b"request"));

        let mut response = http::Response::builder()
            .status(http::StatusCode::CREATED)
            .body(HttpMockBytes::from(Bytes::from_static(b"response")))
            .unwrap();
        response
            .headers_mut()
            .insert("x-binary", HeaderValue::from_bytes(&[0x80]).unwrap());
        let response: HttpMockResponse = response.into();
        let response: http::Response<Bytes> = response.into();

        assert_eq!(response.status(), http::StatusCode::CREATED);
        assert_eq!(response.headers()["x-binary"].as_bytes(), &[0x80]);
        assert_eq!(response.body(), &Bytes::from_static(b"response"));
    }
}

/// A complete response returned by a dynamic responder.
#[derive(Debug, Clone)]
pub struct HttpMockResponse {
    status: http::StatusCode,
    headers: HeaderMap,
    body: HttpMockBytes,
}

impl HttpMockResponse {
    /// Creates a response builder with status 200, no headers, and an empty body.
    pub fn builder() -> HttpMockResponseBuilder {
        HttpMockResponseBuilder::default()
    }

    /// Returns the response status.
    pub fn status(&self) -> http::StatusCode {
        self.status
    }

    /// Returns the response headers.
    pub fn headers(&self) -> &HeaderMap {
        &self.headers
    }

    /// Returns the response body.
    pub fn body(&self) -> &HttpMockBytes {
        &self.body
    }
}

impl From<HttpMockResponse> for http::Response<Bytes> {
    fn from(response: HttpMockResponse) -> Self {
        let mut result = http::Response::new(response.body.into());
        *result.status_mut() = response.status;
        *result.headers_mut() = response.headers;
        result
    }
}

impl<B> From<http::Response<B>> for HttpMockResponse
where
    B: Into<HttpMockBytes>,
{
    fn from(response: http::Response<B>) -> Self {
        let (parts, body) = response.into_parts();
        Self {
            status: parts.status,
            headers: parts.headers,
            body: body.into(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct HttpMockResponseBuilder {
    status: http::StatusCode,
    headers: HeaderMap,
    body: HttpMockBytes,
}

impl Default for HttpMockResponseBuilder {
    fn default() -> Self {
        Self {
            status: http::StatusCode::OK,
            headers: HeaderMap::new(),
            body: HttpMockBytes::from(Bytes::new()),
        }
    }
}

impl HttpMockResponseBuilder {
    /// Sets the HTTP status.
    pub fn status(mut self, status: http::StatusCode) -> Self {
        self.status = status;
        self
    }

    /// Appends a header. Existing values with the same name are preserved.
    pub fn header(mut self, name: http::HeaderName, value: HeaderValue) -> Self {
        self.headers.append(name, value);
        self
    }

    /// Replaces all response headers.
    pub fn headers(mut self, headers: HeaderMap) -> Self {
        self.headers = headers;
        self
    }

    /// Sets the response body.
    pub fn body<B>(mut self, body: B) -> Self
    where
        B: Into<HttpMockBytes>,
    {
        self.body = body.into();
        self
    }

    /// Builds the response.
    pub fn build(self) -> HttpMockResponse {
        HttpMockResponse {
            status: self.status,
            headers: self.headers,
            body: self.body,
        }
    }
}

/// A serializable partial response configuration for static mocks.
#[derive(Serialize, Deserialize, Clone)]
pub struct MockServerHttpResponse {
    pub status: Option<u16>,
    pub headers: Option<Vec<(String, String)>>,
    #[serde(default, with = "opt_vector_serde_base64")]
    pub body: Option<HttpMockBytes>,
    pub delay: Option<u64>,
    #[serde(skip)]
    pub respond_with: Option<ResponseCallback>,
}

impl MockServerHttpResponse {
    pub(crate) fn new() -> Self {
        Self {
            status: None,
            headers: None,
            body: None,
            delay: None,
            respond_with: None,
        }
    }
}

impl Default for MockServerHttpResponse {
    fn default() -> Self {
        Self::new()
    }
}

impl TryFrom<MockServerHttpResponse> for HttpMockResponse {
    type Error = Error;

    fn try_from(response: MockServerHttpResponse) -> Result<Self, Self::Error> {
        let status = http::StatusCode::from_u16(response.status.unwrap_or(http::StatusCode::OK.as_u16()))
            .map_err(|err| Error::ResponseConversionError(err.to_string()))?;

        let mut headers = HeaderMap::new();
        for (name, value) in response.headers.unwrap_or_default() {
            let name = http::HeaderName::from_bytes(name.as_bytes())
                .map_err(|err| Error::ResponseConversionError(err.to_string()))?;
            let value = HeaderValue::from_str(&value).map_err(|err| Error::ResponseConversionError(err.to_string()))?;
            headers.append(name, value);
        }

        Ok(Self {
            status,
            headers,
            body: response.body.unwrap_or_else(|| HttpMockBytes::from(Bytes::new())),
        })
    }
}

impl TryFrom<&http::Response<Bytes>> for MockServerHttpResponse {
    type Error = Error;

    fn try_from(value: &http::Response<Bytes>) -> Result<Self, Self::Error> {
        let mut headers = Vec::with_capacity(value.headers().len());

        for (key, value) in value.headers() {
            let value = value
                .to_str()
                .map_err(|err| HeaderDeserializationError(err.to_string()))?;

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

/// A general abstraction of an HTTP request for all handlers.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct HttpMockRegex(#[serde(with = "serde_regex")] pub regex::Regex);

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

impl RequestRequirements {
    pub fn new() -> Self {
        Self::default()
    }
}

/// A Request that is made to set a new mock.
#[derive(Serialize, Deserialize, Clone)]
pub struct MockDefinition {
    pub request: RequestRequirements,
    pub response: MockServerHttpResponse,
}

impl MockDefinition {
    pub fn new(req: RequestRequirements, mock: MockServerHttpResponse) -> Self {
        Self {
            request: req,
            response: mock,
        }
    }
}

#[derive(Serialize, Deserialize, Clone)]
pub struct ActiveMock {
    pub id: usize,
    pub call_counter: usize,
    pub definition: MockDefinition,
    pub is_static: bool,
}

impl ActiveMock {
    pub fn new(id: usize, definition: MockDefinition, call_counter: usize, is_static: bool) -> Self {
        ActiveMock {
            id,
            definition,
            call_counter,
            is_static,
        }
    }
}

#[derive(Serialize, Deserialize, Clone)]
pub struct ActiveForwardingRule {
    pub id: usize,
    pub config: ForwardingRuleConfig,
}

impl ActiveForwardingRule {
    pub fn new(id: usize, config: ForwardingRuleConfig) -> Self {
        ActiveForwardingRule { id, config }
    }
}

#[derive(Serialize, Deserialize, Clone)]
pub struct ActiveProxyRule {
    pub id: usize,
    pub config: ProxyRuleConfig,
}

impl ActiveProxyRule {
    pub fn new(id: usize, config: ProxyRuleConfig) -> Self {
        ActiveProxyRule { id, config }
    }
}

#[derive(Serialize, Deserialize, Clone)]
pub struct ActiveRecording {
    pub id: usize,
    pub config: RecordingRuleConfig,
    pub mocks: Vec<MockDefinition>,
}

impl ActiveRecording {
    pub fn new(id: usize, config: RecordingRuleConfig) -> Self {
        ActiveRecording {
            id,
            config,
            mocks: vec![],
        }
    }
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

#[derive(Serialize, Deserialize, Clone, Default)]
pub struct RecordingRuleConfig {
    pub request_requirements: RequestRequirements,
    pub record_headers: Vec<String>,
    pub record_response_delays: bool,
}

#[derive(Serialize, Deserialize, Clone, Default)]
pub struct ProxyRuleConfig {
    pub request_requirements: RequestRequirements,
    pub request_header: Vec<(String, String)>,
}

#[derive(Serialize, Deserialize, Clone, Default)]
pub struct ForwardingRuleConfig {
    pub target_base_url: String,
    pub request_requirements: RequestRequirements,
    pub request_header: Vec<(String, String)>,
}

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
            method = Some(Method::from_str(&method_str).map_err(|err| StaticMockConversionError(err.to_string()))?);
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
