//! Core traits and types for Volter.
//!
//! This crate defines the foundational abstractions the rest of the Volter
//! ecosystem builds on:
//!
//! - [`IntoResponse`] — how values turn into HTTP responses.
//! - [`FromRequestParts`] / [`FromRequest`] — typed extraction from requests.
//! - [`Handler`] — async function handlers.
//! - [`HandlerService`] — a [`tower::Service`] adapter for handlers.
//! - [`Body`], [`BoxBody`], [`Request`], [`Response`] — core HTTP type aliases.
//!
//! See `ARCHITECTURE.md` at the workspace root for the reasoning behind
//! this design, and `RULES.md` for the constraints every implementation
//! must follow.

#![deny(missing_docs)]
#![deny(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

mod body;
mod extract;
mod handler;
mod into_response;
mod service;
mod url_params;

pub use body::{empty_body, full_body, Body, BoxBody, BoxError, Request, Response};
pub use extract::{FromRequest, FromRequestParts, State};
pub use handler::Handler;
pub use into_response::IntoResponse;
pub use service::HandlerService;
pub use url_params::UrlParams;

/// Re-export of the `http` crate so downstream users can refer to common
/// HTTP types ( [`http::StatusCode`], [`http::Method`], etc.) through
/// `volter_core::http`.
pub use http;

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::*;
    use crate::body::empty_body;
    use http::StatusCode;
    use std::convert::Infallible;
    use tower::Service;

    // -- IntoResponse tests --------------------------------------------------

    #[test]
    fn response_passthrough() {
        let original = Response::new(empty_body());
        let response = original.into_response();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[test]
    fn status_code_response() {
        let response = StatusCode::NOT_FOUND.into_response();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[test]
    fn str_response() {
        let response = "hello".into_response();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[test]
    fn string_response() {
        let response = String::from("hello").into_response();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[test]
    fn tuple_status_response() {
        let response = (StatusCode::CREATED, "body").into_response();
        assert_eq!(response.status(), StatusCode::CREATED);
    }

    #[test]
    fn unit_into_response() {
        let response = ().into_response();
        assert_eq!(response.status(), StatusCode::NO_CONTENT);
    }

    #[test]
    fn result_ok_into_response() {
        let response: Result<&'static str, StatusCode> = Ok("hello");
        let resp = response.into_response();
        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[test]
    fn result_err_into_response() {
        let response: Result<(), StatusCode> = Err(StatusCode::BAD_REQUEST);
        let resp = response.into_response();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    }

    // -- Handler / HandlerService tests --------------------------------------

    #[tokio::test]
    async fn zero_arg_handler() {
        async fn greet() -> &'static str {
            "hello"
        }

        let response = greet().await.into_response();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn handler_service_call() -> Result<(), Infallible> {
        async fn answer() -> &'static str {
            "forty-two"
        }

        let mut service = HandlerService::new(answer, ());
        let request = http::Request::new(empty_body());
        let response = service.call(request).await?;
        assert_eq!(response.status(), StatusCode::OK);
        Ok(())
    }

    #[tokio::test]
    async fn handler_service_oneshot() -> Result<(), Infallible> {
        async fn ping() -> &'static str {
            "pong"
        }

        let service = HandlerService::new(ping, ());
        let request = http::Request::new(empty_body());
        let response = tower::ServiceExt::oneshot(service, request).await?;
        assert_eq!(response.status(), StatusCode::OK);
        Ok(())
    }
}
