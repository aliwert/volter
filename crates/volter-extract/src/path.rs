//! [`Path`] — typed path parameter extraction.
//!
//! Path parameters are extracted from the request's URI by the router,
//! stored as a [`UrlParams`] extension, and then deserialized into the
//! target type `T` by this extractor via [`serde::Deserialize`].

use std::future::Future;
use std::future::Ready;
use std::pin::Pin;

use serde::de::DeserializeOwned;
use serde_json::Value;

use volter_core::{
    BoxBody, FromRequest, FromRequestParts, IntoResponse, Request, Response, UrlParams,
};

// ---------------------------------------------------------------------------
// PathRejection
// ---------------------------------------------------------------------------

/// The error returned when [`Path`] extraction fails.
///
/// All variants produce a `400 Bad Request` response.
#[derive(Debug, thiserror::Error)]
pub enum PathRejection {
    /// No path parameters were found on the request (the route is not
    /// parameterized but the handler expects them).
    #[error("path parameters not found on request")]
    MissingPathParams,

    /// The path parameters could not be deserialized into the target type
    /// (e.g. expected a number but received a non-numeric string).
    #[error("failed to deserialize path parameters: {0}")]
    InvalidPathParams(#[from] serde_json::Error),
}

impl IntoResponse for PathRejection {
    fn into_response(self) -> Response {
        let mut response = Response::new(volter_core::empty_body());
        *response.status_mut() = http::StatusCode::BAD_REQUEST;
        response
    }
}

// ---------------------------------------------------------------------------
// Path
// ---------------------------------------------------------------------------

/// Extracts typed path parameters from a parameterized route.
///
/// `Path<T>` deserializes the named parameters captured by the router
/// (e.g. `:id` in `/users/:id`) into the target type `T` using
/// [`serde::Deserialize`].
///
/// # Deserialization
///
/// For routes with a **single** parameter, the raw string value is
/// deserialized — you can use `Path<u64>` for numeric IDs or
/// `Path<String>` for string parameters.
///
/// For routes with **multiple** parameters, the params are presented to
/// serde as a JSON object mapping parameter names to string values:
///
/// ```ignore
/// #[derive(Deserialize)]
/// struct PostComment {
///     post_id: u64,
///     comment_id: u64,
/// }
///
/// async fn handler(Path(pc): Path<PostComment>) -> String { ... }
/// ```
///
/// # Extraction failure
///
/// If `T` cannot be deserialized (e.g. a `u64` parameter receives `"abc"`),
/// extraction returns [`PathRejection::InvalidPathParams`], which produces
/// a `400 Bad Request` response.
///
/// # Example
///
/// ```rust
/// use volter_extract::Path;
///
/// async fn user(Path(id): Path<u64>) -> String {
///     format!("User {}", id)
/// }
/// ```
#[derive(Debug, Clone)]
pub struct Path<T>(pub T);

// ---------------------------------------------------------------------------
// Deserialization helper
// ---------------------------------------------------------------------------

/// Deserialize `T` from the extracted path parameters.
///
/// For multi-param routes, an object `{"name": "value", ...}` is built and
/// deserialized (works for structs, `HashMap`, etc.).
///
/// For single-param routes, the raw value is tried first as a JSON value
/// (handles numbers), then as a JSON string (handles `String` and any type
/// that can deserialize from a string).
fn deserialize_path_params<T: DeserializeOwned>(
    params: &[(String, String)],
) -> Result<T, PathRejection> {
    // Single param: try the raw value directly so that `Path<u64>` etc. work.
    if let Some((_, value)) = params.first() {
        // Try as a JSON value (handles numbers, booleans, nested structures).
        if let Ok(v) = serde_json::from_str(value) {
            return Ok(v);
        }
        // Try as a JSON string (handles String and string-based deserialization).
        if let Ok(v) = serde_json::from_value(Value::String(value.clone())) {
            return Ok(v);
        }
    }

    // Build an object from all params, parsing each value as raw JSON first so
    // that e.g. `"42"` becomes a JSON number instead of a JSON string.  If raw
    // parsing fails, fall back to treating the value as a string.
    let map: serde_json::Map<String, Value> = params
        .iter()
        .map(|(k, v)| {
            let value = match serde_json::from_str(v) {
                Ok(val) => val,
                Err(_) => Value::String(v.clone()),
            };
            (k.clone(), value)
        })
        .collect();

    serde_json::from_value(Value::Object(map)).map_err(PathRejection::from)
}

// ---------------------------------------------------------------------------
// FromRequestParts impl
// ---------------------------------------------------------------------------

/// [`Path<T>`] extracts from request parts by reading the [`UrlParams`]
/// extension that the router sets after matching a parameterized route.
///
/// `S` (the application state) is ignored — path parameters come from the
/// URI alone, not from any framework state.
impl<S, T: DeserializeOwned + Send> FromRequestParts<S> for Path<T> {
    type Rejection = PathRejection;
    type Future = Ready<Result<Self, Self::Rejection>>;

    fn from_request_parts(parts: &mut http::request::Parts, _state: &S) -> Self::Future {
        let result = parts
            .extensions
            .get::<UrlParams>()
            .ok_or(PathRejection::MissingPathParams)
            .and_then(|params| deserialize_path_params::<T>(&params.0));

        std::future::ready(result.map(Path))
    }
}

// ---------------------------------------------------------------------------
// FromRequest impl (body-consuming variant)
// ---------------------------------------------------------------------------

/// [`Path<T>`] also implements [`FromRequest`] so it can be the last
/// argument in a multi-extractor handler tuple.  The body is split off
/// and dropped.
impl<S: Clone + Send + 'static, T: DeserializeOwned + Send + 'static> FromRequest<S, BoxBody>
    for Path<T>
{
    type Rejection = PathRejection;
    type Future = Pin<Box<dyn Future<Output = Result<Self, Self::Rejection>> + Send>>;

    fn from_request(req: Request, state: &S) -> Self::Future {
        let state = state.clone();
        Box::pin(async move {
            let (mut parts, _body) = req.into_parts();
            <Self as FromRequestParts<S>>::from_request_parts(&mut parts, &state).await
        })
    }
}
