use std::{sync::Mutex, thread};

use httpmock::server::{HttpMockServer, HttpMockServerBuilder};
use tokio::task::LocalSet;
mod examples;
mod matchers;
mod misc;
mod utils;

/// The rest of this file is only required to simulate that a standalone mock server is
/// running somewhere else.
pub fn with_standalone_server() {
    let disable_server = std::env::var("HTTPMOCK_TESTS_DISABLE_SIMULATED_STANDALONE_SERVER")
        .unwrap_or_else(|_| "0".to_string());

    if disable_server == "1" {
        tracing::info!("Skipping creating a simulated mock server.");
        return;
    }

    let mut started = SERVER_STARTED.lock().unwrap();
    if !*started {
        thread::spawn(move || {
            let srv: HttpMockServer = HttpMockServerBuilder::new()
                .port(5050)
                .build()
                .expect("cannot create mock server");

            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap();
            LocalSet::new().block_on(&runtime, srv.start())
        });
    }
    *started = true
}

static SERVER_STARTED: Mutex<bool> = Mutex::new(false);
