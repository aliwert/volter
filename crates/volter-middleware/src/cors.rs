use std::convert::Infallible;
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};
use std::time::Duration;

use tower::util::BoxCloneService;
use tower::{Layer, Service};

use volter_core::{empty_body, http, Request, Response};

/// The concrete service type used internally by `Router::layer()`.
type Svc = BoxCloneService<Request, Response, Infallible>;

/// Configuration for allowed origins.
#[derive(Clone)]
enum OriginConfig {
    /// `Access-Control-Allow-Origin: *`
    Any,
    /// Only the listed origins are allowed.
    Specific(Vec<String>),
}

/// Configuration for allowed methods.
#[derive(Clone)]
enum MethodConfig {
    /// Reflect the request's `Access-Control-Request-Method`.
    Any,
    /// Only the listed methods are allowed.
    Specific(Vec<http::Method>),
}

/// Configuration for allowed headers.
#[derive(Clone)]
enum HeaderConfig {
    /// Reflect the request's `Access-Control-Request-Headers`.
    Any,
    /// Only the listed headers are allowed.
    Specific(Vec<http::header::HeaderName>),
}

/// A [`tower::Layer`] that sets CORS headers on responses and handles
/// preflight `OPTIONS` requests.
///
/// # Quick start
///
/// ```rust
/// use volter_middleware::CorsLayer;
/// use volter_router::{Router, get};
///
/// async fn handler() -> &'static str { "ok" }
///
/// // Allow all origins, methods, and headers (development-friendly):
/// let app = Router::new()
///     .route("/", get(handler))
///     .layer(CorsLayer::permissive());
/// ```
///
/// # Custom configuration
///
/// ```rust
/// use std::time::Duration;
/// use volter_middleware::CorsLayer;
/// use volter_router::{Router, get};
///
/// async fn handler() -> &'static str { "ok" }
///
/// let app = Router::new()
///     .route("/", get(handler))
///     .layer(
///         CorsLayer::new()
///             .allow_origin("https://example.com")
///             .allow_origin("https://app.example.com")
///             .allow_credentials()
///             .max_age(Duration::from_secs(3600)),
///     );
/// ```
///
/// Preflight `OPTIONS` requests are handled internally and never reach
/// the inner service.
#[derive(Clone)]
pub struct CorsLayer {
    origins: OriginConfig,
    methods: MethodConfig,
    headers: HeaderConfig,
    expose_headers: Vec<http::header::HeaderName>,
    max_age: Option<Duration>,
    credentials: bool,
}

impl CorsLayer {
    /// Allow all origins, methods, and headers.
    ///
    /// Suitable for development. Not recommended for production unless you
    /// truly intend to allow cross-origin requests from any origin.
    pub fn permissive() -> Self {
        CorsLayer {
            origins: OriginConfig::Any,
            methods: MethodConfig::Any,
            headers: HeaderConfig::Any,
            expose_headers: Vec::new(),
            max_age: None,
            credentials: false,
        }
    }

    /// Start with a restrictive CORS configuration (no origins allowed).
    ///
    /// Use the builder methods to configure:
    ///
    /// - [`allow_origin`](Self::allow_origin)
    /// - [`allow_methods`](Self::allow_methods)
    /// - [`allow_headers`](Self::allow_headers)
    pub fn new() -> Self {
        CorsLayer {
            origins: OriginConfig::Specific(Vec::new()),
            methods: MethodConfig::Specific(Vec::new()),
            headers: HeaderConfig::Specific(Vec::new()),
            expose_headers: Vec::new(),
            max_age: None,
            credentials: false,
        }
    }

    /// Add an allowed origin.
    ///
    /// May be called multiple times to add multiple origins.
    pub fn allow_origin(mut self, origin: &str) -> Self {
        let origins = match &mut self.origins {
            OriginConfig::Specific(list) => list,
            _ => {
                self.origins = OriginConfig::Specific(Vec::new());
                match &mut self.origins {
                    OriginConfig::Specific(list) => list,
                    _ => unreachable!(),
                }
            }
        };
        origins.push(origin.to_owned());
        self
    }

    /// Allow all origins (`*`).
    pub fn allow_any_origin(mut self) -> Self {
        self.origins = OriginConfig::Any;
        self
    }

    /// Set the allowed methods.
    ///
    /// May be called multiple times — subsequent calls replace the previous
    /// list.
    pub fn allow_methods<I>(mut self, methods: I) -> Self
    where
        I: IntoIterator<Item = http::Method>,
    {
        self.methods = MethodConfig::Specific(methods.into_iter().collect());
        self
    }

    /// Allow any method (reflects the preflight request method).
    pub fn allow_any_method(mut self) -> Self {
        self.methods = MethodConfig::Any;
        self
    }

    /// Set the allowed headers.
    ///
    /// May be called multiple times — subsequent calls replace the previous
    /// list.
    pub fn allow_headers<I>(mut self, headers: I) -> Self
    where
        I: IntoIterator<Item = http::header::HeaderName>,
    {
        self.headers = HeaderConfig::Specific(headers.into_iter().collect());
        self
    }

    /// Allow any header (reflects the preflight request headers).
    pub fn allow_any_header(mut self) -> Self {
        self.headers = HeaderConfig::Any;
        self
    }

    /// Set headers that browsers are allowed to access in the response.
    pub fn expose_headers<I>(mut self, headers: I) -> Self
    where
        I: IntoIterator<Item = http::header::HeaderName>,
    {
        self.expose_headers = headers.into_iter().collect();
        self
    }

    /// Set the `Access-Control-Max-Age` preflight cache duration.
    pub fn max_age(mut self, duration: Duration) -> Self {
        self.max_age = Some(duration);
        self
    }

    /// Allow credentials (cookies, authorization headers).
    ///
    /// When credentials are enabled, `Access-Control-Allow-Origin` cannot be
    /// `*` — the request's origin will be echoed back instead.
    pub fn allow_credentials(mut self) -> Self {
        self.credentials = true;
        self
    }

    /// Returns `true` if the given origin is allowed.
    fn is_origin_allowed(&self, origin: &str) -> bool {
        match &self.origins {
            OriginConfig::Any => true,
            OriginConfig::Specific(list) => list.iter().any(|o| o == origin),
        }
    }
}

impl Default for CorsLayer {
    fn default() -> Self {
        CorsLayer::permissive()
    }
}

impl Layer<Svc> for CorsLayer {
    type Service = CorsService;

    fn layer(&self, service: Svc) -> Self::Service {
        CorsService {
            inner: service,
            config: self.clone(),
        }
    }
}

/// The [`Service`] produced by [`CorsLayer`].
pub struct CorsService {
    inner: Svc,
    config: CorsLayer,
}

impl Clone for CorsService {
    fn clone(&self) -> Self {
        CorsService {
            inner: self.inner.clone(),
            config: self.config.clone(),
        }
    }
}

impl Service<Request> for CorsService {
    type Response = Response;
    type Error = Infallible;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, req: Request) -> Self::Future {
        let mut inner = self.inner.clone();
        let config = self.config.clone();

        Box::pin(async move {
            let is_preflight = req.method() == http::Method::OPTIONS
                && req.headers().get(http::header::ORIGIN).is_some()
                && req
                    .headers()
                    .get(http::header::ACCESS_CONTROL_REQUEST_METHOD)
                    .is_some();

            if is_preflight {
                return Ok(build_preflight_response(&config, &req));
            }

            let origin = req
                .headers()
                .get(http::header::ORIGIN)
                .and_then(|v| v.to_str().ok())
                .map(|s| s.to_owned());

            let mut response = inner.call(req).await?;

            if let Some(ref origin) = origin {
                apply_cors_headers(&mut response, &config, origin);
            }

            Ok(response)
        })
    }
}

/// Build a 204 No Content response for a preflight request.
fn build_preflight_response(config: &CorsLayer, req: &Request) -> Response {
    let origin = req
        .headers()
        .get(http::header::ORIGIN)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");

    let mut response = Response::new(empty_body());
    *response.status_mut() = http::StatusCode::NO_CONTENT;

    if !config.is_origin_allowed(origin) {
        return response;
    }

    set_origin_header(&mut response, config, origin);

    // Access-Control-Allow-Methods
    let methods_value = match &config.methods {
        MethodConfig::Any => {
            // Reflect the request's Access-Control-Request-Method
            let req_method = req
                .headers()
                .get(http::header::ACCESS_CONTROL_REQUEST_METHOD)
                .and_then(|v| v.to_str().ok())
                .unwrap_or("");
            http::HeaderValue::from_str(req_method)
                .unwrap_or_else(|_| http::HeaderValue::from_static(""))
        }
        MethodConfig::Specific(methods) => {
            let joined = methods
                .iter()
                .map(|m| m.as_str())
                .collect::<Vec<&str>>()
                .join(", ");
            http::HeaderValue::from_str(&joined)
                .unwrap_or_else(|_| http::HeaderValue::from_static(""))
        }
    };
    if !methods_value.is_empty() {
        response
            .headers_mut()
            .insert(http::header::ACCESS_CONTROL_ALLOW_METHODS, methods_value);
    }

    // Access-Control-Allow-Headers
    let headers_value = match &config.headers {
        HeaderConfig::Any => {
            // Reflect the request's Access-Control-Request-Headers
            req.headers()
                .get(http::header::ACCESS_CONTROL_REQUEST_HEADERS)
                .cloned()
        }
        HeaderConfig::Specific(headers) => {
            let joined = headers
                .iter()
                .map(|h| h.as_str())
                .collect::<Vec<&str>>()
                .join(", ");
            http::HeaderValue::from_str(&joined).ok()
        }
    };
    if let Some(value) = headers_value {
        if !value.is_empty() {
            response
                .headers_mut()
                .insert(http::header::ACCESS_CONTROL_ALLOW_HEADERS, value);
        }
    }

    // Access-Control-Max-Age
    if let Some(duration) = config.max_age {
        let secs = duration.as_secs();
        if let Ok(value) = http::HeaderValue::from_str(&secs.to_string()) {
            response
                .headers_mut()
                .insert(http::header::ACCESS_CONTROL_MAX_AGE, value);
        }
    }

    // Access-Control-Allow-Credentials
    if config.credentials {
        response.headers_mut().insert(
            http::header::ACCESS_CONTROL_ALLOW_CREDENTIALS,
            http::HeaderValue::from_static("true"),
        );
    }

    // Vary
    response
        .headers_mut()
        .insert(http::header::VARY, http::HeaderValue::from_static("origin"));

    response
}

/// Apply CORS headers to a normal (non-preflight) response.
fn apply_cors_headers(response: &mut Response, config: &CorsLayer, origin: &str) {
    if !config.is_origin_allowed(origin) {
        return;
    }

    set_origin_header(response, config, origin);

    // Access-Control-Expose-Headers
    if !config.expose_headers.is_empty() {
        let joined = config
            .expose_headers
            .iter()
            .map(|h| h.as_str())
            .collect::<Vec<&str>>()
            .join(", ");
        if let Ok(value) = http::HeaderValue::from_str(&joined) {
            response
                .headers_mut()
                .insert(http::header::ACCESS_CONTROL_EXPOSE_HEADERS, value);
        }
    }

    // Access-Control-Allow-Credentials
    if config.credentials {
        response.headers_mut().insert(
            http::header::ACCESS_CONTROL_ALLOW_CREDENTIALS,
            http::HeaderValue::from_static("true"),
        );
    }

    // Vary
    response
        .headers_mut()
        .insert(http::header::VARY, http::HeaderValue::from_static("origin"));
}

/// Set the `Access-Control-Allow-Origin` header.
fn set_origin_header(response: &mut Response, config: &CorsLayer, origin: &str) {
    let value = match &config.origins {
        OriginConfig::Any if !config.credentials => http::HeaderValue::from_static("*"),
        // With credentials or specific origins, echo the origin
        _ => http::HeaderValue::from_str(origin)
            .unwrap_or_else(|_| http::HeaderValue::from_static("")),
    };
    response
        .headers_mut()
        .insert(http::header::ACCESS_CONTROL_ALLOW_ORIGIN, value);
}
