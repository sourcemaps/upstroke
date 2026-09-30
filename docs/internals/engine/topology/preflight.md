# `src/engine/topology/preflight.rs`

Extended notes for [`src/engine/topology/preflight.rs`](../../../../src/engine/topology/preflight.rs).

The code is the authority for what it does; this file is the whole of its prose, moved out of
the source verbatim. Each section is headed by the line of code the comment sat above, spelled
as it is in the source, so the heading is the grep string that finds the code.

## Module

The `RunnerPreflight`: the one observation of shell and CLI availability
inside the run's boundary.

[`crate::runner::container::resolve::RunnerPreflight`] is a **seam with no
production implementation**, and its own module says why: the probes are
`crate::runner::host::run_shell_probe` over the run's Runner, and building
that runner "needs a run identity, a private root and a slot pair that
`TopologyRun` owns at PR7". This is that implementation.

INV-23, which is the whole specification of this file:

> the `RunnerPreflight` — one non-slotted shell probe (the recorded shell
> executing `exit 0`) and one slotted probe per recorded agent, each a
> registered invocation through the run's Runner — executes at P4 and at
> every resume's recovery step (c) and is the only observation of shell and
> CLI availability inside the boundary.

### The asymmetry is the point

The shell probe is **non-slotted** and the agent probes are **slotted**.
`permits.agent_pool_slots` excludes two invocations by name — "gate
invocations and the shell probe acquire no slot" — and
[`crate::engine::topology::is_slotted`] is the total function of the
identity that decides it. Nothing here carries a second flag that could
disagree with the identity: the registration reads `is_slotted` and takes a
pair exactly when it answers `true`, so a shell probe that acquired a slot
would need `is_slotted` to be wrong about `ProbeTarget::Shell`.

### Why the runner is wrapped rather than the probes hand-built

An [`crate::agent::AgentAdapter`]'s `probe` runs **one or more** processes —
Codex runs four — and each is already a `RunnerRequest` carrying its own
`InvocationId` from [`crate::agent::probe_request`]. R4 is "invocation
registration (**all** Runner processes incl. gates, agent probes, and the
shell probe)", so registering only the identities this module could name in
advance would leave three of Codex's four unaccounted and the ledger would
balance while being wrong.

So the ledger sits **at the boundary**: [`Registering`] is a
[`crate::runner::Runner`] that registers, slots, runs and settles every
request that passes through it, whoever built it. That is also what makes
`R3`'s "released on complete or cancel" and `R4`'s "completed or cancelled
exactly once" true of a probe that *fails*, which is the case
`resume_refuses_by_preflight_probe_when_shell_or_cli_fails_before_any_recovery_event`
exercises.

### `certify` takes `&self`

The trait's signature is `fn certify(&self, policy: &RunnerPolicy)`, and an
implementation that allocates identities has to mutate a ledger behind it.
[`std::sync::Mutex`] rather than a `RefCell`: [`crate::runner::Runner`] is
`Send + Sync`, and [`Registering`] is a `Runner`.

## `pub struct Probed {`

What one pre-flight established.

The agents are carried in the order they were probed, because
`run_started(4).probed_agents` and `run_resumed(4).probed_agents` are lists
and a resume's list is compared against the record's.

## `pub struct Probed` › `pub agents: Vec<String>,`

Every agent whose CLI answered, in probe order.

## `pub struct Probed` › `pub caps: Vec<(String, Caps)>,`

What each agent's CLI reported about itself.

## `impl Probed` › `pub fn agents(&self) -> &[String] {`

The agent names, which is what the durable record carries.

## `pub struct RunPreflight<'a> {`

The run's `RunnerPreflight`: a shell probe and one probe per recorded agent.

Holds no `&mut` anything: the [`RunnerPreflight`] trait's `certify` takes
`&self` because `rebuild_from_record` holds the runner and the record while
it calls it.

## `pub struct RunPreflight<'a>` › `ledger: Mutex<InvocationLedger>,`

Every invocation this pre-flight registered, in one process-local
ledger, with the slot table the agent probes take their pairs from inside
it. R3 and R4 balance at process end.

The `Mutex` is `certify`'s `&self`'s, below: the probes register through a
boundary that is a `Runner`, and a `Runner` is `Sync`. The probes run one at
a time, so it is never contended.

## `pub struct RunPreflight<'a>` › `probed: Mutex<Option<Probed>>,`

What the last `certify` established, for the caller that has to record
`probed_agents`.

## `impl<'a> RunPreflight<'a>` › `pub fn new(`

The pre-flight of a run whose recorded shell is `shell` and whose
recorded agents are `agents`.

`workspace` is where the shell probe runs. The agent probes take
[`crate::agent::probe_workspace`] through
[`crate::agent::probe_request`] — "a probe asks a CLI about itself and
has no workspace of its own" — and that decision is the adapter
module's, not this one's.

## `impl<'a> RunPreflight<'a>` › `pub fn probed(&self) -> Option<Probed> {`

What the last successful [`RunnerPreflight::certify`] established.

## `impl<'a> RunPreflight<'a>` › `pub fn ledgers_balance(&self) -> bool {`

Whether every invocation this pre-flight registered was settled exactly
once, and no slot pair is still held or waited for.

`resource_accounting` R3/R4 at `NoRunFinished`: "released (process
death; empty at restart)" — but a *refusal* is not a process death, and
a pre-flight that refused while still holding a slot would leave the
resume's own later invocations refused a pair its own leak holds. This is
what `resume_preflight_probe_containers_reclaimed_after_refusal`
asserts alongside the containers.

## `impl<'a> RunPreflight<'a>` › `pub fn settlements(&self) -> (usize, usize) {`

How many probe invocations settled completed, and how many cancelled.

Two numbers because R3 has two rows. A refused spawn is a **cancel** —
the process either never started or produced no outcome this run may act
on — and a boundary that completed it would leave a balanced ledger
saying the opposite of what happened.

## `impl<'a> RunPreflight<'a>` › `pub fn running(&self) -> Vec<String> {`

Every identity still registered as running. Empty once `certify`
returns, on either path.

## `impl<'a> RunPreflight<'a>` › `fn registering(&self) -> Registering<'_, InvocationLedger> {`

The runner every probe of this pre-flight actually executes through:
the run's Runner, with the registration wrapped around it, on the
probe boundary ([`Slots::Probe`]) — the shell probe registers without a
pair, each agent probe with its own.

## `impl RunnerPreflight for RunPreflight<'_>` › `fn certify(&self, policy: &RunnerPolicy) -> Result<(), UpstrokeError> {`

The shell probe, then one probe per recorded agent, in that order.

**The order is the contract's**, and it is not alphabetical or
arbitrary: `recovery_order` (c) writes "the non-slotted shell probe
(recorded shell, exit 0) **and** one slotted probe per recorded agent",
and `runner` adds "probes execute through it **sequentially** at
pre-flight". A shell that cannot run a command is the cheaper refusal
and the one that explains every agent failure that would follow it, so
it goes first and the agent probes are never reached when it fails.

`policy` is not consulted here and that is deliberate: the runner this
executes through was *built* from the record — `rebuild_from_record`
verified the runtime, the recorded image id and the volumes before
calling this — so re-reading the record to decide anything would be
this module deciding a question the rebuild already answered. It is
named in the refusal, which is what a caller needs it for.

### Errors

[`UpstrokeError::Refused`] naming the shell or the agent whose CLI did
not answer. Every invocation registered before the refusal is cancelled
and every slot pair released, so the ledger balances on both paths.

## `fn certify(&self, policy: &RunnerPolicy) -> Result<(), UpstrokeError> {` › `let shell_id = PreflightIdentities::shell(0)?;`

(1) The shell. Non-slotted, and `is_slotted` is what makes that true
rather than an argument here.

## `fn certify(&self, policy: &RunnerPolicy) -> Result<(), UpstrokeError> {` › `let mut caps = Vec::with_capacity(self.agents.len());`

(2) One probe per recorded agent, sequentially, in recorded order.

## `fn refused(policy: &RunnerPolicy, what: &str) -> UpstrokeError {`

The refusal shape every pre-flight failure takes.

Names the image the probe ran inside, because "a recorded shell or agent CLI
that fails **inside the recorded image**" is a different fact from the same
CLI failing on the host, and an operator who cannot tell which one has
nothing to fix.

## `pub(super) enum Slots {`

Which pairs a registering boundary gives the requests that cross it.

**Three because the boundaries are three.** INV-23's asymmetry is a real
one — "one non-slotted shell probe (the recorded shell executing `exit 0`)
**and** one slotted probe per recorded agent" — so the shell probe's
boundary ([`super::create::ShellProbe`]) holds no pairs at all, and a
slotted invocation arriving on it is refused rather than quietly run
unslotted. The agent probes' boundary gives each request the pair
`{agent, none}`. And since PR11 the review passes of an attempt or an
integration verification cross a boundary of their own, whose pair carries
the reviewer's pool and whose requests present the standing of the pipeline
they belong to. The alternative was a copy of register/run/settle per case,
and this file's own header is that there is **one place**.

## `pub(super) enum Slots` › `None,`

The shell probe's boundary: no pairs.

## `pub(super) enum Slots` › `Probe,`

The pre-flight's: each agent probe takes `{agent, none}`. A probe runs before
admission, so it holds no pipeline entitlement and needs none
([`crate::engine::topology::select::Standing::preflight`]); the pre-flight
holds no pool table, and its probes run one at a time.

## `pub(super) enum Slots` › `Pipeline {`

A pipeline's review passes and re-asks: the reviewer's pool, and the
pipeline's standing read from the fold by the judge that built the boundary.

## `pub(super) struct Registering<'a, L> {`

The run's Runner with R3 and R4 wrapped around every request.

Its own refusals — a registration the ledger refuses, a slotted request
on a boundary with no pairs or with no agent, a pair not grantable at once —
are `RunnerError`s with the fate `NeverStarted`, because they precede the
inner Runner; a settlement failure after the inner run carries the inner
outcome's fate (`Gone` for an output, the inner error's for an error), so
the boundary never claims more about the process than the Runner it wraps.

One place, so that "each a registered invocation" is true of a process an
adapter or a review pass built as much as of one this module built.

**`pub(super)` since 2026-08-27, because "one place" was not true.** Fresh
creation did not use it: `create.rs`'s P4 registered a single
`probe(agent, 0)` identity around the *whole* adapter call and handed the
adapter the raw Runner, so an adapter that runs ten processes — a current
Codex probe runs version, two help probes, six strict-config probes and the
model catalog — put one of them in the ledger. A failure at ordinal 1 was
recorded as ordinal 0 cancelled: the ledger named the process that
*succeeded* and held no record of the one that failed. The `bf927f3` review
found it as its third P1; the doc above was already the argument against it.

**Generic over the ledger's owner since PR11.** The pre-flight and run
creation own their ledger (`L = InvocationLedger`); a judge borrows the run's
for the length of one review pass (`L = &mut InvocationLedger`). Either way
the `Mutex` guards the register and settle calls only, never the Runner call
between them.

## `pub(super) struct Registering<'a, L>` › `completed: AtomicU32,`

How many of this boundary's requests completed. A judge compares it with the
number of processes a review pass reports, which is how a pass that ran a
process outside the boundary is caught.

## `impl<L: BorrowMut<InvocationLedger> + Send> Runner for Registering<'_, L>` › `fn run<'a>(&'a self, request: &'a RunnerRequest, call: RunnerCall<'a>) -> RunFuture<'a> {`

Register (with the pair when slotted), run, settle — and settle on the
failure path too. Since PR11 the inner Runner's `run` is a future, so the
wrapper is one too: `admit` (register, and take the pair) and `settle`
(complete or cancel, which releases the pair) are synchronous and run on
either side of the one `.await`, so no ledger guard is ever held across it —
which is also what keeps the future `Send`. The call is forwarded to the
inner Runner unchanged, so a cancellation or an observer the caller carries
reaches the process.

The settlement is not in a `Drop`: a `Drop` that swallowed a refusal
would make a ledger that did not balance look like one that did, and
`permits.protocol` asks for "registered/completed/cancelled **exactly
once**". A run that returns `Err` is a **cancel**, not a complete: the
process either never started or did not produce an outcome this run may
act on, and `R3`'s two lifecycle rows are "released on cancel" and
"released on complete or cancel" precisely so the two are told apart.

## `impl<L: BorrowMut<InvocationLedger>> Registering<'_, L>` › `fn admit(&self, request: &RunnerRequest) -> Result<(), RunnerError> {`

Everything before the inner run: register, with the pair for a slotted
invocation, through [`InvocationLedger::register_at_once`] — a pair not
grantable at once is withdrawn inside the ledger and refused, because the
boundary's caller cannot wait (INV-18). The refusals that precede the ledger
— a slotted request on a boundary with no pairs, or naming no agent — are
made before anything is registered, so every refusal here leaves the ledger
settled.

## `fn admit(&self, request: &RunnerRequest) -> Result<(), RunnerError>` › `Slots::None => {`

The non-slotted boundary cannot account a slotted invocation, and
running it anyway would put an agent probe through a path that
takes no pair — the process would execute with `permits.
agent_pool_slots` never consulted.

## `fn admit(&self, request: &RunnerRequest) -> Result<(), RunnerError>` › `let Some(agent) = request.agent.as_ref() else {`

The agent whose per-agent slot this is. A slotted invocation
without an agent binding cannot be accounted, and refusing is
the assertion rather than inventing a name for it.

## `impl<L: BorrowMut<InvocationLedger>> Registering<'_, L>` › `fn settle(`

Everything after the inner run: complete the registration on an output or
cancel it on an error, exactly once, which releases its pair. A settlement
failure carries the inner outcome's fate.
