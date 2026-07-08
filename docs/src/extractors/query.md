# Query

The `Query<T>` extractor parses URL query strings into typed structs using
`serde_urlencoded`.

## Extracting Query Parameters

```rust
use serde::Deserialize;
use volter::*;

#[derive(Deserialize)]
struct SearchParams {
    q: String,
    page: Option<u32>,
    per_page: Option<u32>,
}

async fn search(Query(params): Query<SearchParams>) -> String {
    let page = params.page.unwrap_or(1);
    let per_page = params.per_page.unwrap_or(20);
    format!("Searching for '{}' (page {page}, {per_page} per page)", params.q)
}
```

## Required vs Optional Fields

Fields without `#[serde(default)]` are required. The extractor returns a
`QueryRejection` (400 Bad Request) if they are missing:

```rust
#[derive(Deserialize)]
struct RequiredParams {
    q: String,             // required — 400 if missing
    page: Option<u32>,     // optional — None if missing
}
```

Use `#[serde(default)]` for fields with a fallback value:

```rust
#[derive(Deserialize)]
struct Pagination {
    #[serde(default = "default_page")]
    page: u32,
    #[serde(default)]
    per_page: u32,
}

fn default_page() -> u32 { 1 }
```

## URL Encoding

`Query<T>` handles URL-encoded values automatically:

```rust
// GET /search?q=hello+world
let q = params.q; // "hello world"
```

## Rejection

```rust
pub enum QueryRejection {
    InvalidQueryParams(serde_urlencoded::de::Error),  // → 400 Bad Request
}
```

The rejection implements `IntoResponse`, returning `400 Bad Request` with a
description of the parsing error.

## What Query Cannot Do

- Nested structures (use separate query structs and compose them in your handler)
- Repeated keys as arrays (use `Vec<T>` for repeated query parameters, as
  supported by `serde_urlencoded`)

## Multi-value Parameters

`serde_urlencoded` supports repeated keys mapped to `Vec<T>`:

```rust
#[derive(Deserialize)]
struct FilterParams {
    tags: Vec<String>,
}

// GET /items?tags=rust&tags=web&tags=api
// tags = ["rust", "web", "api"]
```
