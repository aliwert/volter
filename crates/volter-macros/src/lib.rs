//! Derive and attribute macros for volter.
//!
//! See `ARCHITECTURE.md` → "Macro-generated ergonomics" and `RULES.md` §7
//! before adding a macro here: generated code must stay debuggable (prefer
//! named helper functions over one giant inlined block) and must follow
//! every rule that hand-written code follows — no hidden panics, no hidden
//! blocking calls, no hidden `unsafe`.
//!
//! # Derive macros
//!
//! - `#[derive(FromRequestParts)]` — generate a
//!   [`FromRequestParts`] implementation that
//!   deserializes the struct from URL query parameters (delegates to
//!   [`Query`]).
//! - `#[derive(FromRequest)]` — generate a
//!   [`FromRequest`] implementation that
//!   deserializes the struct from a JSON request body (delegates to
//!   [`Json`]).
//!
//! Both derives require the struct to implement
//! [`serde::de::DeserializeOwned`] (typically via `#[derive(serde::Deserialize)]`).
//!
//! # Attribute macros
//!
//! - `#[get("/path")]` — register a handler for GET requests.  Expands the
//!   function and generates a `{FN_NAME_UPPER}_ROUTE` const that can be
//!   passed to [`Router::route_attr`].
//! - `#[post("/path")]` — same for POST requests.
//! - `#[put("/path")]` — same for PUT requests.
//! - `#[patch("/path")]` — same for PATCH requests.
//! - `#[delete("/path")]` — same for DELETE requests.
//! - `#[head("/path")]` — same for HEAD requests.
//! - `#[options("/path")]` — same for OPTIONS requests.
//!
//! volter's core routing API must always work without any macro from this
//! crate (`Router::new().route("/x", get(handler))`) — macros are additive
//! sugar, never a requirement.

#![deny(missing_docs)]
#![deny(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use proc_macro::{Delimiter, TokenStream, TokenTree};

use syn::{parse_macro_input, Data, DeriveInput, GenericParam, ItemFn, TypeParam};

/// Extract the path string from attribute arguments.
///
/// Some Rust versions pass `args` as a bare string literal (`"/path"`),
/// others wrap it in a parenthesised group (`("/path")`).  Handle both.
fn parse_attr_path(args: TokenStream) -> syn::Result<String> {
    let vec: Vec<TokenTree> = args.into_iter().collect();

    // If the first token is a parenthesised group, unwrap it.
    let inner = if vec.len() == 1 {
        match vec.into_iter().next() {
            Some(TokenTree::Group(g)) if g.delimiter() == Delimiter::Parenthesis => g.stream(),
            Some(other) => other.into(),
            None => {
                return Err(syn::Error::new(
                    proc_macro2::Span::call_site(),
                    "expected a path string, e.g. \"/\"",
                ));
            }
        }
    } else if vec.is_empty() {
        return Err(syn::Error::new(
            proc_macro2::Span::call_site(),
            "expected a path string, e.g. \"/\"",
        ));
    } else {
        // Multiple tokens without a wrapping group — not valid.
        return Err(syn::Error::new(
            proc_macro2::Span::call_site(),
            "unexpected tokens in attribute arguments",
        ));
    };

    syn::parse::<syn::LitStr>(inner)
        .map(|lit| lit.value())
        .map_err(|_| {
            syn::Error::new(
                proc_macro2::Span::call_site(),
                "expected a string literal path, e.g. \"/\"",
            )
        })
}

/// Create the full generics including a phantom `__S` state parameter and
/// trait bounds.
///
/// `require_static` controls `+'static` on the `Self` bound.
/// `require_clone` controls `Clone` on the `__S` state parameter (needed
/// when the generated impl must pass `&__S` across an `.await`).
fn full_generics(input: &DeriveInput, require_static: bool, require_clone: bool) -> syn::Generics {
    let mut generics = input.generics.clone();
    generics.params.push(GenericParam::Type(TypeParam {
        attrs: vec![],
        ident: syn::Ident::new("__S", proc_macro2::Span::call_site()),
        colon_token: None,
        bounds: Default::default(),
        eq_token: None,
        default: None,
    }));
    let wc = generics
        .where_clause
        .get_or_insert_with(|| syn::WhereClause {
            where_token: Default::default(),
            predicates: Default::default(),
        });
    if require_static {
        wc.predicates
            .push(syn::parse_quote!(Self: ::serde::de::DeserializeOwned + Send + 'static));
    } else {
        wc.predicates
            .push(syn::parse_quote!(Self: ::serde::de::DeserializeOwned + Send));
    }
    if require_clone {
        wc.predicates
            .push(syn::parse_quote!(__S: Clone + Send + 'static));
    }
    generics
}

// ---------------------------------------------------------------------------
// FromRequestParts derive
// ---------------------------------------------------------------------------

/// Derive macro for `volter::FromRequestParts`.
///
/// Generates an implementation of `FromRequestParts` that deserializes the
/// struct from URL query parameters using `serde_urlencoded` (the same
/// library that [`Query<T>`](`volter::Query`) uses internally).
///
/// The struct must implement [`serde::de::DeserializeOwned`].
///
/// # Example
///
/// ```ignore
/// use volter::*;
///
/// #[derive(Deserialize, FromRequestParts)]
/// struct Pagination {
///     page: u32,
///     per_page: u32,
/// }
///
/// async fn list(page: Pagination) -> String {
///     format!("page={}, per_page={}", page.page, page.per_page)
/// }
/// ```
#[proc_macro_derive(FromRequestParts)]
pub fn derive_from_request_parts(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = &input.ident;

    if !matches!(&input.data, Data::Struct(_)) {
        return syn::Error::new_spanned(name, "FromRequestParts can only be derived on structs")
            .to_compile_error()
            .into();
    }

    let full = full_generics(&input, false, false);
    let (impl_generics, _, where_clause) = full.split_for_impl();
    let (_, ty_generics, _) = input.generics.split_for_impl();

    let expanded = quote::quote! {
        const _: () = {
            impl #impl_generics ::volter::FromRequestParts<__S> for #name #ty_generics
            #where_clause
            {
                type Rejection = ::volter::QueryRejection;
                type Future = ::std::future::Ready<Result<Self, Self::Rejection>>;

                fn from_request_parts(
                    parts: &mut ::volter::http::request::Parts,
                    _state: &__S,
                ) -> Self::Future {
                    // Equivalent to `Query::from_request_parts` — parse the
                    // query string via `serde_urlencoded` and deserialise
                    // directly into `Self`, skipping the `Query<Self>(…)`
                    // intermediate wrapper that `into_inner()` would need.
                    let query_string = parts.uri.query().unwrap_or("");
                    let result = ::volter::serde_urlencoded::from_str::<Self>(query_string)
                        .map_err(::volter::QueryRejection::from);
                    ::std::future::ready(result)
                }
            }
        };
    };

    TokenStream::from(expanded)
}

// ---------------------------------------------------------------------------
// FromRequest derive
// ---------------------------------------------------------------------------

/// Derive macro for `volter::FromRequest`.
///
/// Generates an implementation of `FromRequest` that deserializes the
/// struct from a JSON request body (delegates to
/// [`Json<T>`](`volter::Json`)).
///
/// The struct must implement [`serde::de::DeserializeOwned`].
///
/// # Example
///
/// ```ignore
/// use volter::*;
///
/// #[derive(Deserialize, FromRequest)]
/// struct CreateUser {
///     name: String,
///     age: u32,
/// }
///
/// async fn create(user: CreateUser) -> String {
///     format!("created {} (age {})", user.name, user.age)
/// }
/// ```
#[proc_macro_derive(FromRequest)]
pub fn derive_from_request(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = &input.ident;

    if !matches!(&input.data, Data::Struct(_)) {
        return syn::Error::new_spanned(name, "FromRequest can only be derived on structs")
            .to_compile_error()
            .into();
    }

    let full = full_generics(&input, true, true);
    let (impl_generics, _, where_clause) = full.split_for_impl();
    let (_, ty_generics, _) = input.generics.split_for_impl();

    let expanded = quote::quote! {
        const _: () = {
            impl #impl_generics ::volter::FromRequest<__S, ::volter::BoxBody> for #name #ty_generics
            #where_clause
            {
                type Rejection = ::volter::JsonRejection;
                type Future = ::std::pin::Pin<Box<dyn ::std::future::Future<Output = Result<Self, Self::Rejection>> + Send>>;

                fn from_request(
                    req: ::volter::Request,
                    _state: &__S,
                ) -> Self::Future {
                    let state = _state.clone();
                    ::std::boxed::Box::pin(async move {
                        let json = <::volter::Json<Self> as ::volter::FromRequest<__S, ::volter::BoxBody>>::from_request(req, &state).await?;
                        Ok(json.0)
                    })
                }
            }
        };
    };

    TokenStream::from(expanded)
}

// ---------------------------------------------------------------------------
// Route attribute macros
// ---------------------------------------------------------------------------

/// Shared implementation for `#[get]` and `#[post]` attribute macros.
fn route_attr_impl(args: TokenStream, input: TokenStream, method: &str) -> TokenStream {
    let path_str = match parse_attr_path(args) {
        Ok(s) => s,
        Err(err) => return err.to_compile_error().into(),
    };
    let path_lit = syn::LitStr::new(&path_str, proc_macro2::Span::call_site());
    let func = parse_macro_input!(input as ItemFn);

    if func.sig.asyncness.is_none() {
        return syn::Error::new_spanned(
            &func.sig,
            format!("`#[{method}(\"...\")]` requires an async function"),
        )
        .to_compile_error()
        .into();
    }

    let fn_name = &func.sig.ident;
    let const_name_str = fn_name.to_string().to_uppercase() + "_ROUTE";
    let const_ident = syn::Ident::new(&const_name_str, proc_macro2::Span::call_site());
    let method_ident = syn::Ident::new(method, proc_macro2::Span::call_site());

    let expanded = quote::quote! {
        #func

        const #const_ident: ::volter::RouteAttr = ::volter::RouteAttr::#method_ident(#path_lit);
    };

    TokenStream::from(expanded)
}

/// Register a handler function for GET requests.
///
/// Expands the annotated function into a handler function and a `const`
/// [`RouteAttr`](https://docs.rs/volter-router/latest/volter_router/struct.RouteAttr.html)
/// that can be passed to [`Router::route_attr`].
///
/// # Example
///
/// ```ignore
/// use volter::*;
///
/// #[get("/")]
/// async fn index() -> &'static str {
///     "Hello, World!"
/// }
///
/// let app: Router = Router::new().route_attr(INDEX_ROUTE, index);
/// ```
#[proc_macro_attribute]
pub fn get(args: TokenStream, input: TokenStream) -> TokenStream {
    route_attr_impl(args, input, "get")
}

/// Register a handler function for POST requests.
///
/// See [`get`](macro@get) for usage.
#[proc_macro_attribute]
pub fn post(args: TokenStream, input: TokenStream) -> TokenStream {
    route_attr_impl(args, input, "post")
}

/// Register a handler function for PUT requests.
///
/// See [`get`](macro@get) for usage.
#[proc_macro_attribute]
pub fn put(args: TokenStream, input: TokenStream) -> TokenStream {
    route_attr_impl(args, input, "put")
}

/// Register a handler function for PATCH requests.
///
/// See [`get`](macro@get) for usage.
#[proc_macro_attribute]
pub fn patch(args: TokenStream, input: TokenStream) -> TokenStream {
    route_attr_impl(args, input, "patch")
}

/// Register a handler function for DELETE requests.
///
/// See [`get`](macro@get) for usage.
#[proc_macro_attribute]
pub fn delete(args: TokenStream, input: TokenStream) -> TokenStream {
    route_attr_impl(args, input, "delete")
}

/// Register a handler function for HEAD requests.
///
/// See [`get`](macro@get) for usage.
#[proc_macro_attribute]
pub fn head(args: TokenStream, input: TokenStream) -> TokenStream {
    route_attr_impl(args, input, "head")
}

/// Register a handler function for OPTIONS requests.
///
/// See [`get`](macro@get) for usage.
#[proc_macro_attribute]
pub fn options(args: TokenStream, input: TokenStream) -> TokenStream {
    route_attr_impl(args, input, "options")
}
