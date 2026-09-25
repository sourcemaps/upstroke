---
id: PR321-FIXTURE-RESIDUE-A-GUARD-CANNOT-REACH
severity: P3
disposition: deferred
category: correctness
pr: 321
reviewed_sha: 9d42322c24372033a145f7cf52d8fb65018ef74e
location: src/workspace_manager/fixture.rs:1968
provenance: pre_existing
first_bad: PR7-SCRATCH-FIXTURE-LEAK
guard: whichever slice next opens the kill-child scaffolding, or an owner ruling on nesting a child's TMPDIR
---

## Failure sequence

`PR7-SCRATCH-FIXTURE-LEAK` closed on the measurement below: a full green suite run into a fresh
`TMPDIR` left **1,017 top-level directories, 30,797 inodes and 147 MB** at `d724fb16` and leaves
**29 directories, 130 inodes and 468 KB** at the head that closes it. What is left is this row, and
it is not the same defect: every one of the 29 is a root whose guard exists and **cannot run**,
rather than a root no guard was ever written for.

Counted from the surviving names of that run, `ls` of the fresh `TMPDIR` after
`cargo test --all-targets --all-features` returned 0:

| what survives | how many | why its `Drop` does not run |
|---|---|---|
| `upstroke-scratch-neutral-gitconfig-*` | 15 | one per **child process**. `workspace_manager::fixture::neutral_git_config` acquires a tree in a `OnceLock`; the kill children re-exec this test binary and die by `std::process::abort()`, which runs no destructor |
| `upstroke-pr4-kill-helper-*` | 4 | `runner::host::tests::spawn_funnel_kill_helper` ends at `std::process::exit(0)` on its startup-point branch, before its own removal and past every `Drop` |
| `upstroke-scratch-engine-nopools-*` | 3 | one per child process, same `OnceLock` shape as the first row |
| `upstroke-scratch-{route-hermetic,review-plan,review-nopools,config-tests,config-nopools,config-hermetic}-*` | 6 | one each, in the **parent**: a `static`'s destructor is never run at process exit, which is Rust's documented behaviour and not a defect of these fixtures |
| the sixth `config` root | 1 | as above |

So the residue is 19 roots belonging to processes that end without unwinding and 7 belonging to
`static`s in the parent, one per process in both cases rather than one per fixture.

## What the change that takes this up should do

**The mechanism is known and is not free.** The scaffolding already has the answer for a kill
child: `engine::topology::scaffold::kill_child_and_adopt_in_a_scratch_tree` hands the child
`TMPDIR`, `TMP` and `TEMP` pointing at a tree the **parent** guards, so the child's roots are
inside a tree whose guard does run. Applying it to the seven `kill_child_and_adopt` call sites and
to `spawn_funnel_kill_helper` would close 19 of the 29.

It was **not** applied here, and the reason is measurable rather than a preference: nesting a
child's temporary directory one level deeper is what turned #292's Windows CI leg red. Git for
Windows refuses `git worktree add` when the administrative `.git` path exceeds 220 characters
(`'$GIT_DIR' too big`), the kill children build worktrees, and
`temp_dir()/upstroke-scratch-<tag>-<ulid>/` adds about 45 characters to every path beneath it on a
runner whose `TEMP` is already long. The five sites that do nest are in `master` and green, so one
level is evidently survivable; adding seven more is a change whose cost is paid on a leg this
session cannot run. **The owner should rule on it rather than an implementer assuming it.**

The seven `static` roots are a different question and probably not worth a change: one directory
per process, created once, is the price of a fixture every test in a module reads, and the
alternative — a tree per caller — is the defect `PR7-SCRATCH-FIXTURE-LEAK` just closed.

**Severity.** `P3` is this row's own judgement. The consequence is 29 directories and 130 inodes
per suite run on a machine where nothing sweeps, against the 1,017 and 30,797 the closed row
carried; at that rate the exhaustion `PR7` measured (58,466,304 inodes) is about 450,000 suite runs
away rather than 1,900. No correctness symptom is reachable from it: a recycled pid cannot collide
with a ULID-named root, which is what made the Windows half of `PR7` a correctness defect.
