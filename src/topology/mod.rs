//! Extended notes: `docs/internals/topology/mod.md`

#![forbid(
    clippy::disallowed_methods,
    clippy::disallowed_types,
    clippy::disallowed_macros
)]

pub mod census;
pub mod effects;
pub mod events;
pub mod fold;
pub mod leases;
pub mod paths;
pub mod queue;
pub mod registry;
pub mod schema;
