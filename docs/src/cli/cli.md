# CLI

Volter includes a CLI tool for scaffolding new projects.

## Installation

```bash
cargo install volter-cli
```

## Commands

### `volter new`

Create a new Volter project:

```bash
volter new my-app
cd my-app
cargo run
```

This creates a minimal project structure:

```
my-app/
├── Cargo.toml
└── src/
    └── main.rs
```

With a functional "Hello, World!" server already in place.

### `volter run` (Planned)

Starts the project with optional file-watching for hot reload. Not yet
implemented.

## Note

The CLI is optional. You can build Volter applications with just `cargo init`
and adding `volter` to your `Cargo.toml` manually.
