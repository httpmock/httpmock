mod builder;
mod handler;
pub mod matchers;
mod server;
pub mod state;

#[cfg(feature = "record")]
mod persistence;

#[cfg(feature = "https")]
mod tls;

pub use builder::HttpMockServerBuilder;
#[cfg(feature = "https")]
pub use builder::{DEFAULT_CA_CERTIFICATE, DEFAULT_CA_PRIVATE_KEY};
pub use server::Error;

use crate::server::{handler::HttpMockHandler, server::MockServer, state::HttpMockStateManager};

// We want to expose this error to the user
pub type HttpMockServer = MockServer<HttpMockHandler<HttpMockStateManager>>;

/// Per-request metadata propagated through Hyper services.
#[derive(Clone)]
pub struct RequestMetadata {
    /// The transport scheme used to reconstruct an absolute request target.
    pub scheme: http::uri::Scheme,
}

impl RequestMetadata {
    /// Creates request metadata for a transport scheme.
    pub fn new(scheme: http::uri::Scheme) -> Self {
        Self { scheme }
    }
}
