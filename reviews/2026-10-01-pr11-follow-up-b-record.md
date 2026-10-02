# PR11 follow-up B — linked checkouts' race on the shared worktree registry: the working record

The record of the change that repairs `PR11-LINKED-CHECKOUTS-RACE-THE-SHARED-WORKTREE-REGISTRY`
(`findings/P1_correctness_202610011002_linked-checkouts-race-the-shared-worktree-registry.md`, P1,
`deferred`, `pre_existing`). It is kept on the branch so that a successor session inherits what was
decided and why. Like the PR11 record (`reviews/2026-09-30-pr11-record.md`), it is **not** a design
document. `DESIGN.md` and the packet stay the authority, and a sentence here that disagrees with
either is a defect in this file.

**Branch.** `fix-P1/correctness_linked-checkouts-race-the-shared-worktree-registry`, cut from master
at `92c4ca81f9209d218df4534ee71d3445dc2906e1`, the merge commit of pull request #327 (PR11). It is a
`fix-P1/` lane: review effort `max`, and every P0 and P1 is fixed before the pull request is ready.

**Why this change exists.** On 2026-10-01 the owner decided #327's two P1s in these words, relayed
by `babysit_pr11`: "yea continue, narrow and split out work". PR11 kept its narrowed scope, and this
finding became an explicit follow-up, due after PR11 and before G6. G6 certifies R17 ("coordinator
lock holds: one coordinator process; second coordinator refused") under concurrency. The finding is
not reclassified, and G6 is not waived. The orchestrator's brief is
`~/orch-pr11/briefs/followups/fu-b-cross-process-worktree-registry.md`.

**Who writes it.** The design phase (§1) is `pr11_fub_design`'s (`claude-opus-5-5`, `max`), a fresh
session `orch_pr11` spawned on master `92c4ca81`; its figures are under
`~/orch-pr11/logs/pr11_fub_design/`, cited by paths relative to that directory. **Design round 2**
is `pr11_fub_design2`'s (`claude-opus-5-5`, `max`), spawned on `dfd69410` to answer design review
round 1 (§1.11); its figures are under `~/orch-pr11/logs/pr11_fub_design2/`, cited as `d2/…`. **Design
round 3** is `pr11_fub_design3`'s (`claude-opus-5-5`, `max`), spawned on `0874bcf3` to answer design
review round 2 and the looping signal it raised (§2); its figures are under
`~/orch-pr11/logs/pr11_fub_design3/`, cited as `d3/…`. **Design round 4** is `pr11_fub_design4`'s
(`claude-opus-5-5`, `max`), spawned on `8dd2214c` to carry out the orchestrator's decision on design
review round 3: narrow this change to the registry race and split the dead coordinator's Git writers
out (§3); its figures are under `~/orch-pr11/logs/pr11_fub_design4/`, cited as `d4/…`. Every figure
below is in a saved file the sentence names. A fresh implementer writes the code after the design
review, and its sections follow §3.

## 0. Status

| Phase | State |
|---|---|
| Design rounds 1 and 2 (§1) | **Superseded by §2.** Round 1's lock handed to the Git child, and round 2's engine-only lock with a process record and quiescence waits, are withdrawn, with E-FUB-1, R29, Class C and round 2's unfreeze text. §1 is kept as the history the review rounds cite. |
| Design round 3 (§2) | **Superseded by §3 where §2's banner says.** Design review round 3 (three `gpt-6-astra` lenses at `max` on `8dd2214c`, the design lens refused on [cyber] grounds and recast) returned CHANGES_REQUIRED from all three (`~/orch-pr11/reviews/review-329-d3-triage.md`). Its P1s in round 3's lease raised the looping signal a third time. |
| Design round 4 (§3) | **PROPOSED, narrowed.** The registry race only: tolerant registry access (the parse inside the attempt, an exact classifier, one deadline per access, R-X shared by adds and held alone by the torn plan) and targeted removal, all executed on a scratch shape. The dead coordinator's Git writers (DESC) are filed as `PR329-A-RESUME-REBINDS-A-SLOT-ITS-DEAD-COORDINATORS-GIT-CHILD-STILL-WRITES`, P1, for follow-up C before G6. The legacy race is re-graded P1. **Decision B is the owner's** (ESCALATION item 7): §3.10 prepares B1′ and keeps B2′. This head changes no production code; it adds the DESC finding and renames the legacy one (§3.9, §3.10). |
| Implementation | not started. It waits on design review round 4 and, for the legacy wrap only, on decision B. It does not depend on #328 or follow-up C (§3.12). |

## 1. Design

> **SUPERSEDED by §2 (design round 3).** §1 is rounds 1 and 2: a cross-process lock on the registry,
> first handed to the Git child and then held by the engine alone with a record of its children. Its
> facts about Git and the engine (§1.1, §1.2's census, the strace attribution) are still cited by §2;
> its remedy, its erratum E-FUB-1 and its unfreeze text are withdrawn and are not to be adopted. The
> banner it carried is kept below as history.
>
> *Round 2's banner:* **PROPOSED — pending the owner's decisions on erratum E-FUB-1 (with Class C for
> the vocabulary) and the one-change unfreeze of `src/workspace.rs`.** Everything in §1 assumes A1 +
> B1 in their round-2 texts (§1.8, §1.9) and cites neither as adopted. §1.10 says what each
> alternative changes, so a different decision revises one subsection, not the design.

### 1.1 The defect, and what closing it means

**The defect.**
- **Two locks per coordinator.** Two coordinators of one repository, one in the main checkout and one
  in a linked checkout, each hold their own worktree lock. That lock is
  `<worktree git dir>/upstroke-worktree.lock` (`src/rundir.rs:1857`), one per checkout. The legacy
  resume takes it too (`src/engine/resume.rs:148`). Each coordinator also holds R-X, which is a
  process-local mutex (`src/workspace_manager.rs:1592`).
- **The race.** Git writes a registration into the shared `<common git dir>/worktrees/` one file at a
  time. Every enumeration of that store dies on an entry that is half written: `git worktree list`,
  the sibling scan inside `git worktree add`, `git fsck`, and the manager's own scans. `git worktree
  prune` deletes an entry caught between `mkdir` and its `locked`.

**What it costs.** That depends on the pipeline the failing funnel ran in.
- **In an attempt.** The Git error is the pipeline's. The coordinator cancels the other pipelines
  and ends the command resumably (`src/engine/topology/coordinator.rs:1413`).
- **In a verification.**
  - `run::verified` maps the Git error to `Verified::Unavailable` (`src/engine/topology/run.rs:279`).
  - The frozen `integrate.rs` appends `merge_verification_unavailable`, its outcome chosen at `:871`
    and appended at `:881`.
  - That spends one of the candidate's deferrals. At `max_defers` it parks the candidate with an
    unblock question. The other process finishing its write undoes neither.

**What closing it means.** The finding's own words: "a registration another coordinator is half-way
through writing must never reach `run::verified` as foreign Git state". The brief adds: "a pipeline
must never end the command because another coordinator was mid-mutation". Both are statements about
engine processes. §1.5 states exactly what remains outside them.

### 1.2 Every registry access, at `92c4ca81`

**Method.**
- **Where Git starts.** Production code starts Git in exactly two files.
  - `src/workspace.rs` starts it from one builder, `git_command` (`:43`).
  - `src/workspace_manager.rs` starts it from two: the funnels' `command` (`:4994`) and `read_only_git`
    (`:5452`).
  - The census `runner::contract::tests::every_production_process_start_is_classified`
    (`src/runner/contract.rs:1632`) holds that set. Every other process start is a Runner role's (an
    agent, a gate, a reviewer), `docker`, or the macOS reaper's `/bin/ps`.
- **The listing.** Every call site that reaches one of the three builders is listed with its argv:
  - `census/manager-git-sites-92c4ca81.txt`;
  - `census/legacy-git-sites-92c4ca81.txt`;
  - every holder of R-X and every caller of `revalidate()`, in
    `census/manager-registry-holders-92c4ca81.txt`.
- **The strace runs.** Each distinct argv shape was run under `strace -f -e trace=%file`, twice:
  - in a repository with two complete linked registrations;
  - again with a third, torn registration, holding `locked`, `gitdir` and `HEAD` with `commondir`
    empty. That is the state review round 8's witness wrote.

  The runs record:
  - which registrations each command touches beyond its own;
  - whether it opens the store itself;
  - whether it mutates the store;
  - whether it fails on the torn entry.

  Git 2.43.0. The script is `census/strace/registry_census.py` and the results are
  `census/strace/registry-census-2.43.0.{txt,json}`.
- **Direct reads.** Every Rust read of the store was found by reading the code: `join("worktrees")`,
  `read_dir` of the store, and reads of an admin directory's files.

**Table A — the topology (schema-4) path: every registry access, all of them R-X's.**

| Holder (`src/workspace_manager.rs`) | What it does in the store | Torn sibling (measured) | Who runs it |
|---|---|---|---|
| `add_worktree` (`:2649`): `git worktree add --detach --quiet` (`:2708`) under R-X (`:2704`) | writes a new registration file by file (`mkdir`, `locked`, `gitdir`, `commondir`, `HEAD`), checks it out, unlinks `locked`; and enumerates every sibling first | **dies**: "failed to read …/half/commondir: Success" | the coordinator thread for a task worktree (`dispatch.rs:193`) and the staging worktree (`integrate.rs:585`, before `merge_verification_started`); a pipeline thread for a snapshot (`add_snapshot`, `:3160`, from `attempt.rs:1139`, in an attempt or a verification) |
| `remove_worktree_proving` (`:2988`), scan: `revalidate_removal_proving` (`:5119`) under R-X (`:2999`) | reads every entry's `gitdir` and `locked` (`:5143` on) | **refuses**: "… is locked and has no gitdir" (R7's witness, below) | whoever removes a slot: the coordinator (scrub, staging and snapshot reclaim, finalization, recovery, an interrupted attempt's residue at `attempt.rs:516` → `:547`) and a pipeline (`attempt.rs:1144`, a snapshot as each role finishes) |
| `remove_worktree_proving`, mutation: `remove_bound` (`:3020`) under R-X (`:3007`) | removes the checkout, then the registration: `locked` unlinked, or the admin directory removed directly, or `git worktree prune` (`:3059`, `:3098`, `:3121`), which enumerates the store and deletes every entry with neither `locked` nor `gitdir` | prune does not die; it **deletes** another process's entry caught between `mkdir` and `locked` (R7's four-process run: "could not open …/gitdir for writing") | as the scan |
| `worktree_records` (`:5051`): `git worktree list --porcelain -z` (`:5057`) under R-X (`:5052`) | enumerates every entry and reads each one's `HEAD` through its `commondir` | **dies**: "failed to read …/commondir: Success" | every caller of `revalidate()` (`:1709`). That is the gate before nearly every funnel, listed in the census file: `derive` (`:1639`), the intent and execution-root funnels, the three adds, `verify_worktree`, `remove_intent`, `reclaim_intents`, the Object funnels, `candidate_diff`. Also `quiescence` (`:2768`) and `assert_publishable` (`:3434`). Coordinator and pipeline threads both run it. |
| `slots_with_torn_registrations` (`:5345`), the torn plan, under R-X (`:5349`) | for each intent: the removal scan, then the bound entry's `commondir` length | as the scan | the coordinator, from `remove_intent` (`:2269`) and `verify_worktree` (`:2745`) |

**Table B — reaches the store, and is outside the class.**

| Access | Why it is outside |
|---|---|
| `unreachable_objects`: `git fsck --unreachable …` (`:5486`, through `read_only_git`), **not under R-X** | It enumerates every worktree and dies on a torn sibling (measured). In production it is reached only through `candidate::verify_object` (`src/engine/topology/candidate.rs:304`, `:524`), after `classify_object_residue` has found the candidate commit object **absent**: `object_exists` is asked first (`src/workspace_manager/residue.rs:236-243`), and only then does `observed_residue_elements` run `fsck` (`:373`). That path refuses either way (`Refusal::ObjectMissing`, or the Git error), on the coordinator side and resumably, so a torn read changes the refusal's text and not its outcome. Holding the lock there would also make the read-only classifier create a file (§1.3.9). It stays outside, and this table says so. |
| `registration_for` (`:5945`), a direct read of every entry's `gitdir` and `locked` | No production caller. It is reached only through the three add sites' residue classification (`residue.rs:525`), which the kill samplers and tests call. |
| Commands run in the command's own linked checkout: `rev-parse`, `add`, `write-tree`, `cherry-pick`, `read-tree`, `status`, `diff`, `ls-files`, `rm`, `clean`, `commit-tree` | They read or rename files in their **own** registration only (`index`, `HEAD`, `logs`; the census's `rename lk`). No other engine process writes that registration. Prune deletes only an entry with neither `locked` nor a live `gitdir`, so a complete registration whose checkout exists is never another process's to delete. None of them fails on a torn sibling (measured, every row). |
| `src/runner/container/view.rs:76` and `src/rundir.rs:519` | `view.rs` reads the role's own worktree `commondir` to build the disposable view. `rundir.rs` only computes a path (`common_git_dir`, no file read). |
| Auto-maintenance | No production child of either engine starts it. On Git 2.43.0 `git commit` alone spawns `git maintenance run --auto` among the commands probed (`census/strace/gc-probe-2.43.0.txt`), and the legacy `Workspace::commit` (`src/workspace.rs:1019`) has test callers only. A maintenance run the user's own commits start is the user's Git (§1.5). |

**Table C — the legacy (schema 1–3) path: four registry enumerators.** Each command dies on a torn
sibling (measured). They are all built by `git_command` (`src/workspace.rs:43`) and run on the
legacy coordinator's one driving thread.

| Command (`src/workspace.rs`) | Reached from |
|---|---|
| `git worktree add -q --detach --force <path> <commit>` (`add_gate_worktree`, `:871`) | every legacy attempt with gates or reviewers adds a gate and a review snapshot: `engine/attempt.rs:154`, `:178` → `gate_snapshot_for_candidate_in_store` (`:703`); also `gates.rs:647` → `gate_snapshot_for_candidate` (`:695`) |
| `git worktree remove --force <path>` (`cleanup_gate_workspace`, `:1549`) | the snapshots' `Drop` (`:1419`, `:1673`), and resume's reclaim (`reclaim_gate_workspaces`, `:718`, from `engine/resume.rs:426`) |
| `git worktree list --porcelain -z` (`worktree_is_registered`, `:1602`) | `cleanup_gate_workspace`, after the remove (`:1572`) |
| `git switch -q --no-recurse-submodules -- <branch>` (`switch_branch`, `:450`) | the legacy resume (`engine/resume.rs:454`). It enumerates every worktree through Git's `die_if_checked_out`. The census measured it dying on the torn entry, where `switch --create` (`create_branch`, `:441`) does not. |

**The legacy race, measured at the Git level and reasoned in the engine.**
- **Measured.** Two linked checkouts of one repository ran the engines' own argv concurrently
  (`measure/legacy-race.sh`, `measure/classify-race.py`, `measure/legacy-race-SUMMARY.txt`):
  - legacy against legacy, four loops of 300 cycles per checkout: 12 and 16 failed commands in two
    runs of 7,200. Three and six of them died on the **other** checkout's entry: `failed to read
    …/commondir: Success`, `failed to read '…/locked'`, `Invalid path '…'`.
  - legacy against the topology argv: 10 and 13 failed, five and four on the other checkout's
    entry.
  - With one loop per checkout, the shape one coordinator per checkout gives, three runs of 1,800
    commands failed none (`measure/legacy-legacy-run{1,2,3}.log`). The windows are narrow, and the
    four-loop runs are what hits them.
- **Reasoned, the engine.**
  - A legacy gate snapshot add that fails returns through `?` (`engine/attempt.rs:154`, `:178`).
  - The legacy coordinator answers any `run_attempt` error with `workspace.discard_uncommitted()`
    and then ends the command (`src/engine/coordinator.rs:544-548`). That function is `git reset
    --hard HEAD` and `git clean -fd` (`src/workspace.rs:1230`).
  - So a torn read in a legacy attempt discards the worker's paid edits. The resume settles the
    attempt interrupted and runs it again.
  - This race is reachable in production today, between two legacy runs in linked checkouts. After
    PR12 it is reachable between a legacy run and a topology run. A legacy writer then tears a
    topology verification exactly as the finding describes.
- **Not filed.** Under B1 this change repairs it (§1.9). Under B2 it is filed then (§1.10).

**[R2 · WIN] Which process writes which file of a registration, measured.** Every registry command
the engines run was traced again under `strace -f`, attributing each write under
`<common git dir>/worktrees/` to the process that made it (Git 2.43.0,
`d2/measure/subprocess-writes-2.43.0.txt`, the script beside it):
- **`git worktree add`** itself creates the entry and writes `locked`, `gitdir`, a placeholder `HEAD`
  and `commondir`, in that order, and unlinks `locked` last.
- **Its two subprocesses**, each of which it waits for before it goes on:
  - `update-ref HEAD` installs the final `HEAD` by writing `HEAD.lock` and renaming it, and writes
    `logs/HEAD`;
  - `reset --hard` writes `index` and `ORIG_HEAD` the same way, and the checkout's files.
- **`worktree remove` and `worktree prune`:** the top-level process alone, by unlink and rmdir.
- **`worktree list` and `switch`:** no write under `worktrees/`.

So every file an enumeration reads (`gitdir`, `locked`, `commondir`, `HEAD`) is written by the
top-level Git process the engine started. The one exception is `HEAD`'s final value, which a
subprocess installs by an atomic rename. §1.3.3 and §1.5 rely on this.

**What the lock must cover.**
- **Topology:** R-X's four holders, as they stand, with no new holder. That is the add's Git child,
  a removal from its scan to its prune (two critical sections), the list, and the torn plan's scan.
  This is "taken wherever `registry_lock_of` is taken today", the finding's remedy 1.
  - **[R2 · FROZEN]** One caller of the list leaves the set instead of entering it: `derive`, which
    no longer reads the registry at all (§1.3.10).
- **Legacy (B1):** the four commands of table C.
  - **[R2 · PERM]** And the creation of the lock file itself, before any worker runs (§1.3.10).

### 1.3 The remedy: one cross-process lock in the common git dir

#### 1.3.1 Why remedy 1, and not remedy 2

- **Remedy 1** closes the class at its cause: no engine process reads or writes the store while
  another engine process is writing it. It changes no coordinator's admission: two coordinators in
  two checkouts keep running, as R17's worktree lock has always allowed. Its cost is a new resource
  and two new effect sites.
- **Remedy 2** (one topology coordinator per common git dir) would also need a new repository-wide
  file, so it is remedy 1's cost and more:
  - it changes what R17's "second coordinator refused" means, from "in this checkout" to "in this
    repository";
  - it would not exclude a legacy coordinator unless the legacy path took it too, and then it would
    refuse a second legacy run in a linked checkout, which runs today;
  - the per-checkout worktree lock cannot be re-keyed instead, because the legacy resume takes it
    and the frozen `recover.rs` acquires it.

  Remedy 2 is not needed and not proposed.

#### 1.3.2 The file

**Path and name: `<common git dir>/upstroke-registry.lock`.** The common git dir is the canonical
path `git rev-parse --path-format=absolute --git-common-dir` answers. The manager already holds it,
canonicalized (`common_git_dir`, `src/workspace_manager.rs:6330`).
- **Why that directory.** It is the directory the lock guards the child of (`worktrees/`). It is
  also the only directory every process acting on the repository shares. Each linked checkout's git
  dir is its own, and two processes may name different private roots.
- **Why that name.** It parallels `upstroke-worktree.lock`. It is no name Git writes: Git's
  `<name>.lock` files guard a file `<name>`, and there is no `upstroke-registry`.

**Why the user's `.git` may hold it.**
- **Precedent.** The worktree lock already puts `upstroke-worktree.lock` in every checkout's git dir
  that runs a write command (`src/rundir.rs:1857`). For the main checkout that git dir **is** the
  common git dir, so the new file sits beside an existing one.
- **Git leaves it alone.** Measured on Git 2.43.0: `git gc --prune=now`, `git worktree prune`,
  `git fsck`, `git repack -ad`, `git pack-refs --all` and `git maintenance run --task=gc` all leave
  both files in place, and `fsck` reports nothing
  (`measure/git-leaves-unknown-git-dir-files.txt`). Git reads its directory by known names.

**[R2 · WIN, FILTER] Contents: the in-flight record.** Round 1's file was empty and never read.
Round 2's holds, while a registry access is in flight, the record of every process whose writes to
the store that access may still have outstanding: the holder, and each Git child the access starts.
§1.3.3 says who writes it, when, and what the next acquirer does with it. Between accesses it is
empty.

**[R2 · PERM] Creation, and the permission it needs.**
- **Created** by the first registry access, through `Lock.CreateRegistryLockFile`, or earlier, by the
  check each command makes before it spends anything (§1.3.10).
- **Opened for reading and writing**, because the record is written through the locked descriptor.
  The file is created with the process's default mode, 0666 less the umask, as the worktree lock's
  is.
- **When it cannot be.** A common git dir in which the file is absent and cannot be created, or a
  file this user cannot open for writing, refuses the write command before any agent runs
  (§1.3.10).

**[R2 · DELETE] Never removed**, by any run or any operator (§1.3.9).

#### 1.3.3 The primitive, at MSRV 1.85

**[R2 · WIN, FILTER] Round 1's primitive, and why it is replaced.** Round 1 made the hold outlive its
holder: on Unix the locked descriptor was handed to the registry Git child, and on Windows the
ambient kill-on-close job was to end the child with its holder. Review round 1 refuted both halves.
- **Windows (FUB-D1-WIN, P1).** The documented contracts order nothing between the two events a
  holder's death sets off:
  - the OS releases a dead process's byte-range locks in its own time: "the time it takes for the
    operating system to unlock these locks depends upon available system resources" (`LockFileEx`);
  - the job ends the Git child asynchronously. A kill-on-close job ends its processes when its last
    handle closes (`JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE`), and ending a process is the termination
    `TerminateProcess` documents: "TerminateProcess is asynchronous; it initiates termination and
    returns immediately", and it "requests cancellation of all pending I/O. The terminated process
    cannot exit until all pending I/O has been completed or canceled".

  So another coordinator can acquire the lock while the dead holder's child is still being
  terminated with its writes in flight. Eventual child death, which round 1's T6(d) tested, is not
  the ordering exclusion needs.
- **Unix (FUB-D1-FILTER, P2, executed).** A descriptor handed to the Git child is handed to every
  descendant the child starts. Reproduced with Git 2.43.0 (`d2/witness/filter/filter-inherited.log`,
  the script beside it): a smudge filter started an ordinary background helper; the holder was
  `SIGKILL`ed with the filter paused, and the filter released. Git exited at 37.7 ms with the
  snapshot registered. A fresh acquisition stayed contended at the 3 s bound, and succeeded only
  once the test killed the helper.

**[R2 · WIN, FILTER] The construction, one for both platforms: the lock is held only by the engine
process that took it, and every acquirer waits until every process the previous holder's access
started has terminated.** Four parts follow: the hold, the record, the waiter's check, and why the three give
the ordering.

**[R2 · WIN, FILTER] The hold: the holder's alone.**
- **Unix: `flock(LOCK_EX | LOCK_NB)`** on a descriptor opened close-on-exec, which is never handed
  to a child. A spawn's copy of it closes at that child's `exec` (§1.3.8).
- **Windows: `LockFileEx(LOCKFILE_EXCLUSIVE_LOCK | LOCKFILE_FAIL_IMMEDIATELY)`**, `rundir`'s existing
  `imp::take` (`src/rundir.rs:2676-2721`), on a handle `std` opens non-inheritable.
- **Why `flock` and not `fcntl`.** An `fcntl` lock is the process's: it does not exclude a process
  from itself, and closing any descriptor for the file releases it (the primitive's notes,
  `src/rundir.rs:2544-2576`). An `flock` lock belongs to the open file description, so two threads
  with two descriptors exclude each other and another descriptor's close releases nothing.
- **Either way** the lock is released by the holder's explicit unlock or by the OS at the holder's
  death, and by nothing else. **No hold outlives its holder**, so R17's "released at process exit
  (OS-released on death)" stays exactly true (§1.8).

**[R2 · WIN, FILTER] The record: who may still be writing.**
- **What it is.** The lock file's contents, one line per process, naming it by pid and **start
  identity**:
  - Linux: `/proc/<pid>/stat` field 22, `starttime`; the holder's line also carries the boot's
    `/proc/sys/kernel/random/boot_id`, because `starttime` counts from boot;
  - macOS: `proc_pidinfo(PROC_PIDTBSDINFO)`'s `pbi_start_tvsec` and `pbi_start_tvusec`;
  - Windows: `GetProcessTimes`'s creation time.
- **Who writes which line, and when.** Each line is written by one write, before the step it guards.
  1. **The holder's own line**, right after it acquires, and after it has waited out and cleared a
     dead predecessor's record. It precedes every step of the access, the holder's own writes to the
     store included (`remove_bound`'s unlinks and removals).
  2. **A Git child's line, before the child can run.**
     - Unix: the child writes it itself, between `fork` and `exec`, in the `pre_exec` closure, which
       already exists for exactly this kind of step (`hold_cleanup_lease_for_child`,
       `src/rundir.rs:2193`). It uses async-signal-safe calls only: `getpid`, and on Linux `open`,
       `read` and `close` of `/proc/self/stat` for its `starttime`, then one `write` at the record's
       end, which the parent positions before it spawns. A child whose line cannot be written does
       not `exec`: the closure returns the error, and `spawn` fails.
     - macOS, and Linux where `/proc` cannot be read, add one step: the `pre_exec` line carries the
       pid alone, and the holder adds the child's start time once `spawn` has returned.
     - Windows: the holder writes the line while the child is created suspended (`CREATE_SUSPENDED`):
       its pid and creation time, then it resumes the primary thread. This is the shape the host
       runner already uses for every agent (`spawn_suspended_in_job_with`,
       `src/agent/proc.rs:1028`).
  3. **Cleared by the holder.** It truncates the file to zero length once the access has waited for
     its last child, and only then unlocks.
- **A holder that dies leaves its record.** That record names the holder and every child it started,
  because no child runs before its line exists.

**[R2 · WIN, FILTER] The waiter's check: quiescence before the first read.** An acquirer reads the
record right after it acquires the lock, before it reads or writes the store. For each line, it
waits until the named process has **terminated**: no process holds the pid; or the process holding
it has another start identity; or it is a zombie (Linux `stat` state `Z` or `X`, macOS
`pbi_status == SZOMB`); or (Windows) its process object is signalled.
- **The wait** polls with the lock's own backoff, under the acquisition's one deadline (§1.3.5).
- **A record from another boot** (Linux: the holder line's `boot_id` differs) is stale as a whole and
  is cleared without waiting. On macOS and Windows a start identity is an absolute instant, which no
  process of a later boot can share.
- **An answer that is none of these is not "terminated".** A permission error, or an unreadable
  `/proc` entry, keeps the wait going: the check fails closed. On Windows the query opens the
  process with `PROCESS_QUERY_LIMITED_INFORMATION` first, so a pid now held by a process this user
  cannot wait on is told apart by its creation time, and only a matching process is waited on.
- **"No process holds the pid" is read narrowly.** It is `ENOENT` for `/proc/<pid>` on Linux, `ESRCH`
  from `proc_pidinfo` on macOS, and `ERROR_INVALID_PARAMETER` from `OpenProcess` on Windows. The
  last is observed behaviour rather than a documented contract, and T15 checks it on the Windows
  legs. Every other failure is "unknown" and keeps the wait, so an answer that ever changed would
  fail closed, not open.
- **Then** the acquirer truncates the record and writes its own holder line.
- **A torn final line is ignored.** Each line is written by one write before the step it guards, so
  a line torn by its writer's death guards a step that never ran: a holder that never began its
  access, or a child that never reached `exec` or was never resumed. Any other line that does not
  parse can only be a foreign write, and it is "unknown": the wait fails closed (§1.3.9).

**[R2 · WIN] Why this orders the next read after the writer's last write.** The proof rests on what
"terminated" means, and never on the order in which the OS releases a lock and ends a process.
- **Windows.**
  - "The terminated process cannot exit until all pending I/O has been completed or canceled"
    (`TerminateProcess`).
  - "The process object is signaled" is the last result of terminating a process ("Terminating a
    Process"), and `WaitForSingleObject` on a process handle returns when it is.
  - A pid is reused only after its process has terminated and its object is gone, and the process
    that reuses it was created later: its `GetProcessTimes` creation time differs.

  So once every named process is terminated, none of them has I/O pending, and none can issue more.
  The lock's asynchronous release, which FUB-D1-WIN names, no longer matters: the waiter does not
  trust the release to mean quiescence. It checks.
- **Unix.** A zombie is a process that has terminated and not yet been waited for (`wait(2)`), and a
  pid no process holds names nothing still running. A terminated process executes nothing further:
  its last system call has returned.
- **Completeness.** A child's line exists before the child runs, and the holder's before the holder
  touches the store. A holder that dies at any point therefore leaves a record naming every engine
  process that could still write to the store for that access.

**[R2 · FILTER, WIN] What the record does not name: descendants.** The waiter waits for the
processes the engine started, and for nothing else.
- **Git's own subprocesses** (`worktree add`'s `update-ref` and `reset`) are waited for by the Git
  command that starts them on every path where that command exits by itself (§1.2, measured). A
  command killed first, on Windows by the job when its holder dies, is a killed write. Whatever its
  subprocesses do after that is the killed write's residue (R1, §1.5). They install the entry's
  `HEAD` by rename and write `index` and `ORIG_HEAD`, which no enumeration reads, so none of it can
  leave an enumerated file half written.
- **A filter's background helper** is not a registry writer. Measured
  (`d2/witness/filter/filter-record.log`), with the revised construction against the same filter:
  the holder was `SIGKILL`ed at 25.6 ms; B acquired the lock at 25.6 ms, with Git still paused in
  the filter; B read the record (`holder <pid>`, `writer <pid>`) and waited for Git; the test
  released the filter at 25.7 ms; B proceeded at 35.2 ms, once Git had terminated, with the helper
  still alive.

**[R2 · WIN] The alternative the brief offered, and why it is not this design.** That alternative is
a dedicated holder process that is the writer's parent, holds the lock, and exits only after the
writer. A holder can die before its writer as surely as a coordinator can:
- on Windows its lock is released, and its job's processes are ended, by the same two unordered
  asynchronous events;
- on Unix its `flock` is released when its descriptors close as it exits, and nothing orders that
  after its child's last write: a parent-death signal (`prctl(PR_SET_PDEATHSIG)`, Linux only) is a
  signal the child receives when the parent dies, which starts the child's termination and does not
  complete it.

So exclusion would end, again, before the writer was quiescent. The waiter-side check holds
whatever dies, and in whatever order. The design keeps the holder's lock, plus the record, and needs
no extra process.

**[R2 · DEADLINE] One process, many threads.** Each acquisition opens its own descriptor, and
`flock` per open file description and `LockFileEx` per handle exclude two threads of one process
from each other. So R-X, the process-local mutex, has no work left and is retired (§1.3.4).

**[R2 · WIN] Release.**
- **Normal path.** The access returns once every child it started has been waited for. Then the
  holder truncates the record, unlocks explicitly (`LOCK_UN`; Windows `UnlockFileEx`, `imp::unlock`,
  `src/rundir.rs:2723`), and closes. An explicit unlock ends the lock at once, even if a sibling
  thread's fork still holds a transient copy of the descriptor (§1.3.8).
- **Death.** The OS releases the lock. The record stays, for the next acquirer.

**[R2 · WIN] Where the platform code lives.** The process-identity questions belong to the process
module, which already asks them:
- `agent::proc` exposes `process_alive` and `process_creation_time` (`src/agent/proc/ambient.rs:64`,
  `:70`);
- it already asks macOS's `proc_pidinfo` about a child group's zombie leader
  (`zombie_group_answer`, `src/agent/proc.rs:754-766`);
- the suspended spawn's resume is `resume_only_thread` (`src/agent/proc.rs:1230`).

The implementation adds, beside them, a three-way `process_state(pid, start)` (terminated, alive or
unknown) and a `process_start(pid)`, and exposes the resume. Follow-up A (#328) owns
`src/agent/proc.rs` while it is open, so that edit is sequenced after #328 merges (§1.7). The record
and the funnel are `src/rundir.rs`'s.

#### 1.3.4 The funnel: its shape, and where its hooks run

One funnel in `src/rundir.rs`, beside the other lock funnels, with a closure-shaped API. Holding
the lock across a hook would be the defect PR11's round R1 repaired (`R1-REG-1`: a removal held R-X
across its hooks, and an observer that listed the worktrees waited on itself for ever). A
closure-shaped API makes "released before the `After` hook" structural rather than a convention.

```rust
// src/rundir.rs — names are the design's, the implementer may rename them
pub(crate) fn registry_access<T>(
    common_git_dir: &Path,              // canonical
    hooks: &mut dyn RunDirHooks,
    access: impl FnOnce(&RegistryHold) -> Result<T, UpstrokeError>,
) -> Result<T, UpstrokeError>;          // bounded by REGISTRY_WAIT_BOUND; tests pass their own

pub(crate) struct RegistryHold { /* the locked descriptor and the record's end; no public constructor */ }
impl RegistryHold {
    /// Run one registry Git child under this hold, its record line written before it can run
    /// (Unix: by the child in `pre_exec`; Windows: created suspended, recorded, resumed), and
    /// wait for it. The locked descriptor is never handed to the child.
    pub(crate) fn output(&self, command: &mut std::process::Command) -> std::io::Result<Output>;
}

/// `Lock.CreateRegistryLockFile` alone: create or open the file for reading and writing, and
/// close it. The legacy path's check before any spend (§1.3.10).
pub(crate) fn create_registry_lock_file(
    common_git_dir: &Path,
    hooks: &mut dyn RunDirHooks,
) -> Result<(), UpstrokeError>;
```

**[R2 · DEADLINE, WIN] The sequence inside `registry_access`.** No hook runs while anything is held.

1. **`Lock.CreateRegistryLockFile` (R29).**
   - `hook(Before)` runs.
   - The primitive opens `<common git dir>/upstroke-registry.lock` with `create(true)`,
     `truncate(false)`, read and write, the way `WorktreeLock` opens its file: the site names the
     create even when the file is there (`src/rundir.rs:1896-1905`). It keeps the descriptor for
     step 2.
   - `hook(After)` runs.

   Nothing is locked at either hook.
2. **`Lock.AcquireRegistry` (R17).**
   - `hook(Before)` runs.
   - Then the primitive:
     - refuses at once if this thread is already inside a registry access (the re-entrancy guard,
       §1.3.7);
     - fixes the acquisition's one deadline, `now + bound`;
     - polls the OS lock until it is acquired or the deadline passes (§1.3.5);
     - reads the record and waits, by the same poll and the same deadline, until every process it
       names has terminated (§1.3.3);
     - truncates the record and writes its own holder line;
     - runs `access(&hold)`;
     - truncates the record, unlocks, and closes.
   - `hook(After)` runs. The hold is a **momentary** one, given back inside the site, as
     `Lock.ProbeCleanupExclusive`'s is (`AfterEffect::MomentaryHold`,
     `src/topology/effects/residue_authority.rs:971-977`).

   As the manager's `funnel` does (`src/workspace_manager/hooks.rs:369-380`), an `Err` from the
   primitive is returned without consulting `After`.

**[R2 · DEADLINE] R-X is retired.** Round 1 moved R-X into the funnel and took it, blocking, before
the bounded poll. That put a wait outside the bound (FUB-D1-DEADLINE). Round 2 deletes
`REGISTRY_LOCKS` and `registry_lock_of` (`src/workspace_manager.rs:1590-1601`) and puts no
in-process mutex anywhere in an acquisition. The threads of one process exclude each other through
the OS lock, each on its own descriptor (§1.3.3), so one deadline bounds the whole acquisition.

**The holders, as they become.** Each keeps its critical section exactly. Only the lock around it
changes, and the manager adapts its `EffectHooks` to `RunDirHooks` with a forwarding wrapper.

| Holder | Hooks it passes | Shape change |
|---|---|---|
| `add_worktree` (`:2649`) | the caller's | **[R2 · HOOKS]** Today `funnel(hooks, add_site, closure)`, with R-X inside the closure. It becomes hand-rolled, as the commit-tree sequence already is: `consult(Before)`, then today's checks (`revalidate_acted_through`, the intent check of `:2660`, the slot parent's `create_dir_all`), then `registry_access(…, |hold| …)`, then `consult(After)`. **Inside the closure, after the two Lock sites' hooks have run and before Git, the same checks run again**: `revalidate_acted_through(Primitive::AddWorktree, Some(slot), None)`, which walks the slot's parent; the intent's durability; and the parent's `create_dir_all`. Only then `hold.output(add)`. The checks outside give a refusal before any Lock site runs. The checks inside are the ones that bind, because the new hooks sit between the two. This is `:2660`'s own rule ("Inside the funnel, after the `Before` hook: an intent removed between a check outside and the add would leave a worktree that `reclaim_intents` can never find"), applied to the hooks round 1 added. `Worktree.Add`, `.AddStaging` and `Snapshot.Add` keep their phases around the same primitive. |
| `remove_worktree_proving` scan (`:2996-3003`) | the caller's | `registry_access(…, |_| self.revalidate_removal_proving(…))`, before the removal's funnel, as today. |
| `remove_worktree_proving` mutation (`:3006-3011`) | the caller's | Hand-rolled like the add: `consult(Before)`, `registry_access(…, |hold| self.remove_bound(…, hold))`, then `consult(After)`. `remove_bound`'s first step is already `revalidate_acted_through(Primitive::RemoveWorktree, …)` (`:3027`), so its checks run inside the hold, after the new hooks. Its prunes run through `hold.output`. |
| `worktree_records` (`:5051`) | `NoHooks` | `registry_access(…, &mut NoHooks, |hold| hold.output(list))`. `revalidate()` takes no observer, and threading one through its 25 callers is out of proportion. The same hold is observed executing at the three hooked holders. The unhooked call is the precedent `WorktreeLock::acquire_in` and `rundir::is_running` set (`src/rundir.rs:1889-1894`, `:2356-2385`). |
| `slots_with_torn_registrations` (`:5345`) | its caller's, now passed down from `repair_torn_registrations(hooks, …)` (`:5328`) | `registry_access(…, |_| scan)`. |
| `derive` (`:1639`) | — | **[R2 · FROZEN]** No longer a holder: `derive` reads no registry (§1.3.10). |
| Legacy (B1): `add_gate_worktree`, `cleanup_gate_workspace`'s remove, `worktree_is_registered`, `switch_branch` | `NoHooks`, as every legacy lock call (`resume.rs:148` uses `acquire_in`) | Each Git child is run as `registry_access(&dir, &mut NoHooks, |hold| hold.output(&mut command))`. `dir` is `rev-parse --path-format=absolute --git-common-dir`, canonicalized: the two steps `recorded_objects_scope` already takes (`src/workspace.rs:97-101`). |
| Legacy (B1): `ensure_execution_prerequisites` (`src/workspace.rs:324`) | `NoHooks` | **[R2 · PERM]** Calls `create_registry_lock_file(&dir, &mut NoHooks)`: the check before any spend (§1.3.10). It acquires nothing. |

#### 1.3.5 Blocking, and the bounded wait

**[R2 · DEADLINE] One deadline per acquisition.**
- **What it covers.** It is fixed at the acquisition's first attempt, and it covers every wait an
  acquisition makes: the poll for the OS lock, and the wait on a dead holder's record. No other wait
  exists in an acquisition. R-X is retired, and the re-entrancy guard refuses instead of waiting.
- **The polls.**
  - Unix: `flock(fd, LOCK_EX | LOCK_NB)`. Windows: `imp::take`, which is fail-immediately.
  - On "held by someone" (`EWOULDBLOCK` or `EAGAIN`; `Holder::Someone`), and on a recorded process
    not yet terminated, the funnel sleeps and tries again. The sleep starts at 1 ms and doubles to a
    25 ms cap.
  - Each try checks the deadline first, so an acquisition returns, acquired or refused, by the
    deadline plus one sleep (25 ms) plus one try (a non-blocking system call, or a process query).
  - `EINTR` retries at once. Any other failure is `UpstrokeError::Io` naming the lock file. That
    includes `ENOLCK` or `EOPNOTSUPP` on a filesystem without locks (`Holder::Unknown` on Windows).
    Today's run and worktree locks treat such a filesystem the same way (`src/rundir.rs:2369-2387`).
- **Measured** (`d2/witness/deadline/deadline-model.py`, a primitive model of both shapes, with a
  foreign process holding the lock and a 200 ms bound). The round-1 concurrency lens executed the
  same shape and measured 201 ms and 401 ms.
  - Round 1's shape, blocking R-X then a bounded poll: with two threads the second returned after
    408.3 ms (`deadline-model-n2.log`); with four, the fourth after 811.9 ms, 4.06 times the bound
    (`deadline-model-n4.log`).
  - Round 2's shape, one deadline: every thread returned after 206.6 to 206.8 ms in both runs, 1.03
    times the bound. The same holds with R-X kept as a try-lock inside the deadline (`revised-rx`
    rows), which round 2 does not need.
- **The bound.** It is a named constant, `REGISTRY_WAIT_BOUND = 600 s`, and the funnel takes it as a
  parameter so a test can pass a short one.
  - **Measured holds.** On this repository (868 files): `git worktree add` 84 ms, `git worktree
    list` 1 ms, removal and prune 9 ms (`measure/hold-durations-upstroke-repo.txt`).
  - **Scaling.** Add and removal scale with the files checked out or deleted, so a 100,000-file
    checkout is on the order of seconds, and a million files on the order of a minute and a half.
  - **The margin.** 600 s leaves room for several such holders, or a dead holder's still-running Git
    child, ahead of a waiter. A bound that expires therefore means a holder or a recorded process
    that is stuck, not slow.
- **What stays unbounded, as today.** The access itself, once acquired. The manager's Git children
  have no timeout of their own, so a hung `git worktree add` holds the lock until it is killed.
  Every waiter then refuses at its own deadline, and none queues behind another's.
- **When the deadline passes.** The error is `UpstrokeError::Refused`, naming:
  - the lock file and the bound;
  - the holder, when the platform says (Unix `flock` names none; neither does `LockFileEx`, as
    `imp::take` says);
  - **[R2 · WIN]** when the wait was on the record, the line it waited on (pid and start identity)
    and what the platform last answered for it. So an operator who finds a stuck Git child knows
    which process to end.

**It is never `UpstrokeError::Git`.** That is what keeps an expired wait out of the verification's
durable arm, which matches only the Git variant (`src/engine/topology/run.rs:279`). Where an
expiry lands, by caller:

| Caller | The expiry becomes | Durable? |
|---|---|---|
| a verification's registry access (its snapshot add's list and add) | `JudgeError::Other(Refused)` → `run::verified` → `Err(error)` (`run.rs:289`) → the coordinator fails the command (`coordinator.rs:1471`, then `fail`) | no: ends resumably; nothing is appended for it |
| an attempt's registry access (capture's list, the snapshots) | the pipeline's error (`coordinator.rs:1413`) | no: ends resumably |
| the coordinator's own (dispatch, staging add, scrub, reclaim, finalization, recovery) | a coordinator-side error | no: ends resumably |
| legacy (B1): a gate snapshot add | `?` → `discard_uncommitted()` → the command ends (`coordinator.rs:544-548`) | No terminal is appended. The worker's edits are discarded exactly as on any gate-snapshot failure today, and the resume re-runs the attempt. |
| legacy (B1): the resume's reclaim or switch | the resume refuses | no |

**What the wait does not touch.**
- **Slot holders never wait** (INV-18). A slot pair is held across one Runner call exactly:
  `execute_typed` admits, runs and ends (`src/engine/topology/attempt.rs:1164-1177`). Every registry
  access a pipeline makes is outside that span:
  - the snapshot add after its gate grant, `snapshot` (`:1129-1140`);
  - the removal after the pass has ended, `release` (`:1142-1148`);
  - capture's list, after the worker's pair is released (R-W).
- **"The coordinator never blocks on an entitlement, provisional reservation, or slot"** (INV-18;
  R-F). The lock is none of those. The coordinator thread already waits, synchronously, on its own
  Git children: "the coordinator's only waits are its inbox and the synchronous work it already did
  at width 1 (its own Git, filesystem and appends)" (R-S). The new lock adds a bounded wait for
  another process's registry access, or for a dead holder's still-running child. **[R2 ·
  DEADLINE]** It replaces the wait on R-X, which was unbounded.

#### 1.3.6 Crash behaviour

**[R2 · WIN, ERRATUM] No hold survives its holder.**
- **Unix.** The kernel closes the dead holder's descriptors, and the `flock` is released. A sibling
  thread's fork copy can delay that by the copy's fork-to-exec window, never longer (§1.3.8).
- **Windows.** The OS releases the lock, in its own time ("depends upon available system
  resources").

**[R2 · WIN] What does survive, and what the next acquirer does about it.**
- **The record.** It names the dead holder and every Git child the holder's access started (§1.3.3).
- **The child itself**, if one was running.
  - Unix: nothing in the engine kills a coordinator's Git child, so an orphaned `git worktree add`
    or `git worktree prune` keeps running and normally finishes its write. This was true at
    `92c4ca81` too.
  - Windows: the ambient job ends it with its holder (`src/main.rs:196`, `join_ambient_job`;
    INV-18), asynchronously.
- **The next acquirer** of the repository's lock, in any engine process and in any run, waits until
  every process the record names has terminated (§1.3.3). Only then does it read or write the store.
  So the next read follows the dead access's last write, whichever order the OS released the lock and
  ended the processes in. On Unix the orphaned child's write usually completes, and the next reader
  finds a whole registration.

**What a crash can still leave: a dead writer's torn registration.**
- **When.** When the writer itself was killed mid-write:
  - a holder killed in the middle of a Rust-side removal (`remove_bound`);
  - a Unix Git child killed together with its coordinator, for example by a signal to the process
    group;
  - **[R2 · WIN]** on Windows, any registry child whose holder dies before the child finishes,
    because the job kills it.
- **What it leaves.** A registration that stays torn. **[R2 · WIN]** It is **static** by the time
  anyone reads it: the next acquirer waited for the writer to terminate. The lock cannot complete a
  write whose writer no longer exists.
- **Who repairs it.** The run that owns it repairs it at its next resume, through its intents:
  `verify_worktree` and `remove_intent` run the torn plan (`:2744-2748`, `:2268-2272`).
- **What another run sees.** Until then, another run's enumerations die on it. The same class
  across runs is already filed:
  `PR308-R3-SKIPPED-PRUNE-KEEPS-ANOTHER-RUNS-TORN-REGISTRATION`
  (`findings/P2_crash-consistency_202609191720_a-skipped-prune-keeps-another-runs-torn-registration.md`).
  This change does not narrow or widen it (§1.5, residual R1).

#### 1.3.7 Acquisition order: no cycle

**[R2 · DEADLINE] Every lock and wait a coordinator process has.** R-X is gone from the table.

| Lock or wait | Taken | While holding it, the holder… |
|---|---|---|
| worktree lock (R17, `upstroke-worktree.lock`) | first, at command start, after the read-only refusals; **never waits** (refuses if held) | runs the whole command |
| run lock (R17) and the momentary cleanup probe | second; never waits (refuses) | runs the whole command |
| cleanup lease (R28, shared `flock`) | by reapers, and by each `git update-ref` child for its life | is a reaper or a ref write. Neither is a registry holder, and no registry holder runs `update-ref`. |
| snapshot gate (R-W) | a pipeline waits for a grant **before** `add_snapshot` (`attempt.rs:1135-1137`) | adds and removes snapshots |
| slot pair (PermitBroker) | a pipeline waits for a grant inside `execute_typed`, never inside a registry access | runs one Runner call, with no registry access inside it (§1.3.5) |
| **the registry lock** | inside `Lock.AcquireRegistry`, after its `Before` hook: a poll bounded by the acquisition's deadline, then a wait on a dead holder's record bounded by the same deadline | runs exactly one access: its Git children one at a time, or a scan, or `remove_bound`'s filesystem calls and its prune. It takes no other lock, sends and awaits no message, consults no hook, and calls no registry reader. |

**The proof.**
- **[R2 · DEADLINE] Within one process.**
  - The registry lock is innermost, and nothing is acquired while it is held, so no edge leaves it.
    Its holder waits only for its own Git children and its own filesystem calls.
  - A thread acquiring it waits only through polls bounded by **its own** deadline. It waits neither
    for another thread's deadline nor for a holder's child. Round 1's sentence said otherwise ("A
    thread waiting on R-X waits for a holder whose own wait is bounded: the poll plus its one
    child", record lines 422–423 at `dfd69410`), and it was wrong twice (FUB-D1-DEADLINE): R-X was a
    blocking mutex outside the bound, and a holder's Git child has no bound at all.
  - Every other wait in the table is entered outside the registry lock: grants, slots and the
    coordinator's inbox.
- **Across processes.**
  - A process acquiring the registry lock waits for another process's holder, or for a process a
    dead holder recorded. Each is bounded by its deadline.
  - A live holder waits only for its own Git children.
  - **[R2 · WIN]** A recorded process is a Git child or a dead engine process. A dead one waits on
    nothing. A Git child takes none of these locks: it is Git, and it knows nothing of them. Nor does
    any registry command write a shared ref. `worktree add --detach` writes the new registration's
    own `HEAD`, and `switch` writes its own checkout's `HEAD`. So a registry child cannot wait on a
    Git ref lock another engine child holds, or on anything its waiter holds.
- **The run and worktree locks.** No process holding the registry lock waits on either: both refuse
  rather than wait, and both are taken once, at command start, outside any registry access.

So the wait-for graph is acyclic: worktree lock → run lock → registry lock (bounded) → own Git
child, with a waiter's bounded wait on a recorded process ending at a process that waits for nothing
of the engine's.

**What enforces it** (§1.6, T5 and T11).
- **Re-entrancy guard.** A thread-local guard makes a registry access entered on a thread already
  inside one a `Refused` error. A future change that nests one, or calls a hook that does, fails
  loudly rather than polling its own lock until its deadline.
- **Census.** The census pins the holders.
- **Witness.** `R1-REG-1`'s witness is extended over the new sites.

#### 1.3.8 The inherited-descriptor class

**[R2 · FILTER] No child is handed the descriptor.** Round 1 cleared `CLOEXEC` in the registry
child's `pre_exec`, so the child, and every process the child started, held the lock (§1.3.3,
FUB-D1-FILTER). Round 2 never clears it.
- **What a fork copy can still do.** The PR281 mechanism is this: a sibling thread's `fork` copies
  every descriptor, and the copy lives until the child's `exec` or until the child closes it. The
  descriptor is close-on-exec, so a spawn's copy dies at its `exec`: "a spawn's fork-to-exec
  window", in the cleanup lease's own notes (`src/rundir.rs:2176-2183`). The Unix reaper and the
  job-control guard close inherited descriptors in their setup (`close_inherited_fds`, named at
  `src/rundir.rs:2181-2183`).
- **So a copy can only lengthen a hold**, and only by that window: the copy keeps the lock only if
  the holder dies while the copy exists. For a mutual-exclusion lock, longer is safe. The normal
  release does not wait for copies: it is an explicit `LOCK_UN` (§1.3.3).
- **The registry child's own copy** is one of these: it closes at the child's `exec`. Before that,
  the child writes its record line (§1.3.3), so a holder that dies between `fork` and `exec` leaves
  the lock held by the child's copy until the line is written and the child has `exec`ed.
- **No descendant of a registry child ever holds the lock**, not Git, not a filter, and not a
  filter's background helper. Measured: §1.3.3, `d2/witness/filter/filter-record.log`.
- **Windows.** The handle is not inheritable, and the record names only the holder and its children
  (§1.3.3).
- **What `fcntl` would have cost.** The same inheritance answer (no copy holds the lock), and the two
  hazards of §1.3.3. `flock` on a close-on-exec descriptor gets the inheritance answer without them.

#### 1.3.9 Residue: the file is never removed

**[R2 · DELETE]**
- **Never removed, by a run or by an operator.** Round 1 let an operator delete the file "only when
  no upstroke process is running on the repository". Review round 1 showed that condition splits a
  live lock: an orphaned Git child of a dead coordinator could still be writing under the old file's
  lock while a new start locked a new file (FUB-D1-DELETE). Round 2's file also carries the record
  of exactly such a child, and deleting it would discard that record. So the design gives no
  condition under which the file may be removed, and `DESIGN.md` §15 says so.
- **A file left behind costs nothing.** It holds either nothing, or a record of processes the next
  acquirer finds terminated and clears. It is advisory, and Git ignores it (§1.3.2).
- **A record something else corrupted.** Only a foreign write can put an unparseable line before
  the last. The acquirer refuses at its bound, naming the file (§1.3.3). The remedy is to truncate
  the file, never to remove it, once no Git process the engine started is running on the
  repository. Truncation keeps the file, and so cannot split the lock.
- **What deleting it anyway causes.** That is unsupported, like deleting a `.lock` file under a
  running Git:
  - processes that already have the old file open keep excluding one another on it;
  - a newcomer creates a new file and excludes none of them;
  - the record of any writer still running is lost.
- **Accounting.** It is R29, `persistent_output` at every outcome. The record is its contents (§1.8).
- **Residue classes.** None. The hold is momentary and leaves nothing
  (`AfterEffect::MomentaryHold`). A dead holder's record is R29's state, which the next acquirer
  observes and clears (§1.3.3), not residue a site reclaims.

#### 1.3.10 Before any spend, and only after the command's read-only refusals

**[R2 · FROZEN, PERM] The rules.**
- **Read-only refusals first.** R17's packet text and INV-22 say a coordinator's holds are "taken
  only after the command's read-only refusals". T-RESUME requires a refusal at recovery step (a0)
  to take no lock and create no R25 file.
- **No spend first (FUB-D1-PERM, P1, executed).** The lock file must be created or validated before
  any worker, gate, reviewer or repairer spends, on both paths. Reproduced at the Git and OS level
  (`d2/witness/perm/perm-witness.log`, the script beside it). In a linked checkout of a repository
  whose common `.git` is mode `0500` while its `worktrees/` stays writable:
  - today's per-checkout worktree lock opens: `open(O_CREAT|O_RDWR)` of
    `.git/worktrees/linked/upstroke-worktree.lock` is `OK`;
  - the legacy snapshot argv all succeed: add `rc=0`, list (3 entries), remove `rc=0`;
  - the registry lock file fails: `open(O_CREAT|O_RDWR)` of `.git/upstroke-registry.lock` is
    `FAILED EACCES`.

  The legacy path's first registry access is the gate-snapshot add after the worker (§1.2), and its
  failure there discards the worker's edits (`src/engine/coordinator.rs:544-548`). A resume would
  repeat it. The control in the same log: a file created while the directory was writable can then
  be opened, locked, written and truncated in the read-only directory.
- **No construction effect before a frozen refusal (FUB-D1-FROZEN).** The frozen `resume_with`
  (`src/engine/topology/recover/tests.rs:1267`) builds its manager (`Fixture::manager`, `:139`)
  before its read-only refusals. Its two "refused before any lock" tests then check only that no
  Lock site was observed (`any_lock_site_ran`, `:1310`) and no R25 file exists.

**[R2 · FROZEN] `derive` reads no registry.** At `92c4ca81`, `WorkspaceManager::derive` ends in
`revalidate()` (`src/workspace_manager.rs:1639`), which lists the worktrees. Under round 1 that list
would take the registry hold and create R29 before the frozen arrangement's refusal. Round 2 changes
`derive`:
- **It keeps** the chain checks (`revalidate_chain`), and the two containment checks it can decide
  without the registry: the execution root inside the managed base's own checkout
  (`RootInsideRepositoryWorktree`), and that checkout inside the root (`WorktreeInsideRoot`).
- **It drops** the comparison with every other registered worktree. That comparison is already made
  by the `revalidate()` every funnel runs before its effect: "every create/reclaim/delete
  revalidates" (`DESIGN.md` §15; `create_execution_root`, `:2072-2073`).

So constructing a manager takes no hold, creates no file and runs no Git child that touches the
store. The consequences:
- **The frozen refusals are real again.** `derive` is the only manager call the frozen
  `resume_with` makes before its refusal. Its tests' assertions (no Lock site, no R25), together
  with T14's (`derive` creates no R29 and takes no hold), make "refused before any lock" true of
  R29 and of the registry hold as well. The frozen file is not edited.
- **The ordering `ResumeSeams` forces is harmless.** `ResumeSeams` needs a manager before
  `run_recovery_order` takes the worktree lock inside it (`src/engine/topology/recover.rs:912`).
  Constructing that manager first has no lock effect, so round 1's rule for PR12 ("derive after the
  worktree lock") is no longer needed. Round 1's rule could not be kept anyway: taking the worktree
  lock early and holding it makes the recovery order's own acquisition refuse through the
  process-local claim.
- **The containment tests.** `a_root_inside_a_repository_worktree_refuses` still refuses at `derive`:
  its root is the base checkout. A root inside a *linked* checkout is refused by the first funnel's
  `revalidate()`, before that funnel's effect.

**[R2 · PERM] Topology: the first registry access comes before any spend.** Every agent invocation
of a schema-4 command runs in a worktree or snapshot that the command first adds or verifies. Both
begin with `revalidate()`'s list, which is a registry access that creates or opens the file. So a
file that cannot be created, or opened for writing, refuses the command at its first registry
access, before any worker, gate, reviewer or repairer runs:
- **in a resume**, at the recovery order's first manager funnel (finalization, the execution root's
  creation, or residue reclaim), all after steps (a0) to (c);
- **in a fresh run (PR12)**, at the first registry access. PR12's assembly makes it an explicit
  `revalidate()`, after the worktree lock and before the run is created. That call also restores a
  containment refusal before any run exists (§1.7).

The RunnerPreflight probes may come first. They are pre-spend by design: "Unreadable capability
output is not evidence and refuses before spend" (`design/14_design_execution_engine.md`).

**[R2 · PERM] Legacy: the check in `ensure_execution_prerequisites`.** The legacy path's first
registry access comes after paid work, so round 2 adds a check before it:
`Workspace::ensure_execution_prerequisites` (`src/workspace.rs:324`) calls
`create_registry_lock_file` (`Lock.CreateRegistryLockFile`, `NoHooks`).
- **Both legacy entries call it** after their worktree lock, and before any worker:
  - the fresh run at `src/engine/coordinator.rs:147`, after the lock at `:132`;
  - the resume at `src/engine/resume.rs:432`, after the lock at `:148`.
- **The resume's reclaim at `:426` comes first.** It is a registry access itself, and it refuses the
  same way, also before any attempt is re-run.
- **The check is an execution prerequisite like the two it joins**, the Git floor and
  `check-attr --source`, which the first amendment of the module put in the same function (§1.9).
- **No file outside `src/workspace.rs` changes for it.** The legacy engine modules, frozen at PR5
  (`src/engine/{coordinator,resume,attempt,preflight}.rs`), are not edited (§1.9). So this needs no
  question about B's scope.

**[R2 · PERM] The non-writable common git dir, stated.** If the file does not exist and cannot be
created (`EACCES`, `EROFS`), or exists and this user cannot open it for reading and writing, a write
command refuses **before any agent invocation**. The error is `UpstrokeError::Io` naming
`<common git dir>/upstroke-registry.lock`. Nothing that ran before it is lost: on the legacy fresh
run, no run directory exists yet; on a resume, no attempt has been re-run.
- **The remedy** is the operator's: make the directory writable once, or create the file once with
  write access for every user who runs write commands on the repository.
- **What works today and keeps working:** a file that already exists and is writable works in a
  read-only directory (the control above).
- **A shared repository** needs every user who runs write commands to be able to write the file. The
  worktree lock already imposes the same requirement on the main checkout's git dir.

### 1.4 Effect governance

**The proposed vocabulary** (Class C under the `src/topology/**` freeze; erratum E-FUB-1, §1.8):

| Item | Value |
|---|---|
| `LockSite::CreateRegistryLockFile` | row R29; adjacent `None`; fault row `TRegistry`; scope `Shared` (B1; `Topology` under B2); not read-only; no sub-effect point; no residue class; before state `Absent`; after effect `Referenced`; module `src/rundir.rs` |
| `LockSite::AcquireRegistry` | row R17; adjacent `None`; fault row `TRegistry`; scope `Shared` (B1; `Topology` under B2); not read-only; no sub-effect point; no residue class; before state `Absent`; after effect `MomentaryHold`; module `src/rundir.rs` |
| `ResourceRow::R29` | `external_physical`; `ResourceRow::ALL` 15 → 16 |
| `FaultRow::TRegistry` | the new cross-cutting row T-REGISTRY, as `TAppend` is for every append; outside the fold, as T-APPEND and T-CONTAINER are |

`Adjacent::None` because no append is ordered against a registry access: they happen around many
events, in many transactions. The precedents are the Event and the husk-removal sites
(`effect_sites.json`: nine sites with `"adjacent": "none"`, `"observable_orders": []`). Their
registry entries carry `"order": null`.

**The residue authority.** Neither site registers a residue class. `LockSite::before_state` gains
`Absent` for both, and `after_effect` gains `Referenced` (the file) and `MomentaryHold` (the hold)
(`src/topology/effects/residue_authority.rs:932-978`). **[R2 · WIN]** A dead holder's record is
R29's state, observed and cleared by the next acquirer (§1.3.3). It is not a residue class: no site
reclaims it, and the acquisition site's own primitive is what reads it.

**[R2 · PIN] The instrument census, measured rather than listed.** Round 1 listed the instrument
edits by reading, and missed the compile-time pin `src/topology/effects.rs:749` (FUB-D1-PIN). Round 2
measured them instead:
- **The probe.** The vocabulary change alone (the two variants and every arm, R29, `TRegistry`) was
  applied to a scratch `git archive` of `dfd69410`, built through `upstroke-build` on this lane's
  target, and run (`d2/census/probe/`: `patch-vocab.py`, its four stages' logs, `base-sha.txt`).
  It writes no funnel, caller or test, and nothing of it is on the branch.
- **Stage 1** names every exhaustive match and const pin the compiler refuses
  (`stage1-build.log`).
- **Stage 2** builds, and runs the suite: 19 failures (`stage2-failures.txt`).
- **Stages 3 and 4** move each pin the failures name and regenerate the two effect artifacts
  (`stage3-patch.log`, `artifact-diffs.txt`, `artifact-counts.txt`). The suite is then left with the
  four failures that only the implementation itself satisfies (`stage4-test.log`).
- **The base line of every pin** is in `d2/census/pin-lines-at-base.txt`.

**(a) Class C, under the `src/topology/**` freeze.** Every item below is the vocabulary's own or a
test of it, and round 2 asks the owner to approve all of them (§1.8).

| File | What moves | Found by |
|---|---|---|
| `src/topology/effects/sites.rs` | `LockSite` gains the two variants (`:1145`), `ALL` 6 → 8 (`:1166`), and arms in all nine const fns (`name` through `residue_elements`) | design |
| `src/topology/effects/sites.rs` tests | the `spellings!(LockSite: …)` list (`:1745`) gains both names; the inventory walk's pin `walked, 70` (`:1822`) becomes 72 | compile error; failure |
| `src/topology/effects/vocab.rs` | `ResourceRow::R29` (variant `:165`, `ALL`, `name` `:205`, `domain` external physical `:217`); `FaultRow::TRegistry` after `TAppend` (variant `:426`, `ALL`, `id` `"T-REGISTRY"` `:478`) | design |
| `src/topology/effects/vocab.rs` tests | the ledger-id table gains `(R29, "R29")` (`:912`); the exclusion rules' range `1..=28` becomes `1..=29` (`:932`) and the admitted count 15 → 16 (`:941`) | failure |
| `src/topology/effects/residue_authority.rs` | `before_state` gains `Absent` for both (`:946`); `after_effect` gains `Referenced` and `MomentaryHold` (`:971-976`) | design |
| **`src/topology/effects.rs:749`** | **the production const assertion `INVENTORY_SIZE == 70` becomes 72.** Round 1 omitted it, and without it the crate does not compile. | compile error |
| `src/topology/effects/tests.rs` | `tie!(LockSite, 6, …)` becomes 8 with both slots (`:677`); the expected-attribute table gains two rows after `Lock.ObserveCleanupHold` (`:425`); the Lock group's rows `{R17, R25, R28}` gain R29 (`:903`); `ResourceRow::ALL.len()` 15 → 16 (`:950`); `FaultRow::ALL.len()` and the id count 21 → 22, with `"T-REGISTRY"` in the id list (`:983`, `:1003`); the `Adjacent::None` set gains both sites (`:1073`); the ledger-row spellings gain R29 (`:1773`); `AFTER_EFFECT_ORACLE` and `BEFORE_STATE_ORACLE` gain two rows each (`:4613`, `:4701`); the `Absent` count 41 → 43 (`:5068`); the export's length 70 → 72 (`:6807`) and its row count 15 → 16 (`:6856`) | compile error; failures |
| `src/topology/effects/tests.rs` | `NAMED_IN_THE_DESIGN` (`:521`), "the packet's" list of named sites, gains both names once E-FUB-1 names them in `effect_site_inventory.identity`. Nothing fails without it: it is a list kept in step with the packet. | reading |
| `src/topology/census.rs` tests | `FaultRow::ALL.len()` 21 → 22 (`:4527`) and the summary's row count 21 → 22 (`:4990`); `TRegistry` joins the outside-the-fold list (`:4523`) | failures |

**(b) Outside the freeze, moved by the vocabulary.**

| File | What moves |
|---|---|
| `src/engine/topology/reachability.rs` | `matches_row`'s exhaustive match gains `FaultRow::TRegistry` in its `false` arm (`:457`; a compile error without it), and `outside_the_fold` gains it (`:374`). T-REGISTRY has no fold state of its own: it sits inside other rows' transactions. This module is the census's classifier, outside `src/topology/**` and outside the PR11 record's frozen set (R-D). It is named here so that G6's reviewer meets it as part of this change. |
| `src/engine/topology/coverage.rs`, `coverage/tests.rs` | four `Claim`s, one per site and phase, each naming its T9 test; the count of claimed sites 68 → 70 (`coverage/tests.rs:226`) |
| `effect_sites.json` | regenerated: 70 rows → 72; Shared 29 → 31; `"adjacent": "none"` 9 → 11; rows named 15 → 16; fault rows named 15 → 16; `src/rundir.rs` sites 20 → 22 (`artifact-counts.txt`) |
| `effects/funnel-modules.json` | regenerated: `sites_checked` 70 → 72, and nothing else (`artifact-diffs.txt`): both sites' inventory module and funnel module are `src/rundir.rs`, so no disagreement is added |
| `effects/sequential-registry.json` | regenerated once the claims and their tests exist: `entries` 180 → 184 (two per Lock site, Before and After, as every Lock site has today); `range` 68 → 70 (`artifact-counts.txt`) |

**(c) Satisfied by the implementation, with no pin to move.** These four fail on the probe because it
has no funnel, and pass once the funnel exists:
- `effects::tests::every_site_the_inventory_declares_has_a_funnel_that_names_it_or_is_recorded_absent`;
- `rundir::tests::every_site_this_module_owns_is_reached_through_a_funnel_in_both_phases`;
- `engine::topology::coverage::tests::the_inventory_is_claimed_at_every_required_phase_on_both_hosts`;
- `engine::topology::coverage::tests::the_sequential_registry_is_pinned`.

**(d) Instruments the funnel's own shape moves.** These come from §1.3, not from the vocabulary.
- **`src/runner/contract.rs`'s `every_production_process_start_is_classified`** (`:1632`). It gains a
  `src/rundir.rs` row of 0 `Command::new(`, 1 `.spawn()` and 0 `run_with_timeout`, and
  `expected.len()` goes from 5 to 6 (`:1747`). The one `.spawn()` is `RegistryHold::output`'s: on
  Windows the child must be created suspended and recorded before it is resumed, which `.output()`
  cannot do (§1.3.3). The row's reason is the Git rows' own: authoritative Git, deliberately not
  routed (`DESIGN.md:612`).
- **`effects/allowlist.toml`, the `src/rundir.rs` row's `review` text.** It gains one sentence: the
  registry funnel runs the one Git child each access hands it, with the child's record line written
  before it runs, and hands it no descriptor. Its `path` and `allows` do not move.
- **`effects/wrappers.toml`, the `src/rundir.rs` module.** `funnel` goes from 23 to 26, gaining
  `registry_access`, `output` (`RegistryHold::output`) and `create_registry_lock_file`. `output` is
  reachable only through a `&RegistryHold`, which exists only inside `Lock.AcquireRegistry`. The
  `shared` counts do not move: no callable of `src/rundir.rs` or its children bears any of the three
  names at `dfd69410` (`d2/census/rundir-names.txt`).
  `every_name_more_than_one_callable_bears_is_pinned_by_its_count` re-derives them.
- **`effects/wrappers.toml`, the `src/agent/proc.rs` and `src/agent/proc/ambient.rs` modules.** They
  classify the names §1.3.3 adds beside `process_alive`: `process_state`, `process_start`, and the
  exposed resume. Today `process_alive` is `effect_free` in both rows (`rundir-names.txt`).
- **`effects/wrappers.toml`'s `[libc]` section, and `clippy.toml`.** The section classifies every
  `libc::` item the tree names (`src/effects/tests.rs:6186`).
  - The design's calls are already classified: `write`, `open` and `close` as effects, and `getpid`,
    `read` and `proc_pidinfo` as not effects.
  - The new names are `PROC_PIDTBSDINFO` and `proc_bsdinfo`, both not effects (`proc_pidinfo` is
    called today with `PROC_PIDT_SHORTBSDINFO`).
  - `clippy.toml` gains a denial only if the implementation reaches an effectful `libc::` item it
    does not already deny, and the design reaches none.
- **`effects/allowlist.toml`, the legacy `src/workspace.rs` row** (B1). Its `legacy_effect` text
  records the second amendment (§1.9). Its `path` and `allows`, and `FROZEN_LEGACY_ALLOWLIST`
  (`src/effects.rs:1306`), do not move.

**(e) Prose that states the counts.** Nothing fails on any of these, but each becomes false. Those in
`src/topology/**` are inside the Class C scope, and the notes are the implementation's to update with
the code (`d2/census/prose-counts.txt`):
- "seventy" at `src/topology/effects.rs:131`, `:361` and `:736`, at
  `src/topology/effects/residue_authority.rs:221` ("eight of the seventy"), at
  `src/topology/effects/tests.rs:4632-4637` (seventy, forty-nine, twenty-one) and `:5573`, and at
  `docs/internals/effects/tests/source_oracles.md:519`;
- "fifteen" rows at `src/topology/effects/vocab.rs:125-127` and `src/topology/effects/tests.rs:941`,
  `:954`;
- "twenty-one" fault rows at `docs/internals/engine/topology/reachability.md:56` and
  `docs/internals/topology/census.md:1856`;
- the comment over the `Adjacent::None` set, "Exactly the Event group and the two husk-removal
  sites have no adjacency" (`src/topology/effects/tests.rs:1060-1062`).

**(f) What does not move.** `.cargo/`, `Cargo.toml`'s `[lints]`, `.github/`, `scripts/`, the frozen
legacy list, and every other CI-contract test under `src/effects/`: the site censuses re-derive from
the inventory.

**[R2 · PIN] Round 1's table, re-derived.** Every count the round-1 regression lens gave, against the
probe:

| Instrument | Round-1 lens | Re-derived | Source |
|---|---|---|---|
| `sites.rs`: Lock variants; inventory walk; neither-adjacency count | 6 → 8; 70 → 72; 9 → 11 | 6 → 8; 70 → 72; 9 → 11 | `pin-lines-at-base.txt`; `artifact-counts.txt` |
| `vocab.rs`: resource variants; fault-row variants | 15 → 16; 21 → 22 | 15 → 16; 21 → 22 | `stage2-failures.txt` |
| `residue_authority.rs` | `Absent` ×2; `Referenced`, `MomentaryHold` | same | design |
| `effect_sites.json`: rows; Shared | 70 → 72; 29 → 31 | 70 → 72; 29 → 31 | `artifact-counts.txt` |
| `funnel-modules.json` | `sites_checked` 70 → 72; disagreements unchanged | same | `artifact-diffs.txt` |
| `sequential-registry.json` | 180 → 184; range 68 → 70 | 180 → 184; range 68 → 70 | `artifact-counts.txt` |
| `coverage.rs` | four claims | four claims, and `coverage/tests.rs:226` 68 → 70 | `stage2-failures.txt` |
| `wrappers.toml` (`rundir`) | 23 → 25 | 23 → 26 (round 2 adds `create_registry_lock_file`) | `rundir-names.txt` |
| `allowlist.toml` | the workspace row's text | the workspace row's text, and the `rundir` row's | §1.3.3 |
| `src/topology/effects.rs` | 70 → 72, omitted by round 1 | 70 → 72 | `stage1-build.log` |
| not in the lens's table | — | `reachability.rs`; the `effects/tests.rs`, `vocab.rs`, `sites.rs` and `census.rs` test pins of (a); `contract.rs`; the `agent::proc` rows; `[libc]` | §1.4 (a)–(d) |

**The test counts and pins that move beyond the census.**
- Exact hook traces around an add or a removal, which now carry the two Lock sites' four phases
  inside the enclosing site's phases.
- `any_lock_site_ran` (`recover/tests.rs:1310`) iterates `LockSite::ALL`, so it covers the new
  sites without an edit.

**The implementer's first measurement.** Run the frozen test children (`integrate/tests.rs`,
`repair/tests.rs`, `recover/tests.rs`) unchanged. If any assertion fails only because of the new
observations, the remedy is in non-frozen code, the hooked holders' placement. It is never an edit
to a frozen child. G6's module diff proof admits only a mechanical migration there (PR11 record, R-D).

### 1.5 Both paths closed

**The claim, with A1 + B1.** No engine process, topology or legacy, reads or writes
`<common git dir>/worktrees/` while another engine process's write there is incomplete. Each step
cites the section that carries it.

1. **Every engine write is under the lock.** That is every write to the store by any engine process
   (tables A and C):
   - the add's child;
   - the prune's child;
   - `remove_bound`'s unlinks and removals;
   - the legacy add and remove.

   Each runs inside `Lock.AcquireRegistry` (§1.3.4).
2. **[R2 · WIN] No engine access begins while an engine write is still in flight.**
   - **Normal path.** The holder waits for each child it started before it releases.
   - **Death.** The next acquirer waits until every process the dead holder's record names has
     terminated, before it reads or writes (§1.3.3). A child cannot run before its line exists, and
     a holder cannot touch the store before its own does.
   - **What that holds against.** It holds whatever the OS released first, so the Windows ordering
     gap round 1 left (FUB-D1-WIN) does not arise, and it holds on Unix for an orphaned child that
     is still writing.
   - **The one exception** is a write ended by its writer's death. That write is not in flight but
     abandoned, and what it left is static by the time anyone reads it: residual R1.
3. **Every engine read is under the lock.** That is every enumeration or scan of the store whose
   failure the engine acts on (tables A and C). The exceptions are `fsck` on the refusing path, and
   `registration_for`, which no production path calls (table B). **[R2 · FROZEN]** `derive` no
   longer reads the store at all (§1.3.10).
4. **So no engine read overlaps an engine write**, by mutual exclusion (§1.3.3, §1.3.7) and by the
   waiter's check (§1.3.3).

**The pipeline path.** An attempt's registry access cannot fail on another engine process's write.
So no pipeline error, and no resumable end of the command, comes from one.

**The verification path.**
- **What a verification does in the store.**
  - Its registry accesses are its snapshot add's list and `git worktree add`, in the pipeline.
  - The coordinator does the staging add before `merge_verification_started`
    (`integrate.rs:585`), and the reclaim after the terminal.
- **What reaches `run::verified`.** None of them observes another engine process's half-written
  registration. So the Git arm at `run.rs:279` never receives one. An engine race never produces
  `Verified::Unavailable`, `merge_verification_unavailable`, a spent deferral, or a park.
- **What the arm still receives.** It still maps foreign Git state to `Unavailable`, as it should.
  The residuals below are that state.

**What remains** (not closed by this change, and stated in the `DESIGN.md` text):

| | What | Reaches the verification's arm? | Narrowed by this change? |
|---|---|---|---|
| R1 | A dead writer's torn registration (§1.3.6), until its owner's resume repairs it. **[R2 · WIN]** It is static when read: the next acquirer waits for the writer to terminate. On Windows it also arises when a holder dies while its child is still writing, because the job kills the child. Whatever that child's own Git subprocesses still do after it is killed is part of the same killed write. They write no file an enumeration reads, except `HEAD`, by rename (§1.2). | yes, for another run's enumerations: it is foreign state now, not a race | no; filed as `PR308-R3-SKIPPED-PRUNE-KEEPS-ANOTHER-RUNS-TORN-REGISTRATION` |
| R2 | A host-runner **agent** running `git worktree prune` or `add` in its own worktree (`PR11-HOST-AGENT-PRUNE-RACES-AN-ENGINE-ADD`, `findings/P2_correctness_202610010030_a-host-agents-own-worktree-prune-races-an-engine-add.md`) | yes | **no**. The agent's Git takes no engine lock, so this change neither narrows nor widens it, and the finding stays filed. The container runner's disposable Git view withholds `worktrees` from a role (`src/runner/container/view.rs:240`), and projects nothing else of the common git dir but its listed entries, so a container agent reaches neither the store nor the lock file. |
| R3 | The user's own Git in any checkout of the repository, including a maintenance run their own commit starts | yes | no: foreign Git state nobody can exclude |
| R4 | `fsck` on the refusing path (table B) | not a verification's; a refusal either way | n/a |
| R5 | Under B2: a legacy coordinator (§1.10) | yes, after PR12 | n/a |
| R6 | **[R2 · WIN]** Liveness, never exclusion. On macOS, and on Linux where `/proc` cannot be read, a child's record line carries only its pid between the child's `exec` and the holder's adding its start time (§1.3.3). If the holder dies in that interval, and the child's pid is later reused by a long-lived process before any engine process next acquires, that acquirer waits for the unrelated process and refuses at its bound, naming it. This repeats until that process exits. Linux lines with `/proc`, and Windows lines, always carry a start identity. | no: the refusal is `Refused`, never Git | n/a |

**R8's notes pin moves with this change.**
`engine::topology::run::tests::the_verification_notes_say_a_registry_another_process_is_writing_spends_a_deferral_or_parks`
pins `run.md`'s paragraph. One proposition, "a coordinator in a linked checkout of the same
repository is one such process", becomes false once this lands. In the same change the
implementation must:
- rewrite that paragraph: an engine process in another checkout no longer reaches the arm, while a
  host agent's Git, the user's, and a dead writer's residue still do;
- rewrite the pin's propositions to match;
- keep the pin's second half, the mapping itself, unchanged;
- delete the finding file, which the pin names, and record the ledger row `fixed`.

### 1.6 Regression tests

Each test names its first-bad shape (what it fails on at `92c4ca81`, on round 1's design, or on the
mutation) and its platform budget.
- **Budget.** The Windows guest's harness was 468.44 s at `78f99c70`
  (`~/orch-pr11/logs/pr11_repair_r8/ci/ci-read-36861160153.txt`), and the hosted queue took 32 of its
  45 minutes (the brief, `~/orch-pr11/briefs/pr11_fub_design.md`). So every
  stress run is gated: full cycles on Unix, bounded cycles on Windows. Every Windows-only test is
  bounded to seconds.
- **Placement.** Every test lives outside the frozen modules and their test children.
- **[R2 · T2, WIN, FILTER] Handshakes, never sleeps.** A test that needs another process to be in
  some state waits for a file or a seam that says so, and uses elapsed time only as a watchdog that
  fails the test. The funnel's seam for this is test-only: a `#[cfg(test)]` notice, beside the poll,
  each time an acquisition finds the lock held or a recorded process still alive. It has the
  precedent of `note_removal_attempt` (`src/workspace_manager.rs:1551`, `:6377`).
- **The labels.** Where a test is new in round 2, its working name is given. Each is "fixed (design);
  witnessed in the implementation phase" in the ledger.

**T1 — the two-process witness, linked checkouts, ≥ 1,000 cycles, 0 failures.** Working name:
`two_processes_in_linked_checkouts_see_no_half_written_registration`.
- **Shape.** Review round 7's witness (`~/orch-pr11/reviews/r7-witnesses/conc/witness.patch`),
  kept. There are two `fixture::LinkedChild` processes, one in the main checkout and one in a linked
  checkout. Each holds its own worktree lock and run lock and derives a `WorkspaceManager`. On `GO`,
  each runs 500 cycles of `add_snapshot` and `remove_snapshot` through the production funnels. It
  lives in `src/workspace_manager/tests.rs`.
- **First-bad.** At `92c4ca81` it is red:
  - the lens measured 5 failures in 1,000 at `92593723`
    (`~/orch-pr11/reviews/r7-witnesses/conc/two-process.log`);
  - round R7 measured 27 in 5,000 at its narrowed code, and the port 29 in 5,000 at the merge base
    `79979d24` (the PR11 record, §13).
- **Why it discriminates.** At about five failures per thousand, a 1,000-cycle run passes on the
  base with probability near e⁻⁵ ≈ 0.7 %.
- **Mutation m1.** The poll in `registry_access` made to succeed without taking the OS lock turns it
  red.
- **Platforms.** Unix runs 2 × 500 cycles; R7's run took 3.79 s with no lock, so serialized it
  should take seconds. Windows runs 2 × 20 cycles, gated by a `cfg` on the cycle count and said in
  the test's doc.

**T2 — the verification-path witness: no durable deferral.** **[R2 · T2]** Working name:
`a_verification_beside_a_foreign_registry_writer_spends_no_deferral`.
- **Shape.** Review round 8's witness (`~/orch-pr11/reviews/r8-witnesses/conc/review-witness.patch`),
  inverted.
  - **The trigger.** When the verification's review-input check runs in the staging worktree, the
    policy starts a **foreign holder**: a `LinkedChild` process of the same test binary.
  - **The foreign holder.**
    - It takes the registry lock through the production `registry_access` (`NoHooks`, in its own
      process).
    - It writes the torn registration as R8's witness did: `HEAD`, a `gitdir`, an empty `commondir`.
    - It reports `TORN`, then **holds the torn state until the test tells it to finish**, by a
      `FINISH` file. Then it completes or removes the entry, and releases.
  - **The handshake (FUB-D1-T2).** The policy returns after `TORN`. The verification's next registry
    access, the snapshot add's `revalidate()` list, polls the lock. The test thread waits on the
    funnel's seam for that access's **attempt meeting the held lock**, and only then writes
    `FINISH`. So the reader has provably attempted while the torn state stood, however it was
    scheduled. Time is a watchdog only: if no attempt is seen within 60 s, the test fails.
- **Pass.** For `failures in [1, 2]` with `max_defers = 2`:
  - no `merge_verification_unavailable` is appended;
  - the outcome is `Complete`;
  - invocations balance;
  - replay equals live.
- **First-bad.** At `92c4ca81` there is no lock, and the verification reads the torn state. One torn
  read appends `merge_verification_unavailable` (Deferred). Two append Deferred and then Parked,
  with `run_finished(Parked)`. That is R8's witness outcome, which passed there and is the finding
  (`~/orch-pr11/reviews/r8-witnesses/conc/coordinator-review.log`).
- **Mutations.**
  - m1: the verification never meets a held lock, so it reads the torn state and defers. Red.
  - m4, the foreign holder writing **without** the funnel, is R8's witness itself. It turns red,
    which shows the test separates a fixed race from foreign state.
- **Platforms.** Every platform, with one torn write per verification, in seconds.

**T3 — three processes.** T1's shape with three `LinkedChild` processes: the main checkout and two
linked checkouts. Working name: `three_processes_in_linked_checkouts_see_no_half_written_registration`.
- Unix: 3 × 400 cycles, 1,200 in total. Windows: 3 × 10.
- 0 failures. m1 turns it red. R7's four-process run failed at the base too
  (`~/orch-pr11/reviews/r7-witnesses/conc/witness.log`).

**T4 — crash while holding: what is alive when the waiter enters.** **[R2 · WIN]**
- **(a) A dead holder's still-running child is waited for** (Unix and Windows):
  `a_waiter_enters_only_after_every_process_a_dead_holder_recorded_has_terminated`.
  - **Shape.**
    - A `LinkedChild` holder takes the lock through the funnel and, inside the access, runs a
      stand-in registry child through `RegistryHold::output`. The stand-in writes `started`, waits
      for a `RELEASE` file, writes `ended` and exits. The holder reports `HOLDING` with both pids.
    - On Windows the holder joins no job here, so that the stand-in outlives it, as an orphaned
      Unix child does.
    - The test kills the holder (`LinkedChild::kill`: `SIGKILL`, `TerminateProcess`).
    - A waiter process calls `registry_access` with a 30 s bound, and a closure that records, **on
      entry**: whether each recorded process is alive (the three-way query of §1.3.3), and whether
      `ended` exists.
    - The test waits on the waiter's seam, "waiting on a recorded process", and only then creates
      `RELEASE`.
  - **Pass.** At entry, the holder and the stand-in are both terminated and `ended` exists.
  - **First-bad.**
    - m2: the record ignored by the waiter. The waiter enters while the stand-in runs. Red.
    - m2′: the child's line not written (the `pre_exec` or the suspended-record step dropped). Red,
      the same way.
    - Round 1's design passes (a) on Unix, through the inherited descriptor, and fails it on
      Windows, where the waiter enters while the stand-in still runs. On Unix it fails T12 instead.
  - **The oracle.** File handshakes and the seam, not a clock.
- **(b) No child: the OS release** (Unix and Windows). A holder killed inside a Rust-side access,
  which blocks on a handshake, frees the lock. Another process acquires within its bound, reads a
  record naming only the dead holder, finds it terminated, and enters. That is the OS release:
  immediate on Unix, asynchronous on Windows.
  - **First-bad.** A lock that survives its holder, such as an `O_EXCL` lock file, is red at the
    bound.
- **(c) The residue is foreign: a documentation test.** After (b) with a torn entry left, another
  process's `worktree_records` returns `UpstrokeError::Git` naming the entry. That pins residual R1
  as stated, not closed.

**T5 — the acquisition-order census.** **[R2 · DEADLINE]** Working name:
`the_registry_access_order_census`.
- **(a) Static.** `registry_access(` appears in production code at exactly the listed holders:
  - the manager's five calls (the add, the two removal sections, the list, the torn plan);
  - legacy's four (B1).

  `create_registry_lock_file(` appears exactly at `ensure_execution_prerequisites`. `REGISTRY_LOCKS`
  and `registry_lock_of` appear nowhere: R-X is retired. The census lives in `src/rundir/tests.rs`,
  a subject's test, as `only_the_line_builder_introduces_terminal_layout` is.
  - **First-bad.** A holder added anywhere else, or an in-process mutex reintroduced into an
    acquisition.
- **(b) Re-entrancy.** A `registry_access` entered inside another's closure returns `Refused` at
  once, not at its deadline. The watchdog is 5 s against a 600 s bound.
  - **First-bad.** The guard removed (m3): the inner access polls its own outer hold until its
    deadline, and the watchdog fails.
- **(c) Hooks outside the lock.** `R1-REG-1`'s witness,
  `an_observer_lists_the_worktrees_from_both_hooks_of_a_removal`, extended. An observer calls
  `worktree_records()` (which takes the registry lock) from both phases of every site a holder
  consults:
  - `Worktree.Add`, `Snapshot.Add`, `Worktree.Remove`;
  - `Lock.CreateRegistryLockFile`, `Lock.AcquireRegistry`.

  A second observer waits there for **another thread's** registry access to finish.
  - **First-bad.** m5: `consult(After)` moved inside the hold. It is red with the guard's `Refused`
    (same thread), or at the watchdog (other thread).

**T6 — Windows lock semantics** (`cfg(windows)`; each a few seconds, on the guest and the hosted
queue leg). **[R2 · WIN]**
- **(a) Exclusion across processes.** A `LinkedChild` holds the lock. The parent's access with a
  1 s bound fails `Refused` (not `Git`), and succeeds after the child releases.
- **(b) Release on death.** `TerminateProcess` on the holder. The parent acquires within the bound,
  despite the asynchronous unlock.
- **(c) Not inherited.** The holder spawns an ordinary long-lived child while holding, then
  releases. The parent acquires while that child lives.
- **(d) What is alive at acquisition, with the ambient job:**
  `windows_a_waiter_finds_the_dead_holder_and_its_job_killed_child_terminated_on_entry`.
  - **Shape.** A holder that joined a kill-on-close job runs, through `RegistryHold::output`, a
    stand-in child that writes `started` and then blocks. The test terminates the holder, and the
    job's close kills the stand-in. A waiter acquires.
  - **Pass.** On entry to its closure, the waiter asserts that the holder's and the stand-in's
    process objects are both signalled (the three-way query answers "terminated" for each, by pid
    and creation time).
  - This replaces round 1's T6(d), which asserted only that the child died eventually.
- **(e) The suspended record.** A holder whose child line is written while the child is suspended:
  the test kills the holder between creation and resume (a test seam on that step). The waiter finds
  the line, and the child never ran: no `started` file.
- **What CI settles, and what it cannot.**
  - **Settled.** CI runs (d) and (e) on the Windows guest and on the hosted queue leg, and T4(a) on
    every platform. T4(a) and (e) are deterministic: they settle that the waiter waits for every
    recorded process, and that no child runs unrecorded.
  - **Not settled.** (d) cannot force the window FUB-D1-WIN describes: whether the OS releases the
    lock before the child's termination completes is the OS's timing. A pass is evidence that the
    protocol holds on that run, not proof that the window is closed. The proof is §1.3.3's: the
    waiter does not depend on that order, because it checks the documented "process object is
    signaled" state itself.
  - **Not settled either.** Git for Windows's own subprocess structure. §1.2's attribution is
    measured on Linux with Git 2.43.0, and §1.5's R1 does not depend on it.

**T7 — an expired wait is never durable.**
- **(a)** `run::verified` given the funnel's bound error (not a Git error) returns `Err`, not
  `Verified::Unavailable`.
- **(b)** A verification whose snapshot add meets a lock held past a short test bound ends the
  command with that error. Nothing is appended, and the next resume completes.
- **(c)** **[R2 · WIN]** The same with the wait on a recorded process that outlives the bound: the
  error is `Refused`, and it names the recorded line.
- **First-bad.** The bound error typed as `UpstrokeError::Git`: (a) gives `Unavailable` and (b) a
  durable deferral.

**T8 — the legacy path (B1).**
- **(a) Mixed witness.** T1 with one process driving the legacy `Workspace` in a linked checkout:
  `gate_snapshot_for_candidate_in_store`, then the snapshot's drop, which cleans up. The other
  process drives the manager in the main checkout. Unix 2 × 500, Windows 2 × 20. 0 failures. m1
  turns it red.
  - **First-bad.** The Git-level race measured in §1.2 (`measure/legacy-race-SUMMARY.txt`).
- **(b) What the regression lens holds.**
  - The legacy test set is unchanged: the same names, all green.
  - `src/workspace.rs`'s diff touches only what §1.9 names.

**T9 — the sites' ST-07 evidence.** These are the four tests the registry entries and the
`coverage.rs` claims name. A fault is armed at each new site's `Before` and `After`, at a hooked
holder (a task-worktree add). Each shows:
- what the format says each phase leaves (Before: nothing; After: the hold given back, the record
  empty, and the file present);
- the next step recovering;
- `Lock.ProbeCleanupExclusive`'s and `Lock.CreateWorktreeLockFile`'s pairs as precedent
  (`effects/sequential-registry.json`).

**T10 — no failure after paid work: a lock file that cannot be created refuses before any agent
runs** (`cfg(unix)`; skipped when the effective uid is 0, which no permission refuses).
**[R2 · PERM]** Working name: `a_registry_lock_file_that_cannot_be_created_refuses_before_any_agent_runs`.
- **Shape.** A linked checkout of a repository whose common git dir is made mode `0500`, with no
  lock file. This is the witness's arrangement (`d2/witness/perm/perm-witness.log`).
  - **(a) Legacy, fresh run.** The run refuses with `UpstrokeError::Io` naming
    `<common>/upstroke-registry.lock`. The fake adapter records zero invocations, and no run
    directory exists.
  - **(b) Legacy, resume.** A legacy run interrupted after one attempt, in a repository with no
    lock file and a read-only common dir. That is the state a run started before this change
    meets, and the test reproduces it by deleting the file its first run created. The resume
    refuses the same way, and re-runs no attempt.
  - **(c) Topology.** A scaffolded schema-4 run (`src/engine/topology/scaffold.rs`) refuses at its
    first registry access, with zero invocations.
  - **(d) The control.** The same repository with the file created while the directory was
    writable: all three proceed.
- **First-bad.**
  - Round 1's design: (a) runs the worker, then fails at the gate snapshot's add and discards the
    worker's edits (`coordinator.rs:544-548`). One invocation, and the edits gone.
  - m9: the check in `ensure_execution_prerequisites` removed. The same.
- **Windows** is not covered: there a permission is an ACL, not a mode. The code path is the same
  open, and its error surfaces before any spend in the same place. Say so in the test's doc.

**T11 — one deadline for every waiting thread** (every platform). **[R2 · DEADLINE]** Working name:
`one_deadline_bounds_every_thread_waiting_behind_a_foreign_holder`.
- **Shape.** A `LinkedChild` holds the lock for the whole test. Four threads of the test process
  start an access each, 5 ms apart, with a 1 s bound.
- **Pass.** Each returns `Refused` within [1 s, 1.5 s]. The margin covers one 25 ms sleep and
  scheduling on CI hosts this programme has measured at 2.2 to 6.6 times this box's speed. This is
  not a timing proxy for an ordering: the property under test is the bound itself, and round 1's
  shape misses it by whole bounds.
- **First-bad.**
  - Round 1's shape, a blocking mutex before the bounded poll: thread k returns near (k + 1) bounds,
    so the fourth near 4 s. The model measured the same shape at a 200 ms bound as 408.3 ms and
    811.9 ms (`d2/witness/deadline/`).
  - m7: that mutex reintroduced. Red.

**T12 — no descendant holds the lock** (Unix with a real filter; Windows with a stand-in).
**[R2 · FILTER]** Working name: `a_filters_background_helper_never_holds_the_registry_lock`.
- **Shape.** The witness's arrangement (`d2/witness/filter/filter-witness.py`).
  - A repository whose checkout runs a smudge filter. On its first file the filter writes `PAUSED`,
    waits for `RELEASE`, starts a background helper (no `setsid`; streams redirected) that sleeps
    60 s, and passes its input through.
  - A `LinkedChild` holder runs `git worktree add` through `RegistryHold::output`.
  - The test kills the holder once `PAUSED` exists, then creates `RELEASE`.
  - A waiter acquires with a 5 s bound.
- **Pass.** The waiter enters once Git has terminated, while the helper is still alive. Both are
  asserted on entry.
- **First-bad.** Round 1's design, the descriptor handed to the child (m8: `pre_exec` clearing
  `CLOEXEC`). Measured: contended at the 3 s bound with Git gone and the helper alive
  (`filter-inherited.log`).

**T13 — the add's checks bind after the new hooks.** **[R2 · HOOKS]** Working name:
`an_intent_removed_or_a_parent_relinked_at_the_registry_hook_refuses_the_add`.
- **(a)** An observer at `Before(Lock.AcquireRegistry)` of a task-worktree add removes the slot's
  intent. The add refuses `AddWithoutIntent`, and `git worktree list` shows no new registration.
- **(b)** The observer replaces the slot's parent directory with a link to a victim directory. The
  add refuses (`ReparsePointOnChain`), and the victim is untouched.
- **First-bad.** Round 1's shape, the checks before `registry_access` only: (a) adds a worktree
  `reclaim_intents` can never find, and (b) adds through the link. m10: the checks inside the
  closure removed.

**T14 — constructing a manager touches no registry.** **[R2 · FROZEN]** Working name:
`deriving_a_manager_reads_no_registry_and_creates_no_lock_file`.
- **Shape.**
  - (a) Another process holds the registry lock for the whole test. `WorkspaceManager::derive`
    returns at once, well under its 600 s bound.
  - (b) A repository whose common git dir is read-only and holds no lock file. `derive` succeeds and
    creates nothing.
  - (c) Neither leaves `upstroke-registry.lock`.
- **First-bad.** Round 1's `derive`, ending in `revalidate()` (m11). (a) waits to its bound, (b)
  fails `EACCES` or creates the file. Red.
- **What it makes true of the frozen tests.**
  `resume_with_explicit_private_root_mismatch_refused_before_any_lock` and
  `malformed_recorded_locator_refused_before_any_lock` keep their bytes and assertions. With T14,
  "before any lock" holds of R29 and the registry hold too, because `derive` is their only manager
  call before the refusal.

**T15 — the record's protocol** (unit tests in `src/rundir/tests.rs`, with the process query
injectable; every platform). **[R2 · WIN, FILTER]** Working name: `the_registry_record_protocol`.
- **(a)** A torn final line is ignored. Any other line that does not parse is "unknown", so the wait
  fails closed and refuses at its bound, naming the file.
- **(b)** A line whose pid is now held by a process with another start identity is "terminated".
- **(c)** A line naming a live process is waited on, and at a short bound the access refuses,
  naming the line.
- **(d)** An "unknown" answer keeps the wait going (fail closed) and refuses at the bound, naming
  the answer.
- **(e)** A record from another boot (Linux `boot_id`) is cleared without waiting.
- **(f)** After a normal access the file is empty. After a holder's death it names the holder and
  its children, and nothing else.
- **First-bad.** m12: "unknown" read as "terminated". (d) enters at once. Red.

**The proof the implementer owes.**
- Each mutation (m1–m12) on a scratch tree whose Compiling line names it.
- The witnesses red at `92c4ca81`, or at round 1's design, where this section says so.
- The frozen children unchanged.
- The ten gates.
- CI on every leg, with the Windows harness time read from the guest's log against 468 s.

### 1.7 Out of scope, and the risks

**Out of scope.**
- **R2, the host agent's own Git.** It stays filed: `PR11-HOST-AGENT-PRUNE-RACES-AN-ENGINE-ADD`.
  Excluding it would need the host runner to forbid or wrap an agent's Git. The container runner
  already withholds the store.
- **R1, dead writers' residue.** It stays filed: `PR308-R3-SKIPPED-PRUNE-KEEPS-ANOTHER-RUNS-TORN-REGISTRATION`.
- **R3, the user's Git.** Foreign state, and nobody can exclude it.
- **Timeouts on the manager's Git children.** The registry wait is bounded, but a hung Git child is
  not, today or after this change.

**Risks.**
- **Liveness.** A stuck holder, or **[R2 · WIN]** a dead holder's child that never terminates, stalls
  every other engine process's registry access on the repository for up to 600 s, and then that
  access ends resumably. **[R2 · WIN]** When the wait was on the record, the error names the process.
  **[R2 · DEADLINE]** Each waiter is bounded by its own deadline, and none queues behind another's.
  On the coordinator thread the stall delays grants and completions. That is the same thread the
  coordinator's own Git children already block (R-R). The unbounded wait on R-X is gone.
- **The legacy path changes (B1).**
  - A new file appears in the common git dir. **[R2 · PERM]** It appears at the command's execution
    prerequisites, before any worker, not at its first registry access.
  - **[R2 · PERM]** A repository whose common git dir does not let the user create the file, or whose
    file the user cannot write, now refuses a legacy write command before any work. Today that
    configuration runs, racing.
  - A legacy registry access can now wait, with a bound.
  - A wait past the bound fails the command where it raced before, discarding the worker's edits as
    any gate-snapshot failure does today.
  - The snapshots' cleanup runs in `Drop` (`src/workspace.rs:1419`, `:1673`), so a `Drop` can now
    wait, with a bound. Legacy runs on one thread and holds nothing there.
  - Each legacy registry access costs one extra `git rev-parse` for the common dir.
- **[R2 · PERM] Shared repositories.** Every user who runs a write command must be able to write the
  file. The worktree lock already asks the same of the main checkout's git dir.
- **[R2 · FROZEN] `derive`'s contract narrows.** It no longer refuses a root that contains, or sits
  inside, a *linked* worktree. The first funnel's `revalidate()` refuses that, before its effect.
  **PR12's assembly owes one call:** an explicit `revalidate()` after the worktree lock and before
  it creates the run. That gives a containment refusal before any run exists, and the topology
  path's check before any spend (§1.3.10). Round 1's rule for PR12, "derive after the worktree
  lock", is withdrawn: the frozen recovery order could not have kept it.
- **[R2 · WIN] Sequencing with #328.** The process-identity functions go beside `process_alive` in
  `src/agent/proc.rs` (§1.3.3), which follow-up A owns while #328 is open. So follow-up B's
  implementation starts from a master that contains #328, or asks the orchestrator first.
- **[R2 · WIN] Three platforms' process questions.**
  - Linux reads `/proc/self/stat` in `pre_exec` (async-signal-safe calls only) and `/proc/<pid>/stat`
    in the waiter.
  - macOS uses `proc_pidinfo`, which the tree already calls.
  - Windows creates the child suspended and queries it by handle.

  Each is exercised on CI: T4(a) and T15 on every leg, T6 on the Windows legs.
- **`flock` on network filesystems.** `ENOLCK` and `EOPNOTSUPP` refuse the command, as the run and
  worktree locks already do there (§1.3.5).
- **Windows path length.** The file adds 23 characters to the common git dir's path. The worktree
  lock's path is of the same order. The 220-character `.git` budget is a linked checkout's, and this
  file is not in one.
- **Test churn.** The hook traces and inventory counts in §1.4 move. A frozen test child that would
  move is a stop condition, not an edit (§1.4).
- **Version dependence.** The design's correctness rests on two things that every Git version and
  every supported OS satisfy: "a registration is written over time, and enumerations read it", and
  "a terminated process issues no further I/O". §1.2's attribution of writes to Git's subprocesses is
  Git 2.43.0's. Only R1's description uses it (§1.5).

### 1.8 Erratum E-FUB-1, proposed wording (decision A1)

**[R2 · ERRATUM] Rewritten whole in round 2.** Round 1's text widened R17's resource with a hold that
outlives its coordinator in a Git child, and left R17's lifecycle, `NoRunFinished` and T-REGISTRY
saying the opposite (FUB-D1-ERRATUM). Round 2's design keeps no hold past its holder (§1.3.3), so
R17 keeps its lifecycle exactly. What does outlive a holder, the record of the processes its access
started, is R29's contents, and the erratum accounts for it there. Round 1's text is **superseded and
is not to be adopted**.

To be adopted by the owner, beside `~/tactus-artifacts/2026-08-25-g2-pass-errata.md`'s six. It is
**not adopted**, and nothing in this record or `DESIGN.md` cites it as adopted. The anchors are the
packet's at v17 (`tactus-parallel-design-neutral-v17.json`; round 2's extracts are in
`d2/packet/anchors-v17.txt` and `d2/packet/st14-and-counts-v17.txt`).

> **E-FUB-1 — the worktree registry's cross-process lock (PR11 follow-up B, R7-CONC-1).**
>
> **1.** *`decisions.resource_accounting.rows` — new row R29*, after R28:
> `{"id": "R29", "resource": "upstroke-registry.lock file and its in-flight record (repository-scoped:
> <common git dir>/upstroke-registry.lock; created by a write command's first registry access through
> the lock funnel, or earlier by that command's check before any spend, in either case after its
> read-only refusals; spans runs; never removed, by a run or by an operator). The record is the
> file's contents: while a registry access is in flight it names, by pid and start identity, the
> holder and each Git child the access started, each line written before the step it guards; the
> holder clears it before it releases the lock", "domain": "external_physical", "granularity": "per
> repository (common git dir)", "lifecycle": {"exists": "persistent_output (its hold is R17)",
> "record": "empty between registry accesses; left naming the holder and its children when a holder
> dies inside an access; observed (never adopted) by the next registry access of any engine process,
> which waits until every process it names has terminated and then clears it, before it reads or
> writes the registry and within that access's bound"}, "at_run_end": {"Complete":
> "persistent_output", "Parked": "persistent_output", "Halted": "persistent_output",
> "BudgetExceeded": "persistent_output", "NoRunFinished": "persistent_output; its record may name a
> dead holder and a registry Git child still running or still terminating, which the next registry
> access waits out"}}`.
>
> **2.** *`decisions.resource_accounting.rows[R17].resource`*, appended inside the list of holds: "…,
> the momentary exclusive cleanup.lock probe (Unix), **and the momentary exclusive
> upstroke-registry.lock hold around each registry access (every enumeration or mutation of
> `<common git dir>/worktrees/` by an engine process), held by that process alone and never handed
> to a child**".
>
> **3.** *`decisions.resource_accounting.rows[R17].lifecycle.held`*: "released at process exit
> (OS-released on death); the lock files themselves are R21 (run-scoped)**,** R25 (repository-scoped)
> **and R29 (the registry lock file, repository-scoped)**; a surviving reaper's shared cleanup hold
> is R28". R17's `domain`, `granularity` and `at_run_end`, `NoRunFinished` included ("released
> (OS-released); empty at the next coordinator's start"), are unchanged: no R17 hold outlives the
> process that took it.
>
> **4.** *`decisions.resource_accounting.enforcement_domains.external_physical`*: R29 joins the rows it
> lists ("… R24, R25, **R29**, R26, R27; …"), and after "every worktree, staging, snapshot, and
> container intent is a durable per-owner recovery record in its row, reclaimed at process start
> (never 'empty');" insert "**the registry lock file's record (R29) is the recovery record of a
> registry access, observed and waited out by the next registry access, never reclaimed by a
> site;**". `enforcement_domains.process_local_os` is unchanged.
>
> **5.** *`decisions.resource_accounting.outcome_equations`*: in `Complete`, "…R14/R16/R20/R21 (incl.
> inert answer files and the owner and commit records)/R25/**R29** as classified"; in
> `NoRunFinished`, after "R28 may be held by a live reaper and is observed;" insert "**R29 persists,
> and its record may name a dead holder's registry Git child, which the next registry access
> observes and waits out;**". `Parked`, `Halted` and `BudgetExceeded` name R25 nowhere, and are
> unchanged.
>
> **6.** *`invariants[INV-22].statement`*: "per decisions.resource_accounting (R1-**R29**)"; and in the
> external rows' parenthesis, after "the private owner and commit records persistent", insert "**;
> the registry lock file persistent, its record observed and waited out by the next registry
> access**".
>
> **7.** *`cumulative_review_gates.standing_questions[1]`*: "exactly one inventory row (R1-**R29**)".
>
> **8.** *`decisions.effect_site_inventory.identity`*: "row(): exactly one of R9-R12, R17, R18, R19,
> R21, R22, R23, R24, R25, R26, R27, R28, **R29**)", and among the named sites: "Lock.AcquireRun,
> Lock.AcquireWorktree, Lock.ProbeCleanupExclusive, **Lock.AcquireRegistry**, Lock.Release (R17;
> the worktree lock file creation maps to R25 **and the registry lock file creation,
> Lock.CreateRegistryLockFile, to R29**; the reaper hold is observed through
> Lock.ObserveCleanupHold, R28)".
>
> **9.** *`transaction_fault_matrix` — new row T-REGISTRY*, after T-APPEND:
> `{"transaction": "T-REGISTRY", "boundary": "a registry access, i.e. an enumeration or mutation of
> <common git dir>/worktrees/ by an engine process (Lock.CreateRegistryLockFile, then
> Lock.AcquireRegistry), inside any transaction that adds, removes, lists or scans a worktree
> registration, and a write command's check before any spend (Lock.CreateRegistryLockFile alone);
> the holder dies at any point: before the hold; holding it while it waits out a dead predecessor's
> record; with its own line written and no child yet; with a registry Git child created and
> recorded but not yet running; with the child running; or after the child exited and before the
> record was cleared", "durable_state": "the enclosing transaction's; the R29 file, whose record
> names the holder and each registry Git child it started; no hold (the OS releases it at the
> holder's death)", "authoritative_state": "the enclosing transaction's row; the registry may still
> have a write in flight from a registry Git child the dead holder started (on Unix still running,
> on Windows still terminating) until that child has terminated", "resume_action": "nothing of the
> hold survives, and the enclosing transaction's row decides; the next registry access by any engine
> process, after it acquires the lock and before it reads or writes the registry, waits until every
> process the record names has terminated (no process holds the pid, the process holding it has
> another start identity, it is a zombie, or its process object is signalled; any other answer keeps
> the wait), then clears the record; a torn final line guards a step that never ran and is ignored; a
> record from an earlier boot is cleared without waiting", "refusal_condition": "a hold, or a process
> the record names, not gone within the access's single deadline (600 s) refuses the access
> resumably, with an error naming the lock file and the line waited on, and never as Git state, so
> never as an outage of a verification; a lock file that cannot be created, or opened for reading
> and writing, refuses the write command before any agent invocation", "test":
> "two_processes_in_linked_checkouts_see_no_half_written_registration;
> three_processes_in_linked_checkouts_see_no_half_written_registration;
> a_verification_beside_a_foreign_registry_writer_spends_no_deferral;
> a_waiter_enters_only_after_every_process_a_dead_holder_recorded_has_terminated;
> windows_a_waiter_finds_the_dead_holder_and_its_job_killed_child_terminated_on_entry;
> a_filters_background_helper_never_holds_the_registry_lock;
> one_deadline_bounds_every_thread_waiting_behind_a_foreign_holder;
> a_registry_lock_file_that_cannot_be_created_refuses_before_any_agent_runs;
> deriving_a_manager_reads_no_registry_and_creates_no_lock_file; the_registry_record_protocol;
> the_registry_access_order_census"}`.
>
> **10.** *`decisions.bounded_census.coverage_assertions[1]`*, appended: "**; T-REGISTRY has no durable
> prefix of its own (its prefix is the enclosing transaction's row), so its reachability and
> classification are that row's**".
>
> **11.** *`decisions.invariant_ownership.INV-22`*, after "R28 PR5 (lock funnel observation) / PR7 /
> PR11;": "**R29 follow-up B (PR11)**;".
>
> **12.** *`cumulative_review_gates.gates[G6].integrated_invariants[0]`*: "INV-22 (process-local/broker
> rows R3, R4, R13, R17 **(incl. the momentary registry hold)**, R22, R28 and R19/R26**/R29** under
> concurrency)"; and in `integrated_invariants[3]`, after "a surviving
> reaper hold is observed and refuses the exclusive side;" insert "**a dead registry holder's record
> (R29) is waited out before any registry read;**".
>
> **13.** *Class C under the `src/topology/**` freeze*, approved for exactly what record §1.4 (a)
> lists:
> - the two `LockSite` variants and their arms (`sites.rs`);
> - `ResourceRow::R29` and `FaultRow::TRegistry` (`vocab.rs`);
> - their arms in `residue_authority.rs`;
> - `INVENTORY_SIZE` 70 → 72 (`src/topology/effects.rs:749`);
> - the test pins and oracles of `src/topology/effects/{sites,vocab,tests}.rs` and
>   `src/topology/census.rs` that the vocabulary moves, and the prose in those files that states the
>   counts (§1.4 (e)).
>
> Outside `src/topology/**` and named so that G6 meets them as part of this change:
> `src/engine/topology/reachability.rs`'s `TRegistry` arms (§1.4 (b)). No event, wire or fold
> vocabulary changes.
>
> *Not amended:* `enforcement_domains.process_local_os`, `admission_and_leases.permits
> .crash_reconstruction` and INV-18's recovery text, whose R17 statements stay true. The version
> dispositions and evidence that cite "R1-R28" or "21 rows" describe the packet at their versions.

### 1.9 The legacy unfreeze, proposed wording (decision B1)

`src/workspace.rs` is frozen at PR5. The packet's PR5 contract says "existing Workspace and legacy
engine behavior untouched", and the module has been "AMENDED ONCE … by the route that finding's
guard names ('an owner decision to unfreeze the module for this one change')"
(`effects/allowlist.toml:898-924`). B1 is the second such decision.

**[R2 · PERM, FILTER] Its exact scope, by function, at `dfd69410`.** One file, `src/workspace.rs`.

| Where | What changes |
|---|---|
| a new private helper, beside `recorded_objects_scope` (`:91-109`) | resolves the canonical common git dir, the two steps `recorded_objects_scope` takes at `:97-101`, from a checkout root. `recorded_objects_scope` itself does not move. |
| `ensure_execution_prerequisites` (`:324-331`) | **[R2 · PERM]** one added call: `rundir::create_registry_lock_file(&common, &mut NoHooks)`, after the Git floor and `check-attr` checks and before `refuse_sparse_checkout`. The check before any spend (§1.3.10). |
| `add_gate_worktree` (`:871-906`) | its `git worktree add` child runs as `rundir::registry_access(&common, &mut NoHooks, |hold| hold.output(&mut command))`; the command and the error mapping are otherwise as they are |
| `cleanup_gate_workspace` (`:1549-1600`) | its `git worktree remove --force` child, the same way |
| `worktree_is_registered` (`:1602-1635`) | its `git worktree list --porcelain -z` child, the same way |
| `switch_branch` (`:450-456`) | its `switch` child, the same way. It builds the command as `run_git_with_private_hooks` (`:181-197`) builds its commands (a `PrivateHooksDir`, `core.hooksPath`, `core.fsmonitor=false`), and maps the result as `git_with_private_hooks` does. `run_git_with_private_hooks` does not move. |
| `src/workspace.rs`'s own tests | none edited. Its census `every_git_child_of_this_module_is_built_where_replacements_are_refused` (`:3681`) still finds one `Command::new(`, in `git_command`, because every child above is still built there, and the functions it names all still exist. |

**Outside `src/workspace.rs`.**
- **Not changed.** No legacy engine module: `src/engine/{coordinator,resume,attempt,preflight}.rs`,
  frozen at PR5. Both legacy entries already call `ensure_execution_prerequisites` after their
  worktree lock and before any worker (`coordinator.rs:147`, `resume.rs:432`), so no run-start path
  moves. Nothing reaches into a frozen legacy-engine module, and B's scope is this one file.
- **Tests added.** The legacy halves of T8 and T10 go in `src/engine/tests.rs`, the legacy engine's
  test child. They add tests and edit none, and move no frozen behaviour.
- **The recorded form.** The `effects/allowlist.toml` row below, an instrument edit, and the
  `DESIGN.md` §15 paragraph (§1.3.10, the design/15 text).

**The recorded form** is the row's `legacy_effect` text, extended. Round 2's text supersedes round
1's:

> … The amendment is three things and no more. [the first amendment's text, unchanged] **AMENDED
> TWICE: to close `PR11-LINKED-CHECKOUTS-RACE-THE-SHARED-WORKTREE-REGISTRY` for the legacy engine as
> well, on the owner's decision to unfreeze the module for this one change. The second amendment is
> three things and no more. (1) The four Git children that enumerate or mutate the repository's
> worktree registry — `add_gate_worktree`'s `worktree add`, `cleanup_gate_workspace`'s `worktree
> remove`, `worktree_is_registered`'s `worktree list` and `switch_branch`'s `switch` — run through
> `rundir::registry_access`, each as one `RegistryHold::output` call, which holds
> `<common git dir>/upstroke-registry.lock` around it and records the child before it runs; the
> `switch` is built as `run_git_with_private_hooks` builds its commands. (2)
> `ensure_execution_prerequisites` opens, creating it if absent, that lock file through
> `rundir::create_registry_lock_file`, so a repository where it cannot be opened for reading and
> writing refuses before any worker runs. (3) One private helper resolves the canonical common git
> dir as `recorded_objects_scope` does. Nothing else in the module moves, and no other legacy module
> moves.** Every other behaviour of the module stays frozen.

The row's `path` and `allows` do not move, so `FROZEN_LEGACY_ALLOWLIST` (`src/effects.rs:1306`)
does not move. `DESIGN.md` §15's new paragraph states the same fact for the v0.1 path, and the
sentence that names the module frozen at PR5 is left as it stands.

### 1.10 If the owner decides otherwise

Each alternative revises the subsections named here. **[R2]** Round 2's construction (§1.3.3) is the
same under every alternative: only where the file is accounted, and which paths take the lock,
change.
- **A2: R25 widened instead of a new row R29.** §1.8's R29 text, the record included, moves into
  R25. The sites map `Lock.CreateRegistryLockFile` to R25, and §1.4 drops `ResourceRow::R29` and
  its pins (the 15 → 16 counts stay 15). It puts two granularities in one row, against
  `completeness_rule`.
- **A3: remedy 2.** It replaces §1.3–§1.6. Its cost is in §1.3.1, and it still needs the record,
  because remedy 2's file has the same writer-quiescence problem.
- **A4: no packet change.** §1.4 and §1.8 change to classify the file under R25 in code and
  `DESIGN.md` only. The G6 reviewer, certifying INV-22 by the packet's rows, would meet a resource
  the rows do not name.
- **B2: `src/workspace.rs` stays frozen.** The lock covers the topology path only:
  - the two sites are `Topology`-scoped;
  - §1.9, table C's "what the lock must cover", T8, and T10's legacy halves drop out. The topology
    path's check before any spend stays (§1.3.10);
  - the claim in §1.5 narrows to "no topology coordinator observes another topology coordinator's
    half-written registration".

  The legacy coordinator, which §1.2 shows racing in production today and tearing a topology
  verification after PR12, is then filed as a new finding at P1. Its topology-verification cost is
  R7-CONC-1's own, a durable deferral or a park, so it is P1 on the same reasoning. G6, whose pass
  rule admits no open critical or high finding, would meet it open unless the owner decides
  otherwise. Under B1 nothing is filed: this change repairs it.

### 1.11 Review round 1: findings and how the design answers them

**[R2] New in round 2.**
- **The review.** Design review round 1 ran three `gpt-6-astra` lenses at `max` on `cameron-codex`
  against `dfd69410`, 21:58–22:11Z on 2026-10-01: design and remedy, concurrency, and regression. All
  three returned CHANGES_REQUIRED.
- **Where the texts are.** The verbatim texts are
  `~/orch-pr11/reviews/review-329-d1-{design,concurrency,regression}-dfd69410.review.md`, with
  their hashes in `SHA256SUMS-329-d1`. The orchestrator's deduplication is
  `review-329-d1-triage.md`: ten findings, two P1s and eight P2s.
- **The answers.** Each finding is answered by a design change, not a disclaimer. Every changed
  paragraph carries **[R2]** and the finding's short name.
- **The executed findings.** Where a finding was executed (PERM, DEADLINE, FILTER), round 2
  reproduced it on this box first, and it is now a planned regression test.

| Finding | Sev | What round 1 got wrong | Round 2's change | Where | Test | Evidence |
|---|---|---|---|---|---|---|
| FUB-D1-WIN | P1 | On Windows the dead holder's lock is released asynchronously, unordered against its Git child's asynchronous termination; T6(d) tested eventual death only | No hold outlives its holder. Every holder records itself, and each child it starts, before the child can run. Every acquirer waits until each recorded process has terminated (process object signalled, zombie, or pid gone or reused) before it reads or writes. The proof cites the documented contracts, and the dedicated-holder alternative is shown to fail the same way. | §1.3.2, §1.3.3, §1.3.6, §1.5 | T4(a); T6(d), (e); T15 | Microsoft's `TerminateProcess`, `LockFileEx` and "Terminating a Process"; `d2/witness/filter/filter-record.log` (the Unix protocol); `d2/measure/subprocess-writes-2.43.0.txt` |
| FUB-D1-PERM | P1 | The lock file's creation was first attempted at the legacy gate-snapshot add, after the worker had run | The legacy check before any spend is in `ensure_execution_prerequisites`, called at `coordinator.rs:147` and `resume.rs:432` before any worker. The topology path's first registry access precedes any agent, and PR12 makes it explicit. A non-writable common dir refuses before any work, stated. B's scope stays the one file. | §1.3.2, §1.3.10, §1.9 | T10 (a)–(d) | `d2/witness/perm/perm-witness.log` (EACCES on the new file; worktree lock and legacy argv OK; the control) |
| FUB-D1-HOOKS | P2 | The add's intent and containment checks ran before `registry_access`, whose new hooks sit between them and Git | The checks run again inside the locked closure, after the two Lock sites' hooks and before Git, as `:2660` does for the add's own hook | §1.3.4 | T13 | reasoned |
| FUB-D1-DEADLINE | P2 | A blocking R-X before the bounded poll left a wait outside the bound, and the proof at record lines 422–423 called it bounded | R-X is retired. One deadline, fixed at the first attempt, covers the lock poll and the record wait. The proof is corrected. | §1.3.3, §1.3.4, §1.3.5, §1.3.7 | T11; T5(b) | `d2/witness/deadline/deadline-model-n{2,4}.log`: 408.3 and 811.9 ms against 206.6–206.8 ms at a 200 ms bound |
| FUB-D1-FILTER | P2 | The descriptor handed to Git was inherited by every descendant, so a filter's background helper held the lock after Git exited | No child is handed the descriptor (close-on-exec; Windows non-inheritable). The record names only the processes the engine started, so nothing outlives the writer in the lock. | §1.3.3, §1.3.8 | T12 | `d2/witness/filter/filter-inherited.log` (contended at 3 s after Git exited); `filter-record.log` (proceeds once Git terminates, with the helper alive) |
| FUB-D1-ERRATUM | P2 | E-FUB-1 widened R17 with a hold surviving in a child, while R17's lifecycle, `NoRunFinished` and T-REGISTRY said nothing survives | The design no longer has a surviving hold, so R17 keeps its lifecycle exactly. What survives, the record, is R29's contents, with its lifecycle, observation, outcome equations, INV-22 text, fault row (all seven fields), census note and G6 text. The whole text is given; round 1's is superseded. | §1.8 | T-REGISTRY's tests | `d2/packet/anchors-v17.txt` |
| FUB-D1-DELETE | P2 | An operator was allowed to delete the file when no upstroke process ran, which splits the lock under an orphaned writer | No removal condition is given: never removed, by a run or an operator. What deleting it anyway costs is stated, and `DESIGN.md` §15 says it. | §1.3.2, §1.3.9 | T9 (the file is present after every phase and its recovery) | reasoned |
| FUB-D1-T2 | P2 | The foreign holder dropped the torn state after 500 ms, whether or not the reader had attempted | The holder keeps the torn state until the test sees, through the funnel's test-only seam, the reader's attempt meet the held lock. Time is a watchdog only. | §1.6 | T2 | reasoned |
| FUB-D1-FROZEN | P2 | The frozen `resume_with` derives its manager before its refusal, so `derive`'s list took the registry hold and created R29 before it | `derive` reads no registry: it keeps the chain and base-checkout containment, and the funnels' `revalidate()` keeps the rest. No frozen file is edited, and the frozen refusal tests are real refusal-before-effects again. | §1.3.4, §1.3.10, §1.7 | T14 | reasoned; `recover/tests.rs:139`, `:1267`, `:1310` |
| FUB-D1-PIN | P2 | The instrument list omitted the `INVENTORY_SIZE` const pin, and more | The census is measured by a vocabulary-only probe, and every pin and artifact is listed with its base line and count: `reachability.rs`, every test pin and oracle in `src/topology/**`, `coverage/tests.rs`, `contract.rs`, the `agent::proc` rows and the `[libc]` section, beyond the lens's table. Each of the lens's counts is re-derived. | §1.4 | the moved pins themselves | `d2/census/probe/`, `pin-lines-at-base.txt`, `rundir-names.txt`, `prose-counts.txt` |

**What round 2 did not change.** The census (§1.2), the choice of remedy 1 (§1.3.1), the file's path,
the 600 s bound, the verification's durable-arm analysis (§1.3.5), T1, T3, T7(a)–(b), T8 and T9.

## 2. Round 3 design

> **SUPERSEDED by §3 (design round 4) where this banner says; the rest of §2 stands as §3 cites it.**
> - **Withdrawn:** §2.6, the run's cleanup lease handed to the manager's Git writers, and every claim
>   that this change closes (c): §2.1's closure of (c) "by the kernel", §2.2's "So this change closes
>   it (§2.6) rather than filing it", §2.3's row (iv), §2.8's claim 4, R5 and the (c) row, §2.9's T7
>   and T8, §2.11's FUB-D2-DESC answer, §2.12's optional R28 sentence and §2.13's lease risks. (c) is
>   filed instead (§3.9); §2.2's severity and G6 reading of it stand.
> - **Replaced:** §2.4's classifier and bound by §3.3 and §3.4; §2.7's instrument list by §3.6 (no
>   instrument moves); §2.8 by §3.7; §2.9 by §3.8; §2.10's P2 grading and its texts by §3.10.
> - **Stands:** §2.2's account of (a), (b) and (c)'s mechanisms, §2.3's analysis of the remedy
>   classes (ii) and (iii), §2.4's measurements, §2.5's targeted removal, and §2.11's round 1 and 2
>   tables as history.
>
> *Round 3's banner:* **PROPOSED — for design review round 3.** No owner decision is needed before this design is
> implemented. It adds no resource row, effect site or fault row, so it needs no erratum (§2.12), and
> it edits no frozen module. Decision B is the owner's and optional: whether the frozen legacy
> `src/workspace.rs` takes the same tolerance. The topology closure does not depend on it, and §2.10
> gives both texts. Nothing in §2 is in force until the implementation lands.

### 2.1 Why round 3 steps back, and why it converges

**The signal.** MAINTAINING's second looping signal is raised: "A pass finds a P1 in machinery an
earlier round of this pull request added" (`~/orch-pr11/reviews/review-329-d2-triage.md`).
- Round 1's remedy was a lock handed to the Git child. Its review found a P1 in it: on Windows the
  lock's release is not ordered after the child's termination (FUB-D1-WIN).
- Round 2 replaced it with machinery of its own:
  - an engine-only lock;
  - a record of the holder and of each child, by pid and start identity;
  - acquirers that wait until every recorded process has terminated.
- Round 2's review found three P1s in that machinery:
  - the record's update protocol (FUB-D2-RECORD);
  - its termination oracle (FUB-D2-ENOENT);
  - Git's own subprocesses, which the record never names (FUB-D2-DESC).

  Its P2s point the same way: a helper holding the output pipe (PIPE), and a fork copy of the lock
  (ERRATUM).

**What the defect in the repair was.** Both rounds tried to establish, from outside, which processes
might still write the registry, so that exclusion could last until they stopped. Each review then
found a process the construction could not see:
- a child whose termination is asynchronous (WIN);
- a filter's helper (FILTER, PIPE);
- a fork copy (ERRATUM);
- a process `/proc` hides (ENOENT);
- a subprocess the record never named (DESC);
- a record line the holder could not finish (RECORD).

A third round of that machinery would meet the same kind of finding. A per-access process group or
job would need a recorded group id, and each step of it carries a process-tracking claim of its own:
- a member that calls `setsid` leaves the group;
- Darwin answers `kill(-pgid, 0)` on a zombie-only group with `EPERM` (measured by PR136's sampler
  work, commit `84c21e01` on the unmerged `fix/sampler-kill-and-inspection`);
- a reused group id answers as alive;
- a Windows job dies with its last handle, so a successor cannot wait on a dead holder's job unless
  the holder named it, and naming it keeps it alive.

**What round 3 does instead.** It drops that machinery. Each consequence is closed by a property that
does not depend on knowing which processes exist.
- **(a) and (b), the race, by the reader** (§2.4).
  - A registry access that fails is retried while the store shows it is being changed, or holds an
    entry no reader can read, or Git's error names an entry in it.
  - A failure across a quiet, whole store is returned unchanged.
  - A contention that outlasts a bound becomes a refusal, never Git state.
  - The reader needs no lock and knows nothing of the writer. So the tolerance holds whoever writes
    (another coordinator, a legacy run, an agent's Git, the user's) and whatever becomes of the
    writer's descendants.
- **The engine never deletes another process's entry** (§2.5). Its removals stop running
  `git worktree prune`, which deletes an add caught between its `mkdir` and its `locked`.
- **(c), the corruption a dead coordinator's Git children cause, by the kernel** (§2.6). The machinery
  is one that has survived every pass since PR7: the run's cleanup lease (R28).
  - A resume's lock acquisitions already probe the lease, and every engine `git update-ref` already
    holds it (#275).
  - It is now also handed to every Git child the manager starts to write a worktree or its
    registration, as that child's standard input.
  - So a resume cannot begin while any of them is alive, or any descendant that keeps its standard
    input. Git's own `update-ref` and `reset` under `worktree add` keep it.
  - A shared `flock` is released only when the last descriptor of its open file description closes.
    Nothing is recorded, enumerated or queried.

**Why this converges where rounds 1 and 2 did not.**
- **Its claims are about the store and the kernel, not about a list of processes.** "The store changed,
  holds an entry no reader can read, or is named in the error"; "a shared `flock` is held until the
  last descriptor closes". Every finding of both rounds was a process the list missed. Round 3 keeps no
  list.
- **Its residuals are bounded and stated** (§2.8):
  - a writer stalled inside its registration write for longer than the bound, which ends the command
    resumably and never durably;
  - the Windows side of (c), which is INV-18's ambient job, exactly as for `git update-ref` today.
- **What it reuses has survived review**: R-X and the funnels (PR11); the run's cleanup lease and its
  two observation sites (PR7, and PR10's #275).
- **What holds it** (§2.9), all executed on this box:
  - the finding's own witnesses. Review round 7's two-process witness is red 10 of 10 unpatched and
    green 10 of 10 under the shape. Review round 8's verification witness appends deferrals unpatched
    and none under it.
  - (c)'s witness with the coordinator's real locks, which loses the paid edits unpatched and keeps them
    under the shape.

### 2.2 The consequences to close, and what the evidence says of each

- **(a) A pipeline error that ends the command.** An attempt's registry access fails on another
  process's half-written entry. The coordinator cancels its other pipelines and ends the command
  resumably (`src/engine/topology/coordinator.rs:1413`).
- **(b) A verification's durable deferral or park.** The same failure inside a verification is
  `UpstrokeError::Git`.
  - `run::verified` maps exactly that variant to `Verified::Unavailable`
    (`src/engine/topology/run.rs:279`), and every other error to `Err` (`:289`).
  - The frozen `integrate.rs` then appends `merge_verification_unavailable`: a deferral spent, or the
    candidate parked at `max_defers`.
- **(c) Corruption by a dead coordinator's Git child.** Executed in round 3 with **one** coordinator and
  its own resume, through the production `WorkspaceManager` funnels. The tool is an out-of-tree probe
  built from this branch's `0874bcf3` (`d3/witness/probe/`), on Git 2.43.0.
  - **The sequence.**
    - A coordinator is `SIGKILL`ed inside `git worktree add` (G).
    - G survives on Unix: nothing kills a coordinator's Git children.
    - The resume's `verify_worktree` reads Git's in-progress marker, `locked: initializing`, as
      `VerifyFailure::Unpopulated` (`src/workspace_manager.rs:2771-2772`).
    - So `dispatch::verify_or_recreate` removes the slot and adds it again at the same path
      (`src/engine/topology/dispatch.rs:248-253`).
    - Git names a registration after its path's basename, so the new one has the same administrative
      directory.
  - **Mechanism 1, the late `reset`** (`d3/witness/c-single/witness-exec.log`).
    - G's `reset --hard` child is held before it executes Git.
    - After the resume it runs with `GIT_DIR=<slot>/.git`, which now names the new registration.
    - It resets the recreated checkout, and the worker's edit goes back to `base`.
    - `git status` is clean, so nothing records the loss.
  - **Mechanism 2, the orphaned add's junk removal** (`witness-filter.log`; traced in
    `witness-filter-strace.log` and `run-filter-strace/strace-A-tree.txt`).
    - G's `reset` is held mid-checkout in a smudge filter, and resumes after the resume has recreated
      the slot.
    - Its next write fails in its deleted working directory. Its error write meets the dead
      coordinator's closed pipe (`EPIPE`, then `SIGPIPE`).
    - G sees its child fail and runs `remove_junk`. That function is in `builtin/worktree.c` in every
      version from 2.43 to 2.55 (`d3/git-src/`).
    - `remove_junk` deletes the administrative directory and the checkout **by path**: the recreated
      registration and the recreated checkout, the worker's edits with it.
  - **Cross-run too.** A dead run's freed administrative name can be taken by another run's add with
    the same basename, and `k1-g1` is every run's first task. Round 2's concurrency lens executed an
    orphaned `update-ref` rewriting such a replacement's `HEAD`.
  - **The class is known, with only its liveness face recorded:**
    `PR136-REMOVE-WORKTREE-VS-A-GIT-CHILD-NOTHING-KILLED` (P2).
    - It was filed on `fix/sampler-kill-and-inspection`, whose PR #145 closed unmerged.
    - Its sequence: "The engine dies … while `WorkspaceManager::add_worktree` has a `git worktree add`
      in flight. Nothing kills that child … Its descendants … keep writing into the new worktree".
    - Master cites it only from
      `findings/P3_docs-contract_202609050648_unbindable-task-registration-has-no-design-sentence.md`.
    - It is the same class: a dead coordinator's Git child, which nothing kills, against recovery that
      reuses its paths. DESC and round 3's witnesses add the class's corruption face.
  - **Severity and G6.**
    - P1. Paid work is lost, and so is a registration the resume made. In mechanism 1 the loss is
      silent, and it breaks `DESIGN.md` §4's "ground truth is the diff": the judged diff is no longer
      the agent's.
    - It applies to G6: Q1's "reclaimed or repaired … before any slot reset, admission, or resource
      reuse", and ST-16's and ST-18's crash classes.
    - Under G6's rule an applicable open high finding fails the gate. So this change closes it (§2.6)
      rather than filing it.

### 2.3 The remedy classes, evaluated

| Class | Closes | Leaves | Packet change | Unfreeze | Instruments |
|---|---|---|---|---|---|
| (i) exclusion plus quiescence (rounds 1–2, or a process-group or job variant) | (a), (b) between engine processes, as long as every writer is accounted for | every writer the construction cannot see. Rounds 1–2's findings are that list, and the group or job variant adds its own (§2.1). | a lock resource and a lock site pair: erratum and Class C (§1.8) | `src/workspace.rs`, for the legacy holders | the vocabulary census of §1.4 |
| (ii) never reuse a checkout path or administrative name | (c) | (a), (b) | yes (below) | none (`dispatch.rs` and `naming.rs` are not frozen: PR11 record R-D) | not measured |
| (iii) tolerant readers plus targeted removal | (a), (b): measured (§2.4) | (c) | none | none for the topology path; `src/workspace.rs` only for the legacy path's own race (B, §2.10) | none (the shape probe, §2.7) |
| **(iv) chosen: (iii), plus the run's cleanup lease handed to the manager's Git writers** | (a), (b), (c) | the residuals of §2.8 | none, by #275's precedent (§2.6, §2.12) | none (B optional) | `effects/wrappers.toml` (one callable); `src/runner/contract.rs` (two rows): measured (§2.7) |

**Why (ii) needs the packet.** It must not reuse the **checkout path** as well as the administrative
name, and the packet fixes the path:
- The slot paths are literal in `decisions.workspace_candidates.manager`: "detached linked worktrees
  with durable synced intents (tasks/k<key>-g<gen>, merge/s<seq>)".
- `task_dispatched{key, generation, base_sha, worktree_path, …}` is "written before worktree creation".
- The T-DISPATCH resume action is "verify the worktree at the recorded base with Worktree.Verify (linked
  worktree at the recorded path, …) or remove it with force and recreate it (intent then add)" (packet
  v17, `transaction_fault_matrix[1].resume_action`).

A new path per incarnation therefore changes three things: what the recorded path means, an event
field's value, and the frozen `recover.rs`'s reading of it. Unique administrative names alone do not
close (c):
- mechanism 1 reaches the new registration through `<slot>/.git`;
- mechanism 2 deletes `<slot>` by path;
- Git 2.43 to 2.55 cannot be told the administrative name at all: `add_worktree` takes the path's
  basename, with a counter on a collision (`d3/git-src/v*/builtin/worktree.c`).

So (ii) is a packet change, and not chosen.

**Why (i) is not chosen.** Its condition, from the brief, was to account for **all** Git descendants
without per-PID records. Its group and job forms meet the findings §2.1 lists. Its one sound element
is "establish the descendants' death before reuse", and §2.6 gets that from the kernel's `flock`,
through a lease that already exists, without a group, a job or a record.

### 2.4 (a) and (b): tolerant registry access

**The rule.** Every registry access the manager makes runs as a series of attempts.
1. **Read the store.** That is: whether `<common git dir>/worktrees/` is present; each entry's name;
   and, for each entry, its `gitdir`, `commondir` and `locked`. Each of those three is recorded as
   absent, unreadable, or present with its length and modification time.
2. **Attempt.** Take R-X, run the Git command or the Rust scan, and release R-X.
3. **On success,** return.
4. **On failure,** read the store again. The failure is **contended** when any of these holds:
   1. the store holds an **incomplete entry**: `locked` with no `gitdir`; an empty `gitdir`; a
      non-empty `gitdir` beside an empty `commondir`; or a file the read could not read;
   2. the store **changed** between the two reads: an entry appeared or went, or one of its three
      files changed;
   3. the failure's text **names a path inside the store**. Git prints the entry it failed on (for
      example `failed to read <store>/<name>/commondir`, `could not open '<store>/<name>/locked' for
      writing`, `Invalid path '<store>/<name>'`), either absolutely or as `.git/worktrees/<name>/…`.
      The manager's own scan names the administrative directory it refused.
5. **Not contended:** return the failure unchanged. It is what it was before this change, Git state
   where it was Git state.
6. **Contended, and the bound not reached:** sleep (1 ms, doubling to 50 ms) outside R-X, and attempt
   again.
7. **Contended at the bound:** return `UpstrokeError::Refused`, naming the store, its incomplete
   entries and the bound, and carrying the last failure's text. **It is never `UpstrokeError::Git`.**

**The bound.** `REGISTRY_CONTENTION_BOUND` is 10 s in production. Tests run with a short value (the
probe used 500 ms), so that a test meeting a torn entry it planted does not wait out the production
bound. The bound is per access and fixed at its first attempt.
- A writer's registration is torn only for the few file writes between its `mkdir` and its
  `commondir`: microseconds (`d2/measure/subprocess-writes-2.43.0.txt`).
- A removal's is torn for the recursive deletion of one directory.
- 10 s is four orders of magnitude over both.

**Where it applies** (`src/workspace_manager.rs` at `0874bcf3`):

| Access | Its attempt | Reached from |
|---|---|---|
| `worktree_records` (`:5051`) | `git worktree list --porcelain -z` | every `revalidate()`, `quiescence`, `assert_publishable`, `derive` |
| `add_worktree` (`:2649`) | `git worktree add --detach --quiet` (`:2708`), inside its funnel | `Worktree.Add`, `.AddStaging`, and `Snapshot.Add` (through `add_snapshot`, `:3200`) |
| `remove_worktree_proving`'s scan (`:2988`) | `revalidate_removal_proving` (`:5119`) | every removal |
| `slots_with_torn_registrations` (`:5345`) | the torn plan's scan | `remove_intent`, `verify_worktree` |

- **The add is attempted again only when its failed attempt left nothing at the slot.** Git's own
  failure paths remove their junk (`remove_junk`), so a race leaves nothing behind. An add killed from
  outside, as by the PR136 kill samplers, leaves its residue, and that failure is returned as it is.
- **Not wrapped:**
  - `remove_bound`'s mutation: once it stops pruning it enumerates nothing (§2.5);
  - `git fsck` on the refusing path (table B);
  - `read_only_git`'s reads, which enumerate no registry.

**Why a race is never returned as Git state.** Take a torn entry that Git failed on. One of three
things is true of it:
- it is still incomplete at the second read (clause 1);
- it changed between the two reads (clause 2);
- it was made and unmade entirely inside the attempt, for example another engine's add that failed and
  removed its junk. Then Git's error names it (clause 3).

File-time granularity can hide a write that falls inside one tick from clause 2 alone. Clauses 1 and 3
do not depend on it. So the only failure returned unchanged is one across a store that read quiet and
whole at both ends, with an error that names no entry of it.

**What a genuine failure costs.**
- **When the store is quiet: nothing.** It is returned at once, unchanged.
- **Under other processes' churn,** it is retried until an attempt meets a quiet store. Measured with 80
  genuine failures (an add of a missing ref) among eight churning loops: every one was returned as
  genuine, after at most 1.085 s (median 0.010 s; `d3/measure/race-tolerant-targeted-genuine.log`).
- **Under churn that never quiets for the bound,** it is returned as the refusal, which is not durable.
  That is a conservative misreading, and the next attempt, on a resume, reads it again.

**Static tears.** Two cases keep the store contended without changing it: a dead writer's torn residue
(R1), and a writer stopped inside its registration write.
- The access refuses at the bound, never as Git state.
- So R1 no longer spends a deferral or parks a candidate. A verification that meets it ends the
  command resumably, naming the entry.
- This narrows `PR308-R3-SKIPPED-PRUNE-KEEPS-ANOTHER-RUNS-TORN-REGISTRATION`'s verification
  consequence. Its precondition is unchanged (§2.5).

**R-X stays as PR11 made it.** It is taken around one attempt at a time. The sleeps are outside it, so
no thread waits on another thread's retries. The bound covers the retries; R-X's wait for another
thread's single Git command is PR11's.

**Measured at the Git level** (`d3/measure/tolerance-race.py`, `SUMMARY.txt`; Git 2.43.0). Two
checkouts, four loops in each; each loop runs list, add, the removal's scan as the manager reads it,
and the removal; 16,000 commands a run.
- **Untolerant:**
  - 102 and 174 failures with Git's prune;
  - 225 and 210 with targeted removal;
  - at 16 loops, 276 and 400 in 32,000.
- **Tolerant:**
  - **0** in ten runs with targeted removal (118 to 174 retries a run);
  - **0** in ten runs with prune (64 to 126 retries);
  - **0** in 32,000 at 16 loops (397 retries).
- **The failure kinds, untolerant,** are the finding's and round 1's: the scan's "is locked and has no
  gitdir"; the add's and the list's "failed to read …/commondir: Success"; "failed to read '…/locked'";
  "Invalid path"; and, under prune, "could not open '…/gitdir' for writing"
  (`race-none-prune-run1.log`).

**Measured at the engine level, with the finding's own witnesses** (`d3/census/witness-r7r8/`).
Review round 7's and round 8's patches were applied unchanged to scratch `git archive` copies of
`0874bcf3`: one unpatched, one with (iii) alone (`patch-iii.py`), and one with the designed shape of
§2.7.
- **R7.** Two `LinkedChild` processes, one in the main checkout and one in a linked checkout. Each holds
  its own worktree and run locks and runs 500 snapshot add-and-remove cycles through the production
  funnels.
  - Unpatched: **red 10 of 10, 68 failed operations in 20,000**.
  - (iii) alone: **green 10 of 10, 0 in 20,000** (`r7-summary.txt`).
  - The designed shape: green 5 of 5, 0 in 10,000 (`r7-design-summary.txt`).
- **R8 as its reviewer wrote it:** a torn foreign registration held until the verification's terminal.
  - Unpatched, 3 of 3 runs: Complete after one Deferred, and Parked after two. That is the finding.
  - (iii) alone, 3 of 3 runs, and the designed shape, 2 of 2: the command ends with `Refused`, and
    nothing durable is appended (`r8-summary.txt`, `r8-design-summary.txt`, `r8static-*-*.log`).
- **R8 transient:** the same tear, finished 200 ms later, as a live writer finishes.
  - Unpatched: one and two `merge_verification_unavailable` (Complete, Parked).
  - (iii) alone, 3 of 3 runs, and the designed shape, 2 of 2: **none; Complete both times**
    (`r8-summary.txt`, `r8-design-summary.txt`).

### 2.5 Targeted removal

**What changes.** `remove_bound` (`src/workspace_manager.rs:3020`) runs no `git worktree prune`. Today
it prunes at `:3059`, `:3098` and `:3121`. Instead:
- **A bound registration that still names the slot is removed directly:** `locked` unlinked, then the
  directory. That is the removal the empty-`commondir` branch already makes (`:3096-3119`).
- **Then the store itself, `<common git dir>/worktrees`, if that left it empty,** as
  `git worktree prune` removed it.
  - The frozen `finalize.rs` test `scrub_slots_converges_when_git_has_pruned_the_emptied_registration_store`
    (`src/engine/topology/finalize.rs:486`, assertion `:502-505`) pins exactly this.
  - Without it, the shape probe failed that test (`d3/census/probe-iii/suite-p1.log`); with it, the test
    passes (`suite-p1b.log`).
- **A registration whose `gitdir` has gone** converges with nothing removed, as `reviews/FINDINGS.md`
  §24's owner-authorized rule says: "without inferring or deleting an administration directory".
- **When no registration is bound,** nothing is removed from the store.

**Why the prune goes.**
- `git worktree prune` removes any entry with neither `locked` nor `gitdir`, with no expiry check
  (`should_prune_worktree`, `d3/git-src/v2.43.0/worktree.c:719-737`, the same at 2.55).
- `git worktree add` makes its directory before it writes `locked`
  (`d3/git-src/v2.43.0/builtin/worktree.c:458-483`).
- So an engine's prune can delete another process's add in flight. **Executed**
  (`d3/witness/prune-in-flight/witness.log`):
  - an add is held between its `mkdir` and its `locked` (strace's syscall delay);
  - a bare `git worktree prune` reports "Removing worktrees/vict: gitdir file does not exist";
  - the add fails: "could not open '…/worktrees/vict/locked' for writing";
  - the store reads the same before and after the victim's attempt.
- Clause 3 would catch that victim, because its error names its entry. Removal should not depend on the
  victim's reading, though: no engine process deletes another process's entry at all.

**The store's own removal cannot race another engine process.** Two engine processes on one repository
are in two checkouts, because R17 refuses a second coordinator in one checkout. So while both run, a
linked checkout's registration is in the store, and the store is never empty. Within one process the
removal runs under R-X.

**What it changes for two filed findings.** The implementation updates both findings' texts.
- **`PR5-RD-003-A-PRUNE-STRANDS-A-CHECKOUT-WHOSE-GITDIR-IS-GONE`.** Its state needs a prune to delete a
  `gitdir`-less entry and then the emptied store. No engine prune does that now, and the store is
  removed only when it is empty, so the engine no longer produces the state (a user's prune still can).
  This is the finding's own second shape: "no forced removal prunes an entry whose checkout may stand".
  Neither of §24's two rules gives way.
- **`PR308-R3-SKIPPED-PRUNE-KEEPS-ANOTHER-RUNS-TORN-REGISTRATION`.** Another run's torn registration,
  which Git's prune would have deleted, is no longer deleted by this run's removals. It stays until its
  own run's resume repairs it, or until an operator runs `git worktree prune`. Its verification
  consequence narrows (§2.4: a refusal, not a deferral).

### 2.6 (c): the dead coordinator's Git writers hold the run's cleanup lease

**The rule.** Every Git child the manager starts with a **writer subcommand** is handed the run's
cleanup lease as its standard input. The writer subcommands are `worktree add`, `add`, `rm`, `clean`,
`write-tree`, `cherry-pick` and `read-tree`. `git update-ref` gets the lease the same way: today it
gets it through `pre_exec` (`src/workspace_manager.rs:3553-3570`), and that moves to this form. A
read never gets the lease, wherever it runs.
- **How.** The parent opens `<run's public dir>/cleanup.lock` and takes `flock(LOCK_SH)` on it. It
  passes the file as the child's stdin, spawns, and drops its own copy at once.
- **Why the hold lasts.** The child's descriptor 0 shares the open file description. So the shared hold
  lasts as long as the child, or any descendant that keeps its stdin, is alive.
- **Platforms.** Unix only, as `hold_cleanup_lease_for_child` is (`src/rundir.rs:2193-2247`). On Windows,
  INV-18's ambient kill-on-close job ends the children with the coordinator, as `src/rundir.rs:2238-2240`
  already says for `update-ref`.

**Why that closes (c).**
- A resume, or any coordinator of the checkout, observes R28 at its worktree-lease acquisition and
  refuses while the lease is held (`observe_cleanup_hold`, `src/rundir.rs:1947-1975`, `:2140`).
- It probes the lease exclusively at its run-lock acquisition (`:2075-2079`).
- So no resume begins while a dead coordinator's Git writer, or a descendant of it, can still act on a
  slot's paths, and nothing rebinds a path under a live orphan.
- On Unix this is a fact the kernel keeps, not a record: a `flock` is released only when the last
  descriptor of its open file description closes.

**Which descendants hold it.**
- Git's `run_command` gives a child its own standard input unless told otherwise. `add_worktree` tells
  neither its `update-ref` child (2.43) nor its `reset` child otherwise: there is no `no_stdin` in
  `builtin/worktree.c` at 2.43, 2.50 or 2.55 (`d3/git-src/`).
- **Measured:** G and its held `reset` both have descriptor 0 on the lease file
  (`d3/witness/lease-stdin/probe-fd0.log`).
- **A filter Git feeds through a pipe does not inherit the lease.** So a filter's background helper
  never holds it, and FUB-D1-FILTER's and PIPE's class does not arise. The filter writes no slot path
  itself either: Git does.
- Hooks are disabled through `core.hooksPath` (`src/workspace_manager.rs:4994-5010`).

**Why stdin and not `pre_exec`.** Four variants were measured, each on top of (iii) in a scratch copy
(`d3/census/probe-iii/SUMMARY-D1.txt`). The (iii)-only control's suite took 103 to 126 s.

| Variant | Suite time | Frozen failures in the full suite |
|---|---|---|
| D1: the lease through `pre_exec`, for every builder-run child | 339 s | four `recover/tests.rs` tests, each passing alone |
| D1 + m1: the same, the parent's copy dropped at spawn | 349 s | two |
| D1s: stdin, for every builder-run child | 82 to 110 s | `kill_after_report_before_each_cleanup_step` in 3 of 3 suites, passing alone |
| D1n: stdin, for the children run in a slot, plus `worktree add` and `update-ref` | 78 to 119 s | none, in 4 suites |
| **D1w, the designed rule: stdin, for the writer subcommands only, with (iii)** | **97 to 105 s** | **none, in 2 suites** (`d3/census/rate-design/`) |

- **D1's time.** A `pre_exec` closure takes `std` off `posix_spawn` and onto `fork()` of the large
  multithreaded process, for every Git child.
- **D1's frozen failures.** They are the parallel suite's sibling forks inheriting the parent's copy of
  the lease. That is PR10's reason for withdrawing exactly this attempt
  (`reviews/2026-09-12-pr10-record.md:431-436`).
- **Reading the lease file** gives end of file, as `/dev/null` does. No command the manager runs reads
  its stdin, and one that could would read the same nothing.

**Executed with the coordinator's real locks** (`d3/witness/c-single/witness-locks.py`). Each probe takes
the worktree lock and then the run lock first, as a write command does.

| Build and mechanism | Resume 1 | Then | `a.txt` at the end |
|---|---|---|---|
| unpatched, mechanism 1 (`witness-locks-exec-base.log`) | proceeds: `Unpopulated`, removed, recreated | the late `reset` runs | `base`: **paid edits lost** |
| unpatched, mechanism 2 (`witness-locks-filter-base.log`) | proceeds the same way | the orphaned add's `remove_junk` runs | the slot is gone: **lost** |
| designed shape, mechanism 1 (`witness-locks-exec-design.log`) | **refused at the worktree lock**: "run … still has a process of its own alive …" | the held child is released and exits; resume 2 reads `verified (reused)` | **paid edits intact** |
| designed shape, mechanism 2 (`witness-locks-filter-design.log`) | refused the same way | resume 2 reads `verified (reused)` | **intact** |

**The resume's experience.**
- It refuses while the dead run's Git writers live: usually milliseconds, at most the add's checkout.
- The message that refuses it already names the run and the lease (`src/rundir.rs:1970`). It gains the
  worktree writers among the processes it lists.
- A Git writer that never exits holds the lease until an operator ends it, as a stuck reaper does today.

**What it does not cover.**
- A descendant that closes or replaces its stdin and then writes a slot. No Git writer measured does.
- Foreign processes.
- The legacy engine's Git children, which are frozen (§2.10).
- On Windows, the ambient job's termination is asynchronous. That is the position `update-ref` and every
  agent hold today (INV-18), reasoned and not executed here.

**Packet: none.**
- R28's row names "a surviving Unix cleanup reaper's shared cleanup.lock hold".
- PR #275 added `git update-ref` children as a second holder. It argued from the mechanism: what
  acquires the hold, what releases it, what observes it, and what reclaims it after a crash
  ("nobody"). It read the change as "the row's description catching up with its membership", and both
  of its review lenses accepted that.
- The manager's writers are a third holder class by the same argument: the same file; the same shared
  take, by a process the coordinator started; released by the kernel at the last close; observed by the
  same two sites; never reclaimed.
- §2.12 gives the one sentence an owner who wants the row to name its holders could adopt. Nothing
  depends on it.

### 2.7 Effect governance, instruments and the frozen set

**The shape probe.**
- The designed shape is `d3/census/probe-iii/patch-iii-c3.py` plus `patch-d1w.py`. It was applied to a
  scratch `git archive` of `0874bcf3`, and the whole suite was run twice through `upstroke-build`
  (`d3/census/rate-design/probe-design-{1,2}.log`).
- Clippy `-D warnings` is clean (`d3/census/probe-design/clippy.log`).
- Nothing of it is on the branch.
- The library suite: 2,993 passed, 7 failed, 113 ignored, in both runs. The control, (iii) without the
  lease, gives 2,998, 2 and 113.
- **One difference from §2.4.** The probe attempted every contended add again. The design's condition,
  that an add is attempted again only when its failed attempt left nothing at the slot, is the
  implementation's to add, and T6 tests it.

**The seven tests that move, each the implementation's to move with the code:**
1. `effects::tests::every_externally_reachable_fn_of_a_legacy_or_shared_module_is_classified`: the new
   `rundir` callable (the probe's `take_cleanup_lease_shared`) needs its `effects/wrappers.toml` row.
2. `effects::tests::every_name_more_than_one_callable_bears_is_pinned_by_its_count`: the callable's two
   `cfg` twins need their count pinned.
3. `runner::contract::tests::every_production_process_start_is_classified`
   (`src/runner/contract.rs:1632`): the `src/workspace_manager.rs` row goes from `(2, 0, 0)` to
   `(2, 1, 0)`, for one `.spawn()` (spawn, drop the parent's copy, collect).
4. `runner::contract::tests::every_production_command_spec_payload_is_classified` (`:2422`): the
   `src/workspace_manager.rs` row goes from `(2, 8, 0)` to `(3, 8, 0)`.
5. `workspace_manager::tests::the_one_update_ref_spawn_gives_its_child_the_cleanup_lease`
   (`src/workspace_manager/tests.rs:14806`), not frozen: rewritten for the writers.
6. `workspace_manager::tests::a_removal_records_the_one_attempt_the_unix_arm_makes` (`:1432`), not
   frozen: the direct removal of the administrative directory is a second observed tree removal. The
   removal either goes unobserved or the test counts both.
7. `workspace_manager::tests::an_add_killed_before_it_wrote_gitdir_is_unlisted_and_refuses_forced_cleanup`
   (`:6675`), not frozen. It is `RESIDUE-UNBINDABLE…`'s guard test. Its refusal is now `Refused`, after
   the bound, carrying the same Git text.

Items 1 to 4 are instruments in the sense of CLAUDE.md's first limb, so the implementation's merge is
the owner's, not standing delegation's.

**No vocabulary moves.**
- No resource row, effect site, fault row, coverage claim or sequential-registry entry.
- `effect_sites.json`, `effects/funnel-modules.json`, `effects/sequential-registry.json` and every pin
  under `src/topology/**` stay as they are.
- `effects/allowlist.toml` stays: the manager already allows the governed methods, and already sleeps
  (`src/workspace_manager.rs:1507`, `:5890`).
- R28's doc comments in `src/topology/effects/{sites,vocab,residue_authority}.rs` gain the third holder
  class. That is doc text only, as #275's change was.

**The frozen set.** Nothing in it is edited, and its tests pass under the shape in both runs, none
failing: `recover` 226, `integrate` 20, `repair` 5, `finalize` 5, `fold` 192, `events::log` 47
(`d3/census/probe-iii/frozen-modules-design.txt`). The one frozen test that needed care is
`finalize.rs`'s (§2.5).

**Documentation the implementation moves.**
- **`DESIGN.md` §15.** The sentence that names `git update-ref` as the lease's Git holder names the
  writers too, and the PROPOSED paragraph (`design/15`) becomes §2's.
- **`docs/internals/engine/topology/run.md:479-490`** and its pin,
  `the_verification_notes_say_a_registry_another_process_is_writing_spends_a_deferral_or_parks`
  (`src/engine/topology/run/tests.rs:535`). The paragraph now says that a registry another process is
  writing never reaches this arm, because it is retried or refused. The mapping, which is the pin's
  second half, is unchanged.
- **`src/rundir.rs`:** `hold_cleanup_lease_for_child`'s doc, and the text of the worktree lock's
  refusal (`:1970`).
- **The notes of `src/workspace_manager.rs`,** for the wrapper, the removal and the lease.
- **The findings.** The repaired finding is deleted. `PR5-RD-003…`, `PR308-R3…`,
  `RESIDUE-UNBINDABLE…` and `PR11-HOST-AGENT-PRUNE-RACES-AN-ENGINE-ADD` have their texts updated
  (§2.8).

### 2.8 What is closed, what remains, and what G6 meets

**The claims.**
1. **No engine registry access returns `UpstrokeError::Git` for a failure another process's registry
   write caused** (§2.4). So, for (b): no registry race reaches `run::verified`'s Git arm, no deferral
   is spent on one, and no candidate is parked on one.
2. **(a):** an attempt's registry access passes another process's write. It ends the command only if
   the store stays contended for the bound, and then resumably.
3. **No engine process deletes another process's registration** (§2.5).
4. **(c):** on Unix, no resume and no other coordinator of the checkout begins while a dead
   coordinator's Git writer is alive, or a descendant of it that keeps its stdin (§2.6). So recovery
   never rebinds a path under one.

**What remains.**

| | What | Consequence now | Finding |
|---|---|---|---|
| R1′ | A writer stalled inside its registration write for the whole bound; a dead writer's torn residue | the access refuses resumably, naming the entry; never Git state, never durable | `PR308-R3-…`: consequence narrowed, precondition unchanged |
| R2 | A host agent's own Git | its torn entries are tolerated (clauses 1–3); its prune of an engine add in flight makes the add fail with an error naming its entry, so the add is attempted again (clause 3); the agent's own prune still deletes the entry | `PR11-HOST-AGENT-PRUNE-RACES-AN-ENGINE-ADD`: narrowed to the agent's own commands; stays filed for them |
| R3 | The user's Git in any checkout | tolerated the same way | none |
| R4 | `fsck` on the refusing path (table B) | unchanged: a refusal either way | none |
| R5 | (c) on Windows | INV-18's ambient job ends the children, asynchronously | the existing position, as for `update-ref` and agents |
| R6 | The legacy engine | §2.10 | `PR329-LEGACY-RUNS-IN-LINKED-CHECKOUTS-RACE-THE-SHARED-WORKTREE-REGISTRY` (B2′), or closed (B1′) |
| R7 | File times coarser than one attempt | clause 2 can miss a write within one tick; clauses 1 and 3 do not | stated |

**What G6 meets.** This is the classification the orchestrator's addendum asks for
(`~/orch-pr11/answers/pr11_fub_design3-0.md`), each line with its evidence above.

| Item | Severity | Here | Applies to G6 | Blocks G6 |
|---|---|---|---|---|
| `PR11-LINKED-CHECKOUTS-RACE-THE-SHARED-WORKTREE-REGISTRY`: (a), (b) | P1 | closed by §2.4 and §2.5 once implemented; the finding file is deleted then | yes: R17 and the registry under concurrency, ST-16 | only until this change merges |
| (c), a dead coordinator's Git children: FUB-D2-DESC, and `PR329-A-RESUME-REBINDS-A-SLOT-ITS-DEAD-COORDINATORS-GIT-CHILD-STILL-WRITES` in the ledger | P1 | closed by §2.6 once implemented | yes: Q1, ST-16, ST-18 | only until this change merges |
| `PR329-LEGACY-RUNS-IN-LINKED-CHECKOUTS-RACE-THE-SHARED-WORKTREE-REGISTRY` | P2 | filed at this head (B2′), or closed under B1′ (§2.10) | no: G6 certifies the topology engine and claims nothing of the legacy engine frozen at PR5 | no |
| `PR308-R3-SKIPPED-PRUNE-KEEPS-ANOTHER-RUNS-TORN-REGISTRATION` | P2 | consequence narrowed | — | no |
| `PR5-RD-003-A-PRUNE-STRANDS-A-CHECKOUT-WHOSE-GITDIR-IS-GONE` | P2 | no longer produced by an engine's removal | — | no |
| `PR11-HOST-AGENT-PRUNE-RACES-AN-ENGINE-ADD` | P2 | narrowed (R2) | — | no |

### 2.9 Regression tests

Each test below is the implementation's, and its name is the implementer's to choose. Its first-bad
shape is given, and each is "fixed (design); witnessed in the implementation phase" in the ledger.
Every one lives outside the frozen modules and their test children. Each waits on a handshake or a
seam; time is only a watchdog.

- **T1, two processes in linked checkouts, at least 1,000 cycles, 0 failures.** This is review round 7's
  witness, kept (`~/orch-pr11/reviews/r7-witnesses/conc/witness.patch`).
  - First-bad: unpatched it is red 10 of 10, with 68 failures in 20,000 operations
    (`d3/census/witness-r7r8/r7-summary.txt`).
  - Mutation m1: the wrapper returns its first failure. Red.
  - Windows runs bounded cycles, gated by `cfg` and said in the test's doc.
- **T2, the verification beside a transient foreign tear: no deferral.** This is round 8's witness,
  inverted.
  - A foreign `LinkedChild` tears an entry and keeps it torn until the reader's attempt has met it. A
    test-only notice in the wrapper ("an attempt was contended") is the handshake. Then the foreign
    process finishes the entry.
  - Pass: no `merge_verification_unavailable`, the outcome Complete, invocations balanced, and replay
    equal to live.
  - First-bad: unpatched, one and two deferrals (Complete, then Parked) (`r8-summary.txt`).
- **T2′, the verification beside a static tear (round 8's witness as written).**
  - Pass: the command ends with `Refused`, naming the entry; nothing durable is appended; the next
    resume completes once the entry is gone.
  - First-bad: unpatched, Deferred and Parked (`r8static-base-*.log`).
  - Mutation m2: the bound's error typed `UpstrokeError::Git`. Red: the verification defers.
- **T3, three processes:** T1's shape with three `LinkedChild` processes.
- **T4, the classifier, as unit tests with the store built by hand.**
  - Each clause makes a constructed failure contended: an incomplete entry; a change between the reads;
    an error naming `<store>/<name>`, absolute and relative.
  - A quiet, whole failure is returned unchanged and at once.
  - Contended at the bound is `Refused`, never `Git`.
  - Mutations: each clause removed in turn, with the case that only that clause catches.
- **T5, no engine prune.**
  - A census: `worktree` with `prune` appears in no production argv of `src/workspace_manager.rs`.
  - The store is removed only when empty: a removal beside another registration leaves the store.
  - The frozen `finalize.rs` test stays green.
- **T6, an add whose entry another process prunes.** A seam holds the manager's add between its
  `mkdir` and its `locked`, the way `d3/witness/prune-in-flight/` does with strace, while a foreign
  `git worktree prune` runs.
  - Pass: the add is attempted again and succeeds.
  - First-bad: unpatched, the add fails with Git state.
- **T7, (c) with one coordinator** (Unix). A coordinator child is killed inside `worktree add`, its
  `reset` held by a `GIT_EXEC_PATH` wrapper or by a smudge filter.
  - Pass: the resume is refused while the held process lives. After it exits, the resume reuses the
    slot, and the worker's edit survives.
  - First-bad: unpatched, the edit is lost in both mechanisms (`d3/witness/c-single/witness-locks-*-base.log`).
  - Mutation m3: the lease not handed (stdin `/dev/null`). Red.
- **T8, the lease's holders** (Unix).
  - Descriptor 0 of `worktree add` and of its `reset` is the lease.
  - A filter's background helper does not hold it.
  - A read the manager runs in a slot (`rev-parse`) holds nothing: `Worktree.Verify` takes no R28 hold
    (FUB-D2-VERIFY).
- **T9, the bound.** Four threads meet a static torn entry with a 1 s test bound. Each returns `Refused`
  within the bound plus one backoff and one attempt. No thread's wait is a multiple of the bound, since
  the sleeps hold nothing.
- **T10, the legacy writer.** A legacy `Workspace` gate snapshot add and its drop, in a linked checkout,
  run beside the manager in the main checkout. The manager has 0 failures. That is the engine-level form
  of `d3/measure/race-mixed-run*.log`.

**The budget.** The Windows guest's harness ran 468.44 s at `78f99c70`
(`~/orch-pr11/logs/pr11_repair_r8/ci/ci-read-36861160153.txt`). The Unix-only tests (T7, T8 and the
lease) cost the Windows legs nothing. T1 and T3 run bounded cycles there.

**The proof the implementer owes.**
- Each mutation, on a scratch tree whose Compiling line names it.
- The witnesses red unpatched.
- The frozen children unchanged.
- The ten gates, and CI on every leg.
- **At least five full suites, with every frozen test's failures counted against the same number of
  suites at the base.** The lease's sibling-fork exposure is what PR10 withdrew it for; here it is
  measured clean in 2 suites of the designed rule and 4 of D1n.

### 2.10 The legacy path: decision B

**(i) Do tolerant topology readers close R7-CONC-1's consequences against a legacy writer in a linked
checkout? Yes. Measured** in five mixed runs (`d3/measure/race-mixed-run{1..5}.log`):
- The main checkout ran the legacy engine's own argv, untolerant, as the frozen `src/workspace.rs` runs
  them: `add -q --detach --force`, `list --porcelain -z`, `remove --force`.
- The linked checkout ran the shape's cycle.
- **The topology side failed 0 times in 40,000 commands.** The legacy side failed 13, 10, 14, 5 and 13
  times in 6,000 a run.
- So after PR12, a legacy run cannot tear a topology verification or pipeline. Tolerance does not care
  who the writer is.

**(ii) The legacy engine against itself, in production today.**
- **The sequence.**
  - Two legacy runs work in linked checkouts of one repository.
  - One run's gate-snapshot add, list or remove, or its resume's `switch`, dies on the other's
    half-written entry (§1.2, table C).
  - `?` reaches `discard_uncommitted()` (`src/engine/coordinator.rs:544-548`): the worker's uncommitted
    edits are discarded and the command ends.
  - The resume runs the attempt again.
- **Measured at the Git level** by round 1 (`~/orch-pr11/logs/pr11_fub_design/measure/legacy-race-SUMMARY.txt`):
  12 and 16 failed commands in 7,200 with four loops per checkout, and 0 in 5,400 with one loop per
  checkout, the shape one legacy coordinator per checkout gives.
- **Severity: P2.**
  - The cost is one attempt's paid work and a resumable end.
  - Nothing durable is spent: the legacy path has no deferral or park, and its log stays consistent.
  - The realistic rate is low.
- **G6: it does not apply.** G6 certifies the topology engine's scheduling layer and R17 under
  concurrency, and claims nothing of the legacy engine, frozen at PR5. As a P2 it would not block G6
  anyway.

**B1′, if the owner unfreezes `src/workspace.rs` for this change.** The text below would replace round
2's §1.9 text, which is withdrawn.
- **Scope, one file:**
  - the four registry children (`add_gate_worktree` `:871`, `cleanup_gate_workspace` `:1549`,
    `worktree_is_registered` `:1602`, `switch_branch` `:450`) each run through the manager's tolerant
    registry access, exposed `pub(crate)` for the purpose;
  - one private helper resolves the canonical common git dir, the two steps `recorded_objects_scope`
    already takes (`:97-101`);
  - nothing else of the module moves, and no legacy engine module moves;
  - no lock file and no new prerequisite, so round 2's `ensure_execution_prerequisites` change is gone
    with the lock.
- **The recorded form** is the `effects/allowlist.toml` row's `legacy_effect` text, extended. Its
  `path` and `allows`, and `FROZEN_LEGACY_ALLOWLIST` (`src/effects.rs:1306`), do not move.

  > … **AMENDED TWICE: to close
  > `PR329-LEGACY-RUNS-IN-LINKED-CHECKOUTS-RACE-THE-SHARED-WORKTREE-REGISTRY`, on the owner's decision
  > to unfreeze the module for this one change. The second amendment is one thing and no more: the four
  > Git children that enumerate the repository's worktree registry — `add_gate_worktree`'s `worktree
  > add`, `cleanup_gate_workspace`'s `worktree remove`, `worktree_is_registered`'s `worktree list` and
  > `switch_branch`'s `switch` — run through the workspace manager's tolerant registry access, which
  > retries a failure the store shows contended and refuses one that stays contended past its bound,
  > never as Git state; with one private helper resolving the canonical common git dir as
  > `recorded_objects_scope` does. Nothing else in the module moves, and no other legacy module
  > moves.** Every other behaviour of the module stays frozen.

- **What it closes:** the legacy-side race; the finding is deleted.
- **What it leaves for G6:** nothing of this race.
- **Its test:** T10 with the roles swapped, the legacy side measured.

**B2′, if the module stays frozen: the filing.** This head adds
`findings/P2_correctness_202610020230_legacy-runs-in-linked-checkouts-race-the-shared-worktree-registry.md`
(`PR329-LEGACY-RUNS-IN-LINKED-CHECKOUTS-RACE-THE-SHARED-WORKTREE-REGISTRY`, P2, `deferred`,
`pre_existing`). Its guard is the change that routes the four calls through the tolerant access under
the owner's unfreeze, which is B1′ in this pull request or a later change.
- **What it leaves for G6:** one open P2 that does not apply to G6 and does not block it.

**Recommendation: B2′.**
- The topology closure does not depend on the legacy path.
- The owner's direction was "narrow and split out work".
- The race is P2 and outside G6.
- B1′ stays available as a one-file change whenever the owner wants it. If the owner takes B1′ in this
  pull request, the implementation deletes the finding file, and its ledger row becomes `fixed`.
- **Not established by this round:** whether a dead legacy coordinator's Git children can corrupt what
  its own resume does (§2.13). The legacy engine's Git children hold no lease.

### 2.11 Review rounds 1 and 2, answered

**Design review round 2** ran three `gpt-6-astra` lenses at `max` on `0874bcf3`, 23:42–23:58Z on
2026-10-01. All three returned CHANGES_REQUIRED. The texts are
`~/orch-pr11/reviews/review-329-d2-{design,concurrency,regression}-0874bcf3.review.md`, with their
hashes in `SHA256SUMS-329-d2`; the triage is `review-329-d2-triage.md`.

| Finding | Sev | Kind | What it found | Round 3's answer | Where | Evidence |
|---|---|---|---|---|---|---|
| FUB-D2-DESC | P1 | executed | Git's unrecorded `update-ref` and `reset` descendants outlive their killed parent and overwrite a recreated workspace or a replacement registration's `HEAD` | Re-executed with **one** coordinator, in two mechanisms; the class is PR136's. Closed by the kernel: the run's cleanup lease is handed to every Git writer as stdin, and the descendants inherit it, so no resume begins while any lives. No quiescence claim is made. | §2.2, §2.6 | `d3/witness/c-single/`, `witness-locks-*-{base,design}.log`, `d3/witness/lease-stdin/probe-fd0.log` |
| FUB-D2-RECORD | P1 | executed model | updating a PID-only record line is not crash-safe | No record exists. | §2.1 | — |
| FUB-D2-ENOENT | P1 | reasoned | under `hidepid`, `/proc` reports a hidden live process absent | No process is queried. The lease is kernel lock state, which every user sees through `flock`. | §2.6 | — |
| FUB-D2-PIPE | P2 | executed | a filter helper holds the captured output pipe, so the lock is never released | No cross-process lock is held across output collection. A helper holding Git's output pipe delays that one call, and R-X's other in-process users behind it, exactly as at `92c4ca81`; no other process waits on it. The lease is not on the helper's descriptors (a filter's stdin is a pipe), so it is released when Git exits even while the helper lives. | §2.6 | `probe-fd0.log` |
| FUB-D2-ERRATUM | P2 | executed | an inherited `flock` survives fork-before-exec, contradicting R17's "released at process exit" | No new lock and no erratum: R17 is untouched. The cleanup lease's fork copies are R28's known, measured window (PR281, #320), and this design keeps it to the spawn: the parent drops its copy at once, and reads take none. | §2.6, §2.12 | `d3/census/probe-iii/SUMMARY-D1.txt` |
| FUB-D2-WINAPI | P2 | reasoned | exposing the raw Windows `resume_only_thread` needs a classification and a denial | Nothing in `agent::proc` is exposed; Windows takes no lease. | §2.6 | — |
| FUB-D2-VERIFY | P2 | reasoned | `Worktree.Verify` becomes effectful while it is classified read-only | The tolerant read only reads, and only writers take the lease: Verify's `rev-parse` and list take nothing, and the site stays read-only. | §2.4, §2.6, T8 | `patch-d1w.py` (the writer rule) |

**Design review round 1's findings, under round 3.** Rounds 1 and 2 answered them for a lock; round 3
withdraws the lock.

| Finding | Sev | Status under round 3 |
|---|---|---|
| FUB-D1-WIN | P1 | No lock, so no release order is needed. (a) and (b) are read-side. (c)'s Windows side is INV-18's ambient job, the existing position, reasoned (§2.8, R5). |
| FUB-D1-PERM | P1 | No new file. The lease file is created by `RunLock::acquire` at the command's start, before any spend, as today. The legacy path is untouched (B2′). |
| FUB-D1-HOOKS | P2 | No new hooks or sites. The add's checks inside its funnel are unchanged. |
| FUB-D1-DEADLINE | P2 | R-X is unchanged from PR11 and claimed bounded nowhere. The retries have one bound per access, and their sleeps hold nothing, so threads do not multiply it (T9). |
| FUB-D1-FILTER | P2 | Nothing is handed to Git's children but the lease on stdin, which filters do not inherit (§2.6). Round 2's answer was incomplete for pipes (PIPE); round 3 holds nothing across output collection. |
| FUB-D1-ERRATUM | P2 | No erratum (§2.12). Round 2's answer left the fork copy unaccounted (D2-ERRATUM). Round 3 adds no lock. |
| FUB-D1-DELETE | P2 | No new file. The lease file is R21's run-directory file, as today. |
| FUB-D1-T2 | P2 | The principle is kept: T2 waits on a seam showing the reader's contended attempt, and time is a watchdog (§2.9). |
| FUB-D1-FROZEN | P2 | `derive` keeps its `revalidate()`. Its tolerant list takes no lock, creates no file and starts no writer, so the frozen refusals precede every effect as before, and the frozen tests pass under the shape (§2.7). Round 2's narrowing of `derive` is withdrawn. |
| FUB-D1-PIN | P2 | The instrument census is measured on the designed shape by a whole-suite probe (§2.7). Nothing in `src/topology/**` moves except R28's doc text. |

**Round 2's three open items** (`~/orch-pr11/handovers/pr11_fub_design2.md`):
1. **The truncation remedy's condition** (§1.3.9). Withdrawn with the record.
2. **One host and one PID namespace.** No PID or `boot_id` exists in round 3. Tolerance reads the store,
   so it holds across hosts and namespaces. The lease is R28's `flock`, whose reach is the existing one:
   a network filesystem without `flock` refuses, as the run and worktree locks already do there.
3. **The unwinding `Drop` rule.** No record exists. The parent's copy of the lease is a `File` dropped
   when the child is spawned, or on unwinding. The child's copy is the kernel's to release.

### 2.12 Decision A: none is needed

No packet text is required:
- no resource row (R29 is withdrawn);
- no effect site (the two Lock sites are withdrawn);
- no fault row (T-REGISTRY is withdrawn);
- no change to R17;
- no Class C.

The packet's `tasks/k<key>-g<gen>`, `merge/s<seq>` and `task_dispatched.worktree_path` are untouched
(§2.3).

R28 gains holders by PR #275's precedent, without an amendment (§2.6). An owner who prefers R28's row to
name its holders could adopt this sentence. **It is optional, and nothing depends on it:**

> *`decisions.resource_accounting.rows[R28].resource`:* "a surviving Unix cleanup reaper's shared
> cleanup.lock hold (one per reaper; a reaper may outlive the coordinator while it settles its process
> groups), **and the same shared hold of each Git child the coordinator started to write a ref, a
> worktree or a worktree's registration, held through a descriptor that child and its descendants
> inherit, for as long as one of them keeps it**"; *`granularity`:* "per reaper process **or Git
> child**".

### 2.13 Risks, sequencing, and what is out of scope

**Sequencing.** The implementation does not depend on #328.
- #328 does not touch `src/workspace_manager.rs`, `src/rundir.rs` or `src/runner/contract.rs`.
- The two changes meet only in `effects/wrappers.toml` (different module rows) and in `design/15`
  (different paragraphs).
- Round 2's dependency was on `src/agent/proc.rs`, which round 3 does not need.

**Risks.**
- **The lease's sibling-fork exposure in the parallel suite.** The suite hosts many runs in one process,
  so a sibling test's fork can inherit a copy of the parent's lease for the length of a spawn. That is
  what PR10 withdrew the add's lease for. The designed rule was measured clean in 2 full suites, and D1n
  in 4. The implementation owes the comparison in §2.9.
- **Liveness of a resume.** It waits for a dead run's Git writers. A hung writer holds the lease until
  it is killed, as a stuck reaper does today. The refusal names the run and the lease.
- **Genuine failures under sustained churn** wait up to the bound (10 s) and are then returned as a
  non-durable refusal (§2.4).
- **Lost healing.** Another run's torn registration is no longer deleted by this run's removals: it
  waits for its own run or an operator (`PR308-R3-…`, §2.5).
- **Windows (c)** rests on INV-18's ambient job, asynchronously (R5).
- **File times coarser than one attempt** weaken clause 2 alone (R7).
- **Git versions.** This box has 2.43.0. CI has 2.50.1 on the Windows guest and 2.55.0 elsewhere,
  including the hosted Windows legs (from round 2's CI logs).
  - From 2.43 to 2.55, `add_worktree` writes `locked` first, then `gitdir` and `commondir`.
    `remove_junk` deletes by path, and the sibling scan comes before the `mkdir`.
  - From 2.50 on, `add_worktree` writes `HEAD` in-process, with no `update-ref` child, and its `reset`
    child still inherits stdin (`d3/git-src/`).
  - Git for Windows runs the same `builtin/worktree.c`. Its torn states are the same; Windows sharing
    violations on a read count as contention (clause 1's unreadable file).
- **`hold_cleanup_lease_for_child` loses its production caller.** It is kept for the fixtures that use
  it (`src/workspace_manager/fixture.rs:761`) or moved to them; the implementation decides.

**Out of scope, and said so.**
- **A dead legacy coordinator's Git children against its own resume.** The legacy engine's Git
  children hold no lease, and this round did not measure whether the legacy resume reuses a path one
  of them can still write. It is not filed: no failure sequence is established.
- **`RESIDUE-UNBINDABLE-TASK-REGISTRATION-HAS-NO-DESIGN-SENTENCE`'s policy.** With the lease, a resume's
  run lock proves on Unix that no Git writer of its run is alive. That could license
  `WriterProof::NoWriterAlive` at a resume's reclaims, but this change does not use it. The finding's
  premise sentence is updated, and its policy question stays open.
- **Foreign Git (R2, R3)** is tolerated by the reader, but its own commands are not the engine's to
  exclude.

## 3. Round 4 design (narrowed)

> **PROPOSED — for design review round 4.** This section supersedes §2 where it says so (the banner at §2's head
> lists where). It is narrowed: #329 repairs the registry race and nothing else. The topology closure needs no owner
> decision. It adds no resource row, effect site or fault row, edits no frozen module and moves no instrument (§3.6).
> **Decision B is the owner's** (`~/orch-pr11/ESCALATION.md` item 7). §3.10 prepares B1′, wrapping the four legacy
> registry enumerators, as the proposal, and keeps B2′, the filing, as the fallback. Neither is implemented. Nothing in
> §3 is in force until the implementation lands.

Design round 4 is `pr11_fub_design4`'s (`claude-opus-5-5`, `max`), spawned on `8dd2214c` to carry out the
orchestrator's decision on design review round 3 (`~/orch-pr11/reviews/review-329-d3-triage.md`). Its figures are under
`~/orch-pr11/logs/pr11_fub_design4/`, cited as `d4/…`.

### 3.1 The looping signal again, the narrowing, and why this round converges

**The signal appeared again.** MAINTAINING's second signal is "A pass finds a P1 in machinery an earlier round of this
pull request added". Design review round 3 (`8dd2214c`) found P1s in round 3's own machinery. That machinery was the
run's cleanup lease handed to the manager's Git writers as standard input, which closed (c), the corruption a dead
coordinator's Git children cause.
- A smudge filter's background helper gets a pipe, not the lease. It outlives Git and reverts paid edits in the
  recreated slot. Executed by all three lenses (FUB-D3-DESC-FILTER). Git's own `checkout--worker` gets a pipe too.
- On Windows nothing establishes that the dead coordinator's job has emptied before a slot is recreated
  (FUB-D3-DESC-WIN).

That is the third remedy for (c) to fail review: a lock handed to the child (round 1), a process record (round 2), and
a lease on standard input (round 3). The signal has now appeared three times.

**The narrowing.** Narrowing is the author's (MAINTAINING, "When a pull request may be looping").
- **This round keeps what survived:** tolerant registry reads and targeted removal. Every lens of round 3 judged that
  direction right for (a), (b) and (d), the registry race `PR11-LINKED-CHECKOUTS-RACE-THE-SHARED-WORKTREE-REGISTRY`.
- **It splits out what did not converge.** (c) is not R7-CONC-1. It is a crash-recovery corruption class, reachable
  with one coordinator, and older than PR11 (§3.9). It is filed as its own P1 finding,
  `PR329-A-RESUME-REBINDS-A-SLOT-ITS-DEAD-COORDINATORS-GIT-CHILD-STILL-WRITES`. Its file carries rounds 1–3's
  evidence, their lessons, and follow-up C's candidate directions. Follow-up C takes it up before G6.
- MAINTAINING's rule for this case is exactly that: "keep what has survived a pass, drop the machinery those rounds
  invented, and record what it was for as a finding carrying its proposal".
- **The title changes** to name only what the pull request repairs. Round 3's title named the withdrawn lease ("a
  resume waits out a dead coordinator's Git writers"), and MAINTAINING says "a title still naming a withdrawn fix is
  the next finding".

**Why this round converges.** What remains of round 3's review against the race are refinements of a direction every
lens accepted, not new machinery. Each has an executed witness, and this round ran every one of them against round 4's
shape on a scratch copy (§3.3, §3.4; `d4/census/probe-iv/SUMMARY.txt`):
- **FUB-D3-TORNOK** is a classifier refinement. The parse moves inside the attempt, and the reads cover `HEAD`.
- **FUB-D3-PERM** is a classifier refinement. Contention must be evidence that another process wrote the store; the
  add's own entry is read as such evidence only after one confirming attempt.
- **FUB-D3-BOUND** is a bound refinement. There is one deadline per access, and R-X stops serialising, so that a
  bounded wait for it is not a new refusal (§3.4 says why the first form failed).

None of the three adds a process, a file, a lock across processes, or anything that must know which processes exist.
The R-X change removes waits rather than adding them. The DESC machinery, where rounds 1–3 kept finding the next
process the construction could not see, leaves this pull request entirely.

### 3.2 What #329 repairs now, and what it no longer claims

| | Consequence | Round 4 |
|---|---|---|
| (a) | An attempt's registry access fails on another process's half-written entry; the coordinator cancels its other pipelines and ends the command (`src/engine/topology/coordinator.rs:1413`). | **Repaired** (§3.3–§3.5). |
| (b) | The same failure in a verification is `UpstrokeError::Git`; `run::verified` maps it to `Verified::Unavailable` (`src/engine/topology/run.rs:279`), and the frozen `integrate.rs` spends a deferral or parks the candidate. | **Repaired.** |
| (c) | A dead coordinator's Git writer, or a process it started, writes into the slot its resume recreated. | **Not repaired here.** Filed P1, blocks G6, follow-up C (§3.9). Round 3's lease on stdin is withdrawn with every claim that #329 closes (c). |
| (d) | A legacy writer in another checkout tears a topology reader. | **Repaired**: tolerance does not care who the writer is (§2.10's mixed runs; §3.3). |
| (e) | A legacy reader dies on another checkout's half-written entry, and the legacy coordinator discards the attempt's paid output. | **Re-graded P1** in its finding file. B1′ is proposed, pending the owner's decision B (§3.10). |

### 3.3 Tolerant registry access, as round 4 specifies it

**The attempt.** One attempt is the registry access's whole unit of work:
- the list is `git worktree list --porcelain -z` **and** `parse_worktree_records` over its output;
- the add is `git worktree add --detach --quiet`;
- the removal's scan is `revalidate_removal_proving`;
- the torn plan's scan is the body of `slots_with_torn_registrations`.

**Output the parser refuses is a failed attempt**, classified like any other. That is FUB-D3-TORNOK's repair. Round 3
classified the Git command alone and parsed afterwards (`src/workspace_manager.rs:5067`). Git 2.43 exits 0 over the
prefix the design-recast lens constructed (`gitdir` written, `locked` holding `initializing`, `HEAD` opened and empty,
no `commondir`). It prints a record whose `HEAD` is the zero id, with neither `branch` nor `detached`
(`d4/witness/git-level.log`, TORNOK (a)). The parser refuses that record as `UpstrokeError::Git`
(`src/workspace_manager/parsers.rs:540`), so round 3 returned it as Git state. `src/workspace_manager/tests.rs:6339`
already documents this output.

**Output the parser accepts is a success,** even when the second read finds an entry still in progress. Two outputs a
writer in progress produces are accepted:
- Git's own placeholder `HEAD` (the zero id) before `commondir` exists prints a detached record (`git-level.log`,
  TORNOK (b));
- so does every state after `commondir` is written.

Those records are foreign, and no consumer reads a foreign record's `HEAD`. `revalidate` reads paths
(`src/workspace_manager.rs:1712`), `assert_publishable` reads branches (`:3434`), and `quiescence` reads the record of
its own slot (`:5070`). A record's path comes from `gitdir`, which Git writes before any record can show the entry.
Until an add's own `update-ref` or `symbolic-ref` child runs, its record is detached at the zero id, so it claims no
branch; after that it shows the branch the add checks out. Refusing accepted output would only turn a dead writer's
parseable residue into a refusal of every list.

**The two reads.** Each attempt is bracketed by two reads of the store, `<common git dir>/worktrees/`: one immediately
before the attempt and one immediately after a failure.
- A read is either the reason the store could not be listed (an absent store is no entries), or, for each entry by
  name, the state of four files: `gitdir`, `commondir`, `HEAD` and `locked`.
- Each file is absent, or present with its bytes (the files are tens of bytes; the read takes at most 4 KiB), its
  modification time and its inode, or unreadable with the error kind that refused it.
- Round 3 read three files, by length and modification time only. Adding `HEAD` is FUB-D3-TORNOK's other half.
  Adding the bytes and the inode closes round 3's same-tick blind spot: Git's add rewrites `HEAD` from the zero-id
  placeholder to the commit, which is the same length, through a lock file and a rename, so the inode changes.

**The classifier.** A failed attempt is **contended** when any of these holds:
1. **An entry in progress (C1).** At either read, some entry's `gitdir`, `commondir` or `HEAD` is absent or empty.
   - Git 2.43's add writes `locked`, `gitdir`, the checkout's `.git`, `HEAD` and then `commondir`, each file opened
     and truncated before it is written (`d3/git-src/v2.43.0/builtin/worktree.c`).
   - So every prefix of an add before `commondir` holds its bytes has one of the three absent or empty, and so does
     every prefix of a removal that has deleted one of them. After that, Git reads the entry whole, as a detached
     record, and a reader has nothing to retry.
   - A whole registration has all three written.
2. **The store changed (C2).** The two reads differ: an entry appeared or went, or one of its four files differs in
   presence, bytes, modification time, inode or error kind.
3. **An entry that existed only during the attempt (C3).** The failure names an entry that neither read holds.
   - "Names" means one of three things. The failure's text contains the store's path followed by the entry's name: the
     canonical spelling, or the spelling relative to the directory the command ran in, which is how Git prints it
     (`fatal: failed to read .git/worktrees/<name>/commondir`). Or the parser refused output that lists a checkout no
     entry of either read registers through its `gitdir`. Or a scan refused the administrative directory by name.
   - Git read that entry during the attempt, and no read sees it. So another process made it and removed it while
     the attempt ran: an add that failed and removed its junk is the case.

**The add's own entry.** When neither C1 nor C2 holds, and every entry C3 finds carries the add's own administrative
name, the attempt is not contended. That name is the slot's basename, or the basename followed by the decimal counter Git appends on a
collision (`add_worktree` in `builtin/worktree.c`; a slot component is ASCII alphanumerics, `-` and `_`,
`src/workspace_manager/naming.rs:161`, so Git's sanitising leaves it alone). The add is attempted **exactly once
more**, and a failure of that second attempt that names only its own entry is returned unchanged.
- This is FUB-D3-PERM's repair. Its construction names the add's own entry: "`fatal: could not create directory of
  '.git/worktrees/new': Permission denied`".
- Nothing in the store distinguishes that from contention, and a second try does. The store is byte-identical before
  and after (`git-level.log`, PERM), and Git prints the same text the second time.
- The second attempt is what lets the rule also cover three contention cases that name the add's own entry:
  - another process's prune deleting the add's entry in flight (R2's case, `PR11-HOST-AGENT-PRUNE-RACES-AN-ENGINE-ADD`);
  - a removal emptying and deleting the store between Git creating it and creating the entry;
  - another run's same-named slot (`kalpha-g1` is every run's first task) made and unmade during the sibling scan.
- An entry of another slot whose name has the own form (`kalpha-g12` beside `kalpha-g1`) is read as the add's own.
  That costs the one second attempt and nothing else.

**What each verdict does.**

| Verdict | Then |
|---|---|
| Not contended | Returned at once, **unchanged**: the error it was, Git state where it was Git state. |
| Own entry only | One more attempt, if the add left nothing at the slot; that attempt's failure is classified again, and an own-entry verdict then is returned unchanged. |
| Contended | Another attempt after a backoff (1 ms doubling to 50 ms), if the deadline allows (§3.4). At the deadline, `UpstrokeError::Refused` naming the store, the reason and the deadline, and carrying the last failure's text. **Never `UpstrokeError::Git`.** |

- **The add is attempted again only when its failed attempt left nothing at the slot**, as round 3 specified. Git's
  own failure paths remove their junk (`remove_junk`). An add killed from outside leaves residue, and its failure is
  returned as it is.
- **An unreadable file is not C1.** A file Git is writing is readable, and empty until written. Permission is not a
  writer, so a quiet store with a file nobody may read is quiet. An unreadable file is contention only when it
  changes, which C2 sees. Windows reports a file being deleted, or opened by another process without read sharing, as
  unreadable; while that changes it is C2. One held unreadable across both reads is classified quiet (§3.7, R9).

**Why it is exact, in both directions.**
- **A failure another process's registry write caused is never returned as Git state.**
  - The entry Git or the scan failed on is in progress at a read (C1), changed between the reads (C2), or existed only
    between them. In the last case it is named, so C3 applies, or it is the add's own name, which gets the second
    attempt.
  - The measured failure kinds of §2.4 all name their entry: "failed to read …/commondir: Success", "failed to read
    '…/locked'", "Invalid path '…/<name>'", "could not open '…/<name>/gitdir' for writing" and the scan's "is locked
    and has no gitdir" (`d3/measure/race-none-*.log`).
  - A refused parse names its checkout, which C3 matches against the `gitdir` of every entry either read holds.
- **A failure across a quiet, whole store is returned unchanged.** Whole means no C1, quiet means no C2. On such a
  store, C3 can find only an entry that existed during the attempt, which is not quiet, or the add's own entry, which
  costs exactly one more attempt. FUB-D3-PERM's construction is executed below. Its quiet store with an unwritable
  directory returns Git state after two attempts, in 1–2 ms, never `Refused`.
- **A refusal is never Git state, and Git state is never retried into a refusal on a quiet store.** Round 3 could not
  say the second half.

**Executed on round 4's shape** (`d4/census/probe-iv/`; each test alone, three rounds, on the unpatched tree, round 3's
tolerance and round 4's shape; `witness-runs/TABLE.txt`):

| Witness | Unpatched | Round 3's tolerance | Round 4's shape |
|---|---|---|---|
| TORNOK, static: the lens's prefix as a foreign entry, then a list | Git, 0 ms | **Git, 0 ms** (first-bad) | `Refused` at 500 ms, naming the entry |
| TORNOK, transient: the writer finishes 150 ms later | Git, 0 ms | Git, 0 ms | Ok, 168–170 ms |
| TORNOK, transient, with T2's handshake (round 4 only) | — | — | Ok after one contended attempt |
| Control: a whole entry whose `HEAD` names nothing (Git prints the TORNOK record shape) | Git, 0 ms | Git, 0 ms | **Git, 0 ms**: returned unchanged |
| PERM: one complete registration, the store made unwritable, then an add | Git, 1 ms | **`Refused`, 525–527 ms** (first-bad) | **Git, 1–2 ms**, two `git worktree add` attempts, nothing created at the slot |

The witness test files are kept in `d4/census/probe-iv/` (`d4-witnesses-common.rs`, `d4-witnesses-iv.rs`). The
implementation's tests (§3.8) are written fresh.

**Where it applies** (`src/workspace_manager.rs` at `8dd2214c`, which is master's):

| Access | Its attempt | R-X (§3.4) |
|---|---|---|
| `worktree_records` (`:5051`) | the list and its parse | none |
| `add_worktree` (`:2649`) | the add (`:2708`), inside its funnel | shared |
| `remove_worktree_proving`'s scan (`:2988`) | `revalidate_removal_proving` (`:5119`) | none |
| `slots_with_torn_registrations` (`:5345`) | the torn plan's scan | exclusive |

**Not wrapped:**
- `remove_bound`'s mutation: it enumerates nothing once it no longer prunes (§3.5), and it takes no R-X now;
- `git fsck` on the refusing path (table B);
- `read_only_git`'s reads, which enumerate no registry.

### 3.4 One deadline per access, and what R-X becomes

**The deadline.** Each access has one deadline, `REGISTRY_ACCESS_DEADLINE` after it begins: 10 s in production, and
500 ms under test, where round 3 had its bound. Within it:
- **every wait for R-X** is a try-loop that sleeps outside the lock and gives up at the deadline;
- **no attempt starts** after the deadline;
- **every backoff sleep** ends at the deadline.

**What is bounded.** The access deadline bounds the access from end to end. The retry budget is not a second number:
it is whatever of the deadline the attempts and the waits have not used. At the deadline the access returns `Refused`,
naming what it was waiting for: the store's contention, or R-X.

**What is not bounded: one attempt's own runtime.**
- The deadline is checked between attempts. Nothing interrupts a Git command or a scan that has started; killing a
  Git writer mid-write is exactly what leaves a torn registration.
- Hooks are disabled for the manager's commands through `core.hooksPath` (`src/workspace_manager.rs:4994-5010`). A
  filter the repository configures, the size of a checkout, and a slow or network filesystem are not.
- So an access returns by its deadline **plus the runtime of the one attempt it started before the deadline**, and that
  runtime has no bound here.
- On this box, an add that checks out 100,000 files took 0.82 to 1.81 s, and deleting that checkout about 0.5 s
  (`d4/measure/add-duration.log`, three runs each). Slower filesystems are slower, and Windows is not measured.
- A pipeline whose own Git command stalls also stalls the command's end, because the coordinator joins its pipelines
  (`src/engine/topology/coordinator.rs:261`). That is unchanged from `92c4ca81`. The deadline bounds the accesses that
  would otherwise wait behind such a command; it does not end the command.

**What R-X becomes.** R-X (`REGISTRY_LOCKS`, `src/workspace_manager.rs:1553-1601`) serialised every registry access of
one process, so that none saw another's half done. Tolerance makes that unnecessary for every reader and writer:
each one survives another's half-done work exactly as it survives another process's. One exclusion is still needed.
- The torn plan reads an empty `commondir` as a dead add's residue, and the repair removes that slot
  (`slots_with_torn_registrations`, `:5345`; `repair_torn_registrations`, `:5328`).
- An add of this same process in flight passes through that state for the microseconds between `commondir`'s open and
  its write.
- The run lock keeps other processes of the run out, and nothing else keeps this process's own adds out.

So R-X becomes a read-write lock:
- **adds hold it shared:** two adds of one process no longer wait for each other;
- **the torn plan holds it alone;**
- **the list, the removal's scan and the removal's mutation take nothing.**

**Why not the simpler form.** The orchestrator's brief named a `try_lock` loop on R-X as it is. This round built that
first (`d4/census/probe-iv/patch-iv-mutex.py`; `suite-mutex-attempt/`).
- Its suite failed PR11's own in-process concurrency test,
  `concurrent_snapshot_adds_and_removals_on_one_repository_never_fail` (`src/workspace_manager/tests.rs:7207`): "3 of
  120 add/remove cycles failed", each one "… stayed held by this process's other registry work until the access's
  deadline (500ms)".
- Bounding a wait for a lock that serialises every access turns ordinary queueing under load into refusals. In
  production that queue is every pipeline's adds and removals: a large tree or a slow disk would refuse parallel work
  that PR11 completes.
- With R-X shared by adds, the same suite passes that test, twice (`suite-2.log`, `suite-3.log`).

**Executed** (`witness-runs/TABLE.txt`; another thread holds R-X for 1,500 ms, three times the test deadline):

| | Unpatched | Round 3's tolerance | Round 4's shape |
|---|---|---|---|
| A list begins while R-X is held | waits it out: Ok at 1,501 ms | Ok at 1,501 ms | **Ok at 0 ms**: the list takes no R-X |
| An add begins while R-X is held (alone, as the torn plan holds it in round 4) | waits it out: Ok at 1,504–1,506 ms | Ok at 1,504–1,506 ms | **`Refused` at 501–502 ms** |

**The waits that remain**, both bounded by the waiter's deadline:
- **An add waits while a torn plan runs.** The plan's scan reads small files, and its repairs run after it releases R-X,
  each through `remove_worktree_proving`, which takes none.
- **The plan waits for this process's adds in flight.** It runs only when an enumeration has already failed, in
  `verify_worktree` (`:2745`) and `remove_intent` (`:2269`). A plan that cannot take R-X before its deadline makes
  `repair_torn_registrations` answer `false`, and its caller returns the refusal it already had: the command ends
  resumably, and the next resume plans again.

### 3.5 Targeted removal

Unchanged from §2.5:
- no `git worktree prune`;
- a bound registration that still names the slot is removed directly, `locked` first;
- the store itself is removed when that leaves it empty, which the frozen
  `scrub_slots_converges_when_git_has_pruned_the_emptied_registration_store` pins (`src/engine/topology/finalize.rs:486`);
- a registration whose `gitdir` has gone converges with nothing removed.

One thing changes: the removal's mutation no longer holds R-X (§3.4). Two removals of one process act on different
slots, and the store's removal is `rmdir`, which fails on a store that is not empty. An add of the same process that has
not yet made its entry meets a missing store as `could not create directory of '<store>/<own name>'`, which is the own-entry
case of §3.3, and its second attempt creates the store again.

### 3.6 Effect governance, instruments and the frozen set

**Measured on the shape** (`d4/census/probe-iv/`: `patch-iv.py` on a scratch `git archive` of `8dd2214c`; nothing of
it on the branch):
- Clippy `-D warnings` over all targets: rc 0 (`suite/clippy-run.txt`).
- The whole suite, twice: the library passed 2,998, failed 2 and ignored 113 (`suite/suite-2.log`, `suite-3.log`; a
  first run's log was lost and its census kept, `NOTE-lost-suite-log.txt`).
- **The two failures are the two non-frozen tests round 3's probe also moved**, each the implementation's to move:
  - `workspace_manager::tests::a_removal_records_the_one_attempt_the_unix_arm_makes`
    (`src/workspace_manager/tests.rs:1432`): the direct removal of the administrative directory is a second observed
    removal;
  - `workspace_manager::tests::an_add_killed_before_it_wrote_gitdir_is_unlisted_and_refuses_forced_cleanup` (`:6675`),
    `RESIDUE-UNBINDABLE…`'s guard test: its static tear is now `Refused` after the deadline, carrying the same Git
    text.
- **Every frozen module's tests pass:** `recover` 226, `integrate` 20, `repair` 5, `finalize` 5, `fold` 192,
  `events::log` 47 (`frozen-modules-suite-{2,3}.txt`).
- **Every instrument census passes:** the wrapper classification and its name pins, the effectful-wrapper denials, the
  process-start and payload censuses, the allowlist scan and the sequential registry. **No instrument moves.** Round
  3's four instrument edits were all the lease's (the `effects/wrappers.toml` row and its name count, two
  `src/runner/contract.rs` rows), and they go with it. So do FUB-D3-CLIPPY's `clippy.toml` denial and FUB-D3-R28's
  packet question.

**What the implementation moves:**
- **Code:** `src/workspace_manager.rs` only: the tolerant access (private), R-X's type, and targeted removal. These
  hunks stay inside the registry-access wrapper and the removal, disjoint from follow-up C's Git writer spawn
  configuration.
- **Tests:** the two above, and the new tests of §3.8.
- **Docs:**
  - `DESIGN.md` §15's PROPOSED paragraph becomes the in-force one;
  - `docs/internals/engine/topology/run.md:479-490` and its pin
    `the_verification_notes_say_a_registry_another_process_is_writing_spends_a_deferral_or_parks`
    (`src/engine/topology/run/tests.rs:535`): the paragraph now says that a registry another process is writing never
    reaches this arm, because it is retried or refused; the mapping, the pin's second half, is unchanged;
  - the module notes of `src/workspace_manager.rs`, for R-X, the wrapper and the removal;
  - the findings: the repaired file is deleted, and the texts of `PR5-RD-003…`, `PR308-R3…`, `RESIDUE-UNBINDABLE…` and
    `PR11-HOST-AGENT-PRUNE-RACES-AN-ENGINE-ADD` are updated (§3.7).
- **Not `src/rundir.rs`,** and no R28 text: round 3's lease is withdrawn.

**So the implementation is a subject under CLAUDE.md's first limb.** Its diff changes no gate, CI-contract test, lint,
toolchain or runner configuration, and no effect allowlist: only the product's source, its own regression tests and
documentation. B1′ is the exception (§3.10).

### 3.7 What is closed, what remains, and what G6 meets

**The claims, once implemented.**
1. **No manager registry access returns `UpstrokeError::Git` for a failure another process's registry write caused**
   (§3.3). So, for (b): no registry race reaches `run::verified`'s Git arm, no deferral is spent and no candidate is
   parked on one.
2. **(a):** an attempt's registry access passes another process's write. It ends the command only if the store stays
   contended until the access's deadline, and then resumably.
3. **(d):** the same, whoever the writer is: a legacy run, another topology run, an agent's Git, the user's.
4. **A failure across a quiet, whole store is returned as it was:** at once, or, when it names the add's own entry,
   after exactly one more attempt.
5. **Each access returns by its deadline plus one attempt's runtime.** The only waits for R-X left are an add's while a
   torn plan of its process scans, and a plan's while adds of its process run, each until its own deadline (§3.4).
6. **No engine process deletes another process's registration** (§3.5).

**What remains.**

| | What | Consequence now | Finding |
|---|---|---|---|
| R1′ | A registration that stays torn until the deadline: a writer killed mid-registration, or one stalled there | the access refuses resumably, naming the entry; never Git state, never durable. On the topology path the command ends; on the legacy path see §3.10 | `PR308-R3-…` (consequence narrowed, precondition unchanged); `PR329-LEGACY-…` (§3.10) |
| R2 | A host agent's own Git | its torn entries are tolerated; its prune of an engine add in flight costs the add one more attempt (§3.3); the agent's own prune still deletes entries | `PR11-HOST-AGENT-PRUNE-RACES-AN-ENGINE-ADD`: narrowed to the agent's own commands |
| R3 | The user's Git in any checkout | tolerated the same way | none |
| R4 | `fsck` on the refusing path (table B) | a refusal either way | none |
| R5 | (c): a dead coordinator's Git writers against a recreated slot | not this change's | `PR329-A-RESUME-REBINDS-A-SLOT-ITS-DEAD-COORDINATORS-GIT-CHILD-STILL-WRITES` (P1, follow-up C, blocks G6) |
| R6 | The legacy engine's registry readers | §3.10 | `PR329-LEGACY-RUNS-IN-LINKED-CHECKOUTS-RACE-THE-SHARED-WORKTREE-REGISTRY` (P1; closed for writes in flight under B1′) |
| R7 | A Git message that names no entry for an entry made and unmade inside one attempt | C1 and C2 do not see such an entry, and C3 needs the name; none measured (§3.3) | stated |
| R8 | One attempt's runtime (§3.4) | an access can exceed its deadline by one Git command or scan | stated |
| R9 | Windows: a store spelt through an 8.3 alias in Git's text, or a file held unreadable across both reads | C3 misses the alias spelling, and the file reads quiet; C1 and C2 still apply. Reasoned, not executed | stated |

**What G6 meets.** This is the classification the triage asks for.

| Item | Severity | Here | Applies to G6 | Blocks G6 |
|---|---|---|---|---|
| `PR11-LINKED-CHECKOUTS-RACE-THE-SHARED-WORKTREE-REGISTRY`: (a), (b), (d) | P1 | repaired by §3.3–§3.5 once implemented; the finding file is deleted then | yes: R17 and the registry under concurrency, ST-16 | only until this change merges |
| `PR329-A-RESUME-REBINDS-A-SLOT-ITS-DEAD-COORDINATORS-GIT-CHILD-STILL-WRITES`: (c) | P1 | **filed at this head**, `pre_existing`; follow-up C | yes: Q1 ("reclaimed or repaired … before any slot reset, admission, or resource reuse"), ST-16, ST-18, INV-22 | **yes, until follow-up C merges; filing is not a waiver** |
| `PR329-LEGACY-RUNS-IN-LINKED-CHECKOUTS-RACE-THE-SHARED-WORKTREE-REGISTRY`: (e) | **P1** (re-graded) | filed; B1′ proposed, pending the owner's decision B | **the lenses split:** the pure legacy case does not (concurrency and regression lenses); the mixed case, a topology writer tearing a legacy reader, may, through R17 and Q6 (design-recast lens) | the mixed case, if G6's reviewers hold it applicable and B1′ is not taken; filing is not a waiver |
| `PR308-R3-SKIPPED-PRUNE-KEEPS-ANOTHER-RUNS-TORN-REGISTRATION` | P2 | consequence narrowed | — | no |
| `PR5-RD-003-A-PRUNE-STRANDS-A-CHECKOUT-WHOSE-GITDIR-IS-GONE` | P2 | no longer produced by an engine removal | — | no |
| `PR11-HOST-AGENT-PRUNE-RACES-AN-ENGINE-ADD` | P2 | narrowed (R2) | — | no |

### 3.8 Regression tests

Each test is the implementation's, named by the implementer. Each lives outside the frozen modules and their test
children, and waits on a handshake or a seam, with time only as a watchdog. T1, T2, T2′, T3, T5, T6 and T10 are §2.9's
tests, carried over unchanged in substance. T4 and T9 are revised, T11 to T13 are new, and §2.9's T7 and T8, the lease
tests, are withdrawn with it.

- **T1, two processes in linked checkouts, at least 1,000 cycles, 0 failures.** This is review round 7's witness.
  - Unpatched: red 10 of 10, 68 failed operations in 20,000 (`d3/census/witness-r7r8/r7-summary.txt`).
  - Round 4's shape: green 5 of 5, 0 in 10,000 (`d4/census/probe-iv/witness-r7r8/SUMMARY.txt`).
  - Mutation m1: the wrapper returns its first failure. Red.
- **T2, the verification beside a transient foreign tear: no deferral.** The handshake is the test-only seam that
  counts contended attempts; it is the shape `note_removal_attempt` already uses
  (`src/workspace_manager.rs:1528-1551`).
  - Unpatched: one and two deferrals.
  - Round 4's shape: Complete, `merge_verification_unavailable` 0, for both (`witness-r7r8/SUMMARY.txt`).
- **T2′, the verification beside a static tear.** It ends `Refused`, naming the entry, with nothing durable appended.
  - Round 4's shape: `Refused`; 12 events; no `merge_verification_unavailable`, question or `run_finished`
    (`witness-r7r8/SUMMARY.txt`).
  - Mutation m2: the deadline's refusal typed `UpstrokeError::Git`. Red: the verification defers.
- **T3, three processes:** T1's shape with three `LinkedChild` processes.
- **T4, the classifier, as unit tests over stores built by hand.** It covers C1 for each of `gitdir`, `commondir` and
  `HEAD`, absent and empty; C2 for a change of bytes at equal length within one clock tick (the inode and the bytes
  see it); C3 by text, absolute and relative, and by a refused output's checkout; the own-entry rule; and the quiet
  verdict. Mutations: each clause removed in turn, against the case only it catches.
- **T5, no engine prune.** A census that `worktree` with `prune` appears in no production argv of
  `src/workspace_manager.rs`; the store is removed only when empty; the frozen `finalize.rs` test stays green.
- **T6, an add whose entry another process prunes.** A seam holds the add between its `mkdir` and its `locked` while a
  foreign `git worktree prune` runs. The add succeeds on its second attempt.
- **T9, the deadline (revised).** A thread holds R-X alone, as the plan does. An add begins: it refuses within the
  deadline plus one backoff, and a list begins and succeeds at once.
  - Round 4's shape: the add refuses at 501–502 ms; the list succeeds at 0 ms. The unpatched tree and round 3's waited
    1,501–1,506 ms (`witness-runs/TABLE.txt`).
- **T10, the legacy writer beside the manager.** A legacy `Workspace` gate-snapshot add and its drop, in a linked
  checkout, run beside the manager in the main checkout. The manager has 0 failures.
- **T11, TORNOK (new).** The design-recast lens's prefix as a foreign entry.
  - Static: the list refuses at the deadline and is never Git state.
  - Transient: it succeeds once the writer finishes (with T2's handshake).
  - The control, a whole entry whose `HEAD` names nothing: the list returns Git state at once.
  - Round 3's tolerance: Git state for both the static and the transient case. Round 4's shape: `Refused`, Ok and Git
    state respectively, three rounds each (`witness-runs/TABLE.txt`).
- **T12, PERM (new).** One complete registration and the store made unwritable, then an add. It returns Git state, the
  add ran twice, and nothing is left at the slot.
  - Round 3's tolerance: `Refused` at 525–527 ms. Round 4's shape: Git state at 1–2 ms
    (`witness-runs/TABLE.txt`).
  - A mutation that drops the own-entry rule returns `Refused`: red.
- **T13, R-X shared (new).** PR11's `concurrent_snapshot_adds_and_removals_on_one_repository_never_fail` stays green,
  and a variant in which every add's checkout is held by a seam past the test deadline still completes every cycle.
  - The mutex form failed the unmodified test, 3 of 120 cycles (`suite-mutex-attempt/suite-1.log`).

**The budget.** The Windows guest's harness ran 468.44 s at `78f99c70`
(`~/orch-pr11/logs/pr11_repair_r8/ci/ci-read-36861160153.txt`). T1 and T3 run bounded cycles there, and the rest are
single scenarios.

**The proof the implementer owes:**
- each mutation, on a scratch tree whose Compiling line names it;
- the witnesses red unpatched;
- the frozen children unchanged;
- the ten gates, and CI on every leg;
- at least five full suites, with every frozen test's failures counted against the same number of suites at the
  base. The lease's sibling-fork exposure, which §2.9 asked this for, is gone, but a change to a lock every pipeline
  takes deserves the same count.

### 3.9 DESC, split out

**What it is.** One coordinator is `SIGKILL`ed inside an engine Git write. Its Git processes, and processes they started,
outlive it on Unix. Its resume removes and recreates the same slot path, `tasks/k<key>-g<gen>`, which the packet names
literally and `TaskDispatched.worktree_path` records. The orphan then acts on the recreated slot. Witnessed outcomes:
- paid edits reverted to `base` by a late `reset`, by Git's `update-ref`, or by a smudge filter's background helper;
- `remove_junk` deleting the recreated registration and checkout;
- a replacement registration's `HEAD` rewritten.

**Where it is now.** It is filed as
`findings/P1_correctness_202610020436_a-resume-rebinds-a-slot-its-dead-coordinators-git-child-still-writes.md`:
- id `PR329-A-RESUME-REBINDS-A-SLOT-ITS-DEAD-COORDINATORS-GIT-CHILD-STILL-WRITES`, the id round 3's ledger gave the
  class, kept so the ledger has one row for it;
- P1, `deferred`, `pre_existing`, category `correctness`.

The file carries:
- the executed failure sequence with one coordinator;
- the evidence of all three design reviews;
- why each remedy failed: a lock handed to the child (round 1), a process record (round 2), and a lease on standard
  input (round 3);
- follow-up C's candidate directions: group or job emptiness before reuse; unique slot paths per incarnation, which is a
  packet change and so the owner's; disabling filters and parallel checkout for engine writes; and a documented
  residual for persistent user helpers, which is the owner's call.

**G6.** It blocks G6 under Q1's "reclaimed or repaired … before any slot reset, admission, or resource reuse", ST-16,
ST-18 and INV-22. Filing it is not a waiver. Its guard is follow-up C, before G6, briefed in the orchestrator's
follow-up C brief (`fu-c-orphan-git-writers-before-slot-reuse.md`, under `~/orch-pr11/briefs/followups/`).

**`PR136-REMOVE-WORKTREE-VS-A-GIT-CHILD-NOTHING-KILLED` is the same class.**
- Its file was on `fix/sampler-kill-and-inspection` (PR #145, closed unmerged), at
  `reviews/findings/P2_correctness_202609042055_remove-worktree-vs-a-git-child-nothing-killed.md`; its text before
  `510f24c4` trimmed it gives the full sequence.
- Its sequence: "The engine dies … while `WorkspaceManager::add_worktree` has a `git worktree add` in flight. Nothing
  kills that child … Its descendants (`git checkout` and what that spawns) … keep writing into the new worktree".
- It recorded the liveness face of that sequence: recovery's forced removal fails `DirectoryNotEmpty` against the live
  writer and does not converge. It named the missing capability: distinguishing the in-flight window from the residue
  "needs liveness of the writer".
- DESC is the same precondition, a dead coordinator's Git child that nothing kills against recovery that reuses its
  paths, with the corruption face added: the removal succeeds and the orphan writes afterwards.
- Master cites PR136's id only from
  `findings/P3_docs-contract_202609050648_unbindable-task-registration-has-no-design-sentence.md`. The new file is
  where both faces are tracked, with PR136's id as its prior id.

### 3.10 The legacy path: (e) at P1, and decision B

**The re-grade.** `PR329-LEGACY-RUNS-IN-LINKED-CHECKOUTS-RACE-THE-SHARED-WORKTREE-REGISTRY` moves from P2 to P1, as
all three lenses of round 3 graded it. Its file is renamed to the `P1_` prefix and keeps its id:
`findings/P1_correctness_202610020230_legacy-runs-in-linked-checkouts-race-the-shared-worktree-registry.md`.
- **Executed** by the concurrency and regression lenses
  (`~/orch-pr11/reviews/329-d3-witnesses/pr329-d3-reg-legacy-chbhff16/result.txt`; the concurrency lens's
  `legacy/result.json`, quoted in its review text).
  - With one Git writer per checkout, A was held after opening its registration's `commondir`.
  - B's exact legacy snapshot-add argv exited 128: "failed to read …/commondir: Success".
  - `discard_uncommitted()`'s `reset --hard` and `clean -fd` then turned B's "paid worker edits" back into `base` and
    removed its new file.
- **The path is production's:**
  - the snapshot add's error propagates through `?` (`src/engine/attempt.rs:154`);
  - the legacy coordinator answers any attempt error with `discard_uncommitted()` (`src/engine/coordinator.rs:544-548`);
  - which is `reset --hard` and `clean -fd` (`src/workspace.rs:1230-1235`).
- **That meets MAINTAINING's serious-P1 criterion,** "loss or corruption of data in a user repository". A consistent
  event log, a resumable command and a low measured rate do not restore discarded output, so round 3's P2 grading is
  withdrawn.
- **The same discard follows a registration that stays torn** (a writer killed mid-registration, R1′), because the
  legacy reader dies on residue as it does on a write in flight. The finding's failure sequence now says so.

**G6, as the lenses split.**
- The concurrency and regression lenses hold that the **pure** legacy-against-legacy sequence does not apply to G6: it
  exercises the frozen legacy error path, not topology recovery, slots or ledgers.
- The design-recast lens holds that the **mixed** case, a topology writer tearing a legacy reader, which the filing
  covers, applies through shared-registry R17 conformance and Q6 ("untouched user checkout" among the sequential
  guarantees). On that view it blocks G6 while open.
- This round records the split and does not decide it. Filing waives nothing.

**B1′, the proposal, pending the owner's decision B** (ESCALATION item 7, recommended yes). The four legacy registry
enumerators run through the same tolerant access:
- `switch_branch`'s `git switch` (`src/workspace.rs:450`);
- `add_gate_worktree`'s `git worktree add -q --detach --force` (`:871`);
- `cleanup_gate_workspace`'s `git worktree remove --force` (`:1549`);
- `worktree_is_registered`'s `git worktree list --porcelain -z` (`:1602`).

What B1′ changes:
- **In `src/workspace.rs`, those four calls and nothing else.** Each call's existing `Command`, and its own success
  check, becomes the attempt closure handed to the access. The access's own-entry name is `add_gate_worktree`'s
  target basename. For `cleanup_gate_workspace`'s removal, its own registration is the entry the removal deletes,
  excluded from C2 because the attempt itself changes it.
  - One private helper resolves the canonical common git dir in the two steps `recorded_objects_scope` already takes
    (`:97-101`).
  - No other function of the module moves, and no other legacy module moves. In particular `discard_uncommitted`,
    the coordinator's error handling and the gate snapshot's lifecycle stay as they are.
- **In `src/workspace_manager.rs`, the access becomes callable from the legacy module:** one `pub(crate)` function
  over a common git dir, an own-entry name, a retry condition and the attempt.
  - Its body reads the store, sleeps and takes R-X; the Git command is the caller's.
  - So it is classified `effect_free`, by the same reading that classifies `worktree_records`, which runs Git.
  - Under B2′ the access stays private and no classification moves.
- **Instruments (B1′ only):**
  - `effects/wrappers.toml`: that function's name in `src/workspace_manager.rs`'s `effect_free` list.
  - `effects/allowlist.toml`: `src/workspace.rs`'s `legacy_effect` text, as below. Its `path`, its `allows` and
    `FROZEN_LEGACY_ALLOWLIST` (`src/effects.rs:1306`) do not move.
  - No `clippy.toml` change: an `effect_free` function is not denied.
  - These are instruments under CLAUDE.md's first limb, so a B1′ implementation is merged by the owner, or under a
    delegation the owner writes for that pull request, not under standing delegation.

**The unfreeze text.** The `legacy_effect` entry for `src/workspace.rs` (`effects/allowlist.toml:899-923`) changes in
two places:
- "AMENDED ONCE" becomes "AMENDED TWICE";
- the last sentence, "The schema-4 equivalents live behind funnels in `crate::workspace_manager` and nothing here calls
  them: the constant is read, and no funnel is called.", is replaced by:

> The second amendment, to close `PR329-LEGACY-RUNS-IN-LINKED-CHECKOUTS-RACE-THE-SHARED-WORKTREE-REGISTRY` on the
> owner's decision to unfreeze the module for this one change, is one thing and no more: the four Git children that
> enumerate the repository's worktree registry — `switch_branch`'s `git switch`, `add_gate_worktree`'s `git worktree
> add`, `cleanup_gate_workspace`'s `git worktree remove` and `worktree_is_registered`'s `git worktree list` — each run
> through `crate::workspace_manager`'s tolerant registry access, which attempts one again while the repository's
> worktree store shows another process writing it and refuses one still contended at its deadline, never as Git
> state; one private helper resolves the canonical common git dir as `recorded_objects_scope` does. Every other
> behaviour of the module stays frozen. The schema-4 equivalents live behind funnels in `crate::workspace_manager`, and
> nothing here calls a funnel: the constant is read, and the tolerant access is called, which takes no site.

**What B1′ closes.** It closes every write in flight that finishes within the deadline, from any writer: the race in
(e), and its mixed case. Its test is T10 with the roles swapped (the legacy engine in a linked checkout beside a
topology writer, 0 legacy failures), plus the two lenses' witnesses with A released within B's deadline: B's snapshot
add then succeeds on a later attempt, and nothing is discarded.

**What B1′ does not close: R1′ on the legacy path.**
- A registration that stays torn until the deadline still fails the legacy access. It fails as `Refused` after the
  deadline, naming the entry, rather than as a Git error at once.
- The frozen coordinator answers it with `discard_uncommitted()`, as it answers any failed attempt today.
- Closing that would need the legacy coordinator not to discard on a refusal: a second unfreeze, of
  `src/engine/coordinator.rs`, which this round does not propose.
- So under B1′ the finding file stays open, narrowed to that residue, at P1, with that remedy.
- That residue is not specific to parallel execution. A sequential topology run and the legacy engine leave the same
  residue when killed, and the dead run's own resume repairs its own slots' residue.

**B2′, the fallback, if the module stays frozen.** The finding stays filed at P1 with B1′ as its guard.
- What it leaves for G6: the pure legacy case, which two lenses hold inapplicable, and the mixed case, which one lens
  holds applicable through R17 and Q6. If G6's reviewers agree with that lens, an open applicable P1 fails G6. The
  route to a passing G6 then runs through B1′ in a later change, under the owner's decision.
- What B2′ costs the topology path: nothing. Tolerance closes (d) without the legacy side.

### 3.11 Design review round 3, answered

**Design review round 3** ran three `gpt-6-astra` lenses at `max` on `8dd2214c` on 2026-10-02, from 03:05:05Z to
03:35:23Z (the `run-lens` logs).
- The design lens's first run was refused on [cyber] grounds: "This content was flagged for possible cybersecurity
  risk", no output (`~/orch-pr11/reviews/review-329-d3-design-8dd2214c.log`, kept as `…-CYBER-REFUSED.log`). Under the
  owner's standing rule that is not a review, so it was recast as a conformance reading (`lens-fub3-design-recast.md`)
  and run again from 03:21:30Z.
- All three returned CHANGES_REQUIRED. The texts are
  `~/orch-pr11/reviews/review-329-d3-{concurrency,regression,design-recast}-8dd2214c.review.md`, with their hashes in
  `SHA256SUMS-329-d3`; the witnesses are in `329-d3-witnesses/`, and the triage is `review-329-d3-triage.md`.

| Finding | Sev | Kind | Round 4 | Where | Evidence |
|---|---|---|---|---|---|
| FUB-D3-TORNOK | P1 | executed | **Fixed (design), witnessed on the shape.** The parse is inside the attempt; the reads cover `HEAD`; C1 counts an absent or empty `gitdir`, `commondir` or `HEAD`. | §3.3 | `d4/witness/git-level.log`; `d4/census/probe-iv/witness-runs/TABLE.txt` (static, transient, handshake and control) |
| FUB-D3-PERM | P2 | executed | **Fixed (design), witnessed on the shape.** C3 needs an entry neither read holds; the add's own entry gets one more attempt; an unreadable file is not C1. | §3.3 | `git-level.log` (PERM); `witness-runs/TABLE.txt` (`d4_perm_add`, `d4_iv_perm_attempts`) |
| FUB-D3-BOUND | P2 | reasoned | **Fixed (design), witnessed on the shape.** One deadline per access covers every wait for R-X; R-X is read-write and taken only by adds (shared) and the torn plan (alone). One attempt's runtime is stated unbounded. | §3.4 | `witness-runs/TABLE.txt` (`d4_iv_bound_*`); `suite-mutex-attempt/suite-1.log` (the form not chosen) |
| FUB-D3-DESC-FILTER | P1 | executed | **Filed**, with the lease withdrawn and every claim that #329 closes (c) removed. | §3.9 | the finding file |
| FUB-D3-DESC-WIN | P1 | reasoned | **Filed**, the same finding. | §3.9 | the finding file |
| FUB-D3-LEGACY | P1 | executed | **Re-graded P1**; the lenses' G6 split recorded; B1′ prepared, B2′ kept. | §3.10 | the renamed finding file |
| FUB-D3-CLIPPY | P2 | reasoned | **Moot.** The lease helper it asked to deny is not added: the mechanism is withdrawn, and round 4 adds no effectful callable. | §3.6 | — |
| FUB-D3-R28 | P2 | reasoned | **Moot.** No new R28 holder: R28's row, its `NoRunFinished` case and INV-22's accounting stay as the packet has them, and no erratum is needed. Follow-up C may raise its own packet question. | §3.6 | — |

### 3.12 Risks, sequencing, and what is out of scope

**Sequencing.**
- The implementation does not depend on #328, which owns `src/agent/proc.rs` and the container launch funnel.
- It does not depend on follow-up C. C owns Git writer spawn configuration and the recovery-side wait before slot
  reuse; this change's hunks are the registry-access wrapper, R-X and the removal. Whichever merges second rebases.
- The two meet in `design/15` in different paragraphs.

**Risks.**
- **R-X changes shape.** Adds no longer wait for each other in one process, and only the torn plan excludes them.
  Every reader of the registry relies on tolerance instead of exclusion. That is measured clean in two suites and in
  PR11's concurrency test, and T13 and the five-suite count are owed.
- **Static tears now cost a deadline.** A resume over its own torn residue, or any run over another run's, waits the
  deadline before it refuses or repairs (R1′). Round 3's design had the same cost under its bound.
- **Genuine failures under sustained churn** wait up to the deadline and are then returned as a non-durable refusal.
- **Lost healing.** Another run's torn registration is no longer pruned by this run's removals (`PR308-R3-…`).
- **Git versions.** This box has 2.43.0; CI has 2.50.1 on the Windows guest and 2.55.0 elsewhere.
  - From 2.43 to 2.55, `add_worktree` writes `locked`, then `gitdir` and `commondir`.
  - From 2.50 on it writes `HEAD` in-process (`d3/git-src/`).
  - C1 counts each of the three files, absent or empty, so the order among them does not matter to it.
- **Windows (R9)** is reasoned, not executed.

**Out of scope, and said so.**
- (c), DESC: follow-up C (§3.9).
- A dead legacy coordinator's Git children against its own resume, which §2.13 already set out of scope.
- `RESIDUE-UNBINDABLE-TASK-REGISTRATION-HAS-NO-DESIGN-SENTENCE`'s policy question: round 3's suggestion that the lease
  could license `WriterProof::NoWriterAlive` goes with the lease.
- Foreign Git's own commands (R2, R3).
