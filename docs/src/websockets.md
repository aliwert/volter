# WebSockets

Volter supports WebSocket upgrades via the `WebSocketUpgrade` extractor.

## Enabling the Feature

WebSocket support is gated behind the `ws` feature flag (not enabled by
default):

```bash
cargo add volter --features ws
```

## Basic Echo Server

```rust
use volter::*;
use volter::ws::{Message, WebSocketUpgrade};

async fn echo(ws: WebSocketUpgrade) -> impl IntoResponse {
    ws.on_upgrade(|mut socket| async move {
        while let Some(Ok(msg)) = socket.recv().await {
            if socket.send(msg).await.is_err() {
                break;
            }
        }
    })
}
```

The handler receives a `WebSocketUpgrade` extractor. Calling
`on_upgrade(callback)` immediately returns a `101 Switching Protocols`
response, and the callback runs in a spawned tokio task once hyper completes
the upgrade.

## The WebSocket Upgrade Flow

1. The handler receives a `WebSocketUpgrade` parameter
2. Volter checks if the request has the `Upgrade: websocket` header
3. If yes, `on_upgrade()` produces a `101 Switching Protocols` response
4. The callback receives a `WebSocket` with `recv()` / `send()` methods
5. If the `Upgrade` header is missing, `426 Upgrade Required` is returned
6. If `Sec-WebSocket-Key` is missing, `400 Bad Request` is returned

## Sending and Receiving Messages

```rust
use volter::ws::{Message, WebSocketUpgrade};

async fn handle_ws(ws: WebSocketUpgrade) -> impl IntoResponse {
    ws.on_upgrade(|mut socket| async move {
        // Send a text message
        let _ = socket.send(Message::Text("Welcome!".into())).await;

        // Read messages
        while let Some(Ok(msg)) = socket.recv().await {
            match msg {
                Message::Text(text) => {
                    let _ = socket.send(Message::Text(format!("Echo: {text}"))).await;
                }
                Message::Binary(data) => {
                    let _ = socket.send(Message::Binary(data)).await;
                }
                Message::Ping(_) => {
                    let _ = socket.send(Message::Pong(vec![])).await;
                }
                Message::Close(frame) => {
                    let _ = socket.send(Message::Close(frame)).await;
                    break;
                }
                _ => {}
            }
        }
    })
}
```

## WebSocket with State

```rust
use volter::*;
use volter::ws::WebSocketUpgrade;

#[derive(Clone)]
struct AppState {
    max_message_size: usize,
}

async fn ws_handler(
    State(state): State<AppState>,
    ws: WebSocketUpgrade,
) -> impl IntoResponse {
    ws.on_upgrade(move |mut socket| async move {
        // state is available here
    })
}
```

## WebSocket with Query Parameters

```rust
use serde::Deserialize;
use volter::*;
use volter::ws::WebSocketUpgrade;

#[derive(Deserialize)]
struct WsParams {
    room: String,
    token: String,
}

async fn chat(
    Query(params): Query<WsParams>,
    ws: WebSocketUpgrade,
) -> impl IntoResponse {
    // Validate token and join room
    ws.on_upgrade(move |mut socket| async move {
        // ...
    })
}
```

## Important Notes

- `WebSocketUpgrade` implements `FromRequestParts`, so it runs before the body
  is consumed
- The `on_upgrade` callback runs in a spawned tokio task — state must be
  `'static` if captured by the closure
- See the `websocket` example for a complete, runnable server:

  ```bash
  cargo run -p websocket
  ```
