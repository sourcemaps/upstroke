//! Extended notes: `docs/internals/engine/mod.md`

#![deny(
    clippy::disallowed_methods,
    clippy::disallowed_types,
    clippy::disallowed_macros
)]

mod assembly;
mod attempt;
mod classify;
mod coordinator;
mod options;
mod preflight;
mod report;
mod resume;
pub(crate) mod topology;

pub use coordinator::{run, run_harness, run_with};
pub use options::{
    DEFAULT_ATTEMPT_TIMEOUT, DEFAULT_MAX_DEFERS, Harness, ResumeOptions, RunOptions,
};
pub use report::{PoolDrainRow, RunOutcome, RunReport, TaskReport, TaskRunStatus, topo_order};
pub use resume::{resume, resume_harness, resume_with};

pub use crate::agent::{AdapterSource, BuiltinAdapters};
pub use crate::events::{AttemptRecord, FailureRecord};
pub use crate::ladder::{AttemptFailure, FailureKind, FailureOrigin};

#[cfg(test)]
mod tests;
