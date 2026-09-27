mod builder;
mod handler;
pub(crate) mod matchers;
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
