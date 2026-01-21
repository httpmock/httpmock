// TODO: Remove this at some point
#![allow(clippy::needless_lifetimes)]

#[cfg(feature = "remote")]
pub use adapter::remote::RemoteMockServerAdapter;
pub use adapter::{local::LocalMockServerAdapter, MockServerAdapter};
pub use mock::{Mock, MockExt};
#[cfg(feature = "proxy")]
pub use proxy::{ForwardingRule, ForwardingRuleBuilder, ProxyRule, ProxyRuleBuilder};
#[cfg(feature = "record")]
pub use proxy::{Recording, RecordingRuleBuilder};
pub use server::MockServer;
pub use spec::{Then, When};

use crate::common;

mod adapter;
mod mock;
mod output;
mod proxy;
mod server;
pub mod spec;

/// Type alias for [regex::Regex](../regex/struct.Regex.html).
pub type Regex = common::data::HttpMockRegex;

pub use crate::common::data::Method;
