//! JSON body example for Volter.
//!
//! Run with: `cargo run -p json-example`
//!
//! Test with:
//!   curl -X POST http://localhost:3000/users \
//!     -H "Content-Type: application/json" \
//!     -d '{"name":"Alice","age":30}'

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
use volter::{get, http, BoxBody, BoxError, Json, Request, Router};

/// A user creation payload.
#[derive(Deserialize, Serialize)]
struct CreateUser {
    name: String,
    age: u8,
}

/// Create-user handler — echoes the JSON body back as a JSON response.
async fn create_user(Json(payload): Json<CreateUser>) -> Json<CreateUser> {
    Json(payload)
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
    let app = Router::new().route("/users", get(create_user));

    let addr = std::net::SocketAddr::from(([0, 0, 0, 0], 3000));
    let listener = TcpListener::bind(addr).await.unwrap();
    eprintln!("Listening on http://{addr}");
    eprintln!("Try: curl -X POST http://localhost:3000/users -H 'Content-Type: application/json' -d '{{\"name\":\"Alice\",\"age\":30}}'");

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
