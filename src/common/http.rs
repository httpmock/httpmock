use async_trait::async_trait;
use bytes::Bytes;
use http::{Request, Response};
use http_body_util::{BodyExt, Full};
#[cfg(all(not(target_arch = "wasm32"), any(feature = "remote-https", feature = "https")))]
use hyper_rustls::HttpsConnector;
#[cfg(not(target_arch = "wasm32"))]
use hyper_util::{
    client::legacy::{connect::HttpConnector, Client},
    rt::TokioExecutor,
};
use std::{convert::TryInto, sync::Arc};
use thiserror::Error;
#[cfg(not(target_arch = "wasm32"))]
use tokio::runtime::Runtime;

#[cfg(feature = "server")]
use crate::server::RequestMetadata;

#[derive(Error, Debug)]
pub enum Error {
    #[cfg(not(target_arch = "wasm32"))]
    #[error("cannot send request: {0}")]
    HyperError(#[from] hyper::Error),
    #[cfg(not(target_arch = "wasm32"))]
    #[error("cannot send request: {0}")]
    HyperUtilError(#[from] hyper_util::client::legacy::Error),
    #[cfg(not(target_arch = "wasm32"))]
    #[error("runtime error: {0}")]
    RuntimeError(#[from] tokio::task::JoinError),
    #[cfg(target_arch = "wasm32")]
    #[error("request error: {0}")]
    RequestError(String),
    #[error("unknown error")]
    Unknown,
}

#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
pub trait HttpClient {
    async fn send(&self, req: Request<Bytes>) -> Result<Response<Bytes>, Error>;
}

#[cfg(not(target_arch = "wasm32"))]
pub struct HttpMockHttpClient {
    runtime: Option<Arc<Runtime>>,
    #[cfg(any(feature = "remote-https", feature = "https"))]
    client: Arc<Client<HttpsConnector<HttpConnector>, Full<Bytes>>>,
    #[cfg(not(any(feature = "remote-https", feature = "https")))]
    client: Arc<Client<HttpConnector, Full<Bytes>>>,
}

#[cfg(not(target_arch = "wasm32"))]
impl<'a> HttpMockHttpClient {
    #[cfg(any(feature = "remote-https", feature = "https"))]
    pub fn new(runtime: Option<Arc<Runtime>>) -> Self {
        // see https://github.com/rustls/rustls/issues/1938
        if rustls::crypto::CryptoProvider::get_default().is_none() {
            rustls::crypto::ring::default_provider()
                .install_default()
                .expect("cannot install rustls crypto provider");
        }

        let builder = hyper_rustls::HttpsConnectorBuilder::new()
            .with_native_roots()
            .expect("cannot set up using native root certificates")
            .https_or_http()
            .enable_http1();

        #[cfg(feature = "http2")]
        let builder = builder.enable_http2();

        let https_connector = builder.build();

        Self {
            runtime,
            client: Arc::new(Client::builder(TokioExecutor::new()).build(https_connector)),
        }
    }

    #[cfg(not(any(feature = "remote-https", feature = "https")))]
    pub fn new(runtime: Option<Arc<Runtime>>) -> Self {
        Self {
            runtime,
            client: Arc::new(Client::builder(TokioExecutor::new()).build(HttpConnector::new())),
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
#[async_trait]
impl HttpClient for HttpMockHttpClient {
    async fn send(&self, req: Request<Bytes>) -> Result<Response<Bytes>, Error> {
        let (mut req_parts, req_body) = req.into_parts();

        // If the request is origin-form or incomplete, reconstruct an absolute URI
        let uri = req_parts.uri.clone();

        let needs_target = uri.scheme().is_none() || uri.authority().is_none();
        if needs_target {
            if let Some(host) = req_parts
                .headers
                .get(http::header::HOST)
                .and_then(|v| v.to_str().ok())
            {
                // Prefer scheme from the URI if present; otherwise use RequestMetadata; fallback to http
                let scheme = {
                    let from_uri = uri.scheme_str();
                    #[cfg(feature = "server")]
                    let from_meta = req_parts
                        .extensions
                        .get::<RequestMetadata>()
                        .map(|m| m.scheme);
                    #[cfg(not(feature = "server"))]
                    let from_meta: Option<&str> = None;
                    from_uri.or(from_meta).unwrap_or("http")
                };

                let path_and_query = uri.path_and_query().map(|pq| pq.as_str()).unwrap_or("/");

                if let Ok(new_uri) = format!("{}://{}{}", scheme, host, path_and_query).parse() {
                    req_parts.uri = new_uri;
                }
            }
        }

        // Remove Host header and let hyper set it (HTTP/1.1) or :authority (HTTP/2)
        req_parts.headers.remove(http::header::HOST);
        let hyper_req = Request::from_parts(req_parts, Full::new(req_body));

        let res = if let Some(rt) = self.runtime.clone() {
            let client = self.client.clone();
            rt.spawn(async move { client.request(hyper_req).await })
                .await??
        } else {
            self.client.request(hyper_req).await?
        };

        let (res_parts, res_body) = res.into_parts();
        let body = res_body.collect().await?.to_bytes();

        Ok(Response::from_parts(res_parts, body))
    }
}


#[cfg(all(target_arch = "wasm32", not(target_os = "wasi")))]
pub struct HttpMockHttpClient {
    client: reqwest::Client,
}

#[cfg(all(target_arch = "wasm32", not(target_os = "wasi")))]
impl HttpMockHttpClient {
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::new(),
        }
    }
}

#[cfg(all(target_arch = "wasm32", not(target_os = "wasi")))]
#[async_trait(?Send)]
impl HttpClient for HttpMockHttpClient {
    async fn send(&self, req: Request<Bytes>) -> Result<Response<Bytes>, Error> {
        use reqwest::Method as ReqwestMethod;

        let (mut parts, body) = req.into_parts();

        // Ensure absolute URL (similar to non-wasm path)
        let uri = parts.uri.clone();
        let needs_target = uri.scheme().is_none() || uri.authority().is_none();
        if needs_target {
            if let Some(host) = parts
                .headers
                .get(http::header::HOST)
                .and_then(|v| v.to_str().ok())
            {
                let scheme = uri.scheme_str().unwrap_or("http");
                let path_and_query = uri.path_and_query().map(|pq| pq.as_str()).unwrap_or("/");
                if let Ok(new_uri) = format!("{}://{}{}", scheme, host, path_and_query).parse() {
                    parts.uri = new_uri;
                }
            }
        }

        // Build reqwest request
        let method = ReqwestMethod::from_bytes(parts.method.as_str().as_bytes())
            .map_err(|e| Error::RequestError(e.to_string()))?;
        let url = parts.uri.to_string();
        let mut rb = self.client.request(method, url);

        // Transfer headers (excluding Host; browser/fetch sets it)
        let mut hdrs = reqwest::header::HeaderMap::new();
        for (name, value) in parts.headers.iter() {
            if name == http::header::HOST { continue; }
            // reqwest uses the same header types; clone is fine
            hdrs.insert(
                reqwest::header::HeaderName::from_bytes(name.as_str().as_bytes())
                    .map_err(|e| Error::RequestError(e.to_string()))?,
                reqwest::header::HeaderValue::from_bytes(value.as_bytes())
                    .map_err(|e| Error::RequestError(e.to_string()))?,
            );
        }
        rb = rb.headers(hdrs);

        // Body
        rb = rb.body(body.to_vec());

        let resp = rb
            .send()
            .await
            .map_err(|e| Error::RequestError(e.to_string()))?;

        let status = resp.status();
        let resp_headers = resp.headers().clone();
        let bytes = resp
            .bytes()
            .await
            .map_err(|e| Error::RequestError(e.to_string()))?;

        // Build http::Response
        let mut builder = http::Response::builder().status(status);
        {
            let headers_mut = builder.headers_mut().unwrap();
            for (name, value) in resp_headers.iter() {
                // reqwest uses http::HeaderName/Value, so we can clone directly
                headers_mut.insert(name.clone(), value.clone());
            }
        }

        Ok(builder
            .body(Bytes::from(bytes.to_vec()))
            .map_err(|e| Error::RequestError(e.to_string()))?)
    }
}

// WASI-specific implementation using wasi-http-client
#[cfg(all(target_arch = "wasm32", target_os = "wasi"))]
pub struct HttpMockHttpClient {
    client: wasi_http_client::Client,
}

#[cfg(all(target_arch = "wasm32", target_os = "wasi"))]
impl HttpMockHttpClient {
    pub fn new() -> Self {
        Self { client: wasi_http_client::Client::new() }
    }
}

#[cfg(all(target_arch = "wasm32", target_os = "wasi"))]
#[async_trait(?Send)]
impl HttpClient for HttpMockHttpClient {
    async fn send(&self, req: Request<Bytes>) -> Result<Response<Bytes>, Error> {
        let (mut parts, body) = req.into_parts();

        // Ensure absolute URL (similar to non-wasi path)
        let uri = parts.uri.clone();
        let needs_target = uri.scheme().is_none() || uri.authority().is_none();
        if needs_target {
            if let Some(host) = parts
                .headers
                .get(http::header::HOST)
                .and_then(|v| v.to_str().ok())
            {
                let scheme = uri.scheme_str().unwrap_or("http");
                let path_and_query = uri.path_and_query().map(|pq| pq.as_str()).unwrap_or("/");
                if let Ok(new_uri) = format!("{}://{}{}", scheme, host, path_and_query).parse() {
                    parts.uri = new_uri;
                }
            }
        }

        let url = parts.uri.to_string();

        // Build wasi-http-client request by method
        // Note: wasi-http-client currently provides per-method builders
        let mut rb = match parts.method.as_str() {
            "GET" => self.client.get(&url),
            "POST" => self.client.post(&url),
            "PUT" => self.client.put(&url),
            "DELETE" => self.client.delete(&url),
            "PATCH" => self.client.patch(&url),
            // Some methods may not be supported by wasi-http-client; return clear error for others
            other => {
                return Err(Error::RequestError(format!(
                    "Unsupported HTTP method on WASI: {}",
                    other
                )));
            }
        };

        // Transfer headers (excluding Host)
        for (name, value) in parts.headers.iter() {
            if name == http::header::HOST { continue; }
            if let Ok(val_str) = value.to_str() {
                rb = rb.header(name.as_str(), val_str);
            }
        }

        // Body
        let body_vec = body.to_vec();
        if !body_vec.is_empty() {
            rb = rb.body(&body_vec);
        }

        // Send synchronously (API is blocking), within async fn is fine
        let resp = rb
            .send()
            .map_err(|e| Error::RequestError(e.to_string()))?;

        let status = resp.status();
        // Extract response body; default to empty on error
        let resp_body: Vec<u8> = resp.body().unwrap_or_default();

        // Build minimal http::Response
        let builder = http::Response::builder().status(status);
        Ok(builder
            .body(Bytes::from(resp_body))
            .map_err(|e| Error::RequestError(e.to_string()))?)
    }
}
