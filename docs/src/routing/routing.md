# Routing

Routing maps incoming HTTP requests to handler functions. Volter's `Router` is a
[`tower::Service`] — it composes with any tower middleware.

## Basic Routing

Create a router and attach handlers with `.route()`:

```rust
use volter::{get, post, Router};

let app = Router::new()
    .route("/", get(index))
    .route("/users", get(list_users))
    .route("/users", post(create_user));
```

The first argument is the path, the second is a `MethodRouter` created by the
`get()` or `post()` free function. Other HTTP methods produce a `405 Method Not
Allowed` response.

## Route Patterns

### Static Paths

Exact path matching — the fastest option:

```rust
.route("/about", get(about_page))
.route("/contact", get(contact_page))
```

### Parameterized Paths

Named parameters start with `:` and are captured by the `Path` extractor:

```rust
.route("/users/:id", get(user_by_id))
.route("/posts/:post_id/comments/:comment_id", get(comment_by_id))
```

The segment count must match exactly. `/users/42` matches `/users/:id`;
`/users/42/profile` does not.

## Method Routing

The `get()` and `post()` functions each return a `MethodRouter` that matches a
single HTTP method:

```rust
use volter::{get, post, Router};

async fn list() -> &'static str { "list" }
async fn create() -> &'static str { "create" }

let app = Router::new()
    .route("/items", get(list))
    .route("/items", post(create));
```

A GET request to `/items` calls `list`; a POST request calls `create`;
any other method returns `405`.

## Merging Routers

Combine independent routers with `.merge()`:

```rust
let users_routes = Router::new()
    .route("/users", get(list_users));

let posts_routes = Router::new()
    .route("/posts", get(list_posts));

let app = users_routes.merge(posts_routes);
```

When two routers define the same path and method, the last merged wins.

## Nesting Routers

Mount a router under a path prefix with `.nest()`:

```rust
let api = Router::new()
    .route("/users", get(list_users))
    .route("/posts", get(list_posts));

let app = Router::new()
    .nest("/api/v1", api);
```

This serves `list_users` at `GET /api/v1/users` and `list_posts` at
`GET /api/v1/posts`. The prefix is matched segment-by-segment: `/api/v1`
matches `/api/v1/users` but not `/api/v2`.

Each nested router preserves its own state and middleware.

## Route Attribute Macros

As an alternative to the free-function API, you can annotate handlers with
`#[get("/")]` and `#[post("/")]`:

```rust
use volter::*;

#[get("/")]
async fn index() -> &'static str {
    "Hello!"
}

let app = Router::new().route_attr(INDEX_ROUTE, index);
```

See the [Route Attribute Macros](../macros/route-attr.md) chapter for details.

## Route Matching Order

1. **Post-layer static routes** — O(1) hashmap lookup
2. **Post-layer parameterized routes** — linear scan
3. **Post-layer nested routers** — prefix strip, then delegate
4. **Pre-layer (layered) service** — routes wrapped by `.layer()`
5. **404 Not Found** — nothing matched

[`tower::Service`]: https://docs.rs/tower/latest/tower/trait.Service.html
