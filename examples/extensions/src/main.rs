//! Extension extractor example for Volter.
//!
//! Demonstrates manually inserting a request extension and extracting it
//! in a handler (simulating what middleware would do).
//!
//! Run with: `cargo run -p extensions-example`
//!
//! Test with:
//!   curl http://localhost:3000/profile

use tokio::net::TcpListener;
use volter::{get, serve_with, Extension, Request, Router};

/// An authenticated user attached to each request by (simulated) middleware.
#[derive(Clone, Debug)]
struct User {
    name: String,
    id: u64,
}

/// Profile handler — extracts the `User` extension and returns user info.
async fn profile(Extension(user): Extension<User>) -> String {
    format!("User {{ name: {}, id: {} }}", user.name, user.id)
}

#[tokio::main]
async fn main() -> Result<(), volter::BoxError> {
    let app = Router::new().route("/profile", get(profile));

    let addr = std::net::SocketAddr::from(([0, 0, 0, 0], 3000));
    let listener = TcpListener::bind(addr).await?;
    eprintln!("Listening on http://{addr}");
    eprintln!("Try: curl http://localhost:3000/profile");

    // Inject a User extension into every request before dispatch,
    // simulating what auth middleware would do in production.
    serve_with(listener, app, |req: &mut Request| {
        req.extensions_mut().insert(User {
            name: "Alice".into(),
            id: 42,
        });
    })
    .await
}
