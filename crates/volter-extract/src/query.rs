//! [`Query`] — typed query string extraction.
//!
//! Query parameters are parsed from the request URI's query string using
//! `serde_urlencoded` and deserialized into the target type `T`.

use std::future::Ready;

use serde::de::DeserializeOwned;

use volter_core::{FromRequestParts, IntoResponse, Response};

// ---------------------------------------------------------------------------
// QueryRejection
// ---------------------------------------------------------------------------

/// The error returned when [`Query`] extraction fails.
///
/// All variants produce a `400 Bad Request` response.
#[derive(Debug, thiserror::Error)]
pub enum QueryRejection {
    /// The query string could not be deserialized into the target type
    /// (e.g. expected a number but received a non-numeric value).
    #[error("failed to deserialize query string: {0}")]
    InvalidQueryParams(#[from] serde_urlencoded::de::Error),
}

impl IntoResponse for QueryRejection {
    fn into_response(self) -> Response {
        let mut response = Response::new(volter_core::empty_body());
        *response.status_mut() = http::StatusCode::BAD_REQUEST;
        response
    }
}

// ---------------------------------------------------------------------------
// Query
// ---------------------------------------------------------------------------

/// Extracts typed query parameters from the request URI's query string.
///
/// `Query<T>` parses the query string using [`serde_urlencoded`] and
/// deserializes it into the target type `T`.
///
/// If the URI has no query string, an empty string is deserialized —
/// this works for structs where all fields are either `Option<T>` or
/// have `#[serde(default)]` annotations.
///
/// # Deserialization
///
/// Query parameters are presented to `serde` as a flat key-value map:
///
/// ```ignore
/// #[derive(Deserialize)]
/// struct UsersQuery {
///     page: u32,
///     search: Option<String>,
/// }
/// ```
///
/// A request to `GET /users?page=2&search=rust` produces
/// `UsersQuery { page: 2, search: Some("rust".into()) }`.
///
/// A request to `GET /users` (no query string) produces
/// `UsersQuery { page: 0, search: None }`.
///
/// # Extraction failure
///
/// If the query string cannot be deserialized (e.g. `page=abc` for a `u32`
/// field), extraction returns [`QueryRejection::InvalidQueryParams`], which
/// produces a `400 Bad Request` response.
///
/// # Example
///
/// ```rust
/// use volter_extract::Query;
///
/// #[derive(serde::Deserialize)]
/// struct UsersQuery {
///     page: u32,
/// }
///
/// async fn users(Query(query): Query<UsersQuery>) -> String {
///     format!("Page {}", query.page)
/// }
/// ```
#[derive(Debug, Clone)]
pub struct Query<T>(pub T);

// ---------------------------------------------------------------------------
// FromRequestParts impl
// ---------------------------------------------------------------------------

/// [`Query<T>`] extracts from the URI query string.  `S` (the application
/// state) is ignored — query parameters come from the URI alone.
impl<S, T: DeserializeOwned + Send> FromRequestParts<S> for Query<T> {
    type Rejection = QueryRejection;
    type Future = Ready<Result<Self, Self::Rejection>>;

    fn from_request_parts(parts: &mut http::request::Parts, _state: &S) -> Self::Future {
        let query_string = parts.uri.query().unwrap_or("");
        let result = serde_urlencoded::from_str(query_string)
            .map(Query)
            .map_err(QueryRejection::from);
        std::future::ready(result)
    }
}
