use std::net::SocketAddr;

use async_trait::async_trait;

#[cfg(feature = "record")]
use bytes::Bytes;
use thiserror::Error;

#[cfg(feature = "proxy")]
use crate::common::data::{ActiveForwardingRule, ActiveProxyRule, ForwardingRuleConfig, ProxyRuleConfig};
use crate::common::data::{ActiveMock, ClosestMatch, MockDefinition, RequestRequirements};
#[cfg(feature = "record")]
use crate::common::data::{ActiveRecording, RecordingRuleConfig};

pub mod local;

#[derive(Error, Debug)]
pub enum ServerAdapterError {
    #[error("mock with ID {0} not found")]
    MockNotFound(usize),
    #[cfg(feature = "remote")]
    #[error("invalid mock definition: {0}")]
    InvalidMockDefinitionError(String),
    #[cfg(feature = "remote")]
    #[error("cannot serialize JSON: {0}")]
    JsonSerializationError(serde_json::error::Error),
    #[cfg(feature = "remote")]
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

    async fn verify(&self, rr: &RequestRequirements) -> Result<Option<ClosestMatch>, ServerAdapterError>;

    #[cfg(feature = "proxy")]
    async fn create_forwarding_rule(
        &self,
        config: ForwardingRuleConfig,
    ) -> Result<ActiveForwardingRule, ServerAdapterError>;
    #[cfg(feature = "proxy")]
    async fn delete_forwarding_rule(&self, mock_id: usize) -> Result<(), ServerAdapterError>;

    #[cfg(feature = "proxy")]
    async fn create_proxy_rule(&self, config: ProxyRuleConfig) -> Result<ActiveProxyRule, ServerAdapterError>;
    #[cfg(feature = "proxy")]
    async fn delete_proxy_rule(&self, mock_id: usize) -> Result<(), ServerAdapterError>;

    #[cfg(feature = "record")]
    async fn create_recording(&self, mock: RecordingRuleConfig) -> Result<ActiveRecording, ServerAdapterError>;
    #[cfg(feature = "record")]
    async fn delete_recording(&self, id: usize) -> Result<(), ServerAdapterError>;

    #[cfg(feature = "record")]
    async fn export_recording(&self, id: usize) -> Result<Option<Bytes>, ServerAdapterError>;

    #[cfg(feature = "record")]
    async fn create_mocks_from_recording<'a>(
        &self,
        recording_file_content: &'a str,
    ) -> Result<Vec<usize>, ServerAdapterError>;
}
