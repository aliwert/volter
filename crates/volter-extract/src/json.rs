//! [`Json`] — typed JSON body extraction and response.
//!
//! `Json<T>` implements both [`FromRequest`] (for extraction) and
//! [`IntoResponse`] (for response serialization), so the same type can be
//! used in handlers that receive JSON and return JSON.

use std::future::Future;
use std::pin::Pin;

use bytes::Bytes;
use http_body_util::BodyExt as _;
use serde::de::DeserializeOwned;
use serde::Serialize;

use volter_core::{
    empty_body, full_body, BoxBody, BoxError, FromRequest, IntoResponse, Request, Response,
};

// ---------------------------------------------------------------------------
// JsonRejection
// ---------------------------------------------------------------------------

/// The error returned when [`Json`] extraction fails.
#[derive(Debug, thiserror::Error)]
pub enum JsonRejection {
    /// The `Content-Type` header is missing.
    #[error("content-type header is missing")]
    MissingJsonContentType,

    /// The `Content-Type` header is present but is not `application/json`
    /// or an `application/*+json` subtype.
    #[error("unsupported content-type")]
    UnsupportedJsonContentType,

    /// The request body could not be read.
    #[error("failed to read request body")]
    BodyReadError(#[source] BoxError),

    /// The request body is not valid JSON, or the JSON does not match the
    /// expected type `T`.
    #[error("failed to deserialize JSON body: {0}")]
    InvalidJsonBody(#[from] serde_json::Error),
}

impl IntoResponse for JsonRejection {
    fn into_response(self) -> Response {
        let status = match self {
            Self::MissingJsonContentType | Self::UnsupportedJsonContentType => {
                http::StatusCode::UNSUPPORTED_MEDIA_TYPE
            }
            Self::InvalidJsonBody(_) => http::StatusCode::BAD_REQUEST,
            Self::BodyReadError(_) => http::StatusCode::INTERNAL_SERVER_ERROR,
        };
        let mut response = Response::new(empty_body());
        *response.status_mut() = status;
        response
    }
}

// ---------------------------------------------------------------------------
// Json
// ---------------------------------------------------------------------------

/// Extracts a typed JSON body from a request, or serializes a value as a
/// JSON response.
///
/// # Extraction
///
/// `Json<T>` implements [`FromRequest`] and consumes the request body.
/// It requires a `Content-Type` header of `application/json` (or
/// `application/*+json`).  The body is collected and deserialized into `T`.
///
/// # Response
///
/// `Json<T>` also implements [`IntoResponse`] when `T: Serialize`.
/// The response has status `200 OK` and `Content-Type: application/json`.
///
/// # Example
///
/// ```rust
/// use volter_extract::Json;
/// use serde::{Deserialize, Serialize};
///
/// #[derive(Deserialize, Serialize)]
/// struct CreateUser {
///     name: String,
///     age: u8,
/// }
///
/// async fn create_user(Json(payload): Json<CreateUser>) -> Json<CreateUser> {
///     Json(payload)
/// }
/// ```
#[derive(Debug, Clone)]
pub struct Json<T>(pub T);

// ---------------------------------------------------------------------------
// FromRequest impl
// ---------------------------------------------------------------------------

/// Collects the request body and deserializes it as JSON into `T`.
///
/// The application state `S` is ignored — JSON body deserialization does
/// not use state.
impl<S, T: DeserializeOwned + Send + 'static> FromRequest<S, BoxBody> for Json<T> {
    type Rejection = JsonRejection;
    type Future = Pin<Box<dyn Future<Output = Result<Self, Self::Rejection>> + Send>>;

    fn from_request(req: Request, _state: &S) -> Self::Future {
        Box::pin(async move {
            let (parts, body) = req.into_parts();

            // Check Content-Type: accept only application/json or
            // application/*+json.
            let content_type = parts
                .headers
                .get(http::header::CONTENT_TYPE)
                .and_then(|v| v.to_str().ok())
                .unwrap_or("");

            let mime = content_type.split(';').next().unwrap_or("").trim();

            if mime != "application/json"
                && !(mime.starts_with("application/") && mime.ends_with("+json"))
            {
                return if content_type.is_empty() {
                    Err(JsonRejection::MissingJsonContentType)
                } else {
                    Err(JsonRejection::UnsupportedJsonContentType)
                };
            }

            // Collect the full body.
            let collected = body.collect().await.map_err(JsonRejection::BodyReadError)?;
            let bytes = collected.to_bytes();

            // Deserialize as JSON.
            let value = serde_json::from_slice::<T>(&bytes)?;

            Ok(Json(value))
        })
    }
}

// ---------------------------------------------------------------------------
// IntoResponse impl
// ---------------------------------------------------------------------------

/// Serializes `T` as a JSON response with status `200 OK` and
/// `Content-Type: application/json`.
///
/// If serialization fails, a `500 Internal Server Error` is returned with
/// an empty body.
impl<T: Serialize> IntoResponse for Json<T> {
    fn into_response(self) -> Response {
        match serde_json::to_vec(&self.0) {
            Ok(bytes) => {
                let mut response = Response::new(full_body(Bytes::from(bytes)));
                response.headers_mut().insert(
                    http::header::CONTENT_TYPE,
                    http::HeaderValue::from_static("application/json"),
                );
                response
            }
            Err(_) => {
                let mut response = Response::new(empty_body());
                *response.status_mut() = http::StatusCode::INTERNAL_SERVER_ERROR;
                response
            }
        }
    }
}
