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

use serde::{Deserialize, Serialize};
use tokio::net::TcpListener;
use volter::{get, serve, Json, Path, Query, Router, State};

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

#[tokio::main]
async fn main() -> Result<(), volter::BoxError> {
    let app = Router::with_state(AppState {
        node_id: "volter-1".into(),
    })
    .route("/items/:id", get(update_item));

    let addr = std::net::SocketAddr::from(([0, 0, 0, 0], 3000));
    let listener = TcpListener::bind(addr).await?;
    eprintln!("Listening on http://{addr}");
    eprintln!("Try: curl -X POST 'http://localhost:3000/items/42?format=json' -H 'Content-Type: application/json' -d '{{\"name\":\"updated item\"}}'");

    serve(listener, app).await
}
