//! WebSocket support for volter.
//!
//! A WebSocket endpoint is just a normal route — there is no separate
//! "WebSocket app" concept.  Use the [`WebSocketUpgrade`] extractor in a
//! handler and call [`WebSocketUpgrade::on_upgrade`] with a callback that
//! receives an upgraded [`WebSocket`] connection.
//!
//! # Example
//!
//! ```rust
//! use volter_ws::{WebSocketUpgrade, WebSocket, Message};
//! use volter_core::IntoResponse;
//!
//! async fn handler(ws: WebSocketUpgrade) -> impl IntoResponse {
//!     ws.on_upgrade(|mut socket| async move {
//!         while let Some(Ok(msg)) = socket.recv().await {
//!             if socket.send(msg).await.is_err() {
//!                 break;
//!             }
//!         }
//!     })
//! }
//! ```

#![deny(missing_docs)]
#![deny(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};

use futures_util::{SinkExt, StreamExt};
use http::StatusCode;
use hyper::rt::{Read, Write};
use tokio::io::{self, AsyncRead, AsyncWrite, ReadBuf};
use tokio_tungstenite::tungstenite::protocol::Role;
use tokio_tungstenite::WebSocketStream;

use volter_core::{
    empty_body, BoxBody, FromRequest, FromRequestParts, IntoResponse, Request, Response,
};

/// A WebSocket message — re-export of [`tokio_tungstenite::tungstenite::Message`].
pub use tokio_tungstenite::tungstenite::Message;

// ---------------------------------------------------------------------------
// HyperUpgradedIo — bridge hyper's IO traits to tokio's
// ---------------------------------------------------------------------------

/// Adapts [`hyper::upgrade::Upgraded`] (which implements [`hyper::rt::Read`]
/// and [`hyper::rt::Write`]) to implement [`tokio::io::AsyncRead`] and
/// [`tokio::io::AsyncWrite`] so it can be used with `tokio-tungstenite`.
struct HyperUpgradedIo {
    inner: hyper::upgrade::Upgraded,
}

impl AsyncRead for HyperUpgradedIo {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        let len = buf.remaining();
        if len == 0 {
            return Poll::Ready(Ok(()));
        }

        let n = {
            let unfilled = buf.initialize_unfilled();
            let mut hyper_buf = hyper::rt::ReadBuf::new(unfilled);
            let cursor = hyper_buf.unfilled();

            let this = self.get_mut();
            let pinned = Pin::new(&mut this.inner);
            match pinned.poll_read(cx, cursor) {
                Poll::Ready(Ok(())) => hyper_buf.filled().len(),
                Poll::Ready(Err(e)) => return Poll::Ready(Err(e)),
                Poll::Pending => return Poll::Pending,
            }
        };
        buf.advance(n);
        Poll::Ready(Ok(()))
    }
}

impl AsyncWrite for HyperUpgradedIo {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        let this = self.get_mut();
        Pin::new(&mut this.inner).poll_write(cx, buf)
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        let this = self.get_mut();
        Pin::new(&mut this.inner).poll_flush(cx)
    }

    fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        let this = self.get_mut();
        Pin::new(&mut this.inner).poll_shutdown(cx)
    }
}

// ---------------------------------------------------------------------------
// WebSocket
// ---------------------------------------------------------------------------

/// An upgraded WebSocket connection.
///
/// Provides `recv` and `send` methods for bidirectional communication
/// with the connected client.
pub struct WebSocket {
    inner: WebSocketStream<HyperUpgradedIo>,
}

impl WebSocket {
    /// Receive a message from the client.
    ///
    /// Returns `None` when the connection is closed or an error occurs
    /// that cannot be recovered from.
    pub async fn recv(&mut self) -> Option<Result<Message, tokio_tungstenite::tungstenite::Error>> {
        self.inner.next().await
    }

    /// Send a message to the client.
    ///
    /// Returns an error if the connection has been closed or a write
    /// failure occurs.
    pub async fn send(
        &mut self,
        msg: Message,
    ) -> Result<(), tokio_tungstenite::tungstenite::Error> {
        self.inner.send(msg).await
    }
}

// ---------------------------------------------------------------------------
// WebSocketUpgrade
// ---------------------------------------------------------------------------

/// An extractor for WebSocket upgrades.
///
/// Implements [`FromRequestParts`] — a handler takes it as an argument
/// and calls [`on_upgrade`](WebSocketUpgrade::on_upgrade) to produce
/// a `101 Switching Protocols` response.
///
/// # Non-WebSocket requests
///
/// If the request is missing the `Upgrade: websocket` header, extraction
/// returns [`WsRejection::MissingUpgradeHeader`] (`426 Upgrade Required`).
/// If the `Sec-WebSocket-Key` header is missing, extraction returns
/// [`WsRejection::MissingWebSocketKey`] (`400 Bad Request`).
pub struct WebSocketUpgrade {
    /// The `OnUpgrade` future inserted by hyper (when available).
    on_upgrade: Option<hyper::upgrade::OnUpgrade>,
}

impl WebSocketUpgrade {
    /// Spawn the WebSocket upgrade and invoke `callback` with the
    /// upgraded connection.
    ///
    /// Returns a `101 Switching Protocols` response immediately.  The
    /// callback runs in a spawned tokio task once hyper completes the
    /// connection upgrade.
    pub fn on_upgrade<F, Fut>(self, callback: F) -> Response
    where
        F: FnOnce(WebSocket) -> Fut + Send + 'static,
        Fut: Future<Output = ()> + Send + 'static,
    {
        if let Some(upgrade) = self.on_upgrade {
            tokio::spawn(async move {
                if let Ok(upgraded) = upgrade.await {
                    let io = HyperUpgradedIo { inner: upgraded };
                    let ws_stream = WebSocketStream::from_raw_socket(io, Role::Server, None).await;
                    let socket = WebSocket { inner: ws_stream };
                    callback(socket).await;
                }
            });
        }

        let mut response = Response::new(empty_body());
        *response.status_mut() = StatusCode::SWITCHING_PROTOCOLS;
        response
    }
}

// ---------------------------------------------------------------------------
// WsRejection
// ---------------------------------------------------------------------------

/// The error returned when [`WebSocketUpgrade`] extraction fails.
#[derive(Debug)]
pub enum WsRejection {
    /// The request is missing the `Upgrade: websocket` header.
    MissingUpgradeHeader,
    /// The request is missing the `Sec-WebSocket-Key` header.
    MissingWebSocketKey,
}

impl IntoResponse for WsRejection {
    fn into_response(self) -> Response {
        let status = match self {
            Self::MissingUpgradeHeader => StatusCode::UPGRADE_REQUIRED,
            Self::MissingWebSocketKey => StatusCode::BAD_REQUEST,
        };
        let mut response = Response::new(empty_body());
        *response.status_mut() = status;
        response
    }
}

// ---------------------------------------------------------------------------
// FromRequestParts impl
// ---------------------------------------------------------------------------

impl<S: Send + Sync + 'static> FromRequestParts<S> for WebSocketUpgrade {
    type Rejection = WsRejection;
    type Future = std::future::Ready<Result<Self, Self::Rejection>>;

    fn from_request_parts(parts: &mut http::request::Parts, _state: &S) -> Self::Future {
        let has_upgrade = parts
            .headers
            .get(http::header::UPGRADE)
            .and_then(|v| v.to_str().ok())
            .map(|v| v.trim().eq_ignore_ascii_case("websocket"))
            .unwrap_or(false);

        if !has_upgrade {
            return std::future::ready(Err(WsRejection::MissingUpgradeHeader));
        }

        if !parts.headers.contains_key(http::header::SEC_WEBSOCKET_KEY) {
            return std::future::ready(Err(WsRejection::MissingWebSocketKey));
        }

        let on_upgrade = parts.extensions.remove::<hyper::upgrade::OnUpgrade>();

        std::future::ready(Ok(WebSocketUpgrade { on_upgrade }))
    }
}

// ---------------------------------------------------------------------------
// FromRequest impl (body-consuming variant)
// ---------------------------------------------------------------------------

impl<S: Clone + Send + Sync + 'static> FromRequest<S, BoxBody> for WebSocketUpgrade {
    type Rejection = WsRejection;
    type Future = Pin<Box<dyn Future<Output = Result<Self, Self::Rejection>> + Send>>;

    fn from_request(req: Request, state: &S) -> Self::Future {
        let state = state.clone();
        Box::pin(async move {
            let (mut parts, _body) = req.into_parts();
            <Self as FromRequestParts<S>>::from_request_parts(&mut parts, &state).await
        })
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::*;
    use http::Request;

    fn websocket_request() -> http::Request<volter_core::BoxBody> {
        Request::builder()
            .header("Upgrade", "websocket")
            .header("Connection", "Upgrade")
            .header("Sec-WebSocket-Key", "dGhlIHNhbXBsZSBub25jZQ==")
            .header("Sec-WebSocket-Version", "13")
            .body(empty_body())
            .unwrap()
    }

    fn plain_request() -> http::Request<volter_core::BoxBody> {
        Request::builder().uri("/").body(empty_body()).unwrap()
    }

    fn request_no_key() -> http::Request<volter_core::BoxBody> {
        Request::builder()
            .header("Upgrade", "websocket")
            .header("Connection", "Upgrade")
            .body(empty_body())
            .unwrap()
    }

    #[tokio::test]
    async fn valid_websocket_request_extracts_ok() {
        let req = websocket_request();
        let (mut parts, _body) = req.into_parts();
        let result = WebSocketUpgrade::from_request_parts(&mut parts, &());
        assert!(result.await.is_ok());
    }

    #[tokio::test]
    async fn missing_upgrade_header_returns_rejection() {
        let req = plain_request();
        let (mut parts, _body) = req.into_parts();
        let result = WebSocketUpgrade::from_request_parts(&mut parts, &());
        assert!(matches!(
            result.await,
            Err(WsRejection::MissingUpgradeHeader)
        ));
    }

    #[tokio::test]
    async fn missing_websocket_key_returns_rejection() {
        let req = request_no_key();
        let (mut parts, _body) = req.into_parts();
        let result = WebSocketUpgrade::from_request_parts(&mut parts, &());
        assert!(matches!(
            result.await,
            Err(WsRejection::MissingWebSocketKey)
        ));
    }

    #[tokio::test]
    async fn on_upgrade_returns_101_without_upgrade_future() {
        let ws = WebSocketUpgrade { on_upgrade: None };
        let resp = ws.on_upgrade(|_socket| async move {});
        assert_eq!(resp.status(), StatusCode::SWITCHING_PROTOCOLS);
    }

    #[tokio::test]
    async fn rejection_missing_upgrade_header_into_response() {
        let rejection = WsRejection::MissingUpgradeHeader;
        let resp = rejection.into_response();
        assert_eq!(resp.status(), StatusCode::UPGRADE_REQUIRED);
    }

    #[tokio::test]
    async fn rejection_missing_key_into_response() {
        let rejection = WsRejection::MissingWebSocketKey;
        let resp = rejection.into_response();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    }
}
