//! Demonstrates `#[derive(FromRequestParts)]` and `#[derive(FromRequest)]`.
//!
//! Run with: `cargo run -p derive-extractors`

use serde::Deserialize;
use tokio::net::TcpListener;
use volter::*;

// ---------------------------------------------------------------------------
// JSON body extraction via `#[derive(FromRequest)]`
// ---------------------------------------------------------------------------

/// User creation payload parsed from a JSON request body.
///
/// `#[derive(FromRequest)]` generates a `FromRequest` implementation that
/// delegates to `Json<T>`.  This lets the type be used as a single-extractor
/// handler argument or as the last argument in a multi-extractor handler.
#[derive(Deserialize, FromRequest)]
struct CreateUser {
    name: String,
    age: u8,
}

async fn create_user(user: CreateUser) -> String {
    format!("Created user: {} (age {})", user.name, user.age)
}

// ---------------------------------------------------------------------------
// Query-string extraction via `#[derive(FromRequestParts)]`
// ---------------------------------------------------------------------------

/// Search parameters extracted from the URL query string.
///
/// `#[derive(FromRequestParts)]` generates a `FromRequestParts`
/// implementation that delegates to `Query<T>`.  This lets the type be
/// used as a non-last argument in a multi-extractor handler.  For
/// single-extractor handlers, use `#[derive(FromRequest)]` instead.
#[derive(Deserialize, FromRequestParts)]
struct SearchQuery {
    q: String,
    limit: Option<u32>,
}

/// Demonstrate extraction in a 2-argument handler with `State`.
///
/// `SearchQuery` is the first argument (parts-only), `State` is the
/// second (also parts-only, but acts as the "body" extractor position).
async fn search(query: SearchQuery, _state: State<AppState>) -> String {
    match query.limit {
        Some(limit) => format!("Searching for '{}' (limit: {limit})", query.q),
        None => format!("Searching for '{}' (no limit)", query.q),
    }
}

/// Direct extraction (no router / handler).
#[derive(Deserialize, FromRequestParts)]
struct Pagination {
    page: u32,
    per_page: u32,
}

// ---------------------------------------------------------------------------
// State
// ---------------------------------------------------------------------------

#[derive(Clone)]
#[allow(dead_code)]
struct AppState {
    db_url: String,
}

// ---------------------------------------------------------------------------
// Main
// ---------------------------------------------------------------------------

#[tokio::main]
async fn main() -> Result<(), volter::BoxError> {
    // Direct extraction — no router needed.
    let req = http::Request::builder()
        .uri("/list?page=2&per_page=20")
        .method("GET")
        .body(empty_body())
        .unwrap();
    let (mut parts, _body) = req.into_parts();
    let pagination = Pagination::from_request_parts(&mut parts, &()).await;
    eprintln!(
        "Direct extraction: page={}, per_page={}",
        pagination.as_ref().map(|p| p.page).unwrap_or(0),
        pagination.as_ref().map(|p| p.per_page).unwrap_or(0),
    );

    // Router with both handler styles.
    let app = Router::with_state(AppState {
        db_url: "postgres://localhost/volter".into(),
    })
    .route("/search", get(search))
    .route("/users", post(create_user));

    let addr = std::net::SocketAddr::from(([0, 0, 0, 0], 3000));
    let listener = TcpListener::bind(addr).await?;
    eprintln!("Listening on http://{addr}");
    eprintln!(
        "  GET  /search?q=rust&limit=10  (FromRequestParts derive, 2-arg handler with State)"
    );
    eprintln!("  POST /users                    (FromRequest derive)");

    serve(listener, app).await
}
