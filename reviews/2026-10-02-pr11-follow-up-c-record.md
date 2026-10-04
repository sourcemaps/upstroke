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
figures are under `~/orch-pr11/logs/pr11_fuc_design2/`, cited as `c2/…`. Design round 3 (§3), which answers design
review round 2 and frames the closure choice, is `pr11_fuc_design3`'s (the same model and effort, a fresh session on this
branch at `a9be94bc`); its figures are under `~/orch-pr11/logs/pr11_fuc_design3/`, cited as `c3/…`. Design round 4
(§4), which answers design review round 3 and repairs U, is `pr11_fuc_design4`'s (the same model and effort, a fresh
session on this branch at `a0464f43`); its figures are under `~/orch-pr11/logs/pr11_fuc_design4/`, cited as `c4/…`.
Design round 5 (§5), which answers design review round 4 and is the last design round before the owner's consolidated
question, is `pr11_fuc_design5`'s (the same model and effort, a fresh session on this branch at `12375c7e`); its figures
are under `~/orch-pr11/logs/pr11_fuc_design5/`, cited as `c5/…`. The implementation (§6) is `pr11_fuc_impl`'s (the
same model and effort, a fresh session on this branch at `30026823`), after design review round 5 and the orchestrator's
assessment of the unit to build (`~/orch-pr11/c-impl/UNIT.md`); its figures are under `~/orch-pr11/logs/pr11_fuc_impl/`,
cited as `c6/…`. Repair round 2 (§6.9), after CI on `1fc0c911`, is `pr11_fuc_impl2`'s (the same model and effort, a fresh
session on this branch at `1fc0c911`); its figures are under `~/orch-pr11/logs/pr11_fuc_impl2/`, cited as `c6r2/…`.
Repair round 3 (§6.10), after the implementation review of `83516466`, is `pr11_fuc_impl3`'s (the same model and effort,
a fresh session on this branch at `83516466`); its figures are under `~/orch-pr11/logs/pr11_fuc_impl3/`, cited as
`c6r3/…`.

**The evidence plan was conservative, by direction.** Read our code, read Git's source at the three versions that
matter, cite the corruption witnesses #329 already executed at base rather than rebuild them, and run one new witness
that uses only our own `git` commands in a temporary directory. That witness holds its filter with a release file, and
its only signal is a `SIGKILL` of its own coordinator child. Nothing was traced, preloaded or injected. Round 2 kept the
same plan: it read Git's source at four tags and Microsoft's, Linux's and POSIX's documentation, and its one new witness
runs only `git` commands in temporary directories, observed through Git's own trace2 event stream, with no signals (§2,
opening). Round 3 kept it again: it read our code at master `5c222ff2` and Git's source at four tags, and its one new
witness runs only `git` commands in temporary directories, holding them with a filter or a hook that waits on a release
file and sending no signal at all (§3, opening). Round 4 kept it again: it read our code at `5c222ff2`, the standard
library's random-key source at Rust 1.85.0, and Git's split-index source at two tags, and its one new witness runs only
`git` commands in temporary directories, held before they run or in a smudge filter, with no signal (§4, opening). Round
5 narrowed it further, by the orchestrator's direction, after provider-safeguard pauses (`~/orch-pr11/ESCALATION.md`
item 8; corrected at implementation, the decision appendix's §11): it read our code at `5c222ff2` and Git's source at
two tags, and its two new witnesses run only ordinary `git` commands, one after another, in temporary directories. They
record what Git wrote only from Git's own trace2 stream and plain directory listings taken before and after, and hold or
signal no process (§5, opening).

## 0. Status

| Phase | State |
|---|---|
| Implementation (§6) | **IMPLEMENTED; REVIEWED AT `83516466` (CHANGES_REQUIRED, ONE P1) AND REPAIRED IN ROUND 3 (§6.10); THE REPAIR NOT YET REVIEWED — U as §4 and §5 specify, with the PR11 decision appendix's §11 rows for this change.** It is built on #329's head, merged in at `e46b71d3`, and uses #329's targeted removal and tolerant registry access as they are. No frozen file changes, and D4 did not trigger (§6.3). Accounting, the packet and every Git child's inherited environment are unchanged. Its merge is the orchestrator's once the owner's decisions it waits on are made: #329 merged, E-FUC-3's adoption, O4, O7 (or R-REF's disposition) and O3's or O3-R's route for FUC-D5-GITINDEXFILE (§6.9). Repair round 2 (§6.9) fixed CI's one red leg on `1fc0c911`, a pre-existing Windows-only test whose fixture planted its file under the slot's untagged name, and audited every platform-gated test for the same derivation: there is no other. Repair round 3 (§6.10) fixed the implementation review's five items: the torn-registration repair removes the proven torn instance alone, a resume's recreate reclaims every earlier incarnation's intent before its replacement, Q's three open findings are filed, and two test-only items. |
| Design, round 5 (§5) | Reviewed by design review round 5 on `30026823` (`~/orch-pr11/reviews/review-330-d5-triage.md`), whose items against U §6 takes. Round 5 read: **PROPOSED — the last design round before the owner's consolidated question; the owner's decisions D1 to D5 (§5.9).** It answers design review round 4, whose three lenses returned CHANGES_REQUIRED on `12375c7e` with one P1: Git's shared rerere state crosses U's instance boundary. Every engine Git command now runs with rerere disabled, executed on 2.43.0 and 2.55.0 for its effect: no engine pick reads or writes `rr-cache` (§5.2). The common git dir is censused path by path, by plain listings and trace2 on both versions, and the census adds `worktree.useRelativePaths=false` (§5.3). Terminal finalization's last step sweeps every earlier incarnation's instance in non-frozen manager code, and the window after it is R-UR, P3, with E-FUC-3's stated exception (§5.4). R-REF is the owner's decision D5 and blocks G6 until it is made (§5.5). This head changes no production code. |
| Design, round 4 (§4) | Superseded in part by §5; §5.11 lists what it replaces. Round 4 read: **PROPOSED — U repaired, retention withdrawn, Q frozen; the owner's decisions D1 to D4 (§4.15).** It answers design review round 3, whose three lenses returned CHANGES_REQUIRED on `a0464f43` with one P1, in U's tag, and the looping signal raised the third time (§4.1). The production incarnation id now carries host randomness through the standard library's `RandomState`, and the tag is 60 bits of a hash of it (§4.2). Every walk discovers other incarnations' instances no intent names, executed again with saved evidence on 2.43.0 and 2.55.0 (§4.3). A dead instance that cannot be removed refuses the command, so Q1's order and the outcome equations stand (§4.5). The frozen oracles' replacements are specified fixture by fixture (§4.6). R-REF is regraded and filed (§4.8), Q's open items are restated (§4.9), and R-GU is analysed from DESC's side and filed by #329 (§4.11). This head changes no production code. |
| Design, round 3 (§3) | Superseded in part by §4; §4.17 lists what it replaces. Round 3 read: **PROPOSED — the closure choice, framed for the owner's decisions D1 to D4 (§3.8).** It answers design review round 2, whose three lenses returned CHANGES_REQUIRED on `a9be94bc`, and the looping signal (§3.1): it proposes the smaller change. Recovery recomputes every slot from non-frozen code and never reads the recorded `worktree_path`, so slot paths and registration names can be unique per coordinator incarnation with no frozen code changed; the packet's T-DISPATCH, R9 and naming texts change instead (erratum E-FUC-3, revised, §3.3.7). Executed with git commands only on 2.43.0 and 2.55.0: a dead incarnation's late add, its `remove_junk` and a `setsid` helper damage a same-path replacement and leave a uniquely named one intact (§3.3.3). R-G is P1, and its in-window variant needs follow-up D's legacy change (§3.4). Q is repaired where no new layer is needed, with PGIDREUSE left open (§3.5). This head changes no production code. |
| Design, round 2 (§2) | Superseded in part by §3; §3.10 lists what it replaces. Round 2 read: **PROPOSED — pending the owner's decisions D1, D2 and D3 (§2.12).** It answers design review round 1, whose three lenses returned CHANGES_REQUIRED on `6b28452d`. Every engine Git command disables automatic maintenance and lazy fetching (§2.2). On Unix, the writer group no longer has its sentinel as leader, and the sentinel leaves when its coordinator dies (§2.4.1). The record names its PID namespace and filesystem class, and an observer that cannot see the group refuses (§2.4.3). Release is bounded (§2.4.4). On Windows, the recommended closure is a writer keeper started before the ambient join, which holds a job the coordinator joins and reports, through a durable marker, when that job is empty (D2b′, §2.5.4). The accounting is two rows, R29 and R30 (§2.7). This head changes no production code. |
| Design, round 1 (§1) | Superseded in part by §2; §2.13 lists every statement it replaces. Round 1 read: **PROPOSED — pending the owner's decisions D1 and D2 (§1.12).** On Unix the remedy is the brief's candidate (1): every engine Git child runs in one process group per write command, led by a sentinel and recorded durably before any Git child joins it, and the next write command of the checkout reuses nothing until that group is established empty. On Windows no in-lane mechanism can observe a dead coordinator's job empty (§1.6), so the Windows treatment is the owner's (D2). The packet's resource and site inventories must name the new record and its observation (D1). This head changes no production code. |

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

These are instruments under `CLAUDE.md`'s first limb, whose merge the owner's standing direction for PR11 leaves to the
orchestrator; an erratum is the owner's to adopt (corrected at repair round 2, §6.9).

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

> **Round 3:** §3.4 replaces the paragraph on the legacy engine's own Git commands: a missing `gitdir` prunes with no expiry, and R-G is P1. The settings themselves stand under either closure (§3.3.6).

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

> **Round 3:** under U, the recommended closure (§3.3), none of §2.4 is built. Under Q, §3.5 amends §2.4.1 (FUC-D2-SENTINELKILL, FUC-D2-PUBORDER), §2.4.3 (FUC-D2-XMACHINE), §2.4.4 and §2.4.6, and says that FUC-D2-PGIDREUSE stays open.

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

> **Round 3:** no longer the recommendation; §3.8's D2 recommends U. Under Q, §3.5 amends the opening (FUC-D2-WINOPEN), the keeper's loop (FUC-D2-KEEPERREF) and its refusal's advice (FUC-D2-XMACHINE).

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

> **Round 3:** §3.3 replaces D2c: the tag is the per-process incarnation's, not an epoch counted from durable starts (FUC-D2-D2CEPOCH), §3.2 settles the frozen-module question, and §3.3.7 replaces this E-FUC-3.

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

> **Round 3:** §3.7 replaces the grading and G6 columns. R-G is P1 (§3.4), and R-1, R-1W and R-W close under U (§3.3.3).

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

> **Round 3:** under Q, §3.5 amends the joint rule (FUC-D2-ACCOUNTSTATE) and the observations (FUC-D2-FROZENTESTS). Under U, R29 and R30 do not exist (§3.3.4).

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

All but the ledger's subject code are instruments under `CLAUDE.md`'s first limb, whose merge the owner's standing
direction for PR11 leaves to the orchestrator; each erratum is the owner's to adopt (corrected at repair round 2, §6.9).

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

> **Round 3:** under U the legacy commands meet no record, and nothing here applies.

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

> **Round 3:** under U, §3.3.6 gives the placement. §3.2 shows that no frozen file moves under either closure.

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

> **Round 3:** under Q, §3.5 replaces T6's oracle (FUC-D2-T6). Under U, §3.3.6 lists the tests.

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

> **Round 3:** §3.7 replaces this table.

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

> **Round 3:** §3.8 replaces this table and adds D4.

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

## 3. Round 3: the closure choice

**What this section is.** Design review round 2 ran three `gpt-6-astra` lenses at `max` on `a9be94bc`, and all three
returned CHANGES_REQUIRED (`~/orch-pr11/reviews/review-330-d2-{design,concurrency,regression}-a9be94bc.review.md`, hashed
in `~/orch-pr11/reviews/SHA256SUMS-330-d2`). The orchestrator's triage (`~/orch-pr11/reviews/review-330-d2-triage.md`)
names the findings used below, raises the looping signal, and frames this round: decide by evidence whether slot names can
be made unique per coordinator incarnation without touching frozen code, repair the quiescence path only where no new layer
is needed, place R-G in follow-up D's legacy scope, give one residual table, and frame the owner's decisions. Where this
section disagrees with §2 or §1, it governs; §3.10 lists what it replaces, and §2's headings carry pointers.

**Who wrote it, and the evidence.** `pr11_fuc_design3` (`claude-opus-5-5`, `max`), a fresh session on this branch at
`a9be94bc`. Its figures are under `~/orch-pr11/logs/pr11_fuc_design3/`, cited as `c3/…`; round 2's stay `c2/…`.
- **Our code at master `5c222ff2`**, follow-up A's merge. Every line §3 cites is printed there by `c3/code/cite.py` into
  `c3/code/code-citations-5c222ff2.txt`, with each file's blob at `5c222ff2` and at this branch's base `92c4ca81`.
  - Every cited file is byte-identical at the two commits except `src/engine/topology/coordinator.rs` and
    `src/runner/container/exec.rs`, which follow-up A changed. §3 cites those two at `5c222ff2`.
  - The whole frozen set, production files and test children, is byte-identical at both (`c3/census/census-5c222ff2.txt`
    §1, from `c3/census/census.sh`).
  - Among the files §1 and §2 cite, only `src/agent/proc.rs` moved, and §3 relies on none of its lines.
- **Git source** at v2.43.0, v2.50.1, v2.55.0 and Git for Windows v2.50.1.windows.1, re-extracted from the tag tarballs
  round 2 fetched, whose hashes match round 2's (`c3/git-src/PROVENANCE.txt`, `TARBALLS.sha256`). The cited lines are in
  `c3/git-src/git-src-citations.txt`, which `c3/git-src/cite.sh` regenerates.
- **The packet** (`~/tactus-artifacts/tactus-parallel-design-neutral-v17.json`, sha256 `a34417d6…`): the texts a
  uniqueness erratum amends, extracted read-only into `c3/packet/packet-extract.txt`.
- **One new witness** within the evidence plan (`c3/witness/uniq/`): only `git` commands in temporary directories. No
  signal is sent to anything, and nothing is traced, preloaded or injected. It ran on the box's Git 2.43.0 and on a Git
  2.55.0 built privately from its tag (`~/orch-pr11/logs/pr11_fub_design6/gits/2.55.0/bin/git`, #329 round 6's build).

### 3.1 The looping signal, and why this round proposes the smaller change

**The signal.** Round 2's pass found P1s in machinery the earlier rounds of this pull request added: a child of the dead
coordinator joining a reused writer group (FUC-D2-PGIDREUSE, in §2.4.1's group), the cross-machine refusal's advice
(FUC-D2-XMACHINE, in §2.4.3's record rules), and D2c's epoch repeating at a crash prefix (FUC-D2-D2CEPOCH, §2.5.6). Its
P2s are in the sentinel, the keeper and the accounting that round 2 invented to keep round 1's group safe. That is
`MAINTAINING.md`'s second signal, and the second of its two cases: "a third round of machinery invented to keep the second
round's machinery safe". #329's rounds 1 to 3 showed the same pattern on this class.

**What `MAINTAINING.md` asks for then:** "the smaller change is the one to propose: keep what has survived a pass, drop
the machinery those rounds invented". This round does that.
- **It adds no layer.** It establishes from the code that the brief's candidate (2), slot names unique per coordinator
  incarnation, needs no frozen code (§3.2), and that it closes the finding's sequences without observing any process,
  executed on 2.43.0 and 2.55.0 (§3.3).
- **It keeps what survived a pass:** §2.2's Git settings, which all three round-2 lenses accepted for the manager.
- **It would drop**, if the owner chooses it: the writer group and its record, the sentinel, the keeper, the
  three-outcome probe, rows R29 and R30, and errata E-FUC-1 and E-FUC-2.
- **The quiescence path is still given**, repaired where no new layer is needed (§3.5), with what it cannot close stated
  plainly, because the choice between the two is the owner's (D2, §3.8).

**Why this converges where rounds 1 and 2 did not.** Every earlier mechanism had to establish that a set of processes
was gone, and each review found a process the set missed. Uniqueness makes no claim about any process. Its one claim is
that no later incarnation creates a path or registration name an earlier incarnation used, and that is a property of
string arithmetic in two non-frozen files. What it leaves is residue and disk at dead names (§3.3.4), not a slot two
incarnations share.

### 3.2 Uniqueness feasibility: where recovery's slot paths come from

**The question.** Can every slot path and administrative name the engine creates be made unique per coordinator
incarnation in non-frozen naming, with no change to frozen recovery or the fold? Or does recovery recreate at the fold's
recorded `TaskDispatched.worktree_path`, which would need a G6-frozen change?

**Recovery recomputes the slot; it never reads the recorded path.** At `5c222ff2` (`c3/code/code-citations-5c222ff2.txt`):
1. Step (g) is `recreate_open_no_attempt` (`src/engine/topology/recover.rs:1029`, `:1616-1627`).
2. Its `open_no_attempt` builds each `OpenGeneration` from the fold's `TaskFold` generations, with
   `slot: task_slot(key, generation.id)` (`recover.rs:1629-1656`, the slot at `:1650`).
3. `task_slot` is in the non-frozen `dispatch.rs`: `Slot::Task { key, generation }` (`src/engine/topology/dispatch.rs:122-128`).
4. `verify_or_recreate` (`dispatch.rs:242-256`) passes that `Slot` to the manager's `verify_worktree`, then
   `remove_worktree` and `create_worktree` (`write_intent`, then `add_worktree`, `dispatch.rs:187-194`).
5. Every one of them turns the `Slot` into a path through `slot_target`, that is `slot_path`
   (`src/workspace_manager.rs:2040-2043`, `:1674-1678`), which is `Slot::relative`, that is `Slot::parts`
   (`src/workspace_manager/naming.rs:177-195`). `add_worktree` runs `git worktree add --detach --quiet <that path>`
   (`workspace_manager.rs:2649-2711`).

**`TaskDispatched.worktree_path` is written once and read by nothing.**
- It is a `String` in the event (`src/topology/events.rs:649-658`), written by the non-frozen `dispatch.rs:141-156` from
  `manager.slot_path(&slot)`.
- The fold keeps no copy (`src/topology/fold/apply.rs:100-125`) and checks none.
- Recovery never reads it. The frozen recovery tests write arbitrary values into it (`"wt/g0"`,
  `src/engine/topology/recover/tests.rs:1729`, `:1744`, `:1768`).
- No other production code names it. A coordinator test drops it from a canonical comparison
  (`src/engine/topology/coordinator.rs:1953`, inside the test module that opens at `:1739`).
- `c3/census/census-5c222ff2.txt` §2 lists every occurrence in `src/`.

**The rest of the frozen code passes `Slot` values and never renders them.**
- The frozen merge module builds `Slot::Staging { sequence }` with a struct literal (`src/engine/topology/integrate.rs:31-35`).
  So a uniqueness tag cannot be a new field of `Slot`.
- Recovery, the merge module and finalization walk `manager.intents()` and match variants with `{ .. }`
  (`recover.rs:1158-1169`, `:1196-1219`, `:1445-1467`; `integrate.rs:1017-1026`; `finalize.rs:121-139`), and
  `reclaim_closed_generations` compares slots by equality (`recover.rs:1451`, `:1459-1460`).
- Every removal goes through the manager.
- Outside tests, a whole slot becomes a path or a file name in three lines of the non-frozen manager:
  `workspace_manager.rs:1677` (`slot_path`), `:1683` (`intent_path`) and `:2275` (`remove_intent`'s own join). The
  intent-name parser is the inverse (`naming.rs:317-352`), a refusal message prints a slot (`workspace_manager.rs:2679`),
  and the container runner reads only a slot's first component, its namespace directory
  (`src/runner/container/exec.rs:148-172`). Census §11 and §12 list every use.

**So the answer is yes, in non-frozen code.**
- Everything a uniqueness tag needs is in `naming.rs` and the manager. `dispatch.rs` need not change.
- No byte of `recover.rs`, the fold, `integrate.rs` or `finalize.rs` changes.
- The event schema does not change either. Only the value of `worktree_path` does, and nothing reads it.
- **The administrative name follows the path.** Git names a registration after the path's sanitized basename and adds a
  counter only when that name is taken (`builtin/worktree.c:445-465` at 2.43.0; `:460-480` at 2.50.1 and at the Windows
  tag; `:494-514` at 2.55.0). The witness's registrations were named `k1-g1_01KZTA7X` and `k1-g1_01KZTB9Q`.

**What it is instead: a packet change, the owner's.** The packet states the reuse that uniqueness removes
(`c3/packet/packet-extract.txt`):
- `decisions.workspace_candidates.manager` names the paths literally: "(tasks/k<key>-g<gen>, merge/s<seq>)".
- T-DISPATCH's `resume_action` verifies "the worktree at the recorded base with Worktree.Verify (linked worktree at the
  recorded path, …)", and its `durable_state` includes the worktree path.
- T-REPAIR-DISPATCH says "Worktree.Verify the recorded worktree at the recorded base".
- R9's `OpenNoAttempt` lifecycle says "reused only after Worktree.Verify".
- §3.3.7 gives erratum E-FUC-3, revised. It replaces §2.5.6's text.

**One fact about production.** The topology engine is dead code outside tests at this head
(`#![cfg_attr(not(test), allow(dead_code))]`, `src/engine/topology.rs:3`; no production `WorkspaceManager::derive` or
`run_recovery_order`, census §9). The change that wires it must give the coordinator's manager the coordinator's own
incarnation, the per-process id generated before any lock (`src/engine/topology/prelock.rs:114-116`). §3.3.1 depends on
that, and the implementation states it where the wiring lands.

### 3.3 U: a slot instance per incarnation

> **Round 4:** §4 repairs U: the tag's source (§4.2), discovery of instances no intent names (§4.3), no retention
> (§4.5), and the frozen oracles' replacements (§4.6).

U is the brief's candidate (2) and §2.5.6's D2c, re-specified so that it needs no durable epoch and no frozen code.

#### 3.3.1 The names

> **Round 4:** §4.2 replaces the tag (now twelve characters, over an incarnation id that carries host randomness), "Why
> the tag is unique", the Windows paragraph and the split rule.

- **The tag.** Eight characters of Crockford base32, rendering the first 40 bits of SHA-256 over a fixed domain string
  and the manager's incarnation id. The manager already holds that id: `WorkspaceManager::derive` takes it and stores it
  (`workspace_manager.rs:1394-1399`, `:1616-1641`), and `write_intent` records it (`:2224`).
- **An instance's component** is today's component, an underscore, and the tag: `tasks/k<key>-g<gen>_<tag>`,
  `merge/s<seq>_<tag>`, `snapshots/<name>_<tag>`. `_` is a legal component character (`safe_component`,
  `naming.rs:148-175`) that no slot component uses today, so the split is unambiguous.
- **Its intent** is `intents/<namespace>.<component>_<tag>.intent`. The record's `slot` field names the instance, so it
  still mirrors the relative path (`naming.rs:398-400`). The `incarnation` field is unchanged (`:403-405`).
- **Its registration** is `<common git dir>/worktrees/<component>_<tag>`, Git's own naming from the basename (§3.2).
- **`Slot` values do not change.** The tag is the manager's, applied when it renders a slot, because the frozen merge
  module builds `Slot::Staging` by struct literal (§3.2).
- **A name written before U** (an untagged intent or directory) is an instance of no current incarnation. It is reclaimed
  like any other dead instance (§3.3.2).

**Why the tag is unique.** It is unique if the incarnation id is, and the packet already makes that id the per-process
identity:
- `decisions.workspace_candidates.run_creation` lists "generation of the coordinator incarnation id (per-process ULID)"
  among the pre-lock checks (`c3/packet/packet-extract.txt`). The code generates it there
  (`prelock.rs:114-116`; `RealIds::incarnation`, `seams.rs:206-208`).
- R26 names every container by "run id and incarnation", and every intent records it.
- `crate::ulid` is SHA-256 over the clock's milliseconds, the pid and a per-process counter (`src/ulid.rs:17-51`). So two
  incarnations share a tag only if they share all three, or if their 40-bit prefixes collide (2^-40 for a pair).
- One coordinator dies before its successor starts, so their pids can be equal only after the pid is reused. Their
  milliseconds can then be equal only if the clock stepped back, or on two machines sharing the checkout whose clocks
  disagree; and their counters must agree as well.

**FUC-D2-D2CEPOCH does not arise.** §2.5.6 counted the epoch from the run's durable starts. A recovery that died between
its first write and `RunResumed` (`recover.rs:1029`, before `:1482`; the epoch's increment is the fold's) left the next
process the same count, and so the same path. The tag is counted from nothing durable. The id is drawn at process start,
before any lock, so a recovery that dies anywhere leaves its own tag behind, and the next process draws another.

**Windows.** The tag spends Git for Windows' `$GIT_DIR` budget of 220 bytes (corrected at implementation, the decision
appendix's §11), measured on the guest (`c3/pathbudget/tag-length.txt`, citing
`~/pr10-evidence/fix-g5-b/r9/guest/gitdir-threshold.log`).
- The tightest path PR11 measured is 207 characters (`~/orch-pr11/logs/pr11_impl_g/measure/pathbudget-child-temporary.txt`).
- So `207 + 1 + L <= 220` gives L at most 12. Eight characters give 216.
- The tightest fixtures' tags are in `coordinator.rs`'s tests, which are not frozen, so the implementation can shorten
  them for margin. It re-measures the class on the guest as PR11 did.

#### 3.3.2 What the manager does with a slot

> **Round 4:** retention is withdrawn: a dead instance that cannot be removed refuses the command (§4.5). `intents()` also
> reports other incarnations' instances that no intent names (§4.3).

| Operation | Acts on |
|---|---|
| `slot_path`, `intent_path`, `add_worktree`, `write_intent`, `verify_worktree`, and every Git funnel run in a slot | the current incarnation's instance |
| `intents()` | every intent of every incarnation, reported as **logical** slots, each once, sorted. So the frozen walks see the slots they see today. |
| `remove_worktree`, `remove_intent` | **every instance of the logical slot**: the current one, every other incarnation's found through its intent, and an instance directory under the namespace whose intent has already gone. One call is still one execution of its site, so the hooks fire once, as the frozen tests that count them expect (`recover/tests.rs:19752-19758` counts `Worktree.Remove` exactly). Inside the funnel the removal acts on each instance in turn, each bound to its own registration (#329's targeted removal). |

**How a removal fails.**
- **The current instance:** as today. An error ends the command resumably.
- **A dead instance:** if it is still in use (a directory not empty during removal, a sharing violation or access
  denial on Windows), it is **retained**. Its intent stays, the walk reports it as retained residue, and every later walk
  retries.
- **Any other error, a hook's refusal included:** the command's, as today. So the fault-injection tests keep their
  refusals.

**Why a dead instance may be left behind:** nothing will ever use it again. Leaving it costs disk, while a command that
refused on it would let a dead incarnation's writer hold recovery: the dependency U exists to remove.

**The alternative, if the owner reads Q1 literally.** Q1 asks that durable recovery records be reclaimed "before any slot
reset, admission, or resource reuse", and a retained instance's intent outlives the next admission. E-FUC-3's item 2
makes retention the registry's rule for a dead instance (§3.3.7). Without it, the removal of a dead instance refuses as
today's removals do. That keeps Q1's literal order, and its cost is that a writer still using its dead instance holds the
next command until it ends: R-3, as Q has it.

**At a fresh-process recovery, through the frozen code:**
1. `verify_or_recreate` verifies the current instance. It does not exist, so the verdict is
   `VerifyFailure::NotRegistered` (`workspace_manager.rs:2768-2770`).
2. `remove_worktree` removes the dead incarnation's instance, or retains it.
3. `create_worktree` adds the current instance.
4. `Reuse::Verified` therefore arises only within the incarnation that created the slot.

**What that costs, and what it does not.**
- An open generation's worktree is checked out again at every resume.
- It holds no paid work. An `OpenNoAttempt` slot is the base checkout, and for a repair the materialization the
  continuation runs again.
- A `RetainedIdle` generation is already closed at a fresh-process recovery (`recover.rs:1425-1443`).
- Promotion recovery reads refs and the fold, not a worktree (`candidate.rs:427-472`).

#### 3.3.3 What U closes, without observing any process

**Why it closes the class.** Everything a dead incarnation's Git writers do afterwards names the slot by path or by
registration name, and no later incarnation creates either:
- the add gives its children `GIT_DIR=<path>/.git` and `GIT_WORK_TREE=<path>` (`builtin/worktree.c:526-554` at 2.43.0,
  `:558-600` at 2.55.0);
- Git changes into that work tree before it writes (`setup_work_tree`, `setup.c:427-450` at 2.43.0);
- `remove_junk` deletes the registration and the checkout by path (`builtin/worktree.c:258-272` at 2.43.0, `:273-287` at
  2.55.0);
- a prune deletes by registration name, in the loop iteration that decided it (`:142-168` and `:203-217` at 2.43.0).

**Executed** (`c3/witness/uniq/witness-uniq-v2.43.0.log`, `witness-uniq-v2.55.0.log`).
- **Coordinator A's add is held, and A does not wait for it.** That is what a dead coordinator's add sees: Git cannot
  tell whether its parent lives.
- **The successor then does what `verify_or_recreate` does.** It removes the slot's checkout, removes the registration
  directly, adds the slot again at commit C2, and writes the paid edits.
- **The hold is released.** A checks out C1, so a HEAD A writes into B's registration is visible.

| Route of the dead add | Git | Same path (today) | A path unique to the incarnation |
|---|---|---|---|
| **exec**: held in the `reference-transaction` hook of its HEAD write, before the lock on 2.55.0 and after it on 2.43.0; then the HEAD write and the `reset --hard` child, by path | 2.55.0 | A's add ends rc 0. B's HEAD is rewritten to C1, and `a.txt` reads `C1 base`: **paid edits lost** | A's add fails: `not a git repository: '…/k1-g1_01KZTA7X/.git'`. **B intact** |
| | 2.43.0 | `update_ref failed for ref 'HEAD'`, then `remove_junk`: **B's registration and checkout deleted** | the same failure, on A's paths. **B intact** |
| **filter**: held in its `reset --hard` child's smudge filter; then the reset fails and the add runs `remove_junk` | both | `unable to create file b.dat`, then **B's registration and checkout deleted** | the same failure, on A's paths. **B intact** |
| **helper**: the add completes; its filter started a helper that called `setsid` (R-1's shape) and later runs `git -C <the path it kept> reset --hard HEAD` | both | B's `a.txt` back to `C2 base`: **paid edits lost** | `fatal: cannot change to '…/k1-g1_01KZTA7X'`. **B intact** |

`SUMMARY` lines: `exec/same=B DAMAGED; exec/unique=B INTACT; filter/same=B DAMAGED; filter/unique=B INTACT;
helper/same=B DAMAGED; helper/unique=B INTACT`, on both versions. Each run ends with A and the helper gone, and nothing of
the witness left running.

**By class:**
- **The finding's sequences.** The late writes by path and the late `reset`, `remove_junk`, and the HEAD rewrite are
  closed. All are executed above.
- **Cross-run reuse of a registration name** (`k1-g1` is every run's first task): closed. Each name carries the
  creating incarnation's tag.
- **FUC-D2-PGIDREUSE** does not arise: there is no writer group. A child of the dead coordinator that execs late still
  acts on its own incarnation's paths.
- **R-1 and R-1W, a deliberate escape, for slot paths and registrations:** closed. The helper row executes R-1's shape.
- **R-W, Windows I/O in flight at termination, for slot paths:** closed. Pending I/O completes into the dead instance.
  Its removal fails while handles are open (the retry covers about one second, `workspace_manager.rs:1464-1509`). The
  dead instance is then retained (§3.3.2), so recovery does not wait for it.
- **R-G's name-reuse variant** (§3.4): closed. A prune paused after deciding deletes the dead instance's name, not the
  replacement's.

#### 3.3.4 Residue, and how disk is reclaimed

> **Round 4:** §4.3 replaces "When its writer is still running" (a Git process that has not yet run does recreate a
> removed instance, executed), "What can come back" and the walks' reclaim; §4.5 replaces "How the accounting reads".

**When a dead instance goes.** At the first existing walk that removes its logical slot; U adds no walk:
- an open generation's at step (g) (`recover.rs:1029`, through `dispatch.rs:251`);
- a closed generation's at step (e) (`recover.rs:1022`, `:1445-1467`);
- a promoted generation's in `finish_promotions` (`:1299-1318`);
- staging and snapshot instances in `reclaim_stale_residue` and `finish_integration` (`:991`, `:1096-1169`, `:1196-1219`);
- anything left over at finalization (`finalize.rs:121-139`).

**When its writer is still running.**
- **On Linux, Git's own processes cannot recreate a removed instance.** They work relative to the work tree they changed
  into at start (`setup.c:427-450`), and a removed directory accepts no new entry. The witness's filter route shows it:
  the dead reset failed with `unable to create file b.dat: No such file or directory`.
- A creation racing the removal makes the removal fail with the directory not empty, and the instance is retained.
- **On Windows** an open handle, or a process's current directory, blocks the removal, and the instance is retained
  until it closes.
- In every unique-name run of the witness, the store held only B's registration at the end, and `tasks/` only B's
  checkout: nothing of A's came back.

**What can come back.** A program that writes by absolute path can recreate the dead path after its removal: a helper,
not Git.
- The next walk that removes the slot removes it again.
- After the run's finalization nothing does. The execution root then stays: R18 is "pruned by finalization when empty;
  otherwise resumably_open", and `remove_execution_root` answers `false` (`workspace_manager.rs:2114-2175`).
- That is R-UR (§3.7): disk at a dead name, P3. The operator removes it.

**How the accounting reads.**
- R9, R10 and R24 keep their rows, at a finer granularity: per generation (or transaction, or snapshot role) **and
  creating incarnation**.
- A retained dead instance is residue of its row, reclaimed at the next walk, like the `NoRunFinished` cells' "intents
  reclaimed on resume".
- E-FUC-3 (§3.3.7) gives the ended outcomes that clause.
- No row, site, process or lock is added.

#### 3.3.5 What U leaves

> **Round 4:** §4.8 replaces the Refs bullet, and §4.6 the two oracles' replacement.

- **R-G's in-window variant** (§3.4). A prune that reads the replacement's own entry before its `locked` exists is the
  one U cannot reach, because the name it read is the new one. That needs the legacy change of §3.4.
- **The engine's own repository-wide prune.** `remove_bound` runs `git worktree prune` (`workspace_manager.rs:3061`,
  `:3100`, `:3123`; census §10).
  - If a coordinator dies inside it, that orphaned prune is the same in-window hazard for the successor's add.
  - #329's targeted removal deletes every one of those prunes (its record §3.5, kept in §5.4, at `ed3a97d9`). Both
    changes land before G6.
  - U's closure of this route depends on #329's text staying so. Under Q the orphaned prune is a group member and is
    waited for.
- **Refs** are unchanged (R-REF, §3.7). On Unix every engine `update-ref` holds the run's cleanup lease
  (`workspace_manager.rs:3553-3570`). On Windows the ref-lock reclaim rests on the ambient job's kill-on-close, as
  `design/26_design_merge_queue_protocol.md:398` says. Every engine ref write is a compare-and-swap, so a late landing
  makes the successor's next write of that ref refuse rather than overwrite.
- **Two frozen oracles go vacuous.** `finalize.rs:421-422` and `:456` assert that no registration path ends with
  `kalpha-g1`, `s1-integration` or `kbravo-g1`. Under U no path ends with an untagged name, so both pass whatever is
  registered.
  - A non-frozen test that asserts the tagged names restores the coverage.
  - Restoring them in place is a byte change to a frozen file: D4 (§3.8).
- **Frozen tests that must change: none found by reading** (census §4–§8). Their reasoning:
  - The in-process recovery tests plant and resume through one manager, `Fixture::manager` with the creator's incarnation
    (`recover/tests.rs:139-147`, `:7057-7101`). So their tags agree, and the one cross-incarnation expectation of
    `Reuse::Verified` (`:12793-12809`) is in process.
  - Five child coordinators derive their managers with `RESUMER` (`:4400-4408` and the four sites census §6 lists). The
    parent's resume then meets the child's instances as dead ones and removes them through the logical removal of
    §3.3.2. Their assertions are about convergence, the log, refs and the execution root, not slot names.
  - Reading can miss. The implementation's whole-suite run is the oracle, and a frozen test that must change makes U need
    D4.

#### 3.3.6 What U costs, and where it lives

> **Round 4:** §4.2 adds the id constructor in `src/ulid.rs` and its use in `RealIds::incarnation`, and §4.3 the
> discovery; §4.2 and §4.6 add the tests.

- **Code**, all non-frozen:
  - `src/workspace_manager/naming.rs`: the instance name, its parse, and `SlotId` with a tag.
  - `src/workspace_manager.rs`: the tag, the renderings, logical `intents()`, per-instance removal and retention, and the
    torn-registration repair per instance.
  - `src/engine/topology/dispatch.rs` does not change.
  - **Siblings.** #329 edits the same file's add, scans and `remove_bound`. The hunks are adjacent in
    `remove_worktree_proving`, and whichever merges second rebases.
- **Kept from §2:** §2.2's Git settings on every manager Git child. They stop the engine's own Git from starting
  maintenance, which could otherwise prune a successor's entry from a detached process. Round 2's lenses accepted them.
- **Not needed:**
  - the writer group, its record and wait (§2.4);
  - the probe and the keeper (§2.5.2–§2.5.4);
  - R29 and R30 (§2.7), and errata E-FUC-1 and E-FUC-2;
  - the legacy commands' wait (§2.8).
- **Instruments.** U adds no effect site, resource row, process start or governed primitive. The namespace listing uses
  `fs::read_dir`, which the manager, an allowlisted funnel module, already uses. The implementation's census confirms
  this; an instrument it finds moved is a first-limb change, whose merge the owner's standing direction for PR11 leaves
  to the orchestrator, and the erratum is the owner's to adopt in any case (corrected at repair round 2, §6.9).
- **`DESIGN.md` §15**, whose "Synced intents" contract the intent names belong to, and the manager's and naming's
  internals notes.
- **Tests**, added in non-frozen files:
  - the witness's three routes against the production funnels, at today's naming (red) and U's (green);
  - a two-incarnation reclaim: a dead instance removed, and one retained while a test-held process keeps it in use;
  - an untagged instance from before U, reclaimed;
  - the intent parser's grammar with tags;
  - the two tagged-name assertions that replace the vacuous oracles;
  - on Windows, a dead instance held open by a test-held handle, retained while recovery proceeds;
  - the guest path budget re-measured.

#### 3.3.7 Erratum E-FUC-3, revised: the text for the owner

> **Round 4:** superseded by §4.14, E-FUC-3 in full.

In the errata file's form: anchor, current text, amendment. It replaces §2.5.6's E-FUC-3.
1. **`decisions.workspace_candidates.manager`.**
   - Current, in part: "detached linked worktrees with durable synced intents (tasks/k<key>-g<gen>, merge/s<seq>)".
   - Amended: "detached linked worktrees with durable synced intents (tasks/k<key>-g<gen>_<tag>, merge/s<seq>_<tag>,
     snapshots/<name>_<tag>, where <tag> renders the creating coordinator incarnation's id; a worktree is used only by
     the incarnation that created it, and every other incarnation's instance of the slot is residue)".
2. **`decisions.workspace_candidates.cleanup`.** Append: "every reclaim of a slot removes every incarnation's instance of
   it, through that instance's intent or, once the intent has gone, its directory; an earlier incarnation's instance that
   is still in use is retained, keeps its intent, and is reclaimed by a later reclaim".
3. **`resource_accounting.rows[R9]`.**
   - `granularity`: "per generation" becomes "per generation and creating incarnation".
   - `lifecycle.OpenNoAttempt`, current: "resumably_open during a live run (reused only after Worktree.Verify; otherwise
     recreated with force); closed at run end".
   - Amended: "resumably_open during a live run (reused by the incarnation that created it only after Worktree.Verify,
     otherwise recreated with force; a fresh-process recovery creates the generation's worktree under its own
     incarnation, and earlier incarnations' instances are reclaimed as residue); closed at run end".
   - `lifecycle.RetainedIdle`: "retried only after Worktree.Verify" becomes "retried by the incarnation that created it
     only after Worktree.Verify".
4. **`resource_accounting.rows[R10]` and `[R24]`.** Each `granularity` gains "and creating incarnation". Each
   `at_run_end` cell that says "pruned" gains "(an earlier incarnation's instance still in use is retained as residue and
   reclaimed later)".
5. **`transaction_fault_matrix[T-DISPATCH]`.**
   - `durable_state`: "worktree path" becomes "worktree path (the creating incarnation's instance)".
   - `resume_action`, current, in part: "verify the worktree at the recorded base with Worktree.Verify (linked worktree at
     the recorded path, …) or remove it with force and recreate it (intent then add)".
   - Amended: "in the incarnation that created it, verify the worktree at the recorded base with Worktree.Verify (linked
     worktree at its path, …) or remove it with force and recreate it (intent then add); in a fresh process, reclaim
     every earlier incarnation's instance and create the worktree under the current incarnation (intent then add)".
6. **`transaction_fault_matrix[T-REPAIR-DISPATCH]`.** `resume_action`: "Worktree.Verify the recorded worktree" becomes
   "in the incarnation that created it, Worktree.Verify the recorded worktree … ; in a fresh process, recreate it under
   the current incarnation".

The packet's test names in those rows still describe the behavior: `kill_after_dispatch_recreates_worktree_without_spend`
recreates, and so does every resume now.

### 3.4 R-G: the legacy engine's maintenance, and the change follow-up D's unfreeze must carry

> **Round 4:** the attribution and the legacy starter are corrected in place below (§4.10). The last paragraph, on
> processes the engine did not start, is replaced by §4.11.

**§2.2's prerequisite was false** (FUC-D2-RG: every lens reasoned it from Git's source; none executed R-G). §2.2 said a legacy maintenance prune deletes a
registration "only if its `gitdir` file is older than" `gc.worktreePruneExpire`. Git's eligibility check says otherwise
(`should_prune_worktree`, `worktree.c:719-785` at 2.43.0, `:929-1018` at 2.55.0; `c3/git-src/git-src-citations.txt`):
- a `locked` file keeps the entry;
- **a missing `gitdir` prunes it, with no expiry at all** (`:734-737` at 2.43.0);
- an unreadable, short or empty `gitdir` prunes it too;
- when the checkout `gitdir` names is gone, a **missing** `index` prunes it, and otherwise the **index's** age against
  the expiry decides (`:772-777`).

So the default `3.months.ago` protects nothing in the interval that matters.

**The interval.** `git worktree add` makes the registration's directory (`builtin/worktree.c:458-465` at 2.43.0) before
it writes `locked` (`:479-483`) and `gitdir` (`:492-494`). A prune decides each entry and deletes it in the same loop
iteration (`:203-217`). Deletion is by name (`delete_git_dir`, `:142-155`), with no second look. That allows two
variants:
- **R-G1, name reuse** (the design lens's sequence). A prune decides on an old entry, for instance a torn one, and is
  paused. Recovery removes that entry and recreates the slot under the same name. The prune resumes and deletes the
  replacement. **U closes it:** the replacement's name is new.
- **R-G2, in the window** (the concurrency and regression lenses' sequence). A prune reads the new entry between its
  `mkdir` and its `locked`, decides "gitdir file does not exist", and deletes it.
  - If it deletes before `locked`, the add fails: "could not open … locked for writing", executed by #329's round 3
    (`d3/witness/prune-in-flight/witness.log:4`). #329's tolerant access then attempts the add again, because its
    destination is untouched (#329 record §5.3–§5.5 at `ed3a97d9`).
  - If the prune is descheduled between its decision and its deletion until after the add has finished, a completed
    registration is deleted under a slot in use.
  - **U does not close R-G2, and neither does Q.**

**How a legacy command starts that prune** (corrected in round 4, from follow-up D's round 2: #331 at `b13b4857`, its
record §2.5).
- **Not through `git commit`.** Round 3 named `Workspace::commit` (`src/workspace.rs:1019-1023`), but it has no
  production caller: its callers are all in the module's tests. The legacy coordinator publishes through `commit-tree`
  and `update-ref`.
- Of the builtins that call `run_auto_maintenance` (`am`, `commit`, `fetch`, `merge`, `rebase`), the legacy engine runs
  none in production.
- **So its one path to automatic maintenance is a lazy fetch.** In a partial clone, a legacy child that must read an
  object the clone lacks starts a promisor `fetch`, and that fetch runs automatic maintenance. D's round 2 executed it
  through the real legacy engine (its §2.5, witness rg2).
- At 2.43.0 and 2.50.1, maintenance runs the gc task, whose `gc --auto` prunes worktrees only once its thresholds are met
  (`builtin/gc.c:64`, `:612`, `:734-742` at 2.43.0).
- **At 2.55.0 the `geometric` strategy is the default** for unscheduled maintenance, not a configuration, and it
  includes the `worktree-prune` task, which runs whenever one registration is prunable (`builtin/gc.c:379-428`,
  `:1845-1925`; D's §2.5).
- None of it is in a writer group or a keeper's job, and the legacy engine is PR5-frozen.

**Grading.** P1, the triage's consolidated grade (corrected in round 4, §4.10). The design and concurrency lenses gave
reasoned P1 sequences, and the regression lens a narrower reasoned P2, its fresh-add case matching the host-agent prune
class; none executed R-G. D's round 2 proposes P2 on its evidence; the triage's P1 stands until the owner reclassifies.
- G6: Q1, a topology registration deleted under a slot in use.
- **It blocks G6 until closed or explicitly excluded by the owner.** It is not this change's to close: both of its
  starters are outside the topology engine.

**The requirement for follow-up D's unfreeze (decision B), stated for the orchestrator to carry.**
- `git_command` (`src/workspace.rs:43-51`) is the one builder of every legacy Git child, as the frozen legacy test pins
  (`:3745-3751`). Its callers are `:163`, `:188`, `:231`, `:289`, `:380`, `:419`, `:460`, `:879`, `:911`, `:1039`,
  `:1096`, `:1131`, `:1557` and `:1607`.
- **It adds:** `-c maintenance.auto=false -c gc.auto=0 -c gc.autoDetach=false -c maintenance.autoDetach=false`. These are
  the four `-c` values §2.2 gives the topology builder.
- They reach every Git process the legacy child starts, through `GIT_CONFIG_PARAMETERS` (§2.2's version table).
- Under them, no Git process the legacy child starts runs automatic maintenance at 2.43, 2.50.1, the Windows tag or
  2.55. Round 2 executed that for the manager's commands at 2.43 and 2.55 (`c2/witness/gc/`), and follow-up D's round 2
  for the legacy engine's lazy fetch at 2.43.0, 2.50.1 and 2.55.0 (#331 at `b13b4857`, record §2.5; corrected in round
  4).
- The pin counts `Command::new(` and checks the replacement-object environment, so adding arguments leaves it passing.
  Any legacy test that pins the exact argument list is D's to measure.
- With that change, R-G1 and R-G2 are closed for maintenance a legacy command starts. U closes R-G1 independently;
  Q closes neither.

**What neither change reaches.** Maintenance, or a prune, started by a process the engine did not start: the user's own
`git commit` or `git fetch` in any checkout of the repository, or an IDE's. §1.13 puts it out of this finding's scope.
- An agent's own prune is `PR11-HOST-AGENT-PRUNE-RACES-AN-ENGINE-ADD` (P2, guarded by PR12), narrowed by #329 to the
  agent's own commands.
- The user's is filed nowhere. The orchestrator may want it named in decision B's question, since R-G2 is the same
  mechanism whoever starts the prune.

### 3.5 The quiescence path Q, repaired where no new layer is needed

> **Round 4:** Q is frozen. §4.9 restates its open items: PGIDREUSE (a viable ordering exists, at the `pre_exec` cost),
> and the sentinel's fallback and observation identity, which this subsection called fixed.

Q is §2's design: the Unix writer group and, for Windows, the keeper (D2b′). This subsection answers each of round 2's
findings against it. Under U none of it is built.

**FUC-D2-PGIDREUSE: Q cannot close it without a new layer.**
- **The lens's sequence.** A Git child of the dead coordinator is created but has not yet joined `P`. The coordinator
  dies, the sentinel leaves, the successor reads `ESRCH`, and `P` is later reused for a group in the same session. The
  late child's join then succeeds, and it runs against the reused slot.
- **What was weighed:**
  - *The coordinator itself in `P`, so that children are born in it.* That leaves the terminal's foreground group, so
    `^C` no longer reaches the coordinator and a terminal read stops it. A process group is per process, not per
    thread, so every pipeline's children, agents included, would join `P`.
  - *A spawner process inside `P` that starts every Git child.* That is a new process layer.
  - *A `pre_exec` closure that checks its parent after joining and exits if the parent is gone.* That takes `std` off
    `posix_spawn`, and #329's round 3 measured the suite at about three times slower (`d3/census/probe-iii/SUMMARY-D1.txt`,
    which §1.4 cites).
  - *A sentinel that stays until no half-created child remains.* It cannot see one without `/proc`, which the finding's
    second requirement excludes.
- **It remains under Q:** P1 as triaged, reasoned and not executed, and it blocks G6 under Q. Under U it does not arise
  (§3.3.3).

**FUC-D2-XMACHINE: fixed.** §2.4.3's two advisory refusals, and D2b′'s refusal in §2.5.4, now name the only sufficient
conditions:
- **Another PID namespace:** remove the record "once no process of group `P` remains in the recorded namespace, which
  `kill -0 -- -P` run there answers with no such process, or once that namespace's machine has restarted".
- **Another boot, or another platform on a shared filesystem:** remove the record "once the machine that wrote it has
  restarted since the recorded boot, or once `kill -0 -- -P` on that machine answers no such process".
- **Windows, the keeper gone with no marker:** remove the record "once the machine has restarted, or once no process the
  coordinator started after its first engine Git child remains".
- None of them says "once the coordinator is gone": the coordinator's death is the condition that does not establish
  quiescence.

**The P2s, each fixed by changing §2's protocol rather than adding to it:**

| Id | Round 2's defect | The repair |
|---|---|---|
| FUC-D2-SENTINELKILL | Release's `SIGKILL` can leave the sentinel a zombie in `P` if the coordinator dies before reaping it. | Release sends `SIGTERM`, then `SIGCONT`. The sentinel's `SIGTERM` handler calls `setsid()` and then `_exit(0)`, both async-signal-safe. `SIGKILL` follows only after the one-second bound. A sentinel stopped by `SIGSTOP` is continued, and leaves `P` before it dies. The crash table gains the row "inside release, after the signal and before the reap": `P` holds no sentinel; the next command waits for `ESRCH`. |
| FUC-D2-PUBORDER | The record could be published before the sentinel had installed its dispositions. | The sentinel writes one byte on a pipe once its dispositions are installed and its other descriptors closed, then closes the pipe. The coordinator publishes the record only after reading that byte, bounded at one second. Otherwise it kills and reaps the sentinel and the leader, both bounded, publishes nothing, and fails the Git command resumably. |
| FUC-D2-WINOPEN | The keeper's acknowledgement had no channel and no bound. Unix's reap of `X` had no bound either. | The keeper acknowledges on its standard output, a pipe to the coordinator, within one second. End of file is the keeper's death. On either failure the coordinator closes `N`, publishes nothing, and fails the Git command resumably. Unix reaps `X` with the same one-second bound. |
| FUC-D2-KEEPERREF | The keeper's own handle to the coordinator kept `ActiveProcesses` from reaching 0. | The keeper closes its coordinator handle as soon as the handle is signaled, before its drain loop. The count falls only once all references are released (`c2/docs/ms-jobobject-basic-accounting-information.txt:89`). |
| FUC-D2-ACCOUNTSTATE | The joint ledger rule rejected "R29 present, R30 released", the drained record release keeps for the next command. | The ended-outcome clauses admit three states: both released; both held (a surviving member); and R29 retained for reclamation with R30 released (a drained record). `ledger.rs`'s `equation` and `check` take the same three. |
| FUC-D2-FROZENTESTS | New fields in `PhysicalInventory` and `ProcessLocal` would break the frozen recovery tests' struct literals (`recover/tests.rs:24323`, `:24472`, `:24482`), and default "absent" values would leave R29 and R30 unproved there. | Neither struct gains a field. `ledger::observe` reads R29 and R30 itself, from the lease side's process-local writer table (§1.4): the record path and group of the checkout this process last held. So the frozen tests' unchanged calls observe both rows for real, and a fixture that never held a lease reads both absent, which is then true. New non-frozen tests (T5, T6, T10, TW1, TW4) assert the held and drained states. The preservation proof states the split. |
| FUC-D2-T6 | T6 expected the record kept after a stopped group, but the orphaned-group hang-up can empty `P` (both lenses executed it). | T6's oracle follows the observed group: `ESRCH` at release means unlinked, members mean kept. A second case uses a helper that ignores `SIGHUP` and must keep the record. |

**What stays residual under Q, said plainly:**
- PGIDREUSE (P1, blocks G6);
- R-1 and R-1W (P1 as the finding is written, P3 under D3(a));
- R-G (P1, follow-up D's);
- under D2a or D2d, R-W (P1, blocks G6);
- the liveness residuals R-Z, R-L, R-T, R-2, R-3, R-K and R-NS (§2.6).

### 3.6 Round 2's findings, and where each is answered

> **Round 4:** FUC-D2-RG's lens column is corrected in place; round 3's findings are mapped in §4.12.

| Id | Sev | Lens | Round 2's defect | Round 3 |
|---|---|---|---|---|
| FUC-D2-RG | P1 | design (P1), concurrency (P1), regression (P2), none executed (corrected in round 4) | R-G was graded P3 on a false prerequisite: no `gitdir` prunes with no expiry. | §3.4: the prerequisite corrected, P1, two variants. U closes R-G1. Both close with the legacy change stated for follow-up D's unfreeze. |
| FUC-D2-PGIDREUSE | P1 | concurrency | A late child joins a reused group after the successor read `ESRCH`. | §3.5: Q cannot close it without a new layer, which is said plainly. U does not have it (§3.3.3). |
| FUC-D2-XMACHINE | P1 | design | The refusals advised removing the record once the coordinator was gone. | §3.5: each refusal names a sufficient condition. |
| FUC-D2-D2CEPOCH | P1 | design | D2c's epoch, counted from durable starts, repeats when recovery dies before `RunResumed`. | §3.3.1: U's tag comes from the per-process incarnation id, drawn before any lock, and counts nothing durable. |
| FUC-D2-SENTINELKILL | P2 | design | Release's `SIGKILL` reintroduced the unreaped sentinel. | §3.5. |
| FUC-D2-ACCOUNTSTATE | P2 | design, regression | The joint rule rejected a drained record awaiting reclamation. | §3.5. |
| FUC-D2-FROZENTESTS | P2 | regression | New ledger fields needed changes in a frozen test child. | §3.5: no field; the rows are observed through the lease side's table. |
| FUC-D2-PUBORDER | P2 | concurrency | The record was published before the sentinel's dispositions. | §3.5. |
| FUC-D2-WINOPEN | P2 | concurrency | The keeper's opening had no channel and no bound. | §3.5. |
| FUC-D2-KEEPERREF | P2 | concurrency | The keeper's own handle kept the job's count up. | §3.5. |
| FUC-D2-T6 | P2 | regression (P2), concurrency (P3) | T6 expected the record kept when the hang-up had emptied the group. | §3.5. |

### 3.7 The residuals, in one table

> **Round 4:** superseded by §4.13. The R-G rows are corrected in place.

"U" is §3.3 with #329's targeted removal, "Q" is §3.5's repaired group with D2b′, and "today" is master. G6's pass rule
fails on an open critical or high finding. Applicability names the three items the orchestrator asked about: Q1's "reclaimed or
repaired … before any slot reset, admission, or resource reuse", ST-18's crash-and-resume class (with ST-16), and
INV-22's resource accounting.

| Residual | What | Severity and evidence | G6: Q1 / ST-18 / INV-22 | Blocks G6? | Closed by |
|---|---|---|---|---|---|
| **DESC**, the filed finding | A dead coordinator's Git writers act on a slot its successor recreated or kept. | **P1.** Executed at base by #329. Executed again here, today's naming: three routes, both Gits, edits lost or registration deleted (§3.3.3). | Q1 yes / ST-18 yes / INV-22 yes | **Yes**, until a closure is implemented, reviewed and accepted | **U** (executed, without observing a process), with #329; or **Q** with D2b′ |
| **R-1** | Unix: a configured program detaches deliberately and later writes a slot path or registration it kept. | P1 as the finding is written; P3 under D3(a). Executed: `c1/witness/pg/witness-pg-setsid.log`, and this round's helper route. | Q1 yes / ST-18 yes / INV-22 yes, for its cleanup claims | Under **U: no**, closed for slot paths and registrations (helper, unique: B intact). Under **Q: yes**, unless D3(a). | U; or D3(a)'s boundary under Q |
| **R-1W** | Windows: the same, through a process started outside the job. | As R-1. Reasoned. | Q1 yes / ST-18 yes / INV-22 yes | Under U: no. Under Q: yes unless D3(a). | U; or D3(a) |
| **R-W** | Windows: I/O pending at termination completes after the successor moved on. | **P1** (§2.5.1). Reasoned from Microsoft's documentation, not executable without a driver. | Q1 yes / ST-18 yes / INV-22 yes | Under **U: no**: it lands in a dead instance, which is retained while open. Under **Q-D2b′: no** once implemented. Under **D2a or D2d: yes**. | U; or D2b′ |
| **R-G1** | Legacy-started maintenance (a lazy fetch in a partial clone; corrected in round 4): a paused prune deletes a recreated registration that reused the old name. | **P1**, the triage's grade (design and concurrency P1, regression P2; corrected in round 4). Reasoned from `worktree.c` and `builtin/worktree.c` (§3.4). | Q1 yes / ST-18 and INV-22 for the registration's cleanup and accounting | **Yes**, until U or the legacy change | **U**, for topology names; or follow-up D's legacy change (§3.4) |
| **R-G2** | Legacy-started maintenance (a lazy fetch in a partial clone; corrected in round 4): a prune reads the new entry before `locked`, and deletes it after the add completed. | **P1** (triage). Reasoned. Needs the prune descheduled between its decision and its deletion. | Q1 yes / ST-18 and INV-22 as R-G1 | **Yes**, until the legacy change, or the owner excludes it | **Follow-up D's legacy change only** (§3.4). Neither U nor Q. |
| **PGIDREUSE** | Unix, Q only: a late child joins a reused group after the successor read `ESRCH`. | **P1** (triage). Reasoned, not executed. | Q1 yes / ST-18 yes / INV-22 yes | Under **Q: yes**. Under **U: does not arise**. | U only (§3.5) |
| **R-P** | The dead coordinator's own `git worktree prune` (`workspace_manager.rs:3061`, `:3100`, `:3123`) reads the successor's new entry before `locked`. | R-G2's mechanism, with the engine as the starter; P1 by the same grading. Reasoned. | Q1 yes / ST-18 yes / INV-22 yes | **Yes**, until #329 lands, which G6 requires for #329's own finding | **#329's targeted removal** (no engine prune at all). Under Q, also the group's wait. |
| **R-REF** | Windows: a terminated `update-ref`'s ref write lands after the successor reclaimed its lock. | P3, liveness. Reasoned: every engine ref write is a compare-and-swap (`--no-deref <ref> <new> <old>`), so a late landing makes the successor's next write refuse. The basis is `design/26_design_merge_queue_protocol.md:398`'s kill-on-close clause, as master states it. | Q1 yes / ST-18 no / INV-22 no | No | Q-D2b′ would order it after completion. U leaves it as today. |
| **R-UR** | U only: a dead instance retained while still in use, or recreated by a program writing by absolute path after its removal. | P3: disk and liveness, never a shared slot. The witness shows Git's own late writes cannot recreate a removed instance (§3.3.4). | Q1 yes: its intent can outlive the next admission, which E-FUC-3's item 2 makes the registry's rule for a dead instance (no dead instance is ever reused). INV-22 yes, admitted by items 3 and 4. | No, with E-FUC-3. Without item 2, Q1's literal order needs §3.3.2's refusing variant. | The next walk removes it; after finalization, the operator |
| **R-Z, R-L, R-T, R-2, R-3, R-K, R-NS** | Q only: liveness refusals (§2.6). | P3, or stated. | Q1's refusal path only | No | Under U none of them exists. A hung writer holds only its own dead instance. |
| **XMACHINE** | Q only: the cross-machine refusal's advice. | Was P1. | — | No | Fixed (§3.5) |
| **Two vacuous frozen oracles** | U only: `finalize.rs:421-422` and `:456` match untagged names. | P3, test coverage. Read, not executed. | INV-22's finalization evidence | No | New non-frozen assertions; in place only under D4 |
| **R-GU** | Maintenance or a prune started by a process the engine did not start: the user's own Git, an IDE. | Outside this finding (§1.13). R-G2's mechanism. An agent's own prune is `PR11-HOST-AGENT-PRUNE-RACES-AN-ENGINE-ADD` (P2, guarded by PR12). | — | Not this change's | Not this change's; for the orchestrator, beside decision B |

**What G6 then meets, under the recommended choices.**
- With U, #329 and follow-up D's legacy change implemented and accepted, nothing in this table is an open high.
- Under Q instead, PGIDREUSE stays open and blocks G6, and so do R-1 and R-1W unless D3(a).
- Without the legacy change, R-G2 stays open and blocks G6 under either, unless the owner excludes it.

### 3.8 The owner's decisions

> **Round 4:** superseded by §4.15.

The orchestrator puts one consolidated question to the owner after this round's review. These are its parts.

| | Decision | Options | Recommendation | What each leaves open |
|---|---|---|---|---|
| **D2** | **How the finding closes.** It is the central choice, and D1 and D3 follow from it. | **U**: a slot instance per incarnation (§3.3), with erratum E-FUC-3 revised (§3.3.7); a dead instance still in use is either retained (E-FUC-3's item 2) or refused on, as §3.3.2 sets out. **Q-D2b′**: the repaired writer group and keeper (§3.5), with E-FUC-1 and E-FUC-2. **Q-D2a**: the group with the coordinator wait on Windows. **D2d**: no Windows change. | **U**, retaining | **U:** R-G2 (follow-up D's), R-REF (as today), R-UR (P3), two vacuous oracles (replaced). It needs #329's targeted removal. **Q-D2b′:** PGIDREUSE (P1, blocks G6), R-1 and R-1W (D3), R-G1 and R-G2, and seven liveness residuals. **Q-D2a:** the same plus R-W (P1, blocks G6). **D2d:** the finding's Windows half (P1, blocks G6). |
| **D1** | **Accounting.** | Under U: E-FUC-3's items 3 and 4. No new row; R9, R10 and R24 are counted per creating incarnation. Under Q: D1a, E-FUC-1 with R29 and R30 as §3.5 amends them; or D1b. | **Follows D2:** E-FUC-3 under U, D1a under Q. | Declining under U: the code would reclaim instances the packet's rows do not name. Declining under Q: the group and record go unaccounted, as §2.12 said. |
| **D3** | **R-1 and R-1W's scope** | (a) extend `DESIGN.md` §15's and INV-18's boundary for deliberately daemonizing code to the programs the engine's Git commands run, and file R-1 at P3; (b) close it with U; (c) keep it at P1. | **Moot under U**, which closes it for slot paths and registrations, executed. Under Q it is the owner's scope call; round 2's evidence (§2.6) stands. | Under Q with (c): R-1 blocks G6, and no in-lane remedy exists but U. |
| **D4** | **A frozen file** (new, conditional). It is a ruling on G6's "fold, queue, merge, repair, and recovery modules byte-identical to the G5 range", and PR11's two-tier reading of it (R-D, `reviews/2026-09-30-pr11-record.md`). | Neither U nor Q, as specified here, changes a frozen file. D4 arises only if (i) the owner wants `finalize.rs:421-422` and `:456` restored in place under U; or (ii) the implementation's whole-suite run finds a frozen test that must change: under U, a multi-incarnation recovery test; under Q, one that the observation route of §3.5 does not cover. The options are then (a) permit that named test-only change, proved with PR11's two tiers; or (b) refuse, and the implementation finds another form or the option is dropped. | **Not needed**: replace the two oracles with non-frozen assertions. | If (ii) occurs and (b) is chosen, the chosen closure needs a form that leaves the test as it is. |

**For decision B, which the orchestrator carries:** follow-up D's unfreeze includes §3.4's change to `git_command`
(`src/workspace.rs:43-51`): `-c maintenance.auto=false -c gc.auto=0 -c gc.autoDetach=false -c maintenance.autoDetach=false`
on every legacy Git child. It closes R-G1 and R-G2 for legacy-started maintenance, and R-G2 has no other closure.

### 3.9 What round 3's review should test hardest

> **Round 4:** §4.16.

- §3.2's trace. Does any frozen path read a recorded worktree path, or render a slot into a path or name by itself?
- §3.3.2's per-instance removal and retention, against every frozen walk of `intents()`, with `intents()` returning
  logical slots.
- §3.3.5's claim that no frozen test must change, and the two oracles it says go vacuous.
- The tag's uniqueness argument, and the Windows budget it spends.
- E-FUC-3's text, item by item, against `c3/packet/packet-extract.txt`.
- §3.4's two R-G variants, and whether the legacy change closes both.
- §3.5's argument that Q cannot close PGIDREUSE without a new layer.

### 3.10 What §3 replaces

| Earlier text | Replaced by |
|---|---|
| §0's status line | §0, updated |
| §2.2: the legacy maintenance prune deletes "only if its `gitdir` file is older than" the expiry | §3.4 |
| §2.4.1: release kills the sentinel; the record is published after `setpgid` alone | §3.5, FUC-D2-SENTINELKILL and FUC-D2-PUBORDER (Q only) |
| §2.4.3: "remove it once that machine's or virtual machine's coordinator is known to be gone", and the namespace refusal's advice | §3.5, FUC-D2-XMACHINE |
| §2.4.4 and §2.4.6: the release and its crash rows | §3.5 |
| §2.5.4: the opening handshake and the keeper's loop | §3.5, FUC-D2-WINOPEN and FUC-D2-KEEPERREF |
| §2.5.6: D2c, its epoch and E-FUC-3 | §3.3 and §3.3.7 |
| §2.6: R-G at P3; R-1, R-1W and R-W's G6 rows | §3.4 and §3.7 |
| §2.7.2: the joint rule, and the observations in `PhysicalInventory` and `ProcessLocal` | §3.5, FUC-D2-ACCOUNTSTATE and FUC-D2-FROZENTESTS |
| §2.9: the placement | §3.3.6 under U; unchanged under Q |
| §2.10: T6's oracle | §3.5, FUC-D2-T6 |
| §2.11 and §2.12 | §3.7 and §3.8 |

## 4. Round 4: U repaired, retention withdrawn, Q frozen

**What this section is.** Design review round 3 ran three `gpt-6-astra` lenses at `max` on `a0464f43`, and all three
returned CHANGES_REQUIRED (`~/orch-pr11/reviews/review-330-d3-{design,concurrency,regression}-a0464f43.review.md`, hashed
with the lens prompts in `~/orch-pr11/reviews/SHA256SUMS-330-d3`).
- **What every lens accepts:** U's isolation property, executed (§3.3.3); §3.2's trace; decision B's builder coverage;
  that no amendment is adopted; and six of round 2's repairs (XMACHINE, PUBORDER, WINOPEN, KEEPERREF, T6, ACCOUNTSTATE).
- **What they found:** one P1, in U's tag; P2s in its reclamation, its accounting and Q1's order, the frozen oracles'
  replacements, the Windows budget, R-REF's grade and three of Q's repairs; and a P3 in R-G's attribution.
- **The work list** is the orchestrator's triage (`~/orch-pr11/reviews/review-330-d3-triage.md`, items 1 to 10). Three
  addenda add to it (`~/orch-pr11/answers/pr11_fuc_design4-0.md`, `-1.md` and `-2.md`): a re-run of the late add with
  saved evidence, #329's probe, where R-GU is filed, and R-G's legacy starter.

Where this section disagrees with §3, §2 or §1, it governs. §4.17 lists what it replaces, and §3's headings carry
pointers.

**Who wrote it, and the evidence.** `pr11_fuc_design4` (`claude-opus-5-5`, `max`), a fresh session on this branch at
`a0464f43`. Its figures are under `~/orch-pr11/logs/pr11_fuc_design4/`, cited as `c4/…` and hashed in
`c4/SHA256SUMS-round4`. Master is still `5c222ff2`. The branch is not rebased, and §4 cites our code at `5c222ff2`, as §3
does.
- **Our code.** `c4/code/cite4.py` prints every line §4 cites into `c4/code/code-citations-r4-5c222ff2.txt`, with each
  file's blob at `5c222ff2` and at the base `92c4ca81`. Every one is byte-identical at both, except
  `src/engine/topology/coordinator.rs` (follow-up A's), which §4 cites at `5c222ff2`.
- **The standard library at Rust `1.85.0`, the crate's MSRV.** The files behind `RandomState`, fetched read-only into
  `c4/std-src/1.85.0/`, with hashes and the cited lines in `c4/std-src/PROVENANCE-and-citations.txt`.
- **Tokio 1.53.1's use of it**, from the registry source `Cargo.lock` pins
  (`c4/deps/host-randomness-in-the-code-base.txt`).
- **Git source.** `read-cache.c`, `path.c` and `split-index.c` at v2.43.0 and v2.55.0, and Git for Windows
  v2.50.1.windows.1's `compat/mingw.c`, `refs/files-backend.c`, `lockfile.c` and `tempfile.c`, fetched read-only into
  `c4/git-src/`. Round 3's copies of `builtin/worktree.c` (`c3/git-src/`). The cited lines are in
  `c4/git-src/git-src-citations-r4.txt`.
- **The packet's anchors** that E-FUC-3 touches, Q1 to Q6 and G6, extracted read-only by `c4/packet/extract.py` into
  `c4/packet/packet-extract-r4.txt`, with the v17 packet's sha256 (`a34417d6…`).
- **One new witness, within the evidence plan** (`c4/witness/lateadd/`). Only `git` commands ran, in temporary
  directories, on Git 2.43.0 and on 2.55.0 (#329 round 6's build). Each late writer is held before it runs or in a smudge
  filter, waiting on a release file. No signal is sent, and nothing is traced, preloaded or injected.
- **Arithmetic, saved:** the tag's collision bound (`c4/tag/collision-bound.txt`, from `bound.py`) and its Windows
  budget (`c4/pathbudget/tag-length-r4.txt`, with the frozen fixtures' tag census `frozen-fixture-tags.txt`).

### 4.1 The looping signal, the third time, and why this repair is local

**The signal.** Round 3's pass found a P1 in machinery round 3 added: U's tag (FUC-D3-TAGUNIQUE). That is
`MAINTAINING.md`'s second signal, "a pass finds a P1 in machinery an earlier round of this pull request added". It is
the third time #330 has raised it.

**Which of its two cases this is.** `MAINTAINING.md` sets "an inverted condition with a regression test" apart from "a
third round of machinery invented to keep the second round's machinery safe". This is the first.
- **What the defect in the repair was.** Round 3's tag was a hash of the incarnation id, and it claimed the tag was
  unique because the id was. The production id hashes only the clock, the pid and a per-process counter
  (`src/ulid.rs:17-51`; `RealIds::incarnation`, `src/engine/topology/seams.rs:206-208`). Nothing in it names the machine
  or the PID namespace, and nothing is random. Round 3's 40-bit truncation added collisions between distinct ids.
- **What the fix is.** The fix changes the id's source and adds no layer (§4.2).
  - The production incarnation id's 80-bit field also hashes 128 bits the process draws from the host. The draw comes
    through the standard library's OS-seeded `RandomState`, which the coordinator already uses through Tokio.
  - The tag stays a function of the id alone, now 12 characters (60 bits).
  - No process, record, lock, row, site or crate is added.
- **What test holds it.** A test that two ids built from equal clock, pid and counter differ when their host draws
  differ, and the wrapper's observation test that the production id is that construction. A mutation that zeroes the
  draw must turn both red (§4.2).
- **What else this round changes:** the reclamation, which the reviewers' executed late add showed incomplete (§4.3); the
  accounting, by withdrawing retention (§4.5); and statements. None of it changes the isolation property every lens
  accepted.

**What happens next, as the orchestrator has set it** (triage, "Process control"): if this round's review finds another
P1 in U's own machinery, the design stops iterating. The owner then gets D2 as it stands, U against Q, with the residual
table (§4.13).

### 4.2 The tag's source (FUC-D3-TAGUNIQUE, P1)

> **Round 5:** two statements here are corrected in place (§5.6): the tag's uniqueness is probabilistic, and the
> three frozen cross-process tests are named.

**What round 3 got wrong.** "Unique if the incarnation id is" was false twice over.
- **Equal ids.** The production id is `crate::ulid::ulid()` (`seams.rs:206-208`): SHA-256 over a domain string, the
  clock's milliseconds, the pid and a per-process counter (`ulid.rs:17-51`). Two processes in different PID namespaces,
  or on machines that share a checkout, can draw equal values for all three, and both draw before the lease
  (`prelock.rs:114-116`). B then renders A's instance names, and A's orphan reaches B's slot. That is the design and
  concurrency lenses' sequence; it needs neither a clock step nor PID reuse.
- **Distinct ids.** Round 3 kept 40 bits of the hash. Over a million incarnations sharing a repository, the union bound
  on any collision is 0.45 (`c4/tag/collision-bound.txt`).

**The repair: the incarnation id carries host randomness, and the tag is a function of the id.**
- **A new constructor in `src/ulid.rs`**, which only `RealIds::incarnation` calls.
  - The id keeps `ulid()`'s shape: a 48-bit millisecond prefix and an 80-bit field.
  - The field is SHA-256 over a new domain string, the same three parts, and 16 bytes from the host. The 16 bytes are
    `BuildHasher::hash_one` of two distinct constants under one fresh `std::collections::hash_map::RandomState`.
  - `ulid()` does not change, nor do its vectors or its other callers: run ids, scratch names, staging names.
- **The tag:** the first 60 bits of SHA-256 over a fixed domain string and the manager's incarnation id, as twelve
  Crockford base32 characters. Nothing else goes into it.
- **What the host draw is** (`c4/std-src/PROVENANCE-and-citations.txt`).
  - `RandomState::new()` keys SipHash-1-3 with 128 bits each thread draws once from the operating system, stepping one
    key per new state (`library/std/src/hash/random.rs:56-88` at 1.85.0).
  - The draw is `getrandom` on Linux, falling back to `/dev/urandom` and panicking on any other failure
    (`sys/random/linux.rs:89-169`). It is `ProcessPrng` on Windows (`sys/random/windows.rs:3-10`) and
    `CCRandomGenerateBytes` on macOS (`sys/random/apple.rs:12-15`).
  - It is never a fixed key.
- **Why not a crate.** `Cargo.lock` has no `getrandom`, `rand` or `rand_core`
  (`c4/deps/host-randomness-in-the-code-base.txt`). `standards/15` asks a new dependency for "a concrete benefit over the
  standard library or an existing dependency".
  - The standard library already gives the OS draw, and the coordinator already depends on it.
  - Its Tokio runtime is built with `Builder::new_multi_thread()` (`coordinator.rs:216-222` at `5c222ff2`). That
    runtime's seed is `RandomState::new().hash_one(...)` (tokio 1.53.1 `src/loom/std/mod.rs:40-44`,
    `util/rand.rs:36-38`, `runtime/builder.rs:276-285` and `:335`).
  - That is the "equivalent the code base already has" the triage asks for.
- **No instrument moves.**
  - `clippy.toml`'s denylist names nothing of `RandomState`.
  - `src/ulid.rs` and `src/engine/topology/seams.rs` are outside `CLASSIFIED_MODULES` (`src/effects.rs:1365-1420`), so
    the new constructor needs no row in `effects/wrappers.toml`.
  - The tag's rendering and every other change U makes in `src/workspace_manager.rs` and `naming.rs`, which are in that
    list, are private functions, which the census does not classify.

**Why the randomness is in the id, and not a salt added to the tag.** A per-process salt in the tag would also defeat the
collision, but it would move a path that frozen tests read across processes.
- **The child.** `staging_path_kill_child` drives the run in a child process and is killed in the staging path
  (`recover/tests.rs:17871-17904`).
  - It drives through `drive_hooked`, `drive_with`, `drive_as` and `resume_as_certified_by` (`:8806-8813`,
    `:8876-8892`, `:8933-8945`, `:7066-7077`).
  - Both the resume and the drive use `fixture.manager()` (`:7076`, `:8993`). That manager is derived with the run's
    recorded incarnation (`:139-147`), which the child reads from the planted log (`:14809-14822`).
- **The two parents** (corrected in round 5: with the child, three frozen tests). Each asserts the child's staging
  worktree at **its own** manager's `slot_path`, "R10: the staging worktree and its intent stand":
  `a_kill_before_the_proposals_pin_leaves_a_picked_staging_worktree_the_next_resume_reclaims_and_the_candidate_integrates`
  (`:17907`, its assertion at `:17947-17954`), and
  `a_kill_after_the_proposals_pin_leaves_a_pinned_staging_worktree_the_next_resume_reclaims_with_its_pin_and_the_candidate_integrates`
  (`:18141`, its assertion at `:18161-18168`), after a child killed at the pin.
- **So the tag must be a function of the id.** Then both processes render the same instance, as today. With a salt, the
  parents' paths would not exist, and those frozen tests would need D4.
- **Production and tests differ only in the id** (corrected in round 5). Production ids carry a host draw, so two
  production incarnations share a tag only with the probability the bound below gives, under its model. The uniqueness
  is probabilistic, not absolute. The fixed test ids (`CREATOR`, `RESUMER`, `recover/tests.rs:61-62`) keep the frozen
  tests deterministic.

**Why exclusive creation alone cannot substitute.** Suppose the successor created its instance exclusively and drew
another name when the name was taken. That refuses a collision with an instance that **exists** at the moment of
creation. The hazard is a name an earlier incarnation **used**:
- a dead instance can be absent at the successor's creation and come back later. The reclaim removed it, or the dead add
  had not yet run, and a late writer recreates it. §4.3's witness executes exactly that: a late add recreates both the
  checkout and the registration;
- a dead writer can act after the successor's creation, through a path it already holds.

Git's add is already an exclusive creation: it refuses an existing, non-empty path. Round 3's same-path rows were damaged
after exactly such a creation, of a path the successor had just removed (`c3/witness/uniq/witness-uniq-v2.43.0.log`,
`witness-uniq-v2.55.0.log`). Non-reuse is a property of the choice. It needs a choice no earlier incarnation can have
made, which here means a random one, and so it holds with the probability the bound below gives, not absolutely
(corrected in round 5).

**The collision bound, honestly** (`c4/tag/collision-bound.txt`).
- **The model:** SHA-256 as a random function, and each process's host draw uniform and independent.
- **Per pair of incarnations:** distinct ids share a tag with probability 2^-60. The ids themselves are equal with
  probability at most 2^-80, and only within one millisecond. Together, at most 8.7×10^-19 per pair.
- **Path reuse** needs two incarnations of one run, that is one execution root. For a run resumed a thousand times the
  bound is 4.3×10^-13.
- **Registration-name reuse** needs two incarnations of any runs of one repository, which share one registry, and the
  earlier name gone. Over a million incarnations in a repository's life the bound is 4.3×10^-7.
- **Both are union bounds over every pair.** A reuse is a hazard only while the earlier incarnation's writer still runs.
- **What the bound rests on.**
  - The standard library documents its seed as a "reasonable best-effort ... from a high quality, secure source of
    randomness". It warns that seeds drawn while the entropy pool is low, during boot, may be weaker
    (`collections/hash/map.rs:17-25`).
  - On Linux the key draw passes `GRND_INSECURE`, which does not wait for the pool (`sys/random/linux.rs:89-126`).
  - A host that returns repeated randomness to two processes voids the bound. Even then the id still hashes the clock,
    the pid and the counter, so it is never weaker than today's.

**The length, and the Windows budget** (`c4/pathbudget/tag-length-r4.txt`). U adds `_` and twelve characters, 13 in all,
to every task, merge and snapshot `.git` path. Git for Windows' budget is 220
(`~/pr10-evidence/fix-g5-b/r9/guest/gitdir-threshold.log:12-13`).
- **The tightest class** is PR11's nested kill children: 207, so 220 under U, at the budget. Their fixture tags are in
  `coordinator.rs`'s tests (`:13046`, `:4040`), which are not frozen, and the implementation shortens them.
- **The frozen test files' fixtures.** Their longest tag that reaches a slot expands to 44 characters
  (`finalize-killed-declared-pin-complete-before`, `recover/tests.rs:23177-23181`): 201 today, 214 under U. The longest
  tag-shaped literal anywhere in those files expands to 49, which would still give 219. No frozen test changes for the
  budget.
- **The guest.** The implementation re-measures the governed class there, as PR11 did.

**The names, restated** (replacing §3.3.1's first two bullets).
- An instance's component is the slot's component, `_`, and the tag: `tasks/k<key>-g<gen>_<tag>`, `merge/s<seq>_<tag>`,
  `snapshots/<name>_<tag>`.
- Its intent is `intents/<namespace>.<component>_<tag>.intent`, and its registration is Git's, named from the basename.
- **The split.** The tag's alphabet has no `_`, so an instance splits at its last `_`, whatever its component holds. Only
  exactly twelve Crockford characters after that `_` make a tag. Anything else is an untagged name: an instance of no
  current incarnation, reclaimed like any other (§3.3.1).

### 4.3 Instances no intent names (FUC-D3-LATEADD)

> **Round 5:** §5.4 replaces "so nothing discovered outlives finalization's walk" and "What still escapes": the
> last step of finalization sweeps again, and its remaining window is analysed.

**Executed again, with saved evidence** (first addendum; `c4/witness/lateadd/witness_lateadd.py`, with
`witness-lateadd-v2.43.0.log`, `witness-lateadd-v2.55.0.log` and their `.json`).
- **The dead incarnation A** wrote its intent. Its `git worktree add --detach --quiet <its instance> <C1>` was held
  **before it ran**: a stub waited on a release file, then executed `git`.
- **The successor B** removed what existed of A's instance, and A's intent. It wrote its own intent, added its own
  differently named instance at C2, and wrote the paid edits.
- **Then the hold was released.**
- **Two namings.** In `same`, A and B share a logical slot (`tasks/k1-g1_<A>` beside `tasks/k1-g1_<B>`). In `other`, A's
  logical slot is its own (`snapshots/s1-integration_<A>` beside B's `snapshots/s2-integration_<B>`).

| Recorded, on 2.43.0 and 2.55.0 alike | `other`: A's slot has no other instance | `same`: one logical slot, two incarnations |
|---|---|---|
| the late add's rc | 0 | 0 |
| A's checkout back | yes | yes |
| A's registration back | yes (`s1-integration_7XQ4M2ZD9KHA`) | yes (`k1-g1_7XQ4M2ZD9KHA`) |
| A's intent back | no | no |
| what an intent-only walk sees (round 3's `intents()`) | `snapshots/s2-integration` only: A's slot is never reached | `tasks/k1-g1`, through B's intent |
| what this section's discovering walk sees | A's instance, by its directory and by its registration, as another incarnation's | the same, beside B's own |
| B intact: paid edits, registration, HEAD at C2 | yes | yes |
| after a reclaim bound to A's own paths | the store holds B's registration alone; B intact | the same |

Every `VERDICT` line reads `late_add_rc=0; dead_checkout_recreated=true; dead_registration_present=true;
dead_intent_present=false`, on both versions and both namings, and for `other` also
`intent_walk_reaches_dead_slot=false`. Nothing of the witness was left running. The lens's result reproduces: isolation
survives, and round 3's reclamation did not reach the recreated instance.

**What this corrects in §3.3.4.** "On Linux, Git's own processes cannot recreate a removed instance" holds only for a
process already working in the removed directory, as the filter route showed. A Git process that has not yet run
creates its worktree path's leading directories (`builtin/worktree.c:486-488` at v2.43.0, `:535-537` at v2.55.0).

**The repair: every walk discovers them, through `intents()`.** `intents()` reports, as logical slots, each once and
sorted:
1. every intent of every incarnation (round 3);
2. every directory `<root>/<namespace>/<component>[_<tag>]` whose tag is not this manager's, an untagged name counting as
   another incarnation's;
3. every registration whose `gitdir` names `<root>/<namespace>/<instance>/.git` with a tag not this manager's.

Every walk already enumerates `intents()`: recovery's (`recover.rs:1162`, `:1212`, `:1451`), the merge module's
(`integrate.rs:1021`), finalization's (`finalize.rs:247`), an attempt's and a verification's snapshot reclaims
(`attempt.rs:542`, `run.rs:2098`), `reclaim_intents` and the torn plan (`workspace_manager.rs:2380`, `:5354`). So each
walk meets such an instance as it meets one with an intent, and removes it under its own predicate through §3.3.2's
per-instance removal. Finalization's scrub keeps every slot of its kind (`finalize.rs:240-259`), so nothing discovered
outlives finalization's walk.

**Why the current incarnation's own instances are left out.**
- A frozen test plants a torn registration that no intent names at the current manager's own path. It requires the scrub
  to refuse and to leave that registration byte-identical
  (`scrub_slots_still_refuses_a_torn_registration_no_intent_names`, `finalize.rs:631-660`).
- This incarnation creates only through an intent written first, so an intentless instance at its own tag is not one of
  its dead instances. The existing rule stays for it: "a torn registration that no intent names is not this reclaim's
  to remove".
- For another incarnation's tag the rule is U's: its instances are residue of the slot's row, however they were found.

**Ownership.** Everything under `<root>/{tasks,merge,snapshots}/<component>` is already the manager's by its containment
rule (`is_manager_slot_path`, `workspace_manager.rs:2003-2023`). A worktree there passes `revalidate`'s third clause, and
anything else inside the root refuses. A discovered instance is removed only through §3.3.2's per-instance removal: it is
contained, bound to the registration whose `gitdir` names it (#329's targeted removal), and never a repository-wide
prune.

**How the registry is read: through #329's tolerant access, and never through Git's enumeration.**
- **The scan** reads each entry of `<common git dir>/worktrees/` and its `gitdir`, with the bounded read the manager's
  scans use. An entry or file that vanishes mid-scan is skipped. An entry with no `gitdir`, or an empty one, names no
  path, and is skipped too.
- **The access.** The scan is one attempt of `tolerant_registry_access(common_git_dir, RegistryHold::Unheld, ...)`, whose
  veto answers `Again::Attempt` (#329 record §6.4 at `85f5b09b`). Any other I/O error is attempted again until the
  deadline, and then refuses as `RegistryRefused`, resumably.
- **Why `Unheld`,** as #329 gives the list and the removal's scan: the only half-written entries of this process are its
  own adds in flight. Those name no path yet, or this manager's tag, and the scan reports neither.
- **Why not `git worktree list`.** Git's enumeration dies on a torn `commondir`. The torn-registration repair is reached
  only through slots that `intents()` returns first (`workspace_manager.rs:2236-2253`, `:2379-2416`).

**The helper writing by absolute path.** A program that recreates a dead instance's directory by absolute path, with no
registration, is found by (2). The next walk that removes its slot removes it, or finalization does.

**What still escapes: a writer that outlives finalization** (R-UR, narrowed, P3).
- A dead incarnation's late add, or its helper, can recreate its instance after finalization's walk. Because the add
  creates leading directories, it can recreate the execution root above it too.
- Nothing walks after that. The residue is a checkout and, for an add, a registration that the user's
  `git worktree list` shows.
- It is never a live slot, and the operator removes it.
- Isolation holds. The outcome equations hold at finalization (§4.5); this residue arises after the run they describe.

### 4.4 #329's probe, conditionally, and shared state outside an instance (both addenda)

> **Round 5:** §5.2 and §5.3 replace "What a slot's commands can write in the common directory" and "So U's premise
> stands": rerere's replay files and the common config under relative paths were missing, and both are now switched off.

**The probe** (#329 record §6.3 at `85f5b09b`). After a failed add, the add's veto runs `read-tree -u --reset
--no-recurse-submodules <commit>` with `--git-dir=<common git dir>`, `--work-tree=<destination>` and
`GIT_INDEX_FILE=<destination>/.git/index`.
- #329 round 7's review executed two P1s in it, on 2.43, 2.50 and 2.55
  (`~/orch-pr11/reviews/review-329-d7-*-85f5b09b.review.md`).
  - With split index on, it writes a `sharedindex.*` into the common git dir, and expires one the main checkout still
    references.
  - Its configuration is the main checkout's, not the add's.
- #329's round 8 is expected to drop it (second addendum), so this subsection is brief and conditional.

**Where it writes, executed with split index off** (`c4/witness/lateadd/`, the `probe` and `probefilt` rows).
- Its destination is the current incarnation's instance.
- **A dead coordinator's orphaned probe** was held before it ran, or in its checkout's smudge filter. After the successor
  removed its destination, it failed (`Unable to create '<A's instance>/.git/index.lock': No such file or directory`, or
  `unable to create file b.dat`). It recreated nothing, and changed nothing in the common git dir outside `worktrees/`
  (`objects/` excluded).
- B stayed intact in every row. That includes today's shared path, where the probe's index lock met B's `.git` file
  (`Not a directory`).

**What U would need if a probe survived.**
- Its git dir inside its own destination, for instance a `.git` directory holding `commondir` and `HEAD`, as follow-up
  D's round 2 uses. Its index, its split-index files and their expiry then stay in the instance.
- Its destination the current incarnation's instance. The add's own destination is that by construction.
- Its residue, an unregistered checkout with a `.git` directory, removable under §3.3.2's per-instance removal. #329's
  widened store-absent exception removes that shape, and §4.3's directory scan finds a dead incarnation's.
- Under the refusing variant (§4.5), a residue that cannot be removed refuses the command.

**The general point SPLITINDEX raises for U's premise.** Can an engine Git command write outside its own instance, into
the common git dir or the main checkout's index? What was checked:
- **Split index is kept per git dir.** Git writes `sharedindex.<hex>` through `git_path`, and expires the others by
  scanning `get_git_dir()` (`read-cache.c:3216-3240`, `:3267` and `:3354` at v2.43.0; `:3212-3236`, `:3268` and `:3359`
  at v2.55.0).
  - A linked worktree's git dir is its own registration.
  - Neither `index` nor `sharedindex` is in `common_list`, the list of paths a worktree shares (`path.c:116-142` at
    v2.43.0, `:98-124` at v2.55.0, identical).
  - The probe escaped because its `--git-dir` was the common directory.
- **Where our commands run.** Every manager Git invocation runs from the base checkout or from a slot
  (`c4/git-src/manager-git-cwd-census.txt`).
  - From the base: the add, the prunes #329 deletes, `update-ref`, `commit-tree`, and reads (`show-ref`, `for-each-ref`,
    `symbolic-ref`, `cat-file`, `rev-parse`, `worktree list`).
  - None of these writes an index. The add's checkout is a child with `GIT_DIR=<new>/.git` (`builtin/worktree.c:386-393`,
    `:527-528` and `:551` at v2.43.0; `:405`, `:585-586` and `:593` at v2.55.0). `read_only_git` passes
    `--no-optional-locks` (`workspace_manager.rs:5452-5466`).
  - From a slot, a command's git dir is the slot's own registration.
- **What a slot's commands can write in the common directory** (`common_list`): objects, which only add; the instance's
  own registry entry; and `rr-cache`, where a staging or repair cherry-pick records a preimage when rerere is enabled.
  - A preimage replays nothing. Only a recorded resolution does, and nothing in the engine records one.
  - So none of it can change what a later instance, or the user's checkout, holds.
  - The converse, a resolution the user recorded entering an engine cherry-pick, is an input of today's code and outside
    this finding.
- **So U's premise stands for the engine's own commands.** A dead incarnation's late writes stay in its own instance and
  its own registration. Refs are the exception, as R-REF says (§4.8).

### 4.5 Retention withdrawn: the refusing variant (FUC-D3-ACCOUNT, FUC-D3-Q1RETAIN)

> **Round 5:** §5.4 qualifies "Nothing outlives a successful walk": it holds for what is present at the final sweep,
> and E-FUC-3's items 2 and 8 state the exception.

**The choice.** The triage prefers the refusing variant "unless evidence shows its liveness cost is unacceptable". The
evidence below does not show that, so U now has no retention.
- **A dead instance that cannot be removed refuses the command resumably, before admission,** as any removal does today:
  `remove_tree_once_handles_close` retries on Windows for 40 × 25 ms (`workspace_manager.rs:1455-1475`), and the funnel
  then returns its error.
- **Nothing outlives a successful walk.** R9's, R10's, R18's and R24's ended outcomes do not change.

**What that keeps.**
- **Q1's order, literally.** Every durable recovery record of a dead instance is reclaimed before any slot reset,
  admission or reuse, or the command refuses first.
- **The texts that would otherwise need amending:** `admission_and_leases.permits.crash_reconstruction`, the recovery
  order, and Halted's "no resumably_open item". None is amended.
- **The outcome equations** still say pruned (`c4/packet/packet-extract-r4.txt`).
- **The ledger.** `ledger.rs`'s ended rows still expect R9, R10, R18 and R24 Absent
  (`src/engine/topology/ledger.rs:917-928`). Neither the observation nor the check changes.

**Its liveness cost, against today's.**
- **On Linux,** removing a directory that a process holds open or works in succeeds. Only a creation racing the removal
  makes it fail ("directory not empty"), and the next walk tries again. The witness's dead writers that were already
  running recreated nothing after their removal (§4.3, §3.3.4).
- **On Windows,** an open handle or a working directory blocks deletion. The dead coordinator's kill-on-close job ends
  its processes, so their handles close, and pending I/O completes or is cancelled. A process outside that job that
  holds a handle (R-1W) holds the command until it exits.
- **So it costs what master's removal already costs,** plus R-1W's process for as long as it lives. Q's R-3 had the same
  cost, and the refusal names the path.

**E-FUC-3** (§4.14) loses round 3's retention clauses. Items 2 and 4 no longer admit retained residue, and R18, the
outcome equations, Halted, Q1 and the ledger need no text.

### 4.6 The two frozen oracles' replacements, fixture by fixture (FUC-D3-ORACLES)

**What the two assertions cover on master** (the lenses' R1 tables; `finalize.rs` at `5c222ff2`).
- **`:421-422`**, in `CrossKind::assert_converged` (`:402-427`).
  - The fixture: alpha's task `g1` intact, and snapshot `s1-integration` torn, its `commondir` emptied by
    `tear_registration` (`:370-400`).
  - It is asserted after the task scrub and then the snapshot scrub
    (`scrub_slots_repairs_a_torn_registration_of_a_kind_a_later_step_reclaims`, `:463-484`).
  - It is also asserted after both scrubs are retried, at **every** injected hook phase
    (`a_cross_kind_scrub_stopped_at_any_phase_converges_on_the_next`, `:566-629`).
  - It catches a Git registration for either slot that survives convergence, beyond the two captured administrative
    paths the test checks separately.
- **`:456`**, in `scrub_slots_converges_past_a_torn_registration_of_the_kind_it_reclaims` (`:430-461`).
  - The fixture: alpha's and bravo's task `g1`, bravo's torn and sorting second.
  - It is asserted after the task scrub returns 2, and after the intent and checkout checks.
  - Alpha's administrative directory has no separate absence assertion, so this predicate alone catches alpha surviving
    registered.

Under U both predicates pass whatever is registered, because no registration path ends in an untagged name.

**Where the replacements live.** `finalize.rs` and its inline tests are frozen, and its `scrub_slots` is private to it.
So the replacements go into `src/workspace_manager/tests.rs`, which is not frozen.
- They use the same fixture the frozen tests use: `workspace_manager::fixture::Fixture`, with `add_task`,
  `add_snapshot`, `tear_registration` and `registration_of`.
- They run `scrub_slots`' own sequence through the manager's public calls: for each slot `intents()` returns that the
  step keeps, `remove_worktree_proving(…, WriterProof::NoWriterAlive)`, then `remove_intent`.
- `scrub_slots` is frozen (`finalize.rs:240-259`), so that sequence cannot drift from it.

**The replacement oracle** asks Git, not a suffix. After the scrubs, `worktree_records()` must list no registration whose
path is any instance of either logical slot: any `<root>/<namespace>/<component>_<tag>` with the slot's component and
any tag, or the untagged name.

| Replacement | Fixture, as the frozen test builds it | Histories | Assertion, at each history's end |
|---|---|---|---|
| **R-O1** (for `:421-422`, first history) | alpha `g1` added; snapshot `integration(1)` added and torn; `intents()` is exactly the two; the enumeration dies on the snapshot's registration | the task scrub, then the snapshot scrub, with no injection | no registration names an instance of `kalpha-g1` or of `s1-integration`; both captured administrative paths absent; both checkouts and both intents gone |
| **R-O2** (for `:421-422`, every phase) | the same, rebuilt for each stop | for stop = 1, 2, …: both scrubs with the hook answering `Error` at the stop-th phase, then both scrubs again without injection, until a run completes untouched | after every retry, R-O1's assertion; the completed run's phases are exactly the frozen test's ten: `Worktree.Remove`, `Snapshot.Remove`, `Worktree.RemoveIntent`, `Snapshot.Remove`, `Snapshot.RemoveIntent`, each `Before` then `After` |
| **R-O3** (for `:456`) | alpha `g1` and bravo `g1` added, bravo torn and sorting second | the task scrub, returning 2 | no registration names an instance of `kalpha-g1` or of `kbravo-g1`; and alpha's own administrative directory, captured with `registration_of` before the scrub, is absent, which the frozen test does not assert |

**Each replacement also runs on a second fixture:** the frozen test's, with a second incarnation's instance of alpha
planted through a second manager with another fixed incarnation id.

**The mutations that make each replacement fail.** Each is applied to the per-instance removal, and the implementation's
record keeps the red and the green runs.
- **M-O1, stranded registration:** the removal deletes an untorn instance's checkout but leaves the registration it
  bound; the torn-registration branch is unchanged. R-O1, R-O2 and R-O3 turn red. In R-O3's fixture the frozen
  `:430-461` stays green, which is the vacuity the replacements exist to close.
- **M-O2, a dead instance's registration left:** the removal skips the registrations of other incarnations. On the
  second fixtures, R-O1, R-O2 and R-O3 turn red, because their oracle covers every instance.

**Physical-instance evidence, with several dead incarnations** (what the frozen `slots_present` and `referenced_objects`
cannot give). Those helpers map each logical intent to the **current** manager's path and add the namespace listing
(`recover/tests.rs:24175-24213`). They count dead directories, but not a registration whose checkout is gone, and they do
not tell incarnations apart.
- **P-1, in process.** Three managers over one run, with three fixed incarnation ids, leave instances of shared and of
  distinct logical slots: with intents; intentless, recreated by `git worktree add` at a dead tag; and registration-only,
  the checkout removed.
  - After each recovery walk, a census of `<root>/{tasks,merge,snapshots}/*` and of every registration naming the root
    must list only the current incarnation's instances.
  - After finalization it must list none, and R18 must be removed.
- **P-2, across processes.** Child processes, each deriving its manager with another fixed incarnation id, are killed
  at injected points inside the add in turn, as the frozen children are killed at theirs, and the parent then resumes.
  The same census applies after recovery and after finalization.
- **P-3, the late add.** §4.3's witness, through the production funnels: a held add of a dead incarnation is released
  after the successor's walk. The next walk's census finds and removes it, and B stays intact.

**D4's trigger, widened.** D4 arises if any of these holds:
1. the owner wants `finalize.rs:421-422` and `:456` restored in place;
2. the implementation's whole-suite run finds a frozen test that must change;
3. a frozen oracle's coverage cannot be preserved outside the frozen file, though every test passes. That means a
   replacement that cannot reproduce its fixture and phases, or one that M-O1 or M-O2 does not turn red.

**The tests for the tag** (§4.2), in non-frozen files.
- **T-ID1** (`src/ulid.rs`): the construction over parts and a draw. Equal clock, pid and counter with different draws
  give different ids, and vectors pin the construction.
- **T-ID2:** the wrapper's observation seam records the parts and the draw, and the test rebuilds the id from them, as
  `the_public_wrapper_returns_exactly_a_parts_construction` does for `ulid()`.
- **T-ID3:** `RealIds::incarnation()` is that construction.
- **T-TAG** (naming):
  - two managers with one id render one instance;
  - `CREATOR`, `RESUMER` and `FIRST_RESUMER` render distinct tags;
  - a tag is twelve Crockford characters;
  - the split at the last `_` round-trips for every slot kind.
- **M-ID:** the draw zeroed, which must turn T-ID1 and T-ID2 red.

### 4.7 Run directories begun before U (FUC-D3-WINCOMPAT)

**None can exist.** A topology run directory written before U needs a binary that runs the topology engine.
- The topology engine is dead code outside tests at this head: `#![cfg_attr(not(test), allow(dead_code))]`
  (`src/engine/topology.rs:3`). No production code derives a manager or runs the recovery order (census §9,
  `c3/census/census-5c222ff2.txt`). Production writes schema 3 (the PR11 record's R-G).
- The gate order keeps it so until U is in. G6 waits for this change, and "PR12 may not merge until G6 passes"
  (`cumulative_review_gates.gates[G6].blocks_later_prs`, `c4/packet/packet-extract-r4.txt`). PR12 is the slice that runs
  the topology engine for a production command.

**The rule: refuse, with guidance.** Untagged names are not kept for an older run: keeping them would be the defect
itself. Instead:
- **An untagged instance** is another incarnation's, and is reclaimed like any other (§3.3.1's rule, §4.3's discovery).
- **On Windows only, before an add,** the manager compares the length in UTF-8 bytes of the `$GIT_DIR` path the add
  will hand Git for the new worktree, as Git for Windows renders it, with Git's budget of 220 bytes (`strlen` against
  `PATH_MAX - 40`) (corrected at implementation, the decision appendix's §11). If
  it is over, the add refuses at once.
  - The refusal names the path, its byte length and the budget.
  - It says the run cannot move its private root, since recovery refuses an explicit root other than the recorded one
    (`recover.rs:138-147`). So the run must be finished by a binary whose names fit, or abandoned.
- **The refusal is the add's own,** made before any registry access. No retry policy of #329's access can then mistake
  Git's `'$GIT_DIR' too big` for registry contention.
- **What it covers:** a run begun before U, which cannot exist, and a run after U whose private root is too deep, which
  can.

### 4.8 R-REF, regraded and filed (FUC-D3-RREF)

> **Round 5:** §5.5 replaces the "Blocks G6" bullet: R-REF is the owner's decision D5, and it blocks G6 until D5 is
> made and carried out.

**It is master's, and U leaves it as master has it.**
- On Unix every engine `update-ref` holds the run's cleanup lease for as long as it lives
  (`workspace_manager.rs:3553-3570`), and a resume refuses while anyone holds it.
- On Windows that lease holds nothing and is never held (`src/rundir.rs:2503-2528`). The ref-lock reclaim rests on "the
  ambient kill-on-close job ends the children with the coordinator" (`design/26_design_merge_queue_protocol.md:398`).

**Round 3's grade, and why it does not hold.** Round 3 graded R-REF P3 because "every engine ref write is a
compare-and-swap". The lenses showed the gap: Git checks the expected old value under its lock and publishes later, and
it does not repeat the check at publication.

**What the Windows source settles, and what it does not.**
- **How Git for Windows publishes a ref:** it renames its lock file over the ref. `CreateFileW(<ref>.lock, DELETE,
  FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE)`, then `SetFileInformationByHandle(FileRenameInfoEx)` with
  `FILE_RENAME_FLAG_REPLACE_IF_EXISTS | FILE_RENAME_FLAG_POSIX_SEMANTICS`, then `CloseHandle` (`compat/mingw.c:2845-2910`
  at v2.50.1.windows.1; `c4/git-src/git-src-citations-r4.txt`).
- **Settled:**
  - termination stops every thread, and "the terminated process cannot exit until all pending I/O has been completed or
    canceled" (`c2/docs/ms-terminateprocess.txt:73-75`). So once termination begins, a publication lands only if the
    thread is already inside that one `SetFileInformationByHandle` call;
  - the successor's reclaim deletes `<ref>.lock` by name, and the dead child's handle was opened with
    `FILE_SHARE_DELETE`, so the delete is not refused.
- **Not settled by any documentation this record holds:**
  - whether a rename through a handle can still land once another process has deleted the file's name, under POSIX
    delete semantics;
  - how long such a call can stay pending, behind a filter driver or on a network filesystem.
- **So the lenses' order is neither shown nor excluded on Windows:** A checks `X→Y`; B reclaims and completes `X→Y`, then
  `Y→Z`; A's rename then lands `Y`.

**The grade.** P2, a reasoned window whose ceiling is not established.
- **The consequence if reached:** a run ref moved back.
  - It is fail-closed when the engine writes that ref again, or resumes: the disagreement refuses, by the merge-queue
    table's "`task_merged` exists but the ref disagrees" (`design/26_design_merge_queue_protocol.md:399-400`).
  - It stands at the end of the run when nothing writes that ref again.
- **If a native Windows execution shows the late rename landing after the reclaim,** it is P1, and it blocks G6.
- **G6 applicability:** Q1, because the ref lock is reclaimed as residue before reuse; Q6, one writer per ref and FIFO
  integration; ST-18, kills between finalization's ref deletions. Not INV-22, since no row changes.
- **Blocks G6:** yes, until the owner's decision D5 is made and carried out (corrected in round 5, §5.5).

**Filed** as `PR330-A-DEAD-COORDINATORS-WINDOWS-REF-WRITE-CAN-LAND-AFTER-ITS-RESUME-RECLAIMED-THE-LOCK`
(`findings/P2_crash-consistency_202610021249_a-dead-coordinators-windows-ref-write-can-land-after-its-resume-reclaimed-the-lock.md`):
`pre_existing`, `deferred`. What would close it:
- a hold that the dead coordinator's Windows `update-ref` children keep and the successor observes, like the Unix lease,
  designed and executed natively;
- or Q-D2b′'s keeper reporting the dead job empty before the reclaim;
- or the owner's acceptance of the Windows residual, as master has it.

### 4.9 Q, frozen: its open items, stated accurately (triage item 7)

Q is not repaired further. If the owner chooses Q, a further round follows then. Its open items, as round 3's review
leaves them:
- **PGIDREUSE: P1, and it blocks G6 under Q.** A Git child of the dead coordinator that has not yet joined the writer
  group `P` can join after recovery saw `P` empty, once `P` is reused (§3.5).
  - **Round 3's "Q cannot close it without a new layer" is withdrawn (FUC-D3-PGIDLAYER).** An ordering inside the
    existing child closes it: the child joins `P` in `pre_exec`, then checks that its original parent is still its
    parent, and exits without `exec` if not.
  - If the parent had died, the child refuses. If the parent was alive at the check, the child was already in `P`, so any
    later emptiness observation sees it.
  - Its cost is the one §3.5 weighed and wrongly called structural. A `pre_exec` closure takes
    `std::process::Command` off `posix_spawn`, which #329's round 3 measured at about three times the suite's runtime:
    339 s against a control of about 84 to 110 s (`d3/census/probe-iii/SUMMARY-D1.txt:7-16`).
  - It is unbuilt and unreviewed.
- **The sentinel's fallback: P2 (FUC-D3-SENTINELFALLBACK).** §3.5's repair holds when the sentinel runs its `SIGTERM`
  handler.
  - If the sentinel has not run it within the one-second bound, release sends `SIGKILL`. A coordinator that then dies
    before reaping leaves the sentinel's zombie in `P` under a non-reaping adopter. Every later acquisition refuses until
    the adopter reaps it or the machine restarts.
  - The crash-table row "after the signal and before the reap: `P` holds no sentinel" is false. The correct row: `P`
    holds no sentinel once the handler has run. Before that, and after the `SIGKILL` fallback, it may hold the
    sentinel's zombie, and the next command refuses until the adopter reaps it.
  - So it is a residual: P2, liveness, with the operator's remedy. §3.5 marked it fixed.
- **Observation identity: P2 (FUC-D3-OBSERVE).** §3.5's FROZENTESTS repair read R29 and R30 from "the checkout this
  process last held".
  - That process-global selection can read another fixture's checkout.
  - Observation must be bound to the observed run and checkout. A binding that the frozen tests' unchanged
    `ledger::observe(fold, events, physical, process)` calls can carry would have to come from the fold or the events
    they pass, the run's identity. That is unverified.
  - So FROZENTESTS is open again under Q.

### 4.10 R-G's attribution, corrected (FUC-D3-RGATTRIB, P3)

Corrected in place in §3.4, §3.6 and §3.7, each marked "corrected in round 4".
- **The grades:** the design and concurrency lenses gave reasoned P1 sequences. The regression lens gave a narrower
  reasoned P2, its fresh-add case matching the host-agent prune class. None executed R-G.
- **The consolidated grade** stays the triage's P1.
- **The executed evidence** stays as §3.4 describes it: maintenance spawning and its switches (`c2/witness/gc/`), and a
  test-started prune deleting an entry before its `locked` exists (`d3/witness/prune-in-flight/witness.log`). Neither is
  a legacy-started deletion of a completed registration.
- **The legacy starter, corrected too** (third addendum; follow-up D's round 2, #331 at `b13b4857`, its record §2.5).
  - `Workspace::commit` has no production caller, so the legacy engine's one path to automatic maintenance is a lazy
    fetch in a partial clone.
  - At 2.55.0 the `geometric` strategy, which includes `worktree-prune`, is the default, not a configuration.
  - Both are corrected in place in §3.4 and §3.7. R-G's grade stays the triage's P1 until the owner reclassifies; D's
    round 2 proposes P2 on its evidence.
- **Round 3's other nit**, §3.3.1's split rule, is restated in §4.2.

### 4.11 R-GU, from DESC's side (triage item 9; second addendum)

**Where it is filed.** Not on this branch. #329's round 8 files the external-prune class as one finding, with every
starter and both faces (second addendum).
- **The starters:** a host agent, the user's or an IDE's Git, and Git's automatic maintenance in any checkout.
- **The faces:** the add failure after the takeover, and the deletion of a completed registration after the add
  returned (#329's R13).

This subsection gives R-GU's analysis from DESC's side: the user's or an IDE's Git as the starter, the post-return face,
in U's context.

**The face, kept separate.**
- **The add-failure prefix** is a prune deleting the new entry before or after Git's takeover, so that the add fails.
  That is #329's (FUB-D6-PRUNE).
- **The post-return face** is a prune that decided in the add's window, before `locked` existed, and deletes the
  completed registration after the add returned `Ok`, under a slot in use.
  - #329 round 7 executed it with a pause (`prune-after-success`: `Ok`, and the registration gone afterwards, on 2.43.0,
    2.50.1 and 2.55.0; `~/orch-pr11/logs/pr11_fub_design7/witness/MATRIX.txt:20-21`, `:66-67`, `:112-113`).
  - On 2.55.0 any commit, fetch or merge in any checkout can start the background maintenance that prunes (#329 record
    §6.2).

**What the engine does next, in U's context.**
- **The current instance's registration deleted,** under the slot in use. That is the class's own consequence, and U
  does not change it. #329's round-7 concurrency lens graded it P1 (its F3):
  - a verification meets foreign Git state, and `run::verified` defers or parks the candidate durably (`run.rs:252-291`;
    `integrate.rs:864-890`);
  - an attempt's next Git command ends the command and cancels the other pipelines (`coordinator.rs:1405-1431`,
    `:1490-1499`);
  - when the deletion also empties the store, the slot's checkout keeps a `.git` file, which #329 round 7's removal
    refuses on every attempt, so cleanup does not converge.
- **A dead instance's registration deleted.**
  - Nothing live uses a dead instance, so isolation is untouched. U's directory scan (§4.3) still finds the checkout and
    reclaims it.
  - **One interaction is U's own: under the refusing variant, an instance whose removal refuses on every attempt refuses
    every resume, before admission.** The store-absent case above, met at a dead instance, does that until the operator
    removes the checkout.
  - Master meets the same refusal at the same slot, which has one path there. U widens where it can occur, to every
    instance a run has left, but not what it is.
- **U never deletes a registration it did not bind through its `gitdir`** (§3.3.2, §4.3). So U adds no route by which a
  prune's deletion becomes damage.

**Severity in C's context.**
- The class's own severity is graded where it is filed: #329's finding carries it.
- DESC's mechanism neither causes nor closes it.
- The part that is U's, a dead instance's store-absent refusal holding every resume, is a liveness cost, P2. A repair of
  #329's store-absent removal, which #329's round-7 review asks for, closes it for both.

**G6 applicability.**
- **Q6 and R17:** R17 excludes a second coordinator from one checkout only, and the user's checkout is theirs to use
  during a run (Q6's "untouched user checkout"). So the user's Git is a concurrent actor the design admits, and its prune
  reaches the shared registry.
- **Q1:** the next resume reclaims the slot whose registration was deleted, before admission. Where the store is gone
  too, the reclaim refuses rather than reuse, so the order holds, at a liveness cost.
- **ST-18 and INV-22:** finalization's convergence, and so R9's, R10's and R24's, fails in the store-absent case.
- **Blocks G6:** as #329's finding is graded. A P1 there blocks G6 until #329's round 8 closes it or the owner excludes
  it. Filing alone does not clear it, and C cannot: neither U nor Q reaches a process the engine did not start.

### 4.12 Round 3's findings, and where each is answered

| Id | Sev | Lens | Round 3's defect | Round 4 |
|---|---|---|---|---|
| FUC-D3-TAGUNIQUE | P1 | design, concurrency; regression "not proved" | The tag's non-reuse rested on clock, pid and counter differing, with no machine or namespace identity and no randomness; 40 bits collide besides. | §4.2: the production id carries a host draw; the tag is 60 bits over the id; the bound and the budget are stated. |
| FUC-D3-LATEADD | P2 | concurrency (executed), design, regression | A late add recreates its checkout and registration without its intent, and no intent walk reaches them. | §4.3: re-executed with saved evidence; `intents()` discovers other incarnations' intentless directories and registrations. |
| FUC-D3-ACCOUNT | P2 | all three | Retained instances balanced no equation, and `ledger.rs` expects them absent. | §4.5: retention withdrawn. |
| FUC-D3-Q1RETAIN | P2 | design, concurrency, regression | Retention changed Q1's order, and was called a literal reading. | §4.5: the refusing variant keeps Q1's order. |
| FUC-D3-ORACLES | P2 | all three | The replacements for the two vacuous oracles did not preserve their fixtures and phases. | §4.6: three replacements, fixture by fixture and phase by phase; two mutations; physical-instance tests; D4 widened. |
| FUC-D3-WINCOMPAT | P2 | regression | A run directory begun before U with a `.git` path of 212 to 220 becomes unresumable. | §4.7: none can exist; the rule refuses an over-budget instance at once. |
| FUC-D3-RREF | P2 | design, regression; concurrency "not established" | Compare-and-swap does not establish R-REF's P3 and non-blocking grade. | §4.8: regraded P2 with an unestablished ceiling; pre-existing; filed. |
| FUC-D3-SENTINELFALLBACK | P2 | all three | The `SIGKILL` fallback brings SENTINELKILL back, and the crash-table row is false. | §4.9: Q frozen; stated as Q's open P2. |
| FUC-D3-OBSERVE | P2 | concurrency | "The checkout this process last held" can be another checkout's. | §4.9: Q frozen; stated as Q's open P2. |
| FUC-D3-PGIDLAYER | P2 | concurrency | "PGIDREUSE needs a new layer" was unsupported. | §4.9: the viable ordering and its `pre_exec` cost stated; PGIDREUSE stays open under Q. |
| FUC-D3-RGATTRIB | P3 | all three | R-G's grade was attributed to all three lenses. | §4.10: corrected in place. |

**The coordination the triage carries to #329 (R-P), restated for U's dependency.** #329 must keep every explicit engine
`git worktree prune` deleted, remove only the registration bound to the instance, and add no global-prune fallback. U's
closure of R-P rests on it (§3.3.5).

### 4.13 The residuals, in one table (replacing §3.7)

> **Round 5:** replaced by §5.8.

"U" is §4's design with #329's targeted removal and tolerant access. "Q" is §3.5's group with D2b′, frozen as §4.9 leaves
it. G6's pass rule fails on an open critical or high finding. Applicability names Q1 ("reclaimed or repaired … before
any slot reset, admission, or resource reuse"), Q4 (two incarnations owning one worktree), Q6, ST-18 and INV-22.

| Residual | What | Severity and evidence | G6 applicability | Blocks G6? | Closed by |
|---|---|---|---|---|---|
| **DESC**, the filed finding | A dead coordinator's Git writers act on a slot its successor recreated or kept. | **P1.** Executed at base by #329, and by round 3 at today's naming: three routes on both Gits. | Q1, Q4, ST-18, INV-22 | **Yes**, until a closure is implemented, reviewed and accepted | **U** with #329; or **Q-D2b′** |
| **R-1** | Unix: a configured program detaches deliberately and later writes a slot path or registration it kept. | P1 as the finding is written; P3 under D3(a). Executed: `c1/witness/pg/witness-pg-setsid.log`, and round 3's helper route. | Q1, Q4, ST-18, INV-22 | Under U, no: closed for slot paths and registrations. Under Q, yes unless D3(a). | U; or D3(a) under Q |
| **R-1W** | Windows: the same, through a process outside the job. | As R-1. Reasoned. Under U's refusing variant, while its process holds a handle in a dead instance, each resume refuses; once the handle closes the removal can succeed although the process still runs, so U's safety rests on never reusing the instance's identity, not on the removal draining the process (corrected at implementation, the decision appendix's §11): P3, liveness. | Q1, ST-18, INV-22 | Under U, no. Under Q, yes unless D3(a). | U; or D3(a) |
| **R-W** | Windows: I/O pending at termination completes after the successor moved on. | **P1** (§2.5.1). Reasoned from Microsoft's documentation. | Q1, ST-18, INV-22 | Under U, no: it lands in a dead instance, and the refusing variant refuses until that instance can be removed. Under Q-D2b′, no once implemented. Under D2a or D2d, yes. | U; or D2b′ |
| **R-G1** | Legacy-started maintenance, through a lazy fetch in a partial clone (#331's record §2.5): a paused prune deletes a recreated registration that reused the old name. | **P1**, the triage's grade (design and concurrency P1, regression P2; D's round 2 proposes P2). Reasoned. | Q1; ST-18 and INV-22 for the registration's cleanup | **Yes**, until U or the legacy change | **U**, for topology names; or follow-up D's legacy change |
| **R-G2** | Legacy-started maintenance, through a lazy fetch in a partial clone: a prune reads the new entry before `locked`, and deletes it after the add completed. | **P1** (triage). D's round 2 executed the legacy engine's maintenance deleting a registration in an add's pre-`locked` state, at 2.55.0 under Git's defaults (its §2.5, witness rg2); the deletion of a completed registration is reasoned. | Q1, ST-18, INV-22 | **Yes**, until the legacy change or the owner's exclusion | **Follow-up D's legacy change only** |
| **PGIDREUSE** | Unix, Q only: a late child joins a reused group after the successor read `ESRCH`. | **P1.** Reasoned. A viable ordering exists, unbuilt (§4.9). | Q1, ST-18, INV-22 | Under Q, yes. Under U, it does not arise. | U; or the parent check under Q |
| **R-P** | The dead coordinator's own `git worktree prune` deletes the successor's new entry. | P1 class. Reasoned. | Q1, ST-18, INV-22 | **Yes**, until #329 lands, which G6 requires for #329's own finding | #329's targeted removal, kept so (§4.12) |
| **R-REF** | Windows: a terminated `update-ref`'s rename lands after the successor reclaimed its lock. | **P2, ceiling not established** (§4.8). Reasoned from Git for Windows' source and Microsoft's documentation. Pre-existing. Filed. | Q1, Q6, ST-18 | Yes, until the owner's decision D5 (corrected in round 5, §5.5) | A Windows hold the successor observes; or Q-D2b′'s keeper; or the owner's acceptance |
| **R-UR** | U only: a writer that outlives finalization recreates its instance, and perhaps the execution root, afterwards. | P3: disk, and a stray registration in the user's list, never a live slot. The late add is executed (§4.3). | INV-22, after the run it describes | No | The operator |
| **R-GU** | The user's or an IDE's prune deletes a completed registration after the add returned (§4.11). | The class's own grade is #329's finding's (its round-7 lens: P1). U's own part, a dead instance's store-absent refusal, is P2, liveness. | Q6 and R17, Q1, ST-18, INV-22 | As #329's finding is graded | #329's round 8, or the owner's exclusion |
| **Q's open items** | The sentinel's fallback, and observation identity (§4.9). | P2 each. Reasoned. | Q1's refusal path; INV-22's observations | Not by themselves; Q is blocked by PGIDREUSE | A further round, if the owner chooses Q |
| **Q's liveness residuals** | R-Z, R-L, R-T, R-2, R-3, R-K, R-NS (§2.6). | P3, or stated. | Q1's refusal path | No | Under U, none of them exists |
| **Two vacuous frozen oracles** | U only: `finalize.rs:421-422` and `:456` match untagged names. | P2, a preservation gap until §4.6's replacements are demonstrated against their mutations. | ST-18's and INV-22's finalization evidence | No standalone high; the preservation must be shown | §4.6; D4 otherwise |

**What G6 then meets, under the recommended choices.**
- **With U, #329 and follow-up D's legacy change implemented and accepted,** and #329's external-prune finding closed or
  excluded, no row is an open high but R-REF, which blocks G6 until the owner's decision D5 (corrected in round 5,
  §5.5), and R-UR stays at P3.
- **Under Q instead,** PGIDREUSE stays open and blocks G6, and so do R-1 and R-1W unless D3(a).
- **Without the legacy change,** R-G2 stays open and blocks G6 under either, unless the owner excludes it.

### 4.14 Erratum E-FUC-3, in full (replacing §3.3.7)

> **Round 5:** replaced by §5.7: item 2 changes, and item 8 is new.

In the errata file's form: anchor, current text, amendment. Each amendment is the anchor's complete new text, or an
appended clause where it says so. Round 3's numbering is kept, and item 7 is new. **No amendment is adopted at this
head; the packet stays v17.**

1. **`decisions.workspace_candidates.manager`.**
   - Current: "WorkspaceManager (src/workspace_manager.rs) owns execution-root derivation and containment, detached
     linked worktrees with durable synced intents (tasks/k<key>-g<gen>, merge/s<seq>), exact snapshot worktrees with
     intents, engine refs, byte-safe changed-path capture, worktree quiescence verification, object-residue
     classification, and forced removal; the user's checkout is read only for base capture; every worktree, snapshot,
     ref, pin, Git object, lock, reservation, container start, event-log open or append, and run-directory write goes
     through typed funnel APIs that take a typed site (see effect_site_inventory)".
   - Amended: "WorkspaceManager (src/workspace_manager.rs) owns execution-root derivation and containment, detached
     linked worktrees with durable synced intents (tasks/k<key>-g<gen>_<tag>, merge/s<seq>_<tag>), exact snapshot
     worktrees with intents (snapshots/<name>_<tag>), engine refs, byte-safe changed-path capture, worktree quiescence
     verification, object-residue classification, and forced removal; <tag> is twelve Crockford base32 characters
     rendering the first 60 bits of SHA-256 over a fixed domain string and the id of the coordinator incarnation that
     created the worktree, so a slot has one instance per creating incarnation; an instance is used only by the
     incarnation that created it, and every other incarnation's instance of a slot is residue of the slot's row; the
     user's checkout is read only for base capture; every worktree, snapshot, ref, pin, Git object, lock, reservation,
     container start, event-log open or append, and run-directory write goes through typed funnel APIs that take a
     typed site (see effect_site_inventory)".
2. **`decisions.workspace_candidates.cleanup`.** Appended after its last clause: "; every reclaim of a slot removes every
   incarnation's instance of it, each with force, contained, and bound to the registration whose gitdir names that
   instance; it reaches an instance through the instance's intent, through its directory under the slot's namespace, or
   through a registration whose gitdir names it, so an instance an earlier incarnation's writer recreated after its
   intent was removed is reclaimed by the next reclaim of its slot, and terminal finalization reclaims every instance
   that remains; an instance that cannot be removed refuses the reclaim resumably, before any admission, as any removal
   does".
3. **`resource_accounting.rows[R9]`.**
   - `granularity`: "per generation" becomes "per generation and creating incarnation".
   - `lifecycle.OpenNoAttempt`, current: "resumably_open during a live run (reused only after Worktree.Verify; otherwise
     recreated with force); closed at run end". Amended: "resumably_open during a live run (reused by the incarnation
     that created it only after Worktree.Verify, otherwise recreated with force; a fresh-process recovery removes every
     earlier incarnation's instance and creates the generation's worktree as its own); closed at run end".
   - `lifecycle.RetainedIdle`, current: "resumably_open during a live run (retried only after Worktree.Verify; otherwise
     closed); closed at run end or fresh-process recovery, then pruned". Amended: "resumably_open during a live run
     (retried by the incarnation that created it only after Worktree.Verify; otherwise closed); closed at run end or
     fresh-process recovery, then every incarnation's instance pruned".
   - `at_run_end.NoRunFinished`, current: "resumably_open for open generations; intents reclaimed on resume". Amended:
     "resumably_open for open generations; on resume every earlier incarnation's instance and intent is reclaimed, and
     each open generation's worktree is created as the resuming incarnation's".
   - Every other cell is unchanged; the ended cells still say "pruned".
4. **`resource_accounting.rows[R10]` and `[R24]`.** `granularity` only. R10's "per stale integration transaction"
   becomes "per stale integration transaction and creating incarnation". R24's "per verification role per attempt or
   per integration transaction" becomes "per verification role per attempt or per integration transaction, and creating
   incarnation". Lifecycles and ended cells are unchanged; round 3's retention exceptions are withdrawn.
5. **`transaction_fault_matrix[T-DISPATCH]`.**
   - `durable_state`, current: "generation, base, worktree path, lease relationship, source candidate for repairs".
     Amended: "generation, base, worktree path (the dispatching incarnation's instance; recovery renders the slot from
     the generation and does not read this path), lease relationship, source candidate for repairs".
   - `resume_action`, current: "live process/recovery: verify the worktree at the recorded base with Worktree.Verify
     (linked worktree at the recorded path, HEAD == base, index unlocked, no cherry-pick/merge/sequencer state) or remove
     it with force and recreate it (intent then add); for repairs re-run the recorded materialization in a verified or
     fresh worktree; continue attempt (no spend repeats); at run end: generation_closed{RunEnding}".
   - Amended: "live process (the incarnation that created the worktree): verify the worktree, its own instance, at the
     recorded base with Worktree.Verify (linked worktree at its instance path, HEAD == base, index unlocked, no
     cherry-pick/merge/sequencer state) or remove it with force and recreate it (intent then add); recovery (a fresh
     process, whose incarnation created no instance of the generation): remove with force every earlier incarnation's
     instance of the generation's worktree, then create the worktree as this incarnation's instance at the recorded base
     (intent then add), an instance that cannot be removed refusing the command resumably before admission; for repairs
     re-run the recorded materialization in a verified or fresh worktree; continue attempt (no spend repeats); at run
     end: generation_closed{RunEnding}".
   - `refusal_condition` and `test` are unchanged. `kill_after_dispatch_recreates_worktree_without_spend` still describes
     the behavior: every fresh-process resume recreates.
6. **`transaction_fault_matrix[T-REPAIR-DISPATCH]`.** `resume_action`, current: "Worktree.Verify the recorded worktree at
   the recorded base; on failure (missing, residue, wrong HEAD) remove it with force and recreate it; re-run cherry-pick
   --no-commit from the protected candidates ref deterministically; observed kind recorded when attempt_started is
   appended; a scrubbed worktree releases its materialization objects to R27 and objects of an interrupted
   materialization are Git's". Amended: "in the incarnation that created the repair worktree: Worktree.Verify that
   worktree at the recorded base; on failure (missing, residue, wrong HEAD) remove it with force and recreate it; in a
   fresh process: remove with force every earlier incarnation's instance of the repair worktree and create it as this
   incarnation's instance at the recorded base, an instance that cannot be removed refusing the command resumably; in
   either case re-run cherry-pick --no-commit from the protected candidates ref deterministically; observed kind
   recorded when attempt_started is appended; a scrubbed worktree releases its materialization objects to R27 and
   objects of an interrupted materialization are Git's".
7. **`decisions.workspace_candidates.run_creation`** (new). In the list of a fresh run's pre-lock checks, current:
   "generation of the coordinator incarnation id (per-process ULID) and the run id (no effect)". Amended: "generation of
   the coordinator incarnation id (per-process ULID whose 80-bit field is SHA-256 over the clock, the pid, a per-process
   counter and 128 bits the process draws from the host's random source, so that two processes, in any PID namespace or
   on any machine sharing the checkout, draw the same id with negligible probability) and the run id (no effect)".

**What E-FUC-3 no longer touches.** Q1, `admission_and_leases.permits.crash_reconstruction`, the recovery order, R18,
the outcome equations and Halted keep their v17 text: the refusing variant needs none of them amended (§4.5).

### 4.15 The owner's decisions (replacing §3.8)

> **Round 5:** replaced by §5.9, which adds D5 for R-REF.

The orchestrator puts one consolidated question to the owner after this round's review. These are its parts.

| | Decision | Options | Recommendation | What each leaves open |
|---|---|---|---|---|
| **D2** | **How the finding closes.** It is the central choice; D1 and D3 follow from it. | **U**, as §4 repairs it: a random production incarnation id, discovery of intentless instances, and no retention, with E-FUC-3's seven items. **Q-D2b′**: §3.5's group and keeper, frozen with its open items (§4.9), with E-FUC-1 and E-FUC-2. **Q-D2a**: the group with the coordinator wait on Windows. **D2d**: no Windows change. | **U** | **U:** R-G2 (follow-up D's), R-REF (as master has it, filed), R-UR (P3), R-GU (#329's), and the two oracles until §4.6's replacements are demonstrated. It needs #329's targeted removal and tolerant access. **Q-D2b′:** PGIDREUSE (P1, blocks G6), the sentinel fallback and observation identity (P2), R-1 and R-1W (D3), R-G1 and R-G2, and seven liveness residuals. **Q-D2a:** the same plus R-W (P1, blocks G6). **D2d:** the finding's Windows half (P1, blocks G6). |
| **D1** | **Accounting.** | Under U: E-FUC-3's items 3 and 4, granularity only. No new row, and the equations, Halted and `ledger.rs` unchanged. Under Q: D1a (E-FUC-1, with R29 and R30), or D1b. | **Follows D2.** | Declining under U: the code would reclaim instances the rows do not name. Declining under Q: the group and its record go unaccounted. |
| **D3** | **R-1 and R-1W's scope.** | (a) extend the boundary for deliberately daemonizing code to the programs the engine's Git commands run, and file R-1 at P3; (b) close it with U; (c) keep it at P1. | **Moot under U.** Under Q it is the owner's scope call. | Under Q with (c), R-1 blocks G6. |
| **D4** | **A frozen test file** (conditional). A ruling on G6's "fold, queue, merge, repair, and recovery modules byte-identical to the G5 range". | It arises only on §4.6's widened trigger. Then (a) permit that named test-only change, proved with PR11's two tiers; or (b) refuse, and the implementation finds another form or the option is dropped. | **Not needed:** §4.6's replacements, demonstrated against their mutations. | If the trigger fires and (b) is chosen, the closure needs a form that leaves the frozen file as it is. |

**Also part of the consolidated question.**
- **Retention,** round 3's other variant, is withdrawn. The owner can still ask for it, at the price the triage lists:
  - Q1 as an owner exception;
  - `permits.crash_reconstruction` and the recovery order;
  - R9's, R10's, R18's and R24's lifecycle and ended cells;
  - the outcome equations and Halted;
  - `ledger.rs`'s observation and check.
- **R-REF** (§4.8) is master's, filed at P2, with its ceiling unestablished. It is the owner's decision D5 (corrected
  in round 5, §5.5 and §5.9).
- **R-GU** (§4.11) is filed by #329's round 8, with the external-prune class. Its grade there decides whether it blocks
  G6.
- **Decision B,** which the orchestrator carries, is unchanged: follow-up D's `git_command` change (§3.4).

### 4.16 What round 4's review should test hardest

> **Round 5:** replaced by §5.10.

- §4.2: the id's construction, and the claim that the tag must be a function of the id, against the three frozen
  cross-process tests; the bound; the budget.
- §4.3: discovery against every frozen walk of `intents()` and every frozen assertion on it (`finalize.rs:387`, `:404`,
  `:438`, `:447`, `:493`, `:512`; `recover/tests.rs`'s `contains` and `is_empty`); the exclusion of the current tag
  against `scrub_slots_still_refuses_a_torn_registration_no_intent_names`; the registry read through #329's access.
- §4.4: the split-index reading, and the list of what a slot's commands write in the common directory.
- §4.5: the refusing variant's liveness cost.
- §4.6: the replacements, fixture by fixture, and the two mutations.
- §4.8: R-REF's regrade.
- §4.14: E-FUC-3's seven items, against `c4/packet/packet-extract-r4.txt`.

### 4.17 What §4 replaces

| Earlier text | Replaced by |
|---|---|
| §0's status line, and the header's authorship and evidence paragraphs | §0 and the header, updated |
| §3.3.1: the tag (40 bits of a hash of the clock-pid-counter id), "Why the tag is unique", the Windows paragraph, and the split rule | §4.2 |
| §3.3.2: retention of a dead instance in use, and the refusing variant as the owner's alternative | §4.5: the refusing variant is the design |
| §3.3.2: a removal scanning the namespace for instances whose intent has gone, only inside a removal already invoked | §4.3: discovery in `intents()` |
| §3.3.4: "On Linux, Git's own processes cannot recreate a removed instance", "What can come back", and the walks' reclaim | §4.3 |
| §3.3.4: "How the accounting reads" | §4.5 |
| §3.3.5: the Refs bullet | §4.8 |
| §3.3.5: the two vacuous oracles and their replacement | §4.6 |
| §3.3.6: the code list and the tests | §4.2 and §4.6 add to them |
| §3.3.7: E-FUC-3 | §4.14 |
| §3.4: "P1, as the triage records from all three lenses"; the paragraph on processes the engine did not start | corrected in place (§4.10); §4.11 |
| §3.5: PGIDREUSE "cannot close without a new layer"; SENTINELKILL and FROZENTESTS "fixed" | §4.9 |
| §3.6: FUC-D2-RG's lens column | corrected in place (§4.10) |
| §3.7 | §4.13 |
| §3.8 | §4.15 |
| §3.9 | §4.16 |

## 5. Round 5: rerere off, the common git dir's census, terminal accounting, and R-REF as decision D5

**What this section is.** Design review round 4 ran three `gpt-6-astra` lenses at `max` on `12375c7e`, and all three
returned CHANGES_REQUIRED (`~/orch-pr11/reviews/review-330-d4-{design,concurrency,regression}-12375c7e.review.md`,
hashed in `~/orch-pr11/reviews/SHA256SUMS-330-d4`). The lenses ran no new witness. The concurrency lens checked all 79
files in `c4/SHA256SUMS-round4`.
- **What every lens accepts:** the tag as a probabilistic construction (R1), discovery for instances present when a walk
  inspects (R2), the split-index reading (R3), the refusing variant's order (R4), the oracle replacements as specified
  (R5), R-G's attribution and R-GU's placement (R7), E-FUC-3's seven items as complete clauses (R8), and Q's open items
  as stated (R9).
- **What they found:** one P1, FUC-D4-RERERE: Git's shared rerere state crosses U's instance boundary. Two P2s:
  FUC-D4-RREF, R-REF's G6 clearance, and FUC-D4-TERMACCOUNT, terminal accounting.
- **The work list** is the orchestrator's triage (`~/orch-pr11/reviews/review-330-d4-triage.md`, items 1 to 6).

**This is the last design round of #330 before the owner's consolidated question,** whatever its review finds. The
triage's process control applies it this way: FUC-D4-RERERE is a P1 against U's isolation premise, a shared-state route
U's argument missed, and not a defect in machinery U added. Its repair is a switch of the same kind as R-G's maintenance
switches. A P1 that round 5's review finds goes into the owner's package as an open item with its candidate fix, and no
sixth round follows without the owner.

Where this section disagrees with §4, §3, §2 or §1, it governs. §5.11 lists what it replaces, and §4's headings carry
pointers.

**Who wrote it, and the evidence.** `pr11_fuc_design5` (`claude-opus-5-5`, `max`), a fresh session on this branch at
`12375c7e`. Its figures are under `~/orch-pr11/logs/pr11_fuc_design5/`, cited as `c5/…`. Master is still `5c222ff2`. The
branch is not rebased, and §5 cites our code at `5c222ff2`.
- **Our code.** `c5/code/cite5.py` prints every line §5 cites into `c5/code/code-citations-r5-5c222ff2.txt`, with each
  file's blob at `5c222ff2` and at the base `92c4ca81`. Every cited file is byte-identical at both.
- **Git source** at v2.43.0 and v2.55.0, extracted read-only from the release tarballs, whose sha256 matched
  kernel.org's signed list (`~/orch-pr11/logs/pr11_fub_design7/gits/verify-tarballs.txt`). The files are in
  `c5/git-src/`. The cited lines are in `c5/git-src/git-src-citations-r5.txt`, which `cite5.py` there regenerates. The
  callers of rerere and of automatic maintenance are in `c5/git-src/callers-r5.txt`.
- **The manager's Git invocations:** `c5/census/git-argv-census-5c222ff2.txt`, from `git-argv-census.py`.
- **Two new witnesses, within a stricter evidence plan** set by the orchestrator's direction for this round, after
  provider-safeguard pauses (`~/orch-pr11/ESCALATION.md` item 8; corrected at implementation, the decision appendix's
  §11). Only ordinary
  `git` commands ran, one after another, in temporary directories. No process was held or signalled, and nothing was
  traced, preloaded or injected.
  - What Git wrote was recorded only from Git's own `GIT_TRACE2_EVENT` stream and plain directory listings taken before
    and after each command: name, size, sha256 and modification time.
  - A listing cannot see a file made and removed inside one command. Such files are taken from Git's source.
  - `c5/witness/rerere/`: what the rerere switch changes for the engine's two cherry-picks, on both versions (§5.2).
  - `c5/census/`: what every engine command type leaves written under the common git dir, on both versions, with
    today's flags and with round 5's, under Git's defaults and under user settings that widen what Git writes (§5.3).
  - Neither runs anything concurrently. **FUC-D4-RERERE's damage sequence itself was not executed.** It stays the
    lenses' reasoned sequence, and §5.2's closure does not rest on executing it.

### 5.1 Round 4's findings, and where each is answered

| Id | Sev | Lens | Round 4's defect | Round 5 |
|---|---|---|---|---|
| FUC-D4-RERERE | P1 | all three, reasoned | Git's rerere keeps its scratch and recorded resolutions in the common git dir, which U's per-incarnation names do not separate, and the engine's cherry-picks do not disable rerere. §4.4's list of what a slot's commands write in the common directory was incomplete. | §5.2: every engine Git command runs with `rerere.enabled=false`. §5.3: the common git dir, path by path, traced by listing. |
| FUC-D4-RREF | P2 | all three, reasoned | R-REF's non-blocking grade, and its place outside the owner's decisions, bypassed its own filed guard. Its consequence if reachable is P1, and no P2 ceiling is established. | §5.5 and §5.9: R-REF is the owner's decision D5, with the exact G6 obligation. |
| FUC-D4-TERMACCOUNT | P2 | all three, reasoned on the executed late add | A late add after the task scrub, but before finalization returns, recreates a checkout and registration that no later frozen step removes, and finalization still succeeds. | §5.4: a final sweep inside the manager's `remove_execution_root`, with its remaining window analysed. E-FUC-3's items 2 and 8 state the exception (§5.7). |
| R1's wording | nit | all three | §4.2 called production ids unique outright; the bound is probabilistic. The count of frozen cross-process tests was inconsistent. | §5.6, corrected in place in §4.2. |

### 5.2 Rerere off on every engine Git command (FUC-D4-RERERE, P1)

**The finding,** by its id. With rerere enabled and a recorded resolution, a conflicting engine cherry-pick replays the
resolution through files under `<common git dir>/rr-cache/<conflict id>/`. That directory is shared by every worktree of
the repository (`common_list`, `path.c:135` at v2.43.0, `:117` at v2.55.0). So it is shared by every incarnation's
instance, and by the user's own checkout. The lenses' sequence, reasoned from Git's source at both versions, has a dead
incarnation's orphaned pick and the successor's own pick meet there. Round 4's §4.4 had listed rerere preimages only.

**The repair: one switch, on every engine Git command.**
- **`-c rerere.enabled=false`**, added by the builder `WorkspaceManager::command` (`src/workspace_manager.rs:4994-5009`
  at `5c222ff2`) beside its existing `core.hooksPath`, `core.fsmonitor=false` and `protocol.file.allow=never`.
  - Every manager command is built there: `git` (`:4946-4954`), `git_with_identity` (`:5011-5027`), `git_ok` and
    `git_line` through `git` (`:5029-5045`), and `update_ref` (`:3553-3570`).
  - So both cherry-picks run with it: the proposal pick (`:4026-4031`) and the repair materialization's (`:4250-4258`).
- **The same value on `read_only_git`** (`:5452-5464`), as round 2's settings are on both (§2.2). None of its commands
  reaches rerere: Git's callers of `repo_rerere` are cherry-pick, revert and rebase (through the sequencer), merge, am,
  `apply --3way`, stash, commit and `git rerere` itself, and `gc` runs `git rerere gc` (`c5/git-src/callers-r5.txt`,
  at both versions). The value is set there so that the two builders carry one switch set, and so that a configured
  program a read starts inherits it.
- **Only that key.** `rerere.autoUpdate` needs no value: with rerere disabled, Git never reaches the code that reads its
  effect (below).
- **Why it holds whatever the repository or the user configures.** A `-c` value is command-line scope, which outranks
  every configuration file, and Git hands it to every Git child through `GIT_CONFIG_PARAMETERS` (§2.2's citations).

**Why it closes the route.** With `rerere.enabled=false`, Git's rerere does nothing at all.
- `is_rerere_enabled()` returns 0 at its first test (`rerere.c:865-866` at v2.43.0, `:888-889` at v2.55.0).
- `setup_rerere()` then returns before it takes the worktree's `MERGE_RR` lock, reads `MERGE_RR`, or opens anything
  under `rr-cache` (`:877-883`; `:901-907`). `repo_rerere()` returns 0 (`:907-909`; `:931-933`).
- The cherry-pick's one rerere entry is that call, made when a pick fails (`sequencer.c:2403` at v2.43.0, `:2491` at
  v2.55.0).
- So an engine Git command with the switch neither reads nor writes the shared rerere state. The successor's own pick
  is the live party in every step of the lenses' sequence, and with the switch it does none of them, whatever a dead
  process does under `rr-cache`. That holds even when the dead incarnation ran a binary from before the switch.

**Executed** (`c5/witness/rerere/witness_rerere.py`, with `witness-rerere-v2.43.0.log`, `witness-rerere-v2.55.0.log` and
their `.json`). In a fresh repository per run, the user records a resolution the ordinary way. The engine's slot is then
added through the builder, and one of its two picks runs in the slot.

| Recorded, on 2.43.0 and 2.55.0 alike | Today's flags, rerere enabled (explicitly, or implicitly because `rr-cache/` exists) | With `-c rerere.enabled=false` |
|---|---|---|
| the conflicted file | the user's recorded resolution, applied | Git's conflict markers |
| `rr-cache/<id>/` | `thisimage` written, `postimage` touched | unchanged: names, sizes, hashes, times |
| `MERGE_RR` in the slot's git dir | written | absent |
| unmerged entries, `rerere.autoUpdate` unset | kept | kept |
| unmerged entries, `rerere.autoUpdate=true` | none: the path is staged | kept |
| what the manager then reads | `Conflict`; with `autoUpdate`, the repair's error "left no unmerged entry" (`:4266-4277`) and the proposal's `Unclassified` (`:4084-4140`) | `Conflict`, in every run |

Every `VERDICT` line with the switch reads `markers=True; resolution_applied=False; unmerged=1; merge_rr=False;
rr_cache_touched=False; manager_reads=Conflict`, in all ten combinations on both versions. The control with no
`rr-cache` and no setting, the engine's own fixtures, reads the same with either flag set. Trace2 recorded no child
process in any pick. Nothing of the witness was left running.

**The behaviour change.** Engine cherry-picks no longer apply recorded resolutions, and no longer record preimages.
- **What the repair path then sees:** `Materialized::Conflict`, with Git's conflict markers in the working tree and the
  unmerged entries in the index, as on a machine without rerere. The worker resolves from the markers.
- **What it no longer sees:** a resolution recorded at some other time, applied silently into its working tree while the
  index still says the path conflicts. Nor, under `rerere.autoUpdate=true`, a repair that fails with "left no unmerged
  entry", or a proposal classified `Unclassified`, though Git found a conflict.
- **The packet's determinism.** T-REPAIR-DISPATCH re-runs the pick "deterministically", and G4's adversarial test 9 and
  PR9's proof test 5 say the materialization is reproduced deterministically. A replay makes the materialization depend
  on `rr-cache` as it stands at each run, and a resolution recorded between two runs changes it. With the switch, rerere
  state is no longer an input to the pick; `merge.conflictStyle`, merge drivers and attributes still are (corrected at
  implementation, the decision appendix's §11). The switch enforces what those texts already say about rerere, and the
  packet needs no text for it: it names no Git setting at all (no `rerere`, `hooksPath`, `fsmonitor` or `maintenance` in
  v17).
- **What the user keeps.** Their own commands use and record resolutions as before. The engine neither applies nor
  touches them.
- **Who relies on rerere:** nobody in the tree.
  - The only occurrence of "rerere" at `5c222ff2` is a historical ledger line about a census of a pick's argv
    (`reviews/FINDINGS.md:6377`).
  - No test enables rerere. The fixtures create no `rr-cache`, so rerere is already off in every test, by Git's default.
  - Neither `DESIGN.md` nor the docs mention it.
  - The kill sampler builds its own command with `core.fsmonitor=false` alone
    (`src/workspace_manager/tests.rs:13285-13294`). It samples residue in fixtures without `rr-cache`, so it needs no
    change.

**The legacy engine** runs no cherry-pick, merge, rebase, am, stash or `apply --3way`. Its one rerere caller is `git
commit` (`src/workspace.rs:1019-1023`), which records resolutions of a conflict its own worktree's `MERGE_RR` lists
(`builtin/commit.c:1870` at v2.43.0). That function has no production caller (#331's round 2, §4.10). So rerere is C's
alone, as the triage found. Follow-up D's `git_command` change can carry the same value, which costs nothing there.

**Tests and a mutation**, in non-frozen files.
- **T-RR1:** a fixture with `rerere.enabled=true`, a recorded resolution and `rerere.autoUpdate=true`. The repair
  materialization reads `Conflict` with markers, `rr-cache/` lists the same names, sizes and hashes before and after,
  and no `MERGE_RR` exists in the slot's git dir.
- **T-RR2:** the same for the proposal pick and `proposal_state`.
- **M-RR:** the switch dropped from the builder. T-RR1 and T-RR2 turn red.

**What it adds:** one `-c` pair on each of two private functions (§5.3 adds a second). No process, record, lock,
row, site, crate or packet text is added, and no instrument moves.

### 5.3 The common git dir, path by path (triage item 2)

**How it was taken** (`c5/census/census_commondir.py`, with `census-commondir-v2.43.0.log`,
`census-commondir-v2.55.0.log` and their `.json`).
- **Every command type the manager runs** (`c5/census/git-argv-census-5c222ff2.txt`), in the manager's working
  directories and through the manager's flags: five adds (task, two staging, snapshot, repair); the builder's reads and
  `read_only_git`'s; `add -- <path>`, `rm --quiet --force`, `add -u` and `add -A` with the manifest pathspecs, `clean`,
  `write-tree`; `commit-tree` from the base; `update-ref` creating, compare-and-swapping and deleting a loose and a
  packed ref; the proposal pick, conflicting and clean; `read-tree --reset -u HEAD` and the repair pick; and `worktree
  prune`, which #329 deletes, for completeness.
- **Two flag sets:** today's (`:4994-5009`, `:5452-5464`), and round 2's settings with `rerere.enabled=false` added.
- **Two sets of user settings** in the repository's config: Git's defaults, and every setting found that widens what
  these commands write: `rerere.enabled=true` with a recorded resolution, `rerere.autoUpdate=true`,
  `core.logAllRefUpdates=always`, `core.splitIndex=true`, `worktree.useRelativePaths=true`, `core.bigFileThreshold=1k`
  with a larger file, `core.untrackedCache=true`, and a clean and smudge filter.
  - `core.fsyncMethod=batch` was in that list. With `core.bigFileThreshold=1k` it made 2.43.0's `add -A` fail, "unable
    to rename temporary file to …/objects/pack/pack-<hash>.pack" (`run-v2.43.0-with-fsync-batch-failed.out`), so it is
    left to the source reading below.
- **What was recorded for each step:** the listing of the whole common git dir (which is also the user's checkout's
  git dir), of the user's working tree and of the execution root, before and after; and Git's trace2 events for the
  children it started and for any maintenance or detach region.
- **Two follow-ups** (`census_reads_and_config.py`, with `census-reads-and-config-v2.43.0.log` and `-v2.55.0.log`): each
  read alone, under `core.splitIndex=true`, and the common config's text around an add under
  `worktree.useRelativePaths=true`, with and without `-c worktree.useRelativePaths=false`.
- **What a listing cannot see,** a file made and removed inside one command, is taken from Git's source: a lock is
  `<path>.lock`, created with `O_EXCL` and renamed over its file or removed (`tempfile.c:136-141` at v2.43.0, `:140-148`
  at v2.55.0); an object's temporary is renamed into place; under `core.fsyncMethod=batch` a whole temporary object
  directory, `objects/tmp_objdir-<prefix>-XXXXXX`, is moved into the object directory (`tmp-objdir.c:134` at v2.43.0,
  `:154` at v2.55.0); and the add's `locked` goes when it completes (§4.3). None of them names another instance's file.

**The table.** "Today" is the builder at `5c222ff2`. "Round 5" is today's flags with round 2's settings,
`rerere.enabled=false` and `worktree.useRelativePaths=false`. A path the table does not list was written by no step, on
either version, under either set of user settings.

| Under the common git dir | Which engine command writes it | Class | Under round 5's switches | Evidence |
|---|---|---|---|---|
| `worktrees/<its own name>/`: `HEAD`, `ORIG_HEAD`, `commondir`, `gitdir`, `index`, `logs/HEAD`; under `core.logAllRefUpdates=always` `logs/ORIG_HEAD`; under `core.splitIndex` `sharedindex.*`; a pick's `AUTO_MERGE`, `MERGE_MSG`, `CHERRY_PICK_HEAD` and `logs/CHERRY_PICK_HEAD`; the add's transient `locked` | the add, the staging commands, `write-tree`, both picks, `read-tree` | **per instance** | unchanged | every step's listing, both versions |
| `worktrees/<its own name>/MERGE_RR` | a conflicting pick, with rerere enabled | per instance | **gone**: rerere is off | `census-commondir-v2.*.log`, the wide profile's picks |
| `worktrees/<its own name>/modules/` | a submodule's git dir: `modules` is not in `common_list` (`path.c:116-141` at v2.43.0, `:98-123` at v2.55.0), and the add's checkout passes `--no-recurse-submodules` (`builtin/worktree.c:405` at v2.55.0; trace2 at both versions) | per instance | unchanged | source; trace2 |
| `worktrees/<another name>/`, whole | `worktree prune` alone | **R-P**, the stated residual #329 closes by deleting every engine prune | unchanged; gone once #329 lands | the prune step's listing |
| `worktrees/<another name>/sharedindex.*`, its time only | `fsck` in a slot, under `core.splitIndex`: with no objects named it reads every worktree's index (`builtin/fsck.c:1030-1055` at v2.43.0, `:1111-1140` at v2.55.0), and reading a split index sets its shared file's time to now (`read-cache.c:2362-2366` and `:2431` at v2.43.0, `:2342-2346` and `:2407` at v2.55.0) | **stated, no severity**: a time moved forward on an unchanged file; it can postpone Git's expiry of that shared index, never remove or change one | unchanged | `census-reads-and-config-v2.*.log:6`: fsck alone moves the three files' times, and `content_changed=[]` |
| `objects/`: loose objects; an existing object's time (Git freshens it instead of rewriting it, `object-file.c:962-983` at v2.43.0, `:68-89` at v2.55.0); under `core.bigFileThreshold`, packs `objects/pack/pack-<hash>.{pack,idx}`; transient temporaries and, under `core.fsyncMethod=batch`, a temporary object directory | staging, `write-tree`, `commit-tree`, both picks | **content-addressed and idempotent**: every name is its content's hash, and nothing is replaced in place | unchanged | the listings; source |
| `objects/info/commit-graph`, `objects/info/commit-graphs/`, `objects/pack/multi-pack-index` | none: only `commit-graph write`, `multi-pack-index write`, `repack`, `gc`, maintenance and fetch write them. `fsck` runs `commit-graph verify` and `multi-pack-index verify` (2.43.0) and `refs verify` (2.55.0), which read | not written | unchanged | trace2 children of the read steps |
| `refs/upstroke/runs/<run>/…` and the run's integration ref, their `<ref>.lock`; under `core.logAllRefUpdates=always` their reflogs in `logs/refs/…`; `packed-refs` and `packed-refs.lock` when the ref deleted is packed | `update-ref` from the base | **stated residual: shared by design.** The run's refs belong to the run, and every incarnation of the run writes them. On Unix the cleanup lease orders them (`:3553-3570`); on Windows that is R-REF, the owner's decision D5 (§5.5). The engine never removes `packed-refs.lock` (`:3626-3628`), so a dead holder makes later writers of the packed file fail, closed | unchanged | the five `update-ref` steps' listings |
| `rr-cache/<id>/`: `thisimage` written, `postimage`'s time set | a conflicting pick, with rerere enabled | the shared state of FUC-D4-RERERE | **gone**: rerere is off (§5.2) | the wide profile's two picks, at both versions; §5.2's witness |
| `config` | Git 2.48 and later, under `worktree.useRelativePaths=true`: the first add sets `core.repositoryformatversion = 1` and `extensions.relativeWorktrees = true` in the common config (`worktree.c:1106-1111` at v2.55.0) | was **unlisted**: a repository-format change an engine command makes, which Gits older than 2.48 then refuse | **gone**, by the new `-c worktree.useRelativePaths=false` | `census-reads-and-config-v2.55.0.log:14` (written) and `:17` (not, with the switch); `census-commondir-v2.55.0.log:198` |
| the user's checkout's own files at the common dir's root: `index`, `sharedindex.*`, `HEAD`, `ORIG_HEAD`, `logs/HEAD`, `FETCH_HEAD`, `MERGE_*`, `CHERRY_PICK_HEAD`, `AUTO_MERGE`, `sequencer/` | none, except `fsck` setting the time of that checkout's `sharedindex.*` under `core.splitIndex`, as above | not written | unchanged | every step's listing; the user's working tree too was unchanged in every step |
| `shallow`; `info/refs` and `objects/info/packs` | none: only fetch, clone, repack and `update-server-info` write them, and lazy fetching and every transport are off (§2.2) | not written | unchanged | listings; §2.2 |
| `gc.pid`, `gc.log`, maintenance's locks, and maintenance's own writes (pack-refs, reflog expiry, `rerere gc`, `worktree prune`) | none: `run_auto_maintenance`'s callers are commit, rebase, fetch, merge and am at both versions (`c5/git-src/callers-r5.txt`), and an engine command reaches one only through a lazy fetch, which is off | not written | unchanged | no maintenance or detach region in any step's trace2 |
| `info/` (`exclude`, `attributes`, `grafts`; `info/sparse-checkout` is per worktree, `path.c:121` at v2.43.0, `:103` at v2.55.0) | none | not written | unchanged | listings |
| `lfs/` | only a configured `git-lfs` filter, into its own content-addressed store | content-addressed, the filter's | unchanged | the filter's design; anything else a configured program does is R-1's boundary |
| `hooks/`, `description`, `branches/`, `remotes/`, `svn/`, `lost-found/`, `common/` | none: the hooks path is the empty `hooks-none`, and `fsck` runs without `--lost-found` | not written | unchanged | listings |

**The children every step started** (trace2): the add's `update-ref HEAD` child (2.43.0) and its `reset --hard
--no-recurse-submodules --quiet` (both), each with the new registration as its git dir; the configured filter; and
`fsck`'s verifiers. Nothing else, and no maintenance or detach region, in any step on either version.

**What the census changes in §4.4.** §4.4 listed objects, the instance's own registry entry and rerere preimages. The
census adds three classes.
- Rerere's replay files, the P1's shared state, now switched off.
- The common config under relative paths, a write round 4 did not know, now switched off.
- `fsck`'s time-only freshening of other worktrees' shared indexes, stated with no severity.

Refs stay the one class every incarnation of a run shares by design, and R-REF is their Windows residual.

**The read side, for completeness.** Which files under the common dir does a successor's command read that a dead
incarnation's writer can still write?
- **Objects:** their names are their contents' hashes, and a temporary appears under its final name only whole.
- **The run's refs:** R-REF, or the lease.
- **The registry:** §4.3's discovery reads it through #329's tolerant access, never Git's enumeration.
- **The common config:** with the switch, no engine command writes it.
- **`rr-cache`:** with the switch, no engine command reads it.
- **Other worktrees' indexes,** which `fsck` reads: Git replaces an index by renaming its lock over it, so a reader sees
  one whole index or the other — but a split index is two files read in two steps, and a late writer can expire the
  shared base between them, so a cross-instance `fsck` can fail; with the status read, that failure refuses
  (corrected at implementation, the decision appendix's §11).

So after round 5 an engine command shares no file it writes with a dead incarnation's writer except objects, which
cannot change, and the run's refs, which are R-REF's; a cross-instance observation such as `fsck` can still meet a late
writer's split index (`PR258-SHARED-STORE-PREDICATE-READS-SIBLINGS` carries reachability observations) (corrected
at implementation, the decision appendix's §11).

**The switch set, in full** (both builders, `:4994-5009` and `:5452-5464`):
- **Today's:** `-c core.hooksPath=<root>/hooks-none` (the builder only), `-c core.fsmonitor=false`,
  `-c protocol.file.allow=never` (the builder only), and `GIT_NO_REPLACE_OBJECTS=1`.
- **Round 2's** (§2.2): `-c maintenance.auto=false`, `-c gc.auto=0`, `-c gc.autoDetach=false`,
  `-c maintenance.autoDetach=false`, and `GIT_NO_LAZY_FETCH=1`, `GIT_ALLOW_PROTOCOL=` (empty), `GIT_TERMINAL_PROMPT=0`.
- **Round 5's:** `-c rerere.enabled=false` and `-c worktree.useRelativePaths=false`. Git before 2.48 ignores the second
  key.

**What the new switch changes for users.** Under a user's `worktree.useRelativePaths=true`, the engine's registrations
are written with absolute paths, as they are under Git's default. The manager reads either form already
(`registration_checkout`, `src/workspace_manager/parsers.rs:137-175`). The engine no longer upgrades the repository's
format. The user's own adds still do, as the user asked.

**Tests and a mutation** (non-frozen): **T-CFG1**, a fixture repository with `worktree.useRelativePaths=true`, an
engine add, and the common config byte-identical before and after, run where Git is 2.48 or later and skipped
otherwise; and **M-CFG**, the switch dropped, which turns T-CFG1 red on such a Git.

### 5.4 Terminal accounting: a final sweep inside the manager (FUC-D4-TERMACCOUNT)

**The finding,** by its id. The frozen finalizer walks each kind once.
- **The order:** the task scrub, the snapshot scrub, the staging scrub, the pins, the candidates refs, and the execution
  root last (`finalize.rs:37-44`, `:117-165`).
- **The gap:** an instance an earlier incarnation's add recreates after its kind's scrub is never walked again. The
  late add is executed (§4.3).
- **Then:** `remove_execution_root` finds the root not empty and answers `Ok(false)` (`workspace_manager.rs:2114-2175`).
  Finalization succeeds with R9 and R18 present, while the ledger's ended rows expect them absent (`ledger.rs:917-928`).
- **No removal failed, so the refusing variant never refused.** §4.3's "nothing discovered outlives finalization's walk"
  and E-FUC-3's "terminal finalization reclaims every instance that remains" were false.

**The repair: the last step sweeps again, in non-frozen code.**
- **Where.** `remove_execution_root` is a manager method. The frozen finalizer calls it last, after
  `remove_staging_leftovers` (`finalize.rs:161-164`), and its step applies to every outcome (`:59-65`).
  - `remove_staging_leftovers` is the precedent: a manager method that only terminal finalization calls
    (`workspace_manager.rs:2405-2420`).
  - `finalize.rs` does not change.
- **What, before its own funnel.**
  - **The scan:** §4.3's discovery sources (2) and (3), over all three namespaces. Every directory
    `<root>/{tasks,merge,snapshots}/<component>[_<tag>]` whose tag is not this manager's (an untagged name counts as
    another incarnation's), and every registration whose `gitdir` names one of them, read through #329's tolerant
    access as §4.3 reads it.
  - **The removal:** the per-instance removal of each, through the removal funnel of its kind, as every walk removes a
    discovered instance (§4.3).
  - **Each removal is its own kind's site and row.** The R18 site still removes only the scaffolding and the root, as
    today.
- **What it leaves:** the current incarnation's own instances, as §4.3 does.
  - At the last step of a Complete or Halted finalization, the scrubs have already removed them.
  - A torn one no intent names stays the finalizer's existing refusal
    (`scrub_slots_still_refuses_a_torn_registration_no_intent_names`, `finalize.rs:631-660`).
  - A live one keeps the root, as `the_execution_root_is_pruned_only_when_it_is_empty` asserts
    (`src/workspace_manager/tests.rs:255-283`).
- **A removal that fails refuses resumably,** as §4.5 has every removal of a dead instance do. Finalization then
  returns that error, and a resume finalizes again.

**What the frozen tests see.**
- Every frozen expectation of `execution_root_removed` is `true`: `recover/tests.rs:14454`, `:18840`, `:23904`,
  `:24653`, `:24732`, `:24818` and `:25220`, with BudgetExceeded's `:24984` checking only a prefix.
- No frozen test leaves another incarnation's instance in place at finalization: each resume's recovery walks remove
  them first (§4.3).
- So the sweep finds nothing there and fires no hook. Its scan is a read, and only a removal is an effect.
- The implementation's whole-suite run is what confirms it. A frozen test that changes is D4's trigger (2).

**The remaining window, analysed.** The sweep narrows the window to its own last scan. It cannot close it: a writer
that has not yet acted can act after any check.

| When the late writer acts | What happens | What it is |
|---|---|---|
| before the sweep's scan | the sweep removes it | reclaimed |
| between the scan and the root's emptiness check | the root is not empty: `Ok(false)`, and finalization returns with `execution_root_removed: false` and the instance present | **R-UR, P3.** Finalization's result says the root was kept. A later resume of a Complete or Halted run finalizes again, and its sweep reclaims it; a Parked or BudgetExceeded resume's recovery does |
| between the emptiness check and the root's removal | `remove_dir` fails, "directory not empty", and finalization returns that error | refused, resumably: no success with the instance present |
| after the root is removed | a late add recreates its leading directories, the root included (§4.3) | **R-UR, P3**, as round 4 stated |
| at the scan, its registration not yet holding a `gitdir` | the scan passes the entry over, as §4.3's rule does, and the add completes later | **R-UR, P3** |

**So the terminal claim, narrowed.** When finalization removes the execution root, every instance any scan could
attribute to the run had gone at the final sweep, and the root was empty when it was removed. An instance that an
earlier incarnation's still-running writer creates after the final sweep is R-UR. That holds whether the instance
appears before or after finalization returns.

**Its INV-22 consequence.** The ended equations (R9, R10, R18 and R24 pruned; `ledger.rs:917-928`) hold for everything
present at the final sweep. R-UR's instance has no class in the run's accounting: no walk of the run will reach it, and
it is the operator's to remove. E-FUC-3's items 2 and 8 state that exception (§5.7), so the owner adopts it with D1
under U, or declines it. Without it, INV-22's terminal equation would be claimed where a late writer can still break
it.

**Tests and a mutation** (non-frozen, `src/workspace_manager/tests.rs`).
- **T-FIN1:** a second manager with another fixed incarnation id adds an instance and its registration after the first
  manager's task scrub. `remove_execution_root` then answers `true`, and neither the directory nor the registration
  remains.
- **T-FIN2:** an intentless instance at the current manager's own tag is left, and the root is kept.
- **M-FIN:** the sweep skipped. T-FIN1 turns red.

### 5.5 R-REF: the owner's decision D5 (FUC-D4-RREF)

**What is withdrawn.** §4.8 graded R-REF as not blocking G6, and §4.15 set it beside the decisions rather than among
them. Both are withdrawn, and corrected in place. The filed P2 grades a reasoned window. It is not a ceiling.
- **The consequence if reachable is P1.** On Windows, a run can end with one of its refs moved back where nothing
  writes that ref again (§4.8; the finding's failure sequence).
- **Its reachability is neither shown nor excluded.** Git for Windows' source and Microsoft's documentation settle
  neither whether a rename pending at termination can land after the lock's name was deleted, nor how long it can stay
  pending (§4.8).
- **The finding's own guard binds before G6:** "a native Windows execution of a terminated update-ref against a
  reclaimed ref lock decides its ceiling, or the owner accepts the Windows residual as master has it"
  (`findings/P2_crash-consistency_202610021249_a-dead-coordinators-windows-ref-write-can-land-after-its-resume-reclaimed-the-lock.md`).
  The finding is unchanged.
- **U does not change it, and Q changes it only through its keeper, option (c2) below** (corrected at implementation,
  the decision appendix's §11), and choosing U implies no acceptance of it. U renames no ref. After §5.3, refs are the
  one class of the common git dir every incarnation of a run shares.

**The exact unresolved G6 obligation.** Before G6 passes, one of these, recorded as the owner's decision D5:
- **(a)** a reviewed native Windows determination of whether a terminated engine `update-ref` child can publish its ref
  after the successor reclaimed the lock, then whatever its result requires;
- **(b)** the owner's explicit, reviewed acceptance of the Windows residual, as master has it;
- **(c)** a closure, implemented, reviewed and accepted.

Filing R-REF, choosing U, or its P2 grade discharges none of them.

**D5's options.**
- **(a) The native determination.**
  - **What it measures:** on the Windows guest, with the Git for Windows version CI uses, whether a ref write that
    the engine's `update-ref` child had begun publishing when its job closed can still land after the successor has
    deleted the lock's name, reclaimed it as `reclaim_own_ref_lock` does, and written the ref twice. The measure is
    the ref's final value against the successor's last write, over a stated number of trials.
  - **Each trial must show the child inside its publication at the termination.** The publication has to be held
    there by a means the determination's own review accepts, such as a slow test volume or a delaying filter, as the
    finding names. Otherwise an absence of late landings measures nothing.
  - **Who runs it:** the implementer of this follow-up, on the PR11 lab's Windows guest. It is recorded in the
    implementation's record and reviewed with the implementation. Alternatively, the change that takes up the Windows
    ref-lock reclaim, as the finding says. The owner chooses.
  - **How its result maps:**
    - A late landing observed in any trial: R-REF is P1 and blocks G6 until (c) is implemented and accepted, or the
      owner accepts it under (b) knowing it is reachable.
    - No late landing in a matrix where every trial shows the hold effective: R-REF stays filed at P2 with that
      evidence, and the owner may accept it under (b) on it. Absence across trials is evidence, not proof, and the
      decision says so.
    - No hold that a review accepts as effective: no determination exists, and the owner chooses (b) or (c).
- **(b) Explicit acceptance.** The owner accepts the Windows residual as master has it, recorded as D5 (b), with
  `design/26_design_merge_queue_protocol.md:398`'s sentence corrected. It would then say that kill-on-close does not
  establish that a terminated child's pending rename has not landed.
- **(c) A closure.** Either of two:
  - **(c1)** each engine `update-ref` child on Windows keeps a hold that the successor observes and waits out, as the
    Unix cleanup lease does. This is new machinery, designed and executed natively, so it is the owner's to commission.
  - **(c2)** under Q only, Q-D2b′'s keeper reports the dead coordinator's job empty before any reclaim.

**Recommendation: (a).** It is the only option that settles the grade. Its result then picks between (b) and (c).

**Applicability:** Q1 (the ref lock is reclaimed as residue before reuse), Q6 (one writer per ref, FIFO integration) and
ST-18 (kills between finalization's ref deletions). Not INV-22: no row changes.

### 5.6 Round 4's wording, corrected (triage item 5)

Both are corrected in place in §4.2, each marked "corrected in round 5".
- **The uniqueness is probabilistic.** §4.2 said "Production ids are random, so no production pair shares a tag".
  - It now says that two production incarnations share a tag only with the probability §4.2's bound gives, under its
    model: at most 4.3×10^-13 over a thousand incarnations of one run, and 4.3×10^-7 over a million of one repository
    (`c4/tag/collision-bound.txt`).
  - "a choice no earlier incarnation can have made" is qualified the same way.
  - E-FUC-3's item 7 already says "negligible probability", and its item 1 claims no uniqueness, so neither changes.
- **Three frozen cross-process tests, named.** §4.2 named the child, `staging_path_kill_child`
  (`recover/tests.rs:17871`), and called the second parent "a sibling". §4.16 counted three. They are the child and its
  two parents:
  - `a_kill_before_the_proposals_pin_leaves_a_picked_staging_worktree_the_next_resume_reclaims_and_the_candidate_integrates`
    (`:17907`, its assertion at `:17947-17954`);
  - `a_kill_after_the_proposals_pin_leaves_a_pinned_staging_worktree_the_next_resume_reclaims_with_its_pin_and_the_candidate_integrates`
    (`:18141`, its assertion at `:18161-18168`).

### 5.7 Erratum E-FUC-3, in full (replacing §4.14)

In the errata file's form: anchor, current text, amendment. Each amendment is the anchor's complete new text, or an
appended clause where it says so. **Round 5 changes item 2's appended clause and adds item 8**, and items 3 and 4 gain
a note that points to them. Items 1, 5, 6 and 7 are §4.14's, word for word. **No amendment is adopted at this head; the
packet stays v17.** The switches of §5.2 and §5.3 need no packet text: v17 names no Git setting.

1. **`decisions.workspace_candidates.manager`.**
   - Current: "WorkspaceManager (src/workspace_manager.rs) owns execution-root derivation and containment, detached
     linked worktrees with durable synced intents (tasks/k<key>-g<gen>, merge/s<seq>), exact snapshot worktrees with
     intents, engine refs, byte-safe changed-path capture, worktree quiescence verification, object-residue
     classification, and forced removal; the user's checkout is read only for base capture; every worktree, snapshot,
     ref, pin, Git object, lock, reservation, container start, event-log open or append, and run-directory write goes
     through typed funnel APIs that take a typed site (see effect_site_inventory)".
   - Amended: "WorkspaceManager (src/workspace_manager.rs) owns execution-root derivation and containment, detached
     linked worktrees with durable synced intents (tasks/k<key>-g<gen>_<tag>, merge/s<seq>_<tag>), exact snapshot
     worktrees with intents (snapshots/<name>_<tag>), engine refs, byte-safe changed-path capture, worktree quiescence
     verification, object-residue classification, and forced removal; <tag> is twelve Crockford base32 characters
     rendering the first 60 bits of SHA-256 over a fixed domain string and the id of the coordinator incarnation that
     created the worktree, so a slot has one instance per creating incarnation; an instance is used only by the
     incarnation that created it, and every other incarnation's instance of a slot is residue of the slot's row; the
     user's checkout is read only for base capture; every worktree, snapshot, ref, pin, Git object, lock, reservation,
     container start, event-log open or append, and run-directory write goes through typed funnel APIs that take a
     typed site (see effect_site_inventory)".
2. **`decisions.workspace_candidates.cleanup`** (changed in round 5). Appended after its last clause: "; every reclaim
   of a slot removes every incarnation's instance of it, each with force, contained, and bound to the registration
   whose gitdir names that instance; it reaches an instance through the instance's intent, through its directory under
   the slot's namespace, or through a registration whose gitdir names it, so an instance an earlier incarnation's
   writer recreated after its intent was removed is reclaimed by the next reclaim of its slot; terminal finalization,
   as its last step before it removes the execution root, reclaims every earlier incarnation's instance of every kind
   that it finds there; an instance that cannot be removed refuses the reclaim resumably, before any admission, as any
   removal does; an instance that an earlier incarnation's still-running writer creates after terminal finalization's
   last reclaim is not reclaimed by the run, has no class in its accounting, and is the operator's to remove".
   - Round 4's clause ended its second part "and terminal finalization reclaims every instance that remains", which
     FUC-D4-TERMACCOUNT showed false (§5.4).
3. **`resource_accounting.rows[R9]`.**
   - `granularity`: "per generation" becomes "per generation and creating incarnation".
   - `lifecycle.OpenNoAttempt`, current: "resumably_open during a live run (reused only after Worktree.Verify; otherwise
     recreated with force); closed at run end". Amended: "resumably_open during a live run (reused by the incarnation
     that created it only after Worktree.Verify, otherwise recreated with force; a fresh-process recovery removes every
     earlier incarnation's instance and creates the generation's worktree as its own); closed at run end".
   - `lifecycle.RetainedIdle`, current: "resumably_open during a live run (retried only after Worktree.Verify; otherwise
     closed); closed at run end or fresh-process recovery, then pruned". Amended: "resumably_open during a live run
     (retried by the incarnation that created it only after Worktree.Verify; otherwise closed); closed at run end or
     fresh-process recovery, then every incarnation's instance pruned".
   - `at_run_end.NoRunFinished`, current: "resumably_open for open generations; intents reclaimed on resume". Amended:
     "resumably_open for open generations; on resume every earlier incarnation's instance and intent is reclaimed, and
     each open generation's worktree is created as the resuming incarnation's".
   - Every other cell is unchanged; the ended cells still say "pruned". Items 2 and 8 state their one exception, an
     instance an earlier incarnation's writer creates after terminal finalization's last reclaim (§5.4).
4. **`resource_accounting.rows[R10]` and `[R24]`.** `granularity` only. R10's "per stale integration transaction"
   becomes "per stale integration transaction and creating incarnation". R24's "per verification role per attempt or
   per integration transaction" becomes "per verification role per attempt or per integration transaction, and creating
   incarnation". Lifecycles and ended cells are unchanged; round 3's retention exceptions are withdrawn. Items 2 and
   8 state the ended cells' one exception, as for R9.
5. **`transaction_fault_matrix[T-DISPATCH]`.**
   - `durable_state`, current: "generation, base, worktree path, lease relationship, source candidate for repairs".
     Amended: "generation, base, worktree path (the dispatching incarnation's instance; recovery renders the slot from
     the generation and does not read this path), lease relationship, source candidate for repairs".
   - `resume_action`, current: "live process/recovery: verify the worktree at the recorded base with Worktree.Verify
     (linked worktree at the recorded path, HEAD == base, index unlocked, no cherry-pick/merge/sequencer state) or remove
     it with force and recreate it (intent then add); for repairs re-run the recorded materialization in a verified or
     fresh worktree; continue attempt (no spend repeats); at run end: generation_closed{RunEnding}".
   - Amended: "live process (the incarnation that created the worktree): verify the worktree, its own instance, at the
     recorded base with Worktree.Verify (linked worktree at its instance path, HEAD == base, index unlocked, no
     cherry-pick/merge/sequencer state) or remove it with force and recreate it (intent then add); recovery (a fresh
     process, whose incarnation created no instance of the generation): remove with force every earlier incarnation's
     instance of the generation's worktree, then create the worktree as this incarnation's instance at the recorded base
     (intent then add), an instance that cannot be removed refusing the command resumably before admission; for repairs
     re-run the recorded materialization in a verified or fresh worktree; continue attempt (no spend repeats); at run
     end: generation_closed{RunEnding}".
   - `refusal_condition` and `test` are unchanged. `kill_after_dispatch_recreates_worktree_without_spend` still describes
     the behavior: every fresh-process resume recreates.
6. **`transaction_fault_matrix[T-REPAIR-DISPATCH]`.** `resume_action`, current: "Worktree.Verify the recorded worktree at
   the recorded base; on failure (missing, residue, wrong HEAD) remove it with force and recreate it; re-run cherry-pick
   --no-commit from the protected candidates ref deterministically; observed kind recorded when attempt_started is
   appended; a scrubbed worktree releases its materialization objects to R27 and objects of an interrupted
   materialization are Git's". Amended: "in the incarnation that created the repair worktree: Worktree.Verify that
   worktree at the recorded base; on failure (missing, residue, wrong HEAD) remove it with force and recreate it; in a
   fresh process: remove with force every earlier incarnation's instance of the repair worktree and create it as this
   incarnation's instance at the recorded base, an instance that cannot be removed refusing the command resumably; in
   either case re-run cherry-pick --no-commit from the protected candidates ref deterministically; observed kind
   recorded when attempt_started is appended; a scrubbed worktree releases its materialization objects to R27 and
   objects of an interrupted materialization are Git's".
7. **`decisions.workspace_candidates.run_creation`** (new). In the list of a fresh run's pre-lock checks, current:
   "generation of the coordinator incarnation id (per-process ULID) and the run id (no effect)". Amended: "generation of
   the coordinator incarnation id (per-process ULID whose 80-bit field is SHA-256 over the clock, the pid, a per-process
   counter and 128 bits the process draws from the host's random source, so that two processes, in any PID namespace or
   on any machine sharing the checkout, draw the same id with negligible probability) and the run id (no effect)".

8. **`decisions.resource_accounting.outcome_equations`** (new in round 5). Appended to each of the `Complete`, `Parked`,
   `Halted` and `BudgetExceeded` texts: "; an instance that an earlier incarnation's still-running writer creates after
   terminal finalization's last reclaim is excepted from R9, R10, R18 and R24 (decisions.workspace_candidates.cleanup)".
   - It states in the equations what item 2 states in the cleanup decision, so INV-22's "the ledger equation for the
     run outcome holds, including ... terminal finalization" is claimed only where it holds (§5.4).
   - `ledger.rs`'s ended rows do not change (`src/engine/topology/ledger.rs:917-928`). No test runs a dead incarnation's
     writer past finalization, so its observations still find those rows absent.

**What E-FUC-3 does not touch.** Q1, `admission_and_leases.permits.crash_reconstruction`, the recovery order, R18's row
and Halted's "no resumably_open item" keep their v17 text: the refusing variant needs none of them amended (§4.5), and
item 8's exception covers the R18 and resumably-open residue of R-UR's window (§5.4).

### 5.8 The residuals, in one table (replacing §4.13)

"U" is §5's design: §4's, with the final sweep (§5.4) and the round-5 switches (§5.2, §5.3), and with #329's targeted
removal and tolerant access. "Q" is §3.5's group with D2b′, frozen as §4.9 leaves it. Under either, every engine Git
command carries §5.3's switch set. G6's pass rule fails on an open critical or high finding. Applicability names Q1
("reclaimed or repaired … before any slot reset, admission, or resource reuse"), Q4 (two incarnations owning one
worktree), Q6, ST-18 and INV-22. "Closed" means the mechanism closes that route once it is implemented, reviewed and
accepted; this head closes nothing in production.

| Residual | What | Severity and evidence | G6 applicability | Blocks G6? | Closed by |
|---|---|---|---|---|---|
| **DESC**, the filed finding | A dead coordinator's Git writers act on a slot its successor recreated or kept. | **P1.** Executed at base by #329, and by round 3 at today's naming: three routes on both Gits. | Q1, Q4, ST-18, INV-22 | **Yes**, until a closure is implemented, reviewed and accepted, with §5.3's switches | **U** with #329; or **Q-D2b′** |
| **RERERE** (FUC-D4-RERERE) | Git's rerere state lives in the common git dir, so a dead incarnation's engine cherry-pick and the successor's own can meet in it whatever their instance names. | **P1**, reasoned by all three lenses from Git's source at 2.43 and 2.55. Not executed. The switch's effect is executed: with it, no engine pick reads or writes `rr-cache` or `MERGE_RR` (§5.2, `c5/witness/rerere/`). | Q1, Q4, ST-18 | **Yes**, until the switch is implemented and accepted, as part of DESC's closure | `rerere.enabled=false` on every engine Git command, under U or Q (§5.2) |
| **CONFIG** | Git 2.48 and later, under the user's `worktree.useRelativePaths=true`: the engine's first add upgrades the repository's format in the common config, and Gits older than 2.48 then refuse the repository. | P3: no slot is damaged; an engine command changes the user's repository configuration. Executed on 2.55.0 (`c5/census/census-reads-and-config-v2.55.0.log:14`). | Q6 (the untouched user checkout) | No | `worktree.useRelativePaths=false` on every engine Git command (§5.3) |
| **R-1** | Unix: a configured program detaches deliberately and later writes a slot path or registration it kept. | P1 as the finding is written; P3 under D3(a). Executed: `c1/witness/pg/witness-pg-setsid.log`, and round 3's helper route. | Q1, Q4, ST-18, INV-22 | Under U, no: closed for slot paths and registrations. Under Q, yes unless D3(a). | U; or D3(a) under Q |
| **R-1W** | Windows: the same, through a process outside the job. | As R-1. Reasoned. Under U's refusing variant, while its process holds a handle in a dead instance, each resume refuses; once the handle closes the removal can succeed although the process still runs, so U's safety rests on never reusing the instance's identity, not on the removal draining the process (corrected at implementation, the decision appendix's §11): P3, liveness. | Q1, ST-18, INV-22 | Under U, no. Under Q, yes unless D3(a). | U; or D3(a) |
| **R-W** | Windows: I/O pending at termination completes after the successor moved on. | **P1** (§2.5.1). Reasoned from Microsoft's documentation. | Q1, ST-18, INV-22 | Under U, no: it lands in a dead instance, and the refusing variant refuses until that instance can be removed. Under Q-D2b′, no once implemented. Under D2a or D2d, yes. | U; or D2b′ |
| **R-G1** | Legacy-started maintenance, through a lazy fetch in a partial clone (#331's record §2.5): a paused prune deletes a recreated registration that reused the old name. | **P1**, the triage's grade (design and concurrency P1, regression P2; D's round 2 proposes P2). Reasoned. | Q1; ST-18 and INV-22 for the registration's cleanup | **Yes**, until U or the legacy change | **U**, for topology names; or follow-up D's legacy change |
| **R-G2** | Legacy-started maintenance, through a lazy fetch in a partial clone: a prune reads the new entry before `locked`, and deletes it after the add completed. | **P1** (triage). D's round 2 executed the legacy engine's maintenance deleting a registration in an add's pre-`locked` state, at 2.55.0 under Git's defaults (its §2.5, witness rg2); the deletion of a completed registration is reasoned. | Q1, ST-18, INV-22 | **Yes**, until the legacy change or the owner's exclusion | **Follow-up D's legacy change only** |
| **PGIDREUSE** | Unix, Q only: a late child joins a reused group after the successor read `ESRCH`. | **P1.** Reasoned. A viable ordering exists, unbuilt (§4.9). | Q1, ST-18, INV-22 | Under Q, yes. Under U, it does not arise. | U; or the parent check under Q |
| **R-P** | The dead coordinator's own `git worktree prune` deletes the successor's new entry. | P1 class. Reasoned. | Q1, ST-18, INV-22 | **Yes**, until #329 lands, which G6 requires for #329's own finding | #329's targeted removal, kept so (§4.12) |
| **R-REF** | Windows: a terminated `update-ref`'s rename lands after the successor reclaimed its lock. | Filed at P2, a reasoned window, which is not a ceiling: **P1 if reachable**, and reachability is neither shown nor excluded (§4.8, §5.5). Pre-existing on master. | Q1, Q6, ST-18 | **Yes**, until the owner's decision D5 is made and carried out | D5: (a) a native determination and what its result requires; (b) explicit acceptance; or (c) a closure (§5.5) |
| **R-UR** | U only: a writer of an earlier incarnation that is still running at terminal finalization's final sweep creates its instance afterwards, before or after finalization returns, and perhaps the execution root. | P3: disk, and a stray registration in the user's list, never a live slot. The late add is executed (§4.3). Terminal accounting holds for everything present at the final sweep (§5.4). | INV-22 and ST-18, under E-FUC-3's stated exception (items 2 and 8) | No | The operator; a later finalization or resume of the run reclaims what it finds |
| **R-GU** | The user's or an IDE's prune deletes a completed registration after the add returned (§4.11). | The class's own grade is #329's finding's (its round-7 lens: P1). U's own part, a dead instance's store-absent refusal, is P2, liveness. | Q6 and R17, Q1, ST-18, INV-22 | As #329's finding is graded | #329's round 8, or the owner's exclusion |
| **Q's open items** | The sentinel's fallback, and observation identity (§4.9). | P2 each. Reasoned. | Q1's refusal path; INV-22's observations | Not by themselves; Q is blocked by PGIDREUSE | A further round, if the owner chooses Q |
| **Q's liveness residuals** | R-Z, R-L, R-T, R-2, R-3, R-K, R-NS (§2.6). | P3, or stated. | Q1's refusal path | No | Under U, none of them exists |
| **Two vacuous frozen oracles** | U only: `finalize.rs:421-422` and `:456` match untagged names. | P2, a preservation gap until §4.6's replacements are demonstrated against their mutations. | ST-18's and INV-22's finalization evidence | No standalone high; the preservation must be shown | §4.6; D4 otherwise |

**Stated with no severity** (§5.3): `fsck` moves the times of other worktrees' shared index files under
`core.splitIndex`, and their contents do not change.

**What G6 then meets, under the recommended choices.**
- **With U, #329 and follow-up D's legacy change implemented and accepted,** §5.3's switches in both builders, #329's
  external-prune finding closed or excluded, and **D5 decided and carried out**, no row is an open high. R-UR stays at
  P3 under E-FUC-3's stated exception.
- **Without D5,** R-REF blocks G6 under either closure.
- **Under Q instead,** PGIDREUSE stays open and blocks G6, and so do R-1 and R-1W unless D3(a).
- **Without the legacy change,** R-G2 stays open and blocks G6 under either, unless the owner excludes it.

### 5.9 The owner's decisions, D1 to D5 (replacing §4.15)

The orchestrator puts one consolidated question to the owner after this round's review. These are its parts. Round 5
is the last design round before it.

| | Decision | Options | Recommendation | What each leaves open |
|---|---|---|---|---|
| **D2** | **How the finding closes.** It is the central choice; D1 and D3 follow from it. | **U**, as §4 repairs it and §5 completes it: a random production incarnation id, discovery of intentless instances, no retention, a final sweep at finalization, and the round-5 switches, with E-FUC-3's eight items. **Q-D2b′**: §3.5's group and keeper, frozen with its open items (§4.9), with E-FUC-1 and E-FUC-2, and the same switches. **Q-D2a**: the group with the coordinator wait on Windows. **D2d**: no Windows change. | **U** | **U:** R-G2 (follow-up D's), R-REF (D5), R-UR (P3, under E-FUC-3's stated exception), R-GU (#329's), and the two oracles until §4.6's replacements are demonstrated. It needs #329's targeted removal and tolerant access. **Q-D2b′:** PGIDREUSE (P1, blocks G6), the sentinel fallback and observation identity (P2), R-1 and R-1W (D3), R-G1 and R-G2, R-REF (D5), and seven liveness residuals. **Q-D2a:** the same plus R-W (P1, blocks G6). **D2d:** the finding's Windows half (P1, blocks G6). |
| **D1** | **Accounting.** | Under U: E-FUC-3's items 3 and 4 (granularity), and items 2 and 8 (the exception for an instance a still-running writer creates after terminal finalization's last reclaim). No new row; Halted and `ledger.rs` unchanged. Under Q: D1a (E-FUC-1, with R29 and R30), or D1b. | **Follows D2.** | Declining under U: the code would reclaim instances the rows do not name, and the equations would claim what a late writer can break. Declining under Q: the group and its record go unaccounted. |
| **D3** | **R-1 and R-1W's scope.** | (a) extend the boundary for deliberately daemonizing code to the programs the engine's Git commands run, and file R-1 at P3; (b) close it with U; (c) keep it at P1. | **Moot under U.** Under Q it is the owner's scope call. | Under Q with (c), R-1 blocks G6. |
| **D4** | **A frozen test file** (conditional). A ruling on G6's "fold, queue, merge, repair, and recovery modules byte-identical to the G5 range". | It arises only on §4.6's widened trigger, which also covers a frozen test the final sweep would change (§5.4). Then (a) permit that named test-only change, proved with PR11's two tiers; or (b) refuse, and the implementation finds another form or the option is dropped. | **Not needed:** §4.6's replacements, demonstrated against their mutations. | If the trigger fires and (b) is chosen, the closure needs a form that leaves the frozen file as it is. |
| **D5** | **R-REF before G6** (§5.5). Pre-existing on master; U leaves it unchanged, and under Q its closure (c2) is Q-D2b′'s keeper (corrected at implementation, the decision appendix's §11). | (a) a reviewed native Windows determination of whether a terminated engine `update-ref` child can publish after the successor reclaimed its lock, then what its result requires; (b) explicit, reviewed acceptance of the Windows residual as master has it, with `design/26`'s sentence corrected; (c) a closure: (c1) a Windows hold the successor observes, or (c2) Q-D2b′'s keeper. | **(a)**, which alone settles the grade, and then (b) or (c) as its result requires. | Without a decision, R-REF blocks G6. Under (b) the Windows window stays, P1 if reachable. Under (c1), new machinery, the owner's to commission. |

**Also part of the consolidated question.**
- **Retention,** round 3's other variant, stays withdrawn. The owner can still ask for it, at the price the triage
  lists:
  - Q1 as an owner exception;
  - `permits.crash_reconstruction` and the recovery order;
  - R9's, R10's, R18's and R24's lifecycle and ended cells;
  - the outcome equations and Halted;
  - `ledger.rs`'s observation and check.
- **R-GU** (§4.11) is filed by #329's round 8, with the external-prune class. Its grade there decides whether it blocks
  G6.
- **Decision B,** which the orchestrator carries, is unchanged: follow-up D's `git_command` change (§3.4).
- **The process control.** A P1 that round 5's review finds enters this question as an open item, with its candidate
  fix and exact scope, and no further design round follows without the owner.

### 5.10 What round 5's review should test hardest

- §5.2: the switch's placement in both builders; that `setup_rerere` returns before any `rr-cache` or `MERGE_RR` access
  when rerere is disabled, at both versions; the behaviour change and who relies on rerere; and the witness.
- §5.3: the table, path by path, against Git's `common_list` and the census; whether any engine command, or a child of
  one, writes a path under the common git dir the table does not list; and the read side.
- §5.4: the final sweep's placement and exclusions, the frozen tests' expectations, and the window table.
- §5.5 and §5.9: D5's options and the exact G6 obligation.
- §5.7: E-FUC-3's item 2 and new item 8, and their INV-22 consequence.
- §5.8: the residual table in full.

### 5.11 What §5 replaces

| Earlier text | Replaced by |
|---|---|
| §0's status line, and the header's authorship and evidence paragraphs | §0 and the header, updated |
| §4.2: "Production ids are random, so no production pair shares a tag"; "a choice no earlier incarnation can have made"; the second parent as "a sibling test" | corrected in place (§5.6) |
| §4.3: "so nothing discovered outlives finalization's walk"; "What still escapes: a writer that outlives finalization" | §5.4 |
| §4.4: "What a slot's commands can write in the common directory", and "So U's premise stands for the engine's own commands" | §5.2 and §5.3 |
| §4.5: "Nothing outlives a successful walk. R9's, R10's, R18's and R24's ended outcomes do not change." | §5.4, and E-FUC-3's items 2 and 8 |
| §4.8: the "Blocks G6" bullet; §4.13's R-REF row and its G6 paragraph | §5.5: D5, and corrected in place |
| §4.13 | §5.8 |
| §4.14 | §5.7 |
| §4.15, with its R-REF bullet corrected in place | §5.9 |
| §4.16 | §5.10 |

## 6. Implementation

**What this section is.** The implementation of U, as §4 repairs it and §5 completes it, with the decision appendix's
§11 rows for this change (`~/orch-pr11/owner-package/DECISION-APPENDIX.md`, AM-9). It is the work of `pr11_fuc_impl`
(`claude-opus-5-5`, `max`), a fresh implementer the PR11 orchestrator spawned under
`~/orch-pr11/briefs/pr11_fuc_impl.md`, whose scope is the orchestrator's assessment `~/orch-pr11/c-impl/UNIT.md`. Its
evidence is under `~/orch-pr11/logs/pr11_fuc_impl/`, cited as `c6/…`. Where this section disagrees with §1 to §5, it
records what the code does.

**What it does not do**, by the brief:
- **The packet is v17.** E-FUC-3 (§5.7) is not adopted, and the implementation does not edit the packet. U's naming
  (`tasks/k<key>-g<gen>_<tag>`) departs from v17's literal text, so **this change's merge needs the owner's adoption of
  E-FUC-3**; its items are §5.7's, cited and not restated as adopted.
- **Accounting is unchanged** (the owner's O4, D1, unadopted). `src/engine/topology/ledger.rs` and Halted are not
  touched, no late-instance exception is built (E-FUC-3's items 2 and 8; the appendix's A-1 to A-12), and no accounting
  or output-preservation check is weakened. A late instance an earlier incarnation's still-running writer creates after
  the final sweep stays unaccounted, so G6's ledger and Q2 clauses stay blocked for any run such a writer crosses
  (`FUC-D5-ACCOUNT`, §6.7).
- **No Git child's inherited environment changes** (ENV-1, O3; option (C), O3-R). FUC-D5-GITINDEXFILE stays open, filed
  with `FUB-D9-ENV` (§6.7).
- **R-REF's native Windows determination** is neither attempted nor planned: it is provider-blocked and the owner's (O7,
  D5). R-REF stays filed.
- Follow-up D, F, Q and retention are not built, and #329's mechanism is used as it is.

**The merge with #329.** `e46b71d3` merges #329's head `54a1ff147ee99ac1a61f47d483cf7b3852fe4158` into this branch's
design head `30026823`, with no rebase, so no finding's `reviewed_sha` is re-stamped. The DESC finding was added on both
sides; #329's text, which carries its dated note, is taken (blob `81ec6201`). U builds on #329's tolerant registry
access, its targeted removal and its deletion of every engine prune.

### 6.1 Per file: what changed

**`src/ulid.rs`** (noted; its prose is `docs/internals/ulid.md`'s).
- `incarnation_ulid`, the production incarnation id (§4.2): `ulid`'s layout and parts, its 80-bit field SHA-256 over a
  domain string of its own (`upstroke.incarnation.v1`), the clock's milliseconds, the pid, the per-process counter and
  16 bytes `host_draw` takes from the host: `BuildHasher::hash_one` of two distinct constants under one fresh
  `RandomState`, which the operating system seeds. `ulid`, its vectors and its other callers do not change; both
  constructions share `render`.
- An observation seam for the incarnation's parts and draw, as `ulid` has for its parts.
- **Tests** (T-ID1, T-ID2): `incarnation_parts_and_draws_construct_the_independently_computed_vectors` (vectors from
  Python's `hashlib`, `c6/vectors/vectors.py`, which first reproduces a `ulid` vector),
  `equal_clock_pid_and_nonce_with_different_draws_construct_different_ids`, and
  `the_incarnation_wrapper_returns_exactly_a_parts_and_draw_construction`.

**`src/engine/topology/seams.rs`** (noted). `RealIds::incarnation` is `incarnation_ulid`. **Test** (T-ID3):
`the_production_incarnation_is_the_host_drawn_construction`.

**`src/workspace_manager/naming.rs`.**
- `InstanceTag` (§4.2): twelve Crockford characters, the first 60 bits of SHA-256 over
  `upstroke.slot-instance-tag.v1` and the incarnation id; a function of the id alone.
- `SlotInstance`: a logical `Slot` and its creating incarnation's tag, or none for a name written before instances
  existed. Its readers, `from_intent_name` and `from_entry`, split at the last `_`; only exactly twelve characters of
  the alphabet after it make a tag, and every reader compares the instance's own rendering with what it read.
- `Slot`'s tag-aware renderings (`instance_relative`, `instance_intent_name`, `instance_id`); `relative`, `intent_name`
  and `id` are the untagged ones. `SlotId` parses an instance's identifier, and `IntentRecord::new` takes the instance's
  tag, so a record's `slot` names its instance (`design/15`'s "Synced intents").
- **Tests**: `a_tag_is_twelve_crockford_characters_of_its_incarnations_hash` (vectors for the frozen tests' `CREATOR`,
  `RESUMER`, `FIRST_RESUMER` and the fixture's `inc-1`, which render distinct tags),
  `only_twelve_crockford_characters_after_the_last_underscore_make_a_tag`,
  `every_instance_shape_survives_the_split_at_its_last_underscore` and `a_record_names_its_instance` (T-TAG), with
  naming's existing tests moved onto the instance reader.

**`src/workspace_manager.rs`.**
- **One instance per incarnation.** The manager holds its incarnation's tag. `slot_path` and `intent_path` render its
  own instance, and so every funnel that adds, verifies or runs a command in a slot acts on its own instance only.
- **Discovery** (§4.3). `intents()` reports, as logical slots, each once and sorted: every intent of every
  incarnation; every directory under the three slot namespaces that is not this incarnation's (`namespace_instances`);
  and every registration whose `gitdir` names one (`registered_instances`, a tolerant registry access with no hold,
  reading each entry's `gitdir` by bytes and never Git's enumeration). This incarnation's own instances count only
  through their intent, so the frozen finalizer's refusal of a torn registration no intent names stands.
- **Removal of every instance** (§3.3.2, §4.5). `remove_worktree_proving` binds each instance of the slot to its own
  registration in one registry attempt (`bind_instances`, over `instance_tags_of`) and removes each in turn inside one
  execution of the slot's removal site, so a caller that counts the site's hooks counts one; `remove_intent` removes
  every instance's intent in one execution of its site. A removal that fails refuses resumably: nothing is retained.
  The acted-through walk runs at each instance's own paths (`acted_through_instance_paths`).
- **The torn-registration plan** reads every instance of every slot, so an earlier incarnation's torn add is repaired
  whichever instance of its slot it is in, and the repair removes that torn instance alone (corrected at repair round
  3: it removed every instance of the slot, a live successor's included; §6.10).
- **The final sweep** (§5.4). `remove_execution_root` first runs `sweep_earlier_instances`: every earlier instance found
  by directory or registration is removed through its kind's removal site, under the finalizer's
  `WriterProof::NoWriterAlive`, bound to its own registration. The R18 site itself is unchanged.
- **The switch set** (§2.2, §5.2, §5.3) in both builders, `command` and `read_only_git` (now built by
  `read_only_command`): `ENGINE_GIT_SWITCHES` (`maintenance.auto=false`, `gc.auto=0`, `gc.autoDetach=false`,
  `maintenance.autoDetach=false`, `rerere.enabled=false`, `worktree.useRelativePaths=false`) and
  `ENGINE_GIT_ENVIRONMENT` (`GIT_NO_LAZY_FETCH=1`, `GIT_ALLOW_PROTOCOL` empty, `GIT_TERMINAL_PROMPT=0`), each one list.
- **FUC-D5-WINPATHBYTES** (§4.7, appendix §11). On Windows, `add_worktree` refuses at once, before any registry access,
  when the `$GIT_DIR` Git for Windows would be handed, rendered by the crate's Git-for-Windows speller
  (`GitdirRule::Windows`), is longer than 220 bytes in UTF-8 (`refuse_git_dir_over_budget`; `Refusal::GitDirOverBudget`
  names the path, its byte length and the budget, and says the run cannot move its private root).
- **FUC-D5-SPLITREAD's code** (appendix §11). `unreachable_objects` returns `UpstrokeError::Git`, naming the command,
  its exit and its standard error, when `fsck` exits non-zero, and its callers (`residue.rs`'s classifier) pass it on,
  so a failed observation refuses rather than certifying "no residue" or "published". This closes
  `PR128-RESIDUE-UNREACHABLE-OBJECTS-IGNORES-THE-EXIT-STATUS`.

**`src/workspace_manager/tests.rs`.** The regression tests, each with the mutations that turn it red (§6.2). Two of
#329's tests spelled a registration by its untagged name and now render the instance's.

**Other tests.** `src/engine/topology/coordinator.rs`'s tests compare gate snapshot directories with the manager's
rendering of their instances, and three of its fixture tags are shortened for Windows' budget (§6.5).
`src/engine/topology/candidate/tests.rs` binds its task registration by the instance's name. Their production code is
unchanged.

**Instruments** (`CLAUDE.md`'s first limb, whose merge the owner's standing direction for PR11 leaves to the
orchestrator; the merge waits on the owner's decisions §6.9 names, E-FUC-3's adoption among them; corrected at repair
round 2).
- `effects/wrappers.toml`: naming's new crate-visible functions (`from_entry`, `instance_id`, `instance_intent_name`,
  `instance_relative`, `of_incarnation`, `tag`) and the manager's `instance_tag` classified `effect_free`, and naming's
  shared names pinned (`as_str` 3 to 4, `slot` 2). §3.3.6 expected no row to move; the census found these, and each is
  pure string work.
- `src/runner/contract.rs`, `every_production_command_spec_payload_is_classified`: the manager's `.env(` count 8 to 10,
  the loop over `ENGINE_GIT_ENVIRONMENT` in each builder, with its text (§2.7.4 named this census).
- No effect site, resource row, process start, lock, crate or governed primitive is added.

**`design/15_design_event_log_resume_run_layout.md`** (DESIGN §15, in force with this change): the run layout's
instance names; a paragraph, "A dead coordinator's Git writers and the slot its resume uses", stating U, the switch set,
Windows' byte budget and what stays outside it; #329's paragraph's pointer to it; and the "Synced intents" contract's
`slot` naming the instance. It says it needs E-FUC-3's adoption.

**Internals notes** (§13): `docs/internals/ulid.md` and `docs/internals/engine/topology/seams.md`, for the two noted
modules whose code changed; the manager, naming and their tests carry their own prose.

**The record's text corrections** (appendix §11), each marked "corrected at implementation" in place: FUC-D5-SPLITREAD's
claim (§5.3, the read side), FUC-D5-WINPATHBYTES (§3.3.1, §4.7), FUC-D5-DETERMINISM (§5.2), FUC-D5-WINHANDLEBOUND (§4.13
and §5.8, R-1W), FUC-D5-D5Q (§5.5, §5.9) and the provenance of round 5's evidence plan (the header and §5's opening).

### 6.2 Witnesses and mutations

Every regression test is red on its first-bad shape, and each has a mutation that brings its defect back.

**The first-bad shape** (`c6/basewit/`, built by `c6/tools/basewit-c.py`): the head's
`src/workspace_manager/tests.rs` laid over the base `e46b71d3`, whose code is #329's head, with one test-only shim the
tests need to compile (`instance_tag()`, answering an empty tag, since the base has no instances). The base's production
code is unchanged, so each witness meets the defect it was written for.
- **15 of the 21 carried tests are red at base** (`c6/basewit/test-git243.log`), each failing on
  the defect itself: two incarnations render one path; the successor verifies the dead instance as its own; the
  dead instance's late add cannot even be planted beside the successor's, because the paths coincide; the scrubs leave
  the dead instance's registration; the DESC routes lose the successor's paid edits; the rerere picks replay and
  record; `fsck`'s failure reads as an empty listing.
- **Green at base, as they should be:** four controls whose first-bad shape is a mutation, not the base
  (`this_incarnations_own_instance_with_no_intent_is_not_discovered_and_keeps_the_root`,
  `an_earlier_instance_that_cannot_be_removed_refuses_the_reclaim_and_the_next_converges`,
  `a_torn_registration_an_earlier_incarnation_left_is_repaired_with_its_slot`, and T-CFG1 under the box's Git 2.43.0,
  which predates `worktree.useRelativePaths`, so the test asserts nothing there); and #329's two tests, changed only in
  the registration name they render. **T-CFG1 is red at base under Git 2.55.0** (`c6/basewit/test-t-cfg1-git255.log`):
  the engine's add changed the repository's format.
- **No first-bad shape:** the tests of the change's new API itself (the switch set's pin, the byte budget's rendering,
  the untagged instance, and the new tests in `naming.rs`, `ulid.rs` and `seams.rs`). Their red is the mutations'.

**The mutations** (`c6/mutation/<name>/`, each with `mutation.diff`, `test.log` and `summary.txt`; the rows are
`c6/tools/campaign-c.py`, applied by `c6/tools/mutate.py`): each an exact single-occurrence substitution in a copy of
the worktree's tracked files, built and tested through `upstroke-build` on this session's private base. Every row
compiled (its log names the copy on its Compiling line), and every row turned at least one test red: 25 of 25 killed
(`c6/mutation/campaign-1-summary.txt`).

**Which code they ran on.** The campaign and the base witnesses ran on the local checkpoint `8ee9a861`, never pushed.
The code commit differs from it only in `src/ulid.rs`'s import, `std::hash::RandomState` for the same type's older path,
and `src/workspace_manager/tests.rs` is the same blob in both (`c6/commits/checkpoint-vs-code.txt`).

**The two frozen oracles' mutations, as §4.6 predicted them.** M-O1 (`m-o1-stranded-registration`) turns R-O1, R-O2
and R-O3 red, and three frozen finalize tests besides; the frozen
`scrub_slots_converges_past_a_torn_registration_of_the_kind_it_reclaims` (`finalize.rs:430-461`) stays green under it,
with `scrub_slots_still_refuses_a_torn_registration_no_intent_names`: the vacuity the replacements exist to close. M-O2
(`m-o2-earlier-registration-left`) turns R-O1, R-O2 and R-O3 red on their second fixtures, and all five frozen finalize
tests stay green under it. So each frozen oracle's coverage is carried outside the frozen file, and D4's third
condition does not hold.

| Mutation | What it brings back | Red at the mutation | Log |
|---|---|---|---|
| `m-tag-shared` | every incarnation renders one tag, the shared naming U removes | 11 red: `a_successors_reclaim_removes_an_earlier_incarnations_instance_and_never_verifies_it`, `an_earlier_incarnations_instance_with_no_intent_is_found_by_every_walk`, `desc_filter_route_a_dead_adds_junk_removal_cannot_reach_the_successors_slot`, `desc_helper_route_a_dead_filters_late_helper_cannot_reach_the_successors_slot`, `one_incarnation_renders_one_instance_of_a_slot_and_another_incarnation_another`, `p1_after_every_walk_only_the_current_incarnations_instances_remain`, `p3_a_late_add_released_after_the_successors_walk_is_found_and_removed`, `r_o1_a_cross_kind_scrub_leaves_no_instance_of_either_slot_registered`, `r_o2_a_cross_kind_scrub_stopped_at_any_phase_converges_with_no_instance_registered`, `r_o3_a_task_scrub_past_a_torn_registration_leaves_no_instance_registered`, `the_final_sweep_removes_an_instance_an_earlier_incarnation_added_after_the_scrub` | `c6/mutation/m-tag-shared/` |
| `m-id-construction` | the draw is not hashed: the id is a function of clock, pid and counter | 2 red: `equal_clock_pid_and_nonce_with_different_draws_construct_different_ids`, `incarnation_parts_and_draws_construct_the_independently_computed_vectors` | `c6/mutation/m-id-construction/` |
| `m-id-draw` | the host draw is never taken: sixteen zero bytes | 1 red: `the_incarnation_wrapper_returns_exactly_a_parts_and_draw_construction` | `c6/mutation/m-id-draw/` |
| `m-id-realids` | `RealIds::incarnation` uses `ulid()`, without the draw | 1 red: `the_production_incarnation_is_the_host_drawn_construction` | `c6/mutation/m-id-realids/` |
| `m-disc-none` | discovery finds no earlier instance | 2 red: `an_earlier_incarnations_instance_with_no_intent_is_found_by_every_walk`, `p1_after_every_walk_only_the_current_incarnations_instances_remain` | `c6/mutation/m-disc-none/` |
| `m-disc-no-registry` | discovery ignores registrations | 2 red: `an_earlier_incarnations_instance_with_no_intent_is_found_by_every_walk`, `p1_after_every_walk_only_the_current_incarnations_instances_remain` | `c6/mutation/m-disc-no-registry/` |
| `m-disc-no-namespace` | discovery ignores the slot namespaces' directories | 1 red: `an_earlier_incarnations_instance_with_no_intent_is_found_by_every_walk` | `c6/mutation/m-disc-no-namespace/` |
| `m-own-not-excluded` | this incarnation's own intentless instances are discovered too | 3 red: `scrub_slots_still_refuses_a_torn_registration_no_intent_names`, `p3_a_late_add_released_after_the_successors_walk_is_found_and_removed`, `this_incarnations_own_instance_with_no_intent_is_not_discovered_and_keeps_the_root` | `c6/mutation/m-own-not-excluded/` |
| `m-remove-own-only` | a slot's removal removes only this incarnation's instance | 11 red: `a_successors_reclaim_removes_an_earlier_incarnations_instance_and_never_verifies_it`, `a_torn_registration_an_earlier_incarnation_left_is_repaired_with_its_slot`, `an_earlier_incarnations_instance_with_no_intent_is_found_by_every_walk`, `an_earlier_instance_that_cannot_be_removed_refuses_the_reclaim_and_the_next_converges`, `an_untagged_instance_from_before_instances_is_reclaimed_like_an_earlier_incarnations`, `desc_filter_route_a_dead_adds_junk_removal_cannot_reach_the_successors_slot`, `p1_after_every_walk_only_the_current_incarnations_instances_remain`, `p2_instances_of_killed_incarnations_in_other_processes_are_reclaimed_by_the_resume`, `r_o1_a_cross_kind_scrub_leaves_no_instance_of_either_slot_registered`, `r_o2_a_cross_kind_scrub_stopped_at_any_phase_converges_with_no_instance_registered`, `r_o3_a_task_scrub_past_a_torn_registration_leaves_no_instance_registered` | `c6/mutation/m-remove-own-only/` |
| `m-intent-own-only` | an intent's removal removes only this incarnation's | 9 red: `a_successors_reclaim_removes_an_earlier_incarnations_instance_and_never_verifies_it`, `an_earlier_instance_that_cannot_be_removed_refuses_the_reclaim_and_the_next_converges`, `an_untagged_instance_from_before_instances_is_reclaimed_like_an_earlier_incarnations`, `p1_after_every_walk_only_the_current_incarnations_instances_remain`, `p2_instances_of_killed_incarnations_in_other_processes_are_reclaimed_by_the_resume`, `p3_a_late_add_released_after_the_successors_walk_is_found_and_removed`, `r_o1_a_cross_kind_scrub_leaves_no_instance_of_either_slot_registered`, `r_o2_a_cross_kind_scrub_stopped_at_any_phase_converges_with_no_instance_registered`, `r_o3_a_task_scrub_past_a_torn_registration_leaves_no_instance_registered` | `c6/mutation/m-intent-own-only/` |
| `m-retain` | an earlier instance whose removal fails is skipped (retention) | 1 red: `an_earlier_instance_that_cannot_be_removed_refuses_the_reclaim_and_the_next_converges` | `c6/mutation/m-retain/` |
| `m-o1-stranded-registration` | M-O1: an untorn instance's checkout is removed and its registration left | 6 red: `a_cross_kind_scrub_stopped_at_any_phase_converges_on_the_next`, `scrub_slots_converges_when_git_has_pruned_the_emptied_registration_store`, `scrub_slots_repairs_a_torn_registration_of_a_kind_a_later_step_reclaims`, `r_o1_a_cross_kind_scrub_leaves_no_instance_of_either_slot_registered`, `r_o2_a_cross_kind_scrub_stopped_at_any_phase_converges_with_no_instance_registered`, `r_o3_a_task_scrub_past_a_torn_registration_leaves_no_instance_registered` | `c6/mutation/m-o1-stranded-registration/` |
| `m-o2-earlier-registration-left` | M-O2: other incarnations' registrations are left | 3 red: `r_o1_a_cross_kind_scrub_leaves_no_instance_of_either_slot_registered`, `r_o2_a_cross_kind_scrub_stopped_at_any_phase_converges_with_no_instance_registered`, `r_o3_a_task_scrub_past_a_torn_registration_leaves_no_instance_registered` | `c6/mutation/m-o2-earlier-registration-left/` |
| `m-fin-no-sweep` | no final sweep in `remove_execution_root` | 2 red: `p3_a_late_add_released_after_the_successors_walk_is_found_and_removed`, `the_final_sweep_removes_an_instance_an_earlier_incarnation_added_after_the_scrub` | `c6/mutation/m-fin-no-sweep/` |
| `m-rr-switch-dropped` | M-RR: `rerere.enabled=false` dropped from the switch set | 3 red: `both_builders_carry_the_engine_switch_set_and_its_bindings`, `t_rr1_the_repair_pick_neither_replays_nor_records_a_rerere_resolution`, `t_rr2_the_proposal_pick_neither_replays_nor_records_a_rerere_resolution` | `c6/mutation/m-rr-switch-dropped/` |
| `m-cfg-switch-dropped` | M-CFG: `worktree.useRelativePaths=false` dropped (run under Git 2.55.0) | 2 red: `both_builders_carry_the_engine_switch_set_and_its_bindings`, `t_cfg1_an_engine_add_leaves_the_common_config_as_it_was_under_relative_paths` | `c6/mutation/m-cfg-switch-dropped/` |
| `m-sw-binding-dropped` | one environment binding (`GIT_TERMINAL_PROMPT`) dropped | 1 red: `both_builders_carry_the_engine_switch_set_and_its_bindings` | `c6/mutation/m-sw-binding-dropped/` |
| `m-sw-read-only-unswitched` | `read_only_git` built without the switches | 1 red: `both_builders_carry_the_engine_switch_set_and_its_bindings` | `c6/mutation/m-sw-read-only-unswitched/` |
| `m-splitread-status-ignored` | M-SPLITREAD: `fsck`'s exit status ignored | 1 red: `a_failed_fsck_is_an_error_naming_the_command_and_never_an_empty_listing` | `c6/mutation/m-splitread-status-ignored/` |
| `m-win-characters` | M-WIN-CHARACTERS: the budget counted in characters | 1 red: `the_windows_git_dir_budget_counts_utf8_bytes_of_the_path_git_for_windows_is_handed` | `c6/mutation/m-win-characters/` |
| `m-win-unrendered` | M-WIN-UNRENDERED: the budget measured on the path as the engine holds it, not as Git for Windows is handed it | 1 red: `the_windows_git_dir_budget_counts_utf8_bytes_of_the_path_git_for_windows_is_handed` | `c6/mutation/m-win-unrendered/` |
| `m-torn-own-only` | the torn-registration plan reads only this incarnation's instance | 1 red: `a_torn_registration_an_earlier_incarnation_left_is_repaired_with_its_slot` | `c6/mutation/m-torn-own-only/` |
| `m-untagged-unparsed` | a name without a tag is not read as an instance | 12 red: `a_record_round_trips_and_cannot_be_built_disagreeing`, `every_instance_shape_survives_the_split_at_its_last_underscore`, `every_slot_shape_survives_the_intent_name_round_trip`, `the_intent_record_schema_is_pinned`, `the_parser_reads_the_grammar_and_validate_reads_containment`, `the_reader_accepts_exactly_the_fields_a_record_writes`, `the_record_kind_is_one_of_three_words`, `the_record_refuses_a_kind_that_disagrees_with_its_slot`, `the_record_slot_id_mirrors_the_relative_path`, `the_record_slot_is_refused_on_read_outside_its_grammar`, `two_tasks_judged_at_one_generation_and_attempt_name_different_snapshots`, `an_untagged_instance_from_before_instances_is_reclaimed_like_an_earlier_incarnations` | `c6/mutation/m-untagged-unparsed/` |
| `m-split-first` | an instance name split at its first `_` | 1 red: `every_instance_shape_survives_the_split_at_its_last_underscore` | `c6/mutation/m-split-first/` |
| `m-tag-40-bits` | an eight-character (40-bit) tag | 4 red: `a_record_names_its_instance`, `a_tag_is_twelve_crockford_characters_of_its_incarnations_hash`, `a_successors_reclaim_removes_an_earlier_incarnations_instance_and_never_verifies_it`, `one_incarnation_renders_one_instance_of_a_slot_and_another_incarnation_another` | `c6/mutation/m-tag-40-bits/` |

| Test | At base | Killed by |
|---|---|---|
| `a_cross_kind_scrub_stopped_at_any_phase_converges_on_the_next` | frozen (`finalize.rs`), unchanged | `m-o1-stranded-registration` |
| `a_failed_fsck_is_an_error_naming_the_command_and_never_an_empty_listing` | red | `m-splitread-status-ignored` |
| `a_record_names_its_instance` | no base shape (the change's own API) | `m-tag-40-bits` |
| `a_record_round_trips_and_cannot_be_built_disagreeing` | existing test | `m-untagged-unparsed` |
| `a_successors_reclaim_removes_an_earlier_incarnations_instance_and_never_verifies_it` | red | `m-tag-shared`, `m-remove-own-only`, `m-intent-own-only`, `m-tag-40-bits` |
| `a_tag_is_twelve_crockford_characters_of_its_incarnations_hash` | no base shape (the change's own API) | `m-tag-40-bits` |
| `a_torn_registration_an_earlier_incarnation_left_is_repaired_with_its_slot` | green | `m-remove-own-only`, `m-torn-own-only` |
| `an_add_killed_before_it_wrote_gitdir_is_unlisted_and_refuses_forced_cleanup` | green |  |
| `an_earlier_incarnations_instance_with_no_intent_is_found_by_every_walk` | red | `m-tag-shared`, `m-disc-none`, `m-disc-no-registry`, `m-disc-no-namespace`, `m-remove-own-only` |
| `an_earlier_instance_that_cannot_be_removed_refuses_the_reclaim_and_the_next_converges` | green | `m-remove-own-only`, `m-intent-own-only`, `m-retain` |
| `an_untagged_instance_from_before_instances_is_reclaimed_like_an_earlier_incarnations` | no base shape (the change's own API) | `m-remove-own-only`, `m-intent-own-only`, `m-untagged-unparsed` |
| `both_builders_carry_the_engine_switch_set_and_its_bindings` | no base shape (the change's own API) | `m-rr-switch-dropped`, `m-cfg-switch-dropped`, `m-sw-binding-dropped`, `m-sw-read-only-unswitched` |
| `desc_filter_route_a_dead_adds_junk_removal_cannot_reach_the_successors_slot` | red | `m-tag-shared`, `m-remove-own-only` |
| `desc_helper_route_a_dead_filters_late_helper_cannot_reach_the_successors_slot` | red | `m-tag-shared` |
| `equal_clock_pid_and_nonce_with_different_draws_construct_different_ids` | no base shape (the change's own API) | `m-id-construction` |
| `every_instance_shape_survives_the_split_at_its_last_underscore` | no base shape (the change's own API) | `m-untagged-unparsed`, `m-split-first` |
| `every_slot_shape_survives_the_intent_name_round_trip` | existing test | `m-untagged-unparsed` |
| `incarnation_parts_and_draws_construct_the_independently_computed_vectors` | no base shape (the change's own API) | `m-id-construction` |
| `no_removal_prunes_another_processs_registration_and_the_store_goes_only_when_empty` | green |  |
| `one_incarnation_renders_one_instance_of_a_slot_and_another_incarnation_another` | red | `m-tag-shared`, `m-tag-40-bits` |
| `p1_after_every_walk_only_the_current_incarnations_instances_remain` | red | `m-tag-shared`, `m-disc-none`, `m-disc-no-registry`, `m-remove-own-only`, `m-intent-own-only` |
| `p2_instances_of_killed_incarnations_in_other_processes_are_reclaimed_by_the_resume` | red | `m-remove-own-only`, `m-intent-own-only` |
| `p3_a_late_add_released_after_the_successors_walk_is_found_and_removed` | red | `m-tag-shared`, `m-own-not-excluded`, `m-intent-own-only`, `m-fin-no-sweep` |
| `r_o1_a_cross_kind_scrub_leaves_no_instance_of_either_slot_registered` | red | `m-tag-shared`, `m-remove-own-only`, `m-intent-own-only`, `m-o1-stranded-registration`, `m-o2-earlier-registration-left` |
| `r_o2_a_cross_kind_scrub_stopped_at_any_phase_converges_with_no_instance_registered` | red | `m-tag-shared`, `m-remove-own-only`, `m-intent-own-only`, `m-o1-stranded-registration`, `m-o2-earlier-registration-left` |
| `r_o3_a_task_scrub_past_a_torn_registration_leaves_no_instance_registered` | red | `m-tag-shared`, `m-remove-own-only`, `m-intent-own-only`, `m-o1-stranded-registration`, `m-o2-earlier-registration-left` |
| `scrub_slots_converges_when_git_has_pruned_the_emptied_registration_store` | frozen (`finalize.rs`), unchanged | `m-o1-stranded-registration` |
| `scrub_slots_repairs_a_torn_registration_of_a_kind_a_later_step_reclaims` | frozen (`finalize.rs`), unchanged | `m-o1-stranded-registration` |
| `scrub_slots_still_refuses_a_torn_registration_no_intent_names` | frozen (`finalize.rs`), unchanged | `m-own-not-excluded` |
| `t_cfg1_an_engine_add_leaves_the_common_config_as_it_was_under_relative_paths` | green under Git 2.43.0, which predates the setting; red under Git 2.55.0 | `m-cfg-switch-dropped` |
| `t_rr1_the_repair_pick_neither_replays_nor_records_a_rerere_resolution` | red | `m-rr-switch-dropped` |
| `t_rr2_the_proposal_pick_neither_replays_nor_records_a_rerere_resolution` | red | `m-rr-switch-dropped` |
| `the_final_sweep_removes_an_instance_an_earlier_incarnation_added_after_the_scrub` | red | `m-tag-shared`, `m-fin-no-sweep` |
| `the_incarnation_wrapper_returns_exactly_a_parts_and_draw_construction` | no base shape (the change's own API) | `m-id-draw` |
| `the_intent_record_schema_is_pinned` | existing test | `m-untagged-unparsed` |
| `the_parser_reads_the_grammar_and_validate_reads_containment` | existing test | `m-untagged-unparsed` |
| `the_production_incarnation_is_the_host_drawn_construction` | no base shape (the change's own API) | `m-id-realids` |
| `the_reader_accepts_exactly_the_fields_a_record_writes` | existing test | `m-untagged-unparsed` |
| `the_record_kind_is_one_of_three_words` | existing test | `m-untagged-unparsed` |
| `the_record_refuses_a_kind_that_disagrees_with_its_slot` | existing test | `m-untagged-unparsed` |
| `the_record_slot_id_mirrors_the_relative_path` | existing test | `m-untagged-unparsed` |
| `the_record_slot_is_refused_on_read_outside_its_grammar` | existing test | `m-untagged-unparsed` |
| `the_windows_git_dir_budget_counts_utf8_bytes_of_the_path_git_for_windows_is_handed` | no base shape (the change's own API) | `m-win-characters`, `m-win-unrendered` |
| `this_incarnations_own_instance_with_no_intent_is_not_discovered_and_keeps_the_root` | green | `m-own-not-excluded` |
| `two_tasks_judged_at_one_generation_and_attempt_name_different_snapshots` | existing test | `m-untagged-unparsed` |

### 6.3 The frozen set, and D4

- **Tier proof** (`c6/frozen/frozen-proof-61b018bf.txt`, from `c6/tools/frozen-proof.sh`). Part 1, this change's own
  delta over PR11's R-D set (26 production files, 8 whole-file test children): byte-identical, 34 of 34, against master
  `5c222ff2` and against #329's head `54a1ff14`. Part 2, the cumulative comparison against G5's range `d724fb16`, by the
  appendix's §8.5 rule (E-G6-1, not adopted): PASS with exactly master's four enumerated paths, 241 insertions and 74
  deletions, none of them this change's.
- **No frozen test changed, and none failed**, in the whole-suite runs (§6.4). D4's trigger (§4.6's widened one, which
  §5.9 extends to a frozen test the final sweep would change) did not fire: (1) is the owner's to raise; (2) no frozen
  test must change; (3) each frozen oracle's coverage is preserved outside the frozen file, by R-O1 to R-O3 against M-O1
  and M-O2 (§6.2), and M-O1 leaves the frozen `scrub_slots_converges_past_a_torn_registration_of_the_kind_it_reclaims`
  green, the vacuity the replacements close.
- **Schema 4 stays unreachable in production, and the legacy path is unchanged**
  (`c6/frozen/legacy-activation-61b018bf.txt`): `TOPOLOGY_ACTIVATION` is `Inactive` and `MAX_READABLE_SCHEMA` 3,
  unchanged; no legacy module (`src/workspace.rs`, the legacy engine, `src/main.rs`, `src/rundir.rs`) changed; every
  changed path under `src/` and `effects/` is one this section names.

### 6.4 The whole suite, and residue base against head

**One whole suite at each, alternately, never concurrently** (the `real_docker` tests' container names are shared on
this box), through `c6/tools/suite-run.sh`: `cargo test --all-targets --all-features` through `upstroke-build`, each in
a fresh `TMPDIR` of its own, with every tracked Rust input touched first and a listing taken 60 s in to show the suite
allocates there. Base is `e46b71d3` (#329's code; `c6/suites/base-e46b71d3/`), head is the code commit `61b018bf`
(`c6/suites/head-61b018bf/`), each an archive of its commit, each named on its log's Compiling line.
- **Base:** rc 0; the library passed 3,068, failed 0 and ignored 130 (164.65 s), the binary's 10 passed; 63 entries in
  its `TMPDIR` 60 s in.
- **Head:** rc 0; the library passed 3,098, failed 0 and ignored 132 (110.42 s), the binary's 10 passed; 64 entries in
  its `TMPDIR` 60 s in. By name, the 30 more passing are this change's new tests, and the 2 more ignored are its two
  child helpers (`instance_kill_child`, `late_add_child`), which its tests spawn; no base test is missing at head
  (`c6/suites/names-diff.txt`).
- **The frozen census** (`c6/suites/frozen-census.txt`, from `c6/tools/frozen-census.py`): the same at both: `recover`
  226, `integrate` 20, `repair` 5, `finalize` 5, `fold` 192 and `events::log` 47 passed, with the legacy `engine::tests`
  188 result lines and `workspace::tests` 47, none failed (the `engine::tests` count includes an ignored helper's own
  result lines, as #329's record §9.7 explains); the seven instrument censuses passed at both.
- **The residue** (`c6/suites/residue.txt`, from `c6/tools/residue-compare.py`): both suites left the same 44 top-level
  entries and 154 in all in their fresh `TMPDIR`, class by class; no class differs, so the new tests leave nothing
  behind.

### 6.5 Platform notes

- **Linux** is executed here: the box's Git 2.43.0, and under a Git 2.55.0 built from its tag
  (`~/orch-pr11/logs/pr11_fub_design6/gits/2.55.0/bin/git`) T-CFG1's base witness and its mutation, and at the code
  commit this change's tests with naming's, `ulid`'s, `seams`' and the frozen finalize tests: 58 passed, none failed
  (`c6/git255/`; a first attempt reused another tree's binary and is void, `c6/git255/void-1/VOID.txt`). T-CFG1 needs
  2.55.0: 2.43.0 predates `worktree.useRelativePaths`, and the test asserts nothing there.
- **Windows and macOS are compiled and linted here, never run** (`c6/platform/at-61b018bf/`, rc 0 each): `cargo clippy
  --target x86_64-pc-windows-msvc` and `--target aarch64-apple-darwin`, `--all-targets --all-features -D warnings`, and
  `cargo +1.85.0 check --target x86_64-pc-windows-msvc --locked --all-targets --all-features` with `-D warnings`. The
  two Windows tests — `a_git_dir_over_the_byte_budget_is_refused_before_any_registry_access_on_windows`, the native
  boundary (217 ASCII characters and two `é` refused before the registry phase, 220 bytes added), and
  `an_earlier_instance_held_open_refuses_the_reclaim_until_its_handle_closes` — and the budget guard itself run only on
  CI's `test (winguest)` leg. The filter, helper and late-add witnesses and P-3 are `cfg(unix)`. CI is the truth for
  both platforms.
- **Windows' `$GIT_DIR` budget, by arithmetic over PR11's measured layout**
  (`~/orch-pr11/logs/pr11_impl_g/measure/pathbudget-child-temporary.txt`, §4.2's `c4/pathbudget/tag-length-r4.txt`): U
  adds 13 bytes. The tightest class, the finalization kill child at width three, was 207 with the fixture tag
  `interleaving-finalize-kill` (26); that tag is now `finalize-kill` (13), so 207 under U. The closure kill child's
  `coordinator-closure-kill` (24) is `closure-kill` (12): 205 becomes 206. The container coordinator child's
  `coordinator-child` (17) is `coord-child` (11): 203 to 205 become 210 to 212. The frozen test files' fixtures stay at
  most 214, and 219 over-inclusively (§4.2, `c4/pathbudget/frozen-fixture-tags.txt`). Every class is within 220; the
  guard refuses any that is not, by name. A guest measurement of the class, as PR11 made, is CI's `test (winguest)` run
  of the whole suite under the guard.

### 6.6 What stays open, under this implementation

As §5.8 leaves them, with the implementation's changes:

| Residual | State at this head | Owner |
|---|---|---|
| **DESC** | Slot-reuse routes closed (§6.1, §6.2); the finding stays open for the rows below, updated in its file | — |
| **R-REF** | Unchanged; filed, `deferred` (`PR330-A-DEAD-COORDINATORS-WINDOWS-REF-WRITE-CAN-LAND-AFTER-ITS-RESUME-RECLAIMED-THE-LOCK`) | O7 (D5) |
| **R-UR and FUC-D5-ACCOUNT** | The final sweep built; the window after its scan stays, unaccounted (`FUC-D5-ACCOUNT`, filed) | O4 (D1) |
| **R-GU** | Unchanged; #329's `PR329-AN-EXTERNAL-PRUNE-DELETES-AN-ENGINE-WORKTREES-REGISTRATION` | O1 |
| **R-G2** | Unchanged; R-G1 closed by U (`FUC-D2-RG`, filed) | follow-up D (O8), O10 |
| **FUC-D5-GITINDEXFILE** | Unchanged; filed with `FUB-D9-ENV`, whose protocol route this change's binding closes | O3 or O3-R |
| **The two frozen oracles** | Replaced and demonstrated (R-O1 to R-O3, M-O1, M-O2) | closed |
| **RERERE, CONFIG** | The switches built (T-RR1, T-RR2, T-CFG1) | closed |
| **Q's items** (PGIDREUSE, the sentinel fallback, observation identity) | Q is not built; under U they do not arise. Filed at repair round 3, each guarded on O5 selecting Q (§6.10) | O5, if the owner chooses Q |

### 6.7 Findings at this touch

- **Filed:** `FUC-D2-RG` (P1, R-G2, follow-up D's), as
  `findings/P1_correctness_202610031557_legacy-started-maintenance-prunes-a-registration-in-its-add-window.md`; #330's
  ledger had carried R-G as a `deferred` row with no file since design round 3. And `FUC-D5-ACCOUNT` (P2, R-UR's
  accounting, O4's), as
  `findings/P2_docs-contract_202610031557_a-late-instance-after-the-final-sweep-has-no-accounting-class.md`.
- **Extended:** `FUB-D9-ENV` (#329's,
  `findings/P1_correctness_202610031425_the-managers-git-children-inherit-the-coordinators-git-repository-context.md`)
  with FUC-D5-GITINDEXFILE: its evidence (`~/orch-pr11/reviews/review-330-d5-triage.md:22` and `:62`), the consequence
  that it defeats U's instance isolation, and its owner (O3 or O3-R); #330's ledger row maps to that file.
- **Updated, kept open:** the DESC finding, with what this change closes and why it stays open.
- **Deleted as fixed:** `PR128-RESIDUE-UNREACHABLE-OBJECTS-IGNORES-THE-EXIT-STATUS`, by
  `a_failed_fsck_is_an_error_naming_the_command_and_never_an_empty_listing` and its mutation.
- **Filed at repair round 3** (corrected at repair round 3; §6.10): Q's three open items, `FUC-D2-PGIDREUSE` (P1),
  `FUC-D3-SENTINELFALLBACK` (P2) and `FUC-D3-OBSERVE` (P2), each with its original severity and `reviewed_sha` and a
  guard conditional on the owner's O5 selecting Q. Q's machinery does not exist in the code, and under U they do not
  arise (§3.3.3, §4.9, §5.8). This section first left them unfiled, their rows `rejected` as not relevant to this change
  as built; the implementation review held that every open finding of the design reviews is filed (C-I3).

### 6.8 Notes for the owner and the reviewers

- **E-FUC-3's item 3, `at_run_end.NoRunFinished`, holds as written** (corrected at repair round 3; §6.10). When a
  fresh process's resume recreates an open generation (`verify_or_recreate`, `src/engine/topology/dispatch.rs`), it
  removes every instance's checkout and registration, then every instance's intent, and only then writes the resuming
  incarnation's own intent and adds its instance (`create_worktree`). So "on resume every earlier incarnation's instance
  and intent is reclaimed" is true of the intents too, for an open generation the resume recreates as for every slot it
  reclaims, before any admission, as §4.5 requires: one intent of the generation after each of three resumes, where
  `83516466` left 2, 3 and 4. This bullet first said the earlier intent stayed until the slot's next intent removal and
  offered the owner a follow-up or a narrowed clause; the implementation review held that neither was right (C-I2), and
  the erratum is neither narrowed nor deferred.
- **The final sweep's proof.** Its removals pass `WriterProof::NoWriterAlive`, the proof terminal finalization's own
  scrub already passes for every slot (`finalize.rs:240-259`), and run inside `remove_execution_root`, before the root's
  emptiness check, so the frozen finalizer's call (`finalize.rs:163`) is unchanged. A dead incarnation's orphaned Git
  writer is outside what that proof speaks for: that is R-UR's window (§5.4).
- **Discovery reads the registry, and refuses as #329's removal scan refuses.** `intents()` reads every registration's
  `gitdir` through #329's tolerant access. An entry whose `gitdir` cannot be decoded, which #329's forced removal
  already refuses after the access's deadline, now refuses every walk the same way. No discovery path runs `git worktree
  list`.
- **Nothing is retained** (§4.5). A removal of an instance that fails refuses the whole command resumably, as any
  removal does; the next walk retries it. On Windows, an instance a process outside the dead job holds open refuses each
  resume until the handle closes (R-1W, corrected in place).

### 6.9 Repair round 2: CI's Windows leg, the platform audit, and the merge wording

**What this subsection is.** The work of `pr11_fuc_impl2` (`claude-opus-5-5`, `max`), a fresh repairer the PR11
orchestrator spawned on this branch at `1fc0c911` under `~/orch-pr11/briefs/pr11_fuc_impl2.md`. Its scope is CI's red
legs on `1fc0c911` and the merge wording, and nothing else. Its evidence is under `~/orch-pr11/logs/pr11_fuc_impl2/`,
cited as `c6r2/…`. §6.8's E-FUC-3 item-3 point and Q's three `rejected` rows (§6.7) are left as they are: the
implementation review assesses both first.

**CI on `1fc0c911`** (run 37137405579; `c6r2/ci/ci-summary-37137405579.txt`, each job's log beside it):
- **Green:** `lint`, `lint (windows)` and `lint (macos)`; the three `msrv (Rust 1.85, …)` legs; `test (ubuntu-latest)`;
  `test (macos-latest)`, whose library passed 3,029 and ignored 104; and the pull-request policy run.
- **Red:** `test (winguest)` (job 111244637558): the library passed 2,869, failed 1 and ignored 88. The failure is
  `workspace_manager::tests::a_worktree_whose_killed_child_is_still_closing_is_removed_not_refused`, which panicked at
  `src\workspace_manager\tests.rs:928:55` with "plant the file: Os { code: 3, kind: NotFound, … }". `upstroke-ci`
  failed on that leg alone ("TEST_WINDOWS did not succeed").

**The cause is the test's fixture, not the product.**
- The test is `cfg(windows)` and pre-existing. Its text is byte-identical at master `5c222ff2`, at #329's head and at
  `1fc0c911` (`c6r2/rootcause/the-test-at-master-and-1fc0c911.txt`), and it never ran in this change's local gates.
- It added the slot's worktree and then spelled the worktree's path as `execution_root().join(slot.relative())`, the
  slot's untagged name. Under U the add makes this incarnation's instance, `tasks/kalpha-g1_<tag>` (§4.2), and nothing
  makes the untagged directory, so writing the held file failed with Windows' error 3, the path not found.
- The product does what U specifies. `add_worktree` returns `slot_path` (through `slot_target`), and a held checkout's
  removal fails as `Filesystem { operation: "remove" }` naming the contained instance, the mapping #329's head has
  (`c6r2/rootcause/add-return-and-removal-error.txt`). Once the file is planted in the instance, both cases assert
  against the checkout the removal acts on: the closing case its attempt count across that checkout's removal and its
  absence after, the held case its refusal naming it. Whether they pass is the Windows test leg's to say.

**The fix** (`3e891d55`, test-only): the test takes the path `add_worktree` returns. Its assertions are unchanged. It
runs only on the Windows test leg, `test (winguest)` on a pull request and `test (windows-latest)` in the merge queue,
which is the truth for it.

**Reproduced on Linux, and pinned there** (`c6r2/tools/r2-campaign.py`, results in `c6r2/mutation/`). A probe, never
committed, runs the test's fixture under `cfg(unix)`:
- **Control** (`r2-control`). Round 1's spelling refuses the plant with `NotFound` (os error 2, Linux's code for the
  same absence), and the untagged path does not exist. This round's spelling plants inside the instance, and the
  removal takes the instance with the file. With
  `a_successors_reclaim_removes_an_earlier_incarnations_instance_and_never_verifies_it`, the T-TAG test and the Unix
  removal-seam pin `a_removal_records_the_one_attempt_the_unix_arm_makes`: 4 passed.
- **Mutation** (`r2-m-add-returns-untagged`). `add_worktree` returns the untagged path, the one the old spelling
  assumed. It is killed: the probe and `a_successors_reclaim_…` fail on the add's return (2 failed, 2 passed). That test
  is the cross-platform pin of what the Windows test now relies on.

**Every platform-gated test, audited for the same derivation** (`c6r2/audit/`; tools `expand.sh`, `rsitems.py`,
`platform-diff.py`, `audit-filter.py` and `audit-show.py` in `c6r2/tools/`):
- **Method.** The library's test build is macro-expanded and never run (`-Zunpretty=expanded`), for
  `x86_64-unknown-linux-gnu`, `x86_64-pc-windows-msvc` and `aarch64-apple-darwin`. Expansion resolves every `cfg`
  attribute and `cfg!` branch, so the items whose text differs between the three builds, or that only some compile, are
  exactly the test build's platform-dependent code, whether the gate is on the item, its module or a branch inside it.
- **Inventory** (`c6r2/audit/at-1fc0c911/summary.txt`). 3,230 tests on Linux (132 ignored), 2,958 on Windows (88) and
  3,133 on macOS (104): the counts the Windows and macOS legs ran, and the local gates' 3,098 passed and 132 ignored
  (`c6/gates/at-1fc0c911/03-test.log`). 458 tests are not run on all three targets, and 1,388 items differ between them.
- **Filter.** Every such item whose text matches a pattern for spelling a slot, snapshot or worktree path or name from
  an untagged name (`.relative()`, `.intent_name()`, a slot's `.id()`, `execution_root().join(`, literal namespace and
  `k<key>-g<gen>` names, `SnapshotName`), or for planting or reading Git state by hand (the registry store, `gitdir`,
  `commondir`, `locked`, `git worktree`, `registration_of`, the fixtures' `git` helpers). It matched 43 items, and each
  was read (`c6r2/audit/at-1fc0c911/audit-show.txt`).
- **Result** (`c6r2/audit/at-1fc0c911/audit-classification.txt`). **One derived a slot path from an untagged name: this
  test.** Thirteen render the manager's instance (`slot_path`, `intent_path`, the path the add returns, its
  `git_dir_of`). Six name only a namespace directory or the store. Twenty-one touch no slot: among them a container
  view, a host-runner `gitdir` fixture, the legacy engine's repository shapes, the crate's own sources and the fixtures'
  repository setup. One is production code master already has, the registration read whose Windows arm reads a held
  marker.
  `runner::container::exec::tests::real_docker_a_worktree_binary_cannot_shadow_the_certified_cli` checks out a worktree
  by hand at `tasks/kalpha-g0` as a container's working directory. No manager API reads that path, so U does not reach
  it, and it is unchanged.
- **At `3e891d55`** (`c6r2/audit/at-3e891d55/summary.txt`): 42 flagged, none spelling `.relative()`. The Linux and
  macOS expansions differ from `1fc0c911` only in libtest's line numbers.

**Windows and macOS are compiled and linted here, never run** (`c6r2/platform/at-3e891d55/`, rc 0 each):
- `cargo clippy --all-targets --all-features -D warnings` for `x86_64-pc-windows-msvc` and `aarch64-apple-darwin`;
- `cargo check --locked --all-targets --all-features` for `x86_64-pc-windows-msvc`, on stable and on 1.85.0, with
  `-D warnings` passed inside the wrapper (`rustflags-arrival.txt`: it arrives).

**The frozen proof at `3e891d55` is zero**: 34 of 34 against master and against #329's head, and Part 2 is master's
four paths (`c6r2/frozen/frozen-proof-3e891d55.txt`). No legacy module or activation constant changed
(`c6r2/frozen/legacy-activation-3e891d55.txt`).

**The merge wording, corrected** (the orchestrator's note at the end of round 1's handover,
`~/orch-pr11/handovers/pr11_fuc_impl.md`, under `~/orch-pr11/ORCH-PR11.md` §1):
- **Touching an instrument does not make this merge the owner's.** The two instruments this change moves,
  `effects/wrappers.toml` and `src/runner/contract.rs`'s payload census, are first-limb changes. The owner's standing
  direction for PR11 gives the merge to the orchestrator and keeps only `MAINTAINING.md` step 7's second limb, and this
  change touches no second-limb path (`c6r2/merge/limb-paths.txt`).
- **The merge action is the orchestrator's,** under that direction's bar, once the genuine prerequisites are met. They
  are the owner's decisions:
  - E-FUC-3's adoption;
  - O4;
  - O7, or R-REF's disposition;
  - the O3 or O3-R route for FUC-D5-GITINDEXFILE;
  - #329 merged.
- **No other merge permission is needed.**
- **Corrected in place**, each marked "corrected at repair round 2": §1.9, §2.7.4, §3.3.6 and §6.1.

### 6.10 Repair round 3: the implementation review's C-I1 to C-I5

**What this subsection is.** The work of `pr11_fuc_impl3` (`claude-opus-5-5`, `max`), a fresh repairer the PR11
orchestrator spawned on this branch at `83516466` under `~/orch-pr11/briefs/pr11_fuc_impl3.md`. Its scope is the
orchestrator's triage of the implementation review of `83516466` (`~/orch-pr11/reviews/review-330-i1-triage.md`), items
C-I1 to C-I5, and nothing else. Its evidence is under `~/orch-pr11/logs/pr11_fuc_impl3/`, cited as `c6r3/…`.
- **Not in this round, by the brief.** #329's newer head is not merged: C takes B's final repaired head in a later step,
  with history intact. E-FUC-3 and O4 stay unadopted, the packet is untouched, accounting is unchanged, O7's method
  stays stopped, and no frozen file changes (below).
- **The commits.** `a417333e` (`fix(workspace)`, C-I1 and C-I2), `c961ab44` (`test(workspace)`, C-I4 and C-I5, in
  `src/workspace_manager/tests.rs` alone), and this record's commit with the three findings (C-I3). The two code commits
  were split from one working tree, and the split was proved by recomposition: the second's `tests.rs` hashes to the
  working tree's, and the range's diffstat is the tree's (`c6r3/commits/split-B.txt`).

**CI on `83516466`** (`c6r3/ci/summary-37152638617.txt`, `c6r3/ci/runs-at-83516466.txt`): run 37152638617, green on
every leg.
- `lint`, `lint (windows)`, `lint (macos)`, the three `msrv (Rust 1.85, …)` legs and `upstroke-ci`: success.
- `test (ubuntu-latest)`: the library passed 3,098 and ignored 132. `test (winguest)`: 2,870 passed and 88 ignored, the
  closing-handle control round 2 fixed among them. `test (macos-latest)`: 3,029 passed and 104 ignored.
- The pull-request policy run 37152638625: success. Two runs at the same head (37152638412, 37152638423) were cancelled
  by the body edit that followed the push.

**The review** (`~/orch-pr11/reviews/review-330-i1-{regular,regression}-83516466.review.md`, hashed in
`SHA256SUMS-330-i1-lenses`; their witnesses in `330-i1-witnesses/`, hashed in `SHA256SUMS-330-i1-witnesses`). Both
lenses returned CHANGES_REQUIRED, with one P1. The triage combines them into five items, each carrying a witness or a
filing or standards duty, so each is fixed.

#### C-I1 (P1): a torn instance's repair removes that instance alone

- **Reproduced first** (`c6r3/witness/r3-wit-regular-at-83516466/`). The regular lens's witness patch over `83516466`
  (`review330-witness.patch`, sha256 `95a951a7…`, the orchestrator's copy: `c6r3/witness/inputs.sha256`):
  `review330_torn_dead_instance_repair_preserves_the_live_instance` fails, printing `dead_registration=false
  live_checkout=false live_registration=false paid_edits=false`, and its control with a whole earlier registration
  passes.
- **The cause.** `torn_plan` reported a logical slot when any instance of it was torn, and `repair_torn_registrations`
  then ran `remove_worktree_proving` on that slot, which removes every instance. The sequence: the successor runs
  `alpha` and writes its paid edits; a dead incarnation's late add recreates its own instance of `alpha`, with no
  intent, and is left torn; the successor retires the unrelated slot `beta`, and `beta`'s intent removal, revalidating
  through Git's enumeration, runs the repair, which removed the successor's instance of `alpha` with the dead one.
  Distinct names did not keep the successor's output: the repair crossed the instance boundary.
- **The fix** (`a417333e`, `src/workspace_manager.rs`). The plan, `instances_with_torn_registrations` (formerly
  `slots_with_torn_registrations`) over `torn_plan`, names each torn instance by its slot and its tag, and every torn
  instance of a slot, not the first only. The repair removes each through `remove_instance_proving`: that instance's
  registration bound by the removal's scan, in a tolerant registry access with no hold, then its checkout and that
  registration removed by `remove_bound` inside one execution of the slot's removal site. No other instance is bound or
  touched. The final sweep removes each earlier instance through the same helper, in the same order as before (the
  binding under `WriterProof::NoWriterAlive`, then the funnel). A slot's retirement, `remove_worktree_proving`, and
  terminal finalization still remove every instance, as U requires. The docs of the repair, its plan, `remove_intent`,
  `verify_worktree` and the registry lock say so, and DESIGN §15 gains the sentence.
- **The test:** `a_torn_earlier_instances_repair_leaves_the_successors_live_instance_of_its_slot`, two shapes in turn.
  First a whole earlier registration, the control: nothing is repaired, and `beta`'s retirement touches no instance of
  `alpha`. Then a torn one: the repair removes the dead instance's checkout and registration, and the successor's paid
  edits, registration and intent stay, its instance still verifies, and Git enumerates again.
- **Red on its first-bad shape** (`c6r3/mutation/r3-firstbad-83516466-code/`): this head's tests over `83516466`'s
  `src/workspace_manager.rs` and `src/engine/topology/dispatch.rs`
  (`c6r3/mutation/inputs/firstbad-83516466-code.patch`). It fails at "torn: PAID EDITS LOST: the successor's checkout
  was removed", after its whole shape, the control, passed. All three of the row's tests are red there: this one, the
  C-I2 test and P-1.
- **Killed by its mutation** `m-torn-whole-slot`, the repair removing the whole slot again
  (`c6r3/mutation/m-torn-whole-slot/`): killed, 1 red
  (`a_torn_earlier_instances_repair_leaves_the_successors_live_instance_of_its_slot`), 59 passed.

#### C-I2 (P2): a resume's recreate reclaims every earlier intent before its replacement

- **Reproduced first** (`c6r3/witness/r3-wit-regression-at-83516466/`). The regression lens's probe over `83516466`
  (`probe.rs`, sha256 `5016e113…`, appended as `c6r3/witness/regression-probe.patch`): three fresh incarnations resuming
  one open generation through `verify_or_recreate` leave 2, 3 and 4 intents (`left: [2, 3, 4]`, `right: [1, 1, 1]`). The
  regular lens's witness at the same head reports 1, 2 and 3 earlier intents surviving.
- **The cause.** `verify_or_recreate` (`src/engine/topology/dispatch.rs`) removed every instance's checkout and
  registration (`remove_worktree`) and then created the replacement (`create_worktree`: this incarnation's intent, then
  its add). Under per-incarnation instances an intent is its creator's own file, so each resume left the earlier
  incarnations' intents of the generation in place, against E-FUC-3's item 3 ("on resume every earlier incarnation's
  instance and intent is reclaimed") and §4.5's reclamation of every durable record of a dead instance before admission.
  §6.8 had disclosed it and offered the owner a follow-up or a narrowed clause; the review held that neither was right.
- **The fix** (`a417333e`). The recreate removes every instance's intent (`remove_intent`) between the removal of the
  worktrees and the creation: the walks' own reclaim of the slot, worktree then intent, followed by intent then add. In
  a live process the only intent is its own, which is removed and written again. Recovery's `recreate_open_no_attempt`
  runs it before `run_resumed`, so the reclaim precedes admission. `docs/internals/engine/topology/dispatch.md` replaces
  "The intent is re-written rather than removed and re-written" with the reason, and DESIGN §15 says that a resume
  reclaims each open generation's earlier instances, intents included. E-FUC-3 is neither narrowed nor deferred, and
  §6.8 is corrected in place.
- **The tests.** `every_resume_that_recreates_an_open_generation_leaves_one_intent_its_own`: the dispatching
  incarnation's instance with its intent, then three fresh incarnations through the production `verify_or_recreate`; the
  counts are asserted first, 1, 1, 1, then that each resume's census holds only its own instance and intent, and that
  nothing of the dispatching incarnation's instance is left. P-1
  (`p1_after_every_walk_only_the_current_incarnations_instances_remain`) recreates its open generation through
  `verify_or_recreate`, as recovery does, and reclaims every other slot as the walks do; it had simulated the resume by
  the walks' reclaim of the open slot too, which removes intents. The census (`instance_census`) gains a third half, the
  intents directory, which `assert_only_own_instances` reads, and P-2's final check reads it as well.
- **Red on its first-bad shape** (`c6r3/mutation/r3-firstbad-83516466-code/`):
  `every_resume_that_recreates_an_open_generation_leaves_one_intent_its_own` fails with `left: [2, 3, 4]`, `right: [1,
  1, 1]`, and P-1 fails after the second incarnation's walk, the first incarnation's intent of the open generation
  (`tasks.k0-g1_<its tag>.intent`) left beside the second's.
- **Killed by its mutation** `m-recreate-keeps-intents`, the `remove_intent` call deleted
  (`c6r3/mutation/m-recreate-keeps-intents/`): killed, 2 red
  (`every_resume_that_recreates_an_open_generation_leaves_one_intent_its_own`,
  `p1_after_every_walk_only_the_current_incarnations_instances_remain`), 53 passed.

#### C-I3 (P2): Q's three open findings, filed

Each in `findings/README.md`'s form, with its original severity, `reviewed_sha` and location (those of its ledger row),
`deferred`, and a guard conditional on the owner's O5 selecting Q; Q is not implemented:
- `FUC-D2-PGIDREUSE` (P1, `a9be94bc`):
  `findings/P1_correctness_202610040118_a-delayed-git-child-joins-a-reused-writer-group-under-q.md`;
- `FUC-D3-SENTINELFALLBACK` (P2, `a0464f43`):
  `findings/P2_liveness_202610040118_the-sentinels-sigkill-fallback-leaves-its-zombie-in-the-writer-group-under-q.md`;
- `FUC-D3-OBSERVE` (P2, `a0464f43`):
  `findings/P2_correctness_202610040118_q-reads-the-resources-of-the-checkout-this-process-last-held.md`.

Their ledger rows change from `rejected` to `deferred` and map to these files. Each file's failure sequence is its
design lens's, and its remedy is §4.9's.

#### C-I4 (P2): the DESC witnesses' shell words

- **Reproduced first**, at `83516466`, under three temporary directories of this session's
  (`c6r3/witness/r3-c-i4-{space,apostrophe-only,apostrophe}-at-83516466/`): `…/temp with spaces`, `…/apos'trophe` and
  `…/it's temp`. Under each, both DESC witnesses fail before they measure isolation: the helper route at "the filter
  started its helper", the filter route at "the dead add's checkout reaching its filter did not happen within 60s".
- **The cause.** Git hands a filter's command to the shell, and `filtered_commits` installed `sh <script path>`
  unquoted, so a space split the path and an apostrophe left a quote open; both scripts also assigned their directory as
  `dir='<path>'`, which an apostrophe ends.
- **The fix** (`c961ab44`): `sh_quoted` renders a path as one POSIX shell word, single-quoted with each `'` closed,
  escaped and reopened (`'\''`), and both the filter's command and both scripts' `dir=` take it. Git's filter interface
  takes shell text, so the path is handed to it as one quoted word rather than spliced raw (standards §9).
- **This change's tests pass under each directory** (`c6r3/mutation/r3-control-{space,apostrophe,both}/`): 55, 55 and 55
  passed, none failed, the DESC witnesses, P-1 to P-3, both new tests, naming's, `ulid`'s and `seams`' among them.
- **Each half is necessary**, each mutation run under the directory that bites it:
  - `m-filter-unquoted-space`, the filter's command unquoted, under the space: killed, both DESC witnesses red;
  - `m-quote-no-escape-apostrophe`, `sh_quoted` without the escape, under the apostrophe: killed, both DESC witnesses
    red;
  - `m-dir-unescaped-apostrophe`, both scripts' `dir='<path>'` back (`c6r3/mutation/inputs/dir-unescaped.patch`), under
    the apostrophe: killed, both DESC witnesses red.
- **They still fail on their first-bad shapes there.** Every incarnation rendering one tag (`m-tag-shared`), under the
  space and the apostrophe: both witnesses are red under each (`c6r3/mutation/m-tag-shared-space/`,
  `c6r3/mutation/m-tag-shared-apostrophe/`), each fails at "PAID EDITS LOST: the dead incarnation's writer reached the
  successor's checkout", the defect it witnesses, and no longer before its filter starts. And at base, #329's code with
  this head's tests laid over it (`c6r3/tools/basewit-c.py`, round 1's form; its test-only shim now answers the tag by
  reference), under all three directories (`c6r3/basewit/desc-at-base-{space,apostrophe,both}/`): both fail at the same
  assertion under each, "PAID EDITS LOST" (`c6r3/basewit/summary.txt`).

#### C-I5 (P3): no unannotated `unreachable!`

`instance_kill_child` ended in `unreachable!`, which standards §7 denies in tests without a per-site `#[expect]`. It now
fails with `panic!`, naming the premise ("the add's funnel returned past the kill armed at its … phase"), as
`dispatch_kill_child` does. P-2's oracle is unchanged: it requires each child's death by abort (`died_by_abort`), so a
kill that stopped killing still fails it, the child now exiting by the panic. Executed: with the child's
`Injection::Kill` made `Injection::Proceed` (`m-kill-child-proceeds`, `c6r3/mutation/m-kill-child-proceeds/`), killed, 1
red (`p2_instances_of_killed_incarnations_in_other_processes_are_reclaimed_by_the_resume`), 0 passed: "the inc-a child
died by abort at its before phase", the child having exited 101 (`unix_wait_status(25856)`). P-2 passes in every control
above.

#### Round 1's mutations, run again on this head

The rows round 1 ran (§6.2) whose tests this round changes or adds to: the torn plan and the final sweep, the census,
P-1, P-2, the DESC witnesses and R-O1 to R-O3. Each is round 1's exact substitution, which still matches once
(`c6r3/tools/r3-campaign.py`), run over this round's tests:

| Mutation | Red on this head | Of them, this round's new tests | Log |
|---|---|---|---|
| `m-tag-shared` | 13 of 55 | the C-I1 test, the C-I2 test | `c6r3/mutation/m-tag-shared/` |
| `m-disc-none` | 2 of 55 | neither | `c6r3/mutation/m-disc-none/` |
| `m-disc-no-registry` | 2 of 55 | neither | `c6r3/mutation/m-disc-no-registry/` |
| `m-disc-no-namespace` | 1 of 55 | neither | `c6r3/mutation/m-disc-no-namespace/` |
| `m-own-not-excluded` | 3 of 60 | neither | `c6r3/mutation/m-own-not-excluded/` |
| `m-remove-own-only` | 13 of 55 | the C-I1 test, the C-I2 test | `c6r3/mutation/m-remove-own-only/` |
| `m-intent-own-only` | 10 of 55 | the C-I2 test | `c6r3/mutation/m-intent-own-only/` |
| `m-retain` | 1 of 55 | neither | `c6r3/mutation/m-retain/` |
| `m-o1-stranded-registration` | 15 of 60 | the C-I2 test | `c6r3/mutation/m-o1-stranded-registration/` |
| `m-o2-earlier-registration-left` | 14 of 60 | the C-I1 test, the C-I2 test | `c6r3/mutation/m-o2-earlier-registration-left/` |
| `m-fin-no-sweep` | 2 of 55 | neither | `c6r3/mutation/m-fin-no-sweep/` |
| `m-torn-own-only` | 2 of 60 | the C-I1 test | `c6r3/mutation/m-torn-own-only/` |

Every row is killed. The failing tests of every row of this round are listed in `c6r3/mutation/campaign-r3-summary.txt`.

**The reviewers' witnesses on this head** (`c6r3/mutation/r3-reviewer-witness-final/`,
`c6r3/mutation/r3-regression-probe-final/`): the regular lens's three tests pass (3 passed: the resume reclaims every
earlier intent, the torn repair keeps the live instance, and the control), and the regression lens's probe passes (1
passed, its counts 1, 1, 1).

**The control on this head** (`c6r3/mutation/r3-control/`): the same tests with the frozen finalize tests, 60 passed,
none failed.

#### The frozen set, the platforms and the gates

- **The frozen proof is zero at `c961ab44`** (`c6r3/frozen/frozen-proof-c961ab44.txt`): 34 of 34 byte-identical against
  master `5c222ff2` and against #329's head `54a1ff14`; Part 2 is master's four paths (+241 −74). This record's commit
  changes no frozen path, and the pull request's body gives the proof at its head. No legacy module or activation
  constant changed (`c6r3/frozen/legacy-activation-c961ab44.txt`); `src/engine/topology/dispatch.rs`, which is not
  frozen, is the one production file beyond round 1's. D4 did not trigger: no frozen test changed, and the frozen
  finalize tests pass in the control above; the whole suite's run is the gates'.
- **Windows and macOS are compiled and linted here, never run** (`c6r3/platform/at-c961ab44/`): `cargo clippy
  --all-targets --all-features -- -D warnings` for `x86_64-pc-windows-msvc` and `aarch64-apple-darwin`, and `cargo check
  --locked --all-targets --all-features` for `x86_64-pc-windows-msvc` on stable and on 1.85.0 with `-D warnings` passed
  inside the wrapper (`rustflags-arrival.txt`: it arrives), rc 0 each, with the host's 1.85.0 check under the same flag.
  The two new tests are not platform-gated, so they run on every CI leg; `sh_quoted` and the DESC witnesses are
  `cfg(unix)`. CI is the truth for Windows and macOS.
- **The ten gates** run at the head that carries this record; the pull request's body gives them.

#### Corrected in place

§0, the header, §6.1 (the torn plan), §6.6 (Q's row), §6.7 (Q's findings, filed) and §6.8 (E-FUC-3's item 3, which now
holds as written), each marked "corrected at repair round 3".
