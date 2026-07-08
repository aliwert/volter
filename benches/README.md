# Benchmarks

Criterion benchmarks, tracked in CI to catch regressions (see `TOOLS.md` →
"Testing"). Priority order once there's something to benchmark:

1. Router matching throughput vs. `matchit` (justifies or disproves the
   custom router — see `ARCHITECTURE.md` → "Router architecture").
2. `Json<T>` extraction + response serialization throughput.
3. End-to-end request/response latency through the full middleware stack.
