//! [`Extension`] — typed request-scoped value extraction.
//!
//! Extensions are middleware-injected values that travel with each request
//! (e.g. an authenticated user, a request ID, or a database connection
//! handle).  Unlike [`State`], a missing `Extension` is a runtime error
//! because middleware ordering cannot always be verified statically.

use std::future::Future;
use std::future::Ready;
use std::pin::Pin;

use volter_core::{BoxBody, FromRequest, FromRequestParts, IntoResponse, Request, Response};

// ---------------------------------------------------------------------------
// ExtensionRejection
// ---------------------------------------------------------------------------

/// The error returned when [`Extension`] extraction fails.
///
/// A missing extension indicates a misconfigured middleware stack — no
/// middleware inserted the requested type before the handler ran.  This is
/// a server error (500), not a client error.
#[derive(Debug, thiserror::Error)]
pub enum ExtensionRejection {
    /// The requested extension type was not found on the request.
    #[error("request extension not found: {0}")]
    MissingExtension(&'static str),
}

impl IntoResponse for ExtensionRejection {
    fn into_response(self) -> Response {
        let mut response = Response::new(volter_core::empty_body());
        *response.status_mut() = http::StatusCode::INTERNAL_SERVER_ERROR;
        response
    }
}

// ---------------------------------------------------------------------------
// Extension
// ---------------------------------------------------------------------------

/// Extracts a request-scoped value that was injected into the request by
/// middleware.
///
/// Unlike [`State<T>`](volter_core::State) which is set once per router,
/// extensions are per-request values inserted by middleware layers (e.g.
/// an auth middleware inserting a `User` struct).
///
/// # Extraction failure
///
/// If the extension is not present on the request, extraction returns
/// [`ExtensionRejection::MissingExtension`], which produces a
/// `500 Internal Server Error` response.  This is deliberate —
/// a missing extension indicates incorrect middleware configuration, not
/// malformed user input.
///
/// # Example
///
/// ```rust
/// use volter_extract::Extension;
///
/// #[derive(Clone, Debug)]
/// struct User {
///     name: String,
/// }
///
/// async fn profile(Extension(user): Extension<User>) -> String {
///     format!("Welcome, {}!", user.name)
/// }
/// ```
#[derive(Debug, Clone)]
pub struct Extension<T>(pub T);

// ---------------------------------------------------------------------------
// FromRequestParts impl
// ---------------------------------------------------------------------------

/// [`Extension<T>`] extracts from the request's extensions map.
///
/// The application state `S` is ignored — extensions come from middleware,
/// not from framework configuration.
impl<S, T: Send + Sync + 'static> FromRequestParts<S> for Extension<T> {
    type Rejection = ExtensionRejection;
    type Future = Ready<Result<Self, Self::Rejection>>;

    fn from_request_parts(parts: &mut http::request::Parts, _state: &S) -> Self::Future {
        let result = parts
            .extensions
            .remove::<T>()
            .ok_or(ExtensionRejection::MissingExtension(
                std::any::type_name::<T>(),
            ))
            .map(Extension);
        std::future::ready(result)
    }
}

// ---------------------------------------------------------------------------
// FromRequest impl (body-consuming variant)
// ---------------------------------------------------------------------------

/// [`Extension<T>`] also implements [`FromRequest`] so it can be the last
/// argument in a multi-extractor handler tuple.  The body is split off
/// and dropped.
impl<S: Clone + Send + 'static, T: Send + Sync + 'static> FromRequest<S, BoxBody> for Extension<T> {
    type Rejection = ExtensionRejection;
    type Future = Pin<Box<dyn Future<Output = Result<Self, Self::Rejection>> + Send>>;

    fn from_request(req: Request, state: &S) -> Self::Future {
        let state = state.clone();
        Box::pin(async move {
            let (mut parts, _body) = req.into_parts();
            <Self as FromRequestParts<S>>::from_request_parts(&mut parts, &state).await
        })
    }
}
