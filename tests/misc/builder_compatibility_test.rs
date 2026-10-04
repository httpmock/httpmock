use std::any::Any;

use httpmock::{MockServer, Then, When};
use reqwest::blocking::Client;

fn with_path(when: When, path: &str) -> When {
    when.path(path)
}

fn run_static_callback(callback: Box<dyn FnOnce() + 'static>) {
    callback();
}

struct Setup {
    when: When,
    then: Then,
}

// Preserves owned-builder usage in helpers, structs, Any, and immediately invoked 'static callbacks.
#[test]
fn owned_builders_support_existing_helpers_and_static_callbacks() {
    let server = MockServer::start();
    let path = String::from("/users");

    let mock = server.mock(|when, then| {
        let setup = Setup { when, then };
        run_static_callback(Box::new(move || {
            assert!((&setup.when as &dyn Any).is::<When>());
            with_path(setup.when, &path).and(When::any_request);
            setup.then.status(201);
        }));
    });

    assert_eq!(Client::new().get(server.url("/users")).send().unwrap().status(), 201);
    mock.assert();
}

// Creates and uses a mock in a spawned task so a non-Send setup future fails to compile.
#[tokio::test]
async fn mock_setup_can_run_in_a_send_task() {
    let server = MockServer::start_async().await;

    tokio::spawn(async move {
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
#[cfg(feature = "proxy")]
#[tokio::test]
async fn forwarding_and_proxy_setup_can_run_in_a_send_task() {
    let server = MockServer::start_async().await;
    let target = MockServer::start_async().await;

    tokio::spawn(async move {
        let forwarding = server
            .forward_to_async(target.base_url(), |rule| {
                rule.filter(|when| {
                    when.path("/forwarded");
                });
            })
            .await;
        forwarding.delete_async().await;

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
#[cfg(feature = "record")]
#[tokio::test]
async fn recording_setup_can_run_in_a_send_task() {
    let server = MockServer::start_async().await;

    tokio::spawn(async move {
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
    target.mock(|when, then| {
        when.any_request();
        then.status(201);
    });
    let gateway = MockServer::start();

    gateway.forward_to(target.base_url(), |rule| {
        let mut pending_filter = None;
        rule.filter(|when| pending_filter = Some(when));
        pending_filter.unwrap().path("/wanted");
    });

    let client = Client::new();
    assert_eq!(client.get(gateway.url("/wanted")).send().unwrap().status(), 201);
    assert_eq!(client.get(gateway.url("/other")).send().unwrap().status(), 404);
}

// Preserves staged setup: a saved filter must still affect proxying before the outer callback returns.
#[cfg(feature = "proxy")]
#[test]
fn proxy_filter_can_be_staged_inside_the_outer_callback() {
    let target = MockServer::start();
    target.mock(|when, then| {
        when.any_request();
        then.status(201);
    });
    let gateway = MockServer::start();

    gateway.proxy(|rule| {
        let mut pending_filter = None;
        rule.filter(|when| pending_filter = Some(when));
        pending_filter.unwrap().path("/wanted");
    });

    let client = Client::builder()
        .proxy(reqwest::Proxy::all(gateway.base_url()).unwrap())
        .build()
        .unwrap();
    assert_eq!(client.get(target.url("/wanted")).send().unwrap().status(), 201);
    assert_eq!(client.get(target.url("/other")).send().unwrap().status(), 404);
}

fn assert_send_sync<T: Send + Sync>() {}

// Checks every public builder is Send + Sync so users can move or share them across threads.
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
