# Path

The `Path<T>` extractor captures named parameters from route patterns.

## Defining Parameterized Routes

Use `:name` syntax in the route pattern:

```rust
let app = Router::new()
    .route("/users/:id", get(user_by_id))
    .route("/posts/:post_id/comments/:comment_id", get(comment));
```

## Extracting a Single Parameter

For routes with one parameter, wrap the matching Rust type:

```rust
async fn user_by_id(Path(id): Path<u64>) -> String {
    format!("User {id}")
}
```

The router captures the `:id` segment as a string, and `Path<u64>` tries to
parse it as a `u64`. If parsing fails, a `PathRejection` (400 Bad Request) is
returned.

## Extracting Multiple Parameters

For routes with several parameters, use a struct:

```rust
use serde::Deserialize;

#[derive(Deserialize)]
struct CommentParams {
    post_id: u64,
    comment_id: u64,
}

async fn comment(Path(params): Path<CommentParams>) -> String {
    format!("Post {} / Comment {}", params.post_id, params.comment_id)
}
```

The field names in the struct must match the route parameter names (`post_id`,
`comment_id`).

## Supported Types

Any type that implements `DeserializeOwned` can be used:

```rust
async fn by_name(Path(name): Path<String>) -> String {
    format!("Hello {name}")
}
```

- `String` — captures the raw segment
- Numeric types (`u64`, `i32`, `f64`, etc.) — parses the segment
- UUID, ULID — via Serde's `deserialize_from_str`
- Custom enums — via Serde's deserialization

## Rejection

```rust
pub enum PathRejection {
    MissingPathParams,              // → 400 Bad Request
    InvalidPathParams(serde_json::Error),  // → 400 Bad Request
}
```

- `MissingPathParams` — the route pattern has parameters but the request
  extension was not set (internal error, should not happen in normal use)
- `InvalidPathParams` — parsing failed (e.g., `:id` = `"abc"` for `Path<u64>`)

## Important Notes

- Route parameters are **not** query parameters. Use `Query<T>` for `?key=value`
- The parameter name in the route (`:user_id`) must match the struct field name
  (`user_id`)
- Segment count must match exactly: `/users/:id` does **not** match
  `/users/42/profile`
