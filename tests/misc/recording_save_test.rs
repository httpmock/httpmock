use httpmock::prelude::*;
use reqwest::blocking::Client;

#[test]
fn saving_the_same_scenario_twice_keeps_both_files() {
    let target_server = MockServer::start();
    target_server.mock(|when, then| {
        when.any_request();
        then.status(200).body("hi");
    });

    let recording_server = MockServer::start();
    recording_server.forward_to(target_server.base_url(), |rule| {
        rule.filter(|when| {
            when.any_request();
        });
    });
    let recording = recording_server.record(|rule| {
        rule.filter(|when| {
            when.any_request();
        });
    });

    Client::new().get(recording_server.url("/hello")).send().unwrap();

    let dir = std::path::Path::new(env!("CARGO_TARGET_TMPDIR")).join("recording_save_test");
    let _ = std::fs::remove_dir_all(&dir);

    // Both saves almost always land in the same second, so they would get
    // the same timestamped name.
    let first = recording.save_to(&dir, "scenario").unwrap();
    let second = recording.save_to(&dir, "scenario").unwrap();

    assert_ne!(first, second);
    assert!(first.exists());
    assert!(second.exists());
}
