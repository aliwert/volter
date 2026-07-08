# Contributing

## Getting Started

1. Clone the repository:

   ```bash
   git clone https://github.com/aliwert/volter
   cd volter
   ```

2. Build all crates:

   ```bash
   cargo build --workspace --all-features
   ```

3. Run tests:

   ```bash
   cargo test --workspace --all-features
   ```

## Code Requirements

All code must pass the following checks:

```bash
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --all-features
```

## Deny Rules (RULES.md)

The project has strict deny rules enforced via Clippy lint attributes:

- `clippy::unwrap_used` — use `?` or match instead of `.unwrap()`
- `clippy::expect_used` — same as unwrap
- `clippy::panic` — no panics in production code
- `clippy::indexing_slicing` — use `.get()` instead of direct indexing
- `unsafe` — not allowed in production code

These are enforced at the crate level in `lib.rs` and `main.rs`.

## MSRV

The minimum supported Rust version is **1.79**. All code must compile with
this version. Use `cargo +1.79 check --workspace --all-features` to verify.

## Pull Request Process

1. Create a feature branch from `main`
2. Make your changes
3. Add tests for new functionality
4. Run all checks (fmt, clippy, test)
5. Verify MSRV compatibility
6. Open a pull request

## Architecture

See [Architecture](architecture/architecture.md) for the crate layout and core
design patterns.

## Adding a New Feature

1. Check that the feature fits Volter's scope (web framework primitives)
2. Follow existing patterns in the relevant crate
3. Add public items with rustdoc documentation
4. Add integration tests in the `volter` crate's `tests/` directory
5. Add an example if the feature introduces a new user-facing concept

## Code of Conduct

Be respectful, constructive, and patient. This is a learning project — not
everyone has the same level of experience.
