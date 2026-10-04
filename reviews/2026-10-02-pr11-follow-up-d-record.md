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
`fud2/…`. Round 3 (§3) is `pr11_fud_design3`'s; its figures are under `~/orch-pr11/logs/pr11_fud_design3/`, cited as
`fud3/…`. Round 4 (§4) is `pr11_fud_design4`'s; it runs no witness, and its own files are under
`~/orch-pr11/logs/pr11_fud_design4/`, cited as `fud4/…`. The implementation (§5) is `pr11_fud_impl`'s; its figures are
under `~/orch-pr11/logs/pr11_fud_impl/`, cited as `fudi/…`. Its repair round 2 (§5.16) is `pr11_fud_impl2`'s; its
figures are under `~/orch-pr11/logs/pr11_fud_impl2/`, cited as `fudi2/…`. Its implementation round 3 (§5.17), which
implements R-D1's preservation design, is `pr11_fud_impl3`'s; its figures are under `~/orch-pr11/logs/pr11_fud_impl3/`,
cited as `fudi3/…`. Its repair round 4 (§5.18) is `pr11_fud_impl4`'s; its figures are under
`~/orch-pr11/logs/pr11_fud_impl4/`, cited as `fudi4/…`. Its repair round 5 (§5.19) is `pr11_fud_impl5`'s; its figures
are under `~/orch-pr11/logs/pr11_fud_impl5/`, cited as `fudi5/…`.

## 0. Status

| Phase | State |
|---|---|
| Design (§1) | **PROPOSED, pending the owner's decision B** and design review. Corrected B1′ (§1.3) and B-PRESERVE (§1.4) carry #329's design review round 5 findings against them: FUB-D5-INDEX, FUB-D5-RESTORE and FUB-D5-UNFREEZETEXT. They are designed against #329's round-6 helper contract (§1.2). Each was executed through the real legacy engine on scratch prototypes, with the mutations that turn them red (§1.9). Design review round 1 (`37e4d8c4`): three lenses, CHANGES_REQUIRED, no P1. Round 2 (§2) amends §1 where it is marked. |
| Design, round 2 (§2) | **PROPOSED, superseded by §3 where §3 says** (the probe, R-D9's G6 row, T-L2, T-L3 and T-L8, the `src/workspace.rs` text's add sentence). It answers round 1's review: the recovery commands refuse replacement objects (FUD-D1-REPLACE), every resume names every kept pin of the run (FUD-D1-PINWARN), and the progress claim is qualified (FUD-D1-PROGRESS). It carries R-G: every legacy Git child runs with Git's automatic maintenance off, as #330's round 3 requires. #329's round 7 published while this round ran (`85f5b09b`), and §1.2 is conformed to its dated §5.5 (§2.6). D's add adopts round 7's veto, which closes FUD-D2-PRUNE for a prune before the add returns (§2.7). Two of the five exact unfreeze texts change (§2.9). Each change was executed through the real legacy engine on Git 2.43.0, 2.50.1 and 2.55.0, with a mutation that turns it red (§2.8). This head changes no code: it carries this record's §2 and the amended PROPOSED paragraph in `design/15`. |
| Design, round 3 (§3) | **PROPOSED, superseded by §4 where §4 says** (R-D9's boundary, C-SIDE's description and cost, the empty-destination arm's description, `design/15`'s add-veto sentence). It answers round 2's review (`b13b4857`: three lenses, CHANGES_REQUIRED, two P1s, both inherited). The probe and D's private-gitdir variant are withdrawn: after a failed add, an empty destination the access can remove is attempted again, and anything else refuses at once and keeps the output, never Git state (FUD-D2-PROBECONFIG, FUD-D2-HOOKS). R-D9's G6 classification is corrected: the mixed case applies and blocks G6, as part of #329's external-prune finding. A consequence-side closure for it is evaluated, executed on a prototype and given its exact text, and not adopted. T-L2's oracle is split (FUD-D2-TL2). R-G stays P1 for the owner. §1.2 is conformed to #329's round 8 (`f7a9256c`). One of the five exact unfreeze texts changes (§3.8). Each change was executed through the real legacy engine on Git 2.43.0, 2.50.1 and 2.55.0, with a mutation that turns it red. This head changes no code. |
| Design, round 4 (§4) | **PROPOSED — the last design round before the owner's consolidated question,** pending the owner's decision B. It answers round 3's review (`ac18321f`: three lenses, CHANGES_REQUIRED; the concurrency lens re-run after a capacity error). It is text only: no machinery, no adopted closure, and the five exact unfreeze texts unchanged. R-D9 is divided by whether the deletion fails the add, so a deletion before a successful add returns is inside it (FUD-D3-DURINGADD). C-SIDE is restated as a partial mitigation with two P1 residuals, its stronger forms are weighed, its cost is stated as three widened `effects/allowlist.toml` texts, and it is aligned with #329's corrected closure 1 at `8df42436` (FUD-D3-CSIDEPROOF). The add veto's empty-destination arm is described by what it observes, in `design/15` too (FUD-D3-TAKEOVERWORD). This head changes no code. |
| Implementation (§5) | **Implemented on this branch as a draft, and PROPOSED — conditional on the owner's decision O8 (decision B), and not granted.** D's reviewed proposal at `9b2262f4`, on #329's head `54a1ff14` merged in (not rebased), with design review round 4's applicable fixes (the PR11 decision appendix's §10.3 and §11 D rows). C-SIDE (O1) and ENV-1's widening (O3) are isolated and not implemented. **Not merge-ready:** it waits on O8's adoption and on #329 merged (O9, O14, O11). Every new test is red on its first-bad shape and killed by a mutation (§5.6, §5.7). The implementation is not yet reviewed. |
| Implementation, repair round 2 (§5.16) | **The early review's E1 to E4 repaired**, on the same terms: draft, PROPOSED, conditional on O8, not merge-ready. Two lenses read `20e27724` before CI was green on every leg, so their review is early evidence and clears nothing for the merge. E1 corrects the moved-`HEAD` safety claim: the resume's branch check guards the checkout only while `HEAD` differs from the head the event log records, and the executed loss is R-D1's, which stays open at P1 for a preserving mechanism or the owner. E2 narrows R-D7 to refusals that recur after a successful reclaim. E3 rewrites T-L5 so that no Git child it observes runs inside the access's deadline. E4 recounts the Windows path budget with `ScratchTree::acquire`'s prefix. CI at `20e27724` failed only on #329's own test, routed to #329. |
| Implementation, round 3 (§5.17) | **R-D1's preservation design, round 4, implemented as a draft, and PROPOSED — conditional on O8, exactly as D's own texts.** The proposal `~/orch-pr11/owner-package/RD1-PRESERVATION-PROPOSAL.md` (sha256 `f9e81c07…5415`; its regression lens PASSED, its required regular verdict UNMET, its routing held): part K (a kept pin written whatever `HEAD` is), part G4 (the resume's guarded exact discard of the attempt in flight, behind a kept pin and a copy that same resume made durable) and part N (an attempt error after the worker ran, and a publication failure, keep the checkout and pin it), with the design's three instrument rows. B's published head `55029628` is merged **provisionally**, not as B's final head. N1 and N2 are filed. Not merge-ready, unreviewed, and R-D1's preservation is the owner's through O8. |
| Implementation, repair round 4 (§5.18) | **The hosted-Linux failure of T-R10 at `202c0805` repaired, on the same terms: draft, PROPOSED, conditional on O8, and not merge-ready.** CI run 37202558686 failed one test on the runner's Git 2.55.0. T-R10's operator `fetch --prune` ran Git's automatic maintenance, whose `geometric` default (from Git 2.54) can prune the planted torn registration before the operator's repair, in a race. It is a test defect: F2's fetch now runs with automatic maintenance off, and nothing it asserts changes. The cause is executed on a Git 2.55.0 stand-in, red before, green after, and red again under a mutation; round 3's other 69 tests do not depend on it. No production code, finding, instrument, cost or limit changes; #329's `55029628` stays merged provisionally. Unreviewed. |
| Implementation, repair round 5 (§5.19) | **D2's D-I2-1 repaired, on the same terms: draft, PROPOSED, conditional on O8, and not merge-ready.** D2, one regression lens at `fb1717b1`, executed that the orphan shared index files an inherited `GIT_TEST_SPLIT_INDEX` leaves outlast the checkout's next split write. The record, the notes, the R-D7 finding and the body now state the true reach: Git's expiry removes an orphan only at a later split write of the checkout's own index that creates a shared index file, once the orphan is older than the checkout's `splitIndex.sharedIndexExpire`; with the default it stays at least two weeks, with `never` for good, and captures can accumulate them. An `include_str!` test pins the notes, red at `fb1717b1` and green after. No behaviour, production code, instrument or other cost changes; #329's `55029628` stays merged provisionally; R-D1's regular review stays unmet. Unreviewed. |

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
  1,004 ms under test. It leaves its intent, which the next resume's reclaim takes. *(Implementation, §5.9:
  FUD-D4-REFUSALTIME — two 10-second retry budgets, plus each access's last attempt and its veto, which no deadline
  bounds.)*

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
  - When `HEAD` moved, a resume refuses on the moved branch before its discard (`src/engine/resume.rs:533-541`) only
    while `HEAD` differs from the head the run's event log records. A branch moved before the capture and back after
    it, or put back as that refusal advises, passes the check, and the resume discards the checkout with no pin.
    *(Implementation, §5.16: E1 of the early review, executed; this replaces "a resume refuses on the moved branch
    before its discard", which held only while `HEAD` stays moved.)*
  - When the ref store failed, a resume discards as today. One storage fault can do both: a common Git directory that
    cannot be written fails the snapshot's registration and then the pin's write, and a resume after writability
    returns discards the checkout. *(Implementation, §5.9: FUD-D4-PINSAMEFAULT, the PR11 decision appendix §10.3,
    replacing "That takes two independent faults.")*
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
  add's and then its cleanup's. *(Implementation, §5.9: FUD-D4-REFUSALTIME — two 10-second retry budgets, plus each
  access's last attempt and its veto, which no deadline bounds; a switch or a removal refuses after its 10-second
  budget plus its last attempt's runtime.)*
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
2. **A legacy attempt that a registry refusal stops after capture loses nothing, conditional on the pin's write.** Its
   captured candidate stays in the checkout and is pinned. The resume names the pin with commands that take it back,
   deletions included, and never removes it. The exception is the pin-write failure of §1.4, which one storage fault
   can cause. *(Implementation, §5.9: FUD-D4-PINSAMEFAULT.)* *(Implementation round 3, §5.17: R-D1's preservation
   design writes the kept pin whatever `HEAD` is and has the resume remove the checkout's copy only once a kept pin and a
   copy of it the resume made durable both hold it, writing the pin at the resume when the refusal's could not be; claim
   2 no longer depends on the refusal's pin, within the covered cases §5.17 gives, conditional on O8.)*
3. **Every other legacy error path is as at master.** *(Implementation round 3, §5.17: no longer. R-D1's part N keeps
   the checkout and pins what it holds on every attempt error after the worker ran and on every publication failure of a
   reviewed candidate — `PR331-AN-ATTEMPT-ERROR-AFTER-THE-WORKER-RAN-DISCARDS-ITS-OUTPUT` and
   `PR331-A-REVIEWED-CANDIDATE-IS-DISCARDED-WHEN-ITS-PUBLICATION-FAILS`, filed; its new refusals and costs are §5.17's.)*

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
| R-D1 | A pin that cannot be written (`HEAD` moved, or the ref store failed at that moment) | the output is only in the checkout, and the refusal says so; a resume refuses on a moved `HEAD` before its discard only while `HEAD` differs from the head the event log records, and discards with no pin once it does not (Implementation, §5.16: E1, executed); a ref-store fault means the resume discards as today, and one storage fault can cause it (FUD-D4-PINSAMEFAULT; Implementation, §5.9). *(Implementation round 3, §5.17: R-D1's preservation design implemented, conditional on O8: the pin is written whatever `HEAD` is, and the resume pins and copies the checkout before any discard; closed in §5.17's covered cases once merged.)* | §1.4 |
| R-D2 | A crash after capture | the resume discards, as today; pre-existing, not this finding. *(Implementation round 3, §5.17: the resume's guarded discard pins and copies the leftovers first; E1 and E2 executed through the engine.)* | §1.4 |
| R-D3 | D's own Git child killed by a signal between the takeover and its first write | attempted again until the deadline, then refused: kept, never Git state | §1.3 |
| R-D4 | Windows: no file identity in std at MSRV | the veto reads only "an empty directory"; a takeover whose junk removal left the destination empty refuses at the deadline: kept | §1.3 |
| R-D5 | A resumed run that fails | it returns its error without the resume's warnings, as every legacy warning; the pin was named by its own refusal, and stays | §1.4 |
| R-D6 | A genuine switch or removal failure | refuses after the deadline instead of failing at once; nothing is discarded on either path | §1.6 |
| R-D7 | Kept pins | accumulate until the operator removes them | §1.6 |
| R-D8 | #329's helper | D's (e1) and (e2) closure depends on it converging and landing as contracted | §1.2 |

**The finding, after D lands:** its file is deleted, and (e1), (e1′), (e2) and (e2′) close. Without decision B, see
§1.12. *(Implementation, §5.12: the file is kept, narrowed, instead. Claim 2 holds conditional on the pin's write
(FUD-D4-PINSAMEFAULT), so R-D1 remains inside the finding's consequence, and the rule for the file is that it goes only
if this record says D wholly closes it.)*

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
  - D's implementation deletes the file either way. *(Implementation, §5.12: kept and narrowed to R-D1 instead, for
    the reason §1.8's note gives; #329 merged first in effect, by the merge `255f67b8`, and its text was taken.)*
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

> **Implementation (§5.9):** the implemented warning carries the PR11 decision appendix §10.3's two corrections:
> FUD-D4-RESTOREBYTES (the index exactly, the working files through the checkout's own end-of-line and filter
> conversions, so compare their bytes) and FUD-D4-CHERRYPICKMERGE (check what the pick staged before removing the pin,
> because a configured merge driver can make it succeed having applied none of the kept change). §5.3 gives it exactly.

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

- **What each run checks,** with the controls, against what `P` records, in a repository with no end-of-line
  conversion or smudge filter for those paths *(Implementation, §5.9: FUD-D4-RESTOREBYTES)*:
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

> **Round 3:** §3.3 conforms §1.2 to #329's §5.5 as its round 8 dates it (`f7a9256c`). Change 6 below no longer holds:
> the probe is withdrawn, and D's add veto is the removal proof alone.

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

> **Round 3:** the probe and D's private-gitdir variant are withdrawn (FUD-D2-PROBECONFIG, FUD-D2-HOOKS). After a
> failed add, an empty destination the access can remove is attempted again, and anything else answers `Undecidable`
> (§3.2). R-D9's G6 classification is corrected (§3.4). What follows is round 2's text, kept as history.

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

> **Round 3:** T-L2, T-L3 and T-L8, and the mutations `d3-m-r6veto` and `d3-m-noprobe`, are replaced (§3.6, §3.7).

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

> **Round 3:** §3.8 restates all five texts again. The `src/workspace.rs` text's add sentence changes; the other four
> are these, word for word.

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

> **Round 3:** its add-veto sentence changes again (§3.9).

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
  is named no more." *(Implementation, §5.9: the sentence now says when a resume reaches its lookup
  (FUD-D4-WARNDELIVERY) and what the restore brings back of the working files (FUD-D4-RESTOREBYTES).)*
- **One sentence is added:** "Every legacy Git command also runs with Git's automatic maintenance off, so none of them
  starts a `git maintenance` or `git gc --auto` that could prune a registration another checkout is writing."

No sentence `src/export.rs` pins moves, and the hunk stays apart from #329's PROPOSED paragraph.

### 2.11 What legacy users see, and what remains, after round 2

> **Round 3:** §3.10 amends the snapshot-add item, R-D3, R-D4 and R-D9, and withdraws R-D13 with the probe.

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

> **Round 3:** §3.11 replaces this table. R-D9 applies to G6 in the mixed case and blocks it.

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

> **Round 3:** round 3 adds no planned test, and T-L3 and T-L8 run no probe. These estimates stand as upper bounds
> (§3.13).

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

> **Round 3:** FUD-D2-PRUNE is closed by `Undecidable`, not the probe (§3.14).

| Finding | Sev | Kind | D | Where | Evidence |
|---|---|---|---|---|---|
| FUD-D1-REPLACE | P2 | executed (design and regression lenses) | **Fixed (design), witnessed.** Both recovery commands carry `--no-replace-objects -c core.useReplaceRefs=false` | §2.2 | `fud2/git-level/TABLE.txt` on three Gits; w10, w11, w12, w12g red on `r1` and `d2-m-plain`, green on `d2`, on three Gits |
| FUD-D1-PINWARN | P2 | executed (concurrency and regression lenses), reasoned (design lens, P3) | **Fixed (design), witnessed.** The resume looks for a kept pin at every attempt the replayed log records, with no change to `src/events/mod.rs`. Retiring a pin stops the warning | §2.3 | w13 and w14 red on `r1` and `d2-m-interrupted`, green on `d2`; w15 green |
| FUD-D1-PROGRESS | P3 | executed (concurrency lens) | **Fixed (design).** The claim is stated in the access's terms: a write that finishes before its last attempt starts is passed, and round 7's final attempt at the deadline is that attempt | §2.4 | the lens's 480.9 ms and 500.1 ms; round 7's `final-attempt` |
| R-G (`FUC-D2-RG`) | P1 (triage); P2 on this evidence | executed here | **Fixed (design), witnessed.** `git_command` refuses automatic maintenance, as #330's §3.4 requires. #330's premise of a legacy `git commit` is corrected | §2.5 | rg2 and l7 red on `base`, `r1` and `d2-m-maint`, green on `d2`, on three Gits; rg1 and rg3 green everywhere |
| FUD-D2-PRUNE | P1 class | reasoned, found here; executed as its end state | **Fixed (design), witnessed, for a prune before the add returns:** D's add adopts round 7's veto. A deletion after the add returned stays R-D9 | §2.6, §2.7 | v1, v2 and l8 red on the round-6 veto, green on `d3`, on three Gits |
| §1.2 against #329's §5.5 | — | conformance | **Conformed** to round 7's dated §5.5 (`85f5b09b`) | §2.6 | the six dated changes, each adopted |

## 3. Round 3 design

> **PROPOSED — pending design review and the owner's decision B** (`~/orch-pr11/ESCALATION.md` item 7). Round 3
> answers design review round 2 on `b13b4857`. §1 and §2 stay the design except where §3 replaces them. Each place it
> does is marked in §2, and §3.12 lists them. §3.8 restates all five exact unfreeze texts in full and marks every
> change. **Every changed or new text is a PROPOSAL until it has been reviewed.** The owner decides B only on reviewed
> exact text.

**Who writes it.** Round 3 is `pr11_fud_design3`'s (`claude-opus-5-5`, `max`), a fresh session the PR11 orchestrator
spawned with the brief `~/orch-pr11/briefs/pr11_fud_design3.md`. Its work list is the triage
`~/orch-pr11/reviews/review-331-d2-triage.md`, items 1 to 5. Its figures are under `~/orch-pr11/logs/pr11_fud_design3/`,
cited as `fud3/…`. Code is cited at master `5c222ff2`, as in round 2; the branch is not rebased.

### 3.1 What round 2's review found, and what round 3 changes

**The review.** Three `gpt-6-astra` lenses at `max` reviewed `b13b4857`: design (as a conformance reading),
concurrency and regression. Each returned **CHANGES_REQUIRED**, with **two P1s**, both inherited rather than D's own.
- The texts are `~/orch-pr11/reviews/review-331-d2-{design,concurrency,regression}-b13b4857.review.md`. Their hashes,
  the lens prompts' and the witnesses' are in `SHA256SUMS-331-d2`; all 63 check (`sha256sum -c`).
- The reviewers' witnesses are in `~/orch-pr11/reviews/331-d2-witnesses/`.
- **What all three accepted, and round 3 keeps as it is:**
  - REPLACE, PINWARN and PROGRESS (§2.2 to §2.4), the regression lens re-running w10 to w12g;
  - §2.6's conformance to #329's dated round-7 contract;
  - the `Undecidable` default for an absent or unremovable destination, "sound for legacy preservation, though not yet
    this head's rule";
  - §2.9's five texts as covering the files;
  - R-G's census and its correction of #330's premises, with the P1 kept pending the owner.

**The work list** (the triage's items 1 to 5).

| Item | Severity | What round 3 does | Where | Evidence |
|---|---|---|---|---|
| FUD-D2-PROBECONFIG | P1 (all three lenses, executed) | The probe and D's private-gitdir variant are withdrawn. A destination gone, or empty and not removable, after a failed add answers `Undecidable`. | §3.2 | v1, v1c, v1n, v2, v3 and l8 through the real legacy engine on three Gits |
| FUD-D2-HOOKS | P2 (all three, executed) | Closed by the same withdrawal: the veto starts no Git child. | §3.2 | the trace2 census in v1: one `read-tree` per run on round 2's design, none on round 3's |
| FUD-D2-RD9 | P1 (all three, executed) | The G6 classification is corrected: the mixed case applies and blocks G6, under #329's external-prune finding. The consequence-side closure is evaluated, executed on a prototype, and given its exact text. It is not adopted. | §3.4 | p1, p1r, p2 and p4 red on round 3, green on the candidate; the controls p3 and p5 green on both |
| FUD-D2-TL2 | P3 (concurrency, executed) | T-L2's oracle is split from the final-attempt rule. | §3.6 | the static tear at Git's speed and with every add 0.6 s slow, on three Gits |
| R-G | P1, pending the owner | Kept at P1. D's P2 argument is recorded as the owner's reclassification question, not resolved. | §3.5 | rg1 to rg3 and l7 re-run on round 3 |
| #329's contract | — | #329's round 8 published at `f7a9256c` while this round ran. §1.2 is conformed to its dated §5.5. | §3.3 | — |

**What changes in the design's text:**
- the `src/workspace.rs` unfreeze text's add sentence (§3.8); the other four texts are unchanged;
- §2.7's veto (§3.2), and §2.6's conformance (§3.3);
- R-D9's G6 row and residual (§3.4, §3.10, §3.11);
- T-L2, T-L3 and T-L8, and the planned mutations (§3.6, §3.7);
- the PROPOSED paragraph in `design/15` (§3.9).

**The probe.** Every witness below ran through the real legacy engine, or the legacy module, on scratch `git archive`
copies of `5c222ff2`. Nothing of it is on the branch (`fud3/probe/setup.sh`).
- **`base3`:** master, with the witnesses.
- **`d3w`:** round 2's conformed prototype `d3` (§2.7: the probe in D's private-gitdir form), with round 3's witnesses.
- **`rd3`:** `d3` with the probe withdrawn (`fud3/probe/patch-rd3.py`): round 3's design.
- **`rd3-m-return`:** `rd3` with a destination gone, or empty and not removable, answering `Return` (#329's option (i)).
- **`rd3cp`:** `rd3` with the candidate consequence-side closure of §3.4, in the form §3.4 gives
  (`patch-rd3.py --closure cside-private`). **It is a prototype for the evaluation, not the design.**
- **`rd3c`:** the candidate's first form (`--closure cside`), which adds a crate-visible method and so fails the effects
  classification census (§3.4). Kept as evidence.
- **`rd3-nowit`, `rd3cp-nowit`, `rd3c-nowit`:** the same without witnesses, for clippy and the whole suite.
- **The git wrapper** (`fud3/probe/wrap/git`), first on `PATH` where a witness needs an end state Git leaves only under a
  race or a fault, then the real Git:
  - `prune`: the first add runs and succeeds, then its registration and destination are removed and it exits 128 (a
    prune after the takeover, with Git's junk removal done; round 2's v1);
  - `always`: while a control file exists, every add does the same (a genuine failure after the takeover that recurs);
  - `nonempty`: the first add leaves a file in its destination and exits 128 (round 2's v2);
  - `postreturn`: the first add succeeds and is left alone; the snapshot's verification then finds its registration
    deleted (R-D9);
  - `slow`: every add sleeps 0.6 s first (FUD-D2-TL2).
- **How each ran:** each test alone, `--exact`, one thread, from its own tree, three rounds, at Git's defaults
  (`GIT_CONFIG_NOSYSTEM=1`, an empty global file), on Git 2.43.0, 2.50.1 and 2.55.0 unless a row says otherwise. Every
  binary has its own Compiling line (`fud3/probe/build-summary.txt`; final hashes in `BINARIES.txt`).
- **The tables:** `fud3/probe/witness-runs/TABLE.txt` (passes per cell), with `VERDICTS.txt`, `RESULTS.txt`, one log per
  run, and the extracted `FIGURES.txt`. Each witness asserts round 3's requirement, and each p-series witness the
  candidate closure's, so a red is a red.

### 3.2 The probe withdrawn: the add's veto is the removal proof alone (FUD-D2-PROBECONFIG, FUD-D2-HOOKS)

> **Round 4:** the rule stands. Its empty-destination arm is described by what it observes, not as proof that Git
> never took the destination over: read "the removal proof" as "the removal predicate" (FUD-D3-TAKEOVERWORD, §4.4).
> Everything here is about an add that fails. A deletion that leaves the add successful never reaches the veto, and is
> R-D9 (FUD-D3-DURINGADD, §4.2).

**What the lenses executed.**
- **PROBECONFIG (P1).** Git's add copies the source worktree's `config.worktree`, and its sparse-checkout patterns, into
  the new worktree. D's probe read only the common configuration. Two shapes made the probe fail where the real add
  succeeds: a worktree-specific attributes file over a shared failing smudge filter, and `core.protectNTFS` true when
  shared and false in the source worktree with `git~1/payload` in the tree. The probe answered `Return`, and the
  coordinator discarded the paid output. R-D13 described only the safe direction of that disagreement.
- **HOOKS (P2).** The probe ran through `git_output` without the add's private `core.hooksPath` and
  `core.fsmonitor=false`, so a configured fsmonitor hook and `post-index-change` ran, outside the destination.
  "`read-tree` runs no hook" (§2.7) was false.

**The rule, as round 3 states it** (#329's round 8 dates the same for both adds, its record §7.2, §7.3 and §7.6).
After a failed add, the veto reads the destination with `symlink_metadata`, and `read_dir` when it is a directory:

| The destination after the failed attempt | The veto's answer | What the access does |
|---|---|---|
| an empty directory the access can remove | `Attempt` | removes it, makes the private directory again, and attempts the add again (the removal proof, kept from round 7) |
| gone | `Undecidable` | refuses at once as `RegistryRefused`; B-PRESERVE pins the captured candidate |
| an empty directory the access cannot remove | `Undecidable` | the same |
| a non-empty directory, anything not a directory, or metadata it cannot read | `Undecidable` | the same |
| a directory it removed and cannot make again | `Undecidable` | the same |

- **No answer is `Return`.** So the add's own error never comes back as Git state, and the legacy coordinator never
  discards because of it.
- **The veto starts no Git child.** It reads no configuration, checks nothing out and runs no hook, so PROBECONFIG and
  HOOKS have nothing left to act on. `src/workspace.rs` gains no `.env(` and no builder call site, as in round 2, and now
  no call through `git_output` either.
- **Why `Undecidable` and not `Return` for a destination that is gone.** Git takes the destination over only after its
  registry phase, and its junk removal after any later failure removes it (§1.3; #329's §5.3). A destination gone
  therefore means the failure came after the takeover, which is either the add's own (a checkout that cannot be made)
  or a prune that deleted the add's registration (FUD-D2-PRUNE). Nothing D may read tells the two apart: round 7's
  probe was the attempt to, and it is withdrawn. `Return` would discard the paid output in the second case;
  `Undecidable` keeps it in both.
- **Why not `Attempt` (#329's (ii′)).** It would attempt a genuine failure's full checkout again until the deadline,
  churning the shared store, and refuse anyway. #329's round 8 measured it (its record §7.3) and chose `Undecidable`;
  D's addendum set the same. One helper keeps one meaning.

**Executed through the real legacy engine** (`fud3/probe/witness-runs/TABLE.txt` and `FIGURES.txt`; nine runs per cell,
three rounds on each of the three Gits).

| Witness | `base3` (master) | `d3w` (round 2) | `rd3` (round 3) | `rd3-m-return` |
|---|---|---|---|---|
| v1: the registration pruned after the takeover (`prune`) | red: Git, the output discarded | red: the probe answers `Attempt`, and the run commits (3 adds) | **ok**: `RegistryRefused` after 1 add, in 168 to 188 ms; the candidate pinned and in the checkout | red: Git, discarded |
| v1c: the same, under the design lens's worktree-specific attributes file over a required failing smudge filter | red: Git, discarded | **red: Git, discarded** (PROBECONFIG, reproduced) | **ok**: refused, kept | red |
| v1n: the same, under `core.protectNTFS` true when shared and false in the main checkout's worktree configuration, with `git~1/payload` | red: Git, discarded | **red: Git, discarded** (PROBECONFIG, reproduced) | **ok**: refused, kept | red |
| v2: a file in the destination after the failure (`nonempty`) | red | ok: refused at once, kept | **ok**: refused at once, kept | ok |
| v3: a failure after the takeover that recurs on every attempt (`always`), then fixed | red: Git on the run and on the resume, no pin; then the resume commits | ok, but only at the deadline: 14 or 15 adds, each followed by a probe, on the run, and as many again on the resume | **ok**: refused after 1 add on the run and on the resume, two pins; the fixed resume commits and names both | red: as `base3` |
| l8 (T-L8): the veto on five destinations, no Git | — | — | **ok**: `Attempt` for the empty one (empty again); `Undecidable` for gone, a file in it, a file in its place, and an empty one under a read-only parent | **red**: gone and unremovable answer `Return` |

- **FUD-D2-HOOKS, executed as a census.** v1 ran with Git's trace2 stream on, one file per Git process. Every `d3w` run
  started exactly one `read-tree` process (the probe), and no `rd3`, `rd3-m-return` or `base3` run started one
  (`VERDICTS.txt`, `read_tree_processes`; the first round's traces are kept as `fud3/probe/traces/*.tar.gz`).
- **#329 round 5's legacy workspace witnesses on `rd3`** (`FIGURES.txt`, Git 2.43.0, three rounds):
  - the transient tear: `Ok` after 2 adds, the destination empty after the first failure, so the removal proof, in 9 to
    10 ms;
  - the static tear: `RegistryRefused` after 16 add attempts, the last the final attempt, at 1,006 to 1,008 ms, the
    intent left for the reclaim;
  - **the checkout that cannot be made** (a 300-byte name): **`RegistryRefused` after 1 add, in 8 to 11 ms**, where
    round 2 returned Git after 1 add and 1 probe;
  - the removal: 2 attempts, the snapshot unregistered and its directory gone.

**The genuine post-takeover failure's cost (triage item 1).**
- **Which failures they are.** #329's §7.3 lists them for the add both engines share: a tree the destination cannot
  hold (G1), a filter that fails (G2), a destination or filesystem that refuses a write (G3), a missing object (G4).
  For the legacy snapshot, G1 needs the snapshot's longer path to cross a platform limit the run's own checkout did
  not, since that checkout already holds the candidate's paths: Windows without `core.longpaths`, macOS's 1,024 bytes,
  Linux's `PATH_MAX`. G2 needs a filter the legacy preflight's `check-attr` did not see, since it refuses filtered paths
  first (`tree_input_problem`, `src/workspace.rs:813-815`). G3 and G4 are the run store's filesystem and a partial
  clone with lazy fetching off.
- **What each costs a legacy user.**
  - **At master:** the add's Git error ends the attempt, the coordinator discards the worker's output
    (`src/engine/coordinator.rs:544-548`), and the command fails. Every resume pays for another worker attempt and
    fails the same way.
  - **Under round 3:** the attempt ends at once as a resumable registry refusal that names Git's message, and the
    captured candidate is kept in the checkout and pinned. Every resume pays for another worker attempt, refuses again,
    and keeps one more pin. Once the cause is fixed (the environment, the path, the filter), the resume completes and
    names every pin.
  - **So the liveness is master's, and the output survives where master destroyed it.** v3 executes exactly this. On
    `rd3` and on `base3` alike the three invocations ran the worker three times (`worker_runs=3`, nine runs each). `rd3`
    kept two pins and named both after the fix; `base3` kept nothing.
  - **The costs that remain:** pins accumulate, one per refused attempt (R-D7); the refusal costs the pending
    snapshot's cleanup, which finds the destination gone and returns at once; and the operator, not the ladder, decides
    what to do with a candidate the snapshot cannot hold.
- **Where it differs from the topology add.** #329 files the same refusal as a P2 liveness cost for a verification
  (`PR329-A-GENUINE-CHECKOUT-FAILURE-AFTER-THE-TAKEOVER-REFUSES`), because master's topology answer, a deferral and then
  a question, made progress. The legacy answer at master is a discard and a failed command, so for legacy the refusal
  loses nothing master kept. D files nothing for it.

**R-D3, R-D4 and R-D13, after the withdrawal.**
- **R-D3** (D's own Git child killed by a signal mid-add): a destination left non-empty, or gone, is `Undecidable`,
  refused at once and kept. One left empty and removable is attempted again; it then meets whatever the killed add left,
  and either completes or is refused at the deadline, kept. Never Git state.
- **R-D4** (Windows, a delete-pending destination): an empty directory the access cannot remove is `Undecidable`, a
  refusal that keeps.
- **R-D13** (the probe's fidelity) is withdrawn with the probe.

**The bound.** #329's §7.6 states the veto's term as a few metadata calls, one `rmdir` and one `mkdir`, for both adds. A
refused legacy add returns by the deadline plus its last attempt's runtime plus that term. A refusal at once (v1) took
168 to 188 ms for the whole legacy run under test.

### 3.3 §1.2 conformed to #329's §5.5, as round 8 dates it

**Round 8 published before this round finished.** #329 pushed its design round 8 at `f7a9256c` at 2026-10-02T13:36Z
(its record and new findings saved as `fud3/sibling/`). Its §5.5 carries "Changed 2026-10-02, design round 8", with §7.6
as the current text and §6.4 standing where §7.6 does not change it. The triage's instruction applies: §1.2 is conformed
to it. The orchestrator still compares the two before D is implemented.

| # | Round 8's dated change (§5.5; §7.6 governs) | D, conformed |
|---|---|---|
| 1 | the registry-free checkout probe is withdrawn; no add veto runs a Git command; D adopts no probe, "including a private gitdir/commondir variant" | §3.2: D's veto starts no Git child; D's private-gitdir variant is withdrawn |
| 2 | the add veto is the removal proof alone: an empty destination the access can remove is `Attempt`; one absent, or empty and not removable, is `Undecidable`, never `Return`; anything else `Undecidable` | §3.2's table, word for word in substance; executed as l8 |
| 3 | the bound's veto term is a few metadata calls, one `rmdir` and one `mkdir`, for both adds | §3.2, "The bound" |
| 4 | the external-prune class is filed as `PR329-AN-EXTERNAL-PRUNE-DELETES-AN-ENGINE-WORKTREES-REGISTRATION` (P1); D's R-D9 refers to it and files no duplicate | §3.4; R-D9 refers to it |

**The clauses D depends on, exactly.** All are round 8's "Unchanged from round 7" list, which §2.6 adopted:
- the helper's signature: `tolerant_registry_access(common_git_dir, hold, again, attempt)`, with `again` returning
  `Again { Attempt, Return, Undecidable { why } }`, its name, its module and its `effect_free` row;
- `Undecidable` refusing at once as `RegistryRefused`, naming `why` (§6.4 step 5);
- the final attempt after a sleep the deadline cut short (§6.4 step 7), and the end-to-end bound;
- `CONTENDED_ATTEMPTS` and `contended_attempts(common_git_dir)`, for tests only, keyed as passed;
- `RegistryHold`, the canonical `common_git_dir`, and nothing sampled;
- the deadline: 10 s in production and 500 ms under `cfg(test)`;
- `RegistryRefused` as the one refusal variant, the one B-PRESERVE keys on.

**What D no longer uses:** `Again::Return`, from its add. The switch and the removal pass `Attempt` as before.

**§2.6's note on round 7's claim stands.** §6.4 says a store a writer leaves whole by the deadline is passed, and its
step 7 refuses when the last attempt that read the store fails after it. D's §2.4 states the steps' terms.

### 3.4 R-D9, the post-return deletion: its legacy consequence, its G6 classification corrected, and a consequence-side closure evaluated

> **Round 4:** R-D9 is divided by whether the deletion fails the add, and a deletion before a successful add returns is
> inside it (FUD-D3-DURINGADD, §4.2, which amends the comparison table below). C-SIDE is a partial mitigation, not a
> closure: "Why it is sound", "What it leaves", the coverage argument, its consistency with #329 and its cost are
> replaced by §4.3 (FUD-D3-CSIDEPROOF), which aligns it with #329's corrected closure 1. C-SIDE's placement, its exact
> texts and its executed results stand.

**What the lenses executed (FUD-D2-RD9, P1).** A prune decides on the snapshot's entry before `locked` exists and is
held. The add completes and the access returns `Ok`. The prune then deletes the entry, and the next Git command in the
snapshot fails, "not a git repository". On all three Gits, with real barriers, this happened, including under
`--expire 3.months.ago`. Through the engine, the snapshot's verification failed as Git state, and the coordinator
discarded. Round 2's §2.7 said this residual does not block G6. The lenses found that true only for a legacy-only
repository.

**Executed through the real legacy engine on round 3** (`rd3`, nine runs per cell; `FIGURES.txt`):

| Witness | `base3` | `rd3` (round 3) |
|---|---|---|
| p1: the gate snapshot's registration deleted after its add returned, before its verification (`postreturn`) | Git ("verifying gate worktree failed: … not a git repository"), the output discarded | the same |
| p1r: the same at the review snapshot (no gates) | the same | the same |
| p2: the gate snapshot's registration deleted while its gate runs; the gate's `git status` then exits 128 | the gate failure is judged: the chain spent, the task parked with a question, the output discarded | the same |
| p4: the review snapshot's registration deleted while the review runs; the review fails | the review failure is judged: parked, discarded | the same |

- **p2 and p4 are later gates and reviewers.** The deletion lands after snapshot construction returned, so neither
  closure (a) nor (b) of §2.7 reaches them, as the concurrency lens said.
- **A valid candidate is not only discarded there: its attempt is settled as a failure,** and the ladder escalates,
  retries or asks a human about work nothing judged.

**The G6 classification, corrected.**
- **Legacy-only** (no topology run shares the repository): P1 by consequence, the discard of paid output. Outside G6's
  topology claims, as (e1) and (e1′) are.
- **Mixed** (a topology run shares the repository, and the pruner is the user, an IDE, a host agent, or Git's
  maintenance in any checkout): **it applies to G6 through Q6 and R17, the shared registry, and it blocks G6.** It is
  part of #329's `PR329-AN-EXTERNAL-PRUNE-DELETES-AN-ENGINE-WORKTREES-REGISTRATION`, whose face 2 names this legacy
  consequence and whose G6 section governs: it blocks G6 until a closure is implemented and validated, or the owner
  rules explicitly on its scope.
- **What does not downgrade it:** the host-agent finding's P2 label and its PR12 guard (a host agent in one checkout can
  overlap another checkout's add at width one), and filing. Filing is no waiver.
- **D files no separate finding** (the triage). Round 2's ledger row `FUD-D2-PRUNE-AFTER-RETURN` becomes the review's
  `FUD-D2-RD9`, which refers to #329's finding.

**Every discard of paid output a snapshot failure can reach** (the legacy engine at `5c222ff2`; the snapshot calls are
`src/engine/attempt.rs:154` and `:178`, the only production callers of the legacy snapshot):

| Discard | What reaches it from a snapshot |
|---|---|
| `src/engine/coordinator.rs:547` (`run_attempt` failed) | the snapshot's construction failing: its add (`src/workspace.rs:836`, now §3.2's veto) or its verification's `git status` (`:837`, `:908-944`); `gates::run_all` failing to run a gate (`src/engine/attempt.rs:159`); a review pass failing to run (`:185-205`) |
| `src/engine/coordinator.rs:774` (a judged failure, unless the ladder retries in the same session) | a gate's failure in the gate snapshot (`src/engine/attempt.rs:159-167`); a review's failure, or its reviewer unavailable, in the review snapshot (`:209`) |
| `src/engine/coordinator.rs:460` (the budget stops the run) | the same judged failure, after an in-session retry (`RetrySameRung { resume: true }`) skipped `:774` |
| `src/engine/coordinator.rs:708`, `:719` (the settlement or its question cannot be written) | the same judged failure, with a second fault in the event log or the question file |
| `src/engine/resume.rs:569` (the resume's discard) | none of its own: it follows a refusal, which B-PRESERVE has pinned, or a crash (R-D2) |

The other discards (`:660`, `:677`, `:739`) follow a successful attempt, in which no snapshot failed.

**Why the re-check cannot sit at those discards.** By the time `run_attempt` returns, both snapshots have been dropped,
and their drop's cleanup (`src/workspace.rs:1673-1685`, `:1419-1433`, `:1549-1600`) has removed their registrations: a
whole registration and a deleted one look the same at the coordinator. So the re-check has to run where the failure is
seen, while its snapshot still exists. That is in `src/workspace.rs` for the verification, and in
`src/engine/attempt.rs` for gates and reviews. Converting the failure there reaches none of the discards above.

**The consequence-side closure (C-SIDE), stated exactly.** It is the legacy form of the first candidate in #329's
finding, "re-check the checkout's registration before any durable negative outcome".
- **The check.** A snapshot's registration is whole when the directory its `.git` file names still holds `gitdir`,
  `commondir`, `HEAD` and `index`. It reads files and starts no Git child. It resolves a relative `gitdir:` line against
  the snapshot (`worktree.useRelativePaths`, from Git 2.48; `fud3/git-level/relative-paths-citation.txt`), and compares
  no path spelling, so Windows' `C:/…` form needs nothing.
- **Where, and what it does.**
  - **The verification** (`src/workspace.rs:837`): when it fails after the add succeeded, and the registration is not
    whole, the snapshot call returns `RegistryRefused` instead of the Git error. `run_attempt`'s existing
    `note_refused` records the candidate, keyed on the variant.
  - **A gate** (`src/engine/attempt.rs:159`): when `gates::run_all` reports a failure or an error, and the gate
    snapshot's registration is not whole, `run_attempt` records the candidate and returns `RegistryRefused`.
  - **A review pass** (`:185-209`): the same, when the pass errs or `review_failure` reports a failure, on the review
    snapshot.
  - **The coordinator is unchanged.** B-PRESERVE's arm pins the recorded candidate and refuses resumably (§1.4).
- **One private reader in each module, and no new crate-visible function.** `src/workspace.rs` reads the pending
  snapshot's registration, and `src/engine/attempt.rs` reads the live snapshot's from its root, through
  `Workspace::root`, which it already calls. The reader is about a dozen lines, written twice.
  - **Why not one shared method.** The first form put it on `GateWorkspace` as a crate-visible method. Its whole suite
    failed `effects::tests::every_externally_reachable_fn_of_a_legacy_or_shared_module_is_classified`: "src/workspace.rs
    unclassified: ["registration_whole"]" (`fud3/probe/suite-rd3c-nowit/suite-1.log`).
  - Classifying it would take an `effects/wrappers.toml` row, which is an instrument. The private form needs none.
- **Why it is sound.** Nothing re-creates a deleted snapshot registration. The engine names each snapshot
  `upstroke-gates-<pid>-<ulid>` and never adds one twice, and `git worktree repair` re-registers nothing for a deleted
  entry (#329's face-2 runs, its finding's Evidence). So a registration missing at the check was deleted before it, and
  preserving is the safe direction even when the failure was genuine and the deletion came after it. A registration
  whole at the check was whole throughout the failed step, so that failure was the snapshot's own, and is judged as
  before.
- **What it leaves.**
  - An entry a pruner, stopped or killed partway through deleting it, left with those four files; or one whose
    deleted `index` a later Git command in the snapshot wrote again.
  - A failure in a snapshot with a whole registration, which is judged as today, whatever caused it.
  - A gate or review that passes in a snapshot whose registration is gone: nothing is discarded, and the candidate
    commits.

**Executed on the prototype `rd3cp`** (nine runs per cell; `TABLE.txt`; the first form `rd3c` gives the same cells):

| Witness | `rd3` (round 3) | `rd3cp` (round 3 with C-SIDE) |
|---|---|---|
| p1: deletion before the gate snapshot's verification | red: Git, discarded | **ok**: `RegistryRefused`, the candidate pinned and in the checkout |
| p1r: the same at the review snapshot | red | **ok** |
| p2: deletion while a gate runs | red: judged, parked, discarded | **ok** |
| p4: deletion while a review runs | red | **ok** |
| p3 (control): a gate that runs Git in a whole snapshot and fails | ok: judged, parked, discarded, no pin | ok: the same |
| p5 (control): a review that fails in a whole snapshot | ok: the same | ok: the same |
| round 1's fifteen, round 2's w10 to w15, and l8 | ok | ok |

- `rd3cp-nowit` also ran clippy and the whole suite (§3.7).
- **`rd3` is C-SIDE's mutation:** the same tree without the check, red on p1, p1r, p2 and p4.

**Weighed against (a) and (b)** (§2.7's two closures; each column as it would be adopted on round 3's veto):

| Case | round 3 | (a) the verification inside the add's attempt | (b) keep on every snapshot failure after capture | C-SIDE |
|---|---|---|---|---|
| deletion during the add (face 1) | refused, kept | the same | the same | the same |
| deletion after the add returned, before the verification (p1, p1r) | Git, discarded | refused, kept: a failed attempt leaves a populated destination, `Undecidable` | refused, kept | refused, kept |
| deletion while a gate or a review runs (p2, p4) | judged, discarded | judged, discarded | judged, discarded: a gate's or review's failure is not an error | refused, kept |
| a genuine verification failure, registration whole | Git, discarded | **kept**: a change for genuine failures | **kept** | Git, discarded, as at master |
| a genuine snapshot error before the add, such as the store (w4b) | Git, discarded | the same | **kept, on every resume** | the same as round 3 |
| a genuine gate or review failure, registration whole (p3, p5) | judged, discarded | the same | the same | the same |
| the texts it widens | — | `src/workspace.rs` | `src/engine/attempt.rs`, `src/engine/coordinator.rs`; the expectations of `m-keepall` and T-P7b reverse | `src/workspace.rs`, `src/engine/attempt.rs`, `src/engine/tests.rs`'s wording; no instrument |
| the registry lock | — | held shared through the verification's `git status` | — | — |

**Does it cover later gates and reviewers?** C-SIDE does, and neither (a) nor (b) does.
- It covers every failure a gate or a review pass reports while its snapshot exists. The snapshot is alive until the
  gates, or every review pass, have run.
- It does not need to cover what passes: nothing is discarded then.
- A deletion that lands after the check cannot have caused the failure the check followed: the registration was whole
  until the check.
- It does not establish that a registration survives after construction. It establishes something narrower and
  sufficient for the discard: no failure is judged once its snapshot's registration is gone.

**The exact unfreeze text it would need.** It is not part of the five texts of §3.8. If the owner takes C-SIDE, three of
them change as follows; `src/engine/coordinator.rs` and `src/engine/resume.rs` do not, because the coordinator's arm
keys on a recorded candidate, whatever recorded it.
- **`src/workspace.rs`:** after "The other two are attempted again whatever failed.", one sentence is added, and
  nothing else moves:

  > When a snapshot's verification fails after its add succeeded, the module reads whether the snapshot's own worktree
  > registration is whole — the directory its `.git` file names still holds `gitdir`, `commondir`, `HEAD` and `index`
  > — through one private function that reads files and starts no Git child; a verification that fails while the
  > registration is not whole refuses as a registry refusal, and one that fails while it is whole is returned as
  > before.
- **`src/engine/attempt.rs`:** the text of §3.8 is replaced by:

  > AMENDED ONCE, on the owner's decision to close the static and deadline residue of
  > `PR329-LEGACY-RUNS-IN-LINKED-CHECKOUTS-RACE-THE-SHARED-WORKTREE-REGISTRY` and the legacy consequence of
  > `PR329-AN-EXTERNAL-PRUNE-DELETES-AN-ENGINE-WORKTREES-REGISTRATION`: `run_attempt` takes one more argument, a slot in
  > which it records the candidate it captured — the branch ref, parent and tree `capture_candidate` returned — when
  > the gate or the review snapshot's worktree-registry access refuses (`UpstrokeError::RegistryRefused`), and when a
  > gate fails or cannot run in the gate snapshot, or a review pass fails or cannot run in the review snapshot, while
  > that snapshot's own worktree registration is no longer whole; in that case the attempt returns a registry refusal
  > instead, and the gate's or the review's result is not judged. One private function reads whether a snapshot's
  > registration is whole — the directory its `.git` file names still holds `gitdir`, `commondir`, `HEAD` and `index`
  > — from the snapshot's root, reading files and starting no process; two more record the candidate, one comparing
  > the error's variant and one building the refusal, each copying the three strings; and the type that holds them is
  > new. Every other error `run_attempt` returns, every judged result, and every other step, are as before. It calls
  > nothing of the workspace, the runner or the event log that it did not already call, and nothing else in the module
  > moves.
- **`src/engine/tests.rs`:** "They drive the same entry points through a worktree registry another process has torn."
  becomes "They drive the same entry points through a worktree registry another process has torn, and through a
  snapshot whose registration another process has deleted."

**Its tests, if adopted:** p1 and p1r need a git wrapper first on `PATH`, process-wide, so they stay design evidence.
The planned tests would be T-C1 (a gate that deletes its snapshot's registration and fails: refused and kept, as p2),
T-C2 (the same for a review, as p4), T-C3 and T-C4 (p3 and p5, the controls), and T-C5 in `src/workspace.rs`'s inline
module (the verification over a registration deleted between the add and it, through the module's existing
`gate_snapshot_for_candidate_in_with` and its `add_worktree` closure). Its mutation is `rd3`: the check removed. Its
proof includes the effects censuses: the private form passes every one (§3.7).

**Consistent with #329, in spirit and in its differences.**
- **The same rule.** #329's candidate 1 re-checks a topology checkout's registration before a durable negative outcome
  and refuses resumably when it is gone, on the same monotonicity argument.
- **What D's check reads differently.** #329's reads the entry's `gitdir` naming the checkout back. D's reads four
  files' presence, for two reasons: it compares no spelling (Windows' `C:/…`, relative paths), and it also catches an
  entry partly deleted.
- **#329's candidates 2 and 3 have no legacy counterpart.** The legacy snapshot is ephemeral and never recreated, and
  its cleanup counts an unregistered destination as reclaimed (`src/workspace.rs:1572-1580`), with or without the
  store.
- **Candidates 4 and 5** (environmental requirements, the owner's scope ruling) apply to the legacy face unchanged.

**Why it is not adopted.** The brief allows no new machinery in this round, and C-SIDE is new machinery: a reader in two
modules, three call sites, and three widened texts. It is the smallest closure of R-D9's legacy consequence that covers the
verification, the gates and the reviews, and changes nothing for a genuine failure. It goes to the owner's consolidated
question with #329's finding, as a candidate with its exact text.

### 3.5 R-G: the P1 kept, and D's P2 argument as the owner's question

- **The grade stays P1.** All three lenses retained it: the constructed partial-clone state demonstrates a concrete
  mechanism, and the negative controls do not establish that its preconditions are impossible. The regression lens
  re-ran rg2 through the real engine and confirmed it.
- **D's argument for P2, recorded for the owner, not resolved.**
  - The legacy engine runs no production `git commit`; `Workspace::commit` has only test callers (§2.5).
  - Its one path to automatic maintenance is a promisor lazy fetch. A full clone (rg1) and an ordinary blob-less clone
    (rg3) never fetched and never started maintenance in 72 of 72 runs, and the failing state (rg2) was constructed.
  - MAINTAINING reclassifies down "a P1 whose failure needs speculative preconditions".
- **The question for the owner:** is R-G's legacy face a P1, or a P2 on prevalence? D closes it either way, by the
  builder's four settings (§2.5).
- **Re-run on round 3** (`rd3`, three rounds, three Gits): rg1, rg2, rg3 and l7 are green, 9 of 9 each.

### 3.6 FUD-D2-TL2: T-L2's oracle, split

**The finding (P3, executed by the concurrency lens).** T-L2's "16 attempts under the 500 ms test deadline" is a
measurement of fast attempts. An attempt slower than the deadline is the only one: the access refuses after it,
correctly.

**Executed on `rd3`** (#329 round 5's static-tear witness; three rounds on each of three Gits; `FIGURES.txt`):
- at Git's speed: `RegistryRefused` after 16 add attempts, the 16th the final attempt, at 1,006 to 1,008 ms;
- with every add 0.6 s slow (`slow`): `RegistryRefused` after **1** add attempt, at 1,118 to 1,119 ms;
- both are the same correct outcome, and both leave the intent for the reclaim.

**The split.**
- **T-L2 asserts only what is portable:** `RegistryRefused`, never Git; at least one add attempt; the error's text names
  the store and the deadline; the snapshot's intent is left for the reclaim. No attempt count.
- **The final-attempt rule is #329's to test, apart from real Git:** its T4 asserts that a failure repaired after the
  last regular attempt is passed by the final attempt at the deadline, on the helper with a synthetic attempt, and its
  mutation m8 (no final attempt) turns it red (#329's record §6.7, standing under §7.8).
- **T-L1 stays handshake-driven** (`CONTENDED_ATTEMPTS`): a tear finished after the first failed attempt is passed. That
  is the legacy add's own progress claim, and it uses no time but a watchdog.

### 3.7 Tests: what round 3 changes in the plan

**Revised:**
- **T-L2:** as §3.6.
- **T-L3, a checkout that cannot be made** (a 300-byte name): `RegistryRefused` after exactly 1 attempt, and the
  destination gone. Round 2 had Git state after 1 attempt and 1 probe. Executed: #329 round 5's witness on `rd3`, above.
- **T-L8, the veto on five destinations, called directly, no Git:** an empty one is `Attempt` and is empty again; one
  gone is `Undecidable` and nothing is made there; one holding a file, a file in its place, and an empty one the access
  cannot remove (Unix: its parent read-only) are `Undecidable`. Executed: l8.

**Unchanged:** T-L1, T-L4, T-L5, T-L7, and T-P1 to T-P15. Round 3 adds no test. v1, v1c, v1n, v2 and v3 need a git
wrapper first on `PATH`, process-wide, so they stay design evidence: their decisions are T-L8's, T-L3 takes the real add
through them, and T-P1 takes a refusal through the coordinator.

**The planned mutations:**
- **`m-return`** (executed as `rd3-m-return`): a destination gone, or empty and not removable, answers `Return`. Red: v1,
  v1c, v1n, v3 and l8, on the three Gits. It replaces round 2's `d3-m-r6veto` and `d3-m-noprobe`, which tested the
  withdrawn probe.
- **`again` always `Attempt` on the add:** T-L3 red: the checkout is attempted again until the deadline, then refused.
- §1.9's and §2.8's other mutations stand.

**Round 1's and round 2's witnesses on round 3** (`rd3`; and on `rd3cp` and `rd3c`, to show the candidate moves none of
them):
- round 1's fifteen (w1 to w9, w3b, w4b, and #329 round 5's four workspace witnesses), on Git 2.43.0, three rounds: all
  green on all three;
- round 2's w10 to w15, on Git 2.43.0, three rounds: all green on all three;
- rg1 to rg3 and l7 on the three Gits, on `rd3`: green, 9 of 9 each.

**The suites** (`fud3/probe/suite-rd3-nowit/`, `suite-rd3cp-nowit/`, `suite-rd3c-nowit/`; every source touched first,
its own Compiling line):

| Tree | Run | Result |
|---|---|---|
| `rd3-nowit` (round 3) | clippy `-D warnings`, all targets | rc 0 |
| `rd3-nowit` | suite 1 | 3,026 passed, 2 failed, 126 ignored, in 118.66 s |
| `rd3-nowit` | suite 2 | 3,026 passed, 2 failed, 126 ignored, in 124.86 s |
| `rd3cp-nowit` (C-SIDE, the form §3.4 gives) | clippy | rc 0 |
| `rd3cp-nowit` | suite 1 | 3,026 passed, 2 failed, 126 ignored, in 109.22 s |
| `rd3c-nowit` (C-SIDE's first form) | clippy | rc 0 |
| `rd3c-nowit` | suite 1 | 3,025 passed, **3** failed: the two below, and the classification census naming `registration_whole` |

- **The two failures in every run** are the non-frozen manager tests every #329 round since 4 has moved, #329's to move:
  `a_removal_records_the_one_attempt_the_unix_arm_makes` and
  `an_add_killed_before_it_wrote_gitdir_is_unlisted_and_refuses_forced_cleanup`.
- **Every `engine::tests` and `workspace::tests` test passed in every run** (188 and 47 `ok` lines), and so did every
  effects census but the first form's classification census (`fud3/probe/SUITES.txt`).
- **No load flake** appeared in these five suites, where round 2's conformed prototype met two filed ones.

### 3.8 The exact unfreeze texts, all five, restated in full

> **Round 4:** the five texts stand, byte for byte. The clause table's first row reads with §4.4: the arm observes an
> empty destination it can remove. The text's reason, "was never taken over by Git", is the one FUD-D3-TAKEOVERWORD
> corrects, and §4.4 gives its replacement for the owner.

These are amendments to `effects/allowlist.toml`. Each entry's `path`, `allows`, `packet` and `shrinks_when` stay as
they are, and so does `FROZEN_LEGACY_ALLOWLIST` (`src/effects.rs:1306`). **One text changes in round 3, marked CHANGED:**
`src/workspace.rs`, its add sentence and the private functions it names. The other four are the texts of §2.9, word for
word. All five stay PROPOSED until reviewed. §3.4 gives the texts C-SIDE would need; they are not these.

**`src/workspace.rs` (`:898-924`) — CHANGED in round 3.** "AMENDED ONCE" becomes "AMENDED TWICE". The last sentence,
"The schema-4 equivalents live behind funnels in `crate::workspace_manager` and nothing here calls them: the constant
is read, and no funnel is called.", is replaced by:

> The second amendment, to close `PR329-LEGACY-RUNS-IN-LINKED-CHECKOUTS-RACE-THE-SHARED-WORKTREE-REGISTRY` on the
> owner's decision to unfreeze the module for this one change, is two things and no more. First, the three Git
> children that enumerate the repository's worktree registry — `switch_branch`'s `git switch`, `add_gate_worktree`'s
> `git worktree add`, and `cleanup_gate_workspace`'s `git worktree remove` together with the `git worktree list` that
> decides whether it took the registration — each run as the attempt of
> `crate::workspace_manager::tolerant_registry_access`, which attempts one again until its deadline and then refuses
> as a registry refusal (`UpstrokeError::RegistryRefused`), never as Git state. The add holds the registry lock
> shared. After a failed add, an empty destination it can remove was never taken over by Git, so it is removed, made
> again and attempted again; anything else at the destination — gone, an empty directory it cannot remove, a
> directory that is not empty, anything that is not a directory, or metadata it cannot read — refuses at once as a
> registry refusal, so the add's own failure is never returned as Git state. The other two are attempted again
> whatever failed. One private helper resolves the canonical common git dir as `recorded_objects_scope` does, through
> `git_command`, and private functions make the destination again and decide the add's veto, which reads only the
> destination and starts no Git child. Second, `git_command`, the one builder every Git child of the module starts
> from, also passes `-c maintenance.auto=false -c gc.auto=0 -c gc.autoDetach=false -c maintenance.autoDetach=false`
> from one new constant, so no Git child of the module, and no Git process one of them starts, runs Git's automatic
> maintenance, and none of them prunes a registration another checkout is writing. The test module gains the
> regression tests for both. Every other behaviour of the module stays frozen. The schema-4 equivalents live behind
> funnels in `crate::workspace_manager`, and nothing here calls a funnel: the constant is read, and the tolerant access
> is called, which takes no site.

**What changed from §2.9's text, exactly:**
- **Removed:** "After a failed add, an empty destination it can remove is removed, made again and attempted again; an
  absent destination, or an empty one it cannot remove, is decided by a checkout of the add's commit into it that reads
  no registration — `read-tree` through the module's existing builder, in a Git directory of the destination's own
  that names the repository's common git dir — and the add's failure is returned as Git state only when that checkout
  cannot be made either; anything else at the destination refuses at once as a registry refusal."
- **Added in its place:** "After a failed add, an empty destination it can remove was never taken over by Git, so it is
  removed, made again and attempted again; anything else at the destination — gone, an empty directory it cannot
  remove, a directory that is not empty, anything that is not a directory, or metadata it cannot read — refuses at once
  as a registry refusal, so the add's own failure is never returned as Git state."
- **Removed:** "and private functions make the destination again and probe it." **Added:** "and private functions make
  the destination again and decide the add's veto, which reads only the destination and starts no Git child."
- Nothing else in the text moves.

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

**`src/engine/resume.rs` (`:855-867`) — unchanged from round 2.** One paragraph is appended to `legacy_effect`:

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

**How each changed clause matches its implementation** (§1.5's and §2.9's tables stand for every other clause):

| Text | Clause | The implementation that makes it true |
|---|---|---|
| `workspace.rs` | "an empty destination it can remove was never taken over by Git, so it is removed, made again and attempted again" | `legacy_add_veto`'s directory arm: `read_dir` empty, `remove_dir` ok, `remake_destination` → `Again::Attempt` (`fud3/probe/patch-rd3.py`); executed: l8, the transient tear |
| `workspace.rs` | "anything else at the destination — gone, … or metadata it cannot read — refuses at once as a registry refusal" | every other arm of `legacy_add_veto` answers `Again::Undecidable { why }`, which #329's contract refuses at once (§6.4 step 5); executed: l8, v1, v1c, v1n, v2, v3, T-L3's witness |
| `workspace.rs` | "so the add's own failure is never returned as Git state" | no arm answers `Again::Return`; the access returns Git only on `Return` (§6.4 step 4); `rd3-m-return` is the mutation |
| `workspace.rs` | "private functions make the destination again and decide the add's veto, which reads only the destination and starts no Git child" | `remake_destination` and `legacy_add_veto`: `symlink_metadata`, `read_dir`, `remove_dir`, `create_private_dir`; no `git_output`, no builder call; executed: no `read-tree` process in any `rd3` v1 trace |

**Their union, unchanged:** the four production files and the appended tests; #329 owns the helper, its error variant
and its classification.

### 3.9 `design/15`: the PROPOSED paragraph, amended

> **Round 4:** its add-veto sentence and its source change again (§4.4).

The paragraph stays PROPOSED and in the same place. Round 3 changes two things in it, and adds no sentence elsewhere:
- **Its source** reads "§1, as §2 and §3 amend it".
- **Its add-veto sentence** becomes: "After a failed snapshot add, an empty destination the engine can remove was never
  taken over, and the add is attempted again; anything else at the destination refuses as a registry refusal, so the
  add's own failure never comes back as Git state."

No sentence `src/export.rs` pins moves, and the hunk stays apart from #329's PROPOSED paragraph.

### 3.10 What legacy users see, and what remains, after round 3

> **Round 4:** §4.5 amends R-D3, R-D4 and R-D9.

**§2.11, amended:**
- **A snapshot add after a failure.** An empty, removable destination is made again and the add attempted again.
  Anything else ends the command at once with a registry refusal that names Git's message, the captured output kept in
  the checkout and pinned. No probe runs.
- **A genuine failure after Git took the destination over** (a checkout the snapshot's longer path cannot hold, a
  failing filter, a full or read-only store, a missing object): a resumable refusal, the output pinned, where master
  discarded it and failed the command. Each resume over a cause that persists pays one more worker attempt, as master's
  did, and keeps one more pin (§3.2).

**§1.8's and §2.11's residuals, amended:**

| | What | Consequence | Where |
|---|---|---|---|
| R-D3 (amended) | D's own Git child killed by a signal mid-add | a destination left non-empty or gone is refused at once and kept; one left empty and removable is attempted again; never Git state | §3.2 |
| R-D4 (amended) | a takeover whose junk removal left the destination, on any platform; on Windows, a delete-pending destination | the removal proof answers `Attempt` when the access can remove it, and `Undecidable` otherwise: a refusal that keeps | §3.2 |
| R-D9 (reclassified) | the post-return deletion: a prune that decided in the add's window deletes the registration after the add returned, before the verification or while a gate or a review runs | the failure is judged or returned as Git state, and the coordinator discards; executed (p1, p1r, p2, p4). Legacy-only: P1, outside G6. Mixed: applies to G6 (Q6, R17) and blocks it, as part of `PR329-AN-EXTERNAL-PRUNE-DELETES-AN-ENGINE-WORKTREES-REGISTRATION`. C-SIDE closes it except a partly deleted entry; not adopted | §3.4 |
| R-D13 | the probe's fidelity | withdrawn with the probe | §3.2 |
| R-D1, R-D2, R-D5 to R-D8, R-D10 to R-D12 | as §1.8 and §2.11 | as there | — |

### 3.11 The G6 classification, round 3

> **Round 4:** §4.6 replaces the FUD-D2-PRUNE and R-D9 rows: what divides them is whether the deletion fails the add.

| Case | What D closes, given #329's helper | Severity | Applies to G6 | Blocks G6 |
|---|---|---|---|---|
| (e1), (e1′), (e2), (e2′) | as §1.8, unchanged by round 3 | P1 each | (e2) and (e2′) yes; (e1) and (e1′) no | (e2) and (e2′) until #329 and D are implemented and validated, (e2′) unless the owner rules otherwise |
| FUD-D2-PRUNE (face 1: a prune deletes the registration during the add) | its consequence, the discard: a destination gone is `Undecidable`, so refused at once and kept (v1, v1c, v1n). Its liveness, a resumable end, is #329's face 1 | P1 class | legacy-only no; mixed, as #329's finding's face 1 | its discard: no, once D lands; the class's liveness: as #329's finding |
| R-D9 (face 2: the deletion after the add returned) | **residual** at this head; C-SIDE, evaluated and not adopted, would close it but for a partly deleted entry | P1 | **legacy-only: no. Mixed: yes, Q6 and R17** | **yes for the mixed class**, as part of #329's finding, until a closure is implemented and validated or the owner rules on its scope |
| R-G (`FUC-D2-RG`) | closed by the builder's four settings (§2.5) | P1, pending the owner's reclassification (§3.5) | yes: Q1; ST-18 and INV-22 per #330's lenses | until D lands, or the owner reclassifies or excludes it |

### 3.12 What §3 replaces in §1 and §2

| §1 or §2 says | Replaced by |
|---|---|
| §2.6, change 6: D's add adopts round 7's veto with its probe, in D's private-gitdir form | §3.3: round 8's dated §5.5; the removal proof alone |
| §2.7: the probe, "`read-tree` runs no hook", the fidelity paragraph, and "absent, or an empty directory the access cannot remove: the probe decides" | §3.2 |
| §2.7: R-D9 "does not block G6" | §3.4: legacy-only no; mixed yes, under #329's finding |
| §2.8: T-L2's "16 attempts", T-L3's "Git state after 1 attempt and 1 probe", T-L8's five cases, and the mutations `d3-m-r6veto` and `d3-m-noprobe` | §3.6 and §3.7 |
| §2.9: the `src/workspace.rs` text's add sentence and "private functions make the destination again and probe it" | §3.8 |
| §2.10: the `design/15` add-veto sentence | §3.9 |
| §2.11: "A snapshot add after a failure … one probe … comes back as Git state", R-D3, R-D4, R-D9 and R-D13 | §3.10 |
| §2.12: the FUD-D2-PRUNE row's G6 columns | §3.11 |
| §2.15: FUD-D2-PRUNE "fixed … for a prune before the add returns: D's add adopts round 7's veto" | §3.14: the same closure, by `Undecidable` |

### 3.13 The Windows and macOS time budget, round 3

- **Round 3 adds no planned test.** Two get cheaper:
  - **T-L3** runs one add and no probe: 8 to 11 ms here, where round 2 measured 10 ms with its probe;
  - **T-L8** runs no Git at all: five directory operations, where round 2's ran one probe.
- **T-L2** keeps its fixed waits.
- **So §2.14's estimates stand as upper bounds:** about +13 s on the guest harness, +48 s hosted and +23 s on macOS for
  rounds 1 and 2 together (`fud2/wintime/BUDGET-r2.txt`). None of it is executed on those legs, and CI is the truth.
- **C-SIDE, if adopted,** adds T-C1 to T-C5: four engine tests whose witnesses took 0.165 to 0.171 s each here (the
  median over nine runs), and one module test. By §1.11's factors (19.8× guest, 26.3× hosted, 7.9× macOS) that is about
  13 s, 18 s and 5 s of test time, or about +1 s, +4 s and +2 s of harness wall over 12, 4 and 3 threads
  (`fud3/wintime/BUDGET-r3.txt`). It is an estimate, not a measurement.

### 3.14 The findings round 3 answers

> **Round 4:** FUD-D2-PRUNE is fixed for a deletion that fails the add. FUD-D2-RD9 includes a deletion before a
> successful add returns, and C-SIDE is a partial mitigation (§4.8).

| Finding | Sev | Kind | D | Where | Evidence |
|---|---|---|---|---|---|
| FUD-D2-PROBECONFIG | P1 | executed (all three lenses) | **Fixed (design), witnessed.** The probe and D's private-gitdir variant are withdrawn; a destination gone, or empty and not removable, answers `Undecidable`, so the output is kept | §3.2 | v1c and v1n red (Git, discarded) on `d3w` and green on `rd3`; v1, v3 and l8 green on `rd3`; `rd3-m-return` red |
| FUD-D2-HOOKS | P2 | executed (all three) | **Fixed (design), witnessed.** The veto starts no Git child | §3.2 | one `read-tree` process in every `d3w` v1 run, none in any `rd3` run |
| FUD-D2-RD9 | P1 | executed (all three) | **Reclassified and referred, not fixed.** Legacy-only: outside G6; mixed: applies and blocks G6, as part of #329's external-prune finding. C-SIDE evaluated, executed and given its exact text, not adopted | §3.4 | p1, p1r, p2, p4 red on `rd3`, green on `rd3cp` (and on the first form `rd3c`); p3, p5 green on all three |
| FUD-D2-TL2 | P3 | executed (concurrency) | **Fixed (design), witnessed.** T-L2 asserts no count; the final-attempt rule is #329's T4 | §3.6 | the static tear: 16 add attempts at Git's speed, 1 with a 0.6 s add, `RegistryRefused` in both |
| R-G (`FUC-D2-RG`) | P1 | executed (round 2; re-run by the regression lens) | **Kept at P1; the owner's question.** D's P2 argument recorded | §3.5 | rg1 to rg3 and l7 green on `rd3` |
| FUD-D2-PRUNE | P1 class | reasoned (round 2) | **Fixed (design), witnessed,** now by `Undecidable` rather than the probe | §3.2 | v1 green on `rd3`, red on `rd3-m-return` |
| §1.2 against #329's §5.5 | — | conformance | **Conformed** to round 8's dated §5.5 (`f7a9256c`) | §3.3 | the four dated changes, each adopted |

## 4. Round 4 design

> **PROPOSED — the last design round of #331 before the owner's consolidated question,** pending the owner's decision
> B (`~/orch-pr11/ESCALATION.md` item 7). Round 4 answers design review round 3 on `ac18321f`. It changes how the design
> describes itself, not what the design does: it adds no machinery, adopts no closure, and leaves the five exact
> unfreeze texts as §3.8 gives them, byte for byte. §1 to §3 stay the design except where §4 replaces them. Each place
> it does is marked in §3, and §4.7 lists them.

**Who writes it.** Round 4 is `pr11_fud_design4`'s (`claude-opus-5-5`, `max`), a fresh session the PR11 orchestrator
spawned with the brief `~/orch-pr11/briefs/pr11_fud_design4.md`. Its work list is the triage
`~/orch-pr11/reviews/review-331-d3-triage.md`, items 1 to 4. It runs no witness of its own: every figure below is the
reviewers' or an earlier round's, in the saved file its sentence names. Round 4's own files (the hash checks, the
gates, the body and CI) are under `~/orch-pr11/logs/pr11_fud_design4/`, cited as `fud4/…`. Code is cited at master
`5c222ff2`; the branch is not rebased.

### 4.1 What round 3's review found, and what round 4 changes

**The review.** Three `gpt-6-astra` lenses at `max`, on the `cameron-codex` account, reviewed `ac18321f`: design,
concurrency and regression. Each returned **CHANGES_REQUIRED**.
- **The concurrency lens ran twice.** Its first run ended after 121,418 tokens on "Selected model is at capacity",
  with no review (`~/orch-pr11/reviews/review-331-d3-concurrency-ac18321f-CAPACITY-ERROR.log`). The orchestrator re-ran
  it on the same model and account, and the re-run's text is the concurrency review.
- **The texts** are `~/orch-pr11/reviews/review-331-d3-{design,concurrency,regression}-ac18321f.review.md`, and the
  triage is `review-331-d3-triage.md`. The hashes of the texts, their logs, the capacity log, the lens prompts and the
  witnesses are in `SHA256SUMS-331-d3`, and the lens prompts' again in `SHA256SUMS-331-d3-lenses`. All 184 and all 4
  check (`fud4/review/sha256sum-c-331-d3.txt`, `sha256sum-c-331-d3-lenses.txt`).
- **The witnesses** are under `~/orch-pr11/reviews/331-d3-witnesses/`, one directory per lens run, cited below by
  directory name:
  - `review331-d3-design-u6ezehsy/`, the design lens's;
  - `review331-d3-concurrency-1l4wygsk/`, the concurrency lens's re-run;
  - `review331-d3-reg-iilv6n42/`, the regression lens's;
  - `review331-d3-conc-r91cjnww/` is the capacity-ended run's scratch, kept and not relied on.
- **What all three accepted, and round 4 keeps as it is** (the triage's converged list):
  - the withdrawal, for failed adds: round 2's two configuration cases, re-run independently on the three Gits,
    discarded on round 2's prototype and were refused and pinned on round 3's;
  - the G6 classification: legacy-only outside G6, mixed applicable and blocking;
  - C-SIDE's placement: the verification, gate errors and verdicts, review errors and verdicts; not the coordinator;
  - R-G: the P1 kept, and D's P2 argument the owner's question;
  - §3.3's conformance to #329's contract;
  - the five texts, byte-compared;
  - T-L2's split;
  - §3.10's costs and §3.13's estimates.

**The work list** (the triage's items 1 to 4).

| Item | Severity | What round 4 does | Where | Evidence |
|---|---|---|---|---|
| FUD-D3-DURINGADD | P1 (all three lenses, executed) | R-D9 is divided by whether the deletion fails the add, not by whether it lands before or after the add returns. A deletion that leaves the add successful, landing before its return included, is R-D9's residual. | §4.2 | the Git level with real barriers on 2.43.0, 2.50.1 and 2.55.0; through `rd3` (Git, discarded) and `rd3cp` (refused, pinned) on the same three |
| FUD-D3-CSIDEPROOF | P1 (concurrency, regression), P2 (design) | C-SIDE is restated as a partial mitigation: what it catches, its two residuals and their severity, whether checking earlier or checking more would close them, and its instrument cost exactly. It is aligned with #329's corrected closure 1. Not adopted. | §4.3 | a deleted `index` written again, through `rd3cp` and at the Git level; a split index's shared file deleted, through `rd3cp`; on the three Gits |
| FUD-D3-TAKEOVERWORD | P3 (design, reasoned) | The add veto's empty-destination arm is described by what it observes, not by what Git did. `design/15`'s sentence changes; the five texts do not. | §4.4 | — |
| #329's round 9 | — | #329's head became its round 9, `8df42436`, while this round ran: the pull request was updated at 2026-10-02T15:30:16Z (`fud4/check/pr-329-head.txt`). C-SIDE's description is aligned with its corrected closure 1. | §4.3 | #329's record §8.3 and §8.4 at `8df42436` |

**What changes in the design's text:**
- R-D9's boundary, in §3.4's comparison table, §3.10 and §3.11 (§4.2, §4.5, §4.6);
- C-SIDE's soundness, residuals and cost (§4.3);
- the description of the add veto's empty-destination arm, and `design/15`'s add-veto sentence (§4.4).

**What does not change:**
- **the five exact unfreeze texts,** §3.8's, byte for byte; §4.4 says how the one clause FUD-D3-TAKEOVERWORD touches is
  read;
- every rule, planned test, mutation and prototype result of §1 to §3, and C-SIDE's exact texts (§3.4);
- §3.13's time budget: round 4 adds no planned test.

### 4.2 R-D9 divided by failed and successful add (FUD-D3-DURINGADD)

**What the lenses executed.** A prune decides on a snapshot's entry before `locked` exists, and is held. The add
completes its checkout and clears Git's junk-cleanup flag. The prune deletes the entry. The add then unlinks `locked`,
which tolerates a missing file, and exits 0. The access returns `Ok`, so D's veto, which runs only after a failed
attempt, never runs. The verification's `git status` fails, and the coordinator discards.
- **At the Git level, with real barriers.** On 2.43.0, 2.50.1 and 2.55.0 a real
  `git worktree prune --expire 3.months.ago` deleted the entry ("gitdir file does not exist") after the add's checkout
  was complete and while the add was still running. The add then exited 0, and `git status` in the checkout exited
  128 (the concurrency lens: `review331-d3-concurrency-1l4wygsk/during-add-results.json`, its script
  `boundary-witnesses.py`, whose barriers are #329's preloaded pause shims; cited, not re-run).
  - #329's design review round 8 recorded the same on the three versions
    (`~/orch-pr11/reviews/329-d8-witnesses/pr329-d8-prune-before-return-9o_2jeby/results.json`).
  - #329's round 9 re-executed it with Git's own `post-checkout` hook, 18 of 18 (its record §8.3, at `8df42436`).
- **Through the real legacy engine,** with a git wrapper first on `PATH` that deletes the registration before it
  returns the add's success, on the three Gits:
  - `rd3` (round 3's design): `run=Git`, nothing kept, the checkout clean
    (`review331-d3-design-u6ezehsy/results.json` and `duringadd-*-rd3.log`; `review331-d3-reg-iilv6n42/results.json`
    and `git-version-*-during-add-success-rd3.log`);
  - `rd3cp` (round 3 with C-SIDE): `RegistryRefused`, the candidate pinned and in the checkout (the same files, the
    `rd3cp` logs).

**Round 3 was wrong in two places.** §3.4's first comparison row, "deletion during the add (face 1): refused, kept",
and §3.11's FUD-D2-PRUNE row, "a prune deletes the registration during the add: … refused at once and kept". Both hold
only for a deletion that fails the add.

**The boundary, restated: what decides the consequence is whether the deletion fails the add,** as #329's round 9
divides its two faces (its §8.3). For a prune that decided in the add's window:

| Where its deletion lands | The add | What D does (round 3's design, unchanged) | Status |
|---|---|---|---|
| before the add writes its entry's `locked` | fails before it takes the destination over; the destination is still the empty directory the snapshot made | `Attempt`: the destination is made again and the add attempted again, with a new entry (reasoned from Git's order, §1.3) | closed by the retry |
| after `locked`, before the add's last write into its entry (`gitdir`, `commondir`, `HEAD`, or the checkout's index) | fails on that write; Git's junk removal removes the destination | `Undecidable`: refused at once, the candidate pinned (v1, v1c, v1n on the three Gits, `fud3/probe/witness-runs/TABLE.txt`) | **closed** (FUD-D2-PRUNE); the resumable end is #329's face 1 |
| after the add's last write into its entry, before the add returns (DURINGADD) | **succeeds**, exit 0; the access returns `Ok` | the verification fails: Git state, discarded | **R-D9** |
| after the add returned, before the verification (p1, p1r) | succeeds | Git state, discarded | **R-D9** |
| while a gate or a review runs (p2, p4) | succeeds | judged, discarded | **R-D9** |

**So R-D9 is the successful-add face.** It is #329's face 2 for the legacy snapshot, as #329's round 9 states face 2:
the add returns `Ok`, and its registration is deleted, wholly or in part, at any time from the prune's decision onward.
That is before the add returns (DURINGADD), before the verification (p1, p1r), or while a gate or a review runs (p2,
p4). Its grade and its G6 classification are §3.4's:
- P1 by consequence, the discard of paid output;
- legacy-only, outside G6;
- mixed, it applies through Q6 and R17 and blocks G6, as part of
  `PR329-AN-EXTERNAL-PRUNE-DELETES-AN-ENGINE-WORKTREES-REGISTRATION`.

**What would address DURINGADD is what addresses p1,** because its verification meets the same state:
- §2.7's (a) and (b) keep the output, since they keep every verification failure after a successful add;
- C-SIDE keeps it while a checked name is missing at its check (executed above: `rd3cp` refused and pinned on the three
  Gits), with §4.3's residuals.

None of them is adopted.

**§3.4's comparison table, amended.** Its first row is divided, its C-SIDE column carries §4.3's residuals, and its
texts row states the instrument. Every other cell is round 3's.

| Case | round 3 (D as designed) | (a) the verification inside the add's attempt | (b) keep on every snapshot failure after capture | C-SIDE |
|---|---|---|---|---|
| a deletion that fails the add (face 1: v1, v1c, v1n) | refused, kept; before the takeover, attempted again | the same | the same | the same |
| a deletion that leaves the add successful, before it returned (DURINGADD) | **Git, discarded** | refused, kept: a failed attempt leaves a populated destination, `Undecidable` | refused, kept | refused, kept while a checked name is missing at the check (§4.3) |
| a deletion after the add returned, before the verification (p1, p1r) | Git, discarded | refused, kept | refused, kept | as the row above |
| a deletion while a gate or a review runs (p2, p4) | judged, discarded | judged, discarded | judged, discarded: a gate's or a review's failure is not an error | as the row above |
| R-REWRITE or R-OUTSIDE (§4.3) at the verification | Git, discarded | refused, kept | refused, kept | **Git, discarded** |
| R-REWRITE or R-OUTSIDE at a gate or a review | judged, discarded | judged, discarded | judged, discarded | **judged, discarded** |
| a genuine verification failure, registration whole | Git, discarded | **kept**: a change for genuine failures | **kept** | Git, discarded, as at master |
| a genuine snapshot error before the add, such as the store (w4b) | Git, discarded | the same | **kept, on every resume** | the same as round 3 |
| a genuine gate or review failure, registration whole (p3, p5) | judged, discarded | the same | the same | the same |
| the `effects/allowlist.toml` texts it widens beyond §3.8's five, each an instrument text change | — | `src/workspace.rs`'s | `src/engine/attempt.rs`'s and `src/engine/coordinator.rs`'s; the expectations of `m-keepall` and T-P7b reverse | three: `src/workspace.rs`'s, `src/engine/attempt.rs`'s and `src/engine/tests.rs`'s wording; no `effects/wrappers.toml` row (§4.3) |
| the registry lock | — | held shared through the verification's `git status` | — | — |

### 4.3 C-SIDE, a partial mitigation, aligned with #329's corrected closure 1 (FUD-D3-CSIDEPROOF)

**What the lenses executed.**
- **A deleted `index` written again before the check.**
  - **At the verification, through `rd3cp`, on the three Gits.** The snapshot's `index` is deleted after its add
    returned. The verification's `git status` exits 0 and reports every path deleted and untracked, which fails the
    verification. A `git read-tree HEAD` in the snapshot, injected by the reviewers' wrapper, writes the index again.
    C-SIDE then finds the four names, the verification's Git error is returned, and nothing is kept: `run=Git`,
    `kept=[]` (`review331-d3-design-u6ezehsy/index-rewritten-results.json` and `index-rewritten-*.log`;
    `review331-d3-concurrency-1l4wygsk/index-engine-results.json`, `recreate` 1).
  - **Without the injected command the same run is refused and pinned**
    (`review331-d3-concurrency-1l4wygsk/index-engine-results.json`, `recreate` 0;
    `review331-d3-reg-iilv6n42/results.json`, `index-before-status` on `rd3cp`).
  - **At a gate, at the Git level, on the three Gits.** With `index` deleted, a gate's `git diff --cached --exit-code`
    exits 1 where the control exits 0. A later `git add -A` exits 0, and the four names are whole again
    (`review331-d3-reg-iilv6n42/followup-results.json`, `index-recreated-by-later-git`). The same holds with
    `git diff --quiet HEAD` and `git read-tree HEAD`
    (`review331-d3-concurrency-1l4wygsk/index-recreated-results.json`). The judgement and the discard that follow are
    reasoned from `src/engine/attempt.rs:159-167` and `src/engine/coordinator.rs:774`.
  - **What does not write it:** `git status`, the verification's command. With `index` gone it prints every path
    deleted and untracked, and leaves `index` absent (`review331-d3-reg-iilv6n42/results.json`, `git-index-recreation`:
    `index_recreated` false on the three Gits). #329's round 9 found the same for `git status` in the engine's read
    form (its §8.4).
- **A split index's shared file.** With `core.splitIndex=true`, deleting the snapshot's `sharedindex.<hash>` breaks the
  checkout ("index file open failed: No such file or directory") while `gitdir`, `commondir`, `HEAD` and `index`
  remain. Through `rd3cp` on the three Gits: `run=Git`, `kept=[]`, the status 128
  (`review331-d3-design-u6ezehsy/additional-results.json`, `sharedindex-*.log`).
- **What C-SIDE did catch.**
  - `HEAD`, `commondir` or `index` removed alone, and the whole entry during the add: refused and pinned through `rd3cp`
    on the three Gits (`review331-d3-design-u6ezehsy/results.json`, modes `HEAD`, `commondir`, `index` and
    `duringadd`).
  - Round 3's p1, p1r, p2 and p4 (`fud3/probe/witness-runs/TABLE.txt`).
  - `gitdir` removed alone breaks nothing: `git status` exits 0 and prints nothing
    (`review331-d3-reg-iilv6n42/followup-results.json`, `partial-delete-control`;
    `review331-d3-concurrency-1l4wygsk/git-level-results.json`).

**Round 3's proof was wrong.** §3.4 argued that "a registration whole at the check was whole throughout the failed
step". Present at the check means present at the check.
- A deleted `index` can be written again by a later Git command.
- A checkout can be broken by a missing file that the four names do not include.

Round 3 listed both under "What it leaves", and its proof contradicted that list. The premise holds for the entry as a
whole and for `HEAD` and `commondir`, not for `index` (below). The same premise was the hint for #329's closure 1, and
the orchestrator withdrew it (`~/orch-pr11/answers/pr11_fub_design9-0.md`).

**C-SIDE is a partial mitigation, not a closure.** #329's round 9 states its closure 1 the same way after the same
addendum (its record §8.4, at `8df42436`). C-SIDE is that check's legacy form, and this round uses #329's names for
the two residuals.
- **What it catches, soundly** (#329's §8.4, and the executions above):
  - a prune only removes names, so a checked name missing at the check stays missing, and the complete deletion is
    always caught;
  - `HEAD` and `commondir` cannot be written again from the snapshot: with either gone, every Git command there exits
    128 before it does anything (`review331-d3-concurrency-1l4wygsk/git-level-results.json`). Nothing in the legacy
    engine writes them again either: it runs no `git worktree repair`, and never adds a snapshot twice (§3.4);
  - a missing `gitdir` alone fails nothing there (`git status` exits 0), so a failure with only `gitdir` gone was not
    the deletion's; C-SIDE keeps it anyway, which is the safe direction;
  - so a failure the deletion caused is kept whenever the deletion reached `HEAD` or `commondir`, or reached `index`
    and nothing wrote it again before the check, whenever the deletion landed: DURINGADD, p1, p1r, p2 or p4.
- **What it leaves: two residuals.** Each is P1 where it occurs, because the coordinator then discards paid output,
  MAINTAINING's data-loss criterion. Each is inside R-D9, and so inside
  `PR329-AN-EXTERNAL-PRUNE-DELETES-AN-ENGINE-WORKTREES-REGISTRATION`.
  - **R-REWRITE: a removed `index` written again before the check.**
    - Any index-writing Git command in the snapshot does it while `HEAD` and `commondir` remain (`read-tree HEAD` and
      `add -A`, executed above).
    - It needs the prune's pass to reach `index` before `HEAD`, `commondir` and `gitdir`, and to be held, killed, or
      not yet further, from the failing command until the check.
    - **Who can write it, by placement:**
      - at the verification, nothing of the engine's. C-SIDE reads right after the verification's `git status`
        (`src/workspace.rs:837`), which does not write a missing index. Only another process in the snapshot can;
      - at a gate, the gate's own command, a process it left running, or another process. `gates::run_all` returns at
        the first failing gate and starts nothing of its own (`src/gates.rs:196-235`);
      - at a review, the reviewer agent's own Git, or another process. The engine runs the agent in the snapshot and
        no Git command there itself (`review::run_review`, `src/review.rs:462`). This case is reasoned, not executed.
  - **R-OUTSIDE: a file the snapshot needs that the check does not read.**
    - **`sharedindex.<hash>` under `core.splitIndex=true`** (executed above). Its name is random, so a prune's pass can
      meet it first: #329's round 9 listed it before `HEAD` in 4 of 9 entries on this box's ext4 (its §8.4).
    - **`config.worktree` and `info/sparse-checkout`,** which Git's add copies into the new entry from a base with
      worktree configuration or a sparse checkout (§3.2). Their removal changes what the snapshot's commands do rather
      than failing them (reasoned, as #329's §8.4).
    - It needs the pass to reach that file before the four names, and to be held, killed, or not yet further at the
      check: an entry the prune has left whole as C-SIDE reads it.
  - **They are not a separate finding.** They stay inside R-D9's class, which stays open while they do. The owner's
    ruling must name them, as #329's §8.11 asks for its closure 1.

**Would checking earlier, or checking more, close them?** Neither form is adopted.
- **Checking before any further Git command runs in the snapshot.** At the scope the engine controls, C-SIDE already
  does: at each of its three placements no Git command of the engine's runs between the failure and the check (above).
  - The commands that write `index` again are the gate's own, the reviewer's own, or another process's.
  - Checking before them would mean running inside a user's gate command or an agent's process, or keeping every other
    process out of the snapshot's entry. The engine does neither, at any scope.
  - **So it closes nothing more.** R-REWRITE stays wherever such a command can run, and R-OUTSIDE needs no later
    command at all.
- **Checking every file the snapshot depends on.**
  - **By name, the set is open.** Which per-worktree files a checkout needs depends on its configuration and on Git's
    version. The shared index is named only inside `index`'s binary `link` extension, and `config.worktree` and
    `info/sparse-checkout` exist only under some configurations. A reader of all of them reads Git's internal formats,
    version by version.
  - **By asking Git: a read of the index at the check.** `git ls-files --stage` exits 128 without the shared index
    (executed by #329's round 9; its §8.4 and its option A′).
    - It closes R-OUTSIDE's split-index part at all three placements.
    - It does not cover a configuration file whose loss changes behaviour without failing a read.
    - It costs a Git child at each negative outcome. That child must carry the snapshot's private hook settings, as
      the verification's `git status` does, or a configured fsmonitor hook runs (round 2's FUD-D2-HOOKS).
    - It costs an instrument edit. The gate and review checks sit in `src/engine/attempt.rs`, which starts no Git
      child of its own, so the read would be a crate-visible workspace method with an `effects/wrappers.toml` row
      (§3.4's first form).
  - **By a list recorded once the verification has passed.** A gate's or a review's failure with a recorded name now
    absent is not whole. It narrows R-OUTSIDE at gates and reviews to the dependencies that exist and are recorded when
    the list is taken: a dependency created later, such as the new `sharedindex.*` a gate's `update-index
    --split-index` writes, or one that disappears between the verification and the inventory, is outside it, so it is
    one more partial mitigation (FUD-D4-LATERDEP, CR-9). *(Implementation, §5.9: the PR11 decision appendix §11's D
    row, replacing "It covers R-OUTSIDE at gates and reviews, by name, whatever the configuration, with no Git
    child.")* It cannot reach the verification itself, which is the first look at the entry.
  - **No form closes R-REWRITE.** A name written again is present, and cannot be told from one never removed (#329's
    §8.4).
- **At the verification alone, both residuals close by not judging the verification at all.** §2.7's (a) and (b) keep
  the output on every verification failure after a successful add, genuine or not, at the cost §4.2's table prices.
- **So the most a design on the consequence side reaches,** without adopting any of it, is (a) or (b) at the
  verification, and C-SIDE with a read of the index or a recorded list at gates and reviews.
  - That leaves R-REWRITE at gates and reviews, and whatever part of R-OUTSIDE the chosen read does not cover.
  - What no check after the failure can reach is the prune itself: #329's closures 4 (the starters' configuration,
    with its documentation) and 5 (the owner's ruling on scope), which apply to the legacy face unchanged (§3.4).

**The predicate, against #329's corrected check.** #329's §8.4 states its check in three items, and says its items 1
and 2 are C-SIDE's.
- **C-SIDE's reader,** as prototyped (`fud3/probe/patch-rd3.py`, `snapshot_registration_whole`) and as its texts give
  it, reads the `.git` file's `gitdir:` line, resolved against the snapshot, and the four names as regular files.
- **#329's check adds** that the entry is in this repository's own store, that `gitdir`, `commondir` and `HEAD` are
  non-empty, and its item 3: `commondir` resolves to the repository's common git dir, and `gitdir` names the checkout's
  `.git` as the same file.
- **A prune only removes names,** so none of these additions changes what either check catches of the class, and
  neither changes the residuals. The additions reject an entry in another repository's store, an entry another add
  registered again under the same name, and a truncated file. None of them is a prune's state, and a legacy snapshot's
  name is unique (`upstroke-gates-<pid>-<ulid>`, never added twice; §3.4).
- **For R-D9 the two are the same check with the same residuals.** If the owner wants one check for both engines, the
  legacy reader takes #329's three items as they stand, and C-SIDE's texts name them in place of the four names. That
  is not proposed here.

**Its cost, exactly.** Round 3's comparison row said "no instrument", which the lenses read as "no wrapper row".
- **Three `effects/allowlist.toml` texts widen beyond §3.8's:** `src/workspace.rs`'s (one sentence added),
  `src/engine/attempt.rs`'s (replaced) and `src/engine/tests.rs`'s (its wording), as §3.4 gives them. The effect
  allowlists are an instrument (`CLAUDE.md`'s list), so this is an instrument text change, inside decision B's five
  texts.
- **No `effects/wrappers.toml` row: the readers are private.**
  - The crate-visible first form failed the classification census (`fud3/probe/suite-rd3c-nowit/suite-1.log`).
  - The private form passed every census in round 3's suite (`fud3/probe/SUITES.txt`).
  - The regression lens re-ran the payload, Git-child, classification and frozen-list censuses on `rd3cp`, and each
    passed (`review331-d3-reg-iilv6n42/followup-results.json`, `census`).
- **No row, `path`, `allows`, `packet` or `shrinks_when` change,** and no `FROZEN_LEGACY_ALLOWLIST` change.
- **Its planned tests,** T-C1 to T-C5, and their time (§3.4, §3.13) stand. If it is adopted, its tests also pin the two
  residuals as residuals, with the reviewers' constructions, so that a later change sees them move, as #329's §8.10
  plans for its closure 1.

**What stays as round 3 gave it:** C-SIDE's placement, its exact texts (§3.4), its prototype's results, and the decision
not to adopt it. It goes to the owner's consolidated question beside #329's closure 1, as a partial mitigation with
R-REWRITE and R-OUTSIDE named. #329's §8.11 lists it so, in its row for a legacy run's discard of paid output.

### 4.4 The add veto's empty destination, described by what it observes (FUD-D3-TAKEOVERWORD)

**The finding (P3, the design lens, reasoned).** §3.2 calls the empty-destination arm "the removal proof", and three
texts say such a destination "was never taken over": §3.8's `src/workspace.rs` text, §3.9's `design/15` sentence, and
round 3's body. Removability does not show that.
- Git takes the destination over when it sets its junk work tree to it.
- A Git child killed by `SIGKILL` after that, and before it writes the destination's `.git`, runs no junk removal.
- The destination is then left empty and removable, although Git did take it over.

That is R-D3 (§1.3 item 2, reason 2; §3.2), where the rule already attempts the add again.

**The rule does not change, and it stays conservative.** After a failed add, the veto reads the destination. An empty
directory the access can remove, and make again, answers `Attempt`; anything else answers `Undecidable` (§3.2's
table). Why attempting again is safe needs no history:
- **An empty destination holds nothing.** The worker's output is the captured candidate, in the run's own checkout. A
  snapshot's destination holds only what its add checked out. Removing an empty one loses nothing.
- **The next attempt is bounded.** It starts from the same empty private directory, and the access ends it in success
  or, at its deadline, in `RegistryRefused`, which B-PRESERVE keeps and pins (§1.4). It never ends in Git state.
- **What reaches the arm, none of it told apart:**
  - a failure before the takeover, the case the arm is for (the transient tear: `Ok` after 2 adds, the destination
    empty after the first failure, `fud3/probe/witness-runs/FIGURES.txt`);
  - a takeover whose junk removal emptied the destination and could not remove it, since become removable (R-D4);
  - a takeover cut short by a signal before Git wrote there (R-D3). The next attempt then meets whatever the killed add
    left in the store, and either completes or is refused at the deadline, kept (§3.2's R-D3).

**"The removal proof" is renamed "the removal predicate."** Where §2 and §3 say "the removal proof", read "the removal
predicate". It is the same rule, which observes the destination and proves nothing about Git's history.

**Where the wording changes.**
- **`design/15`'s PROPOSED paragraph, its add-veto sentence.** "After a failed snapshot add, an empty destination the
  engine can remove was never taken over, and the add is attempted again; …" becomes "After a failed snapshot add, an
  empty destination the engine can remove holds nothing to lose, so it is made again and the add is attempted again;
  anything else at the destination refuses as a registry refusal, so the add's own failure never comes back as Git
  state."
  - Its source reads "§1, as §2 to §4 amend it".
  - Nothing else in the paragraph moves, no sentence `src/export.rs` pins moves, and the hunk stays apart from #329's
    PROPOSED paragraph.
- **This record:** §3.2's arm and its "removal proof" lines, §3.8's clause table and §3.10's R-D4 row read as above.
- **Not the five exact texts.** §3.8's `src/workspace.rs` text keeps "After a failed add, an empty destination it can
  remove was never taken over by Git, so it is removed, made again and attempted again", because this round changes
  none of the five texts (the triage, item 4).
  - Its rule is the one above. Only its stated reason is the one FUD-D3-TAKEOVERWORD corrects, and no implementation
    clause depends on that reason (§3.8's clause table: `legacy_add_veto`'s directory arm).
  - **The replacement, for the owner to take with decision B** if the reason is to be corrected in the text itself,
    with nothing else moving: "After a failed add, an empty destination it can remove holds nothing to lose, so it is
    removed, made again and attempted again". It is not part of the five texts unless the owner takes it.

### 4.5 What legacy users see, and what remains, after round 4

**§3.10, amended.** What a legacy user sees does not change: round 4 changes no rule. The residuals are described
again:

| | What | Consequence | Where |
|---|---|---|---|
| R-D3 | D's own Git child killed by a signal mid-add | as §3.10; a child killed after Git took the destination over and before it wrote there leaves it empty and removable, so the add is attempted again | §3.2, §4.4 |
| R-D4 | a takeover whose junk removal left the destination, on any platform; on Windows, a delete-pending destination | the removal predicate answers `Attempt` when the access can remove the destination, and `Undecidable` otherwise: a refusal that keeps | §3.2, §4.4 |
| R-D9 (re-divided) | the successful-add face: a prune that decided in the add's window deletes the registration, wholly or in part, and the add still succeeds; the deletion lands before the add returns (DURINGADD), before the verification, or while a gate or a review runs | the failure is returned as Git state or judged, and the coordinator discards; executed (DURINGADD, p1, p1r, p2, p4). Legacy-only: P1, outside G6. Mixed: applies to G6 (Q6, R17) and blocks it, as part of `PR329-AN-EXTERNAL-PRUNE-DELETES-AN-ENGINE-WORKTREES-REGISTRATION`. C-SIDE, not adopted, is a partial mitigation: it keeps the output whenever a checked name is missing at its check, and leaves R-REWRITE and R-OUTSIDE, each P1 where it occurs | §4.2, §4.3 |
| R-D1, R-D2, R-D5 to R-D8, R-D10 to R-D12 | as §3.10 | as there | — |

A deletion that fails the add is not a residual: §3.2's veto refuses it at once and keeps the output (FUD-D2-PRUNE),
or attempts it again when it came before the takeover.

### 4.6 The G6 classification, round 4

§3.11, with its FUD-D2-PRUNE and R-D9 rows replaced:

| Case | What D closes, given #329's helper | Severity | Applies to G6 | Blocks G6 |
|---|---|---|---|---|
| (e1), (e1′), (e2), (e2′) | as §1.8, unchanged | P1 each | (e2) and (e2′) yes; (e1) and (e1′) no | (e2) and (e2′) until #329 and D are implemented and validated, (e2′) unless the owner rules otherwise |
| FUD-D2-PRUNE: **a prune's deletion that fails the add** (#329's face 1) | its consequence, the discard: after the takeover a destination gone is `Undecidable`, refused at once and kept (v1, v1c, v1n); before the takeover the empty destination is attempted again. Its liveness, a resumable end, is #329's face 1 | P1 class | legacy-only no; mixed, as #329's face 1 | its discard: no, once D lands; the class's liveness: as #329's finding |
| R-D9: **a prune's deletion that leaves the add successful,** before or after the add returns (#329's face 2) | **residual** at this head. C-SIDE, evaluated and not adopted, is a partial mitigation that leaves R-REWRITE and R-OUTSIDE (§4.3) | P1; each residual P1 where it occurs | **legacy-only: no. Mixed: yes, Q6 and R17** | **yes for the mixed class,** as part of #329's finding: until closures are implemented and validated and the owner rules on what they leave (C-SIDE's residuals at least), or the owner rules on scope. Filing is no waiver |
| R-G (`FUC-D2-RG`) | as §3.11, unchanged | P1, pending the owner's reclassification (§3.5) | yes: Q1; ST-18 and INV-22 per #330's lenses | until D lands, or the owner reclassifies or excludes it |

### 4.7 What §4 replaces in §1 to §3

| §1 to §3 say | Replaced by |
|---|---|
| §3.2: "the removal proof, kept from round 7", and "the removal proof" wherever §2 and §3 name the empty-destination arm | §4.4: the removal predicate, described by what it observes |
| §3.4's comparison table: its first row, "deletion during the add (face 1): refused, kept", its C-SIDE column, and its row "the texts it widens … no instrument" | §4.2: the table amended |
| §3.4, of C-SIDE: "Why it is sound", "What it leaves", "A deletion that lands after the check cannot have caused the failure the check followed", "no failure is judged once its snapshot's registration is gone", "on the same monotonicity argument", "What D's check reads differently", "Classifying it would take an `effects/wrappers.toml` row … The private form needs none", and "the smallest closure of R-D9's legacy consequence" | §4.3: a partial mitigation, its two residuals, the two stronger checks, its cost exactly, and #329's corrected closure 1 |
| §3.8's clause table, first row: "was never taken over by Git" | §4.4: read as the observable predicate; the text itself unchanged |
| §3.9: the `design/15` add-veto sentence, and its source "§1, as §2 and §3 amend it" | §4.4 |
| §3.10: R-D3, R-D4's "the removal proof", and R-D9's row | §4.5 |
| §3.11: the FUD-D2-PRUNE row ("during the add") and R-D9's row | §4.6 |
| §0's round-3 row, §3.1 and §3.4: C-SIDE called "a consequence-side closure" or "the candidate closure" | §4.3: a consequence-side partial mitigation |
| §3.14: FUD-D2-PRUNE's "Fixed (design), witnessed" | §4.8: for a deletion that fails the add |
| §2.15: FUD-D2-PRUNE's "for a prune before the add returns", already replaced by §3.14 | §4.8: for a deletion that fails the add |

### 4.8 The findings round 4 answers

| Finding | Sev | Kind | D | Where | Evidence |
|---|---|---|---|---|---|
| FUD-D3-DURINGADD | P1 | executed (all three lenses) | **Reclassified into R-D9 and referred, not fixed.** R-D9 is divided by failed versus successful add. A deletion that leaves the add successful, before its return included, is R-D9's residual, under #329's finding (face 2 as its round 9 states it) | §4.2 | the Git level with real barriers on three Gits (`during-add-results.json`; #329's round-8 witness); `rd3` Git and discarded, `rd3cp` refused and pinned, on three Gits |
| FUD-D3-CSIDEPROOF | P1 / P2 | executed (all three) | **Described accurately, not fixed.** C-SIDE is a partial mitigation. Its residuals R-REWRITE and R-OUTSIDE are P1 where they occur, inside R-D9's class. Checking earlier closes nothing more; checking more narrows R-OUTSIDE only. Its cost is three widened `effects/allowlist.toml` texts and no wrapper row. Aligned with #329's corrected closure 1 (`8df42436`) | §4.3 | the index written again, through `rd3cp` and at the Git level; the split index through `rd3cp`; on three Gits |
| FUD-D3-TAKEOVERWORD | P3 | reasoned (design) | **The descriptions fixed; the text's clause kept.** The removal predicate is described by what it observes, in `design/15` and this record. The five texts are unchanged, by the brief, and the clause's replacement is given for the owner | §4.4 | — |
| FUD-D2-PRUNE | P1 class | reasoned (round 2), witnessed (round 3) | **Fixed (design), witnessed, for a deletion that fails the add,** by round 3's `Undecidable`. A deletion that leaves the add successful is R-D9 | §4.2, §4.6 | v1, v1c and v1n green on `rd3`, red on `rd3-m-return` |
| FUD-D2-RD9 | P1 | executed (round 2's lenses) | **Reclassified and referred, not fixed,** as round 3 said, with DURINGADD inside it and C-SIDE a partial mitigation | §4.2, §4.3 | as §3.14, and the executions above |
| #329's corrected closure 1 | — | conformance | **Aligned.** C-SIDE is its legacy form, a partial mitigation with the same two residuals | §4.3 | #329's record §8.3, §8.4 and §8.11 at `8df42436` |

## 5. Implementation

> **Implemented on this branch, a draft, and PROPOSED — conditional on the owner's decision O8 (decision B), and not
> granted.** O8 is a ruling on packet invariants PR5 and PR12 (§1.7) and a real contract, adoption and merge decision.
> The five `effects/allowlist.toml` texts, with (i), §4.4's one-clause replacement, and the `design/15` paragraph are
> proposals. The canonical packet and every existing owner grant are unchanged. **This draft is not merge-ready.** Its
> merge waits on O8's adoption and on #329 merged, which itself needs O9, O14 and O11. O10 is not a prerequisite
> (`~/orch-pr11/answers/pr11_fud_impl-1.md`): R-G keeps its P1 grade until this change shows it closed and the reviewers
> confirm it.

**Who and on what.** `pr11_fud_impl` (`claude-opus-5-5`, `max`), a fresh implementer `orch_pr11` spawned on 2026-10-03
on `9b2262f4` (design round 4). Its brief is `~/orch-pr11/briefs/pr11_fud_impl.md`, with one answer
(`~/orch-pr11/answers/pr11_fud_impl-1.md`), and its scope is `~/orch-pr11/d-impl/UNIT.md`'s unit. Its figures are under
`~/orch-pr11/logs/pr11_fud_impl/`, cited as `fudi/…`. It implements §1 to §4 as design review round 4 left them, with
that round's applicable review fixes: the PR11 decision appendix's §10.3 (RESTOREBYTES, CHERRYPICKMERGE, PINSAMEFAULT,
WARNDELIVERY, REFUSALTIME) and its §11 D rows (FUD-D4-LATERDEP's claim narrowed, FUD-D4-PINSAMEFAULT corrected), as
§5.9 gives them.

### 5.1 What it implements, and what it does not

**In:**
- **Corrected B1′** (§1.3, as §2.7, §3.2 and §4.4 leave it): the three registry children of `src/workspace.rs` as
  attempts of #329's `tolerant_registry_access`, the add's veto as the removal predicate with no probe, and the removal
  and its list as one attempt. This is (e1) and (e2).
- **B-PRESERVE** (§1.4, as §2.2 and §2.3 amend it): the captured candidate recorded at both snapshot calls, kept and
  pinned by the coordinator, and named by every resume that reaches its lookup, over every attempt the log records.
  This is (e1′) and (e2′).
- **R-G** (§2.5): `git_command` with Git's automatic maintenance off.
- **The five exact unfreeze texts** (§3.8; the appendix's Annex R.1) with **(i)**, §4.4's one-clause replacement, and
  D's `design/15` paragraph, each marked proposed, conditional on O8.
- **The regression tests and mutations** §1.9, §2.8, §3.6 and §3.7 plan (§5.5 to §5.7).

**Out, and unchanged:**
- **(ii) C-SIDE's three widened texts** (§3.4), with O1, and their open P1s FUD-D4-HEADRECREATE and FUD-D4-LATERDEP's
  C-SIDE form; nothing of C-SIDE is implemented.
- **(iii) ENV-1's widening of the `src/workspace.rs` text,** with O3: no Git child's inherited environment changes (no
  ENV-1, option (C) or ENV-R). FUD-D4-ENV is filed on its own (§5.12).
- **Anything of follow-ups C or F,** and B's mechanism beyond the helper D calls.
- **Every frozen file of G6's set** (PR11's R-D list): §5.10 proves it byte-identical.

**No acceptance is inferred.** R-D1, R-D5 and R-D7, and, without C-SIDE, R-D9's successful-add discards stay stated and
filed (§5.12). No residual, data-loss, accounting or environment acceptance is made or implied here.

### 5.2 Provenance: B merged, not rebased

- **`255f67b8`** merges #329's head `54a1ff147ee99ac1a61f47d483cf7b3852fe4158` into `9b2262f4` (parents `9b2262f4` and
  `54a1ff14`), as follow-up C did. A rebase would orphan the reviewed SHAs every `reviewed_sha` of this ledger names, and
  `reviewed_sha` is never re-stamped (`MAINTAINING.md`, "Merge commits only, everywhere").
- **The one conflict** is the legacy finding file, which both branches add with different texts (add/add). B's text is
  taken (blob `b0c59cad`, which carries B's dated note of 2026-10-03), as the brief directs. The merged tree differs from
  B's head only by this record and D's `design/15` paragraph.
- **`cb1beacf`** is the implementation, on the merge. The commit after it carries this section, the record's §10.3 and
  §11 corrections in place, and the findings (§5.12).

### 5.3 What changed, per file

**`src/workspace.rs`** (PR5-frozen; its proposed text is the twice-amended row):
- `AUTO_MAINTENANCE_REFUSED` (`:45`): `-c maintenance.auto=false -c gc.auto=0 -c gc.autoDetach=false -c
  maintenance.autoDetach=false`, passed by `git_command` (`:56`) right after `REPLACE_REFS_REFUSED`. R-G (§2.5).
- `switch_branch` (`:464`): the `git switch` child is one attempt of `tolerant_registry_access`, `RegistryHold::Unheld`,
  `again` always `Again::Attempt`, keyed by `canonical_common_dir`. The filter and checkout-tree refusals run once
  before it, as before.
- `add_gate_worktree` (`:898`): the `git worktree add` child and its exit check are one attempt, `RegistryHold::Shared`,
  vetoed by `legacy_add_veto`.
- `cleanup_gate_workspace` (`:1583`): the removal and `worktree_is_registered`'s list are one attempt, success when the
  list does not register the path, `RegistryHold::Unheld`, `again` always `Attempt`. Everything after it is unchanged.
- `canonical_common_dir` (`:1644`): `git rev-parse --path-format=absolute --git-common-dir` through `git_path`, so
  through `git_command`, then `fs::canonicalize`, the steps `recorded_objects_scope` takes.
- `legacy_add_veto` (`:1655`) and `remake_destination` (`:1688`): §3.2's table. An empty directory the access can remove
  is removed and made again (`create_private_dir`), `Attempt`; gone, an empty directory it cannot remove, a non-empty
  directory, anything that is not a directory (a link included) and unreadable metadata are `Undecidable`, naming why.
  No arm answers `Return`; no Git child starts.
- The module still has one `Command::new(`, in the builder, and the fourteen builder call sites in the same functions
  and order: its census, `every_git_child_of_this_module_is_built_where_replacements_are_refused`, is unchanged and
  green. It gains no `.env(`, so `src/runner/contract.rs`'s payload census (five) is unchanged. No `#[cfg(test)]` item
  enters the production region.
- The test module gains seven tests and their helpers, appended after its existing tests; every byte of the module
  before its closing brace is unchanged (`fudi/frozen/append-only-cb1beacf.txt`).

**`src/engine/attempt.rs`** (PR5-frozen): `RefusedCandidate` (`:91`), `note_refused` (`:97`), and `run_attempt`'s new
`refused` argument (`:113`). The gate snapshot (`:177`) and the review snapshot (`:211`) each gain
`.inspect_err(|error| note_refused(…))` before their `?`. Nothing else moves: the return type, every other `?`, the
capture.

**`src/engine/coordinator.rs`** (PR5-frozen): `KEPT_PIN_SUFFIX` (`:281`), and at `run_attempt`'s one call (`:546`) the
arm: with a refused candidate recorded, `prepare_commit_from_candidate` pins it at the attempt's `prepared_pin_ref`
followed by `-kept`, and the arm returns `UpstrokeError::RegistryRefused` naming the pin ("…; the worker's output for
attempt <n> of `<task>` is kept in this checkout and pinned at `<pin>`") or its failure ("…, and pinning it at `<pin>`
failed: <error>"), with no discard. Every other error discards, as before. *(Implementation round 3, §5.17: no
longer — part N keeps and pins on every attempt error after the worker ran, and the refused arm's pin is written whatever
`HEAD` is.)*

**`src/engine/resume.rs`** (PR5-frozen): `KEPT_PIN_SUFFIX` imported; the lookup (`:562`) over every attempt the replayed
log records (`progress[i].attempts`), through `prepared_pin_target`; the discard unchanged; and one warning (`:584`)
when any kept pin exists. The warning, exactly (§2.2's, with the appendix §10.3's two corrections):

> the worker output of the attempt(s) a worktree-registry refusal stopped is kept, and no resume removes it:
> `<pin>`[, `<pin>`…]. Each pin is a commit on the HEAD its output was captured on. To take the output back as the
> repository records it — its index exactly, its working files through the checkout's own end-of-line and filter
> conversions, so compare their bytes before relying on them — deletions included, and not as `git replace`
> substitutes for it: while HEAD is still the pin's parent, `git --no-replace-objects -c core.useReplaceRefs=false
> restore --source=<pin> --staged --worktree -- .` from the checkout's root; on a later HEAD, `git --no-replace-objects
> -c core.useReplaceRefs=false cherry-pick --no-commit <pin>`, and before removing the pin check what it staged (`git
> diff --cached`), because a configured merge driver can make it succeed having applied none of the kept change. `git
> update-ref -d <pin>` removes the pin, and every later resume then stops naming it

**`src/engine/tests.rs`** (PR5-frozen, append-only): eighteen tests and their helpers appended; the file at the merge is
a byte prefix of the file at `cb1beacf` (38,021 bytes appended; `fudi/frozen/append-only-cb1beacf.txt`). No existing
test changes.

**`effects/allowlist.toml`:** the five `legacy_effect` texts, exactly (`fudi/texts/apply_texts.py`, which extracts them
from this record at `9b2262f4`, checks the extract against the appendix's Annex R.1 hash `d55c2227…` and R.2's
`55f88d73…`, writes them, and verifies by parsing the TOML that each entry's text is the old text with its amendment
and that every other field of every entry, the funnel section and the rest of the file are equal;
`fudi/texts/apply-texts.log`):
- `src/workspace.rs`: "AMENDED ONCE" becomes "AMENDED TWICE", and the last sentence is replaced by §3.8's text with
  (i): "After a failed add, an empty destination it can remove holds nothing to lose, so it is removed, made again and
  attempted again", in place of "… was never taken over by Git, so it is removed, made again and attempted again";
- `src/engine/attempt.rs`, `src/engine/coordinator.rs`, `src/engine/resume.rs` and `src/engine/tests.rs`: §3.8's
  paragraph appended, word for word.
- **Each of the five entries carries a TOML comment above it:** "PROPOSED, conditional on the owner's decision O8
  (decision B), and not granted". The exact texts themselves are unchanged by that marking, which is why it is a
  comment.
- 0 rows added or removed; 0 `path`, `allows`, `packet` or `shrinks_when` changes; `FROZEN_LEGACY_ALLOWLIST`
  (`src/effects.rs`) unchanged; 0 `effects/wrappers.toml` rows (the helper's `effect_free` row is #329's).

**`design/15_design_event_log_resume_run_layout.md`:** D's paragraph, "A legacy attempt the worktree registry refused",
now says it is implemented on this draft and proposed, conditional on O8; that once in force it replaces the last item
of #329's paragraph, which says the frozen legacy accesses do not take #329's access; and two sentences are corrected
(§5.9: WARNDELIVERY, RESTOREBYTES). No sentence `src/export.rs` pins moves.

**`docs/internals/`** (§13): the notes of the five noted modules gain a section per new item and test, the
`LEGACY-EFFECT` sections say the amendment is proposed, and two headings follow their changed code lines
(`attempt.md`'s review-snapshot heading and `coordinator.md`'s `run_attempt` call). The modules carry no new comment.

### 5.4 The contract it calls: #329's helper at `54a1ff14`

Every clause §3.3 lists, checked against #329's code at `54a1ff14` (`src/workspace_manager.rs`):
- the signature `tolerant_registry_access(common_git_dir, hold, again: &mut dyn FnMut() -> Again, attempt)` (`:1783`),
  `RegistryHold` (`:1628`) and `Again { Attempt, Return, Undecidable { why } }` (`:1641`); D answers `Attempt` and
  `Undecidable` only, so `Again::Return`'s production `expect(dead_code)` stays fulfilled;
- `Undecidable` refusing at once as `RegistryRefused`, naming `why` (step 5), and the final attempt after a sleep the
  deadline cut short (step 7), with the end-to-end bound that includes the last attempt and the veto;
- the deadline, 10 s in production and 500 ms under `cfg(test)`;
- `CONTENDED_ATTEMPTS` and `contended_attempts`, `cfg(test)`, keyed by the common git dir as passed: D's tests key them
  with `canonical_common_dir`'s spelling;
- the canonical key, nothing sampled, and the `effect_free` row in `effects/wrappers.toml` (`src/workspace_manager.rs`'s
  list, `"tolerant_registry_access"`).

D adds nothing to #329's files.

### 5.5 The tests, by the record's numbers

Each test waits on a handshake, never on time alone: the after-capture hook of the legacy attempt, or #329's
`CONTENDED_ATTEMPTS` counter for the repository's canonical common git dir (§1.2 item 8), read by a writer thread
(`OnceContended`) that finishes the tear only after an access has failed on it and decided to attempt again. That
thread has a sixty-second watchdog and stops when the access returns, so a mutation that never contends fails at its
assertion rather than wedging. Every planted registration is written as Git writes it (`GitdirRule::native().spelling`:
`/` on Windows), as #329's `8364009d` taught. No test needs a seam in production code.

| Record | Test | Where |
|---|---|---|
| T-L1 | `a_snapshot_add_beside_a_tear_its_writer_finishes_is_attempted_past` | `src/workspace.rs` |
| T-L2 | `a_snapshot_add_beside_a_tear_that_stays_refuses_as_the_registrys_and_leaves_its_intent` | `src/workspace.rs` |
| T-L3 | `a_snapshot_whose_checkout_cannot_be_made_refuses_after_one_attempt_and_leaves_nothing` | `src/workspace.rs` |
| T-L4 | `a_snapshot_removal_beside_a_tear_its_writer_finishes_takes_the_registration` | `src/workspace.rs` |
| T-L5 | `a_branch_switch_beside_a_tear_its_writer_finishes_switches` | `src/workspace.rs` |
| T-L6, (e1) [w7] | `a_legacy_run_completes_past_a_tear_its_writer_finishes` | `src/engine/tests.rs` |
| T-L6, (e2) [w8] | `a_legacy_run_completes_past_a_topology_slot_its_writer_finishes` | `src/engine/tests.rs` |
| T-L7 | `every_git_child_of_this_module_runs_with_automatic_maintenance_off` | `src/workspace.rs` |
| T-L8 | `the_snapshot_add_veto_attempts_again_only_over_an_empty_destination_it_can_remove` | `src/workspace.rs` |
| T-P1, T-P3, T-P5 [w1] | `a_registry_refusal_after_capture_keeps_and_pins_the_captured_candidate_across_both_resumes` | `src/engine/tests.rs` |
| T-P2 [w2] | `a_kept_pin_holds_the_captured_tree_when_the_index_changed_after_capture` | `src/engine/tests.rs` |
| T-P4 [w3] | `following_the_kept_pin_warning_at_its_parent_restores_deletions_too` | `src/engine/tests.rs` |
| T-P4b [w3b] | `following_the_kept_pin_warning_on_a_later_head_applies_exactly_the_kept_change` | `src/engine/tests.rs` |
| T-P6, (e2′) [w5] | `a_topology_slots_torn_registration_refuses_a_legacy_snapshot_and_keeps_its_output` | `src/engine/tests.rs` |
| T-P7 [w4] | `an_attempt_error_that_is_not_a_registry_refusal_still_discards` | `src/engine/tests.rs` |
| T-P7b [w4b] | `a_snapshot_failure_that_is_not_the_registrys_still_discards` | `src/engine/tests.rs` |
| T-P8 [w6] | `a_kept_pin_that_cannot_be_written_is_reported_and_nothing_is_discarded` | `src/engine/tests.rs` |
| T-P9 [w9] | `a_review_snapshot_refused_after_capture_keeps_and_pins_the_candidate` | `src/engine/tests.rs` |
| T-P10 [w10] | `the_kept_pin_restore_writes_the_recorded_blob_under_a_blob_replacement` | `src/engine/tests.rs` |
| T-P11 [w11] | `the_kept_pin_restore_writes_the_recorded_tree_under_a_tree_replacement` | `src/engine/tests.rs` |
| T-P12 [w12] | `the_kept_pin_pick_applies_the_recorded_change_under_a_tree_replacement` | `src/engine/tests.rs` |
| T-P12g [w12g] | `the_kept_pin_pick_takes_the_recorded_parent_under_a_graft` | `src/engine/tests.rs` |
| T-P13 [w13] | `every_kept_pin_is_named_after_a_second_refusal_on_the_next_resume` | `src/engine/tests.rs` |
| T-P14 [w14] | `a_kept_pin_is_named_after_a_resume_that_failed` | `src/engine/tests.rs` |
| T-P15 [w15] | `a_removed_kept_pin_is_named_no_more` | `src/engine/tests.rs` |

**Where a test departs from its plan, and why:**
- **T-L2** asserts only what §3.6 calls portable — a registry refusal, never Git; the add attempted again at least once
  (the handshake's count moved); the refusal names the store and its deadline; the intent left — and adds that the
  reclaim takes the intent once the residue is removed. No attempt count.
- **T-L3** runs two shapes: a tree holding a `.git/` path, which no checkout may make, on every platform, and the
  300-byte name on Unix only, as #329's T15 does. Both refuse after one attempt (the handshake's count does not move),
  naming a destination gone.
- **T-L5** switches to a branch at a new commit, so the switch moves `HEAD`. *(Implementation, §5.16: E3.)* It runs the
  access twice: the first meets the tear unrepaired and refuses, having attempted again; `HEAD` and the status are read
  after it returns, with no access running; the second access's writer then finishes the tear on the handshake. It
  first read `HEAD` and the status on the writer thread, inside the access's deadline.
- **T-L8** adds a link to an empty directory (Unix), `Undecidable` with the link's target untouched; its read-only
  parent's case refuses to run, rather than passing vacuously, where the mode bit does not bind.
- **T-P13**'s second tear is planted by the resumed attempt's own worker (`TearingWorker`), as §2.8 planned: no seam.
- **The tests that compare working-file bytes** (T-P2, T-P4, T-P4b, T-P10 to T-P12g) set `core.autocrlf=false` in their
  repository: FUD-D4-RESTOREBYTES's precondition, made explicit (Git for Windows may configure `true` system-wide).

### 5.6 Red on the first-bad shape

`fudi/tools/mutations.py` builds each shape from `git archive` of the head, as committed, in a scratch tree with fresh
mtimes under the lane's iso base, and runs the 25 new tests with `--exact`; every log's Compiling line names its own
scratch tree (`fudi/mutation/<shape>/summary.txt`, all twenty `True`).
- **`control`** (the head unchanged): 25 of 25 green.
- **`base-firstbad`:** the four legacy modules as master and #329's head have them (`255f67b8`'s, byte for byte; the
  allowlist too), with the new tests kept. T-L8 is left out, because it calls the veto master does not have. **22 of the
  24 fail.** The two that pass are T-P7 and T-P7b, the controls: an error that is not the registry's discards, at master
  and here alike, by design; their first-bad shapes are `m-keepany` and `m-keepall` below. T-L8's are `m-b1pred` and
  `m-return`.

### 5.7 Mutations

Twenty shapes, run 2026-10-03T15:41:34Z to 15:53:51Z at `cb1beacf` (`fudi/mutation/campaign.log`; per shape
`mutation.diff`, `base.txt`, `run.txt`, `test.log`, `summary.txt`; the table `fudi/mutation/TABLE-TIDS.md`, the
per-test cover `fudi/mutation/COVERAGE.txt`):

| Shape | What it changes | Undoes | Red (of the 25 new tests) | Green |
|---|---|---|---|---|
| `control` | none: HEAD as committed | every new test green in the scratch setup | none | 25 |
| `base-firstbad` | the four legacy modules as master and B's head had them (255f67b8), the new tests kept; T-L8 dropped because it calls the veto master lacks | the first-bad shape of every test but T-L8 | 22: T-L1, T-L2, T-L3, T-L4, T-L5, T-L6 (e1), T-L6 (e2), T-L7, T-P1/3/5, T-P2, T-P4, T-P4b, T-P6, T-P8, T-P9, T-P10, T-P11, T-P12, T-P12g, T-P13, T-P14, T-P15 | 2 |
| `m-add-returns` | the add's veto answers Return: a failed add is Git state at once, as at master | corrected B1' at the add | 19: T-L1, T-L2, T-L3, T-L6 (e1), T-L6 (e2), T-P1/3/5, T-P2, T-P4, T-P4b, T-P6, T-P8, T-P9, T-P10, T-P11, T-P12, T-P12g, T-P13, T-P14, T-P15 | 6 |
| `m-b1pred` | the veto never attempts again over an empty destination (the removal treated as failed) | FUB-D4-B1PREDICATE | 5: T-L1, T-L2, T-L6 (e1), T-L6 (e2), T-L8 | 20 |
| `m-return` | a destination gone, or empty and not removable, answers Return (round 2's veto, #329's option (i)) | FUD-D2-PROBECONFIG/FUD-D2-PRUNE's Undecidable | 2: T-L3, T-L8 | 23 |
| `m-always-attempt` | the add's veto always answers Attempt | the veto at all | 1: T-L3 | 24 |
| `m-removal-alone` | round 4's B1': the removal once with its status ignored, the list in its own access | FUB-D4-B1REMOVE | 1: T-L4 | 24 |
| `m-remove-unwrapped` | the removal and its list as master ran them, outside any access | corrected B1' at the removal | 2: T-L4, T-P1/3/5 | 23 |
| `m-switch-unwrapped` | switch_branch's git switch as master ran it | corrected B1' at the switch | 1: T-L5 | 24 |
| `m-maint` | git_command without the four maintenance settings | R-G | 1: T-L7 | 24 |
| `m-index` | the coordinator pins the live index and HEAD at the refusal | FUB-D5-INDEX | 2: T-P2, T-P8 | 23 |
| `m-discard` | the refused arm discards, as every other error | B-PRESERVE | 14: T-P1/3/5, T-P2, T-P4, T-P4b, T-P6, T-P8, T-P9, T-P10, T-P11, T-P12, T-P12g, T-P13, T-P14, T-P15 | 11 |
| `m-keepany` | the coordinator never discards after an attempt error | only a registry refusal keeps | 2: T-P7, T-P7b | 23 |
| `m-keepall` | a snapshot's Git error records the candidate too | only a registry refusal keeps | 1: T-P7b | 24 |
| `m-no-review-record` | the review snapshot's call records nothing | the review snapshot's site | 1: T-P9 | 24 |
| `m-restore` | round 5's warning: `git checkout <pin> -- .`, and no command for a later HEAD | FUB-D5-RESTORE | 6: T-P4, T-P4b, T-P10, T-P11, T-P12, T-P12g | 19 |
| `m-plain` | the warning's commands without the replacement controls | FUD-D1-REPLACE | 4: T-P10, T-P11, T-P12, T-P12g | 21 |
| `m-interrupted` | the lookup over the attempts still in flight only | FUD-D1-PINWARN | 2: T-P13, T-P14 | 23 |
| `m-removepin` | the resume removes each kept pin it finds | the resume's keep (FUB-D4-RESUME) | 11: T-P1/3/5, T-P2, T-P4, T-P4b, T-P6, T-P10, T-P11, T-P12, T-P12g, T-P13, T-P14 | 14 |
| `m-noexist` | the warning names every recorded attempt's pin, existing or not | a removed pin is named no more | 1: T-P15 | 24 |

- **Every one of the 25 tests is killed by at least one mutation** (`COVERAGE.txt`: "tests with no killing mutation: 0"),
  and every mutation kills at least one test.
- **The planned mutations, against the record's plan** (§1.9, §2.8, §3.7): `m-index`, `m-restore` (as round 5's
  warning, so T-P4b has no command for a later `HEAD` either), `m-discard`, `m-removepin`, `m-keepall`, `m-b1pred`
  (the current rule's analogue of round 4's "nothing at the slot": an empty destination is not attempted again),
  "`again` always true on the add" (`m-always-attempt`), "the removal alone as the attempt" (`m-removal-alone`, round
  4's form), "`switch_branch` unwrapped" (`m-switch-unwrapped`), `d2-m-plain` (`m-plain`), `d2-m-interrupted`
  (`m-interrupted`), `d2-m-maint` (`m-maint`) and `m-return`. Added: `m-add-returns`, `m-remove-unwrapped`,
  `m-keepany`, `m-no-review-record` and `m-noexist`, so that T-L2, T-P3's refusal, T-P7, T-P9 and T-P15 each have a
  mutation of their own.

### 5.8 Five suites, the frozen census, the instruments and the residue

`fudi/tools/five-suites.sh`: five `cargo test --all-targets --all-features` at the implementation (`cb1beacf`, git
archive) and five at the base (`255f67b8`, the merge: #329's code and the four legacy modules as master has them),
alternating, each with every Rust source touched, on its own slot pool, and with `TMPDIR` at a fresh directory whose
leftovers are listed afterwards (`fudi/suites/<run>/`, 2026-10-03T15:56:23Z to 16:22:32Z; the summary is
`fudi/suites/summary.out`, from `identity.txt`, `frozen-census.txt` and `residue.txt`).
- **Every run rc 0,** each Compiling line naming its own tree. Head: 3,093 passed, 0 failed, 130 ignored, five of five
  (105.14 s to 154.11 s). Base: 3,068 passed, 0 failed, 130 ignored, five of five (101.37 s to 137.89 s). The 25 more
  at the head are this change's tests.
- **The frozen census** (`frozen-census.txt`): no test failed in any run. The G6-frozen modules' tests pass in every
  run at the same counts at head and base (recover 226, integrate 20, repair 5, finalize 5, `topology::fold` 192,
  `events::log` 47). `workspace::tests`: 47 at the base, 54 at the head. `engine::tests`: 188 at the base and 206 at the
  head, in four runs of five each; one run of each prints one fewer `ok` line, because the existing
  `v1_sibling_run_helper`'s verdict line was split by a child's output (`head-2/test.log:736`, `base-3/test.log:711`),
  and its run's totals are unchanged.
- **The instrument tests** — the effects classification, the allow placement, the disallowed-wrapper list, the
  process-start and payload censuses, the name-count pin and the sequential registry — pass in all ten.
- **The residue, base against head, in a fresh `TMPDIR`** (`residue.txt`): 44 top-level and 154 recursive entries left
  in every one of the ten runs, by class, and **no class's count differs** between the runs. This change leaves nothing
  in `TMPDIR` that the base does not.

### 5.9 The review fixes applied: the appendix's §10.3 and §11 D rows

Each is a text item: "D's five exact texts do not change for them" (the appendix's §10.3). Where the record's earlier
rounds carried the text, it is corrected in place and marked "*(Implementation, §5.9: …)*".

| Item | Sev | Where it now stands | The correction |
|---|---|---|---|
| FUD-D4-RESTOREBYTES | P2 | the resume's warning (§5.3); `design/15`'s paragraph; §2.2's byte claims | the warning: "To take the output back as the repository records it — its index exactly, its working files through the checkout's own end-of-line and filter conversions, so compare their bytes before relying on them — deletions included, and not as `git replace` substitutes for it:"; `design/15`: "… as the repository records it — the index exactly, the working files through the checkout's own conversions — deletions included"; §2.2's checks hold "in a repository with no end-of-line conversion or smudge filter for those paths". The tests that compare bytes set `core.autocrlf=false`, that precondition made explicit |
| FUD-D4-CHERRYPICKMERGE | P2 | the warning's later-`HEAD` clause | the same command, then "and before removing the pin check what it staged (`git diff --cached`), because a configured merge driver can make it succeed having applied none of the kept change." |
| FUD-D4-PINSAMEFAULT | P2 | R-D1 in §1.4, its §1.8 row, claim 2 (§1.8) | "When the ref store failed, a resume discards as today. One storage fault can do both: a common Git directory that cannot be written fails the snapshot's registration and then the pin's write, and a resume after writability returns discards the checkout."; the §1.8 row "one storage fault can cause it"; claim 2 "conditional on the pin's write". §2.11's R-D1 row reads "as §1.8" and follows |
| FUD-D4-WARNDELIVERY | P3 | `design/15`'s paragraph; the body | "When it reaches its lookup — a resume that refuses earlier, at its reclaim, names none, and one whose later step fails returns that error without its warnings — it looks for a kept pin at every attempt the run's log records, and names each one it finds"; the body promises no more |
| FUD-D4-REFUSALTIME | P3 | the body's time claims; §1.3's and §1.6's | "two 10-second retry budgets, the add's and then its cleanup's, plus each access's last attempt and its veto, which no deadline bounds"; a switch or a removal "refuses after its 10-second retry budget plus its last attempt's runtime" |
| FUD-D4-LATERDEP's claim (§11) | P1/P2 | §4.3, the recorded-list alternative | "It narrows R-OUTSIDE at gates and reviews to the dependencies that exist and are recorded when the list is taken: a dependency created later, such as the new `sharedindex.*` a gate's `update-index --split-index` writes, or one that disappears between the verification and the inventory, is outside it, so it is one more partial mitigation (FUD-D4-LATERDEP, CR-9)." The claim survives nowhere else in this record (searched for "whatever the configuration") |

FUD-D4-LATERDEP's C-SIDE form and FUD-D4-HEADRECREATE stay open with C-SIDE (O1), recorded in the external-prune
finding (§5.12).


### 5.10 The frozen proof, in the appendix's §8.5 form

`fudi/tools/frozen-proof.sh`, read-only `git`, at `cb1beacf` (`fudi/frozen/frozen-proof-cb1beacf.txt`); the commit after
it changes no file under `src/` or `effects/`:
- **D's own delta over G6's frozen set** (PR11's R-D list: 26 production files and 8 whole-file test children): from
  #329's head `54a1ff14` to `cb1beacf`, **34 of 34 byte-identical**, and from master `5c222ff2` likewise. **Zero.**
- **The cumulative comparison against G5's range `d724fb16`:** 4 paths, 241 insertions and 74 deletions —
  `src/engine/topology/recover/tests.rs` (+108 −26), `src/events/log/tests.rs` (+83 −33), `src/events/mod.rs` (+31 −12)
  and `src/topology/registry.rs` (+19 −3) — the same four as master's to G5, which are #322's and #327's. E-G6-1's
  two-tier rule, executed as the appendix's round 2 wrote it (`g5-rule.py prove`): **PASS**, enumerated differences 4,
  unenumerated 0. O12 is not adopted, so this is reported as the cumulative difference it is, master's known gap, and
  not as a clean module diff proof.
- **The PR5-frozen legacy section** (`FROZEN_LEGACY_ALLOWLIST`, 24 paths, unchanged): from `54a1ff14`, exactly D's named
  five differ — `src/workspace.rs`, `src/engine/attempt.rs`, `src/engine/coordinator.rs`, `src/engine/resume.rs` and
  `src/engine/tests.rs`. No other frozen file changes.
- **Schema 4 stays unreachable in production:** `TOPOLOGY_ACTIVATION` is `TopologyActivation::Inactive`
  (`src/topology/schema.rs:27`), and D changes no path under `src/topology/` or `src/engine/topology/`.
- **The two test files are append-only:** the engine tests at the merge are a byte prefix of `cb1beacf`'s (38,021 bytes
  appended), and every byte of `src/workspace.rs`'s test module before its closing brace is unchanged (18,794 bytes
  added before it) (`fudi/frozen/append-only-cb1beacf.txt`).

### 5.11 Platforms

- **Linux:** every figure here.
- **Windows and macOS, type-checked and linted only** (`fudi/platform/code-cb1beacf/`, at `cb1beacf`, each Checking line
  naming this worktree): `cargo clippy --target x86_64-pc-windows-msvc --all-targets --all-features -- -D warnings`,
  `RUSTFLAGS='-D warnings' cargo +1.85.0 check --target x86_64-pc-windows-msvc --locked --all-targets --all-features`,
  and `cargo clippy --target aarch64-apple-darwin --all-targets --all-features -- -D warnings`: each rc 0, with no
  warning. **Nothing ran on Windows or macOS here. CI is the truth for those legs** — `test (winguest)` runs the whole
  suite on the guest; the hosted Windows lane runs only in the merge queue.
- **What the tests do per platform.** T-L3's 300-byte name and T-L8's link and read-only-parent cases are `cfg(unix)`;
  every other case runs everywhere. Every registration a test plants is written as Git for Windows writes it (`/`).
- **Windows paths** (`fudi2/paths/BUDGET.txt`): the paths a probe printed on Linux at `36d50d2e`
  (`fudi2/paths/probe/test.log`), with the guest's `TEMP` (`C:\Users\Administrator\AppData\Local\Temp`, 41 characters)
  and its scratch roots' shape, `<TEMP>\upstroke-<tag>-<10>`, both read from the winguest job log at `20e27724`. The
  longest `$GIT_DIR` the new tests make is T-L4's snapshot registration: **149 characters** of Git's 220
  (`PATH_MAX - 40`) with a six-digit pid, 153 with a ten-digit one. A topology slot's administrative directory is 101,
  and its worktree path is **147 characters** before its file names; the longest snapshot checkout, an engine test's in
  the run's gate store, is 186 (190 with a ten-digit pid) before its file names, of `MAX_PATH`'s 260. *(Implementation,
  §5.16: E4. This bullet said 140 and 138: `fudi/wintime/GIT_DIR.txt` left out the nine-character `upstroke-` prefix that
  `ScratchTree::acquire` gives every scratch root, and T-L5 and T-L7, which it listed among the longest, make no
  snapshot.)*
- **CI's toolchain** (rustc 1.99.0, where this box has 1.97.1): the production code this change adds calls no recently
  renamed or deprecated API — `Result::inspect_err` (stable since 1.76), and `std::fs`'s `symlink_metadata`, `read_dir`,
  `remove_dir` and `canonicalize`.
- **The time budget:** §1.11, §2.14 and §3.13's estimates stand as upper bounds (round 3 removed the probe); none of it
  is executed on those legs.

### 5.12 The findings at this touch

**Filed** (`findings/README.md`'s form, by `findings/PROCESS.md`):
- **FUD-D4-ENV** (P1, `pre_existing`, its own file): the legacy `git_command` inherits `GIT_INDEX_FILE`. Its
  `reviewed_sha` is `9b2262f4`, the head round 4 reviewed; its executed consequences are the concurrency lens's
  inherited-index interleaving (the capture takes the base tree), the regression lens's snapshot with no `index` of its
  own (C-SIDE's false refusal), the design lens's re-check that reads another index, and CR-1's `add -A` into the main
  index; its guard is O3 (ENV-1, with decision B's widened `src/workspace.rs` text) or O3-R, before G6, which it blocks
  with the class (CR-1). This change does not take it up: no inherited environment changes.
- **`PR331-A-RESUME-THAT-STOPS-BEFORE-ITS-REPORT-NAMES-NO-KEPT-PIN`** (P3, R-D5 with FUD-D4-WARNDELIVERY's delivery
  gap): a resume refused at its reclaim, or failing after its lookup, names no kept pin.
- **`PR331-KEPT-PINS-ACCUMULATE-UNTIL-THE-OPERATOR-REMOVES-THEM`** (P3, R-D7). *(Implementation, §5.16: narrowed by
  E2 to refusals that recur after a successful reclaim.)*

**Extended rather than duplicated:**
- **`PR329-AN-EXTERNAL-PRUNE-DELETES-AN-ENGINE-WORKTREES-REGISTRATION`** gains a dated section, "At #331's
  implementation": face 1's legacy discard closed by D once merged (with its tests); face 2's legacy discards — R-D9,
  FUD-D2-RD9 and FUD-D3-DURINGADD — open, unchanged and filed there; and C-SIDE's open findings for the change that takes
  it up (FUD-D3-CSIDEPROOF, FUD-D4-HEADRECREATE, FUD-D4-LATERDEP's C-SIDE form, FUD-D4-ENV's false refusal).
- **The legacy finding itself is kept, updated, and not deleted.** The rule is that it goes if, and only if, this record
  says D wholly closes it (§1.12). It does not: claim 2 is "conditional on the pin's write" (§5.9, PINSAMEFAULT), and
  §1.8's (e1′) row "conditional on the pin being written". So the file keeps P1 and B's text, with its guard updated and
  a dated section: what D closes once merged, with the regression tests, and what remains, R-D1 — one storage fault can
  fail both the registration and the pin, and a resume after the store is writable again discards the checkout
  (executed at the Git level by the restore lens on the three Gits; the engine's consequence reasoned). *(Implementation,
  §5.16: and a `HEAD` moved from the captured parent, which refuses the pin, once `HEAD` is back at the head the event
  log records — E1, executed; the file gains a dated section for it.)* *(Implementation round 3, §5.17: R-D1's
  preservation design is implemented on this draft, conditional on O8; the file stays open until the merge, which waits on
  O8, #329 and the required reviews, and gains a dated section naming the engine witnesses.)*

**Not filed, because this change closes them** once merged:
- **with a regression test and a mutation** (§5.5 to §5.7): FUB-D5-INDEX, FUB-D5-RESTORE, FUB-D4-B1PREDICATE,
  FUB-D4-B1REMOVE, FUB-D4-RESUME, FUD-D1-REPLACE, FUD-D1-PINWARN, FUD-D2-PROBECONFIG, and FUD-D2-PRUNE for a deletion
  that fails the add;
- **by construction:** FUD-D2-HOOKS (the veto starts no Git child; T-L8 calls it where no Git runs) and FUD-D2-TL2
  (T-L2 asserts no attempt count);
- **by text:** FUB-D5-UNFREEZETEXT (§5.3's texts), FUD-D1-PROGRESS (§2.4's claim) and FUD-D3-TAKEOVERWORD (by (i),
  proposed).

**R-G** (`FUC-D2-RG`) is graded P1 and not regraded. What this implementation shows is that every Git child the
module builds passes the four settings (T-L7, red at master and under `m-maint`); that they stop the maintenance a lazy
fetch one of those children starts is the design's rg2 (§2.5, on three Gits), which is not re-run here (§5.13). Its
closure waits on the reviewers' confirmation. The five §10.3 items are text corrections, not filings (§5.9).

**Already filed elsewhere, and referred to:** R-D10 (`PR11-HOST-AGENT-PRUNE-RACES-AN-ENGINE-ADD` for agents, #330's
R-GU for the user); R-D9 (above). R-D2, R-D3, R-D4, R-D6, R-D8, R-D11 and R-D12 stay stated in §1 to §4 as residuals of
the design, as before; none is accepted.

### 5.13 What waits, and what is not verified

- **O8.** Nothing here is in force until the owner adopts O8 and this pull request merges. The texts are proposed.
- **#329's merge**, which needs O9, O14 and O11. D calls its helper, so D merges after it.
- **The reviews.** This implementation is unreviewed. The orchestrator runs two lenses once CI is green on every leg.
  *(Implementation, §5.16: two lenses read `20e27724` early, before CI was green on every leg; their review is early
  evidence and clears nothing for the merge. The required regular and regression lenses run on D's head integrated
  with #329's repaired head, once that is green on every leg.)*
- **R-G's grade** stays P1. What is shown here is T-L7 and its mutation; with the design's rg2, which is not re-run,
  that is the closure the reviewers are asked to confirm.
- **Windows and macOS** are CI's to show (§5.11). The hosted Windows queue lane runs only in the merge queue.
- **Not executed here:** the R-G engine witnesses rg1 to rg3 (design evidence: they need a trace2 directory
  process-wide), the git-wrapper witnesses v1, v1c, v1n, v2, v3 and p1 to p5 (design evidence: a wrapper first on
  `PATH`, process-wide). The wrappers' decisions are carried by T-L3, T-L8 and T-P1, as §2.8 and §3.7 planned; of
  R-G's, T-L7 carries the settings, not their effect on a lazy fetch.

### 5.14 What passes only through proposed instruments

The one instrument this change touches is `effects/allowlist.toml`: the five `legacy_effect` texts, which are
**proposed, conditional on O8**. No other instrument moves (no row, no `path`/`allows`/`packet`/`shrinks_when`, no
`effects/wrappers.toml` row, no `clippy.toml`, `Cargo.toml` `[lints]`, `.cargo/`, `.github/` or `scripts/` change, and
no CI-contract test under `src/effects/`).

**What passes only through them, executed** (`fudi/tools/allow-probe.py`, `fudi/instruments/allow-probe.txt` and
`allow-probe-summary.txt`). Two scratch trees, from the base `255f67b8` and from `cb1beacf`, each with the module-level
`#![allow(...)]` of the five files replaced by an empty line and nothing else, linted with `cargo clippy --keep-going
--all-targets --all-features -- -D warnings`: 539 denials at the base and 598 at the head; **62 lie on lines this change
adds**.
- **Gate 2, `cargo clippy --all-targets --all-features -- -D warnings`,** passes over those 62 lines only through the
  five files' module-level allows, which the five rows admit; the rows' `allows` are unchanged, and what reviews this
  change's effects into them is the five proposed texts.
  - **Production, 5:** two new effects — `std::fs::remove_dir` in `legacy_add_veto` (`src/workspace.rs:1680`) and
    `Workspace::prepared_pin_target`, denied by path, in the resume's lookup (`src/engine/resume.rs:569`) — and three
    existing calls on lines this change rewrote: the add's and the removal's `Command::output`, now inside their
    attempts (`src/workspace.rs:925`, `:1607`), and `run_attempt`'s call with its new argument
    (`src/engine/coordinator.rs:547`).
  - **Test code, 57,** in `src/engine/tests.rs` and `src/workspace.rs`'s test module: `run_with` 15, `resume_harness`
    1, `std::fs::write` 15, `create_dir` 7, `create_dir_all` 4, `remove_dir_all` 3, `remove_file` 2,
    `set_permissions` 2, `std::process::Command` 3 with `output`, `spawn` and `status` one each, `write_all` 1, and
    `std::os::unix::fs::symlink` 1.
- **Gate 3's effects censuses** `every_allow_of_a_governed_lint_is_module_level_and_in_the_allowlist` and
  `every_allowlist_entry_carries_its_justification_and_names_a_real_file` pass with the five proposed texts in the file;
  the second reads each text's `LEGACY-EFFECT` marker, which each keeps.
- **Not dependent on them:** the 25 new tests compile and pass without any allow, because `cargo test` runs no clippy
  lint; the other gates read nothing of the texts.

**Stated plainly: no check reads the texts' new words.** The one reader of the field (`src/effects/tests.rs:3186`)
asserts the `LEGACY-EFFECT` marker; no gate script reads the file, and `clippy.toml` names it only in comments.
Executed (`fudi/tools/master-texts.sh`, `fudi/instruments/master-texts/`): at this head's code with the base's
`effects/allowlist.toml`, master's five texts, the effects tests pass, 211 of 211, the allow-placement census and the
justification census among them. That is exactly why the texts matter and why they are not the gates' to grant:
without O8, this change's edits to the five PR5-frozen files are unreviewed widenings of frozen modules, whatever the
gates report. The canonical packet and every existing owner grant are unchanged.

### 5.15 The gates

The ten gates of `CLAUDE.md` run at the head this pull request is pushed at, through `upstroke-build` on this lane's
private base, in the foreground. A record cannot carry the result of a run on the commit that contains it, so the pull
request body records that head and the result, with the logs under `fudi/gates/`.

### 5.16 Repair round 2: the early review's E1 to E4

> **On §5's terms:** a draft, PROPOSED, conditional on the owner's decision O8, and not merge-ready. Its merge waits on
> O8's adoption and on #329 merged (O9, O14, O11). Nothing here adopts an owner decision. The round adds commits on
> `20e27724`, with no rebase, and no `reviewed_sha` is re-stamped.

**Who and on what.** `pr11_fud_impl2` (`claude-opus-5-5`, `max`), a fresh repairer `orch_pr11` spawned on 2026-10-03 on
`20e27724`. Its brief is `~/orch-pr11/briefs/pr11_fud_impl2.md`; round 1's brief, answer and unit bind where it does not
change them. Its scope is exactly items E1 to E4 of the early review's triage
(`~/orch-pr11/reviews/review-331-i1-early-triage.md`). Its figures are under `fudi2/…`.

**CI at `20e27724`** (run 37138196522, `fudi2/ci/jobs-37138196522.tsv`): every job passed but `test (winguest)` (job
111246977272) and the `upstroke-ci` rollup that follows it. The one failing test is #329's own
`workspace_manager::tests::concurrent_snapshot_adds_and_removals_on_one_repository_never_fail` (Windows, os error 5;
`fudi/ci/failed-job-111246977272-winguest.log:3147`, its result at `:3220`: 2,866 passed, 1 failed). **It is #329's,
routed to #329 as R7** for its next fresh round; its read site and first-bad commit are unproved, and D takes #329's
repaired head later, by a merge. On that leg all 25 of D's tests passed (`fudi2/e3/winguest-20e27724-d-tests.txt`).
The pull request policy run at that head passed (`fudi2/ci/runs-20e27724.json`).

**The early review** (`~/orch-pr11/reviews/review-331-i1-{regular,regression}-20e27724.review.md`): two lenses, both
CHANGES_REQUIRED with no P1, run before CI was green on every leg. They are early evidence and clear nothing for the
merge; the required lenses run later, on D's integrated head (§5.13). The witness files this round used check against
`~/orch-pr11/reviews/SHA256SUMS-331-i1-early-witnesses`.

**E1 (P2): the moved-`HEAD` safety claim, corrected. The loss stays in R-D1, at P1.**
- **Reproduced first,** at `20e27724`, from the regular lens's witness source verbatim, appended to a `git archive` tree
  (`fudi2/repro/e1e2/`, 3 of 3; the tool is `fudi2/tools/repro_e1e2.py`). The event log records `HEAD` A. The worker's
  adapter moves the branch to B before the capture, so the capture records parent B. The after-capture hook resets the
  branch to A and tears a registration, and the snapshot is refused. The pin is refused too: its error names the
  captured branch at B and `HEAD` at A. The first resume compares `HEAD` with A, passes, and discards
  `agent-output.txt`, with no pin and no warning. The second shape, `HEAD` moved after the capture and put back as the resume's refusal advises,
  ends the same way.
- **Corrected where the claim stood:** the legacy finding (its R-D1 bullet in place, a dated section with both sequences
  and their witnesses, and its guard), §1.4's residual, §1.8's R-D1 row, §5.12, and the body. `design/15`'s paragraph
  never made the claim (it says every resume discards the checkout's copy), so it is unchanged.
- **What correcting the claim does not do.** It closes neither this case nor R-D1, an existing P1, and it implies no
  preservation waiver and no G6 clearance. R-D1 stays P1 in the legacy finding, which now carries this case and its
  witness, so the case stays in the preservation follow-through. **A faithful preserving mechanism is still required,
  or a concrete owner decision.** The PR11 orchestrator tracks it as a required item: a design round for R-D1's
  preservation, commissioned after this repair.
- **No small, reviewed-design-faithful change keeps this output.** One candidate was executed, in scratch trees only
  (`fudi2/repro/K1-SUMMARY.txt`; the tool is `fudi2/tools/k1.py`). K1 writes the kept pin whatever `HEAD` is:
  `prepare_commit_from_candidate` skips its `HEAD` comparison for a `-kept` pin. It keeps the output in both sequences
  — the pin holds the captured tree and the resume names it — where at this head both of its witnesses fail. It is not
  D's design:
  - it changes `Workspace::prepare_commit_from_candidate`, which D's proposed `src/workspace.rs` text does not name
    (§3.8: "two things and no more"), and whose `HEAD` refusal §1.4 takes into the design and R-D1 names;
  - it turns D's own T-P8 red;
  - it leaves R-D1's ref-store shape as it was.

  It is input for the commissioned design round, and nothing of it is on this branch.

**E2 (P3): R-D7 narrowed.** Reproduced at `20e27724` (`fudi2/repro/e1e2/`). With the tear left unrepaired, two resumes
each refuse at their reclaim (`reclaim_gate_workspaces`, `src/engine/resume.rs:426`), before the worker and before the
pin lookup (`:562`). The event log is unchanged, and exactly one pin survives.
- `PR331-KEPT-PINS-ACCUMULATE-UNTIL-THE-OPERATOR-REMOVES-THEM` now files only refusals that recur after a successful
  reclaim, with this evidence:
  - a checkout the snapshot cannot make after Git's takeover, whose cleanup succeeds (§3.2's v3);
  - a tear made again during the resumed attempt (T-P13).
- Its per-resume lookup claim carries R-D5's qualification, and the body says the same.
- §3.2 and §3.10 speak only of the post-takeover failure, and they stand.

**E3 (P2): T-L5 times no Git child it observes** (`36d50d2e`, test code only; `fudi2/e3/TABLE.txt`).
- **Why not a handshake inside one access.** The access consults its deadline after every failed attempt, before the
  next (#329's contract, step 6, `src/workspace_manager.rs:1753`). So whatever the test waits for between a failed
  attempt and the next one is timed by that deadline. The only wait it never measures is one inside an attempt that
  then succeeds, and an attempt is `switch_branch`'s production closure; no test here needs a seam in production code
  (§5.5).
- **So T-L5 runs the access twice.**
  - The first access meets the tear unrepaired and refuses as a registry refusal, having attempted again: the
    handshake's count moved.
  - `HEAD` and the status are read after it returns, with no access running, so no deadline times
    `git symbolic-ref HEAD` or `git status`.
  - The second access's writer finishes the tear only after that access has failed on it once (`OnceContended`, the
    `CONTENDED_ATTEMPTS` handshake), with two file writes and no Git child.
  - The assertions stay strict: a refusal and never Git, then success, and `HEAD` on `other`.
- **Executed,** each in a `git archive` tree of `36d50d2e` whose Compiling line names it, with the regression lens's
  700 ms wrapper, hash-identical (`fudi2/e3/wrapper/git`):

| Shape | 700 ms wrapper | Result |
|---|---|---|
| control, D's 25 new tests | no; yes | 25 passed; 25 passed |
| T-L5 alone, three runs each | no; yes | passed, 0.52 s to 0.53 s; passed, 1.44 s to 1.45 s |
| `m-inwindow`: T-L5 as `20e27724` has it, the handshake removed | no; yes | passed; **failed** at `src/workspace.rs:4310`, a registry refusal after 14 attempts |
| `m-early-repair`: the tear finished before the second access | no | **failed**: "the switch failed on the tear at least once" |
| `m-switch-unwrapped` (§5.7) | no | **failed**: a Git error, not a registry refusal |
| `base-firstbad` (§5.6) | no | **failed**: a Git error, not a registry refusal |

- **What still bounds it** is what bounds every `OnceContended` test (§5.5): the second access's own attempts run within
  #329's 500 ms test deadline.

**E4 (P3): the Windows path budget, recounted** (`fudi2/paths/BUDGET.txt`, from an executed probe,
`fudi2/paths/probe/test.log`).
- §5.11 now gives:
  - **149 characters of 220** for the longest `$GIT_DIR`, T-L4's registration (153 with a ten-digit pid);
  - **147 of 260** for a topology slot's worktree, before its file names;
  - 186 (190) for the longest snapshot checkout.
- Round 1's 140 and 138 left out the `upstroke-` prefix.
- No other path-length figure is in this record (`fudi2/paths/record-search.txt`, a search for `MAX_PATH`,
  `PATH_MAX`, `GIT_DIR`, "characters", 220 and 260): §1.9 names the 220 budget without a figure, and §3.2 names the
  platforms' limits without one.
- CI's `test (winguest)` ran every new test green at `20e27724` (above).

**The re-run, the frozen sets and the instruments.**
- **Re-run.** T-L5 is the only test this round changes, and nothing else under `src/` or `effects/` moves. The mutations
  that kill it in §5.7, `m-switch-unwrapped` and `base-firstbad`, are red on it again, and the control is 25 of 25
  green (the table above).
- **The frozen sets** (`fudi2/frozen/`): G6's 34 frozen files are byte-identical from #329's head `54a1ff14` and from
  master `5c222ff2`. The PR5-frozen legacy section differs from `54a1ff14` in exactly D's five files. Both test files
  are still append-only against the merge `255f67b8`.
- **Instruments:** none changes. The five proposed texts are byte-identical to `20e27724`'s.
- **Notes:** `docs/internals/workspace.md`'s T-L5 section follows the test. The module gains no comment.

**What waits, and what is not verified here.**
- CI on every leg is not waited on.
- The early review is not re-run; the required lenses run later, on D's integrated head.
- Windows and macOS execution is CI's.
- R7 is #329's.
- O8 stays the owner's.

The pull request body records the head the ten gates passed at, with the logs under `fudi2/gates/`.

### 5.17 Implementation round 3: R-D1's preservation design, round 4 (K, G4 and N), N1 and N2, and B's head merged provisionally

> **On §5's terms:** a draft, PROPOSED, conditional on the owner's decision O8, and not merge-ready. Its merge waits on
> O8's adoption, on #329 merged (O9, O14, O11), on the integration of #329's *final* head, and on the required reviews:
> **the regular review of R-D1's design is unmet** (its routing held and unassigned), and this draft does not clear it.
> Nothing here adopts an owner decision, a contract or a freeze. The round adds four commits on `88376a10` — the
> provisional merge `47e62ee5`, the implementation `45b1a7b1`, the test revision `a4972d09` and this record's commit —
> with no rebase, and no `reviewed_sha` is re-stamped.

**Who and on what.** `pr11_fud_impl3` (`claude-opus-5-5`, `max`), a fresh implementation session `orch_pr11` spawned on
2026-10-04 on `88376a10` (CI run 37152667513 green on every job, `fudi3/ci/jobs-37152667513.tsv`). Its brief is
`~/orch-pr11/briefs/pr11_fud_impl3.md`; round 1's and round 2's briefs and `~/orch-pr11/d-impl/UNIT.md` bind where it
does not change them. Its authority is the supervisor's scheduling determination
`/home/ubuntu/babysit-pr11/evidence/d-r4-implementation-authority-20261004.md` (sha256 `26a15e96…`), not a new owner
grant. The design is `~/orch-pr11/owner-package/RD1-PRESERVATION-PROPOSAL.md`, sha256
`f9e81c078dc4be62e1c4cda24563ecb061e3971b1b06e438cc63a85a09a95415` (round 4; its regression lens PASSED with no
finding, `~/orch-pr11/reviews/review-rd1r4-f9e81c07-triage.md`). Its figures are under `fudi3/…`. The stopped round-2
regular review of R-D1's design was not read, opened, run, used or listed, nor its parent directory; nothing here
retries, rewords, reroutes or relabels it.

#### 5.17.1 #329's published head, merged provisionally

- **`47e62ee5`** merges #329's published draft head `550296284ffdcf02b7e6e4cde70d202c4caceb76` (parents `88376a10` and
  `55029628`), a merge and not a rebase. **It is PROVISIONAL:** #329's step-5 witness work and CAS-1 remain open, so
  this is not #329's final head. A later step integrates #329's actual final code and text, with its validation, native
  CI and review; this merge cannot count as that final integration. #329's CI at `55029628` (run 37186359003) passed
  every job (`fudi3/ci/jobs-37186359003-B.tsv`).
- **No textual conflict** (`fudi3/merge-preview.txt`): #329's delta since `54a1ff14` touches none of D's legacy files.
- **One semantic conflict, resolved in the merge** so that it compiles. #329's repair rounds 3 and 6 (`eef97e41`, its
  R1; `22d70ef6`, its I2-1) give `tolerant_registry_access` a `pause_for` argument, the wait between attempts that the
  access used to sleep itself, so D's three legacy calls no longer compiled. They now pass a private
  `legacy_registry_pause`, a sleep on the calling thread, which is the wait an access with no hooks makes
  (`EffectHooks::registry_pause`'s default): they wait as before. D's proposed `src/workspace.rs` text gains one clause
  naming it ("and one more sleeps out, on the calling thread, each pause the three accesses wait between their
  attempts"), proposed and conditional as the rest, and `docs/internals/workspace.md` a section.
- **At the merge:** `cargo clippy --all-targets --all-features -- -D warnings` and `cargo fmt --check` rc 0, and `cargo
  test --all-targets --all-features` 3,164 passed, 0 failed, 130 ignored, and the binary's 10
  (`fudi3/build/test-merge-2.log`, `clippy-merge-2.log`, `fmt-merge-2.log`; `MERGE-STATE.txt`).
- **What #329's head brings that is #329's own:** its proposed frozen hunk H1 in `src/engine/topology/integrate.rs`, a
  G6-frozen file (#329's `e369b251`, H2 withdrawn by `0edfc509`), conditional on #329's freeze ruling and not adopted.
  It is on this branch only through the provisional merge (§5.17.8).

#### 5.17.2 What it implements, and how faithfully

- **Two code commits.** `45b1a7b1` implements the design; `a4972d09` revises four of its tests after the first full
  mutation pass (§5.17.3), test code and its notes only. The final code is `a4972d09`'s, and every figure below is at
  it unless it says otherwise.
- **The design's two patches, applied as they are:** `rd1r4-proposed.patch` (sha256 `5b7a5e5b…`) and
  `rd1r4-instruments.patch` (`06dedba6…`), `fudi3/patch-inputs.sha256`. Applied to the merge's files, each designed file
  is the implementation's byte for byte — `src/engine/coordinator.rs`, `effects/wrappers.toml`, `clippy.toml` and
  `src/runner/contract.rs` — or differs only as stated here (`fudi3/patch/FIDELITY.txt`):
  1. **one compile fix:** `canonical_common_dir`'s `Workspace` literal, a fourth literal (D's own, `cb1beacf`) that the
     design's census of three missed, gains `private_index: None`;
  2. **rustfmt**, layout only, in `src/workspace.rs` and `src/engine/resume.rs` (`fudi3/build/rustfmt-*.diff`);
  3. **`with_private_index`'s two-line comment moved** to its section of `docs/internals/workspace.md`: a module with a
     notes file carries no other comment (`CODING_STANDARDS.md` §13);
  4. `src/engine/tests.rs`: the designed file (the design's T-P7, T-P7b and T-P8 hunks and its helper
     `block_the_kept_pin_and_tear_after_capture`) is a byte prefix of the implementation's, the new tests appended after
     it; `src/workspace.rs`'s test module keeps every designed byte before its closing brace, with T-K1 after them.
- **Part K** (`src/workspace.rs`): `prepare_commit_from_candidate` observes and compares `HEAD` only for a pin whose
  name does not end with `KEPT_PIN_SUFFIX`, which moves here (`pub(crate)`); the coordinator and the resume import it.
- **Part G4** (`src/workspace.rs`, `src/engine/resume.rs`): with exactly one attempt in flight, the resume removes the
  checkout's uncommitted paths only through `Workspace::discard_into_kept_pin`: a capture into a private index file of
  the checkout's own Git directory, named by `GIT_INDEX_FILE` with `-c core.splitIndex=false -c
  splitIndex.sharedIndexExpire=never` (`PRIVATE_INDEX_CONTROLS`, round 4's RD3-1), every `diff-tree` on that private
  index; the footprint of what stands in the revert's way, added with `-f`; the attempt's copies read first, a missing
  pin put back from one (`git bundle unbundle`, a create-only `update-ref`), a pin and a copy that disagree refused; a
  fresh copy written by this resume — `.partial`, `list-heads`, `sync_all`, rename, the directory's `sync_all` —
  whatever copies exist (round 4's RD3-2); the two-phase revert (`read-tree -m -u R T`, then `checkout-index` with no
  `-f`); the index reset; the post-check. Any failure refuses, discarding nothing the pin and its durable copy do not
  hold. The moved-branch refusal names `git update-ref refs/heads/<branch> <recorded>`.
- **Part N** (`src/engine/coordinator.rs`): every attempt error after the worker ran (N1) and every publication failure
  of a reviewed candidate (N2: its branch not the run's, its prepared commit refused, its settlement not appended) keeps
  the checkout and pins what it holds (`kept_on_error`; `Workspace::pin_checkout` for N1), returning the error with a
  warning naming the pin or its failure.
- **The O8 texts** (`effects/allowlist.toml`; `fudi3/texts/apply_o8_texts.py`): the proposal's §8.2 with round 2's
  §6.2 where §8.2 keeps it — `src/workspace.rs`'s four edits (its third paragraph as round 4 gives it),
  `src/engine/coordinator.rs`'s and `src/engine/resume.rs`'s replacement paragraphs, and `src/engine/tests.rs`'s
  amendment — each still under D's TOML comment "PROPOSED, conditional on the owner's decision O8 (decision B), and not
  granted"; `src/engine/attempt.rs`'s text and every `path`, `allows`, `packet` and `shrinks_when` unchanged; the
  proposal's `--` rendered as the file's em dash. **`design/15`:** round 2's four places, the discard sentence as round
  4 gives it (§8.3), the PROPOSED marking kept; no sentence `src/export.rs` pins moves.
- **The three instrument rows** (`rd1r4-instruments.patch`, byte for byte): `Workspace::pin_checkout` and
  `Workspace::discard_into_kept_pin` in `effects/wrappers.toml`'s `effectful` list for `src/workspace.rs`, with the
  block comment, and their two `clippy.toml` denials; and `src/runner/contract.rs`'s payload census row for
  `src/workspace.rs`, five `.env(` to seven. **They are first-limb instrument rows under the standing delegation, as the
  supervisor reads it, and not an owner adoption.** The canonical packet and every adopted grant are unchanged.

#### 5.17.3 The tests

Each test waits on a handshake, never time alone: an after-capture hook, a child's exit, or a `git` shim's own marker.
Every call into the legacy engine is bounded (`bounded`, `bounded_child`: 300 s), so a mutation that wedges fails rather
than hangs. No test needs a seam in production code. The rows are the proposal's §10.1 tables (round 4's, round 3's and
round 2's with round 1's T-K1 and T-R1 to T-R12); "added" rows are this round's, for R-D1 cases and controls the tables
left to the harness. "D's head" is the first-bad shape `base-firstbad` — the four legacy modules as the merge
`47e62ee5` has them, the implementation's tests kept (`KEPT_PIN_SUFFIX` spelled `"-kept"`, its home at D's head being
private) — and the second base is the earlier design round's patch, `patch -p1` on the merge's modules, compiled with
the implementation's tests (round 1's for rounds 1 and 2, round 2's for round 3, round 3's for round 4; an added row's
is in the table). The campaign ran at
`a4972d09` (`fudi3/mutation/`, one directory per shape with its diff, log and summary; `fudi3/tools/campaign.py`;
`fudi3/record/TESTS.md`, `MUTATIONS.md`, `COVERAGE.txt`).

**A first full pass, superseded** (`fudi3/mutation-pass3-superseded/`, its `SUPERSEDED.txt`; two earlier partial passes
are `mutation-pass1-superseded/` and `mutation-pass2-superseded/`). Run at `45b1a7b1` and read against the design's
rows, it found four tests asserting more, or other, than their rows, and two transforms not doing what their
definitions say. `a4972d09` revises the tests:
- **T-GRESET** also required the first resume to be a resume refusal, which D's head ends as a Git error from
  `git clean`; its row asserts only the second resume.
- **T-X18** required a kept pin; its row asserts that `cache.bin` stays and that no pin holds it.
- **T-X20** let its first resume die inside the resumed attempt, so its second resume met attempt 2. The row's two
  resumes in a row meet one attempt, and only then does round 1's exact rule see the nested repository it wedges on.
- **T-R9** compared the checkout with the pin after the restore, which a pin of `HEAD`'s tree also satisfies; round 1's
  row has the resume pin the leftovers.

The transforms, corrected in `fudi3/tools/campaign.py` (the pass used `campaign-before-fidelity3.py`):
- **`shared-index`** kept G4's footprint, where the harness's mutation of that name takes none;
- **`m-g-trust`** dropped only the pin's identity check, where round 1 defines it as trusting any ref at the kept name,
  content included.

The pass below is the whole campaign again, at `a4972d09`.

| Row | Test | Case | Round | D's head (first-bad): predicted / actual | Second base: predicted / actual | Killing mutations (actual) |
|---|---|---|---|---|---|---|
| T-K1 | `a_kept_pin_is_written_whatever_head_is_and_a_publication_pin_is_not` | A, K's scope | 1 | red / red | R1: green / green | `k-headcheck`, `m-k-all`, `m-k-observe-only` |
| T-R1 | `a_branch_moved_before_capture_and_back_keeps_the_output_through_the_first_resume` | A1, A1t | 1 | red / red | R1: green / green | `k-headcheck`, `m-add-returns`, `m-discard`, `m-g-parent-is-head`, `m-index`, `m-removepin`, `m-restore` |
| T-R2 | `a_branch_moved_before_capture_and_reset_hard_back_keeps_the_output_in_the_pin` | A1h | 1 | red / red | R1: green / green | `k-headcheck`, `m-add-returns`, `m-discard`, `m-index`, `m-removepin` |
| T-R3 | `a_branch_moved_after_capture_keeps_the_output_pinned_at_its_captured_identity` | A2, A2h | 1 | red / red | R1: green / green | `k-headcheck`, `m-add-returns`, `m-advice`, `m-discard`, `m-index`, `m-removepin` |
| T-R4 | `a_kept_pin_is_written_whatever_head_is_when_the_snapshot_is_refused` | A3 to A6 | 1 | red / red | R1: green / green | `k-headcheck`, `m-add-returns`, `m-discard`, `m-index`, `m-k-observe-only` |
| T-R5 | `one_storage_fault_that_refuses_the_snapshot_and_the_pin_loses_nothing` | B1 | 1 | red / red | R1: green / green | `m-add-returns`, `m-discard`, `m-g-after-discard`, `m-g-none`, `m-index`, `m-removepin` |
| T-R6 | `a_resume_refuses_while_the_ref_store_cannot_take_the_kept_pin` | B2 | 1 | red / red | R1: green / red | `m-add-returns`, `m-discard`, `m-g-discard-anyway`, `m-g-none`, `m-pinfail-discards`, `m-removepin` |
| (B3, B4) | `a_store_fault_at_the_pins_objects_or_its_reflog_loses_nothing` | B3, B4 | added | - / red | R1: - / red | `m-g-after-discard`, `m-g-none`, `m-pinfail-discards`, `m-removepin`, `n1-discard`, `n1-nopin` |
| T-R7 | `a_resume_writes_the_kept_pin_a_blocked_name_refused_and_refuses_while_it_is_blocked` | C1, C2, C4 | 1 | red / red | R1: green / red | `m-add-returns`, `m-discard`, `m-g-after-discard`, `m-g-discard-anyway`, `m-g-none`, `m-pinfail-discards`, `m-removepin` |
| T-R8 | `a_foreign_ref_at_the_kept_name_is_neither_trusted_nor_named_as_the_output` | C3 | 1 | red / red | R1: green / red | `m-add-returns`, `m-discard`, `m-g-after-discard`, `m-g-discard-anyway`, `m-g-none`, `m-g-parent-is-head`, `m-g-trust`, `m-pinfail-discards`, `m-removepin` |
| T-R9 | `a_run_that_dies_after_its_capture_loses_nothing_on_resume` | E1 | 1 | red / red | R1: green / green | `m-g-after-discard`, `m-g-none`, `m-removepin` |
| T-R9 (E2) | `a_run_that_dies_between_its_pins_commit_and_its_ref_loses_nothing_on_resume` | E2 | 1 | red / red | R1: green / green | `m-g-after-discard`, `m-g-none`, `m-removepin` |
| T-R10 | `a_kept_pin_removed_before_the_resume_is_written_again_from_the_checkout` | F1, F2 | 1 | red / red | R1: green / green | `m-add-returns`, `m-discard`, `m-g-after-discard`, `m-g-none`, `m-removepin` |
| T-R11 | `a_resume_refuses_while_the_checkout_holds_more_than_its_kept_pin` | X3 | 1 | red / red | R1: green / red | `m-add-returns`, `m-discard`, `m-g-discard-anyway`, `m-g-none`, `m-g-subject-only`, `m-g-trust`, `m-removepin` |
| T-R12 | `leftovers_with_no_attempt_in_flight_are_discarded_as_before` | X2, the control | 1 | green / green | R1: green / green | `m-g-every-leftover` |
| T-RD1-1 | `a_foreign_index_reset_during_the_resumes_capture_cannot_change_the_kept_tree` | RD1-1 | 2 | red / red | R1: red / red | `m-add-returns`, `m-discard`, `m-g-after-discard`, `m-g-none`, `m-pinfail-discards`, `m-removepin`, `shared-index` |
| T-RD1-2 | `an_assume_unchanged_entry_holding_output_is_kept_before_the_discard` | RD1-2 | 2 | red / red | R1: red / red | `from-real-index`, `m-add-returns`, `m-discard`, `m-g-after-discard`, `m-g-none`, `m-pinfail-discards`, `m-removepin`, `shared-index` |
| T-RD1-3 | `the_output_survives_a_pin_deleted_after_the_resumes_last_pin_check` | RD1-3 | 2 | red / red | R1: red / red | `m-add-returns`, `m-discard`, `m-g-none`, `no-copy`, `reset-clean` |
| T-RD1-4 | `an_ignore_rule_that_changes_with_the_discard_exposes_nothing_to_it` | RD1-4 V1, V3 | 2 | red (V3) / red | R1: red / red | `m-g-none`, `reset-clean` |
| T-RD1-5 | `an_interrupted_text_write_does_not_stop_the_resume` | RD1-5 | 2 | red (lost) / red | R1: red / red | `m-g-after-discard`, `m-g-none`, `m-removepin`, `review-diff` |
| T-GRESET | `a_discard_stopped_part_way_finishes_on_the_next_resume_without_operator_cleanup` | G-RESET | 2 | green / green | R1: red / red | `equality`, `m-add-returns`, `m-discard`, `m-removepin`, `no-post-check` |
| T-N1a | `an_undecodable_diff_keeps_the_output_pinned` | N1, RD1-5c | 2 | red / red | R1: red / red | `n1-discard`, `n1-nopin`, `review-diff` |
| T-N1b = T-P7 | `an_attempt_error_that_is_not_a_registry_refusal_keeps_the_output_pinned` | N1 | 2 | red / red | R1: red / red | `n1-discard`, `n1-nopin` |
| T-N1c | `a_branch_moved_during_the_capture_keeps_the_output_pinned` | N1a, N1ah | 2 | red / red | R1: red / red | `m-g-parent-is-head`, `m-removepin`, `n1-discard`, `n1-nopin` |
| (N1d) | `an_error_after_the_worker_wrote_and_before_its_capture_keeps_the_output_pinned` | N1d | added | - / red | R1: - / red | `n1-discard`, `n1-nopin` |
| T-N1e = T-P7b | `a_snapshot_failure_that_is_not_the_registrys_keeps_the_output_pinned` | N1, R-C-RELSTORE's shape | 2 | red / red | R1: red / red | `m-keepall`, `n1-discard`, `n1-nopin` |
| (N1e) | `an_earlier_attempts_retained_output_is_kept_when_the_next_worker_cannot_start` | N1e | added | - / red | R1: - / red | `n1-discard`, `n1-nopin` |
| T-N2a | `a_reviewed_candidate_refused_on_a_moved_head_is_kept_and_pinned` | N2a, N2ah | 2 | red / red | R1: red / red | `k-headcheck`, `m-k-all`, `n2-discard` |
| T-N2b | `a_candidate_captured_on_another_branch_is_kept_and_pinned` | N2c | 2 | red / red | R1: red / red | `n2-discard` |
| T-N2c | `a_reviewed_candidate_whose_settlement_is_not_appended_is_kept` | N2d | 2 | red / red | R1: red / red | `m-removepin`, `n2-discard` |
| T-X10 | `an_unreadable_leftover_and_a_nested_repository_are_left_in_place_and_named` | X10, N1b2 | 2 | red / red | R1: red / red | `m-g-after-discard`, `m-g-none`, `m-removepin`, `n1-discard`, `no-ignore-errors`, `reset-clean` |
| T-X16 | `an_operators_new_file_after_a_pinned_refusal_stays_in_the_checkout` | X16 | 2 | red / red | R1: red / red | `equality`, `m-add-returns`, `m-discard`, `m-g-none`, `no-leave-in-place`, `reset-clean` |
| T-X18 | `the_discard_reads_the_checkouts_per_worktree_ignore_rules` | X18 | 2 | green / green | R1: green / green | `no-worktree-config` |
| T-X19 | `a_branch_moved_during_the_discard_refuses_with_the_output_kept` | X19 | 2 | green, no refusal / red | R1: green, no refusal / red | `m-add-returns`, `m-discard`, `m-g-discard-anyway`, `m-g-none`, `no-copy`, `no-head-check`, `reset-clean` |
| T-X20 | `a_nested_repository_with_a_commit_does_not_wedge_the_resume` | X20 | 2 | green / green | R1: red / red | `equality`, `revert-gitlinks` |
| (X4) | `every_kind_of_change_is_pinned_and_an_ignored_file_survives` | X4 | added | - / red | R1: - / green | `m-g-after-discard`, `m-g-none`, `m-removepin`, `m-restore` |
| (X6) | `a_resume_that_dies_before_its_revert_is_finished_by_the_next` | X6, C6 | added | - / red | R3: - / red | `copy-overwrite`, `m-add-returns`, `m-discard`, `m-g-none`, `m-removepin`, `no-copy`, `reset-clean`, `reuse-copy` |
| (X17) | `a_leftover_whose_name_is_not_utf8_is_pinned_then_reverted` | X17 | added | - / red | R1: - / green | `m-g-after-discard`, `m-g-none`, `m-removepin` |
| T-RD2-1 | `a_retry_after_a_deleted_pin_restores_it_from_the_untouched_copy` | RD2-1 | 3 | red / red | R2: red / red | `copy-overwrite`, `equality`, `m-add-returns`, `m-discard`, `m-g-discard-anyway`, `m-g-none`, `m-removepin`, `no-copy`, `no-post-check`, `no-recover`, `reset-clean` |
| T-RD2-1b | `the_output_survives_its_pin_deleted_after_the_restore` | RD2-1b | 3 | red / red | R2: red / red | `copy-overwrite`, `equality`, `m-add-returns`, `m-discard`, `m-g-discard-anyway`, `m-g-none`, `no-copy`, `no-post-check`, `no-recover`, `reset-clean` |
| T-RD2-1m | `a_kept_pin_moved_from_its_copy_refuses_and_discards_nothing` | RD2-1m | 3 | red / red | R2: red / red | `copy-overwrite`, `m-add-returns`, `m-discard`, `m-g-discard-anyway`, `m-g-none`, `no-copy`, `no-post-check`, `reset-clean` |
| T-RD2-1c | `a_copy_cut_short_before_its_rename_is_never_taken_for_the_copy` | RD2-1c | 3 | red / red | R2: green / green | `m-add-returns`, `m-discard`, `m-g-none`, `no-copy` |
| T-RD2-2 | `an_ignored_directory_where_head_has_a_file_is_pinned_before_the_discard` | RD2-2 | 3 | red / red | R2: red / red | `m-g-after-discard`, `m-g-none`, `m-removepin`, `no-copy`, `no-footprint`, `no-worktree-config`, `private-gitdir`, `shared-index` |
| T-RD2-2r | `an_ignored_file_where_head_has_a_directory_is_pinned_before_the_discard` | RD2-2r | 3 | red / red | R2: red / red | `m-g-after-discard`, `m-g-none`, `m-removepin`, `no-copy`, `no-footprint`, `no-worktree-config`, `private-gitdir`, `shared-index` |
| T-RD2-2p | `an_obstruction_a_refusals_pin_lacks_is_refused_and_kept` | RD2-2p | 3 | red / red | R2: red / red | `equality`, `footprint-unheld-removed`, `m-add-returns`, `m-discard`, `m-g-discard-anyway`, `m-g-none`, `m-g-trust`, `no-footprint`, `no-worktree-config`, `private-gitdir`, `shared-index` |
| T-RD2-2u | `an_unreadable_file_in_the_discards_way_is_refused_and_kept` | RD2-2u | 3 | red / red | R2: red / red | `m-g-none` |
| T-RD2-2n | `a_nested_repository_in_the_discards_way_is_refused_and_kept` | RD2-2n | 3 | red / red | R2: red / red | `equality`, `m-g-after-discard`, `m-g-discard-anyway`, `m-g-none`, `no-footprint`, `no-worktree-config`, `phase-b-force`, `private-gitdir`, `read-tree-revert`, `reset-clean`, `shared-index` |
| T-RD2-2b | `a_file_written_after_the_capture_is_never_written_over` | RD2-2b | 3 | red / red | R2: red / red | `m-g-discard-anyway`, `m-g-none`, `m-g-subject-only`, `m-g-trust`, `phase-b-force`, `read-tree-revert`, `reset-clean` |
| T-RD2-3 | `a_branch_conditioned_configuration_is_the_captures_configuration` | RD2-3 | 3 | green / green | R2: red / red | `m-add-returns`, `m-discard`, `m-removepin`, `no-worktree-config`, `private-gitdir` |
| T-RD2-3e | `an_exact_gitdir_conditioned_configuration_is_the_captures_configuration` | RD2-3e | 3 | green / green | R2: red / red | `m-add-returns`, `m-discard`, `m-removepin`, `no-worktree-config`, `private-gitdir` |
| T-RD2-3s | `a_suffix_gitdir_conditioned_configuration_is_the_captures_configuration` | RD2-3s | 3 | green / green | R2: red / red | `m-add-returns`, `m-discard`, `m-removepin`, `no-worktree-config`, `private-gitdir` |
| T-RD2-3w | `a_relative_include_in_the_worktree_configuration_is_the_captures_configuration` | RD2-3w | 3 | green / green | R2: red / red | `m-add-returns`, `m-discard`, `m-removepin`, `no-worktree-config`, `private-gitdir` |
| T-RD2-3h | `a_remote_conditioned_configuration_is_read_alike` | RD2-3h, the control | 3 | green / green | R2: green / green | `m-add-returns`, `m-discard`, `m-removepin` |
| T-RD2-3p | `a_condition_only_a_private_directory_meets_changes_no_byte` | RD2-3p | 3 | red / red | R2: red / red | `m-g-after-discard`, `m-g-none`, `m-removepin`, `no-worktree-config`, `private-gitdir` |
| T-RD3-1 | `a_capture_leaves_a_split_index_readable_and_pins` | RD3-1 | 4 | red / red | R3: red / red | `n1-discard`, `n1-nopin`, `no-split-control`, `no-split-controls` |
| T-RD3-1g | `a_resume_capture_leaves_a_split_index_readable` | RD3-1g | 4 | green / green | R3: red / red | `m-add-returns`, `m-discard`, `no-split-controls` |
| T-RD3-1e | `an_inherited_split_index_variable_expires_nothing` | RD3-1e, RD3-1ge | 4 | red / red | R3: red / red | `n1-discard`, `n1-nopin`, `no-expire-control`, `no-split-controls` |
| T-RD3-1f | `the_discard_never_queries_the_checkouts_fsmonitor` | RD3-1f | 4 | green / green | R3: red / red | `diff-tree-on-checkout`, `m-add-returns`, `m-discard` |
| (§10.4) | `a_hostile_repository_keeps_its_index_readable_and_its_monitor_unqueried` | §10.4's shared-state check | added | - / red | R3: - / red | `diff-tree-on-checkout`, `n1-discard`, `n1-nopin`, `no-split-control`, `no-split-controls` |
| T-RD3-2 | `a_resume_writes_and_syncs_its_own_copy_before_the_revert` | RD3-2, RD3-2c | 4 | red / red | R3: red / red | `copy-overwrite`, `m-add-returns`, `m-discard`, `m-g-none`, `no-copy`, `reset-clean`, `reuse-copy` |
| T-RD3-2r | `a_pin_restored_from_a_copy_is_copied_afresh_before_the_revert` | RD3-2r | 4 | red / red | R3: red / red | `copy-overwrite`, `m-add-returns`, `m-discard`, `m-g-none`, `m-removepin`, `no-copy`, `no-recover`, `reset-clean`, `reuse-copy` |
| T-RD3-2p | `a_resume_that_cannot_write_its_own_copy_refuses_and_discards_nothing` | RD3-2p | 4 | red / red | R3: red / red | `m-add-returns`, `m-discard`, `m-g-discard-anyway`, `m-g-none`, `no-copy`, `reuse-copy` |
| T-RD2-1cr | `a_copy_cut_short_is_never_restored_from` | RD2-1cr | 4 | red / red | R3: green / green | `copy-in-place`, `m-add-returns`, `m-discard`, `m-g-after-discard`, `m-g-none`, `m-removepin`, `no-copy` |
| T-P8 | `a_kept_pin_that_cannot_be_written_is_reported_and_nothing_is_discarded` | R-D1, reconciled | D | green / green | R1: green / green | `m-add-returns`, `m-discard`, `m-pinfail-discards` |

**Where a verdict differs from the design's prediction, and why** (each is in the table above):
- **T-R6, T-R8 and T-R11 are red on round 1's own patch,** where round 2's table predicts green. Round 1 refuses and
  discards nothing in each, in other words: "pinning them at … before discarding them failed", and "does not hold
  exactly: it is not upstroke's kept commit of that attempt" for both of the others. The tests take round 4's words
  ("keeping them before the discard stopped", "is not upstroke's kept commit of this attempt", "holds otherwise").
- **T-R7 is red on round 1's patch** for a reason of substance: round 1 ends a symbolic kept name as a Git error
  ("refusing to follow it", nothing discarded), where its own row, and round 4, refuse.
- **T-X19 is red at D's head and on round 1's patch,** where the table reads "green, no refusal". Neither has a revert
  for the branch to move during, so the shim never moves it and the test's first assertion fails. The table's column
  reads the harness twin, under which neither loses anything.
- **`m-pinfail-discards` does not turn T-R5 red,** where round 1 §4.2 predicts it. T-R5's fault, `.git` and
  `.git/refs` read-only, also stops the mutant's discard (`git reset --hard` cannot create `.git/index.lock`), so
  nothing is lost. T-P8 and T-R7, whose faults leave the index writable, kill it.

**Where a test departs from its row, and why:**
- **T-X10:** the resume's guarded discard leaves the unreadable file and the nested repository in place, and then the
  resumed attempt's own capture fails on them (`git add -A` on the checkout's index), as master's does on the nested
  repository; so the leftovers warning is not delivered (R-D5). The test reads the run's `run_resumed` record, which
  names what was discarded and not what was left.
- **T-X20:** the first resume runs with the run's event log read-only. It stops after its guarded discard having
  recorded nothing, so the second finds the same attempt in flight. Whether the second's attempt commits is the nested
  repository's to decide, so the test asserts no refusal and the repository kept.
- **T-RD2-3 to T-RD2-3h:** two resumes, the first dying inside the resumed attempt (a child of the test binary), so
  that "two resumes complete" holds of both guarded discards.
- **T-GRESET and T-RD2-1, -1m on Windows:** the proposal names `share_mode(0)`, which would deny Git's own read of the
  file; the capture would then not hold it, and the part-way stop the rows predict would not arise. The holder shares
  read, so Git reads the file and its unlink fails. Not executed here; `test (winguest)` runs it.
- **T-RD2-1cr:** a planted `.partial` cannot kill `copy-in-place`, which changes what a crash leaves, not what a test
  plants; the test crashes the resume itself, with a shim that cuts the file `bundle create` wrote and kills the run.
- **T-RD3-1:** the after-capture hook records the `sharedindex.*` set at the arm, so the comparison sees only the pin's
  own writes and not the coordinator's capture before it.
- **T-RD3-1f and the §10.4 check:** only the hook's calls in the checkout's own directory are counted (a snapshot's
  worktree is another), and only while the engine runs.
- **B3 (added):** an object store that takes no object fails the gate snapshot's own commit first, a Git error, so the
  run ends through N1's arm with its pin failing too, not through the refused arm; the resume pins once healed.
- **The shim's `foreign`:** an action's own Git runs without the engine child's variables (`GIT_INDEX_FILE`,
  `GIT_NO_REPLACE_OBJECTS`), as another client's would; an early draft of T-RD1-1 reset the capture's private index
  instead of the checkout's and passed vacuously, which `foreign` closed before any figure here.

**Not covered by an engine test, stated:** D1 (a reftable lock: this box's Git is 2.43.0, with no reftable), D2 and B5
(reasoned, as the design has them), and the harness-only variants RD1-1b, RD1-2t, RD1-3m and X15, RD1-4 V2, RD2-1t, X8,
X9, X13, X14 and X21; the design executes them in its harness on four Gits.

#### 5.17.4 Mutations

| Shape | What it changes | Red (actual) | The design's engine killers | Predicted but green |
|---|---|---|---|---|
| `copy-in-place` | the copy is written at its final name, with no partial name and no rename after the fsync | 1: T-RD2-1cr | T-RD2-1cr | - |
| `copy-overwrite` | round 2's one copy name, written again on every resume, and no scan of the copies | 6: (X6), T-RD2-1, T-RD2-1b, T-RD2-1m, T-RD3-2, T-RD3-2r | T-RD2-1, T-RD2-1b, T-RD2-1m | - |
| `diff-tree-on-checkout` | G4's diff-tree reads the checkout's index through the plain builder again | 2: (§10.4), T-RD3-1f | T-RD3-1f | - |
| `equality` | the pin must hold exactly the checkout's tree (round 1's rule), not contain its changes | 7: T-GRESET, T-RD2-1, T-RD2-1b, T-RD2-2n, T-RD2-2p, T-X16, T-X20 | T-GRESET, T-X16, T-X20, T-RD2-1 | - |
| `footprint-unheld-removed` | content in the revert's way that the pin does not hold is removed instead of refused | 1: T-RD2-2p | T-RD2-2p | - |
| `from-real-index` | G4's private index starts from a copy of the checkout's index, not from HEAD's tree | 1: T-RD1-2 | T-RD1-2 | - |
| `k-headcheck` | K undone: a kept pin compares HEAD with the captured parent again | 6: T-K1, T-N2a, T-R1, T-R2, T-R3, T-R4 | T-K1, T-R1, T-R2, T-R3, T-R4, T-N2a | - |
| `m-add-returns` | the add's veto answers Return: a failed add is Git state at once, as at master | 52: (X6), T-GRESET, T-L1, T-L2, T-L3, T-L6 (e1), T-L6 (e2), T-P1/3/5, T-P10, T-P11, T-P12, T-P12g, T-P13, T-P14, T-P15, T-P2, T-P4, T-P4b, T-P6, T-P8, T-P9, T-R1, T-R10, T-R11, T-R2, T-R3, T-R4, T-R5, T-R6, T-R7, T-R8, T-RD1-1, T-RD1-2, T-RD1-3, T-RD2-1, T-RD2-1b, T-RD2-1c, T-RD2-1cr, T-RD2-1m, T-RD2-2p, T-RD2-3, T-RD2-3e, T-RD2-3h, T-RD2-3s, T-RD2-3w, T-RD3-1f, T-RD3-1g, T-RD3-2, T-RD3-2p, T-RD3-2r, T-X16, T-X19 | D's shape | - |
| `m-advice` | the moved-branch refusal without its git update-ref clause | 1: T-R3 | T-R3 | - |
| `m-always-attempt` | the add's veto always answers Attempt | 1: T-L3 | D's shape | - |
| `m-b1pred` | the veto never attempts again over an empty destination | 5: T-L1, T-L2, T-L6 (e1), T-L6 (e2), T-L8 | D's shape | - |
| `m-discard` | the refused arm discards | 47: (X6), T-GRESET, T-P1/3/5, T-P10, T-P11, T-P12, T-P12g, T-P13, T-P14, T-P15, T-P2, T-P4, T-P4b, T-P6, T-P8, T-P9, T-R1, T-R10, T-R11, T-R2, T-R3, T-R4, T-R5, T-R6, T-R7, T-R8, T-RD1-1, T-RD1-2, T-RD1-3, T-RD2-1, T-RD2-1b, T-RD2-1c, T-RD2-1cr, T-RD2-1m, T-RD2-2p, T-RD2-3, T-RD2-3e, T-RD2-3h, T-RD2-3s, T-RD2-3w, T-RD3-1f, T-RD3-1g, T-RD3-2, T-RD3-2p, T-RD3-2r, T-X16, T-X19 | D's shape | - |
| `m-g-after-discard` | G's order: the pin is written after the discard, so it holds HEAD's tree | 18: (B3, B4), (X17), (X4), T-R10, T-R5, T-R7, T-R8, T-R9, T-R9 (E2), T-RD1-1, T-RD1-2, T-RD1-5, T-RD2-1cr, T-RD2-2, T-RD2-2n, T-RD2-2r, T-RD2-3p, T-X10 | T-R5, T-R9, T-R10 | - |
| `m-g-discard-anyway` | G's refusal: a failed keep is ignored and the discard runs | 12: T-R11, T-R6, T-R7, T-R8, T-RD2-1, T-RD2-1b, T-RD2-1m, T-RD2-2b, T-RD2-2n, T-RD2-2p, T-RD3-2p, T-X19 | T-R6, T-R7 | - |
| `m-g-every-leftover` | G's scope: it acts with no attempt in flight too | 1: T-R12 | T-R12 | - |
| `m-g-none` | G undone: the resume discards as at D's head | 35: (B3, B4), (X17), (X4), (X6), T-R10, T-R11, T-R5, T-R6, T-R7, T-R8, T-R9, T-R9 (E2), T-RD1-1, T-RD1-2, T-RD1-3, T-RD1-4, T-RD1-5, T-RD2-1, T-RD2-1b, T-RD2-1c, T-RD2-1cr, T-RD2-1m, T-RD2-2, T-RD2-2b, T-RD2-2n, T-RD2-2p, T-RD2-2r, T-RD2-2u, T-RD2-3p, T-RD3-2, T-RD3-2p, T-RD3-2r, T-X10, T-X16, T-X19 | T-R5, T-R6, T-R7, T-R8, T-R9, T-R10, T-R11 | - |
| `m-g-parent-is-head` | G requires the pin's parent to be HEAD | 3: T-N1c, T-R1, T-R8 | T-R1 | - |
| `m-g-subject-only` | G's containment: identity and message only | 2: T-R11, T-RD2-2b | T-R11 | - |
| `m-g-trust` | G's check: any ref at the kept name is trusted, its identity and its content alike (round 1's definition) | 4: T-R11, T-R8, T-RD2-2b, T-RD2-2p | T-R8, T-R11 | - |
| `m-index` | the coordinator pins the live index and HEAD at the refusal | 6: T-P2, T-R1, T-R2, T-R3, T-R4, T-R5 | D's shape | - |
| `m-interrupted` | the lookup over the attempts still in flight only | 2: T-P13, T-P14 | D's shape | - |
| `m-k-all` | K's scope: every pin skips HEAD | 2: T-K1, T-N2a | T-K1 | - |
| `m-k-observe-only` | K's observation skip undone: only the comparison is skipped for a kept pin | 2: T-K1, T-R4 | T-K1, T-R4 | - |
| `m-keepall` | a snapshot's Git error records the candidate too | 1: T-N1e = T-P7b | D's shape | - |
| `m-maint` | git_command without the four maintenance settings | 1: T-L7 | D's shape | - |
| `m-no-review-record` | the review snapshot's call records nothing | 1: T-P9 | D's shape | - |
| `m-noexist` | the warning names every recorded attempt's pin, existing or not | 1: T-P15 | D's shape | - |
| `m-pinfail-discards` | the refused arm discards when its pin fails | 7: (B3, B4), T-P8, T-R6, T-R7, T-R8, T-RD1-1, T-RD1-2 | T-P8, T-R5, T-R7 | T-R5 |
| `m-plain` | the warning's commands without the replacement controls | 4: T-P10, T-P11, T-P12, T-P12g | D's shape | - |
| `m-removal-alone` | round 4's B1': the removal once, its list in its own access | 1: T-L4 | D's shape | - |
| `m-remove-unwrapped` | the removal and its list as master ran them | 2: T-L4, T-P1/3/5 | D's shape | - |
| `m-removepin` | the resume removes each kept pin it finds | 44: (B3, B4), (X17), (X4), (X6), T-GRESET, T-N1c, T-N2c, T-P1/3/5, T-P10, T-P11, T-P12, T-P12g, T-P13, T-P14, T-P2, T-P4, T-P4b, T-P6, T-R1, T-R10, T-R11, T-R2, T-R3, T-R5, T-R6, T-R7, T-R8, T-R9, T-R9 (E2), T-RD1-1, T-RD1-2, T-RD1-5, T-RD2-1, T-RD2-1cr, T-RD2-2, T-RD2-2r, T-RD2-3, T-RD2-3e, T-RD2-3h, T-RD2-3p, T-RD2-3s, T-RD2-3w, T-RD3-2r, T-X10 | D's shape | - |
| `m-restore` | round 5's warning: git checkout <pin> -- ., and no command for a later HEAD | 8: (X4), T-P10, T-P11, T-P12, T-P12g, T-P4, T-P4b, T-R1 | D's shape | - |
| `m-return` | a destination gone, or empty and not removable, answers Return | 2: T-L3, T-L8 | D's shape | - |
| `m-switch-unwrapped` | switch_branch's git switch as master ran it | 1: T-L5 | D's shape | - |
| `n1-discard` | N1 undone: an attempt error discards the checkout, as at D's head | 11: (B3, B4), (N1d), (N1e), (§10.4), T-N1a, T-N1b = T-P7, T-N1c, T-N1e = T-P7b, T-RD3-1, T-RD3-1e, T-X10 | T-N1a, T-N1b = T-P7, T-N1c, T-N1e = T-P7b | - |
| `n1-nopin` | N1 half-undone: an attempt error keeps the checkout but pins nothing | 10: (B3, B4), (N1d), (N1e), (§10.4), T-N1a, T-N1b = T-P7, T-N1c, T-N1e = T-P7b, T-RD3-1, T-RD3-1e | T-N1c | - |
| `n2-discard` | N2 undone: a publication failure discards the checkout, as at D's head | 3: T-N2a, T-N2b, T-N2c | T-N2a, T-N2b, T-N2c | - |
| `no-copy` | no copy of the pin outside the repository's refs before the discard | 13: (X6), T-RD1-3, T-RD2-1, T-RD2-1b, T-RD2-1c, T-RD2-1cr, T-RD2-1m, T-RD2-2, T-RD2-2r, T-RD3-2, T-RD3-2p, T-RD3-2r, T-X19 | T-RD1-3, T-RD2-1, T-RD2-1b, T-RD2-1m, T-RD2-1c | - |
| `no-dir-sync` | the copy's directory is never fsynced after the rename | 0: none | none in-process (trace) | - |
| `no-expire-control` | the view sets core.splitIndex=false but not splitIndex.sharedIndexExpire=never | 1: T-RD3-1e | T-RD3-1e | - |
| `no-file-sync` | the copy's data is never fsynced before the rename | 0: none | none in-process (trace) | - |
| `no-footprint` | nothing in the revert's way is added again with -f before the pin is written or checked | 4: T-RD2-2, T-RD2-2n, T-RD2-2p, T-RD2-2r | T-RD2-2, T-RD2-2r | - |
| `no-head-check` | no check after the revert that the run branch did not move | 1: T-X19 | T-X19 | - |
| `no-ignore-errors` | a path add cannot take fails the whole capture (plain add -A) | 1: T-X10 | T-X10 | - |
| `no-leave-in-place` | a new file the pin never held refuses, instead of being left in place | 1: T-X16 | T-X16 | - |
| `no-post-check` | no check after the revert that every path it changed is HEAD's again | 4: T-GRESET, T-RD2-1, T-RD2-1b, T-RD2-1m | T-GRESET, T-RD2-1 | - |
| `no-recover` | the copies are kept, but a missing pin is captured again instead of recovered | 3: T-RD2-1, T-RD2-1b, T-RD3-2r | T-RD2-1 | - |
| `no-split-control` | the view sets splitIndex.sharedIndexExpire=never but not core.splitIndex=false | 2: (§10.4), T-RD3-1 | T-RD3-1 | - |
| `no-split-controls` | the private-index view sets neither core.splitIndex=false nor splitIndex.sharedIndexExpire=never | 4: (§10.4), T-RD3-1, T-RD3-1e, T-RD3-1g | T-RD3-1, T-RD3-1g, T-RD3-1e | - |
| `no-worktree-config` | the capture is round 2's private Git directory with no copy of the checkout's config.worktree | 10: T-RD2-2, T-RD2-2n, T-RD2-2p, T-RD2-2r, T-RD2-3, T-RD2-3e, T-RD2-3p, T-RD2-3s, T-RD2-3w, T-X18 | T-X18, T-RD2-3, T-RD2-3w | - |
| `phase-b-force` | phase B writes with checkout-index -f, over whatever is at the path | 2: T-RD2-2b, T-RD2-2n | T-RD2-2b, T-RD2-2n | - |
| `private-gitdir` | the capture is round 2's private Git directory (detached HEAD, config.worktree copied) | 9: T-RD2-2, T-RD2-2n, T-RD2-2p, T-RD2-2r, T-RD2-3, T-RD2-3e, T-RD2-3p, T-RD2-3s, T-RD2-3w | T-RD2-3, T-RD2-3e, T-RD2-3s, T-RD2-3w, T-RD2-3p | - |
| `read-tree-revert` | round 2's one read-tree -m -u R HEAD, which takes ignored files for expendable | 2: T-RD2-2b, T-RD2-2n | T-RD2-2b, T-RD2-2n | - |
| `reset-clean` | the discard is reset --hard and clean -qfd, as before, not the exact revert | 13: (X6), T-RD1-3, T-RD1-4, T-RD2-1, T-RD2-1b, T-RD2-1m, T-RD2-2b, T-RD2-2n, T-RD3-2, T-RD3-2r, T-X10, T-X16, T-X19 | T-RD1-4, T-X10, T-X16, T-RD2-2b, T-RD2-2n | - |
| `reuse-copy` | an existing copy is relied on with no fsync, and no fresh copy is written (round 3's rule) | 4: (X6), T-RD3-2, T-RD3-2p, T-RD3-2r | T-RD3-2, T-RD3-2p, T-RD3-2r | - |
| `revert-gitlinks` | the discard also reverts submodule paths, which no discard can remove | 1: T-X20 | T-X20 | - |
| `review-diff` | G4's capture also decodes the review diff as strict UTF-8 (capture_candidate's) | 2: T-N1a, T-RD1-5 | T-RD1-5 | - |
| `shared-index` | G4 captures through the checkout's own index, with no footprint (round 1's capture, as the harness's mutation of that name) | 6: T-RD1-1, T-RD1-2, T-RD2-2, T-RD2-2n, T-RD2-2p, T-RD2-2r | T-RD1-1, T-RD1-2, T-RD2-2, T-RD2-2r | - |

Each shape is a scratch tree of `a4972d09` (`git archive`), changed by exactly its transform (its `mutation.diff`), and
built and tested through `upstroke-build` on this lane's private pool. Every Compiling line names its scratch tree. A
shape runs the 86 tests: this round's 61 (T-K1 and the 60 engine tests) and D's 25. The shapes are:
- round 4's 7 (`no-split-controls` to `no-file-sync`) and round 3's 24 read against G4, the proposal's §10.2. The 24
  include round 1's `k-headcheck` and the N mutations `n1-discard`, `n1-nopin` and `n2-discard`;
- round 1's other 11 engine-level mutations (round 1 §4.2, `m-k-observe-only` to `m-pinfail-discards`), read against
  G4;
- D's mutations from §5.7 but `m-keepany`, 17 (`m-add-returns` to `m-noexist`), re-run.

"The design's engine killers" are the tests those sections name; "Predicted but green" lists any that stayed green.

**Coverage** (`fudi3/record/COVERAGE.txt`, from the 64 summaries):
- every Compiling line names its scratch tree;
- the control is 86 of 86 green;
- `base-firstbad` turns 52 of the 86 red, round 1's patch 42, round 2's 20 and round 3's 9;
- **every one of the 86 tests is turned red by at least one mutation** (the census's "tests with no killing mutation:
  0");
- every mutation turns at least one test red, but `no-dir-sync` and `no-file-sync`, which only the trace sees
  (§5.17.5);
- every engine killer the design names is red, but T-R5 under `m-pinfail-discards` (§5.17.3);
- D's 17 shapes keep their meaning, and D's 25 tests stay green at every base. Of D's §5.7 shapes, `m-keepany` is not
  re-run: part N makes it the design, and `n1-discard` is its inverse.

**Red before, green after.** Of the 64 rows above (the 61 new tests, and D's T-P7, T-P7b and T-P8 as the design
reconciles them):
- 52 are red at D's head;
- 8 are green there and red on the design round whose regression they guard: T-GRESET and T-X20 on round 1's patch,
  T-RD2-3, -3e, -3s and -3w on round 2's, and T-RD3-1g and T-RD3-1f on round 3's;
- 4 are green at every base: the matrix's controls T-R12, T-X18 and T-RD2-3h, and the reconciled T-P8. T-R12 is red
  under `m-g-every-leftover`, T-X18 under `no-worktree-config`, and T-P8 under `m-pinfail-discards`. T-RD2-3h is red
  only under D's `m-add-returns`, `m-discard` and `m-removepin`: no R-D1 mutation turns it red, as none should, since
  the configuration it conditions reads alike in the capture and in the checkout.

All 64 are green at `a4972d09`.

#### 5.17.5 Durability by trace, and the shared-state check

- **The copy's barrier, traced on Linux** (`fudi3/tools/durability_trace.py`, run by `trace_mutants.py`;
  `fudi3/durability/mut-control/SUMMARY.txt`): T-RD3-2 under `strace -f -ttt` — a resume over a copy planted under its
  final name — shows, in the thread that writes the copy, `openat` of the fresh
  `.partial` for writing, `fsync` of it returning 0, its `rename` to the final `.bundle`, `openat` of the run's public
  directory and its `fsync` returning 0, all before the first `git read-tree -m` is executed. The found copy is not
  relied on.
- **The trace's own mutations,** each a scratch tree built apart: under `no-dir-sync` no fsync of the copies' directory
  precedes the first `read-tree -m` (`mut-no-dir-sync/`), and under `no-file-sync` no fsync of the `.partial` precedes
  its close and rename (`mut-no-file-sync/`). Each is reported NOT durable; the control built the same way is durable.
  In process both are green (§5.17.4), as the design predicts: only the trace, or the harness's model, sees them.
- **Built from `45b1a7b1`,** whose production code and T-RD3-2 are `a4972d09`'s (`a4972d09` changes test code
  elsewhere). An earlier run of the tool, before it tracked `close`, read a directory fsync on a reused descriptor as
  the file's under `no-file-sync`, with the same verdict (`fudi3/durability/SUPERSEDED-fd-reuse/README.txt`); the
  first trace, `durability/head/`, is a WIP build's.
- **macOS and Windows are not traced.** `sync_all` is `F_FULLFSYNC` there and `FlushFileBuffers`; `sync_parent` does
  nothing on Windows. Reasoned, as the design states.
- **The shared-state check** (the proposal's §10.4) is the engine test
  `a_hostile_repository_keeps_its_index_readable_and_its_monitor_unqueried`: `core.splitIndex=true` with
  `splitIndex.sharedIndexExpire=now`, `core.untrackedCache=true` and a logging fsmonitor hook; N1's pin adds and removes
  no shared index, the index reads after the pin and after the resume, and no engine child queries the checkout's
  fsmonitor. CI's Linux and macOS legs run it.

#### 5.17.6 The findings

- **Filed from their drafts** (`~/orch-pr11/logs/pr11_rd1_round4/drafts/`), in `findings/README.md`'s form, with their
  original severity (P1), review pin (`reviewed_sha` `88376a10`), provenance (`pre_existing`) and first-bad commit:
  `PR331-AN-ATTEMPT-ERROR-AFTER-THE-WORKER-RAN-DISCARDS-ITS-OUTPUT` (N1,
  `findings/P1_correctness_202610040917_an-attempt-error-after-the-worker-ran-discards-its-output.md`) and
  `PR331-A-REVIEWED-CANDIDATE-IS-DISCARDED-WHEN-ITS-PUBLICATION-FAILS` (N2, `…202610040918_…`). Only the guard's first
  words change, to name this round, and each gains a dated section: implemented on this draft, conditional on O8, with
  its engine witnesses and their mutations; open until the merge. **Their G6 non-applicability stays a claim:** it needs
  classifying on the final range, the Q6 / topology-writer face and the R-C-RELSTORE face included. There is no waiver.
- **The legacy finding stays open,** with its guard naming R-D1's preservation design and a dated section listing the
  engine witnesses of R-D1's cases and what stays unshown (D1, D2, B5). Nothing regrades it or clears G6.
- **R-D5** (`PR331-A-RESUME-THAT-STOPS-BEFORE-ITS-REPORT-NAMES-NO-KEPT-PIN`) and **R-D7**
  (`PR331-KEPT-PINS-ACCUMULATE-UNTIL-THE-OPERATOR-REMOVES-THEM`) gain dated sections: G4's three warnings share R-D5's
  delivery limit (T-X10 executes it), and N's and G4's pins, G4's copies, `.partial`s, unsynced final names, private
  index files and orphan shared index files accumulate under R-D7.
- **The R-D1 design reviews' items** — round 1's RD1-1 to RD1-6, G-RESET, N1 and N2; round 2's regression items RD2-1
  to RD2-3; round 3's RD3-1 and RD3-2 — are what round 4's design answers. This implementation carries their
  regression tests (§5.17.3), and the pull request body names them with their triages; they are not ledger rows,
  being items of a design document rather than of this branch. Their closure is for the required reviews to confirm.

#### 5.17.7 The costs and limits, kept

All of the proposal's §9.1 and §9.3, in the code's behaviour and here:
- **The new copy and refusal costs:** one copy per resume that reaches the revert, never written over; refusals while
  the pin's store, name or copy cannot be written, while the checkout conflicts with its pin, while a discard is stopped
  part-way (one resume), while the branch moves during a discard, while a pin disagrees with its copy or two copies
  disagree, while a restore fails, while something in the discard's way is not held by a pin made before G4, cannot be
  added, or is a nested repository, while the log records more than one attempt in flight (the proposal's §5.2), and
  **while the resume cannot complete its own copy's barrier** — a full or read-only file system, a directory fsync that
  fails — even when the attempt already has a copy (T-RD3-2p); after an attempt error or a failed publication, a dirty,
  pinned checkout that a new run refuses until a resume or a clean; ignored content in the discard's way entering the
  repository's objects and the copy; the resume's extra Git children.
- **The O3 exception:** an inherited `GIT_TEST_SPLIT_INDEX` still splits the private index's writes; they unlink nothing
  (`splitIndex.sharedIndexExpire=never`) and leave orphan `sharedindex.*` files in the checkout's Git directory. Git
  removes one only at a later split write of the checkout's own index that creates a shared index file, and only once
  it is older than the checkout's `splitIndex.sharedIndexExpire`: with the default, `2.weeks.ago`, it stays until it
  is older than two weeks and such a write follows; with `never`, automatic expiry never reclaims it; and repeated
  captures can accumulate them. The variable is O3's class, with FUD-D4-ENV's, and the cost is the owner's choice.
  *(Repair round 5, §5.19: D-I2-1. This said that Git's next split write of the checkout's index removes them.)*
- **The phase-A window:** a write into a file in the instant between Git's up-to-date check and its unlink is lost, as
  in Git's own checkout, reset and read-tree; disclosed, not designed away.
- **"No loss" only in the covered cases** (§5.17.3 and the proposal's §1, §4.6, §10), never as a universal guarantee.
- **What this proves, and what it does not.** The engine witnesses here are the actual legacy engine on Linux, Git
  2.43.0. The proposal's harness models the engine's Git sequences on four Gits; its power loss is a model; its census
  is ordinary Git. None of it, and nothing here, is promoted to physical power-loss proof or native-platform proof: no
  power was lost, no disk failed, and nothing ran on Windows or macOS here.

#### 5.17.8 The frozen effects

`fudi3/tools/frozen-proof3.sh` at the final code commit `a4972d09` (`fudi3/frozen/frozen-proof-a4972d09.txt`; this
record's commit changes no code, and the pull request body cites the same proof at the head it is pushed at):
- **G6's frozen set** (PR11's R-D list: 26 production files and 8 whole-file test children):
  - **zero delta from #329's provisional head `55029628`,** 34 of 34 byte-identical;
  - from master `5c222ff2`, 33 of 34: `src/engine/topology/integrate.rs` differs by +16 −4. That is exactly
    master..`55029628`'s difference, #329's proposed frozen hunk H1, inherited through the provisional merge and
    conditional on #329's freeze ruling. This round's own delta over the set is zero.
- **Against G5's range `d724fb16`** (O12 not adopted, so not a clean proof): five files, +257 −78. They are master's own
  four (+241 −74, #322's and #327's) and #329's H1. E-G6-1's two-tier rule, executed and not adopted, reports FAIL for
  exactly one unenumerated path, `src/engine/topology/integrate.rs`: #329's H1, not this round's.
- **The PR5-frozen legacy section** (24 paths, `FROZEN_LEGACY_ALLOWLIST` at the head, `src/effects.rs` unchanged): from
  `55029628`, exactly D's five differ.
- **D's O8 five-file set, exactly** (`git diff --numstat`, insertions and deletions):

  | File | This round, `88376a10..a4972d09` | of which the merge `47e62ee5` | of which the implementation, `47e62ee5..a4972d09` | Cumulative, master..`a4972d09` (= from `55029628`, = from `54a1ff14`) | Blob at `a4972d09` |
  |---|---|---|---|---|---|
  | `src/workspace.rs` | +921 −22 | +8 −0 | +913 −22 | +1,537 −82 | `acc92d0e…` |
  | `src/engine/attempt.rs` | **unchanged** (blob `51534706…` at `88376a10`, the merge and `a4972d09`) | — | — | +53 −10 | `51534706…` |
  | `src/engine/coordinator.rs` | +87 −11 | — | +87 −11 | +116 −10 | `d2a6be71…` |
  | `src/engine/resume.rs` | +113 −10 | — | +113 −10 | +138 −4 | `f829a38d…` |
  | `src/engine/tests.rs` | +3,724 −16 | — | +3,724 −16 | +4,840 −0 | `9d43ed5d…` |

- **Append-only** (`fudi3/frozen/append-only-a4972d09.txt`):
  - against master, `src/engine/tests.rs` is master's blob with bytes appended, and `src/workspace.rs`'s test module
    keeps every byte master's had before its closing brace;
  - against the merge, the test module only gains T-K1. `src/engine/tests.rs` is **not** append-only: the design's
    patch changes D's T-P7, T-P7b and T-P8 (the 16 deletions) before appending.
- **Schema 4:** `TOPOLOGY_ACTIVATION` is `Inactive`, and no path under `src/topology/` or `src/engine/topology/` changes
  from `55029628`.
- **Every other path this round changes:**
  - `clippy.toml`, `effects/wrappers.toml` and `src/runner/contract.rs` (the instrument rows);
  - `effects/allowlist.toml` (the O8 texts) and `design/15`;
  - the notes of `workspace`, `engine/coordinator`, `engine/resume` and `engine/tests`;
  - the record, and the finding files of §5.17.6.

#### 5.17.9 What passes only through proposed instruments

- **The three instrument rows** (`fudi3/tools/instrument_probe.py`, `fudi3/instruments/instr-*/summary.txt`, each a
  scratch tree of `a4972d09` with one row taken back out, built with `--all-targets`; the same at `45b1a7b1`,
  `instruments/at-45b1a7b1/`): with every row, the five effects and contract tests pass; without the two
  `effects/wrappers.toml` rows,
  `every_externally_reachable_fn_of_a_legacy_or_shared_module_is_classified` and
  `every_effectful_wrapper_is_on_the_disallowed_list` fail; without the two `clippy.toml` denials,
  `every_effectful_wrapper_is_on_the_disallowed_list` fails; with the payload census at five,
  `every_production_command_spec_payload_is_classified` fails. (An earlier `--lib` run, superseded, failed the two
  denial censuses for a missing rlib, its own message: `fudi3/instruments/SUPERSEDED-lib-only-README.txt`.)
- **The O8 texts** (`fudi3/tools/allow-probe.py`, `fudi3/instruments/allow-probe.txt`): with the five modules'
  module-level allows removed, 88 clippy denials lie on lines this round adds at `a4972d09` — 17 in production
  (`src/workspace.rs` 15, among them the copy's `OpenOptions`, `sync_all`, `rename` and `remove_file`;
  `src/engine/coordinator.rs` 1, the call of `Workspace::pin_checkout`; `src/engine/resume.rs` 1, the call of
  `Workspace::discard_into_kept_pin`, both denied by the new `clippy.toml` rows) and 71 in tests. Gate 2 passes over
  them only through those allows, which the five rows admit and whose texts are the proposed O8 texts. No check reads
  the texts' words (§5.14).

#### 5.17.10 Platforms

- **Linux:** every figure here.
- **Windows and macOS, type-checked and linted only** (`fudi3/platform/code-a4972d09/`, and `code-45b1a7b1/` before the
  test revision; each Checking line names this worktree): Windows clippy, Windows MSRV with `RUSTFLAGS='-D warnings'`,
  macOS clippy, and the Linux MSRV with `-D warnings`, each rc 0 with no warning. The first run, at a WIP commit, failed
  Windows on an unused variable in one test (`fudi3/platform/code-0b91221e/`), fixed before the implementation commit.
  **Nothing ran on Windows or macOS here; CI is the truth for those legs.**
- **Per platform:** the shim, mode-bit and fsmonitor tests are `cfg(unix)`; the non-UTF-8 name test is Linux-only (APFS
  refuses such names); T-GRESET and T-RD2-1, -1m run on Windows with the read-sharing holder.

#### 5.17.11 What waits, and what is not verified

- **O8,** and every contract or freeze adoption: the owner's. The texts and design sentences are proposed.
- **#329's final head:** this merge is provisional; its final integration, with validation, native CI and review, is
  owed.
- **The required reviews:** the regular review of R-D1's design is unmet; this implementation is unreviewed.
- **N1's and N2's G6 classification** on the final range.
- **Windows and macOS execution** is CI's (`test (winguest)` with a CRLF checkout, the hosted legs).
- **Not executed here:** D1, D2, B5; physical power loss; a failed fsync on a real disk.

#### 5.17.12 CI history

- **`88376a10`:** run 37152667513 passed every job and the `upstroke-ci` rollup, and the policy run 37152667575 passed
  (`fudi3/ci/jobs-37152667513.tsv`, `runs-88376a10.json`); the two cancelled runs at that head were superseded by the
  body edit.
- **#329's `55029628`,** merged provisionally: run 37186359003 passed every job (`fudi3/ci/jobs-37186359003-B.tsv`).
- CI at this round's head is not waited on; the orchestrator reads native CI.

The pull request body records the head the ten gates passed at, with the logs under `fudi3/gates/`.

### 5.18 Repair round 4: the hosted-Linux failure of T-R10 at `202c0805`

> **On §5's terms, as §5.17 left them:** a draft, PROPOSED, conditional on the owner's decision O8, and not merge-ready.
> Its merge waits on O8's adoption, on #329 merged (O9, O14, O11), on the integration of #329's *final* head (the merge
> of `55029628` stays PROVISIONAL), and on the required reviews: **the regular review of R-D1's design is unmet**, and
> nothing here substitutes for it. Nothing here adopts an owner decision, a contract or a freeze. The round adds two
> commits on `202c0805`, the test fix `6bdd02c3` and this record's commit, with no rebase, and no `reviewed_sha` is
> re-stamped.

**Who and on what.** `pr11_fud_impl4` (`claude-opus-5-5`, `max`), a fresh repair session `orch_pr11` spawned on
2026-10-04 on `202c0805`. Its brief is `~/orch-pr11/briefs/pr11_fud_impl4.md`; round 1's to 3's briefs and
`~/orch-pr11/d-impl/UNIT.md` bind where it does not change them. Two notes of the orchestrator's came with it:
`~/orch-pr11/answers/pr11_fud_impl4-0.md`, a boundary correction (§5.18.8), and `pr11_fud_impl4-0b.md`, which names a
permitted Git 2.55.0 build and a lead to test, not a cause. Its figures are under `~/orch-pr11/logs/pr11_fud_impl4/`,
cited as `fudi4/…`. The stopped round-2 regular review of R-D1's design was not read, opened, run or used, and nothing
here retries, rewords, reroutes or relabels it. Two broad searches of this round could have enumerated its parent
directory, as §5.18.8 records.

#### 5.18.1 What failed

- **CI run 37202558686 at `202c0805`** (`fudi4/ci/jobs-37202558686.tsv`): `test (ubuntu-latest)` (job 111437892233)
  failed, and with it the `upstroke-ci` rollup; every other job passed. The policy run 37202558600 passed. The two cancelled runs at
  that head, 37202557270 and 37202557276, were superseded by the body edit (`fudi4/ci/runs-202c0805.json`).
- **The failing job** ran on the image ubuntu-24.04, version 20260927.320.1, with rustc 1.99.0 and **Git 2.55.0** (the
  checkout step's own `git version`). It tested the merge `5527bf0c` of `202c0805` into master `5c222ff2`. Its log is
  `~/orch-pr11/logs/orch-ci/331-202c0805/failed-job-111437892233-ubuntu.log`, sha256 `26e608b6…`. The lib ran 3,224
  passed, 1 failed and 137 ignored, in 485.95 s.
- **The one failure was T-R10,** `a_kept_pin_removed_before_the_resume_is_written_again_from_the_checkout`. It panicked
  at `src/engine/tests.rs:10920:52`, in `repair_the_torn_registration`: "the operator removes the residue", `NotFound`.
  The operator's repair found no residue to remove. The panic does not name the arm.
- **The other test legs** (`fudi4/ci/job-111437892309.log`, `job-111437892265.log`): `test (macos-latest)` ran Git
  2.55.0 (`/opt/homebrew/bin/git`), and T-R10 passed there; `test (winguest)` ran Git 2.50.1.windows.1, and T-R10
  passed. This box's ten gates had passed at `202c0805` on Git 2.43.0 (`fudi3/gates/final-202c0805/`).

#### 5.18.2 The cause

**T-R10's own fixture.** Its F2 arm prunes the kept pin with the operator's
`git fetch -q --prune <remote> +refs/upstroke/*:refs/upstroke/*`. The torn registration the run refused on is still in
place then: the operator repairs it only after the fetch. By default, every `git fetch` ends by running Git's automatic
maintenance, `git maintenance run --auto`, detached. What that maintenance does depends on Git's version.
The Git facts are from Git 2.55.0's source, excerpted in `fudi4/stand-in/git-2.55.0-source-excerpts.txt`:
- **From Git 2.54, unscheduled maintenance takes the `geometric` strategy.** Git 2.54.0's release notes say so:
  "\"git maintenance\" starts using the \"geometric\" strategy by default". The strategy includes the `worktree-prune`
  task, which runs `git worktree prune --expire 3.months.ago` once one registration is prunable.
- **The planted torn registration is prunable.** Its `gitdir` names a checkout that does not exist, and it has no
  `index` and no `locked` (`should_prune_worktree`: "gitdir file points to non-existent location").
  `git worktree prune --dry-run --verbose` says so on Git 2.43.0, 2.50.1 and 2.55.0 alike (`fudi4/stand-in/mechanism.txt`).
- **Before 2.54 the default is the `gc` strategy,** whose `gc --auto` does nothing below its thresholds, so the fetch
  leaves the registration alone (measured on Git 2.43.0 and 2.50.1, below).

So on Git 2.55.0 the fetch's maintenance prunes the registration, in a detached process that races the test's next
steps. Where the prune wins, the operator's repair finds nothing and panics; where the repair wins, the prune finds
nothing. **On the hosted runner the repair found nothing,** the signature the stand-in gives when the prune wins. On
macOS's Git 2.55.0, T-R10 passed in the same run, as the race allows; whether its maintenance pruned there is not
established. Windows's Git 2.50.1 does not prune.

**Git alone** (`fudi4/stand-in/mechanism.sh`, `mechanism.txt`): a repository with the same residue and the same fetch.
On Git 2.43.0 and 2.50.1 the registration survives the fetch; on 2.55.0 it is gone. With
`-c maintenance.auto=false -c gc.auto=0`, or with `--no-auto-maintenance`, it survives on all three. In every case the
fetch still prunes the pin.

**Through the engine, on a stand-in** (`fudi4/repro/`, `fudi4/tools/run-one.sh`): this box's Linux (Ubuntu 24.04.4,
kernel 6.8, `fudi4/stand-in/box.txt`) with the permitted Git 2.55.0 build
`~/orch-pr11/logs/pr11_fub_design7/gits/2.55.0/bin/git`. It reports `git version 2.55.0` and was built from Git's
verified source tarball (`fudi4/stand-in/git-2.55.0-build.txt`). It has no system configuration, and this box's global
configuration sets no `maintenance.*`, `gc.*` or `worktree.*` key (`fudi4/stand-in/box-git-config.txt`). T-R10 as
committed at `202c0805` ran one test per run, through `upstroke-build`:
- on Git 2.55.0 with the maintenance detached, as on the runner, **red in 5 of 10 runs**, each with the CI's panic at
  `src/engine/tests.rs:10920:52` (`base-255-detach-summary.txt`, `base-255-detach-failures.txt`), and in 3 of 6
  traced runs;
- on Git 2.55.0 with the maintenance synchronous, **red in 10 of 10**, with the same panic (`base-255-sync-summary.txt`).
  Git's own `GIT_TEST_MAINT_AUTO_DETACH=false` does that, and it changes nothing but whether the maintenance detaches;
- on this box's Git 2.43.0, green (`base-243-1.log`).

**The trace of a red run** (`GIT_TRACE2_EVENT`, one file per Git process: `fudi4/repro/trace-255-detach-1/`, its census
`trace-255-detach-1-census.txt`, `fudi4/tools/trace2_census.py`):
- The only `git worktree prune --expire 3.months.ago` ran under the maintenance that F2's `git fetch -q --prune`
  started. That was the only maintenance to run a task.
- The engine's 385 top-level Git processes all carry the four `AUTO_MAINTENANCE_REFUSED` settings, and none started
  maintenance.
- The fixture's commits started maintenance that ran no task.

**What the stand-in can and cannot show.** It runs the hosted leg's code path where it matters: the same Git release's
maintenance, strategy and prune, through the same test and the actual engine, on Linux. It cannot reproduce the
runner's scheduling or file system, or whatever `/home/runner/.gitconfig` holds: the job log shows that the file
exists, not what is in it. The runner's failure has the signature the stand-in gives with its maintenance on. The
detached race's rate, 5 of 10, is this box's, not the runner's.

**Not D's code.** The engine removed and moved nothing. Its Git children carry `maintenance.auto=false` (R-G), and none
started maintenance, here or in §5.18.5's sweeps. With the fetch's maintenance off, the operator's repair finds the
residue in all 30 runs on Git 2.55.0 (§5.18.4), as it does on Git 2.43.0.

**A note on the residue, observed and not changed.** Git's own `git worktree add` writes `locked` ("initializing")
before `gitdir` and `commondir` (2.55.0's `builtin/worktree.c`, in the excerpts). So a registration that Git's own add
leaves torn after it wrote `gitdir` carries `locked`, and `should_prune_worktree` skips it. The planted residue has no
`locked`, as D's tests have always planted it. The residue's shape is the design's, and this round leaves it alone.

#### 5.18.3 A test defect

The defect is an assumption T-R10's fixture makes about its environment: that the operator's own Git leaves the planted
residue in place between the fetch and the repair. The assumption held on Git 2.43.0 and 2.50.1, and fails from 2.54.
- **Not the engine's:** the failure comes before the resume, in the test's own repair step, and nothing of the
  engine's removed the residue.
- **Not a flake:** the cause is executed. With the race taken out the test is red 10 of 10, and the fix is shown by a
  mutation (§5.18.4).
- **Not B's:** T-R10, its F2 arm and `repair_the_torn_registration` are D's code.

#### 5.18.4 The fix, and its witness

**The fix** (`6bdd02c3`, test code and its notes only). F2's fetch runs with `-c maintenance.auto=false -c gc.auto=0`.
Those are two of the four settings `git_command` gives every legacy Git child (`AUTO_MAINTENANCE_REFUSED`). Git's
automatic maintenance never starts, so the residue stays until the operator's repair, as it does on Git 2.43.0. What
the repair finds no longer depends on Git's maintenance policy.
- **What F2 tests is unchanged.** The kept pin is still pruned by `fetch --prune` with a refspec covering
  `refs/upstroke/*`, as round 1's proposal defines F2 (`~/orch-pr11/owner-package/RD1-PRESERVATION-PROPOSAL-e9f78041.md`).
- **No assertion changes,** and the update-ref arm is untouched.
- `src/engine/tests.rs` carries no comment (§13). The notes of `engine/tests` say why the settings are there.

**Red before and green after, on the Git 2.55.0 stand-in** (`fudi4/repro/`):
- at `202c0805`, red 10 of 10 with synchronous maintenance and 5 of 10 detached (§5.18.2);
- at `6bdd02c3`, green 10 of 10 synchronous (`fix-255-sync-summary.txt`) and 20 of 20 detached
  (`fix-255-detach-summary.txt`), and green 3 of 3 on Git 2.43.0 (`fix-243-summary.txt`);
- in a traced green run (`trace-fix-255-sync-census.txt`), F2's fetch starts no maintenance, and no maintenance runs a
  task.

**The mutation that brings it back**, `m-f2-maintenance`, removes the two settings, leaving F2's fetch as it was at
`202c0805`. It runs through `fudi4/tools/campaign.py`, which is round 3's campaign with IMPL at `6bdd02c3`. On Git 2.55.0
with synchronous maintenance, T-R10 is red with the CI's panic (`src/engine/tests.rs:10920:52`, `NotFound`). On Git
2.43.0 it is green.

**T-R10 still asserts what its row requires** (`fudi4/mutation/<shape>@git-2.43.0/` and `@git-2.55.0-sync/`). Each
shape is a `git archive` scratch tree of `6bdd02c3` whose Compiling line names it, and T-R10 ran alone in it:

| Shape | Git 2.43.0 | Git 2.55.0, maintenance synchronous | Round 3's campaign (§5.17.4), Git 2.43.0 |
|---|---|---|---|
| `control` | green | green | green |
| `m-f2-maintenance` | green | **red** ("the operator removes the residue, NotFound") | — |
| `base-firstbad` | **red** ("the resume wrote the pin again") | **red** ("the resume wrote the pin again") | red |
| `base-r1` | green | green | green |
| `m-add-returns` | **red** ("update-ref: a registry refusal, never …") | **red** ("update-ref: a registry refusal, never …") | red |
| `m-discard` | **red** ("update-ref: a registry refusal, never …") | **red** ("update-ref: a registry refusal, never …") | red |
| `m-g-after-discard` | **red** ("update-ref: from the checkout, which held the output") | **red** ("update-ref: from the checkout, which held the output") | red |
| `m-g-none` | **red** ("the resume wrote the pin again") | **red** ("the resume wrote the pin again") | red |
| `m-removepin` | **red** ("the resume wrote the pin again") | **red** ("the resume wrote the pin again") | red |

Every call into the engine is bounded (`bounded_run` and `bounded_resume`, 300 s).

#### 5.18.5 The siblings

**Every other test of round 3, and the engine path they share** (`fudi4/sweep/`; `fudi4/tools/sweep.sh`,
`sweep_summary.py`, `overlap.py`, `engine_path.py`). The sweep covered round 3's 70 tests
(`fudi4/sweep/round3-test-paths.txt`): 69 in `src/engine/tests.rs`, 7 of them children their parents run, and T-K1. Each
ran alone, on the Git 2.55.0 stand-in, with a trace, at `202c0805` and at `6bdd02c3`, with the maintenance synchronous
and detached:

| | `202c0805`, synchronous | `202c0805`, detached | `6bdd02c3`, synchronous | `6bdd02c3`, detached |
|---|---|---|---|---|
| Green | 69 of 70 (T-R10 red) | 70 of 70 (T-R10's repair won its race) | 70 of 70 | 70 of 70 |
| Automatic maintenance processes | 168 | 168 | 167 | 167 |
| Of them, that ran a task | 1: `worktree-prune`, under T-R10's F2 fetch | 1: the same | 0 | 0 |
| Engine Git processes started while one ran | — | 0 | — | 0 |
| The engine's top-level Git processes (all four settings) / maintenance under them | 17,212 / 0 | 17,375 / 0 | 17,375 / 0 | 17,373 / 0 |

**Fixed on this cause: T-R10 alone.** In no other round-3 test did a maintenance its fixture started find anything to
do: none ran a task, in either mode.

**The rest, with the evidence** (`*/SUMMARY.txt`, `fudi4/sweep/ENGINE-PATH.txt`, `ENGINE-PATH-fix.txt`, `OVERLAP.txt`).
The other maintenance-capable Git commands in round 3's tests are all the fixtures' own, and no maintenance they
started ran a task, in either mode:
- `commit -q -m seed` and `commit -q -m fixture`, 155 times per sweep: master's `temp_engine_repo` and `seed`, before
  the engine runs and before any residue is planted;
- `restore_the_pin_from`'s `git fetch <copy> <pin>:<pin>`, 9 times, after the residue is repaired;
- `git gc -q --prune=now` (`forget_every_unreferenced_object`), 8 times: an explicit gc, synchronous on every Git and
  intended by its tests, at a point where no registration is prunable;
- the nested repositories' commits, twice: their maintenance runs inside the nested repository;
- F2's `git push`, once: its maintenance runs in the bare remote, which holds no registration.

Each of those maintenance processes lasted at most 2.5 ms in the detached sweeps, and no engine Git process started
while one ran. They stay as they are: the evidence shows the assumption holds for them, and master's helpers lie
outside D's append-only touch.

**The engine path.** In all four sweeps, every top-level Git process of the engine carries the four settings, and none
started maintenance. That is R-G's switches at work, and `every_git_child_of_this_module_runs_with_automatic_maintenance_off`
guards them.

**Not swept:** the tests outside round 3, #329's among them. CI's two Git 2.55.0 legs ran them once each: on Linux the
lib passed 3,224 and failed only T-R10, and on macOS it passed 3,153 and failed none (`fudi4/ci/job-111437892309.log`).

#### 5.18.6 What this round leaves as it was

- **B stays a PROVISIONAL integration** at `55029628`. N1 and N2, the three instrument rows, the O8 texts, `design/15`
  and every cost and limit of §5.17.7 are unchanged, in behaviour and in text.
- **No production code changes.** No finding file changes: the CI failure is the ledger's `FUD-CI4-MAINTPRUNE`, fixed.
- **Beyond §5.17.7's statement of what is proven:** round 3's 70 tests now also ran on Git 2.55.0, on this box's Linux.
  That is a second Git on one platform, and not native-platform proof.

#### 5.18.7 The frozen effects

`fudi4/tools/frozen-proof4.sh` is round 3's proof with this round's base, `202c0805`. It ran at the code commit
`6bdd02c3` (`fudi4/frozen/frozen-proof-6bdd02c3.txt`). This record's commit changes no code, and the pull request body
cites the same proof at the head it is pushed at.
- **G6's frozen set:** **zero delta from #329's provisional head `55029628`,** 34 of 34 byte-identical. From master,
  33 of 34: `src/engine/topology/integrate.rs` differs by +16 −4, #329's H1, as in §5.17.8. This round's delta over the
  set is zero.
- **Against G5's range `d724fb16`:** unchanged from §5.17.8, five files, +257 −78. E-G6-1's rule, executed and not
  adopted, fails on #329's H1 alone.
- **The PR5-frozen legacy section:** from `55029628`, exactly D's five differ.
- **D's O8 five-file set:**
  - **this round** (`202c0805..6bdd02c3`): only `src/engine/tests.rs` changes, +4 −0 (blob `f8c1963f…`).
    `src/workspace.rs`, `src/engine/coordinator.rs` and `src/engine/resume.rs` keep their round-3 blobs, and
    **`src/engine/attempt.rs` is unchanged** (`51534706…`);
  - **cumulative from master** (the same from `55029628` and from `54a1ff14`): `src/workspace.rs` +1,537 −82,
    `src/engine/attempt.rs` +53 −10, `src/engine/coordinator.rs` +116 −10, `src/engine/resume.rs` +138 −4, and
    `src/engine/tests.rs` +4,844 −0.
- **Append-only** (`fudi4/frozen/append-only-6bdd02c3.txt`): against master, `src/engine/tests.rs` is master's blob with
  bytes appended, and `src/workspace.rs`'s test module is unchanged from round 3. Against `202c0805`, the four lines go
  inside T-R10, in D's own appended region.
- **Schema 4:** `TOPOLOGY_ACTIVATION` is `Inactive`, and no topology path changes.
- **Every other path this round changes:** `docs/internals/engine/tests.md`, and this record.

#### 5.18.8 The boundary event

The orchestrator's `~/orch-pr11/answers/pr11_fud_impl4-0.md` (13:07:33Z) carries the supervisor's correction. Before
it, while looking for a Git 2.55.0 build, this round ran two broad traversals. They are preserved, with each command,
its start and end times and its output files, in `fudi4/boundary/` (`commands.md`):
- **`find / -maxdepth 6 -type f -name git -perm -u+x`** ran from 13:04:37Z to 13:05:17Z. It could have enumerated the
  immediate entries of `~/orch-pr11/reviews/rd1r2-witnesses/` (depth 6 below `/`), but nothing below them.
- **`find /home/ubuntu /srv /opt -xdev -type f -name git -perm -u+x`** started at 13:05:23Z, and this round stopped it
  at 13:07:44Z. It could have enumerated all of that directory, at every depth, and the stopped review's rollouts
  wherever they lie under `/home/ubuntu`. Its second command, a walk of `~/orch-pr11` to depth 4, never started.

Neither result is read as an absence: a walk with a name filter still enumerated what it walked. Of the 40 paths the
first printed, none names the directory. All 40 were discarded, none was used, and the stopped artifacts were not
opened to audit the exposure. The Git 2.55.0 build used is the one the orchestrator's second note names. A shallow
clone of Git's `v2.55.0` tag, made before the correction, served only to read Git's source (`fudi4/git-source/`).

#### 5.18.9 Platforms, what waits, and the CI history

- **Platforms:** every figure here is Linux, on this box's Git 2.43.0 and the Git 2.55.0 stand-in. Windows and macOS
  are CI's.
- **What waits** is as §5.17.11 has it: O8, #329's final head, the required reviews (the regular review of R-D1's
  design unmet), and N1's and N2's G6 classification.
- **CI at `202c0805`:** run 37202558686 failed `test (ubuntu-latest)` on T-R10 (§5.18.1). Every other job passed, among
  them `test (macos-latest)` and `test (winguest)`. Policy run 37202558600 passed.
- **CI at this round's head is not waited on;** the orchestrator reads native CI.

The pull request body records the head the ten gates passed at, with the logs under `fudi4/gates/`.

### 5.19 Repair round 5: D2's D-I2-1, the understated orphan-cleanup cost

> **On §5's terms, as §5.18 left them:** a draft, PROPOSED, conditional on the owner's decision O8, and not merge-ready.
> Its merge waits on O8's adoption, on #329 merged (O9, O14, O11), on the integration of #329's *final* head (the merge
> of `55029628` stays PROVISIONAL), and on the required reviews: **the regular review of R-D1's design is unmet**, and
> nothing here substitutes for it. Nothing here adopts an owner decision, a contract or a freeze. The round adds two
> commits on `fb1717b1`, the text-and-test fix `8ee1353d` and this record's commit, with no rebase, and no
> `reviewed_sha` is re-stamped.

**Who and on what.** `pr11_fud_impl5` (`claude-opus-5-5`, `max`), a fresh repair session `orch_pr11` spawned on
2026-10-04 on `fb1717b1`. Its brief is `~/orch-pr11/briefs/pr11_fud_impl5.md`; round 1's to 4's briefs and
`~/orch-pr11/d-impl/UNIT.md` bind where it does not change them. Its figures are under
`~/orch-pr11/logs/pr11_fud_impl5/`, cited as `fudi5/…`. The stopped round-2 regular review of R-D1's design was not
read, opened, run or used, and nothing here retries, rewords, reroutes or relabels it. This round ran no `find`,
`ls -R`, `du` or `grep -r` over the roots its brief forbids; its searches were `git grep` in its own worktree.

#### 5.19.1 The finding

- **D2** is #331's review round i2 at `fb1717b1`: one lens, regression (`gpt-6-astra`, `max`), CHANGES_REQUIRED
  (`~/orch-pr11/reviews/review-331-i2-regression-fb1717b1.review.md`; the triage `review-331-i2-triage.md`). It is not
  a two-lens review. R-D1's required regular review stays HELD and UNMET, and D2 does not stand in for it.
- **D-I2-1 (P3, executed):** §5.17.7, the notes of `PRIVATE_INDEX_CONTROLS`, the R-D7 finding and the pull request body
  said that the orphan `sharedindex.*` files an inherited `GIT_TEST_SPLIT_INDEX` leaves are removed by Git's next split
  write of the checkout's index. On Git 2.43.0, in an ordinary and a linked checkout, the reviewer split the checkout's
  index with the default expiry, ran the private capture with `GIT_TEST_SPLIT_INDEX=1` and G4's exact split-index
  controls, removed the private index and ran the checkout's next `git update-index --split-index`: **both orphans
  remained.** Expiry `now` removed them, and `never` kept them
  (`~/orch-pr11/reviews/331-i2-witnesses/review-split-expiry-0imgakuv/`: `witness.py`, `ordinary-and-linked.json`,
  `result.json`).
- **Why the evidence had not caught it:** the engine tests split the checkout's index through
  `split_the_checkouts_index`, which sets the expiry to `now` (`src/engine/tests.rs:14997`), so they could not support
  the broader claim.
- **The triage's disposition:** fix, mandatory, since the finding carries an executed reproduction. Qualify the record,
  the body, the notes and the R-D7 finding, and pin the notes with an `include_str!` test, red at `fb1717b1` and green
  after.
- **Where the claim stood at `fb1717b1`** (`fudi5/notes/claim-sites-fb1717b1.txt`: `git grep`, and the live body):
  §5.17.7 (`:3724`–`3726`), `docs/internals/workspace.md:118`–`123`, the R-D7 finding's `:70`–`71`, and the body's
  costs. R-D1's proposal says it too, in its §4.6.3, §6.4, §9.1 (twice) and §9.3
  (`fudi5/notes/proposal-claim-sites.txt`, with the file's sha256 `f9e81c07…`). That file is the owner package's, not
  this branch's, and this round does not edit it; for this implementation §5.19.3's statement supersedes those
  sentences.

#### 5.19.2 Reproduced

**The reviewer's sequence, executed again** (`fudi5/repro/repro.py`), on this box's Git 2.43.0 and on the permitted
Git 2.55.0 build (`fudi5/repro/git-binaries.txt`). Each capture is the engine's own Git sequence at `fb1717b1` for a
checkout with nothing in the revert's way (`fill_capture`, `private_tree`, `changed_paths`: `read-tree`,
`add -A --ignore-errors`, `write-tree`, `diff-tree`), each child built as the engine builds it (`git_command`'s
settings and `GIT_NO_REPLACE_OBJECTS`, a private `core.hooksPath`, `core.fsmonitor=false`, `PRIVATE_INDEX_CONTROLS`
with `GIT_INDEX_FILE` on a private file in the checkout's own Git directory, and `CAPTURE_CONTROLS`), with
`GIT_TEST_SPLIT_INDEX=1` inherited; the private file is then removed, as `CheckoutCapture::remove` does. It models the
engine's Git sequence; it does not run the engine. "An eligible write" below is the checkout's own
`git update-index --split-index` on its split index, and the script checks that each one created a new shared index
file: the index's shared index has a new inode, since Git writes a new one to a temporary file and renames it into
place, and identical content keeps the same name. Both Gits gave the same counts (`fudi5/repro/summary-git-2.43.0.txt`,
`summary-git-2.55.0.txt`, `checks.txt`; every file name and command line in `report-*.json` and `commands-*.json`):

| Checkout | Expiry | Orphans after one capture | Left by the next eligible write | Aged, then another eligible write |
|---|---|---|---|---|
| ordinary and linked | the default (unset) | 2 | 2 | aged 13 days: 2 left; aged 15 days: 0 |
| ordinary and linked | `never` | 2 | 2 | aged 400 days: 2 left |
| ordinary and linked | `now` | 2 | 0 | — |

- **Accumulation:** three captures, each with new output and each followed by an eligible write, left 2, 3 and 4
  orphans, under the default and under `never`, in both checkouts.
- **A split write that creates no shared index file runs no expiry:** with the expiry `now` and 30 tracked files, the
  checkout's `git add` of one changed file wrote its split index on the same shared index and left both orphans; the
  eligible write after it took them.
- **A checkout whose own index is not split** kept both under `now` through the operator's later `add`, `commit`, `add`
  and `status`, run without the variable; its index stayed unsplit, so no split write of it ran.
- **Git's own documentation** (Git 2.43.0's `git-config` and `git-update-index`,
  `fudi5/repro/git-2.43.0-docs-splitindex.txt`): shared index files "that were not modified since the time this
  variable specifies will be removed when a new shared index file is created"; the default is "2.weeks.ago", and
  "never" suppresses expiration altogether.
- A first run's eligible-write column compared the shared index's name only, which identical content keeps. It is
  superseded, and its orphan counts equal the rerun's (`fudi5/repro/superseded-name-only-detection/SUPERSEDED.txt`).

#### 5.19.3 The true reach, stated where the claim stood

Under an inherited `GIT_TEST_SPLIT_INDEX` the private index's writes unlink nothing, and leave orphan `sharedindex.*`
files in the checkout's Git directory. **Git removes an orphan only at a later split write of the checkout's own index
that creates a shared index file, and only once the orphan is older than the checkout's configured
`splitIndex.sharedIndexExpire`.** With the default, `2.weeks.ago`, an orphan stays until it is older than two weeks
and such a write follows; with `never`, automatic expiry never reclaims it; and repeated captures can accumulate them.
It stays O3's class, with FUD-D4-ENV's, and a cost for the owner's choice: disclosed, not accepted.
- **§5.17.7** is corrected in place and marked.
- **`docs/internals/workspace.md`,** the section of `PRIVATE_INDEX_CONTROLS`, states the same. Its premise is made exact
  too: "every split write" unlinks the older shared indexes now reads every split write that creates a shared index
  file, Git's documented trigger, which the write of one file above shows.
- **The R-D7 finding:** its bullet is corrected in place, and a dated section added; `reviewed_sha` is unchanged.
- **The body's costs,** and a ledger row, `FUD-I2-ORPHANEXPIRY`, `fixed`.

No behaviour changes: the engine's Git sequences, the controls and the cost are what they were. Only the statement of
the cost's reach changes.

#### 5.19.4 The pin

`workspace::tests::the_private_index_notes_tie_an_orphan_shared_index_to_the_checkouts_expiry`, appended to
`src/workspace.rs`'s test module, under the lane's convention for a notes misstatement. It `include_str!`s
`docs/internals/workspace.md`, splits it on `"\n## "`, finds the section by its heading, collapses its whitespace, and
asserts six clauses of the corrected statement present (the eligible write, the expiry, the default's window, `never`,
the accumulation, the O3 class) and the retired clause absent. It goes through the notes because `Cargo.toml` excludes
`findings/` and `reviews/` from the package. No behaviour test can guard the sentence: Git's behaviour is unchanged, so
one would be green at `fb1717b1`. The reviewer's sequence, executed again in §5.19.2, is the evidence. The test
carries no comment (§13); its notes section says what it pins.

The runs (`fudi5/pin/MATRIX.txt`): `pin-run.sh` builds each tree through `upstroke-build` on this round's private base,
and a run counts only if its log names the tree on its Compiling line.
- **Red** applied alone to a `git archive` of `fb1717b1`, the test without the notes' correction
  (`red-tree-construction.txt`): rc 101 on the first clause, twice (`red-fb1717b1.log`, `red-fb1717b1-2.log`).
- **Green** at `8ee1353d` (`green-8ee1353d.log`), and with the notes and `src/workspace.rs` given CRLF line ends, as a
  winguest checkout has them (`mut-crlf.log`).
- **Each assertion bites alone** (`pin-matrix.sh` and `notes-mutate.py`, on a `git archive` of `8ee1353d`): each of the
  six clauses reworded fails on that clause; the retired clause put back fails on the absence check; the heading
  renamed fails to find the section; and the corrected statement moved to another section fails on the first clause.
  The control is green.
- An earlier green run is not counted: it reused the red tree's binary in the shared slot
  (`fudi5/pin/superseded/SUPERSEDED.txt`).

#### 5.19.5 Platforms, the frozen effects, and what stays

- **Platforms** (`fudi5/platform/code-8ee1353d/`, each Checking line naming this worktree): Windows clippy, Windows
  MSRV with `-D warnings`, macOS clippy and the Linux MSRV with `-D warnings`, each rc 0 with no warning. Nothing ran
  on Windows or macOS here; CI is the truth for those legs.
- **The frozen effects** (`fudi5/tools/frozen-proof5.sh`, round 4's proof with this round's base `fb1717b1`, at the code
  commit: `fudi5/frozen/frozen-proof-8ee1353d.txt`; this record's commit changes no code, and the pull request body
  cites the proof at the head it is pushed at):
  - **G6's frozen set:** zero delta from #329's provisional head `55029628`, 34 of 34 byte-identical. From master, 33
    of 34: `src/engine/topology/integrate.rs` differs by +16 −4, #329's H1, as in §5.17.8. This round's delta over the
    set is zero.
  - **Against G5's range `d724fb16`:** unchanged, five files, +257 −78. E-G6-1's rule, executed and not adopted, fails
    on #329's H1 alone.
  - **The PR5-frozen legacy section:** from `55029628`, exactly D's five differ.
  - **D's O8 five-file set, this round** (`fb1717b1..8ee1353d`): `src/workspace.rs` +55 −0, inside its test module
    (blob `213f67a8…`). `src/engine/coordinator.rs`, `src/engine/resume.rs` and `src/engine/tests.rs` keep their
    blobs, and **`src/engine/attempt.rs` is unchanged** (`51534706…`).
  - **Cumulative from master** (the same from `55029628` and from `54a1ff14`): `src/workspace.rs` +1,591 −81,
    `src/engine/attempt.rs` +53 −10, `src/engine/coordinator.rs` +116 −10, `src/engine/resume.rs` +138 −4, and
    `src/engine/tests.rs` +4,844 −0. §5.18.7 gave `src/workspace.rs` +1,537 −82: `git diff` now pairs one line,
    `self.remove_prepared_pin(prepared)`, as unchanged where it listed it as deleted and added, so 1,537 + 55 − 1 and
    82 − 1. The production region is byte-identical to `fb1717b1`'s (`fudi5/frozen/numstat-pairing.txt`).
  - **Append-only** (`fudi5/frozen/append-only-8ee1353d.txt`): against master and against `fb1717b1`,
    `src/workspace.rs`'s test module keeps every byte it had before its closing brace, with 2,352 bytes added this
    round; `src/engine/tests.rs` is master's blob with bytes appended, and unchanged this round.
  - **Schema 4:** `TOPOLOGY_ACTIVATION` is `Inactive`, and no topology path changes.
  - **Every other path this round changes:** `docs/internals/workspace.md`, the R-D7 finding, and this record. No
    instrument changes: nothing under `effects/`, `clippy.toml`, `src/runner/contract.rs`, `.github/`, `scripts/`,
    `.cargo/` or `Cargo.toml`, and no second-limb path.
- **What stays as it was:** B's PROVISIONAL integration (`55029628`); N1 and N2; the three instrument rows; the O8
  texts; `design/15`; and every cost and limit of §5.17.7, apart from this correction of the O3 exception's reach. No
  production code changes. The held regular review's questions, C-SIDE (O1), ENV-1 (O3) and the O7 method are not
  touched.

#### 5.19.6 What waits, and the CI history

- **What waits** is as §5.17.11 has it: O8, #329's final head, the required reviews (the regular review of R-D1's
  design unmet; this repair unreviewed), and N1's and N2's G6 classification.
- **CI at `fb1717b1`:** run 37207835491 passed every job and the `upstroke-ci` rollup, and the policy run 37207835464
  passed (`fudi5/ci/jobs-37207835491.tsv`, `runs-fb1717b1.json`); the two cancelled runs at that head were superseded
  by the body edit.
- **CI at this round's head is not waited on;** the orchestrator reads native CI.

The pull request body records the head the ten gates passed at, with the logs under `fudi5/gates/`.
