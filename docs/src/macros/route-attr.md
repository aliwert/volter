# Route Attribute Macros

The `#[get]` and `#[post]` attribute macros provide a shorthand for defining
routes directly on your handler functions.

## Basic Usage

```rust
use volter::*;

#[get("/")]
async fn index() -> &'static str {
    "Hello, World!"
}

#[post("/users")]
async fn create_user(Json(payload): Json<CreateUser>) -> String {
    format!("Created user")
}

let app: Router = Router::new()
    .route_attr(INDEX_ROUTE, index)
    .route_attr(CREATE_USER_ROUTE, create_user);
```

Each attribute macro:

1. Preserves the original function (visibility, docs, and signature are
   unchanged)
2. Generates a `const` with the path, e.g. `INDEX_ROUTE`, of type `RouteAttr`
3. The const is passed to `Router::route_attr()` to register the route

## Generated Names

The const is named by uppercasing the function name and appending `_ROUTE`:

| Function              | Generated const        |
| --------------------- | ---------------------- |
| `fn index()`          | `INDEX_ROUTE`          |
| `fn create_user()`    | `CREATE_USER_ROUTE`    |
| `fn get_user_by_id()` | `GET_USER_BY_ID_ROUTE` |

## Route Parameters

Attribute macros work with path parameters too:

```rust
#[get("/users/:id")]
async fn get_user(Path(id): Path<u64>) -> String {
    format!("User {id}")
}
```

## State

```rust
#[derive(Clone)]
struct AppState { db_url: String }

#[get("/dashboard")]
async fn dashboard(State(state): State<AppState>) -> String {
    format!("DB: {}", state.db_url)
}
```

## Type Safety

The generated `RouteAttr` stores only the path and HTTP method — it does not
store the handler. The handler type is inferred when you call
`route_attr(ATTR, handler)`, so you get full type checking at registration
time rather than at macro expansion time.

## Compared to Inline Routing

```rust
// Attribute macro style:
#[get("/")]
async fn home() -> &'static str { "home" }

Router::new().route_attr(HOME_ROUTE, home)

// Inline style (equivalent):
async fn home() -> &'static str { "home" }

Router::new().route("/", get(home))
```

The attribute macro style keeps the route pattern next to the function, which
can be easier to maintain as the number of handlers grows.

## Limitations

- Only `GET` and `POST` are supported currently
- Arguments must be a single string literal (e.g. `#[get("/path")]`)
- The function must be `async`
- Works best with `Router::new()` — stateful routers require the state type
  to be inferred from `route_attr` calls
