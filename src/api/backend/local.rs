use std::{net::SocketAddr, sync::Arc};

use async_trait::async_trait;
#[cfg(feature = "record")]
use bytes::Bytes;

use crate::{
    api::backend::{
        Adapter, Error,
        Error::{MockNotFound, Upstream},
    },
    common::data::{
        ActiveForwardingRule, ActiveMock, ActiveProxyRule, ActiveRecording, ClosestMatch, ForwardingRuleConfig,
        MockDefinition, ProxyRuleConfig, RecordingRuleConfig, RequestRequirements,
    },
    server::state,
};

pub(in crate::api) struct Local {
    pub(in crate::api) addr: SocketAddr,
    state: Arc<state::Manager>,
}

impl Local {
    pub(in crate::api) fn new(addr: SocketAddr, local_state: Arc<state::Manager>) -> Self {
        Local {
            addr,
            state: local_state,
        }
    }
}

#[async_trait]
impl Adapter for Local {
    fn host(&self) -> String {
        self.addr.ip().to_string()
    }

    fn port(&self) -> u16 {
        self.addr.port()
    }

    fn address(&self) -> &SocketAddr {
        &self.addr
    }

    async fn reset(&self) -> Result<(), Error> {
        self.state.reset();
        Ok(())
    }

    async fn create_mock(&self, mock: &MockDefinition) -> Result<ActiveMock, Error> {
        let active_mock = self
            .state
            .add_mock(mock.clone(), false)
            .map_err(|e| Upstream(e.to_string()))?;
        Ok(active_mock)
    }

    async fn fetch_mock(&self, mock_id: usize) -> Result<ActiveMock, Error> {
        let mock = self
            .state
            .read_mock(mock_id)
            .map_err(|e| Upstream(e.to_string()))?
            .ok_or(MockNotFound(mock_id))?;
        Ok(mock)
    }

    async fn delete_mock(&self, mock_id: usize) -> Result<(), Error> {
        self.state
            .delete_mock(mock_id)
            .map_err(|e| Upstream(format!("Cannot delete mock: {:?}", e)))?;
        Ok(())
    }

    async fn verify(&self, mock_rr: &RequestRequirements) -> Result<Option<ClosestMatch>, Error> {
        let closest_match = self
            .state
            .verify(mock_rr)
            .map_err(|e| Upstream(format!("Cannot delete mock: {:?}", e)))?;
        Ok(closest_match)
    }

    async fn create_forwarding_rule(&self, config: ForwardingRuleConfig) -> Result<ActiveForwardingRule, Error> {
        Ok(self.state.create_forwarding_rule(config))
    }

    async fn delete_forwarding_rule(&self, id: usize) -> Result<(), Error> {
        self.state.delete_forwarding_rule(id);
        Ok(())
    }

    async fn create_proxy_rule(&self, config: ProxyRuleConfig) -> Result<ActiveProxyRule, Error> {
        Ok(self.state.create_proxy_rule(config))
    }

    async fn delete_proxy_rule(&self, id: usize) -> Result<(), Error> {
        self.state.delete_proxy_rule(id);
        Ok(())
    }

    async fn create_recording(&self, config: RecordingRuleConfig) -> Result<ActiveRecording, Error> {
        Ok(self.state.create_recording(config))
    }

    async fn delete_recording(&self, id: usize) -> Result<(), Error> {
        self.state.delete_recording(id);
        Ok(())
    }

    #[cfg(feature = "record")]
    async fn export_recording(&self, id: usize) -> Result<Option<Bytes>, Error> {
        Ok(self
            .state
            .export_recording(id)
            .map_err(|err| Upstream(err.to_string()))?)
    }

    #[cfg(feature = "record")]
    async fn create_mocks_from_recording<'a>(&self, recording_file_content: &'a str) -> Result<Vec<usize>, Error> {
        Ok(self
            .state
            .load_mocks_from_recording(recording_file_content)
            .map_err(|err| Upstream(err.to_string()))?)
    }
}
