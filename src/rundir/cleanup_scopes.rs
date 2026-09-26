//! The thread's registry of run directories inside an explicit cleanup
//! scope, read by `rundir::cleanup` and declared here rather than in
//! `rundir.rs`.
//!
//! v0.1 drives a run synchronously inside an explicit scope. Thread-local
//! registration gives concurrent library/test runs the exact cleanup path
//! for their own reapers instead of conservatively leasing every run active
//! in the process.
//!
//! `rundir.rs` allows `clippy::disallowed_methods` and
//! `clippy::disallowed_types`, and no macro is invoked outside a function
//! body in a file that does not forbid every governed lint: what a macro
//! writes there is an item no census reads, under an allowance that lets it
//! hold an effect. This module forbids all three, so the `thread_local!`
//! that declares the registry writes its items where nothing can be lowered.
#![forbid(
    clippy::disallowed_methods,
    clippy::disallowed_types,
    clippy::disallowed_macros
)]

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::path::PathBuf;

thread_local! {
    pub(super) static ACTIVE: RefCell<BTreeMap<PathBuf, usize>> = const { RefCell::new(BTreeMap::new()) };
}
