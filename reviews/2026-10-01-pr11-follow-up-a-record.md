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
- **Implementation review, round 1: §3**, at `17d7c605`. Three `gpt-6-astra` lenses at `max`: delta
  and regression one P2 each, both executed; concurrency passed. `pr11_fua_r1` (`claude-opus-5-5`,
  `max`) repaired both: the free `container::launch` is test-only and the census reads macro
  arguments (`FUA-I1-MACRO`); the test watchdog collects its killed child within a bound
  (`FUA-I1-WATCHDOG`); the sweep's one pre-existing instance is filed.
- **Implementation review, round 2: §4**, at `8d0d8b86`. Three `gpt-6-astra` lenses at `max`: regression
  passed; delta and concurrency one P2 each, both executed. `pr11_fua_r2` (`claude-opus-5-5`, `max`) repaired both
  (amended, `FUA-I2-MACRO-WS`, `FUA-I2-EINTR`): every container start primitive takes a proof only the reaper's
  cover mints, so a start with no cover is a compile error and the census a lexical backstop; a container
  reaper's bounded endings rest once between their polls, so an interrupted rest goes back to the deadline.
- **Implementation review, round 3: §5**, at `71e55dfc`. Three `gpt-6-astra` lenses at `max`: delta and concurrency
  passed; regression found one P2, executed. `pr11_fua_r3` (`claude-opus-5-5`, `max`) repaired it (`FUA-I3-HOSTCANCEL`).
  The rest-selection test now forks and ends its real reapers in an isolated child under `run_isolated`, so a host
  reaper stopped after acknowledging CANCEL fails the test at its deadline instead of wedging the suite. The change is
  test-only.
- **Next:** implementation review round 4 (delta and regression: the change is test-only, and concurrency passed at
  `71e55dfc`), and the rounds `MAINTAINING.md` prescribes. G6's input range must include this follow-up's merge.

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
production path to `create_container` and `start_container`. (Amended, `FUA-I2-MACRO-WS`: since review round 2 the
type system enforces that too. The funnels and the runtime's `create` and `start` take proofs only the cover mints,
and the census is a lexical backstop: §4.1.)

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
  acknowledgement (two seconds), the exit (two `HELPER_END_BUDGET`s). (Amended, `FUA-I2-EINTR`: a deadline
  checked between polls holds only if the pause between them returns, and `thread::sleep` does not return under
  a stream of interruptions; since review round 2 a container reaper rests once between its polls, §4.2.) Host reapers keep
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

1. **The funnels.** (Amended, `FUA-I1-MACRO`: since review round 1 the free `launch` is test-only, so
   `container.rs`'s two calls below left the production region, §3.1.) The production namings of
   `create_container` and `start_container` are exactly:
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
5. **The control.** (Amended, `FUA-I1-MACRO`: since review round 1, `exec.rs` alone, §3.1.) The sorted list of
   production regions naming `start_container` is exactly
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
So the census is the guard, not a backstop, and it is written to refuse every naming. (Amended,
`FUA-I1-MACRO`: review round 1 found a naming this census could not read — a launcher passed to a macro as
`launch: …`, skipped as a field initializer — and took the visibility route as far as it goes. With the
orchestrator's authorization of exactly that `funnel` row, the free `launch` is test-only (`#[cfg(test)] mod
uncovered`), so no production build can name it; `create_container` and `start_container` are
`pub(in crate::runner::container)`, so nothing outside the container module tree can. The census is now the guard
inside that tree and in test builds, and it names every occurrence inside a macro's argument and every colon form:
§3.1. Amended again, `FUA-I2-MACRO-WS`: review round 2 evaded the repaired reader with whitespace before a macro's
`!`, and the guarantee moved into types. Every start primitive takes a proof only `Reaping::cover` mints, the census is
a lexical backstop and not a proof, and inside the tree it is no longer the guard: §4.1.)

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
  isolated callers — runs under a deadline and is killed after it (corrected, `FUA-I1-WATCHDOG`:
  a kill ends nothing a tracer or an uninterruptible sleep holds. `run_isolated` collects its
  killed child within a bound since review round 1, and fails the test when it cannot, §3.2. The
  two-process coordinators are killed through `LinkedChild::kill`, which still waits for its child
  without one, filed as `PR328-LINKED-CHILD-KILL-WAITS-WITHOUT-A-DEADLINE`), so a mutation fails the test
  instead of wedging the run (round R6's two wedged first attempts); children that a mutation could
  leave waiting run completing doubles, not holding ones; a child that must be observed alive waits
  for the parent's word before it exits (round R6's `exit_on_the_parents_word`); and a wait for
  running containers waits on the fake's running listing, not on its journal's `Start` entries (round
  R5's first contention run: 111 of 200 failed on that race, record `:5250`–`:5252`).
- **A test that ends a real host reaper does so in an isolated child** (added, `FUA-I3-HOSTCANCEL`). A host
  reaper's `cancel` and `cleanup` wait for its acknowledged exit without a bound, by design. So only a process a
  watchdog can kill may make that wait: the lease child and the rest-selection test's child both run under
  `run_isolated` (§5).

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

One instrument file changes, `effects/wrappers.toml`, in the `src/agent/proc.rs` module only (amended,
`FUA-I1-MACRO`: review round 1 changes one row of the `src/runner/container.rs` module as well, authorized for it,
§3.3):

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
- Review round 1 also changes `src/runner/container.rs`: the free `launch` test-only and the two
  funnels' visibility (§3.1).
- Review round 2 (amended, `FUA-I2-MACRO-WS`, `FUA-I2-EINTR`) changes `src/runner/container/runtime.rs` (the trait's
  `create` and `start` take the proofs), `container.rs`, `exec.rs` (`mod cover`), `src/agent/proc.rs` (`EndingRest`),
  every runtime double, and creation's two relay waits in `engine/topology/create/tests.rs` (§4).
- Review round 3 (`FUA-I3-HOSTCANCEL`) changes only `src/agent/proc.rs`'s `#[cfg(test)]` test module, where the
  rest-selection test becomes the caller of an isolated child, and its notes (§5).

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

(Review round 1 changes both readers and checks 1, 2 and 5: §3.1. Amended, `FUA-I2-MACRO-WS`: review round 2 makes
the census a lexical backstop behind type-level proofs and adds shape checks to check 4: §4.1.) Its two readers are tested on written input:
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
its child was still in the unbounded wait at the deadline, `status None`). (Corrected,
`FUA-I1-WATCHDOG`: as written at `17d7c605` the kill was followed by `process.wait()`, which has no
bound, so a child the kill could not make collectable held the harness past its own deadline; the
regression lens held one at its exit stop and found the harness in `wait4` 68 s into a 60-second
deadline. Since review round 1 the watchdog collects within a bound and otherwise fails the test,
§3.2.)

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
  (`UnleasedExit`'s two bounded waits); the fail-closed arm waits for nothing. (Amended, `FUA-I2-EINTR`: the exit
  wait's pause did not return under interrupted rests; the round-2 sweep is §4.2.) The one deadline-less
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
(`mutation/fua-r{0,1,2}-*/test.log`). (Review round 1 changes one more row, authorized for it:
`src/runner/container.rs`'s `funnel` row loses `"launch"`, now test-only; §3.3.) Nothing else in any instrument
changes: no new effect
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

## 3. Implementation review round 1

Repaired by `pr11_fua_r1` (`claude-opus-5-5`, `max`). Every figure below is in a saved file under
`~/orch-pr11/logs/pr11_fua_r1/` that the sentence names; code lines are at `17d7c605` unless another commit is
named. The round's commits change `src/agent/proc.rs`'s tests, `src/runner/container.rs`, the census in
`src/runner/container/exec/tests.rs`, one row of `effects/wrappers.toml`, the notes, and the finding ledger. No other
production code changes.

### 3.0 The review and its triage

Three `gpt-6-astra` lenses at `max` reviewed `17d7c605`. Their texts are
`~/orch-pr11/reviews/review-328-i1-{delta,regression,concurrency}-17d7c605.review.md`, hashed in `SHA256SUMS-328-i1`;
the reviewers' witnesses are in `~/orch-pr11/reviews/328-i1-witnesses/{delta,reg}/`, hashed in
`SHA256SUMS-328-i1-witnesses`.
- **Delta and fix-check:** CHANGES_REQUIRED, one P2, executed (`FUA-I1-MACRO`, §3.1). It found no other defect in the
  four design-review corrections or the instrument rows.
- **Regression:** CHANGES_REQUIRED, one P2, executed (`FUA-I1-WATCHDOG`, §3.2). The frozen files are byte-identical,
  legacy and host behaviour unchanged, 3,023 library tests and 10 binary tests pass, residue is equal, Windows took
  456.12 s and macOS 1,136.19 s.
- **Concurrency:** PASS.

The orchestrator's triage (`~/orch-pr11/reviews/review-328-i1-triage.md`) fixes both: each carries an executed
witness, and `CLAUDE.md` has such findings fixed whatever their label. The round asked the orchestrator one question
(`~/orch-pr11/questions/pr11_fua_r1-1.md`) and followed its answer (`~/orch-pr11/answers/pr11_fua_r1-1.md`):
- the free `launch` is made test-only, with the one instrument row that needs (§3.3);
- the sweep's one pre-existing hit is filed rather than fixed (§3.2).

### 3.1 `FUA-I1-MACRO`: a macro-mediated launch escaped the domination census

**The reviewer's text** (delta lens):

> `namings` skips any identifier followed by a single colon, assuming a field initializer. Macro arguments can use
> that syntax to name a launcher: […] `uncovered_delegate!(launch: hooks, runtime, view, plan)` […] This expands to the
> unarmed free `container::launch`. The census skips `launch:` and never sees the expanded call. […] All **23 submitted
> tests**, including the domination census, passed. A new witness observed the container **Running**, its start
> recorded as **uncovered**, and **no armed reaper selecting its scope**. Production Clippy with `-D warnings` and six
> related effect/macro guards passed.

**The witness, red.** A scratch copy of `17d7c605` carried the reviewer's `mutation.diff` verbatim and this round's own
witness test, which calls the mutation's `delta_uncovered` over the fake runtime with covers observed
(`witness/macro-17d7c605-reviewer-mutation/`). All 24 tests passed, the census included. The witness printed
"container … state Some(Running); starts observed (name, covered by an armed reaper) [(…, false)]; an armed reaper
selects this runner's scope: false". On the same mutation, `cargo clippy --all-targets --all-features -- -D warnings`
exits 0 (`witness/macro-17d7c605-reviewer-mutation-clippy/`).

**The witness, green.** At the campaign commit `d741a8aa`, the reviewer's mutation (`r1-c9`) turns the census red:
it names `launch` as a macro argument inside `delta_uncovered`. In a production build the compiler refuses the same
mutation (`r1-p1`, `error[E0425]`: cannot find function `launch` in module `super`). The census witness committed for it,
`the_naming_reader_names_a_launcher_inside_a_macro_argument_whatever_follows_it`, carries the reviewer's macro
verbatim. It is red on the reader as reviewed (`r1-o1`) and green on the reader as repaired.

**Root cause, in two layers.**
1. The reader classified an occurrence by its neighbouring characters. It skipped `name:` as a field initializer
   (`exec/tests.rs:4993`, since `4bc56d21`), and a macro's argument is a token tree the macro can splice anywhere,
   whatever the text around it.
2. Underneath that, the unarmed free `launch` was nameable in production at all. §1.4 made the census "the guard, not
   a backstop": privacy alone cannot hide the free `launch` from `exec.rs`, a child of its own module, and taking it out
   of production needed an instrument row the follow-up's brief reserved for the orchestrator. A reading of source text
   always has forms it cannot see; the reviewer found one.

**The fix, compiler first.**
- **The free `launch` is test-only.** It moves, with the two private helpers only it uses (`cancel_created`,
  `render_residue`), into `#[cfg(test)] mod uncovered` in `container.rs`. `#[cfg(test)] pub use uncovered::launch;`
  keeps the path the tests name, so the frozen `recover/tests.rs:3957` and the other callers compile unchanged. The
  module sits before `mod fake;`, so the file's first test-only cut is a module, as `effects::production_region`
  requires.
  - **Before relying on it, the round proved no production path ever called the free `launch`** (the orchestrator's
    condition (i); `proof/q1-free-launch-callers-17d7c605/summary-callers.txt`). The repaired reader, run over
    `17d7c605`'s tree with the effects module's own region rules, finds the identifier in production only as
    `InvocationPlan`'s two `launch:` fields (`exec.rs:207`, `:565`), neither a naming of the function. Every call and
    import is in a test region. The path grep outside the test files is empty, and the effects census passed at
    `17d7c605` (`gates-impl/final-17d7c605/03-test.log`). So production behaviour at runtime is unchanged: the move
    takes out of the production build only functions no production path reached.
  - In a production build any naming of it, however spelled, fails to compile. The probes `r1-p1` to `r1-p3` cover the
    reviewer's macro, a fully qualified path from a topology module and an alias.
- **The funnels are `pub(in crate::runner::container)`.** `create_container` and `start_container` were `pub`, so any
  module of the crate could start a container no reaper covers. Restricted to the container module tree, a naming
  from anywhere else fails to compile (probes `r1-p4` and `r1-p5`: a call from a topology module, and a `pub use`
  re-export). The effects census still derives both as reachable funnels, because it classifies by the visibility a
  function declares; private `fn` would have dropped them from its domain.

**The fix, census second.** The census remains the guard where the compiler cannot be: inside the container module
tree, and in test builds, where the free `launch` exists. (Amended, `FUA-I2-MACRO-WS`: round 2 found the next spelling
the reader missed. Since then the compiler is the guard inside the tree too, and the census is a backstop: §4.1.)
- `namings` now names every occurrence of an unarmed launcher inside a macro invocation's delimiters as a
  `MacroArgument` (`macro_arguments` finds the spans), whatever precedes or follows it, a `.launch` included.
- Outside a macro it names a `name:` as a `Field`, pinned rather than skipped: `exec.rs`'s two `InvocationPlan`
  fields.
- `primitive_namings` counts macro arguments and colon forms the same way, in the twenty-seven modules where clippy
  cannot refuse the runtime's primitives. That adds four pins: the parameters `create:` (`agent/proc.rs`) and `start:`
  (twice, `export.rs`), and `util::tail`'s local `start` inside `format!`.
- The notes name what no reading of source text can see: an identifier a procedural macro builds, a path inside a
  string an attribute hands a derive, a file `include!`d from outside `src/`. The compiler covers those for the free
  `launch` everywhere and for the funnels outside the tree.

**What the census's expectations lost, and why** (condition (iii)). With the free `launch` test-only, its body left
the production region. The expected namings lose exactly `container.rs`'s two calls inside it, of `create_container`
and of `start_container`. The control's set of files naming `start_container` loses `container.rs` for the same reason:
those calls were its only production naming.

**Class search.**
- **The census's two readers** are the class's members, and both are repaired.
- **The other source census this follow-up added**, `no_reaper_test_reads_a_hold_as_released_once`, reads a named
  list of this follow-up's test functions for a one-shot "not held" read. A macro that builds the negation would evade
  it, but its subject is test code this programme writes, and it keeps requirement 8's flake out rather than guarding a
  production boundary. It is not changed.
- **The tree's older effects censuses** read text too. They are instruments outside this change, and their known
  limits are already filed.

**Mutations** (§3.4's method). Each runs against the 25 tests of `tools/round1-tests.txt`: the 22 of the reaper set,
the two reader tests and the watchdog witness.

| id | the mutation | witness for | red | green |
|---|---|---|---|---|
| `r1-control` | none | the set at the campaign commit | none | 25 |
| `r1-c2` | a production fn in `exec.rs` calls `start_container(` | the census, check 1 | 1: `every_container_start_in_production_is_reached_only_through_a_covered_launch` | 24 |
| `r1-c3` | a production fn in `engine/topology/preflight.rs` calls `crate::runner::container::launch(` (a test build, where it exists) | the census, check 2: a fully qualified path | 1: `every_container_start_in_production_is_reached_only_through_a_covered_launch` | 24 |
| `r1-c4` | `Covered {` built outside `cover` | the census, check 4 | 1: `every_container_start_in_production_is_reached_only_through_a_covered_launch` | 24 |
| `r1-c5` | `use super::launch as start_uncovered;` in `exec.rs` and a call of it | the census: an alias | 1: `every_container_start_in_production_is_reached_only_through_a_covered_launch` | 24 |
| `r1-c6` | `let start = super::start_container;` in `exec.rs` and a call of it | the census: a function value | 1: `every_container_start_in_production_is_reached_only_through_a_covered_launch` | 24 |
| `r1-c7` | `pub(in crate::runner::container) use super::create_container as create_uncovered;` in `census.rs` | the census: a re-export (one the compiler admits) | 1: `every_container_start_in_production_is_reached_only_through_a_covered_launch` | 24 |
| `r1-c8` | `runtime.start("probe")` in `engine/resume.rs`, which allows `disallowed_methods` | the census, check 3 | 1: `every_container_start_in_production_is_reached_only_through_a_covered_launch` | 24 |
| `r1-c9` | the review's mutation verbatim: `uncovered_delegate!(launch: hooks, runtime, view, plan)` in `exec.rs` | `FUA-I1-MACRO`: a macro argument | 1: `every_container_start_in_production_is_reached_only_through_a_covered_launch` | 24 |
| `r1-c10` | the review's macro naming `start_container:` in `exec.rs`, inside the module tree | the census: a macro argument the compiler admits | 1: `every_container_start_in_production_is_reached_only_through_a_covered_launch` | 24 |
| `r1-c11` | `delegate!(start: runtime, "probe")` expanding to `runtime.start("probe")` in `engine/resume.rs` | the census, check 3: a macro argument | 1: `every_container_start_in_production_is_reached_only_through_a_covered_launch` | 24 |
| `r1-o1` | `namings` as reviewed: skips `name:`, no macro rule | the census witness, red on the reader as reviewed | 3: `every_container_start_in_production_is_reached_only_through_a_covered_launch`, `the_naming_reader_names_a_launcher_inside_a_macro_argument_whatever_follows_it`, `the_naming_reader_names_an_alias_a_function_value_and_a_re_export_of_an_unarmed_launcher` | 22 |
| `r1-p1` | `r1-c9`, clippy on the production library | the compiler refuses the free `launch` | rc 101: `error[E0425]` cannot find function `launch` in module `super` at `src/runner/container/exec.rs:1074:25` | — |
| `r1-p2` | `r1-c3`, clippy on the production library | the compiler refuses the free `launch` | rc 101: `error[E0425]` cannot find function `launch` in module `crate::runner::container` at `src/engine/topology/preflight.rs:118:31` | — |
| `r1-p3` | `r1-c5`, clippy on the production library | the compiler refuses the free `launch` | rc 101: `error[E0432]` unresolved import `super::launch` at `src/runner/container/exec.rs:23:5` | — |
| `r1-p4` | `engine/topology/preflight.rs` calls `crate::runner::container::create_container(` | the compiler refuses a funnel outside the module tree | rc 101: `error[E0603]` function `create_container` is private at `src/engine/topology/preflight.rs:118:31` | — |
| `r1-p5` | `pub use super::create_container as create_uncovered;` in `census.rs` | the compiler refuses widening a funnel | rc 101: `error[E0364]` `create_container` is private, and cannot be re-exported at `src/runner/container/census.rs:20:9` | — |

Each red census names what was planted (`mutation/<name>/test.log`):
- `r1-c9`: `("src/runner/container/exec.rs", "launch", MacroArgument, Some("delta_uncovered"))`;
- `r1-c10`: `("src/runner/container/exec.rs", "start_container", MacroArgument, Some("start_through_a_macro"))`;
- `r1-c11`: `("src/engine/resume.rs", "start: in a macro argument", Some("start_through_a_macro"))`;
- `r1-c3`: `("src/engine/topology/preflight.rs", "launch", Call, Some("launch_uncovered"))`;
- `r1-c5`: `("src/runner/container/exec.rs", "launch", Alias, None)`;
- `r1-c7`: `("src/runner/container/census.rs", "create_container", Reexport, None)`.

A `pub use` re-export of a funnel no longer compiles at all (`r1-p5`); `r1-c7` re-exports one at the visibility the
compiler admits, for the census to read.

### 3.2 `FUA-I1-WATCHDOG`: the test watchdog waited for a killed child with no bound

**The reviewer's text** (regression lens):

> `run_isolated` sends `SIGKILL` when its deadline expires, then calls unbounded `process.wait()`. A child that
> remains uncollectable therefore wedges the harness despite the watchdog. I reproduced this on a clean rebuild of
> `17d7c605…`: ran the stopped-after-acknowledgement test, held its isolated child at `PTRACE_EVENT_EXIT`, and let the
> 60-second deadline expire. At 68.08 seconds, the harness remained blocked in `wait4(child, …, 0)`. […] This is newly
> introduced, separate from the deferred Docker wait, and contradicts the record's claim that the watchdog prevents
> wedging.

**The witness, red, by the reviewer's own method**, out of tree (`witness/watchdog-ptrace-17d7c605/`;
`tools/exit-hold-tracer.c`). The tracer starts `17d7c605`'s test binary as its child, so it may trace what the binary
starts under Yama's `ptrace_scope` 1. It seizes the isolated child of
`a_container_reaper_stopped_after_acknowledging_its_cancel_does_not_wedge_its_caller` with `PTRACE_O_TRACEEXIT`, holds
it at its exit stop for 90 s, and samples the harness's threads once a second:
- the child was held from 2.004 s;
- the harness thread polled until the deadline and was in `wait4(353144, …, 0)` from 60.012 s to 91.030 s;
- the test failed "status None" when the tracer released the child at 92.03 s ("finished in 92.03s").

The deadline's `SIGKILL` did not release a child already held at its exit stop, so a kill alone ends nothing here.

**The witness, red, in the suite.** The committed witness is
`the_watchdog_fails_its_test_rather_than_wait_for_a_child_its_kill_did_not_end`. Its isolated caller refuses `kill` on
its own thread with the module's seccomp policy, so `run_isolated`'s `SIGKILL` at a 2-second deadline cannot end
`child_that_outlives_its_kill_child`, which lives 60 s or until its parent is gone. The caller reports how long the
watchdog took and how it ended. Applied to a copy of `17d7c605` it fails: "the watchdog ended its run after 60009 ms
and returned status None" (`witness/watchdog-17d7c605-red/test.log`).

It refuses the kill rather than tracing the child because `ptrace`, `prctl`, the `PTRACE_*` and `PR_SET_PTRACER`
constants and `__WALL` are `libc::` items this crate does not name. Using them would need rows in
`effects/wrappers.toml`'s `[libc]` section, an instrument edit the round did not ask for. The reviewer's exact sequence
is therefore reproduced out of tree, before and after.

**The witness, green.**
- The committed witness passes at the head (at `d741a8aa` its isolated caller reports "the watchdog ended its run after 12001 ms and failed its test": the child was "still not collectable" 10s after a `SIGKILL` the policy refused ("Operation not permitted (os error 1)"), and the parent passed in 12.01 s (`witness/watchdog-witness-d741a8aa/`, the same binary as the reproduction below)).
- The reviewer's sequence at the head (`witness/watchdog-ptrace-d741a8aa/`: the child was held at its exit stop from 2.015 s, the harness thread was sampled in a blocking `wait4` 0 times (it polled from its deadline on), the test failed at 70.01 s with the child "still not collectable" 10 s after a delivered `SIGKILL`, and the test binary had ended at 70.059 s, its child still held).
- With the fix reverted (`r1-w1`) the witness is red again; with the watchdog giving up silently, a child it cannot
  collect reported as merely killed (`r1-w2`), it is red too.

**Root cause.** `run_isolated` took its `SIGKILL` for the end of the child and collected it with `process.wait()`,
which has no bound (`src/agent/proc.rs:9034`, since `4bc56d21`). A kill makes a child collectable only if it reaches
the child and nothing holds the child. It fails when:
- a tracer holds the child at its exit stop (the review's case);
- the child is in an uninterruptible sleep;
- the kill is refused, or aimed at a process group the child has left.

The watchdog then waited as long as the child did: the wedge it existed to prevent.

**The fix.**
- After the kill, `run_isolated` keeps polling `try_wait`, and the polls stop at `ISOLATED_CHILD_COLLECTION_BOUND`
  (10 s).
- A child collected in that time is reported as before, `status: None`.
- A child still not collectable fails the test with a message saying so, the kill's result (`delivered` or the errno),
  and the child's output. It is left to the process's exit, as `UnleasedExit` leaves a reaper it could not collect.

**The sweep (class search): every wait in this follow-up's test harness and production code, for a signal followed by
an unbounded wait.**
- **Production.** `cancel_unleased`'s `UnleasedExit` was already bounded on both paths, and is unchanged: a bounded
  wait, `SIGKILL`, a second bounded wait; with the identity path on, `end_helper_through_identity`, whose wait is
  bounded too.
- **`run_isolated`**: fixed, above.
- **Three of the new isolated children** signalled and then waited with a blocking `waitpid`. Each now waits through
  `waited_within`, `waitpid(…, options | WNOHANG)` polled within the same bound:
  - `container_reaper_whose_cancellation_fails_child` (`SIGKILL`, then a collection);
  - `container_reaper_stopped_before_its_cancellation_child` (`SIGSTOP`, then the stop);
  - `container_reaper_stopped_after_its_acknowledgement_child` (`SIGSTOP`, then the stop).

  A signal that lands makes a blocking wait and a bounded poll indistinguishable, so each child is witnessed in two
  more ways:
  - It refuses every blocking `wait4` on its thread (`answer_a_wait_by_number_that_would_block_with`, Linux), so a
    wait there that goes back to blocking fails the child at once.
  - It reports only after its wait, and its parent requires the report.

  The second is needed because a failed assertion in a child that holds a container reaper unwinds through the
  reaper's drop. That drop's missed CANCEL acknowledgement ends the child by `SIGTERM`, the very status two of the
  parents expect. The first campaign, at `a2e21304`, had the policy alone
  (`mutation-campaign-a-a2e21304/TABLE-r1.md`):
  - `r1-s2`'s reverted wait passed;
  - `r1-s1`'s was red only because libtest exited before the monitor's `SIGTERM`.

  At `d741a8aa`, `r1-s1` to `r1-s3` are red. `r1-s1x`, `r1-s1` without the policy, stays green: a blocking wait the
  signal satisfies cannot be told from a poll, which is why the policy is there.
- **`a_stopped_container_reaper_ends_its_caller_rather_than_releasing_it`** sends `SIGCONT` and waits for nothing; its
  reads are polls within its deadline.
- **The isolated writer and the relay's self-check** (`write_program_in_its_own_process`,
  `run_program_in_its_own_process`) run through `agent::proc::test_support::run_with_timeout`. That is the production
  supervised runner, with PR6's termination contract, not this follow-up's code, and it is not changed.
- **The two-process witnesses** wait for their child's events and exit within `LINK_BOUND` (`Served::event`,
  `Served::exited`). But they kill it through `Served::kill`, which calls `LinkedChild::kill`
  (`src/workspace_manager/fixture.rs:3107`): `child.kill()`, then `child.wait()` with no bound. `Served`'s and
  `LinkedChild`'s `Drop`s call it again. **This is the one instance of the shape this round did not fix.**
  - It predates the follow-up (first bad `ad1a36b5`, PR11 phase 5).
  - It lives in `src/workspace_manager*`, which follow-up B (#329) owns, and B's own two-process tests lean on
    `LinkedChild`.
  - By the orchestrator's decision it is filed, `fixture.rs` untouched:
    `PR328-LINKED-CHILD-KILL-WAITS-WITHOUT-A-DEADLINE` (P3, `pre_existing`, deferred, guarded by #329's
    implementation phase).
  - Its callers are six PR11 phase-5 tests and helpers and this follow-up's two two-process witnesses.

**Mutations:**

| id | the mutation | witness for | red | green |
|---|---|---|---|---|
| `r1-control` | none | the set at the campaign commit | none | 25 |
| `r1-w1` | `run_isolated` waits for its killed child with `process.wait()`, as at `17d7c605` | `FUA-I1-WATCHDOG` | 1: `the_watchdog_fails_its_test_rather_than_wait_for_a_child_its_kill_did_not_end` | 24 |
| `r1-w2` | a child still not collectable at the bound is reported as merely killed (`status: None`) | "report failure when the child stays uncollectable" | 1: `the_watchdog_fails_its_test_rather_than_wait_for_a_child_its_kill_did_not_end` | 24 |
| `r1-s1` | `container_reaper_whose_cancellation_fails_child` waits for its killed reaper with a blocking `waitpid` | the sweep | 1: `a_container_reapers_failed_cancellation_ends_its_caller_through_the_signal_monitor` | 24 |
| `r1-s1x` | `r1-s1` without `answer_a_wait_by_number_that_would_block_with` | why the sweep's policy is there | none | 25 |
| `r1-s2` | `container_reaper_stopped_before_its_cancellation_child` waits for the stop with a blocking `waitpid` | the sweep | 1: `a_stopped_container_reaper_ends_its_caller_rather_than_releasing_it` | 24 |
| `r1-s3` | `container_reaper_stopped_after_its_acknowledgement_child` waits for the stop with a blocking `waitpid` | the sweep | 1: `a_container_reaper_stopped_after_acknowledging_its_cancel_does_not_wedge_its_caller` | 24 |

What each red witness reported (`mutation/<name>/test.log`):
- `r1-w1`: "the watchdog ended its run after 60007 ms and returned status None", the behaviour at `17d7c605`.
- `r1-w2`: "after 12011 ms and returned status None". The bound held, but a child it could not collect was reported
  as an ordinary deadline kill, with no failure.

### 3.3 The instrument row, and the sentences this round corrects

**One instrument row**, authorized by the orchestrator for exactly this and nothing else
(`~/orch-pr11/answers/pr11_fua_r1-1.md`):
- `effects/wrappers.toml`, `[[module]] path = "src/runner/container.rs"`: `"launch"` leaves the `funnel` list, with a
  one-line comment saying it is test-only since this follow-up (`FUA-I1-MACRO`).
- **Why it is safe:** the free `launch` had no production caller (§3.1's proof). The row named a function only tests
  reach, and every production container start is still classified through `ContainerRunner::launch`'s calls of the two
  funnels, whose rows are unchanged.
- Both effects tests that read the row pass with it removed: `every_externally_reachable_fn_of_a_legacy_or_shared_module_is_classified`
  and `every_funnel_classified_fn_names_a_site` (`dev/full-q1.log`). On a scratch copy that kept the row they failed:
  "invented: [\"launch\"]" and "classifies `launch` a funnel and declares no such fn"
  (`witness/q1-experiment-tests/test.log`).
- No other row, allowlist, `clippy.toml`, `src/effects/**` or frozen file changes.

**Corrected or amended above, each marked:**
- §2.3's sentence on the watchdog: it said the kill alone kept a mutation from wedging, and the watchdog then waited
  without a bound.
- §1.5's harness rule: every child "is killed after it" held for `run_isolated` only once a bounded collection
  followed, and not for the two-process children (the filed P3).
- §1.4's "Why the census and not visibility": the round took the visibility route.
- §2.2 and §2.8: pointers to this section.

### 3.4 How the mutations were run, gates and platforms

**The campaign.** Each mutation is a scratch copy of the campaign commit `d741a8aa`'s tracked files with exact
substitutions (`tools/campaign-r1.py` through `tools/mutate.py`), built from itself.
- Every test log's `Compiling upstroke v0.1.0 (…/mut/src/<name>)` line names its own copy; the probes' `Checking`
  lines do the same.
- The test mutations run against `tools/round1-tests.txt`. The compiler probes run `cargo clippy --lib --all-features
  -- -D warnings` on the production library, where the free `launch` does not exist.
- The evidence is `mutation/<name>/{mutation.diff,test.log,summary.txt}`, and `mutation/TABLE-r1.md` tabulates it.
- Each copy differs from the campaign commit only in its mutated files (`mutation/copies-vs-d741a8aa.txt`).
- The unmutated control passed all 25.

**Repeats.** The round's 25 tests ran 20 consecutive times at `d741a8aa` while campaign B loaded the box. All 20 were
green, about 12 s each, the watchdog witness's two bounds being most of it
(`proof/repeat-under-campaign-b-d741a8aa/summary.txt`). Each run used the binary that `witness/build-d741a8aa-norun.log`
compiled from this worktree.

**Gates and CI.** The ten gates run at the pushed head, in the foreground, cargo through `upstroke-build` on this
session's private base. They and CI's legs, read by the newest run per workflow, are in the pull request body and
`~/orch-pr11/handovers/pr11_fua_r1.md`. This section is committed before the gates run, so it names no figure of
theirs.

**Platforms.**
- **Linux** (this box) runs everything.
- **macOS** runs the census and both reader tests, all `cfg(unix)`, but not the watchdog witness or the sweep's policy:
  both use seccomp and are Linux-only. There the bounded waits run unwitnessed.
- **Windows** gains no test. Its visibility and module changes are type-checked and linted for
  `x86_64-pc-windows-msvc`, not run.
- CI speaks for macOS and Windows.

**Not verified here:**
- macOS and Windows behaviour (CI's);
- a child held in a real uninterruptible sleep, which only the reviewer's tracer and the refused kill stand in for;
- `LinkedChild::kill`'s wait (filed).

## 4. Implementation review round 2

Repaired by `pr11_fua_r2` (`claude-opus-5-5`, `max`). Every figure below is in a saved file under
`~/orch-pr11/logs/pr11_fua_r2/` that the sentence names; code lines are at the campaign commit `b612aa16` unless another
commit is named. The round's commits are `99d38e65` (`FUA-I2-MACRO-WS`), `b612aa16` (`FUA-I2-EINTR`), `2aa86514` (the
notes), `6f5a0e40` (one test attribute) and this section. They change `src/runner/container.rs`, `exec.rs`, `runtime.rs`
and `src/agent/proc.rs` in production, the container and topology test doubles and tests, and the notes. No instrument
and no frozen file changes.

### 4.0 The review and its triage

Three `gpt-6-astra` lenses at `max` reviewed `8d0d8b86`. Their texts are
`~/orch-pr11/reviews/review-328-i2-{delta,regression,concurrency}-8d0d8b86.review.md`, hashed in `SHA256SUMS-328-i2`;
the reviewers' witnesses are in `~/orch-pr11/reviews/328-i2-witnesses/{delta,conc}/`, hashed in
`SHA256SUMS-328-i2-witnesses`.
- **Regression:** PASS. The 26 frozen production files and eight frozen test children are byte-identical, the
  instrument changes are exactly the declared rows, 3,025 library and 10 binary tests passed, and the exact-head CI
  passed (Windows 537.89 s, macOS 1,368.94 s).
- **Delta and fix-check:** CHANGES_REQUIRED, one P2, executed (`FUA-I2-MACRO-WS`, §4.1).
- **Concurrency:** CHANGES_REQUIRED, one P2, executed (`FUA-I2-EINTR`, §4.2).

The orchestrator's triage (`~/orch-pr11/reviews/review-328-i2-triage.md`) fixes both under the witness rule and raises
the looping note for the census, §4.3. Neither is a P1 and the design stands. The round asked no question.

### 4.1 `FUA-I2-MACRO-WS`: an uncovered start must fail to compile, not merely fail the census

**The reviewer's text** (delta lens):

> `macro_arguments` requires an identifier byte immediately before `!`. Rust permits whitespace there, and rustfmt
> preserves it inside this nested macro definition: […] `delegate ! (plan.start_container, $($argument),*)` […] The
> reader misses `delegate ! (...)`, then skips `.start_container` as field access. The expansion calls
> `super::start_container`. The same construction hides `create_container`; both remain accessible inside `exec.rs`.
> […] All **25 submitted tests**, including the domination census, passed. […] A fake-runtime witness observed
> **Running**, **coverage false**, and **no armed reaper selecting the scope**. All-target Clippy with `-D warnings`
> and `fmt --check` passed. […] This leaves `FUA-I1-MACRO` incompletely repaired and contradicts the claim that the
> census reads anything inside macro arguments.

**The witness, red at `8d0d8b86`.** A scratch copy of `8d0d8b86` carried the reviewer's `mutation.diff` verbatim
(`witness/macro-8d0d8b86-reviewer-mutation/`, by `tools/witness-copy.py`). Round 1's 25 tests and the reviewer's
witness, 26 in all, passed, the census among them. The witness printed "launched true; container upstroke-… state
Some(Running); starts observed (name, covered by an armed reaper) [(…, false)]; an armed reaper selects this runner's
scope: false". `cargo clippy --all-targets --all-features -- -D warnings` on the same copy exited 0
(`witness/macro-8d0d8b86-reviewer-mutation-clippy/`).

**The witness, now a compile error.** The same mutation, verbatim, on a copy of `99d38e65`:
- a production build (`cargo clippy --lib --all-features -- -D warnings`) exits 101 with `error[E0308]: mismatched
  types` (the create is handed `&CreateSpec` where it takes `CoveredCreate`) and `error[E0061]: this function takes 5
  arguments but 4 arguments were supplied` (the start lacks its `CoveredStart`)
  (`witness/macro-99d38e65-reviewer-mutation-production/`);
- a test build (`cargo test --lib --all-features --no-run`) exits 101 with the same two errors
  (`witness/macro-99d38e65-reviewer-mutation-test-build/`).

The campaign repeats it at `b612aa16` as `r2-p1`, with the variants that try to supply a proof (§4.4).

**Root cause, in two layers.**
1. The reader classified an invocation by the byte before its `!`. A reading of source text has spellings it cannot
   see, and round 1's repair of it (`FUA-I1-MACRO`, §3.1) closed one spelling and left the next.
2. Underneath, the guarantee was a reading of text at all. Round 1 took the free `launch` out of production and
   restricted the funnels to the container module tree, but inside that tree `create_container` and `start_container`
   took no cover, and the runtime's own `create` and `start` were callable wherever the call was spelled so clippy's
   `disallowed_methods` did not see it — in the twenty-seven modules that allow that lint, by any spelling at all. The
   census was the only thing between a launch inside the tree and a start no reaper covers.

**The fix: the guarantee moves into types.**
- **Every start primitive takes a proof.** `create_container` and `start_container` (`src/runner/container.rs:278`,
  `:336`), and `ContainerRuntime::create` and `::start` (`src/runner/container/runtime.rs:274`, `:276`), take a
  `CoveredCreate` or a `CoveredStart` by value. The funnels pass theirs on to the runtime, so the proof is consumed by
  the call that starts the container. All twelve runtime implementations take them: `DockerCli`, the two fakes and
  nine test doubles.
- **Only `Reaping::cover` mints one.** The proofs, the in-flight guard `Covered` and `cover` itself live in `pub(super)
  mod cover` (`src/runner/container/exec.rs:1002`), a leaf module at the end of `exec.rs`. Their fields are private
  to `cover`, so neither the rest of `exec.rs` nor any sibling or parent module can construct one. That is why
  `clippy.toml`'s note that "a token, a sealed trait or a private constructor would all be reachable from a sibling"
  does not apply: it is true of a token defined in `runner::container`, and this one is defined in a leaf.
  `cover` (`exec.rs:1025`) validates the labels and arms as before, then returns `Covered` and the two proofs.
- **Each proof is bound to what the cover validated.** `CoveredCreate` borrows the very spec `cover` checked against
  the reaper's scope, and the create is made from that spec: `create_container` and every `ContainerRuntime::create`
  read `covered.spec()` and take no spec of their own. `CoveredStart` borrows that spec's name, and `start_container`
  refuses an intent for any other name before the runtime is asked (`expect_intent_for`, `attempted: false`). Both
  carry the cover's single lifetime, the borrow of the runner's `Reaping` and of the spec at once, so neither can
  outlive the runner whose reaper covers it.
- **`ContainerRunner::launch` passes the two through** (`exec.rs:605`); until now it took `&Covered` and read nothing
  from it.
- **Nameable inside the container module tree only.** The proofs are re-exported `pub(in crate::runner::container)`
  (`exec.rs:32`). Declared `pub` in a private module, they can appear in the public trait without a
  `private_interfaces` lint, and outside the tree a production build cannot name them (`r2-p5`, `error[E0603]`).
- **Tests mint through `without_a_reaper`.** It is `#[cfg(test)]`, in a child module of `cover` (`exec.rs:1100`). The
  test-only free `container::launch` uses it, so its signature is unchanged and the frozen `recover/tests.rs` that
  calls it compiles unchanged. So do the container suites' direct calls of the funnels and the runtimes. A test-only
  `pub(crate)` re-export in `container.rs` lets the doubles outside the tree name the types. No production build has
  either (`r2-p3`, `error[E0599]`). The `#[cfg(test)]` child module is the file's first `#[cfg(test)]`, so
  `effects::production_region` cuts `exec.rs` at a module after every production item.

**What the compiler does not cover, and the census.** Two things remain text the compiler cannot judge:
- code inside `cover` itself, which can mint;
- the runtime's implementation: `DockerCli`'s `create` and `start`, and the private `DockerCli::exec` they share, which
  runs whatever `docker` argv it is handed.

The domination census stays as a **lexical backstop, not a proof**: it is not made to read `name ! (…)`, and nothing
depends on it doing so. What it now holds (`src/runner/container/exec/tests.rs`, from `:5790`):
- the four primitives' signatures still name their proofs, so the type-level guarantee cannot be removed in silence;
- `Covered {`, `CoveredCreate {` and `CoveredStart {` are each built once, in `cover`;
- `cover` declares no production module of its own, since a child module sees its private fields;
- no production region names `without_a_reaper`.

`docker` and `DockerCli` stay confined to `container.rs` by `effects`' denylist census. Every `RuntimeOp::Create` stays
pinned to `DockerCli::create`, and `RuntimeOp::Start` to `DockerCli::start`, by check 3.

**The new test.** `a_start_cover_starts_only_the_container_it_was_minted_for` creates a container under its own
intent. It then hands the start a proof minted for another worker's container, which is refused with
`attempted: false` and nothing reaching the runtime, and the container is still `Exited`. The proof minted for it
then starts it.

**Class search: every path that starts a container** (`measure/class-search-macro-ws-b612aa16.txt`).
- The funnels and the trait's two methods take proofs. The compiler holds all twelve implementations to the trait.
- No production `docker` argv starts a container by any verb but `create` and `start`.
- `RuntimeOp::Create` and `::Start` occur in production only in `DockerCli`'s two methods.
- `DockerCli` is named only by `container.rs`; `runtime.rs` names `DOCKER_PROGRAM` as `reaper_program`'s default.
- The fakes' relayed `Create` and `Start` are test-only.
- The reaper's own `docker kill` and `rm` start nothing.

The two older source censuses that pin `runtime.create(`/`runtime.start(` (`container/tests.rs`'s
`every_container_effect_in_the_tree_goes_through_the_funnel` and `resolve/tests.rs`'s read-only check) are unchanged,
and pass.

### 4.2 `FUA-I2-EINTR`: interrupted rests must return to the deadline check

**The reviewer's text** (concurrency lens):

> The new `UnleasedExit` branches at `src/agent/proc.rs:2439` and `2479` use polling helpers whose pauses call
> `thread::sleep` at lines 3886 and 3938. […] Sleep syscalls repeatedly return `EINTR`. Rust retries inside
> `thread::sleep`, preventing the outer loop from checking its deadline. The caller never reaches the
> deadline-triggered SIGKILL or returns from `Drop`. […] Both callers required the external watchdog's kill after
> **8 seconds**, exceeding the promised two two-second budgets. Uninterrupted controls returned in approximately
> **2 seconds**. […] The underlying helpers predate this PR, but their new use does not establish the claimed
> `FUA-D1-CONC-1` bound.

**The witness, red at `8d0d8b86`.**
- The reviewer's `witness.patch` on a copy of `8d0d8b86` (`witness/eintr-8d0d8b86-reviewer-witness/`): the two controls
  returned in 2.02 s, and both interrupted modes, number and identity, were killed by its watchdog at 8.01 s,
  `status None`.
- This round's own witness, applied to `8d0d8b86` (its block taken verbatim from `b612aa16` and inserted into
  `8d0d8b86`'s `proc.rs`, `witness/patches/own-eintr-witness-b612aa16.patch`;
  `witness/eintr-8d0d8b86-own-witness-from-b612aa16/`): both paths `status None` at their 30-second deadline, each
  child having printed "the reaper is dropped" before it was held, and the test failed in 60.03 s.

**The witness, green at `b612aa16`** (`tools/eintr-evidence.sh`):
- The reviewer's `witness.patch` (`witness/eintr-b612aa16-reviewer-witness/`): the drop returned after 2.00 s in every
  mode, the interrupted ones (2.000176 s, number; 2.000177 s, identity) as well as the controls.
- The committed witness, `a_container_reaper_stopped_after_acknowledging_its_cancel_ends_its_caller_with_every_rest_refused`,
  in this worktree (`witness/eintr-b612aa16-own-witness/`): it passed in 4.24 s. Its child, run alone per path,
  reported "every rest refused, the drop returned after 2000ms; the stopped stand-in was collected: true" through the
  number and through the identity, and "the harness's bounded wait came back after 100ms: None".

The committed witness is Linux-only (seccomp). Its child stops a stand-in, wraps it in a `ContainerReaper` whose CANCEL
is acknowledged, and opens a pidfd on the stand-in for the identity path. Then it refuses every rest on its thread
(`clock_nanosleep` answered `EINTR`, the call a sleep makes here) and first sees the refusal in force. It drops the
reaper and reports. It then asks the harness's own `waited_within` about a second child that never exits.

**Root cause.** `UnleasedExit`'s two bounded waits (`FUA-D1-CONC-1`) were built from the module's pre-existing bounded
endings, `wait_for_an_ended_helper` and `wait_for_an_ended_helper_through_identity`. Those check their deadline between
polls and pause with `thread::sleep(HELPER_END_POLL_SLICE)`. Rust's `thread::sleep` makes an interrupted `nanosleep`
again for what it had left, and a refused one again for all of it. Under a stream of `EINTR` the pause never returned,
the deadline was never checked again, and the drop, with the coordinator's shutdown behind it, waited without bound.
The bound `FUA-D1-CONC-1` claimed held only for an uninterrupted thread.

**The fix.**
- `EndingRest` (`src/agent/proc.rs:3787`) is how a bounded ending pauses between polls:
  - `Resuming` is `thread::sleep`, as before;
  - `SingleAttempt` is `rest_once` (`:3799`): one `nanosleep` with a null remainder, which an interruption ends early
    and nothing makes again, so the loop goes back to its deadline check.
- Every `Reaper` carries its kind's rest (`rest: EndingRest`, `:1676`), fixed where it is forked: `fork_reaper` takes it.
  - `spawn_reaper` forks host reapers `Resuming`.
  - `arm_container_reaper` forks container reapers `SingleAttempt`.
  - `abandon` and the bounded arms of `close_and_wait_reporting` (`UnacknowledgedCleanup`, `AbandonedHelper`,
    `UnleasedExit`) pass `self.rest` to the waits. So a container reaper's exit wait after CANCEL, on both paths, and
    the abandonment an arming failure takes both rest once.
- The pre-existing helpers gained the parameter. Their pre-existing callers pass `Resuming`: the guards'
  `end_unready_guard` sites, and host reapers through their field. So no host path's pause changed, and
  `a_container_reaper_rests_once_between_its_ending_polls_and_a_host_reaper_as_before` pins both kinds.

**The sweep** (`measure/class-search-eintr-b612aa16.txt`).
- **This PR's production waits.**
  - READY (`await_ready`) and the CANCEL acknowledgement (`acknowledged`) read through `read_guard_ack`. It recomputes
    the remaining time every turn and answers `poll`'s `EINTR` by going back to that check, so it already has the
    single-attempt shape.
  - The CANCEL frame's `write_raw` retries `EINTR`, but it writes five bytes into a pipe nothing else has written, so
    it cannot block and no signal can interrupt it.
  - The exit wait and the arming abandonment are fixed above.
- **The reaper child's loops** (`reaper_loop`'s 10 ms poll, `read_bounded`, `reap_bounded`'s `raw_sleep_10ms`) are PR6's,
  shared with host reapers' container half, and have the resuming shape. No signal can interrupt a syscall in the
  reaper child: `install_reaper_dispositions` leaves no handler (every catchable signal ignored, the synchronous ones
  and `SIGCHLD` default), so only a seccomp policy inherited from the forking thread could inject `EINTR` there. They
  sit in the machinery already filed as `PR328-REAPER-DOCKER-WAIT-HAS-NO-DEADLINE-AFTER-SIGKILL`, and are not changed
  here.
- **This follow-up's test harness.** Every deadline loop it added now rests once:
  - in `proc.rs`: `run_isolated`, `waited_within`, `wait_out_a_fail_closed_termination`, the stopped-reaper witness's
    relay wait, the lease child's `not_held_within` and `child_that_outlives_its_kill_child`, through `rest_once`;
  - in `engine/topology/create/tests.rs`: its two relay waits, through `workspace_manager::fixture::rest_within`, which
    the coordinator's witnesses already used.

  The witness's harness check holds `waited_within`'s rest.

### 4.3 The looping signal (`MAINTAINING.md`, "When a pull request may be looping")

This round found a defect in machinery an earlier round of this pull request added: the domination census, written
in phase 2 and repaired in round 1. **The census was being repaired one spelling at a time.**
- Round 1 found it evaded by a macro's `launch:` argument and taught the reader macro arguments.
- Round 2 found it evaded by whitespace before a macro's `!`.

A third repair of the reader would have been the next turn of the same loop: machinery invented to keep the previous
round's machinery safe, which a reading of text cannot finish. This round makes the smaller change `MAINTAINING.md`
asks for:
- **It keeps what survived every pass**: the cover inside the launch funnel, the reaper and its disarm rule.
- **It moves the guarantee from text to types.** A start primitive without its proof is a compile error. Names are
  resolved after macros expand, so no spelling reaches a primitive without a value only `Reaping::cover` can mint.
- **It demotes the census to a lexical backstop.** The census is kept, and says it is not a proof.

What holds the convergence is the compiler, witnessed by probes that each fail to build:
- the reviewer's mutation (`E0308`, `E0061`);
- the same mutation with forged proofs (`E0451`);
- with the test-only mint (`E0599` in production);
- a runtime start with no proof where clippy allows the call (`E0308`);
- the proof named outside the tree (`E0603`);
- a consumed proof used again (`E0382`);
- a proof made to outlive its runner (a lifetime error).

`FUA-I2-EINTR` is not of that kind. It is a defect in this pull request's own bounded wait, repaired at its cause and
witnessed on both paths.

### 4.4 How the mutations were run, gates and platforms

**The campaign** (`tools/campaign-r2.py` through `tools/mutate.py`).
- Each mutation is a scratch copy of the campaign commit `b612aa16`, by `git archive` (the worktree is not read), with
  an exact single-occurrence substitution, built from itself. Every log's `Compiling` or `Checking` line names its own
  copy.
- Each copy differs from `b612aa16` only in its mutated file (`mutation/copies-vs-b612aa16.txt`).
- Test mutations run `tools/round2-tests.txt`: round 1's 25 tests and this round's three. The unmutated control passed
  all 28.
- Compiler probes run twice: clippy on the production library, and a test build with no run.
- `mutation/TABLE-r2.md` tabulates `mutation/<name>/{mutation.diff,test.log,summary.txt}`.

| id | the mutation | witness for | result |
|---|---|---|---|
| `r2-control` | none | the set at the campaign commit | 28 green |
| `r2-p1` | the reviewer's mutation verbatim | `FUA-I2-MACRO-WS` | production and test builds: `error[E0308]` and `error[E0061]` |
| `r2-p2` | the reviewer's macro handed forged proofs (`CoveredCreate { spec: … }`) | the proofs' private fields | both builds: `error[E0451]` field `spec` / `name` is private |
| `r2-p3` | the reviewer's macro handed `without_a_reaper` proofs | the test-only mint | production: `error[E0599]`; test build: compiles (see `r2-c4`) |
| `r2-p4` | `runtime.start("probe")` in `engine/resume.rs`, which allows `disallowed_methods` (round 1's `r1-c8`) | the trait's proof | both builds: `error[E0308]` |
| `r2-p5` | `engine/topology/preflight.rs` names `crate::runner::container::exec::CoveredCreate` | nameable only in the tree | both builds: `error[E0603]` struct import is private |
| `r2-p6` | `launch` passes `to_create` to a second create | consumed by the call | both builds: `error[E0382]` use of moved value |
| `r2-p7` | a method returning a cover's `CoveredStart<'static>` | bound to the runner | both builds: `lifetime may not live long enough` |
| `r2-c1` | a second mint, inside `cover` | census: constructions | red: the census, "`CoveredCreate {…}` is built only by the cover …, and exec.rs builds it 2 time(s)" |
| `r2-c2` | a child module inside `cover` minting with `Self { spec }` | census: no module in `cover` | red: the census, "the module that mints the proofs declares no module of its own in production" |
| `r2-c3` | a production fn in `exec.rs` naming `without_a_reaper` | census: the test-only mint | red: the census, naming `mint_for_a_test` |
| `r2-c4` | `r2-p3` in a test build | census: the test-only mint | red: the census, naming `delta_uncovered` twice |
| `r2-b1` | `start_container` without its intent check of the proof's name | the start binding | red: `a_start_cover_starts_only_the_container_it_was_minted_for` (the start was attempted) |
| `r2-e1` | the single-attempt arm back to `thread::sleep` | `FUA-I2-EINTR` | red: the witness, both paths `status None` after "the reaper is dropped" |
| `r2-e2` | `rest_once` resumes an interrupted rest with its remainder | the single attempt | red: the witness, both paths `status None` |
| `r2-e3` | `arm_container_reaper` forks `Resuming` | the container reaper's rest | red: the white-box test, `(Some(Resuming), Resuming)`; the witness alone passes it |
| `r2-e4` | `spawn_reaper` forks `SingleAttempt` | host reapers unchanged | red: the white-box test, `(Some(SingleAttempt), SingleAttempt)` |
| `r2-e5` | `waited_within` back to `thread::sleep` | the harness sweep | red: the witness, both paths: the drop returned after 2000 ms, the harness's wait never came back |

Each test mutation turned red exactly the one test its row names, the other 27 green.

**Round 1's census mutations against the new types**, measured as test builds of `b612aa16`
(`tools/r1-census-at-r2.py`, `mutation/r1-at-b612aa16-*`):
- **Six no longer compile.**
  - `r1-c2` (a second production start), `r1-c6` (a function value of `start_container`) and `r1-c10` (a macro
    argument naming a funnel) fail with `error[E0061]`.
  - `r1-c8` and `r1-c11` (the runtime's `start` where clippy allows it, plain and through a macro) fail with
    `error[E0308]`.
  - `r1-c4` (a `Covered` built outside `cover`) fails with `error[E0425]` and `error[E0422]`, because `Covered` is no
    longer in scope there.
- **Four still build, and only in test builds.**
  - `r1-c3`, `r1-c5` and `r1-c9` name the test-only free `launch`; round 1's `r1-p1` to `r1-p3` showed a production
    build refusing each.
  - `r1-c7` re-exports a funnel without calling it.
  - The census turns red on each of the four, as in round 1 (`mutation/r1-at-b612aa16-{c3,c5,c7,c9}-test/`).

**Gates and CI.** The ten gates run at the pushed head, in the foreground, cargo through `upstroke-build` on this
session's private base. They and CI's legs, read by the newest run per workflow, are in the pull request body and
`~/orch-pr11/handovers/pr11_fua_r2.md`. This section is committed before they run, so it names no figure of theirs.
Before it:
- the whole suite at `b612aa16` passed 3,028 library tests (0 failed, 122 ignored) and 10 binary tests
  (`dev/test-full-2-b612aa16.log`);
- against round 1's head the suite gained the three tests above and one ignored child, and lost none
  (`measure/new-tests-b612aa16-vs-8d0d8b86.txt`);
- the module diff proof passes: the 26 frozen production files and the eight frozen test children are identical to
  the merge base (`measure/module-diff-proof-2aa86514.txt`).

**Platforms.**
- **Linux** (this box) runs everything.
- **macOS** runs the census, the start-binding test and the white-box rest test, all `cfg(unix)`, but not the EINTR
  witness, which uses seccomp and is Linux-only. There the single-attempt rest runs unwitnessed.
- **Windows** gains no test. The proofs and the rest compile there:
  - `clippy --target x86_64-pc-windows-msvc --all-targets` and `cargo +1.85.0 check --target x86_64-pc-windows-msvc
    --locked --all-targets` are clean (`gates/xtarget-2aa86514/`, and `dev/clippy-8-windows.log` after `6f5a0e40`);
  - so is `clippy --target aarch64-apple-darwin --all-targets`.
- CI speaks for macOS and Windows.

**Not verified here:**
- macOS and Windows behaviour (CI's);
- an `EINTR` stream from real signals rather than a seccomp policy;
- the reaper child's inherited resuming rests, which no signal can reach (§4.2).

### 4.5 The sentences this round amends

Each is marked in place with `FUA-I2-MACRO-WS` or `FUA-I2-EINTR`:
- §0: this round's entry and the next step.
- §1.2: the census enforced "what the type system cannot"; since this round the type system enforces it.
- §1.3: "every wait in the container reaper's life has a deadline" held only if the pauses between its polls return.
- §1.4, "Why the census and not visibility": the census is now a lexical backstop behind types, not the guard.
- §1.9: the files this round changes.
- §2.2, §2.4 and §3.1: pointers here.

## 5. Implementation review round 3

Repaired by `pr11_fua_r3` (`claude-opus-5-5`, `max`). Every figure below is in a saved file under
`~/orch-pr11/logs/pr11_fua_r3/` that the sentence names; code lines are at the fix commit `f8b022a5` unless another
commit is named. The round's commits are `f8b022a5` (the test), `aed7a299` (the notes) and this section. They change
only `src/agent/proc.rs`'s `#[cfg(test)]` test module and its notes: no production code, no instrument and no frozen
file.

### 5.0 The review and its triage

Three `gpt-6-astra` lenses at `max` reviewed `71e55dfc`. Their texts are
`~/orch-pr11/reviews/review-328-i3-{delta,regression,concurrency}-71e55dfc.review.md`, hashed in `SHA256SUMS-328-i3`;
the regression lens's witness is in `~/orch-pr11/reviews/328-i3-witnesses/`, hashed in `SHA256SUMS-328-i3-witnesses`.
- **Delta and fix-check:** PASS. Round 2's macro mutation fails to build at the head, in production and test builds
  (`E0308`, `E0061`), and the proof boundary holds.
- **Concurrency:** PASS. 60 focused Linux tests passed.
- **Regression:** CHANGES_REQUIRED, one P2, executed (`FUA-I3-HOSTCANCEL`, §5.1). It found no other regression: the
  merge base is `92c4ca81`, the 26 frozen production files and eight frozen test children are byte-identical, and the
  exact-head CI passed, macOS and Windows included.

The orchestrator's triage (`~/orch-pr11/reviews/review-328-i3-triage.md`) fixes the P2 under the witness rule, as a
test-only change, and sets round 4 to delta and regression. The round asked no question.

### 5.1 `FUA-I3-HOSTCANCEL`: the rest-selection test made a deliberately unbounded wait in the libtest process

**The reviewer's text** (regression lens):

> **P2 — The new rest-selection test can wedge the entire suite (executed).** At src/agent/proc.rs:9224, the test
> calls `host.cancel()` directly in the main test process, without a watchdog. Host cancellation deliberately waits
> indefinitely for the acknowledged reaper’s exit.
>
> Concrete sequence: the host reaper acknowledges CANCEL → it becomes stopped before completing exit → `host.cancel()`
> blocks in `AcknowledgedExit` → the suite cannot finish. On the exact head, I held this helper at `PTRACE_EVENT_EXIT`
> and observed blocking `wait4(..., 0)` at both 3 and 10 seconds. The test finished only after I released it at 12
> seconds. […]
>
> Run this test’s real reaper operations in an isolated child under the existing bounded `run_isolated` harness.
> Preserve production host cancellation semantics.

**The witness** (`tools/hold-host-reaper.py`; `witness/SUMMARY.txt`). It is the reviewer's method, made independent of
where the test's body runs:
- The test binary runs under a ptrace tracer that follows every process the binary or its isolated children start.
- The test's body forks three helpers: the signal guard, the container reaper, then the host reaper. The tracer holds
  the third helper of the first test-binary process to fork three — the host reaper, wherever the body runs — at its
  `PTRACE_EVENT_EXIT` stop, which a reaper reaches only after acknowledging CANCEL.
- It holds it 180 s, past the fixed test's 120-second deadline and the 10-second collection bound after it. It samples
  every test-binary thread's system call 11 times, from 3 s to 175 s, then releases the reaper.
- Every run is under `upstroke-build` on this lane's witness base.

The runs:
- **Red at `71e55dfc`** (`witness/red-71e55dfc/`). The libtest process's test thread was in
  `wait4(<held reaper>, …, 0)` at all 11 samples. The libtest process ended 180.011 s after the hold began, 0.01 s
  after the release, with the test passing ("finished in 180.03s").
- **Green at `aed7a299`** (`witness/green-aed7a299/`; its Rust inputs are `f8b022a5`'s). The isolated child's test
  thread was in that wait, and the libtest process's in `run_isolated`'s polling rest. The libtest process ended
  120.0 s after the hold began, with the reaper still held. The test failed at its deadline, "the isolated caller
  armed and dropped a container reaper, and spawned and cancelled a host reaper: status None" ("finished in 120.02s").
- **The mutation, the test run directly again** (`witness/mutation-i3-direct-again/`). It is
  `mutation/i3-direct-again.patch`, the reverse of `f8b022a5`'s `proc.rs` diff, on a copy of `aed7a299` built from
  itself. It is red, as at `71e55dfc`: the wait at all 11 samples, and the end 180.011 s after the hold.

**Root cause.** `a_container_reaper_rests_once_between_its_ending_polls_and_a_host_reaper_as_before` came with round 2
(`FUA-I2-EINTR`, §4.2). It is a white-box test: it arms a real container reaper, spawns a real host reaper, and reads
each one's `rest`. It ended the host reaper with `cancel` in the libtest process itself. After an acknowledged CANCEL
that call waits for the reaper's exit through `ReaperEnding::AcknowledgedExit` (`src/agent/proc.rs:2459`, or
`wait_through_identity` on the pidfd path). That wait is unbounded and must stay so, because the reaper's exit is what
releases the cleanup lease. So a reaper stopped between its acknowledgement and its exit held the test, and the suite
with it, for as long as it stayed stopped. Round 2 wrote the test to read two fields and did not count the wait its
cleanup makes.

**The fix** (`f8b022a5`, test-only).
- The reaper operations move to `container_and_host_reaper_rests_child` (`src/agent/proc.rs:9215`), an ignored child.
  It arms a container reaper and drops it, spawns a host reaper, reports both rests, cancels the host reaper, and
  reports that the cancel returned.
- The rests are reported before the cancel, so a run whose cancel never returns still names them.
- The test (`:9233`) runs the child through `run_isolated` under `CONTAINER_REAPER_CHILD_BOUND` (120 s). That is the
  bound of the lease child, which also cancels a real host reaper. The test requires the child's success, its report
  that the cancel returned, and the rests `Some(SingleAttempt)` and `Resuming`.
- Production host cancellation is unchanged:
  - The expanded non-test library (`cargo rustc --lib -- -Zunpretty=expanded`, `cfg(test)` configured out;
    `tools/expand-identity.sh`, `proof/expand/RESULT.txt`) is byte-identical at `71e55dfc` and `f8b022a5` for the
    host, `x86_64-pc-windows-msvc` and `aarch64-apple-darwin`: 10,290,462, 10,121,405 and 10,274,501 bytes. The
    control, round 2's one-token production mutation `r2-e4`, makes it differ.
  - Every changed line of `src/` lies in `mod termination`'s `#[cfg(test)] mod tests`
    (`proof/nontest-hunks-71e55dfc-aed7a299.txt`).
  - The module diff proof passes (`proof/module-diff-proof-aed7a299.txt`).
- Round 2's mutations of this test still kill it; the unmutated control `mutation/i3-control/` passes.
  - `r2-e3` fails it with "rests: container Some(Resuming), host Resuming"
    (`mutation/i3-r2-e3-a-container-reaper-armed-to-resume/`).
  - `r2-e4` fails it with "rests: container Some(SingleAttempt), host SingleAttempt"
    (`mutation/i3-r2-e4-a-host-reaper-forked-to-rest-once/`).

**The sweep** (`measure/sweep-fua-i3-hostcancel.txt`). It covers every test this PR adds over the merge base: 28 run
and 9 ignored children (`measure/new-tests-71e55dfc-vs-base.txt`). It looks for a call, made in the libtest process,
into a production wait that is unbounded by design.
- **The wait and its entry points.** `AcknowledgedExit` is the only arm of `close_and_wait_reporting` that waits
  without `WNOHANG`. It is reached through `Reaper::cancel` and `Reaper::cleanup` after an acknowledgement, and
  through them from `Supervisor::finish` and `Supervisor`'s `Drop` while spawning. `Reaper` and its methods are
  private to `agent::proc::termination`, and `Supervisor` is `pub(super)`. So outside `agent::proc` a test reaches the
  wait only through a host launch.
- **Statically**, the lines this PR adds that name an entry point are the two calls this test made, now in its child,
  and `container_reaper_lease_child`'s `host.cancel()` (`src/agent/proc.rs:9417`). The lease child has run under
  `run_isolated` since it was added.
- **Dynamically**, `tools/census-waits.py` runs each new test alone under `strace -f -k` and lists the blocking
  `wait4` and `waitid` calls of the libtest process's own threads, with their leaf frames (`census/start-71e55dfc/`,
  `census/fix-aed7a299/`). 15 of the 28 new tests make the wait in their libtest process at `71e55dfc`, and 14 at
  `aed7a299`. The one that changed is this test, from one wait to none: its wait is now made by its child. All 28
  passed under the tracer at both commits.
- **The 14 make it at the end of a host launch**, through the product's own launch path, and call no reaper's wait
  themselves:
  - in seven tests, the relay writer and the relay runner this PR adds to test support, through
    `test_support::run_with_timeout` (`write_program_in_its_own_process` and `run_program_in_its_own_process`,
    `src/runner/container.rs:1539` and `:1556`);
  - in one test, the inherited-writer helper of `the_reaper_relay_writer_leaves_no_writer_in_another_threads_fork`
    (`src/runner/host/tests.rs:4743`), which runs its helper as its pre-existing sibling does (`:4691`);
  - in six `exec/tests.rs` tests, ten launches each, the git of `src/runner/container/view.rs`'s fixture
    (`HostRunner`, `:470`, unchanged since the merge base).

  None of them is a direct call, so all are outside this finding's class, and none is changed. A wait at the end of a
  launch belongs to the launch path, and host-launching tests across the suite make it.
- **The test harness's own waits** are not production's. `run_isolated` and `waited_within` are bounded
  (`FUA-I1-WATCHDOG`). `LinkedChild::kill` is filed as `PR328-LINKED-CHILD-KILL-WAITS-WITHOUT-A-DEADLINE` (P3,
  deferred to #329).

### 5.2 The residue run's recovery assertion: the filed PR281 neighbour, cited and not repaired

The regression lens compared the entries a whole suite leaves behind, at the merge base and at the head. Its files are
copied, with `SHA256SUMS`, to `residue/reviewer-copy/`, and summarised in `EXCERPT.txt`.
- Each archive compiled from itself, and both left the same 44 top-level and 154 total entries.
- The base run passed 3,000 library and 10 binary tests.
- The head run failed one library test of 3,028:
  `engine::topology::recover::tests::an_answer_published_into_the_run_directory_is_ingested_by_the_next_incarnations_first_step`.
  It failed at `src/engine/topology/recover/tests.rs:7120`, the one-shot `assert!(!rundir::is_running(…))`,
  "answer-file: and no process holds the run", in `the_runs_first_resume_by_an_incarnation_that_then_dies` (`:7105`).
- That file is a frozen test child, byte-identical to the merge base.

The lens reproduced the failure on the merge base. It held an inherited cleanup lease in a parked fork through the
unchanged assertion (`base-lease-witness.patch`, `base-lease-witness.log`), and got the same message, at line 7125 of
the patched file: the five lines it inserted moved the assertion down. That is the filed neighbour of
`PR281-CLEANUP-LEASE-HOLD-OUTLIVED-AND-ITS-UNREADABLE-TWIN`
(`findings/P2_correctness_202609131202_a-cancelled-job-hides-which-assertion-failed.md`, P2, deferred): a sibling test
thread's fork inherits the run's cleanup-lease descriptor and holds it for an instant, and a one-shot read of the
lease sees the hold. This round cites it and does not repair it. The read is in a frozen file, and the remedy is the
change that closes the inheritance, which that finding's guard names.

### 5.3 Why this round converges (`MAINTAINING.md`, "When a pull request may be looping")

This round repairs a test round 2 added, so it says why the repair converges.
- Neither signal is raised. The finding is a P2 and the premise stands.
- The defect was a missing harness, not a turn of invented machinery. The fix moves the test's real reaper operations
  into `run_isolated`, the harness this follow-up already has and round 1 bounded, and it adds no new mechanism.
- The witness holds the fix: red at `71e55dfc`, green at `aed7a299`, red again under the mutation.
- §1.5's harness rules now carry the lesson.

### 5.4 Gates, CI and platforms

The ten gates run at the pushed head, in the foreground, cargo through `upstroke-build` on this session's private
base. They and CI's legs, read by the newest run per workflow, are in the pull request body and
`~/orch-pr11/handovers/pr11_fua_r3.md`. This section is committed before they run, so it names no figure of theirs.
Before it:
- the whole suite at `aed7a299` passed 3,028 library tests (0 failed, 123 ignored) and 10 binary tests
  (`dev/test-full-aed7a299.log`);
- against `71e55dfc` the suite gained one ignored child, the new one, and lost nothing
  (`measure/new-tests-aed7a299-vs-71e55dfc.txt`).

**Platforms.**
- **Linux** (this box) runs everything. The tracer witness is Linux-only (`ptrace`, `/proc`).
- **macOS** runs the test and its child. Both are in `mod termination` (`cfg(unix)`), and `run_isolated` needs only a
  process group and `kill`. This box ran neither on macOS; CI's macOS leg does.
- **Windows** gains no test: `mod termination` is Unix-only. The expanded non-test library is identical there too
  (above).
- CI speaks for macOS and Windows.

### 5.5 The sentences this round amends

Each is marked in place with `FUA-I3-HOSTCANCEL`:
- §0: this round's entry and the next step.
- §1.5: a harness rule for a test that ends a real host reaper.
- §1.9: the files this round changes.
