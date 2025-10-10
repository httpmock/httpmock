use crate::common::data::{ForwardingRuleConfig, ProxyRuleConfig, RecordingRuleConfig};
use std::{borrow::Borrow, net::SocketAddr, sync::Arc};

use crate::api::{
    adapter::{
        ServerAdapterError,
        ServerAdapterError::{
            InvalidMockDefinitionError, JsonDeserializationError, JsonSerializationError,
            UpstreamError,
        },
    },
    MockServerAdapter,
};
use async_trait::async_trait;
use bytes::Bytes;
use http::{Request, StatusCode};

use crate::common::{
    data::{
        ActiveForwardingRule, ActiveMock, ActiveProxyRule, ActiveRecording, ClosestMatch,
        MockDefinition, RequestRequirements,
    },
    http::HttpClient,
};

pub struct RemoteMockServerAdapter {
    addr: SocketAddr,
    http_client: Arc<dyn HttpClient + Send + Sync + 'static>,
}

impl RemoteMockServerAdapter {
    pub fn new(addr: SocketAddr, http_client: Arc<dyn HttpClient + Send + Sync + 'static>) -> Self {
        Self { addr, http_client }
    }

    fn validate_request_requirements(
        &self,
        requirements: &RequestRequirements,
    ) -> Result<(), ServerAdapterError> {
        match requirements.is_true {
            Some(_) => Err(InvalidMockDefinitionError(
                "Anonymous function request matchers are not supported when using a remote mock server".to_string(),
            )),
            None => Ok(()),
        }
    }

    async fn do_request(&self, req: Request<Bytes>) -> Result<(u16, String), ServerAdapterError> {
        let (code, body_bytes) = self.do_request_raw(req).await?;

        let body =
            String::from_utf8(body_bytes.to_vec()).map_err(|e| UpstreamError(e.to_string()))?;

        Ok((code, body))
    }

    async fn do_request_raw(
        &self,
        req: Request<Bytes>,
    ) -> Result<(u16, Bytes), ServerAdapterError> {
        let mut response = self
            .http_client
            .send(req)
            .await
            .map_err(|e| UpstreamError(e.to_string()))?;

        Ok((response.status().as_u16(), response.body().clone()))
    }
}

#[async_trait]
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
        let request = Request::builder()
            .method("DELETE")
            .uri(format!("http://{}/__httpmock__/state", &self.addr))
            .body(Bytes::new())
            .map_err(|e| UpstreamError(e.to_string()))?;

        let (status, body) = self.do_request(request).await?;

        if status != StatusCode::NO_CONTENT {
            return Err(UpstreamError(format!(
                "Could not reset the mock server. Expected response status 204 but was {} (response body = '{}')",
                status, body
            )));
        }

        Ok(())
    }

    async fn create_mock(&self, mock: &MockDefinition) -> Result<ActiveMock, ServerAdapterError> {
        self.validate_request_requirements(&mock.request)?;

        let json = serde_json::to_string(mock).map_err(|e| JsonSerializationError(e))?;

        let request = Request::builder()
            .method("POST")
            .uri(format!("http://{}/__httpmock__/mocks", &self.address()))
            .header("content-type", "application/json")
            .body(Bytes::from(json))
            .map_err(|e| UpstreamError(e.to_string()))?;

        let (status, body) = self.do_request(request).await?;

        if status != StatusCode::CREATED.as_u16() {
            return Err(UpstreamError(format!(
                "Could not create mock. Expected response status 201 but was {} (response body = '{}')",
                status, body
            )));
        }

        let response: ActiveMock =
            serde_json::from_str(&body).map_err(|e| JsonDeserializationError(e))?;

        Ok(response)
    }

    async fn fetch_mock(&self, mock_id: usize) -> Result<ActiveMock, ServerAdapterError> {
        let request = Request::builder()
            .method("GET")
            .uri(format!(
                "http://{}/__httpmock__/mocks/{}",
                &self.address(),
                mock_id
            ))
            .body(Bytes::new())
            .map_err(|e| UpstreamError(e.to_string()))?;

        let (status, body) = self.do_request(request).await?;

        if status != StatusCode::OK {
            return Err(UpstreamError(format!(
                "Could not fetch mock from the mock server. Expected response status 200 but was {} (response body = '{}')",
                status, body
            )));
        }

        let response: ActiveMock =
            serde_json::from_str(&body).map_err(|e| JsonDeserializationError(e))?;

        Ok(response)
    }

    async fn delete_mock(&self, mock_id: usize) -> Result<(), ServerAdapterError> {
        let request = Request::builder()
            .method("DELETE")
            .uri(format!(
                "http://{}/__httpmock__/mocks/{}",
                &self.address(),
                mock_id
            ))
            .body(Bytes::new())
            .map_err(|e| UpstreamError(e.to_string()))?;

        let (status, body) = self.do_request(request).await?;

        if status != StatusCode::NO_CONTENT {
            return Err(UpstreamError(format!(
                "Could not delete mock from the mock server. Expected response status 204 but was {} (response body = '{}')",
                status, body
            )));
        }

        Ok(())
    }

    async fn delete_mock_after_calls(
        &self,
        mock_id: usize,
        count: usize,
    ) -> Result<(), ServerAdapterError> {
        let request = Request::builder()
            .method("POST")
            .uri(format!(
                "http://{}/__httpmock__/mocks/{}/delete_after/{}",
                &self.address(),
                mock_id,
                count
            ))
            .body(Bytes::new())
            .map_err(|e| UpstreamError(e.to_string()))?;

        let (status, body) = self.do_request(request).await?;

        if status != StatusCode::NO_CONTENT {
            return Err(UpstreamError(format!(
                "Could not set delete_after_calls on the mock server. Expected response status 204 but was {} (response body = '{}')",
                status, body
            )));
        }

        Ok(())
    }

    async fn delete_all_mocks(&self) -> Result<(), ServerAdapterError> {
        let request = Request::builder()
            .method("DELETE")
            .uri(format!("http://{}/__httpmock__/mocks", &self.address()))
            .body(Bytes::new())
            .map_err(|e| UpstreamError(e.to_string()))?;

        let (status, body) = self.do_request(request).await?;

        if status != StatusCode::NO_CONTENT {
            return Err(UpstreamError(format!(
                "Could not delete all mocks from the mock server. Expected response status 204 but was {} (response body = '{}')",
                status, body
            )));
        }

        Ok(())
    }

    async fn verify(
        &self,
        requirements: &RequestRequirements,
    ) -> Result<Option<ClosestMatch>, ServerAdapterError> {
        let json = serde_json::to_string(requirements).map_err(|e| JsonSerializationError(e))?;

        let request = Request::builder()
            .method("POST")
            .uri(format!("http://{}/__httpmock__/verify", &self.address()))
            .header("content-type", "application/json")
            .body(Bytes::from(json))
            .map_err(|e| UpstreamError(e.to_string()))?;

        let (status, body) = self.do_request(request).await?;

        if status == StatusCode::NOT_FOUND {
            return Ok(None);
        }

        if status != StatusCode::OK {
            return Err(UpstreamError(format!(
                "Could not verify mock. Expected response status 200 but was {} (response body = '{}')",
                status, body
            )));
        }

        let response: ClosestMatch =
            serde_json::from_str(&body).map_err(|e| JsonDeserializationError(e))?;

        Ok(Some(response))
    }

    async fn delete_history(&self) -> Result<(), ServerAdapterError> {
        let request = Request::builder()
            .method("DELETE")
            .uri(format!("http://{}/__httpmock__/history", &self.address()))
            .body(Bytes::new())
            .map_err(|e| UpstreamError(e.to_string()))?;

        let (status, body) = self.do_request(request).await?;

        if status != StatusCode::NO_CONTENT {
            return Err(UpstreamError(format!(
                "Could not delete request history from the mock server. Expected response status 204 but was {} (response body = '{}')",
                status, body
            )));
        }

        Ok(())
    }

    async fn create_forwarding_rule(
        &self,
        config: ForwardingRuleConfig,
    ) -> Result<ActiveForwardingRule, ServerAdapterError> {
        self.validate_request_requirements(&config.request_requirements)?;

        let json = serde_json::to_string(&config).map_err(|e| JsonSerializationError(e))?;

        let request = Request::builder()
            .method("POST")
            .uri(format!(
                "http://{}/__httpmock__/forwarding_rules",
                &self.address()
            ))
            .header("content-type", "application/json")
            .body(Bytes::from(json))
            .map_err(|e| UpstreamError(e.to_string()))?;

        let (status, body) = self.do_request(request).await?;

        if status != StatusCode::CREATED.as_u16() {
            return Err(UpstreamError(format!(
                "Could not create forwarding rule. Expected response status 201 but was {} (response body = '{}')",
                status, body
            )));
        }

        let response: ActiveForwardingRule =
            serde_json::from_str(&body).map_err(|e| JsonDeserializationError(e))?;

        Ok(response)
    }

    async fn delete_forwarding_rule(&self, id: usize) -> Result<(), ServerAdapterError> {
        let request = Request::builder()
            .method("DELETE")
            .uri(format!(
                "http://{}/__httpmock__/forwarding_rules/{}",
                &self.address(),
                id
            ))
            .body(Bytes::new())
            .map_err(|e| UpstreamError(e.to_string()))?;

        let (status, body) = self.do_request(request).await?;

        if status != StatusCode::NO_CONTENT {
            return Err(UpstreamError(format!(
                "Could not delete forwarding rule from the mock server. Expected response status 204 but was {} (response body = '{}')",
                status, body
            )));
        }

        Ok(())
    }

    async fn delete_all_forwarding_rules(&self) -> Result<(), ServerAdapterError> {
        let request = Request::builder()
            .method("DELETE")
            .uri(format!(
                "http://{}/__httpmock__/forwarding_rules",
                &self.address()
            ))
            .body(Bytes::new())
            .map_err(|e| UpstreamError(e.to_string()))?;

        let (status, body) = self.do_request(request).await?;

        if status != StatusCode::NO_CONTENT {
            return Err(UpstreamError(format!(
                "Could not delete all forwarding rules from the mock server. Expected response status 204 but was {} (response body = '{}')",
                status, body
            )));
        }

        Ok(())
    }

    async fn create_proxy_rule(
        &self,
        config: ProxyRuleConfig,
    ) -> Result<ActiveProxyRule, ServerAdapterError> {
        self.validate_request_requirements(&config.request_requirements)?;

        let json = serde_json::to_string(&config).map_err(|e| JsonSerializationError(e))?;

        let request = Request::builder()
            .method("POST")
            .uri(format!(
                "http://{}/__httpmock__/proxy_rules",
                &self.address()
            ))
            .header("content-type", "application/json")
            .body(Bytes::from(json))
            .map_err(|e| UpstreamError(e.to_string()))?;

        let (status, body) = self.do_request(request).await?;

        if status != StatusCode::CREATED.as_u16() {
            return Err(UpstreamError(format!(
                "Could not create proxy rule. Expected response status 201 but was {} (response body = '{}')",
                status, body
            )));
        }

        let response: ActiveProxyRule =
            serde_json::from_str(&body).map_err(|e| JsonDeserializationError(e))?;

        Ok(response)
    }

    async fn delete_proxy_rule(&self, id: usize) -> Result<(), ServerAdapterError> {
        let request = Request::builder()
            .method("DELETE")
            .uri(format!(
                "http://{}/__httpmock__/proxy_rules/{}",
                &self.address(),
                id
            ))
            .body(Bytes::new())
            .map_err(|e| UpstreamError(e.to_string()))?;

        let (status, body) = self.do_request(request).await?;

        if status != StatusCode::NO_CONTENT {
            return Err(UpstreamError(format!(
                "Could not delete proxy rule from the mock server. Expected response status 204 but was {} (response body = '{}')",
                status, body
            )));
        }

        Ok(())
    }

    async fn delete_all_proxy_rules(&self) -> Result<(), ServerAdapterError> {
        let request = Request::builder()
            .method("DELETE")
            .uri(format!(
                "http://{}/__httpmock__/proxy_rules",
                &self.address()
            ))
            .body(Bytes::new())
            .map_err(|e| UpstreamError(e.to_string()))?;

        let (status, body) = self.do_request(request).await?;

        if status != StatusCode::NO_CONTENT {
            return Err(UpstreamError(format!(
                "Could not delete all proxy rules from the mock server. Expected response status 204 but was {} (response body = '{}')",
                status, body
            )));
        }

        Ok(())
    }

    async fn create_recording(
        &self,
        config: RecordingRuleConfig,
    ) -> Result<ActiveRecording, ServerAdapterError> {
        self.validate_request_requirements(&config.request_requirements)?;

        let json = serde_json::to_string(&config).map_err(|e| JsonSerializationError(e))?;

        let request = Request::builder()
            .method("POST")
            .uri(format!(
                "http://{}/__httpmock__/recordings",
                &self.address()
            ))
            .header("content-type", "application/json")
            .body(Bytes::from(json))
            .map_err(|e| UpstreamError(e.to_string()))?;

        let (status, body) = self.do_request(request).await?;

        if status != StatusCode::CREATED.as_u16() {
            return Err(UpstreamError(format!(
                "Could not create recording. Expected response status 201 but was {} (response body = '{}')",
                status, body
            )));
        }

        let response: ActiveRecording =
            serde_json::from_str(&body).map_err(|e| JsonDeserializationError(e))?;

        Ok(response)
    }

    async fn delete_recording(&self, id: usize) -> Result<(), ServerAdapterError> {
        let request = Request::builder()
            .method("DELETE")
            .uri(format!(
                "http://{}/__httpmock__/recordings/{}",
                &self.address(),
                id
            ))
            .body(Bytes::new())
            .map_err(|e| UpstreamError(e.to_string()))?;

        let (status, body) = self.do_request(request).await?;

        if status != StatusCode::NO_CONTENT {
            return Err(UpstreamError(format!(
                "Could not delete recording from the mock server. Expected response status 204 but was {} (response body = '{}')",
                status, body
            )));
        }

        Ok(())
    }

    async fn delete_all_recordings(&self) -> Result<(), ServerAdapterError> {
        let request = Request::builder()
            .method("DELETE")
            .uri(format!(
                "http://{}/__httpmock__/recordings",
                &self.address()
            ))
            .body(Bytes::new())
            .map_err(|e| UpstreamError(e.to_string()))?;

        let (status, body) = self.do_request(request).await?;

        if status != StatusCode::NO_CONTENT {
            return Err(UpstreamError(format!(
                "Could not delete all recordings from the mock server. Expected response status 204 but was {} (response body = '{}')",
                status, body
            )));
        }

        Ok(())
    }

    #[cfg(feature = "record")]
    async fn export_recording(&self, id: usize) -> Result<Option<Bytes>, ServerAdapterError> {
        let request = Request::builder()
            .method("GET")
            .uri(format!(
                "http://{}/__httpmock__/recordings/{}",
                &self.address(),
                id
            ))
            .body(Bytes::new())
            .map_err(|e| UpstreamError(e.to_string()))?;

        let (status, body) = self.do_request_raw(request).await?;

        if status == StatusCode::NOT_FOUND {
            return Ok(None);
        } else if status != StatusCode::OK {
            return Err(UpstreamError(format!(
                "Could not fetch mock from the mock server. Expected response status 200 but was {}",
                status
            )));
        }

        Ok(Some(body))
    }

    #[cfg(feature = "record")]
    async fn create_mocks_from_recording<'a>(
        &self,
        recording_file_content: &'a str,
    ) -> Result<Vec<usize>, ServerAdapterError> {
        let request = Request::builder()
            .method("POST")
            .uri(format!(
                "http://{}/__httpmock__/recordings",
                &self.address(),
            ))
            .body(Bytes::from(recording_file_content.to_owned()))
            .map_err(|e| UpstreamError(e.to_string()))?;

        let (status, body) = self.do_request(request).await?;

        if status != StatusCode::OK {
            return Err(UpstreamError(format!(
                "Could not create mocks from recording. Expected response status 200 but was {}",
                status
            )));
        }

        let response: Vec<usize> =
            serde_json::from_str(&body).map_err(|e| JsonDeserializationError(e))?;

        Ok(response)
    }
}
