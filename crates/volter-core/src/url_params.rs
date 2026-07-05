//! [`UrlParams`] — path parameters extracted by the router.
//!
//! This type is stored as a request extension by the router and read by the
//! [`Path`](crate::extract::FromRequestParts) extractor.  It lives in
//! `volter-core` because both `volter-router` (the writer) and
//! `volter-extract` (the reader) depend on this crate.

/// Path parameters extracted by the router from a parameterized route.
///
/// Each entry is a `(name, value)` pair corresponding to a named segment
/// in the route pattern (e.g. `:id` → `("id", "42")`).
///
/// This type is not intended for direct use by application code — use
/// the [`Path`](crate::extract::FromRequestParts) extractor instead.
#[derive(Clone, Debug, Default)]
pub struct UrlParams(pub Vec<(String, String)>);
