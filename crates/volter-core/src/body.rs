//! Body type aliases and construction helpers.
//!
//! Defines the concrete body types used throughout Volter: [`BoxBody`],
//! [`Body`], [`Request`], [`Response`], and [`BoxError`].

use std::pin::Pin;
use std::task::{Context, Poll};

use bytes::Bytes;
use http_body::Frame;

/// A boxed, dynamic error used where callers need `Send + Sync + 'static`.
pub type BoxError = Box<dyn std::error::Error + Send + Sync>;

/// A type-erased, `Send` HTTP body used throughout Volter.
pub type BoxBody = http_body_util::combinators::BoxBody<bytes::Bytes, BoxError>;

/// The default request/response body type used throughout Volter.
pub type Body = BoxBody;

/// The request type used throughout Volter: a thin alias over
/// [`http::Request`] with a boxed, streaming body.
pub type Request<B = BoxBody> = http::Request<B>;

/// The response type used throughout Volter.
pub type Response<B = BoxBody> = http::Response<B>;

// ---------------------------------------------------------------------------
// Internal body type
//
// We use a custom `VolterBody` instead of `http_body_util::Full` / `Empty`
// because the `BodyExt::boxed` method returns `UnsyncBoxBody` (not `Send`)
// and `boxed_send` is unavailable in this version of `http-body-util`.
// The only remaining way to obtain a `Send` body is via
// `BoxBody::new(body)` where `body: Body + Send + 'static`, so we provide
// the smallest possible wrapper that satisfies these bounds.
// ---------------------------------------------------------------------------

struct VolterBody(Option<Bytes>);

impl http_body::Body for VolterBody {
    type Data = Bytes;
    type Error = BoxError;

    fn poll_frame(
        self: Pin<&mut Self>,
        _cx: &mut Context<'_>,
    ) -> Poll<Option<Result<Frame<Self::Data>, Self::Error>>> {
        if let Some(data) = self.get_mut().0.take() {
            Poll::Ready(Some(Ok(Frame::data(data))))
        } else {
            Poll::Ready(None)
        }
    }

    fn is_end_stream(&self) -> bool {
        self.0.is_none()
    }

    fn size_hint(&self) -> http_body::SizeHint {
        let mut hint = http_body::SizeHint::new();
        if let Some(ref data) = self.0 {
            hint.set_exact(data.len() as u64);
        }
        hint
    }
}

/// Create an empty boxed body.
pub(crate) fn empty_body() -> BoxBody {
    BoxBody::new(VolterBody(None))
}

/// Create a boxed body from a chunk of bytes.
pub(crate) fn full_body(bytes: Bytes) -> BoxBody {
    BoxBody::new(VolterBody(Some(bytes)))
}
