---
id: CLASS-INTERMITTENT-SUBPROCESS-KILL-SETTLE-RESIDUE-FAILURES
severity: P1
disposition: deferred
category: liveness
pr: 
reviewed_sha:
location: reviews/2026-09-10-gate-G4.md:540
provenance: pre_existing
first_bad:
guard: project owner, undirected
---

## Failure sequence

**A programme-wide intermittent failure rate in subprocess kill, settle and residue paths.** Members, all observed 2026-09-03: `W2-MACOS-HOST-CONTAINMENT-ROLE-GROUP-FINGERPRINT` (macOS, pre-exec process group); `PR104-WINDOWS-SETTLE-PATH-HINT-FINGERPRINT`, already in §2 (winguest, two settle kill tests, trailing slash); `PR107-WINDOWS-SETTLE-REPLAY-ALREADYSTARTED-FINGERPRINT` (winguest, the **same two** tests, `AlreadyStarted`); `PR107-LINUX-WORKSPACE-RESIDUE-EMPTY-GITDIR-FINGERPRINT` (ubuntu, residue-and-kill, empty gitdir). **Four members, four distinct fingerprints, three platforms, three subsystems.** Sightings per member are enumerated in §43 by reading **every failing test job of every CI run on `master` and the eight W1/W2 branches, per attempt**: the macOS member has **twelve** across six branches spanning 2026-09-01 to 09-03 and fires on `master` itself, and the Windows trailing-slash member has **three**, not the one and two their own entries recorded. This row exists because the sightings were being disclosed as packets happened to meet them, which is exactly the shape that let `W1-MACOS-PROC-LATE-REAPER-SELF-SIGTERM` reach four instances before anyone counted it — and re-deriving the population found every per-member count understated

## What the change that takes this up should do

Owner, as the ledger records it: project owner, undirected.

**What is established**: every member is intermittent — the identical commit produced both green and red runs, named per member; every member sits in a subprocess kill, settle or residue path; it spans all three CI platforms, so it is not one bad runner; and it is caused by no one packet — E, M1, M2, M3, M4, M5 and M6 have each shown a member, **and `master` has shown one three times**. **The macOS member predates W2's base commit** `1cbdccd` (2026-09-02T20:55Z) — its earliest sighting is 2026-09-01T11:32:42Z, 33.4 hours before the programme had a base (both stamps UTC) — which no packet-scoped reading survives. **It is not the C-004 repair**: M4's macOS failure at `c30aca0` predates that merge entirely, and a fix cannot cause a failure that happened before it landed. **On the count**: an earlier statement of this class said *five* fingerprints and counted a fifth macOS member that has since been withdrawn as an instance of the fixed row — the count here is derived from the members named above and nowhere else, which is the property that matters. **The lead, recorded as a hypothesis and asserted nowhere**: whether one launch-gate or reaper mechanism underlies members on three platforms is **untested**, and the evidence does not reach it. Repairing one member does not close this row. Full evidence and the two corrections that produced it: **§43**

Appended to `reviews/FINDINGS.md` §2 by §43, the 2026-09-03 W1/W2 decomposition review, which recorded it as pre-existing: neither introduced nor activated by the diff in front of it. The row carried no severity label; **P1** here is this migration's judgement from the consequence described above, not the reviewer's own word.

## The kill-sampler family — RELATED AND WITNESSED, and explicitly NOT members of this row

**This row limits itself to the four named fingerprints above, and that limit is load-bearing.**
`findings/P2_correctness_202609131200_the-macos-kill-tree-settle-fingerprint.md` was filed as its own
row rather than as a member here, for exactly this reason:

> *"…and this test is none of them; the macOS member is
> `runner::host::tests::every_role_reaches_the_containment_points_of_this_platform` … a different
> test and a different assertion. **Family resemblance is exactly what that row's own text forbids as
> an attribution.**"*

The four **kill samplers** below share this row's *shape* — an intermittent failure in a subprocess
kill path — and are **not** members of it. They are recorded here because
`location:` now points at the register that enumerates them, and a reader arriving there should know
what is and is not claimed.

**The register**, `reviews/2026-09-10-gate-G4.md:540`: *"`--skip sampled_` excludes six tests, counted
rather than assumed: the four kill samplers … The samplers are excluded for runtime and are run
separately, five times each."*

| sampler | site |
|---|---|
| `sampled_git_child_kills_every_residue_classified_and_recovered` | `src/workspace_manager/tests.rs:10009` |
| `sampled_git_add_and_write_tree_child_kills_every_residue_classified_and_recovered` | `src/engine/topology/attempt/tests.rs:1892` |
| `sampled_repair_materialization_child_kills_every_residue_classified_and_recovered` | `src/engine/topology/dispatch/tests.rs:1357` |
| `sampled_cherry_pick_child_kills_every_residue_classified_and_recovered` | `src/engine/topology/recover/tests.rs:10110` |

**Two of the four are witnessed** as of 2026-09-13. `sampled_git_child_kills_…` has four sightings.
`sampled_repair_materialization_child_kills_…` has one, and it is the stronger evidence: on #283 head
`bdd63edf342a` the **identical commit** produced `test (winguest)` **failure** (run 34782632803) and
then **success** (run 34783138109), with only the pull-request body — not the tree — changed between
them. Red and green on one SHA.

**Measured conditions on the host, 2026-09-13, causation NOT established.** Three QEMU guests commit
**56 vCPUs on a 32-core box (1.75x)**: `winguest-ci` and `winguest-ci-q` at `-smp 12`, and `winguest`
at `-smp 32` running ~16 hours. `cpu` PSI `avg300=6.87` against `memory` `avg300=0.16` and `io`
`avg300=0.34`; both CI guests measure `VmSwap=0`, so memory pressure is **excluded** as the
mechanism. A kill sampler passes only by winning a timing race, and *"none of the 63 kills in 64
spawns landed"* is what a timing probe reports **when it is descheduled** — it observed **nothing**,
not a wrong answer. **This is a resource observation, not a proven cause, and it does not close this
row or any sampler.**
