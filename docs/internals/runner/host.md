# `src/runner/host.rs` — the host runner, `host-v1`

Extended notes for [`src/runner/host.rs`](../../../src/runner/host.rs).

[Source on GitHub](https://github.com/sourcemaps/upstroke/blob/master/src/runner/host.rs).

The host runner implements `host-v1`. The code is the authority for what it does. The source
retains its pointer and the lock protocol beside `HostRunner`, as standards §§6, 10 and 13
require, and keeps above its `#![allow]` the two-line marker naming the funnel section of
`effects/allowlist.toml` (see "Allowlist placement" below). The item contracts below are the API
documentation; the sections after them give the rationale.

Section headings are the item names as they are spelled in the source, so a heading is the grep
string that finds the code.

## Module overview

Everything DESIGN.md:118 gives a runner — "cwd, mounts, environment, supervision, and timeout" —
for the boundary that is this machine. It wraps the process funnel in `crate::agent::proc` rather
than reimplementing it, so "process supervision, timeout, output capture, adapter parsing
unchanged" holds by construction.

Three things live here that are not in the funnel:

* **Environment composition** (DESIGN.md:258-264). The Upstroke environment is the base, the runner
  supplies the reserved keys, and `CommandSpec.env` is an overlay applied last and refused
  pre-flight if it names a reserved key. A request's credential profile
  (`RunnerRequest::credential_profile`) replaces the base's credential location for its agent, for
  a role that `supplies_credentials`.
* **The `RunnerPreflight` shell probe** (INV-23): the recorded shell executing `exit 0` through the
  Runner, role `probe(shell)`, non-slotted, a registered invocation. Only a spawn can establish
  that a shell is available.
* **The write-command startup step** (INV-18). On Windows the coordinator joins its ambient
  kill-on-close Job Object before any spawn, so every host child is a member from creation; a
  failure refuses the write command with a diagnostic.

### Allowlist placement

The funnel section of `effects/allowlist.toml` carries this module's review clause: effects only
inside site-taking APIs, no writable handle returned. `decisions.effect_site_inventory.mechanism`
(2).

## Item contracts

What each item guarantees and refuses. The source carries no rustdoc; this section is it.

### Environment composition

| Item | Contract |
|---|---|
| `RESERVED_ALWAYS` | The environment keys `host-v1` owns: `PATH`, `HOME`, `USERPROFILE`. |
| `CREDENTIAL_LOCATIONS` | Each agent's credential *location* variable — a config **directory**, never a token. |
| `reserved_keys()` | Every key an overlay may not name: `RESERVED_ALWAYS` plus every credential location. |
| `credential_location(agent)` | The credential-location variable of one agent, if the host knows one. |
| `supplies_credentials(role)` | Whether `host-v1` tells this role where an agent's credentials live. Exhaustive with no wildcard, so a new role must be classified rather than defaulting into the side that hands out credentials. |
| `starts_in_the_started_checkout(role, worker_in_checkout)` | Whether this role starts in the checkout the coordinator was started in and so keeps the inherited Git context: every probe, `decide`'s reviewer, and the implement role of a runner built for the legacy workspace. Exhaustive with no wildcard. |
| `give_work_tree_context(composed, overlay, case, run_common_dir)` | For a role in a work tree the engine made: removes the four inherited work-tree selectors and replaces an inherited `GIT_COMMON_DIR` that is not an absolute path with `run_common_dir`, matching names by `case` and leaving any name the overlay sets as the overlay set it. Refuses, before anything starts, when it must replace such a `GIT_COMMON_DIR` and `run_common_dir` is `None`. |

### `HostRunner`

The `Host` / `host-v1` `Runner`.

| Item | Contract |
|---|---|
| `new()` | A host runner over this process's environment. Infallible. |
| `with_environment(env)` | A host runner over an explicit environment. Does not clear `resolved`. |
| `with_hooks(hooks)` | Observe (and, for the ST-07 subset, inject at) the containment sub-effect points of every spawn this runner performs that does not carry its own observer. The installed observer is shared, so invocations on the runner take turns while it is installed. |
| `policy()` | The record this runner declares: `kind: Host`, `policy: host-v1`, no image, no credential volumes, no mounts. |
| `policy_digest()` | `runner_policy_sha256` of `policy()` — the marker's value and the value every container intent carries. |
| `environment()` | The environment contract this runner composes under. |
| `start_write_command()` | The write command's containment startup step (INV-18, host portion), called once, before any spawn: the free `contain_write_command` with this runner's observer. On Unix it does nothing and returns `Ok`. **Errors:** `UpstrokeError::Refused` with a diagnostic when the ambient job cannot be created or joined; the caller refuses the write command before any effect. |
| `shell_probe(shell, workspace, invocation)` | The `RunnerPreflight` shell probe, executed through this runner. **Errors:** `UpstrokeError::Refused` when the recorded shell cannot be spawned, is killed by the probe timeout, exceeds the output bound, or does not exit 0. |
| `program_for(program, composed)` | Which file `program` is at this boundary, decided at most once per `ProgramQuestion` per runner and then remembered. Increments `program_resolutions` on entry; `program_searches` moves whenever `resolve_program` runs, that is on every memo miss, including for a program that names a location and needs no search. **Errors:** first-hand, `resolve_program`'s own: `UpstrokeError::Filesystem` when a candidate cannot be examined for a reason other than its absence, `UpstrokeError::Refused` when nothing of that name is found; replayed from the memo, always `UpstrokeError::Refused` carrying the first error's text. |

**Fields.** The source keeps the lock protocol beside `HostRunner`. The runner owns both locks and
never nests them. `resolved` serializes each lookup and caches its success or error before another
caller can read it; its guard is released before `hooks` is acquired. `hooks` exists only when a
runner-level observer was installed; its guard then covers startup or an entire supervised run, so
a runner with an installed observer supervises one process at a time even when its callers are
concurrent. A call that carries its own observer (`RunnerCall::observed_by`), or a runner with none
installed — production's — takes no `hooks` guard, and its invocations run at once. Guards release
on return or unwind; a poisoned lock retains its inner state. The process funnel's RAII owners
handle child cleanup.

### `ProgramQuestion`

The key of `HostRunner::resolved`; see `ProgramQuestion` below.

### Command construction

| Item | Contract |
|---|---|
| `build_command_at(spec, program)` | A `Command` for a spec whose program the runner has already resolved to a file. On Windows the tail after `cmd.exe`'s `/C` or `/K` goes through `raw_arg`; every other argument goes through `Command::arg`. |
| `cmd_switch_index(program, spec)` | The index of `cmd.exe`'s `/C` or `/K` switch, when this spec invokes `cmd.exe`. Windows only. |

### Observables

| Item | Contract |
|---|---|
| `program_resolutions()` | How many program names **this thread** has resolved at the host boundary, memo hits included. Incremented by `HostRunner::program_for` on entry. |
| `program_searches()` | How many program names **this thread** has searched a filesystem for. Incremented by `resolve_program` on entry. |
| `containment_establishments()` | How many times **this thread** has established write-command containment. Incremented by `Contained::new`, so the count and the tokens cannot disagree. |

`RESOLUTIONS`, `SEARCHES` and `ESTABLISHMENTS` are the thread-local cells behind the three, declared
in `src/runner/host/counters.rs` and imported here ([`counters.md`](host/counters.md) says why).

### Containment

| Item | Contract |
|---|---|
| `Contained` | Proof that this process has performed its write-command containment startup (INV-18, host portion). Its field is private to `mod proof`, which has no descendants. |
| `Contained::new()` | The only constructor, private to `mod proof`: a token exists exactly when the containment step ran and returned `Ok`. |
| `contain_write_command(hooks)` | The write-command containment startup step and the proof that it ran; idempotent. **Errors:** `UpstrokeError::Refused` with a diagnostic when the ambient job cannot be created or joined. On Unix this cannot fail. |
| `start_write_command(hooks)` (free) | `contain_write_command` for a caller with nothing to prove it to — `src/main.rs`. **Errors:** whatever `contain_write_command` refuses. |

### Probe

| Item | Contract |
|---|---|
| `shell_probe_request(shell, workspace, invocation)` | The shell probe's request, for any `Runner`: the argument vector from `ShellKind::spec`, role `Probe(Shell)` (non-slotted), no agent and no credential profile, because the probe certifies the shell, not a CLI. |
| `test_support::build_command(spec)` | Test-only translation witness. Production construction stays inside the process funnel. |

Re-exported from `mod probe`: `SHELL_PROBE_COMMAND`, `SHELL_PROBE_TIMEOUT`, `run_shell_probe`.

### `mod proof`

The containment proof and its sole mint, in a module with no descendants; see `mod proof` below.

## `HostRunner::resolved`

What this runner has already decided each program name is.

DESIGN.md:612: "Probes run through that same runner, **or pre-flight could certify a host
CLI/version different from the one the attempt executes**." Running the probe through the runner
is necessary and not sufficient: `PATH` is a search order over a filesystem that moves, so a bare
name re-searched at every spawn can find a different file. With `PATH=A:B`, the same name in both,
and `A/cli` removed after pre-flight, the attempt runs `B/cli` while `Caps.version` certifies
`A/cli`.

So the answer is remembered per `HostRunner` — per boundary — and not process-wide, which would
hand one boundary's answer to another. Production constructs one runner per run or resume (in
`engine::run_harness` and `engine::resume_harness`, and in `src/engine/topology/entry.rs` for a
schema-4 run) and borrows it as `&dyn Runner` for pre-flight and every attempt, so per runner is
per run. `production_reaches_a_spawn_through_one_host_runner_per_run` keeps that true.

The key is the question, not the name: the program string with the composed `PATH` and `PATHEXT`
that answer it. It is not the whole composed environment, because `host-v1` supplies credential
locations by role (`supplies_credentials`), so a probe's environment and its attempt's differ by
design, and a memo keyed on the environment would miss on exactly the pair DESIGN.md:612 requires
to agree.

## `HostRunner::hooks`

The runner-level observer, present only when `with_hooks` installed one. An invocation that carries
its own observer in its `RunnerCall` uses it without any lock, and a runner with no observer
installed takes no lock, so invocations on one runner run concurrently;
`two_invocations_on_one_host_runner_run_at_once_each_carrying_its_own_observation` holds that (two
children that each wait for the other's marker both succeed, and each observer sees only its own
child's pid and one spawn's points). An installed observer is shared and so taken in turns, which
the tests that install one rely on; `a_runner_level_observer_still_takes_invocations_one_at_a_time`
holds that (under an installed observer, exactly one of the same pair outlives its wait).

## `HostRunner::with_environment`

It does not clear `HostRunner::resolved`. The memo is keyed on the program name with the composed
`PATH` and `PATHEXT`, so an environment that resolves a name differently asks a different question
and misses, and one that resolves it identically is entitled to the same answer. A clear would be
a line no test could see fail; the key is the mechanism, held by
`a_resolution_question_is_the_program_and_the_environment_that_answers_it`.

## `HostRunner::program_for`

`resolve_program` answers the question by searching a filesystem; this decides whether the
question is asked, at most once per `ProgramQuestion` per runner. See `HostRunner::resolved` for
why per runner and not per spawn or process-wide.

**A refusal is remembered too.** Fail-closed: a run whose pre-flight could not find `claude` on the
`PATH` it composes does not find it at the third attempt because something installed one meanwhile.
The stored value is the refusal's message, and `UpstrokeError::Refused` displays as exactly its
message, so the replayed error is the first one byte for byte —
`a_refused_name_is_refused_identically_without_asking_the_filesystem_again` holds that.

`program_resolutions` counts calls here, one per spawn, memo hit or not; `program_searches` counts
the ones that reached `resolve_program`. See `program_resolutions` and `program_searches` for why
there are two.

## `impl Runner for HostRunner` — `run`

`run` returns the boxed future the contract promises; the future does all the work in its first
poll, on the polling thread, through `HostRunner::supervise`, handing on the call's observer,
cancellation and cleanup-lease paths. Why the polling thread is
`docs/internals/runner/contract.md`'s ("Where the work happens"): the reaper's cleanup lease is
thread-scoped, and so is the observation export.

## `impl Runner for HostRunner` › `fn composed_common_dir(`

The `GIT_COMMON_DIR` the child for `request` is started with (DESIGN §8), read from
`fn child_environment(`, the same composition `supervise` gives the child:

- **The entry the child receives.** `supervise` sets the composed vector with `Command::envs`, where
  a later entry for a name replaces an earlier one, so the last match is read.
- **Names matched by the platform's rule** (`KeyCase::current()`), which is how the child's Git
  finds a name, whatever case rule the installed `HostEnvironment` composes by. On Unix, a base
  built with `KeyCase::Insensitive` that holds `git_common_dir` gives the child a variable its Git
  does not read, and this reports `Absent`.
- **The value as composed**, an `OsString`, with no lossy conversion.
- **The work-tree context included.** For a role in a work tree the engine made, the value is what
  the work-tree context leaves: an inherited absolute value as it is, an inherited relative one as
  the common directory of the repository this runner holds, an overlay's value as the overlay set it.
- **`Unknown` when the composition refuses**: an overlay naming a reserved key, or, for a role in a
  work tree the engine made, an inherited relative `GIT_COMMON_DIR` while this runner holds no
  managed repository. A refusal never becomes a reported value; the same composition refuses the
  run of that request as `NeverStarted`.

It reports what the child is started with, not what a role's own code does afterwards.

## `impl HostRunner` › `fn child_environment(`

The one composition of a child's environment: `HostEnvironment::compose` over the request's role,
agent, credential profile and overlay, from this runner's `HostEnvironment` (the process base from
`new`, or whatever `with_environment` installed), then the work-tree context for a role that does
not start in the started checkout. `supervise` gives the child exactly this after `env_clear`, and
`composed_common_dir` reads its report from it, so the two cannot disagree. A step added to the
child's composition belongs here, and the report follows it.

**The work-tree context.** For a role where `starts_in_the_started_checkout` is false, the composed
vector goes through `give_work_tree_context`: the inherited `GIT_DIR`, `GIT_WORK_TREE`,
`GIT_IMPLICIT_WORK_TREE` and `GIT_INDEX_FILE` are removed, and an inherited `GIT_COMMON_DIR` that is
not an absolute path is replaced by the common directory of the managed repository this runner
holds. Gates and review passes run in gate or review snapshots and the topology worker in its slot,
each a linked work tree the engine made. There the inherited selectors would name the started
checkout's administrative directory, index and work tree instead of the work tree's own; without
them Git reads the `.git` file `git worktree add` wrote at the work tree's top. An absolute
`GIT_COMMON_DIR` names the started checkout's common directory from anywhere, so it is kept; a
relative one resolves against whatever directory reads it, so the role is given the absolute path
it named in the started checkout. That path is the common directory `ObjectGraph::RecordedIn`
carries, which the legacy constructor asked Git for at the started checkout and canonicalized. A
runner that holds no managed repository (`new()`, or any other reading) has nothing to give, so the
composition refuses: `supervise` refuses the role as `NeverStarted` before it starts, and
`composed_common_dir` reports `Unknown`. Production builds a bare `new()` only for probes, which
keep the context. A probe keeps the whole composed vector: it starts in the coordinator's directory
(the shell probe in the run's workspace) and runs no Git there. The legacy worker keeps it too
(`worker_in_checkout`), because it works in the started checkout, where the inherited names are the
run's.

`compose` itself is unchanged by this step, so probe and execution still compose the same vector,
as the composition tests pin; the step only edits what the base inherited from the Upstroke
process, and keeps any name the request's overlay sets. The container runner composes from its
image and is not affected. The step starts no process and parses nothing: it compares names by
`KeyCase` and asks of one value only whether it is an absolute path.

## `impl HostRunner` › `fn supervise(`

**Cancellation.** A call cancelled before `supervise` begins is refused before anything else — no
include check, composition or name resolution — as `RunnerError::cancelled` with fate
`NeverStarted`; `a_call_cancelled_before_it_starts_spawns_nothing` holds that nothing was spawned
and the resolution counter did not move. Once the process runs, the funnel's supervision loop reads
the call's `Cancellation` as its `stop` predicate beside the timeout and the output bound; when it
answers yes the funnel terminates the tree through the path a timeout takes (the Unix reaper's group
settlement, the Windows private job), and `supervise` reports `RunnerError::cancelled(.., Gone)`
only because the `stop` closure recorded that it was the reason the funnel stopped. A process that
exits on its own in the same tick is reported as the output it produced.
`cancelling_a_running_invocation_terminates_it_and_reports_it_cancelled` holds the report and its
promptness against a child that would run 300 seconds; on Unix,
`cancelling_an_invocation_ends_every_process_of_its_tree` holds the tree: a child and its
background grandchild hold a fifo open, and the fifo reads end of file once the Runner has
reported.

**The observer.** The call's own `SpawnHooks` observer when it carries one, with no lock; otherwise
the runner-level observer under its guard when one is installed; otherwise a local `NoHooks`.

**The composition** is `child_environment`'s, and a refusal there is `RunnerError::never_started`.

**The leases.** The call's cleanup-lease paths go to `proc::run_with_timeout_classified`, whose Unix
reaper holds them together with those the supervising thread's scope holds, deduplicated, so a
process started on a thread that never entered the run's scope is still reaped by a reaper holding
the run's lease. `a_call_carrying_the_runs_cleanup_leases_is_reaped_by_a_reaper_that_holds_them`
holds it.

### What the error says about the process

Every error is a `RunnerError` carrying a [`ProcessFate`]. The include check of a `RecordedIn`
reading, the environment composition and the name resolution happen before anything is spawned, so
their refusals are `NeverStarted`. Everything after is the funnel's own classification through
`proc::run_with_timeout_classified`: `NeverStarted` for a spawn that created nothing, `Gone` once
the process group (Unix) or the job (Windows) was established empty, `Unresolved` where the funnel
returned without that — a containment failure after the spawn, a reaper that failed, a Windows job
cleanup that failed after the direct child exited. A reaped direct child is never the evidence,
because it says nothing about descendants that shared its group. The integration verification
relies on this: it may settle an outage terminal only on a fate that says no process survives.

### Why the include is checked before every role

Under `ObjectGraph::RecordedIn`, `supervise` asks `ManagedRepository::verify_include` before it
composes anything and does not start the role unless the include file holds exactly
`RECORDED_OBJECTS_INCLUDE`. Git reads a missing include as nothing, so a role started after the
file went missing would read the replaced graph with nothing to say so. Checking per role rather
than per run makes "the include is there whenever a role starts" true, not merely "it was there
when the run began". Every run and resume writes the file before its first role; between those, a
role that finds it missing or altered is refused as `NeverStarted`. The check only reads the file.

### Where the program name is resolved

`CommandSpec::program` is a name, not a location, and DESIGN.md:118 gives the runner the
environment, so the boundary that executes a name says which file it is: the adapter's spec carries
the bare CLI name, and the runner resolves it against the environment it composes. That happens in
`supervise` and nowhere else.

**After composition and before anything is spawned**, both load-bearing. After, because the `PATH`
it resolves against must be the one the child runs under — reserved, so no overlay names it, but
decided by the caller's `HostEnvironment`. Before, because a name that resolves to nothing is a
pre-flight refusal naming the name, not a `NotFound` from a spawn.

**Once per boundary, not once per spawn.** `HostRunner::program_for` searches the first time and
remembers the answer, so a run's pre-flight and its attempts execute the same file even if the
filesystem moves under them (DESIGN.md:612).

### Why the composed environment is the whole environment

The composed environment is the environment: base, reserved values, overlay, and nothing arriving
by a route the record does not describe. "Probe and execution compose the same base, mounts,
reserved values, and overlay, so pre-flight certifies the environment that will actually spend"
(DESIGN.md:263).

The base is `std::env::vars_os()`, so anything that iterator does not yield is not inherited. On
Windows that is the `=C:`-style per-drive current-directory variables, which only affect
drive-relative paths like `D:sub`; every process this runner starts is given an absolute
`current_dir`, so none of them can be resolving one.

## `build_command_at`

On Windows the tail after `cmd.exe`'s `/C` or `/K` is handed over raw: std's Windows quoting
escapes embedded quotes as `\"` per `CommandLineToArgvW` rules, which `cmd.exe` does not un-escape,
so the `/C` tail must go through `raw_arg` to survive intact. A runner that re-quoted the tail would
hand every gate command containing a quote to a different program than the operator wrote, only on
Windows — and `invariants_preserved` says "adapter parsing unchanged".

### Why the resolved program is a separate parameter

The resolved program is a `Path` and `CommandSpec::program` is a `String` (DESIGN.md:222). A `PATH`
directory whose name is not valid Unicode is legal on Unix, and writing the resolved path back into
the spec would have to refuse it or rewrite it into a path that names nothing. It never becomes a
`String`, so neither happens.

`cmd.exe`'s raw-tail rule is keyed on the program that will execute, not the spec's, so it survives
resolution: `cmd`, `cmd.exe` and `C:\Windows\System32\cmd.exe` all have the file stem `cmd`.

## `program_resolutions` and `program_searches`

`program_resolutions` holds the ordering: resolution happens once per spawn, before any of the
spawn. A `SpawnHooks` observer reading it at the first sub-effect point sees the resolution already
done, and done once — see
`tests::a_program_is_resolved_once_per_spawn_before_any_of_it_and_never_before_compose_refuses`. It
is incremented by `HostRunner::program_for` on entry, so a memo hit and a search count alike: it
answers "was the program decided for this spawn, and when", not "did the filesystem move".

`program_searches` holds the identity: a name is resolved once per boundary, so N spawns of one
name through one runner move `program_resolutions` by N and this by one (DESIGN.md:612). It is
incremented by `resolve_program` on entry, so it moves for a program that names a location as well
as for one that is searched for.

The two are separate because each predicate can be lost independently: a memo that never hits
satisfies "once per spawn" and reopens :612, and a resolution moved after the first containment
point satisfies "once per boundary" and reopens the ordering.

## `mod proof` — the containment proof and its sole mint

`Contained`'s field is private to the `proof` module rather than to `runner::host`. Rust privacy
reaches a module and everything below it, so a field private to `runner::host` would be
constructible from `runner::host::naming`, `::environment` and `::probe`. `proof` has no children,
so nothing outside it can reach the field, and the only route to a value is
`contain_write_command`, which performs the join. The mint stays with the type: a proof and the
only code that may create it are read together.

### `Contained`

The type makes "the ambient job is established before anything this run could spawn" a thing the
compiler checks rather than something each entry point must remember. The only values of it are
the ones `contain_write_command` returns after `proc::join_ambient_job` has succeeded.

**The module boundary is the mechanism.** With the field private to `runner::host`, three spellings
would construct one from any child module — `Contained { 0: () }`, `let c = Contained; c(())` and
`Contained(())` — none calling `Contained::new`, so none incrementing
`containment_establishments`, and only the last containing the `Contained(` needle a lexical census
looks for. A `pub(super) fn new` would let a fourth, `proof::Contained::new()`, mint a token with
no join while the counter agreed with it. Each of the four, planted in `runner::host::naming`, was
rejected, as `E0451`, `E0423`, `E0423` and `E0624`. That evidence is a review's, not a `#[test]`'s:
a runtime test cannot observe it, because the property is that the code does not compile.

`src/main.rs` has the same shape for the CLI's own dispatch (its `containment::Contained` proves
*classification*: a write command joined, a read-only one was not asked to). This is the library
half, for callers that never go through `main.rs` — the public `engine::run/run_with/run_harness`
and `resume/resume_with/resume_harness` facades, which a downstream crate may call directly.

## `contain_write_command`

Reached from `src/main.rs` (through `start_write_command`) at the top of every write command,
before any dispatch arm runs, and called by the engine's write coordinator before it touches
anything. `crash_reconstruction`: "at process start every write command creates one
non-inheritable ambient Job Object with JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE and assigns the
coordinator process itself to it … if the ambient job cannot be created or joined the write command
refuses at startup with a diagnostic before any workspace effect".

A free function because the ambient job is a property of the process, not of a runner value: it is
established once at startup, held to process exit, and must exist before anything that could spawn.
Idempotent for the same reason — `windows_job::join_ambient` memoises the process's one answer — so
a coordinator entered through the CLI, which has already joined, re-establishes at no cost and gets
its proof. `HostRunner::start_write_command` calls this function with its own observer, so there is
one join site and one mint in the crate.

### Why the observer is a parameter

No machine can make the real join fail on demand: `windows_job::join_ambient` memoises the
process's one answer, so a binary that has joined once can never observe a failure. A step that took
no observer would have no failure path a test could drive — the same reason `proc::join_ambient_job_with`
takes a `join` closure and `engine::run_contained` a `contain` parameter.

It matters most here. The public facades (`engine::run_harness`, `engine::resume_harness`) and
`src/main.rs`'s dispatch all reach this function, and it is the only place that mints a
`Contained`; if the `map` lost its short-circuit, INV-18's host portion would silently stop holding
on the platform where the failure is real. Production passes `crate::agent::proc::NoHooks` at every
call site; the suite arms an observer that refuses at `AmbientJobJoined` and checks that the refusal
comes back out with no proof minted
(`tests::the_production_containment_mint_propagates_a_join_refusal_and_mints_nothing`).

## `containment_establishments`

The same kind of observable as `proc::ambient_job_established`, which lets INV-18's host portion be
asserted, but narrower in the two ways a census of entry points needs:

* it answers on every platform: on Unix the step is a no-op, but whether the step ran is the
  question an entry point is accountable for on both;
* it counts calls on the calling thread instead of latching once per process, so it can say that
  *this* call established containment. A second write coordinator in a process would find a
  process-wide latch already true whether or not it established anything.

## `start_write_command` (free function)

`contain_write_command` for a caller with nothing to prove it to. `src/main.rs` mints its own
dispatch-level token from this step's success, and the ordering it needs is between its own two
statements. The engine facade takes the proof instead, because its ordering obligation is against a
function it calls.

It carries the observer through for the same reason `contain_write_command` takes one: a body that
dropped the refusal — `let _ = contain_write_command(hooks); Ok(())` — would leave every
`upstroke run` on Windows dispatching with no ambient job and the suite green.

## `shell_probe_request`

A free function because both runners implement the same probe — INV-23: "the RunnerPreflight — one
non-slotted shell probe (the recorded shell executing `exit 0`) and one slotted probe per recorded
agent, each a registered invocation through the run's Runner". The container runner executes this
identical request inside the recorded image.

The argument vector comes from `ShellKind::spec`, the builder a gate's command uses, so the probe
runs under exactly the invocation a gate would, including `cmd.exe`'s `/C` and PowerShell's
`-NoProfile -NonInteractive`. A probe that spelled the shell differently would certify something
else.

## `RESERVED_ALWAYS`

DESIGN.md:260-262: "each supplies role-scoped `HOME`, `PATH`, and credential locations. Adapter
overrides may select profiles or CLI behavior but may not conflict with runner-reserved keys."

`USERPROFILE` is reserved beside `HOME` because on Windows it is the home variable:
`crate::util::user_upstroke_dir` reads it first there, because Git Bash sets `HOME` to an MSYS path
the Windows file APIs cannot open. Reserving only one would let an adapter move the home directory
on one platform through the name the other trusts.

## `CREDENTIAL_LOCATIONS`

The [credential profiles passage in the capacity notes](../capacity.md#credential-profiles) names
`COPILOT_HOME` and `CLAUDE_CONFIG_DIR`; `CODEX_HOME` is the Codex CLI equivalent.

All three are reserved for every request, not only for the agent a request binds: the narrower rule
would let a gate — repository-controlled code, which no agent permission surface bounds — point
another agent's CLI at a directory of its choosing.

## `supplies_credentials`

The one thing the host scopes by role. A gate is repository-controlled code, which no agent
permission surface bounds, and the shell probe is a shell running `exit 0`; neither runs an agent
CLI, so neither is told where an agent's credentials live, even when the request names an agent.
The worker, the review and an agent probe all execute an agent CLI and need it.


## `starts_in_the_started_checkout`

A probe always keeps the coordinator's inherited Git context; the implement role keeps it only when
`worker_in_checkout` is set, which `for_legacy_workspace` does because the legacy worker works in
the started checkout. Gates and review passes always start in a gate or review snapshot, a work
tree the engine made, under either conductor, and the topology worker starts in its slot, so a
runner built by `new()` gives its implement role a work tree's context. `decide` starts its
reviewer in the checkout the command was run in, the started checkout, so it keeps that context as
the probe that admitted the reviewer did, and is never refused for a relative `GIT_COMMON_DIR` it
reads where it starts
(`decide_answers_every_role_keyed_predicate_as_review_does_but_where_it_starts`).

## `give_work_tree_context`

Applied in `child_environment` to the vector `supervise` installs and `composed_common_dir` reports
from; see that section. Names are matched by the runner's `KeyCase`, so on Windows `git_dir` and
`GIT_DIR` are one name; whether a value is absolute is `keeps_common_dir`'s answer,
`Path::is_absolute`. A name the request's overlay sets is the adapter's own choice and is kept,
selector or `GIT_COMMON_DIR`, relative or not. The refusal is an `UpstrokeError::Refused` that
reaches the caller as `RunnerError::never_started`.

## `HostRunner::worker_in_checkout`

Whether the implement role starts in the checkout the coordinator was started in. Only
`for_legacy_workspace` sets it: the legacy worker works at the run's root, where the inherited
names are the run's own. `new()` leaves it unset, so a topology runner gives its worker in a slot a
work tree's context.

## `ProgramQuestion`

Everything that decides which file a program name is at one boundary — the key of
`HostRunner::resolved`. `program` is `CommandSpec::program` verbatim; `path` and `pathext` are the
composed values, `None` when the composed environment does not carry that key at all. That is a
different question from carrying it empty, which is why they are `Option` rather than defaulted.

## `HostRunner::new`

Infallible because `host-v1`'s record is a constant with nothing to inspect;
`crate::runner::policy::resolve_host` is the checked entry point and returns the same record, which
`new_resolves_the_same_record_as_resolve_host` asserts.

## `HostRunner::for_legacy_workspace`

The runner the schema-1..3 conductor installs. It reads `ObjectGraph::RecordedIn(repository)`: its
gates, reviewers and implementers read the recorded graph in the repository the run manages
wherever their Git names its Git directory in a spelling the conditions match, and whatever the
configuration says in other repositories. Two exceptions are filed, not repaired: a spelling the
filesystem equates with the stored one but the conditions do not match reads the replaced graph in
the managed repository
(`PR326-A-SPELLING-THE-FILESYSTEM-EQUATES-READS-REPLACEMENTS-IN-THE-MANAGED-REPOSITORY`, P1), and
an alias spelled in the other case can make the conditions reach a repository beside it
(`PR326-A-JUNCTION-MAKES-A-CASE-SENSITIVE-DIRECTORY-READ-AS-FOLDING`, P2).
[`HostRunner::new`](#hostrunnernew) reads `ObjectGraph::Recorded`, the process-wide variable; the
schema-4 entry gives its runner `RecordedIn` through `with_environment` (`host_runner` in
`src/engine/topology/entry.rs`). `PR326-SCHEMA4-ROLE-ENVIRONMENT-REFUSES-REPLACEMENTS-EVERYWHERE`
records what the process-wide variable does to the repositories a gate creates for itself.

The `ManagedRepository` comes from `Workspace::recorded_objects_scope`, which `engine::run_harness`
and `engine::resume_harness` call on `Workspace::open(repo_root)` before anything else: it names the
repository's canonical common directory and writes the include file under the private root. The
two `#[cfg(test)]` coordinator entries beneath them do the same.

The runner reads the graph its producer reads. `src/workspace.rs`, which produces the v0.1
workspace and its gate snapshots, puts its Git children on the recorded graph, so a runner reading
`AsReplaced` fails `git diff --exit-code HEAD` over an untouched snapshot of a replaced tree. The
process-wide `Recorded` is not used either: the variable reaches the repositories a gate creates for
itself, and on Git 2.41 it loses to a configured `true`. See [`ObjectGraph`](host/environment.md)
for both, and for why producer and consumer must agree.

It also sets `worker_in_checkout`, so its implement role, the legacy worker at the run's root, keeps
the inherited Git context while its gates and review passes are given a work tree's context; and
the `ManagedRepository` it holds is what a relative inherited `GIT_COMMON_DIR` is replaced from.

The constructor is kept separate from `HostRunner::new` so the v0.1 path's choice of graph is stated
at the one place that path builds its runner, its `effect_free` entry in `effects/wrappers.toml`,
and the `CONSTRUCTORS` census. Its production call sites are `engine::run_harness`,
`engine::resume_harness` and the two `#[cfg(test)]` coordinator entries beneath them.

## `HostRunner::policy`

Exposed because INV-23 records it in three places — digested into the marker (P1), in full in the
private owner record (P3b), and in `run_started(4).runner` (P6).

## `HostRunner::start_write_command`

The write command's containment startup step (INV-18, host portion):

> on Windows every host child is a member of the coordinator's ambient kill-on-close Job Object from
> creation … enforced by "ambient job joined at write-command startup (refusal otherwise)"

Called once, before any spawn — the contract's `side_effect_vs_effect_ordering` is "no events;
ambient job before any spawn". On Unix there is nothing to join: containment there is the
per-invocation reaper and the isolated process group, and this returns `Ok` having done nothing.

It calls the free `contain_write_command` with this runner's observer (or `NoHooks` when none is
installed), so there is one join and one mint in the crate.
