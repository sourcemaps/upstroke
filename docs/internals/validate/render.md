# `src/validate/render.rs`

Extended notes for [`src/validate/render.rs`](../../../src/validate/render.rs).

The code is the authority for what it does; this file is the whole of its prose, moved out of
the source verbatim. Each section is headed by the line of code the comment sat above, spelled
as it is in the source, so the heading is the grep string that finds the code.

## Module

What the preview looks like: the echo lines `run` fills a [`Report`] with,
the per-task row, and the table [`Report::render`] delegates to.

Split out of `super` unchanged. [`Report::render`] stays an inherent method
on the parent — it is the rendering surface every caller and the wrapper
census know — and its body is [`report`] here, which is the only thing that
moved.

**The denial is restored rather than inherited.** `super` carries
`#![allow(clippy::disallowed_methods)]` because `write_normalized_json`
writes the normalized plan with `fs::write`, and a module-level allow reaches
every child of the module it sits in. Nothing here writes anything — every
function returns a `String` and the sink is always one of its own — so that
allowance has no business extending here, and this line is what stops it.
That is also what keeps this file out of `effects/allowlist.toml`: an
allowance is what that file records, and this module takes none.

## `#![forbid(`

`disallowed_methods` was forbidden here already, restoring the level `super` allows. `disallowed_types` and `disallowed_macros` joined the fence in #318's third round: `super` states nothing for them, so this file
took it from `-D warnings` alone, a level an inner `allow` the placement scan
does not read lowers (`GUARD-DECISION-SILENT-PRODUCTION-FILES-OUTSIDE-THE-ROLL-CALL`);
`every_unclassified_production_file_states_each_governed_lint_or_inherits_its_forbid`
names the pair the day it is dropped. `forbid` compiles because nothing here
allows a governed lint.

## `pub(super) fn review_echo(plan: &ReviewPlan, cross_family: bool) -> String {`

Who judges the work (§11.2–§11.3), for the preview.

Resolved against the adapters this build ships, not against binaries found
on PATH: `validate` and `--dry-run` execute nothing (§18), so they cannot
probe. Pre-flight is where a named reviewer has to prove it can actually
run — and where a missing one either warns or refuses. The line says so,
because a preview that reads as a promise is worse than one that reads as a
plan.

`cross_family` is the config's `review_cross_family`, passed in because the
plan alone cannot say why it has no alternative: nothing of another family
at the review tier in this build, and
`[routing] review = { cross_family = false }`, both leave `alternative`
`None`. Only the first is "no cross-family reviewer exists in this build".
Saying it of the second is false — the same build resolves one the moment
the switch is unset — so with the switch off the line names the
configuration that turned the reviewer off instead.

The switch turns off the *alternative* only. A `[[routing.overrides]]`
entry's `second_opinion` is an explicit demand the switch leaves alone
(`cross_family_off_leaves_an_explicit_second_opinion_alone`): those passes
stay in the plan, the task rows show them, and creation still requires
their agent. So when the switch is off and the plan demands at least one
second opinion, "cross-family review is turned off" alone would contradict
the rows beneath it; the line says instead that the switch turned off the
alternative, and that the second opinions still run on the agents it names
(each distinct binding once) and still need them installed. With the switch
off and no second opinion demanded, the line is the plain sentence above,
unchanged.

## `pub(super) fn capacity_echo(`

§13's capacity block, for a command that executes nothing.

`validate` and `--dry-run` **do not probe** (§18): every figure here comes
from files — the pools file, and the latest run's event log in this
repository. That is a real distinction rather than a technicality, and the
block says which side of it each line is on, because `upstroke capacity` shows
strictly more by being allowed to spawn the vendors' CLIs.

The same reason the review line says "if installed": a preview that reads as
a promise is worse than one that reads as a plan.

## `if let Some(binding) = second_opinion {`

§11.3: a second reviewer is a per-task routing decision like any other,
so it belongs in the column that shows what this task's paths bought it.

## `pub(super) fn report(report: &Report) -> String {`

The body of [`Report::render`].
