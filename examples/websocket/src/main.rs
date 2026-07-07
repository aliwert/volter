//! Simple echo WebSocket server for Volter.
//!
//! Run with: `cargo run -p websocket`
//!
//! Test with: `websocat ws://127.0.0.1:3000/ws`
//! or any WebSocket client.

#![allow(clippy::collapsible_match)]

use tokio::net::TcpListener;
use volter::ws::{Message, WebSocket, WebSocketUpgrade};
use volter::{get, serve, IntoResponse, Router};

/// WebSocket upgrade handler — validates the upgrade request and returns a
/// `101 Switching Protocols` response.  The callback receives an upgraded
/// [`WebSocket`] connection that echoes every message back to the client.
async fn ws_handler(ws: WebSocketUpgrade) -> impl IntoResponse {
    ws.on_upgrade(echo)
}

/// Echo every received message back to the sender.
async fn echo(mut socket: WebSocket) {
    while let Some(msg) = socket.recv().await {
        match msg {
            Ok(Message::Text(text)) => {
                if socket.send(Message::Text(text)).await.is_err() {
                    break;
                }
            }
            Ok(Message::Binary(data)) => {
                if socket.send(Message::Binary(data)).await.is_err() {
                    break;
                }
            }
            Ok(Message::Close(_)) | Err(_) => break,
            _ => {}
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), volter::BoxError> {
    let app = Router::new().route("/ws", get(ws_handler));

    let addr = std::net::SocketAddr::from(([0, 0, 0, 0], 3000));
    let listener = TcpListener::bind(addr).await?;
    eprintln!("Listening on ws://{addr}/ws");
    eprintln!("Test with: websocat ws://{addr}/ws");

    serve(listener, app).await
}
