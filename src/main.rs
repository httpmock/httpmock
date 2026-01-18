use std::{env, fs, io::{self, Write}, path::PathBuf, process::Command};

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
    /// Jumps into an interactive mode to type mocks in an editor.
    #[clap(short, long)]
    pub interactive: bool,
    /// Interactive mode that asks to load and save.
    #[clap(short = 'I', long = "interactive-ask")]
    pub interactive_ask: bool,
    /// Automatically load persistent mock file if it exists.
    #[clap(short, long)]
    pub load: bool,
    /// Automatically save persistent mock file.
    #[clap(short, long)]
    pub save: bool,
}

impl ExecutionParameters {
    pub fn is_interactive(&self) -> bool {
        self.interactive || self.interactive_ask
    }

    pub fn is_save(&self) -> bool {
        self.save
    }
}

fn open_editor(path: &std::path::Path) -> Result<(), Box<dyn std::error::Error>> {
    let editor = env::var("VISUAL")
        .or_else(|_| env::var("EDITOR"))
        .unwrap_or_else(|_| {
            if cfg!(windows) {
                "notepad".to_string()
            } else {
                "vi".to_string()
            }
        });

    let mut child = Command::new(&editor)
        .arg(path)
        .spawn()
        .map_err(|e| format!("failed to spawn editor '{}': {}", editor, e))?;

    child.wait()?;
    Ok(())
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

    let mut mock_files_dir = params.mock_files_dir.clone();

    let mut temp_dir_to_cleanup = None;
    if params.is_interactive() {
        let temp_dir = env::temp_dir().join(format!("httpmock-{}", std::process::id()));
        fs::create_dir_all(&temp_dir).expect("Cannot create temporary directory");
        temp_dir_to_cleanup = Some(temp_dir.clone());

        let home = env::var("HOME")
            .or_else(|_| env::var("USERPROFILE"))
            .expect("Could not find home directory");
        let persistent_file = PathBuf::from(home).join(".httpmock").join("mocks.yaml");

        // Optionally copy existing mocks from mock_files_dir to the temp dir
        if let Some(ref dir) = mock_files_dir {
            if dir.is_dir() {
                for entry in fs::read_dir(dir).expect("Cannot read mock directory") {
                    if let Ok(entry) = entry {
                        let path = entry.path();
                        if path.is_file() {
                            let _ = fs::copy(&path, temp_dir.join(path.file_name().unwrap()));
                        }
                    }
                }
            }
        }

        let temp_file = temp_dir.join("interactive.yaml");

        let mut load_from_persistent = params.load;
        if params.interactive_ask && persistent_file.exists() {
            print!("Existing persistent mock file found. Do you want to open it? (y/n): ");
            let _ = io::stdout().flush();
            let mut input = String::new();
            if io::stdin().read_line(&mut input).is_ok() {
                let input = input.trim().to_lowercase();
                if input == "y" || input == "yes" {
                    load_from_persistent = true;
                }
            }
        }

        if load_from_persistent && persistent_file.exists() {
            fs::copy(&persistent_file, &temp_file).expect("Cannot copy persistent mock file to temp");
        }

        if !temp_file.exists() {
            let initial_content = r#"# Type your mocks here. Use --- to separate multiple documents.
# See all possible attributes you can use here at https://github.com/httpmock/httpmock/blob/master/tests/resources/static_yaml_mock.yaml.
---
when:
  method: GET
  path: /hello
then:
  status: 200
  body: world
"#;
            fs::write(&temp_file, initial_content).expect("Cannot write to temporary file");
        }

        loop {
            if let Err(e) = open_editor(&temp_file) {
                tracing::error!("Failed to open editor: {}", e);
                break;
            }

            let content = fs::read_to_string(&temp_file).expect("Cannot read temporary file");

            let validated = {
                #[cfg(feature = "record")]
                {
                    match httpmock::server::validate_mock_yaml(&content) {
                        Ok(_) => true,
                        Err(e) => {
                            eprintln!("\nInvalid YAML configuration:\n{}", e);
                            print!("Press Enter to try again or Ctrl+C to abort...");
                            let _ = io::stdout().flush();
                            let mut line = String::new();
                            let _ = io::stdin().read_line(&mut line);
                            false
                        }
                    }
                }
                #[cfg(not(feature = "record"))]
                {
                    tracing::warn!("Interactive mode requires 'record' feature for validation.");
                    true
                }
            };

            if validated {
                let save_persistent = params.is_save();

                if save_persistent {
                    let base_dir = persistent_file.parent().unwrap();
                    fs::create_dir_all(base_dir).expect("Cannot create .httpmock directory in home");
                    fs::copy(&temp_file, &persistent_file).expect("Cannot save mock to home directory");
                }
                mock_files_dir = Some(temp_dir);
                break;
            }
        }
    }

    let server = HttpMockServerBuilder::new()
        .port(params.port)
        .expose(params.expose)
        .print_access_log(!params.disable_access_log)
        .history_limit(params.request_history_limit)
        .static_mock_dir_option(mock_files_dir)
        .build()
        .unwrap();

    server
        .start_with_signals(None, shutdown_signal())
        .await
        .expect("an error occurred during mock server execution");

    if let Some(dir) = temp_dir_to_cleanup {
        if params.interactive_ask {
            let temp_file = dir.join("interactive.yaml");
            if temp_file.exists() {
                print!("Do you want to save your changes to the persistent mock file? (y/n): ");
                let _ = io::stdout().flush();
                let mut input = String::new();
                if io::stdin().read_line(&mut input).is_ok() {
                    let input = input.trim().to_lowercase();
                    if input == "y" || input == "yes" {
                        let home = env::var("HOME")
                            .or_else(|_| env::var("USERPROFILE"))
                            .expect("Could not find home directory");
                        let persistent_file = PathBuf::from(home).join(".httpmock").join("mocks.yaml");
                        let base_dir = persistent_file.parent().unwrap();
                        fs::create_dir_all(base_dir)
                            .expect("Cannot create .httpmock directory in home");
                        fs::copy(&temp_file, &persistent_file)
                            .expect("Cannot save mock to home directory");
                    }
                }
            }
        }
        let _ = fs::remove_dir_all(dir);
    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    #[test]
    fn test_params_parsing() {
        let params = ExecutionParameters::try_parse_from(&["httpmock", "-I"]).unwrap();
        assert!(params.is_interactive());
        assert!(params.interactive_ask);
        assert!(!params.interactive);

        let params = ExecutionParameters::try_parse_from(&["httpmock", "-i"]).unwrap();
        assert!(params.is_interactive());
        assert!(!params.interactive_ask);
        assert!(params.interactive);

        // Grouped short flags
        let params = ExecutionParameters::try_parse_from(&["httpmock", "-is"]).unwrap();
        assert!(params.is_interactive());
        assert!(params.is_save());
        assert!(!params.load);

        let params = ExecutionParameters::try_parse_from(&["httpmock", "-isl"]).unwrap();
        assert!(params.is_interactive());
        assert!(params.is_save());
        assert!(params.load);

        // Individual flags
        let params = ExecutionParameters::try_parse_from(&["httpmock", "-i"]).unwrap();
        assert!(params.is_interactive());
        assert!(!params.is_save());
        assert!(!params.load);

        let params = ExecutionParameters::try_parse_from(&["httpmock", "-l"]).unwrap();
        assert!(!params.is_interactive());
        assert!(params.load);
        assert!(!params.is_save());

        let params = ExecutionParameters::try_parse_from(&["httpmock", "-s"]).unwrap();
        assert!(!params.is_interactive());
        assert!(!params.load);
        assert!(params.is_save());
    }
}
