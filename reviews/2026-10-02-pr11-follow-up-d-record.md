# PR11 follow-up D — a legacy run keeps its paid output when the shared worktree registry is torn: the working record

The record of the change that repairs `PR329-LEGACY-RUNS-IN-LINKED-CHECKOUTS-RACE-THE-SHARED-WORKTREE-REGISTRY`
(`findings/P1_correctness_202610020230_legacy-runs-in-linked-checkouts-race-the-shared-worktree-registry.md`, P1,
`deferred`, `pre_existing`). It is kept on the branch so that a successor session inherits what was decided and why.
Like the PR11 record (`reviews/2026-09-30-pr11-record.md`) and follow-up B's, it is **not** a design document.
`DESIGN.md` and the packet stay the authority, and a sentence here that disagrees with either is a defect in this file.

**Branch.** `fix-P1/correctness_legacy-runs-in-linked-checkouts-race-the-shared-worktree-registry`, cut from master at
`92c4ca81f9209d218df4534ee71d3445dc2906e1`, the merge commit of pull request #327 (PR11). It is a `fix-P1/` lane:
review effort `max`, and every P0 and P1 is fixed before the pull request is ready. Its first commit files the finding
it repairs, byte-identical to #329's copy at `4a126215` (blob `f07b7388`).

**Why this change exists.**
- The finding is the legacy half of #329's registry race. #329's design review round 3 graded it P1 in all three
  lenses, under MAINTAINING's "loss or corruption of data in a user repository": a torn read of the shared
  `.git/worktrees` registry makes a legacy attempt fail, and the legacy coordinator answers with
  `discard_uncommitted()`, which destroys the worker's paid output.
- #329's design review round 5 returned CHANGES_REQUIRED from all three lenses, and the looping signal appeared a fifth
  time. The PR11 orchestrator's decision (`~/orch-pr11/reviews/review-329-d5-triage.md`, 2026-10-02T07:43Z):
  - #329 keeps the topology repair;
  - the legacy half, corrected B1′ plus B-PRESERVE, moves to this change, follow-up D;
  - D is owner-gated (decision B, `~/orch-pr11/ESCALATION.md` item 7), because it unfreezes PR5-frozen modules;
  - D blocks G6 through the mixed cases (e2) and (e2′);
  - D calls #329's tolerant-access helper, so D's implementation follows #329's merge, while its design proceeds now.

**Who writes it.** The design phase (§1) is `pr11_fud_design`'s (`claude-opus-5-5`, `max`), a fresh session spawned by
the PR11 orchestrator on master `92c4ca81`, with the brief `~/orch-pr11/briefs/pr11_fud_design.md` and its addendum
`~/orch-pr11/answers/pr11_fud_design-0.md`. Its figures are under `~/orch-pr11/logs/pr11_fud_design/`, cited as `fud/…`.
#329's are cited as `d5/…` (`~/orch-pr11/logs/pr11_fub_design5/`) and `d6/…` (`~/orch-pr11/logs/pr11_fub_design6/`), and
#329's record (`reviews/2026-10-01-pr11-follow-up-b-record.md`, on #329's branch) by its commit. Every figure below is
in a saved file the sentence names. The probe's index is `fud/probe/SUITES.txt` and the two witness tables. They were
made on scratch `git archive` copies of `92c4ca81`, whose `src/` is `4a126215`'s; nothing of the probe is on the
branch.

## 0. Status

| Phase | State |
|---|---|
| Design (§1) | **PROPOSED, pending the owner's decision B** and design review. Corrected B1′ (§1.3) and B-PRESERVE (§1.4) carry #329's design review round 5 findings against them: FUB-D5-INDEX, FUB-D5-RESTORE and FUB-D5-UNFREEZETEXT. They are designed against #329's round-6 helper contract (§1.2). Each was executed through the real legacy engine on scratch prototypes, with the mutations that turn them red (§1.9). This head changes no code. It carries the finding file (commit 1), this record, and one PROPOSED paragraph in `design/15`. |
| Implementation | **Not started.** It waits on design review, the owner's decision B, and #329's merge. |

## 1. Design

> **PROPOSED — pending the owner's decision B** (`~/orch-pr11/ESCALATION.md` item 7). D unfreezes four PR5-frozen
> modules: `src/workspace.rs`, `src/engine/attempt.rs`, `src/engine/coordinator.rs` and `src/engine/resume.rs`. It
> also appends tests to a fifth, `src/engine/tests.rs`. That runs against PR5's and PR12's packet invariants (§1.7).
> Nothing in §1 is in force until four things have happened: the owner decides B, the design passes review, #329
> merges, and D's implementation lands. §1.5 gives the exact unfreeze texts.

### 1.1 The defect, the four cases, and what D owns

**The failure sequence** (the finding's, on master `92c4ca81`):
- A legacy (schema 1–3) run in one checkout of a repository, and another run, legacy or topology, in another checkout
  of it, main or linked. Each holds its own worktree lock, which is per checkout, so both run.
- The other run writes a registration into the shared `<common git dir>/worktrees/`, one file at a time.
- The legacy run's gate or review snapshot add (`add_gate_worktree`, `src/workspace.rs:871`) enumerates the store and
  dies on the half-written entry, for example "failed to read .git/worktrees/<name>/commondir: Success".
- The error returns through `?` (`src/engine/attempt.rs:154`, `:178`).
- The legacy coordinator answers any `run_attempt` error with `discard_uncommitted()`
  (`src/engine/coordinator.rs:544-548`). That is `git reset --hard HEAD` and `git clean -fd` (`src/workspace.rs:1230-1235`).
- The worker's uncommitted, paid output for that attempt is gone.
- The same follows a registration that stays torn: a writer killed mid-registration leaves it, and the legacy reader
  dies on residue as it dies on a write in flight.

**Executed on master through the real legacy engine.** Row `base` of `fud/probe/witness-runs/TABLE.txt`, three rounds
(w1):
- a static tear planted right after the candidate is captured;
- the run fails as Git state, and the checkout is clean;
- the resume with the residue still there fails as Git state;
- after the repair, the resume runs the attempt again and pays for it again.

**The four legacy registry enumerators.** They are #329's census, table C of its record §1.2, `strace` on Git 2.43.0
(`~/orch-pr11/logs/pr11_fub_design/census/strace/registry-census-2.43.0.txt`, rows 143–196):

| Git child (`src/workspace.rs`) | Reached from |
|---|---|
| `git worktree add -q --detach --force <path> <commit>` (`add_gate_worktree`, `:871`) | the gate and review snapshots of every legacy attempt with gates or reviewers (`src/engine/attempt.rs:154`, `:178`) |
| `git worktree remove --force <path>` (`cleanup_gate_workspace`, `:1549`) | a snapshot's drop (`:1419`, `:1673`), and the resume's reclaim (`src/engine/resume.rs:426`) |
| `git worktree list --porcelain -z` (`worktree_is_registered`, `:1602`) | `cleanup_gate_workspace`, after the remove (`:1572`) |
| `git switch -q --no-recurse-submodules -- <branch>` (`switch_branch`, `:450`) | the resume, over a clean checkout (`src/engine/resume.rs:441-454`) |

Each dies on a torn sibling, from the main or a linked checkout. None of the other legacy children the census ran
enumerates the store or fails on the torn sibling (rows 107–188): the `rev-parse` family, `version`, `check-attr`,
`config`, `symbolic-ref`, `log`, `switch --create` (`create_branch`), `add -A`, `ls-tree`, `commit-tree`, the gate
worktree's `status`, `update-ref`, `commit`, `check-ref-format`, `reset --hard` and `clean`.

**The four cases** (#329's, unchanged):

| | Writer | The legacy reader meets |
|---|---|---|
| (e1) | legacy | a write in flight |
| (e1′) | legacy | residue (a writer killed mid-registration), or contention that outlasts the deadline |
| (e2) | topology | a write in flight |
| (e2′) | topology | a topology writer's residue, or contention that outlasts the deadline |

**What D owns.**
- **Corrected B1′** closes (e1) and (e2): the three registry children each run as an attempt of #329's helper (§1.3).
- **B-PRESERVE** closes (e1′) and (e2′): a registry-refused attempt's captured candidate is kept and pinned, and the
  resume names it with commands that take it back (§1.4).
- Both carry #329's design review round 5 findings that moved here: FUB-D5-INDEX (P1), FUB-D5-RESTORE (P2) and
  FUB-D5-UNFREEZETEXT (P3).
- They also carry round 4's legacy requirements, as #329's record §4.4 met them: the owned, unchanged, empty destination
  (FUB-D4-B1PREDICATE), the removal and its list as one attempt (FUB-D4-B1REMOVE), and `switch_branch`.

### 1.2 What D requires of #329's helper

The orchestrator's addendum asks D to design its three call sites against #329's helper contract, and to state exactly
what D requires of the helper. #329's design round 6 published that contract at `ed3a97d9`, in its record §5.5,
"stable from 2026-10-02". D requires these, and nothing more:

1. **The signature,** in `src/workspace_manager.rs`, which is not frozen:

   ```rust
   pub(crate) enum RegistryHold { Unheld, Shared, Exclusive }

   pub(crate) fn tolerant_registry_access<T>(
       common_git_dir: &Path,
       hold: RegistryHold,
       again: &mut dyn FnMut() -> bool,
       attempt: &mut dyn FnMut() -> Result<T, UpstrokeError>,
   ) -> Result<T, UpstrokeError>
   ```

2. **The predicate.** `again` is the caller's veto.
   - It is called once after each failed attempt, outside R-X.
   - It is never called before the first attempt, or after a success.
   - When it is false, the access returns that attempt's error unchanged, at once.
   - D's add passes its destination predicate (§1.3); D's other two accesses pass `|| true`.
3. **The return variants.**
   - `Ok(T)` from the first successful attempt.
   - The vetoed attempt's own error, unchanged. This is the only way Git state comes back.
   - `UpstrokeError::RegistryRefused { message }` when the deadline passes with the last attempt failed. The message
     names `<common_git_dir>/worktrees`, the deadline, the attempt count and the last failure's text.
   - The same variant when R-X stays held elsewhere in the process until the deadline.
   - B-PRESERVE keys on the variant `RegistryRefused` (§1.4), and on nothing in its text.
4. **The deadline.** `REGISTRY_ACCESS_DEADLINE`, fixed when the call begins: 10 s in production and 500 ms under
   `cfg(test)`. It bounds the waits for R-X, the backoff sleeps (1 ms doubling to 50 ms) and the start of every
   attempt. It does not bound an attempt that has already started.
5. **Nothing sampled.** The helper reads no store state, no error text and no timestamp. Only `again` and the deadline
   decide another attempt.
6. **`common_git_dir` is canonical:** `git rev-parse --path-format=absolute --git-common-dir`, then `fs::canonicalize`.
   D's one private helper computes exactly that (§1.3), so a legacy process's R-X key and refusal text match the
   manager's for the same repository.
7. **The classification is #329's.** #329 adds the helper's name to `src/workspace_manager.rs`'s `effect_free` list in
   `effects/wrappers.toml` (its record §5.5 and §5.8). D adds no row. `RegistryHold` is a type, which the census does
   not classify.
8. **A test handshake keyed by repository.**
   - D's transient-tear tests need to know when an access has failed once and will attempt again. Only then can they
     finish the tear.
   - The contract's `#[cfg(test)]` counter of attempted-again accesses per common git dir (`CONTENDED_ATTEMPTS`) serves.
   - A single armed slot does not. D's probe used round 5's single-slot seam (`D5_SEAM`), and two concurrent seam
     witnesses overwrote each other in a full suite until they were serialized (`fud/probe/SUITES.txt`).
   - If #329 drops the counter, D needs another per-repository handshake from #329. D adds none to #329's files.

**What D does not require:** a destination parameter, any store read by the helper, or any classifier.

**Checked against the contract.** D was prototyped on round 6's own probe shape: `d6/census/probe-vi/patch-vi-b.py
--with-row` on round 4's `patch-iv.py`, then D (`fud/probe/patch-d-legacy-r6.py`, `setup-r6.sh`). Results:
- every witness of §1.9 is green, three rounds each (`fud/probe/witness-runs-r6/TABLE.txt`);
- clippy `-D warnings` over all targets: rc 0 (`fud/probe/suite-d6-nowit/clippy-1.log`);
- the whole suite: 2,998 passed and 2 failed (`fud/probe/suite-d6-nowit/suite-1.log`). The two are the non-frozen
  manager tests every #329 round since 4 has moved;
- every legacy `engine::tests` test (188), every `workspace::tests` test (47) and every effects census passed.

**Reconciliation.** The orchestrator compares this subsection with #329's §5.5 before D is implemented. A later change
to §5.5 is marked there with its date.

### 1.3 Corrected B1′: the three registry children of `src/workspace.rs`

Three call sites change, and one private helper and one private type are added; nothing else in the module moves.

**1. `switch_branch` (`:450-457`).**
- The attempt is the `git switch -q --no-recurse-submodules -- <name>` child (`:455-456`):
  `tolerant_registry_access(&canonical_common_dir(&self.root)?, RegistryHold::Unheld, &mut || true, &mut || …)`.
- `refuse_worktree_filters_before` and `refuse_unsafe_checkout_tree` run once, before the access, as now.
- **A failed attempt changed nothing.** Git refuses a branch checked out elsewhere by scanning the registry
  (`die_if_checked_out`) before it changes anything, so the next attempt is the same attempt.
- Executed at the Git level (`d5/witness/git-level-v.log`, S): rc 128 naming the torn entry, with `HEAD` and the
  status unchanged; once the sibling finished, rc 0.
- **Every failure is attempted again until the deadline,** because the contract has no classifier. A genuine failure
  now refuses after the deadline instead of failing at once. The resume calls this only over a clean checkout
  (`src/engine/resume.rs:441-454`), so its refusal discards nothing.

**2. `add_gate_worktree` (`:871-906`).**
- The attempt is the `git worktree add -q --detach --force <path> <commit>` child (`:879-896`) and its exit check
  (`:897-904`). It holds `RegistryHold::Shared`, as the manager's adds do.
- **The legacy veto (`again`): the owned, unchanged, empty destination.**
  - `PendingGateWorkspace` made the destination (`:1377`). Before the first attempt, the add records its identity
    (`OwnedDestination::of`).
  - Another attempt is allowed only while the destination is still that directory: present, a directory and not a
    link or reparse point, empty, and on Unix the same device and inode.
- **Why the veto is exact.** Git's order is fixed in 2.43.0, 2.50.1 and 2.55.0 (#329's record §5.3 at `ed3a97d9`,
  `builtin/worktree.c`).
  - Git takes a destination over (`junk_work_tree = xstrdup(path)`) only after four steps: its sibling scan, its
    reference, its new entry, and that entry's `locked`.
  - Only after the takeover does it write the entry's `gitdir`, the destination's `.git`, `HEAD` and `commondir`, and
    check out.
  - On any failure after the takeover, `remove_junk` removes the entry and then the destination.
  - So a destination still unchanged and empty after a failure means Git stopped before the takeover: in the registry
    phase, or creating its own entry. The next attempt is the same attempt.
  - A destination gone or changed means the failure is the add's own, in its checkout or its own files. It is
    returned unchanged, at once, as at master.
- **Executed at the Git level** (`d5/witness/git-level-v.log`):
  - P1: rc 128 on a torn sibling. The destination's device and inode are unchanged, and no entry or registration
    names it. Once the sibling finished, rc 0.
  - P2: a commit whose checkout cannot be made. rc 128, and the destination is gone.
- **Executed through real legacy snapshot construction on round 6's helper** (`fud/probe/witness-runs-r6/TABLE.txt`,
  #329 round 5's workspace witnesses, three rounds):
  - a transient tear: Ok after 2 attempts, the destination present and empty after the first failure;
  - a static tear: `RegistryRefused` at 1,004 ms after 15 attempts. That is the add's deadline, then the pending
    snapshot's cleanup's;
  - a checkout that cannot be made (a 300-byte name): Git state after 1 attempt, in 8 ms.
  - The mutation to round 4's "nothing at the slot" is wrong in both directions (`d6-m-b1pred`): the transient tear
    comes back as Git after 1 attempt, and the checkout refuses after 14 attempts.
- **The addendum's fourth clause, "named by no registration", is not part of D's veto.** The addendum and #329's round
  5 text list it beside the other three. D leaves it out, for four reasons:
  1. **It is implied whenever Git exits on its own.** The entry's `gitdir` is the only file that names the
     destination. Git writes it after the takeover, and on any failure after the takeover it removes the destination.
     So "a registration names the destination" implies "the destination is gone".
  2. **The one flow where it is not implied decides against it.** A Git child killed by a signal between the takeover
     and its first write into the destination leaves its entry naming an unchanged, empty destination.
     - With the clause, the veto returns that failure as Git state, and the coordinator discards the output.
     - Without it, the next attempts fail on that entry until the deadline. The refusal is then `RegistryRefused`,
       and B-PRESERVE keeps the output.
  3. **It would make D sample the store.** The round-6 helper reads nothing, so D would read the registry's `gitdir`
     files itself. That is the per-file sampling #329 removed after it leaked in three rounds. On Windows it would
     also have to match Git's `C:/…/.git` spelling against std's verbatim canonical paths.
  4. **#329's topology add vetoes on the same predicate without it** (#329's record §5.4 and §5.5).
- **On Windows** std exposes no stable file identity at MSRV (#329's record §5.2), so the predicate there is the rest
  of it. The destination is in the run's private root, so nothing but this access and its Git child touches it. A
  failure after the takeover whose junk removal left the destination itself, empty, reads as untouched. It is
  attempted again and refuses at the deadline: kept, never Git state. That is #329's R9 on the legacy path.

**3. `cleanup_gate_workspace` (`:1549-1600`).**
- One attempt is the removal and its success decision together: `git worktree remove --force <path>` (`:1557-1571`),
  then `worktree_is_registered`'s list and parse (`:1572`, `:1602-1635`). It takes `RegistryHold::Unheld` and
  `again = || true`.
- **When the attempt succeeds:** when the list does not register the path, whatever the removal's exit status. That is
  the existing treatment, which counts an already-unregistered destination ("is not a working tree") as reclaimed.
- **When it fails:** with the removal's words while the list registers the path, and with the list's own error when
  the list fails.
- Everything after the attempt (`:1581-1599`) is unchanged.
- **Executed.**
  - At the Git level (`d5/witness/git-level-v.log`, R): the removal exits 128, the sibling finishes, and the list exits
    0 and still lists the target. The same removal again exits 0.
  - On round 6's helper (`witness-runs-r6/TABLE.txt`): 2 removal attempts, the snapshot unregistered and its directory
    gone.

**4. One private helper, `canonical_common_dir(root)`.** It runs `git rev-parse --path-format=absolute
--git-common-dir` through `git_path`, and so through `git_command`, as the module's census requires
(`every_git_child_of_this_module_is_built_where_replacements_are_refused`, `:3681`). It then calls `fs::canonicalize`.
These are the two steps `recorded_objects_scope` takes (`:97-101`).

**5. One private type, `OwnedDestination`.** It records the identity at the first attempt, and `unchanged(path)` is
the veto.

**What stays as it is.**
- `worktree_is_registered` keeps its body, and is called only inside the removal's attempt.
- `discard_uncommitted`, the snapshot lifecycle (`PendingGateWorkspace`, `:1304-1433`) and `recorded_objects_scope`
  stay as they are.
- So does `create_branch`, whose `switch --create` does not enumerate the store, and every other function.
- No `#[cfg(test)]` item enters the production region: the module's first `#[cfg(test)]` stays at its `mod tests`, which
  `effects::tests::every_production_region_that_stops_early_stops_at_a_module` requires. A seam inside the removal's
  attempt, as #329 round 5's probe had, breaks that census (`fud/probe/suite-d6/suite-1.log`). D's tests need no seam
  there (§1.9).

**Consequences, stated.**
- A snapshot's drop (`:1419-1433`, `:1673-1685`) waits up to the deadline when the store is in the way, where it
  failed at once and left residue for the resume.
- A refused add costs two deadlines, the add's and then the pending snapshot's cleanup's: 20 s in production, and
  1,004 ms under test. It leaves its intent, which the next resume's reclaim takes.

### 1.4 B-PRESERVE, corrected: `attempt.rs`, `coordinator.rs`, `resume.rs`

**The design in one line.** The coordinator keeps a registry-refused attempt's captured candidate in the checkout and
pins it in the repository. The resume keeps the pin, discards the checkout's copy as it does today so that the attempt
runs again from a clean tree, and names the pin with commands that take its output back, deletions included.

**`src/engine/attempt.rs`, exactly (FUB-D5-INDEX).**
- `run_attempt` (`:91-238`) takes one more argument: `refused: &mut Option<RefusedCandidate>`.
- `pub(super) struct RefusedCandidate { branch_ref: String, parent: String, tree: String }`: the candidate a snapshot's
  registry refusal stopped, as `capture_candidate` captured it.
- One private function, `note_refused(error, branch_ref, parent, tree, refused)`. It writes the three strings into
  `refused` when `error` is `UpstrokeError::RegistryRefused`, and does nothing otherwise.
- The two snapshot calls (`:154-158`, `:178-182`) each gain
  `.inspect_err(|error| note_refused(error, &candidate.branch_ref, &candidate.parent_oid, &candidate.tree_oid, refused))`
  before their `?`.
- Nothing else moves: the return type, every other `?`, and the capture at `:129`.
- **Why the captured identity.** `capture_candidate` (`src/workspace.rs:494-520`) returns the tree `git write-tree`
  wrote at capture, and the branch and parent it checked `HEAD` against before and after.
  - The index at the moment of the refusal is not that tree. Another Git client can unstage or change it in between,
    as the regression lens's witness did (`~/orch-pr11/reviews/329-d5-witnesses/pr329-d5-reg-doo7d3qh/index-witness.log`).
  - Master's own `gates_review_and_commit_use_one_frozen_candidate_tree` (`src/engine/tests.rs:996`) already holds the
    captured candidate authoritative over a mutated index.
- **The form, and the one rejected with evidence.** The first form changed `run_attempt`'s error type to an enum that
  carried the identity, with two `From` impls so that every `?` kept working. On the probe it failed in two places:
  - The effects classification census: "src/engine/attempt.rs unclassified: ["from"]", and two bearers of one name
    (`fud/probe/diag-suite-3.log`). It would have needed an `effects/wrappers.toml` edit.
  - Clippy's `result_large_err` (`fud/probe/suite-d/clippy-1.log`).
  - The out-parameter form passes both: `fud/probe/suite-d/suite-3.log`, `suite-d-nowit/clippy-2.log`, and the same on
    round 6's helper.

**`src/engine/coordinator.rs`, exactly.**
- **`:281-283`.** Beside `prepared_pin_ref`, one constant: `pub(super) const KEPT_PIN_SUFFIX: &str = "-kept";`.
- **`:544-550`.** `let mut refused = None;`, then `run_attempt(…, &mut refused)`. The `Err(error)` arm splits on
  `refused`:
  - **A refused candidate was recorded:** nothing is discarded.
    - The candidate is pinned through `Workspace::prepare_commit_from_candidate(&candidate.branch_ref,
      &candidate.parent, &candidate.tree, "[upstroke] kept: <task> attempt <n>", <prepared_pin_ref(run, index,
      attempt)>-kept)`.
    - The arm returns `UpstrokeError::RegistryRefused` with the refusal's text followed by "; the worker's output for
      attempt <n> of `<task>` is kept in this checkout and pinned at `<pin>`".
    - When pinning fails, the text instead ends "…, and pinning it at `<pin>` failed: <error>".
  - **None was recorded:** `discard_uncommitted()`, and the error is returned, as now.
- **What the new arm calls:** `prepared_pin_ref`, which the module already calls (`:657`), and
  `Workspace::prepare_commit_from_candidate`, which it already calls (`:668`). It builds `UpstrokeError::RegistryRefused`,
  #329's variant.
- **What `prepare_commit_from_candidate` does** (`src/workspace.rs:946-1017`):
  - it validates the branch ref, the parent commit and the tree;
  - it refuses when `HEAD` moved from the captured branch and parent;
  - it writes a hook-free commit with upstroke's identity;
  - it pins the commit with a create-only `update-ref` (old value zero), and verifies the pin.
- **The kept pin's name is unique** per run, task and attempt, because an attempt number is never reused in a run.
- **No prepared-commit path names it:**
  - the resume's orphan removal names `prepared_pin_ref` exactly (`src/engine/resume.rs:552-554`);
  - the schema-3 settlement check builds the exact expected pin (`src/events/mod.rs:1332-1339`);
  - the topology's pins live under `refs/upstroke/runs/` (`src/workspace_manager.rs:116`).

**`src/engine/resume.rs`, exactly: the recovery policy (FUB-D5-RESTORE).**
- **`:26`.** It also imports `KEPT_PIN_SUFFIX`.
- **`:543-560`.** For each interrupted attempt, after the orphan pin's removal, the loop also asks
  `prepared_pin_target(&format!("{pin_ref}{KEPT_PIN_SUFFIX}"))`, and collects each kept pin that exists.
- **`:562-570`.** The uncommitted paths are discarded exactly as before: the warning, `discard_uncommitted()`,
  `RunResumed.discarded`. When any kept pin was found, one more warning follows:

  > the worker output of the interrupted attempt(s) a worktree-registry refusal stopped is kept, and no resume removes
  > it: `<pin>`[, `<pin>`…]. Each pin is a commit on the HEAD its output was captured on. To take the output back,
  > deletions included: while HEAD is still the pin's parent, `git restore --source=<pin> --staged --worktree -- .` from
  > the checkout's root; on a later HEAD, `git cherry-pick --no-commit <pin>`. `git update-ref -d <pin>` removes the pin

- **Why two commands** (`fud/git-level/restore-shapes.log`, Git 2.43.0):
  - **With HEAD at the pin's parent:**
    - round 5's `git checkout <pin> -- .` leaves a deleted file in place: the index differs from the pin by
      "A deleted.txt";
    - `git restore --source=<pin> --staged --worktree -- .` from the checkout's root reproduces the pin's tree exactly;
    - from a subdirectory, `.` covers only that subdirectory.
  - **With HEAD advanced past the parent** (the resumed run committed another task):
    - the same `git restore` also reverts that commit's paths in the index and the checkout: `later.txt` deleted and
      `untouched.txt` restored to its old text;
    - `git cherry-pick --no-commit <pin>` applies only the kept change, staged as `D deleted.txt`, `A new.txt` and
      `M tracked.txt`, and leaves the new commit's files.
  - **The operator reads the warning at the end of the command,** after the resumed run may have committed. So the
    warning names both commands, and when each applies.
- **Executed through the real legacy engine** (`fud/probe/witness-runs/TABLE.txt`, three rounds each):
  - w3: an edit, a new file and a deletion, with HEAD at the parent. Following the warning's restore leaves the index
    equal to the pin's tree, and the deletion is restored.
  - w3b: the resumed run commits the task again. Following the warning's cherry-pick stages exactly the kept change,
    and the new commit stays.
  - Round 5's warning turns both red.
- **No resume removes a kept pin;** it is the operator's.
- **Why recovery and not refusal.** The pin keeps the output on every later invocation. A refusal would stop the run
  for an operator's action without keeping anything more.
- **The first form** (round 5's) refused when the leftovers were a staged candidate. It failed three frozen legacy
  tests (`d5/census/probe-v/ne-suite/firstform-suite-1.log`). The kept pin is a marker that only a registry refusal
  writes.

**Residuals, stated.**
- **A pin that cannot be written.** `HEAD` moved after capture, or the ref store cannot be written at that moment.
  The output is then only in the checkout, and the refusal says so (w6).
  - When `HEAD` moved, a resume refuses on the moved branch before its discard (`src/engine/resume.rs:532-541`).
  - When the ref store failed, a resume discards as today. That takes two independent faults.
- **A resume whose own run later fails** returns that error without its warnings, as every legacy warning does: they
  reach only the report (`src/engine/coordinator.rs:1140-1151`). The pin was named by the refusal that wrote it, and
  it stays.
- **The attempt that runs again pays again.** Adopting the kept output into the run would need the worker's outcome,
  which the legacy log does not record before settlement (`src/engine/attempt.rs:110-128`). That is out of scope.
- **A crash after capture,** which is not a refusal, still discards on resume, as today. It is pre-existing, and not
  this finding.
- **Kept pins accumulate,** one per refused attempt, until the operator removes them.

### 1.5 The exact unfreeze texts, and how each matches its implementation (FUB-D5-UNFREEZETEXT)

These are amendments to `effects/allowlist.toml`. Each entry's `path`, `allows`, `packet` and `shrinks_when` stay as
they are, and so does `FROZEN_LEGACY_ALLOWLIST` (`src/effects.rs:1306`).

**`src/workspace.rs` (`:898-924`).** "AMENDED ONCE" becomes "AMENDED TWICE". The last sentence, "The schema-4
equivalents live behind funnels in `crate::workspace_manager` and nothing here calls them: the constant is read, and no
funnel is called.", is replaced by:

> The second amendment, to close `PR329-LEGACY-RUNS-IN-LINKED-CHECKOUTS-RACE-THE-SHARED-WORKTREE-REGISTRY` on the
> owner's decision to unfreeze the module for this one change, is one thing and no more: the three Git children that
> enumerate the repository's worktree registry — `switch_branch`'s `git switch`, `add_gate_worktree`'s `git worktree
> add`, and `cleanup_gate_workspace`'s `git worktree remove` together with the `git worktree list` that decides whether
> it took the registration — each run as the attempt of `crate::workspace_manager::tolerant_registry_access`, which
> attempts one again until its deadline and then refuses as a registry refusal (`UpstrokeError::RegistryRefused`),
> never as Git state. The add holds the registry lock shared, and is attempted again only while its destination is
> still the empty directory this module made for it (on Unix, the same device and inode); the other two are attempted
> again whatever failed. One private helper resolves the canonical common git dir as `recorded_objects_scope` does,
> through `git_command`, and one private type records the destination; the test module gains the regression tests for
> the three. Every other behaviour of the module stays frozen. The schema-4 equivalents live behind funnels in
> `crate::workspace_manager`, and nothing here calls a funnel: the constant is read, and the tolerant access is called,
> which takes no site.

**`src/engine/attempt.rs` (`:869-877`).** One paragraph is appended to `legacy_effect`:

> AMENDED ONCE, on the owner's decision to close the static and deadline residue of
> `PR329-LEGACY-RUNS-IN-LINKED-CHECKOUTS-RACE-THE-SHARED-WORKTREE-REGISTRY`: `run_attempt` takes one more argument, a
> slot in which it records the candidate it captured — the branch ref, parent and tree `capture_candidate` returned —
> when the gate or the review snapshot's worktree-registry access refuses (`UpstrokeError::RegistryRefused`). The two
> snapshot calls record it through one private function that compares the error's variant and copies the three
> strings, and the type that holds them is new; the error `run_attempt` returns, and every other step, are as before.
> It calls nothing of the workspace, the runner or the event log that it did not already call, and nothing else in the
> module moves.

**`src/engine/coordinator.rs` (`:834-853`).** One paragraph is appended to `legacy_effect`:

> AMENDED ONCE, on the owner's decision to close the static and deadline residue of
> `PR329-LEGACY-RUNS-IN-LINKED-CHECKOUTS-RACE-THE-SHARED-WORKTREE-REGISTRY`: when `run_attempt` fails after recording a
> refused candidate, the coordinator does not discard the checkout; it pins that candidate — the branch ref, parent and
> tree captured before the refusal, never the index as it stands at the refusal — through
> `Workspace::prepare_commit_from_candidate` at the attempt's `prepared_pin_ref` followed by `KEPT_PIN_SUFFIX`, one new
> constant, and returns a registry refusal (`UpstrokeError::RegistryRefused`) that names the pin, or the pin's failure.
> Every other attempt error discards the checkout as before. The new arm calls only `prepared_pin_ref` and
> `Workspace::prepare_commit_from_candidate`, both of which the module already calls, and nothing else in the module
> moves.

**`src/engine/resume.rs` (`:855-867`).** One paragraph is appended to `legacy_effect`:

> AMENDED ONCE, on the owner's decision to close the same finding's static and deadline residue: for each interrupted
> attempt the resume also asks, through `Workspace::prepared_pin_target`, whether the coordinator kept that attempt's
> candidate at its `prepared_pin_ref` followed by `KEPT_PIN_SUFFIX`, a pin no resume removes; it discards the
> checkout's uncommitted paths exactly as before, so the attempt runs again from a clean tree, and one warning names
> every kept pin with the commands that take its output back, deletions included. Of the workspace it calls only
> `prepared_pin_target`, which it already calls, and nothing else in the module moves.

**`src/engine/tests.rs` (`:1138-1150`).** One paragraph is appended to `legacy_effect`:

> AMENDED ONCE, on the owner's decision to close
> `PR329-LEGACY-RUNS-IN-LINKED-CHECKOUTS-RACE-THE-SHARED-WORKTREE-REGISTRY`: the file gains appended regression tests,
> and no existing test changes. They drive the same entry points through a worktree registry another process has torn.
> One of them makes that registration as a topology slot, through `crate::workspace_manager`'s funnels and its test
> fixture's torn-registration shape, under the same allow.

**How each text matches its implementation.** Every clause of each text is listed here with the element of §1.3 or
§1.4 that makes it true. The design lens's FUB-D5-UNFREEZETEXT was that round 5's coordinator text said "calls nothing
it did not already call" while the specified arm called `current_branch_ref()` and `staged_tree_oid()`. D's arm calls
neither: it pins the recorded candidate.

| Text | Clause | The implementation that makes it true |
|---|---|---|
| `workspace.rs` | three Git children, each the attempt of the access | §1.3 items 1–3 |
| `workspace.rs` | "until its deadline and then refuses … never as Git state" | the contract's returns (§1.2 item 3): only a veto returns Git, and only the add vetoes |
| `workspace.rs` | the add holds the lock shared, and is attempted again only while its destination is unchanged and empty | `RegistryHold::Shared`; `again = owned.unchanged(path)` (§1.3 item 2) |
| `workspace.rs` | "the other two are attempted again whatever failed" | the switch's and the removal's `again` always answers true |
| `workspace.rs` | one private helper through `git_command`; one private type | `canonical_common_dir` (through `git_path` → `git_output` → `git_command`); `OwnedDestination` |
| `workspace.rs` | the test module gains tests | T-L1 to T-L5 (§1.9) |
| `workspace.rs` | "nothing here calls a funnel … the tolerant access is called, which takes no site" | `tolerant_registry_access` is `effect_free` and takes no `EffectSiteId` (#329's record §5.5) |
| `attempt.rs` | one more argument; records the captured identity on a registry refusal at the two snapshot calls | `refused: &mut Option<RefusedCandidate>`; `inspect_err` at `:154` and `:178` |
| `attempt.rs` | one private function that compares the variant and copies three strings; a new type | `note_refused`; `RefusedCandidate` |
| `attempt.rs` | the error returned and every other step are as before | the return type, every other `?` and the capture at `:129` unchanged |
| `attempt.rs` | calls nothing of the workspace, the runner or the event log that it did not already call | `note_refused` calls `matches!` and `to_owned`; `inspect_err` is std's |
| `coordinator.rs` | does not discard; pins the recorded candidate, never the index | the arm reads only `candidate.*`; no `staged_tree_oid`, `current_branch_ref` or `head_sha_full` call is added |
| `coordinator.rs` | at `prepared_pin_ref` + `KEPT_PIN_SUFFIX`, one new constant; a refusal naming the pin or its failure | `KEPT_PIN_SUFFIX` at `:281`; the two messages |
| `coordinator.rs` | every other error discards as before | the `None` branch is the existing code |
| `coordinator.rs` | the new arm calls only `prepared_pin_ref` and `prepare_commit_from_candidate`, both already called | `:657`, `:668` |
| `resume.rs` | asks `prepared_pin_target` per interrupted attempt; never removes the kept pin | `:543-560`; no `remove_*_pin` call names it |
| `resume.rs` | discards exactly as before; one warning with the commands | `:562-570` unchanged; the warning of §1.4 |
| `resume.rs` | of the workspace, calls only `prepared_pin_target`, already called | `:553` |
| `tests.rs` | appended tests; no existing test changes; one makes a topology slot through the manager | T-P1 to T-P9, T-L6 (§1.9); T-P6 calls `WorkspaceManager::derive`, `create_execution_root`, `write_intent`, `add_worktree` and `fixture::tear_registration` |

### 1.6 The behaviour change for legacy users

What a schema 1–3 run sees once D is implemented, and #329 with it:

- **A registration another process is writing no longer fails the run.** A legacy run whose repository has other
  worktrees being written meets them in three places: its gate or review snapshot add, the snapshot's removal, and its
  resume's branch switch. Each is attempted again, after a backoff of 1 ms doubling to 50 ms, for up to 10 s. A write
  that finishes in that time is passed, and the run goes on as if nothing happened.
- **A registration that stays in the way refuses resumably.** It may be a writer's residue, contention that outlasts
  10 s, or any other fault of the store.
  - The command ends with a registry refusal that names the store, the deadline, the attempt count and Git's last
    message, instead of a Git error at once.
  - The process exit status is unchanged: 1 for any error (`src/main.rs:205-212`).
- **The paid output survives a refusal after capture.** When a snapshot is refused after the worker's output was
  captured, the checkout is not discarded.
  - The captured candidate is pinned at `refs/upstroke/prepared/<run>/<task index>-<attempt>-kept`.
  - The refusal says so: "…; the worker's output for attempt <n> of `<task>` is kept in this checkout and pinned at
    `<pin>`".
- **On the next resume:**
  - the checkout's copy is discarded as before, and the attempt runs again from a clean tree;
  - one warning names each kept pin, with `git restore --source=<pin> --staged --worktree -- .` (while HEAD is still
    the pin's parent, from the checkout's root) and `git cherry-pick --no-commit <pin>` (on a later HEAD);
  - no resume removes a kept pin. The operator removes it with `git update-ref -d <pin>`.
- **A resume over residue that is still there** refuses at its reclaim, before it discards anything.
- **A new run instead of a resume** refuses over the kept output, as it refuses over any uncommitted work
  (`src/engine/coordinator.rs:149-160`). The pin keeps the output whichever the operator chooses.
- **Genuine failures, before and after D:**

  | Failure | At master | After D |
  |---|---|---|
  | A snapshot whose checkout cannot be made (the add's own failure, after Git took the destination over) | a Git error at once; the coordinator discards | the same |
  | A failure of the switch or the removal that is not contention | a Git error at once | refused after 10 s; nothing is discarded on either path |
  | A legacy add's registry-phase fault that is not contention, such as a store nothing can write | a Git error; the output is discarded | refused after 10 s; the output is kept |

- **Time.** A snapshot's drop may wait up to 10 s when the store is in the way. A refused add costs two deadlines, the
  add's and then its cleanup's.
- **Kept pins accumulate** until the operator removes them.

### 1.7 The packet invariants D touches, and `DESIGN.md`

**PR5's `slice_contract.invariants_preserved[0]`.** It reads "existing Workspace and legacy engine behavior untouched
(moves are behavior-neutral; legacy tests unchanged; legacy run directories with a committed run_started remain
listed; EventLog semantics unchanged for legacy callers)" (`~/orch-pr11/logs/pr11_fub_design/packet/legacy-invariants.txt`).
- Corrected B1′ changes `src/workspace.rs`'s behaviour: a registry access is attempted again, then refused, never
  returned as Git state.
- B-PRESERVE changes the legacy coordinator's error path, `run_attempt`'s arguments, and the legacy resume's warnings.
- No existing legacy test changes. All 188 `engine::tests` tests and all 47 `workspace::tests` tests pass on both
  prototypes (`fud/probe/suite-d/suite-3.log`, `suite-d6-nowit/suite-1.log`). D appends tests to the frozen test file.
- The event log is unchanged: D adds no event and no field.

**PR12's `slice_contract.invariants_preserved[0]`.** It reads "every invariant; legacy resume unchanged; …".
- The resume's control flow is unchanged: it still discards, and runs the attempt again.
- It reads one more ref per interrupted attempt, and adds one warning.
- Its own registry accesses change: the reclaim's removal (`src/engine/resume.rs:426`) and `switch_branch` (`:454`) are
  attempted again or refused, never returned as a Git error.

**Both invariants are the owner's to amend.** Neither is a packet row this lane can change. Decision B is the owner's
reading of the full scope.

**`DESIGN.md` §15.** Its resume paragraph says "A pin without a successful settlement is orphan residue and is
removed without dereferencing symbolic refs" (`design/15_design_event_log_resume_run_layout.md:154`). A kept pin is a
pin without a successful settlement that is not orphan residue and is not removed.
- So D adds a PROPOSED paragraph right after it at this head, "A legacy attempt the worktree registry refused". It
  becomes the in-force text when D is implemented.
- It sits apart from #329's PROPOSED paragraph, which follows the crash-containment paragraph (`:64`), so the two
  hunks do not meet.
- No sentence `src/export.rs` pins moves.

### 1.8 What D closes, what remains, and what G6 meets

**The claims, once D and #329 are implemented:**
1. **No legacy registry access returns `UpstrokeError::Git` for anything the registry's state caused,** whoever the
   writer. Only the add's own failure after Git took its destination over comes back as Git state. This rests on
   #329's helper (§1.2 item 3) and D's veto (§1.3 item 2).
2. **A legacy attempt that a registry refusal stops after capture loses nothing.** Its captured candidate stays in the
   checkout and is pinned. The resume names the pin with commands that take it back, deletions included, and never
   removes it. The exception is the pin-write failure of §1.4.
3. **Every other legacy error path is as at master.**

**The G6 classification.** These are the lenses' consensus (`~/orch-pr11/reviews/review-329-d4-triage.md`), the
finding's reading, and #329's record §5.6 at `ed3a97d9`. Per the addendum, D closes the mixed cases only together with
#329's helper: round 6 replaced rounds 3–5's classifier with the caller's veto (§1.2). D's (e1) and (e2) closure
therefore depends on that helper converging in #329's review and landing as its §5.5 states.

| Case | What D closes, given #329's helper | Severity | Applies to G6 | Blocks G6 |
|---|---|---|---|---|
| (e1) Legacy against legacy, write in flight | **Corrected B1′**, with #329's helper: a write that finishes within the deadline is attempted past. It depends on #329's helper converging | P1 | no | no; it remains a P1 until D lands |
| (e1′) Legacy against legacy, static or deadline residue | **B-PRESERVE with B1′**: the captured candidate is kept and pinned, and the resume names a restore that restores deletions. It is conditional on the pin being written | P1 | no | no; it remains a P1 until D lands |
| (e2) A topology writer tears a legacy reader, write in flight | **Corrected B1′, together with #329's helper** | P1 | yes: Q6, across the shared registry and R17 | **yes, until #329 and D are implemented and validated** |
| (e2′) A topology writer's static or deadline residue makes a legacy reader refuse, then discard paid output | **B-PRESERVE with B1′, together with #329's helper,** whose `RegistryRefused` it keys on. Executed with a real topology slot's torn registration (w5) | P1 | yes: Q6; a crash producer engages Q1; distinct from DESC, because no surviving writer is needed | **yes, until #329 and D are implemented and validated,** unless the owner rules otherwise |

**What remains after D:**

| | What | Consequence | Where |
|---|---|---|---|
| R-D1 | A pin that cannot be written (`HEAD` moved, or the ref store failed at that moment) | the output is only in the checkout, and the refusal says so; a resume on a moved `HEAD` refuses before its discard; a ref-store fault means the resume discards as today, which takes two faults | §1.4 |
| R-D2 | A crash after capture | the resume discards, as today; pre-existing, not this finding | §1.4 |
| R-D3 | D's own Git child killed by a signal between the takeover and its first write | attempted again until the deadline, then refused: kept, never Git state | §1.3 |
| R-D4 | Windows: no file identity in std at MSRV | the veto reads only "an empty directory"; a takeover whose junk removal left the destination empty refuses at the deadline: kept | §1.3 |
| R-D5 | A resumed run that fails | it returns its error without the resume's warnings, as every legacy warning; the pin was named by its own refusal, and stays | §1.4 |
| R-D6 | A genuine switch or removal failure | refuses after the deadline instead of failing at once; nothing is discarded on either path | §1.6 |
| R-D7 | Kept pins | accumulate until the operator removes them | §1.6 |
| R-D8 | #329's helper | D's (e1) and (e2) closure depends on it converging and landing as contracted | §1.2 |

**The finding, after D lands:** its file is deleted, and (e1), (e1′), (e2) and (e2′) close. Without decision B, see
§1.12.

### 1.9 Tests: the planned regressions, the witnesses executed, and the mutations

**The planned tests.** Each is the implementation's, named by the implementer.
- Each waits on a handshake: the helper's `#[cfg(test)]` counter of attempted-again accesses for the repository's
  common git dir (§1.2 item 8), or the legacy engine's capture hook (`after_candidate_capture`). Time is only a
  watchdog.
- Each was prototyped as a witness through the real legacy engine on the scratch prototypes. The witness is named in
  brackets.
- The workspace-level ones are #329 round 5's legacy witnesses, run on D over round 6's helper.

**In `src/workspace.rs`'s inline test module (under its unfreeze):**
- **T-L1:** the snapshot add beside a tear that finishes after the first failed attempt: Ok after 2 attempts. The
  destination is present and empty after the first failure. [d5 `legacy_add_beside_a_transient_tear`]
- **T-L2:** beside a tear that stays: `RegistryRefused` at the deadline, never Git. The intent stays for the reclaim.
  [d5 `legacy_add_beside_a_static_tear`]
- **T-L3:** a snapshot whose checkout cannot be made (a 300-byte name): Git after exactly 1 attempt, and the
  destination is gone. [d5 `legacy_checkout_cannot_be_made`]
- **T-L4:** the removal beside a tear that finishes after the first failed attempt: 2 attempts, the snapshot
  unregistered and its directory gone. [d5 `legacy_removal_beside_a_transient_tear`]
  - This also distinguishes round 4's B1′, which ignored the removal's exit status and wrapped the list apart. There
    the first removal fails, the tear finishes, and the separately wrapped list then succeeds and still lists the
    target, so the drop errors.
- **T-L5:** `switch_branch` beside a tear that finishes after the first failed attempt switches. `HEAD` and the status
  are unchanged after the failed attempt. [d5 `git-level-v.log` S, at the Git level]

**Appended to `src/engine/tests.rs` (no existing test changes):**
- **T-P1:** a static tear right after capture. The run ends `RegistryRefused`, naming the kept pin. The checkout still
  holds the candidate. The pin's commit has the captured parent and tree. [w1]
- **T-P2 (FUB-D5-INDEX):** the index is unstaged after capture. The pin's tree is the captured tree, not the index's.
  After the resume discarded the checkout's copy, following the warning brings the worker's file back. [w2]
- **T-P3:** the resume while the residue stays refuses `RegistryRefused` from the reclaim, and changes nothing: the
  status and the pin are as before. [w1]
- **T-P4 (FUB-D5-RESTORE, at the parent):** a tracked edit, a new file and a deletion. Following the warning's restore
  leaves the index equal to the pin's tree, and the deleted file gone. [w3]
- **T-P4b (FUB-D5-RESTORE, on a later HEAD):** the resumed run commits the task again. Following the warning's command
  for a later HEAD stages exactly `D`, `A` and `M`, and the new commit stays. [w3b]
- **T-P5:** the resume after the repair completes. Its warning names the pin, and the pin remains. [w1]
- **T-P6, (e2′) itself:** the residue is a topology slot's registration. The workspace manager adds it and its
  fixture's `tear_registration` tears it, in the same repository. The run ends `RegistryRefused` with the candidate
  kept. After the repair the resume completes, names the pin, and keeps it. [w5]
- **T-P7:** an attempt error that is not a registry refusal (the capture hook's error) still discards, as at master. [w4]
- **T-P7b:** a snapshot failure after capture that is not the registry's (the snapshot store cannot be made) still
  discards. [w4b]
- **T-P8:** a pin that cannot be written (`HEAD` moved after capture). The refusal says pinning failed, and nothing is
  discarded. [w6]
- **T-P9:** the review snapshot's site (`attempt.rs:178`): no gates and one reviewer, with the same result as T-P1. [w9]
- **T-L6:** the legacy engine completes past a transient tear, with nothing discarded and no pin. It runs once with a
  legacy-shaped writer [w7], and once with a topology slot's registration as the writer, which is (e2) itself [w8].

**The witnesses, executed** (`fud/probe/`; three rounds each; each test alone, `--exact`, one thread, from its own
tree; every binary with its own Compiling line, `build-summary.txt`):

| | w1 | w2 | w3 | w3b | w4 | w4b | w5 | w6 | w7 | w8 | w9 |
|---|---|---|---|---|---|---|---|---|---|---|---|
| master (`base`) | red | red | red | red | ok | ok | red | red | — | — | red |
| #329 round 5's B-PRESERVE as reviewed (`r5`) | ok | **red** | **red** | **red** | ok | ok | ok | red | ok | ok | ok |
| D on round 5's helper (`d`) | ok | ok | ok | ok | ok | ok | ok | ok | ok | ok | ok |
| D on round 6's helper (`d6`) | ok | ok | ok | ok | ok | ok | ok | ok | ok | ok | ok |

Sources: `witness-runs/TABLE.txt` and `witness-runs-r6/TABLE.txt`, with `RESULTS.txt` and one log per test, variant and
round beside each.
- On master, every tear-based witness fails as Git state with the output discarded. w7 and w8 need the round-5 seam,
  which master lacks.
- `r5`'s reds are FUB-D5-INDEX, reproduced through the engine: w2's pinned tree is the index's, not the captured one.
  They are also FUB-D5-RESTORE: w3 and w3b, following round 5's `git checkout <ref> -- .`, leave `deleted.txt`.
- `r5` pins the index on a moved `HEAD` where D refuses to pin (w6).

**The mutations.** Each was built as its own scratch tree with its own Compiling line (`patch-d-mutations.py`), and
each turns at least one witness red in all three rounds:

| Mutation | Undoes | Red |
|---|---|---|
| `m-index`: the coordinator pins the live index and `HEAD` at refusal | FUB-D5-INDEX | w2, w6 |
| `m-restore`: the warning advertises `git checkout <pin> -- .` | FUB-D5-RESTORE | w3, w3b |
| `m-discard`: the refused arm discards, as every other error | B-PRESERVE | w1, w2, w3, w3b, w5, w6, w9 |
| `m-removepin`: the resume removes kept pins | the resume's keep | w1, w2, w3, w3b, w5 |
| `m-keepall`: a Git snapshot error also records the candidate | "only a registry refusal is kept" | w4b |
| `m-b1pred` (round 5's helper) and `d6-m-b1pred` (round 6's): round 4's "nothing at the slot" | FUB-D4-B1PREDICATE | w1, w2, w3, w3b, w5, w6, w7, w8, w9; and the d5 legacy witnesses flip both ways |

**The planned mutations** for the implementation's proof:
- the six above;
- `again` always true on the add, which turns T-L3 red: 15 checkouts, then `RegistryRefused`;
- the removal alone as the attempt, which turns T-L4 red;
- `switch_branch` unwrapped, which turns T-L5 red.

**The suites.** All are `cargo test --all-targets --all-features` through `upstroke-build`, every source touched
first (`fud/probe/SUITES.txt`):

| Tree | Passed | Failed | Notes |
|---|---|---|---|
| D on round 5's helper, with the witnesses (`suite-d/suite-3.log`) | 3,009 | 2 | 105.05 s |
| D on round 6's helper, without the witnesses (`suite-d6-nowit/suite-1.log`) | 2,998 | 2 | 117.20 s; clippy rc 0 |

- **The two failures in each** are the non-frozen manager tests every #329 round since 4 has moved, #329's to move:
  `a_removal_records_the_one_attempt_the_unix_arm_makes` and
  `an_add_killed_before_it_wrote_gitdir_is_unlisted_and_refuses_forced_cleanup`.
- **Every legacy test and every census passed:** all 188 `engine::tests`, all 47 `workspace::tests` and every effects
  census.
- **Two earlier runs are invalid, and kept** (`suite-d/suite-1.log`, `-2.log`). The sources had not been touched, and
  cargo ran the lib-test binary of the mutation tree built last in the shared slot. It printed the tree's own
  Compiling line for another unit. Those runs showed the mutation's symptom exactly.

**The proof the implementer owes:**
- each planned mutation red, on a scratch tree whose Compiling line names it;
- the witnesses red on master where they apply;
- the frozen topology modules and their test children unchanged;
- the ten gates, and CI on every leg;
- at least five full suites, with every frozen test's failures counted against the same number of suites at the base;
- each planned test's Windows `$GIT_DIR` measured against the 220-character budget. The registrations these tests make
  live under the scratch repository's `.git/worktrees/`, which existing legacy tests already use.

### 1.10 Instruments, with counts

| Instrument | D's edit | Count |
|---|---|---|
| `effects/allowlist.toml` | the `legacy_effect` texts of §1.5 | **5 texts amended** (`src/workspace.rs` twice-amended; `src/engine/attempt.rs`, `src/engine/coordinator.rs`, `src/engine/resume.rs` and `src/engine/tests.rs` once); 0 rows added or removed; 0 `path`, `allows`, `packet` or `shrinks_when` changes |
| `src/effects.rs` `FROZEN_LEGACY_ALLOWLIST` | none | 0 |
| `effects/wrappers.toml` | none: the helper's `effect_free` row is #329's (its record §5.5, §5.8). The out-parameter form adds no classified name; the enum form would have added `from` twice (§1.4) | 0 (1 if #329 does not land the row) |
| `clippy.toml` | none: no denied method is added or called outside an existing allow | 0 |
| `.github/`, `scripts/`, `.cargo/`, a toolchain file, `Cargo.toml`'s `[lints]`, the CI-contract tests under `src/effects/` | none | 0 |

**The frozen set D unfreezes:** five PR5-frozen files, `src/workspace.rs`, `src/engine/attempt.rs`,
`src/engine/coordinator.rs`, `src/engine/resume.rs` and `src/engine/tests.rs`. Four of them change production code,
and the test file is append-only. D touches none of G6's frozen topology modules.

**At this head:** no instrument changes. The amendments are the implementation's, after decision B.

### 1.11 The Windows time budget

The planned tests' cost on each slow leg, estimated from measured multipliers (`fud/wintime/BUDGET.txt`):
- **The multipliers come from two existing legacy engine tests of the same shape.** Each was measured solo on this box
  (`fud/probe/ref-times.txt`), and against its duration reconstructed from CI logs of #329's run at `4a126215`, whose
  code is master's. The tool is `wintime-durations.py`, with 12 threads on the guest, 4 hosted and 3 on macOS.
  - `resume_removes_a_pin_whose_successful_settlement_never_landed`: 0.37 s here; 7.25 s on the guest, 9.63 s on the
    hosted queue lane and 2.19 s on macOS.
  - `gates_review_and_commit_use_one_frozen_candidate_tree`: 0.25 s here; 5.02 s, 5.79 s and 2.0 s.
  - So the factors are 19.8× on the guest, 26.3× hosted and 7.9× on macOS, taking the larger of the two each time.
- **How each planned test is costed.** Each witness's Linux wall time is split into its fixed waits (500 ms per refused
  access under test) and its Git work, which is scaled by the factor.

| Leg | Added test time | Over | Harness wall | Current harness | The job |
|---|---|---|---|---|---|
| `test (winguest)` | about 91 s | 12 threads | about +8 s | 485.6 s at `4a126215` (`fud/wintime/winguest-110740864990.log`) | 9m17s of its 20-minute limit |
| hosted `windows-latest`, queue only | about 118 s | 4 threads | about +30 s | 1,631 s at #326's queue run (`~/orch-pr11/logs/wintime/queue326-windows-latest.log`) | about 29m53s of 45 |
| macOS | about 41 s | 3 threads | about +14 s | 1,105 s at `4a126215` (`fud/wintime/macos-110740864997.log`) | 20m22s of 30 |

- **The longest single test** is about 10 s on the guest and 12 s hosted (T-P1, T-P6). That is far from any tail
  effect.
- **None of this is executed on those legs.** It is an estimate from measured multipliers, and CI is the truth for
  them. The hosted lane runs only in the merge queue (`.github/workflows/ci.yml:146-148`), so a pull request's own CI
  never shows it.

### 1.12 Risks, sequencing, and what is out of scope

**If the owner declines.**

| Decision | (e1) | (e1′) | (e2) | (e2′) | The finding |
|---|---|---|---|---|---|
| B-PRESERVE declined, B1′ taken | closed | an open P1; does not apply to G6 | closed | an applicable P1 that blocks G6, unless the owner rules otherwise | stays open, narrowed to the residue |
| Both declined | a P1 | a P1 | an applicable P1 | an applicable P1 | stays as filed |
| B-PRESERVE without B1′ | — | — | — | — | not possible: B-PRESERVE keys on the refusal B1′ produces |

- **The ruling ESCALATION item 7 already offers:** that the mixed residue case does not block G6, with the P1 kept
  filed. Without such a ruling, no waiver is inferred.

**Sequencing.**
- **D's implementation follows #329's merge,** because it calls #329's helper (§1.2), and follows the owner's decision B.
- **The finding file.** #329's `ed3a97d9` rewrote its copy's guard to name D, so the two copies now differ. D carries
  `4a126215`'s, as its brief directs.
  - If #329 merges first, D rebases and takes master's text.
  - D's implementation deletes the file either way.
  - The orchestrator coordinates the order.
- **The hunks do not meet:**
  - D's are in four frozen legacy modules, the frozen test file, five `effects/allowlist.toml` texts and one
    `design/15` paragraph;
  - #329's are in `src/workspace_manager.rs`, `src/error.rs`, one `effects/wrappers.toml` row and another `design/15`
    paragraph;
  - follow-ups A and C own other files.

**Risks.**
- **Static tears cost two deadlines.** A run over another run's residue waits 10 s at the add and 10 s at the cleanup,
  then refuses.
- **Genuine switch and removal failures** refuse after 10 s instead of failing at once (R-D6).
- **`run_attempt` gains an argument.** Its only caller is the legacy coordinator (`src/engine/coordinator.rs:544`).
- **Kept pins accumulate** (R-D7).
- **The legacy add's veto rests on Git's order** of steps before the takeover. That order is audited for 2.43.0,
  2.50.1 and 2.55.0. A Git that wrote into the destination before its registry phase would turn contention into the
  add's own failure, which is Git state and a discard. None of the three does.

**Out of scope, and said so.**
- Adopting a kept candidate into the legacy run (§1.4).
- A crash after capture (R-D2).
- Every other legacy error path.
- DESC (follow-up C) and #329's topology accesses.

### 1.13 The findings D carries, answered

| Finding | Sev | Kind | D | Where | Evidence |
|---|---|---|---|---|---|
| FUB-D5-INDEX | P1 | executed (regression lens) | **Fixed (design), witnessed.** `run_attempt` records the captured branch, parent and tree on a registry refusal at either snapshot (`attempt.rs:154`, `:178`), and the coordinator pins exactly that; `attempt.rs` is in the unfreeze | §1.4, §1.5 | w2 red on `r5` and green on `d` and `d6`; `m-index` red |
| FUB-D5-RESTORE | P2 | executed (all three lenses) | **Fixed (design), witnessed.** The warning names `git restore --source=<pin> --staged --worktree -- .` from the checkout's root while HEAD is the pin's parent, and `git cherry-pick --no-commit <pin>` on a later HEAD; both restore deletions | §1.4 | `fud/git-level/restore-shapes.log`; w3 and w3b; `m-restore` red |
| FUB-D5-UNFREEZETEXT | P3 | reasoned (design lens) | **Fixed (design).** Every text names exactly what its implementation adds and calls; the coordinator's arm calls only `prepared_pin_ref` and `prepare_commit_from_candidate`, both already called | §1.5 | the conformance table |
| FUB-D4-B1PREDICATE | P1 | executed (all three lenses, round 4) | **Carried, witnessed on round 6's helper.** The owned, unchanged, empty destination; the fourth clause is left out, with reasons | §1.3 | d5 `legacy_add_beside_a_transient_tear` and `legacy_checkout_cannot_be_made` on `d6`; `d6-m-b1pred` red |
| FUB-D4-B1REMOVE | P2 | executed (round 4) | **Carried, witnessed.** The removal and its list are one attempt | §1.3 | d5 `legacy_removal_beside_a_transient_tear` on `d6`; `d5/witness/git-level-v.log` R |
| FUB-D4-RESUME | P2 | reasoned (round 4) | **Carried.** The resume keeps the kept pin and names it | §1.4 | w1, w5; `m-removepin` red |
