//! Server-side state for forwarding and proxy rules.

use std::{collections::BTreeMap, str::FromStr};

use http::{HeaderMap, HeaderName, HeaderValue, Uri, uri::Authority};

use crate::{
    common::data::{ActiveForwardingRule, ActiveProxyRule, ForwardingRuleConfig, ProxyRuleConfig},
    prelude::HttpMockRequest,
    server::state::{Error, Manager, request_matches},
};

#[derive(Default)]
pub(super) struct State {
    next_forwarding_rule_id: usize,
    next_proxy_rule_id: usize,
    forwarding_rules: BTreeMap<usize, ForwardingRule>,
    proxy_rules: BTreeMap<usize, ActiveProxyRule>,
}

#[derive(Clone)]
pub(crate) struct ForwardingRule {
    pub active: ActiveForwardingRule,
    pub target: ForwardTarget,
    pub request_headers: HeaderMap,
}

#[derive(Clone)]
pub(crate) struct ForwardTarget {
    scheme: ForwardScheme,
    authority: Authority,
}

#[derive(Clone, Copy)]
enum ForwardScheme {
    Http,
    Https,
}

impl TryFrom<&str> for ForwardTarget {
    type Error = Error;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        let uri =
            Uri::from_str(value).map_err(|err| Error::ValidationError(format!("invalid forwarding target: {err}")))?;
        let parts = uri.into_parts();

        let scheme = match parts.scheme.as_ref().map(|scheme| scheme.as_str()) {
            Some("http") => ForwardScheme::Http,
            Some("https") => ForwardScheme::Https,
            Some(scheme) => {
                return Err(Error::ValidationError(format!(
                    "invalid forwarding target scheme '{scheme}': expected http or https"
                )));
            }
            None => return Err(Error::ValidationError("forwarding target has no scheme".to_string())),
        };

        let authority = parts
            .authority
            .ok_or_else(|| Error::ValidationError("forwarding target has no authority".to_string()))?;

        Ok(Self { scheme, authority })
    }
}

impl ForwardTarget {
    pub fn into_parts(self) -> (&'static str, http::uri::Scheme, Authority) {
        let (name, scheme) = match self.scheme {
            ForwardScheme::Http => ("http", http::uri::Scheme::HTTP),
            ForwardScheme::Https => ("https", http::uri::Scheme::HTTPS),
        };

        (name, scheme, self.authority)
    }
}

impl Manager {
    pub(crate) fn create_forwarding_rule(&self, config: ForwardingRuleConfig) -> Result<ActiveForwardingRule, Error> {
        let target = ForwardTarget::try_from(config.target_base_url.as_str())?;
        let mut request_headers = HeaderMap::with_capacity(config.request_header.len());
        for (name, value) in &config.request_header {
            let name = HeaderName::from_str(name)
                .map_err(|err| Error::ValidationError(format!("invalid forwarding header name: {err}")))?;
            let value = HeaderValue::from_str(value)
                .map_err(|err| Error::ValidationError(format!("invalid forwarding header value: {err}")))?;
            request_headers.append(name, value);
        }
        let mut state = self.state.lock().unwrap();

        let active = ActiveForwardingRule {
            id: state.proxy.next_forwarding_rule_id,
            config,
        };
        let rule = ForwardingRule {
            active: active.clone(),
            target,
            request_headers,
        };

        state.proxy.forwarding_rules.insert(active.id, rule);

        state.proxy.next_forwarding_rule_id += 1;

        Ok(active)
    }

    pub(crate) fn delete_forwarding_rule(&self, id: usize) -> Option<ActiveForwardingRule> {
        let mut state = self.state.lock().unwrap();

        let result = state.proxy.forwarding_rules.remove(&id).map(|rule| rule.active);

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

    pub(crate) fn find_forward_rule<'a>(&'a self, req: &'a HttpMockRequest) -> Result<Option<ForwardingRule>, Error> {
        let state = self.state.lock().unwrap();

        let result = state
            .proxy
            .forwarding_rules
            .values()
            .find(|&rule| request_matches(&state.matchers, req, &rule.active.config.request_requirements))
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
