# Extractors

Extractors are the mechanism handlers use to pull data out of incoming requests.
Volter provides two traits:

- **`FromRequestParts`** — extract from request metadata (URI, headers,
  extensions) without consuming the body. Runs first, enables early rejection.
- **`FromRequest`** — extract from the full request, including the body.
  Runs after all `FromRequestParts` extractors.

## How Extraction Works

When you write a handler with multiple parameters, Volter runs them in order:

```rust
async fn handler(
    // 1. FromRequestParts — runs first
    Query(query): Query<PageQuery>,
    // 2. FromRequestParts — runs second
    State(state): State<AppState>,
    // 3. FromRequest — runs last (may consume body)
    Json(body): Json<CreateUser>,
) -> impl IntoResponse {
    // All extractors have already succeeded by this point
}
```

The last parameter may implement either trait. All earlier parameters must
implement `FromRequestParts`. This means metadata extractors run before the
body is consumed, allowing fast rejection of invalid requests.

## Available Extractors

| Extractor                      | Trait              | Source              | Rejection                     |
| ------------------------------ | ------------------ | ------------------- | ----------------------------- |
| [`Query<T>`](query.md)         | `FromRequestParts` | URL query string    | `QueryRejection` → 400        |
| [`Path<T>`](path.md)           | `FromRequestParts` | URL path parameters | `PathRejection` → 400         |
| [`Extension<T>`](extension.md) | `FromRequestParts` | Request extensions  | `ExtensionRejection` → 500    |
| [`State<T>`](state.md)         | `FromRequestParts` | Application state   | Never fails                   |
| [`Json<T>`](json.md)           | `FromRequest`      | JSON body           | `JsonRejection` → 400/415/500 |

## Mapping Rejections to Responses

Every extractor defines its own rejection type. Each rejection implements
`IntoResponse`, so you can compose handlers freely:

```rust
// This handler may fail with 400 (invalid query) or 400 (invalid JSON body).
// Volter short-circuits: if Query fails, Json is never extracted.
async fn create(Query(q): Query<SearchParams>, Json(body): Json<CreateUser>) -> impl IntoResponse {
    // ...
}
```

## The `FromRequestParts` Trait

```rust
pub trait FromRequestParts<S>: Sized {
    type Rejection: IntoResponse;
    type Future: Future<Output = Result<Self, Self::Rejection>> + Send;
    fn from_request_parts(parts: &mut http::request::Parts, state: &S) -> Self::Future;
}
```

## The `FromRequest` Trait

```rust
pub trait FromRequest<S, B = BoxBody>: Sized {
    type Rejection: IntoResponse;
    type Future: Future<Output = Result<Self, Self::Rejection>> + Send;
    fn from_request(req: http::Request<B>, state: &S) -> Self::Future;
}
```

Every `FromRequestParts` implementor also implements `FromRequest` (the body is
split off and discarded).
