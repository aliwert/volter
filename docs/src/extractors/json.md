# JSON

The `Json<T>` extractor deserializes request bodies as JSON and serializes
response values as JSON.

## Extracting JSON Bodies

```rust
use serde::Deserialize;
use volter::*;

#[derive(Deserialize)]
struct CreateUser {
    name: String,
    age: u8,
}

async fn create_user(Json(payload): Json<CreateUser>) -> String {
    format!("Created {} (age {})", payload.name, payload.age)
}
```

`Json<T>` requires:

- The request's `Content-Type` header to be `application/json` or
  `application/*+json`
- The body to be valid JSON that deserializes to `T`
- `T: DeserializeOwned + Send + 'static`

## Returning JSON Responses

```rust
use serde::Serialize;

#[derive(Serialize)]
struct User {
    id: u64,
    name: String,
}

async fn get_user(Path(id): Path<u64>) -> Json<User> {
    Json(User { id, name: "Alice".into() })
}
```

The response has status `200 OK` and `Content-Type: application/json`.

## Error Handling

`Json<T>` can fail in several ways:

| Condition                         | Rejection                    | HTTP Status |
| --------------------------------- | ---------------------------- | ----------- |
| Missing `Content-Type`            | `MissingJsonContentType`     | 415         |
| Wrong `Content-Type`              | `UnsupportedJsonContentType` | 415         |
| Invalid JSON syntax               | `InvalidJsonBody(err)`       | 400         |
| Body too large / connection error | `BodyReadError(err)`         | 500         |

Handling specific rejections:

```rust
async fn create(Json(payload): Json<CreateUser>) -> Result<String, JsonRejection> {
    // JsonRejection implements IntoResponse, so return it directly
    let user = payload?;
    Ok(format!("Created {}", user.name))
}
```

## JSON Echo Example

```rust
use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize)]
struct Echo {
    message: String,
}

async fn echo(Json(payload): Json<Echo>) -> Json<Echo> {
    Json(payload)
}
```

## Performance Notes

- JSON deserialization uses `serde_json` under the hood
- The body is fully buffered before deserialization (streaming JSON parsing is
  not supported)
- For large payloads, consider using `RequestBodyLimitLayer` to reject
  oversized bodies early
