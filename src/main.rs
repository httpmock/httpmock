use std::{env, path::PathBuf};

use clap::Parser;
use httpmock::server::HttpMockServerBuilder;
use tracing_subscriber::EnvFilter;

/// Holds command line parameters provided by the user.
#[derive(Parser, Debug)]
#[clap(
    version = "0.6",
    author = "Alexander Liesenfeld <alexander.liesenfeld@outlook.com>"
)]
struct ExecutionParameters {
    #[clap(short, long, env = "HTTPMOCK_PORT", default_value = "5050")]
    pub port: u16,
    #[clap(short, long, env = "HTTPMOCK_EXPOSE")]
    pub expose: bool,
    #[clap(short, long, env = "HTTPMOCK_MOCK_FILES_DIR")]
    pub mock_files_dir: Option<PathBuf>,
    #[clap(short, long, env = "HTTPMOCK_DISABLE_ACCESS_LOG")]
    pub disable_access_log: bool,
    #[clap(
        short,
        long,
        env = "HTTPMOCK_REQUEST_HISTORY_LIMIT",
        default_value = "100"
    )]
    pub request_history_limit: usize,
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("httpmock=info")),
        )
        .init();

    let params: ExecutionParameters = ExecutionParameters::parse();

    tracing::info!("██╗  ██╗████████╗████████╗██████╗ ███╗   ███╗ ██████╗  ██████╗██╗  ██╗");
    tracing::info!("██║  ██║╚══██╔══╝╚══██╔══╝██╔══██╗████╗ ████║██╔═══██╗██╔════╝██║ ██╔╝");
    tracing::info!("███████║   ██║      ██║   ██████╔╝██╔████╔██║██║   ██║██║     █████╔╝");
    tracing::info!("██╔══██║   ██║      ██║   ██╔═══╝ ██║╚██╔╝██║██║   ██║██║     ██╔═██╗");
    tracing::info!("██║  ██║   ██║      ██║   ██║     ██║ ╚═╝ ██║╚██████╔╝╚██████╗██║  ██╗");
    tracing::info!("╚═╝  ╚═╝   ╚═╝      ╚═╝   ╚═╝     ╚═╝     ╚═╝ ╚═════╝  ╚═════╝╚═╝  ╚═╝");

    tracing::info!(
        "Starting {} server V{}",
        env!("CARGO_PKG_NAME"),
        env!("CARGO_PKG_VERSION")
    );

    tracing::info!("{params:?}");

    let server = HttpMockServerBuilder::new()
        .port(params.port)
        .expose(params.expose)
        .print_access_log(!params.disable_access_log)
        .history_limit(params.request_history_limit)
        .static_mock_dir_option(params.mock_files_dir)
        .build()
        .unwrap();

    server
        .start_with_signals(None, shutdown_signal())
        .await
        .expect("an error occurred during mock server execution");
}

#[cfg(not(target_os = "windows"))]
async fn shutdown_signal() {
    let mut hangup_stream = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::hangup())
        .expect("Cannot install SIGINT signal handler");
    let mut sigint_stream =
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::interrupt())
            .expect("Cannot install SIGINT signal handler");
    let mut sigterm_stream =
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("Cannot install SIGINT signal handler");

    tokio::select! {
        _val = hangup_stream.recv() => tracing::trace!("Received SIGINT"),
        _val = sigint_stream.recv() => tracing::trace!("Received SIGINT"),
        _val = sigterm_stream.recv() => tracing::trace!("Received SIGTERM"),
    }
}

#[cfg(target_os = "windows")]
async fn shutdown_signal() {
    tokio::signal::ctrl_c()
        .await
        .expect("Cannot install CTRL+C signal handler");
}
