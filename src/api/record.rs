//! Client-side API for recordings.

use std::{
    cell::Cell,
    path::{Path, PathBuf},
    rc::Rc,
};

use bytes::Bytes;

use crate::{
    When,
    api::server::MockServer,
    common::{
        data::RecordingRuleConfig,
        util::{Join, write_file},
    },
};

/// Represents a recording of interactions (requests and responses) on a mock server.
/// This structure is used to capture and store detailed information about the HTTP
/// requests received by the server and the corresponding responses sent back.
///
/// The `Recording` structure can be especially useful in testing scenarios where
/// monitoring and verifying the exact behavior of HTTP interactions is necessary,
/// such as ensuring that a server is responding with the correct headers, body content,
/// and status codes in response to various requests.
pub struct Recording<'a> {
    pub id: usize,
    pub(crate) server: &'a MockServer,
}

/// Represents a reference to a recording of HTTP interactions on a mock server.
/// This struct allows for management and retrieval of recorded data, such as viewing,
/// exporting, and deleting the recording.
impl<'a> Recording<'a> {
    pub fn new(id: usize, server: &'a MockServer) -> Self {
        Self { id, server }
    }

    /// Synchronously deletes the recording from the mock server.
    /// This method blocks the current thread until the deletion is completed,
    /// ensuring that the recording is fully removed before proceeding.
    ///
    /// # Panics
    /// Panics if the deletion fails, which can occur if the recording does not exist,
    /// or there are server connectivity issues.
    pub fn delete(&mut self) {
        self.delete_async().join();
    }

    /// Asynchronously deletes the recording from the mock server.
    /// This method allows for non-blocking operations, suitable for asynchronous environments
    /// where tasks are performed concurrently without waiting for the deletion to complete.
    ///
    /// # Panics
    /// Panics if the deletion fails, typically due to the recording not existing on the server
    /// or connectivity issues with the server. This method provides immediate feedback by
    /// raising a panic on such failures.
    pub async fn delete_async(&self) {
        self.server
            .server_adapter
            .as_ref()
            .unwrap()
            .delete_recording(self.id)
            .await
            .expect("could not delete mock from server");
    }

    /// Synchronously export the recording as YAML.
    ///
    /// # Returns
    /// Returns a `Result` containing the YAML of the recording as `Option<Bytes>` (absent when no recording could be found),
    /// or an error if the export operation fails.
    ///
    /// # Errors
    /// Errors if the recording cannot be created due to serialization issues or issues with connecting to a remote server.
    pub fn export(&self) -> Result<Option<Bytes>, Box<dyn std::error::Error>> {
        self.export_async().join()
    }

    /// Asynchronously export the recording as YAML.
    ///
    /// # Returns
    /// Returns a `Result` containing the YAML of the recording as `Option<Bytes>` (absent when no recording could be found),
    /// or an error if the export operation fails.
    ///
    /// # Errors
    /// Errors if the recording cannot be created due to serialization issues or issues with connecting to a remote server.
    pub async fn export_async(&self) -> Result<Option<Bytes>, Box<dyn std::error::Error>> {
        let rec = self
            .server
            .server_adapter
            .as_ref()
            .unwrap()
            .export_recording(self.id)
            .await?;
        Ok(rec)
    }

    /// Synchronously saves the recording to a specified directory with a timestamped filename.
    /// The file is named using a combination of the provided scenario name and a UNIX timestamp, formatted as YAML.
    ///
    /// # Parameters
    /// - `dir`: The directory path where the file will be saved.
    /// - `scenario_name`: A descriptive name for the scenario, used as part of the filename.
    ///
    /// # Returns
    /// Returns a `Result` containing the `PathBuf` of the created file, or an error if the save operation fails.
    ///
    /// # Errors
    /// Errors if the file cannot be written due to issues like directory permissions, unavailable disk space, or other I/O errors.
    pub fn save_to<PathRef: AsRef<Path>, IntoString: Into<String>>(
        &self,
        dir: PathRef,
        scenario_name: IntoString,
    ) -> Result<PathBuf, Box<dyn std::error::Error>> {
        self.save_to_async(dir, scenario_name).join()
    }

    /// Asynchronously saves the recording to the specified directory with a scenario-specific and timestamped filename.
    ///
    /// # Parameters
    /// - `dir`: The directory path where the file will be saved.
    /// - `scenario`: A string representing the scenario name, used as part of the filename.
    ///
    /// # Returns
    /// Returns an `async` `Result` with the `PathBuf` of the saved file or an error if unable to save.
    pub async fn save_to_async<PathRef: AsRef<Path>, IntoString: Into<String>>(
        &self,
        dir: PathRef,
        scenario: IntoString,
    ) -> Result<PathBuf, Box<dyn std::error::Error>> {
        let rec = self.export_async().await?;

        let scenario = scenario.into();
        let dir = dir.as_ref();
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_secs();
        let filename = format!("{}_{}.yaml", scenario, timestamp);
        let filepath = dir.join(filename);

        if let Some(bytes) = rec {
            return write_file(&filepath, &bytes, true).await;
        }

        Err("No recording data available".into())
    }

    /// Synchronously saves the recording to the default directory (`target/httpmock/recordings`) with the scenario name.
    ///
    /// # Parameters
    /// - `scenario_name`: A descriptive name for the scenario, which helps identify the recording file.
    ///
    /// # Returns
    /// Returns a `Result` with the `PathBuf` to the saved file or an error.
    pub fn save<IntoString: Into<String>>(
        &self,
        scenario_name: IntoString,
    ) -> Result<PathBuf, Box<dyn std::error::Error>> {
        self.save_async(scenario_name).join()
    }

    /// Asynchronously saves the recording to the default directory structured under `target/httpmock/recordings`.
    ///
    /// # Parameters
    /// - `scenario`: A descriptive name for the test scenario, used in naming the saved file.
    ///
    /// # Returns
    /// Returns an `async` `Result` with the `PathBuf` of the saved file or an error.
    pub async fn save_async<IntoString: Into<String>>(
        &self,
        scenario: IntoString,
    ) -> Result<PathBuf, Box<dyn std::error::Error>> {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("target")
            .join("httpmock")
            .join("recordings");
        self.save_to_async(path, scenario).await
    }
}

pub struct RecordingRuleBuilder {
    pub config: Rc<Cell<RecordingRuleConfig>>,
}

impl RecordingRuleBuilder {
    pub fn record_request_header<IntoString: Into<String>>(self, header: IntoString) -> Self {
        let mut config = self.config.take();
        config.record_headers.push(header.into());
        self.config.set(config);
        self
    }

    pub fn record_request_headers<IntoString: Into<String>>(self, headers: Vec<IntoString>) -> Self {
        let mut config = self.config.take();
        config.record_headers.extend(headers.into_iter().map(Into::into));
        self.config.set(config);
        self
    }

    pub fn filter<WhenSpecFn>(self, when: WhenSpecFn) -> Self
    where
        WhenSpecFn: FnOnce(When),
    {
        let mut config = self.config.take();

        let request_requirements = Rc::new(Cell::new(config.request_requirements));

        when(When {
            expectations: request_requirements.clone(),
        });

        config.request_requirements = request_requirements.take();

        self.config.set(config);

        self
    }

    pub fn record_response_delays(self, record: bool) -> Self {
        let mut config = self.config.take();
        config.record_response_delays = record;
        self.config.set(config);

        self
    }
}
