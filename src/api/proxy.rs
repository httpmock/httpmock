use crate::{
    api::server::MockServer,
    common::{
        data::RecordingRuleConfig,
        data::RequestRequirements,
        util::{write_file, Join},
    },
    When,
};
use std::{
    cell::Cell,
    path::{Path, PathBuf},
    rc::Rc,
};
use bytes::Bytes;

/// Represents a forwarding rule on a [MockServer](struct.MockServer.html), allowing HTTP requests
/// that meet specific criteria to be redirected to a designated destination. Each rule is
/// uniquely identified by an ID within the server context.
pub struct ForwardingRule<'a> {
    pub id: usize,
    pub(crate) server: &'a MockServer,
}

impl<'a> ForwardingRule<'a> {
    pub fn new(id: usize, server: &'a MockServer) -> Self {
        Self { id, server }
    }

    /// Synchronously deletes the forwarding rule from the mock server.
    /// This method blocks the current thread until the deletion has been completed, ensuring that the rule is no longer active and will not affect any further requests.
    ///
    /// # Panics
    /// Panics if the deletion fails, typically due to issues such as the rule not existing or server connectivity problems.
    pub fn delete(&mut self) {
        self.delete_async().join();
    }

    /// Asynchronously deletes the forwarding rule from the mock server.
    /// This method performs the deletion without blocking the current thread,
    /// making it suitable for use in asynchronous applications where maintaining responsiveness
    /// or concurrent execution is necessary.
    ///
    /// # Panics
    /// Panics if the deletion fails, typically due to issues such as the rule not existing or server connectivity problems.
    /// This method will raise an immediate panic on such failures, signaling that the operation could not be completed as expected.
    pub async fn delete_async(&self) {
        self.server
            .server_adapter
            .as_ref()
            .unwrap()
            .delete_forwarding_rule(self.id)
            .await
            .expect("could not delete mock from server");
    }
}
/// Provides methods for managing a proxy rule from the server.
pub struct ProxyRule<'a> {
    pub id: usize,
    pub(crate) server: &'a MockServer,
}

impl<'a> ProxyRule<'a> {
    pub fn new(id: usize, server: &'a MockServer) -> Self {
        Self { id, server }
    }

    /// Synchronously deletes the proxy rule from the server.
    /// This method blocks the current thread until the deletion is complete, ensuring that
    /// the rule is removed and will no longer redirect any requests.
    ///
    /// # Usage
    /// This method is typically used in synchronous environments where immediate removal of the
    /// rule is necessary and can afford a blocking operation.
    ///
    /// # Panics
    /// Panics if the deletion fails due to server-related issues such as connectivity problems,
    /// or if the rule does not exist on the server.
    pub fn delete(&mut self) {
        self.delete_async().join();
    }

    /// Asynchronously deletes the proxy rule from the server.
    /// This method allows for non-blocking operations, suitable for asynchronous environments
    /// where tasks are performed concurrently without interrupting the main workflow.
    ///
    /// # Usage
    /// Ideal for use in modern async/await patterns in Rust, providing a way to handle resource
    /// cleanup without stalling other operations.
    ///
    /// # Panics
    /// Panics if the deletion fails due to server-related issues such as connectivity problems,
    /// or if the rule does not exist on the server. This method raises an immediate panic to
    /// indicate that the operation could not be completed as expected.
    pub async fn delete_async(&self) {
        self.server
            .server_adapter
            .as_ref()
            .unwrap()
            .delete_proxy_rule(self.id)
            .await
            .expect("could not delete mock from server");
    }
}

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
    #[cfg(feature = "record")]
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
    #[cfg(feature = "record")]
    pub async fn export_async(&self) -> Result<Option<Bytes>, Box<dyn std::error::Error>> {
        let rec = self.server
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
    #[cfg(feature = "record")]
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
    #[cfg(feature = "record")]
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
            return Ok(write_file(&filepath, &bytes, true).await?);
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
    #[cfg(feature = "record")]
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
    #[cfg(feature = "record")]
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

pub struct ForwardingRuleBuilder {
    pub(crate) request_requirements: Rc<Cell<RequestRequirements>>,
    pub(crate) headers: Rc<Cell<Vec<(String, String)>>>,
}

impl ForwardingRuleBuilder {
    pub fn add_request_header<Key: Into<String>, Value: Into<String>>(
        mut self,
        key: Key,
        value: Value,
    ) -> Self {
        let mut headers = self.headers.take();
        headers.push((key.into(), value.into()));
        self.headers.set(headers);
        self
    }

    pub fn filter<WhenSpecFn>(mut self, when: WhenSpecFn) -> Self
    where
        WhenSpecFn: FnOnce(When),
    {
        when(When {
            expectations: self.request_requirements.clone(),
        });
        self
    }
}

pub struct ProxyRuleBuilder {
    // TODO: These fields are visible to the user, make them not public
    pub(crate) request_requirements: Rc<Cell<RequestRequirements>>,
    pub(crate) headers: Rc<Cell<Vec<(String, String)>>>,
}

impl ProxyRuleBuilder {
    pub fn add_request_header<Key: Into<String>, Value: Into<String>>(
        mut self,
        key: Key,
        value: Value,
    ) -> Self {
        let mut headers = self.headers.take();
        headers.push((key.into(), value.into()));
        self.headers.set(headers);
        self
    }

    pub fn filter<WhenSpecFn>(mut self, when: WhenSpecFn) -> Self
    where
        WhenSpecFn: FnOnce(When),
    {
        when(When {
            expectations: self.request_requirements.clone(),
        });

        self
    }
}

pub struct RecordingRuleBuilder {
    pub config: Rc<Cell<RecordingRuleConfig>>,
}

impl RecordingRuleBuilder {
    pub fn record_request_header<IntoString: Into<String>>(mut self, header: IntoString) -> Self {
        let mut config = self.config.take();
        config.record_headers.push(header.into());
        self.config.set(config);
        self
    }

    pub fn record_request_headers<IntoString: Into<String>>(
        mut self,
        headers: Vec<IntoString>,
    ) -> Self {
        let mut config = self.config.take();
        config
            .record_headers
            .extend(headers.into_iter().map(Into::into));
        self.config.set(config);
        self
    }

    pub fn filter<WhenSpecFn>(mut self, when: WhenSpecFn) -> Self
    where
        WhenSpecFn: FnOnce(When),
    {
        let mut config = self.config.take();

        let mut request_requirements = Rc::new(Cell::new(config.request_requirements));

        when(When {
            expectations: request_requirements.clone(),
        });

        config.request_requirements = request_requirements.take();

        self.config.set(config);

        self
    }

    pub fn record_response_delays(mut self, record: bool) -> Self {
        let mut config = self.config.take();
        config.record_response_delays = record;
        self.config.set(config);

        self
    }
}
