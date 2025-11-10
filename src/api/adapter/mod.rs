use std::{net::SocketAddr, str::FromStr};

use async_trait::async_trait;
use bytes::Bytes;

use serde::{Deserialize, Serialize};

use crate::common::data::{ActiveForwardingRule, ActiveMock, ActiveProxyRule};

use crate::common::data::{ActiveRecording, ClosestMatch, MockDefinition, RequestRequirements};

pub mod local;

use crate::common::data::{ForwardingRuleConfig, ProxyRuleConfig, RecordingRuleConfig};

use thiserror::Error;

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
    #[error("cannot ping mock server: {0}")]
    PingError(String),
    #[error("unknown error")]
    Unknown,
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
    async fn delete_mock_after_calls(
        &self,
        mock_id: usize,
        count: usize,
    ) -> Result<(), ServerAdapterError>;
    async fn delete_all_mocks(&self) -> Result<(), ServerAdapterError>;

    async fn verify(
        &self,
        rr: &RequestRequirements,
    ) -> Result<Option<ClosestMatch>, ServerAdapterError>;
    async fn delete_history(&self) -> Result<(), ServerAdapterError>;

    async fn create_forwarding_rule(
        &self,
        config: ForwardingRuleConfig,
    ) -> Result<ActiveForwardingRule, ServerAdapterError>;
    async fn delete_forwarding_rule(&self, mock_id: usize) -> Result<(), ServerAdapterError>;
    async fn delete_all_forwarding_rules(&self) -> Result<(), ServerAdapterError>;

    async fn create_proxy_rule(
        &self,
        config: ProxyRuleConfig,
    ) -> Result<ActiveProxyRule, ServerAdapterError>;
    async fn delete_proxy_rule(&self, mock_id: usize) -> Result<(), ServerAdapterError>;
    async fn delete_all_proxy_rules(&self) -> Result<(), ServerAdapterError>;

    async fn create_recording(
        &self,
        mock: RecordingRuleConfig,
    ) -> Result<ActiveRecording, ServerAdapterError>;
    async fn delete_recording(&self, id: usize) -> Result<(), ServerAdapterError>;
    async fn delete_all_recordings(&self) -> Result<(), ServerAdapterError>;

    #[cfg(feature = "record")]
    async fn export_recording(&self, id: usize) -> Result<Option<Bytes>, ServerAdapterError>;

    #[cfg(feature = "record")]
    async fn create_mocks_from_recording<'a>(
        &self,
        recording_file_content: &'a str,
    ) -> Result<Vec<usize>, ServerAdapterError>;
}
