<div align="center">
  <img src="../assets/volter.png" alt="Volter logo" width="180"/>
</div>

# Volter Guide

This directory contains the mdBook source for the Volter user guide.

## Building

```bash
cargo install mdbook
mdbook build docs
```

The output is written to `docs/book/`. Open `docs/book/index.html` in a
browser to view the guide.

## Structure

- `src/` — markdown source files
- `book.toml` — mdBook configuration
- `README.md` — this file

The API reference is hosted on [docs.rs](https://docs.rs/volter/latest/volter/).
This guide complements the API reference with narrative, task-oriented content.
