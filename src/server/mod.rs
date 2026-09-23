mod builder;
mod handler;
pub mod matchers;
pub mod state;
mod transport;

#[cfg(feature = "record")]
mod persistence;

#[cfg(feature = "https")]
mod tls;

pub use builder::HttpMockServerBuilder;
#[cfg(feature = "https")]
pub use builder::{DEFAULT_CA_CERTIFICATE, DEFAULT_CA_PRIVATE_KEY};
pub use transport::{Error, HttpMockServer};

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
