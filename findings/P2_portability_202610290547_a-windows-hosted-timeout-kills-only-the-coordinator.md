---
id: PR345-WINDOWS-HOSTED-TIMEOUT-LEAVES-MANAGER-GIT
severity: P2
disposition: deferred
category: portability
pr: 345
reviewed_sha: 713dc01f6b8f8249593ed217bb3e76b3e61ff912
location: src/engine/topology/coordinator.rs:16415
provenance: introduced_by_feature
first_bad: ab2947b755335da8e114d1c82bdce201b336c960
guard: a follow-up change to the T-ENV-TOPO Windows helper f_hosted_isolated that holds the helper's whole process tree in a job of its own and confirms every descendant has ended before the fixture is reclaimed
---

## Failure sequence

PR #345's review 1 (the regular lens, gpt-6-astra at max) derived this from the source at `713dc01f`.
- No build, test or reproduction ran.
- No surviving process has been observed.

1. **The parent starts the child.** TOPO4 or TOPO5 runs on Windows. Its parent starts the hosted child through
   `f_hosted_isolated` (`src/engine/topology/coordinator.rs:16405`), which spawns it as a `LinkedChild` and waits 300
   seconds.
2. **The child's manager starts Git directly,** with `Command::output` (`src/workspace_manager.rs:6647`). For example,
   the candidate stage's `git add` takes the index lock.
3. **The kill reaches the direct child only.** The child exceeds the bound, and `f_hosted_isolated` calls `child.kill()`
   (`:16415`). `LinkedChild::kill` (`src/workspace_manager/fixture.rs:3860`) is `Child::kill`, which ends the direct
   child only.
4. **No helper-owned job tied to the child's lifetime holds the manager's Git.**
   - The child's path runs `WideEnv::adopted_over`, the scaffold's `resume`, `f_hosted_runner` and `run_concurrently`.
     None of these joins the kill-on-close ambient job.
   - No non-test topology code calls `contain_write_command`, `start_write_command` or `join_ambient_job`.
   - The roles' per-role Windows jobs (`src/agent/proc.rs:944`) hold the roles, not the manager's own Git children.
   - An enclosing CI or session job, if any, is not excluded by this source inspection. It would not by itself end
     those processes when this helper kills its child.
5. **So the Git process can outlive the killed child,** holding its lock or still writing, while the parent's failure
   unwinds into fixture reclamation.

**Context:**
- The Unix counterpart, `run_test_isolated`, kills the isolated process group (`src/agent/proc.rs:9993`).
- This is a timeout path only. It was not observed in the four recorded Windows runs (40797944126, 40806319069,
  40815706494 and 40820327439), in which TOPO4 and TOPO5 passed.
- This finding establishes no cause of any historical CI failure.

## What the change that takes this up should do

- **Hold the child in a job the helper owns,** with kill-on-close, using the existing job supervision in
  `src/agent/proc.rs`.
- **On a timeout,** end that job and confirm that its processes are gone before returning. The fixture is then reclaimed
  only after every descendant has ended.
- **Keep the Unix path** as it is.
