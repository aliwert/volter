//! Query parameters example for Volter.
//!
//! Run with: `cargo run -p query-params`
//!
//! Test with:
//!   curl "http://localhost:3000/users?page=2&search=rust"
//!   curl "http://localhost:3000/users?page=1"

use serde::Deserialize;
use tokio::net::TcpListener;
use volter::{get, serve, Query, Router};

/// Query parameters for the users endpoint.
#[derive(Deserialize)]
struct UsersQuery {
    page: u32,
    search: Option<String>,
}

/// Users handler — extracts query parameters.
async fn users(Query(query): Query<UsersQuery>) -> String {
    match query.search {
        Some(search) => format!("Page {}, search={}", query.page, search),
        None => format!("Page {} (no search)", query.page),
    }
}

#[tokio::main]
async fn main() -> Result<(), volter::BoxError> {
    let app = Router::new().route("/users", get(users));

    let addr = std::net::SocketAddr::from(([0, 0, 0, 0], 3000));
    let listener = TcpListener::bind(addr).await?;
    eprintln!("Listening on http://{addr}");
    eprintln!("Try: curl 'http://localhost:3000/users?page=2&search=rust'");

    serve(listener, app).await
}
