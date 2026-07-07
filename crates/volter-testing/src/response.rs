//! [`TestResponse`] — wraps an HTTP response with ergonomic body-read helpers.
//!
//! Body-acccess methods consume `self` because the body can be read only
//! once.  Callers in test code typically chain `.unwrap()` on the returned
//! [`Result`].

use std::fmt;
use std::str::Utf8Error;
use std::string::FromUtf8Error;

use bytes::Bytes;
use http::HeaderMap;
use http::StatusCode;
use http_body_util::BodyExt;
use serde::de::DeserializeOwned;

use volter_core::BoxError;

// ---------------------------------------------------------------------------
// BodyError
// ---------------------------------------------------------------------------

/// Errors that can occur when consuming a test response body.
#[derive(Debug)]
pub enum BodyError {
    /// The body stream produced an error.
    Stream(BoxError),
    /// The body bytes are not valid UTF-8.
    Utf8(Utf8Error),
    /// JSON deserialization failed.
    Json(serde_json::Error),
}

impl fmt::Display for BodyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BodyError::Stream(e) => write!(f, "body stream error: {e}"),
            BodyError::Utf8(e) => write!(f, "body UTF-8 error: {e}"),
            BodyError::Json(e) => write!(f, "body JSON error: {e}"),
        }
    }
}

impl std::error::Error for BodyError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            BodyError::Stream(e) => Some(e.as_ref()),
            BodyError::Utf8(e) => Some(e),
            BodyError::Json(e) => Some(e),
        }
    }
}

impl From<Utf8Error> for BodyError {
    fn from(e: Utf8Error) -> Self {
        BodyError::Utf8(e)
    }
}

impl From<FromUtf8Error> for BodyError {
    fn from(e: FromUtf8Error) -> Self {
        BodyError::Utf8(e.utf8_error())
    }
}

impl From<serde_json::Error> for BodyError {
    fn from(e: serde_json::Error) -> Self {
        BodyError::Json(e)
    }
}

impl From<BoxError> for BodyError {
    fn from(e: BoxError) -> Self {
        BodyError::Stream(e)
    }
}

// ---------------------------------------------------------------------------
// TestResponse
// ---------------------------------------------------------------------------

/// The response returned by [`TestRequestBuilder::send`](crate::TestRequestBuilder).
///
/// Wraps an `http::Response<BoxBody>` and provides convenience methods for
/// inspecting the status, headers, and body in tests.
pub struct TestResponse {
    /// The underlying HTTP response.
    inner: volter_core::Response,
}

impl TestResponse {
    /// Wrap an `http::Response<BoxBody>` into a `TestResponse`.
    pub(crate) fn new(inner: volter_core::Response) -> Self {
        Self { inner }
    }

    /// Return the response status code.
    pub fn status(&self) -> StatusCode {
        self.inner.status()
    }

    /// Return a reference to the response headers.
    pub fn headers(&self) -> &HeaderMap {
        self.inner.headers()
    }

    /// Consume the response and collect the full body as [`Bytes`].
    ///
    /// # Errors
    ///
    /// Returns [`BodyError::Stream`] if the body stream produces an error.
    pub async fn bytes(self) -> Result<Bytes, BodyError> {
        let collected = self
            .inner
            .into_body()
            .collect()
            .await
            .map_err(BodyError::Stream)?;
        Ok(collected.to_bytes())
    }

    /// Consume the response and decode the body as a UTF-8 string.
    ///
    /// # Errors
    ///
    /// Returns [`BodyError::Stream`] if the body stream produces an error,
    /// or [`BodyError::Utf8`] if the bytes are not valid UTF-8.
    pub async fn text(self) -> Result<String, BodyError> {
        let bytes = self.bytes().await?;
        let text = String::from_utf8(bytes.to_vec())?;
        Ok(text)
    }

    /// Consume the response and deserialize the body as JSON.
    ///
    /// # Errors
    ///
    /// Returns [`BodyError::Stream`] if the body stream produces an error,
    /// [`BodyError::Json`] if deserialization fails.
    pub async fn json<T: DeserializeOwned>(self) -> Result<T, BodyError> {
        let bytes = self.bytes().await?;
        let value = serde_json::from_slice(&bytes)?;
        Ok(value)
    }
}
