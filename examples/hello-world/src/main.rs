//! Hello, World! example for Volter.
//!
//! Run with: `cargo run -p hello-world`

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
use volter::{get, http, BoxBody, BoxError, Request, Router};

/// Root handler — responds with "Hello, World!" to every GET request.
async fn hello_world() -> &'static str {
    "Hello, World!"
}

// ---------------------------------------------------------------------------
// Body adapter: bridges hyper's `Incoming` body into Volter's `BoxBody`
//
// We cannot use `http_body_util::BodyExt::boxed` because it returns
// `UnsyncBoxBody` (not `Send`), and we cannot use `map_err` + `boxed`
// because `MapErr` is not `Send`.  The smallest correct wrapper is
// this adapter.
// ---------------------------------------------------------------------------

/// A `Send` body adapter that wraps [`hyper::body::Incoming`] and maps
/// its error type from [`hyper::Error`] to [`BoxError`].
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
    let app = Router::new().route("/", get(hello_world));

    let addr = std::net::SocketAddr::from(([0, 0, 0, 0], 3000));
    let listener = TcpListener::bind(addr).await.unwrap();
    eprintln!("Listening on http://{addr}");

    loop {
        let (stream, _) = listener.accept().await.unwrap();
        let io = TokioIo::new(stream);
        let app = app.clone();

        tokio::spawn(async move {
            // Wrap our Router in a `hyper::service::service_fn` that
            // converts the incoming hyper body to Volter's BoxBody.
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
