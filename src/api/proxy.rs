//! Client-side API for forwarding and proxy rules.

use std::sync::{Arc, Mutex};

use crate::{
    When,
    api::server::MockServer,
    common::{
        data::RequestRequirements,
        util::{Join, update_mutex},
    },
};

/// Represents a forwarding rule on a [MockServer](struct.MockServer.html), allowing HTTP requests
/// that meet specific criteria to be redirected to a designated destination. Each rule is
/// uniquely identified by an ID within the server context.
pub struct ForwardingRule<'a> {
    pub id: usize,
    pub(crate) server: &'a MockServer,
}

impl<'a> ForwardingRule<'a> {
    pub fn new(id: usize, server: &'a MockServer) -> Self {
        Self { id, server }
    }

    /// Synchronously deletes the forwarding rule from the mock server.
    /// This method blocks the current thread until the deletion has been completed, ensuring that the rule is no longer active and will not affect any further requests.
    ///
    /// # Panics
    /// Panics if the deletion fails, typically due to issues such as the rule not existing or server connectivity problems.
    pub fn delete(&mut self) {
        self.delete_async().join();
    }

    /// Asynchronously deletes the forwarding rule from the mock server.
    /// This method performs the deletion without blocking the current thread,
    /// making it suitable for use in asynchronous applications where maintaining responsiveness
    /// or concurrent execution is necessary.
    ///
    /// # Panics
    /// Panics if the deletion fails, typically due to issues such as the rule not existing or server connectivity problems.
    /// This method will raise an immediate panic on such failures, signaling that the operation could not be completed as expected.
    pub async fn delete_async(&self) {
        self.server
            .server_adapter
            .as_ref()
            .unwrap()
            .delete_forwarding_rule(self.id)
            .await
            .expect("could not delete mock from server");
    }
}
/// Provides methods for managing a proxy rule from the server.
pub struct ProxyRule<'a> {
    pub id: usize,
    pub(crate) server: &'a MockServer,
}

impl<'a> ProxyRule<'a> {
    pub fn new(id: usize, server: &'a MockServer) -> Self {
        Self { id, server }
    }

    /// Synchronously deletes the proxy rule from the server.
    /// This method blocks the current thread until the deletion is complete, ensuring that
    /// the rule is removed and will no longer redirect any requests.
    ///
    /// # Usage
    /// This method is typically used in synchronous environments where immediate removal of the
    /// rule is necessary and can afford a blocking operation.
    ///
    /// # Panics
    /// Panics if the deletion fails due to server-related issues such as connectivity problems,
    /// or if the rule does not exist on the server.
    pub fn delete(&mut self) {
        self.delete_async().join();
    }

    /// Asynchronously deletes the proxy rule from the server.
    /// This method allows for non-blocking operations, suitable for asynchronous environments
    /// where tasks are performed concurrently without interrupting the main workflow.
    ///
    /// # Usage
    /// Ideal for use in modern async/await patterns in Rust, providing a way to handle resource
    /// cleanup without stalling other operations.
    ///
    /// # Panics
    /// Panics if the deletion fails due to server-related issues such as connectivity problems,
    /// or if the rule does not exist on the server. This method raises an immediate panic to
    /// indicate that the operation could not be completed as expected.
    pub async fn delete_async(&self) {
        self.server
            .server_adapter
            .as_ref()
            .unwrap()
            .delete_proxy_rule(self.id)
            .await
            .expect("could not delete mock from server");
    }
}

pub struct ForwardingRuleBuilder {
    // Arc lets setup and this owned builder share state without lifetime parameters.
    // Mutex allows mutation of that state while keeping the builder Send + Sync for spawned tasks.
    pub(crate) request_requirements: Arc<Mutex<RequestRequirements>>,
    pub(crate) headers: Arc<Mutex<Vec<(String, String)>>>,
}

impl ForwardingRuleBuilder {
    pub fn add_request_header<Key: Into<String>, Value: Into<String>>(self, key: Key, value: Value) -> Self {
        update_mutex(&self.headers, |headers| headers.push((key.into(), value.into())));
        self
    }

    pub fn filter<WhenSpecFn>(self, when: WhenSpecFn) -> Self
    where
        WhenSpecFn: FnOnce(When),
    {
        when(When {
            expectations: self.request_requirements.clone(),
        });
        self
    }
}

pub struct ProxyRuleBuilder {
    // TODO: These fields are visible to the user, make them not public
    // Arc lets setup and this owned builder share state without lifetime parameters.
    // Mutex allows mutation of that state while keeping the builder Send + Sync for spawned tasks.
    pub(crate) request_requirements: Arc<Mutex<RequestRequirements>>,
    pub(crate) headers: Arc<Mutex<Vec<(String, String)>>>,
}

impl ProxyRuleBuilder {
    pub fn add_request_header<Key: Into<String>, Value: Into<String>>(self, key: Key, value: Value) -> Self {
        update_mutex(&self.headers, |headers| headers.push((key.into(), value.into())));
        self
    }

    pub fn filter<WhenSpecFn>(self, when: WhenSpecFn) -> Self
    where
        WhenSpecFn: FnOnce(When),
    {
        when(When {
            expectations: self.request_requirements.clone(),
        });

        self
    }
}
