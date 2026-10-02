# PR11 follow-up C — a dead coordinator's Git writers and the slot a resume reuses: the working record

The record of the change that repairs `PR329-A-RESUME-REBINDS-A-SLOT-ITS-DEAD-COORDINATORS-GIT-CHILD-STILL-WRITES`
(`findings/P1_correctness_202610020436_a-resume-rebinds-a-slot-its-dead-coordinators-git-child-still-writes.md`, P1,
`deferred`, `pre_existing`). It is kept on the branch so that a successor session inherits what was decided and why.
Like the PR11 record (`reviews/2026-09-30-pr11-record.md`) and follow-up B's
(`reviews/2026-10-01-pr11-follow-up-b-record.md`, on #329's branch), it is **not** a design document. `DESIGN.md` and
the packet stay the authority, and a sentence here that disagrees with either is a defect in this file.

**Branch.** `fix-P1/correctness_a-resume-rebinds-a-slot-its-dead-coordinators-git-child-still-writes`, cut from master
at `92c4ca81f9209d218df4534ee71d3445dc2906e1`, the merge commit of #327 (PR11). It is a `fix-P1/` lane: review effort
`max`, and every P0 and P1 is fixed before the pull request is ready.

**Why this change exists.** #329's design rounds 1 to 3 (follow-up B) tried three mechanisms to keep a resume from
reusing a slot while a dead coordinator's Git writer could still act on it, and each review found a process the
mechanism missed. On 2026-10-02 the PR11 orchestrator split that class out of #329
(`~/orch-pr11/reviews/review-329-d3-triage.md`), and #329's design round 4 filed it as the finding above, P1 and
G6-blocking, for this follow-up. The orchestrator's
briefs are `~/orch-pr11/briefs/followups/fu-c-orphan-git-writers-before-slot-reuse.md` and
`~/orch-pr11/briefs/pr11_fuc_design.md`.

**The shared finding file, and the merge order.** The finding is not on master: #329 filed it at `a6135a66`. This
branch's first commit (`fa28d140`) carries it byte for byte (blob `d8d4860e`), the shape a fix-P branch takes when it
files its own finding, and `validate-pr-branch.sh` resolves the branch name against it.
- If #329 merges first, this branch rebases onto master and the identical add disappears.
- If this change merges first, its repair deletes the file, and #329 drops its copy and points its ledger row here when
  it rebases. The orchestrator handles #329's side.

**Who writes it.** The design phase (§1) is `pr11_fuc_design`'s (`claude-opus-5-5`, `max`), a fresh session the PR11
orchestrator spawned on this branch at `92c4ca81`. Its figures are under `~/orch-pr11/logs/pr11_fuc_design/`, cited as
`c1/…`. #329's figures are cited by their own paths: `~/orch-pr11/logs/pr11_fub_design3/` as `d3/…`, and
`~/orch-pr11/reviews/329-d3-witnesses/`. A fresh implementer writes the code after the design review and the owner's
decisions, and its sections follow §1.

**The evidence plan was conservative, by direction.** Read our code, read Git's source at the three versions that
matter, cite the corruption witnesses #329 already executed at base rather than rebuild them, and run one new witness
that uses only our own `git` commands in a temporary directory. That witness holds its filter with a release file, and
its only signal is a `SIGKILL` of its own coordinator child. Nothing was traced, preloaded or injected.

## 0. Status

| Phase | State |
|---|---|
| Design (§1) | **PROPOSED — pending the owner's decisions D1 and D2 (§1.12).** On Unix the remedy is the brief's candidate (1): every engine Git child runs in one process group per write command, led by a sentinel and recorded durably before any Git child joins it, and the next write command of the checkout reuses nothing until that group is established empty. On Windows no in-lane mechanism can observe a dead coordinator's job empty (§1.6), so the Windows treatment is the owner's (D2). The packet's resource and site inventories must name the new record and its observation (D1). This head changes no production code. |
| Implementation | not started. It waits on the design review and on D1 and D2. It does not depend on #328 (follow-up A) or #329 (follow-up B) beyond the merge order of the shared finding file (§1.13). |

## 1. Design

### 1.1 The defect, and what closing it means

**The sequence, with one coordinator.** The finding states it; #329 executed it.
- Coordinator A is killed inside an engine Git write, for instance `git worktree add` for the task slot
  `tasks/k<key>-g<gen>`.
- On Unix nothing kills A's Git children. The Git process and every process it starts outlive A.
- A's resume takes the worktree and run locks. `verify_worktree` reads Git's `locked: initializing` as
  `VerifyFailure::Unpopulated` (`src/workspace_manager.rs:2772`).
- `dispatch::verify_or_recreate` removes the slot and adds it again at the same path
  (`src/engine/topology/dispatch.rs:251-252`), and Git names the registration after the path's basename, so the
  recreated registration has the same administrative directory.
- The orphan acts on the slot's paths after the worker has written its paid edits.

**What #329 executed at base,** all on Git 2.43.0 through the production `WorkspaceManager` funnels and the
coordinator's real locks:
- the add's late `reset --hard` turned `paid worker edits` back into `base`
  (`d3/witness/c-single/witness-locks-exec-base.log`, `VERDICT exec/base … PAID EDITS LOST`);
- the orphaned add's `remove_junk` deleted the recreated registration and checkout
  (`d3/witness/c-single/witness-locks-filter-base.log`, `VERDICT filter/base … PAID EDITS LOST`);
- a smudge filter's background helper, which stayed in the original process group with no `setsid`, reset the recreated
  checkout after Git had exited (`~/orch-pr11/reviews/329-d3-witnesses/pr329-d3-reg-recreated-hjzj6eur/result.txt`:
  `helper in same process group=True`, `after helper='base\n'`);
- review round 2's lenses executed an orphaned `update-ref` rewriting a replacement registration's `HEAD`
  (`~/orch-pr11/reviews/review-329-d2-{design,concurrency}-0874bcf3.review.md`).

**What closing it means.** No write command of the checkout resets, admits or reuses a slot, or anything a slot's
registration names, while any process that a dead coordinator's engine Git writes started can still act on it. The
finding's four requirements bind the remedy:
1. it holds for processes Git starts that the engine never sees (`update-ref`, `reset`, a filter and what it starts,
   `checkout--worker`);
2. it depends on no descriptor inherited through standard input, no record updated in place, and no `/proc` visibility;
3. on Windows it observes completion, not only the start of termination;
4. the resume's refusal stays resumable and says what it waits for, and a hung writer may hold recovery until an
   operator ends it, which is stated.

### 1.2 Item 1: every engine Git write, and what it can leave behind

**One builder.** Every Git child the manager starts comes from `WorkspaceManager::command`
(`src/workspace_manager.rs:4994-5009`), through `git` (`:4946`), `git_with_identity` (`:5011`) or `update_ref`
(`:3553`). It disables hooks (`core.hooksPath` to the manager's empty hooks directory) and the fsmonitor, and gives the
child `/dev/null` as standard input (`:5007`). The only other spawner, `read_only_git` (`:5452`), passes
`--no-optional-locks` (`:5456`) and runs reads only. The line numbers are in `c1/code-citations.txt`.

**The writes, at this head:**

| Call site | Command | What it writes |
|---|---|---|
| `add_worktree`, `:2708` (task, staging and snapshot slots) | `worktree add --detach --quiet <slot> <commit>` | the registration under `.git/worktrees/<basename>` and the checkout |
| `remove_bound`, `:3061`, `:3100`, `:3123` | `worktree prune` | deletes registrations Git judges stale |
| `create_ref_zero_old`, `compare_and_swap_ref`, `delete_ref_expected_old`, `:3348`, `:3385`, `:3411` | `update-ref --no-deref …` | a ref and its lock file |
| `candidate_stage`, `:3819`, `:3833`, `:3849`, `:3859` | `add -- :(literal)<p>`, `rm --quiet --force -- …`, `add -u -- …`, `add -A -- .`, `clean --quiet --force -x -- …` | the slot's index and working tree |
| `candidate_write_tree`, `:3883` | `write-tree` | objects |
| `commit_tree`, `:3970` | `commit-tree <tree> -p <parent> -m <msg>` | an object |
| `proposal_cherry_pick`, `:4031` | `cherry-pick <commit>` | the staging slot's index, working tree and `HEAD` |
| `repair_materialize`, `:4244`, `:4253` | `read-tree --reset -u HEAD`, then `cherry-pick --no-commit <commit>` | the slot's index and working tree |

The builder's reads write nothing in a slot: `rev-parse`, `worktree list`, `for-each-ref`, `ls-files`, `diff-files`,
`cat-file`, `diff --cached` (index against a commit, no refresh) and `diff <parent> <tree>` (two trees).

**What those commands start, at Git 2.43.0, 2.50.1 and 2.55.0** (`c1/git-src/<tag>/…`, the line numbers in
`c1/git-src-citations.txt`):
- **`worktree add`** runs `reset --hard --no-recurse-submodules` as a child, with `GIT_DIR` and `GIT_WORK_TREE`
  naming the new slot (`builtin/worktree.c:389-394` at 2.43.0, `:404-409` at 2.50.1 and 2.55.0). At 2.43.0 it also
  runs `update-ref HEAD <commit>` as a child (`:529-542`); 2.50.1 and 2.55.0 write `HEAD` in process
  (`refs_update_ref`, `:528` and `:563`). On failure it runs `remove_junk`, which deletes the checkout and the
  administrative directory **by path** (`:258`, `:469-470` at 2.43.0).
- **A checkout** (`reset --hard`, `read-tree -u`, `cherry-pick`) runs a configured smudge filter as a shell child with a
  pipe for its input (`convert.c:653-657` at 2.43.0), or a long-running filter process with pipes that Git does not
  clean up on exit (`sub-process.c:60`, `:89-96`). With `checkout.workers` above one and at least
  `checkout.thresholdForParallelism` eligible entries (default 100, `parallel-checkout.c:38`, `:668`), it runs
  `checkout--worker` children over pipes (`:474-481`), each writing the entries it is given. Entries that need a filter
  never go to a worker (`is_eligible_for_parallel_checkout`, `CA_CLASS_INCORE_FILTER` and `CA_CLASS_INCORE_PROCESS`).
- **`add`** runs clean filters the same way.
- **`cherry-pick`** runs a configured merge driver as a shell child (`merge-ll.c:229-231` at 2.43.0). It commits a
  plain pick in process (`sequencer.c:1673`) and starts a `git commit` child only to report an error (`:1537`, `:1570`,
  `:1690`).
- **None leaves its process group.** `run-command.c` calls neither `setpgid` nor `setsid` at any of the three
  versions (count 0 in each). Git's only `setsid` is `daemonize()` (`setup.c:1695-1710` at 2.43.0), which `gc` and
  maintenance use to detach, and none of the engine's writer builtins, `reset`, `checkout--worker` or the sequencer
  calls `run_auto_maintenance` (count 0 in each file, at each version).

**Which can write a slot path.** Every one of them except `write-tree`, `commit-tree` and the reads:
- the checkout child and the processes it starts write the working tree;
- `update-ref` at 2.43.0 writes the registration's `HEAD` through `GIT_DIR`, a path the recreated registration reuses;
- `remove_junk` deletes both trees by path;
- a filter and anything it starts can do anything an absolute path lets it do, which is what #329's helper did;
- `worktree prune` deletes registrations;
- the `update-ref` the manager runs writes a ref and its lock under the common directory, which the cleanup lease
  already orders (§1.4, "What this leaves alone").

**What CI's versions change.** CI runs Git 2.55.0, and the Windows guest 2.50.1.windows.1. Neither has the `update-ref`
child. Both keep the `reset --hard` child, the filters, `checkout--worker` and `remove_junk`. Git for Windows creates
every child with `CreateProcessW` and restricts the handles it inherits to its three standard handles
(`c1/git-src/gfw-v2.50.1.windows.1/compat/mingw.c:2131-2138`, `:2209`).

### 1.3 The candidates, evaluated

| Candidate | Unix | Windows | Packet | User-visible |
|---|---|---|---|---|
| **(1) group or job emptiness before reuse** | **sound** for every process Git starts, because membership is inherited (§1.5). A process that deliberately leaves the group escapes, which is residual R-1 (§1.7). | membership is inherited, but no successor can observe a dead coordinator's job empty without a handle held outside it (§1.6) | a resource row and three sites (D1) | none |
| (2) unique slot paths per incarnation | sound for slot paths without observing any process | sound for slot paths | **changes** the literal `tasks/k<key>-g<gen>` and `merge/s<seq>` naming, T-DISPATCH's resume action, and R9, R10 and R24's "reused only after `Worktree.Verify`" lifecycles | an orphan keeps writing into the old path until it ends, as residue the next reclaim meets |
| (3) filters and parallel checkout disabled | **not sufficient**: Git's own `reset` and `update-ref` children outlive the coordinator, as #329's exec witness shows | the same | none | **severe**: an LFS repository's slots hold pointer files, a `git-crypt` repository's slots hold ciphertext, and a clean filter no longer runs at staging |
| (4) a documented residual | only for what (1) leaves on Unix, R-1 | the whole Windows half, if the owner chooses it (D2) | an owner's decision | none |

**Why (1) on Unix.** A process group is the one containment Unix gives every descendant by default. Leaving it takes an
explicit `setsid` or `setpgid`, and Git never makes either call for anything the engine's writes start (§1.2). Its
emptiness is a kernel answer that needs no `/proc`: `kill(-pgid, 0)` fails with `ESRCH` exactly when no process has
that group. Each round of #329 failed on a process its mechanism had to enumerate or that had to inherit a descriptor.
The group needs neither, and it holds the cases that defeated round 3: the filter's helper and `checkout--worker`
(§1.5, executed).

**Why not (2).** It is the owner's packet decision, and it does more than (1) needs on Unix. It is the alternative the
owner may prefer for Windows (D2c).

**Why not (3).** It does not close the class, and its cost lands on every user of a filter.

### 1.4 The remedy on Unix: each write command's Git writer group

**The group.**
- **The sentinel.** At its first Git child under the worktree lease, the manager starts a sentinel through its own
  builder: `git hash-object --stdin`, with a pipe for standard input whose write end the coordinator holds
  (close-on-exec, so no Git child inherits it), with `/dev/null` for its output, and with `process_group(0)`. Its pid is
  the group id `P`. `git hash-object --stdin` reads to end of input and, without `-w`, writes nothing, so the sentinel
  lives as long as the pipe's write end is open. When the coordinator dies it reads end of file and exits. A forked
  helper that kept a copy of the write end would only delay that, and so the wait, until the helper ends.
- **The record, before any member.** The manager then writes the checkout's writer record and makes it durable: a
  temporary file in the checkout's Git directory, synced, renamed to `upstroke-writer-group` beside the
  `upstroke-worktree.lock` the worktree lease already lives in (R25), and the directory synced. On Unix it names the
  boot (`/proc/sys/kernel/random/boot_id` on Linux, `kern.bootsessionuuid` on macOS) and `P`. It is written once,
  never updated in place, and only after it is durable may any Git child join `P`.
- **Membership.** Every Git child the builder spawns while the group exists gets `process_group(P)`, `std`'s own
  setting (`std::os::unix::process::CommandExt::process_group`), applied in the child before `exec`, so the child is in
  `P` from its first instruction of Git. It needs no `pre_exec` closure, which is what took `std` off `posix_spawn` and
  tripled the suite in #329's round 3 (`d3/census/probe-iii/SUMMARY-D1.txt`). That `std` keeps `posix_spawn` with it
  (`POSIX_SPAWN_SETPGROUP`) is not read here; the implementation's suite time against round 3's figures confirms it.
  Every process that child starts inherits `P`.
- **Release.** When the command gives its worktree lease back, it closes the sentinel's pipe, reaps the sentinel, and
  asks `kill(-P, 0)`. On `ESRCH` it removes the record. Otherwise a member has outlived the command, a filter's helper
  say, and the record stays for the next command to wait on. Release happens before the lease's OS lock is let go, so
  the next holder never meets a live coordinator's record.

**The wait: the recovery-side half, at worktree-lease acquisition.** `WorktreeLock::acquire_in_hooked`
(`src/rundir.rs:1906`) already refuses while any run of the checkout still holds the cleanup lease (`:1962-1975`).
Directly after that check it reads the writer record, if one is there:
- a record from another boot is removed: every process of that boot is gone;
- `kill(-P, 0)` answering `ESRCH` means the group is empty, so the record is removed and the command proceeds;
- an answer of success or `EPERM` (members, or members this user may not signal) means it waits, polling every 10 ms;
- if the group is still not empty after 10 s, the command refuses resumably. The refusal names the group, the record
  and the reason, says that a previous coordinator's Git process is either finishing or hung, and says how to end it
  or, if the group number has been reused, how to clear the record.

Every write command of the checkout passes there first. The topology resume takes the worktree lease before the run
lock and before any recovery step (`LocksHeld::take`, `src/engine/topology/recover.rs:271-272`). A fresh topology run's
startup census needs a `WorktreeLocked` witness (`src/engine/topology/startup.rs:531-541`). The legacy commands take
the same lease (`src/engine/coordinator.rs:132`, `src/engine/resume.rs:148`). So no slot is reset, admitted or reused,
in any run of the checkout, while a dead coordinator's writer group has a member.

**Placement.** Nothing frozen moves (§1.8):
- the lease side, which is the record's read, wait and removal and the process-local table that tells a manager its
  checkout's group, goes in `src/rundir.rs`;
- the group's start and membership go in `src/workspace_manager.rs`'s builder and in private functions beside it;
- `src/engine/topology/dispatch.rs` does not change, because the wait comes before anything it does.

**Scope of the table.** The manager joins a group only when its process holds the worktree lease of its checkout. Every
write command holds it for its whole run (R17: "the first effect of every write command"), so production always has a
group. A manager used with no lease, which is a unit test's fixture and nothing else, spawns as it does today.

**What this leaves alone.**
- The manager's `update-ref` keeps its cleanup lease on Unix (`src/workspace_manager.rs:3553-3570`,
  `src/rundir.rs:2193`). It orders the reclaim of a ref lock (`reclaim_own_ref_lock`, fact 1), which this change does
  not replace, and its child now also sits in `P`.
- The legacy engine's own Git children (`src/workspace.rs`, frozen) are not in a group. The legacy path never creates a
  record, and it only reads one if a topology run of the checkout died.
- The agents' groups and their reapers (`src/agent/proc.rs`, follow-up A's) are separate and unchanged.

### 1.5 The ordering proof on Unix, crash behaviour and bounds

**Ordering.**
1. Every process an engine Git write starts is in `P`. The builder gives each Git child `P` before it runs, and
   nothing Git starts leaves `P` (`run-command.c`: 0 `setpgid` or `setsid` calls at each version; §1.2).
   **Executed on Git 2.43.0** (`c1/witness/pg/witness-pg-helper.log`): `worktree add` (pid 2582711), its `reset`
   child (2582713), the smudge filter (2582715), and its background helper (the subshell 2582716 and its `sleep 1`,
   2582718) all report group 2582710, the sentinel's pid. In `witness-pg-workers.log` all four `checkout--worker`
   processes (2583322, 2583323, 2583325, 2583326) report the sentinel's group 2583315.
2. `P` is durable before any member but the sentinel exists. The record is renamed into place and its directory synced
   before the first Git child is spawned, so a kill at any earlier point leaves no member besides a sentinel that exits
   on end of file.
3. While any process is in `P`, the number `P` is not handed out again. Linux frees a pid only when no task uses it as
   pid, process group or session, and macOS skips pids in use as a group id. So until `ESRCH`, `kill(-P, 0)` speaks
   only about this group. This is reasoned from the kernels, not executed here.
4. `kill(-P, 0)` fails with `ESRCH` exactly when no process has group `P`. It reads no `/proc` entry, so `hidepid`
   cannot hide a member. `EPERM` is read as "members". **Executed** (`witness-pg-helper.log`): after the coordinator's
   `SIGKILL` the sentinel was gone within 0.2 s and the group still answered "members", holding `worktree add`,
   `reset`, the filter and its helper. Once the filter was released, the group answered `ESRCH` at 1.033 s, and the
   helper had finished first (`helper finished before the group read empty: True`).
5. Recovery begins only after the worktree-lease acquisition has read `ESRCH`, or a record from another boot (§1.4).

So no slot of the checkout is reset or reused while any process of a dead coordinator's writer group exists.

**Crash behaviour.**

| Where the coordinator dies | What is left | What the next command does |
|---|---|---|
| before the record's rename | a sentinel that exits on end of file, and perhaps a temporary record | removes a leftover temporary record; no group is named |
| after the record, before its first Git child | a record naming a group whose sentinel exits | reads `ESRCH`, removes it, proceeds |
| inside a Git write | the record and the live members | waits for `ESRCH`, up to 10 s, then refuses resumably |
| inside release, after reaping the sentinel | the record | reads `ESRCH`, removes it, proceeds |
| the machine reboots | the record | removes it: another boot |

A second death, of the command that is waiting, changes nothing: the record is still there and the next command waits
again.

**The orphaned group, an executed side effect.** When the coordinator dies, `P` becomes an orphaned process group: no
member has a parent in another group of its session. If a member is stopped at that moment, the kernel sends every
member `SIGHUP` and then `SIGCONT`. The first version of the witness (`c1/witness/pg/witness_pg_selfstop.py`) held its
filter by stopping it, and the whole group was gone 0.2 s after the coordinator's death in all three variants
(`c1/witness/pg/witness-pg-*-selfstop.log`; each script then ended with an error, because the filter it went on to
resume no longer existed). It shortens a wait and nothing depends on it. While the coordinator lives the group is not
orphaned, because the sentinel's and each Git child's parent is the coordinator, which is in another group of the same
session.

**Bounds.**
- The wait lasts at most 10 s and then refuses resumably, so a resume never hangs on a hung writer.
- A hung writer, or a member in uninterruptible sleep, holds recovery until it ends or an operator ends it. That is
  R28's position for a stuck cleanup reaper today, and the finding asks for it to be stated.
- Release costs one sentinel reap and one `kill(-P, 0)` per command.
- The group costs one extra Git process per write command.
- A Git child's spawn is unchanged apart from the `posix_spawn` attribute.

**Terminal signals.** Writers in their own group are a background group of the coordinator's session.
- `SIGINT` and `SIGTSTP` from the terminal reach the coordinator's group and no longer reach its Git children, which
  finish their write. If the coordinator then exits, the next command waits for them.
- A Git child reads `/dev/null` and writes to pipes, so a background group's terminal stops (`SIGTTIN`, `SIGTTOU`) do
  not arise. A filter that opens `/dev/tty` itself could be stopped, and if the coordinator died then, the
  orphaned-group rule above would hang it up.

### 1.6 Windows: why candidate (1) cannot be completed here, and decision D2

**What holds.** Every process the coordinator creates is in its ambient kill-on-close job:
- `join_ambient_job` runs at every write command's start (`src/main.rs:196`);
- the job is created unnamed and non-inheritable (`src/agent/proc.rs:875-880`), kill-on-close (`:940`), and its handle
  is never closed (`:1147`);
- the job does not allow breakaway, so job membership, like a process group, is inherited by everything Git starts;
- Git for Windows passes its children only its standard handles (§1.2).

**What cannot be observed.** A successor can ask whether a job has active processes only through a handle to that job.
- Every handle to the ambient job, and to any job nested in it, is held by the coordinator or by a process the
  coordinator's death terminates.
- A job object's name stops resolving when its last handle closes, however many processes it still holds.
- So after the coordinator's death, no process outside the dying set can open the job and wait for it to empty.
- Handing a job handle to Git's descendants fails the way #329's round 3 failed with the lease: Git for Windows gives a
  child only standard handles, and Git feeds a filter and `checkout--worker` through pipes.

The finding's third requirement, observing completion, therefore needs one of: a holder outside the ambient job, a
packet change, or an owner-accepted residual. This is reasoned from Microsoft's documentation of job objects,
`TerminateJobObject` and process termination; none of it was executed for this design.

**What can be observed, and is proposed as D2a.**
- The Windows record names the coordinator: its pid and its creation time.
- The next write command of the checkout waits, at the same point and with the same 10 s bound, until that process is
  gone. It uses `crate::agent::proc::process_alive(pid, creation_time)`, public already (`src/agent/proc.rs:831-833`),
  which this change calls and does not edit.
- A record naming the waiting process itself is skipped, because that coordinator is not dead.
- The documented order of process termination closes all of a process's handles before its process object is
  signaled. Closing the last handle of a kill-on-close job terminates every process in it. So once the dead
  coordinator is signaled, termination has begun for every member of its job, and none of them runs user code again.
- **The residual, R-W:** an I/O operation a member issued before its termination began may still complete
  afterwards. Microsoft documents that termination is asynchronous, and that outstanding I/O must finish or be
  cancelled first.
- It narrows the gap the lenses found: today the successor's locks can be acquired before the dead coordinator's job
  handle is even closed, because both close in the same unordered handle rundown. It does not close the gap.

**D2's options, for the owner.**
- **D2a (recommended):** the coordinator-identity wait above, with R-W filed as a residual finding the owner grades. The
  argument for P2: R-W needs an operation issued before termination began, whose target an I/O-stack component resolves
  only after the successor has replayed, verified, removed and re-added the slot. It can affect at most one operation
  per thread of each member, and no instance was executed.
- **D2b:** a holder outside the dying set, a Windows helper process outside the ambient job that keeps a handle to the
  writers' job and lets the successor wait for it to empty. It needs the ambient job to permit breakaway for that one
  helper. That is follow-up A's file (`src/agent/proc.rs`) and INV-18's design, and it adds a long-lived process.
- **D2c:** unique slot paths per incarnation (§1.3's (2)), a packet change that closes the slot paths on both platforms
  without observing any process.
- **D2d:** no Windows change: INV-18's position as `DESIGN.md` §15 states it
  (`design/15_design_event_log_resume_run_layout.md:64`, "abrupt conductor death closes its non-inheritable handle and
  lets the kernel terminate ordinary descendants"), graded by the owner.

**The same gap covers agents.** An agent killed by the same kill-on-close job is terminated as asynchronously. What the
owner decides for D2 is, in substance, INV-18's Windows completion question, and its wording should say whether it binds
agents too.

### 1.7 What escapes, and how serious each residual is

- **R-1, deliberate daemonization (Unix).** A process that calls `setsid` or `setpgid`, or `daemon(3)`, leaves `P` and
  is not waited for. **Executed** (`c1/witness/pg/witness-pg-setsid.log`): the filter's helper started under `setsid`
  reported group and session 2582762, not the writer group 2582756. The group answered `ESRCH` at 0.380 s while the
  helper ran on to 2.029 s.
  **Severity: P3.**
  - Git never leaves its group for anything an engine write starts, at any of the three versions.
  - So R-1 needs user-installed code, a filter or a merge driver or a program one of them runs, that deliberately
    detaches and then, after its Git command has ended, writes a slot path or registration it remembered.
  - `DESIGN.md` §15 already places "code that deliberately daemonises out of that group" outside the host-runner
    contract (`design/15_design_event_log_resume_run_layout.md:64`). The implementation extends that sentence to the
    engine's Git children.
  - The executed corruption class, #329's helper, stayed in its group and is closed.
  - Filed as a P3 finding with the implementation.
- **R-2, a reused group number (liveness only, Unix).** If a dead coordinator's group empties, and its number is taken
  by a new process group before any command of the checkout reads the record, the next command waits for that
  unrelated group and then refuses. The refusal says how to clear the record. It is never a proceed while a member
  lives.
  - On this box `pid_max` is 4194304 (`c1/environment.txt`), so a number comes back only after the counter wraps; a
    system with a smaller pid space wraps sooner.
  - The boot check removes every record of a previous boot.
  - **Severity: P3 (liveness).**
- **R-3, a hung writer holds recovery** until it ends or is ended. Stated, as the finding requires; not a finding.
- **R-W, Windows I/O in flight at termination,** under D2a: §1.6.

### 1.8 Item 3: no frozen module moves

- **The frozen set** is G6's "fold, queue, merge, repair, and recovery modules byte-identical to the G5 range" and
  PR11's set, listed in `reviews/2026-09-30-pr11-record.md` R-D.
- **What changes, none of it frozen.** The wait is in `src/rundir.rs`'s `WorktreeLock::acquire_in_hooked`. The frozen
  `recover.rs` calls it at `:271` and is not edited. The group is in `src/workspace_manager.rs`.
  `src/engine/topology/dispatch.rs`, where the finding is located (`:251`), is unchanged: the wait precedes it.
- **The frozen tests.**
  - When no orphan exists, the wait costs one directory read and, after a killed child coordinator, the few
    milliseconds until its sentinel has read end of file and been reaped.
  - The frozen recovery tests that kill a child coordinator already wait for the cleanup lease's release before they
    resume (`wait_for_cleanup_hold_release`, `src/engine/topology/recover/tests.rs:24429`).
  - Their kills land at effect hooks, so the faulted site's own Git command has either not started or has finished.
    A Git child another pipeline has in flight at that moment is now waited for, within the bound; today it is not.
  - The implementation must show the whole frozen set and its tests passing.

### 1.9 Effect governance, instruments and the packet (decision D1)

**The packet must name what this adds.** The record is a durable file, and the group it names is process-local OS state
that outlives the coordinator, observed and never owned or reset by the next one. Neither is in the packet's resource
accounting: R25 is the worktree lock file alone, and R28 is a reaper's or an `update-ref` child's cleanup-lease hold.
The engine's Git children have no Unix row at all today.

**D1a (recommended): erratum E-FUC-1 adds one row and three sites.**
- **R29**, "a write command's Git writer record (`<worktree git dir>/upstroke-writer-group`) and the writer group it
  names". On Unix the group is the sentinel's process group, in which every Git child of the command's manager runs; on
  Windows the record names the coordinator.
  - Domain: external_physical for the record. The group is observed, never owned or reset.
  - Granularity: per worktree git dir, one write command at a time under R17's lease.
  - Lifecycle: written and synced before the command's first Git child; removed at release when the group is observed
    empty; otherwise left, and observed by the next write command's worktree-lease acquisition, which waits up to 10 s
    for the group to empty or the coordinator to end, removes the record, and otherwise refuses resumably.
  - At run end, Complete, Parked, Halted and BudgetExceeded remove it at release, unless a member outlived the command.
    NoRunFinished leaves it for the next write command, which removes it.
- **`Lock.RecordWriterGroup`** (R29): the sentinel's start and the record's write, before the command's first Git
  child. Not read-only.
- **`Lock.ObserveWriterGroup`** (R29): the read, the wait and the removal of an empty group's record, at
  worktree-lease acquisition. Not read-only, because it removes.
- **`Lock.ReleaseWriterGroup`** (R29): the sentinel's reap, the probe and the record's removal when the command gives
  its lease back.
- Each site has one adjacency and one fault row in the packet's model, which is why release is a site of its own. The
  erratum sets them; the closest existing rows are `Lock.AcquireWorktree`'s (before `run_started`, `T-RUNSTART`) for
  the first two and `Lock.Release`'s (after `run_finished`, `T-FINALIZE`) for the third.

**D1b, smaller in the packet:** no new row or site. The record is the content of R25's lock file, written through the
lease's own descriptor and read by the next holder through its own. That needs only R25's description amended. It is an
in-place record, and its ordering argument holds: the record is consumed before it is overwritten, and is durable before
any member joins. #329's round 2 drew findings on an in-place record, though, and writes through an `fcntl`-locked
descriptor would need care. Recommended against.

**The instruments the implementation moves under D1a.** Counts are by reading at this head
(`c1/site-census-by-reading.txt`, `c1/code-citations.txt`). The implementation's census run is the oracle.
- `src/topology/effects/sites.rs`: `LockSite::ALL` goes from 6 to 9, and every site from 70 to 73.
- `src/topology/effects/vocab.rs`: `ResourceRow::ALL` goes from 15 to 16 (R29).
- `effect_sites.json`: three entries, 70 to 73.
- `effects/sequential-registry.json`: the three sites' entries, per the registry's rule.
- `effects/residue-classes.json` and `effects/residue-synthetic.json`: the record left by a kill, reclaimed at the next
  acquisition.
- `src/runner/contract.rs`, `every_production_process_start_is_classified` (`:1632`): the manager's row goes from
  `(2, 0, 0)` to `(2, 1, 0)`, for the sentinel's one `.spawn()` through the existing builder, and its text.
- `effects/wrappers.toml`, and `clippy.toml` if effectful: a row for each crate-visible function the lease side exposes
  to the manager. The design keeps that to one accessor.

These are instruments, so the implementation's merge is the owner's under `CLAUDE.md`'s first limb, as is any erratum.

### 1.10 Item 4: tests

**Witnesses, each red at base.** The kill child is a coordinator in a child process. The resume runs in the parent
through the production locks and funnels. "Paid edits" is a worker's write into the slot after the resume.

| Test | Hold | At base | Under the design |
|---|---|---|---|
| T1, the late `reset` | the add's `reset` held mid-checkout by a smudge filter that waits on a release file | the resume recreates the slot, and the late `reset` reverts the edits (`d3/…/witness-locks-exec-base.log`) or the orphaned add's `remove_junk` deletes the recreated slot (`d3/…/witness-locks-filter-base.log`) | the resume waits while the group has members. Released within the bound, it proceeds after `ESRCH` and the edits stand. Held past the bound, it refuses resumably, naming the group. |
| T2, the filter's background helper | the filter starts a helper that stays in the group and acts after Git exits | the helper's late `reset` reverts the edits (`~/orch-pr11/reviews/329-d3-witnesses/pr329-d3-reg-recreated-hjzj6eur/result.txt`) | the wait covers the helper: `ESRCH` only after it ends |
| T3, `checkout--worker` | `checkout.workers=4`, `checkout.thresholdForParallelism=1`, a large tree | no writer group exists, so the membership assertion fails | every worker observed reports the writer group (as `c1/witness/pg/witness-pg-workers.log`). A worker is held only by its own work, since filtered entries never reach a worker, so the class's wait is T1's. |
| T4, `update-ref` (2.43 only) | the add's `update-ref` child | not in the writer group | in the writer group; on Git 2.50 and later the child does not exist and the test says so |
| T5, the record's crash points | kills before the rename, after the record, and inside release; a record from another boot | — | each row of §1.5's crash table |
| T6, release | an empty group; a helper outliving the command | — | the record is removed for the empty group, and kept for the helper, which the next command waits on |
| T7, R-1 pinned | a helper started under `setsid` | — | it is outside the group and is not waited for, as the residual says |
| T8, Windows (D2a) | a kill child that recorded itself | — | the parent's acquisition waits until the child's process object is signaled, and skips a record naming itself |

**How the tests hold processes.** A release file, as #329's witnesses held theirs. Not `SIGSTOP`: a stopped member
makes the orphaned writer group hang up when its coordinator dies (§1.5), so a stop-based hold proves the kernel's rule
and not the wait.

**Mutations, each to fail a named test.**
- M1: drop `process_group(P)` from the builder → T1, T2 and T3 red.
- M2: skip the wait → T1 and T2 red.
- M3: write the record after the first Git child → T5 red.
- M4: read `EPERM` as empty → a unit test of the probe with an `EPERM` answer red.
- M5: remove the record at release whatever the probe says → T6 red.
- M6: on Windows, skip the coordinator wait → T8 red.

**Windows time.** T1 to T7 are Unix-only: process groups. T8 is the only test the Windows legs run, one kill child and
one wait, a few seconds. The brief's budget is the guest harness at about 468 s and the hosted queue leg's 45-minute
limit (`~/orch-pr11/briefs/pr11_fuc_design.md`), and a few seconds threatens neither. CI's legs are the truth for
Windows and macOS: the local baseline cannot speak for either. CI can show T8 passing on the guest and in the hosted
queue lane. It cannot show R-W's absence, which is reasoned, not executed.

### 1.11 Item 5: G6 classification

**What this closes, on Unix**, with D1:
- Q1's "reclaimed or repaired … before any slot reset, admission, or resource reuse", for the engine's Git children:
  no write command of the checkout gets past worktree-lease acquisition while a dead coordinator's writer group has a
  member.
- ST-16's and ST-18's crash-then-resume classes, for a crash inside an engine Git write.
- INV-22's accounting, for the record and group, as R29.

**What remains.**
- Windows, per D2. Until the owner decides, the finding's Windows half stays a P1 and blocks G6. Under D2a its residual
  R-W is graded by the owner; under D2c it closes.
- R-1, P3, outside the contract, filed with the implementation.
- R-2, a P3 liveness residual, and R-3, stated.

The finding stays `deferred` at this head, and its repair deletes it.

### 1.12 The owner's decisions

| | Decision | Recommendation | If declined |
|---|---|---|---|
| **D1** | the packet names the writer record and its group: erratum E-FUC-1 (R29, `Lock.RecordWriterGroup`, `Lock.ObserveWriterGroup`, `Lock.ReleaseWriterGroup`), or D1b (R25's content) | **D1a** | no Unix remedy is in the packet's accounting, and G6's INV-22 reading would not cover it |
| **D2** | Windows: D2a the coordinator-identity wait with R-W filed and graded; D2b a holder outside the ambient job (follow-up A's file, INV-18); D2c unique slot paths per incarnation (packet); D2d no change, graded | **D2a**, R-W graded P2 | the Windows half of the finding stays an open P1 and blocks G6 |

The Unix design does not depend on D2. D2c would make most of the Unix machinery unnecessary for slot paths, which is
why it is asked as a choice rather than an add-on.

### 1.13 Risks, sequencing, and what is out of scope

- **One more Git process per write command,** and Git children no longer share the coordinator's terminal signals
  (§1.5).
- **A stale record can refuse recovery** (R-2) or wait out a hung writer (R-3). Both refusals say how to proceed.
- **The legacy commands read the record** at their worktree-lease acquisition. They create none, so a v0.1 checkout sees
  a change only after a topology run of the same checkout died.
- **Follow-up A (#328)** owns `src/agent/proc.rs`. This change calls its public `process_alive` on Windows and edits
  nothing there.
- **Follow-up B (#329)** edits `src/workspace_manager.rs`'s registry access and targeted removal. This change's hunks
  are the builder (`command`, `:4994-5009`), new private functions, and the worktree git dir the manager learns at
  `derive` (`:1616`). They are disjoint, and whichever merges second rebases. The shared finding file follows the rule
  in this record's header.
- **Out of scope.**
  - A live coordinator's own lingering helper writing into the slot it serves.
  - Other checkouts of the repository: a registration another checkout's engine removes is #329's targeted-removal
    question.
  - Processes the engine did not start.
