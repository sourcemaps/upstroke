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
branch. Round 2 (§2) is `pr11_fud_design2`'s; its figures are under `~/orch-pr11/logs/pr11_fud_design2/`, cited as
`fud2/…`.

## 0. Status

| Phase | State |
|---|---|
| Design (§1) | **PROPOSED, pending the owner's decision B** and design review. Corrected B1′ (§1.3) and B-PRESERVE (§1.4) carry #329's design review round 5 findings against them: FUB-D5-INDEX, FUB-D5-RESTORE and FUB-D5-UNFREEZETEXT. They are designed against #329's round-6 helper contract (§1.2). Each was executed through the real legacy engine on scratch prototypes, with the mutations that turn them red (§1.9). Design review round 1 (`37e4d8c4`): three lenses, CHANGES_REQUIRED, no P1. Round 2 (§2) amends §1 where it is marked. |
| Design, round 2 (§2) | **PROPOSED, pending design review and the owner's decision B.** It answers round 1's review: the recovery commands refuse replacement objects (FUD-D1-REPLACE), every resume names every kept pin of the run (FUD-D1-PINWARN), and the progress claim is qualified (FUD-D1-PROGRESS). It carries R-G: every legacy Git child runs with Git's automatic maintenance off, as #330's round 3 requires. #329's round 7 published while this round ran (`85f5b09b`), and §1.2 is conformed to its dated §5.5 (§2.6). D's add adopts round 7's veto, which closes FUD-D2-PRUNE for a prune before the add returns (§2.7). Two of the five exact unfreeze texts change (§2.9). Each change was executed through the real legacy engine on Git 2.43.0, 2.50.1 and 2.55.0, with a mutation that turns it red (§2.8). This head changes no code: it carries this record's §2 and the amended PROPOSED paragraph in `design/15`. |
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

> **Round 2:** §2.6 conforms this subsection to #329's §5.5 as its round 7 dated it on 2026-10-02 (`85f5b09b`):
> `Again`, the final attempt, an end-to-end bound, and `CONTENDED_ATTEMPTS`.

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

> **Round 2:** §2.5 adds a second change to this module: `git_command` refuses Git's automatic maintenance (R-G).
> §2.7 replaces item 2's veto with #329's round-7 rule (the removal proof and a registry-free checkout probe), and
> `OwnedDestination` goes. Items 1 and 3 pass `Again::Attempt`.

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

> **Round 2:** §2.2 replaces the resume's warning (FUD-D1-REPLACE). §2.3 replaces the resume's lookup and narrows
> the residual on a resume that fails (FUD-D1-PINWARN). `attempt.rs` and `coordinator.rs` are unchanged.

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

> **Round 2:** §2.9 restates all five texts in full. The `src/workspace.rs` and `src/engine/resume.rs` texts change;
> the other three are these, word for word.

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

> **Round 2:** §2.4 replaces the progress claim (FUD-D1-PROGRESS), and §2.11 amends the warning, maintenance and
> resume-time items.

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

> **Round 2:** §2.11 and §2.12 amend the residuals and the G6 table: R-D4 covers every platform, R-D5 is narrowed,
> R-D9 to R-D12 are added, and R-G and FUD-D2-PRUNE have rows.

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

> **Round 2:** §2.8 adds T-P10 to T-P15 and T-L7, and three mutations. Every witness below is green on round 2's
> prototype too.

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

> **Round 2:** still five `legacy_effect` texts and no rows. Two of the texts change (§2.9).

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

> **Round 2:** §2.14 adds round 2's planned tests: about +5 s on the guest, +18 s hosted and +9 s on macOS.

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

## 2. Round 2 design

> **PROPOSED — pending design review and the owner's decision B** (`~/orch-pr11/ESCALATION.md` item 7). Round 2
> answers design review round 1 on `37e4d8c4`. §1 stays the design except where §2 replaces it. Each place it does
> is marked in §1, and §2.13 lists them. §2.9 restates all five exact unfreeze texts in full and marks the two that
> change. **Every changed or new text is a PROPOSAL until it has been reviewed.** The owner decides B only on
> reviewed exact text.

**Who writes it.** Round 2 is `pr11_fud_design2`'s (`claude-opus-5-5`, `max`), a fresh session the PR11 orchestrator
spawned with the brief `~/orch-pr11/briefs/pr11_fud_design2.md`. Its work list is the triage
`~/orch-pr11/reviews/review-331-d1-triage.md`. Its figures are under `~/orch-pr11/logs/pr11_fud_design2/`, cited as
`fud2/…`. Code is cited at master `5c222ff2`. The five legacy files, `src/events/mod.rs` and the files #329 patches
are blob-identical there and at the branch point `92c4ca81` (`fud2/code-citations-5c222ff2.txt`). The branch is not
rebased this round.

### 2.1 What round 1's review found, and what round 2 changes

**The review.** Three `gpt-6-astra` lenses at `max` reviewed `37e4d8c4`: design (as a conformance reading),
concurrency and regression. Each returned **CHANGES_REQUIRED**, and none found a P1.
- The texts are `~/orch-pr11/reviews/review-331-d1-{design,concurrency,regression}-37e4d8c4.review.md`, with their
  hashes in `SHA256SUMS-331-d1`.
- The reviewers' witnesses are in `~/orch-pr11/reviews/331-d1-witnesses/`.
- **What all three accepted, and round 2 keeps as it is:**
  - the captured-candidate pin;
  - the add veto without the "named by no registration" clause;
  - the scope of the five unfreeze texts;
  - the `CONTENDED_ATTEMPTS` handshake.

**The work list.**

| Item | Severity | What round 2 does | Where | Evidence |
|---|---|---|---|---|
| FUD-D1-REPLACE | P2 | The two recovery commands carry the replacement controls every legacy Git child carries. | §2.2 | Git level on 2.43.0, 2.50.1 and 2.55.0; engine witnesses w10, w11, w12 and w12g on the same three |
| FUD-D1-PINWARN | P2 | The resume looks for a kept pin at every attempt the replayed log records, not only those still in flight. Retirement is stated. | §2.3 | engine witnesses w13, w14 and w15 |
| FUD-D1-PROGRESS | P3 | The claim is qualified to a successful attempt before the deadline. | §2.4 | — |
| R-G (`FUC-D2-RG`) | P1 in #330's round-2 triage | Every legacy Git child runs with Git's automatic maintenance off: follow-up C's stated requirement, matched exactly. | §2.5 | a census of the legacy Git children, and witnesses rg1, rg2, rg3 and l7 through the real legacy engine on the three Gits |
| §1.2 against #329's §5.5 | — | #329's round 7 published while this round ran (`85f5b09b`, 11:44Z). §1.2 is conformed to its dated §5.5: `Again`, the final attempt, the end-to-end bound and `CONTENDED_ATTEMPTS`. | §2.6 | — |
| FUD-D2-PRUNE (found here) | P1 class | D's legacy add inherited #329's FUB-D6-PRUNE. It adopts round 7's veto (§6.3), which closes it for a prune before the add returns. A prune that lands after the add returned remains (R-D9). | §2.7 | v1, v2 and T-L8 through the real legacy engine on the three Gits, on round 7's contract prototyped |

**What changes in the design's text:**
- the `src/workspace.rs` and `src/engine/resume.rs` unfreeze texts (§2.9);
- the resume's warning (§2.2);
- §1.2's contract and §1.3's add veto, conformed to round 7 (§2.6, §2.7);
- §1.6's progress claim (§2.4);
- the PROPOSED paragraph in `design/15` (§2.10).

Nothing in `attempt.rs`'s, `coordinator.rs`'s or `tests.rs`'s text moves.

**The probe.** Every witness below ran through the real legacy engine on scratch `git archive` copies of `5c222ff2`.
Nothing of it is on the branch (`fud2/probe/setup.sh`).
- **`base`:** master.
- **`r1`:** D's round 1 as its lenses reviewed it: #329 round 4's `patch-iv.py`, round 6's `patch-vi-b.py --with-row`
  and round 1's `patch-d-legacy-r6.py`.
- **`d2`:** `r1` plus round 2 (`fud2/probe/patch-d2.py`).
- **Three mutations,** each `d2` with one round-2 change undone: `d2-m-plain`, `d2-m-interrupted` and `d2-m-maint`.
- **`d3`:** `d2` conformed to #329's round 7 (`fud2/probe/patch-d3.py`), with its own two mutations (§2.7). `base-v` and
  `d2-v` are `base` and `d2` with §2.7's witnesses added.
- **How each ran:**
  - every binary has its own Compiling line (`fud2/probe/build-summary.txt`);
  - each test ran alone, `--exact`, on one thread, from its own tree, three rounds;
  - every run had `GIT_CONFIG_NOSYSTEM=1` and an empty `GIT_CONFIG_GLOBAL`, so Git ran at its defaults
    (`fud2/probe/run-witnesses.sh`).
- **The table:** `fud2/probe/witness-runs/TABLE.txt`, with `VERDICTS.txt`, `RESULTS.txt` and one log per run beside it.

### 2.2 FUD-D1-REPLACE: the recovery commands restore the pin as the repository records it

**The defect, executed by two lenses.** Round 1's warning advertised `git restore --source=<pin> --staged --worktree
-- .` and `git cherry-pick --no-commit <pin>` plainly. Both honour `refs/replace/`.
- The coordinator pins the recorded candidate, because every legacy Git child refuses replacements
  (`design/15_design_event_log_resume_run_layout.md:107`; `git_command`, `src/workspace.rs:43-51`). The operator's
  plain command does not.
- **The design lens** replaced the worker's new file's blob. The restore succeeded with the replacement's bytes in
  the file, while the index equalled the pin's tree. Round 1's witness compared only index trees, so it missed this.
- **The regression lens** replaced the captured tree with its parent's, through the real legacy engine. The restore
  left the paid file absent. A separate Git-level run showed the cherry-pick applying nothing.

**The change.** Each of the two commands carries exactly the controls the legacy builder gives every Git child:
- `--no-replace-objects`, Git's option form of `GIT_NO_REPLACE_OBJECTS=1`. It disables replacements and sets the
  variable for the command's own children (`git.c:189-191` at 2.43.0, `:204-206` at 2.50.1, `:209-211` at 2.55.0;
  `fud2/git-src-citations.txt`);
- `-c core.useReplaceRefs=false`, because on Git 2.41 a configured `core.useReplaceRefs = true` outranks the variable
  (design §15:107), and a command-line setting outranks every configuration file.

The legacy resume already advertises the same pair for `git status` (`src/engine/resume.rs:448`).

**Grafts.** The triage asks for the "replacement and graft controls" the legacy workspace's commands use. A graft made
with `git replace --graft` is a replacement ref, so the same two controls refuse it (case `graft` below). The legacy
workspace has no control over the deprecated `info/grafts` file, and none is added:
- Git's `prepare_commit_graft` reads that file whatever the replacement settings (`commit.c:316-330` at 2.55.0).
- A graft entry changes a commit's parents and nothing else. So it cannot change what `restore --source=<pin>` reads,
  which is the pin's tree.
- It can change the cherry-pick's base only through an entry naming the pin itself. Only an operator could write
  that, after the refusal named the pin.

**The warning, exactly** (`src/engine/resume.rs`, replacing §1.4's text):

> the worker output of the attempt(s) a worktree-registry refusal stopped is kept, and no resume removes it:
> `<pin>`[, `<pin>`…]. Each pin is a commit on the HEAD its output was captured on. To take the output back as the
> repository records it, deletions included, and not as `git replace` substitutes for it: while HEAD is still the
> pin's parent, `git --no-replace-objects -c core.useReplaceRefs=false restore --source=<pin> --staged --worktree -- .`
> from the checkout's root; on a later HEAD, `git --no-replace-objects -c core.useReplaceRefs=false cherry-pick
> --no-commit <pin>`. `git update-ref -d <pin>` removes the pin, and every later resume then stops naming it

"Interrupted" is gone from its first words. Since §2.3 it names pins of attempts the log settled long before.

**At the Git level, on the three Gits** (`fud2/git-level/TABLE.txt`, from `restore_replacements.py`).
- **The repository.** It is built as the legacy engine leaves it after a refusal and a resume:
  - a root commit, then the captured parent `B`;
  - the captured tree `T`: an edit, a new file and a deletion;
  - the pin `P = commit-tree T -p B`;
  - the resume's discard, under the engine's controls.
- **The shapes.** Five, each created after the pin:

  | Shape | What it replaces |
  |---|---|
  | none | nothing |
  | blob | the new file's blob, by one with other bytes |
  | tree | `T`, by `B`'s tree |
  | commit | `P`, by `B` |
  | graft | `git replace --graft P <root>` |

- **What each run checks,** with the controls, against what `P` records:
  - **at the parent:** every path of `P`'s tree holds that blob's bytes, no index path lies outside it, and the index
    tree equals `T`;
  - **on a later HEAD:** exactly `A new.txt`, `D deleted.txt` and `M tracked.txt` are staged against HEAD, both
    files hold `P`'s bytes, and the later commit's file stays.

| Shape | At the parent: round 1's command | At the parent: round 2's | On a later HEAD: round 1's | On a later HEAD: round 2's |
|---|---|---|---|---|
| none | restored | restored | restored | restored |
| blob | **wrong:** the file holds the replacement's bytes, and the index is the pin's | restored | **wrong**, the same way | restored |
| tree | **wrong:** the base restored, the paid files absent | restored | **wrong:** nothing applied (rc 0) | restored |
| commit | **wrong**, as for tree | restored | **wrong**, as for tree | restored |
| graft | restored, since a restore reads a tree | restored | **wrong:** the root as the base, a conflict (rc 1) | restored |

The table holds identically on Git 2.43.0, 2.50.1 and 2.55.0.

**Through the real legacy engine** (`fud2/probe/witness-runs/TABLE.txt`, three rounds each).
- **The worker** makes an edit, a new file and a deletion, then the fake's own file.
- **The run:** a static tear refuses the snapshot, the operator repairs it, and the resume runs. Then the witness
  follows the warning's command as an operator would, from the checkout's root.
- **What each compares:** **working-file content** and the index against the pin's recorded tree, as the triage asks.

| Witness | `base` | `r1` | `d2` | `d2-m-plain` |
|---|---|---|---|---|
| w10: blob replacement, at the parent (the design lens's case) | red | red on 2.43, 2.50.1 and 2.55.0 | **ok** on all three | red on all three |
| w11: tree replacement, at the parent (the regression lens's case) | red | red on all three | **ok** on all three | red on all three |
| w12: tree replacement, on a later HEAD (the resume committed the task again) | red | red on all three | **ok** on all three | red on all three |
| w12g: `replace --graft` of the pin, on a later HEAD | red | red on all three | **ok** on all three | red on all three |

`base` is red because master keeps nothing. `base` and `d2-m-interrupted` ran on 2.43 only, and
`d2-m-interrupted` is ok on all four.

### 2.3 FUD-D1-PINWARN: every surviving pin, on every resume

**The defect, executed by two lenses.** Round 1's resume looked for kept pins only among the attempts still in flight
(`interrupted_attempts()`, `src/events/mod.rs:1032-1043`). Its own `AttemptInterrupted` clears that attempt's
`in_flight` (`:811`).
- So a resume that settles the attempt and then fails before its report loses the warning, as every legacy warning is
  lost (R-D5). Every later resume then never looks for that pin again, although the pin survives.
- **The concurrency lens's sequence:** a second refusal on the next resume. The successful resume named only the
  second pin.
- **The regression lens's sequence:** a resume whose worker cannot spawn. The successful resume named no pin.

**The change, inside `src/engine/resume.rs`, with no change to `src/events/mod.rs`.** The resume looks for a kept pin
at every attempt the replayed log records.
- **Where the attempts come from.**
  - For each task index `i`, the log records attempts 1 to `progress[i].attempts`.
  - `progress.attempts` is set only by `AttemptStarted`, to that event's attempt number (`src/events/mod.rs:793`).
  - The coordinator hands out attempt numbers as that plus one (`src/engine/coordinator.rs:482`), so they are
    contiguous and never reused.
- **What it asks.** For each such attempt, `prepared_pin_target(<prepared_pin_ref(run, i, attempt)>-kept)`, the call
  round 1 made per interrupted attempt. It collects every pin that exists.
- **What it reads.** `RunState.progress` and `Progress.attempts` are public fields of the replayed state
  (`src/events/mod.rs:709-719`, `:725`). Nothing in the event log changes.
- **Every name the coordinator can write is among them.** It writes a kept pin only at
  `prepared_pin_ref(run, index, attempt)` plus `-kept`, for the attempt it is running. That attempt's `AttemptStarted`
  is already in the log.
- **The rest is round 1's:** the orphan removal over the interrupted attempts, the discard, and the warning (§2.2).

**Why not a ref listing.** `git for-each-ref refs/upstroke/prepared/<run>/` would take one Git process. But it is a
Git child `src/workspace.rs` does not run today, so it would widen that module's text to a second purpose. The log
already names every attempt that could hold a kept pin.

**What it costs.** Three Git processes per recorded attempt per resume: `check-ref-format`, `symbolic-ref` and
`rev-parse` (`prepared_pin_target`, `src/workspace.rs:1122-1143`). Round 1 paid them per attempt still in flight.
Measured over the legacy engine tests (`engine::tests`, skipping D's witnesses; 179 run): round 1 made 7 kept-pin
lookups and round 2 makes 61. That is 162 more Git processes, on 22,513, or 0.7 % (`fud2/probe/pinwarn-cost/COUNT.txt`).

**How an operator retires a pin, and what the warning then does.**
- The operator runs `git update-ref -d <pin>`, as the warning says.
- The next resume's lookup finds no ref at that name, so the warning stops naming it, and names only the pins that
  remain.
- A resume that finds none adds no warning.

**What the warning covers.** The pins of the run being resumed.
- A run that is never resumed again has its kept pins named by the refusals that wrote them, and by nothing later.
- `git for-each-ref 'refs/upstroke/prepared/*/*-kept'` lists every kept pin in the repository.

**R-D5, restated.** A resume that fails still returns its error without its warnings, as every legacy warning does
(`src/engine/coordinator.rs:1140-1151`). The loss is no longer permanent: the next resume that reports names every pin
again.

**Executed through the real legacy engine** (three rounds each, Git 2.43; `fud2/probe/witness-runs/TABLE.txt`).

| Witness | `r1` | `d2` | `d2-m-interrupted` | `d2-m-plain` |
|---|---|---|---|---|
| w13: an earlier pin, after a second refusal on the next resume (the concurrency lens's sequence) | **red** | **ok** | **red** | ok |
| w14: the pin, after a resume that failed (the regression lens's sequence) | **red** | **ok** | **red** | ok |
| w15: a retired pin is no longer named (`git update-ref -d`, then a resume) | ok | **ok** | ok | ok |

- **w14 also asserts the premise.** After the failed resume, attempt 1 is no longer among the attempts in flight
  (`RESULTS.txt`: `in_flight_after_failure=[2]`).
- **w13's second tear** is planted through round 5's probe seam, so w13 runs only on seamed trees. Its planned test
  T-P13 needs no seam (§2.8).
- **w15 is a contract check, not a discriminator:** round 1 names no pin after a failed resume, so it passes there
  too.
- `base` is red on w14 and w15, because master keeps no pin.

### 2.4 FUD-D1-PROGRESS: the claim, qualified

**§1.6 said** "A write that finishes in that time is passed". The concurrency lens executed the counterexample:
- the foreign writer repaired the registry at 480.9 ms;
- the backoff sleep used up what remained of the 500 ms test deadline;
- the helper refused at 500.1 ms, after six attempts, with no attempt after the repair.

The output was kept. The run did not go on.

**Whether the access makes one final attempt at the deadline** was #329's §5.5 question, carried to B
(`~/orch-pr11/briefs/followups/fu-b-impl-carryover.md`, added 2026-10-02T10:36Z), and D did not decide it. Round 7
decided it while this round ran. An attempt that follows a backoff sleep the deadline cut short is made, and it is the
last (§2.6, change 3).

**So it now reads:**

> A registration another process is writing is passed when an attempt the access starts after the write finished
> succeeds. The access attempts again, after a backoff of 1 ms doubling to 50 ms, until its deadline of 10 s, and makes
> one final attempt at the deadline after a sleep the deadline cut short. A write that finishes before the access's
> last attempt starts is therefore passed. A write that finishes during an attempt that has already read the store and
> ends after the deadline is not: that attempt is the last, and the command ends with a registry refusal, resumably,
> with the output kept, never as Git state.

- **The same wording** applies to §1.8's (e1) and (e2) rows: "a write that finishes within the deadline is attempted
  past" becomes "a write that finishes before the access's last attempt starts is attempted past".
- **Round 7 executed the final attempt.** A static torn entry was repaired 490 ms into a 500 ms test access. Round 6's
  loop refused at 500.1 to 500.2 ms. Round 7's made a 16th attempt at the deadline and returned `Ok` at 501.6 to
  502.9 ms, on all three versions (#329's record §6.4, `d7/witness/FIGURES.txt`).

### 2.5 R-G: every legacy Git child runs with Git's automatic maintenance off

**What is carried, and from where.** `FUC-D2-RG` is a **P1** in #330's round-2 triage
(`~/orch-pr11/reviews/review-330-d2-triage.md`).
- **The finding.** Git's default prune expiry protects no registration in the interval that matters. A prune deletes
  a registration with no `gitdir` at once (`should_prune_worktree`, `worktree.c:734-735` at 2.43.0). So a Git
  maintenance prune that the legacy engine starts can delete a registration another checkout is writing.
- **The orchestrator's triage** puts the legacy change in D's scope.
- **Follow-up C's round 3** (#330 at `a0464f43`, its record §3.4) states it as D's requirement. `git_command`
  (`src/workspace.rs:43-51`), the one builder of every legacy Git child, adds
  `-c maintenance.auto=false -c gc.auto=0 -c gc.autoDetach=false -c maintenance.autoDetach=false`.
- **D carries exactly that,** from one new constant, `AUTO_MAINTENANCE_REFUSED`, passed after `REPLACE_REFS_REFUSED`
  (`fud2/probe/patch-d2.py`).

**Every legacy Git invocation, at `5c222ff2`.**
- **`git_command` is the only builder.** `src/workspace.rs`'s own census holds that every Git child of the module is
  built by `git_command` and nowhere else: exactly one `Command::new(` in the production region, inside the builder
  (`every_git_child_of_this_module_is_built_where_replacements_are_refused`, `:3681`, the counts at `:3745-3751`).
- **Its 14 call sites:** `:163`, `:188`, `:231`, `:289`, `:380`, `:419`, `:460`, `:879`, `:911`, `:1039`, `:1096`,
  `:1131`, `:1557` and `:1607`. Every one is in `src/workspace.rs`, inside D's unfreeze.
- **No other legacy module starts Git.** The other production `Command::new("git")` in `src/` are
  `src/workspace_manager.rs`'s two (`:4997`, `:5453`), which are C's. Every remaining one is in a test region:
  `src/gates.rs`, `src/status.rs` and `src/runner/container/view.rs`, each after its first `#[cfg(test)]`.
- **What the engine actually runs,** traced through the real legacy engine
  (`fud2/probe/witness-runs/RG.txt`, rg1): `add`, `cat-file`, `check-attr`, `check-ref-format`, `clean`, `commit-tree`,
  `config`, `diff`, `ls-files`, `ls-tree`, `reset`, `rev-parse`, `status`, `switch`, `symbolic-ref`, `update-ref`,
  `version`, `worktree` and `write-tree`.

**Which of them can start automatic maintenance.**
- **Five builtins call `run_auto_maintenance`,** at each of 2.43.0, 2.50.1 and 2.55.0: `am`, `commit`, `fetch`,
  `merge` and `rebase`. For example, `builtin/commit.c:1871` and `builtin/fetch.c:2493` at 2.43.0
  (`fud2/git-src-citations.txt`).
- **The legacy engine runs none of them in production.**
  - Its only `git commit` is `Workspace::commit` (`src/workspace.rs:1019-1023`), whose callers are all in the module's
    tests (`:2036`, `:2468`, `:2531`, `:2687`, `:2894`, `:3346`, `:3384`).
  - The legacy coordinator publishes through `commit-tree` and `update-ref`: `prepare_commit_from_candidate`
    (`:946-1017`) and `advance_prepared_commit` (`:1171-1228`), called at `src/engine/coordinator.rs:668` and `:738`
    and `src/engine/resume.rs:504`.
- **So the one path from a legacy child to automatic maintenance is a lazy fetch.**
  - In a partial clone, a child that must read an absent object starts a promisor `fetch`
    (`promisor-remote.c:31` at 2.43.0, `:46` at 2.55.0).
  - That fetch passes nothing that stops its own `run_auto_maintenance` (`builtin/fetch.c:2476-2493` at 2.43.0).
- **At 2.55.0, the default prunes worktrees.** Unscheduled maintenance uses the `geometric` strategy by default
  (`builtin/gc.c:1970-1975` at 2.55.0), and that strategy includes the `worktree-prune` task (`:1916`).
  - Its auto condition is met by one prunable registration (`:391-420`), and it runs
    `git worktree prune --expire 3.months.ago` (`:379-389`).
  - No configuration is needed. #330's record says a repository "configured with the `geometric` maintenance
    strategy" (§3.4); at 2.55.0 that is the default.
- **At 2.43.0 and 2.50.1** maintenance runs the `gc` task. Its `gc --auto` prunes worktrees only once `need_to_gc`'s
  loose-object or pack thresholds are met (`builtin/gc.c:380`, `:612` at 2.43.0).

**Two of #330's premises, corrected for the orchestrator.** #330's §3.4 says a legacy command starts the prune
through `Workspace::commit`, and needs a configured strategy at 2.55.0.
- `Workspace::commit` has no production caller.
- At 2.55.0 the `geometric` strategy is the default, not a configuration.

**C's requirement is unchanged by either.** It closes the only path there is.

**Executed through the real legacy engine** (`fud2/probe/witness-runs/RG.txt` and `TABLE.txt`; three rounds; Git's
defaults; Git 2.43.0, 2.50.1 and 2.55.0).
- **The census.** Git's own trace2 stream, one file per Git process: the engine's children (each carrying
  `core.useReplaceRefs=false`, which only `git_command` passes) and every process they started.
- **The run in each:** a worker that edits a tracked file, adds one and deletes one; a static tear; the repair; and the
  resume.

| Witness | `base` | `r1` | `d2` | `d2-m-maint` |
|---|---|---|---|---|
| rg1: full clone | no fetch, no maintenance | the same | the same | the same |
| rg3: blob-less partial clone, ordinary checkout | no fetch, no maintenance | the same | the same | the same |
| rg2: blob-less partial clone whose checkout lacks HEAD's blobs (constructed); a registration planted with no `locked` and no `gitdir` | **red**: the engine's `git diff` fetches, and the fetch starts `maintenance run --auto` (2.43, attached; 2.50.1 and 2.55.0, `--detach`). At 2.55.0 it runs `git worktree prune --expire 3.months.ago`, which **deletes the planted registration** | **red**, the same | **ok**: the same fetch, no maintenance, and the registration survives | **red**, as `base` |
| l7 (T-L7): through the builder, `git config --get` of each of the four keys, with the repository configuring the opposite | **red** | — | **ok**: `false`, `0`, `false`, `false` | **red** |

- **rg1 and rg3 hold the premise check.** In 72 of 72 runs no legacy child fetched, and none started maintenance.
- **rg3's precondition holds:** its clone lacked `missing_before=1` object of the history.
- **rg2's precondition holds:** `missing_before=5`, behind a clean checkout.
- **Why rg2's checkout is built that way.** `git clone --filter=blob:none --no-checkout`, then the files copied in,
  `read-tree HEAD` and `update-index --refresh`.
  - The capture's `git add -A` writes back the blob of every file still in the checkout. It did in an earlier variant
    of the witness, which fetched nothing.
  - So the fetch comes from the capture's `git diff`, reading the parent's blobs of the edited and the deleted file.

**R-G's severity, G6 position and closure, from this evidence.**
- **The mechanism is executed:**
  - a legacy Git child starts automatic maintenance;
  - at 2.55.0, under Git's defaults, that maintenance prunes a registration in the state an add in flight is in before
    its first write.
  - That is #330's R-G2 interval, and the consequence #330's lenses gave it (`FUC-D2-RG`): a topology registration
    deleted under a slot in use.
- **The starter is narrower than #330 graded.** It needs a partial clone in which a legacy child must read an object
  the clone lacks.
  - A full clone and an ordinary blob-less clone never get there (rg1, rg3).
  - I produced the state only by building the checkout without fetching (rg2), and found no normal flow that leaves
    it.
- **Severity: P2 on this evidence.** MAINTAINING reclassifies down "a P1 whose failure needs speculative
  preconditions … with a ledger row saying why". The triage's P1 stands until the owner reclassifies. The ledger row
  says so.
- **G6:** applicable. Q1 is reached, because a topology registration is deleted under a slot in use, and #330's lenses
  add ST-18 and INV-22 for its cleanup and accounting.
- **Blocks G6:** as the triage's P1, yes until D lands, or the owner excludes it. As a P2, no.
- **Closed by D's change whichever.** The witnesses show it on the three Gits (rg2 and l7, with `d2-m-maint` red).

**What D's change does not reach.** Maintenance, or a prune, that a process the engine does not build starts.
- **A role's own Git** in the managed repository: a worker's `git log -p` lazily fetching in a partial clone, or a
  gate's `git commit`. That is `PR11-HOST-AGENT-PRUNE-RACES-AN-ENGINE-ADD`'s class (P2, guarded by PR12) and #330's
  R-GU.
  - Its P2 rationale is topology width. A legacy run's agent beside a topology run in another checkout does not need
    width; that is for the orchestrator.
  - The v0.1 runner could give roles the same four settings through the `GIT_CONFIG_PARAMETERS` entries it already
    appends. That is `src/runner/host.rs`, outside D's five texts, and a behaviour change for every role. It is not
    proposed.
- **The user's own Git:** #330's R-GU.

**On Windows** `daemonize()` is `ENOSYS`, so maintenance runs attached (#330's §2.2). The settings stop it from
starting all the same: `prepare_auto_maintenance` reads them before it builds the command (`run-command.c:1820` at
2.50.1).

**Tests already pinning the builder** keep passing: the census reads `.args(REPLACE_REFS_REFUSED)` and the
environment, and counts `Command::new(` (`:3745-3760`). The suites below include every `workspace::tests` and
`engine::tests` test.

### 2.6 §1.2 conformed to #329's §5.5, as round 7 dates it

**Round 7 published before this round finished.** #329 pushed its design round 7 at `85f5b09b` at
2026-10-02T11:44Z (`fud2/sibling-heads.log`). Its §5.5 carries a dated change, "Changed 2026-10-02, design round 7",
with §6.4 as the current contract and §6.3 as the add veto (#329's record at `85f5b09b`, saved as
`fud2/b-record-85f5b09b.md`). So the triage's instruction applies: §1.2 is conformed to it here. The orchestrator
still compares the two before D is implemented.

**The six dated changes, and what D does with each.**

| # | Round 7's dated change (§5.5 note; §6.4 governs) | D, conformed |
|---|---|---|
| 1 | `again` returns `Again { Attempt, Return, Undecidable { why } }`, not `bool` | D's switch and removal pass `\|\| Again::Attempt`. D's add passes the veto of change 6 |
| 2 | `Undecidable` refuses at once as `RegistryRefused`, naming `why`, with no further attempt | a veto D cannot decide is a registry refusal, so B-PRESERVE keeps and pins the captured candidate (it keys on the variant, §1.4). Executed: v2 |
| 3 | the final attempt: an attempt that follows a sleep the deadline cut short is made, and it is the last | §2.4's claim, conformed |
| 4 | the bound is end to end: the deadline, plus the last attempt's runtime, plus the veto's after it | D's times (§2.11): a refused add returns by the deadline plus its last attempt plus its veto, which may be one probe (a checkout as long as the add's own), then its pending snapshot's cleanup takes the same again |
| 5 | `CONTENDED_ATTEMPTS` and `contended_attempts(common_git_dir)` are part of the contract, for tests only, keyed by the common git dir as passed | D's handshake (§1.2 item 8) is provided. D's tests read it with `canonical_common_dir`'s spelling |
| 6 | an add's veto no longer reads "taken over" as "its own failure", nor "untouched" as "not taken over"; D's legacy add veto at `37e4d8c4` "has both of round 6's holes … and adopts §6.3" | D's add adopts §6.3's rule, through `src/workspace.rs`'s own builder (§2.7). Its probe places the index in a Git directory of the destination's own rather than through `GIT_INDEX_FILE`, so `src/workspace.rs` gains no `.env(` and no builder call site, and two censuses stand unchanged. This closes FUD-D2-PRUNE for a prune before the add returns, and FUB-D6-INODE |

**What does not change for D** (§6.4, "For D's legacy add"): the helper's name and module, its `effect_free` row,
`RegistryHold`, the canonical `common_git_dir`, nothing sampled, and `RegistryRefused` as the variant B-PRESERVE keys
on. §5.5's sentence describing D's predicate is corrected by round 7's note 6.

**§1.2, as it now reads.** Items 1 to 8 stand, with these changes:
- **Item 1:** `again: &mut dyn FnMut() -> Again`, and `Again` beside `RegistryHold`.
- **Item 2:** a failed attempt's veto answers `Attempt`, `Return` or `Undecidable`. D's add answers by §2.7; the
  switch and the removal answer `Attempt`.
- **Item 3:** `Undecidable` is a fourth way the access returns `RegistryRefused`.
- **Item 4:** the deadline governs as §6.4 says: one final attempt at it, and an end-to-end bound that includes the
  veto.
- **Item 8:** satisfied by §6.4's `CONTENDED_ATTEMPTS`.

**One difference between round 7's claim and its steps, for the orchestrator.** §6.4 says the final attempt means "a
store a writer leaves whole by the deadline is passed". Its step 6 refuses without a final attempt when an attempt
that started before the deadline fails after it. So a writer that finishes during that last attempt, after it read the
store, is not passed. D's claim (§2.4) is stated in the steps' terms.

### 2.7 D's add veto, adopted from round 7, and FUD-D2-PRUNE

**What FUD-D2-PRUNE was** (found while this round re-checked §1.3 against #329's round-6 review). #329's FUB-D6-PRUNE
(P1) is a concurrent `git worktree prune` deleting an add's registration after Git took the destination over. D's add
had the same veto as round 6's topology add, so the same sequence:
1. D's add runs `git worktree add`. Git makes its entry, and a prune elsewhere reads it before `locked` exists and
   decides "gitdir file does not exist" (`worktree.c:734-735` at 2.43.0).
2. Git writes `locked` and takes the destination over. The prune then deletes the entry by name, deciding and
   deleting in one loop iteration (#330's record §3.4, `builtin/worktree.c:203-217`).
3. Git's next write into the entry fails, and `remove_junk` deletes the destination.
4. Rounds 1 and 2's veto saw the destination gone and returned Git state.
5. `run_attempt` returned it (`src/engine/attempt.rs:154`), with no refused candidate recorded.
6. The coordinator discarded the worker's paid output (`src/engine/coordinator.rs:544-548`).

Where the topology add's case ends in a durable park, the legacy one was a discard.

**The veto D adopts:** #329's round 7, its record §6.3, on the legacy add. The access runs in `add_gate_worktree`
(§1.3 item 2), and `PendingGateWorkspace` has made the destination empty before the first attempt, as now. After a
failed attempt the veto reads the destination (`symlink_metadata`, and `read_dir` when it is a directory):
1. **An empty directory the access can remove is untouched.**
   - Git's junk removal after a takeover ends in that same `rmdir` (#329's §6.3: `builtin/worktree.c:258-273` at
     2.43.0, `:273-288` at 2.50.1 and 2.55.0). So a destination still there, empty and removable, was never taken
     over.
   - The access removes it, makes it again (the private directory `PendingGateWorkspace` made), and answers `Attempt`.
     No probe runs.
2. **Absent, or an empty directory the access cannot remove: the probe decides.**
   - **The probe** is the add's checkout without the registry. The destination gets a Git directory of its own,
     `<destination>/.git/`, holding only `commondir` (the canonical common git dir's bytes) and `HEAD` (the commit).
     Then `git --git-dir=<destination>/.git --work-tree=<destination> read-tree -u --reset --no-recurse-submodules
     <commit>` runs through the module's existing `git_output`, and so through `git_command`.
   - **What Git does with it.** It keeps the probe's index in that directory, and reads objects, refs and configuration
     through `commondir`, as it does for any linked checkout's Git directory. Git refuses `.git` as a tree path, so
     nothing checked out collides with it.
   - **If the checkout cannot be made,** the veto answers `Return`, and the attempt's own error comes back as Git
     state. So does a destination, `.git` directory or file the probe cannot make: Git's add must write them too.
   - **If it can,** the access empties the destination, makes it again, and answers `Attempt`.
3. **Anything else is `Undecidable`,** and the access refuses at once (§2.6, change 2): a link, a file, a non-empty
   directory, metadata it cannot read, or a destination it cannot make again.

**Why D's probe is not round 7's letter, and is its rule.**
- **Round 7's probe** names the repository with `--git-dir=<common git dir>` and its index with
  `GIT_INDEX_FILE=<destination>/.git/index`. Built in `src/workspace.rs`, that is one more `.env(` and one more builder
  call site.
- **Two existing censuses pin both.** This round's first prototype had round 7's form, and its two whole suites each
  failed both censuses (`fud2/probe/suite-d3-nowit/superseded-envform/`):
  - `runner::contract::tests::every_production_command_spec_payload_is_classified` counts `src/workspace.rs`'s `.env(`
    at five, in `src/runner/contract.rs`, outside D's five texts;
  - `src/workspace.rs`'s own `every_git_child_of_this_module_is_built_where_replacements_are_refused` names every
    builder call site in source order, and is an existing test D's texts keep unchanged.
- **D's form moves neither.** It runs through the existing `git_output`, as `canonical_common_dir` runs through
  `git_path` (§1.3 item 4). Its index needs no variable, because a Git directory with `commondir` keeps its own index.
- **The same checkout, executed** (`fud2/git-level/probe-gitdir-<version>.log`, `probe_gitdir.py`; Git 2.43.0, 2.50.1
  and 2.55.0, all agreeing):
  - **under strace it touches no path under `<common>/worktrees`,** run from the main checkout or a linked one;
  - **it answers rc 0** with the store intact, with a foreign entry whose `commondir` is empty (where
    `git worktree list` exits 128), with the linked base's own entry torn, and with no store at all;
  - **it fails where `git worktree add` fails, and succeeds where it succeeds:** a valid commit (both rc 0), a
    300-byte name and a `.git` path component (both rc 128).

**Why it holds for the legacy add as for the topology add.**
- **The same Git sequences.** Round 7 executed the rule's cases at the Git level on 2.43.0, 2.50.1 and 2.55.0, with its
  probe's reads traced (#329's `d7/witness/`, its record §6.2 and §6.3). D's probe form is traced above.
- **D's probe runs through `git_command`,** so replacement objects are refused and automatic maintenance is off (§2.5).
  `read-tree` runs no hook.
- **The legacy engine refuses filtered paths first.** It refuses a candidate tree whose paths carry a clean or smudge
  filter before any snapshot (`tree_input_problem`, `src/workspace.rs:813-815`), so the probe runs no configured filter.
- **The probe's fidelity** (round 7's R12) holds here too. It runs in the common git dir's checkout's configuration,
  where the add's checkout runs in the new worktree's. A failure the probe does not reproduce is attempted again and
  refuses at the deadline: kept, never Git state (R-D13).

**Executed through the real legacy engine, on #329's round-7 contract prototyped for the probe**
(`fud2/probe/patch-d3.py`; variants `d3`, `d3-m-r6veto` and `d3-m-noprobe`; `fud2/probe/witness-runs/TABLE.txt`).
- **Round 7 built no prototype.** Its helper's three changes are prototyped here only so that D's adoption runs:
  `Again`, the refusal on `Undecidable`, and the final attempt. The manager's own accesses pass `Again::Attempt`, and
  its add keeps round 6's rule mapped onto `Again`. Nothing here tests the topology add.
- **v1 and v2 use a probe-only git wrapper** first on `PATH` (`fud2/probe/wrap/git`). It reproduces once the end state a
  failed snapshot add leaves, then hands every command to the real Git:
  - **v1, `prune`:** the real add succeeds, then its registration and its destination are removed and it exits 128.
    That is a prune after the takeover, with Git's junk removal done.
  - **v2, `nonempty`:** the add exits 128, leaving a file in the destination. That is round 7's `undecidable` row.
- **T-L8** calls the veto directly on five destinations.

| Witness (three rounds; `base-v` and `d2-v` six, the set having run twice; Git 2.43.0, 2.50.1 and 2.55.0) | `base-v` | `d2-v` (rounds 1 and 2's veto) | `d3` | `d3-m-r6veto` | `d3-m-noprobe` |
|---|---|---|---|---|---|
| v1: the registration pruned after the takeover | red: Git, the output discarded | red, the same | **ok**: the probe answers `Attempt`, and the run commits, with no pin | red, as `d2-v` | ok (v1 does not discriminate it) |
| v2: a file in the destination after the failure | red: Git, discarded | red | **ok**: `RegistryRefused` at once (150 to 162 ms), the candidate pinned, the checkout holding the output | red | ok |
| l8 (T-L8): five destinations, the veto called directly | — | — | **ok**: `Attempt`, `Attempt`, `Return`, `Undecidable`, `Attempt` | red: absent, stray and torn all `Return` | red: the genuine failure answers `Attempt` |

- **Sources:** `fud2/probe/witness-runs/TABLE.txt` and `VERDICTS.txt`; the wrapper's line for each run in `RESULTS.txt`;
  T-L8's lines in `L8-RESULTS.txt`.
- **#329 round 5's four workspace witnesses on `d3`:**
  - the transient tear: `Ok` after 2 attempts, with the destination empty after the first failure, so no probe;
  - the static tear: `RegistryRefused` at 1,006 to 1,007 ms, 16 add attempts with the final one;
  - the checkout that cannot be made: Git after 1 attempt, in 10 ms with its probe;
  - the removal: 2 attempts.
- **Every other witness of §1.9 and §2.2 to §2.5 is green on `d3`.** That is three rounds each: round 1's fifteen,
  w10 to w15, and rg1 to rg3 and l7 on the three Gits.

**What remains of FUD-D2-PRUNE: a prune that lands after the add returned** (R-D9, narrowed).
- **The sequence.** A prune decides on the add's entry in the same window and deletes it after the access returned
  `Ok`. That is round 7's R13, which #329 states as no access's to classify: "a deletion that lands after it returned".
- **For the topology add** the next Git command in that checkout meets a checkout with no registration.
- **For the legacy snapshot** that next command is `verify_gate_worktree`'s `git status` (`src/workspace.rs:908-944`),
  which runs outside the access. It fails as Git state, so the coordinator discards.
- **Severity:** P1 class by consequence, as FUB-D6-PRUNE. The pruner is external (the user's prune or maintenance, an
  IDE's, an agent's Git), and the decision must fall between the add's `mkdir` and its `locked`.
- **G6:** a legacy snapshot is outside G6's topology claims, as (e1) and (e1′) are, and a topology run's agent as the
  pruner is `PR11-HOST-AGENT-PRUNE-RACES-AN-ENGINE-ADD`'s class (P2, guarded by PR12). So it does not block G6.
- **Two closures are known, neither proposed here:**
  - **(a) The verification inside the add's attempt.** Under §2.7's veto, a pruned registration then makes the attempt
    fail with a populated destination. That is `Undecidable`, so it is refused and kept. A genuine verification failure
    would be kept too, where today it is Git state and a discard.
  - **(b) Keep on any snapshot failure after capture.** Round 1's `m-keepall` becomes the design.
  - Either widens the `src/workspace.rs` text, or the `attempt.rs` and `coordinator.rs` texts, beyond round 7's rule.

**FUB-D6-INODE** is closed by the removal proof. A takeover whose junk removal left the destination's directory, on
any platform, either is removable, so untouched and attempted again, or is not, so the probe decides. On Windows the
rule needs no file identity (#329's R9′): a delete-pending destination that cannot be made again is `Undecidable`, a
refusal that keeps.

### 2.8 Tests: what round 2 adds to the plan

§1.9's planned tests, witnesses and mutations stand. Round 2 adds the following. Each was prototyped as the witness in
brackets.

**Appended to `src/engine/tests.rs`:**
- **T-P10 (FUD-D1-REPLACE, at the parent):** a blob replacement on the worker's new file. Following the warning's
  restore leaves every path of the pin's recorded tree in the checkout with that blob's bytes, no other index path,
  and the index's tree equal to the pin's. [w10]
- **T-P11:** the same under a tree replacement of the captured tree by its parent's. [w11]
- **T-P12, T-P12g (on a later HEAD):** the resumed run commits the task again. T-P12 carries a tree replacement of the
  captured tree, made at capture; T-P12g a `git replace --graft` of the pin onto the root commit, made after the
  resumed run's commit. Following the warning's command for a later HEAD stages exactly `A`, `D` and `M`, and the two
  files hold the pin's recorded bytes. [w12, w12g]
- **T-P13 (FUD-D1-PINWARN):** a refusal keeps attempt 1. After the repair, the resume's own attempt 2 is refused too
  and kept. After the second repair, the successful resume names both pins. [w13]
  - The witness plants the second tear through round 5's probe seam.
  - The test needs no seam: the resumed attempt's test worker plants it, as any worker writes in the checkout, before
    the capture.
- **T-P14:** a refusal, then a resume whose worker cannot spawn, then a successful resume, which names the pin.
  Attempt 1 is no longer in flight after the failed resume. [w14]
- **T-P15:** the same with `git update-ref -d <pin>` between the two resumes. The successful resume names no pin.
  [w15]

**In `src/workspace.rs`'s inline test module:**
- **T-L7 (R-G):** a repository configures `maintenance.auto=true`, `gc.auto=6700`, `gc.autoDetach=true` and
  `maintenance.autoDetach=true`. Through the module's builder, `git config --get` of each key reads `false`, `0`,
  `false` and `false`. It executes Git, so it holds on every Git at or above the 2.41 floor. [l7]
- **T-L8 (the adopted veto, §2.7):** called directly on five destinations. An empty one is `Attempt` and is empty
  again. An absent one with a valid commit is `Attempt`, made empty again after the probe. An absent one whose commit
  has a 300-byte name is `Return`. One holding a file is `Undecidable`. An absent one beside a foreign registration
  `git worktree list` dies on is `Attempt`. [l8]
- **T-L1 to T-L3, revised for round 7's contract:**
  - T-L1, a transient tear: `Ok` after 2 attempts, and no probe (the destination is empty and removable).
  - T-L2, a static tear: `RegistryRefused` after the final attempt, 16 attempts under the 500 ms test deadline.
  - T-L3, a checkout that cannot be made: Git state after 1 attempt and 1 probe.
- **Design evidence, not planned tests:**
  - **rg1 to rg3** need a trace2 directory in the environment of the engine's Git children, which a test can only give
    them process-wide.
  - **v1 and v2** need a git wrapper first on `PATH`, process-wide too. Their decisions are T-L8's, and T-L1 to T-L3
    take the real add through them.

**The planned mutations, added:**

| Mutation | Undoes | Red (executed, three rounds, `fud2/probe/witness-runs/TABLE.txt`) |
|---|---|---|
| `d2-m-plain`: round 1's plain commands in the warning | FUD-D1-REPLACE | w10, w11, w12, w12g, on 2.43, 2.50.1 and 2.55.0 |
| `d2-m-interrupted`: the lookup over the attempts still in flight only | FUD-D1-PINWARN | w13, w14 |
| `d2-m-maint`: the builder without the four settings | R-G | rg2 and l7, on the three Gits |
| `d3-m-r6veto`: D's add veto as rounds 1 and 2 had it (an empty directory, else `Return`), on round 7's contract | §2.7's adoption | v1, v2 and l8, on the three Gits |
| `d3-m-noprobe`: the probe skipped (an absent or unremovable destination answers `Attempt`) | §2.7's probe | l8 (a checkout that cannot be made answers `Attempt`), on the three Gits |

**Round 1's witnesses on round 2** (§1.9's fifteen: w1 to w9, w3b, w4b and #329 round 5's four workspace witnesses)
are all green on `d2`, three rounds each, as on `r1`. Round 2 changes nothing they hold. They are green on `d3` too, the
conformed prototype (§2.7).

**The suites** (`fud2/probe/suite-d2-nowit/`; D rounds 1 and 2 on #329 rounds 4 and 6, with no witness; every source
touched first, its own Compiling line):

| Run | Result |
|---|---|
| clippy `-D warnings`, all targets | rc 0 (`clippy-1.log`) |
| suite 1 | 3,026 passed, 2 failed, 126 ignored, in 106.93 s (`suite-1.log`) |
| suite 2 | 3,026 passed, 2 failed, 126 ignored, in 118.93 s (`suite-2.log`) |

- The two failures are the non-frozen manager tests every #329 round since 4 has moved, #329's to move:
  `a_removal_records_the_one_attempt_the_unix_arm_makes` and
  `an_add_killed_before_it_wrote_gitdir_is_unlisted_and_refuses_forced_cleanup`.
- Every `engine::tests` and `workspace::tests` test passed in each run (188 and 47 `ok` lines, counted as §1.9 counted
  them), and so did every effects census.

**The suites on the conformed prototype** (`fud2/probe/suite-d3-nowit/`: the same, with `patch-d3.py`):

| Run | Result |
|---|---|
| clippy `-D warnings`, all targets | rc 0 (`clippy-1.log`) |
| suite 1 | 3,025 passed, 3 failed, 126 ignored, in 106.19 s (`suite-1.log`) |
| suite 2 | 3,025 passed, 3 failed, 126 ignored, in 126.79 s (`suite-2.log`) |

- **Two failures in each** are #329's two manager tests, as above.
- **The third is a load flake, a different one each time:**
  - suite 1: `real_docker_kill_on_an_already_exited_container_is_tolerated`, "still running after 200 observations",
    which is `PR274-DOCKER-TERMINATION-POLL-COUNTS-YIELDS-NOT-TIME`
    (`findings/P3_correctness_202609121234_the-docker-termination-poll-counts-yields-not-time.md`);
  - suite 2: `a_resume_over_a_creation_that_stopped_after_creating_its_integration_ref_adopts_it`, "holds the run's
    cleanup lease; refusing overlapping engine ownership", which is
    `PR281-CLEANUP-LEASE-HOLD-OUTLIVED-AND-ITS-UNREADABLE-TWIN`
    (`findings/P2_correctness_202609131202_a-cancelled-job-hides-which-assertion-failed.md`).
  - Each passed alone three times of three on the suite's own binary (`suite-d3-nowit/alone/`). Neither test's module
    is one D or the prototype changes, and each failure is its finding's own message.
- **Both censuses that round 7's exact probe broke pass:** the payload census and the Git-child census.
- **Every `engine::tests` and `workspace::tests` test passed** (188 and 47 `ok` lines), and so did every effects
  census (101).

### 2.9 The exact unfreeze texts, all five, restated in full

These are amendments to `effects/allowlist.toml`. Each entry's `path`, `allows`, `packet` and `shrinks_when` stay as
they are, and so does `FROZEN_LEGACY_ALLOWLIST` (`src/effects.rs:1306`). **Two texts change in round 2, marked
CHANGED:** `src/workspace.rs` (R-G) and `src/engine/resume.rs` (FUD-D1-PINWARN and FUD-D1-REPLACE). The other three
are round 1's, word for word. All five stay PROPOSED until reviewed.

**`src/workspace.rs` (`:898-924`) — CHANGED in round 2.** "AMENDED ONCE" becomes "AMENDED TWICE". The last sentence,
"The schema-4 equivalents live behind funnels in `crate::workspace_manager` and nothing here calls them: the constant
is read, and no funnel is called.", is replaced by:

> The second amendment, to close `PR329-LEGACY-RUNS-IN-LINKED-CHECKOUTS-RACE-THE-SHARED-WORKTREE-REGISTRY` on the
> owner's decision to unfreeze the module for this one change, is two things and no more. First, the three Git
> children that enumerate the repository's worktree registry — `switch_branch`'s `git switch`, `add_gate_worktree`'s
> `git worktree add`, and `cleanup_gate_workspace`'s `git worktree remove` together with the `git worktree list` that
> decides whether it took the registration — each run as the attempt of
> `crate::workspace_manager::tolerant_registry_access`, which attempts one again until its deadline and then refuses
> as a registry refusal (`UpstrokeError::RegistryRefused`), never as Git state. The add holds the registry lock
> shared. After a failed add, an empty destination it can remove is removed, made again and attempted again; an
> absent destination, or an empty one it cannot remove, is decided by a checkout of the add's commit into it that
> reads no registration — `read-tree` through the module's existing builder, in a Git directory of the destination's
> own that names the repository's common git dir — and the add's failure is returned as Git state only when that
> checkout cannot be made either; anything else at the destination refuses at once as a registry refusal. The other
> two are attempted again whatever failed.
> One private helper resolves the canonical common git dir as `recorded_objects_scope` does, through `git_command`,
> and private functions make the destination again and probe it. Second, `git_command`, the one builder every Git
> child of the module starts from, also passes
> `-c maintenance.auto=false -c gc.auto=0 -c gc.autoDetach=false -c maintenance.autoDetach=false` from one new
> constant, so no Git child of the module, and no Git process one of them starts, runs Git's automatic maintenance,
> and none of them prunes a registration another checkout is writing. The test module gains the regression tests for
> both. Every other behaviour of the module stays frozen. The schema-4 equivalents live behind funnels in
> `crate::workspace_manager`, and nothing here calls a funnel: the constant is read, and the tolerant access is
> called, which takes no site.

**`src/engine/attempt.rs` (`:869-877`) — unchanged from round 1.** One paragraph is appended to `legacy_effect`:

> AMENDED ONCE, on the owner's decision to close the static and deadline residue of
> `PR329-LEGACY-RUNS-IN-LINKED-CHECKOUTS-RACE-THE-SHARED-WORKTREE-REGISTRY`: `run_attempt` takes one more argument, a
> slot in which it records the candidate it captured — the branch ref, parent and tree `capture_candidate` returned —
> when the gate or the review snapshot's worktree-registry access refuses (`UpstrokeError::RegistryRefused`). The two
> snapshot calls record it through one private function that compares the error's variant and copies the three
> strings, and the type that holds them is new; the error `run_attempt` returns, and every other step, are as before.
> It calls nothing of the workspace, the runner or the event log that it did not already call, and nothing else in the
> module moves.

**`src/engine/coordinator.rs` (`:834-853`) — unchanged from round 1.** One paragraph is appended to `legacy_effect`:

> AMENDED ONCE, on the owner's decision to close the static and deadline residue of
> `PR329-LEGACY-RUNS-IN-LINKED-CHECKOUTS-RACE-THE-SHARED-WORKTREE-REGISTRY`: when `run_attempt` fails after recording a
> refused candidate, the coordinator does not discard the checkout; it pins that candidate — the branch ref, parent and
> tree captured before the refusal, never the index as it stands at the refusal — through
> `Workspace::prepare_commit_from_candidate` at the attempt's `prepared_pin_ref` followed by `KEPT_PIN_SUFFIX`, one new
> constant, and returns a registry refusal (`UpstrokeError::RegistryRefused`) that names the pin, or the pin's failure.
> Every other attempt error discards the checkout as before. The new arm calls only `prepared_pin_ref` and
> `Workspace::prepare_commit_from_candidate`, both of which the module already calls, and nothing else in the module
> moves.

**`src/engine/resume.rs` (`:855-867`) — CHANGED in round 2.** One paragraph is appended to `legacy_effect`:

> AMENDED ONCE, on the owner's decision to close the same finding's static and deadline residue: for every attempt the
> replayed log records — each task's attempts from the first to the last one started — the resume also asks, through
> `Workspace::prepared_pin_target`, whether the coordinator kept that attempt's candidate at its `prepared_pin_ref`
> followed by `KEPT_PIN_SUFFIX`, a pin no resume removes; it discards the checkout's uncommitted paths exactly as
> before, so the attempt runs again from a clean tree, and one warning names every kept pin it found with the commands
> that take its output back as the repository records it, deletions included, each carrying the replacement controls
> the legacy workspace's Git children carry (`--no-replace-objects -c core.useReplaceRefs=false`), and says that
> removing a pin stops the warning naming it. Of the workspace it calls only `prepared_pin_target`, which it already
> calls; of the event log it reads the replayed state and changes nothing; and nothing else in the module moves.

**`src/engine/tests.rs` (`:1138-1150`) — unchanged from round 1.** One paragraph is appended to `legacy_effect`:

> AMENDED ONCE, on the owner's decision to close
> `PR329-LEGACY-RUNS-IN-LINKED-CHECKOUTS-RACE-THE-SHARED-WORKTREE-REGISTRY`: the file gains appended regression tests,
> and no existing test changes. They drive the same entry points through a worktree registry another process has torn.
> One of them makes that registration as a topology slot, through `crate::workspace_manager`'s funnels and its test
> fixture's torn-registration shape, under the same allow.

**How each changed clause matches its implementation** (round 1's table, §1.5, stands for every other clause):

| Text | Clause | The implementation that makes it true |
|---|---|---|
| `workspace.rs` | "two things and no more" | the first is §1.3's three call sites and helper, with §2.7's veto in place of `OwnedDestination`; the second is one constant and one `.args(…)` in `git_command` (`fud2/probe/patch-d2.py`, `patch-d3.py`) |
| `workspace.rs` | "after a failed add, an empty destination it can remove is removed, made again and attempted again; an absent … or an empty one it cannot remove, is decided by a checkout … returned as Git state only when that checkout cannot be made either; anything else … refuses at once" | `legacy_add_veto`, `remake_destination` and `probe_destination`, answering round 7's `Again` (§2.7); executed: v1, v2, l8 |
| `workspace.rs` | "`read-tree` through the module's existing builder, in a Git directory of the destination's own that names the repository's common git dir" | `probe_destination` writes `<destination>/.git/commondir` and `HEAD` and calls the existing `git_output`, so `git_command`; no `.env(` and no builder call site is added, so `src/runner/contract.rs`'s payload census and this module's Git-child census stand (the suites, §2.8) |
| `workspace.rs` | "`git_command` … also passes … from one new constant" | `AUTO_MAINTENANCE_REFUSED`, passed after `REPLACE_REFS_REFUSED` (`:41`, `:48`) |
| `workspace.rs` | "no Git child of the module, and no Git process one of them starts, runs Git's automatic maintenance" | `-c` values reach every child Git starts through `GIT_CONFIG_PARAMETERS`, and outrank every configuration file (#330's §2.2 version table); `maintenance.auto=false` returns before `maintenance run` starts (`run-command.c:1803-1805` at 2.43.0, `:1820` at 2.50.1, `:1961-1967` at 2.55.0); executed: rg2 and l7 |
| `workspace.rs` | "the test module gains the regression tests for both" | T-L1 to T-L5 (§1.9), T-L7 and T-L8 (§2.8) |
| `resume.rs` | "for every attempt the replayed log records … asks … `prepared_pin_target`" | the loop over `replayed.state.progress` and `1..=progress.attempts` (§2.3) |
| `resume.rs` | "each carrying the replacement controls … and says that removing a pin stops the warning" | the warning of §2.2 |
| `resume.rs` | "of the event log it reads the replayed state and changes nothing" | `RunState.progress` and `Progress.attempts` read; `src/events/mod.rs` untouched (its blob is master's) |
| `resume.rs` | "of the workspace it calls only `prepared_pin_target`, which it already calls" | `:553`, and round 1's per-attempt call, now per recorded attempt |

### 2.10 `design/15`: the PROPOSED paragraph, amended

The paragraph §1.7 added, "A legacy attempt the worktree registry refused", stays PROPOSED and in the same place.
Round 2 changes four things in it, and adds no sentence elsewhere:
- **Its source** now reads "§1, as §2 amends it".
- **Its add-veto sentence** becomes round 7's rule: "After a failed snapshot add, an empty destination the engine can
  remove was never taken over, and the add is attempted again; otherwise a checkout of the add's commit into the
  destination that reads no registration decides: the failure is the add's own, and comes back as Git state, only when
  that checkout cannot be made either, and anything else at the destination refuses as a registry refusal."
- **Its resume sentence** becomes: "Every resume discards the checkout's copy as before, so the attempt runs again
  from a clean tree. It looks for a kept pin at every attempt the run's log records, and names each one it finds with
  the commands that take its output back as the repository records it, deletions included: with replacement objects
  refused, as every legacy Git command refuses them. It never removes a kept pin; the operator does, and a removed pin
  is named no more."
- **One sentence is added:** "Every legacy Git command also runs with Git's automatic maintenance off, so none of them
  starts a `git maintenance` or `git gc --auto` that could prune a registration another checkout is writing."

No sentence `src/export.rs` pins moves, and the hunk stays apart from #329's PROPOSED paragraph.

### 2.11 What legacy users see, and what remains, after round 2

**§1.6, amended:**
- **The progress claim** is §2.4's.
- **The resume's warning** is §2.2's.
  - Its two commands refuse replacement objects, so they restore what the pin records.
  - Every resume names every kept pin of the run, not only the pins of the attempts it settles.
- **Maintenance.** No command the legacy engine runs starts Git's automatic maintenance any more.
  - In a full clone it never did (rg1).
  - In a partial clone, a lazy fetch one of its commands starts no longer runs maintenance (rg2).
  - The user's own Git commands are unchanged.
- **A snapshot add after a failure.** An empty, removable destination is made again and the add attempted again, with
  no probe. Otherwise one checkout without the registry decides. A genuine checkout failure costs one add and one
  probe, and comes back as Git state, as at master.
- **Resume time.** A resume asks three Git processes per attempt its log records. Across the existing legacy engine
  tests that adds 162 Git processes to 22,513 (`fud2/probe/pinwarn-cost/COUNT.txt`).

**§1.8's residuals, amended and extended:**

| | What | Consequence | Where |
|---|---|---|---|
| R-D1, R-D2 | as §1.8 | as §1.8 | §1.4 |
| R-D3 (amended) | D's own Git child killed by a signal mid-add | under round 7's veto: a destination left non-empty is `Undecidable`, refused at once and kept; one left empty and removable is attempted again, and meets the killed add's entry, so it refuses at the deadline: kept. Never Git state | §2.7 |
| R-D4 (replaced) | a takeover whose junk removal left the destination's directory, on any platform; on Windows, a delete-pending destination | the removal proof or the probe decides; a destination that cannot be made again is `Undecidable`: a refusal that keeps (#329's R9′) | §2.7 |
| R-D5 (narrowed) | a resumed run that fails | it returns its error without the resume's warnings; the next resume that reports names every pin again | §2.3 |
| R-D6 to R-D8 | as §1.8 | as §1.8 | §1.6, §1.2 |
| R-D9 (new, narrowed) | what remains of FUD-D2-PRUNE: an external prune that decided in the add's window deletes the registration after the add returned `Ok` (round 7's R13) | `verify_gate_worktree`'s `git status` fails as Git state, outside the access, so the coordinator discards; two closures known, neither proposed (§2.7) | §2.7 |
| R-D10 (new) | R-G beyond the engine: maintenance or a prune a role's own Git or the user's starts | not reached by D; `PR11-HOST-AGENT-PRUNE-RACES-AN-ENGINE-ADD` (P2) for agents, #330's R-GU for the user | §2.5 |
| R-D11 (new) | the deprecated `info/grafts` file | not refused by the recovery commands, as by no legacy command; it can change only a cherry-pick's base, through an entry an operator wrote for the pin itself | §2.2 |
| R-D12 (new) | a write that finishes during the access's last attempt, after that attempt read the store, when the attempt ends after the deadline | refused, resumably, with the output kept (round 7's step 6 makes no final attempt after it) | §2.4 |
| R-D13 (new) | the probe's fidelity (#329's R12): it checks out in the common git dir's checkout's configuration, and the add in the new worktree's | a checkout failure the probe does not reproduce is attempted again and refuses at the deadline: kept, never Git state | §2.7 |

### 2.12 The G6 classification, round 2

| Case | What D closes, given #329's helper | Severity | Applies to G6 | Blocks G6 |
|---|---|---|---|---|
| (e1), (e1′), (e2), (e2′) | as §1.8: unchanged by round 2, which makes the recovery exact (§2.2) and the warning durable (§2.3) | P1 each | as §1.8: (e2) and (e2′) yes, (e1) and (e1′) no | as §1.8: (e2) and (e2′) yes until #329 and D are implemented and validated, (e2′) unless the owner rules otherwise |
| R-G (`FUC-D2-RG`) | closed by the builder's four settings (§2.5) | the triage's P1; **P2 on this round's evidence**, the owner to reclassify | yes: Q1; ST-18 and INV-22 per #330's lenses | as a P1, yes until D lands or the owner excludes it; as a P2, no |
| FUD-D2-PRUNE | closed for a prune before the add returns, by the adopted round-7 veto (§2.7); R-D9 remains for a deletion after it returned | P1 class by consequence, as FUB-D6-PRUNE | no: a legacy snapshot is outside G6's claims; a topology run's agent as the pruner is the P2 host-agent class | no |

Every lens of round 1 kept (e2) and (e2′) applicable and blocking until #329 and D are implemented and validated, and
(e1) and (e1′) not applicable. Round 2 changes neither.

### 2.13 What §2 replaces in §1

| §1 says | Replaced by |
|---|---|
| §1.2 items 1 to 4 and 8: the contract as #329's round 6 published it (`again` a `bool`; no final attempt; a bound without the veto; the handshake requested) | §2.6: the contract as round 7 dates it |
| §1.3 item 2: the veto "owned, unchanged, empty", its "Why the veto is exact", and the item-5 type `OwnedDestination` | §2.7: round 7's removal proof and registry-free checkout probe, with private functions in place of the type |
| §1.3, the four reasons for leaving out "named by no registration" | moot under §2.7's veto, which reads no registration either |
| §1.4, `resume.rs`: "for each interrupted attempt … collects each kept pin that exists" | §2.3: every attempt the replayed log records |
| §1.4, the warning's text | §2.2 |
| §1.4, residual "A resume whose own run later fails … The pin was named by the refusal that wrote it, and it stays." | §2.3, R-D5 narrowed: the next resume that reports names it again |
| §1.5, the `src/workspace.rs` and `src/engine/resume.rs` texts | §2.9 |
| §1.6, "A write that finishes in that time is passed" | §2.4 |
| §1.6, "one warning names each kept pin, with `git restore …` and `git cherry-pick --no-commit <pin>`" | §2.2 and §2.3 |
| §1.8, (e1) and (e2): "a write that finishes within the deadline is attempted past" | §2.4: "a write followed by a successful attempt before the deadline" |
| §1.8, R-D3, R-D4 and R-D5 | §2.11 |
| §1.9, the planned tests and mutations | §2.8 adds to them |
| §1.10, `effects/allowlist.toml`: five texts | five texts still, two of them changed (§2.9) |
| §1.11, the time budget | §2.14 adds to it |

### 2.14 The Windows and macOS time budget, round 2's additions

Round 2's planned tests, costed by round 1's method (`fud2/wintime/BUDGET-r2.txt`, from `budget-r2.py`):
- each witness's Linux wall time is the median of `d2`'s three rounds;
- that time is split into its fixed waits (500 ms per refused access under test) and its Git work;
- the Git work is scaled by round 1's multipliers: 19.8× on the guest, 26.3× hosted and 7.9× on macOS.

| Leg | Round 2 adds | Over | Harness wall | With round 1's |
|---|---|---|---|---|
| `test (winguest)` | about 55 s | 12 threads | about +5 s | about +13 s on 485.6 s, a 20-minute job |
| hosted `windows-latest`, queue only | about 71 s | 4 threads | about +18 s | about +48 s on 1,631 s, a 45-minute job |
| macOS | about 27 s | 3 threads | about +9 s | about +23 s on 1,105 s, a 30-minute job |

- **The longest added test** is T-P13: about 11 s on the guest and 14 s hosted.
- **T-L8** runs one probe and a few directory operations, about 12 to 13 ms here (`TABLE.txt`), so well under a
  second on every leg. T-L2 gains one attempt (the final one).
- **The resume lookup's own cost** in the existing legacy tests: 162 more Git processes than round 1 across the legacy
  engine tests, 0.7 % of their 22,513 (`fud2/probe/pinwarn-cost/COUNT.txt`), so about that fraction of their Git time on
  each leg.
- **None of this is executed on those legs.** It is an estimate from measured multipliers, and CI is the truth for
  them.
- **Hosted headroom.** On #328's 39-minute hosted figure, the triage left D about 5.5 minutes with round 1's +30 s,
  before B's and C's costs. Round 2's additions take about 18 s more of it.

### 2.15 The findings round 2 answers

| Finding | Sev | Kind | D | Where | Evidence |
|---|---|---|---|---|---|
| FUD-D1-REPLACE | P2 | executed (design and regression lenses) | **Fixed (design), witnessed.** Both recovery commands carry `--no-replace-objects -c core.useReplaceRefs=false` | §2.2 | `fud2/git-level/TABLE.txt` on three Gits; w10, w11, w12, w12g red on `r1` and `d2-m-plain`, green on `d2`, on three Gits |
| FUD-D1-PINWARN | P2 | executed (concurrency and regression lenses), reasoned (design lens, P3) | **Fixed (design), witnessed.** The resume looks for a kept pin at every attempt the replayed log records, with no change to `src/events/mod.rs`. Retiring a pin stops the warning | §2.3 | w13 and w14 red on `r1` and `d2-m-interrupted`, green on `d2`; w15 green |
| FUD-D1-PROGRESS | P3 | executed (concurrency lens) | **Fixed (design).** The claim is stated in the access's terms: a write that finishes before its last attempt starts is passed, and round 7's final attempt at the deadline is that attempt | §2.4 | the lens's 480.9 ms and 500.1 ms; round 7's `final-attempt` |
| R-G (`FUC-D2-RG`) | P1 (triage); P2 on this evidence | executed here | **Fixed (design), witnessed.** `git_command` refuses automatic maintenance, as #330's §3.4 requires. #330's premise of a legacy `git commit` is corrected | §2.5 | rg2 and l7 red on `base`, `r1` and `d2-m-maint`, green on `d2`, on three Gits; rg1 and rg3 green everywhere |
| FUD-D2-PRUNE | P1 class | reasoned, found here; executed as its end state | **Fixed (design), witnessed, for a prune before the add returns:** D's add adopts round 7's veto. A deletion after the add returned stays R-D9 | §2.6, §2.7 | v1, v2 and l8 red on the round-6 veto, green on `d3`, on three Gits |
| §1.2 against #329's §5.5 | — | conformance | **Conformed** to round 7's dated §5.5 (`85f5b09b`) | §2.6 | the six dated changes, each adopted |
