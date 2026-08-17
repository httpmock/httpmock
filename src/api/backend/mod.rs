use std::net::SocketAddr;

use async_trait::async_trait;

use crate::common::data::{
    ActiveForwardingRule, ActiveMock, ActiveProxyRule, ActiveRecording, ClosestMatch, MockDefinition,
    RequestRequirements,
};

mod local;
pub(super) use local::Local;

#[cfg(feature = "record")]
use bytes::Bytes;
use thiserror::Error;

use crate::common::data::{ForwardingRuleConfig, ProxyRuleConfig, RecordingRuleConfig};

#[derive(Error, Debug)]
pub(super) enum Error {
    #[error("mock with ID {0} not found")]
    MockNotFound(usize),
    #[error("invalid mock definition: {0}")]
    InvalidMockDefinition(String),
    #[error("cannot serialize JSON: {0}")]
    JsonSerialization(serde_json::error::Error),
    #[error("cannot deserialize JSON: {0}")]
    JsonDeserialization(serde_json::error::Error),
    #[error("adapter error: {0}")]
    Upstream(String),
}

#[cfg(feature = "remote")]
mod remote;
#[cfg(feature = "remote")]
pub(super) use remote::Remote;

#[async_trait]
pub(super) trait Adapter {
    fn host(&self) -> String;
    fn port(&self) -> u16;
    fn address(&self) -> &SocketAddr;

    async fn reset(&self) -> Result<(), Error>;

    async fn create_mock(&self, mock: &MockDefinition) -> Result<ActiveMock, Error>;
    async fn fetch_mock(&self, mock_id: usize) -> Result<ActiveMock, Error>;
    async fn delete_mock(&self, mock_id: usize) -> Result<(), Error>;

    async fn verify(&self, rr: &RequestRequirements) -> Result<Option<ClosestMatch>, Error>;

    async fn create_forwarding_rule(&self, config: ForwardingRuleConfig) -> Result<ActiveForwardingRule, Error>;
    async fn delete_forwarding_rule(&self, mock_id: usize) -> Result<(), Error>;

    async fn create_proxy_rule(&self, config: ProxyRuleConfig) -> Result<ActiveProxyRule, Error>;
    async fn delete_proxy_rule(&self, mock_id: usize) -> Result<(), Error>;

    async fn create_recording(&self, mock: RecordingRuleConfig) -> Result<ActiveRecording, Error>;
    async fn delete_recording(&self, id: usize) -> Result<(), Error>;

    #[cfg(feature = "record")]
    async fn export_recording(&self, id: usize) -> Result<Option<Bytes>, Error>;

    #[cfg(feature = "record")]
    async fn create_mocks_from_recording<'a>(&self, recording_file_content: &'a str) -> Result<Vec<usize>, Error>;
}
