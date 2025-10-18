// TODO: Remove this at some point
#![allow(clippy::needless_lifetimes)]

pub use adapter::MockServerAdapter;
#[cfg(feature = "server")]
pub use adapter::local::LocalMockServerAdapter;

use serde::{Deserialize, Serialize};
use std::str::FromStr;

#[cfg(feature = "remote")]
pub use adapter::remote::RemoteMockServerAdapter;

#[cfg(feature = "record")]
pub use proxy::{Recording, RecordingRuleBuilder};

#[cfg(feature = "proxy")]
pub use proxy::{ForwardingRule, ForwardingRuleBuilder, ProxyRule, ProxyRuleBuilder};

use crate::common;
pub use mock::{Mock, MockExt};
pub use server::MockServer;
pub use spec::{Then, When};

mod adapter;
mod mock;
mod output;
mod proxy;
mod server;
pub mod spec;

/// Type alias for [regex::Regex](../regex/struct.Regex.html).
pub type Regex = common::data::HttpMockRegex;

pub use crate::common::data::Method;
