use async_trait::async_trait;
use bytes::Bytes;
use std::{borrow::Borrow, fmt::Debug, net::SocketAddr, sync::Arc};

use futures_util::TryFutureExt;

use crate::api::adapter::{MockServerAdapter, ServerAdapterError};

use crate::{
    api::adapter::ServerAdapterError::{MockNotFound, PingError, UpstreamError},
    server::state::{HttpMockStateManager, StateManager},
};

use crate::common::data::{ActiveForwardingRule, ActiveMock, ActiveProxyRule, ActiveRecording};

use crate::common::data::{
    ClosestMatch, ForwardingRuleConfig, MockDefinition, ProxyRuleConfig, RecordingRuleConfig,
    RequestRequirements,
};

pub struct LocalMockServerAdapter {
    pub addr: SocketAddr,
    state: Arc<HttpMockStateManager>,
}

impl LocalMockServerAdapter {
    pub fn new(addr: SocketAddr, local_state: Arc<HttpMockStateManager>) -> Self {
        LocalMockServerAdapter {
            addr,
            state: local_state,
        }
    }
}

#[async_trait]
impl MockServerAdapter for LocalMockServerAdapter {
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
        self.state.reset();
        Ok(())
    }

    async fn create_mock(&self, mock: &MockDefinition) -> Result<ActiveMock, ServerAdapterError> {
        let active_mock = self
            .state
            .add_mock(mock.clone(), false)
            .map_err(|e| UpstreamError(e.to_string()))?;
        Ok(active_mock)
    }

    async fn fetch_mock(&self, mock_id: usize) -> Result<ActiveMock, ServerAdapterError> {
        let mock = self
            .state
            .read_mock(mock_id)
            .map_err(|e| UpstreamError(e.to_string()))?
            .ok_or_else(|| MockNotFound(mock_id))?;
        Ok(mock)
    }

    async fn delete_mock(&self, mock_id: usize) -> Result<(), ServerAdapterError> {
        self.state
            .delete_mock(mock_id)
            .map_err(|e| UpstreamError(format!("Cannot delete mock: {:?}", e)))?;
        Ok(())
    }

    async fn delete_mock_after_calls(
        &self,
        mock_id: usize,
        count: usize,
    ) -> Result<(), ServerAdapterError> {
        self.state
            .delete_mock_after_calls(mock_id, count)
            .map_err(|e| {
                UpstreamError(format!("Cannot set delete_after_calls on mock: {:?}", e))
            })?;
        Ok(())
    }

    async fn delete_all_mocks(&self) -> Result<(), ServerAdapterError> {
        self.state.delete_all_mocks();
        Ok(())
    }

    async fn verify(
        &self,
        mock_rr: &RequestRequirements,
    ) -> Result<Option<ClosestMatch>, ServerAdapterError> {
        let closest_match = self
            .state
            .verify(mock_rr)
            .map_err(|e| UpstreamError(format!("Cannot delete mock: {:?}", e)))?;
        Ok(closest_match)
    }

    async fn delete_history(&self) -> Result<(), ServerAdapterError> {
        self.state.delete_history();
        Ok(())
    }

    async fn create_forwarding_rule(
        &self,
        config: ForwardingRuleConfig,
    ) -> Result<ActiveForwardingRule, ServerAdapterError> {
        Ok(self.state.create_forwarding_rule(config))
    }

    async fn delete_forwarding_rule(&self, id: usize) -> Result<(), ServerAdapterError> {
        self.state.delete_forwarding_rule(id);
        Ok(())
    }

    async fn delete_all_forwarding_rules(&self) -> Result<(), ServerAdapterError> {
        self.state.delete_all_forwarding_rules();
        Ok(())
    }

    async fn create_proxy_rule(
        &self,
        config: ProxyRuleConfig,
    ) -> Result<ActiveProxyRule, ServerAdapterError> {
        Ok(self.state.create_proxy_rule(config))
    }

    async fn delete_proxy_rule(&self, id: usize) -> Result<(), ServerAdapterError> {
        self.state.delete_proxy_rule(id);
        Ok(())
    }

    async fn delete_all_proxy_rules(&self) -> Result<(), ServerAdapterError> {
        self.state.delete_all_proxy_rules();
        Ok(())
    }

    async fn create_recording(
        &self,
        config: RecordingRuleConfig,
    ) -> Result<ActiveRecording, ServerAdapterError> {
        Ok(self.state.create_recording(config))
    }

    async fn delete_recording(&self, id: usize) -> Result<(), ServerAdapterError> {
        self.state.delete_recording(id);
        Ok(())
    }

    async fn delete_all_recordings(&self) -> Result<(), ServerAdapterError> {
        self.state.delete_all_recordings();
        Ok(())
    }

    #[cfg(feature = "record")]
    async fn export_recording(&self, id: usize) -> Result<Option<Bytes>, ServerAdapterError> {
        Ok(self
            .state
            .export_recording(id)
            .map_err(|err| UpstreamError(err.to_string()))?)
    }

    #[cfg(feature = "record")]
    async fn create_mocks_from_recording<'a>(
        &self,
        recording_file_content: &'a str,
    ) -> Result<Vec<usize>, ServerAdapterError> {
        Ok(self
            .state
            .load_mocks_from_recording(recording_file_content)
            .map_err(|err| UpstreamError(err.to_string()))?)
    }
}
