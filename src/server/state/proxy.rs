//! Server-side state for forwarding and proxy rules.

use std::collections::BTreeMap;

use crate::{
    common::data::{ActiveForwardingRule, ActiveProxyRule, ForwardingRuleConfig, ProxyRuleConfig},
    prelude::HttpMockRequest,
    server::state::{Error, Manager, request_matches},
};

#[derive(Default)]
pub(super) struct State {
    next_forwarding_rule_id: usize,
    next_proxy_rule_id: usize,
    forwarding_rules: BTreeMap<usize, ActiveForwardingRule>,
    proxy_rules: BTreeMap<usize, ActiveProxyRule>,
}

impl Manager {
    pub(crate) fn create_forwarding_rule(&self, config: ForwardingRuleConfig) -> ActiveForwardingRule {
        let mut state = self.state.lock().unwrap();

        let rule = ActiveForwardingRule {
            id: state.proxy.next_forwarding_rule_id,
            config,
        };

        state.proxy.forwarding_rules.insert(rule.id, rule.clone());

        state.proxy.next_forwarding_rule_id += 1;

        rule
    }

    pub(crate) fn delete_forwarding_rule(&self, id: usize) -> Option<ActiveForwardingRule> {
        let mut state = self.state.lock().unwrap();

        let result = state.proxy.forwarding_rules.remove(&id);

        if result.is_some() {
            tracing::debug!("Deleting forwarding rule with id={}", id);
        } else {
            tracing::warn!(
                "Could not delete forwarding rule with id={} (no forwarding rule with that id found)",
                id
            );
        }

        result
    }

    pub(crate) fn delete_all_forwarding_rules(&self) {
        let mut state = self.state.lock().unwrap();
        state.proxy.forwarding_rules.clear();

        tracing::debug!("Deleted all forwarding rules");
    }

    pub(crate) fn create_proxy_rule(&self, config: ProxyRuleConfig) -> ActiveProxyRule {
        let mut state = self.state.lock().unwrap();

        let rule = ActiveProxyRule {
            id: state.proxy.next_proxy_rule_id,
            config,
        };

        state.proxy.proxy_rules.insert(rule.id, rule.clone());

        state.proxy.next_proxy_rule_id += 1;

        rule
    }

    pub(crate) fn delete_proxy_rule(&self, id: usize) -> Option<ActiveProxyRule> {
        let mut state = self.state.lock().unwrap();

        let result = state.proxy.proxy_rules.remove(&id);

        if result.is_some() {
            tracing::debug!("Deleting proxy rule with id={}", id);
        } else {
            tracing::warn!(
                "Could not delete proxy rule with id={} (no proxy rule with that id found)",
                id
            );
        }

        result
    }

    pub(crate) fn delete_all_proxy_rules(&self) {
        let mut state = self.state.lock().unwrap();
        state.proxy.proxy_rules.clear();

        tracing::debug!("Deleted all proxy rules");
    }

    pub(crate) fn find_forward_rule<'a>(
        &'a self,
        req: &'a HttpMockRequest,
    ) -> Result<Option<ActiveForwardingRule>, Error> {
        let state = self.state.lock().unwrap();

        let result = state
            .proxy
            .forwarding_rules
            .values()
            .find(|&rule| request_matches(&state.matchers, req, &rule.config.request_requirements))
            .cloned();

        Ok(result)
    }

    pub(crate) fn find_proxy_rule<'a>(&'a self, req: &'a HttpMockRequest) -> Result<Option<ActiveProxyRule>, Error> {
        let state = self.state.lock().unwrap();

        let result = state
            .proxy
            .proxy_rules
            .values()
            .find(|&rule| request_matches(&state.matchers, req, &rule.config.request_requirements))
            .cloned();

        Ok(result)
    }
}
