# PR11 follow-up A — the coordinator's container reaper spans its incarnation: the working record

The record of PR11's follow-up A, kept on the branch so that a successor session inherits what was
decided and why. Each phase adds its section; at any commit on this branch the record is true. It is
**not** a design authority: `DESIGN.md` and the packet stay that, and a sentence here that disagrees
with either is a defect in this file.

**What it repairs.** `PR11-REAPER-CONTAINER-SCOPE-UNREGISTERED` (P1, `deferred`;
`findings/P1_correctness_202610010031_no-topology-caller-registers-the-reapers-container-scope.md`):
on Unix a dead coordinator's labeled containers run on until the next write command's census, because
nothing keeps a reaper armed across a container invocation. PR #327 (PR11) merged on 2026-10-01 as
`92c4ca81` with that finding deferred, and the owner's decision on #327 — relayed verbatim as "yea
continue, narrow and split out work" — made its repair this follow-up, due **after PR11 and before G6**
(`~/orch-pr11/briefs/followups/fu-a-coordinator-container-reaper.md`, which binds the work).

**Branch.** `fix-P1/correctness_no-topology-caller-registers-the-reapers-container-scope`, cut from
master at `92c4ca81f9209d218df4534ee71d3445dc2906e1`, the merge commit of #327. Every `path:line` below
is at that commit unless another is named; `~/orch-pr11/logs/pr11_fua_design/tools/check-citations.py`
prints each cited line from it (`measure/citations-check.txt` beside it).

**Who writes it.** Phase 1, the design (§1), is `pr11_fua_design`'s (`claude-opus-5-5`, `max`); it
writes no production code. Its figures are in saved files under `~/orch-pr11/logs/pr11_fua_design/`
that each sentence names. The design is reviewed (`gpt-6-astra`, `max`; a scope/fix lens, a regression
lens and a concurrency lens) before a fresh implementer builds it. A sibling follow-up,
`fix-P1/correctness_linked-checkouts-race-the-shared-worktree-registry` (follow-up B), runs in
parallel; it owns `src/workspace_manager*`, `src/rundir.rs`, the topology effect sites and `DESIGN.md`'s
run-layout and lock text, and nothing below changes any of them.

## 0. Status

- **Phase 1 (design): §1**, by `pr11_fua_design`, pushed at `793c3784`. Reviewed by three
  `gpt-6-astra` lenses at `max` (round 1): the core held in all three; four P2 corrections.
- **Phase 2 (implementation): §2**, by `pr11_fua_impl` (`claude-opus-5-5`, `max`). §1 was first
  amended with the four corrections, each marked with its id (§1.10), and then implemented with a
  regression test per requirement, a witness per correction, the domination census, and a mutation
  for each. `PR11-REAPER-CONTAINER-SCOPE-UNREGISTERED` is fixed and its file deleted; the inherited
  limit the regression lens found is filed as its own finding.
- **Next:** the orchestrator's implementation review (delta and fix-check over the four corrections
  and the eight requirements, then regression, then concurrency), and the rounds `MAINTAINING.md`
  prescribes. G6's input range must include this follow-up's merge.

## 1. Design

### 1.0 The design in one paragraph

**Arm at the sink, not at the sources.** PR11's rounds R5 to R7 armed the reaper at an entry of the
incarnation (`run_concurrently`, then the resume's pre-flight) and each review found a container
launch that came before the entry: recovery's pre-flight probes (`R6-C2`), then a fresh run's P4
probes (`R7-D1`). Every container any path starts is started by one function,
`ContainerRunner::launch` (`src/runner/container/exec.rs:542`), so the reaper is armed there: the
runner's private `contain` (`exec.rs:822`) **covers** each invocation before it calls `launch`, the
first cover forks the runner's container reaper, and `launch` takes the cover's proof value as an
argument, so no container can be started by this runner without it. The reaper's scope is built from
the runner's own `RunIdentity` — the private root and incarnation every container it starts is
labeled with — so it cannot name anything but what it must kill. The runner owns the guard: it
disarms when it is dropped **and** every launch it began reached an established end; otherwise it
leaves the reaper armed until the process exits. The reaper holds no cleanup lease (§1.3). A census
(§1.4) fails the build's tests when a second production path to a container start appears. No frozen
file changes, and no topology module's production code changes at all.

### 1.1 Every container launch of an incarnation

**How the set was found** (`~/orch-pr11/logs/pr11_fua_design/measure/launch-callers-92c4ca81.txt`,
`git grep` at `92c4ca81`). A container is started only through the Container funnels
`create_container` and `start_container` (`src/runner/container.rs:278`, `:335`), the only callers of
`ContainerRuntime::create` and `ContainerRuntime::start` (`container.rs:288`, `:346`); those two
methods are denied everywhere else by `clippy.toml:231`–`:232`, and the allowance that lets
`container.rs` call them is the funnel's own, held to its place by
`every_allow_of_a_governed_lint_is_module_level_and_in_the_allowlist` (`src/effects/tests.rs`). The two
funnels have exactly two production callers: `ContainerRunner::launch` (`exec.rs:579`, `:616`) and the
free `container::launch` (`container.rs:460`, `:482`); the free `launch` (`container.rs:445`) has no
production caller — its callers are tests, including the frozen `recover/tests.rs:4000`. So every
production container launch is a `ContainerRunner::launch`, whose one caller is `contain`
(`exec.rs:839`), reached only through `invoke` (`exec.rs:806`) and the runner's `Runner::run`
(`exec.rs:800`). A container could otherwise be started only by running the CLI itself, and two
existing censuses close that: `the_denylist_names_every_primitive_the_packet_enumerates`
(`src/effects/tests.rs:3222`) fails any production file but `container.rs` that names `docker`,
`podman`, `bollard` or `DockerCli` (its `NAMES_A_CONTAINER_RUNTIME` table), and
`every_production_process_start_is_classified` (`src/runner/contract.rs:1632`) keeps process starts
in their funnels. What remains is
to enumerate every caller of `Runner::run` on an incarnation's Runner; the table does, from each entry
of the incarnation down to that `Runner::run`.

**No production code builds a `ContainerRunner` at this head.** Its constructions are in tests and in
the test-only scaffold (`src/engine/topology/scaffold.rs:3220`, compiled only under `#[cfg(test)] mod
scaffold`, `src/engine/topology.rs:36`–`:37`), and the whole topology engine is dead code in a
production build (`src/engine/topology.rs:3`). Every row is therefore reachable from tests today and
from PR12's production caller later — which is why the arming point has to dominate callers that do
not exist yet.

| # | launch | the chain from the incarnation's entry to `Runner::run` (every frame at `92c4ca81`) | frozen frame |
|---|---|---|---|
| 1 | fresh run, P4 shell probe (T-RUNSTART P4, before `run_started`) | caller → `create::create_run` (`create.rs:1571`) → `p4_run_preflight` (`:1601` → `:1750`; the run's cleanup scope entered at `:1756`) → `request.probes.shell` (`:1800`) → `RunnerProbes::shell` (`:408`) → `runner::host::run_shell_probe` (`:413` → `host/probe.rs:23`, `run_blocking` `:31`) → `Runner::run_blocking` (`contract.rs:282`) → `ShellProbe::run` (`create.rs:282`) → `Registering::run` (`preflight.rs:276`, inner run `:283`) → `ContainerRunner::run` | none |
| 2 | fresh run, P4 agent probes, one per recorded agent | … `p4_run_preflight` → `request.probes.agent` (`create.rs:1829`) → `RunnerProbes::agent` (`:421`) → `adapter.probe(through)` (`:434`) → the adapter's probe requests (`claude.rs:53`, `:87`, `:118`; `codex.rs:90`, `:124`, `:131`, `:141`, `:176`, `:186`, `:387`; `copilot.rs:51`, `:85`) → `AgentProbe::run` (`create.rs:298`) → `Registering::run` → `ContainerRunner::run` | none |
| 3 | resume, recovery step (c) shell probe | caller → `recover::run_recovery_order` (`recover.rs:912`; the run's cleanup scope `:924`) → `PreflightCertified::certify` (`:959` → `:764`, `preflight.certify` `:768`) → `RunPreflight::certify` (`preflight.rs:113`) → `run_shell_probe` (`:116`) → `Registering::run` (built at `:107`–`:108`) → `ContainerRunner::run` | `recover.rs` (`:912`, `:959`, `:764`–`:768`) |
| 4 | resume, recovery step (c) agent probes | … `RunPreflight::certify` → `adapter.probe(&registering)` (`preflight.rs:129`–`:130`) → the adapter's probe requests (row 2) → `Registering::run` → `ContainerRunner::run` | `recover.rs`, as row 3 |
| 5 | attempt worker, under the coordinator | caller → `TopologyRun::run_concurrently` (`coordinator.rs:200`) → `Coordinator::drive` → `spawn_attempt` (`:673`; `Work { runner: &*seams.runner }` `:704`, carried leases `:682`) → `attempt_body` (`attempt.rs:742`) → `Judge::run_worker` (`:744` → `:916`, `execute` `:930`) → `execute_typed` (`:1164`) → `run_blocking_with` (`:1173`) → `ContainerRunner::run` | none |
| 6 | attempt gates, under the coordinator | … `attempt_body` → `judge_attempt` (`attempt.rs:759`) → `Judge::judge` (`:969`) → `verdict` (`:1207`) → `execute_typed` (`:1173`) → `ContainerRunner::run` | none |
| 7 | attempt review passes and re-asks, under the coordinator | … `Judge::judge` → `Registering::new(…).carrying` (`attempt.rs:1049`–`:1057`) → `self.reviews.run` (`:1058`) → production's `ReviewPasses`, `LegacyReviewPasses` (`engine/attempt.rs:255`, `:262`) → `review::run_review` (`review.rs:462`, `run_blocking` `:547`) → `Registering::run` → `ContainerRunner::run` | none |
| 8 | verification gates and reviews, under the coordinator | `Coordinator::drive` → `Coordinator::integrate` (`coordinator.rs:590`) → `integrate::integrate(&mut DrivenJournal(self), …)` (`:600`) → `journal.verify` (`integrate.rs:696`) → `DrivenJournal::verify` (`run.rs:179`–`:180`) → `Driver::verify` (`coordinator.rs:1632`) → `verify_concurrently` (`:789`) → `spawn_verification` (`:726`; `Work` `:760`, carried leases `:737`) → `verification_body` (`run.rs:339`) → judge → rows 6–7 | `integrate.rs:696` |
| 9 | width-1 `step`: worker, gates, reviews | caller → `TopologyRun::step` (`run.rs:822`) → the attempt path (`Work { runner: seams.runner }` `:933`–`:937`) → `attempt_body` → rows 5–7 | none |
| 10 | width-1 `step`: verification | `step` → `TopologyRun::integrate` (`run.rs:1048`) → `integrate::integrate` (`:1077`) → `journal.verify` (`integrate.rs:696`) → `IntegrationCx::verify` (`run.rs:214`; `Work` `:219`–`:227`) → `verification_body` → rows 6–7 | `integrate.rs:696` |
| 11 | `AttemptContext::{start, run_worker, judge}` | public surfaces (`attempt.rs:423`, `:432`, `:476`) whose one production construction (`run.rs:1685`) calls `announce` alone; their other callers are tests → `Judge` → rows 5–7 | none |
| 12 | closure: halt, budget drain, shutdown, append error, terminal finalization | **no launch**: each cancels or waits on invocations already running, which settle through their own `contain`; `closure.rs` names a Runner only in prose and `finalize.rs` names none | — |
| 13 | recovery steps (a), (a1), (b), (d)–(h) | **no launch**: `ResumeSeams` (`recover.rs:805`) carries no Runner, only the pre-flight; step (a)'s census stops and removes, never starts | — |
| 14 | the startup census, a fresh run's and a resume's | **no launch**: `reclaim` stops and removes (`stop_container`, `remove_container`) | — |
| 15 | the legacy engine (schemas 1–3) | **no container**: its runner is the host's (`engine/resume.rs:52`, `:86`), and a container selection is refused before any effect (`prelock.rs:100`; ST-16 (i)) | — |

Rows 1–11 are every launch; every one of them ends in the same three frames, `ContainerRunner::run`
→ `invoke` → `contain`, below every frame the table lists, frozen or not.

### 1.2 One arming point that dominates them all

**The point.** `ContainerRunner::contain` (`exec.rs:822`) covers its invocation, after the plan
(`exec.rs:390`, which only reads) and before `launch` (`exec.rs:839`), whose first effect is the intent
write (`exec.rs:547`). The cover is a private `Reaping` value the runner owns:

```text
contain(request, cancellation, hooks):
    if cancellation is set -> NeverStarted                      (as today: nothing to cover)
    plan    = self.plan(request)?                               (reads only, as today)
    covered = self.reaping.cover(self.runtime.reaper_program(),
                                 &self.identity,
                                 &plan.launch.spec.labels)?     (arms on the first cover; refuses -> NeverStarted)
    result  = <today's contain body, with launch(hooks, &plan.launch, &covered)>
    covered.settle(fate_of(&result))                            (Ok -> Gone; Err(e) -> e.fate)
    result
```

`launch` gains a parameter, `covered: &Covered<'_>`, and `Covered` has one constructor, inside
`Reaping::cover`. So within `exec.rs` the type system enforces the order — no `launch` without a
cover — and the census (§1.4) enforces what the type system cannot: that `launch` stays the only
production path to `create_container` and `start_container`.

**Why every launch in §1.1 is reached only through it.** Four facts, each checked:

1. A container is started only by `create_container`/`start_container` (`container.rs:278`,
   `:335`), whose methods are denied elsewhere (`clippy.toml:231`–`:232`).
2. Their production callers are `ContainerRunner::launch` and the free `container::launch`, and the
   free one has no production caller (§1.1's method, saved in `launch-callers-92c4ca81.txt`).
3. `ContainerRunner::launch` is private and called once, by `contain` (`exec.rs:839`).
4. `contain` covers before it calls `launch`, and `launch` cannot be called without the cover's
   value.

Rows 1–11 all reach `contain` through `ContainerRunner::run`; none can reach a container start any
other way. Domination here is a property of two functions in one file, not of the order in which a
command's entries run. That is what changes from rounds R5–R7: an arming point above the launches
has to be shown to precede every launch of every caller, and each review found a caller it did not
precede; an arming point inside the launch funnel precedes every launch of every caller, including
PR12's and any later one, by construction.

**It lies in non-frozen code.** `src/runner/container/exec.rs` is not in PR11's frozen set: it is
absent from the 26 frozen production files (`reviews/2026-09-30-pr11-record.md:443`–`:463`, and §5's
`FROZEN_PROD`, `:2114`–`:2123`), and R-D says why — "The runner's execution modules (`host.rs`,
`container/exec.rs`, `contract.rs`) are not runner *identity*: they execute under the recorded policy
and do not decide it" (`reviews/2026-09-30-pr11-record.md:470`–`:472`). G6's rule — "fold, queue, merge, repair, and recovery modules
byte-identical to the G5 range" — names none of the files this design changes (§1.9). `recover.rs`,
`integrate.rs`, `resolve.rs` and every other frozen file are untouched, as is the frozen test child
`recover/tests.rs`: the design keeps `ContainerRunner::new`'s signature (`recover/tests.rs:9232` calls
it) and `RunPreflight::new`'s.

**How the scope is validated before arming and before any probe (`R7-D2`).** `R7-D2`'s sequence was
"a resume is handed a reaper built for a foreign incarnation → the pre-flight arms it unchecked and
starts its probe → … the reaper lists the foreign label, finds nothing". Its root was that the scope
was an input, built by the caller apart from the runner that labels the containers. Here it is not
an input:

- **Derived, never handed in.** The cover builds the scope as
  `ReaperContainerScope::new(runtime.reaper_program(), &identity.private_root, &identity.incarnation)`
  from the runner's own `RunIdentity` — the same value `plan` labels every container with
  (`labels: intent.labels(&self.identity.private_root)`, `exec.rs:447`; `intent.rs:158`–`:166`), and
  both derive the root's label through one function, `intent::private_root_label` (`intent.rs:74`;
  `census.rs:28`). No API accepts a scope or a reaper from outside the runner, so `R7-D2`'s sequence
  cannot be written against it.
- **Well-formed before the fork.** `ReaperContainerScope::new` refuses an empty or filter-breaking
  root label or incarnation (`census.rs:777`), and `render_container_argv` refuses a program it cannot
  resolve to a path (`proc.rs:4749`, `:4771`) — both on the parent side, before anything is forked.
- **Checked against each container before anything forks, and before its intent** (amended,
  `FUA-D1-DES-2`). Every cover takes the scope it would hold — the armed scope, or, on the arming
  cover, the scope it is about to arm, built from the runtime's reaper program and the runner's
  identity — and compares that scope's two filter values (`label=upstroke.private_root=…`,
  `label=upstroke.incarnation=…`; `census.rs:826`, `:828`) with this container's two label values in
  `plan.launch.spec.labels` **first**. A difference refuses the launch before any fork, any intent
  and any effect; only a scope that selects the container is armed. The design as reviewed at
  `793c3784` said "arm if unarmed; refuse if … the armed scope's filters differ", which compares
  after the fork: under its own stale-incarnation mutation (`fua-m3`) a reaper over the wrong scope
  could acknowledge READY before the comparison refused, and a death in that interval had it reclaim
  a scope that is not this incarnation's (the design lens, finding 2). Both sides derive from one
  value, so the check fires only if a later change makes them diverge — a canonicalised root on one
  side, say — and then it fires at the first launch, in every test that launches a container, with
  nothing armed.
- **The first probe is covered like every container.** The cover runs before the first probe's
  intent, whatever entry the probe came from (rows 1–4); there is no "before the first probe" window
  for a check to be late in.

Whether the runner's identity is the *run's* — the recorded private root and this incarnation — is
the runner's construction, and PR12's caller's: a runner built for another root writes intents the
run's census does not scan. The reaper does not depend on it; whatever identity the runner has, its
reaper kills exactly the containers that identity labels. §1.8 records it as out of scope.

**Alternatives rejected.** *An arming point at the incarnation's entries* (rounds R5–R7's): its
domination is a whole-program ordering claim, and three rounds each found a launch before it.
*The topology's registration boundary* (`Registering`, `Judge::execute_typed`): two code paths, not
one, and neither knows whether its runner starts containers. *A process-wide registered scope*
(`set_container_reclaim_scope`, `proc.rs:4735`): one scope per process, which two runs in one process —
every concurrent test binary — overwrite in each other; the cover passes its scope by value and never
reads the global.

### 1.3 Ownership to established termination on every end

**Who holds the guard.** The `ContainerRunner`, in its private `Reaping`: a mutex over the armed
`agent::proc::ContainerReaper`, the count of invocations in flight, and a sticky `unsettled` flag. The
runner is shared by reference or `Arc` with every caller and pipeline (`PipelineSeams::runner`,
`coordinator.rs:48`), so it cannot be dropped while any `contain` is running on it, and the guard
never changes owner: there is no hand-over between creation, recovery and the coordinator. A caller
that uses one runner for its incarnation — the recommended wiring, and PR12's to make — has one
reaper from its first probe to its end; a caller that builds a second runner for the same incarnation
gets a second reaper over the same scope, and each covers its own launches.

**The mechanism, in `src/agent/proc.rs`'s `termination` module** (Unix; re-exported from
`agent::proc` under `cfg(unix)`, as round R5's was):

- `pub fn arm_container_reaper(terminate_site: ProcessSite, scope: &ReaperContainerScope) ->
  Result<ContainerReaper, UpstrokeError>` — refuses any site but `ProcessSite::Terminate` (as
  `Supervisor::begin` does, `proc.rs:1697`); renders the scope before the fork (`render_container_argv`);
  calls `shared_state()?` (`proc.rs:1865`), which installs the signal monitor (`R6-D1`); forks one
  reaper through `spawn_reaper`'s fork with **no cleanup lease** and the rendered scope, and waits for
  its READY within `HELPER_READY_BUDGET`. No process group, no launch claim, no `REGISTER`: the
  reaper's `pgid` stays 0, so it never settles a group and never forks an anchor.
- `pub struct ContainerReaper` with a `Drop` that cancels: `REAPER_CANCEL`, acknowledged within two
  seconds as `Reaper::cancel` (`proc.rs:2355`) does, and fail-closed as it does when the
  acknowledgement does not come. A cancelled reaper exits without listing anything
  (`REAPER_CANCEL if requested == 0`, `proc.rs:2760`, with `pgid` 0): **cancellation kills
  nothing**.
- **The exit after the acknowledgement is waited for with a deadline** (amended, `FUA-D1-CONC-1`).
  The design as reviewed kept `Reaper::cancel`'s unbounded `AcknowledgedExit` wait (`proc.rs:2427`),
  so a reaper stopped between writing `REAPER_OK` (`proc.rs:2764`) and its `_exit` held its
  coordinator's drop — and with it a normal shutdown — for as long as it stayed stopped (the
  concurrency lens). That wait is unbounded for a host reaper because its exit releases the cleanup
  lease its caller is about to act on (`ReaperEnding::AcknowledgedExit`); a container reaper holds no
  lease (below), so nothing downstream depends on its exit. Its drop therefore waits
  `HELPER_END_BUDGET` for the exit; a reaper still there is sent `SIGKILL` — it acknowledged
  `CANCEL`, so the only thing left for it to do is exit, and killing it loses nothing — and waited
  for once more within the same budget; one still not collectable then is left for the process's
  exit. Every wait in the container reaper's life has a deadline: READY (`HELPER_READY_BUDGET`), the
  acknowledgement (two seconds), the exit (two `HELPER_END_BUDGET`s). Host reapers keep
  `Reaper::cancel` and its unbounded wait unchanged.
- `spawn_reaper` (`proc.rs:2498`) takes the container scope as an argument rather than reading the
  process-global one (`proc.rs:2546`); `Supervisor::begin` passes `container_scope_for_a_new_reaper()`
  as today, so host reapers are unchanged. (As built, `spawn_reaper` keeps its signature and the
  fork it shares is `fork_reaper(leases, containers)`: §2.1.)

**The rule, in `exec.rs`'s `Reaping`:**

- `cover` (under the mutex, held across the fork on the first cover so a concurrent first launch
  waits for it): take the armed scope, or build the one to arm; refuse if its filters differ from the
  container's labels, **before arming** (`FUA-D1-DES-2`); arm if unarmed, refusing if arming fails;
  then count the invocation in flight and return `Covered`.
- `Covered::settle(fate)`: if `fate` is `Unresolved` set `unsettled`; decrement in flight.
  `Covered`'s `Drop` without a settle — an unwinding panic inside `contain` — sets `unsettled` and
  decrements.
- `Reaping`'s `Drop`, when the runner is dropped: if armed, and nothing is in flight, and `unsettled`
  was never set, drop the `ContainerReaper` (cancel: disarmed); otherwise release it **without
  cancelling** (`std::mem::forget`, as round R6's `IncarnationReaper` did), so its descriptors stay
  open until the process exits and the reaper then kills and removes every container its scope
  labels.

`ProcessFate` (`src/error.rs:73`) is the established-termination evidence: `Gone` is a release that
observed the container exited and removed it (`exec.rs:850`–`:860`), `NeverStarted` a container never
started (a failed create or start is cancelled by the runner itself), and `Unresolved` a container
that may still run. The disarm rule reads exactly the fates the runner computed for its own
containers; it does not depend on any coordinator ledger, so no caller can get it wrong.

**Every end:**

| end | what happens to the containers | the reaper |
|---|---|---|
| normal end (run complete, parked, budget exceeded, halted after its closure) | every container released by its own `contain` (`Gone`) | disarmed when the runner drops: cancelled, exits without listing — a normal end never kills a live container of its own |
| error return, every container established gone | each `contain` settled `Gone`/`NeverStarted` | disarmed at drop |
| error return with an unresolved container (`R6-C1`) | at least one `contain` settled `Unresolved` | never disarmed by this process: kept armed past the last handle; at the process's death it kills and removes the containers |
| append error under concurrency | in-flight pipelines cancelled; each `contain` releases its container and settles its fate | per the fates, as the two rows above |
| halt, shutdown | in-flight invocations cancelled through their `Cancellation`; each settles | per the fates |
| budget drain | in-flight invocations run to their end and settle | per the fates |
| panic unwinding inside `contain` | the container's fate is unknown | `Covered`'s drop sets `unsettled`: kept armed |
| panic elsewhere | pipelines' panics are caught (`catch_unwind`, `coordinator.rs:700`, `:756`); the coordinator's own unwinds through its `Drop` | per the fates of what ran |
| killed before the first cover | no container of this runner exists | none armed; nothing to do |
| killed while arming (fork done, READY not yet read) | none: `launch` waits for `cover` to return | the reaper, if it reached its loop, sees its parent gone, lists nothing, exits |
| killed between the cover and `create` (intent, view) | no container | lists nothing; the next census removes the intent and view |
| killed after `create` or `start`, while running, or mid-release | the container exists or may | lists by scope (`docker ps --all`), kills and removes each (`reclaim_labeled_containers`, `proc.rs:4812`), lists again; the next census removes the intents and views |
| killed during the disarm (`CANCEL` written) | none of this runner's left (disarm needs every fate established) | exits on `CANCEL`, or sees the death first and lists nothing |
| killed after an unsettled drop | the unresolved containers | kills and removes them |
| `R6-D1`: the cancellation fails (the reaper stopped, killed, or not answering) | — | the drop arms fail-closed termination, as `Reaper::cancel` does, and the monitor arming installed ends the process with `SIGTERM` — the module's rule for a helper whose state it cannot establish, which `R6-D1` requires here too. A reaper that was only slow then finds its coordinator gone and reclaims what the incarnation still runs, or reads its `CANCEL` first and exits without listing (§1.8 weighs keeping this arm) |
| the reaper stopped after acknowledging `CANCEL` (`FUA-D1-CONC-1`) | none of this runner's left (a disarm needs every fate established) | the drop waits `HELPER_END_BUDGET` for its exit, sends it `SIGKILL`, and waits once more within the same budget: the caller goes on within two budgets, and the reaper, killed after its last decision, lists nothing |
| Windows | — | no reaper (ST-16 (e)): `cover` counts and returns its value, `Reaping`'s drop does nothing, and the next write command's census reclaims, as `OrphanWindow::UntilNextWriteCommandStart` documents |

**R28: the container reaper holds no cleanup lease.** Round R5's reaper held the run's cleanup lease
through the paths in force at its fork. This one holds none, by construction (`spawn_reaper` is handed
no lease and does not merge the thread's scopes for it), for three reasons:

1. **A held R28 would contradict a frozen test, and the test is right.** The frozen
   `a_lost_gate_container_is_reclaimed_by_the_next_resume_before_the_verification_is_settled`
   (`recover/tests.rs`, from `:9456`) drives a container gate through `step` with the runtime unable
   to observe, stop or remove it — `Unresolved`, so this design keeps the reaper armed — and then, in
   the same process, with the runner still alive, resumes the run as a new incarnation and requires
   the resume to converge once the runtime is back. Its lock step takes the worktree lock, which
   scans every run's cleanup hold, and the run lock, whose exclusive cleanup probe refuses on any
   shared hold (`recover.rs:271`–`:272`; `rundir.rs:1965`, `:2444`–`:2480`). A reaper holding the run's
   lease would refuse that resume for as long as the test process lives. The test encodes the
   packet's liveness rule — T-CONTAINER's `resume_action`, "owner run == this run -> incarnation !=
   this process's incarnation -> dead by construction -> reclaim", because "the run lock is
   exclusive, so only one incarnation of a run is ever live" (`crash_reconstruction`) — and the
   incarnation is dead because its run lock was released, whatever its process is doing.
2. **The census reclaims the same containers, and the two converge.** Every container the reaper
   kills is labeled and intent-recorded, and the next write command's census reclaims it under the
   incarnation-aware rule whether or not the reaper got there first: "every step idempotent and
   tolerant of already-gone so two concurrent reclaimers converge" (`crash_reconstruction`; ST-16
   (h)). R28's work is ordering a successor behind a reaper that "may outlive the coordinator while
   it settles its process groups" (R28's row) — the one residue a census cannot see; a container
   reaper has no group.
3. **It makes the property's last clause structural.** "Disarming never leaves R28 held" cannot fail
   for a reaper that holds no lease, and `R6-D1`'s "goes on with R28 held, and the next coordinator is
   refused" cannot happen.

What that changes, and what it does not: a coordinator killed on Unix leaves no hold for its
containers, so the next write command may start while the reaper is still killing them; its census
reclaims them too, and when the two `rm`s meet, the census's "removal already in progress" answer is
not evidence, so that command ends resumably and the next one converges (the frozen
`recover/tests.rs` pins that answer, from `:9393`). Host reapers, and the Git children that hold the
lease while they write a ref (`rundir.rs:2193`), keep R28 exactly as before. **Whose R28 a fresh run's
creator's reaper holds before `run_started`** — `R7-D1`'s question — is therefore *none*: its probe
containers are covered by the reaper armed at the first P4 probe's cover, and a creator killed there
leaves the husk and the probe's intent and view to the next census (ST-16 (k)), with the container
itself already killed.

### 1.4 The census that keeps it dominating

**The test**, in the launch funnel's own test module, as the `TopologyFold` census is in the log
funnel's (`the_stable_prefix_barrier_is_the_only_way_a_log_becomes_a_topology_fold`,
`src/events/log/tests.rs`): `every_container_start_in_production_is_reached_only_through_a_covered_launch`
in `src/runner/container/exec/tests.rs`, `#[cfg(unix)]` (§1.6). **What it enumerates**, over every
`src/**/*.rs` production region (`effects::production_code`, skipping
`effects::census_domain::whole_file_test_modules`, as the fold census does):

**It reads namings, not calls** (amended, `FUA-D1-DES-1`). The census as reviewed counted call tokens,
so `use super::launch as start_uncovered;` and a call of `start_uncovered(` reached the free
launcher with no cover while all five checks passed (the design lens, finding 1). An **unarmed
launcher** is anything that starts a container without a cover: the free `launch`, the funnels
`create_container` and `start_container`, and the runtime's primitives `ContainerRuntime::create`
and `::start`. A **naming** of one is any occurrence of its identifier in code (comments and strings
blanked) as a path segment or a bare expression — a call, a `use` or `pub use` of it, an `as` alias, a
function value — other than its definition (`fn launch(`), a method call or field (`.launch`), or a
field initializer (`launch:`). Every naming of an alias, a function value or a re-export names the
original identifier somewhere, so a census of namings sees all three; only a glob import followed by
a call by the same bare name escapes the `use` clause, and that call is itself a naming.

1. **The funnels.** The production namings of `create_container` and `start_container` are exactly:
   in `src/runner/container/exec.rs`, its one `use super::{…}` import (one naming of each, with no
   `as`) and one call of each inside `ContainerRunner::launch`'s body; in `src/runner/container.rs`,
   one call of each inside the free `launch`'s body. Nothing else in any production region.
2. **The free `launch`.** No production naming at all, `container.rs`'s included. A production
   caller of the free `launch` — by name, alias, function value or re-export — is a second path that
   no cover dominates.
3. **The primitives.** `clippy`'s `disallowed_methods` (`clippy.toml:231`–`:232`) refuses every path
   reference to `ContainerRuntime::create` and `::start` — calls and function values alike — in
   every production module whose effective level for the lint is not `allow`. Twenty-seven
   production modules allow it module-wide (each recorded in `effects/allowlist.toml`), and a module
   that states nothing inherits its parent's level, so this check walks each production module's
   stated level up its parents and, in every module where clippy cannot refuse the primitives, pins
   each naming of `create` or `start` — a call by any receiver, a path, or a `use` — to an expected
   list of seventeen: in `container.rs`, `runtime.create(` and `runtime.start(` inside the two
   funnels and the standard library's `File::create(`, and in the other modules the fourteen such
   namings that exist, none of them a container runtime's. A call whose only argument is a
   `bool` literal is not counted — `OpenOptions::create(true)` — since neither primitive takes one.
   A new naming in such a module fails the census until it is added to the list or shown to be a
   container start; `RuntimeOp::Create`/`::Start` are pinned to `DockerCli`'s own `create` and
   `start` in the same walk.
4. **The covered launch.** In `exec.rs`: `fn launch(`'s signature names `Covered`; `self.launch(`
   occurs once, in `fn contain(`, after `self.reaping.cover(`; `Covered {` is constructed once,
   inside `fn cover(`.
5. **The control.** The sorted list of production regions naming `start_container` is exactly
   `["src/runner/container.rs", "src/runner/container/exec.rs"]`, and the walk scanned more than 40
   files and more than 750,000 non-whitespace bytes — the fold census's density checks — so a census
   that scanned nothing cannot pass; the lint-level walk read more than 100 modules as guarded and
   names `container.rs` and `view.rs` among those it is not. A second test runs the naming reader
   over written snippets — an alias, a function value, a re-export, a glob import and a call, beside
   a method, a field and a definition — and requires it to name the first five and none of the
   rest; a third runs the lint-level walk over a written tree — a stated allowance, one a silent
   child inherits, a production forbid beneath it, an allowance that holds in tests alone.

**Why the census and not visibility.** The review's preferred remedy is an unarmed launcher nothing
outside the funnel can name. Privacy cannot give it here: an item private to `runner::container` is
visible to every child module of it — the reason `clippy.toml`'s comment above `:231` gives for
denying the primitives instead — and `create_container` and `start_container` must stay nameable from
`exec.rs`. The free `launch` could be made test-only, but it is a `funnel` row of
`src/runner/container.rs` in `effects/wrappers.toml`, and `effects::tests::checks::reachable_fns_are_classified`
refuses a row whose fn is no longer reachable — an edit of another module's rows, which this
follow-up's brief reserves for the orchestrator — and the frozen `recover/tests.rs:3957` imports it.
So the census is the guard, not a backstop, and it is written to refuse every naming.

**How a mutation proves it can fail.** Four mutations, each a scratch copy built from itself (the
`Compiling` line naming the copy) and each turning the census red with its own message while the
unmutated copy passes:

| id | mutation | fails at |
|---|---|---|
| `fua-c1` | `contain` calls `self.launch(` before `self.reaping.cover(` (the cover's value made by a test-only constructor so it compiles) | 4 |
| `fua-c2` | a new production fn in `exec.rs` calls `start_container(` | 1 and 5 |
| `fua-c3` | a topology module calls `crate::runner::container::launch(` in production | 2 |
| `fua-c4` | `Covered {` constructed outside `cover` | 4 |
| `fua-c5` (`FUA-D1-DES-1`) | `exec.rs` gains `use super::launch as start_uncovered;` and a production fn calling `start_uncovered(` | 2 |
| `fua-c6` (`FUA-D1-DES-1`) | a production fn in `exec.rs` takes `super::start_container` as a function value and calls it | 1 |
| `fua-c7` (`FUA-D1-DES-1`) | `census.rs` re-exports `pub use super::create_container as create_uncovered;` | 1 |

### 1.5 The regression tests

Every test below is `#[cfg(unix)]`. "Two-process" means the phase-5 fixture: a child coordinator whose
`LinkedRuntime` relays every runtime call to the parent's `FakeRuntime` (`scaffold::Served::spawn` and
`ParentSide::attach`, as the round-7 witnesses use them, `~/orch-pr11/reviews/r7-witnesses/delta/witnesses.patch`).
The reaper's `docker` is the relay stub (the harness rules below): it records its calls in its own
directory, answers `ps` from a listing the parent writes, and the parent delivers the recorded `kill`
and `rm` to the fake. (As built, the listing carries every container's labels and the stub filters it
by the `ps` arguments, as the daemon would: §2.1.) Each mutation is a scratch copy built from itself and run against every test of
this table, so each row's red is measured against the whole set.

| requirement | test (kind, file) | the first-bad shape it fails on | mutation |
|---|---|---|---|
| `R6-C2`: arm before the incarnation's first container, a resume's pre-flight included | `a_resuming_incarnations_pre_flight_probe_container_is_killed_by_its_reaper_when_the_coordinator_dies_inside_it` (two-process; `coordinator.rs` tests): the child runs the frozen `run_recovery_order` with a `RunPreflight` over a `ContainerRunner`; the fake holds the shell probe running; the parent kills the child inside the probe; the relay's calls are exactly `ps` (the scope's two filters), `kill` and `rm --force --volumes` of the probe, `ps`; delivered, the probe is stopped and removed by `reaper` with no census listing before it | an arming point after the pre-flight (round R5's): the probe survives and the relay saw no call | `fua-m1` the cover never arms; `fua-m2` the cover skips `InvocationId::Probe` invocations |
| `R7-D1`: also before a fresh run's P4 probes | `a_fresh_runs_p4_probe_container_is_killed_by_its_reaper_before_run_started` (two-process; `create/tests.rs`): the child runs `create_run` with `RunnerProbes` over a `ContainerRunner`; killed inside the shell probe, the run directory still a husk; the relay reclaims the probe | an arming point after creation (round R6's): one running container survives, zero calls (the round-7 witness's own result) | `fua-m1`, `fua-m2` |
| `R7-D2`: scope validated before arming and before any probe | `the_reapers_scope_is_the_runners_identity_and_selects_its_first_probe` (two-process): the relay's first `ps` carries exactly the probe container's `upstroke.private_root` and `upstroke.incarnation` labels as the fake recorded them, and the probe is reclaimed; plus `a_container_whose_labels_differ_from_the_armed_scope_is_refused_before_its_intent` (unit, `exec/tests.rs`, a test-only constructor arming a scope for another incarnation): refused, no intent written, nothing created | a scope from anything but the runner's identity (round R6's caller-built reaper): the reaper lists a foreign label and the probe survives | `fua-m3` the cover builds the scope from a stale incarnation; `fua-m4` the label check removed (the unit test's red) |
| `R6-C1`: disarm only once termination is established | `a_runner_whose_containers_are_unresolved_keeps_its_reaper_armed_past_its_last_handle_until_the_process_exits` (two-process, width three): observe, stop and remove unreachable, so each `contain` settles `Unresolved` and the coordinator returns its error; the child drops every handle and waits for the parent's word; the relay has no call while the child provably lives; the parent restores the runtime and lets the child exit; the relay then reclaims all three; and the control `a_runner_whose_containers_all_ended_disarms_its_reaper_and_the_relay_is_never_called` (in-process, width three, observe unreachable only, so each runner stops and removes its own container) | a guard whose drop cancels on any return (round R5's): three containers survive, zero calls | `fua-m5` `Reaping`'s drop cancels whatever `unsettled` says; `fua-m6` `settle` ignores `Unresolved`; `fua-m7` `Covered`'s drop does not set `unsettled` (a panic witness, `a_panic_inside_contain_leaves_the_reaper_armed`, in an isolated child) |
| `R6-D1`: a failed cancellation releases or reports R28; the monitor is initialized | `a_container_reapers_failed_cancellation_ends_its_caller_through_the_signal_monitor` (isolated child, bounded): arming in a fresh process installs the monitor, the reaper is killed and reaped, the guard dropped, and the child ends by `SIGTERM`; `a_stopped_container_reaper_ends_its_caller_rather_than_releasing_it` (isolated child): the same with the reaper stopped, then continued by the kernel once the child is gone, which then lists by scope and exits; `an_armed_container_reaper_holds_no_cleanup_lease` (isolated child holding the run lock with its cleanup scope entered): R28 read not held, by a bounded wait | arming without `shared_state()` (round R5's): the caller exits 0 with `PENDING_TERMINATION` set and nothing reading it | `fua-m8` arming skips `shared_state()`; `fua-m9` the container fork merges the thread's cleanup scopes (the lease test's red) |
| `R6-D2`: the normal-exit control observes, the relay bound | `a_coordinator_that_ends_disarms_its_reaper_and_kills_nothing_at_width_three` (in-process; relay installed): the run completes; after the runner drops, the relay was never called and every container was released by its own runner; a self-check first runs the stub by its path and sees the call recorded, so the relay is bound in this process | a stub that finds its relay through a variable only a child sets (round R5's): an erroneous reaper's calls go nowhere the test looks | `fua-m10` `ContainerReaper`'s drop ends the reaper by end-of-file (`close_and_wait`) instead of `CANCEL` — the review-round-6 mutant |
| `R6-D3`: write-then-exec fixtures through an isolated writer | `the_reaper_relay_writer_leaves_no_writer_in_another_threads_fork` (Linux): the host suites' FIFO oracle (`runner::host::tests::inherited_writer`) over the fake's relay writer | an in-process `fs::write` of the stub (round R5's): another thread's fork keeps the writer, and the reaper's `execv` fails `ETXTBSY` | `fua-m11` the relay written in-process |
| the CI flake: every "R28/R17 not held" read is a bounded wait | `no_reaper_test_reads_a_hold_as_released_once` (source census over the new tests): a negated `observe_cleanup_hold(`/`is_running(` occurs only inside the module's bounded helper | a one-shot `assert!(!observe_cleanup_hold(..))` after a refusal (round R6's): a sibling thread's fork holds an inherited lease descriptor for a moment (`PR281-CLEANUP-LEASE-HOLD-OUTLIVED-AND-ITS-UNREADABLE-TWIN`) | `fua-m12` one such one-shot read added to a reaper test |
| the design property | `every_container_an_incarnation_starts_is_covered_by_an_armed_reaper_with_its_scope` (in-process): with a test-only observation of armed scopes at each container start, through a fresh run's P4 (shell and agent probes), a resume's pre-flight, `step` (worker, gates, reviews, verification) and `run_concurrently` at width three, every start is covered by a live reaper whose scope is the container's labels, and the count of observed starts equals the count of containers the fake created; with the R6-C1 pair and the R6-D2 control for its other clauses | a launch path the cover does not dominate | `fua-m13` the cover moved after `launch`; `fua-m2`; `fua-m3` |
| the census (§1.4) | `every_container_start_in_production_is_reached_only_through_a_covered_launch` | a second production path to a container start | `fua-c1`–`fua-c4` |
| `FUA-D1-DES-1`: the census refuses an alias, a function value and a re-export | the census above, and `the_naming_reader_names_an_alias_a_function_value_and_a_re_export_of_an_unarmed_launcher` (unit, over written snippets) | a census of call tokens (the reviewed design's): an alias reaches the free launcher and every check passes | `fua-c5`, `fua-c6`, `fua-c7` |
| `FUA-D1-DES-2`: nothing arms until the scope is validated | `a_scope_that_does_not_select_its_containers_labels_is_refused_with_no_reaper_armed` (unit, `exec/tests.rs`): an unarmed runner's cover is handed labels its scope does not select; refused, and no reaper armed — the runner's own state and the process's armed-scope observation both empty | arming before comparing (the reviewed design's rule): a reaper over the wrong scope exists, and acknowledged READY, before the refusal | `fua-m14` the comparison moved after the arming |
| `FUA-D1-CONC-1`: a stop after the `CANCEL` acknowledgement does not wedge the caller | `a_container_reaper_stopped_after_acknowledging_its_cancel_does_not_wedge_its_caller` (isolated child, bounded): a `ContainerReaper` over a stopped stand-in that has acknowledged `CANCEL` is dropped; the drop returns within two `HELPER_END_BUDGET`s and the stand-in is killed and collected | the unbounded `AcknowledgedExit` wait (the reviewed design's): the drop never returns, and the isolated child is killed at its deadline | `fua-m15` the container reaper's exit wait made `AcknowledgedExit` |

**The harness rules, each a lesson of rounds R5–R7:**

- **`R6-D2`, the relay is bound.** The stub finds its relay from its own path (`relay=${0%/*}`; the
  reaper `execv`s the absolute program, so `$0` is it), never from an environment variable. The
  `FakeRuntime` reports the stub as its `reaper_program` once a relay is installed, and a no-op
  program otherwise; every control that asserts a reaper did nothing installs the relay.
- **`R6-D3`, an isolated writer.** The stub is written by `/bin/sh -c 'printf %s "$2" > "$1"'` in a
  process of its own, bounded through `agent::proc::test_support::run_with_timeout`, its mode set by
  path afterwards — in `src/runner/container/fake.rs`, not in `src/workspace_manager/fixture.rs`
  (follow-up B's path, where round R6 put it). (As built, the writer is in `container.rs`'s
  `cfg(test)` tail, since `fake.rs` forbids `std::process::Command`: §2.1.)
- **Bounded "not held" waits.** Every read that a hold is released polls within the module's bound,
  as `holds_nothing` does (`coordinator.rs`, from `:8679`); the census row above keeps it so.
- **Isolated children are bounded.** Every child process — the two-process coordinators and the
  isolated callers — runs under a deadline and is killed after it, so a mutation fails the test
  instead of wedging the run (round R6's two wedged first attempts); children that a mutation could
  leave waiting run completing doubles, not holding ones; a child that must be observed alive waits
  for the parent's word before it exits (round R6's `exit_on_the_parents_word`); and a wait for
  running containers waits on the fake's running listing, not on its journal's `Start` entries (round
  R5's first contention run: 111 of 200 failed on that race, record `:5250`–`:5252`).

### 1.6 Platforms and budgets

- **Windows: nothing new.** There is no reaper (ST-16 (e); `os_matrix`), `arm_container_reaper` is
  `cfg(unix)`, and on Windows `cover` only counts. Every new test, the census included, is
  `#[cfg(unix)]`, so the Windows guest's harness gains no test: it took 468.44 s at `78f99c70`
  (`~/orch-pr11/logs/pr11_repair_r8/ci/ci-read-36861160153.txt`), and the hosted Windows leg of the
  merge queue, whose limit is 45 minutes (`.github/workflows/ci.yml:148`), took 32.0 minutes for #327's
  entry (`~/orch-pr11/logs/pr11_fua_design/measure/queue-run-36866629647-jobs.tsv`). The existing
  container tests that run on Windows over the fake keep their behaviour there.
- **Linux and macOS: one fork per runner.** Each `ContainerRunner`'s first launch forks one reaper and
  its drop cancels it; the test tree constructs a `ContainerRunner` at 23 sites
  (`launch-callers-92c4ca81.txt`), one of them the frozen `recover/tests.rs` helper
  (`production_container_runner`, `:9228`) that six of its tests call. The new tests add about ten
  two-process or isolated tests: round R5's two-process witness ran in a median 1.18 s (record
  `:5247`), and macOS's suite ran 2.8 times as long as Ubuntu's at `78f99c70` (1,090.99 s against
  393.23 s, `ci-read-36861160153.txt`), so every bound is the module's watchdog bound, not the
  round-7 witness's two seconds.
- **The macOS reaper.** The same code: it forks with `setpgid(0, 0)`, ignores `SIGINT`, `SIGTERM`,
  `SIGHUP`, `SIGQUIT`, `SIGTSTP`, `SIGCONT` and `SIGPIPE` (`proc.rs:2637`, `:2650`–`:2659`), and notices
  its coordinator's death through `getppid()` on every 10 ms poll (`proc.rs:2729`) — on macOS the
  helpers' channels are FIFOs, on which "`poll` … never reports the writer's close"
  (`design/15_design_event_log_resume_run_layout.md:66`). **`SIGHUP`:** the macOS finding
  `PR326-MACOS-A-DAEMONIZING-DESCENDANT-HANGS-UP-THE-ROLE` is the kernel's hang-up of a newly orphaned
  process group holding a stopped member — a host role's group with its stopped anchor. A container
  reaper registers no group and forks no anchor (`pgid` 0 never reaches `spawn_group_anchor`), and its
  own group holds only itself, running and ignoring `SIGHUP`, so it cannot trip that finding. The one
  stopped container reaper is the `R6-D1` witness's, which relies on the kernel continuing the orphaned
  stopped group once its caller is gone, on both Unix legs.
- **Docker.** macOS CI has none, so the fake and the stub carry every test; real Docker is G7's.

### 1.7 Instruments

One instrument file changes, `effects/wrappers.toml`, in the `src/agent/proc.rs` module only:

| row | change | why, in one sentence |
|---|---|---|
| `funnel` | add `"arm_container_reaper"` | It takes `ProcessSite::Terminate` by value, refuses any other site, and forks one cleanup reaper through `spawn_reaper` — the Terminate site's own fork, the class `begin` is in. |
| `shared` | `drop = 5` → `drop = 6` (and the `"drop"` comment's "five `Drop` impls" → "six") | `ContainerReaper`'s `Drop` is a sixth callable named `drop` in the file; it cancels the reaper as `Supervisor`'s does, which the `effectful_unnameable` row `"drop"` already classifies. |

**No new effect site or primitive.** Arming forks only through `spawn_reaper` at `Process.Terminate`,
the site `begin` names; the reaper's `docker` calls are the existing fork-side code
(`list_labeled_containers`, `spawn_docker`). Round R5 measured the same two rows against the effects
census: without the funnel row `every_externally_reachable_fn_of_a_legacy_or_shared_module_is_classified`
fails, with `drop = 5` `every_name_more_than_one_callable_bears_is_pinned_by_its_count` fails, and with
both the site, bijection and ST-07 coverage tests pass unchanged (record `:5158`–`:5190`); the
implementer repeats those two mutations.

**No other instrument.** `ContainerRuntime::reaper_program` is a trait method with a default body in
`src/runner/container/runtime.rs`, which is not a `CLASSIFIED_MODULES` file (`src/effects.rs:1365`), so
it needs no row; `DockerCli` (`container.rs:957`) inherits the default, `DOCKER_PROGRAM`
(`container.rs:954`) — the program it runs itself — and gains no fn. `exec.rs` is not a
`CLASSIFIED_MODULES` file either, so `Reaping`, `Covered` and their `Drop`s need no row. No new
source file, so the container module set `the_view_directory_has_one_definition_in_the_tree` pins and
`CLASSIFIED_MODULES` are unchanged; no `clippy.toml`, `effects/allowlist.toml`, `effects/*.json`,
`src/effects/**`, `.github/**`, `scripts/**`, `Cargo.toml` or frozen file. The census of §1.4 is, by
`MAINTAINING.md` step 7's reading, a subject: like `only_the_line_builder_introduces_terminal_layout`,
the named example, it asserts a fact about the product — every container start is covered — and
reaches nothing outside the launch funnel's own rule. Whether the change as a whole has the standing
form is the orchestrator's classification; the two rows above are an instrument edit, declared in the
body.

### 1.8 Out of scope, and the risks

**Out of scope.**
- A Windows reaper or portable watchdog (deferred by the packet; ST-16 (e)'s documented window).
- Real Docker (G7), and PR12's production wiring: building the `ContainerRunner` from the run's
  record (the recorded private root and this incarnation) and using one runner per incarnation.
- Follow-up B's registry race, and anything on its paths.
- Pausing a stopped coordinator's containers: a container reaper mirrors no `SIGSTOP`; nothing in the
  packet asks it to.
- Removing the process-global `set_container_reclaim_scope` and host reapers' container half: their
  callers stay tests, and removing a classified fn is an instrument edit of its own.
- Re-arming a reaper killed by another process: outside the contract, as an externally killed host
  reaper is; the next census reclaims.

**Risks.**
- **The R28 reading (§1.3) is a decision a reviewer may contest.** R28's row says "one per reaper",
  and round R5's container reaper held one. The evidence for holding none is the frozen
  `recover/tests.rs` test that a held lease would fail, the packet's own liveness and convergence
  rules, and the property's last clause becoming structural; the cost is a successor that may race
  the reaper and end once on "removal already in progress". If the review overrules it, the lease
  would have to be released when the incarnation's run lock is released, which needs a new reaper
  frame and a second handle method — more mechanism, which is what rounds R5–R7 warn against.
- **The disarm's acknowledgement budget.** `Reaper::cancel` waits two seconds for the reaper's
  acknowledgement and otherwise fails closed (`proc.rs:2355`–`:2361`), and in this design every
  `ContainerRunner` that launched cancels once when it is dropped — many times per suite, where host
  reapers are cancelled only on a failed spawn. A reaper not scheduled for two seconds would end its
  process with `SIGTERM`: a whole test binary, under load. The fail-closed arm is not what keeps this
  reaper safe: the parent keeps a read end of the command pipe (`_command_keepalive_fd`,
  `proc.rs:2609`), so the five-byte `CANCEL` frame is always in the pipe before any end-of-file, and a
  lease-free reaper that reads it late exits without listing. So the review may prefer a
  container-reaper cancel that, on a missed acknowledgement, leaves the reaper to exit on its own
  rather than ending the process. The design keeps the fail-closed arm because `R6-D1` asks for it and
  every reaper of the module has it, and records the alternative here.
- **A test double without a `reaper_program` override.** The default names `docker`; on this box
  `docker` is on `PATH`, so a double that forgets the override arms a reaper over the real CLI here
  and is refused at its first launch only on macOS CI. Four doubles back a `ContainerRunner` today
  (`FakeRuntime`, `LinkedRuntime`, `exec/tests.rs`'s `Runtime`, `create/tests.rs`'s `Inventory`) and
  each must override; the macOS leg is the backstop.
- **An unsettled runner in a long-lived process** keeps one helper process alive until the process
  exits — by design, since its containers may run. Four frozen `recover/tests.rs` tests end with a
  container `Unresolved` (from `:9278`, `:9393`, `:9456`, `:9788`); each leaves a reaper over the
  fake's no-op program until the test binary exits, holding no lease and touching nothing.
- **Existing tests that kill a coordinator** and say "its three containers outlive it" keep passing,
  because the fake's default program kills nothing; their messages should say that it is the fake's
  no-op reaper, not the absence of one.
- **A `docker` CLI wedged at the coordinator's death holds the reaper with no deadline** (corrected,
  `FUA-D1-REG-1`; the sentence as reviewed said each call was bounded at 30 seconds, which the
  regression lens showed false). The reaper's `docker` calls are PR6's fork-side machinery,
  inherited unchanged: a listing is read for up to 30 seconds (`read_bounded`, `proc.rs:4889`,
  `:4934`; `REAPER_DOCKER_TICKS`), its process is then waited for for up to 30 seconds more
  (`reap_bounded`, `proc.rs:4891`, `:4975`), sent `SIGKILL` (`:4987`), and then waited for by a
  `waitpid(pid, 0)` with **no deadline** (`:4990`). A CLI that `SIGKILL` cannot make collectable — one
  in uninterruptible sleep — holds the reaper there indefinitely; the dead coordinator's containers
  then run on until the next write command's census, the orphan window this follow-up closes,
  reopened for that case. It is not bounded here: the loop is shared by every reaper's container
  half, and bounding it, and witnessing a process `SIGKILL` cannot end, is work of its own. It is
  filed as `PR328-REAPER-DOCKER-WAIT-HAS-NO-DEADLINE-AFTER-SIGKILL` (P3, `pre_existing`, first bad
  `919a728e`, PR6 lane c), with this sequence as its evidence.
- **The macOS and Windows legs** are CI's to speak for (§1.6); this box runs Linux only.

### 1.9 What the implementation changes, by file

- `src/agent/proc.rs`: `arm_container_reaper`, `ContainerReaper` and its `Drop`, the `cfg(unix)`
  re-export; `spawn_reaper` takes the container scope and the lease set as arguments; tests.
- `src/runner/container/exec.rs`: `Reaping`, `Covered`, the cover in `contain`, `launch`'s new
  parameter; tests in `exec/tests.rs`, the census among them.
- `src/runner/container/runtime.rs`: `ContainerRuntime::reaper_program` with its default body.
- `src/runner/container/fake.rs` (test-only): the relay, its isolated writer, the overrides for
  `FakeRuntime` and `LinkedRuntime`, and the armed-scope observation.
- Tests in `src/engine/topology/coordinator.rs`, `create/tests.rs`, `preflight/tests.rs`,
  `src/agent/proc/tests.rs`, `src/runner/host/tests.rs` (the FIFO oracle shared), and the overrides in
  the two test doubles above.
- `effects/wrappers.toml` (§1.7), the notes of every module whose items change
  (`docs/internals/agent/proc.md`, `docs/internals/runner/container/exec.md`, `runtime.md`,
  `census.md`'s "What a later slice must connect", `container.md`'s `OrphanWindow`, `fake.md`), one
  sentence in `design/15_design_event_log_resume_run_layout.md`'s crash-containment paragraph (not
  follow-up B's lock text), this record, and the deletion of the finding's file.
- Not changed: `src/engine/topology/**` production code, `recover.rs`, `recover/tests.rs`, every
  frozen file, `src/rundir.rs`, `src/workspace_manager*`.

### 1.10 The design review's four corrections (round 1, at `793c3784`)

Three `gpt-6-astra` lenses at `max` reviewed this section at `793c3784`
(`~/orch-pr11/reviews/review-328-d1-{design,concurrency,regression}-793c3784.review.md`, hashes in
`SHA256SUMS-328-d1`; the orchestrator's triage `review-328-d1-triage.md`). The core — the reaper armed
inside the launch funnel — held in all three; each returned one or two P2s, all corrections of this
section, made above before any code and marked with their ids:

| id | lens | the correction | where |
|---|---|---|---|
| `FUA-D1-DES-1` | design | the census reads namings — aliases, function values, re-exports — of every unarmed launcher, not call tokens; why visibility cannot do it here | §1.4; witnesses in §1.5 |
| `FUA-D1-DES-2` | design | the cover compares the scope it would hold with the container's labels before it forks; nothing arms until the scope is validated | §1.2, §1.3; witness in §1.5 |
| `FUA-D1-CONC-1` | concurrency | the container reaper's exit after its `CANCEL` acknowledgement is waited for with a deadline, then killed; no wait of its life is unbounded | §1.3; witness in §1.5 |
| `FUA-D1-REG-1` | regression | the risk account states the inherited limit — a deadline-less `waitpid` after `SIGKILL` — and files it rather than claiming a 30-second bound | §1.8 |

## 2. Implementation

Phase 2, by `pr11_fua_impl` (`claude-opus-5-5`, `max`). Every figure below is in a saved file under
`~/orch-pr11/logs/pr11_fua_impl/` that the sentence names. Code lines are cited at the code commit
`4bc56d21` unless another is named; the later commits of this phase change no production source
file, and only `cdd72177` changes a test (the census, in `exec/tests.rs`).

### 2.0 The order of the work

1. **§1 amended before any code** (`6d565f42`): the four corrections, marked with their ids
   (§1.10).
2. **The implementation and its tests** (`4bc56d21`), and the two `effects/wrappers.toml` rows.
3. **Notes, `design/15`, the findings** (`a8ef3fb0`).
4. **The census's primitives check corrected** (`cdd72177`; §2.2), §1.4 with it.
5. **This section**, after the mutation campaign ran at `cdd72177`.

### 2.1 What changed, by file

**`src/agent/proc.rs`** (the Process funnel's `termination` module):

- `fork_reaper(leases, containers)` (`:2559`) is the fork every reaper is made by, split out of
  `spawn_reaper` unchanged; `spawn_reaper` (`:2548`) keeps its signature and its order —
  `verify_group_scanner`, the thread's lease scope merged with the carried paths, the process-wide
  container scope — and hands them to it, so a host reaper is forked as before.
- `pub struct ContainerReaper` and `pub fn arm_container_reaper(terminate_site, scope)`
  (`:4868`, `:4872`), re-exported from `agent::proc` under `cfg(unix)` (`:837`): refuses any site
  but `Process.Terminate`; renders the scope before anything else; `shared_state()?` installs the
  signal monitor (`R6-D1`); forks one reaper through `fork_reaper` with **no** lease and the
  rendered scope, without the scanner check, a launch claim or a `REGISTER` — the reaper's `pgid`
  stays 0.
- `impl Drop for ContainerReaper` (`:4895`) calls `Reaper::cancel_unleased` (`:2377`): the same
  CANCEL frame and two-second acknowledgement as `cancel`, fail-closed when it does not come, and
  then `ReaperEnding::UnleasedExit` (`:2545`): the exit waited for within `HELPER_END_BUDGET`, then
  `SIGKILL`, then once more within the same budget (`:2438`, `:2478`) — `FUA-D1-CONC-1`. Host
  reapers keep `cancel` and its unbounded `AcknowledgedExit` wait.
- Test-only: `ARMED_CONTAINER_REAPERS` (`:4906`) records each armed reaper's listing argv until it
  is cancelled; `armed_container_reaper_selects` answers whether one lists a private root label and
  incarnation (the coverage observation, §2.3). The module's tests gain the four isolated-child
  witnesses and their children (§2.3).

**`src/runner/container/exec.rs`** (the launch funnel):

- `ContainerRunner` gains `reaping: Reaping` (`:268`): a mutex over the armed reaper and its scope,
  the covers in flight, and a sticky `unsettled`.
- `Reaping::cover(runtime, identity, labels)` (`:290`): takes the armed scope, or builds the one it
  would arm from `runtime.reaper_program()` and the runner's `RunIdentity`; refuses, through
  `refuse_unless_selected` (`:324`), a container whose two labels that scope does not select —
  **before** arming (`FUA-D1-DES-2`); arms on the first cover (`arm_container_reaper`, Unix only);
  counts the invocation; returns `Covered`, constructed nowhere else (`:285`).
- `contain` covers after the plan and before the launch (`:961`); `launch` takes `&Covered`
  (`:670`); every return path after the cover settles it with the fate the runner established
  (the launch's error fate, or the release's, `:1004`).
- `Covered`'s drop (`:350`) counts down and marks the runner unsettled unless the fate was
  established `Gone` or `NeverStarted` (an unwinding invocation has none). `Reaping`'s drop
  (`:374`) cancels the reaper only when nothing is in flight and nothing is unsettled, and
  otherwise keeps it armed until the process exits (`Armed::kept_until_the_process_exits`,
  `mem::forget` of the reaper on Unix) — `R6-C1`.
- On Windows the cover validates the labels the same way and arms nothing (no reaper, ST-16 (e)).

**`src/runner/container/runtime.rs`**: `ContainerRuntime::reaper_program` (`:280`), defaulting to
`DOCKER_PROGRAM`, which `DockerCli` inherits. **`src/runner/container/census.rs`**:
`ReaperContainerScope::selects(labels)` (`:818`).

**Test support** — `src/runner/container/fake.rs`: the fake's `reaper_program` is its relay stub once
one is installed and `/usr/bin/true` otherwise (`NO_OP_REAPER_PROGRAM`); the relay stub finds its
relay from `$0` and answers `ps` by the labels in its listing, as the daemon would; the linked
runtime asks the parent's fake for its reaper program over the link (`REAPER_PROGRAM_QUERY`), so a
two-process witness installs its relay once, on the fake it serves; `observing_covers` records, at
each container start, whether an armed reaper selects it. `src/runner/container.rs`'s
`cfg(test)` tail: `write_program_in_its_own_process` (the isolated writer, `R6-D3`; here and not in
`fake.rs` because the fake forbids `std::process::Command`), `run_program_in_its_own_process`
(the relay's self-check), and the `NO_OP_REAPER_PROGRAM` re-export. The two doubles outside the fake
that back a `ContainerRunner` override `reaper_program`: `exec/tests.rs`'s `Runtime` (`:216`) and
`create/tests.rs`'s `Inventory` (`:2894`).

**Tests** in `src/runner/container/exec/tests.rs`, `src/agent/proc.rs`,
`src/engine/topology/coordinator.rs`, `src/engine/topology/create/tests.rs` and
`src/runner/host/tests.rs` (the FIFO oracle shared with the relay writer); two existing coordinator
assertions that a killed coordinator's containers "outlive it" now say why — the fake's no-op reaper
program.

**Not changed:** every frozen file (§2.6), `src/engine/topology/**` production code, `recover.rs`,
`recover/tests.rs`, `src/rundir.rs`, `src/workspace_manager*`, `clippy.toml`, `src/effects/**`,
`Cargo.toml`. `design/15`'s crash-containment paragraph gains one sentence; the notes of all ten
noted modules this phase changed carry the prose (§13).

**Where the implementation departs from §1's text, and why:**

- `spawn_reaper` keeps its signature (§1.3 said it would take the scope and lease set as
  arguments); the shared fork is `fork_reaper`, which takes them. Same effect, smaller diff for the
  host path.
- The container reaper skips `verify_group_scanner`: it registers no group, so the scanner is never
  consulted, and the check could only add a way to refuse.
- The relay writer is in `container.rs`'s test tail, not `fake.rs` (§1.5's harness rules), because
  the fake forbids `std::process::Command`; the stub filters `ps` by labels rather than answering a
  listing the parent pre-filters, so a scope that does not select a container does not find it.
- The child coordinator gets the relay through the link (the parent's fake answers its reaper
  program), not through an environment variable: the relay's binding (`R6-D2`) is then structural in
  both kinds of witness.
- The panic witness (`a_panic_inside_contain_leaves_the_reaper_armed`) runs in-process and reads the
  armed-scope observation after the runner's drop, rather than in an isolated child: a reaper kept
  armed in the test binary is the state it asserts, and it holds no lease and runs the fake's no-op
  program. `R6-C1`'s unit witnesses do the same; its end-to-end witness is two-process.
- The design property is witnessed for creation's P4 probes in `create/tests.rs` as well as for a
  resume's pre-flight, `run_concurrently` and `step` in `coordinator.rs`.
- Some witnesses are named differently from §1.5's table; §2.3 maps each requirement to the test as
  built.

### 2.2 The domination census

`every_container_start_in_production_is_reached_only_through_a_covered_launch`
(`src/runner/container/exec/tests.rs`, census commit `cdd72177`) reads every production region of
`src/` (`effects::production_code`, the whole-file test modules skipped) and runs §1.4's five
checks, as amended (`FUA-D1-DES-1`):

1. the namings — calls, imports, aliases, function values, re-exports — of `create_container` and
   `start_container`: exactly `exec.rs`'s one import of each and one call of each inside
   `ContainerRunner::launch`, and `container.rs`'s one call of each inside the free `launch`;
2. the free `launch`: no production naming at all;
3. the runtime's primitives: in every production module whose effective `clippy::disallowed_methods`
   level is `allow` — stated, or inherited from a parent that states it; twenty-seven modules
   today — each call or path naming `create` or `start` is one of seventeen pinned namings, two of
   them `runtime.create(` and `runtime.start(` inside the funnels and none of the rest a container
   runtime's; `RuntimeOp::Create`/`::Start` only inside `DockerCli`'s own `create` and `start`;
4. the covered launch's shape in `exec.rs`: `launch` takes `Covered`, `self.launch(` once, in
   `contain`, after its one `.cover(`, and `Covered {` built once, in `cover`;
5. the control: the regions naming `start_container` are exactly the two funnels' files; more than
   40 files and 750,000 non-whitespace bytes read; the lint-level walk found more than 100 modules
   guarded and places `container.rs` and `view.rs` among the rest.

Its two readers are tested on written input:
`the_naming_reader_names_an_alias_a_function_value_and_a_re_export_of_an_unarmed_launcher` and
`the_lint_level_walk_reads_a_stated_or_inherited_allowance_and_a_production_forbid`.

**Check 3 was corrected during this phase.** As first written (at `4bc56d21`) it read only
`container.rs` and `view.rs` and relied on clippy everywhere else, and the record's amended §1.4
said every other production module forbids the lint. That was false: twenty-seven production modules
allow `clippy::disallowed_methods` module-wide, so a `runtime.start(…)` planted in, say,
`src/engine/resume.rs` compiles and passes `cargo clippy --lib -- -D warnings` (`fua-p2` below).
Found by this phase's own scan of module headers before any review; repaired in `cdd72177`, with
§1.4 corrected, and the same planted call now fails the census (`fua-c8`). Where the lint is denied
or forbidden, clippy refuses a function value of the primitive as well as a call (`fua-p1`).

### 2.3 The witnesses and their mutations

Twenty-three new tests, every one `cfg(unix)` (the FIFO oracle's Linux), and six ignored children
they spawn: twenty-two and the six at `a8ef3fb0`, by the residue runs' logs against the merge base's
(`measure/new-tests-a8ef3fb0.txt`, `tools/new-tests.py`; no test removed), and the lint-level walk's
reader test, `cdd72177`'s. Each mutation is a scratch copy of the tracked tree at the census commit
`cdd72177` (`tools/campaign.py` through `tools/mutate.py`; each `base.txt` records the head, and
`tools/verify-copies.sh` shows each copy's `src/`, `effects/`, `examples/` and manifests equal to
`cdd72177`'s but for the mutated file, `mutation/copies-vs-cdd72177.txt`), built from itself — every
log's `Compiling upstroke v0.1.0 (…/mut/src/<name>)` line (`Checking` for the two clippy probes)
names its own copy — and run against the twenty-two tests of `tools/reaper-tests.txt` (every new
test but the lint-level walk's reader test), so each row's red is measured against the whole set;
`mutation/<name>/{mutation.diff,test.log,summary.txt}`, tabulated by `tools/mutation-table.py` in
`mutation/TABLE.md` (the table below) and `mutation/SUMMARY.txt`. The unmutated control passes all
twenty-two.

| id | the mutation | witness for | red | green |
|---|---|---|---|---|
| `control` | none | the set at the census commit | none | 22 |
| `fua-m1` | `arm_container_reaper` returns a handle with no reaper | `R6-C2`, `R7-D1`, the design property, `R6-C1`, `R6-D1` | 13: `a_container_reapers_failed_cancellation_ends_its_caller_through_the_signal_monitor`, `a_stopped_container_reaper_ends_its_caller_rather_than_releasing_it`, `a_resuming_incarnations_pre_flight_probe_container_is_killed_by_its_reaper_when_the_coordinator_dies_inside_it`, `a_runner_whose_containers_are_unresolved_keeps_its_reaper_armed_past_its_last_handle_until_the_process_exits`, `every_container_an_incarnation_starts_is_covered_by_an_armed_reaper_with_its_scope`, `the_reapers_scope_is_the_runners_identity_and_selects_its_first_probe`, `a_fresh_runs_p4_probe_container_is_killed_by_its_reaper_before_run_started`, `every_container_a_fresh_runs_creation_starts_is_covered_by_an_armed_reaper_with_its_scope`, `a_container_whose_labels_differ_from_the_armed_scope_is_refused_before_its_intent`, `a_panic_inside_contain_leaves_the_reaper_armed`, `a_runner_whose_container_is_unresolved_keeps_its_reaper_armed_past_its_drop`, `a_runner_whose_containers_all_ended_disarms_its_reaper_at_its_drop`, `a_scope_that_does_not_select_its_containers_labels_is_refused_with_no_reaper_armed` | 9 |
| `fua-m2` | an unarmed runner's cover skips probe invocations | `R6-C2`, `R7-D1`, `R7-D2`, the design property | 6: `a_resuming_incarnations_pre_flight_probe_container_is_killed_by_its_reaper_when_the_coordinator_dies_inside_it`, `every_container_an_incarnation_starts_is_covered_by_an_armed_reaper_with_its_scope`, `the_reapers_scope_is_the_runners_identity_and_selects_its_first_probe`, `a_fresh_runs_p4_probe_container_is_killed_by_its_reaper_before_run_started`, `every_container_a_fresh_runs_creation_starts_is_covered_by_an_armed_reaper_with_its_scope`, `every_container_start_in_production_is_reached_only_through_a_covered_launch` | 16 |
| `fua-m3` | the cover builds its scope from a stale incarnation | `R7-D2` and every container witness | 13: `a_coordinator_that_ends_disarms_its_reaper_and_kills_nothing_at_width_three`, `a_resuming_incarnations_pre_flight_probe_container_is_killed_by_its_reaper_when_the_coordinator_dies_inside_it`, `a_runner_whose_containers_all_ended_disarms_its_reaper_and_the_relay_is_never_called`, `a_runner_whose_containers_are_unresolved_keeps_its_reaper_armed_past_its_last_handle_until_the_process_exits`, `every_container_an_incarnation_starts_is_covered_by_an_armed_reaper_with_its_scope`, `the_reapers_scope_is_the_runners_identity_and_selects_its_first_probe`, `a_fresh_runs_p4_probe_container_is_killed_by_its_reaper_before_run_started`, `every_container_a_fresh_runs_creation_starts_is_covered_by_an_armed_reaper_with_its_scope`, `a_container_whose_labels_differ_from_the_armed_scope_is_refused_before_its_intent`, `a_panic_inside_contain_leaves_the_reaper_armed`, `a_runner_whose_container_is_unresolved_keeps_its_reaper_armed_past_its_drop`, `a_runner_whose_containers_all_ended_disarms_its_reaper_at_its_drop`, `a_scope_that_does_not_select_its_containers_labels_is_refused_with_no_reaper_armed` | 9 |
| `fua-m4` | `refuse_unless_selected` accepts any labels | `R7-D2`, `FUA-D1-DES-2` | 2: `a_container_whose_labels_differ_from_the_armed_scope_is_refused_before_its_intent`, `a_scope_that_does_not_select_its_containers_labels_is_refused_with_no_reaper_armed` | 20 |
| `fua-m5` | `Reaping`'s drop cancels whatever `unsettled` says | `R6-C1` | 3: `a_runner_whose_containers_are_unresolved_keeps_its_reaper_armed_past_its_last_handle_until_the_process_exits`, `a_panic_inside_contain_leaves_the_reaper_armed`, `a_runner_whose_container_is_unresolved_keeps_its_reaper_armed_past_its_drop` | 19 |
| `fua-m6` | `Covered`'s drop reads `Unresolved` as established | `R6-C1` | 2: `a_runner_whose_containers_are_unresolved_keeps_its_reaper_armed_past_its_last_handle_until_the_process_exits`, `a_runner_whose_container_is_unresolved_keeps_its_reaper_armed_past_its_drop` | 20 |
| `fua-m7` | `Covered`'s drop without a settle is not unsettled | `R6-C1` (an unwinding invocation) | 1: `a_panic_inside_contain_leaves_the_reaper_armed` | 21 |
| `fua-m8` | arming skips `shared_state()` | `R6-D1` | 2: `a_container_reapers_failed_cancellation_ends_its_caller_through_the_signal_monitor`, `a_stopped_container_reaper_ends_its_caller_rather_than_releasing_it` | 20 |
| `fua-m9` | the container fork is handed the thread's cleanup leases | the property's last clause (no R28 held) | 1: `an_armed_container_reaper_holds_no_cleanup_lease` | 21 |
| `fua-m10` | the drop closes the pipes instead of sending CANCEL | `R6-D2`, `R6-D1` | 4: `a_container_reapers_failed_cancellation_ends_its_caller_through_the_signal_monitor`, `a_stopped_container_reaper_ends_its_caller_rather_than_releasing_it`, `a_coordinator_that_ends_disarms_its_reaper_and_kills_nothing_at_width_three`, `a_runner_whose_containers_all_ended_disarms_its_reaper_and_the_relay_is_never_called` | 18 |
| `fua-m11` | the relay program written through this process's descriptor | `R6-D3` | 1: `the_reaper_relay_writer_leaves_no_writer_in_another_threads_fork` | 21 |
| `fua-m12` | one one-shot "not held" read added to the lease witness's child | requirement 8 (the bounded reads) | 1: `no_reaper_test_reads_a_hold_as_released_once` | 21 |
| `fua-m13-c1` | `contain` launches with a `Covered` built by hand, then covers | the design property; the census's check 4 (`fua-c1`) | 8: `a_resuming_incarnations_pre_flight_probe_container_is_killed_by_its_reaper_when_the_coordinator_dies_inside_it`, `every_container_an_incarnation_starts_is_covered_by_an_armed_reaper_with_its_scope`, `the_reapers_scope_is_the_runners_identity_and_selects_its_first_probe`, `a_fresh_runs_p4_probe_container_is_killed_by_its_reaper_before_run_started`, `every_container_a_fresh_runs_creation_starts_is_covered_by_an_armed_reaper_with_its_scope`, `a_container_whose_labels_differ_from_the_armed_scope_is_refused_before_its_intent`, `a_panic_inside_contain_leaves_the_reaper_armed`, `every_container_start_in_production_is_reached_only_through_a_covered_launch` | 14 |
| `fua-m14` | the scope compared with the labels after the arming | `FUA-D1-DES-2` | 1: `a_scope_that_does_not_select_its_containers_labels_is_refused_with_no_reaper_armed` | 21 |
| `fua-m15` | the exit wait after the acknowledgement made `AcknowledgedExit` | `FUA-D1-CONC-1` | 1: `a_container_reaper_stopped_after_acknowledging_its_cancel_does_not_wedge_its_caller` | 21 |
| `fua-c2` | a production fn in `exec.rs` calls `start_container(` | the census, check 1 | 1: `every_container_start_in_production_is_reached_only_through_a_covered_launch` | 21 |
| `fua-c3` | `preflight.rs` calls `crate::runner::container::launch(` | the census, check 2 | 1: `every_container_start_in_production_is_reached_only_through_a_covered_launch` | 21 |
| `fua-c4` | `Covered {` built outside `cover` | the census, check 4 | 1: `every_container_start_in_production_is_reached_only_through_a_covered_launch` | 21 |
| `fua-c5` | `use super::launch as start_uncovered;` and a call of it | `FUA-D1-DES-1`: an alias | 1: `every_container_start_in_production_is_reached_only_through_a_covered_launch` | 21 |
| `fua-c6` | `let start = super::start_container;` and a call of it | `FUA-D1-DES-1`: a function value | 1: `every_container_start_in_production_is_reached_only_through_a_covered_launch` | 21 |
| `fua-c7` | `pub use super::create_container as create_uncovered;` in `census.rs` | `FUA-D1-DES-1`: a re-export | 1: `every_container_start_in_production_is_reached_only_through_a_covered_launch` | 21 |
| `fua-c8` | `runtime.start("probe")` in `engine/resume.rs`, which allows `disallowed_methods` | the census, check 3 | 1: `every_container_start_in_production_is_reached_only_through_a_covered_launch` | 21 |
| `fua-r0` | none (the two effects tests as committed) | the rows' control | none | 2 |
| `fua-r1` | `"arm_container_reaper"` removed from the funnel row | the funnel row | 1: `every_externally_reachable_fn_of_a_legacy_or_shared_module_is_classified` | 1 |
| `fua-r2` | `drop = 6` back to `drop = 5` | the `drop` count | 1: `every_name_more_than_one_callable_bears_is_pinned_by_its_count` | 1 |
| `fua-p1` | `let start = R::start;` in `census.rs`, clippy on the lib | census check 3's reliance on clippy where the lint is denied | clippy rc 101: use of a disallowed method `upstroke::runner::container::runtime::ContainerRuntime::start` at `src/runner/container/census.rs:21:17` | — |
| `fua-p2` | `fua-c8`'s planted call, clippy on the lib | why check 3 reads the modules that allow the lint | clippy rc 0: no disallowed-method error | — |

**Every mutation turned red the witness its row names**, and each copy differed from `cdd72177` in
exactly its mutated file (`control` and `fua-r0` in none; `mutation/copies-vs-cdd72177.txt`). The
red column counts the set's tests that failed and names them; the green column counts those that
passed. `fua-r0`–`fua-r2` ran the two effects tests that read `effects/wrappers.toml` (§2.8) rather
than the set. `fua-p1` and `fua-p2` ran `cargo clippy --lib --all-features -- -D warnings` on their
copies, not tests: clippy refuses a function value of the runtime's `start` where the lint is denied,
and passes `fua-c8`'s planted call where it is allowed — why check 3 reads those modules itself.
The census mutants `fua-c2`–`fua-c8` and `fua-m13-c1` each turned the
census red, its message naming the planted naming (`fua-c2`, `-c3`, `-c5`–`-c8`) or the broken shape
(`fua-c4`, `fua-m13-c1`) (`mutation/<name>/test.log`).

**The two-process witnesses** run a child coordinator over the parent's fake (`scaffold::Served`,
`ParentSide`), install the relay on the fake the child is served by — the child's runner asks the
parent's fake for its reaper program over the link, so nothing passes it in the environment — and,
before killing the child, publish the fake's containers with their labels to the relay's listing;
the stub answers `ps` by those labels as the daemon would. A dead coordinator's reaper notices the
death within one of its 10-millisecond polls (`reaper_loop`); each witness waits for its calls
within `REAPER_BOUND` (60 s) and delivers them to the fake as the actor `reaper`, so a container is
gone only by the reaper's own `kill` and `rm`, and no census listing occurred after the death. A
child's events and exit are awaited within `workspace_manager::fixture::LINK_BOUND` (120 s); a child
that must be seen alive past its last handle waits for a file the parent writes (round R6's lesson);
every isolated child of `agent::proc` runs under `run_isolated`'s deadline and is killed with its
process group at it, so a mutation fails rather than wedges (`fua-m15`'s red is exactly that kill:
its child was still in the unbounded wait at the deadline, `status None`).

**Repeats.** The twenty-two tests, thirty consecutive runs at `a8ef3fb0` while the first campaign
loaded the box: thirty of thirty green, about 2.1 s each (`proof/repeat-under-campaign/summary.txt`).

### 2.4 Class searches

Saved in `measure/class-searches.txt` (at `4bc56d21`; `cdd72177` changed only the census's reading):

- **Every runtime that backs a `ContainerRunner` names its reaper program.** Of the twelve
  `ContainerRuntime` implementations, five back a runner: the real `DockerCli` (the trait's `docker`
  default, correctly), `FakeRuntime` and `LinkedRuntime` (`fake.rs`), `exec/tests.rs`'s `Runtime` and
  `create/tests.rs`'s `Inventory`; the four test doubles override `reaper_program`. The other seven
  (`prelock/tests.rs`'s `Inventory`, `startup/tests.rs`'s two, `census/tests.rs`'s `WedgedRuntime`,
  `resolve/tests.rs`'s `LoggingRuntime`, `container/tests.rs`'s two) never back a runner, so their
  default is never asked for. Every `ContainerRunner::new` site and every `scaffold::container_runner`
  caller is listed with the runtime it is handed (24 and 8 sites): each is one of the five.
- **Every wait of the container reaper's life has a deadline**: READY (`await_ready`,
  `HELPER_READY_BUDGET`), the CANCEL acknowledgement (two seconds), the exit after it
  (`UnleasedExit`'s two bounded waits); the fail-closed arm waits for nothing. The one deadline-less
  wait left is inside the reaper, in PR6's inherited `docker` machinery — filed (§1.8,
  `PR328-REAPER-DOCKER-WAIT-HAS-NO-DEADLINE-AFTER-SIGKILL`).
- **Write-then-exec fixtures this change adds**: one, the relay stub, through the isolated writer.
- **"Not held" reads in the new tests**: every one is a bounded poll; the census
  `no_reaper_test_reads_a_hold_as_released_once` keeps it so.
- **Unarmed paths to a container start**: none outside the funnels — the census (§2.2), whose
  primitives check covers every module where clippy cannot refuse them.
- **Modules where a lint the census relies on is lowered**: the module-header scan that found the
  twenty-seven `clippy::disallowed_methods` allowances (§2.2) is the census's own walk; it is the
  class search for the defect check 3 had.

### 2.5 Residue

The whole suite in a fresh, short `TMPDIR`, at the merge base `92c4ca81` and at `a8ef3fb0` (the code
of this phase before the census commit, which reads source only), each from a `git archive` copy
(`tools/residue-run.sh`, `residue/{base,head}/`, compared by class with ULIDs, ten-character random
suffixes and digits normalized by `tools/residue-compare.py`, `residue/compare-base-head.txt`):
44 top-level entries and 154 in all at both, **no class differs**. Each run's 60-second listing
shows the suite allocating there. The head run passed 3,022 tests, the base 3,000.

### 2.6 Frozen files, schema 4 and the legacy path

- **The module diff proof passes** at `4bc56d21` against the merge base `92c4ca81`
  (`proof/module-diff-proof-4bc56d21.txt`): the 26 frozen production files byte-identical, and all
  eight frozen test children identical, `recover/tests.rs` among them. G6's "fold, queue, merge,
  repair, and recovery modules byte-identical to the G5 range" names none of the files this change
  touches.
- **Schema 4 stays unreachable in production**: no production code constructs a `ContainerRunner`
  (§1.1), and nothing in `engine::topology`'s production code changed.
- **The legacy path and host runs are unchanged**: the legacy engine's runner is the host's; a host
  launch's reaper is forked by `spawn_reaper` exactly as before — the same scanner check, leases
  and process-wide scope, handed to the split-out `fork_reaper` — and `Reaper::cancel` keeps its
  unbounded acknowledged-exit wait (`the_acknowledged_exit_wait_after_cleanup_or_cancel_is_still_unbounded`
  passes in the full suite).

### 2.7 Platforms

- **Linux** (this box): every gate, the full suite, the campaign and the repeats.
- **macOS**: the same code. `clippy --target aarch64-apple-darwin --all-targets` is clean
  (`dev/clippy-aarch64-apple-darwin-2.log`); the tests are CI's. The reaper ignores `SIGHUP` and
  registers no group, so it cannot trip `PR326-MACOS-A-DAEMONIZING-DESCENDANT-HANGS-UP-THE-ROLE`;
  the one stopped reaper is `a_stopped_container_reaper_ends_its_caller_rather_than_releasing_it`'s,
  continued by the kernel once its caller is gone or, if it is still stopped five seconds later,
  by the test. `/usr/bin/true` — the fake's no-op reaper program — exists on macOS. The relay stub
  uses `/bin/sh`, `printf` and `mv`. The FIFO oracle is Linux-only (`F_SETPIPE_SZ`), as before.
- **Windows**: no reaper (ST-16 (e)); the cover validates labels and arms nothing. Every new test is
  `cfg(unix)`, so the guest harness gains no test. `clippy --target x86_64-pc-windows-msvc
  --all-targets` and `cargo +1.85.0 check --target x86_64-pc-windows-msvc --locked --all-targets`
  are clean (`dev/clippy-x86_64-pc-windows-msvc-2.log`, `dev/msrv-windows-1.log`) — type-checked
  and linted, not run; the first Windows clippy run found two `cfg(windows)`-only defects
  (`dev/clippy-x86_64-pc-windows-msvc-1.log`), fixed before the commit. The guest's time is CI's
  (§2.9).
- **Docker**: real Docker is G7's; the fake and the relay carry every test here.

### 2.8 Instruments

`effects/wrappers.toml`, the `src/agent/proc.rs` module only:

| row | change | why |
|---|---|---|
| `funnel` | `"arm_container_reaper"` | It takes `ProcessSite::Terminate` by value, refuses any other site, and forks one cleanup reaper through the Terminate site's own fork, with no lease and no process group, for a container runner's incarnation. |
| `shared` | `drop = 5` → `drop = 6` (and the `"drop"` comment's "five" → "six") | `ContainerReaper`'s `Drop` is a sixth callable named `drop` in the file; it cancels the reaper as `Supervisor`'s does, with a bounded wait for its exit. |

Both are required, measured on scratch copies of `cdd72177` (§2.3's method), each run against the two
effects tests that read them: without the funnel row,
`every_externally_reachable_fn_of_a_legacy_or_shared_module_is_classified` fails with
`src/agent/proc.rs` `unclassified: ["arm_container_reaper"]` (`fua-r1`); with `drop = 5`,
`every_name_more_than_one_callable_bears_is_pinned_by_its_count` fails: six callables bear `drop` in
`src/agent/proc.rs` and the record pins five (`fua-r2`); as committed, both pass (`fua-r0`)
(`mutation/fua-r{0,1,2}-*/test.log`). Nothing else in any instrument changes: no new effect
site or primitive, no `clippy.toml`, `src/effects/**`, `Cargo.toml [lints]` or allowlist. The
census of §2.2 is, by `MAINTAINING.md` step 7's reading, a subject: it asserts a fact about the
product — every container start is covered — and governs nothing outside the launch funnel's own
rule.

### 2.9 Gates and CI

The ten gates run at the pushed head, in the foreground, cargo through `upstroke-build` on this
session's base; their logs, and CI's legs read by the newest run per workflow after the push, are in
the pull request body and `~/orch-pr11/handovers/pr11_fua_impl.md`, which name each file. This
section is committed before they run, so it names no figure of theirs.

### 2.10 What is not verified here

- macOS and Windows behaviour: CI's (§2.9).
- Real Docker: G7's.
- The 30-second bounds of the inherited `docker` calls and the deadline-less wait after them were
  not exercised: filed, not changed.
