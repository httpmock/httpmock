use std::{net::SocketAddr, sync::Arc};

use bytes::Bytes;
use http::{Method, Request, StatusCode};
use serde::de::DeserializeOwned;

#[cfg(feature = "proxy")]
use crate::common::data::{ActiveForwardingRule, ActiveProxyRule, ForwardingRuleConfig, ProxyRuleConfig};
#[cfg(feature = "record")]
use crate::common::data::{ActiveRecording, RecordingRuleConfig};
use crate::{
    api::adapter::{
        MockServerAdapter, ServerAdapterError,
        ServerAdapterError::{
            InvalidMockDefinitionError, JsonDeserializationError, JsonSerializationError, UpstreamError,
        },
    },
    common::{
        data::{
            ActiveMock, ClosestMatch, MockDefinition, MockServerHttpResponse, RequestRequirements, UpToNTimesConfig,
        },
        http::HttpMockHttpClient,
    },
};

pub(in crate::api) struct RemoteMockServerAdapter {
    addr: SocketAddr,
    http_client: Arc<HttpMockHttpClient>,
}

impl RemoteMockServerAdapter {
    pub(in crate::api) fn new(addr: SocketAddr, http_client: Arc<HttpMockHttpClient>) -> Self {
        Self { addr, http_client }
    }

    fn validate_request_requirements(&self, requirements: &RequestRequirements) -> Result<(), ServerAdapterError> {
        match requirements.is_true {
            Some(_) => Err(InvalidMockDefinitionError(
                "Anonymous function request matchers are not supported when using a remote mock server".to_string(),
            )),
            None => Ok(()),
        }
    }

    fn validate_response(&self, response: &MockServerHttpResponse) -> Result<(), ServerAdapterError> {
        match response.respond_with {
            Some(_) => Err(InvalidMockDefinitionError(
                "Dynamic responders are not supported by remote/standalone servers".to_string(),
            )),
            None => Ok(()),
        }
    }

    /// Builds a request against the `__httpmock__` API. When `json_body` is
    /// `Some`, the payload is sent with a JSON content type.
    fn build_request(
        &self,
        method: Method,
        path: &str,
        json_body: Option<String>,
    ) -> Result<Request<Bytes>, ServerAdapterError> {
        let mut builder = Request::builder()
            .method(method)
            .uri(format!("http://{}/__httpmock__/{}", self.addr, path));

        let body = match json_body {
            Some(json) => {
                builder = builder.header("content-type", "application/json");
                Bytes::from(json)
            }
            None => Bytes::new(),
        };

        builder.body(body).map_err(|e| UpstreamError(e.to_string()))
    }

    /// Sends a request and returns the response body as a string, failing with
    /// an [`UpstreamError`] when the status does not match `expected_status`.
    /// `context` names the operation for diagnostic error messages.
    async fn send_checked(
        &self,
        request: Request<Bytes>,
        expected_status: StatusCode,
        context: &str,
    ) -> Result<String, ServerAdapterError> {
        let (status, body) = self.do_request(request).await?;

        if status != expected_status {
            return Err(UpstreamError(format!(
                "Could not {}. Expected response status {} but was {} (response body = '{}')",
                context,
                expected_status.as_u16(),
                status.as_u16(),
                body
            )));
        }

        Ok(body)
    }

    /// Sends a JSON request and deserializes the response body.
    async fn request_json<T: DeserializeOwned>(
        &self,
        method: Method,
        path: &str,
        json_body: Option<String>,
        expected_status: StatusCode,
        context: &str,
    ) -> Result<T, ServerAdapterError> {
        let request = self.build_request(method, path, json_body)?;
        let body = self.send_checked(request, expected_status, context).await?;
        serde_json::from_str(&body).map_err(JsonDeserializationError)
    }

    /// Sends a request that is expected to have no meaningful response body,
    /// only verifying the response status.
    async fn request_empty(
        &self,
        method: Method,
        path: &str,
        json_body: Option<String>,
        expected_status: StatusCode,
        context: &str,
    ) -> Result<(), ServerAdapterError> {
        let request = self.build_request(method, path, json_body)?;
        self.send_checked(request, expected_status, context).await?;
        Ok(())
    }

    async fn do_request(&self, req: Request<Bytes>) -> Result<(StatusCode, String), ServerAdapterError> {
        let (code, body_bytes) = self.do_request_raw(req).await?;

        let body = String::from_utf8(body_bytes.to_vec()).map_err(|e| UpstreamError(e.to_string()))?;

        Ok((code, body))
    }

    async fn do_request_raw(&self, req: Request<Bytes>) -> Result<(StatusCode, Bytes), ServerAdapterError> {
        let response = self
            .http_client
            .send(req)
            .await
            .map_err(|e| UpstreamError(e.to_string()))?;

        Ok((response.status(), response.body().clone()))
    }
}

impl MockServerAdapter for RemoteMockServerAdapter {
    fn host(&self) -> String {
        self.addr.ip().to_string()
    }

    fn port(&self) -> u16 {
        self.addr.port()
    }

    fn address(&self) -> &SocketAddr {
        &self.addr
    }

    async fn reset(&self) -> Result<(), ServerAdapterError> {
        self.request_empty(
            Method::DELETE,
            "state",
            None,
            StatusCode::NO_CONTENT,
            "reset the mock server",
        )
        .await
    }

    async fn create_mock(&self, mock: &MockDefinition) -> Result<ActiveMock, ServerAdapterError> {
        self.validate_request_requirements(&mock.request)?;
        self.validate_response(&mock.response)?;

        let json = serde_json::to_string(mock).map_err(JsonSerializationError)?;

        self.request_json(Method::POST, "mocks", Some(json), StatusCode::CREATED, "create mock")
            .await
    }

    async fn fetch_mock(&self, mock_id: usize) -> Result<ActiveMock, ServerAdapterError> {
        self.request_json(
            Method::GET,
            &format!("mocks/{}", mock_id),
            None,
            StatusCode::OK,
            "fetch mock from the mock server",
        )
        .await
    }

    async fn delete_mock(&self, mock_id: usize) -> Result<(), ServerAdapterError> {
        self.request_empty(
            Method::DELETE,
            &format!("mocks/{}", mock_id),
            None,
            StatusCode::NO_CONTENT,
            "delete mock from the mock server",
        )
        .await
    }

    async fn set_up_to_n_times(&self, mock_id: usize, count: usize) -> Result<(), ServerAdapterError> {
        let config = UpToNTimesConfig { up_to_n_times: count };
        let json = serde_json::to_string(&config).map_err(JsonSerializationError)?;
        let request = self.build_request(Method::PUT, &format!("mocks/{}/up_to_n_times", mock_id), Some(json))?;
        let (status, body) = self.do_request(request).await?;

        // Servers that predate `up_to_n_times` have no route for this request and serve it like
        // any other request. Check that the limit was stored, unless the server reported an error
        // such as a zero limit.
        if status != StatusCode::INTERNAL_SERVER_ERROR
            && let Ok(mock) = self.fetch_mock(mock_id).await
            && mock.up_to_n_times.is_none()
        {
            return Err(UpstreamError(
                "The mock server does not support up_to_n_times. Upgrade the mock server to use it.".to_string(),
            ));
        }

        if status != StatusCode::NO_CONTENT {
            return Err(UpstreamError(format!(
                "Could not set the up_to_n_times limit on the mock. Expected response status {} but was {} \
                 (response body = '{}')",
                StatusCode::NO_CONTENT.as_u16(),
                status.as_u16(),
                body
            )));
        }

        Ok(())
    }

    async fn verify(&self, requirements: &RequestRequirements) -> Result<Option<ClosestMatch>, ServerAdapterError> {
        let json = serde_json::to_string(requirements).map_err(JsonSerializationError)?;

        let request = self.build_request(Method::POST, "verify", Some(json))?;
        let (status, body) = self.do_request(request).await?;

        if status == StatusCode::NOT_FOUND {
            return Ok(None);
        }

        if status != StatusCode::OK {
            return Err(UpstreamError(format!(
                "Could not verify mock. Expected response status 200 but was {} (response body = '{}')",
                status.as_u16(),
                body
            )));
        }

        let response: ClosestMatch = serde_json::from_str(&body).map_err(JsonDeserializationError)?;

        Ok(Some(response))
    }

    #[cfg(feature = "proxy")]
    async fn create_forwarding_rule(
        &self,
        config: ForwardingRuleConfig,
    ) -> Result<ActiveForwardingRule, ServerAdapterError> {
        self.validate_request_requirements(&config.request_requirements)?;

        let json = serde_json::to_string(&config).map_err(JsonSerializationError)?;

        self.request_json(
            Method::POST,
            "forwarding_rules",
            Some(json),
            StatusCode::CREATED,
            "create forwarding rule",
        )
        .await
    }

    #[cfg(feature = "proxy")]
    async fn delete_forwarding_rule(&self, id: usize) -> Result<(), ServerAdapterError> {
        self.request_empty(
            Method::DELETE,
            &format!("forwarding_rules/{}", id),
            None,
            StatusCode::NO_CONTENT,
            "delete forwarding rule from the mock server",
        )
        .await
    }

    #[cfg(feature = "proxy")]
    async fn create_proxy_rule(&self, config: ProxyRuleConfig) -> Result<ActiveProxyRule, ServerAdapterError> {
        self.validate_request_requirements(&config.request_requirements)?;

        let json = serde_json::to_string(&config).map_err(JsonSerializationError)?;

        self.request_json(
            Method::POST,
            "proxy_rules",
            Some(json),
            StatusCode::CREATED,
            "create proxy rule",
        )
        .await
    }

    #[cfg(feature = "proxy")]
    async fn delete_proxy_rule(&self, id: usize) -> Result<(), ServerAdapterError> {
        self.request_empty(
            Method::DELETE,
            &format!("proxy_rules/{}", id),
            None,
            StatusCode::NO_CONTENT,
            "delete proxy rule from the mock server",
        )
        .await
    }

    #[cfg(feature = "record")]
    async fn create_recording(&self, config: RecordingRuleConfig) -> Result<ActiveRecording, ServerAdapterError> {
        self.validate_request_requirements(&config.request_requirements)?;

        let json = serde_json::to_string(&config).map_err(JsonSerializationError)?;

        self.request_json(
            Method::POST,
            "recordings",
            Some(json),
            StatusCode::CREATED,
            "create recording",
        )
        .await
    }

    #[cfg(feature = "record")]
    async fn delete_recording(&self, id: usize) -> Result<(), ServerAdapterError> {
        self.request_empty(
            Method::DELETE,
            &format!("recordings/{}", id),
            None,
            StatusCode::NO_CONTENT,
            "delete recording from the mock server",
        )
        .await
    }

    #[cfg(feature = "record")]
    async fn export_recording(&self, id: usize) -> Result<Option<Bytes>, ServerAdapterError> {
        let request = self.build_request(Method::GET, &format!("recordings/{}", id), None)?;

        let (status, body) = self.do_request_raw(request).await?;

        if status == StatusCode::NOT_FOUND {
            return Ok(None);
        } else if status != StatusCode::OK {
            return Err(UpstreamError(format!(
                "Could not export the recording from the mock server. Expected response status 200 but was {}",
                status.as_u16()
            )));
        }

        Ok(Some(body))
    }

    #[cfg(feature = "record")]
    async fn create_mocks_from_recording(
        &self,
        recording_file_content: &str,
    ) -> Result<Vec<usize>, ServerAdapterError> {
        // Note: this endpoint receives the raw recording file content and,
        // unlike the other POST calls, is intentionally sent without a JSON
        // content-type header.
        let request = Request::builder()
            .method(Method::POST)
            .uri(format!("http://{}/__httpmock__/recordings", self.addr))
            .body(Bytes::from(recording_file_content.to_owned()))
            .map_err(|e| UpstreamError(e.to_string()))?;

        let body = self
            .send_checked(request, StatusCode::OK, "create mocks from recording")
            .await?;

        serde_json::from_str(&body).map_err(JsonDeserializationError)
    }
}
