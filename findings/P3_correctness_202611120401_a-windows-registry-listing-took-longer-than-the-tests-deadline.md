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
guard: the change that makes the test show a listing does not wait for the lock without bounding the listing's own duration by the lock's deadline, or gives that bound room for a slow machine, keeping the assertion that the add is refused within its deadline
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
528 ms, 28 ms past the bound. The test uses the lock's deadline as a ceiling on how long an
unlocked listing may take. On that run the listing was slower than that, and the record does not
say why.

**Rate.** This failure appeared in **1 of the 63** `test (winguest)` jobs that completed among
the 86 CI runs created on 2026-11-11 UTC (62 succeeded, 1 failed, 23 were cancelled).

**The cause is not established.** The Windows guest shares its machine with the operator's
baselines and Upstroke runs, and that day the machine was often heavily loaded. Whether it was
loaded when this job ran was not recorded.

## What the change that takes this up should do

- **Show that the listing did not wait for the lock without using the lock's deadline as a limit
  on how long the listing takes.** For instance, compare it with a listing made when nobody holds
  the lock, or give the bound room for a slow machine.
- **Keep the add's refusal within its deadline as it is.**
- **Do not retry or skip the test** to get past this.
