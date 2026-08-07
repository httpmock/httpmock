mod builder;
mod handler;
pub mod matchers;
mod server;
pub mod state;

#[cfg(feature = "record")]
mod persistence;

#[cfg(feature = "https")]
mod tls;

pub use builder::{HttpMockServerBuilder, DEFAULT_CA_PRIVATE_KEY, DEFAULT_CA_CERTIFICATE};
pub use server::Error;

use crate::server::{handler::HttpMockHandler, server::MockServer, state::HttpMockStateManager};

// We want to expose this error to the user
pub type HttpMockServer = MockServer<HttpMockHandler<HttpMockStateManager>>;

/// Per-request metadata propagated through Hyper services.
#[derive(Clone)]
pub struct RequestMetadata {
    /// The scheme ("http" or "https") associated with this request, used by the
    /// upstream client to reconstruct the absolute target when needed.
    pub scheme: &'static str,
}

impl RequestMetadata {
    /// Create new RequestMetadata for a request with the given scheme.
    pub fn new(scheme: &'static str) -> Self {
        Self { scheme }
    }
}
