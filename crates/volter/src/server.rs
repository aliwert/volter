//! High-level server functions that wrap a [`Router`] with the necessary
//! hyper connection loop and body adapter boilerplate.
//!
//! # Architecture
//!
//! Every Volter application needs to:
//!
//! 1. Accept TCP connections on a [`TcpListener`].
//! 2. Wrap each connection in [`TokioIo`] (hyper-util's I/O adapter).
//! 3. Bridge hyper's [`Incoming`] body type to Volter's [`BoxBody`] via
//!    [`IncomingAdapter`].
//! 4. Dispatch the converted [`Request`] to the application's router.
//! 5. Spawn each connection on a new tokio task.
//!
//! [`serve`] and [`serve_with`] encapsulate that pattern, eliminating the
//! boilerplate that was previously duplicated across every example.

use std::pin::Pin;
use std::task::{Context, Poll};

use bytes::Bytes;
use http_body::{Body, Frame};
use hyper::body::Incoming;
use hyper_util::rt::{TokioExecutor, TokioIo};
use hyper_util::server::conn::auto::Builder;
use tokio::net::TcpListener;
use tower::Service;

use crate::{BoxBody, BoxError, Request, Router};

// ---------------------------------------------------------------------------
// Body adapter
// ---------------------------------------------------------------------------

/// Bridges [`hyper::body::Incoming`] into Volter's [`BoxBody`].
///
/// We cannot use `http_body_util::BodyExt::boxed` because it returns
/// `UnsyncBoxBody` (not `Send`), and we cannot use `map_err` + `boxed`
/// because `MapErr` is not `Send`.  This adapter is the smallest correct
/// wrapper.
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

// ---------------------------------------------------------------------------
// serve
// ---------------------------------------------------------------------------

/// Start serving requests with the given [`TcpListener`] and [`Router`].
///
/// For each incoming TCP connection the router is cloned and used to
/// handle all requests on that connection.  Hyper's [`Incoming`] body type
/// is automatically converted to Volter's [`BoxBody`] — no boilerplate
/// needed.
///
/// The function runs forever, accepting connections until the listener
/// returns a fatal error (e.g. the file descriptor has been closed).
/// Per-connection errors are logged to stderr and *do not* propagate.
///
/// # Errors
///
/// Returns [`BoxError`] only when [`TcpListener::accept`] fails — this is
/// unusual in practice and usually indicates the listener has been
/// shut down or is in a bad state.
///
/// # Examples
///
/// ```rust,no_run
/// use volter::{Router, get, serve};
/// use tokio::net::TcpListener;
///
/// async fn hello() -> &'static str {
///     "Hello, World!"
/// }
///
/// #[tokio::main]
/// async fn main() -> Result<(), volter::BoxError> {
///     let app = Router::new().route("/", get(hello));
///     let listener = TcpListener::bind("0.0.0.0:3000").await?;
///     serve(listener, app).await
/// }
/// ```
pub async fn serve<S: Clone + Send + 'static>(
    listener: TcpListener,
    app: Router<S>,
) -> Result<(), BoxError> {
    serve_with(listener, app, |_| {}).await
}

/// Like [`serve`] but accepts a closure that can modify each request after
/// the body has been converted but before it is dispatched to the router.
///
/// This is useful for injecting per-request state such as authentication
/// extensions (see the `extensions` example).
///
/// # Errors
///
/// See [`serve`].
pub async fn serve_with<S, F>(listener: TcpListener, app: Router<S>, f: F) -> Result<(), BoxError>
where
    S: Clone + Send + 'static,
    F: Fn(&mut Request) + Clone + Send + 'static,
{
    loop {
        let (stream, _) = listener.accept().await?;
        let io = TokioIo::new(stream);
        let app = app.clone();
        let f = f.clone();

        tokio::spawn(async move {
            let svc = hyper::service::service_fn(move |req: hyper::Request<Incoming>| {
                let (parts, body) = req.into_parts();
                let mut req = Request::from_parts(parts, convert_body(body));
                f(&mut req);
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
