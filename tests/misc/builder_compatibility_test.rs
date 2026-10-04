//! Protects user setup patterns supported by the owned mock, forwarding, proxy and recording builders.
//!
//! Helpers and user-defined structs can accept or store builders without carrying lifetime parameters.
//! Setup futures must also remain Send so users can configure rules from a spawned task.
//!
//! The request tests check that these setup patterns still produce the intended matching behavior.
//! In particular, a filter builder saved during setup must keep updating its rule after the inner
//! filter callback returns, until the outer rule-configuration callback finishes.
//! The forwarding, proxy and recording spawn-only tests exercise registration and deletion without traffic.

use std::any::Any;

use httpmock::{MockServer, Then, When};
use reqwest::blocking::Client;

// A reusable helper can borrow a path and return the owned builder without lifetime parameters.
fn with_path(when: When, path: &str) -> When {
    when.path(path)
}

// The callback cannot capture temporary borrows, but it still runs immediately during setup.
fn run_static_callback(callback: Box<dyn FnOnce() + 'static>) {
    callback();
}

// User-defined setup objects can store both builders without adding lifetime parameters.
struct Setup {
    when: When,
    then: Then,
}

// Preserves owned-builder usage in helpers, structs, Any, and immediately invoked 'static callbacks.
// Borrowing builders from the surrounding setup would make one or more of these user patterns fail to compile.
#[test]
fn owned_builders_support_existing_helpers_and_static_callbacks() {
    let server = MockServer::start();
    let path = String::from("/users");

    let mock = server.mock(|when, then| {
        let setup = Setup { when, then };
        // Move the builders and owned path into the callback instead of borrowing the outer setup.
        run_static_callback(Box::new(move || {
            // Any requires a 'static type, so When must not borrow from the server.mock callback.
            assert!((&setup.when as &dyn Any).is::<When>());
            // Match /users through the helper. Passing any_request to and exercises method callbacks;
            // any_request adds no restriction and leaves the path matcher in place.
            with_path(setup.when, &path).and(When::any_request);
            setup.then.status(201);
        }));
    });

    // The moved builders must still configure this mock's response and record exactly one match.
    assert_eq!(Client::new().get(server.url("/users")).send().unwrap().status(), 201);
    mock.assert();
}

// tokio::spawn requires a Send future. Keeping mock creation and its await inside the task checks
// that setup does not hold non-Send builder state across a suspension point.
#[tokio::test]
async fn mock_setup_can_run_in_a_send_task() {
    let server = MockServer::start_async().await;

    tokio::spawn(async move {
        // Configure /users to return 201, then send a real request to verify registration completed.
        let mock = server
            .mock_async(|when, then| {
                when.path("/users");
                then.status(201);
            })
            .await;

        let response = reqwest::Client::new().get(server.url("/users")).send().await.unwrap();
        assert_eq!(response.status(), 201);
        mock.assert_async().await;
    })
    .await
    .unwrap();
}

// Registers forwarding and proxy rules in a spawned task to guard their setup futures' Send support.
// No traffic is sent here: these checks cover registration and deletion, not request routing.
#[cfg(feature = "proxy")]
#[tokio::test]
async fn forwarding_and_proxy_setup_can_run_in_a_send_task() {
    let server = MockServer::start_async().await;
    let target = MockServer::start_async().await;

    tokio::spawn(async move {
        // A matching /forwarded request would be sent from server to the fixed target URL.
        let forwarding = server
            .forward_to_async(target.base_url(), |rule| {
                rule.filter(|when| {
                    when.path("/forwarded");
                });
            })
            .await;
        forwarding.delete_async().await;

        // A matching /proxied request would use the destination supplied by the proxy client.
        let proxy = server
            .proxy_async(|rule| {
                rule.filter(|when| {
                    when.path("/proxied");
                });
            })
            .await;
        proxy.delete_async().await;
    })
    .await
    .unwrap();
}

// Registers a recording in a spawned task to guard the setup future's Send support.
// No traffic is sent here; recording contents are outside this setup compatibility check.
#[cfg(feature = "record")]
#[tokio::test]
async fn recording_setup_can_run_in_a_send_task() {
    let server = MockServer::start_async().await;

    tokio::spawn(async move {
        // Only requests with the /recorded path would be eligible for this recording.
        let recording = server
            .record_async(|rule| {
                rule.filter(|when| {
                    when.path("/recorded");
                });
            })
            .await;
        recording.delete_async().await;
    })
    .await
    .unwrap();
}

// Preserves staged setup: a saved filter must still affect forwarding before the outer callback returns.
#[cfg(feature = "proxy")]
#[test]
fn forwarding_filter_can_be_staged_inside_the_outer_callback() {
    let target = MockServer::start();
    // The target accepts every path, so only the gateway's filter can exclude /other.
    target.mock(|when, then| {
        when.any_request();
        then.status(201);
    });
    let gateway = MockServer::start();

    gateway.forward_to(target.base_url(), |rule| {
        let mut pending_filter = None;
        // Save the owned When and let the inner callback return without configuring a path yet.
        rule.filter(|when| pending_filter = Some(when));
        // This later change must still reach the rule before the outer setup callback returns.
        // Extracting the filter's state when the inner callback ends would lose the /wanted matcher.
        pending_filter.unwrap().path("/wanted");
    });

    let client = Client::new();
    // /wanted reaches the target; /other must receive the gateway's unmatched-request response.
    assert_eq!(client.get(gateway.url("/wanted")).send().unwrap().status(), 201);
    assert_eq!(client.get(gateway.url("/other")).send().unwrap().status(), 404);
}

// Preserves staged setup: a saved filter must still affect proxying before the outer callback returns.
#[cfg(feature = "proxy")]
#[test]
fn proxy_filter_can_be_staged_inside_the_outer_callback() {
    let target = MockServer::start();
    // Both paths would return 201 at the target, making the gateway's filtering observable.
    target.mock(|when, then| {
        when.any_request();
        then.status(201);
    });
    let gateway = MockServer::start();

    gateway.proxy(|rule| {
        let mut pending_filter = None;
        // Keep the owned filter builder after its callback returns, then configure the same rule.
        rule.filter(|when| pending_filter = Some(when));
        // If the rule already captured a snapshot of the empty filter, /other would also be proxied.
        pending_filter.unwrap().path("/wanted");
    });

    // Address requests to the target, but send them through the gateway as an HTTP proxy.
    let client = Client::builder()
        .proxy(reqwest::Proxy::all(gateway.base_url()).unwrap())
        .build()
        .unwrap();
    // The saved filter must allow /wanted through and reject /other at the gateway.
    assert_eq!(client.get(target.url("/wanted")).send().unwrap().status(), 201);
    assert_eq!(client.get(target.url("/other")).send().unwrap().status(), 404);
}

// The bounds are checked at compile time; the helper needs no runtime assertion.
fn assert_send_sync<T: Send + Sync>() {}

// Check each builder at compile time: Send permits moving it to another thread, while Sync permits
// sharing references across threads. The feature-gated builders are checked when their feature is enabled.
#[test]
fn builders_are_send_and_sync() {
    assert_send_sync::<When>();
    assert_send_sync::<Then>();
    #[cfg(feature = "proxy")]
    {
        assert_send_sync::<httpmock::ForwardingRuleBuilder>();
        assert_send_sync::<httpmock::ProxyRuleBuilder>();
    }
    #[cfg(feature = "record")]
    assert_send_sync::<httpmock::RecordingRuleBuilder>();
}
