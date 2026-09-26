---
id: PR322-LOCKS-IN-FUNCTION-BODIES-THE-ACTIVATION-RULE-REACHES
severity: P3
disposition: deferred
category: docs-contract
pr: 322
reviewed_sha: 81589745dff85befefabccec0241998cf255c7e8
location: src/engine/topology/prelock/tests.rs:466
provenance: pre_existing
first_bad:
guard: an assessment of each site against §6's own test, in a change that is not also fixing a P1
---

## Failure sequence

`standards/SWEEP.md`'s activation rule puts §6 and §7 over *"every line inside a hunk the change introduces or
modifies, **and the whole body of any [function it modifies]**"*. This pull request modifies functions whose
bodies contain **27 pre-existing `Mutex`/`Arc` lines across 15 functions**. They are therefore in scope under
that rule, and **none of them has been assessed against §6's own test.**

§6 reads: *"`Mutex` and `RwLock` **MUST NOT** guard state that a single owner or message passing can hold. A lock
is allowed only with a documented protected invariant, an explicit acquisition order where more than one lock
exists, and a small critical section."* The test is therefore **whether a single owner could hold that state** —
not whether the site resembles any particular other site.

**What was actually checked, stated precisely so nobody reads more into it.** The round-4 implementer confirmed
that **none of the 27 has W2's shape**: none calls `catch_unwind`, and none holds a `PathBuf` in a lock. That is
a real result and it is why none was fixed. **It is not a finding of compliance.** "Does not resemble W2" and
"a single owner cannot hold this" are different claims, and only the first has been established.

Two related sites, recorded so the next reader does not have to re-derive them:

- **W1** (`src/engine/topology/prelock/tests.rs`) keeps a `Mutex::new(None)` **identical** to the one round 4
  removed from W2. It is outside the activation rule because this pull request does not modify W1, so it was
  correctly left alone — but an identical lock next door to a removed one is exactly the kind of thing a later
  grep finds and misreads as an oversight.
- The lock round 4 removed from the sibling witness (`src/rundir/tests.rs`,
  `the_scratch_tree_is_reclaimed_on_both_exits`) was **added in round 1 of this pull request** (`f3e70039`), not
  round 2 as the orchestrator's brief said. That correction is the implementer's.

## Why it is deferred rather than fixed here, and by whose decision

**The orchestrator's ruling.** Two things make fixing it here wrong:

1. **No deviation has been demonstrated.** §6's `MUST NOT` bites only where a single owner *can* hold the state.
   Rewriting 27 lines across 15 functions on no evidence that any of them deviates would change working tests for
   no established reason, in a pull request whose subject is fixture reclamation.
2. **The activation rule is temporary by its own words** — `SWEEP.md`: *"The activation rule is temporary. It
   exists only because the tree predates the rules."* A sweep of 27 unassessed sites is the sweep's work, not a
   P1 fix's.

**What is NOT being claimed:** that the 27 are compliant. They are unassessed, and this row exists so that is on
the record rather than implied by silence.

## What the change that takes this up should do

Assess each of the 27 against §6's own test — can a single owner or message passing hold that state? — and for
each either rewrite it to a single owner, or **write down the protected invariant** §6 requires, which is the
other thing §6 permits and which none of them currently carries. Take **W1** in the same pass, since it is the
same lock as the one already removed from W2 and leaving it is only correct while nothing modifies W1.

## Reachability, against the owner's rule of 2026-09-11

**Neither limb.** These are locks in test code: not reachable in ordinary use, and not changeable by anyone
without push access.
