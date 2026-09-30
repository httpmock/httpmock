#[cfg(feature = "proxy")]
pub use adapter::ServerAdapterError;
#[cfg(feature = "remote")]
use adapter::remote::RemoteMockServerAdapter;
use adapter::{MockServerAdapter, local::LocalMockServerAdapter};
pub use mock::Mock;
#[cfg(feature = "proxy")]
pub use proxy::{ForwardingRule, ForwardingRuleBuilder, ProxyRule, ProxyRuleBuilder};
#[cfg(feature = "record")]
pub use record::{Recording, RecordingRuleBuilder};
pub use server::MockServer;
pub use spec::{Then, When};

use crate::common;

mod adapter;
mod mock;
mod output;
#[cfg(feature = "proxy")]
mod proxy;
#[cfg(feature = "record")]
mod record;
mod server;
pub mod spec;

/// Type alias for [regex::Regex](../regex/struct.Regex.html).
pub type Regex = common::data::HttpMockRegex;

pub use crate::common::data::Method;
