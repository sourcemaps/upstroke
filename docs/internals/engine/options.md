# `src/engine/options.rs`

Extended notes for [`src/engine/options.rs`](../../../src/engine/options.rs).

The code is the authority for what it does; this file is the whole of its prose, moved out of
the source verbatim. Each section is headed by the line of code the comment sat above, spelled
as it is in the source, so the heading is the grep string that finds the code.

## `#![deny(`

**Fenced against an allow above it, and behaviour-preserving.** From #306
until 2026-09-20 `src/engine/mod.rs` carried
`#![allow(clippy::disallowed_methods)]` for the two conductor entry points its
facade called, and a lint level inherits down the module tree: this file wrote
no attribute, so it inherited that allow, and the placement scan -- which
records what a file writes -- recorded nothing.
The third review of #306 (`PR306-FACADE-ALLOW-ESCAPES-TO-SIBLINGS`) proved
the consequence in `assembly.rs`, a sibling under the same parent: a `pub(super) fn` calling `std::fs::write`,
referenced from a production body under `engine::topology`, passed clippy and
the whole suite. This attribute restores the level the file had before the
facade's allow existed -- the three governed lints were already errors under
`-D warnings` -- and changes nothing else: no statement here reaches a denied
primitive, so the deny reddens nothing today and only matters against an
inherited allow. It is the form `src/engine/topology.rs` wrote first, needs no
row in `effects/allowlist.toml` (the scan records `allow` and `expect`, never
`deny`), and
`effects::tests::every_child_the_engine_facade_declares_re_denies_or_records_what_it_inherits`
refuses its absence.

**The facade's allow is gone, and the fence stays.** On 2026-09-20 the
facade's entry points moved into `coordinator` and `resume`, the facade stopped
calling anything denied, and its allow was removed with its row
(`PR306-FACADE-INLINE-ESCAPE`); it denies the same three lints itself now. So
nothing above this file allows anything today. The guard holds this attribute
regardless, for every module the facade declares: a module that leans on its
parent's level is exempt the day that level changes, which is exactly what #306
did to this one.

## `pub const DEFAULT_ATTEMPT_TIMEOUT: Duration = Duration::from_secs(30 * 60);`

§14: per-attempt wall clock, default 30 minutes.

## `pub const DEFAULT_MAX_DEFERS: u32 = 3;`

How many rate limits (or reviewer outages) one task rides out before the
pool counts as down and a human is asked instead.

Step 10 gave the capacity engine reset times — [`crate::capacity`] carries
them on an estimate, and `pool_exhausted` records one whenever a signal
includes it — so the obvious question is why this bound still exists. Two
reasons, both current: neither CLI actually reports a machine-readable reset
time today, so the field is almost always `None`; and §13 ships the capacity
engine read-only in v0.1, so nothing routes on a reset even when there is
one. Waiting for a reset instead of counting deferrals is capacity-*driven*
behaviour, and it arrives with the rest of it in v0.2. Until then this is
what keeps an exhausted pool from deferring forever.

## `pub struct RunOptions` › `pub repo_root: PathBuf,`

Repo the run executes in (agents run at its root — §14).

## `pub struct RunOptions` › `pub interaction: Option<InteractionMode>,`

CLI override for `[interaction] mode`; `None` takes the config's.

## `pub struct RunOptions` › `pub defer_backoff: Duration,`

First wait after a rate-limited attempt, doubling per consecutive
round of nothing-but-deferred-work.

## `pub struct RunOptions` › `pub private_root: Option<PathBuf>,`

Where the agent-authored half of the run directory goes (§15 split).
`None` takes `~/.upstroke`; tests point it at a scratch directory so they
never touch the real one.

## `pub struct RunOptions` › `pub wait_on_block: Option<Duration>,`

Override `[interaction] wait_on_block_secs` — how long a detached
interactive run waits at a hard block. `None` takes the config's.

## `pub struct RunOptions` › `pub budget_usd: Option<f64>,`

`--budget <usd>`, overriding `[budgets] run_usd` (§17).

## `pub struct RunOptions` › `pub(super) after_candidate_capture: Option<AfterCandidateCapture>,`

Deterministic test seam for changing the mutable index immediately
after the engine has frozen its candidate object identities.

## `pub struct RunOptions` › `pub(super) log_hooks: Option<fn() -> Box<dyn crate::events::log::EventHooks>>,`

The observer the live run's **legacy** append funnel is driven through.

`None` is production and means [`crate::events::log::NoEventHooks`],
which is what `EventLog::append` uses anyway. It is here so a fixture can
make a **live `Run`**'s append fail (`PR5-CONF-010`, `PR5-CONF-011`);
nothing else in the tree can, and both surviving mutations were on the
path that failure takes.

## `impl RunOptions` › `pub fn new(plan_path: PathBuf, repo_root: PathBuf) -> Self {`

Everything but the paths at its documented default.

## `pub struct Harness<'a> {`

Injectable collaborators. `None` means "use the real one", chosen from
config where the config has a say.

## `pub struct Harness<'a>` › `pub answers: Option<&'a dyn AnswerSource>,`

`None` derives the channel from `[interaction] mode` (§12).

## `pub struct Harness<'a>` › `pub sleeper: Option<&'a dyn Sleeper>,`

`None` really sleeps.

## `pub struct ResumeOptions {`

What to continue, and what may be overridden while continuing it.

## `pub struct ResumeOptions` › `pub run_id: String,`

Run id, or any unambiguous prefix of one.

## `pub struct ResumeOptions` › `pub config_path: Option<PathBuf>,`

`None` takes the config the run recorded.

## `pub struct ResumeOptions` › `pub budget_usd: Option<f64>,`

`--budget <usd>` (§17), overriding `[budgets] run_usd` for this resume.

Budgets are **re-derived from today's config and flags**, unlike the
three things a resume takes from the run's own record: the plan (frozen,
and refused on a hash mismatch), the resolved chains (refused, because a
recorded rung is an index into one), and the gates and reviewers (taken
and used, because they are what "this code was verified" means). Those
protect a run's *identity*. A budget is not identity — it is an
operator's ceiling on their own spending, and re-reading it is precisely
what makes a budget stop recoverable in one command instead of a dead
run and a new branch.
