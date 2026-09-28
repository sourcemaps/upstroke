//! Extended notes: `docs/internals/runner/host/counters.md`

#![forbid(
    clippy::disallowed_methods,
    clippy::disallowed_types,
    clippy::disallowed_macros
)]

thread_local! {
    pub(super) static RESOLUTIONS: std::cell::Cell<u64> = const { std::cell::Cell::new(0) };
    pub(super) static SEARCHES: std::cell::Cell<u64> = const { std::cell::Cell::new(0) };
    pub(super) static ESTABLISHMENTS: std::cell::Cell<u64> = const { std::cell::Cell::new(0) };
}
