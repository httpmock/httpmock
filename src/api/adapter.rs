use std::net::SocketAddr;

#[cfg(feature = "record")]
use bytes::Bytes;
use thiserror::Error;

#[cfg(feature = "proxy")]
use crate::common::data::{ActiveForwardingRule, ActiveProxyRule, ForwardingRuleConfig, ProxyRuleConfig};
use crate::common::data::{ActiveMock, ClosestMatch, MockDefinition, RequestRequirements};
#[cfg(feature = "record")]
use crate::common::data::{ActiveRecording, RecordingRuleConfig};

pub(super) mod local;

#[derive(Error, Debug)]
pub(super) enum ServerAdapterError {
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
pub(super) mod remote;

// The futures are `+ Send` because users may await the public async API inside `tokio::spawn`,
// and spelling the bound out here makes the compiler check it at each impl.
pub(super) trait MockServerAdapter {
    fn host(&self) -> String;
    fn port(&self) -> u16;
    fn address(&self) -> &SocketAddr;

    fn reset(&self) -> impl Future<Output = Result<(), ServerAdapterError>> + Send;

    fn create_mock(&self, mock: &MockDefinition)
    -> impl Future<Output = Result<ActiveMock, ServerAdapterError>> + Send;
    fn fetch_mock(&self, mock_id: usize) -> impl Future<Output = Result<ActiveMock, ServerAdapterError>> + Send;
    fn delete_mock(&self, mock_id: usize) -> impl Future<Output = Result<(), ServerAdapterError>> + Send;

    fn verify(
        &self,
        rr: &RequestRequirements,
    ) -> impl Future<Output = Result<Option<ClosestMatch>, ServerAdapterError>> + Send;

    #[cfg(feature = "proxy")]
    fn create_forwarding_rule(
        &self,
        config: ForwardingRuleConfig,
    ) -> impl Future<Output = Result<ActiveForwardingRule, ServerAdapterError>> + Send;
    #[cfg(feature = "proxy")]
    fn delete_forwarding_rule(&self, mock_id: usize) -> impl Future<Output = Result<(), ServerAdapterError>> + Send;

    #[cfg(feature = "proxy")]
    fn create_proxy_rule(
        &self,
        config: ProxyRuleConfig,
    ) -> impl Future<Output = Result<ActiveProxyRule, ServerAdapterError>> + Send;
    #[cfg(feature = "proxy")]
    fn delete_proxy_rule(&self, mock_id: usize) -> impl Future<Output = Result<(), ServerAdapterError>> + Send;

    #[cfg(feature = "record")]
    fn create_recording(
        &self,
        mock: RecordingRuleConfig,
    ) -> impl Future<Output = Result<ActiveRecording, ServerAdapterError>> + Send;
    #[cfg(feature = "record")]
    fn delete_recording(&self, id: usize) -> impl Future<Output = Result<(), ServerAdapterError>> + Send;

    #[cfg(feature = "record")]
    fn export_recording(&self, id: usize) -> impl Future<Output = Result<Option<Bytes>, ServerAdapterError>> + Send;

    #[cfg(feature = "record")]
    fn create_mocks_from_recording(
        &self,
        recording_file_content: &str,
    ) -> impl Future<Output = Result<Vec<usize>, ServerAdapterError>> + Send;
}

// `MockServer` isn't generic and the trait's `impl Future` methods aren't dyn-compatible,
// so a `MockServer` holds its adapter as this enum instead of a `dyn MockServerAdapter`.
pub(super) enum ServerAdapter {
    Local(local::LocalMockServerAdapter),
    #[cfg(feature = "remote")]
    Remote(remote::RemoteMockServerAdapter),
}

macro_rules! dispatch {
    ($self:ident, $adapter:ident => $call:expr) => {
        match $self {
            ServerAdapter::Local($adapter) => $call,
            #[cfg(feature = "remote")]
            ServerAdapter::Remote($adapter) => $call,
        }
    };
}

impl ServerAdapter {
    pub(super) fn host(&self) -> String {
        dispatch!(self, adapter => adapter.host())
    }

    pub(super) fn port(&self) -> u16 {
        dispatch!(self, adapter => adapter.port())
    }

    pub(super) fn address(&self) -> &SocketAddr {
        dispatch!(self, adapter => adapter.address())
    }

    pub(super) async fn reset(&self) -> Result<(), ServerAdapterError> {
        dispatch!(self, adapter => adapter.reset().await)
    }

    pub(super) async fn create_mock(&self, mock: &MockDefinition) -> Result<ActiveMock, ServerAdapterError> {
        dispatch!(self, adapter => adapter.create_mock(mock).await)
    }

    pub(super) async fn fetch_mock(&self, mock_id: usize) -> Result<ActiveMock, ServerAdapterError> {
        dispatch!(self, adapter => adapter.fetch_mock(mock_id).await)
    }

    pub(super) async fn delete_mock(&self, mock_id: usize) -> Result<(), ServerAdapterError> {
        dispatch!(self, adapter => adapter.delete_mock(mock_id).await)
    }

    pub(super) async fn verify(&self, rr: &RequestRequirements) -> Result<Option<ClosestMatch>, ServerAdapterError> {
        dispatch!(self, adapter => adapter.verify(rr).await)
    }

    #[cfg(feature = "proxy")]
    pub(super) async fn create_forwarding_rule(
        &self,
        config: ForwardingRuleConfig,
    ) -> Result<ActiveForwardingRule, ServerAdapterError> {
        dispatch!(self, adapter => adapter.create_forwarding_rule(config).await)
    }

    #[cfg(feature = "proxy")]
    pub(super) async fn delete_forwarding_rule(&self, mock_id: usize) -> Result<(), ServerAdapterError> {
        dispatch!(self, adapter => adapter.delete_forwarding_rule(mock_id).await)
    }

    #[cfg(feature = "proxy")]
    pub(super) async fn create_proxy_rule(
        &self,
        config: ProxyRuleConfig,
    ) -> Result<ActiveProxyRule, ServerAdapterError> {
        dispatch!(self, adapter => adapter.create_proxy_rule(config).await)
    }

    #[cfg(feature = "proxy")]
    pub(super) async fn delete_proxy_rule(&self, mock_id: usize) -> Result<(), ServerAdapterError> {
        dispatch!(self, adapter => adapter.delete_proxy_rule(mock_id).await)
    }

    #[cfg(feature = "record")]
    pub(super) async fn create_recording(
        &self,
        mock: RecordingRuleConfig,
    ) -> Result<ActiveRecording, ServerAdapterError> {
        dispatch!(self, adapter => adapter.create_recording(mock).await)
    }

    #[cfg(feature = "record")]
    pub(super) async fn delete_recording(&self, id: usize) -> Result<(), ServerAdapterError> {
        dispatch!(self, adapter => adapter.delete_recording(id).await)
    }

    #[cfg(feature = "record")]
    pub(super) async fn export_recording(&self, id: usize) -> Result<Option<Bytes>, ServerAdapterError> {
        dispatch!(self, adapter => adapter.export_recording(id).await)
    }

    #[cfg(feature = "record")]
    pub(super) async fn create_mocks_from_recording(
        &self,
        recording_file_content: &str,
    ) -> Result<Vec<usize>, ServerAdapterError> {
        dispatch!(self, adapter => adapter.create_mocks_from_recording(recording_file_content).await)
    }
}
