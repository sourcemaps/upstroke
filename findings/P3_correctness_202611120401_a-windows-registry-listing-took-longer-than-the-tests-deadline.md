---
id: CI-FLAKE-WINDOWS-LIST-PAST-REGISTRY-DEADLINE
severity: P3
disposition: deferred
category: correctness
pr: TBD
reviewed_sha: 47986d013001373cec29f530f75e14f26671e578
location: src/workspace_manager/tests.rs:17769
provenance: undetermined
first_bad:
guard: the change that makes the test tell a slow single git worktree list apart from a listing attempted again until its deadline, for instance by asserting the listing made no attempt beyond its first (contended_attempts for the repository unchanged across it), without widening the 500 ms bound, keeping the assertion that the add is refused within its deadline
---

# A registry listing took 528 ms on the Windows guest, past the 500 ms the test allows

## Failure sequence

`workspace_manager::tests::an_add_refuses_within_its_deadline_while_r_x_is_held_alone_and_a_list_does_not_wait`
(`src/workspace_manager/tests.rs`, line 17727 at the reviewed commit) checks two things while
another holder has the registry's lock. It requires an add to be refused within the registry's
access deadline. It also requires a listing, which takes no such lock, not to wait, and checks
that by asserting `list_took < REGISTRY_ACCESS_DEADLINE` (line 17769). In test builds that
deadline is **500 ms** (`src/workspace_manager.rs`, near line 9502).

**One sighting**, read from the job's log:

| run | job | platform | date | pull request | head |
|---|---|---|---|---|---|
| `43082686034` | `130739025387` | `test (winguest)`, the self-hosted Windows guest | 2026-11-11 | #370 | `fc2d17f9` |

The change at that head does not touch the workspace manager. The suite's result was **3367
passed, 1 failed, 112 ignored**.

The panic was at `src\workspace_manager\tests.rs:17768:5`, with the message:

> the list takes no R-X and does not wait: 528.4304ms

**The listing's duration failed, not the add's refusal.** A listing that took no lock took
528 ms, 28 ms past the bound.

**The listing is itself a registry access under that same deadline.** `worktree_records`
calls `tolerant_registry_access` with `RegistryHold::Unheld` and `Again::Attempt`
(`src/workspace_manager.rs`, near lines 7398 to 7408). A `git worktree list` that fails is
attempted again, with back-off, until the 500 ms deadline, and an attempt that begins before the
deadline runs to its end. A listing that kept failing and then succeeded on its last attempt
ends at about 500 ms plus one Git process, which fits the observed 528.43 ms as well as one slow
Git process does. Such a listing passes the test's `expect` on the listing; only the duration
assertion catches it. A listing that waited for the lock is caught elsewhere: the holder releases
the lock only after the listing returns, so a listing that needed the lock would be refused and
fail the `expect`.

So there are two candidate causes, and the record does not tell them apart:

- **one slow `git worktree list`**, a single attempt that took longer than 500 ms; or
- **a listing that failed and was attempted again until its deadline**, succeeding on an attempt
  begun just before it.

**Rate.** This failure appeared in **1 of the 63** `test (winguest)` jobs that completed among
the 86 CI runs created on 2026-11-11 UTC (62 succeeded, 1 failed, 23 were cancelled).

**The cause is not established.** Neither candidate is ruled out. The Windows guest shares its
machine with the operator's baselines and Upstroke runs, and that day the machine was often
heavily loaded, which would fit either. Whether it was loaded when this job ran was not recorded,
and the log does not say how many attempts the listing made.

## What the change that takes this up should do

- **Tell the two candidate causes apart before repairing either.** For instance, assert that the
  listing made no attempt beyond its first: the test build already counts attempts made again per
  repository (`contended_attempts` in `src/workspace_manager.rs`, near line 9416), so its value
  for the repository should not change across the listing.
- **Do not widen the 500 ms bound.** If the sighting was a listing attempted again until its
  deadline, a wider bound would hide that, and a later fix-check reading this finding would call
  it closed.
- **Keep the add's refusal within its deadline as it is.**
- **Do not retry or skip the test** to get past this.
