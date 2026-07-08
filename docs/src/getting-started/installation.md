# Installation

## Rust Toolchain

Volter requires Rust **1.79 or later**. Check your version:

```bash
rustc --version
```

To install or update Rust, use [rustup]:

```bash
rustup update stable
```

## Add Volter to Your Project

Create a new Rust project:

```bash
cargo new my-app
cd my-app
```

Add Volter as a dependency:

```bash
cargo add volter
cargo add tokio --features full
```

This adds the following to your `Cargo.toml`:

```toml
[dependencies]
volter = "0.1.0"
tokio = { version = "1", features = ["full"] }
```

You only need `tokio` for the async runtime and `TcpListener`. Volter
re-exports everything else (`http`, `tower`, etc.) through the `volter` crate.

## Verify Installation

Replace `src/main.rs` with:

```rust
use tokio::net::TcpListener;
use volter::{get, serve, Router};

async fn hello() -> &'static str {
    "Hello, World!"
}

#[tokio::main]
async fn main() -> Result<(), volter::BoxError> {
    let app = Router::new().route("/", get(hello));
    let listener = TcpListener::bind("0.0.0.0:3000").await?;
    eprintln!("Listening on http://0.0.0.0:3000");
    serve(listener, app).await
}
```

Run the project:

```bash
cargo run
```

Visit `http://localhost:3000` — you should see `Hello, World!`.

## Optional Features

Volter has two optional feature flags:

| Feature  | Default  | Description                                                                                        |
| -------- | -------- | -------------------------------------------------------------------------------------------------- |
| `macros` | Enabled  | Derive macros (`FromRequestParts`, `FromRequest`) and route attribute macros (`#[get]`, `#[post]`, `#[put]`, `#[patch]`, `#[delete]`, `#[head]`, `#[options]`) |
| `ws`     | Disabled | WebSocket support (`WebSocketUpgrade`, `WebSocket`)                                                |

Enable WebSocket support:

```bash
cargo add volter --features ws
```

To disable macros (not recommended unless you have a dependency conflict):

```bash
cargo add volter --no-default-features
```

## Using the CLI

Volter includes a scaffolding CLI:

```bash
cargo install volter-cli
volter new my-api
cd my-api
cargo run
```

See the [CLI chapter](../cli/cli.md) for details.

[rustup]: https://rustup.rs
