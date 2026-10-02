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
`~/orch-pr11/reviews/329-d3-witnesses/`. Design round 2 (§2), which answers design
review round 1, is `pr11_fuc_design2`'s (the same model and effort, a fresh session on this branch at `6b28452d`); its
figures are under `~/orch-pr11/logs/pr11_fuc_design2/`, cited as `c2/…`. A fresh implementer writes the code after the
design review and the owner's decisions, and its sections follow §2.

**The evidence plan was conservative, by direction.** Read our code, read Git's source at the three versions that
matter, cite the corruption witnesses #329 already executed at base rather than rebuild them, and run one new witness
that uses only our own `git` commands in a temporary directory. That witness holds its filter with a release file, and
its only signal is a `SIGKILL` of its own coordinator child. Nothing was traced, preloaded or injected. Round 2 kept the
same plan: it read Git's source at four tags and Microsoft's, Linux's and POSIX's documentation, and its one new witness
runs only `git` commands in temporary directories, observed through Git's own trace2 event stream, with no signals
(§2, opening).

## 0. Status

| Phase | State |
|---|---|
| Design, round 2 (§2) | **PROPOSED — pending the owner's decisions D1, D2 and D3 (§2.12).** It answers design review round 1, whose three lenses returned CHANGES_REQUIRED on `6b28452d`. Every engine Git command disables automatic maintenance and lazy fetching (§2.2). On Unix, the writer group no longer has its sentinel as leader, and the sentinel leaves when its coordinator dies (§2.4.1). The record names its PID namespace and filesystem class, and an observer that cannot see the group refuses (§2.4.3). Release is bounded (§2.4.4). On Windows, the recommended closure is a writer keeper started before the ambient join, which holds a job the coordinator joins and reports, through a durable marker, when that job is empty (D2b′, §2.5.4). The accounting is two rows, R29 and R30 (§2.7). This head changes no production code. |
| Design, round 1 (§1) | Superseded in part by §2; §2.13 lists every statement it replaces. Round 1 read: **PROPOSED — pending the owner's decisions D1 and D2 (§1.12).** On Unix the remedy is the brief's candidate (1): every engine Git child runs in one process group per write command, led by a sentinel and recorded durably before any Git child joins it, and the next write command of the checkout reuses nothing until that group is established empty. On Windows no in-lane mechanism can observe a dead coordinator's job empty (§1.6), so the Windows treatment is the owner's (D2). The packet's resource and site inventories must name the new record and its observation (D1). This head changes no production code. |
| Implementation | not started. It waits on the design review of §2 and on D1, D2 and D3. It does not depend on #328 (follow-up A) or #329 (follow-up B) beyond the merge order of the shared finding file (§1.13). |

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

> **Round 2:** §2.2 replaces this subsection's account of Git's detach paths: a lazy fetch reaches automatic maintenance. §2.3 replaces its list of which children can write a slot.

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

> **Round 2:** D2c's row and "Why not (2)" are corrected in §2.5.6, and Windows in §2.5.

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

> **Round 2:** §2.4 replaces the sentinel, the record's content, the boot rule and the release, and §2.3 brings `read_only_git`'s children into the group.

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

> **Round 2:** §2.4.5, §2.4.6 and §2.4.7 replace the proof, the crash table (its release row was wrong) and the bounds.

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

> **Round 2:** §2.5 replaces this subsection. A keeper outside the ambient job makes the closure possible in lane (D2b′). R-W is re-argued at P1. `process_alive` is replaced by a probe with three outcomes.

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

> **Round 2:** §2.6 replaces this subsection: R-1's premise is re-derived and its severity restated, and R-G, R-Z, R-L, R-T and R-K are added.

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

> **Round 2:** §2.7 replaces this subsection: two rows, R29 and R30, the equations, the ledger, the sites and the counts.

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

> **Round 2:** §2.10 replaces the test table and the mutations.

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

> **Round 2:** §2.11 replaces this subsection.

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

> **Round 2:** §2.12 replaces this subsection: D1, D2 (with D2b′ recommended) and D3.

| | Decision | Recommendation | If declined |
|---|---|---|---|
| **D1** | the packet names the writer record and its group: erratum E-FUC-1 (R29, `Lock.RecordWriterGroup`, `Lock.ObserveWriterGroup`, `Lock.ReleaseWriterGroup`), or D1b (R25's content) | **D1a** | no Unix remedy is in the packet's accounting, and G6's INV-22 reading would not cover it |
| **D2** | Windows: D2a the coordinator-identity wait with R-W filed and graded; D2b a holder outside the ambient job (follow-up A's file, INV-18); D2c unique slot paths per incarnation (packet); D2d no change, graded | **D2a**, R-W graded P2 | the Windows half of the finding stays an open P1 and blocks G6 |

The Unix design does not depend on D2. D2c would make most of the Unix machinery unnecessary for slot paths, which is
why it is asked as a choice rather than an add-on.

### 1.13 Risks, sequencing, and what is out of scope

> **Round 2:** §2.8 corrects the legacy boundary, and §2.9 the placement and scope.

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

## 2. Round 2 design: design review round 1 answered

**What this section is.** Design review round 1 ran three `gpt-6-astra` lenses at `max` on `6b28452d`, and all three
returned CHANGES_REQUIRED (`~/orch-pr11/reviews/review-330-d1-{design,concurrency,regression}-6b28452d.review.md`,
hashed in `~/orch-pr11/reviews/SHA256SUMS-330-d1`). The orchestrator's triage (`~/orch-pr11/reviews/review-330-d1-triage.md`)
gives the findings the ids used below. Its addendum (`~/orch-pr11/answers/pr11_fuc_design2-0.md`) asks for a concrete
Windows **closure** option and for the residual-acceptance options to be shown separately, each with its severity, its
evidence and its G6 applicability. This section is the round-2 design. Where it disagrees with §1, it governs. §2.13
lists every §1 statement it replaces, and §1's own headings carry the same pointers.

**Who wrote it, and the evidence.** `pr11_fuc_design2` (`claude-opus-5-5`, `max`), a fresh session on this branch at
`6b28452d`. Its figures are under `~/orch-pr11/logs/pr11_fuc_design2/`, cited as `c2/…`; round 1's stay `c1/…`.
- **Git source** at v2.43.0, v2.50.1, v2.55.0 and Git for Windows v2.50.1.windows.1, from the tag tarballs
  (`c2/git-src/PROVENANCE.txt`). Only the cited files are kept. Every line number quoted below is in
  `c2/git-src/git-src-citations.txt`, which `c2/git-src/cite.sh` regenerates.
- **Documentation:** Microsoft's Win32 pages, the Linux man-pages and POSIX, saved as text by `c2/docs/fetch_docs.py`.
  The quoted lines are in `c2/docs-citations.txt`.
- **Our code:** the cited lines at `6b28452d` are in `c2/code-citations.txt`, and the instrument counts in
  `c2/census/census-6b28452d.txt`, which `c2/census/census.py` regenerates.
- **One new witness**, within the evidence plan: only our own `git` commands, in temporary directories, observed
  through Git's own `GIT_TRACE2_EVENT` stream, with no signals, tracing or preloading (`c2/witness/gc/`). It ran on this
  box's Git (`git version 2.43.0`, Ubuntu package `1:2.43.0-1ubuntu7.3`) and on a Git 2.55.0 built privately from its tag
  for the purpose (`c2/witness/gc/gits.txt`).

### 2.1 The findings, and where each is answered

| Id | Sev | What round 1 got wrong | Round 2 |
|---|---|---|---|
| FUC-D1-GC | P1 | Git's own automatic maintenance can detach from the group. §1.2 counted only direct `run_auto_maintenance` calls, and a partial clone's lazy fetch reaches one. | Every engine Git command disables automatic maintenance and lazy fetching (§2.2). The escape and its closure were executed on 2.43 and 2.55. R-1's premise is re-derived. |
| FUC-D1-PIDNS | P1 | A boot id and a group number do not name a group across PID namespaces. | The record names its PID namespace, and an observer in another namespace refuses (§2.4.3). A record from another boot is removed only where no other running kernel can share the checkout. |
| FUC-D1-WINOBS | P1 | D2a's `process_alive` (`src/agent/proc.rs:1217`) reads a failed observation as death. | A new three-outcome probe (§2.5.2). The recommended Windows option never proceeds on a probe at all (§2.5.4). |
| FUC-D1-RELEASE | P2 | Release reaped the sentinel without bound, while holding the worktree lease. | Release kills its own sentinel and is bounded at one second (§2.4.4). |
| FUC-D1-UNREAPED | P2 | An exited sentinel left unreaped by its adopter held the group for ever. | The sentinel is not the group's leader, and it leaves the group before it exits (§2.4.1). Zombies of Git members under an adopter that never reaps are residual R-Z (§2.6). |
| FUC-D1-ORACLE | P2 | T5 adopted a crash row that equated "sentinel reaped" with "group empty". | The crash table is corrected, and T5 crosses each release crash point with a surviving helper (§2.4.6, §2.10). |
| FUC-D1-ACCOUNT | P2 | Neither D1 option accounted for the group. The inventory omitted the ledger and the outcome equations. | Two rows, R29 for the record and R30 for the group, each in its own domain. The lifecycles, the equations, the ledger and every instrument are given with counts, for D1a and for D1b (§2.7). |
| FUC-D1-RW | P2 | R-W's prerequisites ignored `Reuse::Verified` (`src/engine/topology/dispatch.rs:249`) and several overlapped I/Os per thread, so its P2 was unsupported. | Re-argued (§2.5.1). R-W remains only under D2a, at the finding's own severity, P1 (§2.5.5). The recommended option closes it. |
| FUC-D1-WRITETREE | P3 | `write-tree` was excluded from the slot writers. | Corrected, and so is the claim that the manager's reads cannot start a writer (§2.3). |
| FUC-D1-LEGACYBOUND | P3 | The legacy boundary missed a normally completed topology command. | Corrected, with a mixed-schema test (§2.8, T13). |

### 2.2 Git's own processes: what every engine Git command is told (FUC-D1-GC)

**The escape, executed.** In a blob-less partial clone, the engine's own add, `git worktree add --detach <slot> HEAD`,
checks out through a `reset --hard` child. That child fetches the missing blobs through a `fetch` child, and the fetch
runs automatic maintenance (`c2/witness/gc/witness-gc-v2.43.0.log:3-12`).
- On Git 2.55.0 the maintenance child is `git maintenance run --auto --no-quiet --detach`, and it entered Git's
  `maintenance`/`detach` trace region. That region is `daemonize()`: a fork, the parent's exit, and `setsid()`
  (`c2/witness/gc/witness-gc-v2.55.0.log:3-11`; `builtin/gc.c:1814-1817` and `setup.c:2186-2214` at v2.55.0). The process
  that did it left the add's process group. Nothing in the repository needed maintenance.
- On 2.43.0 the same add ran `git maintenance run --auto --no-quiet` without `--detach`. That version's `gc --auto`
  detaches only when a threshold is met (`builtin/gc.c:673` at v2.43.0).
- `PR326-MACOS-A-DAEMONIZING-DESCENDANT-HANGS-UP-THE-ROLE` records the same 2.55.0 detach after every `git commit`
  (`findings/P2_correctness_202609281056_a-daemonizing-descendant-hangs-up-the-role-on-macos.md`).

So the design lens's sequence needs no user helper: the detached maintenance's `gc` runs `worktree prune` from outside
the group. Its finding stands as written, and round 1's premise for R-1 was false.

**The settings, on every Git child the manager spawns.** The builder `WorkspaceManager::command`
(`src/workspace_manager.rs:4994`), and therefore `git`, `git_with_identity` and `update_ref`, adds these. So does the free
function `read_only_git` (`:5452`), which round 1 left out (§2.3):

| Setting | Form | What it stops |
|---|---|---|
| `maintenance.auto=false` | `-c` | `run_auto_maintenance` returns before starting `maintenance run`. |
| `gc.auto=0` | `-c` | `gc --auto` finds nothing to do, by whatever path it is reached. |
| `gc.autoDetach=false`, `maintenance.autoDetach=false` | `-c` | If maintenance is ever reached despite the two above, it runs attached, inside the group or job. |
| `GIT_NO_LAZY_FETCH=1` | environment | Git never fetches a missing object from the promisor remote. |
| `GIT_ALLOW_PROTOCOL=` (empty) | environment | Every transport is refused before any helper, `ssh` or `upload-pack` starts. This covers the Gits that predate `GIT_NO_LAZY_FETCH`. |
| `GIT_TERMINAL_PROMPT=0` | environment | Git's own credential prompts fail instead of reading `/dev/tty`. A writer group is a background group of the coordinator's session, and a read there stops the group (R-T, §2.6). |

**How they hold, version by version** (`c2/git-src/git-src-citations.txt`):
- **They reach every Git process.** A `-c` value is put into `GIT_CONFIG_PARAMETERS` (`git.c:249` and `config.c:464` at
  2.43.0; `git.c:264`/`config.c:470` at 2.50.1 and the Windows tag; `git.c:269`/`config.c:462` at 2.55.0). Every child
  inherits it, and `prepare_other_repo_env` keeps it even across a change of repository (`run-command.c:1820`, `:1855`,
  `:1856`, `:2000`). Command-line scope outranks every configuration file. The three variables are inherited the same way.
- **`maintenance.auto=false`** returns early at `run-command.c:1803-1805` (2.43.0), `:1820-1822` (2.50.1), `:1821-1823`
  (the Windows tag) and `:1961-1967` (2.55.0). At 2.55.0 an unset `maintenance.auto` falls back to `gc.auto`, which is
  why both are set.
- **`gc.auto=0`** makes `need_to_gc` return 0 (`builtin/gc.c:386`, `:635`, `:635`, `:670`), and `gc --auto` reaches
  `daemonize()` only after that check (`:673`, `:956`, `:956`, `:989`).
- **The detach settings** are read at `run-command.c:1829-1831` (2.50.1), `:1830-1832` (the Windows tag) and `:1974-1976`
  (2.55.0), and as `gc.autodetach` at `builtin/gc.c:167` (2.43.0).
- **`GIT_NO_LAZY_FETCH`** is honoured in two places. `promisor-remote.c:32` returns before the fetch child starts, and
  `setup.c:1657` (`:1066` at 2.55.0) clears `fetch_if_missing` in every Git process.
  - Upstream it first shipped in 2.45.0 (`Documentation/RelNotes/2.45.0.adoc:120`). The May 2024 maintenance releases
    backported the `promisor-remote.c` check (`c2/git-src/v2.43.4/promisor-remote.c:26-27`, the same at v2.39.4).
    Ubuntu's `2.43.0-1ubuntu7.3` carries it as the CVE-2024-32465 patch (`c2/git-src/box-git-package.txt`).
  - Upstream 2.43.0 to 2.43.3 do not honour it: `NO_LAZY_FETCH` occurs nowhere in the v2.43.0 tree's C sources
    (`c2/git-src/tree-greps.txt`), and `c2/git-src/v2.43.3/promisor-remote.c` lacks the check.
- **`GIT_ALLOW_PROTOCOL=`**, set and empty, gives an allow-list that names no transport. The list is read at
  `transport.c:999` (2.43.0), `:1043`, `:1043` and `:1053`, and checked at `:1074`, `:1118`, `:1118` and `:1128`.
  `transport_helper_init` checks it before any helper starts (`transport-helper.c:1302`, `:1324`, `:1350`, `:1343`), and
  `git_connect` checks it for `ssh`, `git` and `file` before connecting (`connect.c:1253`, `:1471`, `:1493` at 2.43.0).
  Its documentation is `Documentation/git.txt:924` at 2.43.0 (`git.adoc:969`, `:969`, `:986`).
- **`GIT_TERMINAL_PROMPT=0`** is read at `prompt.c:62` (2.43.0) and `:64` (the rest).
- **An empty value on Windows.** It is passed in the environment block as `GIT_ALLOW_PROTOCOL=`. Should a runtime
  drop it, the Gits that honour `GIT_NO_LAZY_FETCH` (Git for Windows 2.50.1 in CI) still refuse the fetch. Whatever
  the fetch would start stays inside the coordinator's job, and with maintenance off nothing in it detaches.
- **On Windows** `daemonize()` is `ENOSYS` (`setup.c:2005-2007` at the Windows tag), and `gc` and `maintenance` "continue
  in foreground" (`builtin/gc.c:953-956`, `:1634-1637`). Maintenance therefore never detaches there: it runs inside the
  coordinator's job, `worktree prune` included. The settings stop it from running at all.

**Executed** (`c2/witness/gc/`, one fresh partial clone per variant, each add run with
`-c protocol.file.allow=always` for the witness's `file://` remote):

| Git | No settings | `maintenance`/`gc` settings | + `GIT_ALLOW_PROTOCOL=` | + `GIT_NO_LAZY_FETCH=1` |
|---|---|---|---|---|
| 2.43.0 (Ubuntu `7.3`) | fetch, upload-pack, `maintenance run --auto`; add rc 0 | fetch, upload-pack, no maintenance; rc 0 | the fetch child fails before any upload-pack; add rc 128 | no fetch child; add rc 128 |
| 2.55.0 | fetch, upload-pack, `maintenance run --auto --detach`, **detach entered**; rc 0 | fetch, upload-pack, no maintenance, no detach; rc 0 | the fetch child fails before any upload-pack; rc 128 | no fetch child; rc 128 |

The verdict lines are `witness-gc-v2.43.0.log:12,22,29,35` and `witness-gc-v2.55.0.log:11,20,26,31`. The rc 128 rows show
the user-visible effect below. The witness left no process behind (`ps` after both runs found none).

**What users see.**
- **A partial clone.** An engine Git command that needs an object the clone does not hold now fails, naming it ("could
  not fetch … from promisor remote", `witness-gc-v2.55.0.log:25`). The command's error is reported as today, and the
  run is resumable once the operator has fetched the objects, with `git fetch` or a checkout in the main worktree.
  Fetching on the engine's behalf would run network helpers, credential daemons and terminal prompts inside the writer
  group (§2.4.7). That is why the brief chose to disable lazy fetching rather than permit it.
- **Maintenance.** The engine's commands no longer trigger it. The user's own commands still do.
- **Configured programs that use a Git transport** under an engine command, such as a filter that runs `git fetch`,
  fail. `git-lfs`, the common filter, does not use Git's transports and never reads `GIT_ALLOW_PROTOCOL` (no
  occurrence in its v3.6.1 Go sources, `c2/git-src/tree-greps.txt`).
- **Credentials.** A configured program that asks Git for credentials, such as `git-lfs` running `git credential fill`,
  gets an error rather than a terminal prompt when no helper holds them.

**R-1's premise, re-derived.** Under the settings, no Git process that an engine command starts leaves its process
group or job on its own:
- **`daemonize()`** has two callers at 2.43.0 and three at the other tags (`c2/git-src/tree-greps.txt`): `gc --auto`
  past `need_to_gc`, stopped by `gc.auto=0`; from 2.50.1, `maintenance run --detach`, reached from
  `prepare_auto_maintenance`, stopped by `maintenance.auto=false`, or from an explicit invocation, which the engine
  never makes; and `git daemon --detach`, never run.
- **`setsid`** appears outside `daemonize()` only in `builtin/fsmonitor--daemon.c:1478` (2.55.0), and `setpgid` and
  `setpgrp` appear nowhere (`c2/git-src/tree-greps.txt`).
  - Only starting the built-in fsmonitor reaches that `setsid`, and the builder's `core.fsmonitor=false` prevents it.
  - A boolean `core.fsmonitor` also wins over Git for Windows' deprecated `core.useBuiltinFSMonitor`
    (`fsmonitor-settings.c:147-155` at the Windows tag).
- **`run-command.c`** calls neither `setsid` nor `setpgid` (§1.2).
- **Git for Windows** creates every child without `CREATE_BREAKAWAY_FROM_JOB` (`compat/mingw.c:2086`, `:2113`) and calls
  no job API at all: no `BREAKAWAY` or `JobObject` occurs in its C sources (`c2/git-src/tree-greps.txt`).

So a process leaves the group or job only if a program that **user configuration** has Git run detaches deliberately:
a filter, a merge driver, a signing program, a diff driver, or something one of them starts. That is R-1 (§2.6). Two
examples, neither of which writes a slot:
- `gpg-agent`, which `gpg` starts when `commit.gpgSign` is set, detaches by design and writes only under `~/.gnupg`.
- `git-lfs` 3.6.1 never detaches. Its non-test sources have no `Setsid`, `Setpgid` or breakaway
  (`c2/git-src/tree-greps.txt`).

**The legacy engine's own Git commands are outside this.** They come from `src/workspace.rs`'s own builder, which the
legacy boundary keeps unchanged. A legacy `git commit` still runs automatic maintenance, and at 2.55.0 detaches it. Its
`worktree prune --expire <gc.worktreePruneExpire>` (default `3.months.ago`, `builtin/gc.c:64` at 2.43.0, `:162` at 2.55.0)
prunes a registration only if its `gitdir` file is older than that (`worktree.c:774` at 2.43.0, `:1004` at 2.55.0). So
it can race a topology recovery's remove-and-recreate only under a user's short `gc.worktreePruneExpire`. That is R-G
(§2.6).

### 2.3 Which engine Git children can start a slot writer (FUC-D1-WRITETREE, and the reads)

- **`write-tree` writes the slot's index.** `write_index_as_tree` rewrites it through `write_locked_index`
  (`cache-tree.c:739` at 2.43.0, `:749`, `:749`, `:774`). The repository measured it: an index's bytes went from 104 to
  165, with the `TREE` extension added (`src/workspace_manager.rs:2799-2806`). §1.2's "Every one of them except
  `write-tree`, `commit-tree` and the reads" becomes **every one of them except `commit-tree`**. `write-tree` runs
  through the builder, so §1's group already covered it: only the census was wrong.
- **The manager's reads can start a writer too.** `read_only_git` runs `git status --porcelain=v1 -z --no-renames
  --untracked-files=all` (`src/workspace_manager.rs:5749-5765`). Executed, as `read_only_git` runs it, on a stat-dirty
  but content-clean file, it started the configured clean filter once, at 2.43.0 and at 2.55.0
  (`c2/witness/gc/witness-status-filter-v2.{43,55}.0.log:4`). A filter can start anything.
- **So every Git child the manager spawns joins the writer group**: the builder's (`git`, `git_with_identity`,
  `update_ref`) and `read_only_git`'s. That excludes only the `rev-parse` calls the manager makes while deriving the
  checkout, before it holds the lease (`common_git_dir`, `src/workspace_manager.rs:6330`), and they start no filter.

### 2.4 Unix, revised: the group, its record, the wait and the release

#### 2.4.1 The group: no leader, and a sentinel that leaves it (FUC-D1-UNREAPED)

**When.** At the first Git child the manager spawns for a write command while its process holds that checkout's
worktree lease. §1.4's process-local table is kept: the lease records the checkout, and the manager looks the checkout up.

**How**, in this order:
1. **The leader.** Fork a leader `X`. `X` calls `setpgid(0, 0)`, then waits, polling `getppid()` as `S` does below, and
   `_exit`s if the coordinator is gone first. The coordinator also calls `setpgid(X, X)`, so the group exists once its
   own call has returned, whichever of the two ran first. `P` is `X`'s pid.
2. **The sentinel.** Fork a sentinel `S`. `S` does four things:
   - calls `setpgid(0, P)`;
   - ignores `SIGHUP`, `SIGTTIN`, `SIGTTOU` and `SIGTSTP`;
   - closes every descriptor it inherited;
   - polls `getppid()` every 50 ms, one `nanosleep` per pause. When the answer is no longer the coordinator's pid, `S`
     calls `setsid()` and `_exit(0)`.

   The coordinator also calls `setpgid(S, P)`.
3. **The leader goes.** The coordinator kills and reaps `X`. `P` now has one member, `S`, and `S` is not its leader.
4. **The record is published** (§2.4.2).
5. **Every Git child the manager spawns** gets `process_group(P)` (§2.3). That is `std`'s `posix_spawn` attribute, with no
   `pre_exec`, as §1.4 argued.

**A failure fails closed.** If any step fails (a fork, a `setpgid`, the record's publication, or a later child's
join because `P` has gone), the Git command that needed the group fails with that error, and the command ends
resumably. No Git child ever runs outside the group.

**Why each choice.**
- **No leader.** `setsid()` fails for a process-group leader (`c2/docs/man2-setsid.txt:54-56`,
  `c2/docs/posix-setsid.txt:45`). So only a member that does not lead `P` can leave it.
- **The sentinel leaves before it exits.** `kill(-P, 0)` succeeds while a zombie is in `P` (`c2/docs/man2-kill.txt:80-83`).
  The regression lens executed exactly that: an adopter that never reaped the exited sentinel kept 1,000 probes positive
  for 10.051 s, and `SIGKILL` did not clear the group; only reaping did (finding 2 of
  `review-330-d1-regression-6b28452d.review.md`). `S` now calls `setsid()` before it exits. Its zombie, if its adopter
  never reaps it, sits in a session of its own and holds nothing of `P`.
- **`getppid()`, not a pipe.** Once the creator has terminated, `getppid()` names the adopter: `init(1)` or a subreaper
  (`c2/docs/man2-getppid.txt:36-41`). `S` holds no pipe from the coordinator, so a fork that kept a copy of one cannot
  delay it. That was the regression lens's embedding case, executed with blocking past 10 s (finding 1).
- **Job-control signals ignored.** `S` never stops. A terminal stop of `P`, `SIGTTIN` after a member reads `/dev/tty`,
  was the concurrency lens's executed case (finding 3), and it does not stop `S`. Nor does the hang-up the kernel sends a
  newly orphaned group that holds a stopped member (`c2/docs/posix-orphaned-group-exit.txt:116-117`) end `S` before it
  has left.
- **Only async-signal-safe calls after `fork`:** `setpgid`, `sigaction`, `close`, `nanosleep`, `getppid`, `setsid` and
  `_exit`. The agent reaper's forks in `src/agent/proc.rs` keep the same discipline.
  - `X` and `S` never `exec`. They share the coordinator's memory copy-on-write, and `S` keeps none of its descriptors
    once it has closed them.
  - The interval before that close is the window `src/rundir.rs:2174-2187` already documents for every fork.
- **Cost.** Two forks at a write command's first Git child, and one sleeping process for the command's life. §1.4's
  `git hash-object --stdin` sentinel is gone.

#### 2.4.2 The record names its observation domain (FUC-D1-PIDNS)

`<worktree git dir>/upstroke-writer-group`, a short text record. Its format version is `2`; §1's was never shipped.
- **Platform:** `linux`, `macos` or `windows`.
- **Boot.**
  - Linux: `/proc/sys/kernel/random/boot_id`, generated once per boot (`c2/docs/man4-random.txt:193-197`).
  - macOS: `kern.bootsessionuuid`.
- **PID namespace** (Linux only): the `st_dev` and `st_ino` of `/proc/self/ns/pid`. Two processes share a namespace
  exactly when these are equal (`c2/docs/man7-namespaces.txt:134-138`). If they cannot be read, the record says
  `unknown`. That needs `/proc`, which a Linux coordinator already needs: agent launch refuses without it
  (`src/agent/proc.rs:4198-4225`).
- **Filesystem class of the Git directory.**
  - `local` on Linux when `statfs`'s `f_type` is ext2/3/4, xfs, btrfs, f2fs, tmpfs or overlayfs, the magics
    `c2/docs/man2-statfs.txt:73,89,91,92,117,136,145` list. On macOS, when `f_fstypename` is `apfs` or `hfs`.
  - `shared` otherwise: NFS, SMB, 9p, FUSE (virtiofs included), cluster filesystems, or any type not listed.
- **`P`.**

It is published as §1.4 published it: a temporary file, synced, renamed into place, its directory synced. It is written
once and never updated. The design lens checked that ordering and found it holds for contained writers.

#### 2.4.3 The wait at worktree-lease acquisition

The check runs at the same point as §1.4's, directly after the cleanup-hold check in `WorktreeLock::acquire_in_hooked`
(`src/rundir.rs:1962`):

| What the observer finds | What it does |
|---|---|
| No record, or only a leftover temporary record | Removes the temporary file and proceeds. A temporary file names no published group, and no Git child joins before the rename. |
| A record it cannot read or parse | Refuses resumably, naming the file. |
| The same platform and boot, and on Linux the same PID namespace | Probes `kill(-P, 0)`. `ESRCH`: removes the record and proceeds. Success or `EPERM`: polls every 10 ms for up to 10 s, then removes and proceeds on `ESRCH` or refuses resumably. Any other answer: refuses. |
| The same boot, but another PID namespace, or a namespace either side cannot name | Refuses resumably. The refusal names both namespaces and says to resume from the recorded one, or to remove the record once that namespace's processes are known to be gone. |
| Another boot or another platform, and **both** sides saw a `local` filesystem | Removes the record and proceeds. |
| Another boot or another platform, otherwise | Refuses resumably. The refusal names the record's boot and says to remove it once that machine's or virtual machine's coordinator is known to be gone. |

**Why the namespace rule.** `kill(2)` interprets a pid in the caller's PID namespace. A process can signal, and so probe,
only processes in its own namespace and the namespaces below it (`c2/docs/man7-pid_namespaces.txt:96-103`). The
concurrency lens executed the failure: a successor in another namespace, on the same boot, read `ESRCH` while the
recorded helper was alive in its group (finding 1).

Equal identities are safe even when they arise by reuse. A namespace lives while any process, open descriptor or bind
mount refers to it (`c2/docs/man7-namespaces.txt:128-130`). So its inode number can be reused within a boot only after
every process of the recorded namespace has gone, and an observer whose namespace now carries that number can err only
towards waiting on an unrelated group.

**Why the boot rule changed.** A different boot id means a different kernel boot. That is either a previous boot of this
machine, whose processes are all gone, or another kernel running now that shares the checkout through a network or
virtual-machine filesystem. Round 1 removed every such record, which is wrong in the second case.
- A filesystem of a listed `local` type is mounted read-write by one running kernel at a time. If the writer and the
  observer each saw the Git directory on one, they cannot be two kernels running at once, so the record's boot has
  ended.
- The rule rests on the premise the worktree lease already has: two coordinators of one checkout exclude each other
  only where their kernels share the lock file's locks. That is the same kernel, or a network filesystem that
  propagates locks. On such a filesystem the record is never removed automatically.

**Every refusal says what it waits for and how to proceed** (the finding's fourth requirement). It names the record,
the group, the namespace or boot, and the step that clears it.

#### 2.4.4 Release, bounded (FUC-D1-RELEASE)

When the command gives its worktree lease back (`WorktreeLock`'s release path, `src/rundir.rs:1872`), and before the OS
lock is closed, it does three things:
1. **Ends its own sentinel.** It sends `kill(S, SIGKILL)` and polls `waitpid(S, WNOHANG)` for at most one second.
   - `S` is this process's unreaped child, so its pid names `S` until it is reaped.
   - `SIGKILL` ends it whether it is running, sleeping or stopped. That covers the concurrency lens's stopped group.
2. **Probes the group.** On `ESRCH` it unlinks the record and syncs the directory. On success or `EPERM` it leaves the
   record, because members outlived the command. On any other answer it leaves it too.
3. **Closes the lock.**

Nothing in the release waits for another process to make progress. The one-second bound is never reached by an `S` that
only sleeps in `nanosleep`. If it ever were, `S`'s zombie would stay in `P`, the record would stay, and the next command
would wait until this process exits and `S`'s adopter reaps it. Neither executed case remains:
- the stopped group: `SIGKILL` ends a stopped `S`;
- the retained pipe: `S` has none.

**A side effect, already executed in round 1.** If `S` was the last member with a parent in the session outside `P`, its
death orphans `P`. If a member of `P` is stopped, the kernel then sends every member `SIGHUP` and `SIGCONT`
(`c2/docs/posix-orphaned-group-exit.txt:116-117`; `c1/witness/pg/witness-pg-*-selfstop.log`). A stopped straggler is
hung up, rather than holding recovery for ever.

#### 2.4.5 The ordering proof, revised

1. **Every process an engine Git write starts is in `P`.**
   - The manager gives each of its Git children `P` (§2.3).
   - Nothing Git starts leaves `P` under the settings (§2.2). Membership was executed in round 1
     (`c1/witness/pg/witness-pg-helper.log`, `witness-pg-workers.log`).
2. **`P` is recorded before any member but `S` exists.** The record is published at step 4, and Git children are
   spawned only after it.
3. **`P` is not handed out again while any process has it as its group.**
   - This is round 1's reading of the kernels.
   - The design lens checked it against Linux's `kernel/pid.c:318-335` at v6.8 and Darwin's `kern_fork.c:1145-1174`.
     The concurrency lens checked it against the same two sources.
4. **`kill(-P, 0)` answers `ESRCH` exactly when no process in the caller's PID namespace has group `P`.** A zombie
   counts (`c2/docs/man2-kill.txt:80-83`). The probe runs only from the recorded namespace.
5. **`S` leaves `P` only in two ways.** It calls `setsid` within 50 ms of its coordinator's death, or the coordinator
   kills and reaps it at release. `X` is reaped before the record exists.
6. **Recovery begins only after the acquisition has done one of two things:** read `ESRCH` from the recorded boot and
   namespace, or established that the record's boot has ended.

So no slot of the checkout is reset, admitted or reused while a process of a dead coordinator's writer group exists.
An observer that cannot see that group does not proceed.

#### 2.4.6 The crash table, corrected (FUC-D1-ORACLE)

| Where the coordinator dies | What is left | What the next command does |
|---|---|---|
| Before the record's rename | `X` or `S`; perhaps a temporary record. `S` leaves within 50 ms, and `X` ends when its parent does. | Removes the temporary record. No group is named. |
| After the record, before the first Git child | The record, and `P = {S}`, which is leaving. | Waits about 50 ms, reads `ESRCH`, removes the record and proceeds. |
| Inside a Git write, or while a member it started lives | The record, and `P` holding the members (and `S`, leaving). | Waits for `ESRCH` for up to 10 s, then refuses resumably. |
| Inside release, after `S` is killed and reaped, before the probe | The record, and `P` holding whatever outlived the command. | The same: waits, then refuses. It never proceeds on the sentinel's reaping alone. This is the row §1.5 had wrong. |
| Inside release, after the probe read `ESRCH`, before the unlink | The record, and an empty `P`. | Reads `ESRCH`, removes the record and proceeds. |
| Inside release, after the probe read members | The record, and the members. | Waits for `ESRCH` for up to 10 s, then refuses resumably. |
| The machine restarts | The record, now from another boot. | Removes it if both sides saw a `local` filesystem; otherwise refuses resumably. |
| Any of the above, with the successor in another PID namespace | The record. | Refuses resumably. |

A second death, of the command that was waiting, changes nothing.

T5 kills at every row and, in each release row, with a helper outliving the command as well (§2.10).

#### 2.4.7 Bounds, and what still costs liveness

- **Acquisition** waits at most 10 s, then refuses resumably.
- **Release** takes at most about 1 s.
- **Cost:**
  - two forks at a command's first Git child;
  - one sleeping process per write command;
  - one `kill(-P, 0)` per release;
  - each Git child's spawn is unchanged but for `posix_spawn`'s group attribute.
- **Liveness residuals** R-2, R-3, R-Z, R-L and R-T are in §2.6.
- **The frozen recovery tests.** A killed child coordinator's `S` leaves within 50 ms.
  - So an in-process resume waits about that long, plus any Git child another pipeline had in flight (§1.8's point,
    kept).
  - A successor in the same process as a coordinator that never released its lease would wait on that coordinator's
    live `S`. Release runs from `WorktreeLock`'s drop, so only a leaked lease could cause that. None exists at this
    head, and the implementation's census confirms it.

### 2.5 Windows: a closure option, and the alternatives (decision D2)

#### 2.5.1 What has to be observed, and why the coordinator's process object is not it (FUC-D1-RW)

**What the dead coordinator's process object tells a successor.** When the coordinator dies, its handles close before
its process object is signaled (`c2/docs/ms-terminating-a-process.txt:44-54`). Its handle to the ambient job is that
job's last, so the kernel terminates every process of the job and of the jobs nested in it
(`c2/docs/ms-job-objects.txt:122`, `ms-jobobject-basic-limit-information.txt:142`). That termination is asynchronous:
- `TerminateProcess` "initiates termination and returns immediately";
- "the terminated process cannot exit until all pending I/O has been completed or canceled"
  (`c2/docs/ms-terminateprocess.txt:73-75`).

So a signaled coordinator says only that termination of its job's processes has begun.

**R-W, with its prerequisites corrected.** A write of the dead coordinator's job lands in a slot after the successor
has moved on, when three things hold:
1. **I/O pending at termination.** A process of the dead coordinator's job has operations pending on a slot's files
   when termination reaches it. That can be any number of operations, on any of its threads: overlapped I/O lets one
   thread have several outstanding at once (`c2/docs/ms-synchronous-and-asynchronous-io.txt:44`, `:92`). §1.6's "one operation
   per thread" was wrong.
2. **Completion after the successor's wait.** It needs no late path resolution.
   - When recovery verifies the slot and keeps it (`Reuse::Verified`, `src/engine/topology/dispatch.rs:249`), a write
     through a handle opened before termination lands in the very file the next worker edits.
   - When recovery removes and recreates the slot, an operation that resolves its path late lands in the recreated file,
     which was §1.6's only case.

   §1.6's "only after … removed and re-added" was wrong.
3. **Landing after verification**, or after the next worker's edits.

Nothing bounds the window: "cannot exit until" its I/O completes or is cancelled carries no time limit, and cancellation
is the I/O stack's to perform. **R-W's severity is therefore the finding's own.** Its third requirement reads: "On Windows
it must observe completion, not just initiate termination: that the old job is empty". An option that leaves R-W open
leaves the finding's Windows half open:
- **P1**;
- applicable to Q1 (a slot reused while a dead coordinator's write can still land), to ST-16, ST-18 and INV-22;
- **blocking G6** under its pass rule, "no open critical/high finding".

§1.6's P2 rested on the two premises corrected above, and is withdrawn.

#### 2.5.2 A probe with three outcomes (FUC-D1-WINOBS)

**What is wrong with `process_alive`.** It (`src/agent/proc.rs:1217-1228`) answers `false` in four cases:
- when `OpenProcess` fails (`OpenHandle::open`, `:1171-1182`);
- when the creation time cannot be read;
- when the creation time differs;
- for every wait result but `WAIT_TIMEOUT`.

`OpenProcess` checks the requested access against the process's security descriptor
(`c2/docs/ms-openprocess.txt:56`), so an access denial reads as a death.

**The new helper.** `probe_process(pid, creation_time)` returns `Alive`, `Gone` or `Unobserved`. It is a new private
function in this change's own code. `src/agent/proc.rs`, follow-up A's file, is not edited.
- **`OpenProcess` with `SYNCHRONIZE | PROCESS_QUERY_LIMITED_INFORMATION` fails.**
  - `ERROR_INVALID_PARAMETER` is `Gone`: no process has the identifier. Microsoft documents that code only for the Idle
    process's identifier, 0 (`ms-openprocess.txt:69`), which no record names.
  - Any other error is `Unobserved`.
- **`GetProcessTimes` fails:** `Unobserved`.
- **The creation time differs:** `Gone`. An identifier is valid only until its process terminates
  (`c2/docs/ms-process-handles-and-identifiers.txt:44`), so a different process holding it means the recorded one has
  terminated.
- **`WaitForSingleObject(handle, 0)`:** `WAIT_OBJECT_0` is `Gone`, because the process has terminated. `WAIT_TIMEOUT` is
  `Alive`. Anything else is `Unobserved`.

`Unobserved` always refuses.

**Which options depend on it.**
- D2a proceeds on `Gone`, including that one reading of `ERROR_INVALID_PARAMETER`.
- The recommended option, §2.5.4, proceeds on no probe result at all: it uses the probe only to decide whether to keep
  waiting.

#### 2.5.3 The candidates (the addendum's item 1)

- **(a) A job per write command that the successor opens by name.** Only a named job can be opened, and a job is
  destroyed once its last handle has closed and its processes have ended (`c2/docs/ms-job-objects.txt:120-122`). Nothing
  documents a name staying openable after the last handle closes.
  - The coordinator's handles die with it.
  - Git for Windows hands a child only its standard handles (`PROC_THREAD_ATTRIBUTE_HANDLE_LIST`, `compat/mingw.c:2209`
    at the Windows tag; §1.2). So a filter, a `checkout--worker` or anything they start never holds a handle to the job.

  A name alone therefore does not keep the job reachable. Something outside the dying set must hold a handle, which is
  (b).
- **(b) A keeper that holds the writers' job.** **Recommended**, in the form §2.5.4 gives. It is started before the
  ambient join, holds an unnamed job that the coordinator joins, and reports completion through a durable marker. §1.6's
  D2b needed `JOB_OBJECT_LIMIT_BREAKAWAY_OK` on the ambient job, which would let any descendant that asks break away
  (`c2/docs/ms-nested-jobs.txt:74`). This form needs no breakaway.
- **(c) Per-spawn assignment instead of a join.** Each Git child could be created suspended, assigned to the job and
  resumed, as the host runner does for agents (`src/agent/proc.rs:1028`). That would keep the coordinator itself out of
  the job.
  - At this MSRV, `std` exposes neither a child's primary-thread handle nor attribute lists: both are nightly-only in
    1.85.0 (`c2/docs/rust-1.85.0-windows-process-ext.txt`).
  - So every resume would need a system-wide thread snapshot, as `proc.rs`'s `resume_only_thread` takes.

  The join covers the same processes with one call and no per-spawn cost.
- **(d) Unique slot paths (D2c).** A packet change, set out in §2.5.6. It closes slot paths without observing any
  process, and leaves refs and registry prunes.
- **(e) Anything sounder.** None found. No Windows mechanism lets a process that was not running at the coordinator's
  death observe its job without a holder. A service or scheduled task as the holder is a heavier form of (b), with more
  privilege.

#### 2.5.4 The recommended closure, D2b′: a writer keeper and a joined job

**The keeper.** The `upstroke` binary in a hidden mode.
- **Started before the ambient join.** Every write command starts it before it joins the ambient job (`main.rs`'s
  `containment::establish`, `src/main.rs:185-200`), so it is outside that job.
- **Isolated.** It is detached from the console, takes a pipe from the coordinator as its standard input, and inherits
  nothing else. It starts no process and runs no repository code.
- **Library code.** The library supplies both halves. In-process tests start no keeper, and behave as today. The
  Windows tests run it in child coordinators (§2.10).

**The writers' job `N`.**
- Unnamed and non-inheritable (`c2/docs/ms-createjobobjectw.txt:56`, `:62`).
- No limits, and no kill-on-close: the ambient job's kill-on-close already ends `N`'s members, since `N` is nested in it
  (`ms-job-objects.txt:122`, `ms-nested-jobs.txt:88`).

**Opening**, at the manager's first Git child of a write command while the lease is held:
1. Create `N`.
2. Duplicate `N`'s handle into the keeper with `JOB_OBJECT_QUERY` access. Send it the handle, the record's path and a
   random 128-bit id, and wait for its acknowledgement.
3. Assign the coordinator itself to `N`. `N` is empty, so it nests under the coordinator's immediate job, the ambient job
   (`c2/docs/ms-assignprocesstojobobject.txt:65`). The ambient join already relies on nesting
   (`src/agent/proc.rs:1116-1119`).
4. Publish the record. It holds the platform, `GetTickCount64` at writing, the filesystem class and the id, and the pid
   and creation time of both the keeper and the coordinator. The filesystem class comes from `GetDriveTypeW` of the Git
   directory's volume: `DRIVE_FIXED` is `local`, and anything else is `shared` (`c2/docs/ms-getdrivetypew.txt:89-95`).
5. Spawn Git children as today.

From step 3, every process the coordinator creates is associated with `N` and every job above it
(`ms-nested-jobs.txt:74`), and so is everything those processes create. Git for Windows creates its children without
`CREATE_BREAKAWAY_FROM_JOB` (`compat/mingw.c:2086`, `:2113`), and neither job allows breakaway.

**The keeper's loop.**
- It waits on the coordinator's process handle, opened at its start, or for a `drop <id>` line, on which it closes `N`.
- Once the coordinator is signaled, it polls `N`'s `ActiveProcesses` every 10 ms, with no bound, until it reads 0. It
  uses `QueryInformationJobObject` with `JobObjectBasicAccountingInformation`, the reading the host runner uses to
  "observe that job empty" for an agent's private job (`src/agent/proc.rs:962-975`).
- It does not use the completion port's `JOB_OBJECT_MSG_ACTIVE_PROCESS_ZERO`: delivery "is not guaranteed"
  (`c2/docs/ms-jobobject-associate-completion-port.txt:69`).
- At 0 it publishes `<git dir>/upstroke-writer-drained-<id>` (a temporary file, flushed and renamed) and exits.

**Release**, at the coordinator's normal end and before the lock is closed:
- `N`'s `ActiveProcesses` is 1 (the coordinator alone): unlink the record and send `drop <id>`.
- More than 1: keep the record. When the coordinator exits, the ambient job's kill-on-close ends the leftovers, and the
  keeper drains `N` and writes the marker.

**The successor**, at worktree-lease acquisition:

| What it finds | What it does |
|---|---|
| No record, or only a leftover temporary record | Removes the temporary file and proceeds. |
| A record it cannot read | Refuses resumably. |
| The drained marker for the record's id | Removes the record and the marker, and proceeds. |
| No marker, but a later boot: its own `GetTickCount64` (`c2/docs/ms-gettickcount64.txt:42`) is smaller than the recorded one, and both sides saw a `DRIVE_FIXED` volume | Removes the record and proceeds. |
| No marker, and the keeper probes `Alive` | Waits for the marker, polling every 10 ms for up to 10 s. Proceeds when it appears; otherwise refuses resumably, naming the keeper and the job. |
| No marker, and the keeper probes `Gone` or `Unobserved` | Refuses resumably: the keeper ended before it observed the job empty. The refusal says to remove the record once the machine has restarted or no process of that coordinator remains. |
| A marker that matches no record | Removes it. That is a release that unlinked its record and died before sending `drop`. |

**The ordering proof.**
1. **Every process the coordinator creates after step 3 is in `N`**, with everything it creates in turn.
2. **The record is published after the keeper holds `N` and the coordinator has joined it**, and before the first Git
   child.
3. **0 means every writer has finished, its I/O included.** `N`'s `ActiveProcesses` counts every process associated with
   it, those of its child jobs included (`ms-nested-jobs.txt:84`). It falls only "when the terminated process exits and
   all references to the process are released" (`c2/docs/ms-jobobject-basic-accounting-information.txt:89`). A
   terminated process "cannot exit until all pending I/O has been completed or canceled"
   (`ms-terminateprocess.txt:73`). The coordinator is itself in `N`. So 0 means the coordinator has exited, and so has
   every process it started after joining, each with its I/O completed or cancelled.
4. **The coordinator's death does not end the keeper.** The keeper was started before the ambient join and is outside
   both jobs. It holds `N`'s handle, so `N` stays queryable.
5. **The successor proceeds only on the marker**, written after 0, or on a later boot of a machine whose volume no other
   running kernel holds.

So no slot is reset or reused while any process of the dead coordinator's writers' job can still act on it, I/O in
flight included. **R-W is closed.**

**Crash behaviour.**

| Where | What is left | What the next command does |
|---|---|---|
| Before the keeper holds `N` | No record. | Proceeds. |
| After the keeper holds `N`, before the record | No record; the keeper holds an `N` with no member. | Proceeds. The keeper drains at once when the coordinator ends, and its marker names no record, so the next acquisition removes it. |
| After the record | The record, and `N` holding the coordinator's processes. | The coordinator's death ends them. The keeper marks when `N` is empty, and the successor waits up to 10 s for the marker. |
| A member that cannot finish | As above, but `N` never empties. | Refuses resumably each time, naming the keeper. That is R-3's position. |
| The keeper killed while the coordinator lives | `N`, held only by the coordinator. | A release that reads 1 unlinks the record. Otherwise the record stays, and the next command refuses: the keeper is gone and there is no marker. |
| The keeper and the coordinator killed together, as by ending every `upstroke` process | The record. | Refuses resumably. The operator removes the record once the processes are gone: R-K, liveness. |
| The machine restarts | The record. | The tick rule removes it while the new boot is younger than the old one was when the record was written. Otherwise it refuses resumably (R-K). |
| Inside release, after the unlink | No record. | Proceeds. |

**Bounds.**
- The successor waits at most 10 s.
- Release is one query, one unlink and one line.
- The keeper runs while `N` has members, with no bound, as the Unix group does.
- A process outside `N` that holds a handle to one of its members, such as a monitoring tool, keeps the count up until
  it lets go, because the count falls only once "all references to the process are released". The successor refuses
  meanwhile: liveness, R-K.
- No Git spawn costs anything extra.

**What the addendum asked to cover.**
- **Access rights.** Two handles to `N` exist: the coordinator's, with full access as creator, and the keeper's, a
  duplicate with `JOB_OBJECT_QUERY`. Nobody opens `N`. The successor opens only the keeper's process, with `SYNCHRONIZE`
  and `PROCESS_QUERY_LIMITED_INFORMATION`. Where that is denied, for another user or against a stricter descriptor, the
  probe is `Unobserved` and the successor refuses. It never proceeds on it.
- **A job outliving its creator.** `N` outlives the coordinator because the keeper holds a handle. A job is destroyed only
  "when its last handle has been closed and all associated processes have been terminated" (`ms-job-objects.txt:122`).
- **Naming collisions.** There are none: `N` has no name, and the marker's name carries the 128-bit id.
- **Nesting with A's and INV-18's ambient job.**
  - `N` becomes a child of the ambient job, which may itself be the child of an outer job: a CI runner's, or an OpenSSH
    session's (`src/agent/proc.rs:1116-1119`).
  - An agent's private job nests below `N`. Kill-on-close of the ambient job terminates processes "associated with the
    job and its child jobs" (`ms-job-objects.txt:122`).
  - No code at this head queries the coordinator's immediate job through a null handle: every job call names its job's
    handle (`src/agent/proc.rs:856-1162`). The implementation keeps that true.
  - INV-18 says "on Windows every host child is a member of the coordinator's ambient kill-on-close Job Object from
    creation". The keeper is the one exception, so erratum E-FUC-2 (§2.7.6) amends INV-18.

**What it covers beyond the finding.** Every process the coordinator starts after its first manager Git child is in
`N`, agents and gates included.
- Their termination is observed complete too. §1.6's "the same gap covers agents" closes for them.
- So are the engine's `update-ref` children. The ref-lock reclaim that `design/26_design_merge_queue_protocol.md:398`
  grounds on "on Windows the ambient kill-on-close job ends the children with the coordinator" now follows their
  completion, not only its start.

**What it leaves.**
- **R-1W:** a program that has another process start a process outside the job (a service, the task scheduler, WMI).
  That is deliberate, as R-1 is.
- **R-K:** the keeper ended before writing the marker. Liveness only: the operator removes the record.

**Cost.** One keeper process per write command on Windows, spawned once at startup; the join; and the keeper's polling,
which starts only after the coordinator has ended.

#### 2.5.5 The residual-acceptance alternatives (the addendum's item 2)

| Option | What it leaves | Severity and evidence | G6 |
|---|---|---|---|
| **D2a**: wait for the coordinator's process object, through the three-outcome probe | R-W (§2.5.1). Its progress after the coordinator's process object is deleted also rests on the one reading of `ERROR_INVALID_PARAMETER` (§2.5.2). | **P1**: the finding's third requirement, reasoned from Microsoft's documentation and not executed | Applies to Q1, ST-16, ST-18 and INV-22. **Blocks G6** unless the owner reclassifies R-W. |
| **D2d**: no Windows change | The finding's Windows half, exactly as filed | **P1**, as filed | **Blocks G6.** |

R-1's exclusion, the Unix residual option, is decision D3 (§2.6, §2.12).

#### 2.5.6 D2c, unique slot paths: the erratum text, and what it leaves

**Erratum E-FUC-3**, only if the owner chooses D2c. It uses the errata file's form: anchor, current text, amendment.
1. **`decisions.workspace_candidates.manager`.**
   - Current: "(tasks/k<key>-g<gen>, merge/s<seq>)".
   - Amended: "(tasks/k<key>-g<gen>-e<epoch>, merge/s<seq>-e<epoch>; snapshot worktrees likewise), where <epoch> is the
     incarnation of the coordinator that created the worktree, counted from the run's durable starts; no worktree of an
     earlier epoch is reused: a fresh-process recovery recreates each open generation's worktree under its own epoch and
     reclaims the earlier epochs' worktrees as residue".
2. **`decisions.resource_accounting.rows[R9].lifecycle.OpenNoAttempt`.**
   - Current: "resumably_open during a live run (reused only after Worktree.Verify; otherwise recreated with force)".
   - Amended: "resumably_open during a live run (reused only within the epoch that created it and only after
     Worktree.Verify; otherwise, and always at a fresh-process recovery, recreated under the current epoch, the earlier
     epoch's worktree reclaimed as residue)".
   - R9's `RetainedIdle`, R10 and R24 change likewise.
3. **`transaction_fault_matrix` T-DISPATCH, `resume_action`:** recreate under the current epoch.

**What it closes.** On both platforms, a dead coordinator's writes into a slot path or registration that a successor
reuses, R-1's and R-W's included: nothing is reused across epochs.

**What it leaves.**
- **The engine's refs and their lock reclaim.** `design/26_design_merge_queue_protocol.md:398`'s Windows clause is
  unchanged.
- **Registry operations.** An orphaned `worktree prune` can still race the successor's add, the class of
  `PR11-HOST-AGENT-PRUNE-RACES-AN-ENGINE-ADD`.
- **The reclaim of an earlier epoch's worktree** can race its own orphan. On Windows an open handle blocks deletion, so the
  reclaim must tolerate it and retry.
- **Unix.** §1.12's "most of the Unix machinery unnecessary for slot paths" holds only for slot paths. The group still
  orders the manager's own `worktree prune`, and every write outside the slot paths.
- **Frozen modules.** `TaskDispatched.worktree_path` records the dispatch's own epoch. If a frozen module reads it to
  locate the live worktree, D2c also needs a change there and the owner's amendment of G6's byte-identical rule. The
  implementation would have to establish which.

**Not recommended.** It is a packet change with an open frozen-module question, and on Windows it closes less than D2b′.

### 2.6 The residuals, carried explicitly (the addendum's item 3)

Each residual is listed with its severity, the evidence for it, the G6 question it bears on, and whether it blocks G6.
G6 blocks on an open critical or high finding, under its pass rule.

#### Residuals that let a write escape

**R-1. Unix: a configured program detaches deliberately.**
- **What escapes.** A program that user configuration has Git run (a filter, a merge driver, a signing program, a diff
  driver, or something one of them starts) calls `setsid`, `setpgid` or `daemon(3)`, and after its Git command has ended
  writes a slot path or registration it kept.
- **Evidence.** The escape was executed in round 1: the helper's group and session were 2582762, not 2582756; the group
  read `ESRCH` at 0.380 s while the helper ran to 2.029 s (`c1/witness/pg/witness-pg-setsid.log:9,15,17`). Under the settings,
  Git itself never detaches (§2.2). `git-lfs` never does either (`c2/git-src/tree-greps.txt`).
- **Severity.** **P1** as the finding's first requirement is written: "a configured filter and anything it starts".
  **P3** if the owner extends to the engine's Git children a boundary that exists twice already (decision D3):
  - `DESIGN.md` §15: "code that deliberately daemonises out of that group remains outside the host-runner contract"
    (`design/15_design_event_log_resume_run_layout.md:64`);
  - the packet's INV-18 recovery: "escaped daemonized host descendants are outside host guarantees".
- **G6.** Q1: a slot reused while the program can write.
- **Blocks G6?** Yes, unless D3 accepts the exclusion or D2c closes it for slot paths.

**R-1W. Windows: a configured program escapes the job deliberately.** The same as R-1, by having another process start
a process outside the job: a service, the task scheduler or WMI. Breakaway itself is refused, because neither job
allows it (§2.5.4). Its severity, evidence and G6 position are R-1's, and so is decision D3.

**R-W. Windows, under D2a only.** I/O in flight at termination lands after recovery (§2.5.1).
- **Evidence.** Reasoned from Microsoft's documentation; not executed.
- **Severity.** **P1**.
- **G6.** Q1, ST-16, ST-18, INV-22.
- **Blocks G6?** **Yes** under D2a. It is closed under D2b′, and for slot paths under D2c.

**R-G. The legacy engine's maintenance prunes a registration being recreated.** A legacy `git commit` starts automatic
maintenance, detached at 2.55.0 (§2.2). If its `gc` runs `worktree prune` while a topology recovery removes and recreates
a slot registration, it can delete the replacement by path. That is the design lens's sequence (finding 1), with a
legacy command as the starter.
- **What it needs.** A `gc.worktreePruneExpire` shorter than the registration's age, which the 3-month default never is
  for a fresh registration (`builtin/gc.c:64`, `worktree.c:774` at 2.43.0); a legacy and a topology command on one
  checkout; and the race window.
- **Evidence.** The detach was executed (§2.2); the race is reasoned.
- **Severity.** **P3**. It belongs to the class of `PR11-HOST-AGENT-PRUNE-RACES-AN-ENGINE-ADD` (P2), but needs a
  non-default setting that class does not.
- **G6.** Q1.
- **Blocks G6?** No. It is filed with the implementation.

#### Residuals that cost liveness only

Each of these refuses; none proceeds.

| Id | Where | What costs liveness | Evidence | Severity | Blocks G6? |
|---|---|---|---|---|---|
| R-Z | Unix | A Git member that exits after its coordinator's death, under an adopter that never reaps (a container init that does not), stays a zombie in `P`. `kill(2)` counts it (`c2/docs/man2-kill.txt:80-83`), so every acquisition refuses until the adopter reaps it or exits. The sentinel's own case is closed (§2.4.1). | The regression lens executed it for the sentinel; reasoned for Git members | P3 | No |
| R-L | Unix | A long-lived process that a Git child starts without leaving `P` keeps `P` from emptying. Git's own credential-cache daemon is one: spawned only on a credential `store` (`builtin/credential-cache.c:164` at 2.43.0, `:180` at 2.55.0), never detaching (no `setsid` or `daemonize` in `credential-cache--daemon.c`), and alive until its cache times out. The next write command refuses, naming the group. | Source | P3 | No |
| R-T | Unix | A member that reads the terminal, or writes it under `tostop`, stops `P`, because `P` is a background group of the coordinator's session (`c2/docs/posix-general-terminal-interface.txt:93-98`, `:102-107`). The coordinator then waits on that Git command until an operator ends it. `^C` ends the coordinator, and its death orphans `P`, so the kernel hangs up the stopped members (`c2/docs/posix-orphaned-group-exit.txt:116-117`). `GIT_TERMINAL_PROMPT=0` removes Git's own prompts. On master the same read stops the whole coordinator group if the coordinator is in the background, and prompts the user if it is in the foreground. | Executed by the concurrency lens (finding 3); POSIX | P3 | No |
| R-2 | Unix | A group number reused after the group emptied makes the next command wait on an unrelated group, within the recorded namespace. This is §1.7's R-2. | Kernel sources, read by round 1 and both lenses | P3 | No |
| R-3 | Both | A hung writer holds recovery until it ends or an operator ends it. The finding's fourth requirement asks for this to be stated. | — | Stated, not a finding | No |
| R-K | Windows, D2b′ | The keeper ended before writing its marker: killed together with the coordinator, or across a restart the tick rule does not recognise. The next command refuses until the operator removes the record. | Reasoned | P3 | No |
| R-NS | Linux | A successor in another PID namespace refuses until it resumes from the recorded namespace or the operator removes the record. | §2.4.3 | Stated | No |

**Filing.** R-1 and R-1W are filed with the implementation at the severity D3 decides. R-G, R-Z, R-L, R-T, R-2 and R-K
are filed at P3.

### 2.7 Resource accounting and instruments, decision D1 (FUC-D1-ACCOUNT)

#### 2.7.1 Two resources, two rows

The packet's completeness rule asks for "one row per distinct physical or logical resource/ownership granularity,
non-overlapping", and gives each row one enforcement domain. The record and the group are two resources, in two domains.
- **The record is a durable file**, reclaimed by tabled action. That is `external_physical`, like an intent.
- **The group is OS state**, held by its members and released by the OS when the last of them exits, never by cleanup.
  The next coordinator observes it and refuses until it is released. That is `process_local_os`, and R28, "a surviving
  Unix reaper's shared cleanup.lock hold … observed (never owned or reset)", is the precedent.

The design lens objected that one row cannot be both (finding 4). Under D1a they are:
- **R29, a write command's writer record.** `<worktree git dir>/upstroke-writer-group`, and under D2b′ its keeper's
  drained marker `upstroke-writer-drained-<id>` beside it.
  - Domain: `external_physical`.
  - Granularity: per physical worktree Git directory, one write command at a time under R17's lease.
  - Lifecycle: published before the command's first Git child. Unlinked at release when R30 is observed released.
    Otherwise left, and reclaimed at the next write command's worktree-lease acquisition once R30 is observed released or
    its boot is established ended. A record the acquisition cannot establish stays, and that command refuses resumably.
    Under D2b′ the keeper publishes its marker once R30 is released, and the marker is removed with the record.
- **R30, a write command's writer group.**
  - On Unix, the process group its manager's Git children join: leader reaped, sentinel a member until the coordinator
    ends.
  - On Windows under D2b′, the writers' job: the coordinator and everything it starts after joining, with the keeper's
    handle to it.
  - Domain: `process_local_os`.
  - Granularity: per write command.
  - Lifecycle: held while any member lives, and released by the OS when the last exits. Never reset. Observed, never
    owned, by the next write command's acquisition, which refuses until it is released.

#### 2.7.2 The outcome equations, and the ledger

**The packet's equations gain these clauses.**
- **Complete, Parked, Halted, BudgetExceeded.** "R29 released (retained only while R30 is held); R30 released (held only
  by a member that outlived the command, observed by the next write command)".
- **NoRunFinished.** "R29 resumably_open (reclaimed at the next write command's acquisition); R30 may be held by
  surviving members and is observed, never reset; not part of the coordinator's empty-at-start ledger (R17)".

**`src/engine/topology/ledger.rs`.** The design lens found that adding a `ResourceRow` alone leaves the independent ledger
blind (finding 5).
- **`Row`** (`:14`): R29 and R30 are added. `Row::ALL` (`:46`) goes from 28 to 30, with `resource()` and `domain()` arms
  (`:112`): R29 `ExternalPhysical`, R30 `ProcessLocalOs`.
- **Observations.** `PhysicalInventory` (`:292`) gains `writer_record_present`, and under D2b′ `writer_markers`.
  `ProcessLocal` (`:284`) gains `writer_group_held`. The observation list gains R29 and R30, beside R25's (`:770`) and
  R28's (`:820`).
- **`equation`** (`:841`).
  - The four ended outcomes: R29 and R30 each `Released`/`Absent`.
  - `NoRunFinished`: each `ResumablyOpen`/`Any`, as R28 is (`:980`).
- **`check`** (`:988`) gains one joint rule. At an ended outcome, R29 and R30 may be present only together, which is the
  surviving-helper case. One without the other is a disagreement. The joint case is reported as retained for the next
  command, not as balanced.
- **The ledger's tests** (`src/engine/topology/ledger/tests.rs`) re-pin their counts, and so do its notes
  (`docs/internals/engine/topology/ledger.md`). G6's per-resource ledger artifact gains both rows.

#### 2.7.3 The sites

| Site | Row | Adjacent | Fault row | Scope | Read-only | Parent-side points |
|---|---|---|---|---|---|---|
| `Lock.OpenWriterGroup` | R30 | `Before(RunStarted)` | T-RUNSTART | Shared | no | Unix: leader forked, sentinel joined, leader reaped. Windows (D2b′): job created, keeper holds it, coordinator joined. |
| `Lock.PublishWriterRecord` | R29 | `Before(RunStarted)` | T-RUNSTART | Shared | no | — |
| `Lock.ObserveWriterGroup` | R30 | `Before(RunStarted)` | T-RUNSTART | Shared | **yes** | — |
| `Lock.ReclaimWriterRecord` | R29 | `Before(RunStarted)` | T-RUNSTART | Shared | no | — |
| `Lock.CloseWriterGroup` | R30 | `After(RunFinished)` | T-FINALIZE | Shared | no | Unix: sentinel killed, sentinel reaped, group probed. Windows: job counted, `drop` sent. |
| `Lock.RetireWriterRecord` | R29 | `After(RunFinished)` | T-FINALIZE | Shared | no | — |
| `Process.SpawnWriterKeeper` (D2b′) | R30 | `Before(RunStarted)` | T-RUNSTART | Shared | no | — |
| `Lock.PublishDrainedMarker` (D2b′, run by the keeper) | R29 | `None` | the registry assigns it; T-FINALIZE is closest | Shared | no | — |

- **Placement.** They sit beside `Lock.AcquireWorktree` and `Lock.Release`, which have the same adjacencies and fault
  rows (`src/topology/effects/sites.rs:1188-1219`).
- **Two sites per record, one per context.** The removal at acquisition and the removal at release are two sites
  because each site has one adjacency.
- **The observation is read-only**, like `Lock.ObserveCleanupHold`. It reads R29's file and probes R30. Its waiting is
  not an effect.

#### 2.7.4 The instrument inventory, D1a, with counts

Counts at this head are from `c2/census/census-6b28452d.txt`. Under D2b′ the counts in parentheses apply instead.

| Instrument | Change |
|---|---|
| `src/topology/effects/vocab.rs` | `ResourceRow::ALL` 15 → 17 (`:131-186`). Every exhaustive arm gains R29 and R30, and the vocabulary tests' counts move. |
| `src/topology/effects/sites.rs` | `LockSite::ALL` 6 → 12 (13); `ProcessSite::ALL` 2 (3); all sites 70 → 76 (78). Each new variant gets `row`, `adjacent`, `fault_row`, `scope`, `is_read_only`, `sub_effects` and `residue_classes`. |
| `src/topology/effects/residue_authority.rs` | `LockSite`'s per-site matches (`before_state`, `:940`, and the rest), and `ProcessSite`'s under D2b′, gain the new arms. A missing arm fails to compile, the regression lens's point. |
| `src/topology/effects/tests.rs` | `tie!(LockSite, 6, …)` → 12 (13) (`:677`); `tie!(ProcessSite, 2, …)` → (3) (`:688`); `assert_eq!(ResourceRow::ALL.len(), 15)` → 17 (`:950`). |
| `effect_sites.json` | 70 → 76 (78) entries, regenerated. |
| `effects/funnel-modules.json` | `sites_checked` 70 → 76 (78) (`:29`). `the_checked_in_funnel_module_record_states_where_the_bodies_are` (`src/effects/tests.rs:6220`) compares it. |
| `effects/sequential-registry.json` | `range` 68 → 74 (76) site names. `entries` 180 → at least 192 (196): two phases per new site, plus one entry per parent-side point and injection mode, each naming the test that executes it. |
| `effects/residue-classes.json`, `effects/residue-synthetic.json` | Unchanged. Their nine entries are the Object sites' command-internal classes, and no new site wraps a Git command. |
| `effects/wrappers.toml` (54 rows), `clippy.toml` | A row for each crate-visible effectful function the lock funnel exposes to the manager. The design keeps that to the group-opening entry point and the Git-child configuration it returns. |
| `effects/allowlist.toml` | The forks, `kill`, `setpgid`, `setsid`, and the Windows job and process calls are disallowed primitives. They stay inside `src/rundir.rs`, an allowlisted funnel module, so no module is added. That file's recorded lint set is checked against what it now needs. |
| `src/runner/contract.rs`, the process-start census (`:1632`) | It counts `Command::new(`, `.spawn()` and `run_with_timeout`, not forks, so the Unix side moves nothing. Under D2b′ the keeper's spawn adds a row for `src/rundir.rs`, `(1, 1, 0)`, and `assert_eq!(expected.len(), 5)` (`:1747`) becomes 6. |
| `src/runner/contract.rs`, the command-payload census (`:2500`) | The manager's row is `(2, 8, 0)`. The three new environment variables move it if they are spelled `.env(`, on both the builder and `read_only_git`, and its text moves with it. Under D2b′ the keeper's `.stdin(` adds a row. |
| `src/engine/topology/ledger.rs` and its tests | §2.7.2. |
| `docs/internals` notes | `rundir`, `workspace_manager`, `engine/topology/ledger` and the effects vocabulary and sites notes, each pinned by the notes gates. |
| `DESIGN.md` | §15 (`design/15_design_event_log_resume_run_layout.md:64`) gains the writer group, under D2b′ the keeper, and under D3(a) the R-1 boundary. Under D2b′, §26's Windows clause (`design/26_design_merge_queue_protocol.md:398`) changes too. |

All but the ledger's subject code are instruments under `CLAUDE.md`'s first limb, so the implementation's merge is the
owner's, as is each erratum.

#### 2.7.5 D1b, for comparison

- **R25 amended.** "The worktree lock file, which also carries the current write command's writer record".
  - The record is written in place, through the lease's own descriptor. It is never written by reopening the file,
    because closing any descriptor of it releases every lock the process holds on it (`src/rundir.rs:2566`).
  - It carries a checksum. A torn record, from a writer that died mid-write, is discarded: the writer starts no Git
    child before its write is complete.
  - The legacy commands, which today only lock the file, would read content from it.
- **R29 is the writer group**, as D1a's R30.
- **The same six sites**, with the record's two now on R25. Under D2b′ the keeper's marker cannot be written through
  the coordinator's descriptor, so it needs R25 amended again or a row of its own. That removes D1b's one saving.
- **Counts.** `ResourceRow::ALL` 15 → 16; ledger `Row::ALL` 28 → 29; `LockSite::ALL` 6 → 12 (13); all sites 70 → 76 (78).
- **Recommended against.** It saves one row, and costs an in-place protocol on the file every write command locks.

#### 2.7.6 The errata, as text for the owner

**E-FUC-1 (D1a)**, in the errata file's form:
1. **`decisions.resource_accounting.rows`:** add R29 and R30 as §2.7.1 gives them, with their `at_run_end` cells from
   §2.7.2.
2. **`enforcement_domains.process_local_os`.**
   - Current, in part: "R17 (…), R22 (…), R28 (…)".
   - Amended: append ", R30 (a write command's writer group: on Unix the process group its engine Git children join, on
     Windows the writers' job and its keeper's handle; held by its members, released by the OS when the last exits,
     observed by the next write command, which refuses until it is released)".
3. **`enforcement_domains.external_physical`.**
   - Current, in part: "…, R26, R27".
   - Amended: append ", R29 (the writer record and its drained marker: published before a write command's first Git
     child, reclaimed at the next write command's worktree-lease acquisition)".
4. **`outcome_equations`:** append §2.7.2's clauses to each outcome.
5. **`effect_site_inventory.identity`.**
   - Current: "row(): exactly one of R9-R12, R17, R18, R19, R21, R22, R23, R24, R25, R26, R27, R28".
   - Amended: append "R29, R30", and add §2.7.3's sites to the named sites.
6. **The row range.** INV-22's "R1-R28" and Q2's "(R1-R28)" become "R1-R30".
7. **ST-09 and ST-10:** add "R30 asserted observed while a simulated surviving writer holds it, refusing the next write
   command, and released afterwards; R29 asserted reclaimed".
8. **INV-18's `recovery`:** after the R28 clause, add "a write command's writer group (R30) is observed and refuses the
   next write command's worktree-lease acquisition until released".

**E-FUC-2 (D2b′ only).**
- **INV-18's statement.**
  - Current: "on Windows every host child is a member of the coordinator's ambient kill-on-close Job Object from
    creation".
  - Amended: "… from creation, except the write command's writer keeper, which the command starts before it joins the
    ambient job, which starts no process and runs no repository or agent code, and which holds only the handle of the
    command's writers' job until that job is observed empty".
- **INV-18's `enforced_by`:** add "writer keeper started before the ambient join; writers' job joined at the command's
  first engine Git child".

### 2.8 The legacy boundary, corrected (FUC-D1-LEGACYBOUND)

The legacy commands create no record. They take the same lease, through `WorktreeLock::acquire_in`
(`src/engine/coordinator.rs:132`, `src/engine/resume.rs:148`), and so read the record at acquisition. A schema 1–3
command therefore waits, or refuses after 10 s, whenever a topology command of the same checkout has left a record
naming a group with a member. That happens:
- after a topology command died inside, or after, a Git write;
- after one **completed normally** while a process its Git writes started still ran, a filter's helper say. §2.4.4 then
  keeps the record.

§1.13's "only after a topology run of the same checkout died" missed the second case. T13 tests it.

On Windows under D2b′ the legacy commands read Windows records the same way. Every write command starts a keeper,
because the schema is not known at startup. A legacy command's keeper receives no job and exits with the command: one
extra process per legacy write command on Windows.

### 2.9 Placement, the frozen set, and the siblings

- **Nothing frozen moves** (R-D's list in `reviews/2026-09-30-pr11-record.md`).
  - `src/rundir.rs` takes the lease side: the observation and reclaim at acquisition, the process-local table, the
    group's opening entry point and its release, and on Windows the keeper's library half and the probe.
  - `src/workspace_manager.rs` takes the builder's settings and group membership, and `read_only_git`'s.
  - `src/main.rs` takes, under D2b′, the keeper's start before the ambient join, and its hidden mode.
  - None is frozen. `recover.rs` calls `acquire_in_hooked` at `:271` as before, and `dispatch.rs` is unchanged.
- **Follow-up A (#328)** owns `src/agent/proc.rs` and the container launch funnel. This design edits neither. The probe
  is a new helper. The forks are new code that keeps `proc.rs`'s discipline after `fork`. If A later moves
  `process_alive` to three outcomes, the two can share it.
  - If A edits `main.rs`'s `containment` module, D2b′'s hunk would meet it, and whichever merges second rebases.
- **Follow-up B (#329)** owns the registry wrapper and targeted removal in `src/workspace_manager.rs`. This design's
  hunks there are the builder (`:4994-5009`), `read_only_git` (`:5452-5464`) and new private functions. They are
  disjoint.
- **Other checkouts of the repository.** The record is per checkout. A coordinator in another checkout of the same
  repository takes another lease, and reads only its own checkout's record.
  - Administrative names are shared under the common directory: `k1-g1` is every run's first task. So a dead
    coordinator's orphan could still write a registration that another checkout's live run has just created
    under the same name.
  - That is the cross-checkout registry class follow-up B (#329) owns, as the regression lens's classification
    recorded ("Cross-checkout registry races | Remain follow-up B's scope").
  - The record is where B's repository-wide exclusion can wait: each checkout's record sits in its own Git
    directory, under the common one.

### 2.10 Tests and mutations, revised

**How the tests hold processes.** Holds are release files, as in §1.10, plus the lease and job states each test sets
up for itself. Every signal a test sends goes to its own children.

**Unix.** The Linux and macOS legs run these unless a row says otherwise.

| Test | What it does | What it asserts |
|---|---|---|
| T1, the late `reset` | §1.10's T1. | The resume waits while the group has members, proceeds after `ESRCH`, and the edits stand. Held past the bound, it refuses resumably, naming the group. |
| T2, the filter's helper | §1.10's T2. | `ESRCH` comes only after the helper ends. |
| T3, `checkout--worker` | §1.10's T3. | Every worker observed reports `P`. |
| T4, `update-ref` | §1.10's T4. | At 2.43 the child is in `P`. On later Gits the child does not exist, and the test says so. |
| T5, crash points | Kills at every row of §2.4.6, each release row twice: with no surviving member, and with T6's surviving helper. | Each row's outcome. In particular, the kill after the sentinel's reaping, with a helper alive, waits or refuses and never proceeds. That is the corrected oracle. |
| T6, release | An empty group; a helper outliving the command; the whole group stopped with `kill(-P, SIGSTOP)`. | Empty: the record is unlinked. Helper: it is kept, and the next command waits. Stopped: release returns within its one-second bound, keeps the record, and the lease is free. |
| T7, R-1 pinned | A helper started under `setsid`. | It is outside `P`, and nothing waits for it. |
| T8, PID namespaces | Unit tests over the decision table: a record naming another namespace identity, or `unknown`. Executed too, on Linux where the platform allows an unprivileged PID namespace: a successor in a new namespace. | Both refuse. Where no namespace can be made, the unit tests stand alone and the test says so. |
| T9, boot and filesystem | Unit tests: another boot with `local`/`local`, `local`/`shared`, `shared`/`local`; another platform. | Removal only in the first case. |
| T10, the sentinel leaves (Linux) | In an isolated child process that is a subreaper and never reaps, kill a child coordinator. | `P` reads `ESRCH` once `S` has left, though `S`'s zombie remains. That is the regression lens's retention, now harmless. |
| T11, Git settings | A unit test pins the builder's and `read_only_git`'s arguments and environment. Executed in a blob-less partial clone, as `c2/witness/gc/witness_gc.py` runs it. | The engine's add fails, naming the missing object. It starts no fetch, or on a Git without `GIT_NO_LAZY_FETCH` no transport, and no `maintenance` child (a trace2 `child_start` census). |
| T12, the reads join | A clean filter started by `read_only_git`'s `status`. | The filter reports `P`. |
| T13, legacy after a normal topology completion | A topology command completes with a surviving helper; then a schema 1–3 command runs. | The schema 1–3 command waits, refuses after 10 s while the helper lives, and proceeds once it has ended. |

**Windows.** The `winguest` leg and the hosted queue lane run these. A child coordinator here is the test binary in a
coordinator mode that starts its keeper first, as `main.rs` does.

| Test | What it does | What it asserts |
|---|---|---|
| TW1, kill | The child runs a manager Git command held by a smudge filter waiting on a release file. The parent opens handles to the `git` and filter processes, then terminates the child. | The parent's acquisition waits for the marker, and both held processes are signaled before the marker exists. |
| TW2, keeper and coordinator killed together | The parent terminates the child and its keeper, with a member still held. | No marker: the acquisition refuses, naming the keeper. After the test releases the member and removes the record, it proceeds. |
| TW3, the probe | `Alive` (a live child); `Gone` (an exited child, and a creation-time mismatch); `Unobserved` (an `OpenProcess` failure through the probe's seam). | The three outcomes, and that `Unobserved` refuses. |
| TW4, normal release | Count 1; and a surviving process. | Count 1: the record is unlinked and no marker is written. Surviving process: the record is kept, and the marker follows the child's exit. |
| TW5, nesting | A process started after the join; an agent started after the join. | The process is in `N` and in the ambient job (`IsProcessInJob` on both). The agent's private job nests below `N`. |
| TW6, legacy | A schema 1–3 command's acquisition, over a Windows record. | It waits for the marker. |

**What the Windows legs can show.**
- They can show: the ordering for ordinary members (the marker comes only after the members are signaled), the keeper's
  protocol, nesting, and the probe.
- They cannot show an I/O still pending in the kernel completing after termination began: no test holds one without a
  driver. R-W's closure rests on Microsoft's documented semantics (§2.5.4, step 3 of the proof).
- Each TW test runs one child coordinator, one keeper and a few Git commands: seconds apiece. The rest of the Windows
  suite runs in process and starts no keeper. Neither the guest harness's roughly 468 s nor the hosted queue leg's
  45-minute limit is threatened, and the implementation measures both.
- The record and the marker sit directly in the test repository's Git directory, so the deepest new path is that
  directory plus a 56-character name.

**Mutations**, each made to fail a named test.

| Mutation | Fails |
|---|---|
| M1: drop `process_group(P)` | T1, T2, T3 |
| M2: skip the wait | T1, T2 |
| M3: publish the record after the first Git child | T5 |
| M4: read `EPERM` as empty | the decision-table unit test |
| M5: unlink the record at release whatever the probe says | T6 |
| M6: skip the namespace comparison | T8 |
| M7: remove another boot's record whatever its filesystem class | T9 |
| M8: let the sentinel exit without `setsid` | T10 |
| M9: drop the maintenance settings | T11 |
| M10: leave `read_only_git` outside the group | T12 |
| M11 (Windows): write the marker without waiting for 0 | TW1 |
| M12 (Windows): proceed when the keeper is gone and there is no marker | TW2 |
| M13 (Windows): read an `OpenProcess` failure as `Gone` | TW3 |

### 2.11 G6 classification

The table reads the recommended choices: D1a and D2b′, with D3 open.

| Portion | Closed or residual | Severity and evidence | Applies to | Blocks G6? |
|---|---|---|---|---|
| Unix: ordinary descendants (Git's children, filters, helpers, `checkout--worker`) | Closed once implemented | §2.4.5; round 1's executed membership | Q1, ST-16, ST-18, INV-22 | No, once implemented |
| Unix: Git-started maintenance (FUC-D1-GC) | Closed by the settings | Executed on 2.43 and 2.55 (§2.2) | Q1 | No |
| Unix: a successor in another PID namespace | Refuses, so never proceeds wrongly | Failure executed by the concurrency lens; closed by the rule | Q1 | No |
| Unix: another boot, or another kernel | Removed only where its boot has provably ended; otherwise refuses | Reasoned (§2.4.3) | Q1 | No |
| Unix and Windows: deliberate detach or escape (R-1, R-1W) | Residual | P1 as the finding is written; P3 under D3(a) | Q1 | Yes, unless D3(a), or D2c for slot paths |
| Windows under D2b′ | Closed once implemented, I/O in flight included | Microsoft's documented semantics; CI shows the ordering | Q1, ST-16, ST-18, INV-18, INV-22 | No, once implemented |
| Windows under D2a instead | R-W open | P1 | Q1, ST-16, ST-18, INV-22 | **Yes** |
| Windows under D2d instead | The finding's Windows half open | P1 | Q1, ST-16, ST-18, INV-22 | **Yes** |
| The legacy engine's maintenance prune (R-G) | Residual | P3 | Q1 | No |
| Liveness (R-Z, R-L, R-T, R-2, R-K) | Residual refusals | P3 each | Q1's refusal path | No |
| Accounting (D1a) | R29 and R30, with their equations and the ledger | — | Q2, INV-22 | No, once E-FUC-1 is adopted |

The finding stays `deferred` at this head. The implementation's repair deletes it.

### 2.12 The owner's decisions

| | Decision | Recommendation | If declined |
|---|---|---|---|
| **D1** | Accounting: D1a, erratum E-FUC-1 (rows R29 and R30, the six sites, the equations); or D1b, R25's content plus one row | **D1a** | The group and its record are unaccounted, and G6's INV-22 reading would not cover them. |
| **D2** | Windows: **D2b′**, the keeper-held joined job (E-FUC-2); D2c, unique slot paths (E-FUC-3); D2a, the coordinator wait, leaving R-W at P1; D2d, no change, leaving the filed P1 | **D2b′** | D2a and D2d each leave a P1 that blocks G6. D2c needs a packet change and leaves refs and prunes. |
| **D3** | R-1 and R-1W: (a) extend `DESIGN.md` §15's and INV-18's existing boundary for deliberately daemonising code to the programs the engine's Git commands run, and file R-1 at P3; (b) close slot paths with D2c as well; (c) keep R-1 at P1 | None: it is a scope decision. The evidence (§2.6) is that Git itself never detaches under the settings, that the common filter does not, and that the boundary already exists for agents. | Under (c), R-1 blocks G6, and no in-lane remedy exists but D2c. |

The Unix design depends on neither D2 nor D3.

### 2.13 What §2 replaces in §1

| §1 | Replaced by |
|---|---|
| §0's status line | §0, updated |
| §1.2: "none of the engine's writer builtins … calls `run_auto_maintenance`" taken as closing Git's detach paths | §2.2: lazy fetch reaches it; the settings and their version table |
| §1.2: "Every one of them except `write-tree`, `commit-tree` and the reads" | §2.3: every child but `commit-tree`; the reads join |
| §1.12: "D2c would make most of the Unix machinery unnecessary for slot paths" | §2.5.6: true for slot paths only; the prune and every non-slot write still need the group |
| §1.4: the sentinel (`git hash-object --stdin`, the group's leader, exiting on end of file) | §2.4.1: no leader; a forked sentinel that leaves the group |
| §1.4: the record's content (boot and `P`) | §2.4.2: platform, boot, PID namespace, filesystem class, `P` |
| §1.4: "a record from another boot is removed" | §2.4.3: only where both sides saw a local filesystem |
| §1.4: release "closes the sentinel's pipe, reaps the sentinel" | §2.4.4: kill and a bounded reap |
| §1.2: "The only other spawner, `read_only_git` … runs reads only" and "The builder's reads write nothing in a slot", which left them outside the group | §2.3: the reads join |
| §1.5: the ordering proof and the crash table's release row | §2.4.5, §2.4.6 |
| §1.5: "Release costs one sentinel reap" | §2.4.7 |
| §1.6: D2a recommended; R-W graded P2 on "only after … removed and re-added" and "at most one operation per thread"; `process_alive` called | §2.5: D2b′ recommended; R-W re-argued at P1; a three-outcome probe |
| §1.7: R-1 "Severity: P3"; R-1's premise | §2.2 (premise), §2.6 (P1 as written, P3 under D3) |
| §1.9: D1a's single row R29 "external_physical for the record"; the inventory's counts (9 Lock sites, 73 sites, 16 rows); D1b "no new row or site" | §2.7 |
| §1.10: T5 "each row of §1.5's crash table"; T8 | §2.10 |
| §1.11 and §1.12 | §2.11 and §2.12 |
| §1.13: "a v0.1 checkout sees a change only after a topology run of the same checkout died" | §2.8 |
