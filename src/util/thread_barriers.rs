//! The per-thread durability-barrier count behind
//! `util::barriers_on_this_thread`, declared here rather than in `util.rs`.
//!
//! `util.rs` allows `clippy::disallowed_methods`, and no macro is invoked
//! outside a function body in a file that does not forbid every governed
//! lint: what a macro writes there is an item no census reads, under an
//! allowance that lets it hold an effect. This module forbids all three, so
//! the `thread_local!` that declares the cell writes its items where nothing
//! can be lowered.
#![forbid(
    clippy::disallowed_methods,
    clippy::disallowed_types,
    clippy::disallowed_macros
)]

use super::BarrierCounts;

thread_local! {
    pub(super) static THREAD_BARRIERS: std::cell::Cell<BarrierCounts> =
        const { std::cell::Cell::new(BarrierCounts { file: 0, directory: 0 }) };
}
