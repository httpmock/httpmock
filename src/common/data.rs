extern crate serde_regex;

use std::{
    cmp::Ordering,
    convert::{TryFrom, TryInto},
    fmt,
    fmt::Debug,
    str::FromStr,
    sync::Arc,
};

use bytes::Bytes;
#[cfg(feature = "cookies")]
use headers::{Cookie, HeaderMapExt};
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub type ResponseCallback = Arc<dyn Fn(&HttpMockRequest) -> HttpMockResponse + Send + Sync>;
pub type RequestPredicate = Arc<dyn Fn(&HttpMockRequest) -> bool + Send + Sync>;

use crate::{
    common::{
        data::Error::{HeaderDeserialization, RequestConversion},
        util::HttpMockBytes,
    },
    server::{RequestMetadata, matchers::generic::MatchingStrategy},
};

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("Cannot deserialize header: {0}")]
    HeaderDeserialization(String),
    #[error("cannot convert to/from static mock: {0}")]
    StaticMockConversion(String),
    #[error("Cannot convert request to/from internal structure: {0}")]
    RequestConversion(String),
    #[error("Response conversion error: {0}")]
    ResponseConversion(String),
}

/// A general abstraction of an HTTP request of `httpmock`.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct HttpMockRequest {
    scheme: String,
    uri: String,
    method: String,
    headers: Vec<(String, String)>,
    version: String,
    body: HttpMockBytes,
}

impl HttpMockRequest {
    pub(crate) fn new(
        scheme: String,
        uri: String,
        method: String,
        headers: Vec<(String, String)>,
        version: String,
        body: HttpMockBytes,
    ) -> Self {
        // TODO: Many fields from the struct are exposed as structures from http package to the user.
        //  These values here are also converted to these http crate structures every call.
        //  ==> Convert these values here into http crate structures and allow returning an error
        //      here instead of "unwrap" all the time later (see functions below).
        //      Convert into http crate structures once here and store the converted
        //          values in the struct instance here rather than only String values everywhere.
        //     This will require to make the HttpMockRequest serde compatible
        //     (http types are not serializable by default).
        Self {
            scheme,
            uri,
            method,
            headers,
            version,
            body,
        }
    }

    /// Parses and returns the URI of the request.
    ///
    /// # Attention
    ///
    /// - This method returns the full URI of the request as an `http::Uri` object.
    /// - The URI returned by this method does not include the `Host` part. In HTTP/1.1,
    ///   the request line typically contains only the path and query, not the full URL with the host.
    /// - To retrieve the host, you should use the `HttpMockRequest::host` method which extracts the `Host`
    ///   header (for HTTP/1.1) or the `:authority` pseudo-header (for HTTP/2 and HTTP/3).
    ///
    /// # Returns
    ///
    /// An `http::Uri` object representing the full URI of the request.
    pub fn uri(&self) -> http::Uri {
        self.uri.parse().unwrap()
    }

    /// Parses the scheme from the request.
    ///
    /// This function extracts the scheme (protocol) used in the request. If the request contains a relative path,
    /// the scheme will be inferred based on how the server received the request. For instance, if the request was
    /// sent to the server using HTTPS, the scheme will be set to "https"; otherwise, it will be set to "http".
    ///
    /// # Returns
    ///
    /// A `String` representing the scheme of the request, either "https" or "http".
    pub fn scheme(&self) -> String {
        let uri = self.uri();
        if let Some(scheme) = uri.scheme() {
            return scheme.to_string();
        }

        self.scheme.clone()
    }

    /// Returns the URI of the request as a string slice.
    ///
    /// # Attention
    ///
    /// - This method returns the full URI as a string slice.
    /// - The URI string returned by this method does not include the `Host` part. In HTTP/1.1,
    ///   the request line typically contains only the path and query, not the full URL with the host.
    /// - To retrieve the host, you should use the `host` method which extracts the `Host`
    ///   header (for HTTP/1.1) or the `:authority` pseudo-header (for HTTP/2 and HTTP/3).
    ///
    /// # Returns
    ///
    /// A string slice representing the full URI of the request.
    pub fn uri_str(&self) -> &str {
        self.uri.as_ref()
    }

    /// Returns the host that the request was sent to, based on the `Host` header or `:authority` pseudo-header.
    ///
    /// # Attention
    ///
    /// - This method retrieves the host from the `Host` header of the HTTP request for HTTP/1.1 requests.
    ///   For HTTP/2 and HTTP/3 requests, it retrieves the host from the `:authority` pseudo-header.
    /// - If you use the `HttpMockRequest::uri` method to get the full URI, note that
    ///   the URI might not include the host part. In HTTP/1.1, the request line
    ///   typically contains only the path and query, not the full URL.
    ///
    /// # Returns
    ///
    /// An `Option<String>` containing the host if the `Host` header or `:authority` pseudo-header is present, or
    /// `None` if neither is found.
    pub fn host(&self) -> Option<String> {
        // Check the Host header first (HTTP 1.1)
        if let Some((_, host)) = self.headers.iter().find(|(k, _)| k.eq_ignore_ascii_case("host")) {
            return Some(host.split(':').next().unwrap().to_string());
        }

        // If Host header is not found, check the URI authority (HTTP/2 and HTTP/3)
        let uri = self.uri();
        if let Some(authority) = uri.authority() {
            return Some(authority.as_str().split(':').next().unwrap().to_string());
        }

        None
    }

    /// Returns the port that the request was sent to, based on the `Host` header or `:authority` pseudo-header.
    ///
    /// # Attention
    ///
    /// 1. This method retrieves the port from the `Host` header of the HTTP request for HTTP/1.1 requests.
    ///    For HTTP/2 and HTTP/3 requests, it retrieves the port from the `:authority` pseudo-header.
    ///    This method attempts to parse the port as a `u16`. If the port cannot be parsed as a `u16`, this method will continue as if the port was not specified (see point 2).
    /// 2. If the port is not specified in the `Host` header or `:authority` pseudo-header, this method will return 443 (https) or 80 (http) based on the used scheme.
    ///
    /// # Returns
    ///
    /// An `u16` containing the port if the `Host` header or `:authority` pseudo-header is present and includes a valid port,
    /// or 443 (https) or 80 (http) based on the used scheme otherwise.
    pub fn port(&self) -> u16 {
        // Check the Host header first (HTTP 1.1)
        if let Some((_, host)) = self.headers.iter().find(|(k, _)| k.eq_ignore_ascii_case("host"))
            && let Some(port_str) = host.split(':').nth(1)
            && let Ok(port) = port_str.parse::<u16>()
        {
            return port;
        }

        // If Host header is not found, check the URI authority (HTTP/2 and HTTP/3)
        let uri = self.uri();
        if let Some(authority) = uri.authority()
            && let Some(port_str) = authority.as_str().split(':').nth(1)
            && let Ok(port) = port_str.parse::<u16>()
        {
            return port;
        }

        if self.scheme().eq("https") {
            return 443;
        }

        80
    }

    pub fn method(&self) -> http::Method {
        http::Method::from_bytes(self.method.as_bytes()).unwrap()
    }

    pub fn method_str(&self) -> &str {
        self.method.as_ref()
    }

    pub fn headers(&self) -> http::HeaderMap<http::HeaderValue> {
        let mut header_map: http::HeaderMap<http::HeaderValue> = http::HeaderMap::new();
        for (key, value) in &self.headers {
            let header_name = http::HeaderName::from_bytes(key.as_bytes()).unwrap();
            let header_value = http::HeaderValue::from_str(value).unwrap();

            header_map.append(header_name, header_value);
        }

        header_map
    }

    pub fn headers_vec(&self) -> &Vec<(String, String)> {
        self.headers.as_ref()
    }

    pub fn query_params(&self) -> Vec<(String, String)> {
        form_urlencoded::parse(self.uri().query().unwrap_or("").as_bytes())
            .into_owned()
            .collect()
    }

    pub fn query_param_length(&self) -> usize {
        form_urlencoded::parse(self.uri().query().unwrap_or("").as_bytes()).count()
    }

    pub fn body(&self) -> &HttpMockBytes {
        &self.body
    }

    pub fn body_string(&self) -> String {
        self.body.to_string()
    }

    pub fn body_ref(&self) -> &[u8] {
        self.body.as_ref()
    }

    // Move all body functions to HttpMockBytes
    pub fn body_vec(&self) -> Vec<u8> {
        self.body.to_vec()
    }

    pub fn body_bytes(&self) -> bytes::Bytes {
        self.body.to_bytes()
    }

    pub fn version(&self) -> http::Version {
        match self.version.as_ref() {
            "HTTP/0.9" => http::Version::HTTP_09,
            "HTTP/1.0" => http::Version::HTTP_10,
            "HTTP/1.1" => http::Version::HTTP_11,
            "HTTP/2.0" => http::Version::HTTP_2,
            "HTTP/3.0" => http::Version::HTTP_3,
            // Attention: This scenario is highly unlikely, so we panic here for the users
            // convenience (user does not need to deal with errors for this reason alone).
            _ => panic!("unknown HTTP version: {:?}", self.version),
        }
    }

    pub fn version_ref(&self) -> &str {
        self.version.as_ref()
    }

    #[cfg(feature = "cookies")]
    pub(crate) fn cookies(&self) -> Result<Vec<(String, String)>, Error> {
        let mut result = Vec::new();

        if let Some(cookie) = self.headers().typed_get::<Cookie>() {
            for (key, value) in cookie.iter() {
                result.push((key.to_string(), value.to_string()));
            }
        }

        Ok(result)
    }
}

fn http_headers_to_vec<T>(req: &http::Request<T>) -> Result<Vec<(String, String)>, Error> {
    req.headers()
        .iter()
        .map(|(name, value)| {
            // Attempt to convert the HeaderValue to a &str, returning an error if it fails.
            let value_str = value.to_str().map_err(|e| RequestConversion(e.to_string()))?;
            Ok((name.as_str().to_string(), value_str.to_string()))
        })
        .collect()
}

impl<B> TryFrom<&http::Request<B>> for HttpMockRequest
where
    B: Clone + IntoMockBytes,
{
    type Error = Error;

    fn try_from(value: &http::Request<B>) -> Result<Self, Self::Error> {
        let metadata = value
            .extensions()
            .get::<RequestMetadata>()
            .unwrap_or_else(|| panic!("request metadata was not added to the request"));

        let headers = http_headers_to_vec(value)?;

        // Convert the (cloned) body into Bytes, supporting several common body types.
        let body_bytes = value.body().clone().into_httpmock_bytes()?;
        let body = HttpMockBytes(body_bytes);

        Ok(HttpMockRequest::new(
            metadata.scheme.to_string(),
            value.uri().to_string(),
            value.method().to_string(),
            headers,
            format!("{:?}", value.version()),
            body,
        ))
    }
}

impl<B> From<http::Request<B>> for HttpMockRequest
where
    B: Clone + IntoMockBytes,
{
    fn from(req: http::Request<B>) -> Self {
        // Use by-ref conversion; we still have access to extensions while owning `req`.
        <HttpMockRequest as TryFrom<&http::Request<B>>>::try_from(&req)
            .expect("invalid http::Request for HttpMockRequest: missing metadata or invalid headers/body")
    }
}

impl From<&HttpMockRequest> for http::Request<bytes::Bytes> {
    fn from(req: &HttpMockRequest) -> Self {
        let mut builder = http::Request::builder()
            .method(req.method())
            .uri(req.uri())
            .version(req.version());

        for (k, v) in req.headers() {
            builder = builder.header(k.map_or(String::new(), |v| v.to_string()), v)
        }

        builder
            .body(req.body().to_bytes())
            .expect("failed to convert HttpMockRequest into http::Request<Bytes>")
    }
}

impl From<&HttpMockRequest> for http::Request<String> {
    fn from(req: &HttpMockRequest) -> Self {
        let mut builder = http::Request::builder()
            .method(req.method())
            .uri(req.uri())
            .version(req.version());

        for (k, v) in req.headers() {
            builder = builder.header(k.map_or(String::new(), |v| v.to_string()), v)
        }

        let body = String::from_utf8(req.body_vec()).expect("request body is not valid UTF-8");
        builder
            .body(body)
            .expect("failed to convert HttpMockRequest into http::Request<String>")
    }
}

impl From<&HttpMockRequest> for http::Request<()> {
    fn from(req: &HttpMockRequest) -> Self {
        let mut builder = http::Request::builder()
            .method(req.method())
            .uri(req.uri())
            .version(req.version());

        for (k, v) in req.headers() {
            builder = builder.header(k.map_or(String::new(), |v| v.to_string()), v)
        }

        builder
            .body(())
            .expect("failed to convert HttpMockRequest into http::Request<()>")
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

/// Normalizes various response body types into `bytes::Bytes`.
/// Used by the blanket implementation `TryFrom<http::Response<B>> for HttpMockResponse`.
/// Implementations prefer zero-copy where possible (e.g., `bytes::Bytes` clones the Arc).
/// Returns `Result` to allow fallible conversions if needed in the future.
pub trait IntoMockBytes {
    fn into_httpmock_bytes(self) -> Result<bytes::Bytes, Error>;
}

impl IntoMockBytes for bytes::Bytes {
    fn into_httpmock_bytes(self) -> Result<bytes::Bytes, Error> {
        // Zero-copy-ish: Bytes is ref-counted; this just clones the handle.
        Ok(self)
    }
}

impl IntoMockBytes for Vec<u8> {
    fn into_httpmock_bytes(self) -> Result<bytes::Bytes, Error> {
        Ok(bytes::Bytes::from(self))
    }
}

impl IntoMockBytes for String {
    fn into_httpmock_bytes(self) -> Result<bytes::Bytes, Error> {
        Ok(bytes::Bytes::from(self.into_bytes()))
    }
}

impl IntoMockBytes for &'static str {
    fn into_httpmock_bytes(self) -> Result<bytes::Bytes, Error> {
        Ok(bytes::Bytes::from(self))
    }
}

impl IntoMockBytes for Box<[u8]> {
    fn into_httpmock_bytes(self) -> Result<bytes::Bytes, Error> {
        Ok(bytes::Bytes::from(self))
    }
}

impl IntoMockBytes for std::borrow::Cow<'_, [u8]> {
    fn into_httpmock_bytes(self) -> Result<bytes::Bytes, Error> {
        Ok(match self {
            std::borrow::Cow::Borrowed(b) => bytes::Bytes::copy_from_slice(b),
            std::borrow::Cow::Owned(v) => bytes::Bytes::from(v),
        })
    }
}

impl IntoMockBytes for std::borrow::Cow<'_, str> {
    fn into_httpmock_bytes(self) -> Result<bytes::Bytes, Error> {
        Ok(match self {
            std::borrow::Cow::Borrowed(s) => bytes::Bytes::copy_from_slice(s.as_bytes()),
            std::borrow::Cow::Owned(s) => bytes::Bytes::from(s.into_bytes()),
        })
    }
}

// Support empty bodies like `http::Response::builder().body(())` used in tests
impl IntoMockBytes for () {
    fn into_httpmock_bytes(self) -> Result<bytes::Bytes, Error> {
        Ok(bytes::Bytes::new())
    }
}

impl<B> TryFrom<&http::Response<B>> for HttpMockResponse
where
    B: Clone + IntoMockBytes, // Clone only if you need to read body/headers without moving
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

        // Body: need a `B` value. Since we only have `&Response<B>`, either:
        //  - require `B: Clone` and clone it, or
        //  - restrict this impl to specific `B` you can borrow from (e.g., Bytes)
        let body_bytes = resp.body().clone().into_httpmock_bytes()?;

        Ok(HttpMockResponse {
            status: Some(resp.status().as_u16()),
            headers: Some(headers),
            body: Some(HttpMockBytes(body_bytes)),
        })
    }
}

impl<B> From<http::Response<B>> for HttpMockResponse
where
    B: Clone + IntoMockBytes,
{
    fn from(resp: http::Response<B>) -> Self {
        // Avoid recursive TryFrom<http::Response<B>> derived from this From impl.
        // Convert by reference using the blanket &Response<B> implementation.
        <HttpMockResponse as TryFrom<&http::Response<B>>>::try_from(&resp)
            .expect("invalid http::Response for HttpMockResponse")
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
