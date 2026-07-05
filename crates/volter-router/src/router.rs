//! [`Router`] — the central request dispatcher.
//!
//! A `Router` maps request paths to [`MethodRouter`] instances.  When a
//! request arrives, it extracts the path, looks up the corresponding method
//! router, and delegates dispatch to it.  If no route matches, a `404 Not
//! Found` response is returned.

use std::collections::HashMap;
use std::convert::Infallible;
use std::task::{Context, Poll};

use tower::Service;
use volter_core::{Request, Response};

use crate::error::not_found_response;
use crate::method_router::{BoxedFuture, MethodRouter};

/// A Volter router.
///
/// Routes incoming HTTP requests to handlers based on the request path and
/// HTTP method.  Implements [`tower::Service`] so it composes with the
/// entire `tower` / `tower-http` middleware ecosystem.
///
/// # Example
///
/// ```rust
/// use volter_router::{Router, get};
///
/// async fn index() -> &'static str {
///     "Hello, World!"
/// }
///
/// let app: Router = Router::new().route("/", get(index));
/// ```
pub struct Router {
    routes: HashMap<String, MethodRouter>,
}

impl Router {
    /// Create a new empty router.
    pub fn new() -> Self {
        Self {
            routes: HashMap::new(),
        }
    }

    /// Register a route.
    ///
    /// The `path` argument is the request path (e.g. `"/"`, `"/hello"`).
    ///
    /// Internally, each [`MethodRouter`] is stored per path.  When a later PR
    /// introduces a radix tree, this method's signature will remain the same
    /// — only the internal storage changes.
    pub fn route(mut self, path: &str, method_router: MethodRouter) -> Self {
        self.routes.insert(path.to_owned(), method_router);
        self
    }
}

impl Default for Router {
    fn default() -> Self {
        Self::new()
    }
}

impl Clone for Router {
    fn clone(&self) -> Self {
        Self {
            routes: self.routes.clone(),
        }
    }
}

impl Service<Request> for Router {
    type Response = Response;
    type Error = Infallible;
    type Future = BoxedFuture;

    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, req: Request) -> Self::Future {
        let path = req.uri().path().to_owned();

        if let Some(method_router) = self.routes.get(&path) {
            method_router.call(req)
        } else {
            Box::pin(async move { Ok(not_found_response()) })
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::*;
    use crate::get;
    use http::Method;
    use http::StatusCode;
    use volter_core::empty_body;

    fn request(method: Method, path: &str) -> Request {
        http::Request::builder()
            .method(method)
            .uri(path)
            .body(empty_body())
            .unwrap()
    }

    #[tokio::test]
    async fn get_root() {
        async fn root() -> &'static str {
            "Hello, World!"
        }

        let mut app = Router::new().route("/", get(root));
        let response = app.call(request(Method::GET, "/")).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn get_hello() {
        async fn hello() -> &'static str {
            "Hello!"
        }

        let mut app = Router::new().route("/hello", get(hello));
        let response = app.call(request(Method::GET, "/hello")).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn unknown_path_returns_404() {
        async fn root() -> &'static str {
            "Hello!"
        }

        let mut app = Router::new().route("/", get(root));
        let response = app.call(request(Method::GET, "/unknown")).await.unwrap();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn wrong_method_returns_405() {
        async fn root() -> &'static str {
            "Hello!"
        }

        let mut app = Router::new().route("/", get(root));
        let response = app.call(request(Method::POST, "/")).await.unwrap();
        assert_eq!(response.status(), StatusCode::METHOD_NOT_ALLOWED);
    }

    #[tokio::test]
    async fn multiple_routes() {
        async fn root() -> &'static str {
            "root"
        }
        async fn hello() -> &'static str {
            "hello"
        }

        let mut app = Router::new()
            .route("/", get(root))
            .route("/hello", get(hello));

        let root_resp = app.call(request(Method::GET, "/")).await.unwrap();
        assert_eq!(root_resp.status(), StatusCode::OK);

        let hello_resp = app.call(request(Method::GET, "/hello")).await.unwrap();
        assert_eq!(hello_resp.status(), StatusCode::OK);

        let unknown_resp = app.call(request(Method::GET, "/nope")).await.unwrap();
        assert_eq!(unknown_resp.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn router_implements_tower_service() {
        async fn root() -> &'static str {
            "root"
        }

        let app = Router::new().route("/", get(root));
        let response = tower::ServiceExt::oneshot(app, request(Method::GET, "/"))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }
}
