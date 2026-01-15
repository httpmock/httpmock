use std::net::SocketAddr;

use async_trait::async_trait;

use crate::common::data::{
    ActiveForwardingRule, ActiveMock, ActiveProxyRule, ActiveRecording, ClosestMatch,
    MockDefinition, RequestRequirements,
};

pub mod local;

#[cfg(feature = "record")]
use bytes::Bytes;
use thiserror::Error;

use crate::common::data::{ForwardingRuleConfig, ProxyRuleConfig, RecordingRuleConfig};

#[derive(Error, Debug)]
pub enum ServerAdapterError {
    #[error("mock with ID {0} not found")]
    MockNotFound(usize),
    #[error("invalid mock definition: {0}")]
    InvalidMockDefinitionError(String),
    #[error("cannot serialize JSON: {0}")]
    JsonSerializationError(serde_json::error::Error),
    #[error("cannot deserialize JSON: {0}")]
    JsonDeserializationError(serde_json::error::Error),
    #[error("adapter error: {0}")]
    UpstreamError(String),
}

#[cfg(feature = "remote")]
pub mod remote;

#[async_trait]
pub trait MockServerAdapter {
    fn host(&self) -> String;
    fn port(&self) -> u16;
    fn address(&self) -> &SocketAddr;

    async fn reset(&self) -> Result<(), ServerAdapterError>;

    async fn create_mock(&self, mock: &MockDefinition) -> Result<ActiveMock, ServerAdapterError>;
    async fn fetch_mock(&self, mock_id: usize) -> Result<ActiveMock, ServerAdapterError>;
    async fn delete_mock(&self, mock_id: usize) -> Result<(), ServerAdapterError>;

    async fn verify(
        &self,
        rr: &RequestRequirements,
    ) -> Result<Option<ClosestMatch>, ServerAdapterError>;

    async fn create_forwarding_rule(
        &self,
        config: ForwardingRuleConfig,
    ) -> Result<ActiveForwardingRule, ServerAdapterError>;
    async fn delete_forwarding_rule(&self, mock_id: usize) -> Result<(), ServerAdapterError>;

    async fn create_proxy_rule(
        &self,
        config: ProxyRuleConfig,
    ) -> Result<ActiveProxyRule, ServerAdapterError>;
    async fn delete_proxy_rule(&self, mock_id: usize) -> Result<(), ServerAdapterError>;

    async fn create_recording(
        &self,
        mock: RecordingRuleConfig,
    ) -> Result<ActiveRecording, ServerAdapterError>;
    async fn delete_recording(&self, id: usize) -> Result<(), ServerAdapterError>;

    #[cfg(feature = "record")]
    async fn export_recording(&self, id: usize) -> Result<Option<Bytes>, ServerAdapterError>;

    #[cfg(feature = "record")]
    async fn create_mocks_from_recording<'a>(
        &self,
        recording_file_content: &'a str,
    ) -> Result<Vec<usize>, ServerAdapterError>;
}
