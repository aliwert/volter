//! Middleware example for Volter.
//!
//! Demonstrates using `Router::layer(...)` with tower-native middleware.
//!
//! Run with: `cargo run -p middleware-example`
//!
//! Test with:
//!   curl -H "X-Auth: admin" http://localhost:3000/
//!   curl http://localhost:3000/profile

use std::convert::Infallible;
use std::pin::Pin;
use std::task::{Context, Poll};

use tokio::net::TcpListener;
use tower::util::BoxCloneService;
use tower::{Layer, Service};
use volter::{get, serve, BoxError, Extension, Request, Response, Router};

type Svc = BoxCloneService<Request, Response, Infallible>;

// ---------------------------------------------------------------------------
// Auth middleware
// ---------------------------------------------------------------------------

/// A user that has been authenticated.
#[derive(Clone, Debug)]
struct User {
    name: String,
    role: String,
}

/// A [`tower::Layer`] that inspects the `X-Auth` header and inserts an
/// [`Extension<User>`] into the request if the header is present.
#[derive(Clone)]
struct AuthLayer;

impl Layer<Svc> for AuthLayer {
    type Service = AuthService;

    fn layer(&self, inner: Svc) -> Self::Service {
        AuthService { inner }
    }
}

#[derive(Clone)]
struct AuthService {
    inner: Svc,
}

impl Service<Request> for AuthService {
    type Response = Response;
    type Error = Infallible;

    type Future =
        Pin<Box<dyn std::future::Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, mut req: Request) -> Self::Future {
        let auth_header = req
            .headers()
            .get("x-auth")
            .and_then(|v| v.to_str().ok())
            .map(|s| s.to_owned());

        if let Some(role) = auth_header {
            req.extensions_mut().insert(User {
                name: format!("Alice ({role})"),
                role,
            });
        }

        let mut inner = self.inner.clone();
        Box::pin(async move { inner.call(req).await })
    }
}

// ---------------------------------------------------------------------------
// Handlers
// ---------------------------------------------------------------------------

/// A protected endpoint that needs authentication.
async fn profile(Extension(user): Extension<User>) -> String {
    format!(
        "Welcome, {}! You are logged in as {}.",
        user.name, user.role
    )
}

/// A public endpoint that does not require auth.
async fn public() -> &'static str {
    "This is a public endpoint. Try /profile."
}

// ---------------------------------------------------------------------------
// Main
// ---------------------------------------------------------------------------

#[tokio::main]
async fn main() -> Result<(), BoxError> {
    let app = Router::new()
        .route("/", get(public))
        .route("/profile", get(profile))
        .layer(AuthLayer);

    let addr = std::net::SocketAddr::from(([0, 0, 0, 0], 3000));
    let listener = TcpListener::bind(addr).await?;
    eprintln!("Listening on http://{addr}");
    eprintln!("Try: curl http://localhost:3000/");
    eprintln!("Try: curl -H 'X-Auth: admin' http://localhost:3000/profile");

    serve(listener, app).await
}
