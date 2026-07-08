# Benchmarks

Criterion benchmarks are defined in `crates/volter/benches/router.rs` and run
via `cargo bench -p volter`. Each benchmark measures the wall-clock time of a
single request dispatch through the framework (no real TCP).

> These numbers are provided for transparency and may vary depending on
> hardware, operating system, compiler version, and runtime environment.

## Environment (current baseline)

| Attribute       | Value                  |
|-----------------|------------------------|
| Machine         | MacBook Pro M3 Max     |
| Memory          | 64 GB RAM              |
| Build           | `cargo bench` (release)|
| Benchmark tool  | Criterion              |

## Results

| Benchmark                   | Median time | Description                                  |
|-----------------------------|-------------|----------------------------------------------|
| `static_route`              | 313 ns      | Single static route, no extractors           |
| `path_params`               | 567 ns      | `/:id` pattern match + `Path<i32>`           |
| `query_extraction`          | 524 ns      | Query string deserialization (`Query<T>`)    |
| `json_extraction`           | 611 ns      | JSON body deserialization (`Json<T>`)        |
| `multi_extractor`           | 793 ns      | `State<App>` + `Path<i32>` + `Query<T>`      |
| `middleware/bare`           | 310 ns      | Plain route, no middleware                   |
| `middleware/with_layers`    | 1,292 ns    | 4 middleware layers (RequestId, Trace, Timeout, CatchPanic) |
| `full_pipeline`             | 339 ns      | End-to-end via `TestClient` (clone + dispatch) |

All timings are the median of 100 samples with a 5-second measurement window
and 2-second warm-up.

## Key observations

- **Static route dispatch** is ~313 ns, comparable to a few hash-map lookups.
- **Path parameters** add ~254 ns (pattern matching over the param-routes
  list).
- **Middleware layers** add ~250 ns each on this hardware (estimated from the
  4-layer stack adding ~980 ns vs bare).
- **Full pipeline overhead** (TestClient clone + request construction) adds
  ~26 ns over a direct `Service::call`.
- JSON deserialization and query-string parsing are dominated by `serde_json`
  and `serde_urlencoded`, not the framework itself.

## CI integration

These benchmarks are intended to be tracked across commits to catch
regressions. See `TOOLS.md` → "Benchmarking" for the CI workflow.
