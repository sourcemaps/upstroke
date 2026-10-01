---
id: PR11-REAPER-CONTAINER-SCOPE-UNREGISTERED
severity: P2
disposition: deferred
category: correctness
pr: 327
reviewed_sha: 1fe988cd140dab27731206db4812ab2a927618b7
location: src/agent/proc.rs:4735
provenance: pre_existing
first_bad: PR7's TopologyRun, which the census notes name as the registrar (docs/internals/runner/container/census.md, "What a later slice must connect")
guard: PR12, the slice whose production caller first runs a schema-4 run on the container runner — it registers the scope and keeps a supervisor live across container invocations, or documents the Unix orphan window as the Windows one is
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

**Latent.** No production run selects a container runner for a schema-4 run in this build (the PR11
record, `R-G`); the census notes say PR12 is where a run first does.

## What the change that takes this up should do

Register the run's container scope (`ReaperContainerScope::new` from the run's private root and
incarnation) once run identity exists, and keep a supervisor live across every container invocation —
a reaper armed for the run's whole life rather than per process spawn — so a dead coordinator's
containers are killed by its reaper as the `os_matrix` row says; or, if that is judged not worth a
mechanism, state the Unix orphan window where the Windows one is stated and leave the reclaim to the
census. Witness it with a killed coordinator whose containers are observed gone before any census runs.
