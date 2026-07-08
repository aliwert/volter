# Performance

Benchmark results from the Criterion benchmark suite in
`crates/volter/benches/router.rs`.

> These numbers are provided for transparency and may vary depending on
> hardware, operating system, compiler version, and runtime environment.

## Test environment

| Attribute       | Value                  |
|-----------------|------------------------|
| Machine         | MacBook Pro M3 Max     |
| Memory          | 64 GB RAM              |
| Build           | `cargo bench` (release)|
| Benchmark tool  | Criterion              |

## Methodology

Each benchmark measures the wall-clock time of a single request dispatch
through the framework. No real TCP socket is bound — requests are constructed
in memory and dispatched directly through the router's `tower::Service::call`
implementation.

- **Sample size**: 100 measurements per benchmark
- **Measurement time**: 5 seconds per benchmark
- **Warm-up**: 2 seconds before measurement begins
- **Runtime**: `tokio::runtime::Runtime::block_on` for every iteration

## Results

| Benchmark                   | Median      | Description                                  |
|-----------------------------|-------------|----------------------------------------------|
| `static_route`              | 312.93 ns   | Single static route, no extractors           |
| `path_params`               | 566.83 ns   | `/:id` pattern match + `Path<i32>`           |
| `query_extraction`          | 523.72 ns   | Query string deserialization (`Query<T>`)    |
| `json_extraction`           | 610.58 ns   | JSON body deserialization (`Json<T>`)        |
| `multi_extractor`           | 792.91 ns   | `State<App>` + `Path<i32>` + `Query<T>`      |
| `middleware/bare`           | 310.01 ns   | Plain route, no middleware                   |
| `middleware/with_layers`    | 1,291.90 ns | 4 middleware layers                          |
| `full_pipeline`             | 338.71 ns   | End-to-end via `TestClient`                  |

## Observations

**Router dispatch** — Static routes cost ~313 ns (a hash-map lookup). Adding
a path parameter (`/:id`) raises this to ~567 ns due to the linear scan over
registered parameterised routes.

**Extractors** — `Query<T>` (~524 ns) and `Json<T>` (~611 ns) are dominated
by serde deserialization. The framework overhead beyond serde is minimal
(extension insertion for path params, body buffering for JSON).

**Middleware** — A stack of four layers (RequestId, Trace, Timeout,
CatchPanic) adds ~980 ns over a bare route, or roughly 250 ns per layer on
this hardware. Each layer adds a `BoxCloneService` wrapper and a small amount
of per-request bookkeeping.

**End-to-end** — The `TestClient` pipeline (which clones the router and
constructs a fresh request) adds ~26 ns compared to a direct
`Service::call`.

## Running locally

```bash
cargo bench -p volter
```

HTML reports with distribution plots are written to
`target/criterion/report/index.html`.

## CI

Benchmarks are tracked across commits to catch regressions. For the CI
workflow, see `TOOLS.md` → "Benchmarking".
