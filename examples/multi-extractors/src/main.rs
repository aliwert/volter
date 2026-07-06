//! Multi-extractor example for Volter.
//!
//! Demonstrates a handler that uses State + Path + Query + Json in one
//! function.
//!
//! Run with: `cargo run -p multi-extractors-example`
//!
//! Test with:
//!   curl -X POST 'http://localhost:3000/items/42?format=json' \
//!     -H "Content-Type: application/json" \
//!     -d '{"name":"updated item"}'

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::pin::Pin;
use std::task::{Context, Poll};

use bytes::Bytes;
use http_body::{Body, Frame};
use hyper::body::Incoming;
use hyper_util::rt::{TokioExecutor, TokioIo};
use hyper_util::server::conn::auto::Builder;
use serde::{Deserialize, Serialize};
use tokio::net::TcpListener;
use tower::Service;
use volter::{get, http, BoxBody, BoxError, Json, Path, Query, Request, Router, State};

/// Shared application state.
#[derive(Clone)]
struct AppState {
    node_id: String,
}

/// Path parameters for `/items/:id`.
#[derive(Deserialize)]
struct ItemPath {
    id: u64,
}

/// Query parameters for `?format=...`.
#[derive(Deserialize)]
struct ItemQuery {
    format: Option<String>,
}

/// JSON body for the update payload.
#[derive(Deserialize, Serialize)]
struct UpdateBody {
    name: String,
}

/// Multi-extractor handler: State + Path + Query + Json.
async fn update_item(
    State(state): State<AppState>,
    Path(path): Path<ItemPath>,
    Query(query): Query<ItemQuery>,
    Json(body): Json<UpdateBody>,
) -> String {
    format!(
        "[node:{}] updated item {} (format: {:?}) with name '{}'",
        state.node_id, path.id, query.format, body.name
    )
}

// ---------------------------------------------------------------------------
// Body adapter: bridges hyper's `Incoming` body into Volter's `BoxBody`
// ---------------------------------------------------------------------------

struct IncomingAdapter(Incoming);

impl Body for IncomingAdapter {
    type Data = Bytes;
    type Error = BoxError;

    fn poll_frame(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Result<Frame<Self::Data>, Self::Error>>> {
        Pin::new(&mut self.get_mut().0)
            .poll_frame(cx)
            .map(|opt| opt.map(|res| res.map_err(|e| Box::new(e) as BoxError)))
    }

    fn is_end_stream(&self) -> bool {
        self.0.is_end_stream()
    }

    fn size_hint(&self) -> http_body::SizeHint {
        self.0.size_hint()
    }
}

fn convert_body(incoming: Incoming) -> BoxBody {
    BoxBody::new(IncomingAdapter(incoming))
}

#[tokio::main]
async fn main() {
    let app = Router::with_state(AppState {
        node_id: "volter-1".into(),
    })
    .route("/items/:id", get(update_item));

    let addr = std::net::SocketAddr::from(([0, 0, 0, 0], 3000));
    let listener = TcpListener::bind(addr).await.unwrap();
    eprintln!("Listening on http://{addr}");
    eprintln!("Try: curl -X POST 'http://localhost:3000/items/42?format=json' -H 'Content-Type: application/json' -d '{{\"name\":\"updated item\"}}'");

    loop {
        let (stream, _) = listener.accept().await.unwrap();
        let io = TokioIo::new(stream);
        let app = app.clone();

        tokio::spawn(async move {
            let svc = hyper::service::service_fn(move |req: http::Request<Incoming>| {
                let (parts, body) = req.into_parts();
                let req = Request::from_parts(parts, convert_body(body));
                let mut app = app.clone();
                async move { app.call(req).await }
            });

            let builder = Builder::new(TokioExecutor::new());
            if let Err(err) = builder.serve_connection(io, svc).await {
                eprintln!("connection error: {err}");
            }
        });
    }
}
