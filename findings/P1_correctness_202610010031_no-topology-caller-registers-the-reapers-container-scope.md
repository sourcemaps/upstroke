---
id: PR11-REAPER-CONTAINER-SCOPE-UNREGISTERED
severity: P1
disposition: deferred
category: correctness
pr: 327
reviewed_sha: 1fe988cd140dab27731206db4812ab2a927618b7
location: src/agent/proc.rs:4735
provenance: pre_existing
first_bad: PR7's TopologyRun, which the census notes name as the registrar (docs/internals/runner/container/census.md, "What a later slice must connect")
guard: the follow-up change that wires the coordinator's container reaper, after PR11 and before G6 certifies ST-16 — ST-16 is gated "G2, G6, G7" and the packet says "PR12 may not merge until G6 passes"
---

## Failure sequence

On Unix, a schema-4 run whose pipelines run on the container runner:

    the coordinator process dies (a kill, an OOM, a crash) while its pipelines' containers run
    -> no cleanup reaper holds a container scope: nothing in `engine::topology` calls
       `set_container_reclaim_scope` (its callers are tests), and a container invocation spawns no
       process through the process funnel, so no reaper is even alive across it
    -> the dead coordinator's labeled containers run on — an agent in one keeps editing and keeps
       spending against its provider window
    -> they are reclaimed only when the next write command's startup census finds them

The packet's `os_matrix` Unix row says the reaper "additionally kills the dead coordinator's labeled
containers, closing the orphan window"; on this build that holds for the mechanism, which PR6 built
and tests at its own level (`agent::proc::tests::unix_reaper_kills_labeled_containers`), and not at
the coordinator. So the orphan window the packet documents for Windows is, on this build, Unix's as
well. Recorded by PR11 (its record's `R-AR` and §11) and filed here because nothing tracked it; PR11's
own scope is the census side under concurrency, which phase 5 exercises.

**The full review's reading** (three lenses at `a9e88039`, the PR11 record's §13, "The full review
(round R5)", where it is `FULL-SC-1`, graded P1 and triaged PR11's under concurrency): "Record R-AR
substitutes next-command census cleanup for the packet's Unix requirement: the surviving reaper kills
the dead coordinator's labeled containers, closing the orphan window. ST-16(d), included in PR11's
concurrent proof obligations, requires that behavior. Concrete sequence: start three container
pipelines → kill the coordinator on Unix → issue no further write command. Nothing registers its
container scope or keeps a supervisor alive across those invocations. The containers can continue
editing and spending indefinitely." The executed width-three test still asserts the census's half:
`a_resuming_incarnation_reclaims_its_earlier_incarnations_containers_before_its_ledgers_probes_and_admission`
says of a killed coordinator's containers "its three containers outlive it".

**How this file came back.** Filed P2 in PR11's phase 7. The full review graded it P1 (`FULL-SC-1`),
and round R5 answered it with new machinery — a run-lifetime container reaper — and deleted this file.
Review round 6 found two P1s and three P2s in that machinery, which round R6 repaired; review round 7
found two more P1s in it (`R7-D1`, `R7-D2`). That is `MAINTAINING.md`'s second looping signal, and
round R7 narrowed the pull request: the machinery is withdrawn and this file is restored under its id,
at the full review's grade (the PR11 record's §13, "Review round 7 (round R7): narrowing"). The
withdrawn implementation stays in the branch's history, the last of it at `92593723`.

**Latent.** No production run selects a container runner for a schema-4 run in this build (the PR11
record, `R-G`); the census notes say PR12 is where a run first does. It stays P1, `deferred`: on
2026-10-01 the owner kept PR11's narrowed scope and split this finding out into a follow-up change due
after PR11 and before G6, without reclassifying it and without waiving G6 (the PR11 record's §12). G6's
pass rule ends "no open critical/high finding", so the follow-up lands before G6 certifies ST-16.

## What the change that takes this up should do

Register the run's container scope (`ReaperContainerScope::new` from the run's private root and
incarnation) once run identity exists, and keep a supervisor live across every container invocation —
a reaper armed for the incarnation's whole life rather than per process spawn — so a dead coordinator's
containers are killed by its reaper as the `os_matrix` row says; or, if that is judged not worth a
mechanism, state the Unix orphan window where the Windows one is stated and leave the reclaim to the
census, which needs the owner (ST-16 (d) and the `os_matrix` row say otherwise). Witness it with a
killed coordinator whose containers are observed gone before any census runs.

**The property to establish** (round R6's): while any container this incarnation started may still be
running, an armed reaper holding the run's container scope exists; it is disarmed only once every such
container's termination is established; a normal end never kills a live container of its own; and
disarming never leaves R28 held.

**What it costs, measured in round R5** (the PR11 record's §13, `FULL-SC-1`, "Measured, before any
change"): registering the scope needs no instrument, but keeping a reaper alive across a container
invocation does — reapers are forked only by the process funnel's `Supervisor::begin`, once per host
launch, under the process-wide launch claim, so a run-lifetime reaper is a new reachable fn in
`src/agent/proc.rs`, a `CLASSIFIED_MODULES` file, with its `effects/wrappers.toml` rows (a `funnel`
row and the `drop` count). That is an instrument change, and `MAINTAINING.md` step 7's first limb
applies to it.

**Requirements, each a lesson of rounds R5 to R7** (the PR11 record's §13 has each finding whole):

1. **`R6-C2` — arm before the incarnation's first container, a resume's pre-flight probes included.**
   A resuming incarnation's frozen recovery order runs the caller's pre-flight, whose shell probe is a
   container -> the coordinator is killed inside the probe, before anything armed at the coordinator's
   entry -> the probe container survives, no R28 held, no reaper call.
2. **`R7-D1` — also arm before a fresh run's P4 creation probes.** Round R6 exempted them on ST-16
   (k), and review round 7 rejected that reading: (k) specifies the census path, it does not exempt
   fresh probes from T-CONTAINER's Unix kill and removal. Creation publishes its owner record ->
   `RunnerProbes` starts a probe container -> the coordinator is killed before `run_started`, and no
   census runs -> the container survives, R28 not held, zero reaper calls. The reviewer's control armed
   before creation as a diagnostic; whose R28 a creator's reaper holds before `run_started` is part of
   the design.
3. **`R7-D2` — validate the reaper's scope before arming it and before any probe.** A resume is handed
   a reaper built for a foreign incarnation -> the pre-flight arms it unchecked and starts its probe ->
   the coordinator is killed inside the probe -> the reaper lists the foreign label, finds nothing and
   releases R28 -> the probe keeps running.
4. **`R6-C1` — disarm only once termination is established**, never on an error return that leaves
   invocations unresolved. Three containers run -> the runtime cannot observe, stop or remove them, so
   each Runner reports its process unresolved and the coordinator returns its error with their
   registrations held -> the guard's drop cancels the reaper -> the runtime recovers and the coordinator
   dies -> all three survive with no reaper call.
5. **`R6-D1` — a failed cancellation releases or reports R28, and the signal monitor is initialized.**
   A process whose every invocation runs in a container arms the reaper without installing the monitor
   -> the reaper stops -> the cancellation is not acknowledged and sets `PENDING_TERMINATION`, which
   nothing reads -> the caller goes on with R28 held, and the next coordinator is refused.
6. **`R6-D2` — the normal-exit control must observe: the relay is bound.** The test stub found its
   relay through a variable only the two-process witness set -> the in-process control's reaper wrote
   its calls nowhere it looked -> a mutant that settled the reaper on end-of-file left every coordinator
   test green.
7. **`R6-D3` — write-then-exec fixtures go through an isolated writer.** The stub was written through
   the test process's own descriptor in the multithreaded harness -> another thread's fork kept the
   writer -> the reaper's `execv` of the stub can fail `ETXTBSY`, and it lists and kills nothing
   (`W2-HOST-TESTS-WRITE-THEN-EXEC-ETXTBSY`'s mechanism, which #162 repaired for the host shims).
8. **The CI flake — every "R28 or R17 not held" read in a test is a bounded wait, never one-shot.** A
   test's last assertion read the run's cleanup hold once, right after its refusal -> a sibling test
   thread's fork held an inherited descriptor of the cleanup lease for a moment
   (`PR281-CLEANUP-LEASE-HOLD-OUTLIVED-AND-ITS-UNREADABLE-TWIN`'s mechanism) -> `test (ubuntu-latest)`
   failed on correct code at `92593723`. Poll it false within the module's bound, as `holds_nothing`
   does.
