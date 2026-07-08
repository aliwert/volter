# Derive Macros

Volter provides two derive macros — `FromRequestParts` and `FromRequest` — that
automatically implement the corresponding extractor traits for your types.

## FromRequestParts

Derive to parse query parameters into a struct:

```rust
use serde::Deserialize;
use volter::*;

#[derive(Deserialize, FromRequestParts)]
struct SearchParams {
    q: String,
    page: Option<u32>,
}
```

This generates a `FromRequestParts` implementation that deserializes the URL
query string via `serde_urlencoded::from_str` — the same parsing used by
`Query<T>`.

### Usage in Handlers

```rust
async fn search(params: SearchParams) -> String {
    format!("Searching for '{}'", params.q)
}
```

Without the derive, you would write:

```rust
async fn search(Query(params): Query<SearchParams>) -> String { ... }
```

### Requirements

- The type must implement `serde::DeserializeOwned` (typically via
  `#[derive(serde::Deserialize)]`)
- The type must be `Send + 'static`

## FromRequest

Derive to parse JSON request bodies into a struct:

```rust
use serde::Deserialize;
use volter::*;

#[derive(Deserialize, FromRequest)]
struct CreateUser {
    name: String,
    email: String,
}
```

This generates a `FromRequest` implementation that clones the state, then
delegates to `Json<Self>::from_request`.

### Usage in Handlers

```rust
async fn create_user(user: CreateUser) -> Result<String, JsonRejection> {
    Ok(format!("Created {}", user.name))
}
```

### Requirements

- The type must implement `serde::DeserializeOwned`
- The type must be `Send + 'static`
- The state type `S` must implement `Clone + Send + 'static` (the generated
  code clones the state to pass across the `.await` boundary)

## Rejection Types

| Derive             | Rejection        | HTTP Status     |
| ------------------ | ---------------- | --------------- |
| `FromRequestParts` | `QueryRejection` | 400 Bad Request |
| `FromRequest`      | `JsonRejection`  | 400 / 415 / 500 |

Both rejection types implement `IntoResponse`, so you can return them directly
from handlers.

## Limitations

- Structs only (enums are not supported)
- No field-level attributes or custom validation
- No support for `#[from_request(via = ...)]` or other configuration
  attributes
