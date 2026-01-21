use std::{
    borrow::Cow,
    env,
    fs::{create_dir_all, File},
    future::Future,
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::Arc,
    task::{Context, Poll},
};

use bytes::Bytes;
/// Extension trait for efficiently blocking on a future.
use crossbeam_utils::sync::{Parker, Unparker};
use futures_timer::Delay;
use futures_util::{pin_mut, task::ArcWake};
use serde::{Deserialize, Serialize, Serializer};
use std::{cell::Cell, time::Duration};

// ===============================================================================================
// Misc
// ===============================================================================================
pub(crate) fn update_cell<T: Sized + Default, F: FnOnce(&mut T)>(v: &Cell<T>, f: F) {
    let mut vv = v.take();
    f(&mut vv);
    v.set(vv);
}

// ===============================================================================================
// Retry
// ===============================================================================================
#[doc(hidden)]
pub(crate) async fn with_retry<T, U, F, Fut>(retries: usize, f: F) -> Result<T, U>
where
    F: Fn() -> Fut,
    Fut: Future<Output = Result<T, U>>,
{
    let mut result = (f)().await;
    for i in 1..=retries {
        if result.is_ok() {
            return result;
        } else {
            Delay::new(Duration::from_secs(1 * i as u64)).await;
        }
        result = (f)().await;
    }
    result
}

// ===============================================================================================
// Environment
// ===============================================================================================
#[doc(hidden)]
pub(crate) fn read_env(name: &str, default: &str) -> String {
    match std::env::var(name) {
        Ok(value) => value,
        Err(_) => default.to_string(),
    }
}

// ===============================================================================================
// Futures
// ===============================================================================================
#[doc(hidden)]
pub trait Join: Future {
    fn join(self) -> <Self as Future>::Output;
}

impl<F: Future> Join for F {
    fn join(self) -> <Self as Future>::Output {
        struct ThreadWaker(Unparker);

        impl ArcWake for ThreadWaker {
            fn wake_by_ref(arc_self: &Arc<Self>) {
                arc_self.0.unpark();
            }
        }

        let parker = Parker::new();
        let waker = futures_util::task::waker(Arc::new(ThreadWaker(parker.unparker().clone())));
        let mut context = Context::from_waker(&waker);

        let future = self;
        pin_mut!(future);

        loop {
            match future.as_mut().poll(&mut context) {
                Poll::Ready(output) => return output,
                Poll::Pending => parker.park(),
            }
        }
    }
}

// ===============================================================================================
// Files
// ===============================================================================================
pub fn get_test_resource_file_path(relative_resource_path: &str) -> Result<PathBuf, String> {
    match env::var("CARGO_MANIFEST_DIR") {
        Ok(manifest_path) => Ok(Path::new(&manifest_path).join(relative_resource_path)),
        Err(e) => Err(e.to_string()),
    }
}

pub async fn write_file<P: AsRef<Path>>(
    resource_path: P,
    content: &Bytes,
    create_dir: bool,
) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let mut path = resource_path.as_ref().to_path_buf();

    if path.is_relative() {
        let current_dir = env::current_dir()?;
        path = current_dir.join(path);
    }

    if create_dir {
        if let Some(parent) = path.parent() {
            create_dir_all(parent)?;
        }
    }

    let mut file = File::create(&path)?;
    file.write_all(content)?;
    file.flush()?;

    Ok(path)
}

// Checks if the executing thread is running in a Tokio runtime.

#[cfg(test)]
mod test {
    use crate::common::util::{with_retry, Join};

    #[test]
    fn with_retry_error_test() {
        let result: Result<(), &str> = with_retry(1, || async { Err("test error") }).join();

        assert_eq!(result.is_err(), true);
        assert_eq!(result.err().unwrap(), "test error")
    }
}

/// A wrapper around `bytes::Bytes` providing utility methods for common operations.
#[derive(Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct HttpMockBytes(pub Bytes);

impl HttpMockBytes {
    /// Converts the bytes to a `Vec<u8>`.
    ///
    /// # Returns
    /// A `Vec<u8>` containing the bytes.
    pub fn to_vec(&self) -> Vec<u8> {
        self.0.to_vec()
    }

    /// Cheaply clones the bytes into a new `Bytes` instance.
    /// See
    ///
    /// # Returns
    /// A `Bytes` instance containing the same data.
    pub fn to_bytes(&self) -> Bytes {
        self.0.clone()
    }

    /// Checks if the byte slice is empty.
    ///
    /// # Returns
    /// `true` if the byte slice is empty, otherwise `false`.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Checks if the byte slice is blank (empty or only contains ASCII whitespace).
    ///
    /// # Returns
    /// `true` if the byte slice is blank, otherwise `false`.
    pub fn is_blank(&self) -> bool {
        self.is_empty() || self.0.iter().all(|&b| b.is_ascii_whitespace())
    }

    /// Checks if the byte slice contains the specified substring.
    ///
    /// # Arguments
    /// * `substring` - The substring to search for.
    ///
    /// # Returns
    /// `true` if the substring is found, otherwise `false`.
    pub fn contains_str(&self, substring: &str) -> bool {
        if substring.is_empty() {
            return true;
        }

        self.0
            .as_ref()
            .windows(substring.as_bytes().len())
            .any(|window| window == substring.as_bytes())
    }

    /// Checks if the byte slice contains the specified byte slice.
    ///
    /// # Arguments
    /// * `slice` - The byte slice to search for.
    ///
    /// # Returns
    /// `true` if the byte slice is found, otherwise `false`.
    pub fn contains_slice(&self, slice: &[u8]) -> bool {
        self.0
            .as_ref()
            .windows(slice.len())
            .any(|window| window == slice)
    }

    /// Checks if the byte slice contains the specified `Vec<u8>`.
    ///
    /// # Arguments
    /// * `vec` - The vector to search for.
    ///
    /// # Returns
    /// `true` if the vector is found, otherwise `false`.
    pub fn contains_vec(&self, vec: &Vec<u8>) -> bool {
        self.0
            .as_ref()
            .windows(vec.len())
            .any(|window| window == vec.as_slice())
    }

    /// Converts the bytes to a UTF-8 string, potentially lossy.
    /// Tries to parse input as a UTF-8 string first to avoid copying and creating an owned instance.
    /// If the bytes are not valid UTF-8, it creates a lossy string by replacing invalid characters
    /// with the Unicode replacement character.
    ///
    /// # Returns
    /// A `Cow<str>` which is either borrowed if the bytes are valid UTF-8 or owned if conversion was required.
    pub fn to_maybe_lossy_str(&self) -> Cow<str> {
        match std::str::from_utf8(&self.0) {
            Ok(valid_str) => Cow::Borrowed(valid_str),
            Err(_) => Cow::Owned(String::from_utf8_lossy(&self.0).to_string()),
        }
    }
}

impl From<Bytes> for HttpMockBytes {
    fn from(value: Bytes) -> Self {
        Self(value)
    }
}

impl From<&Bytes> for HttpMockBytes {
    fn from(value: &Bytes) -> Self {
        Self(value.clone())
    } // cheap clone handle
}

impl From<bytes::BytesMut> for HttpMockBytes {
    fn from(value: bytes::BytesMut) -> Self {
        Self(value.freeze())
    }
}

// Copying conversions
impl From<Vec<u8>> for HttpMockBytes {
    fn from(value: Vec<u8>) -> Self {
        Self(Bytes::from(value))
    }
}

impl From<&[u8]> for HttpMockBytes {
    fn from(value: &[u8]) -> Self {
        Self(Bytes::copy_from_slice(value))
    }
}

impl From<String> for HttpMockBytes {
    fn from(value: String) -> Self {
        Self(Bytes::from(value))
    }
}

impl From<&str> for HttpMockBytes {
    fn from(value: &str) -> Self {
        Self(Bytes::copy_from_slice(value.as_bytes()))
    }
}

impl<'a> From<Cow<'a, str>> for HttpMockBytes {
    fn from(value: Cow<'a, str>) -> Self {
        match value {
            Cow::Borrowed(s) => Self::from(s),
            Cow::Owned(s) => Self::from(s),
        }
    }
}

impl<'a> From<Cow<'a, [u8]>> for HttpMockBytes {
    fn from(value: Cow<'a, [u8]>) -> Self {
        match value {
            Cow::Borrowed(b) => Self::from(b),
            Cow::Owned(v) => Self::from(v),
        }
    }
}

// Reverse: HttpMockBytes -> Bytes
impl From<HttpMockBytes> for Bytes {
    fn from(value: HttpMockBytes) -> Bytes {
        value.0
    }
}

/* ===== AsRef / Borrow / Deref-ish ergonomics ===== */

impl AsRef<[u8]> for HttpMockBytes {
    fn as_ref(&self) -> &[u8] {
        self.0.as_ref()
    }
}

impl std::borrow::Borrow<[u8]> for HttpMockBytes {
    fn borrow(&self) -> &[u8] {
        self.0.as_ref()
    }
}

/* ===== Friendly equality with common counterparts ===== */

impl PartialEq<[u8]> for HttpMockBytes {
    fn eq(&self, other: &[u8]) -> bool {
        self.0.as_ref() == other
    }
}

impl PartialEq<&[u8]> for HttpMockBytes {
    fn eq(&self, other: &&[u8]) -> bool {
        self.0.as_ref() == *other
    }
}

impl PartialEq<Vec<u8>> for HttpMockBytes {
    fn eq(&self, other: &Vec<u8>) -> bool {
        self.0.as_ref() == other.as_slice()
    }
}

impl PartialEq<Bytes> for HttpMockBytes {
    fn eq(&self, other: &Bytes) -> bool {
        self.0 == other
    }
}

impl PartialEq<&str> for HttpMockBytes {
    fn eq(&self, other: &&str) -> bool {
        self.0.as_ref() == other.as_bytes()
    }
}

impl PartialEq<String> for HttpMockBytes {
    fn eq(&self, other: &String) -> bool {
        self.0.as_ref() == other.as_bytes()
    }
}

/* ===== Display / Debug ===== */

impl std::fmt::Display for HttpMockBytes {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match std::str::from_utf8(&self.0) {
            Ok(s) => write!(f, "{s}"),
            Err(_) => write!(f, "{}", base64::encode(&self.0)),
        }
    }
}

impl std::fmt::Debug for HttpMockBytes {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Show UTF-8 inline when possible; otherwise prefix and base64
        match std::str::from_utf8(&self.0) {
            Ok(s) => f.debug_tuple("HttpMockBytes").field(&s).finish(),
            Err(_) => f
                .debug_tuple("HttpMockBytes")
                .field(&format!(
                    "<{} bytes, b64:{}>",
                    self.0.len(),
                    base64::encode(&self.0)
                ))
                .finish(),
        }
    }
}

pub fn title_case(s: &str) -> String {
    let mut result = String::new();
    let mut capitalize_next = true;

    for c in s.chars() {
        if c.is_whitespace() {
            capitalize_next = true;
            result.push(c);
        } else if capitalize_next {
            result.push(c.to_uppercase().next().unwrap());
            capitalize_next = false;
        } else {
            result.push(c.to_lowercase().next().unwrap());
        }
    }

    result
}

pub fn is_none_or_empty<T>(option: &Option<Vec<T>>) -> bool {
    match option {
        None => true,
        Some(vec) => vec.is_empty(),
    }
}

pub fn read_file<P: AsRef<Path>>(absolute_resource_path: P) -> Result<Vec<u8>, String> {
    let mut buffer = Vec::new();

    let len = File::open(&absolute_resource_path)
        .and_then(|mut f| f.read_to_end(&mut buffer))
        .map_err(|e| e.to_string())?;

    tracing::trace!(
        "Read {} bytes from file {:?}",
        &len,
        &absolute_resource_path
            .as_ref()
            .as_os_str()
            .to_str()
            .expect("Invalid file path")
    );

    Ok(buffer)
}
