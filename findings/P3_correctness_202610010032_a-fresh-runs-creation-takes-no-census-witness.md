---
id: PR11-FRESH-CREATE-TAKES-NO-CENSUS-WITNESS
severity: P3
disposition: deferred
category: correctness
pr: 327
reviewed_sha: 1fe988cd140dab27731206db4812ab2a927618b7
location: src/engine/topology/create.rs:1571
provenance: pre_existing
first_bad: PR7's create_run, whose signature takes PreLockChecked and no census witness
guard: PR12, the change that gives create_run its first production caller — make create_run take startup's FreshCensused (or prove the census in that caller, with a test that fails when it is skipped)
---

## Failure sequence

    a write command creates a fresh schema-4 run: it builds a `PreLockChecked` and calls
    `create::create_run`
    -> nothing in `create_run`'s signature requires that the startup census ran first: a resume's
       chain carries `CensusComplete` through its typestate, a fresh creation carries nothing
    -> a caller that skips the census still compiles, and `create_run` runs this incarnation's
       pre-flight probes, and goes on to admission, before any dead owner's or dead incarnation's
       containers in the private root are reclaimed
    -> the packet's order — "the census completes before slot/reservation state is initialized, before
       admission, before any invocation uses an agent's credential volume, and before this
       incarnation's probes" (`admission_and_leases.permits.crash_reconstruction`) — rests on a
       sentence in `startup.rs`, not on a type

`startup::FreshCensused` exists — "a fresh run's startup census completed, under the worktree lock",
constructible only by running both halves of the census — and `create_run` does not ask for it. The
PR11 record's `R-AO` noted the gap ("Noted, not changed"); it is filed here because a precondition only
a comment carries is the shape a later caller breaks.

**Latent, and P3.** No schema-4 production caller of `create_run` exists in this build (the PR11
record, `R-G`), and the precondition is stated where the census is.

## What the change that takes this up should do

Take `FreshCensused` (or a `CensusComplete` it carries) as a parameter of `create_run`, so a creation
that skipped the census does not compile, as a resume that skipped it does not; or, if the caller must
stay free to arrange its own order, prove the census in that caller with a test that fails when the
census is removed from the path.
