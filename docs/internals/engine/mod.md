# `src/engine/mod.rs`

Extended notes for [`src/engine/mod.rs`](../../../src/engine/mod.rs).

The code is the authority for what it does; this file is the whole of its prose, moved out of
the source verbatim. Each section is headed by the line of code the comment sat above, spelled
as it is in the source, so the heading is the grep string that finds the code.

## `#![deny(`

**This file allows nothing, and that is what it is for.** Everything written
in it -- a private `fn`, an inline `mod x { .. }` at any depth -- is visible to
every module under `engine::topology`, because a topology module descends from
it, and nothing classifies it: `src/engine/mod.rs` is in neither
`CLASSIFIED_MODULES` nor `effects/wrappers.toml`, and the classification domain
reads `pub`, `pub(crate)` and `pub(super)` functions, which would not see a
private item here anyway. So an allow of a governed lint on this file is an
exemption for anything anyone places in it, reachable from the topology.

That is not hypothetical. From #306 (`94c21c45`, `PR7-WRAPPERS-EMPTY-DOMAIN`)
until 2026-09-20 this file carried `#![allow(clippy::disallowed_methods)]`, a
reviewed row of `effects/allowlist.toml`'s legacy section, so that its six
entry points could call `coordinator::run_harness_inner_on` and
`resume::resume_harness_inner_on`, effectful wrappers that change denied by
path. A lint level is scoped by the module tree, so the allow reached every
child declared below that wrote no attribute of its own, and the placement
scan could not see it, because `governed_allows` records what a file writes.
`src/engine/topology.rs` was fenced in the same commit; the third review of
#306 (`PR306-FACADE-ALLOW-ESCAPES-TO-SIBLINGS`) walked through
[`assembly`](assembly.md), an unfenced sibling, and round 3 fenced the five of
them; and the fourth review (`PR306-FACADE-INLINE-ESCAPE`) then walked through
what no fence on a child can reach -- an attribute-free inline module written
in this file, and a function placed directly in it, each calling
`std::fs::write`, each referenced from the production body of `park_question`
in [`topology::integrate`](topology/integrate.md), each with clippy at exit 0
and the suite green.

The row carried its own end state: "its entry points move into the allowed
legacy modules they call, at which point this file calls nothing denied." That
is what happened. `run`, `run_with`, `run_harness` and their two seams are
defined in [`coordinator`](coordinator.md), `resume`, `resume_with`,
`resume_harness` and theirs in [`resume`](resume.md) -- both rows of the legacy
section already, both classified -- and this file re-exports the six public
names, so every public path is what it was. The allow, its row and its entry in
`FROZEN_LEGACY_ALLOWLIST` are gone, and this file carries the fence its
children carry instead: all three governed lints denied at file level, so that
its level is stated here rather than left to the crate root and the command
line. `effects::tests::the_engine_facade_allows_no_governed_lint_and_refuses_both_escape_routes`
refuses an allow written anywhere in this file again, in any form, refuses a
missing deny, and compiles both of the review's routes under this file's own
leading attributes, read from it, where each is a build error;
`effects::tests::every_inline_module_under_the_engine_facade_is_walked_and_answered_for`
derives every inline module under this file at every depth; and the two #306
guards still hold the topology root and every out-of-line child.

## Module

Sequential execution engine (DESIGN.md §14) and the verification ladder it
drives (§11.4, §12, §19).

Pre-flight, run branch, then a scheduler that drains the task graph one
attempt at a time: agent run → engine-captured diff → gates with evidence
axes (§11.1) → read-only review with a structured verdict (§11.2) →
engine-owned commit. A failed attempt does not end the task — it feeds the
failure back to the same rung (resuming the session where the adapter
supports it), then escalates a rung on a fresh session with the accumulated
feedback, and finally asks a human, who is the top rung.

The scheduler's defining property is invariant 6: **a question parks only
the tasks it affects.** Everything else keeps draining, and the run
hard-blocks only when the runnable frontier is empty and everything left is
waiting on an answer. That is the moment — and the only moment — a human is
asked.

Every transition here is an event (invariant 4). The engine never mutates
run state directly: it appends to `events.jsonl` and folds the event back in
through [`crate::events::RunState::apply`], the same function `resume` and
`status` use to rebuild state from the file. A live run and a replay of its
own log therefore cannot disagree — there is no second path for them to
disagree along. `report.json` is written from that state as a projection for
humans; nothing ever reads it back.

## `pub(crate) mod topology;`

The schema-4 run lifecycle.

**`pub(crate)`, and that is what makes `production_effect = "none"` true
rather than an erratum.** It was `pub`, on the argument that the capability
types this module *will* carry are to be guarded by compile-fail fixtures
building out-of-process against the public path — and that behind a private
`mod` those refusals collapse to `E0603`, a compile-fail test passing
because the module is unreachable rather than because the token did its
job. That argument is sound and its subject does not exist: the same doc
admitted "no such fixture exists yet: the visibility is set ahead of the
types".

**What the speculative `pub` actually bought was a defect.** `create_run`,
`Started::into_handle`, `TopologyRun::resumed` and `step` formed a
non-`#[cfg(test)]` writer path reachable by any downstream caller of this
library. Such a caller writes P0–P8 schema-4 state; this build's recovery
refuses every schema-4 log at reader ceiling 3; so `upstroke resume` could
not resume what the released library itself created. Found by the frontier
review of `75da796`, finding 1, and it is the reason the module doc one
level down — "a schema-4 run is reachable only from a `#[cfg(test)]` writer
selector" — was false as written.

**Set back when the fixtures exist**, not before: the visibility follows the
types rather than leading them, which is the opposite of the order tried
here. `the_engine_facade_exposes_exactly_the_items_the_packet_enumerates`
now forbids `pub mod ` in this file, so the next attempt has to be
deliberate.

## `pub use coordinator::{run, run_harness, run_with};`

The v0.1 conductor's three public run entry points, and `pub use resume::{..}`
below is the same for the three that resume.

**Re-exported, not defined here, and the difference is the point.** A `pub use`
is not a call. The entry points reach `coordinator::run_harness_inner_on` and
`resume::resume_harness_inner_on`, which are denied by path, and a module that
calls a denied path needs an allow; a module that re-exports one does not. So
the six live in the two conductor modules they drive, which carry recorded
allows and are classified, and this file -- which a topology module descends
from, see the section above -- calls nothing at all.

**The public paths are unchanged**: `upstroke::engine::run`, `::run_with`,
`::run_harness`, `::resume`, `::resume_with` and `::resume_harness` are the
paths `decisions.phase_zero_modules.modules["src/engine/mod.rs"]` froze, with
the signatures they had; the bodies moved unchanged but for the path to the
conductor, which lost its module prefix.
`engine::tests::the_engine_facade_exposes_exactly_the_items_the_packet_enumerates`
holds that the names re-exported from the two conductor modules are exactly
those six, that each module's own top-level `pub fn`s are exactly the ones
re-exported from it, and that the explicit-`Runner` seams are not among them;
`every_public_write_coordinator_entry_point_establishes_containment` drives all
six through these paths.

**They are denied by path like the conductors they wrap.** Each reaches its
conductor without leaving its file, so `effects/wrappers.toml` classifies it
`effectful` and `clippy.toml` denies it, and a denial binds a DefId rather
than a spelling: a call to `engine::run` is refused as
`upstroke::engine::coordinator::run`. The callers are `src/main.rs`,
`engine::tests` and `runner::container::resolve::tests`, each under its own
recorded allow of `clippy::disallowed_methods`. A topology module calling the
legacy conductor through the facade is a build error now; until 2026-09-20 it
was not.

## `pub use crate::agent::{AdapterSource, BuiltinAdapters};`

Re-exported so `engine::AdapterSource` still resolves for callers that
reasonably think of it as the engine's seam.
