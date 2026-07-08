//! Type-erased, cloneable service wrapper for route storage.
//!
//! Replaced by [`tower::util::BoxCloneService`] directly in [`MethodRouter`].
//! See `method_router.rs` for the actual implementation.
//!
//! This file is kept intentionally empty to document the decision:
//!
//! - `BoxCloneService<Request, Response, Infallible>` replaces the custom
//! `BoxedRouteService` + `ErasedService` + `Arc<Mutex<...>>` approach.
//! - `MapFuture` in tower 0.5 derives `Clone`, so the boxing pipeline
//! (`inner.map_future(|f| Box::pin(f))`) preserves cloneability without
//! any `unsafe`, custom vtables, or mutex locks.
//! - The lock-free design means the router hot path never acquires a mutex,
//! avoiding contention and poison-recovery complexity.
