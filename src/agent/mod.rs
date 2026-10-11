//! Extended notes: `docs/internals/agent/mod.md`

#![deny(
    clippy::disallowed_methods,
    clippy::disallowed_types,
    clippy::disallowed_macros
)]

pub mod adapter;
pub mod bin;
pub mod claude;
pub mod codex;
pub mod copilot;
pub mod proc;

pub use adapter::*;
pub(crate) use adapter::{advertises_flag, missing_effort_levels};
pub use proc::ProcessOutput;
