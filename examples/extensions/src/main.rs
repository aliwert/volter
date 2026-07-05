//! Extension extractor example for Volter.
//!
//! Demonstrates manually inserting a request extension and extracting it
//! in a handler (simulating what middleware would do).
//!
//! Run with: `cargo run -p extensions-example`
//!
//! Test with:
//!   curl http://localhost:3000/profile

#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::pin::Pin;
use std::task::{Context, Poll};

use bytes::Bytes;
use http_body::{Body, Frame};
use hyper::body::Incoming;
use hyper_util::rt::{TokioExecutor, TokioIo};
use hyper_util::server::conn::auto::Builder;
use tokio::net::TcpListener;
use tower::Service;
use volter::{get, http, BoxBody, BoxError, Extension, Request, Router};

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
    let app = Router::new().route("/profile", get(profile));

    let addr = std::net::SocketAddr::from(([0, 0, 0, 0], 3000));
    let listener = TcpListener::bind(addr).await.unwrap();
    eprintln!("Listening on http://{addr}");
    eprintln!("Try: curl http://localhost:3000/profile");

    loop {
        let (stream, _) = listener.accept().await.unwrap();
        let io = TokioIo::new(stream);
        let app = app.clone();

        tokio::spawn(async move {
            let svc = hyper::service::service_fn(move |req: http::Request<Incoming>| {
                let (mut parts, body) = req.into_parts();

                // Simulate auth middleware: attach a User to every request.
                parts.extensions.insert(User {
                    name: "Alice".into(),
                    id: 42,
                });

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
