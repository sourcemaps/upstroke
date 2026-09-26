---
id: PR322-MODE-CHANGING-WITNESSES-CAN-LEAVE-A-TREE-RM-CANNOT-REMOVE
severity: P3
disposition: deferred
category: crash-consistency
pr: 322
reviewed_sha: c2586eac04b52daafbb2ec18b110a3d454b5585d
location: src/runner/container/census/tests.rs:3822
provenance: pre_existing
first_bad: all seven sites are present at a3767bcc, the merge base of #322
guard: a change that arranges each failing removal the way `rundir::tests`'s `PlantedRoot::lock_a_child` does (an empty 0o000 child, which `rm -rf` removes), or that restores every mode through a guard declared before the change and asserts nothing between
---

## Failure sequence

Seven Unix-only tests take the write or search permission off a directory inside their fixture to
make a removal or a listing fail, and put it back afterwards. If the test ends inside that window,
the tree is left with a directory `rm -rf` cannot empty — it cannot list or unlink inside a
directory it cannot search or write — and the build box's out-of-tree sweeper
(`find … -name 'upstroke-*' -exec rm -rf {} +`) then fails on that root on every run for good.
When a failing assertion ends the test inside the window, the fixture's own guard fails the same way
on its reclaim and, the thread being already unwinding, reports the failure on stderr and
suppresses it rather than raising it; after a kill no guard runs at all.

Found by #322's round-2 delta review (its D5) after its replay lens's R-3 was repaired at the one
witness that lens named, where the mechanism was executed: a SIGKILL inside the window, then the
sweeper's own command, which exited 1 with `Permission denied` on the residue. At these seven it is
reasoned from that mechanism, not executed. Each line below was read at `c2586eac`, and each occurs
once at `a3767bcc`:

| site at `c2586eac` | mode | restored |
|---|---|---|
| `src/runner/container/census/tests.rs:3822` | `0o500` on the views directory | only on the normal path, so a failing assertion in the window leaves it too |
| `src/runner/container/tests.rs:4507` | `0o500` on a view's parent | only on the normal path |
| `src/rundir/tests.rs:3987` | `0o500` on a directory the test seals | only on the normal path |
| `src/workspace_manager/tests.rs:3920` | `0o000` on the intents directory | by `RestoreMode`'s `Drop`, so only a kill leaves it |
| `src/workspace_manager/tests.rs:3952` | `0o555` on the root | by `RestoreMode`'s `Drop` |
| `src/workspace_manager/tests.rs:11110` | `0o000` on a git directory | by `RestoreMode`'s `Drop` |
| `src/workspace_manager/tests.rs:14125` | `0o000` on a fan-out directory | by `RestoreMode`'s `Drop` |

## Why it is deferred rather than fixed here

All seven predate #322, which did not introduce or touch any of these windows; the orchestrator's
round-3 brief directs the row filed and the fix kept out of that pull request, whose class is the
fixture roots' naming and reclamation, so as not to widen its diff. No failing test or mutation
witness is attached — the evidence is the executed mechanism at R-3's site and a reading of these
seven — so `MAINTAINING.md` step 5's non-discretionary limb does not reach it.

## What the change that takes this up should do

Per site, either arrange the failure with an **empty** directory at `0o000` — `rm -rf` removes an
empty directory it cannot read, because removing it needs only the parent's write bit, which is how
R-3 was repaired (`PlantedRoot::lock_a_child`) — or, where the test needs a populated directory
without permission, restore the mode through a guard declared before the change, as `RestoreMode`
already does for four of the seven, so that at least a failing assertion cannot leave it. A kill
inside the window stays possible for any mode change; the replay lens's out-of-tree mitigation, a
`chmod -R u+w` before the sweeper's `rm -rf`, covers that residue and is the box's to adopt.

## Reachability, against the owner's rule of 2026-09-11

**Neither limb.** Test-only residue on the machine that ran the test; no user input reaches it and
nothing a required check runs changes.
