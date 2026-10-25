//! Keep this in its own integration-test binary: rustls's default provider can only be installed once
//! per process, so another test installing one first would make this one fail.

#![cfg(feature = "proxy")]

use httpmock::MockServer;

#[test]
fn starting_a_server_leaves_the_default_provider_to_the_application() {
    // With `proxy`, building the server also builds its HTTPS client.
    let _server = MockServer::start();

    rustls::crypto::aws_lc_rs::default_provider().install_default().unwrap();
}
