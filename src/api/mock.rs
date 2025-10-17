use std::{io::Write, net::SocketAddr};
use tabwriter::TabWriter;

use crate::api::output;
#[cfg(feature = "color")]
use colored::*;
use serde::{Deserialize, Serialize};

use crate::api::server::MockServer;

use crate::common::util::Join;

/// Provides a reference to a mock configuration stored on a [MockServer](struct.MockServer.html).
/// This structure is used for interacting with, monitoring, and managing a specific mock's lifecycle,
/// such as observing call counts or removing the mock from the server.
///
/// This reference allows you to control and verify the behavior of the server in response to
/// incoming HTTP requests that match the mock criteria.
///
/// # Example
/// Demonstrates how to create and manipulate a mock on the server. This includes monitoring its usage
/// and effectively managing its lifecycle by removing it when necessary.
///
/// ```rust
/// use httpmock::prelude::*;
/// use reqwest::blocking::get;
///
/// // Arrange
/// let server = MockServer::start();
///
/// // Create and configure a mock
/// let mut mock = server.mock(|when, then| {
///    when.path("/test");
///    then.status(202);
/// });
///
/// // Act by sending a request and verifying the mock's hit count
/// let response1 = get(&server.url("/test")).unwrap();
/// assert_eq!(mock.hits(), 1); // Verify the mock was triggered
///
/// // Remove the mock and test the server's response to the same path again
/// mock.delete();
/// let response2 = get(&server.url("/test")).unwrap();
///
/// // Assert
/// assert_eq!(response1.status(), 202);
/// assert_eq!(response2.status(), 404); // Expect a 404 status after the mock is deleted
/// ```
pub struct Mock<'a> {
    // Please find the reason why id is public in
    // https://github.com/httpmock/httpmock/issues/26.
    pub id: usize,
    pub(crate) server: &'a MockServer,
}

impl<'a> Mock<'a> {
    pub fn new(id: usize, server: &'a MockServer) -> Self {
        Self { id, server }
    }

    /// Verifies that the mock server received exactly one HTTP request matching all specified
    /// request conditions for this mock. This method is useful for confirming that a particular
    /// operation interacts with the server as expected in test scenarios.
    ///
    /// **Attention**: To assert receipt of multiple requests, use [Mock::assert_hits](struct.Mock.html#method.assert_hits)
    /// or [Mock::hits](struct.Mock.html#method.hits) methods instead.
    ///
    /// # Example
    /// Demonstrates creating a mock to match a specific request path, sending a request to that path,
    /// and then verifying that exactly one such request was received.
    ///
    /// ```rust
    /// use httpmock::prelude::*;
    /// use reqwest::blocking::get;
    ///
    /// // Arrange: Start a mock server and set up a mock
    /// let server = MockServer::start();
    /// let mut mock = server.mock(|when, then| {
    ///     when.path("/hits");
    ///     then.status(200);
    /// });
    ///
    /// // Act: Send a request to the specified path
    /// get(&server.url("/hits")).unwrap();
    ///
    /// // Assert: Check that the server received exactly one request that matched the mock
    /// mock.assert();
    /// ```
    ///
    /// # Panics
    /// This method will panic if the mock server did not receive exactly one matching request or if
    /// there are issues with the mock server's availability.
    pub fn assert(&self) {
        self.assert_async().join()
    }

    /// Asynchronously verifies that the mock server received exactly one HTTP request matching all
    /// specified request conditions for this mock. This method is suited for asynchronous testing environments
    /// where operations against the mock server occur non-blockingly.
    ///
    /// **Attention**: To assert the receipt of multiple requests asynchronously, consider using
    /// [Mock::assert_hits_async](struct.Mock.html#method.assert_hits_async) or
    /// [Mock::hits_async](struct.Mock.html#method.hits_async).
    ///
    /// # Example
    /// Demonstrates setting up an asynchronous mock, sending a request, and verifying that exactly
    /// one such request was received.
    ///
    /// ```rust
    /// use httpmock::prelude::*;
    /// use reqwest::get;
    /// use syn::token;
    ///
    /// let rt = tokio::runtime::Runtime::new().unwrap();
    /// rt.block_on(async {
    ///     // Arrange: Start a mock server asynchronously and set up a mock
    ///     let server = MockServer::start_async().await;
    ///     let mut mock = server.mock_async(|when, then| {
    ///         when.path("/hits");
    ///         then.status(200);
    ///     }).await;
    ///
    ///     // Act: Send a request to the specified path asynchronously
    ///     get(&server.url("/hits")).await.unwrap();
    ///
    ///     // Assert: Check that the server received exactly one request that matched the mock
    ///     mock.assert_async().await;
    /// });
    /// ```
    ///
    /// # Panics
    /// This method will panic if the mock server did not receive exactly one matching request or if
    /// there are issues with the mock server's availability.
    pub async fn assert_async(&self) {
        self.assert_hits_async(1).await
    }

    /// Verifies that the mock server received the specified number of HTTP requests matching all
    /// the request conditions defined for this mock.
    ///
    /// This method is useful for confirming that a series of operations interact with the server as expected
    /// within test scenarios, especially when specific interaction counts are significant.
    ///
    /// **Attention**: Use [Mock::assert](struct.Mock.html#method.assert) for the common case of asserting exactly one hit.
    ///
    /// # Example
    /// Demonstrates creating a mock, sending multiple requests, and verifying the number of received requests
    /// matches expectations.
    ///
    /// ```rust
    /// use httpmock::prelude::*;
    /// use reqwest::blocking::get;
    ///
    /// // Arrange: Start a mock server and configure a mock
    /// let server = MockServer::start();
    /// let mut mock = server.mock(|when, then| {
    ///     when.path("/hits");
    ///     then.status(200);
    /// });
    ///
    /// // Act: Send multiple requests to the configured path
    /// get(&server.url("/hits")).unwrap();
    /// get(&server.url("/hits")).unwrap();
    ///
    /// // Assert: Check that the server received exactly two requests that matched the mock
    /// mock.assert_hits(2);
    /// ```
    ///
    /// # Panics
    /// This method will panic if the actual number of hits differs from the specified `hits`, or if
    /// there are issues with the mock server's availability.
    #[deprecated(since = "0.8.0", note = "please use `assert_calls` instead")]
    pub fn assert_hits(&self, hits: usize) {
        self.assert_calls(hits)
    }

    /// Verifies that the mock server received the specified number of HTTP requests matching all
    /// the request conditions defined for this mock.
    ///
    /// This method is useful for confirming that a series of operations interact with the server as expected
    /// within test scenarios, especially when specific interaction counts are significant.
    ///
    /// **Attention**: Use [Mock::assert](struct.Mock.html#method.assert) for the common case of asserting exactly one hit.
    ///
    /// # Example
    /// Demonstrates creating a mock, sending multiple requests, and verifying the number of received requests
    /// matches expectations.
    ///
    /// ```rust
    /// use httpmock::prelude::*;
    /// use reqwest::blocking::get;
    ///
    /// // Arrange: Start a mock server and configure a mock
    /// let server = MockServer::start();
    /// let mut mock = server.mock(|when, then| {
    ///     when.path("/hits");
    ///     then.status(200);
    /// });
    ///
    /// // Act: Send multiple requests to the configured path
    /// get(&server.url("/hits")).unwrap();
    /// get(&server.url("/hits")).unwrap();
    ///
    /// // Assert: Check that the server received exactly two requests that matched the mock
    /// mock.assert_calls(2);
    /// ```
    ///
    /// # Panics
    /// This method will panic if the actual number of hits differs from the specified `hits`, or if
    /// there are issues with the mock server's availability.
    pub fn assert_calls(&self, count: usize) {
        self.assert_calls_async(count).join()
    }

    /// Asynchronously verifies that the mock server received the specified number of HTTP requests
    /// matching all defined request conditions for this mock.
    ///
    /// This method supports asynchronous testing environments, enabling non-blocking verification
    /// of multiple interactions with the mock server. It's particularly useful when exact counts
    /// of interactions are critical for test assertions.
    ///
    /// **Attention**: For asserting exactly one request asynchronously, use
    /// [Mock::assert_async](struct.Mock.html#method.assert_async) for simpler syntax.
    ///
    /// # Example
    /// Demonstrates setting up an asynchronous mock, sending multiple requests, and verifying the
    /// number of requests received matches expectations.
    ///
    /// ```rust
    /// use httpmock::prelude::*;
    /// use reqwest::get;
    ///
    /// let rt = tokio::runtime::Runtime::new().unwrap();
    /// rt.block_on(async {
    ///     // Arrange: Start a mock server asynchronously and set up a mock
    ///     let server = MockServer::start_async().await;
    ///     let mut mock = server.mock_async(|when, then| {
    ///         when.path("/hits");
    ///         then.status(200);
    ///     }).await;
    ///
    ///     // Act: Send multiple asynchronous requests to the configured path
    ///     get(&server.url("/hits")).await.unwrap();
    ///     get(&server.url("/hits")).await.unwrap();
    ///
    ///     // Assert: Check that the server received exactly two requests that matched the mock
    ///     mock.assert_hits_async(2).await;
    /// });
    /// ```
    ///
    /// # Panics
    /// This method will panic if the actual number of hits differs from the specified `hits`, or if
    /// there are issues with the mock server's availability.
    #[deprecated(since = "0.8.0", note = "please use `assert_calls_async` instead")]
    pub async fn assert_hits_async(&self, hits: usize) {
        self.assert_calls_async(hits).await
    }

    /// Asynchronously verifies that the mock server received the specified number of HTTP requests
    /// matching all defined request conditions for this mock.
    ///
    /// This method supports asynchronous testing environments, enabling non-blocking verification
    /// of multiple interactions with the mock server. It's particularly useful when exact counts
    /// of interactions are critical for test assertions.
    ///
    /// **Attention**: For asserting exactly one request asynchronously, use
    /// [Mock::assert_async](struct.Mock.html#method.assert_async) for simpler syntax.
    ///
    /// # Example
    /// Demonstrates setting up an asynchronous mock, sending multiple requests, and verifying the
    /// number of requests received matches expectations.
    ///
    /// ```rust
    /// use httpmock::prelude::*;
    /// use reqwest::get;
    ///
    /// let rt = tokio::runtime::Runtime::new().unwrap();
    /// rt.block_on(async {
    ///     // Arrange: Start a mock server asynchronously and set up a mock
    ///     let server = MockServer::start_async().await;
    ///     let mut mock = server.mock_async(|when, then| {
    ///         when.path("/hits");
    ///         then.status(200);
    ///     }).await;
    ///
    ///     // Act: Send multiple asynchronous requests to the configured path
    ///     get(&server.url("/hits")).await.unwrap();
    ///     get(&server.url("/hits")).await.unwrap();
    ///
    ///     // Assert: Check that the server received exactly two requests that matched the mock
    ///     mock.assert_calls_async(2).await;
    /// });
    /// ```
    ///
    /// # Panics
    /// This method will panic if the actual number of hits differs from the specified `hits`, or if
    /// there are issues with the mock server's availability.
    pub async fn assert_calls_async(&self, hits: usize) {
        let active_mock = self
            .server
            .server_adapter
            .as_ref()
            .unwrap()
            .fetch_mock(self.id)
            .await
            .expect("cannot deserialize mock server response");

        if active_mock.call_counter == hits {
            return;
        }

        if active_mock.call_counter > hits {
            assert_eq!(
                active_mock.call_counter, hits,
                "The number of matching requests was higher than expected (expected {} but was {})",
                hits, active_mock.call_counter
            )
        }

        let closest_match = self
            .server
            .server_adapter
            .as_ref()
            .unwrap()
            .verify(&active_mock.definition.request)
            .await
            .expect("Cannot contact mock server");

        output::fail_with(active_mock.call_counter, hits, closest_match)
    }

    /// Returns the number of times the specified mock has been triggered on the mock server.
    ///
    /// This method is useful for verifying that a mock has been invoked the expected number of times,
    /// allowing for precise control and assertion of interactions within test scenarios.
    ///
    /// # Example
    /// Demonstrates setting up a mock, sending a request, and then verifying that the mock
    /// was triggered exactly once.
    ///
    /// ```rust
    /// use httpmock::prelude::*;
    /// use reqwest::blocking::get;
    ///
    /// // Arrange: Start a mock server and create a mock
    /// let server = MockServer::start();
    /// let mut mock = server.mock(|when, then| {
    ///     when.path("/hits");
    ///     then.status(200);
    /// });
    ///
    /// // Act: Send a request to the mock path
    /// get(&server.url("/hits")).unwrap();
    ///
    /// // Assert: Verify the mock was called once
    /// assert_eq!(1, mock.hits());
    /// ```
    ///
    /// # Panics
    /// This method will panic if there are issues accessing the mock server or retrieving the hit count.
    #[deprecated(since = "0.8.0", note = "please use `calls` instead")]
    pub fn hits(&self) -> usize {
        self.calls()
    }

    /// Returns the number of times the specified mock has been triggered on the mock server.
    ///
    /// This method is useful for verifying that a mock has been invoked the expected number of times,
    /// allowing for precise control and assertion of interactions within test scenarios.
    ///
    /// # Example
    /// Demonstrates setting up a mock, sending a request, and then verifying that the mock
    /// was triggered exactly once.
    ///
    /// ```rust
    /// use httpmock::prelude::*;
    /// use reqwest::blocking::get;
    ///
    /// // Arrange: Start a mock server and create a mock
    /// let server = MockServer::start();
    /// let mut mock = server.mock(|when, then| {
    ///     when.path("/hits");
    ///     then.status(200);
    /// });
    ///
    /// // Act: Send a request to the mock path
    /// get(&server.url("/hits")).unwrap();
    ///
    /// // Assert: Verify the mock was called once
    /// assert_eq!(1, mock.calls());
    /// ```
    ///
    /// # Panics
    /// This method will panic if there are issues accessing the mock server or retrieving the hit count.
    pub fn calls(&self) -> usize {
        self.calls_async().join()
    }

    /// Asynchronously returns the number of times the specified mock has been triggered on the mock server.
    ///
    /// This method is particularly useful in asynchronous test setups where non-blocking verifications
    /// are needed to confirm that a mock has been invoked the expected number of times. It ensures test
    /// assertions align with asynchronous operations.
    ///
    /// # Example
    /// Demonstrates setting up an asynchronous mock, sending a request, and then verifying the number
    /// of times the mock was triggered.
    ///
    /// ```rust
    /// use httpmock::prelude::*;
    /// use reqwest::get;
    ///
    /// let rt = tokio::runtime::Runtime::new().unwrap();
    /// rt.block_on(async {
    ///     // Arrange: Start an asynchronous mock server and create a mock
    ///     let server = MockServer::start_async().await;
    ///     let mut mock = server
    ///         .mock_async(|when, then| {
    ///             when.path("/hits");
    ///             then.status(200);
    ///         })
    ///         .await;
    ///
    ///     // Act: Send an asynchronous request to the mock path
    ///     get(&server.url("/hits")).await.unwrap();
    ///
    ///     // Assert: Verify the mock was called once
    ///     assert_eq!(1, mock.hits_async().await);
    /// });
    /// ```
    ///
    /// # Panics
    /// This method will panic if there are issues accessing the mock server or retrieving the hit count asynchronously.
    #[deprecated(since = "0.8.0", note = "please use `calls_async` instead")]
    pub async fn hits_async(&self) -> usize {
        self.calls_async().await
    }

    /// Asynchronously returns the number of times the specified mock has been triggered on the mock server.
    ///
    /// This method is particularly useful in asynchronous test setups where non-blocking verifications
    /// are needed to confirm that a mock has been invoked the expected number of times. It ensures test
    /// assertions align with asynchronous operations.
    ///
    /// # Example
    /// Demonstrates setting up an asynchronous mock, sending a request, and then verifying the number
    /// of times the mock was triggered.
    ///
    /// ```rust
    /// use httpmock::prelude::*;
    /// use reqwest::get;
    ///
    /// let rt = tokio::runtime::Runtime::new().unwrap();
    /// rt.block_on(async {
    ///     // Arrange: Start an asynchronous mock server and create a mock
    ///     let server = MockServer::start_async().await;
    ///     let mut mock = server
    ///         .mock_async(|when, then| {
    ///             when.path("/hits");
    ///             then.status(200);
    ///         })
    ///         .await;
    ///
    ///     // Act: Send an asynchronous request to the mock path
    ///     get(&server.url("/hits")).await.unwrap();
    ///
    ///     // Assert: Verify the mock was called once
    ///     assert_eq!(1, mock.calls_async().await);
    /// });
    /// ```
    ///
    /// # Panics
    /// This method will panic if there are issues accessing the mock server or retrieving the hit count asynchronously.
    pub async fn calls_async(&self) -> usize {
        let response = self
            .server
            .server_adapter
            .as_ref()
            .unwrap()
            .fetch_mock(self.id)
            .await
            .expect("cannot deserialize mock server response");

        response.call_counter
    }

    /// Removes the specified mock from the mock server. This operation is useful for testing scenarios
    /// where the mock should no longer intercept requests, effectively simulating an environment
    /// where certain endpoints may go offline or change behavior dynamically during testing.
    ///
    /// # Example
    /// Demonstrates creating a mock, verifying its behavior with a request, then deleting the mock and
    /// verifying that subsequent requests are not intercepted.
    ///
    /// ```rust
    /// use httpmock::prelude::*;
    /// use reqwest::blocking::get;
    ///
    /// // Arrange: Start a mock server and set up a mock
    /// let server = MockServer::start();
    /// let mut mock = server.mock(|when, then| {
    ///    when.path("/test");
    ///    then.status(202);
    /// });
    ///
    /// // Act: Send a request to the mock and verify its behavior
    /// let response1 = get(&server.url("/test")).unwrap();
    /// assert_eq!(mock.hits(), 1);  // Verify the mock was called once
    ///
    /// // Delete the mock from the server
    /// mock.delete();
    ///
    /// // Send another request and verify the response now that the mock is deleted
    /// let response2 = get(&server.url("/test")).unwrap();
    ///
    /// // Assert: The first response should be 202 as the mock was active, the second should be 404
    /// assert_eq!(response1.status(), 202);
    /// assert_eq!(response2.status(), 404);
    /// ```
    ///
    /// This method ensures that the mock is completely removed, and any subsequent requests to the
    /// same path will not be intercepted by this mock, typically resulting in a 404 Not Found response
    /// unless another active mock matches the request.
    pub fn delete(&mut self) {
        self.delete_async().join();
    }

    /// Configures the specified mock to be removed from the mock server after it has received `count`
    /// calls. This operation is useful for testing scenarios where a client sends multiple requests
    /// to an endpoint that may go offline or change behavior dynamically during testing, and you
    /// cannot manually delete the mock between requests.
    ///
    /// # Example
    /// Demonstrates creating two mocks, setting the first to delete itself after receiving a call,
    /// then verifying the second mock is now intercepting the requests.
    ///
    /// ```rust
    /// use httpmock::prelude::*;
    /// use reqwest::blocking::get;
    ///
    /// // Arrange: Start a mock server and set up a mock
    /// let server = MockServer::start();
    /// let mut mock_1 = server.mock(|when, then| {
    ///    when.path("/test");
    ///    then.status(202);
    /// });
    ///
    /// // Set the mock to delete itself from the server after receiving a single call
    /// mock_1.delete_after_calls(1);
    ///
    /// // Create a second mock for the same path that will take over when the first mock is
    /// // deleted
    /// let mut mock_2 = server.mock(|when, then| {
    ///    when.path("/test");
    ///    then.status(302);
    /// });
    ///
    /// // Act: Send a request to the mock and verify its behavior
    /// let response1 = get(&server.url("/test")).unwrap();
    ///
    /// // Send another request and verify the response now that the second mock is active
    /// let response2 = get(&server.url("/test")).unwrap();
    /// mock_2.assert();  // Verify the second mock was called once
    ///
    /// // Assert: The first response should be 202 as the first mock was active, the second should be
    /// // 302 from the second mock
    /// assert_eq!(response1.status(), 202);
    /// assert_eq!(response2.status(), 302);
    /// ```
    ///
    /// # Panics
    /// This method will panic if `count` is zero.
    pub fn delete_after_calls(&mut self, count: usize) {
        self.delete_after_calls_async(count).join();
    }

    /// Asynchronously deletes this mock from the mock server. This method is the asynchronous equivalent of
    /// [Mock::delete](struct.Mock.html#method.delete) and is suited for use in asynchronous testing environments
    /// where non-blocking operations are preferred.
    ///
    /// # Example
    /// Demonstrates creating an asynchronous mock, sending a request to verify its behavior, then deleting
    /// the mock and verifying that subsequent requests are not intercepted.
    ///
    /// ```rust
    /// use httpmock::prelude::*;
    /// use reqwest::get;
    ///
    /// let rt = tokio::runtime::Runtime::new().unwrap();
    /// rt.block_on(async {
    ///     // Arrange: Start an asynchronous mock server and create a mock
    ///     let server = MockServer::start_async().await;
    ///     let mut mock = server
    ///         .mock_async(|when, then| {
    ///             when.path("/test");
    ///             then.status(202);
    ///         })
    ///         .await;
    ///
    ///     // Act: Send a request to the mock path and verify the mock's hit count
    ///     let response1 = get(&server.url("/test")).await.unwrap();
    ///     assert_eq!(mock.hits_async().await, 1);  // Verify the mock was called once
    ///
    ///     // Delete the mock asynchronously from the server
    ///     mock.delete_async().await;
    ///
    ///     // Send another request and check the response now that the mock is deleted
    ///     let response2 = get(&server.url("/test")).await.unwrap();
    ///
    ///     // Assert: The first response should be 202 as the mock was active, the second should be 404
    ///     assert_eq!(response1.status(), 202);
    ///     assert_eq!(response2.status(), 404);
    /// });
    /// ```
    ///
    /// This method ensures that the mock is completely removed asynchronously, and any subsequent requests to the
    /// same path will not be intercepted by this mock, typically resulting in a 404 Not Found response
    /// unless another active mock matches the request.
    pub async fn delete_async(&self) {
        self.server
            .server_adapter
            .as_ref()
            .unwrap()
            .delete_mock(self.id)
            .await
            .expect("could not delete mock from server");
    }

    /// Asynchronously configures the specified mock to be removed from the mock server after it has received `count` calls.
    /// This method is the asynchronous equivalent of [Mock::delete_after_calls](struct.Mock.html#method.delete_after_calls)
    /// and is suited for use in asynchronous testing environments where non-blocking operations are preferred.
    ///
    /// # Example
    /// Demonstrates creating two asynchronous mocks, setting the first to delete itself after receiving a call,
    /// then verifying the second mock is now intercepting the requests.
    ///
    /// ```rust
    /// use httpmock::prelude::*;
    /// use reqwest::get;
    ///
    /// let rt = tokio::runtime::Runtime::new().unwrap();
    /// rt.block_on(async {
    ///     // Arrange: Start an asynchronous mock server and create a mock
    ///     let server = MockServer::start_async().await;
    ///     let mut mock_1 = server
    ///         .mock_async(|when, then| {
    ///             when.path("/test");
    ///             then.status(202);
    ///         })
    ///         .await;
    ///
    ///     // Set the mock to delete itself from the server after receiving a single call
    ///     mock_1.delete_after_calls_async(1).await;
    ///
    ///     // Create a second mock for the same path that will take over when the first mock is
    ///     // deleted
    ///     let mock_2 = server
    ///         .mock_async(|when, then| {
    ///             when.path("/test");
    ///             then.status(302);
    ///         })
    ///         .await;
    ///
    ///     // Act: Send a request to the mock path. The first mock will delete itself
    ///     let response1 = get(&server.url("/test")).await.unwrap();
    ///
    ///     // Send another request, which now goes to the second mock
    ///     let response2 = get(&server.url("/test")).await.unwrap();
    ///     mock_2.assert_async().await;  // Verify the second mock was called once
    ///
    ///     // Assert: The first response should be 202 as the first mock was active, the second should be 302
    ///     assert_eq!(response1.status(), 202);
    ///     assert_eq!(response2.status(), 302);
    /// });
    /// ```
    ///
    /// This method will set the mock to delete itself after receiving N calls asynchronously,
    /// and any subsequent requests to the same path will not be intercepted by this mock, typically
    /// resulting in a 404 Not Found response
    /// unless another active mock matches the request.
    ///
    /// # Panics
    /// This method will panic if `count` is zero.
    pub async fn delete_after_calls_async(&self, count: usize) {
        assert_ne!(0, count, "delete_after_calls must be greater than zero");

        self.server
            .server_adapter
            .as_ref()
            .unwrap()
            .delete_mock_after_calls(self.id, count)
            .await
            .expect("could not set mock delete_after_calls from server");
    }

    /// Returns the network address of the mock server where the associated mock object is stored.
    ///
    /// This method provides access to the IP address and port number of the mock server, useful for
    /// connecting to it in tests or displaying its address in debugging output.
    ///
    /// # Example
    /// Demonstrates how to retrieve and print the address of a mock server after it has been started.
    ///
    /// ```rust
    /// use httpmock::prelude::*;
    /// use std::net::SocketAddr;
    ///
    /// // Arrange: Start a mock server
    /// let server = MockServer::start();
    ///
    /// // Print the address of the server
    /// let address: &SocketAddr = server.address();
    /// println!("{}", address);
    /// // Output will be something like "127.0.0.1:12345", where 12345 is the port the server is running on.
    /// ```
    pub fn server_address(&self) -> &SocketAddr {
        self.server.server_adapter.as_ref().unwrap().address()
    }
}

/// The [MockExt](trait.MockExt.html) trait extends the [Mock](struct.Mock.html)
/// structure with some additional functionality, that is usually not required.
pub trait MockExt<'a> {
    /// Creates a new [Mock](struct.Mock.html) instance that references an already existing
    /// mock on a [MockServer](struct.MockServer.html). This method is typically used in advanced scenarios
    /// where you need to re-establish a reference to a mock after its original instance has been dropped
    /// or lost.
    ///
    /// # Parameters
    /// * `id` - The ID of the existing mock on the [MockServer](struct.MockServer.html).
    /// * `mock_server` - A reference to the [MockServer](struct.MockServer.html) where the mock is hosted.
    ///
    /// # Example
    /// Demonstrates how to recreate a [Mock](struct.Mock.html) instance from a mock ID to verify
    /// assertions or perform further actions after the original [Mock](struct.Mock.html) reference
    /// has been discarded.
    ///
    /// ```rust
    /// use httpmock::{MockServer, Mock, MockExt};
    /// use reqwest::blocking::get;
    ///
    /// // Arrange
    /// let server = MockServer::start();
    /// let initial_mock = server.mock(|when, then| {
    ///     when.path("/test");
    ///     then.status(202);
    /// });
    ///
    /// // Store away the mock ID and drop the initial Mock instance
    /// let mock_id = initial_mock.id();
    /// drop(initial_mock);
    ///
    /// // Act: Send an HTTP request to the mock endpoint
    /// let response = get(&server.url("/test")).unwrap();
    ///
    /// // Recreate the Mock instance using the stored ID
    /// let recreated_mock = Mock::new(mock_id, &server);
    ///
    /// // Assert: Use the recreated Mock to check assertions
    /// recreated_mock.assert();
    /// assert_eq!(response.status(), 202);
    /// ```
    /// For more detailed use cases, see [`Issue 26`](https://github.com/httpmock/httpmock/issues/26) on GitHub.
    fn new(id: usize, mock_server: &'a MockServer) -> Mock<'a>;

    /// Returns the unique identifier (ID) assigned to the mock on the [MockServer](struct.MockServer.html).
    /// This ID is used internally by the mock server to track and manage the mock throughout its lifecycle.
    ///
    /// The ID can be particularly useful in advanced testing scenarios where mocks need to be referenced or manipulated
    /// programmatically after their creation.
    ///
    /// # Returns
    /// Returns the ID of the mock as a `usize`.
    ///
    /// # Example
    /// Demonstrates how to retrieve the ID of a mock for later reference or manipulation.
    ///
    /// ```rust
    /// use httpmock::MockExt;
    /// use httpmock::prelude::*;
    ///
    /// // Arrange: Start a mock server and create a mock
    /// let server = MockServer::start();
    /// let mock = server.mock(|when, then| {
    ///     when.path("/example");
    ///     then.status(200);
    /// });
    ///
    /// // Act: Retrieve the ID of the mock
    /// let mock_id = mock.id();
    ///
    /// // The mock_id can now be used to reference or manipulate this specific mock in subsequent operations
    /// println!("Mock ID: {}", mock_id);
    /// ```
    ///
    /// This method is particularly useful when dealing with multiple mocks and needing to assert or modify
    /// specific mocks based on their identifiers.
    fn id(&self) -> usize;
}

impl<'a> MockExt<'a> for Mock<'a> {
    fn new(id: usize, mock_server: &'a MockServer) -> Mock<'a> {
        Mock {
            id,
            server: mock_server,
        }
    }

    fn id(&self) -> usize {
        self.id
    }
}

pub struct MockSet<'a> {
    pub ids: Vec<usize>,
    pub(crate) server: &'a MockServer,
}

impl<'a> MockSet<'a> {
    pub fn delete(&mut self) {
        self.delete_async().join();
    }

    pub async fn delete_async(&self) {
        for id in &self.ids {
            self.server
                .server_adapter
                .as_ref()
                .unwrap()
                .delete_mock(*id)
                .await
                .expect("could not delete mock from server");
        }
    }
}
