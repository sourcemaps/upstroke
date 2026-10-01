# PR11 follow-up B — registry access exclusive across coordinator processes: the working record

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
session `orch_pr11` spawned on master `92c4ca81`. Every figure below is in a saved file under
`~/orch-pr11/logs/pr11_fub_design/` on the build box, and the sentence names the file. A fresh
implementer writes the code after the design review, and its sections follow §1.

## 0. Status

| Phase | State |
|---|---|
| Design (§1) | **written, PROPOSED**. It waits on two owner decisions: erratum E-FUB-1 with Class C for the vocabulary (§1.8), and the one-change unfreeze of `src/workspace.rs` (§1.9). The orchestrator escalated both, recommending A1 + B1, and the design review runs while the owner decides (`~/orch-pr11/answers/pr11_fub_design-1.md`). This head changes no production code. |
| Implementation | not started. It waits on the design review and the owner's decisions. |

## 1. Design

> **PROPOSED — pending the owner's decisions on erratum E-FUB-1 (with Class C for the vocabulary)
> and the one-change unfreeze of `src/workspace.rs`.** Everything in §1 assumes A1 + B1 (§1.8,
> §1.9) and cites neither as adopted. §1.10 says what each alternative changes, so a different
> decision revises one subsection, not the design.

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

**What the lock must cover.**
- **Topology:** R-X's four holders, as they stand, with no new holder. That is the add's Git child,
  a removal from its scan to its prune (two critical sections), the list, and the torn plan's scan.
  This is "taken wherever `registry_lock_of` is taken today", the finding's remedy 1.
- **Legacy (B1):** the four commands of table C.

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

**Contents and lifecycle.**
- **Empty, and never read or written.** The file exists only to be locked.
- **Created on first acquisition** by any registry access through the funnel (§1.3.4).
- **Never removed by any run** (§1.3.9).

#### 1.3.3 The primitive, at MSRV 1.85

`std::fs::File::lock` is Rust 1.89, so the crate's own primitives in `src/rundir.rs` are used, as
the brief asks. **On Unix, though, the primitive is `flock`, not `fcntl`, and the reason is the one
the brief's crash question exposes.**

**Unix: `flock(LOCK_EX | LOCK_NB)` on a descriptor the registry Git child inherits.**
- **Who writes a registration.** The Git child does. If the lock were the coordinator's alone, the
  kernel would release it the moment the coordinator dies, while an orphaned `git worktree add` or
  `git worktree prune` is still writing the store. Nothing in the engine kills that child on Unix,
  and another process's next registry access would read the half-written entry: the finding's race, re-opened
  by a crash.
- **What `fcntl(F_SETLK)` does.** Those locks are held by the process and not inherited (the
  primitive's notes, `src/rundir.rs:2544-2576`). That is right for the run and worktree locks,
  whose holder is the coordinator. It is wrong here, where the writer is the child.
- **Holder of record: the open file description.** An `flock` lock belongs to the open file
  description. The descriptor is opened `CLOEXEC`, and for a registry Git child its `CLOEXEC` is
  cleared between `fork` and `exec`. So the child holds the same lock for as long as it lives, and
  the lock ends when the last descriptor on it closes: the coordinator's and the child's.
- **Precedent.** This is the mechanism `hold_cleanup_lease_for_child` already uses for every
  `git update-ref` child and the run's cleanup lease (`src/rundir.rs:2193`, with its `pre_exec`).
  It is the same module, the same `libc::flock`, and the same reasoning: "a coordinator killed
  mid-write leaves a child whose liveness the next resume sees as a kernel fact". The differences:
  this lock is exclusive and never blocking, where the lease is shared and blocking.

**Windows: `LockFileEx(LOCKFILE_EXCLUSIVE_LOCK | LOCKFILE_FAIL_IMMEDIATELY)`.**
- The call is `rundir`'s existing `imp::take` (`src/rundir.rs:2676-2721`), on a fresh handle per
  acquisition. Release is the explicit `UnlockFileEx` of `imp::unlock` (`:2723`) followed by the
  handle's close.
- **Not inherited.** The handle is not inheritable: `std` opens non-inheritable handles.
- **The Git child dies with the coordinator.** A coordinator's Git child is a member of the ambient
  kill-on-close Job Object, which every write command joins before anything else
  (`src/main.rs:196`, `join_ambient_job`; INV-18). So the child cannot outlive its holder, and the
  Unix inheritance has nothing to cover here. The `hold_cleanup_lease_for_child` Windows half says
  the same (`src/rundir.rs:2237-2240`).

**One process, many threads.** Each acquisition opens its own descriptor. Two threads with two
descriptors therefore exclude each other under `flock`, and under `LockFileEx` on two handles. The
lock excludes threads by itself. R-X stays outside it (§1.3.4), so the threads of one process queue
on a mutex rather than poll, and no thread spends its wait bound on a sibling thread.
- **`fcntl`'s two hazards do not arise.** They are "closing **any** descriptor for the file releases
  every lock this process holds on it", and that `fcntl` does not exclude a process from itself.
  `flock` is per open file description, so another descriptor's close releases nothing.

**Release.**
- **Normal path.** The funnel releases with an explicit `flock(fd, LOCK_UN)` (Windows: `UnlockFileEx`)
  and then closes. By then the Git child has exited, because `output()` waits for it. An explicit
  unlock ends the lock at once, even if a sibling thread's fork still holds a transient copy of the
  descriptor (§1.3.8).
- **Death.** A process that dies runs no unlock. The kernel closes its descriptor, and a living Git
  child's inherited descriptor keeps the lock (§1.3.6).

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
) -> Result<T, UpstrokeError>;

pub(crate) struct RegistryHold { /* the locked descriptor; no public constructor */ }
impl RegistryHold {
    /// Run one registry Git child under this hold. On Unix the child inherits the locked
    /// descriptor (CLOEXEC cleared in `pre_exec`), so the lock outlives this process for as
    /// long as the child lives.
    pub(crate) fn output(&self, command: &mut std::process::Command) -> std::io::Result<Output>;
}
```

**The sequence inside `registry_access`.** No hook runs while anything is held.

1. **`Lock.CreateRegistryLockFile` (R29).**
   - `hook(Before)` runs.
   - The primitive opens `<common git dir>/upstroke-registry.lock` with `create(true)`,
     `truncate(false)`, read and write, the way `WorktreeLock` opens its file: the site names the
     create even when the file is there (`src/rundir.rs:1896-1905`).
   - `hook(After)` runs.

   Nothing is locked at either hook.
2. **`Lock.AcquireRegistry` (R17).**
   - `hook(Before)` runs.
   - Then the primitive:
     - refuse if this thread is already inside a registry access (the re-entrancy guard, §1.3.7).
       The check comes before R-X, because R-X does not re-enter: a nested access that reached it
       would wait on its own thread;
     - take R-X for the canonical common git dir;
     - poll the OS lock under the bound (§1.3.5);
     - run `access(&hold)`;
     - unlock and close;
     - drop R-X.
   - `hook(After)` runs. The hold is a **momentary** one, given back inside the site, as
     `Lock.ProbeCleanupExclusive`'s is (`AfterEffect::MomentaryHold`,
     `src/topology/effects/residue_authority.rs:971-977`).

   As the manager's `funnel` does (`src/workspace_manager/hooks.rs:369-380`), an `Err` from the
   primitive is returned without consulting `After`.

**R-X moves into the funnel.** `REGISTRY_LOCKS` and `registry_lock_of`
(`src/workspace_manager.rs:1592-1601`) move from the manager to `rundir`, unchanged in meaning: one
process-local mutex per canonical common git dir, poisoning ignored, the table never pruned. R-X is
taken inside `Lock.AcquireRegistry`, after its `Before` hook, so the hooks of both new sites run
outside R-X as well. No code takes R-X except this funnel.

**The holders, as they become.** Each keeps its critical section exactly. Only the lock around it
changes, and the manager adapts its `EffectHooks` to `RunDirHooks` with a forwarding wrapper.

| Holder | Hooks it passes | Shape change |
|---|---|---|
| `add_worktree` (`:2649`) | the caller's | Today `funnel(hooks, add_site, closure)` with R-X inside the closure. It becomes hand-rolled, as the commit-tree sequence already is: `consult(Before)`, the same checks, `registry_access(…, |hold| hold.output(add))`, then `consult(After)`. `Worktree.Add`, `.AddStaging` and `Snapshot.Add` keep their phases around the same primitive, and the two Lock sites' phases fall inside it, outside any lock. |
| `remove_worktree_proving` scan (`:2996-3003`) | the caller's | `registry_access(…, |_| self.revalidate_removal_proving(…))`, before the removal's funnel, as today. |
| `remove_worktree_proving` mutation (`:3006-3011`) | the caller's | Hand-rolled like the add: `consult(Before)`, `registry_access(…, |hold| self.remove_bound(…, hold))`, then `consult(After)`. `remove_bound`'s prunes run through `hold.output`. |
| `worktree_records` (`:5051`) | `NoHooks` | `registry_access(…, &mut NoHooks, |hold| hold.output(list))`. `revalidate()` takes no observer, and threading one through its 25 callers is out of proportion. The same hold is observed executing at the three hooked holders. The unhooked call is the precedent `WorktreeLock::acquire_in` and `rundir::is_running` set (`src/rundir.rs:1889-1894`, `:2356-2385`). |
| `slots_with_torn_registrations` (`:5345`) | its caller's, now passed down from `repair_torn_registrations(hooks, …)` (`:5328`) | `registry_access(…, |_| scan)`. |
| Legacy (B1): `add_gate_worktree`, `cleanup_gate_workspace`'s remove, `worktree_is_registered`, `switch_branch` | `NoHooks`, as every legacy lock call (`resume.rs:148` uses `acquire_in`) | Each Git child is run as `registry_access(&dir, &mut NoHooks, |hold| hold.output(&mut command))`. `dir` is `rev-parse --path-format=absolute --git-common-dir`, canonicalized, which is the two steps `recorded_objects_scope` already takes (`src/workspace.rs:97-101`). |

#### 1.3.5 Blocking, and the bounded wait

**Never blocking in the kernel, always bounded in the caller.**
- **The acquisition is a poll.**
  - Unix: `flock(fd, LOCK_EX | LOCK_NB)`. Windows: `imp::take`, which is fail-immediately.
  - On "held by someone" (`EWOULDBLOCK` or `EAGAIN`; `Holder::Someone`) the funnel sleeps and tries
    again. The sleep starts at 1 ms and doubles to a 25 ms cap.
  - `EINTR` retries at once. Any other failure is `UpstrokeError::Io` naming the lock file, which
    includes `ENOLCK` or `EOPNOTSUPP` on a filesystem without locks (`Holder::Unknown` on Windows).
    Today's run and worktree locks treat such a filesystem the same way (`src/rundir.rs:2369-2387`).
- **The bound.** It is a named constant, `REGISTRY_WAIT_BOUND = 600 s`, measured from the first
  attempt. The funnel takes it as a parameter, so a test can pass a short one.
  - **Measured holds.** On this repository (868 files): `git worktree add` 84 ms, `git worktree
    list` 1 ms, removal and prune 9 ms (`measure/hold-durations-upstroke-repo.txt`).
  - **Scaling.** Add and removal scale with the files checked out or deleted, so a 100,000-file
    checkout is on the order of seconds and a million files on the order of a minute and a half.
  - **The margin.** 600 s leaves room for several such holders queued ahead of a waiter. A bound
    that expires therefore means a holder that is stuck, not slow. The manager's Git children have
    no timeout of their own today, so a hung `git worktree add` holds the lock until it is killed.
- **When the bound expires.** The error is `UpstrokeError::Refused`, naming:
  - the lock file;
  - the bound;
  - the holder when the platform says (Unix `flock` names none; neither does `LockFileEx`, as
    `imp::take` says).

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
  R-F). The lock is none of those. The coordinator thread already waits, synchronously, on R-X and
  on its own Git children: "the coordinator's only waits are its inbox and the synchronous work it
  already did at width 1 (its own Git, filesystem and appends)" (R-S). The new lock adds the wait
  for another process's registry access, bounded.

#### 1.3.6 Crash behaviour

**Released on process death.**
- **Unix.** When the holder dies, the kernel closes its descriptors. If no registry Git child is
  alive, the lock is free at once, apart from the transient copies of §1.3.8. If one is alive, its
  inherited descriptor keeps the lock until it exits. It normally finishes its write, because nothing in
  the engine kills a coordinator's Git child on Unix, so the next holder reads a **complete**
  registration.
- **Windows.** When the holder dies, its handle closes and its Git child is killed through the
  ambient job. The OS releases a dead process's locks asynchronously: "the time it takes for the
  operating system to unlock these locks depends upon available system resources". The poll and
  its bound absorb that.

**What a crash can still leave: a dead writer's torn registration.**
- **When.**
  - A holder killed in the middle of a Rust-side removal (`remove_bound`).
  - A Unix Git child killed together with its coordinator, for example by a signal to the process
    group.
  - A Windows Git child killed through the job.
- **What it leaves.** A registration that stays torn. The lock cannot exclude a writer that no longer
  exists.
- **Who repairs it.** The run that owns it repairs it at its next resume, through its intents:
  `verify_worktree` and `remove_intent` run the torn plan (`:2744-2748`, `:2268-2272`).
- **What another run sees.** Until then, another run's enumerations die on it. The same class
  across runs is already filed:
  `PR308-R3-SKIPPED-PRUNE-KEEPS-ANOTHER-RUNS-TORN-REGISTRATION`
  (`findings/P2_crash-consistency_202609191720_a-skipped-prune-keeps-another-runs-torn-registration.md`).
  This change does not narrow or widen it (§1.5, residual R1).

#### 1.3.7 Acquisition order: no cycle

**Every lock and wait a coordinator process has.**

| Lock or wait | Taken | While holding it, the holder… |
|---|---|---|
| worktree lock (R17, `upstroke-worktree.lock`) | first, at command start, after the read-only refusals; **never waits** (refuses if held) | runs the whole command |
| run lock (R17) and the momentary cleanup probe | second; never waits (refuses) | runs the whole command |
| cleanup lease (R28, shared `flock`) | by reapers, and by each `git update-ref` child for its life | is a reaper or a ref write. Neither is a registry holder, and no registry holder runs `update-ref`. |
| snapshot gate (R-W) | a pipeline waits for a grant **before** `add_snapshot` (`attempt.rs:1135-1137`) | adds and removes snapshots |
| slot pair (PermitBroker) | a pipeline waits for a grant inside `execute_typed`, never inside a registry access | runs one Runner call, with no registry access inside it (§1.3.5) |
| **R-X** (in-process mutex) | inside `Lock.AcquireRegistry`, after its `Before` hook | only polls the OS lock |
| **the registry lock** | inside R-X; a bounded poll | runs exactly one access: one Git child, or a scan, or `remove_bound`'s filesystem calls and its one prune. It takes no other lock, sends and awaits no message, consults no hook, and calls no registry reader. |

**The proof.**
- **Within one process.**
  - R-X and the registry lock are innermost, and nothing is acquired while either is held, so no
    edge leaves them. Their holders wait only for their own Git child and their own filesystem calls.
  - A thread waiting on R-X waits for a holder whose own wait is bounded: the poll plus its one
    child.
  - Every other wait in the table is entered outside R-X and the registry lock: grants, slots and
    the coordinator's inbox.
- **Across processes.**
  - A process waiting on the registry lock waits for another process's holder.
  - That holder waits only for its own Git child.
  - A Git child takes none of these locks: it is Git, and it knows nothing of them. Nor does any of
    the registry commands write a shared ref. `worktree add --detach` writes the new registration's
    own `HEAD`, and `switch` writes its own checkout's `HEAD`. So a registry child cannot wait on a
    Git ref lock another engine child holds.
- **The run and worktree locks.** No process holding the registry lock waits on either: both refuse
  rather than wait, and both are taken once, at command start, outside any registry access.

So the wait-for graph is acyclic: worktree lock → run lock → (R-X → registry lock) → own Git child.

**What enforces it** (§1.6, T5).
- **Re-entrancy guard.** A thread-local guard makes a registry access entered on a thread already
  inside one a `Refused` error. A future change that nests one, or calls a hook that does, fails
  loudly rather than deadlocking as `R1-REG-1` did.
- **Census.** The census pins the holders.
- **Witness.** `R1-REG-1`'s witness is extended over the new sites.

#### 1.3.8 The inherited-descriptor class

The PR281 mechanism is this: a sibling thread's `fork` copies every descriptor, and that copy lives
until the child's `exec` or until the child closes it.
- **What that means for the registry lock.** A copy taken while a registry descriptor is open holds
  the lock for that window. For a mutual-exclusion lock that can only make the hold **longer**,
  never shorter. Other holders, in this process and others, wait out the copy within their bound.
- **How long the window lasts.**
  - Spawns: the descriptor is `CLOEXEC`, so a spawn's copy dies at its `exec`: "a spawn's
    fork-to-exec window", in the cleanup lease's own notes (`src/rundir.rs:2176-2183`).
  - The Unix reaper and the job-control guard close inherited descriptors in their setup
    (`close_inherited_fds`, named at `src/rundir.rs:2181-2183`).
- **The normal release does not wait for those copies.** It is an explicit `LOCK_UN` (§1.3.3).
- **A copy that outlives its spawn.** Only a registry Git child receives the descriptor across
  `exec`, and only its own children inherit it from there.
  - Both engines run every registry child with hooks disabled: an empty `core.hooksPath`, both
    builders.
  - Both run it with `-c core.fsmonitor=false`, so no fsmonitor daemon starts.
  - No registry command starts auto-maintenance (§1.2, table B).

  The only long-lived grandchild left is a long-running filter process of the checkout. Git waits
  for it before it exits.
- **What `fcntl` would have cost.** `fcntl`'s inheritance answer is the opposite: no copy holds the
  lock. That is what makes it wrong for the orphaned-child case, §1.3.3.
- **Windows.** The handle is not inheritable, and children are in the kill-on-close job (§1.3.3).

#### 1.3.9 Residue: the file is never removed

- **Why it is never removed.** Removing a lock file while another process waits on it, or holds it,
  splits the lock. A newcomer creates a new inode at the same path and excludes no one. So no run
  removes `upstroke-registry.lock`: it spans runs, like `upstroke-worktree.lock` (R25, "never
  removed by a run").
- **Accounting.** It is R29, `persistent_output` at every outcome (§1.8). It holds no bytes, and Git
  ignores it (§1.3.2).
- **What an operator may do.** Delete it, only when no upstroke process is running on the
  repository. A file left behind is harmless, because the lock is advisory and released by the OS.
- **Residue classes.** None. The hold is momentary and leaves nothing (`AfterEffect::MomentaryHold`),
  and the file is persistent output, not residue to reclaim.

#### 1.3.10 Taken only after the command's read-only refusals

R17's packet text and INV-22 say a coordinator's holds are "taken only after the command's read-only
refusals". T-RESUME requires a refusal at recovery step (a0) to take no lock and create no R25 file.

**Within the engine.** `WorkspaceManager::derive` is a registry holder: its `revalidate()` lists the
worktrees (`:1639`). So the rule is that **a command derives its manager after its worktree lock**.
Then the first registry hold, and R29's creation, follow the command's read-only refusals.

**In this build.**
- **No production caller of `derive` exists** (R-G). PR12's assembly inherits the rule, and §1.7
  names it as a risk.
- **Test arrangements derive first.** `scaffold.rs:3093` and `recover/tests.rs`'s `resume_with`
  (`:1275`) derive before `run_recovery_order`.
  - There the list takes the momentary hold through `NoHooks`, and may create R29, before the
    recovery order's (a0) refusals.
  - That is the arrangement's order, not the recovery order's. The assertions of
    `resume_with_explicit_private_root_mismatch_refused_before_any_lock` and
    `malformed_recorded_locator_refused_before_any_lock` stay true:
    - no Lock site observed (`any_lock_site_ran`, `:1310`);
    - no R25 file.
  - The frozen `recover/tests.rs` is not edited.

### 1.4 Effect governance

**The proposed vocabulary** (Class C under the `src/topology/**` freeze; erratum E-FUB-1, §1.8):

| Item | Value |
|---|---|
| `LockSite::CreateRegistryLockFile` | row R29; adjacent `None`; fault row `TRegistry`; scope `Shared` (B1; `Topology` under B2); not read-only; no sub-effect point; no residue class; before state `Absent`; after effect `Referenced`; module `src/rundir.rs` |
| `LockSite::AcquireRegistry` | row R17; adjacent `None`; fault row `TRegistry`; scope `Shared` (B1; `Topology` under B2); not read-only; no sub-effect point; no residue class; before state `Absent`; after effect `MomentaryHold`; module `src/rundir.rs` |
| `ResourceRow::R29` | `external_physical`; `ResourceRow::ALL` 15 → 16 |
| `FaultRow::TRegistry` | the new cross-cutting row T-REGISTRY, as `TAppend` is for every append |

`Adjacent::None` because no append is ordered against a registry access: they happen around many
events, in many transactions. The precedents are the Event and the husk-removal sites
(`effect_sites.json`: nine sites with `"adjacent": "none"`, `"observable_orders": []`). Their
registry entries carry `"order": null`.

**The residue authority.** Neither site registers a residue class. `LockSite::before_state` gains
`Absent` for both, and `after_effect` gains `Referenced` (the file) and `MomentaryHold` (the hold)
(`src/topology/effects/residue_authority.rs:932-978`).

**The instrument diff the implementation will make.** One sentence per row. (A1 + B1. Under B2, the
`src/workspace.rs` row does not move and the two sites are `Topology`-scoped.)

- **The vocabulary.**
  - `src/topology/effects/sites.rs`: `LockSite` gains the two variants, and every exhaustive const fn
    arm with them. `ALL` goes from 6 to 8, and the inventory walk's pinned count (`:1822`, "walked,
    70") from 70 to 72.
  - `src/topology/effects/vocab.rs`: `ResourceRow::R29` and `FaultRow::TRegistry`, with their names
    in the serialized vocabulary.
  - `src/topology/effects/residue_authority.rs`: the two sites' before states and after effects.
- **The inventories.**
  - `effect_sites.json` is regenerated by its test: 70 rows → 72, the two new rows as the table
    above.
  - `effects/funnel-modules.json`: `sites_checked` 70 → 72. There is no new disagreement, because
    both sites' inventory module and funnel module are `src/rundir.rs`.
  - `effects/sequential-registry.json`: four entries, one per site and per hook phase, each naming
    its evidence test (T9). The expected residue is the momentary-hold and created-file semantics the
    format derives from the site.
  - `src/engine/topology/coverage.rs`: four `Claim`s naming the same tests. It is not a frozen file.
- **The allowlists.**
  - `effects/wrappers.toml`, the `src/rundir.rs` module: `funnel` gains `registry_access` and
    `output` (`RegistryHold::output`). The latter is reachable only through a `&RegistryHold`, which
    exists only inside `Lock.AcquireRegistry`.
    - **`shared` counts.** If either name is borne by another callable of the file, its count moves.
      `every_name_more_than_one_callable_bears_is_pinned_by_its_count` derives that.
    - **`src/workspace_manager.rs`.** No row moves: the holders are existing functions, and
      `registry_lock_of` is private and leaves the file.
  - `effects/allowlist.toml`, the legacy `src/workspace.rs` row: its `legacy_effect` text records the
    second amendment, the four registry Git children run through `rundir::registry_access` (§1.9).
    The row's `path` and `allows` do not move, so `FROZEN_LEGACY_ALLOWLIST`
    (`src/effects.rs:1306`) does not move.
  - The `src/rundir.rs` funnel row's text stays true ("every … lock effect is issued inside a
    `funnel` call naming one `EffectSiteId`"), and so does the `src/workspace_manager.rs` row's.
- **What does not move.**
  - `clippy.toml`. `libc::flock`, `LockFileEx` and `UnlockFileEx`, and `std::process::Command`'s
    `output` are already denied outside the allowlisted funnel modules, and `src/rundir.rs` is one.
  - The CI-contract tests under `src/effects/`. `every_production_process_start_is_classified`
    counts `Command::new(`, `.spawn()` and `run_with_timeout`, and no file gains one. The site
    censuses re-derive from the inventory.

**The test counts and pins that move, as far as this design can see them.**
- `LockSite::ALL.len()` users, and the "walked 70" pin.
- The adjacency census `the_observable_orders_are_the_ones_the_adjacency_admits`: its `neither`
  count rises by 2.
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
2. **The lock lasts as long as its writer.** A write in flight ends before its lock is released. On
   the normal path the funnel waits for the child. On Unix, a dying holder's lock lives on in its
   writing child (§1.3.3). On Windows the child dies with its holder (§1.3.3).
   - The one exception is a write ended by the writer's death, which is not in flight but abandoned:
     residual R1.
3. **Every engine read is under the lock.** That is every enumeration or scan of the store whose
   failure the engine acts on (tables A and C). The exceptions are `fsck` on the refusing path, and
   `registration_for`, which no production path calls (table B).
4. **So no engine read overlaps an engine write**, by mutual exclusion (§1.3.3, §1.3.7).

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
| R1 | A dead writer's torn registration (§1.3.6), until its owner's resume repairs it | yes, for another run's enumerations: it is foreign state now, not a race | no; filed as `PR308-R3-SKIPPED-PRUNE-KEEPS-ANOTHER-RUNS-TORN-REGISTRATION` |
| R2 | A host-runner **agent** running `git worktree prune` or `add` in its own worktree (`PR11-HOST-AGENT-PRUNE-RACES-AN-ENGINE-ADD`, `findings/P2_correctness_202610010030_a-host-agents-own-worktree-prune-races-an-engine-add.md`) | yes | **no**. The agent's Git takes no engine lock, so this change neither narrows nor widens it, and the finding stays filed. The container runner's disposable Git view withholds `worktrees` from a role (`src/runner/container/view.rs:240`), so a container agent cannot reach the store. |
| R3 | The user's own Git in any checkout of the repository, including a maintenance run their own commit starts | yes | no: foreign Git state nobody can exclude |
| R4 | `fsck` on the refusing path (table B) | not a verification's; a refusal either way | n/a |
| R5 | Under B2: a legacy coordinator (§1.10) | yes, after PR12 | n/a |

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

Each test names its first-bad shape (what it fails on at `92c4ca81`, or on the mutation) and its
platform budget.
- **Budget.** The Windows guest's harness was 468.44 s at `78f99c70`
  (`~/orch-pr11/logs/pr11_repair_r8/ci/ci-read-36861160153.txt`), and the hosted queue took 32 of its
  45 minutes (the brief, `~/orch-pr11/briefs/pr11_fub_design.md`). So every
  stress run is gated: full cycles on Unix, bounded cycles on Windows. Every Windows-only test is
  bounded to seconds.
- **Placement.** Every test lives outside the frozen modules and their test children.

**T1 — the two-process witness, linked checkouts, ≥ 1,000 cycles, 0 failures.**
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

**T2 — the verification-path witness: no durable deferral.**
- **Shape.** Review round 8's witness (`~/orch-pr11/reviews/r8-witnesses/conc/review-witness.patch`),
  inverted.
  - **The trigger.** When the verification's review-input check runs in the staging worktree, the
    policy starts a **foreign holder**: a `LinkedChild` process of the same test binary.
  - **The foreign holder.**
    - It takes the registry lock through the production `registry_access` (`NoHooks`, in its own
      process).
    - It writes the torn registration as R8's witness did: `HEAD`, a `gitdir`, an empty `commondir`.
    - It reports `TORN`, holds 500 ms, completes or removes the entry, and releases.
  - **The handshake.** The policy returns only after `TORN`. So the verification's next registry
    access, the snapshot add's `revalidate()` list, starts while the foreign process holds the lock.
    It must wait.
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
  - m1 turns it red.
  - m4, the foreign holder writing **without** the funnel, is R8's witness itself. It turns red,
    which shows the test separates a fixed race from foreign state.
- **Platforms.** Every platform, with one torn write per verification, in seconds.

**T3 — three processes.** T1's shape with three `LinkedChild` processes: the main checkout and two
linked checkouts.
- Unix: 3 × 400 cycles, 1,200 in total. Windows: 3 × 10.
- 0 failures. m1 turns it red. R7's four-process run failed at the base too
  (`~/orch-pr11/reviews/r7-witnesses/conc/witness.log`).

**T4 — crash while holding.**
- **(a) Unix: the lock lives on in the writing child.**
  - **Shape.**
    - A `LinkedChild` holder takes the lock through the funnel.
    - Inside the access it runs a stand-in for a registry child through `RegistryHold::output`. The
      stand-in writes `started`, waits for a file the test creates, then writes `ended` and exits.
    - The holder reports `HOLDING`.
    - The test `SIGKILL`s the holder (`LinkedChild::kill`).
    - Another process polls the lock with a 5 s bound. It must still be refused.
    - The test then creates the release file.
    - The waiter must acquire, within its bound, and only after `ended` exists.
  - **The oracle.** File handshakes, not a clock.
  - **First-bad.**
    - m2: the hold not handed to the child (the `pre_exec` dropped).
    - Equally: `fcntl` as the primitive.

    Either way the waiter acquires while the stand-in is still running, which is red.
- **(b) Unix and Windows: no child.** A holder killed inside a Rust-side access, which blocks on a
  handshake, frees the lock. Another process acquires it within its bound. That is the OS release:
  immediate on Unix, asynchronous on Windows.
  - **First-bad.** A lock that survives its holder, such as an `O_EXCL` lock file, is red at the
    bound.
- **(c) The residue is foreign: a documentation test.** After (b) with a torn entry left, another
  process's `worktree_records` returns `UpstrokeError::Git` naming the entry. That pins residual R1
  as stated, not closed.

**T5 — the acquisition-order census.**
- **(a) Static.** `registry_access(` appears in production code at exactly the listed holders:
  - the manager's five calls (the add, the two removal sections, the list, the torn plan);
  - legacy's four (B1).

  `REGISTRY_LOCKS` is named only inside the funnel. The census lives in `src/rundir/tests.rs`, a
  subject's test, as `only_the_line_builder_introduces_terminal_layout` is.
  - **First-bad.** A holder added anywhere else, or R-X taken outside the funnel.
- **(b) Re-entrancy.** A `registry_access` entered inside another's closure returns `Refused`
  within a watchdog, and does not hang.
  - **First-bad.** The guard removed (m3): the inner access deadlocks on R-X, and the watchdog fails.
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
queue leg).
- **(a) Exclusion across processes.** A `LinkedChild` holds the lock. The parent's access with a
  1 s bound fails `Refused` (not `Git`), and succeeds after the child releases.
- **(b) Release on death.** `TerminateProcess` on the holder. The parent acquires within the bound,
  despite the asynchronous unlock.
- **(c) Not inherited.** The holder spawns an ordinary long-lived child while holding, then
  releases. The parent acquires while that child lives.
- **(d) The ambient job ends the writing child.** A holder that joined the ambient job, killed while
  its registry child runs, leaves no child within a bound.

**T7 — an expired wait is never durable.**
- **(a)** `run::verified` given the funnel's bound error (not a Git error) returns `Err`, not
  `Verified::Unavailable`.
- **(b)** A verification whose snapshot add meets a lock held past a short test bound ends the
  command with that error. Nothing is appended, and the next resume completes.
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
  - `src/workspace.rs`'s diff touches only the four call sites and the common-dir helper (§1.9).

**T9 — the sites' ST-07 evidence.** These are the four tests the registry entries and the
`coverage.rs` claims name. A fault is armed at each new site's `Before` and `After`, at a hooked
holder (a task-worktree add). Each shows:
- what the format says each phase leaves (Before: nothing; After: the hold given back, and the file
  present);
- the next step recovering;
- `Lock.ProbeCleanupExclusive`'s and `Lock.CreateWorktreeLockFile`'s pairs as precedent
  (`effects/sequential-registry.json`).

**The proof the implementer owes.**
- Each mutation (m1–m5) on a scratch tree whose Compiling line names it.
- The witnesses red at `92c4ca81` where this section says so.
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
- **Liveness.** A stuck holder in one process stalls every other engine process on the repository
  for up to 600 s, and then the waiter ends resumably. On the coordinator thread that stall delays
  grants and completions. That is the same thread R-X and the coordinator's own Git children already
  block (R-R).
- **The legacy path changes (B1).**
  - A new file appears in the common git dir on the first legacy registry access.
  - A legacy registry access can now wait, with a bound.
  - A wait past the bound fails the command where it raced before, discarding the worker's edits as
    any gate-snapshot failure does today.
  - The snapshots' cleanup runs in `Drop` (`src/workspace.rs:1419`, `:1673`), so a `Drop` can now
    wait, with a bound. Legacy runs on one thread and holds nothing there.
  - Each legacy registry access costs one extra `git rev-parse` for the common dir.
- **PR12's assembly** must derive its manager after the worktree lock (§1.3.10). Otherwise the first
  registry hold and R29's creation precede the command's read-only refusals, against INV-22's "taken
  only after the command's read-only refusals".
- **`flock` on network filesystems.** `ENOLCK` and `EOPNOTSUPP` refuse the command, as the run and
  worktree locks already do there (§1.3.5).
- **Windows path length.** The file adds 23 characters to the common git dir's path. The worktree
  lock's path is of the same order. The 220-character `.git` budget is a linked checkout's, and this
  file is not in one.
- **Test churn.** The hook traces and inventory counts in §1.4 move. A frozen test child that would
  move is a stop condition, not an edit (§1.4).
- **Version dependence.** The census is Git 2.43.0's. The design's correctness rests only on "a
  registration is written over time, and enumerations read it": every Git version satisfies that,
  so no version's internals are assumed.

### 1.8 Erratum E-FUB-1, proposed wording (decision A1)

To be adopted by the owner, beside `~/tactus-artifacts/2026-08-25-g2-pass-errata.md`'s six. It is
**not adopted**, and nothing in this record or `DESIGN.md` cites it as adopted. The anchors are the
packet's at v16. v17 differs from v16 in `packet_version` and Q5 alone, so the anchors are the same
there (`packet/` files under the log directory).

> **E-FUB-1 — the worktree registry's cross-process lock (PR11 follow-up B, R7-CONC-1).**
>
> *`decisions.resource_accounting.rows` — new row R29*, after R28:
> `{"id": "R29", "resource": "upstroke-registry.lock file (repository-scoped: <common git
> dir>/upstroke-registry.lock; created on first acquisition by any registry access through the
> lock funnel, after that command's read-only refusals; spans runs; never removed by a run)",
> "domain": "external_physical", "granularity": "per repository (common git dir)", "lifecycle":
> {"exists": "persistent_output (its hold is R17)"}, "at_run_end": {"Complete":
> "persistent_output", "Parked": "persistent_output", "Halted": "persistent_output",
> "BudgetExceeded": "persistent_output", "NoRunFinished": "persistent_output"}}`.
>
> *`decisions.resource_accounting.rows[R17].resource`*, appended inside the list of holds: "…, the
> momentary exclusive cleanup.lock probe (Unix), **and the momentary exclusive
> upstroke-registry.lock hold around each registry access (every enumeration or mutation of
> `<common git dir>/worktrees/` by an engine process; on Unix also held by the registry Git child it
> is handed to, for that child's life)**".
>
> *`decisions.resource_accounting.enforcement_domains.external_physical`*: R29 joins the rows it
> lists ("… R24, R25, **R29**, R26, R27; …").
>
> *`decisions.resource_accounting.outcome_equations.Complete`*: "…R21 (…)/R25/**R29** as
> classified".
>
> *`invariants[INV-22].statement`*: "per decisions.resource_accounting (R1-**R29**)".
>
> *`decisions.effect_site_inventory.identity`*: "row(): exactly one of R9-R12, R17, R18, R19, R21,
> R22, R23, R24, R25, R26, R27, R28, **R29**)", and among the named sites: "Lock.AcquireRun,
> Lock.AcquireWorktree, Lock.ProbeCleanupExclusive, **Lock.AcquireRegistry**, Lock.Release (R17;
> the worktree lock file creation maps to R25 **and the registry lock file creation,
> Lock.CreateRegistryLockFile, to R29**; the reaper hold is observed through
> Lock.ObserveCleanupHold, R28)".
>
> *`transaction_fault_matrix` — new row T-REGISTRY*, after T-APPEND: boundary "a registry access's
> momentary exclusive hold (Lock.CreateRegistryLockFile, Lock.AcquireRegistry), inside any
> transaction that adds, removes, lists or scans a worktree registration"; durable state "the
> enclosing transaction's; the hold is OS-released at process death (on Unix it lives on in the
> registry Git child it was handed to until that child exits); the R29 file persists"; resume
> action "nothing of the hold survives: the enclosing transaction's row decides; the file is
> adopted"; refusal condition "a hold not released within the bound refuses the command resumably,
> never as an outage of a verification"; tests "two and three processes in linked checkouts, a
> verification beside a foreign holder, a crash while holding, the acquisition order, and the
> Windows lock semantics".
>
> *`decisions.invariant_ownership.INV-22`*: "**R29 follow-up B (PR11)**".
>
> *`cumulative_review_gates.gates[G6].integrated_invariants[0]`*: INV-22's process-local rows read
> "R3, R4, R13, R17 (**incl. the registry hold**), R22, R28", and **R29** joins the rows G6 certifies,
> introduced in its range.
>
> *Class C under the `src/topology/**` freeze*, approved for: the two `LockSite` variants,
> `ResourceRow::R29`, `FaultRow::TRegistry`, and their arms in `residue_authority.rs`. No event, wire
> or fold vocabulary changes.

### 1.9 The legacy unfreeze, proposed wording (decision B1)

`src/workspace.rs` is frozen at PR5. The packet's PR5 contract says "existing Workspace and legacy
engine behavior untouched", and the module has been "AMENDED ONCE … by the route that finding's
guard names ('an owner decision to unfreeze the module for this one change')"
(`effects/allowlist.toml:898-924`). B1 is the second such decision. Its recorded form is the row's
`legacy_effect` text, extended:

> … The amendment is three things and no more. [the first amendment's text, unchanged] **AMENDED
> TWICE: to close `PR11-LINKED-CHECKOUTS-RACE-THE-SHARED-WORKTREE-REGISTRY` for the legacy engine
> as well, on the owner's decision to unfreeze the module for this one change. The four Git
> children that enumerate or mutate the repository's worktree registry — `add_gate_worktree`'s
> `worktree add`, `cleanup_gate_workspace`'s `worktree remove`, `worktree_is_registered`'s
> `worktree list` and `switch_branch`'s `switch` — run through `rundir::registry_access`, which
> holds `<common git dir>/upstroke-registry.lock` around each, and a helper resolves that common
> directory as `recorded_objects_scope` does. Nothing else in the module moves.** Every other
> behaviour of the module stays frozen.

`DESIGN.md` §15's new paragraph states the same fact for the v0.1 path, and the sentence that names
the module frozen at PR5 is left as it stands.

### 1.10 If the owner decides otherwise

Each alternative revises the subsections named here.
- **A2: R25 widened instead of a new row R29.** §1.8's row text moves into R25. The sites map
  `Lock.CreateRegistryLockFile` to R25, and §1.4 drops `ResourceRow::R29`. Nothing else moves. It
  puts two granularities in one row, against `completeness_rule`.
- **A3: remedy 2.** It replaces §1.3–§1.6. Its cost is in §1.3.1.
- **A4: no packet change.** §1.4 and §1.8 change to classify the file under R25 in code and
  `DESIGN.md` only. The G6 reviewer, certifying INV-22 by the packet's rows, would meet a resource
  the rows do not name.
- **B2: `src/workspace.rs` stays frozen.** The lock covers the topology path only:
  - the two sites are `Topology`-scoped;
  - §1.9, table C's "what the lock must cover", and T8 drop out;
  - the claim in §1.5 narrows to "no topology coordinator observes another topology coordinator's
    half-written registration".

  The legacy coordinator, which §1.2 shows racing in production today and tearing a topology
  verification after PR12, is then filed as a new finding at P1. Its topology-verification cost is
  R7-CONC-1's own, a durable deferral or a park, so it is P1 on the same reasoning. G6, whose pass
  rule admits no open critical or high finding, would meet it open unless the owner decides
  otherwise. Under B1 nothing is filed: this change repairs it.
