//! Micro-benchmarks for the Volter router and extractors.
//!
//! Each benchmark measures the throughput of a specific dispatch path
//! through the framework (no real TCP — all in-process).
//!
//! Run with:
//!     cargo bench -p volter
//!
//! For detailed HTML reports, open `target/criterion/report/index.html`.

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use serde::Deserialize;
use std::time::Duration;
use tower::Service;
use volter::*;
use volter_testing::TestClient;

// ---------------------------------------------------------------------------
// Helper types
// ---------------------------------------------------------------------------

#[derive(Clone, Deserialize)]
struct Greeting {
    name: String,
}

#[derive(Clone, Deserialize)]
struct SearchQuery {
    q: Option<String>,
}

#[derive(Clone)]
struct AppState {
    label: &'static str,
}

// ---------------------------------------------------------------------------
// Helper functions
// ---------------------------------------------------------------------------

fn req_get(path: &str) -> Request {
    Request::builder()
        .method(http::Method::GET)
        .uri(path)
        .body(empty_body())
        .unwrap()
}

fn req_body(method: http::Method, path: &str, body: &[u8], content_type: &str) -> Request {
    Request::builder()
        .method(method)
        .uri(path)
        .header(http::header::CONTENT_TYPE, content_type)
        .body(full_body(bytes::Bytes::copy_from_slice(body)))
        .unwrap()
}

// ---------------------------------------------------------------------------
// Benchmarks
// ---------------------------------------------------------------------------

fn bench_static_route(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();

    let router = Router::new().route("/", get(|| async { "Hello, World!" }));

    c.bench_function("static_route", |b| {
        b.iter(|| {
            rt.block_on(async {
                let mut r = router.clone();
                let resp = r.call(req_get("/")).await.unwrap();
                black_box(resp.status());
            })
        })
    });
}

fn bench_path_params(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();

    let router = Router::new().route(
        "/users/:id",
        get(|Path(id): Path<i32>| async move { format!("user {id}") }),
    );

    c.bench_function("path_params", |b| {
        b.iter(|| {
            rt.block_on(async {
                let mut r = router.clone();
                let resp = r.call(req_get("/users/42")).await.unwrap();
                black_box(resp.status());
            })
        })
    });
}

fn bench_query_extraction(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();

    let router = Router::new().route(
        "/search",
        get(|Query(q): Query<SearchQuery>| async move { format!("{:?}", q.q) }),
    );

    c.bench_function("query_extraction", |b| {
        b.iter(|| {
            rt.block_on(async {
                let mut r = router.clone();
                let resp = r.call(req_get("/search?q=hello")).await.unwrap();
                black_box(resp.status());
            })
        })
    });
}

fn bench_json_extraction(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let payload = br#"{"name":"ferris"}"#;

    let router = Router::new().route(
        "/greet",
        post(|Json(g): Json<Greeting>| async move { format!("hello {}", g.name) }),
    );

    c.bench_function("json_extraction", |b| {
        b.iter(|| {
            rt.block_on(async {
                let mut r = router.clone();
                let req = req_body(http::Method::POST, "/greet", payload, "application/json");
                let resp = r.call(req).await.unwrap();
                black_box(resp.status());
            })
        })
    });
}

fn bench_multi_extractor(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let state = AppState { label: "state" };

    let router = Router::with_state(state).route(
        "/users/:id/search",
        get(
            |State(s): State<AppState>,
             Path(id): Path<i32>,
             Query(q): Query<SearchQuery>| async move { format!("{} {} {:?}", s.label, id, q.q) },
        ),
    );

    c.bench_function("multi_extractor", |b| {
        b.iter(|| {
            rt.block_on(async {
                let mut r = router.clone();
                let resp = r.call(req_get("/users/42/search?q=hello")).await.unwrap();
                black_box(resp.status());
            })
        })
    });
}

fn bench_middleware_stack(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();

    let plain = Router::new().route("/", get(|| async { "Hello, World!" }));

    let layered = Router::new()
        .route("/", get(|| async { "Hello, World!" }))
        .layer(RequestIdLayer::new())
        .layer(TraceLayer::new())
        .layer(TimeoutLayer::new(Duration::from_secs(5)))
        .layer(CatchPanicLayer::new());

    let mut group = c.benchmark_group("middleware");

    group.bench_function("bare", |b| {
        b.iter(|| {
            rt.block_on(async {
                let mut r = plain.clone();
                let resp = r.call(req_get("/")).await.unwrap();
                black_box(resp.status());
            })
        })
    });

    group.bench_function("with_layers", |b| {
        b.iter(|| {
            rt.block_on(async {
                let mut r = layered.clone();
                let resp = r.call(req_get("/")).await.unwrap();
                black_box(resp.status());
            })
        })
    });

    group.finish();
}

fn bench_full_pipeline(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();

    let app = Router::new().route("/", get(|| async { "Hello, World!" }));
    let client = TestClient::new(app);

    c.bench_function("full_pipeline", |b| {
        b.iter(|| {
            rt.block_on(async {
                let resp = client.get("/").send().await;
                black_box(resp.status());
            })
        })
    });
}

// ---------------------------------------------------------------------------
// Criterion registration
// ---------------------------------------------------------------------------

criterion_group! {
    name = benches;
    config = Criterion::default()
        .measurement_time(Duration::from_secs(5))
        .warm_up_time(Duration::from_secs(2))
        .sample_size(100);
    targets =
        bench_static_route,
        bench_path_params,
        bench_query_extraction,
        bench_json_extraction,
        bench_multi_extractor,
        bench_middleware_stack,
        bench_full_pipeline,
}
criterion_main!(benches);
