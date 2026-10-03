use std::{
    any::Any,
    panic::{AssertUnwindSafe, catch_unwind},
};

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

struct InvalidPath;

impl TryFrom<InvalidPath> for String {
    type Error = &'static str;

    fn try_from(_: InvalidPath) -> Result<Self, Self::Error> {
        Err("invalid path")
    }
}

#[test]
fn caught_setter_panic_preserves_earlier_matchers() {
    let server = MockServer::start();
    let mock = server.mock(|when, then| {
        let when = when.path("/retained");
        let result = catch_unwind(AssertUnwindSafe(|| when.path(InvalidPath)));
        assert!(result.is_err());
        then.status(201);
    });

    let client = Client::new();
    assert_eq!(client.get(server.url("/other")).send().unwrap().status(), 404);
    assert_eq!(client.get(server.url("/retained")).send().unwrap().status(), 201);
    mock.assert();
}

#[cfg(feature = "record")]
fn fresh_recording_builder() -> httpmock::RecordingRuleBuilder {
    httpmock::RecordingRuleBuilder {
        config: Default::default(),
    }
}

#[cfg(feature = "record")]
#[test]
fn recording_builders_can_own_and_share_configuration() {
    let first = fresh_recording_builder();
    let second = httpmock::RecordingRuleBuilder {
        config: first.config.clone(),
    };

    second.record_response_delays(true);
    assert!(first.config.take().record_response_delays);
}

#[cfg(feature = "record")]
#[test]
fn caught_recording_filter_panic_preserves_configuration() {
    let rule = fresh_recording_builder().record_response_delays(true).filter(|when| {
        when.path("/retained");
    });
    let config = rule.config.clone();

    let result = catch_unwind(AssertUnwindSafe(|| {
        rule.filter(|when| {
            when.path("/updated").path(InvalidPath);
        });
    }));

    assert!(result.is_err());
    let config = config.take();
    assert!(config.record_response_delays);
    assert_eq!(config.request_requirements.path.as_deref(), Some("/updated"));
}
