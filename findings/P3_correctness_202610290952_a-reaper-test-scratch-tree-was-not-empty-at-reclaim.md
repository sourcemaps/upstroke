---
id: PR345-REAPER-TEST-SCRATCH-TREE-NOT-EMPTY-AT-RECLAIM
severity: P3
disposition: deferred
category: correctness
pr: 345
reviewed_sha: ff6ada29b3b1d65a2f92c8f20418db8190251228
location: src/agent/proc/tests.rs:3348
provenance: undetermined
first_bad:
guard: a follow-up change that establishes what still wrote into `unix_reaper_kills_labeled_containers`' bare-dies scratch tree when the tree was reclaimed, and orders that reclaim after it, with the scratch guard's reclaim panic kept
---

## Failure sequence

Observed once, and not reproduced. PR #345's CI run 40847264881, job 123358376584 (`test (macos-latest)`), ran at
`ff6ada29` through the merge `bf3dc0da`, on runner image macos-26-arm64 20261013.0424.1.

1. **The test failed in the lib harness.** `agent::proc::tests::unix_reaper_kills_labeled_containers` printed
   `... FAILED`. Its failure block reads: "panicked at src/rundir/scratch_tree.rs:761:21: scratch tree
   /var/folders/n6/hn37q6pj7sj_9dmqqc2113800000gn/T/upstroke-bare-dies-2SC531W24P was not reclaimed: Directory not
   empty (os error 66)".
2. **The lib summary** is `3423 passed; 1 failed; 131 ignored`, finished in 1502.92 s. Cargo exited 101, so main and probe
   did not run.
3. **The panic is the reclaim's, not an assertion's.** Read from the source at `ff6ada29`:
   - each of the test's three cells holds a scratch tree (`src/agent/proc/tests.rs:3348`);
   - the cell's docker stub appends every invocation's arguments to `argv.log` in that tree before it acts, and its
     `kill` and `rm` create `killing` and `removed` there (`:3356` to `:3360`);
   - when the cell ends, the tree's guard reclaims it with `fs::remove_dir_all` (`src/rundir/scratch_tree.rs:335`);
   - a failed reclaim panics only on the arm for a thread that is not unwinding (`:761`). So every assertion of the
     bare-dies cell had passed before the reclaim failed, and the path-dies and path-lives cells did not run.
4. **Nothing in the cell waits for the reaper process** (whose pid it reads at `:3409`), or for that process's docker-stub
   children, to end before the tree is reclaimed.
5. **A candidate mechanism, not established:** a process still writing into the tree while `remove_dir_all` ran left it
   not empty for the final directory removal. Neither the writer nor the cause is identified.

**Kept separate:**
- **Every earlier result** in the retained native logs is `... ok`. On macOS that is runs 40797944126, 40806319069,
  40815706494, 40820327439 and 40831274212, and the merge-queue runs 40834103792 and 40838006947; on Ubuntu it is every
  run. The test is `cfg(unix)`, so Windows does not run it.
- **The PR changes no path under `src/agent/` or `src/rundir/`.** `src/agent/proc/tests.rs`, `src/agent/proc.rs` and
  `src/rundir/scratch_tree.rs` are byte-identical at master `478592e0` and at `ff6ada29`. That establishes no cause and
  does not exclude an interaction.
- **Nothing is claimed beyond this one observation:** no flake, no pre-existing cause, no product regression, no
  harmlessness. The 45-minute budget played no part, because the job ended in 27 min 38 s.

## What the change that takes this up should do

- **Establish the writer** from a reproduction or from the reaper's own record, not from this one log.
- **If it is the reaper or its stub children,** make the cell wait for them to end before its tree is reclaimed. Keep the
  reaper's assertions unchanged.
- **If it is the reaper's own docker calls,** judge them against the reaper's contract before changing the test. The
  reaper's existing findings are `PR328-REAPER-DOCKER-WAIT-HAS-NO-DEADLINE-AFTER-SIGKILL` and
  `PR172-REAPER-ROUNDS-STUB-NEVER-RAN`.
- **Keep the scratch guard's panic on a failed reclaim:** a leaked tree must still fail the test.
