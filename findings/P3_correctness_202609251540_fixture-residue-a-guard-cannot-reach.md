---
id: PR321-FIXTURE-RESIDUE-A-GUARD-CANNOT-REACH
severity: P3
disposition: deferred
category: correctness
pr: 321
reviewed_sha: f3e7003975c85d4a95e3f6b8e12050fec4f174d0
location: src/workspace_manager/fixture.rs:1968
provenance: pre_existing
first_bad: PR7-SCRATCH-FIXTURE-LEAK
guard: whichever slice next opens the kill-child scaffolding, or an owner ruling on nesting a child's TMPDIR
---

## Failure sequence

`PR7-SCRATCH-FIXTURE-LEAK` closed on the measurement below: a full green suite run into a fresh
`TMPDIR` left **1,017 top-level directories, 30,797 inodes and 147 MB** at `d724fb16` and leaves
**28 directories, 123 inodes and 432 KB** at the head that closes it. What is left is this row, and
it is not the same defect: every one of the 28 is a root whose guard exists and **cannot run**,
rather than a root no guard was ever written for.

Counted from the surviving names of that run, `ls` of the fresh `TMPDIR` after
`cargo test --all-targets --all-features` returned 0:

| what survives | how many | why its `Drop` does not run |
|---|---|---|
| `upstroke-neutral-gitconfig-*` | 15 | one per process that reaches `workspace_manager::fixture::neutral_git_config`, which holds its tree in a `OnceLock`: the parent's is a `static`, whose destructor Rust never runs at exit, and the kill children re-exec this test binary and die by `std::process::abort()`, which runs no destructor either |
| `upstroke-pr4-kill-helper-*` | 4 | `runner::host::tests::spawn_funnel_kill_helper` ends at `std::process::exit(0)` on its startup-point branch, before its own removal and past every `Drop` |
| `upstroke-engine-nopools-*` | 3 | the same `OnceLock` shape as the first row, in the processes that run an engine test |
| `upstroke-{route-hermetic,review-plan,review-nopools,config-tests,config-nopools,config-hermetic}-*` | 6 | one each, in the **parent**: a `static`'s destructor is never run at process exit, which is Rust's documented behaviour and not a defect of these fixtures |

So every one of the 28 is either a `OnceLock`'s tree, whose `static` destructor Rust never runs, or
a process that ends by `abort()` or `exit` without unwinding — one per process in both cases,
rather than one per fixture.

## What the change that takes this up should do

**The mechanism is known and is not free.** The scaffolding already has the answer for a kill
child: `engine::topology::scaffold::kill_child_and_adopt_in_a_scratch_tree` hands the child
`TMPDIR`, `TMP` and `TEMP` pointing at a tree the **parent** guards, so the child's roots are
inside a tree whose guard does run. Applying it to the seven `kill_child_and_adopt` call sites and
to `spawn_funnel_kill_helper` would close the four `kill-helper` roots and whichever of the `OnceLock` roots belong to a child.

It was **not** applied here, and the reason is measured rather than a preference: nesting a child's
temporary directory one level deeper spends the Windows path budget twice. Git for Windows dies
with `fatal: '$GIT_DIR' too big` when `$GIT_DIR` exceeds `PATH_MAX - 40`, which is 220 characters,
and the five sites that already nest are the deepest chains the suite builds — measured at 198
characters at this head, against 246 before the root name was shortened, on a `TMPDIR` head the
same length as the CI guest's. Each extra nested root costs another `upstroke-<tag>-<10>` segment.
Adding seven more such chains is a change whose cost is paid on a leg this session cannot run.
**The owner should rule on it rather than an implementer assuming it.**

The seven `static` roots are a different question and probably not worth a change: one directory
per process, created once, is the price of a fixture every test in a module reads, and the
alternative — a tree per caller — is the defect `PR7-SCRATCH-FIXTURE-LEAK` just closed.

**Severity.** `P3` is this row's own judgement. The consequence is 28 directories and 123 inodes
per suite run on a machine where nothing sweeps, against the 1,017 and 30,797 the closed row
carried; at that rate the exhaustion `PR7` measured (58,466,304 inodes) is about 475,000 suite runs
away rather than 1,900. No correctness symptom is reachable from it: a recycled pid cannot collide
with a ULID-named root, which is what made the Windows half of `PR7` a correctness defect.
