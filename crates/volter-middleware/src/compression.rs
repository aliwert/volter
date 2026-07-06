use std::convert::Infallible;
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};

use tower::util::BoxCloneService;
use tower::{Layer, Service};
use tower_http::compression::{
    predicate::DefaultPredicate, Compression as TowerCompression, CompressionLevel,
};

use volter_core::{BoxBody, Request, Response};

/// The concrete service type used internally by `Router::layer()`.
type Svc = BoxCloneService<Request, Response, Infallible>;

/// A [`tower::Layer`] that compresses response bodies based on the
/// `Accept-Encoding` request header.
///
/// Delegates compression to `tower-http` — no compression algorithms are
/// reimplemented here.
///
/// # Quick start
///
/// ```rust
/// use volter_middleware::CompressionLayer;
/// use volter_router::{Router, get};
///
/// async fn handler() -> &'static str { "hello" }
///
/// let app = Router::new()
///     .route("/", get(handler))
///     .layer(CompressionLayer::new());
/// ```
///
/// # Algorithm selection
///
/// ```rust
/// use volter_middleware::CompressionLayer;
/// use volter_router::{Router, get};
///
/// async fn handler() -> &'static str { "hello" }
///
/// // Only allow gzip:
/// let app = Router::new()
///     .route("/", get(handler))
///     .layer(CompressionLayer::gzip());
/// ```
#[derive(Clone, Debug)]
pub struct CompressionLayer {
    gzip: bool,
    br: bool,
    zstd: bool,
    deflate: bool,
    quality: CompressionLevel,
}

impl CompressionLayer {
    /// Allow all supported compression algorithms.
    ///
    /// The client's `Accept-Encoding` header determines the actual encoding
    /// used. Supported: gzip, brotli (br), zstd, deflate.
    pub fn new() -> Self {
        CompressionLayer {
            gzip: true,
            br: true,
            zstd: true,
            deflate: true,
            quality: CompressionLevel::default(),
        }
    }

    /// Only accept gzip compression.
    pub fn gzip() -> Self {
        CompressionLayer {
            gzip: true,
            br: false,
            zstd: false,
            deflate: false,
            quality: CompressionLevel::default(),
        }
    }

    /// Only accept brotli compression.
    pub fn br() -> Self {
        CompressionLayer {
            gzip: false,
            br: true,
            zstd: false,
            deflate: false,
            quality: CompressionLevel::default(),
        }
    }

    /// Only accept zstd compression.
    pub fn zstd() -> Self {
        CompressionLayer {
            gzip: false,
            br: false,
            zstd: true,
            deflate: false,
            quality: CompressionLevel::default(),
        }
    }

    /// Only accept deflate compression.
    pub fn deflate() -> Self {
        CompressionLayer {
            gzip: false,
            br: false,
            zstd: false,
            deflate: true,
            quality: CompressionLevel::default(),
        }
    }
}

impl Default for CompressionLayer {
    fn default() -> Self {
        CompressionLayer::new()
    }
}

impl Layer<Svc> for CompressionLayer {
    type Service = CompressionService;

    fn layer(&self, service: Svc) -> Self::Service {
        let compression = TowerCompression::new(service)
            .gzip(self.gzip)
            .br(self.br)
            .zstd(self.zstd)
            .deflate(self.deflate)
            .quality(self.quality);

        CompressionService { inner: compression }
    }
}

/// The [`Service`] produced by [`CompressionLayer`].
pub struct CompressionService {
    inner: TowerCompression<Svc, DefaultPredicate>,
}

impl Clone for CompressionService {
    fn clone(&self) -> Self {
        CompressionService {
            inner: self.inner.clone(),
        }
    }
}

impl Service<Request> for CompressionService {
    type Response = Response;
    type Error = Infallible;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    #[inline]
    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx).map_err(|e| match e {})
    }

    fn call(&mut self, req: Request) -> Self::Future {
        let response_future = self.inner.call(req);

        Box::pin(async move {
            let response = response_future.await?;
            let (parts, body) = response.into_parts();
            // body is CompressionBody<BoxBody> — box it back to BoxBody
            let boxed = BoxBody::new(body);
            Ok(Response::from_parts(parts, boxed))
        })
    }
}
