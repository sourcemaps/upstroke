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
out (§3); its figures are under `~/orch-pr11/logs/pr11_fub_design4/`, cited as `d4/…`. **Design round 5** is
`pr11_fub_design5`'s (`claude-opus-5-5`, `max`), spawned on `a6135a66` to answer design review round 4 (§4); its
figures are under `~/orch-pr11/logs/pr11_fub_design5/`, cited as `d5/…`. **Design round 6** is `pr11_fub_design6`'s
(`claude-opus-5-5`, `max`), spawned on `4a126215` to carry out the orchestrator's decision on design review round 5:
narrow this change to the topology registry race, move the legacy half to follow-up D, and evaluate a store-activity
window first (§5); its figures are under `~/orch-pr11/logs/pr11_fub_design6/`, cited as `d6/…`. **Design round 7** is
`pr11_fub_design7`'s (`claude-opus-5-5`, `max`), spawned on `ed3a97d9` to answer design review round 6 (§6); its figures
are under `~/orch-pr11/logs/pr11_fub_design7/`, cited as `d7/…`. **Design round 8** is `pr11_fub_design8`'s
(`claude-opus-5-5`, `max`), spawned on `85f5b09b` to carry out the orchestrator's decision on design review round 7:
narrow, withdraw round 7's probe, decide the failure after an add's takeover, and file the external-prune class (§7);
its figures are under `~/orch-pr11/logs/pr11_fub_design8/`, cited as `d8/…`. **Design round 9** is `pr11_fub_design9`'s
(`claude-opus-5-5`, `max`), spawned on `f7a9256c` to answer design review round 8, the last design round before the
owner's consolidated question: the external-prune finding's closures made accurate, face 2's boundary, and two rules
(§8); its figures are under `~/orch-pr11/logs/pr11_fub_design9/`, cited as `d9/…`. Every figure below is in a saved
file the sentence names. **The implementation** (§9) is `pr11_fub_impl`'s (`claude-opus-5-5`, `max`), spawned on
`8df42436` after design review round 9; its figures are under `~/orch-pr11/logs/pr11_fub_impl/`, cited as `impl/…`.
**Its repair round 2** (§9.12) is `pr11_fub_impl2`'s (`claude-opus-5-5`, `max`), spawned on `59d206b3` after CI went red
there; its figures are under `~/orch-pr11/logs/pr11_fub_impl2/`, cited as `impl2/…`. **Its repair round 3** (§9.13) is
`pr11_fub_impl3`'s (`claude-opus-5-5`, `max`), spawned on `54a1ff14` after the implementation review's first round; its
figures are under `~/orch-pr11/logs/pr11_fub_impl3/`, cited as `impl3/…`. **Its repair round 4** (§9.14) is
`pr11_fub_impl4`'s (`claude-opus-5-5`, `max`), spawned on `f9c88fdb` after #331's `test (winguest)` failed in this
change's shared manager code; its figures are under `~/orch-pr11/logs/pr11_fub_impl4/`, cited as `impl4/…`.
**Its repair round 5** (§9.15) is `pr11_fub_impl5`'s (`claude-opus-5-5`, `max`), spawned on `c8aab155` after
`test (winguest)` failed round 3's R-T census on the guest's CRLF line endings, at `f9c88fdb` and again at `c8aab155`;
its figures are under `~/orch-pr11/logs/pr11_fub_impl5/`, cited as `impl5/…`. **Its repair round 6** (§9.16) is
`pr11_fub_impl6`'s (`claude-opus-5-5`, `max`), spawned on `a58c2ce3` after the delta review of repair rounds 3 to 5; its
figures are under `~/orch-pr11/logs/pr11_fub_impl6/`, cited as `impl6/…`. **Its repair round 7** (§9.17) is
`pr11_fub_impl7`'s (`claude-opus-5-5`, `max`), spawned on `a337efa7` after `test (winguest)` failed round 6's integration
witness there; its figures are under `~/orch-pr11/logs/pr11_fub_impl7/`, cited as `impl7/…`. **Its repair round 8**
(§9.18) is `pr11_fub_impl8`'s (`claude-opus-5-5`, `max`), spawned on `519cfc9e` after the delta review of repair rounds
6 and 7; its figures are under `~/orch-pr11/logs/pr11_fub_impl8/`, cited as `impl8/…`. **Its repair round 9** (§9.19)
is `pr11_fub_impl9`'s (`claude-opus-5-5`, `max`), spawned on `ce55ca91` after the delta review of repair round 8; its
figures are under `~/orch-pr11/logs/pr11_fub_impl9/`, cited as `impl9/…`. **Its step-5 round** (§9.20) is
`pr11_fub_step5`'s (`claude-opus-5-5`, `max`), spawned on `55029628` after the supervisor's step-5 merge triage made
round 8's two findings mandatory merge work; its figures are under `~/orch-pr11/logs/pr11_fub_step5/`, cited as
`step5/…`. **Its CAS-1 round** (§9.21) is `pr11_fub_cas1`'s (`claude-opus-5-5`, `max`), spawned on `4253b2ca` on an
isolated branch, after follow-up C's integration round found that a publication's compare-and-swap re-check waited on
the coordinator's thread; its figures are under `~/orch-pr11/logs/pr11_fub_cas1/`, cited as `cas1/…`. **Its B4 round** (§9.22) is
`pr11_fub_impl10`'s (`claude-opus-5-5`, `max`), spawned on `d7865780` after the i5 delta review, with follow-up C's
fix P routed to it; its figures are under `~/orch-pr11/logs/pr11_fub_impl10/`, cited as `impl10/…`.

## 0. Status

| Phase | State |
|---|---|
| Design rounds 1 and 2 (§1) | **Superseded by §2.** Round 1's lock handed to the Git child, and round 2's engine-only lock with a process record and quiescence waits, are withdrawn, with E-FUB-1, R29, Class C and round 2's unfreeze text. §1 is kept as the history the review rounds cite. |
| Design round 3 (§2) | **Superseded by §3 where §2's banner says.** Design review round 3 (three `gpt-6-astra` lenses at `max` on `8dd2214c`, the design lens refused on [cyber] grounds and recast) returned CHANGES_REQUIRED from all three (`~/orch-pr11/reviews/review-329-d3-triage.md`). Its P1s in round 3's lease raised the looping signal a third time. |
| Design round 4 (§3) | **Superseded by §4 where §3's banner says.** Design review round 4 (three `gpt-6-astra` lenses at `max` on `a6135a66`: design as a conformance reading, concurrency, regression) returned CHANGES_REQUIRED from all three (`~/orch-pr11/reviews/review-329-d4-triage.md`): two P1s executed (the own-entry exception; B1′'s retry predicate), three P2s and one P3. The P1 in round 4's own exception raised the looping signal a fourth time. |
| Design round 5 (§4) | **Superseded by §5 where §4's banner says.** Design review round 5 (three `gpt-6-astra` lenses at `max` on `4a126215`: design, concurrency, regression) returned CHANGES_REQUIRED from all three (`~/orch-pr11/reviews/review-329-d5-triage.md`): three P1s executed (C3's spelling, an optional file made and unmade, B-PRESERVE's mutable index), two P2s and one P3. The P1s in round 5's own machinery raised the looping signal a fifth time. The legacy half, corrected B1′ and B-PRESERVE, moved to follow-up D. |
| Design round 6 (§5) | **Superseded by §6 where §6's banner says.** Design review round 6 (three `gpt-6-astra` lenses at `max` on `ed3a97d9`: concurrency on its first run; design and regression each refused on [cyber] grounds twice and recast as conformance readings, which ran) returned CHANGES_REQUIRED from all three (`~/orch-pr11/reviews/review-329-d6-triage.md`): one P1 executed (FUB-D6-PRUNE, a prune after the add's takeover), four P2s (INODE executed; DABSENCE, STATICRESUME and BOUND reasoned) and one P3 (PLATFORM). The P1 in round 6's own takeover veto raised the looping signal a sixth time. Round 6 chose no classifier (§5.3) and published the helper's contract for follow-up D (§5.5, now with a dated change). Its witnesses ran on Linux on Git 2.43.0 and 2.55.0 and a subset on the Windows guest's 2.50.1 (§6.10). This head changed no production code. |
| Design round 7 (§6) | **Superseded by §7 where §7's banner says.** Design review round 7 (three `gpt-6-astra` lenses at `max` on `85f5b09b`: design, concurrency and regression in round 6's recast conformance-reading form, none refused) returned CHANGES_REQUIRED from all three (`~/orch-pr11/reviews/review-329-d7-triage.md`). It found three P1s: SPLITINDEX and CONFIG, executed in round 7's own probe, and R13, an applicable high the G6 table omitted. It also found two P2s (ENVCENSUS, R9WIN) and a note on the widened store-absent exception. The P1s in round 7's own machinery raised the looping signal a seventh time, in its strongest form. Round 7 published the helper's three-way contract, the final attempt, the end-to-end bound and `CONTENDED_ATTEMPTS`, and all three lenses accepted them. Its Git-level evidence ran on Linux only (§6.10). |
| Design round 8 (§7) | **Superseded by §8 where §8's banner says.** Design review round 8 (three `gpt-6-astra` lenses at `max` on `f7a9256c`: design, concurrency and regression in the conformance-reading form, none refused) returned CHANGES_REQUIRED from all three (`~/orch-pr11/reviews/review-329-d8-triage.md`). All three found that the narrowing holds. The P1s were in the external-prune finding's candidate closures (closure 1 passed a partly deleted registration and missed failed gates; closure 2 missed `Missing`) and in face 2's boundary (a deletion before the add returns still gives Ok). Five P2s and P3s: the populated destination, scheduled maintenance, R14's G6 row, the host width, T15. Round 8's narrowing (§7.2, §7.3) stands. |
| Design round 9 (§8) | **Implemented where §9 says.** Design review round 9 (three `gpt-6-astra` lenses at `max` on `8df42436`: design, concurrency and regression; then two added focused lenses, preserve and gitenv) returned CHANGES_REQUIRED from all five (`~/orch-pr11/reviews/review-329-d9-triage.md`). B's own narrowed mechanism holds in all three general lenses. Every P1 is in the external-prune finding's candidate closures, except FUB-D9-ENV, a pre-existing defect of the Git builders, which is O3's. Round 9 was the last design round before the owner's consolidated question. It adds no machinery and implements no closure. A destination that is not an empty directory at the start is Git state before Git's add runs, once the add's prevalidation has passed (§8.2, as qualified at repair round 3). Face 2 starts at the prune's decision (§8.3). The external-prune finding's closures are made accurate: closure 1 a whole-registration check at every durable negative outcome, stated as a partial mitigation with two P1 residuals after the orchestrator's addendum (§8.4), closure 2 preservation at the destructive boundaries, a retained retry's included (§8.5), closure 4 qualified for scheduled maintenance, effective configuration and a prune already running (§8.6); the host width is reconciled (§8.7); R14 needs the owner's disposition before G6 (§8.8); the G6 table is given again (§8.9); T15 is fixed (§8.10); and the owner gets options with a recommendation (§8.11). The witnesses ran ordinary Git commands on Linux, on upstream 2.43.0, 2.50.1 and 2.55.0. This head changes no production code. |
| Implementation (§9) | **Implemented; the pull request stays a draft.** `58c7c203`, on a merge of master `5c222ff2` (§9.2), with repair round 2's two test-only commits after CI was red at `59d206b3`: rustc 1.99.0's deprecation of `fetch_update` in three test policies, and two tests' planted registrations on Windows. The same round files FUB-D9-ENV (§9.12). It contains the tolerant registry access, the add's destination and veto, targeted removal with no engine prune, the typed refusal, the one instrument row, B's `design/15` paragraph in force and FUB-D9-TAKEOVERWORD, with #328's `LinkedChild` bound carried in. The merge waits on O9, O14 and O11, and `design/26` is untouched pending them (§9.11). It does not depend on follow-up C or D. D's implementation follows this change's merge, because D calls the helper (§5.5, §6.4). **Repair round 3** (§9.13) fixes the implementation review's R1 to R6 (two `gpt-6-astra` lenses at `max` on `54a1ff14`, CHANGES_REQUIRED, no P1). R1: no registry retry waits on the coordinator's thread; the coordinator answers its messages during the wait, on every census path. Four of those paths are in the frozen `integrate.rs`, so the round carries **a proposed frozen hunk, H1 and H2 (`e369b251`, +33/−7), conditional on the owner's freeze ruling and not adopted** (§9.13.1). The merge also waits on that ruling. R2 to R6 are a held retry, an owned writer, a qualified guarantee, a corrected precondition and Git's spelling. **Repair round 4** (§9.14) fixes R7, the shared Windows CI failure that #331's `test (winguest)` met in this change's manager code: the add's gate, and a verification's lookup, resolve the paths a worktree list names inside the list's registry access, so a sibling whose checkout another removal is deleting no longer fails them, and a path that stays unreadable refuses resumably. The read is master's; this change's lock-free lists and removals widened its window. It changes no frozen file. **Repair round 5** (§9.15) fixes R8: round 3's R-T census matched a line break in `run.rs` as read from disk, so the Windows guest's CRLF checkout failed it at `f9c88fdb` and `c8aab155`. It now normalises the line endings first, and no other source census in the crate depends on them. It changes no frozen file and no production code. **Repair round 6** (§9.16) fixes the delta review's I2-1 to I2-7 (two `gpt-6-astra` lenses at `max` on `a58c2ce3`, CHANGES_REQUIRED, no P1). A shutdown answered inside a registry access's wait now stops its transition, and closure and finalization wait through the coordinator. One timer thread is started ahead of need, and where none can start the coordinator refuses, typed and resumable, never sleeping. R-T is checked for every call, every coordinator witness is bounded, and the body's populated-destination sentence is corrected. It withdraws H2 through a non-frozen adapter, so **the proposed frozen change is H1 alone** (+16/−4, §9.13.1 amended), and the merge waits on the freeze ruling for H1. **Repair round 7** (§9.17) fixes `test (winguest)`'s failure of round 6's integration witness at `a337efa7`. The tear's prober reported a cancel without sampling after it, so a contended attempt counted during its last wait, about 1.2 ms before the cancel, read as none wherever its waits are as coarse as Windows' clock tick. It now samples after every wait. The fix is test code only, and no frozen file changes. **Repair round 8** (§9.18) answers the delta review of rounds 6 and 7 (two `gpt-6-astra` lenses at `max` on `519cfc9e`, CHANGES_REQUIRED, two P3 evidence findings and no production defect). Round 6's failure of a frozen recovery test is recorded as an unexplained candidate regression of undetermined provenance, and round 7's lease refusal as the fingerprint the filed PR281 records, its cause in that sighting unproven. Each is filed as its own finding with its G6 obligation, and every sentence that set either aside or attributed it beyond the evidence, or did so to a sibling sighting of the same shape, is corrected in place. It changes no source file. **Repair round 9** (§9.19) answers the delta review of round 8 (two `gpt-6-astra` lenses at `max` on `ce55ca91`: the regression lens PASS, the regular lens one P3). §9.18.4's conclusion that step 5's rule for a witnessed finding did not apply to round 8's two findings is withdrawn. In its place stand round 8's text-only scope and the deferred investigation, and whether those findings, each carrying an archived failing test, must be fixed before B's merge or are carried by their guards is left to B's merge triage, neither asserted nor waived. It changes no source file and no finding. **The step-5 round** (§9.20) follows the supervisor's step-5 merge triage, which made those two findings mandatory merge work. Its isolated diagnosis reads both as one pre-existing class: a fork that another test thread makes during this test process's own ref write keeps a copy of the run's cleanup lease, and a single observation reads it held. One natural failure of each shape was attributed by deduction to the lease branch, with no holder captured: W2's at this head, and at master the helper's observation in a different test that shares W1's helper. W1's own test failed before this change only by construction, and the first-bad controls are constructions too. The fork construct's counts are an injected construct's, not natural failure rates, and the heterogeneous natural suites establish no rate effect and no absence of exposure this change induces. The class attribution, the first-bad controls, and whether H3 repairs the tests faithfully without weakening their obligation await independent review, and nothing here claims a production cause or a closure (restated at the CAS-1 round, §9.21.9). It proposes **H3**, a frozen hunk in `recover/tests.rs` alone (+133/−3 as first proposed; revised at the B4 round, §9.22.3): W1's observation and W2's first resume make the bounded wait #320 makes before every later resume, with three regression tests. H3 is in RULING P-1's form (PROPOSED RULING B-W), not adopted, and the merge waits on its freeze ruling as on H1's. Both findings stay filed, provenance `pre_existing`, until the change that merges H3 deletes them. **The CAS-1 round** (§9.21) routes the publishability re-check that `compare_and_swap_ref` makes before its funnel through the call's own hooks. The frozen `publish` hands the swap the coordinator's hooks, and the re-check had asked the hook-less check, so during an integration's publication its waits slept on the coordinator's thread. The call is master's, but the waits are this change's: its tolerant access (`58c7c203`) gave that list its backoff, and R1's census missed the site. Follow-up C's integration round found it. Its four witnesses are red at `4253b2ca` and green after. R1's census gains row C6, and a census over the manager now reports any function that takes hooks and reaches a sleeping registry wait. A probe over the topology suite finds no other coordinator-thread access that sleeps. It changes no frozen file and adds no kind of refusal. It also restates step 5's evidence where the record read beyond it (§9.21.9). **The B4 round** (§9.22) answers the i5 delta review (two `gpt-6-astra` lenses at `max` on `d7865780`: the regression lens PASS, the regular lens two findings). I5-1 (P2, executed): H3's waits read an inspection error as a held lease and retried it to success, so H3 is revised, still proposed and not adopted. Its waits tell an inspection error from lease contention before any wait, an observation that fails fails at once, and the reviewer's two witnesses are regression tests, red at `d7865780` and green after, with the pre-H3 controls. I5-2 (P3): CAS-1's census builds its paths with `Path`. The round also applies follow-up C's fix P, routed to it: a tear witness's repository waits to the production deadline, and a new witness reproduces the Linux stand-in's mechanism behind #330's guest failure, with production behaviour unchanged. The two Windows retries carry the reviewer's reading, undecided. |

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

> **SUPERSEDED by §4 (design round 5) where this banner says; the rest of §3 stands as §4 cites it.**
> - **Withdrawn:** §3.3's own-entry exception (its paragraph "The add's own entry", the `Own entry only` verdict and
>   the own-entry branch of "Why it is exact"); §3.7's claim 4 ("after exactly one more attempt"); §3.10's B1′ text,
>   its unfreeze text and its four legacy enumerators; §3.7's and §3.10's G6 reading of the legacy finding ("the lenses
>   split"; "if … B1′ is not taken"); and §3.6's and §3.10's statement that B1′'s instrument edits make it the owner's
>   to merge (§4.6).
> - **Replaced:** §3.3's C3 and verdict table by §4.3; the refusal's type in §3.3 and §3.4 by
>   `UpstrokeError::RegistryRefused` (§4.3); §3.7's claims, rows R6, R7 and R9 and its G6 tables by §4.7; §3.8's T4 and
>   T12 by §4.8; §3.10's B1′ and "What B1′ does not close" by §4.4 and §4.5.
> - **Stands:** §3.1, §3.2, §3.3 otherwise (the attempt, TORNOK, the two reads, C1, C2, the parse inside the list,
>   where it applies), §3.4 otherwise, §3.5, §3.6's measurements, §3.8's other tests, §3.9, §3.11 as history, and
>   §3.12 with §4.10's additions.
>
> *Round 4's banner:* **PROPOSED — for design review round 4.** This section supersedes §2 where it says so (the banner
> at §2's head lists where). It is narrowed: #329 repairs the registry race and nothing else. The topology closure needs
> no owner decision. It adds no resource row, effect site or fault row, edits no frozen module and moves no instrument
> (§3.6). **Decision B is the owner's** (`~/orch-pr11/ESCALATION.md` item 7). §3.10 prepares B1′, wrapping the four
> legacy registry enumerators, as the proposal, and keeps B2′, the filing, as the fallback. Neither is implemented.
> Nothing in §3 is in force until the implementation lands.

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

## 4. Round 5 design

> **SUPERSEDED by §5 (design round 6) where this banner says; the rest of §4 stands as §5 cites it.**
> - **Withdrawn:** §4.2's choice of the classifier with no exception, and its claims; §4.3's C3 (its spelling rule, its
>   unreadable-held clause and its "no exception"), the two reads it rests on (§3.3's C1, C2 and "Why it is exact"), and
>   §4.3's "An add's destination" (an add attempted again only while no registration names its destination).
> - **Moved to follow-up D** (`pr11_fud_design`; the orchestrator's decision on design review round 5): §4.4 (corrected
>   B1′), §4.5 (B-PRESERVE), §4.6 (decision B), the legacy rows of §4.7, §4.8's T-L1 to T-L6 and T-P1 to T-P5, and §4.10's
>   legacy risks. D starts from their text and carries FUB-D5-INDEX, FUB-D5-RESTORE and FUB-D5-UNFREEZETEXT. They are
>   kept here as history and are not #329's.
> - **Replaced:** §4.7's claims, rows and tables by §5.6; §4.8's T4, T12, T14 and T15 by §5.7; §4.9's answers stand.
> - **Stands:** §4.1 as history; §4.2's measurements, among them the retry-everything form's (§5.3 answers its costs);
>   §4.3's `UpstrokeError::RegistryRefused`; and everything of §3 that §4's banner kept.
>
> *Round 5's banner:* **PROPOSED — for design review round 5.** This section supersedes §3 where §3's banner says. The
> topology closure needs no owner decision: it is round 4's tolerant access with its own-entry exception removed (§4.2,
> §4.3), in the non-frozen `src/workspace_manager.rs` and `src/error.rs`, and it moves no instrument. **Decision B is the
> owner's** (`~/orch-pr11/ESCALATION.md` item 7) and has two parts, each a PROPOSAL with its exact unfreeze text:
> corrected B1′ (§4.4) and B-PRESERVE (§4.5). Nothing in §4 is in force until the implementation lands.

Design round 5 is `pr11_fub_design5`'s (`claude-opus-5-5`, `max`), spawned on `a6135a66` to answer design review round 4
(`~/orch-pr11/reviews/review-329-d4-triage.md`). Its figures are under `~/orch-pr11/logs/pr11_fub_design5/`, cited as
`d5/…`. The probe's index is `d5/census/probe-v/SUMMARY.txt`: scratch `git archive` copies of `a6135a66`, whose `src/`
is master's, with each shape applied; nothing of it is on the branch.

### 4.1 The looping signal again, and why this round converges

**The signal appeared again.** Design review round 4 found a P1 in round 4's own machinery: the own-entry exception,
which round 4 added to answer FUB-D3-PERM (FUB-D4-OWNENTRY, executed by the design and concurrency lenses). That is
MAINTAINING's second signal, "A pass finds a P1 in machinery an earlier round of this pull request added", a fourth time
on this pull request (§2.1 counted the first two, §3.1 the third).

**What the defect in the repair was.** Round 4 trusted a second attempt to tell an add's own permanent failure from
contention. Nothing a second attempt sees can tell them apart: another process's registration with the add's own name
can appear and vanish between the reads on every attempt, and the store and Git's text are the same both times
(§4.2). The fix removes the exception; it is not a refinement of it.

**Why this round converges.** What remains are a removed exception, two legacy predicates, and the preservation scope
the reviewers specified, each with a planned witness:
- **The removed exception** (§4.2, §4.3) leaves round 4's classifier as every lens reviewed it, minus the part that
  failed. FUB-D3-PERM offered two remedies, "Either distinguish these failures or withdraw the unconditional
  quiet-store claim". Round 4 took the first, and FUB-D4-OWNENTRY broke it; this round takes the second. An unwritable
  store now refuses at the deadline instead of failing at once, and §4.2 gives the reasons and the measurements.
- **The two legacy predicates** (§4.4) are the retry and success conditions the lenses specified for corrected B1′:
  FUB-D4-B1PREDICATE's owned, unchanged, empty destination, and FUB-D4-B1REMOVE's success decision inside the attempt.
  Both sit inside calls B1′ already named.
- **B-PRESERVE** (§4.5) is the scope all three lenses gave: `src/engine/coordinator.rs:544-548`,
  `src/engine/resume.rs:562-569`, their `effects/allowlist.toml` entries and legacy regression tests. It uses only
  primitives the legacy engine already calls.
- **Each was executed on a scratch shape** this round, through the engine where the lenses executed at the Git level
  (§4.2–§4.5), and each becomes a planned test with a mutation that turns it red (§4.8).
- **Nothing this round adds is a process, a lock, a file, an event or a packet row.** The one new type is an error
  variant, which lets a caller tell a registry refusal from others.

### 4.2 FUB-D4-OWNENTRY: retrying everything, weighed against the classifier with no exception

**The defect, executed through the engine's add.**
- The witness is the lenses' sequence, run through `WorkspaceManager::add_worktree` with a probe seam at the two points
  of each attempt. Another process's registration carrying the add's own administrative name appears after the read
  that precedes an attempt, with `gitdir` written and `commondir` empty. The add's sibling scan dies on it, and it is
  gone before the read that follows. Two windows.
- On round 4's shape the add fails as Git state after its two attempts, in 2 ms: "fatal: failed to read
  .git/worktrees/kbeta-g1/commondir: Success". A third attempt succeeds (`d5/census/probe-v/witness-runs/TABLE.txt`,
  `own_entry_two`, three rounds).

**The simplest form, evaluated first: no classifier at all ("retry everything").**
- Every failed attempt of every registry access is attempted again until the one deadline. An add is attempted again
  only while its slot condition holds. At the deadline the access refuses, carrying the last failure's text.
- That form makes no exactness claim and has no own-entry rule. It was built on round 4's shape (`patch-v-ra.py`) and
  measured beside the chosen form (`patch-v-ne.py`, §4.3), three rounds each (`witness-runs/TABLE.txt`). The suites are
  `ra-suite/suite-1.log` and `ne-suite/suite-3.log`.

| Witness | Retry everything | No exception (chosen) |
|---|---|---|
| The lenses' interleaving, two windows | Ok, third add attempt, 7 ms | Ok, third add attempt, 7–8 ms |
| The same, on every attempt | refused at 500 ms, 15 attempts | `RegistryRefused` at 500 ms, 15 attempts |
| FUB-D3-PERM's construction (store not writable) | refused at 500 ms | `RegistryRefused` at 500 ms |
| A whole registration whose `HEAD` names nothing (list) | refused at 500 ms | Git at 0 ms |
| A snapshot whose checkout cannot be made (a 300-byte name) | refused at 504 ms, 15 attempts | Git at 6 ms, 1 attempt |
| TORNOK, static / transient | refused at 500 ms / Ok at 169 ms | `RegistryRefused` at 500 ms / Ok at 169 ms |
| A verification whose snapshot the checkout cannot make (engine) | `Err(Refused)`; no `merge_verification_unavailable`; the log ends at `merge_verification_started` | Complete; one `merge_verification_unavailable`, as on the unpatched tree |
| The whole suite | 2,998 passed, 2 failed | 2,998 passed, 2 failed |

What each form costs:
1. **A genuine permanent failure.**
   - Retrying everything makes every one a resumable refusal after the bound: 10 s in production, plus one attempt's
     runtime. That covers an unwritable store, a corrupt registration nothing is writing, an add whose checkout cannot
     be made, and an invalid reference.
   - The add's are the worst: each retry runs the whole checkout again. The test deadline allowed 15 (`TABLE.txt`,
     `checkout_cannot_be_made`), and production's deadline is twenty times longer.
   - The chosen form returns a failure that names nothing of the registry across a quiet, whole store at once, as it
     was. Only a failure about an entry the reads did not see whole and readable is retried into a refusal; FUB-D3-PERM's
     own new entry is one.
2. **The diagnostics.**
   - Retrying everything can say only that every attempt failed, and quote the last failure. It cannot say whether
     anything was writing the store.
   - The chosen form says what the store showed: an entry in progress, a change between the reads, an entry that
     existed only during the attempt, or a named entry no read could read. It adds the attempt count and the last
     failure's text. A quiet failure comes back as the Git error it is.
3. **The verification semantics.** Retrying everything guarantees that no registry access reaches `run::verified`'s
   Git arm (`src/engine/topology/run.rs:279`), so nothing durable is ever appended for a registry failure. It does
   that for failures that are not the registry's too.
   - The add is the registry access whose command also checks the snapshot out.
   - The packet's `decisions.repairs.not_repairs` keeps defer and park semantics for "foreign Git state": "at
     integration these terminate in merge_verification_unavailable with outcome Deferred or Parked".
   - The verification's notes count "a snapshot the checkout could not make" as such state
     (`docs/internals/engine/topology/run.md:469-473`).
   - Executed at the engine level (`TABLE.txt`, `verification_snapshot`): under retry-everything the verification ends
     `Err(Refused)`, with its transaction open and nothing appended.
   - The next resume then settles it `merge_verification_interrupted` (`src/engine/topology/recover.rs:1129-1141`).
     That event only releases the transaction (`src/topology/fold/apply.rs:39-41`), and the candidate verifies again
     under a new sequence.
   - So a cause that persists repeats on every resume and never reaches the defer and park limit. That is the class PR8's
     triage C3 recorded (`pr8-triage.md`, quoted at `run.md:450-453`).
   - The chosen form defers once and completes, as the unpatched tree does.
4. **The suite does not tell the two apart.** Each fails only the two non-frozen tests round 4 also moved (§3.6): no
   existing test pins what a quiet failure through a registry access becomes. T15 is planned to pin it (§4.8).

**The choice: no exception.**
- Retrying everything is rejected because its simplicity reclassifies failures that are not the registry's. Its third
  cost changes how a packet decision is applied, which this lane cannot do without the owner.
- The chosen form keeps every part of round 4's classifier that survived review, and removes the part that did not.

**What the chosen form claims.**
- **Contention is never returned as Git state.** The entry Git or a scan failed on was in progress at a read (C1),
  changed between the reads (C2), or existed only during the attempt or could not be read whole. In the last two cases
  the failure names it (C3), whatever its name; the add's own name is no exception (§4.3).
- **"The failure names it" rests on an audit** of Git 2.43.0, 2.50.1 and 2.55.0, the versions on this box and CI
  (`d3/git-src/`).
  - `add_worktree` scans its siblings (`get_worktrees` and `check_candidate_path`, v2.43.0 `builtin/worktree.c:429-430`;
    v2.50.1 `:444-445`; v2.55.0 `:478-479`).
  - It then creates its entry and that entry's `locked` file, and only after those takes the destination over
    (`junk_work_tree`, `:489`, `:504`, `:538`).
  - Every die in the scan's reads prints a path ending `worktrees/<name>/<file>`: `setup.c:325`, `:335` and `:332` for
    `commondir` ("failed to read %s"); `worktree.c:242`, `:295` and `:315` for `locked`. So does every die in creating
    the entry and its `locked` file, which name `worktrees/<name>` ("could not create directory of", "could not create
    leading directories of", "could not open … for writing").
  - The scan's other failures name the destination (`check_candidate_path`) or the reference (`invalid reference`),
    which are not contention. "Missing linked worktree name" (`worktree.c:84`) cannot arise for an entry read from the
    directory.
- **A failure that names no registry entry across a quiet, whole store is returned unchanged, at once.** That covers
  the add's checkout, its reference, its destination, and a whole registration that is corrupt.
- **What remains:** a Git version whose registry-phase failure names no entry and leaves no trace at either read. None
  of the three audited has one. §4.3 closes R9's two Windows halves by reasoning; Windows is not executed.

**What a genuine permanent failure becomes under the chosen form.**
- **Quiet, and naming nothing of the registry:** the Git error, at once, as at master. In a verification that is
  `not_repairs`' terminal, as at master.
- **Naming an entry no read saw whole and readable** (FUB-D3-PERM: the store cannot be written, so the add's own new
  entry exists only during the attempt; or a registry file no read can read): it is retried until the deadline. It then
  refuses as `RegistryRefused`, naming the store, what it showed, the attempt count and Git's text. The refusal is
  resumable and never a durable deferral.
- **This is FUB-D3-PERM's second remedy.** The design-recast lens wrote: "Either distinguish these failures or
  withdraw the unconditional quiet-store claim" (`~/orch-pr11/reviews/review-329-d3-design-recast-8dd2214c.review.md`).
  Round 4 distinguished them, and FUB-D4-OWNENTRY showed the distinction unsound. Round 5 withdraws the claim:
  "returned unchanged" now covers only a failure that names nothing of the registry. The finding is fixed that way,
  for two reasons.
  - **Nothing tells an add's own permanent failure from another process's same-named entry made and unmade within the
    attempt.**
    - The store does not: round 4's was byte-identical before and after (`d4/witness/git-level.log`, PERM).
    - A second attempt does not: this round's interleaving gave identical reads and identical text twice.
    - Nor does the path in Git's text. FUB-D3-PERM's failure names the entry's directory itself ("could not create
      directory of '.git/worktrees/new'"). So does a sibling that vanishes mid-scan: "Invalid path
      '<store>/wt-B4-331': No such file or directory" (`d3/measure/race-none-prune-run1.log`).
    - Only the message and `strerror` text differ, and the manager's Git children run in the user's locale.
    - So no rule can return the first as Git state without returning the second.
  - **A store that cannot be written fails every add**, attempts' included, and an attempt's failed add already ends
    the command. The refusal costs the deadline and changes the error's type; it costs nothing durable. On the legacy
    path it is a registry refusal, so B-PRESERVE keeps the attempt's output (§4.5), where master discards it.

**The witness the brief asks for is planned as T14** (§4.8): the lenses' interleaving through the production add, two
windows and every attempt, with an own-entry exception as the mutation that turns it red. It is executed on the scratch
shapes:

| | Round 4's shape | Round 5's shape |
|---|---|---|
| Two windows | Git after two add attempts | Ok after three |
| Every attempt | Git after two add attempts | `RegistryRefused` at the deadline |

### 4.3 The access as round 5 specifies it

These are the deltas against §3.3 and §3.4; everything else in them stands.

**C3 has no exception.** §3.3's "The add's own entry" paragraph and its `Own entry only` verdict are withdrawn. C3 now
holds when the failure names an entry that neither read holds, or one that a read holds with a file it could not read.
"Names" keeps its three forms: the text, a refused parse's checkout, a scan's refusal by name.

**C3 matches any spelling of the store.**
- The text names an entry when it contains the common git dir's final component, then `/worktrees/`, then the entry's
  name, up to a separator, a quote, a colon or whitespace. `\` is read as `/`.
- Whatever precedes it does not matter: the store relative to the directory Git ran in (`.git/worktrees/…`, as Git
  prints it in the main checkout), canonical, through a link, or through an 8.3 alias. The hosted Windows lane's
  `TEMP` is one such alias. Round 4 matched two spellings of the whole store path (R9's first half).
- Only C3 sees an entry made and unmade between the reads, so the own-entry interleaving depends on this match.
- A tree never holds a `.git` path component, so a checkout's failure cannot name `.git/worktrees/…`. A common git
  dir with another final component, such as a bare repository's, could collide with a candidate path in a failed
  checkout; that failure would then refuse at the deadline. This is stated and not measured.

**C3 counts a named entry a read could not read.**
- Windows reports a file being deleted, or held open without read sharing, as unreadable. Held so across both reads,
  round 4 called the store quiet (R9's second half).
- A registry file that stays unreadable is a registry fault, and refusing it at the deadline is right.

**The verdicts** (replace §3.3's table):

| Verdict | Then |
|---|---|
| Not contended | Returned at once, unchanged. |
| Contended | Another attempt after a backoff (1 ms doubling to 50 ms), while the deadline allows. For an add, also only while its caller's condition holds and no entry of the read after the failure registers its destination; otherwise the failure is returned as it is. At the deadline, `UpstrokeError::RegistryRefused`, never `UpstrokeError::Git`. |

**The refusal's type.**
- The deadline's refusal is a new variant, `UpstrokeError::RegistryRefused { message }` in `src/error.rs`, displayed
  as its message like `Refused`. So is a wait for R-X that reaches the deadline.
- On the topology path it propagates exactly as round 4's `Refused` did: `run::verified` maps only `UpstrokeError::Git`
  (`src/engine/topology/run.rs:279-289`).
- It lets a caller tell a registry refusal by its type; B-PRESERVE (§4.5) is the caller that needs it.

**An add's destination.**
- An add names its destination, and is attempted again only while no registration in the read after the failure names
  it. "Names" means its `gitdir`, which Git writes as a real path, compared with the destination's canonical path.
- Git writes that `gitdir` only after it has taken the destination over, so a registration naming the destination
  means Git got that far.
- The caller's own condition still applies: the topology's "nothing at the slot", or B1′'s (§4.4).

**Where it applies:** §3.3's table, unchanged. The legacy module calls one `pub(crate)` entry point,
`tolerant_registry_access`, over a common git dir, a flag for R-X shared, an add's destination, a retry condition and
the attempt. Only B1′ needs it.

**Measured on the shape with B1′ and B-PRESERVE** (`patch-v-ne.py`, `patch-v-legacy.py`):
- Clippy `-D warnings` over all targets: rc 0 (`ne-suite/clippy-2-run.txt`).
- Two whole suites: 2,997 passed with 3 failed, and 2,998 passed with 2 failed (`ne-suite/suite-2.log`, `suite-3.log`).
- The two failures in both are the non-frozen tests round 4 also moved (§3.6).
- Suite 2's third failure is a frozen recovery test: `engine::topology::recover::tests::`
  `an_error_after_the_logs_torn_tail_is_truncated_refuses_the_resume_before_any_effect_and_the_next_resume_converges`,
  "and no process holds the run". It passed alone three times on the same tree (`ne-suite/alone-recover-{1,2,3}.log`).
  It is a single observation of `rundir::is_running` (`recover/tests.rs:7120-7122`), which answers true for this
  process's own claim on the run, the primary lock, the cleanup lease and an inspection failure alike. A lease copy a
  sibling's fork inherited is one way to fail it; what answered here was not established (repair round 8, §9.18.4).
- Every frozen module's tests pass in suite 3 (`frozen-census.txt`): recover 226, integrate 20, repair 5, finalize 5,
  fold 192, `events::log` 47. So do the legacy engine's 188 and `src/workspace.rs`'s 47, and every instrument census.

### 4.4 Corrected B1′: PROPOSAL (decision B, first part)

**What corrects it.**
- **FUB-D4-B1PREDICATE** (P1, executed by all three lenses). The legacy snapshot's destination exists before the add
  (`src/workspace.rs:1377`), so §3.3's "nothing at the slot" was false after every failure and nothing was retried. The
  retry predicate is now explicit.
- **FUB-D4-B1REMOVE** (P2, executed). The removal's success decision moves inside its attempt.
- **Round 4's open note** on the removal: `worktree_is_registered`'s list is no longer a separate access. It is the
  removal's success decision.

**The exact text, `src/workspace.rs`:** three call sites and one private helper. Nothing else in the module moves.

1. **`switch_branch`** (`:450-457`). The `git switch -q --no-recurse-submodules -- <name>` child (`:455-456`) is the
   attempt. It has no destination, and it is attempted again while the store shows contention.
   - Git refuses a branch checked out in another worktree by scanning the registry before it changes anything
     (`die_if_checked_out`), so a contended failure has changed nothing.
   - Executed (`d5/witness/git-level-v.log`, S): rc 128 naming the torn entry, with `HEAD` and the status unchanged;
     rc 0 once the sibling finishes.
2. **`add_gate_worktree`** (`:871-906`). The `git worktree add -q --detach --force <path> <commit>` child
   (`:879-896`) and its exit check (`:897-904`) are the attempt. It holds R-X shared, and its destination is `path`,
   canonical.
   - **The legacy retry predicate.** Before the first attempt the add records the destination's identity. It is
     attempted again only while the destination is still that directory: present, a directory and not a link or
     reparse point, empty, and on Unix with the same device and inode. No registration may name it.
   - **Why the predicate is exact.** Git takes a destination over (`junk_work_tree`) only after its sibling scan, its
     new entry and that entry's `locked` file (v2.43.0 `builtin/worktree.c:429-489`, v2.50.1 `:444-504`, v2.55.0
     `:478-538`). On any later failure it removes the destination with its junk.
   - So a destination still empty and unchanged means Git stopped before taking it over, and the next attempt is the
     same attempt. A destination that is gone means the failure came later, at the checkout or the registration's own
     files, and the failure is returned as it is.
   - **Executed at the Git level** (`git-level-v.log`). P1: rc 128, identity unchanged, no entries, no registration
     naming it, then rc 0 once the sibling finishes. P2: a checkout that cannot be made leaves the destination absent.
   - **Executed through real legacy snapshot construction** (`TABLE.txt`). `legacy_add_transient`: Ok after two
     attempts, the destination present and empty after the first failure. `legacy_checkout_cannot_be_made`: Git after
     one attempt.
   - On Windows, std exposes no stable file identity, so the predicate there is the rest of it. That suffices: the
     destination is under the run's private root, and nothing but this access and Git touches it.
3. **`cleanup_gate_workspace`** (`:1549-1600`). The removal's attempt is the removal and its success decision together:
   `git worktree remove --force <path>` (`:1557-1571`), then `worktree_is_registered`'s list and parse (`:1572`,
   `:1602-1635`).
   - The attempt succeeds when that list does not register the path, whatever the removal's exit status. That is the
     existing treatment, which counts an already-unregistered destination ("is not a working tree") as reclaimed.
   - It fails with the removal's words (`:1573-1579`) while the list registers the path, and with the list's own error
     when the list fails.
   - It is attempted again while the store shows contention. A second removal of a registration already gone fails
     "is not a working tree", and the list then decides.
   - Everything after the attempt (`:1581-1599`) is unchanged.
   - **Executed.** At the Git level (`git-level-v.log`, R): the removal exits 128, the sibling finishes, the list
     exits 0 and still lists the target, and the same removal again exits 0. Through the engine
     (`legacy_removal_transient`): two removal attempts, the snapshot unregistered, its directory gone.
4. **One private helper** resolves the canonical common git dir in the two steps `recorded_objects_scope` takes
   (`:97-101`). It runs `git rev-parse --path-format=absolute --git-common-dir` through `git_path`, so through
   `git_command`, as the module's census requires (`:3681`), then calls `fs::canonicalize`.

What stays as it is:
- `worktree_is_registered` keeps its body and is called only inside the removal's attempt.
- `discard_uncommitted`, the snapshot lifecycle (`PendingGateWorkspace`, `:1304-1433`), `recorded_objects_scope` and
  every other function are unchanged.

One consequence is stated:
- A snapshot's drop (`:1419-1433`, `:1673-1685`) now waits up to the deadline under contention, where it failed at
  once and left residue for the resume.
- Static residue makes the pending snapshot's drop refuse as well: `legacy_add_static` refused at 1,004 ms and left one
  intent, which the resume's reclaim takes up.

**Instruments (B1′ only).**
- **`effects/wrappers.toml`:** `tolerant_registry_access` joins `src/workspace_manager.rs`'s `effect_free` list
  (`:134-180`). Measured: the classification census fails without it (`firstform-suite-1.log`) and passes with it.
- **`effects/allowlist.toml`:** `src/workspace.rs`'s `legacy_effect` text (`:898-924`), as below. Its `path`, its
  `allows` and `FROZEN_LEGACY_ALLOWLIST` (`src/effects.rs:1306`) do not move.
- **No `clippy.toml` change:** an `effect_free` function is not denied.

**The unfreeze text.** In the entry for `src/workspace.rs`, "AMENDED ONCE" becomes "AMENDED TWICE". The last sentence,
"The schema-4 equivalents live behind funnels in `crate::workspace_manager` and nothing here calls them: the constant
is read, and no funnel is called.", is replaced by:

> The second amendment, to close `PR329-LEGACY-RUNS-IN-LINKED-CHECKOUTS-RACE-THE-SHARED-WORKTREE-REGISTRY` on the
> owner's decision to unfreeze the module for this one change, is one thing and no more: the three Git children that
> enumerate the repository's worktree registry — `switch_branch`'s `git switch`, `add_gate_worktree`'s `git worktree
> add`, and `cleanup_gate_workspace`'s `git worktree remove` together with the `git worktree list` that decides whether
> it took the registration — each run as an attempt of `crate::workspace_manager`'s tolerant registry access, which
> attempts one again while the repository's worktree store shows another process's work and refuses one still
> contended at its deadline as a registry refusal, never as Git state. The add is attempted again only while its
> destination is still the empty directory this module made for it and no registration names it. One private helper
> resolves the canonical common git dir as `recorded_objects_scope` does. Every other behaviour of the module stays
> frozen. The schema-4 equivalents live behind funnels in `crate::workspace_manager`, and nothing here calls a funnel:
> the constant is read, and the tolerant access is called, which takes no site.

**What corrected B1′ closes.**
- It closes every write in flight that finishes within the deadline, from any writer, at the three legacy accesses.
  That is (e1) and (e2): the race the finding names and its mixed case.
- It does not close a registration that stays torn until the deadline (R1′). That access refuses, as
  `RegistryRefused` and never as Git state, and the frozen coordinator discards on it as on any error. Closing that is
  B-PRESERVE's (§4.5).

### 4.5 B-PRESERVE: PROPOSAL (decision B, second part)

**What it closes.**
- **(e2′):** a topology writer's static residue (a killed writer's torn registration), or contention that outlasts the
  deadline, makes a legacy access refuse, and the frozen coordinator then discards the paid output.
- **(e1′):** the same from a legacy writer.
- All three lenses hold that (e2′) blocks G6 even after B1′ (§4.7).
- It needs B1′. Without B1′ the legacy add fails as Git state at once, which this change does not touch.

**The design in one line.** The coordinator keeps a registry-refused attempt's output, in the checkout and pinned in
the repository under the run's private ref namespace. The resume keeps the pin, discards the checkout's copy as it does
today so the attempt runs again from a clean tree, and names the pin.

**`src/engine/coordinator.rs`, exactly.**
- **`:544-550`.** The error arm of `match run_attempt(…)` splits in two.
  - `Err(UpstrokeError::RegistryRefused { message })`: **nothing is discarded.**
    - The attempt's captured candidate is the index `capture_candidate` staged (`git add -A`,
      `src/workspace.rs:500`) before any snapshot was attempted (`src/engine/attempt.rs:129`, `:154`, `:178`).
    - It is pinned through `Workspace::prepare_commit_from_candidate` (`src/workspace.rs:946-1017`), which the
      coordinator already calls to settle (`:668`). The branch is `Workspace::current_branch_ref()`, the parent
      `head_sha_full()`, the tree `staged_tree_oid()`, and the subject `[upstroke] kept: <task> attempt <n>`.
    - The pin is the attempt's `prepared_pin_ref` followed by `KEPT_PIN_SUFFIX`.
    - The arm returns `UpstrokeError::RegistryRefused` with the refusal's message followed by "; the worker's output
      for attempt <n> of `<task>` is kept in this checkout and pinned at `<pin>`". When pinning fails it says instead
      "… is kept in this checkout, and pinning it at `<pin>` failed: <error>".
  - `Err(error)`: discarded as today, and the error is returned.
- **`:281-283`.** Beside `prepared_pin_ref`: `pub(super) const KEPT_PIN_SUFFIX: &str = "-kept";`.
  - It is a constant, not a function, so the `effects/wrappers.toml` census of reachable functions does not move.
  - No prepared-commit path names it: the resume's orphan-pin removal (`src/engine/resume.rs:552-554`) and the schema-3
    settlement check (`src/events/mod.rs:1332-1339`) both name `prepared_pin_ref` exactly.
- **Nothing else in the module moves,** and it calls nothing it did not already call.

**`src/engine/resume.rs`, exactly: the recovery policy.**
- **`:26`:** it also imports `KEPT_PIN_SUFFIX`.
- **`:543-560`:** for each interrupted attempt, after the orphan pin's removal, the loop also asks `prepared_pin_target`
  (`src/workspace.rs:1122-1143`) whether that attempt's kept pin exists, and collects the ones that do.
- **`:562-570`:** the uncommitted paths are discarded exactly as before (the warning, `discard_uncommitted()`,
  `RunResumed.discarded`), so the interrupted attempt runs again from a clean tree.
  - When any kept pin was found, one more warning names each one and how to use it: "the worker output of the
    interrupted attempt(s) a worktree-registry refusal stopped is kept at `<pins>`: `git checkout <ref> -- .`
    restores it into a checkout, `git update-ref -d <ref>` removes it".
- **No resume removes a kept pin;** it is the operator's.
- **Why recovery and not refusal.** The pin already keeps the output on every later invocation. A refusal would stop
  the run for an operator's action without keeping anything more.

**Executed on the prototype** (`TABLE.txt`, `preserve_*`, through the real legacy engine). The run is a legacy run with
one gate, and a static tear is planted right after the candidate is captured.
- **The unpatched tree.**
  - The run fails as Git state: "failed to read .git/worktrees/d5-static-residue/commondir: Success".
  - The checkout is clean: the paid output was discarded.
  - The resume, with the residue still there, fails as Git state.
  - After the repair, the resume completes, paying again.
- **The prototype.**
  - The run fails `RegistryRefused` and the checkout still holds "A  agent-output.txt".
  - The kept pin `refs/upstroke/prepared/<run>/0-1-kept` names a commit whose tree is the index's.
  - The resume, with the residue still there, refuses (`RegistryRefused`, from the reclaim) and changes nothing.
  - After the repair it completes. Its warnings say it discarded the checkout's copy and name the kept pin, and the pin
    remains.

**The first form, rejected with evidence.**
- The first form needed no marker. Its resume refused whenever the leftovers were a captured candidate: every path
  staged, nothing unstaged or untracked.
- It failed three frozen legacy tests (`firstform-suite-1.log`). `engine::tests::`
  `resume_removes_a_pin_whose_successful_settlement_never_landed` (`src/engine/tests.rs:6873`) and the two parked-question
  kill tests (`:9646`, `:9654`) each resume over a staged candidate and expect it discarded.
- A marker by whether an attempt is in flight fails the last two the same way: their logs end at a parking settlement
  with the candidate staged.
- The kept pin is a marker that only a registry refusal writes.

**`src/error.rs`:** the `RegistryRefused` variant (§4.3). It belongs to the topology change and is not frozen.

**`effects/allowlist.toml` (`:834-853`, `:855-867`): the exact amendments.** Each entry's `path` and `allows`, and the
frozen list, stay as they are. Each `legacy_effect` text gains one paragraph.
- For `src/engine/coordinator.rs`:

  > AMENDED ONCE, on the owner's decision to close the static and deadline residue of
  > `PR329-LEGACY-RUNS-IN-LINKED-CHECKOUTS-RACE-THE-SHARED-WORKTREE-REGISTRY`: when an attempt ends in a
  > worktree-registry refusal (`UpstrokeError::RegistryRefused`), the coordinator does not discard the checkout; it
  > pins the attempt's captured candidate through `Workspace::prepare_commit_from_candidate` at the attempt's
  > prepared-pin name followed by `KEPT_PIN_SUFFIX`, and the refusal it returns names the pin, or the pin's failure.
  > Every other attempt error discards the checkout as before. It calls nothing it did not already call, and nothing
  > else in the module moves.

- For `src/engine/resume.rs`:

  > AMENDED ONCE, on the owner's decision to close the same finding's static and deadline residue: for each
  > interrupted attempt the resume also asks whether the coordinator kept that attempt's captured candidate at its
  > kept pin, which no resume removes; it discards the checkout's uncommitted paths as before, so the attempt runs
  > again from a clean tree, and its warning names every kept pin and how to restore it. It calls nothing it did not
  > already call, and nothing else in the module moves.

- For `src/engine/tests.rs` (`:1138-1150`): the text is unchanged. The file gains the regression tests below, and no
  existing test changes.

**Legacy regression tests:** T-P1 to T-P5 (§4.8).
- `src/engine/tests.rs` is frozen, and it holds the only legacy engine harness: its fake adapters and run fixtures are
  private to it. So the tests are appended there, which the unfreeze names, and no existing test changes.
- Measured on the prototype: all 188 of the file's tests pass in each of two suites (`frozen-census.txt`).

**The packet invariants it touches.**
- **PR5's `slice_contract.invariants_preserved[0]`:** "existing Workspace and legacy engine behavior untouched
  (moves are behavior-neutral; legacy tests unchanged; …)".
  - Corrected B1′ changes `src/workspace.rs`'s behaviour: a contended registry access is retried, then refused, never
    returned as Git state.
  - B-PRESERVE changes the legacy coordinator's error path and the legacy resume's warning.
  - No existing legacy test changes. B-PRESERVE adds tests to the frozen test file.
- **PR12's `slice_contract.invariants_preserved[0]`:** "… legacy resume unchanged …".
  - The resume's control flow is unchanged: it still discards and runs the attempt again. It reads one more ref per
    interrupted attempt, and warns.
  - B1′ also changes the resume's own registry accesses: `reclaim_gate_workspaces` reaches the removal
    (`src/engine/resume.rs:426`), and `switch_branch` is called at `:454`. A contended one is retried or refused, never
    a Git error.
- Both invariants are the owner's to amend. Neither is a packet row this lane can change.

**What stays open if the owner declines.**
- **B-PRESERVE declined, B1′ taken:** (e2′) remains an applicable P1 and blocks G6, unless the owner rules otherwise.
  (e1′) remains a P1 that does not apply to G6. The finding stays open, narrowed to the residue.
- **Both declined:** (e2) and (e2′) remain applicable P1s and block G6, unless the owner rules otherwise. (e1) and
  (e1′) remain P1s. The finding stays as filed.
- **B-PRESERVE without B1′:** not possible. B-PRESERVE reads the registry refusal that B1′ produces.
- **The ruling ESCALATION item 7 already offers:** that the mixed residue case does not block G6, with the P1 kept
  filed. Without such a ruling no waiver is inferred.

**Residuals, stated.**
- If the pin write fails (the ref store cannot be written at the moment the registry refused), the output is only in
  the checkout. The refusal says so, and a resume then discards it as today. That takes two independent faults.
- The attempt that runs again pays again; the kept output is the operator's to use. Adopting it into the run would
  need the worker's outcome, which the legacy log does not record before settlement (`src/engine/attempt.rs:110-128`).
  That is out of scope.
- A crash after capture, which is not a refusal, still discards on resume, as today. It is pre-existing and not this
  finding.

### 4.6 Decision B, as the owner now meets it (FUB-D4-OWNERREASON)

**The reason the owner is needed is the PR5 unfreeze, not CLAUDE.md's first limb.**
- §3.6 and §3.10 said that B1′'s two instrument edits made a B1′ implementation the owner's to merge. Under this lane's
  direction, instrument edits do not reserve the merge: the owner's merge bar of 2026-09-27/28 reads "three clauses and
  nothing else … limb-1 instrument reasoning no longer gates" (`~/orch-pr11/ESCALATION.md`, log, 2026-09-30T00:15Z).
- What needs the owner is that corrected B1′ and B-PRESERVE change PR5-frozen legacy behaviour, against PR5's and
  PR12's invariants (§4.5). The frozen files are `src/workspace.rs`, `src/engine/coordinator.rs` and
  `src/engine/resume.rs`, and `src/engine/tests.rs` for the tests.
- The instrument edits are disclosed in the body.

**The shape of the decision.**
- It has two parts, B1′ (§4.4) and B-PRESERVE (§4.5), and B-PRESERVE needs B1′.
- The topology closure (§4.2, §4.3) needs no owner decision.

### 4.7 What is closed, what remains, and what G6 meets (FUB-D4-G6TABLE)

**The claims, once implemented.** These are §3.7's, with two corrected:
1. No manager registry access returns `UpstrokeError::Git` for a failure another process's registry work caused,
   whatever the name of the entry the failure names, the add's own included (§4.2). So, for (b), no registry
   contention reaches `run::verified`'s Git arm.
2. (a), as §3.7.
3. (d), as §3.7.
4. A failure that names no registry entry across a quiet, whole store is returned as it was, at once. A failure naming
   an entry no read saw whole and readable refuses at the deadline, as FUB-D3-PERM's construction does. §3.7's "after
   exactly one more attempt" is withdrawn.
5. and 6., as §3.7.

**What remains** (replaces §3.7's rows R6, R7 and R9):

| | What | Consequence now | Finding |
|---|---|---|---|
| R6 | The legacy engine's registry readers | §4.4 and §4.5 | `PR329-LEGACY-RUNS-IN-LINKED-CHECKOUTS-RACE-THE-SHARED-WORKTREE-REGISTRY` (P1; guard: corrected B1′ plus B-PRESERVE) |
| R7 | A Git version whose registry-phase failure names no entry and leaves no trace at either read | none of 2.43.0, 2.50.1 or 2.55.0 has one (§4.2) | stated |
| R9 | Windows | the alias spelling and the file held unreadable across both reads are now C3's (§4.3); reasoned, not executed | stated |

**The cases.** This is the three lenses' consensus, with B-PRESERVE:

| Case | Closed by | Severity | Applies to G6 | Blocks G6 |
|---|---|---|---|---|
| (a) An attempt's pipeline error | §4.2 and §4.3, non-frozen `src/workspace_manager.rs` and `src/error.rs` | P1 | yes: R17, the shared registry, Q6 | until implemented and validated |
| (b) A durable verification deferral or park | §4.2 and §4.3 | P1 | yes: Q6 and durable verification | until implemented and validated |
| (c) DESC | filed, follow-up C | P1 | yes: Q1, INV-22, ST-16, ST-18 | yes; filing is no waiver |
| (d) A legacy writer tears a topology reader | §4.3 | P1 class | yes | until implemented and validated |
| (e1) Legacy against legacy, write in flight | corrected B1′ (PROPOSAL) | P1 | no | no; it remains a P1 until taken |
| (e1′) Legacy against legacy, static or deadline residue | B-PRESERVE (PROPOSAL), with B1′ | P1 | no | no; it remains a P1 until taken |
| (e2) A topology writer tears a legacy reader, write in flight | corrected B1′ | P1 | yes: Q6, across the shared registry and R17 | **yes, until corrected B1′ is implemented and validated** |
| (e2′) A topology writer's static or deadline residue makes a legacy reader refuse, then discard paid output | B-PRESERVE only, which needs B1′ | P1 | yes: Q6; a crash producer engages Q1; distinct from DESC because no surviving writer is needed | **yes, even after B1′**, until B-PRESERVE is implemented and validated, unless the owner rules otherwise |

**The findings** (replaces §3.7's table where it differs):

| Item | Severity | Here | Applies to G6 | Blocks G6 |
|---|---|---|---|---|
| `PR11-LINKED-CHECKOUTS-RACE-THE-SHARED-WORKTREE-REGISTRY`: (a), (b), (d) | P1 | repaired by §4.2–§4.3 once implemented; its file is deleted then | yes | only until this change merges |
| `PR329-A-RESUME-REBINDS-A-SLOT-ITS-DEAD-COORDINATORS-GIT-CHILD-STILL-WRITES`: (c) | P1 | filed; follow-up C | yes | yes, until follow-up C merges |
| `PR329-LEGACY-RUNS-IN-LINKED-CHECKOUTS-RACE-THE-SHARED-WORKTREE-REGISTRY`: (e) | P1 | filed; guard: corrected B1′ plus B-PRESERVE, pending decision B | (e2) and (e2′) yes; (e1) and (e1′) no | yes, through (e2) until corrected B1′ lands, and through (e2′) until B-PRESERVE lands, unless the owner rules otherwise; filing is no waiver |

The other rows of §3.7's table stand.

### 4.8 Regression tests

§3.8's T1, T2, T2′, T3, T5, T6, T9, T10, T11 and T13 are carried unchanged, and so is its proof. The tests below are
revised or new. Each waits on a handshake or a seam, with time only as a watchdog. Each mutation runs on a scratch tree
whose Compiling line names it.

**Revised.**
- **T4, the classifier, as unit tests over stores built by hand** (replaces §3.8's).
  - C1 for each of `gitdir`, `commondir` and `HEAD`, absent and empty.
  - C2 for a change of bytes at equal length within one clock tick.
  - C3 for an entry neither read holds, the add's own name included, in four spellings: relative, canonical, through
    a link, and with backslashes.
  - C3 for a held entry with a file a read could not read.
  - C3 for a refused output's checkout.
  - The quiet verdict.
  - Mutations: each clause removed in turn, against the case only it catches; and an own-entry exception restored.
- **T12, FUB-D3-PERM** (replaces §3.8's). One complete registration, and the store made unwritable, then an add. It
  refuses as `RegistryRefused` after the deadline, carrying Git's "Permission denied", never `Git`, and leaves nothing
  at the slot.

**New: the topology.**
- **T14, FUB-D4-OWNENTRY: the lenses' interleaving through the production add.** A test-only seam at the two points of
  an add's attempt, the shape of `note_contended`, makes a registration with the add's own name appear and vanish.
  - Two windows: Ok on the third attempt.
  - Every attempt: `RegistryRefused` at the deadline.
  - Mutation: an own-entry exception of one confirming attempt turns it red, Git after two attempts.
  - Executed on the scratch shapes (§4.2).
- **T15, `not_repairs` through a registry access.** A verification whose snapshot the checkout cannot make, on a quiet
  registry, terminates `merge_verification_unavailable` (Deferred) and the run completes. A manager unit test adds
  that a snapshot of a tree holding a name past `NAME_MAX` returns Git after one attempt.
  - Mutation: retry-everything turns it red, ending `RegistryRefused` with no terminal.
  - Executed (`verification_snapshot`, `checkout_cannot_be_made`).

**New: corrected B1′,** in `src/workspace.rs`'s inline test module, under its unfreeze.
- **T-L1:** the legacy snapshot add beside a transient tear succeeds on a later attempt, with the pre-created
  destination intact and empty after the first failure. Mutation: round 4's "nothing at the slot" condition turns it
  red (Git after one attempt), which is FUB-D4-B1PREDICATE.
- **T-L2:** beside a static tear the add refuses as `RegistryRefused` at the deadline, never Git.
- **T-L3:** a snapshot the checkout cannot make is Git after exactly one attempt.
- **T-L4:** the removal beside a transient tear that finishes between the removal and its list leaves the snapshot
  unregistered and removed, after two removal attempts. Mutation: the success decision outside the attempt turns it
  red, which is FUB-D4-B1REMOVE.
- **T-L5:** `switch_branch` beside a transient tear switches, and `HEAD` is unchanged after the failed attempt.
- **T-L6:** T10 with the roles swapped: the legacy engine in a linked checkout beside the topology manager's cycle,
  with no legacy failures. The two lenses' round-3 sequences, with A released within B's deadline, end in a successful
  add and nothing discarded.
- T-L1 to T-L4 were executed on the prototype (§4.4).

**New: B-PRESERVE,** appended to `src/engine/tests.rs` under its unfreeze. No existing test changes.
- **T-P1:** a legacy attempt a static tear refuses ends the run `RegistryRefused`, naming the kept pin. The checkout
  still holds the captured candidate, and the pin's tree is its tree. Mutation: the coordinator discarding on every
  error turns it red, with a clean checkout.
- **T-P2:** a resume while the residue stays refuses as `RegistryRefused` (from the reclaim) and discards nothing.
- **T-P3:** a resume after the repair completes; its warning names the pin, and the pin remains. Mutation: a resume
  that removes kept pins turns it red.
- **T-P4, (e2′) itself:** T-P1 to T-P3 with the residue a topology slot's torn registration, made by the manager's own
  torn-registration shape.
- **T-P5:** an attempt error that is not a registry refusal (the capture hook returning an error) still discards, as
  today.
- T-P1 to T-P3 were executed on the prototype (§4.5).

**The proof the implementer owes:** §3.8's, with the frozen census extended to the legacy engine's tests and
`src/workspace.rs`'s, which pass 188 and 47 on the prototype (`frozen-census.txt`).

### 4.9 Design review round 4, answered

**Design review round 4** ran three `gpt-6-astra` lenses at `max` on `a6135a66`: design as a conformance reading,
concurrency, and regression. They are logged in the `run-lens` logs, and all three returned CHANGES_REQUIRED. The texts
are `~/orch-pr11/reviews/review-329-d4-{design,concurrency,regression}-a6135a66.review.md`, with their hashes in
`SHA256SUMS-329-d4`. The witnesses are in `329-d4-witnesses/`, and the triage is `review-329-d4-triage.md`.

| Finding | Sev | Kind | Round 5 | Where | Evidence |
|---|---|---|---|---|---|
| FUB-D4-OWNENTRY | P1 | executed | **Fixed (design), witnessed on the shape.** The own-entry exception is removed: a failure naming an entry neither read holds is contention whatever its name. Retry-everything was evaluated first and rejected with evidence. | §4.2, §4.3 | `d5/census/probe-v/witness-runs/TABLE.txt`: `own_entry_two`, `own_entry_every`, `verification_snapshot`, `checkout_cannot_be_made`; planned T14 and T15 |
| FUB-D4-B1PREDICATE | P1 | executed | **Fixed (design), witnessed.** The legacy add is attempted again only while its pre-created destination is the owned, unchanged, empty directory and no registration names it. | §4.4 | `d5/witness/git-level-v.log` P1 and P2; `legacy_add_transient`; planned T-L1 |
| FUB-D4-B1REMOVE | P2 | executed | **Fixed (design), witnessed.** The removal and its success decision are one attempt; an already-unregistered destination still succeeds. | §4.4 | `git-level-v.log` R; `legacy_removal_transient`; planned T-L4 |
| FUB-D4-G6TABLE | P2 | reasoned | **Fixed (design).** (e2) is closed by corrected B1′; (e2′) only by B-PRESERVE, and it blocks G6 even after B1′. The finding's guard names both. | §4.7 | the legacy finding file |
| FUB-D4-RESUME | P2 | reasoned | **Fixed (design), witnessed.** B-PRESERVE covers `coordinator.rs:544-548` and `resume.rs:562-569` with their allowlist entries; the kept pin survives every resume. | §4.5 | `preserve_*` in `TABLE.txt`; `frozen-census.txt` (legacy 188 of 188) |
| FUB-D4-OWNERREASON | P3 | reasoned | **Fixed.** The owner is needed for the PR5 unfreeze, not CLAUDE.md's first limb. | §4.6 | — |

**FUB-D3-PERM**, design review round 3's P2, is re-answered. Round 4 fixed it with its first remedy, distinguishing the
failures, and that fix is withdrawn with the exception. Round 5 fixes it with its second remedy, withdrawing the
unconditional quiet-store claim (§4.2). Its ledger row says so.

### 4.10 Risks, sequencing, and what is out of scope

These are §3.12's, with the following added.

**Risks.**
- **FUB-D3-PERM's outcome returns.** An unwritable store, or a registry file nobody may read, refuses at the deadline
  (10 s) instead of failing at once. It costs nothing durable (§4.2).
- **A legacy snapshot's drop waits up to the deadline under contention** (§4.4).
- **Kept pins accumulate,** one per refused legacy attempt, until the operator removes them (§4.5).
- **`UpstrokeError` gains a public variant,** `RegistryRefused`.
- **A frozen recovery test's red.** The test that reddened suite 2 asserts, in one observation, that no process holds
  the run after a resume (`src/engine/topology/recover/tests.rs:7120-7122`). A copy of the run's lease descriptor that
  a sibling test's fork inherited answers that observation held for as long as the copy lasts, and so do the primary
  lock, this process's claim and an inspection failure. Which answered was not established, so it is not attributed to
  `PR281-CLEANUP-LEASE-HOLD-OUTLIVED-AND-ITS-UNREADABLE-TWIN` (repair round 8, §9.18.4). It passed alone three times
  (§4.3), and this change touches no lease.

**Sequencing.**
- Corrected B1′ and B-PRESERVE land in #329's implementation only if the owner takes them before it; otherwise in a
  later change, under the legacy finding's guard.
- The topology closure does not wait for them.

**Out of scope, and said so.**
- Adopting a kept candidate into the legacy run (§4.5).
- A crash after capture (§4.5).

## 5. Round 6 design (topology only)

> **SUPERSEDED by §6 (design round 7) where §6's banner says; the rest of §5 stands as §6 cites it.**
> - **Replaced:** §5.3's veto ("What the veto reads", "An existing empty directory is used as it is" as the veto's
>   premise, and "Windows") by §6.3; §5.4's bound by §6.4; §5.6's claims 1, 4 and 5 and rows R1′, R2, R3, R8 and R9 by
>   §6.6; §5.7's T4, T15, T16 and T17 by §6.7.
> - **Widened:** §5.3's store-absent exception for the removal scan (§6.3, "A coordinator killed during the probe").
> - **Changed with a date:** §5.5, the helper's contract (its note; the current text is §6.4).
> - **Corrected:** §5.1's platform sentence (§6.10).
> - **Stands:** §5.2's evaluation of the store-activity window; §5.3's rule that every failed registry attempt is
>   attempted again until the deadline with nothing classified, Git's order of the add's steps, the destination made
>   before Git runs, and "Round 5's rejection of retrying everything, answered"; §5.4's table of where the access
>   applies; §5.8; §5.9; §5.10 with §6.11's additions.
>
> *Round 6's banner:* **PROPOSED — for design review round 6.** This section supersedes §4 where §4's banner says. It is
> narrowed to the topology registry race: consequences (a), (b) and (d), and targeted removal. The legacy half, corrected B1′ and
> B-PRESERVE, is follow-up D's (`pr11_fud_design`), owner-gated and due before G6; this change keeps the helper D will
> call and publishes its contract (§5.5). The closure needs no owner decision. It edits no frozen module; its one
> instrument edit is the helper's `effect_free` row in `effects/wrappers.toml` (§5.8). Nothing in §5 is in force until the
> implementation lands.

Design round 6 is `pr11_fub_design6`'s (`claude-opus-5-5`, `max`), spawned on `4a126215` to carry out the
orchestrator's decision on design review round 5 (`~/orch-pr11/reviews/review-329-d5-triage.md`) and its addendum
(`~/orch-pr11/answers/pr11_fub_design6-0.md`). Its figures are under `~/orch-pr11/logs/pr11_fub_design6/`, cited as
`d6/…`; `d6/SUMMARY.txt` is the index, and `d6/census/probe-vi/SUMMARY.txt` indexes the scratch prototype (`git archive`
copies of `4a126215`, whose `src/` is master's; nothing of it is on the branch).

### 5.1 The looping signal a fifth time, the narrowing, and why this round converges

**The signal appeared again.** Design review round 5 found P1s in round 5's own machinery:
- C3's spelling rule (FUB-D5-SPELLING): a `.git` that is a link to a directory of another name defeats the match;
- C3's account of a held entry (FUB-D5-OPTFILE): an optional `locked` made and unmade in an unchanged registration;
- B-PRESERVE's pin (FUB-D5-INDEX): the mutable index, not the captured candidate.

That is MAINTAINING's second signal, "A pass finds a P1 in machinery an earlier round of this pull request added", a
fifth time on this pull request (§2.1 counted the first two, §3.1 the third, §4.1 the fourth).

**What the defect in the repair was.** Rounds 3 to 5 decided whether a failure was contention by reading Git's error
text and sampling a handful of registry files before and after each attempt. Each round's reviewers built a state those
reads did not see: a torn registration Git lists without failing, whose `HEAD` the reads did not cover (round 3,
FUB-D3-TORNOK); a foreign twin of the add's own entry made and unmade between the reads (round 4, FUB-D4-OWNENTRY); a
spelling of the store, and an optional file made and unmade inside a registration that otherwise never changed (round
5). The classifier leaked in every round because it tried to tell, from outside Git, why Git had failed.

**The narrowing.** Narrowing is the author's (MAINTAINING, "When a pull request may be looping"), and the orchestrator's
decision on round 5 set its line:
- #329 keeps the topology repair: (a), (b), (d) and targeted removal.
- The legacy half moves to follow-up D, a separate fix-P1 pull request, owner-gated (decision B, ESCALATION item 7),
  due before G6. `PR329-LEGACY-RUNS-IN-LINKED-CHECKOUTS-RACE-THE-SHARED-WORKTREE-REGISTRY` stays filed here with its
  guard naming D, and the G6 rows (e2) and (e2′) point at D (§5.6).
- The helper stays here because D calls it; §5.5 is its contract, written to stay stable.
- The title still names only what #329 repairs ("registry access tolerates another checkout's write in flight"), so it
  does not change.

**Why this round converges.**
- **There is no classifier left to leak.** The chosen form (§5.3) reads nothing of the store and no error text. Every
  failed registry attempt is attempted again until the deadline, except an add whose destination Git has taken over.
  That is a fact about a directory the access made itself in the run's private root, and Git's own code order fixes
  what it means (§5.3); nothing another process does can fake it.
- **It is built from two pieces that already survived review.**
  - Round 5's retry-everything form, measured in §4.2. Round 5 rejected it for one reason: it retried the add's
    checkout, so a snapshot the checkout cannot make became a refusal.
  - Round 5's legacy destination predicate (§4.4), of which design review round 5's design lens wrote "corrected
    destination/removal predicates hold" (`review-329-d5-design-4a126215.review.md`).
  - The predicate, applied to the topology add, answers exactly round 5's reason (§5.3). That is MAINTAINING's remedy
    for this signal: "keep what has survived a pass, drop the machinery those rounds invented".
- **Every reviewer witness of rounds 3 to 5 is executed against it** (§5.3; `d6/SUMMARY.txt`). *Corrected by §6.10
  (FUB-D6-PLATFORM, 2026-10-02): every witness ran on Linux, on 2.43.0 and 2.55.0; the Windows guest's 2.50.1 ran a
  subset, without the symlinked spelling, OPTFILE, the 300-byte name, PERM, TORNOK transient, the destination's parent,
  the takeover probes and `prune-own`.*
  - at the Git level on Git 2.43.0, 2.55.0 (CI's hosted Linux and macOS version, §3.12) and 2.50.1 (the Windows
    guest's);
  - through a scratch prototype of the manager, with round 5's witness sources unchanged.
- **The legacy half is isolated in D.** #329's only interface to it is the contract in §5.5.

### 5.2 The store-activity window, evaluated first

**What was evaluated** (the brief's form, implemented as `d6/witness/d6wit.rs` policy W):
- Each attempt is bracketed by a full recursive snapshot of `<common git dir>/worktrees`: every directory and file, with
  its size, inode, and nanosecond modification and change time (on Windows, size, creation and last-write time).
- A failed attempt is contention when the two snapshots differ, or either holds an entry whose `gitdir`, `commondir` or
  `HEAD` is absent or empty. Otherwise it is genuine, and the Git error is returned unchanged.
- No error text is read.

**What the timestamps measure** (`d6/measure/`; `tsprobe.rs`, std only):

| | ext4, this box | tmpfs, this box | NTFS, the Windows guest | APFS, macOS |
|---|---|---|---|---|
| step between distinct file mtimes | 999,998–999,999 ns (p10 to p90); one step of 3,999,995 ns seen | 999,998–999,999 ns | 50.2 µs minimum, 228.1 µs median, 1,428.6 µs p90 | not measured |
| a directory's own times move on a child's create and removal | yes (mtime, ctime) | yes (mtime, ctime, size) | yes (LastWriteTime; CreationTime never) | not measured |
| a child rewritten in place, or a grandchild's change | no | no | no | not measured |

- This box runs Linux 6.8.0-137-generic with `CONFIG_HZ=1000` and `CONFIG_NO_HZ_FULL=y` (`kernel-config.txt`). Its 1 ms
  steps are the kernel's clock tick; the one 4 ms step fits ticks a tickless kernel skips (`ext4-run1.txt`,
  `ext4-run2.txt`, `tmpfs-run1.txt`).
- CI's Linux kernel and macOS are not measured. This lane can reach neither, and a design round adds no code CI would
  run.
- **The brief's question: NTFS does update a directory's last-write time when a child is created or removed**
  (`ntfs-run1.txt`, T2). `read_dir`'s copy of a child directory's last-write time moved with the child's own record
  (T4).
- **std exposes no file identity and no change time on Windows.** `MetadataExt::file_index` and
  `volume_serial_number` are E0658 (`windows_by_handle`) on 1.85.0 and 1.97.1 (`winid-probe.txt`). On Windows the window
  rests on last-write time, creation time and size.

**What a coarse clock misses** (T3). The parent is changed and read, then a child directory with one file in it is made
and, after a delay, removed, and the parent is read again. In this many trials the parent was identical in every
attribute std exposes:

| delay | 0 | 100 µs | 250 µs | 500 µs | 1 ms | ≥ 2 ms |
|---|---|---|---|---|---|---|
| ext4 (of 2,000) | 1,971 | 1,761 | 1,464 | 979 | 0 | 0 |
| tmpfs (of 2,000) | 1,988 | 1,787 | 1,489 | 989 | 0 | 0 |
| NTFS (of 1,000) | 563 | 527 | 445 | 317 | 11 | 0 |

So an entry made and unmade within the clock tick of the store's previous change leaves no trace in any snapshot.

**The window's holes, each executed** (`d6/witness/`; the table in `d6/SUMMARY.txt`):
1. **Contention escapes on a coarse clock (P1 class).**
   - The witness is the production removal scan's shape (`revalidate_removal_proving`, `src/workspace_manager.rs:5119`).
     A foreign add makes its entry and that entry's `locked`, and dies before its `gitdir`, as an add whose destination
     cannot be made does. The scan lists the entry and refuses an entry holding `locked` but no `gitdir`. The foreign
     add's junk removal then takes the entry.
   - The window called that refusal genuine 4,864 times in 5,000 on ext4, 4,930 on tmpfs, and 1,076 in 2,000 on the
     guest's NTFS (`scanmiss-linux.log`, `git-level-vi-2.50.1-windows.log`). That is (a): the access fails at once and
     the command ends.
   - With Git as the reader, the same miss needs a tick longer than Git's own start-up (reasoned). This box shows 1 ms
     with a 4 ms step; CI's kernel and macOS are unmeasured.
   - Closing it needs a settle wait: no window counts as quiet until its first snapshot equals one taken a full
     quantum earlier. That quantum must be bounded per filesystem, and this lane cannot bound it for APFS, network
     filesystems or CI's kernels.
2. **The add's own failure reads as contention.**
   - On a failure after it has made its own entry, Git removes that entry with its junk. The store's own mtime and
     ctime move exactly as a foreign entry made and unmade moves them.
   - The window therefore calls each of these contention: FUB-D5-GENUINE's tree (`.git/worktrees/fake-entry/file.txt`),
     round 5's 300-byte name, and a destination whose parent cannot be written. Each was refused at the deadline after
     15 attempts on Git 2.43.0 and 2.55.0, and for the first two every attempt was a full checkout
     (`w_first=[contention: changed ~<store>(mtime,ctime)]`). On the guest's 2.50.1, FUB-D5-GENUINE's tree was refused
     after 9.
   - That is round 5's cost 3 (§4.2) returning: `not_repairs` sends a snapshot the checkout cannot make to
     `merge_verification_unavailable`. The brief's own genuine clause fails for the case it was written for.
   - The store cannot tell the two apart: a foreign entry made and unmade and the add's own leave the same trace.
3. **Noise decides the genuine verdict.**
   - Every Git command in any linked checkout writes under the store. A `git status` after a tracked file is rewritten
     moves `<store>/<entry>` and `<store>/<entry>/index`; a `git add -A` does too.
   - With four linked checkouts running `git status` in a loop, the window refused FUB-D3-PERM's construction in 10 of
     10 trials (`noise-heavy-2.43.0.log`). It refused the quiet-parse control in 7 of 10; the other three returned Git
     after 119 to 424 ms.
   - So under parallel work, which is the condition #329 exists for, a genuine failure's outcome depends on what
     unrelated checkouts are doing.
4. **Windows** rests on last-write time, creation time and size (above). NTFS behaves as the window needs it to. Its
   steps on this build are 50 µs to 1.4 ms.

**What the window's sampling policy and bound would have to be.**
- **Sampling:** one snapshot immediately before each attempt and one after a failure.
- **Settling:** a quiet window counts as genuine only when its first snapshot equals a snapshot taken at least one
  quantum Q earlier. On a failure in an unsettled window, the access sleeps out Q and attempts once more.
- **Q:** at least 4 ms on this box (the largest step seen) and at least 2 ms on the guest (a miss was still seen at a
  1 ms delay). A bound that holds is established for neither, nor for APFS or CI's kernels. A store whose timestamps
  carry no sub-second part (FAT, HFS+, ext4 with 128-byte inodes) could never settle.
- **The add's own failure:** a separate attribution rule.
- **The bound:** the deadline, plus one attempt's runtime, plus one settle wait per apparently quiet failure.

**The conclusion: the window does not hold as stated.**
- Patched, it would need a per-filesystem quantum bound that this lane cannot establish everywhere a repository may
  live. It would also need the same destination rule §5.3 uses.
- What it would then add over §5.3 is one thing: a quiet registry fault returned as Git state at once. A registration
  nobody can list, an unwritable store and an unreadable registry file are such faults.
- That is registry state reaching `run::verified`'s Git arm (`src/engine/topology/run.rs:279`), which is the class (b)
  exists to close. So the window buys nothing the repair wants, and the next simplest sound form drops it.

### 5.3 The next simplest sound form: no classifier, and the add's takeover

**The rule.**
- Every registry access is one attempt: the list with its parse; the add; the removal's scan; the torn plan's scan.
- A failed attempt is attempted again after the backoff (1 ms, doubling to 50 ms) while the deadline allows. At the
  deadline the access refuses as `UpstrokeError::RegistryRefused`.
- **Nothing is classified.** No store state, no error text and no timestamp decides anything.
- **One veto: an add whose destination Git has taken over.** That failure is the add's own, and it is returned
  unchanged, at once.

**The takeover.**
- **Git's order.** The add's steps run in one order in Git 2.43.0, 2.50.1 and 2.55.0 (`builtin/worktree.c`; copies in
  `d6/git-src-d3-copy/`):

  | Step | v2.43.0 | v2.50.1 | v2.55.0 |
  |---|---|---|---|
  | The sibling scan (`get_worktrees`, `check_candidate_path`) | `:429-430` | `:444-445` | `:478-479` |
  | The reference (`invalid reference`) | `:443` | `:458` | `:492` |
  | The new entry (`mkdir`, with the collision counter) | `:458` | `:473` | `:507` |
  | That entry's `locked` | `:483` | `:498` | `:532` |
  | **The destination taken over** (`junk_work_tree = xstrdup(path)`) | `:489` | `:504` | `:538` |
  | The HEAD update | an `update-ref` child, `:532` | in-process, `:528` | in-process, `:563` |
  | The checkout | `:551` | `:558` | `:593` |

  - Only after taking the destination over does Git write the entry's `gitdir`, the destination's `.git`, `HEAD` and
    `commondir`, update HEAD, and run the checkout.
  - On any failure after it, `remove_junk` removes the entry and then the destination (v2.43.0 `:258`, v2.50.1 and
    v2.55.0 `:273`).
- **What the veto reads.**
  - The add makes its destination as an empty directory before the first attempt.
  - A failed add whose destination is still that directory (on Unix the same device and inode, and empty) stopped
    before taking it over. Its failure came from the registry phase, so it is attempted again.
  - A failed add whose destination is gone or changed failed after taking it over. That is its checkout, or its own
    entry's files, and it is returned as it is.
- **Contention never comes back as Git state.** No step after the takeover reads another entry.
  - Executed on Git 2.43.0 and 2.55.0 (`git-level-vi-2.43.0-r2.log`, `git-level-vi-2.55.0-r1.log`). The add was paused
    at four points: its first creation of `worktrees/kalpha-g1/locked` (after the scan), its own `gitdir` (after the
    takeover), its `HEAD.lock` and the checkout's `index.lock`.
  - At each point a torn foreign entry was planted (`gitdir` written, `commondir` empty). The add exited 0 every time.
  - A list run afterwards, with the entry still there, exited 128 on it: "failed to read
    .git/worktrees/foreign-late/commondir".
  - A prune can remove the add's own entry only before its `locked` exists, which is before the takeover. Executed
    (`prune-own`): the add failed "could not open '.git/worktrees/kalpha-g1/locked' for writing"; the destination
    kept its inode; the same add again succeeded.
- **A failure after the takeover is the add's own, and keeps `not_repairs`' semantics.**
  - FUB-D5-GENUINE's tree and round 5's 300-byte name came back as Git state after one attempt: in 1–2 ms at the Git
    level on 2.43.0 and 2.55.0, in 32 ms on the guest's 2.50.1 (the tree only), and in 7–10 ms through the prototype's
    `add_snapshot` (`d6/census/probe-vi/witness-runs/TABLE.txt`).
  - A verification whose snapshot the checkout cannot make defers on it, as it does at master.
- **Before the takeover, an engine add fails on the registry, or on a fault of the whole repository.**
  - The commit each caller passes is one the run recorded or just resolved. The callers are dispatch's slot base
    (`src/engine/topology/dispatch.rs:193`), integration's head (`src/engine/topology/integrate.rs:586`), and
    `add_snapshot`, which resolves its input before the add (`src/workspace_manager.rs:3169-3173`).
  - The destination is the access's own.
  - An add whose destination is not an empty directory when the access begins is attempted once and returned
    unchanged. That is Git's "already exists", as at master (`destination_not_empty`).
  - A fault of the whole repository, such as a configuration Git cannot read, fails every Git command the run makes.
    Before the takeover it is attempted again and refuses at the deadline, where master failed at once (R10).

**Round 5's rejection of retrying everything, answered** (§4.2's four costs):
1. **Cost 1, a checkout re-run per retry,** is gone. A checkout failure is after the takeover, so it is not retried:
   one attempt in the prototype (`checkout_cannot_be_made`, `genuine_gitpath`), where retry-everything ran 15.
2. **Cost 2, the diagnostics.** The refusal names the store, the deadline, the attempt count and the last failure's
   text. It no longer says what the store showed, because nothing reads the store.
3. **Cost 3, the verification semantics, is gone.**
   - Round 5's construction, the execution root's `snapshots/` made read-only before the judge's snapshot add, ends
     `Finished(Complete)` with one `merge_verification_unavailable` and both tasks merged, as on the unpatched tree.
   - The destination that cannot be made is Git state naming it (below). Three rounds each (`verification_snapshot`).
4. **Cost 4, a suite that does not tell the forms apart.** T15 and T16 pin the difference (§5.7).

**What changes from round 5, deliberately.**
- A registry fault that is not contention now refuses at the deadline instead of failing at once as Git state. Such
  faults are an unwritable store, a whole registration Git cannot list, and an unreadable registry file.
- FUB-D3-PERM's construction already did so in round 5. The control, a whole registration whose `HEAD` names nothing,
  now does too: `RegistryRefused` at 500 ms, three rounds (`quiet_parse`), where round 5 returned Git state at once.
- That is (b) read whole: registry state, transient or persistent, never reaches `run::verified`'s Git arm. A foreign
  registration nobody repairs no longer spends a valid candidate's deferrals or parks it.
- The cost is the deadline, ten seconds per access, and a run that stops resumably until the registration is repaired.

**The destination's lifecycle.**
- **Where it is made.** Inside the add's funnel (`Worktree.Add`, `Worktree.AddStaging`, `Snapshot.Add`), right after
  the parent's existing `create_dir_all` (`src/workspace_manager.rs:2694`). A private helper outside the funnel's body
  makes it.
  - Measured: with the helper's error message inline, `no_sampled_funnel_builds_its_argv_from_a_literal`
    (`src/workspace_manager/tests.rs:12544`), the census of the funnel bodies the kill sampler mirrors, failed: "holds 2
    string literal(s), not 1" (`d6/census/probe-vi/b-suite/suite-1.log`). With the helper it passes (`suite-2`, `-3`,
    `-4`).
  - An existing empty directory is used as it is.
- **A destination that cannot be made** is the add's own failure, returned as `UpstrokeError::Git` naming the path and
  the OS error, at once and without running Git. That is where Git's own add would have failed to create it, so a
  verification defers on it as before.
- **A refused add removes the destination it made,** when it is still that directory. The witnesses leave nothing at
  the slot (`own_entry_every`, `perm_add`: `slot_exists=false`).
- **A new crash state, and how it converges.**
  - A coordinator killed between making the destination and Git's first write leaves an intent, an empty slot and no
    registration.
  - Its residue class is `None`: `classify_object_residue` reads an unregistered slot as `None`
    (`src/workspace_manager/residue.rs:228`).
  - The reclaim's forced removal takes it. When the repository then has no registration store at all, the removal's
    scan (`src/workspace_manager.rs:5147`) returned the store's NotFound as an I/O error on every attempt. It now
    binds nothing for an empty directory at the target, so the removal takes it.
  - Executed: unpatched, Io and the slot kept; the prototype, Ok and the slot gone; three rounds each
    (`store_absent_empty_slot`).
- **The kill sampler** samples Git's command alone (`sampled_command`, `src/workspace_manager/tests.rs:12870`). ST-07's
  histogram for the add sites does not move; the funnel's new step comes before the sampled child.

**Windows.**
- std has no file identity there, so the veto reads only "an empty directory, not a link or reparse point". Nothing but
  this access and its Git child touches the destination, because it is in the run's private root.
- One case reads as untouched: a failure after the takeover whose junk removal left the destination itself, empty.
  It is attempted again and refuses at the deadline, never as Git state.

### 5.4 The access as round 6 specifies it

These are the deltas against §3.3, §3.4 and §4.3.

**Withdrawn:**
- §3.3's two reads, C1, C2 and C3, its "Why it is exact" and its row R7;
- §4.3's spelling rule, held clause and "An add's destination".

**Kept:**
- the attempts and the parse inside the list's attempt (§3.3, FUB-D3-TORNOK);
- R-X read-write and the one deadline (§3.4);
- targeted removal (§3.5);
- `UpstrokeError::RegistryRefused` (§4.3).

**Where it applies** (`src/workspace_manager.rs` at master `92c4ca81`). Every access calls
`tolerant_registry_access` (§5.5):

| Access | Its attempt | `hold` | `again` |
|---|---|---|---|
| `worktree_records` (`:5051`) | the list and its parse | `Unheld` | always |
| `add_worktree` (`:2649`) | the add (`:2708`), inside its funnel, after the destination is made | `Shared` | the destination is still the empty directory it made |
| `remove_worktree_proving`'s scan (`:2988`) | `revalidate_removal_proving` (`:5119`) | `Unheld` | always |
| `slots_with_torn_registrations` (`:5345`) | the torn plan's scan | `Exclusive` | always |

**Not wrapped** (as §3.3):
- `remove_bound`'s mutation;
- `git fsck` on the refusing path;
- `read_only_git`'s reads.

**The removal scan with no store** (`:5147`): an empty directory at the target binds nothing (§5.3); anything else
there keeps the I/O refusal.

**The bound.**
- An access returns by its deadline plus the runtime of the one attempt it started before the deadline (§3.4).
- Within ten seconds the backoff allows at most about 205 attempts. Under the test deadline of 500 ms it allows 15,
  and the witnesses that refused ran 14 or 15 (`d6/census/probe-vi/witness-runs/TABLE.txt`).

### 5.5 The helper's contract, for follow-up D

**Stable from 2026-10-02 (round 6).** Any later change to this subsection is marked here with its date and what
changed, so that D's design can be checked against it again. The orchestrator compares the two before D is
implemented.

> **Checked 2026-10-03, repair round 4 (§9.14): no change to this contract.** Its item R7 widens what the topology's
> list attempt covers where the manager compares the listed worktrees: the resolution of each path the list names
> now runs inside the access. That is the caller's attempt, not the helper. D's legacy accesses resolve no listed
> path, so it asks nothing of D.
>
> **Checked 2026-10-02, design round 9: no change to this contract.** §7.6 stays the current text. Round 9's one rule
> change for the add (§8.2: a destination that is not an empty directory when the access begins is Git state with no
> Git run) sits in the topology add's destination steps, before the helper's loop. D's snapshot destinations are fresh
> names the access makes, so it asks nothing of D. The external-prune finding's closures (§8.4 to §8.6) are proposals
> for a later change; closure 1's check is the topology form of D's C-SIDE (#331's record §3.4).
>
> **Changed 2026-10-02, design round 8.** §7.6 is the current text. §6.4 stands where §7.6 does not change it, and the
> round-7 note below is kept as history. Re-check D's record against these:
> 1. **The registry-free checkout probe is withdrawn** (FUB-D7-SPLITINDEX and FUB-D7-CONFIG, two executed P1s in the
>    probe itself). Item 6 of the round-7 note no longer holds. No add veto runs a Git command, and D adopts no probe,
>    including a private gitdir/commondir variant.
> 2. **The add veto is the removal proof alone.** After a failed attempt:
>    - an empty destination the access can remove is untouched: it is made again, and the answer is `Attempt`;
>    - a destination that is absent, or an empty directory the access cannot remove, answers `Undecidable`, never
>      `Return`;
>    - anything else answers `Undecidable` too.
>
>    For D's legacy add this is the default the orchestrator set, because `Return` leads to legacy's discard. For the
>    topology add it is round 8's decision (§7.3).
> 3. **The bound's veto term** is a few metadata calls, one `rmdir` and one `mkdir`, for both adds.
> 4. **The external-prune class is filed on this branch** as
>    `PR329-AN-EXTERNAL-PRUNE-DELETES-AN-ENGINE-WORKTREES-REGISTRATION` (P1, §7.4). D's R-D9 refers to it and files no
>    duplicate.
>
> **Unchanged from round 7:**
> - `Again` and its three answers, and `Undecidable` refusing at once;
> - the final attempt, the end-to-end bound, and `CONTENDED_ATTEMPTS` with `contended_attempts()`;
> - the helper's name, module and `effect_free` row, `RegistryHold`, the canonical `common_git_dir`, and nothing
>   sampled;
> - the deadline's values, and `RegistryRefused` as the one refusal variant.
>
> **For follow-up C, unchanged and stated for its R-P:**
> - every explicit engine `git worktree prune` stays deleted (§3.5; `src/workspace_manager.rs:3061`, `:3100` and
>   `:3123` at `5c222ff2`);
> - removal stays bound to the instance's registration;
> - there is no global-prune fallback.

> **Changed 2026-10-02, design round 7 (§6.4 is the current text; the round-6 text below is kept as history, and where
> the two differ §6.4 governs).** Re-check D's record §1.2 against these:
> 1. **`again` returns `Again`**, not `bool`: `Attempt`, `Return` (the attempt's error, unchanged), or
>    `Undecidable { why }`. A caller that passed `|| true` passes `|| Again::Attempt`. (FUB-D6-DABSENCE)
> 2. **`Undecidable` refuses at once** as `RegistryRefused`, naming `why`, with no further attempt. A veto that cannot be
>    evaluated is never returned as Git state and never attempted past. (FUB-D6-DABSENCE)
> 3. **The final attempt.** An attempt that follows a sleep the deadline cut short is made, and it is the last, so a
>    store a writer leaves whole by the deadline is passed. Under the 500 ms test deadline an always-failing access now
>    makes 16 attempts. (FUD-D1-PROGRESS, carried here)
> 4. **The bound is end to end:** the deadline, plus the runtime of the access's last attempt, plus the runtime of the
>    veto after it. The helper bounds neither. (FUB-D6-BOUND)
> 5. **`CONTENDED_ATTEMPTS` is part of the contract, for tests only:** `#[cfg(test)] pub(crate) static
>    CONTENDED_ATTEMPTS` and `#[cfg(test)] pub(crate) fn contended_attempts(common_git_dir: &Path) -> usize`, counting
>    each `Attempt` answer per common git dir as passed. (D's record §1.2 item 8)
> 6. **An add's veto no longer reads "taken over" as "its own failure", nor "untouched" as "not taken over".** "What
>    `again` is for" below is replaced by §6.3: an empty destination the access can remove is untouched; otherwise a
>    registry-free checkout of the commit at the destination decides. D's legacy add veto at `37e4d8c4` (owned,
>    unchanged, empty) has both of round 6's holes (FUB-D6-PRUNE, FUB-D6-INODE) and adopts §6.3. (§6.4, "For D's legacy
>    add")
>
> Unchanged: the helper's name, module and `effect_free` row, `RegistryHold`, the canonical `common_git_dir`, nothing
> sampled, the deadline's values, and `RegistryRefused` as the one refusal variant.

**Where it is, and how it is classified.** In `src/workspace_manager.rs`, which is not frozen.
- It is `pub(crate)`, so `src/effects/tests.rs`'s census of externally reachable functions classifies it. Its name
  joins `src/workspace_manager.rs`'s `effect_free` list in `effects/wrappers.toml` (`:134`).
- Measured on the prototype without that row:
  `effects::tests::every_externally_reachable_fn_of_a_legacy_or_shared_module_is_classified` fails, `unclassified:
  ["tolerant_registry_access"]` (`d6/census/probe-vi/census-norow.log`). With the row it passes (suites 3 and 4).
- Its body reads the clock, takes R-X and sleeps. The attempt is the caller's. So it is `effect_free` by the same
  reading that classifies `worktree_records`, which runs Git.
- `RegistryHold` is a type, which the census does not classify.

**The exact signature:**

```rust
/// How a registry access takes this process's registry lock (R-X) around each attempt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RegistryHold {
    /// Not at all: a list, a scan, a removal.
    Unheld,
    /// Shared: an add. Adds of one process do not wait for each other.
    Shared,
    /// Alone: the torn-registration plan, which must not read this process's add in flight as a dead add's residue.
    Exclusive,
}

pub(crate) fn tolerant_registry_access<T>(
    common_git_dir: &Path,
    hold: RegistryHold,
    again: &mut dyn FnMut() -> bool,
    attempt: &mut dyn FnMut() -> Result<T, UpstrokeError>,
) -> Result<T, UpstrokeError>
```

**`common_git_dir`** is the canonical common git directory: `git rev-parse --path-format=absolute --git-common-dir`
followed by `fs::canonicalize`, as `WorkspaceManager::common_git_dir` holds it. It is R-X's key, and a refusal names
`<common_git_dir>/worktrees` as the store. A caller that passes another spelling shares no R-X with this process's
manager of the same repository.

**What it samples: nothing.** It reads no store state, no error text and no timestamp.
- It reads the monotonic clock for its deadline, and takes R-X as `hold` says.
- The only things that decide another attempt are `again` and the deadline.

**When it attempts again.**
1. It runs `attempt`, under R-X when `hold` is `Shared` (the read side) or `Exclusive` (the write side). R-X is
   released when the attempt returns.
2. On `Ok`, it returns at once. `again` is never called before the first attempt or after a success.
3. On `Err`, it calls `again()` once, outside R-X.
4. If `again()` is false, it returns that `Err` exactly as the attempt returned it: any variant, with its text.
5. If it is true and the deadline has passed, it refuses.
6. Otherwise it sleeps the backoff: 1 ms, doubling, at most 50 ms, never past the deadline.
7. If the deadline has passed after the sleep, it refuses; otherwise it goes back to 1.

**What `again` is for.** It is the caller's veto: whether another attempt is safe, and still the same operation.
- **The topology's add** passes "the destination is still the empty directory the add made: the same device and inode
  on Unix, and still empty" (§5.3). Its other accesses pass `|| true`.
- **D's legacy add** passes its own predicate: its owned, pre-created destination unchanged (same device and inode),
  empty, and named by no registration, which is the round 4 and 5 predicate. Any read that predicate makes is D's.
- An add's caller must veto once Git has taken the destination over. Otherwise a failed checkout is attempted again
  until the deadline and comes back refused.

**What it returns:**
- `Ok(T)` from the first successful attempt.
- The failed attempt's own `Err`, unchanged, when `again` vetoes. This is the only way it returns
  `UpstrokeError::Git`.
- `Err(UpstrokeError::RegistryRefused { message })` when the deadline passes with the last attempt failed. The message
  names the store, the deadline, the attempt count and the last failure's display text.
- The same variant when R-X stays held elsewhere in this process until the deadline (`Shared` or `Exclusive`): no
  further attempt runs, and the message names R-X and the deadline.
- `RegistryRefused` is the new variant in `src/error.rs`. It is displayed as its message, like `Refused`, and
  `run::verified` does not map it to `Verified::Unavailable`.

**Its deadline.** One per call, fixed when the call begins: `REGISTRY_ACCESS_DEADLINE`.
- It is ten seconds in production and 500 ms under `cfg(test)`, so D's tests in the same crate get 500 ms.
- It bounds every wait for R-X, every backoff sleep, and the start of every attempt.
- It does not bound an attempt that has already started. An access returns by its deadline plus that attempt's
  runtime.

**Executed** on the prototype (`d6-contract-witnesses.rs`; three rounds; `d6/census/probe-vi/witness-runs/TABLE.txt`):
- a veto after the first failure returns `Git("attempt 1 failed")` after one attempt, the predicate asked once;
- an always-failing attempt is `RegistryRefused` after 15 attempts at 500 ms, the text naming the count and the last
  failure;
- two failures and then success return `Ok(30)` after three attempts;
- with R-X held alone by another thread, a `Shared` access is `RegistryRefused` at 500 ms with no attempt run, and an
  `Unheld` one succeeds at once.

**For tests only.** The `#[cfg(test)]` counter of attempted-again accesses per common git dir (round 4's
`CONTENDED_ATTEMPTS`, T2's handshake) is not part of the production contract.

### 5.6 What is closed, what remains, and what G6 meets

**The claims, once implemented:**
1. **No manager registry access returns `UpstrokeError::Git` for anything the registry's state caused.** That covers
   another process's write in flight, a write a dead process left torn, and a registration nobody is writing. Only an
   add's own failure after Git took its destination over comes back as Git state. So for (b), no registry state
   reaches `run::verified`'s Git arm.
2. **(a):** an attempt's access passes another process's write. It ends the command only if the store stays in the way
   until the deadline, and then resumably.
3. **(d):** the same, whoever the writer is: a legacy run, another topology run, an agent's Git, the user's.
4. **The add's own failure after the takeover is returned as it was, at once.** That is its checkout or its own
   registration's files, so `not_repairs` still defers or parks on it.
5. **Each access returns by its deadline plus one attempt's runtime** (§5.4).
6. **No engine process deletes another process's registration** (§3.5).

**What remains** (it replaces §3.7's and §4.7's rows; R7 is withdrawn, because no text is read):

| | What | Consequence now | Finding |
|---|---|---|---|
| R1′ | A registration that stays torn until the deadline | the access refuses resumably as `RegistryRefused`, never as Git state; the dead run's own resume repairs its residue | `PR308-R3-…` (consequence narrowed) |
| R2 | A host agent's own Git | its torn entries are attempted past; its prune of an engine add's entry before `locked` costs the add another attempt (`prune-own`, executed); the agent's own prune still deletes entries | `PR11-HOST-AGENT-PRUNE-RACES-AN-ENGINE-ADD`, narrowed to the agent's own commands |
| R3 | The user's Git in any checkout | attempted past the same way | none |
| R4 | `fsck` on the refusing path | a refusal either way | none |
| R5 | (c): a dead coordinator's Git writers against a recreated slot | not this change's | `PR329-A-RESUME-REBINDS-A-SLOT-ITS-DEAD-COORDINATORS-GIT-CHILD-STILL-WRITES` (P1, follow-up C, blocks G6) |
| R6 | The legacy engine's registry accesses and its discard | follow-up D, which calls §5.5 | `PR329-LEGACY-RUNS-IN-LINKED-CHECKOUTS-RACE-THE-SHARED-WORKTREE-REGISTRY` (P1; guard: follow-up D, before G6) |
| R8 | One attempt's runtime (§3.4) | an access can exceed its deadline by one Git command or scan | stated |
| R9 | Windows: no file identity in std | the veto reads an empty directory only; a takeover whose junk removal left the destination empty is attempted again and refuses at the deadline | stated |
| R10 | A registry fault that is not contention (an unwritable store, a registration Git cannot list, an unreadable registry file), or a fault of the whole repository met before an add's takeover | refuses at the deadline instead of failing at once as Git state (§5.3) | stated |
| R11 | A clean or smudge filter the repository configures runs inside the checkout, after the takeover | if such a filter itself enumerates the registry and dies on a torn entry, the checkout's failure is the add's own and is returned as Git state; hooks are already disabled (`core.hooksPath`) | stated |

**The cases.** Topology only; the legacy cases point at follow-up D.

| Case | Closed by | Severity | Applies to G6 | Blocks G6 |
|---|---|---|---|---|
| (a) An attempt's pipeline error | §5.3 and §5.4, in the non-frozen `src/workspace_manager.rs` and `src/error.rs` | P1 | yes: R17, the shared registry, Q6 | until implemented and validated |
| (b) A durable verification deferral or park | §5.3 and §5.4 | P1 | yes: Q6 and durable verification | until implemented and validated |
| (c) DESC | filed, follow-up C | P1 | yes: Q1, INV-22, ST-16, ST-18 | yes; filing is no waiver |
| (d) A legacy writer tears a topology reader | §5.3 | P1 class | yes | until implemented and validated |
| (e1) Legacy against legacy, write in flight | follow-up D | P1 | no | no; it remains a P1 until D lands |
| (e1′) Legacy against legacy, static or deadline residue | follow-up D | P1 | no | no; it remains a P1 until D lands |
| (e2) A topology writer tears a legacy reader, write in flight | **follow-up D**, through §5.5 | P1 | yes: Q6, across the shared registry and R17 | **yes, until follow-up D is implemented and validated** |
| (e2′) A topology writer's static or deadline residue makes a legacy reader refuse, then discard paid output | **follow-up D** | P1 | yes: Q6; a crash producer engages Q1 | **yes, until follow-up D is implemented and validated, unless the owner rules otherwise** |

**The findings:**

| Item | Severity | Here | Applies to G6 | Blocks G6 |
|---|---|---|---|---|
| `PR11-LINKED-CHECKOUTS-RACE-THE-SHARED-WORKTREE-REGISTRY`: (a), (b), (d) | P1 | repaired by §5.3–§5.4 once implemented; its file is deleted then | yes | only until this change merges |
| `PR329-A-RESUME-REBINDS-A-SLOT-ITS-DEAD-COORDINATORS-GIT-CHILD-STILL-WRITES`: (c) | P1 | filed; follow-up C | yes | yes, until follow-up C merges |
| `PR329-LEGACY-RUNS-IN-LINKED-CHECKOUTS-RACE-THE-SHARED-WORKTREE-REGISTRY`: (e) | P1 | filed; guard: follow-up D, before G6 | (e2) and (e2′) yes; (e1) and (e1′) no | yes, through (e2) and (e2′), until follow-up D lands, unless the owner rules otherwise; filing is no waiver |

The other rows of §3.7's table stand.

### 5.7 Regression tests

Each test is the implementation's, named by the implementer. Each lives outside the frozen modules and their test
children, and waits on a handshake or a seam, with time only as a watchdog. Each mutation runs on a scratch tree whose
Compiling line names it.

**Carried unchanged in substance** from §3.8:
- T1, two processes, at least 1,000 cycles, 0 failures;
- T2, the verification beside a transient foreign tear, with no deferral;
- T2′, the verification beside a static tear: `RegistryRefused`, nothing durable;
- T3, three processes;
- T5, no engine prune;
- T6, an add whose own entry a prune removes before its `locked` (now also executed at the Git level, `prune-own`);
- T9, the deadline and R-X;
- T10, the legacy writer beside the manager;
- T11, TORNOK, whose control now expects `RegistryRefused`;
- T13, R-X shared.

**Revised and new:**
- **T4, the helper's contract** (it replaces §4.8's classifier T4): the four contract witnesses of §5.5 as unit tests.
  - Mutation m1: the helper returns its first failure. T1, T4 and T14 turn red.
  - Mutation m2: the deadline's refusal typed `UpstrokeError::Git`. T2′ and T4 turn red; the verification defers.
- **T12, FUB-D3-PERM:** `RegistryRefused` after the deadline, carrying Git's "Permission denied", and nothing left at
  the slot. Executed (`perm_add`).
- **T14, FUB-D4-OWNENTRY through the production add:** two windows give Ok on the third attempt; every attempt gives
  `RegistryRefused` at the deadline with nothing at the slot. Executed (`own_entry_two`, `own_entry_every`).
  - Mutation m3: an `again` that always vetoes. Git after one attempt: red.
- **T15, `not_repairs` through the add:**
  - A verification whose snapshot the checkout cannot make terminates `merge_verification_unavailable` (Deferred), and
    the run completes. The constructions are FUB-D5-GENUINE's tree and a destination that cannot be made.
  - At the manager, both the tree and a 300-byte name return Git after one attempt.
  - Executed: `verification_snapshot`, `genuine_gitpath`, `checkout_cannot_be_made`.
  - Mutation m4: an `again` that never vetoes, so the checkout is attempted again. Red: 15 checkouts, `RegistryRefused`,
    and the verification ends with nothing appended.
- **T16, the takeover rule (new):**
  - a refused add removes the destination it made;
  - an add whose destination exists and is not empty is attempted once and returns Git ("already exists"), keeping
    what was there;
  - a destination whose parent cannot be written fails at once as Git state naming it.
  - Mutation m5: no destination made, with round 4's "nothing at the slot" condition. Red: a checkout failure leaves
    nothing at the slot and is attempted again.
- **T17, the store-absent empty slot (new):** an intent, an empty slot and no registration store, then the forced
  removal: Ok, and the slot is gone. Executed (`store_absent_empty_slot`); the unpatched tree gives Io on every
  attempt.
  - Mutation m6: the empty-target branch removed. Red.
- **T18, the reviewers' round-5 interleavings (new), which the implementation reproduces with its seams:**
  - T14's interleaving on a repository whose `.git` is a link to a directory of another name (FUB-D5-SPELLING);
  - a list that fails once and succeeds on its next attempt (FUB-D5-OPTFILE's effect; the lock-and-unlock sequence
    itself needs a pause inside Git and is executed at the Git level, `optfile`).

**The proof the implementer owes:** §3.8's.
- the mutations above;
- the witnesses red unpatched where they apply;
- the frozen children unchanged;
- the ten gates, and CI on every leg;
- at least five full suites, with every frozen test's failures counted against the same number of suites at the base.
  On the prototype, the frozen modules passed in suites 3 and 4: recover 226 in suite 4 and 225 in suite 3, whose one
  failure passed alone (§5.8); integrate 20, repair 5, finalize 5, fold 192, `events::log` 47, the legacy
  `engine::tests` 188 and `workspace::tests` 47 (`d6/census/probe-vi/frozen-census.txt`).

### 5.8 Effect governance, instruments and the frozen set

**Measured on the prototype** (`d6/census/probe-vi/`):
- Clippy `-D warnings` over all targets: rc 0 (`clippy-3`).
- Two whole suites at the final shape: 2,997 passed, 3 failed and 113 ignored in each (`suite-3.log`, `suite-4.log`).
  - Both suites fail the two non-frozen tests every round since round 4 has moved (§3.6). The second,
    `an_add_killed_before_it_wrote_gitdir_is_unlisted_and_refuses_forced_cleanup`, now gets `RegistryRefused` after the
    deadline, carrying the same Git text.
  - Suite 3's third failure is a frozen recovery test's cleanup-lease observation, in
    `a_resume_over_a_creation_that_stopped_after_creating_its_integration_ref_adopts_it`: "still has a process of its
    own alive … holds the run's cleanup lease". It passed alone three times (`alone-recover-{1,2,3}.log`). Its refusal
    is the one `PR281-CLEANUP-LEASE-HOLD-OUTLIVED-AND-ITS-UNREADABLE-TWIN` records from other tests: a fingerprint
    match, whose holder was not identified (repair round 8, §9.18.3).
  - Suite 4's third failure is a real-Docker test: "still running after 200 observations". It passed alone three times
    (`alone-docker-{1,2,3}.log`). That is the class
    `findings/P3_correctness_202609121234_the-docker-termination-poll-counts-yields-not-time.md` names.
- Every instrument census passes in both suites (`frozen-census.txt`).

**What the implementation moves:**
- **Code:** `src/workspace_manager.rs` (the helper and `RegistryHold`, R-X's type, the add's destination and veto,
  targeted removal, and the removal scan's store-absent branch) and `src/error.rs` (`RegistryRefused`). These hunks stay
  inside the registry access, the add's funnel and the removal, disjoint from follow-up C's Git writer spawn
  configuration and from follow-up D's legacy modules.
- **One instrument row:** `tolerant_registry_access` in `src/workspace_manager.rs`'s `effect_free` list
  (`effects/wrappers.toml`), measured in §5.5.
  - Nothing else moves: no `clippy.toml` entry, no `effects/allowlist.toml` text, no `src/effects/` test, and no frozen
    module or frozen test child.
  - Under CLAUDE.md's first limb that row is an instrument edit. In this lane's merge bar the owner's 2026-09-27/28
    direction reads that instrument reasoning no longer gates (§4.6). It is disclosed in the body.
- **Tests:** the two non-frozen tests above, and §5.7's.
- **Docs:**
  - `DESIGN.md` §15's PROPOSED paragraph becomes the in-force one.
  - `docs/internals/engine/topology/run.md`'s R8 paragraph and its pin
    `the_verification_notes_say_a_registry_another_process_is_writing_spends_a_deferral_or_parks`
    (`src/engine/topology/run/tests.rs:535`) change: registry state never reaches the arm, and the mapping is unchanged.
  - The module notes of `src/workspace_manager.rs` change for R-X, the helper, the destination and the removal.
  - The findings: the repaired file is deleted, and `PR5-RD-003…`, `PR308-R3…`, `RESIDUE-UNBINDABLE…` and
    `PR11-HOST-AGENT-PRUNE-RACES-AN-ENGINE-ADD` are updated.

### 5.9 Design review round 5, answered

**Design review round 5** ran three `gpt-6-astra` lenses at `max` on `4a126215` (`cameron-codex`), from 2026-10-02T07:24:52Z;
their logs closed at 07:36:58Z, 07:40:29Z and 07:42:15Z. All three returned CHANGES_REQUIRED. The texts are
`~/orch-pr11/reviews/review-329-d5-{design,concurrency,regression}-4a126215.review.md`, with their hashes in
`SHA256SUMS-329-d5`; the witnesses are in `329-d5-witnesses/`, and the triage is `review-329-d5-triage.md`.

| Finding | Sev | Kind | Round 6 | Where | Evidence |
|---|---|---|---|---|---|
| FUB-D5-SPELLING | P1 | executed | **Fixed (design), witnessed.** Nothing reads Git's text, so no spelling can be missed; the add is attempted again because its destination is untouched. | §5.3 | `d6/witness/`: `spelling-symlinked`, Ok on the second attempt (2.43.0, 2.55.0); planned T18 |
| FUB-D5-OPTFILE | P1 | executed | **Fixed (design), witnessed.** Nothing samples a file; every failed list is attempted again. | §5.3 | `optfile`, Ok on the second attempt (2.43.0, 2.55.0); planned T18 |
| FUB-D5-GENUINE | P2 | executed and reasoned | **Fixed (design), witnessed.** A checkout failure is after the takeover and comes back as Git state after one attempt, whatever its text. | §5.3 | `genuine-gitpath` (all three Git versions); prototype `genuine_gitpath`; planned T15 |
| FUB-D5-INDEX | P1 | executed | **Moved to follow-up D** with B-PRESERVE. | §4 banner | the legacy finding's guard |
| FUB-D5-RESTORE | P2 | executed | **Moved to follow-up D.** | §4 banner | the legacy finding's guard |
| FUB-D5-UNFREEZETEXT | P3 | reasoned | **Moved to follow-up D.** | §4 banner | the legacy finding's guard |

**Round 4's findings, re-read:**
- **FUB-D4-OWNENTRY** stays fixed. It is now witnessed under a form with no classifier at all (`own_entry_two`,
  `own_entry_every`).
- **FUB-D3-PERM** stays fixed by its second remedy (§4.2).
- **FUB-D4-B1PREDICATE, FUB-D4-B1REMOVE and FUB-D4-RESUME** were fixed in the legacy design that moved to D, and their
  rows move with it.

### 5.10 Risks, sequencing, and what is out of scope

These are §3.12's and §4.10's topology risks, with the following changed.

**Risks.**
- **Registry faults cost the deadline** (R10). Every access over a store nobody can write, or over a registration Git
  cannot list, waits ten seconds and refuses. The run stops resumably until the fault is repaired.
- **The destination is made before Git runs.**
  - A coordinator killed in between leaves an empty slot. Residue class `None`; the reclaim takes it (§5.3).
  - A destination that cannot be made is Git state naming it.
- **Windows' veto has no inode** (R9).
- **Filters run after the takeover** (R11).
- **`UpstrokeError` gains a public variant**, `RegistryRefused`, and the crate gains one `pub(crate)` function and one
  `pub(crate)` enum.
- **A lease refusal and a Docker failure** reddened one suite each on the prototype, and each passed alone three times
  (§5.8).
  This change touches neither a lease nor a container.

**Sequencing.**
- The implementation depends on neither #328, follow-up C nor follow-up D.
- **Follow-up D's implementation follows this change's merge,** because D calls §5.5.
- D's (e1) and (e2) closure depends on this contract, not on any classifier. Rounds 3 to 5's classifier is gone.

**Out of scope, and said so.**
- The legacy engine (follow-up D).
- DESC (follow-up C).
- Foreign Git's own commands (R2, R3).
- A dead legacy coordinator's Git children against its own resume (§2.13).

## 6. Round 7 design

> **SUPERSEDED by §7 (design round 8) where §7's banner says; the rest of §6 stands as §7 cites it.**
> - **Withdrawn:** §6.3's registry-free checkout probe, the widening of the store-absent exception, §6.1's convergence
>   argument, §6.4's paragraph for D's legacy add, and §6.7's and §6.8's probe items.
> - **Replaced:** §6.3's rule by §7.2; §6.6's claims 1 and 4 and its rows R2′, R3′, R8′, R9′, R11, R12 and R13 by
>   §7.7; §6.7's T15, T16, T17 and T20 by §7.8.
> - **Corrected:** §6.4's "§3.8" reads "§3.6".
> - **Stands:** §6.2, §6.3's removal proof, §6.4's contract, §6.5, §6.9 and §6.10.
>
> *Round 7's banner:* **PROPOSED — for design review round 7.** This section supersedes §5 only where it says so, item
> by item:
> - §5.3's veto ("What the veto reads", the destination's "existing empty directory is used as it is", and "Windows")
>   is replaced by §6.3;
> - §5.4's bound is replaced by §6.4;
> - §5.5's contract changes as its dated note says, with the current text in §6.4;
> - §5.6's claims 1, 4 and 5 and rows R1′, R2, R3, R8 and R9 are replaced by §6.6;
> - §5.3's store-absent exception ("an empty directory at the target binds nothing") is widened by §6.3;
> - §5.7's T4, T15, T16 and T17 are revised, and T19 to T22 are added, by §6.7;
> - §5.1's and §0's platform sentence is corrected by §6.10.
>
> Everything else in §5 stands: the helper retries every failed attempt until the deadline and classifies nothing, the
> list's parse stays inside its attempt, R-X keeps its read-write shape, targeted removal stays, and
> `UpstrokeError::RegistryRefused` is unchanged. The closure still needs no owner decision and edits no frozen module.
> Nothing in §6 is in force until the implementation lands.

Design round 7 is `pr11_fub_design7`'s (`claude-opus-5-5`, `max`), spawned on `ed3a97d9` to answer design review round 6
(`~/orch-pr11/reviews/review-329-d6-triage.md`). Its figures are under `~/orch-pr11/logs/pr11_fub_design7/`, cited as
`d7/…`; `d7/SUMMARY.txt` is the index. It wrote no production code and built no prototype. Its evidence is Git source
and Git-level witnesses: a Python model of the access (`d7/witness/d7policy.py`) running our own `git` commands, the
three upstream Git builds `d7/gits/{2.43.0,2.50.1,2.55.0}` made from kernel.org tarballs checked against kernel.org's
signed list (`d7/gits/verify-tarballs.txt`), and an LD_PRELOAD pause shim (`d7/witness/d7shim.c`). The reviewers'
round-6 witnesses were copied from `/tmp` to `d7/review-witnesses/`, with hashes in `d7/review-witnesses-SHA256SUMS`.
Master is now `5c222ff2`, follow-up A merged. The branch is not rebased, and a merge with master is clean. Of the files
this record cites with a line, three differ at `5c222ff2` (`d7/census/citations-5c222ff2.txt`):
- `effects/wrappers.toml` and `src/engine/topology/coordinator.rs`, whose cited lines are unchanged;
- `src/agent/proc.rs`, whose `:1028` and `:1230` moved. Both are cited in §1, withdrawn with rounds 1 and 2, and stay as
  history at `92c4ca81`.
`design/15` differs at master only in a paragraph this record does not cite. §6 cites `5c222ff2` throughout.

### 6.1 What design review round 6 found, and why this round converges

**The findings** (triage, 2026-10-02T10:28Z). The concurrency lens ran on its first attempt. The design and regression
lenses were refused on [cyber] grounds twice each and were recast as conformance readings, which ran on the third
attempt. All three returned CHANGES_REQUIRED.

| id | sev | what |
|---|---|---|
| FUB-D6-PRUNE | P1 | A `git worktree prune` deletes the add's registration after Git took the destination over; the add fails, Git's junk removal takes the destination, and §5.3's veto returns the failure as Git state. Executed on 2.43.0 and 2.55.0. |
| FUB-D6-INODE | P2 | A takeover whose junk removal cannot remove the destination leaves the same empty inode, so §5.3 reads a genuine failure as untouched and refuses it at the deadline. Executed on 2.43.0 and 2.55.0. |
| FUB-D6-DABSENCE | P2 | §5.5 does not say what happens when the caller's veto cannot be evaluated. |
| FUB-D6-STATICRESUME | P2 | The dead run's own resume cannot necessarily repair a static tear: `derive` refuses first. |
| FUB-D6-BOUND | P2 | The published bound leaves out the veto's own runtime. |
| FUB-D6-PLATFORM | P3 | "Every reviewer witness on all three Git versions" overstates the Windows evidence. |

**The looping signal (MAINTAINING, "When a pull request may be looping") appeared a sixth time.** FUB-D6-PRUNE is a P1 in
round 6's own machinery, the takeover veto.
- **What the defect in the repair was.** §5.3's veto read the destination's state after a failure as a statement about
  where Git had failed. Both of round 6's add findings are holes in that one inference:
  - "taken over" did not mean "the add's own failure": a prune can make a failure after the takeover (PRUNE);
  - "untouched" did not mean "not taken over": junk removal can fail to remove the destination (INODE).
- **What survives.** Neither finding touches the helper. Retrying every failed attempt until the deadline, with no
  classifier, is unchanged and was not faulted in any lens. DABSENCE, BOUND and the contract items are about the
  helper's published edges, not its loop.
- **The fix replaces the inference with two direct observations** (§6.3):
  - "untouched" is proven by the removal Git's own junk removal performs: the access removes the empty destination, as
    Git would have had it taken it over;
  - "the add's own failure" is decided by running the add's checkout again without the registry, at the destination:
    the registry-free checkout probe. If that checkout cannot be made, the add could not have succeeded; if it can, the
    failure came from the registry.
- **Why it converges where rounds 3 to 6 did not.**
  - Nothing reads Git's text, the store or a timestamp; the probe interprets nothing.
  - Each observation rests on one fact, and each fact is executed on all three Git versions: that Git's junk removal is
    that removal (§6.3), and that the probe opens nothing under the store (strace, §6.3).
  - Everything the observations can get wrong errs one way. A checkout the probe cannot reproduce is attempted again and
    refuses resumably at the deadline. Nothing a registry does can make the probe fail, so no registry state reaches
    Git state through it (R11's filter excepted, as in round 6).
- **The smaller changes, weighed** (MAINTAINING: "the smaller change is the one to propose").
  - Retry every takeover failure too: round 5's form. A checkout that cannot be made refuses at the deadline, which is
    FUB-D5-GENUINE again.
  - Retry a takeover failure a bounded number of times: two prunes return Git. Executed (`double-prune`, policy `once`,
    §6.2).
  - Keep §5.3 and state the exposure: it leaves an open P1 the brief requires closed.
  - The probe is the smallest change that closes the P1 without reopening GENUINE. It adds one Git child, which runs
    only after a failure that §6.3 cannot prove untouched.

### 6.2 FUB-D6-PRUNE: Git's prune rules on 2.43, 2.50 and 2.55, and why no retry rule closes it

**Git's prune decides, then deletes, and never looks again** (`d7/git-src/prune-rule-lines.txt`; full text in
`d7/git-src/prune-rules-citations.txt`):

| Step | v2.43.0 | v2.50.1 | v2.55.0 |
|---|---|---|---|
| `should_prune_worktree` (`worktree.c`) | `:719-785` | `:900-989` | `:929-1018` |
| not a directory: prune ("not a valid directory") | `:729` | `:921` | `:950` |
| `locked` exists: keep | `:732` | `:925` | `:954` |
| no `gitdir`: prune ("gitdir file does not exist"), whatever the expiry | `:735` | `:930` | `:959` |
| `gitdir` unreadable, short or empty: prune | `:740`, `:749`, `:759`, `:767` | `:936`, `:947`, `:953`, `:961` | `:965`, `:976`, `:982`, `:990` |
| `gitdir` names a missing `.git` and the index is older than the expiry: prune | `:775` | `:976` | `:1005` |
| `prune_worktrees`: the decision, then the deletion, with nothing between that reads the entry again (`builtin/worktree.c`) | `:215`, `:216` | `:229`, `:230` | `:229`, `:230` |
| `delete_git_dir`: `remove_dir_recursively` | `:148` | `:155` | `:155` |
| `git worktree prune` with no `--expire`: `expire = TIME_MAX` | `:244` | `:259` | `:259` |

- `prune_dups` (`:192-201`; `:201-210`; `:201-210`) and `delete_worktrees_dir_if_empty` (`:157-160`; `:164-169`;
  `:164-169`) are the other two deletions. The first considers only entries it kept with a `gitdir`, so an add in flight
  holding `locked` is never one; the second removes an empty store, which an add then meets before its takeover.

**Who runs a prune** (`d7/git-src/prune-rules-citations.txt`):
- `git worktree prune` itself.
- `git gc`, with `gc.worktreePruneExpire` (default `3.months.ago`: `builtin/gc.c` `:64`, `:161`, `:162`). Its step is
  `:734` (2.43.0), `:1009` and `:1043`. The rules above the expiry branch ignore the expiry, so gc deletes an add's entry
  caught before its `locked` like any prune.
- **Auto maintenance**, which `git commit`, `git fetch` and `git merge` run (`builtin/commit.c` `:1871`, `:1935`, `:1965`;
  `fetch.c` `:2493`, `:2679`, `:2885`; `merge.c` `:463`, `:490`, `:509`):
  - 2.43.0 and 2.50.1: only the gc task is enabled by default (its `tasks[]` row's `1`, `:1289`, `:1572`), and gc runs
    only when it is needed. 2.50.1 has a `worktree-prune` task (`:1590`), off by default.
  - **2.55.0**: unscheduled maintenance uses the geometric strategy (`initialize_task_config`, `:1974`), which enables
    `worktree-prune` (`:1916`). That task runs whenever at least one entry is prunable (`worktree_prune_condition`,
    `:391-427`, default limit 1, `:394`). **An add in its window is such an entry.** So on 2.55.0 any commit, fetch or
    merge in any checkout of the repository can start a prune while the engine adds.

**The add's window.** Git makes its entry (`mkdir`, v2.43.0 `:458`, v2.50.1 `:473`, v2.55.0 `:507`) and writes its
`locked` (`:483`, `:498`, `:532`) in two steps. A prune that decides between them decides to delete; its deletion can
land at any later moment, and the takeover (`:489`, `:504`, `:538`) is only a few calls after `locked`.
- A deletion that lands **before the takeover** fails the add's own `locked`, which is §5.3's `prune-own`: the destination
  is untouched and the add is attempted again.
- A deletion that lands **after it** fails one of the add's own steps. Git's junk removal then takes the destination —
  the same end state as a checkout that cannot be made. **No predicate over the end state can tell the two apart.**

**Executed** (`d7/witness/d7scenarios.py`, deadline 500 ms, three rounds per row on each version;
`d7/witness/MATRIX.txt`).
- The add pauses before `locked`. The pruner decides and pauses before deleting: `d7shim.c` pauses it at its first
  `opendir` of the entry, which only `remove_dir_recursively` makes. The add writes `locked`, takes the destination over
  and pauses at X. The prune deletes, and the add resumes.
- In each of the 168 runs whose pruner decided, `locked` was absent at the decision, present wherever the add paused
  after its takeover, and the entry was gone after the pruner. The other 12 are `maint-prune-gitdir` on 2.43.0 and
  2.50.1, where maintenance ran no prune (`d7/witness/FIGURES.txt`).

| Scenario | 2.43.0, 2.50.1, 2.55.0 under §5.3 (r6) | under §6.3 (r7) |
|---|---|---|
| `prune-gitdir` (the reviewers' point) | Git after 1 attempt, 9 of 9 | Ok after 2 attempts and 1 probe, 9 of 9 |
| `prune-commondir` | Git, 9 of 9 | Ok, 2 attempts, 1 probe, 9 of 9 |
| `prune-HEAD.lock` (the HEAD update) | Git, 9 of 9 | Ok, 2 attempts, 1 probe, 9 of 9 |
| `prune-index.lock` (the checkout) | Git, 9 of 9 | Ok, 2 attempts, 1 probe, 9 of 9 |
| `gc-prune-gitdir` (`git gc` as the pruner) | Git, 9 of 9 | Ok, 2 attempts, 1 probe, 9 of 9 |
| `maint-prune-gitdir` (`git maintenance run --auto`) | 2.43.0, 2.50.1: Ok after 1 attempt (no prune ran); **2.55.0: Git, 3 of 3** | 2.43.0, 2.50.1: Ok after 1; 2.55.0: Ok, 2 attempts, 1 probe |
| `prune-own` (deleted before the takeover) | Ok after 2 attempts, no probe | Ok after 2 attempts, no probe |
| `double-prune` (a second prune on the second attempt) | Git after 1; policy `once`: **Git after 2** | Ok after 3 attempts and 2 probes, 9 of 9 |

**It happens without pauses.** `d7/witness/d7stress.py`: 2,000 engine-shaped adds, one after another, against four
`git worktree prune` loops (`d7/witness/stress-<version>-<policy>.json`).

| | 2.43.0 | 2.50.1 | 2.55.0 |
|---|---|---|---|
| r6: adds returned as Git | 4 | 16 | 33 |
| r7: adds returned as Git | 0 | 0 | 0 |
| r7: failures after a takeover, each probed and attempted again | 10 | 22 | 21 |
| prunes run beside r7's adds | 48,701 | 45,280 | 46,805 |

Every r7 add ended Ok; no probe failed; no add needed a second probe. Every failure r6 returned names the add's own
entry (`stress-<version>-r6.jsonl`): "could not open '.git/worktrees/<name>/gitdir' for writing: No such file or
directory" (3, 16 and 26), its `commondir` (2.55.0: 5), "not a git repository: …/worktrees/<name>" (2.43.0: 1) and
"could not find created worktree '<name>'" (2.55.0: 2).

**Why no retry rule closes it.** The two end states are the same, so a veto that reads only them must choose.
- Attempt every takeover failure again until the deadline: a checkout that cannot be made is refused (round 5's cost 3,
  FUB-D5-GENUINE), and it costs a checkout per attempt.
- Attempt a takeover failure again a bounded number of times, then return Git: one more prune than the bound returns Git.
  Executed with a bound of one: `double-prune` under policy `once` is Git after 2 attempts on all three versions, and a
  single prune already shows it works for one (`prune-gitdir` under `once`: Ok after 2).
- So the decision must rest on a fact the registry cannot change. §6.3 takes it from the checkout itself.

**Follow-up C's part.** C's design turns off the engine's own auto maintenance through its Git builder
(`maintenance.auto=false`, `gc.auto=0`; #330's scope; D's in the legacy builder). The user's and agents' commands keep
theirs, and 2.55.0 makes those a prune source on every commit. So this change handles a prune on the reader side
whatever started it.

### 6.3 The corrected add veto: the removal proof and the registry-free checkout probe

**The rule.** An add's failure is returned as Git state only when the add could not have succeeded whatever the registry
did.
- The destination is made by the access as an empty directory before its first attempt (as §5.3). An existing empty
  directory is used. A destination that is not an empty directory when the access begins is attempted once and returned
  (Git's "already exists", as §5.3). A destination that cannot be made is returned as Git state at once, naming the path
  and the OS error, without running Git (as §5.3).
- After a failed attempt, the veto reads the destination (`symlink_metadata`, and `read_dir` when it is a directory):
  1. **An empty directory the access can remove is untouched.** The access removes it and makes it again, and answers
     `Attempt`. No probe runs.
  2. **Absent, or an empty directory the access cannot remove: the probe decides.** The add's checkout is run without
     the registry, at the destination. If it cannot be made, the veto answers `Return`, and the attempt's own error is
     returned. If it can, the veto answers `Attempt`.
  3. **Anything else is undecidable**: a link, a reparse point, a file, a non-empty directory, or metadata the access
     cannot read. The veto answers `Undecidable`, and the access refuses at once (§6.4).
- When the veto answers `Attempt`, the destination is an empty directory again before the next attempt. If the access
  cannot make it again, the answer becomes `Undecidable`.
- A refused add removes the destination it made when that is still an empty directory (as §5.3).

**Why case 1 is safe to attempt again.** After the takeover, every failure path of Git's add runs
`remove_junk` (v2.43.0 `builtin/worktree.c:258-273`, v2.50.1 and v2.55.0 `:273-288`). It removes the entry, then the
destination, with `remove_dir_recursively(&sb, 0)`. For an empty destination that ends in the `rmdir` the access performs.
So a destination still there, empty, that the access can remove was never taken over. Git would have removed it.
*Corrected at implementation (FUB-D9-TAKEOVERWORD, P3, the decision appendix's §11): such a destination holds nothing to
lose, which is why another attempt is safe; it does not prove that Git never took it over, because a Git killed by
`SIGKILL` after the takeover, before it wrote anything there, leaves it empty too. That case is covered by the first
exception below.*
- **The exceptions, and where they land.**
  - Git killed by `SIGKILL` runs no junk removal. That failure is not the add's own either, and the next attempt meets
    its residue: attempted again, or refused at the deadline.
  - A removal that failed for Git and succeeds for the access a moment later: a sharing violation on Windows (R9′). The
    destination reads as untouched, the add is attempted again, and its next junk removal or its probe decides.
  - Neither returns Git state.
- **Executed: the INODE witnesses** (`MATRIX.txt` and `FIGURES.txt`, all three versions, three rounds each):

| Scenario | r6 | r7 |
|---|---|---|
| `inode-parent`: an existing empty destination under a parent that cannot be written, FUB-D5-GENUINE's tree (concurrency and regression lenses) | refused at 500.1 ms after 15 attempts; the destination is the same inode, empty | **Git after 1 attempt**: the removal is refused (`Permission denied`), and the probe fails "invalid path '.git/worktrees/fake-entry/file.txt'" |
| `inode-both`: the destination and its parent both 0555, a valid commit (design lens) | refused at 500.1 ms after 15 | **Git after 1**: the probe cannot write the destination |
| `torn-parent`: `inode-parent`'s destination and parent, the failure a torn foreign entry repaired before the second attempt | Ok after 2 | Ok after 2: the removal is refused, the probe succeeds, the add is attempted again |

- That last row is why the removal proof alone would not do. A destination that cannot be removed says nothing about
  where Git failed; the probe decides it, and a registry failure there is attempted again.

**The probe, exactly.**
- The command: the manager's `command` builder (`src/workspace_manager.rs:4994`), so every hook, the fsmonitor and
  replacement objects stay off. It names the repository explicitly, with `--git-dir=<common git dir>` (the canonical
  path the manager and the helper already hold) and `--work-tree=<destination>`:
  `read-tree -u --reset --no-recurse-submodules <commit>`, with `GIT_INDEX_FILE=<destination>/.git/index`.
  - `<destination>/.git` is a plain directory the probe makes first. Git refuses `.git` as a path component in any tree,
    so nothing the probe checks out can collide with its index.
  - If the probe cannot make `<destination>` or `<destination>/.git`, that is the add's own failure: Git's add has to
    write `<destination>/.git` too. The probe answers `Return`.
- **What it reads** (`d7/witness/probe-<version>.txt`, strace `-e trace=%file -f`, logs in `probe-<version>.strace/`):
  nothing under `<common git dir>/worktrees`, run from a base that is the main checkout or a linked one: 0 paths on
  2.43.0, 2.50.1 and 2.55.0.
  - The first form tried found the repository from the base (`-C <base> --work-tree=…`). From a linked base it read
    that checkout's own entry (33 paths on each version), and with that entry torn it failed (rc 128, "failed to read
    …/worktrees/linked-base/commondir"). `--git-dir` removes the dependency: the cited form returned rc 0 over the same
    torn entry. The witness keeps both forms side by side; the first run is `d7/witness/superseded-probe-1/`.
- **A registry Git cannot list does not change its answer** (the same files):

| The store | `git worktree list` | the probe |
|---|---|---|
| intact | rc 0 | rc 0 (from either base) |
| a foreign entry with an empty `commondir` | **rc 128**, "failed to read .git/worktrees/foreign/commondir: Success" | rc 0 (from either base) |
| the linked base's own entry with an empty `commondir` | (not run) | rc 0 from that base |
| mode 000 | rc 0 | rc 0 |
| absent | rc 0 | rc 0 |

- **It fails where the add's checkout fails** (the same files; the add and the probe at fresh destinations of one path
  length):

| commit | `git worktree add` | the probe |
|---|---|---|
| valid | rc 0 | rc 0 |
| FUB-D5-GENUINE's tree | rc 128, invalid path | rc 128, invalid path |
| round 5's 300-byte name | rc 128, unable to create file | rc 128, unable to create file |

- **What it costs.** One checkout, only after a failure the removal cannot prove untouched. A genuine failure is now one
  add and one probe (`genuine-*` rows: 1 attempt and 1 probe, 2.3 to 3.2 ms in the model on this box; `FIGURES.txt`).
  A prune hit is one probe and one more add.
- **Its fidelity, stated** (R12). The probe runs in the configuration of the common git dir's own checkout: the shared
  `config`, and that checkout's `config.worktree` and `info/sparse-checkout` when it has them. The add's checkout runs in
  the new worktree's, which `git worktree add` copies from the base (`copy_filtered_worktree_config`,
  `copy_sparse_checkout`). A difference makes a checkout failure the probe does not reproduce. That failure is attempted
  again and refuses at the deadline. It is never returned as Git state.

**A coordinator killed during the probe** leaves the slot populated with the probe's checkout and its `.git` directory,
unregistered, beside the slot's intent. The reclaim's forced removal must take it.
- **With a registration store** it does. The removal scan binds no registration to the target, because none names it
  (`src/workspace_manager.rs:5232`), and `remove_bound` removes whatever directory is at the target (`:3028`, `:3040`).
  Reasoned from the code at `5c222ff2`.
- **With no store at all** it would not. The scan refuses a target that is present when the store is absent
  (`:5144-5164`), on every attempt. The store can be absent here: the failed add's junk removal took its own entry, and an
  empty store is then removed by a prune or by this change's own removals (§3.5).
- Round 6 already made one exception there: an empty directory at the target binds nothing (§5.3, T17). **Round 7 widens
  it to any directory at the target that is not a linked checkout**, meaning it holds no `.git` file. That covers the
  made destination and the probe's leftovers (whose `.git` is a directory). A checkout with a `.git` file and no store
  keeps the I/O refusal.
- Its residue class is `None`, as round 6's empty slot's is: `add_state` asks for the slot's registration before it
  reads the slot's `.git` (`src/workspace_manager/residue.rs:524-532`).
- T17 gains the probe's residue (§6.7).

**Every other row of the matrix**, all three versions, three rounds each (`MATRIX.txt`):
- `genuine-gitpath`, `genuine-namemax`: Git after 1 attempt under r6 and r7 (r7 with 1 probe); policy `once`: Git after 2.
- `torn-static`: refused at the deadline under both, 15 attempts (r6) and 16 (r7, the final attempt of §6.4).
- `torn-transient`: Ok after 2 under both, no probe.
- `nonempty-entry`: Git after 1 under both ("already exists").
- `dest-unmakeable`: Git at once under both, no Git child run.
- `undecidable`: the first attempt fails on a torn entry and a file appears in the destination: r6 returned **Git**;
  r7 refuses at once.
- `bound-slow` and `final-attempt` are §6.4's.
- `prune-after-success` is R13 (§6.6).

**Windows** (reasoned; no Windows run this round, §6.10).
- std has no file identity there, and the rule no longer needs one: the removal proof is a removal.
- `std::fs::remove_dir` there is `RemoveDirectoryW`. A sharing violation makes it fail; the probe then decides.
- A destination removed while another process holds a handle stays delete-pending, and making it again fails (os error
  5). That answer is `Undecidable`: a refusal, never Git state.
- The probe's commands are the same. CI's Windows legs run T19 to T21 at implementation.

**Where it is.** Inside the add's funnel, in the veto closure the add passes to the helper, outside R-X.
- The probe and the destination steps are private methods outside the funnel's body, so
  `no_sampled_funnel_builds_its_argv_from_a_literal` (`src/workspace_manager/tests.rs:12544`) still finds one literal
  there. Round 6 measured that with its destination helper.
- The kill sampler samples the add's Git command alone (`sampled_command`, `src/workspace_manager/tests.rs:12870`), and
  the probe runs only after a failed attempt, so ST-07's add histogram does not move.

### 6.4 The helper's contract, changed (FUB-D6-DABSENCE, FUB-D6-BOUND, the final attempt, `CONTENDED_ATTEMPTS`)

§5.5 carries the dated note. This is the contract as it now stands. **Changed 2026-10-02, round 7.**

**The signature.** `again` returns a three-way answer. `RegistryHold` is unchanged.

```rust
/// What a registry access does after a failed attempt, as the caller's veto answers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Again {
    /// Attempt again, while the deadline allows.
    Attempt,
    /// Return this attempt's error, unchanged: the failure is the operation's own.
    Return,
    /// The caller could not decide: refuse now, as `RegistryRefused`, naming why.
    Undecidable { why: String },
}

pub(crate) fn tolerant_registry_access<T>(
    common_git_dir: &Path,
    hold: RegistryHold,
    again: &mut dyn FnMut() -> Again,
    attempt: &mut dyn FnMut() -> Result<T, UpstrokeError>,
) -> Result<T, UpstrokeError>
```

**When it attempts again** (replaces §5.5's seven steps):
1. It runs `attempt`, under R-X as `hold` says. R-X is released when the attempt returns. A wait for R-X that reaches the
   deadline refuses, and no attempt runs.
2. On `Ok`, it returns at once. `again` is never called before the first attempt or after a success.
3. On `Err`, it calls `again()` once, outside R-X.
4. On `Return`, it returns that `Err` exactly as the attempt returned it. This is the only way it returns
   `UpstrokeError::Git`.
5. **On `Undecidable { why }`, it refuses at once** as `RegistryRefused`. The message names the store, the attempt count,
   the last failure's text and `why`. No further attempt runs. (FUB-D6-DABSENCE: the safe outcome of a veto that cannot be
   evaluated is a refusal. A caller that keeps a captured candidate on `RegistryRefused`, as D's B-PRESERVE does, keeps it
   here too.)
6. On `Attempt`, it counts the answer in `CONTENDED_ATTEMPTS` (tests only). If the deadline has passed, it refuses.
   Otherwise it sleeps the backoff (1 ms, doubling, at most 50 ms, never past the deadline) and goes back to 1.
7. **The final attempt.** An attempt that follows a sleep the deadline cut short is made, and it is the last. If it fails
   and `again()` answers `Attempt`, the access refuses. So a store a writer leaves whole by the deadline is passed, which
   round 6 did not promise (FUD-D1-PROGRESS on #331, carried here by `briefs/followups/fu-b-impl-carryover.md`).

**The bound, end to end** (FUB-D6-BOUND). Let D be `REGISTRY_ACCESS_DEADLINE`, fixed when the call begins.
- Every wait for R-X and every backoff sleep ends by D.
- No attempt starts after D, except the final attempt, which starts at D.
- After the last attempt, `again()` runs once more.
- **So an access returns by D plus the runtime of its last attempt plus the runtime of the veto after it.** The helper
  bounds neither: an attempt is a Git command or a scan, and the veto is the caller's.
- **The topology add's veto** is a few metadata calls, one `rmdir` and `mkdir`, or one probe: a checkout as long as the
  add's own.
- Executed (`bound-slow`, `FIGURES.txt`): a required smudge filter that sleeps 0.6 s and fails. r6 returned Git at 602.2
  to 603.0 ms, which is one attempt. r7 returned Git at 1,205.0 to 1,205.6 ms, which is one attempt and one probe, both
  past the 500 ms deadline.
- **D's legacy veto** reads its destination's metadata. Its runtime counts the same way.

**The final attempt, executed** (`final-attempt`, `MATRIX.txt`). A static torn entry is repaired 490 ms after the access
began. Round 6's loop refused at 500.1 to 500.2 ms after 15 attempts on every run. Round 7's made a 16th attempt at the
deadline and returned Ok at 501.6 to 502.9 ms, on all three versions, three rounds each (`FIGURES.txt`).
- With the final attempt, an always-failing access makes 16 attempts under the 500 ms test deadline, where round 6's
  witnesses ran 14 or 15 (`torn-static`: 16 under r7).

**`CONTENDED_ATTEMPTS`, the test handshake D requires** (D's record §1.2 item 8; accepted by #331's design review round
1). It is now part of the contract, for tests only:

```rust
/// How many times an access has decided to attempt again, per common git dir exactly as the caller passed it.
#[cfg(test)]
pub(crate) static CONTENDED_ATTEMPTS: std::sync::Mutex<std::collections::BTreeMap<PathBuf, usize>> =
    std::sync::Mutex::new(std::collections::BTreeMap::new());

/// The count for one common git dir; 0 before any.
#[cfg(test)]
pub(crate) fn contended_attempts(common_git_dir: &Path) -> usize
```

- Incremented once at step 6, each time `again()` answers `Attempt`, before the deadline check and the sleep, as rounds 4
  and 6's prototypes did. Never reset or decremented.
- Keyed by `common_git_dir` exactly as passed, so a test reads it with the same canonical path its access used.
- **Placement.** Both items sit after `src/workspace_manager.rs`'s `#[cfg(test)] mod tests;`, so that file's first
  `#[cfg(test)]` stays a module (`every_production_region_that_stops_early_stops_at_a_module`). The production region
  carries only an empty `#[cfg(not(test))] fn note_contended(_: &Path) {}`. Rounds 4 and 6's prototypes had exactly this
  shape, and their censuses passed (§3.6, §5.8; *corrected from "§3.8" by §7, 2026-10-02*).
- A test that must finish a tear only after the access has failed once waits, with a watchdog, for
  `contended_attempts(dir)` to exceed its value before the access began. Keyed per repository, two such tests in one
  suite do not overwrite each other (D's `fud/probe/SUITES.txt`).

**For D's legacy add** (the dated note in §5.5 points here).
- D's veto at `37e4d8c4` is "owned, unchanged (dev+ino), empty". That is §5.3's inference, and it has both of round 6's
  holes. A prune after the takeover returns Git state, and D's coordinator then discards paid output. A takeover whose
  junk removal failed reads as untouched.
- D's add adopts §6.3's rule, built with `src/workspace.rs`'s own Git builder. The probe argv and environment are as
  above; the destination is `PendingGateWorkspace`'s; the common git dir is D's canonical one (`canonical_common_dir`,
  D's record §1.3).
- D's other two accesses pass `|| Again::Attempt`.
- **What does not change for D:** the name and module of the helper, its `effect_free` row, `RegistryHold`, the
  canonical `common_git_dir`, nothing sampled, and `RegistryRefused` as the variant D's B-PRESERVE keys on.

### 6.5 FUB-D6-STATICRESUME: the dead run's own resume over a static tear, and what the operator does

§5.6's R1′ said "the dead run's own resume repairs its residue". That is withdrawn for one class of tear, and qualified.

**What the resume does.**
- A resume runs in a new process and derives its manager first (`WorkspaceManager::derive`,
  `src/workspace_manager.rs:1616`, whose `manager.revalidate()?` lists the store). It does that before the run lock and
  before any event.
- A registration `git worktree list` dies on makes every attempt of that list fail. An example is the empty `commondir` a
  killed add leaves, which is `PR5-RD-002`'s shape. So `derive` refuses as `RegistryRefused` after the deadline (10 s),
  where master returns Git state at once.
- Nothing is written. Every resume of the run refuses the same way until the registration is repaired.
- `remove_intent`'s and `verify_worktree`'s repair are never reached.
- That limitation is filed: `PR5-RD-002-RESUME-DERIVES-THROUGH-A-TORN-ENUMERATION`
  (`findings/P2_crash-consistency_202609191407_a-resume-derives-its-manager-through-a-torn-enumeration.md`, P2,
  `deferred`). This change retains it and does not fix it. It changes only the refusal's variant and its delay.
- At `5c222ff2` no shipped code derives a manager: every caller of `WorkspaceManager::derive` is under `#[cfg(test)]`
  (`src/engine/topology/scaffold.rs`, declared `#[cfg(test)] mod scaffold;` in `src/engine/topology.rs`;
  `d7/census/derive-callers-5c222ff2.txt`). The finding's guard fires with the first production caller.
- Tears the list does not die on are unaffected: the resume derives its manager and goes on as §5.6's R1′ describes. One
  example is an entry whose `gitdir` cannot be read, which the list skips (`get_linked_worktree`'s "invalid gitdir file"
  branch, v2.43.0 `worktree.c:88`, v2.50.1 `:133`, v2.55.0 `:153`; `d7/git-src/list-skips-no-gitdir.txt`).

**What the operator does** (executed on 2.43.0, 2.50.1 and 2.55.0, `d7/witness/static-<version>.txt`). The residue is a
killed add's entry: `locked` ("initializing"), `gitdir`, `HEAD` and a zero-length `commondir`, plus the slot's checkout.

| Command | Result on all three versions |
|---|---|
| `git worktree list --porcelain -z` | rc 128, "failed to read .git/worktrees/k0-g0/commondir: Success" |
| `git worktree prune` | rc 0, and the entry stays (it holds `locked`) |
| `git worktree remove --force --force <slot>` | rc 128, the same message |
| `git worktree unlock <slot>` | rc 128, the same message |
| `git worktree repair` | rc 128 (2.50.1 and 2.55.0 name the file "No such file or directory") |
| `git worktree add` of another slot | rc 128, the same message |
| **remove `<common git dir>/worktrees/<name>` and the slot's checkout, then `git worktree list`** | **rc 0** |
| then `git worktree add` of the same slot | rc 0 |

- The refusal's message names Git's last failure, and that names the file. The operator first checks that no Git process
  is still writing that registration. A dead coordinator's Git child may outlive it on Unix, which is follow-up C's
  `PR329-A-RESUME-REBINDS-A-SLOT-ITS-DEAD-COORDINATORS-GIT-CHILD-STILL-WRITES`.
- Then the operator removes the named registration directory and the slot checkout it names, and resumes.
- `DESIGN.md` §15's PROPOSED paragraph now says so. The finding's grading noted that no document described a manual
  recovery. That sentence changes when the paragraph is in force, which is at implementation, with the finding's text.

### 6.6 What is closed, what remains, and what G6 meets

**The claims, once implemented** (replacing §5.6's 1, 4 and 5; 2, 3 and 6 stand):
1. **No manager registry access returns `UpstrokeError::Git` for anything the registry's state caused.** That covers
   contention, a write a dead process left torn, a registration nobody is writing, and **a prune's deletion before or
   after an add's takeover**. An add returns Git state only when:
   - its commit's checkout cannot be made at its destination, which a checkout that reads no registry shows;
   - its destination cannot be made;
   - or its destination was not empty when the access began.
4. **The add's own failure is returned after one attempt and one probe.** That is the checkout, its destination, or
   either one's files. `not_repairs` still defers or parks on it, including when junk removal could not remove the
   destination (FUB-D6-INODE).
5. **Each access returns by its deadline plus its last attempt's runtime plus its veto's** (§6.4).
7. **A veto that cannot decide refuses.** It never returns Git state, and it never attempts again (§6.4).

**What remains.** These rows replace §5.6's R1′, R2, R3, R8 and R9 and add R12 and R13. R4 to R6, R10 and R11 stand.

| | What | Consequence now | Finding |
|---|---|---|---|
| R1″ | A registration that stays torn until the deadline | the access refuses resumably as `RegistryRefused`. A tear the list does not die on stays the dead run's own resume's to repair, as §5.6's R1′ said. A tear the list dies on makes that resume's `derive` refuse first, and the operator removes it (§6.5) | `PR5-RD-002-RESUME-DERIVES-THROUGH-A-TORN-ENUMERATION` (P2, retained); `PR308-R3-…` (consequence narrowed) |
| R2′ | A host agent's own Git, its prune and its auto maintenance included | its torn entries are attempted past. A prune of an engine add's entry, before or after the takeover, costs another attempt, plus one probe after it, and is never returned as Git state (§6.2) | `PR11-HOST-AGENT-PRUNE-RACES-AN-ENGINE-ADD`: its "the engine's add fails" branch is repaired by §6.3 once implemented; its "finishes over a registration that is gone" branch is R13 |
| R3′ | The user's Git in any checkout, its `git worktree prune`, `git gc` and auto maintenance included (on 2.55.0 every commit, fetch or merge) | as R2′ | none |
| R8′ | The access's runtime past the deadline | its last attempt's runtime and its veto's, neither bounded by the helper (§6.4) | stated |
| R9′ | Windows: no file identity in std; delete-pending directories; handles held on files | the removal proof needs no identity. A junk removal that failed transiently while the access's removal then succeeds reads as untouched and is attempted again. A destination Windows will not make again at once is `Undecidable`: a refusal. A junk removal that left files (a handle held on one) leaves a non-empty destination: `Undecidable`, a refusal, which the resume's reclaim clears. Reasoned, not executed (§6.10) | stated |
| R12 | The probe's fidelity: it runs the checkout in the configuration of the common git dir's own checkout, and the add's checkout runs in the new worktree's, copied from the base | a checkout failure the probe does not reproduce is attempted again and refuses at the deadline, never returned as Git state | stated |
| R13 | A prune that decided in an add's window and deletes after the access returned Ok: Git's own decide-then-delete gap, which every `git worktree add` has | executed (`prune-after-success`, all three versions): the access returned Ok, the registration was gone, and the checkout kept its `.git`. The next Git command in that checkout meets a checkout with no registration. No access can classify a deletion that lands after it returned | `PR11-HOST-AGENT-PRUNE-RACES-AN-ENGINE-ADD` for an agent's prune; stated for the user's |

**The cases.** As §5.6, with (a) and (b)'s evidence updated:

| Case | Closed by | Severity | Applies to G6 | Blocks G6 |
|---|---|---|---|---|
| (a) An attempt's pipeline error | §5.3, §5.4 and §6.3, in the non-frozen `src/workspace_manager.rs` and `src/error.rs` | P1 | yes: R17, the shared registry, Q6 | until implemented and validated |
| (b) A durable verification deferral or park | the same. The prune class is executed at the Git level on three versions, interleaved and unpaused (§6.2) | P1 | yes: Q6 and durable verification | until implemented and validated |
| (c) DESC | filed, follow-up C | P1 | yes: Q1, INV-22, ST-16, ST-18 | yes; filing is no waiver |
| (d) A legacy writer tears a topology reader | §5.3 | P1 class | yes | until implemented and validated |
| (e1), (e1′) Legacy against legacy | follow-up D | P1 | no | no; it remains a P1 until D lands |
| (e2) A topology writer tears a legacy reader | **follow-up D**, through §5.5 as dated, with §6.3's add veto | P1 | yes: Q6, across the shared registry and R17 | **yes, until follow-up D is implemented and validated** |
| (e2′) A topology writer's static or deadline residue, then a legacy discard | **follow-up D** | P1 | yes: Q6; a crash producer engages Q1 | **yes, until follow-up D is implemented and validated, unless the owner rules otherwise** |

The findings table of §5.6 stands.

### 6.7 Regression tests

§5.7's tests stand, with these revised and added. Each test is the implementation's, outside the frozen modules and their
test children. Each waits on a handshake or a seam, and uses time only as a watchdog.

**Revised:**
- **T4, the contract** (adds to §5.7's four witnesses):
  - a veto answering `Undecidable` refuses at once, after one attempt, its message naming `why`;
  - a failure repaired after the last attempt before the deadline is passed by the final attempt, as `final-attempt`
    showed;
  - `contended_attempts` counts exactly the `Attempt` answers;
  - a veto that blocks past the deadline is followed by no attempt, and the access returns after it.
  - Mutation m7: `Undecidable` treated as `Return`. T4 is red: Git instead of `RegistryRefused`.
  - Mutation m8: no final attempt. The final-attempt test is red.
- **T15, `not_repairs` through the add** (adds to §5.7's): FUB-D6-INODE's two shapes (`inode-parent`, `inode-both`;
  Unix) return Git after 1 attempt and 1 probe. A verification over the first terminates `merge_verification_unavailable`
  (Deferred), and the run completes. The regression lens's engine witness `d6_reg_verification_takeover_cleanup_failure`
  (`d7/review-witnesses/pr329-d6-reg-engine-9d3vk6yg/witness-test.rs`) is that test's shape.
  - Mutation m9: the probe skipped, so a takeover failure answers `Attempt`. Red: refused at the deadline.
  - Mutation m10: the removal proof skipped, so an empty destination is untouched (round 6's rule). Red: the INODE shapes
    refuse.
- **T16, the destination's lifecycle** (adds to §5.7's): after an untouched failure the destination is a new empty
  directory, removed and made again; a refused add leaves no destination it made.
- **T17, the store-absent slot** (adds to §5.7's): an intent, a slot holding a probe's leftovers (files and a `.git`
  directory) and no registration store, then the forced removal: Ok, and the slot is gone. A slot holding a `.git` file
  and no store still refuses.
  - Mutation m6 (as §5.7, widened): the branch removed. Red.

**New:**
- **T19, a registration that vanishes after the takeover.**
  - A prune needs a pause inside Git, which is executed at the Git level (`d7/witness/`, §6.2) and not in the Rust suite.
    The suite reproduces the end state it leaves: a failure after the takeover that a second checkout does not meet.
  - A required smudge filter that fails on its first run only fails the first attempt's checkout after the takeover. The
    probe succeeds, and the add returns Ok after 2 attempts and 1 probe. That is the decision a prune's deletion reaches.
  - Mutation m11: the probe's answer ignored, so a takeover failure answers `Return` (round 6's rule). Red: Git after 1
    attempt.
- **T20, the INODE control** (Unix): `torn-parent`. The parent cannot be written and a torn foreign entry is repaired
  before the second attempt. Ok after 2 attempts and 1 probe.
- **T21, the probe reads no registry.** The probe answers `Attempt` with a foreign entry whose `commondir` is empty,
  which `git worktree list` dies on, and with no store at all; from the main checkout and from a linked one.
- **T22, a static tear at `derive`.** `WorkspaceManager::derive` over a registration the list dies on is
  `RegistryRefused` after the test deadline, with nothing written. After §6.5's remedy, `derive` succeeds.

**The proof the implementer owes:** §3.8's, with mutations m7 to m11 added.

### 6.8 Effect governance, instruments and the frozen set

**What the implementation moves, against §5.8:**
- **Code:** `src/workspace_manager.rs` gains `Again`, the probe and the destination steps as private methods, and the
  test handshake. The removal scan's store-absent exception is widened (§6.3). `src/error.rs` is unchanged against §5.8.
- **Instruments: no row beyond §5.8's one.**
  - `Again` is a type, which the classification census does not classify.
  - The probe and the destination steps are private, and only externally reachable functions are classified.
  - They run inside the add's existing funnel, through the manager's one `command` builder, as every manager Git child
    does (`effects/allowlist.toml`'s `src/workspace_manager.rs` entry: "Every effect is issued inside a `funnel` call").
  - That is reasoned from the census's rules. Round 7 built no prototype, and the implementation measures it as round 6
    measured the helper's row.
- **The frozen set:** unchanged. No frozen module or frozen test child.
- **Docs:** as §5.8, with `DESIGN.md` §15's paragraph now round 7's.
- **Findings, at implementation:**
  - `PR11-HOST-AGENT-PRUNE-RACES-AN-ENGINE-ADD`'s failing-add branch is repaired, and R13 remains.
  - `PR5-RD-002-RESUME-DERIVES-THROUGH-A-TORN-ENUMERATION`'s `derive` refusal becomes `RegistryRefused` after the
    deadline.
- **On this branch at this head:** the legacy finding's B1′ bullet now names §6.3 as the add veto D adopts.

### 6.9 Design review round 6, answered

| Finding | Sev | Kind | Round 7 | Where | Evidence |
|---|---|---|---|---|---|
| FUB-D6-PRUNE | P1 | executed (concurrency, design) | **Fixed (design), witnessed.** A takeover failure is decided by the registry-free probe; a prune's failure is attempted again, never Git. Checked against Git's prune rules, gc and maintenance on 2.43.0, 2.50.1, 2.55.0 | §6.2, §6.3 | `MATRIX.txt` (prune ×4, gc, maint, double-prune), `stress-*.json`, `prune-rules-citations.txt`; planned T19, T21 |
| FUB-D6-INODE | P2 | executed (all three) | **Fixed (design), witnessed.** Untouched means an empty destination the access can remove; otherwise the probe decides | §6.3 | `inode-parent`, `inode-both`, `torn-parent`; planned T15, T20 |
| FUB-D6-DABSENCE | P2 | reasoned (design) | **Fixed (contract, dated in §5.5).** `Again::Undecidable` refuses at once | §6.4 | T4 (planned); the model's `undecidable` row |
| FUB-D6-STATICRESUME | P2 | reasoned (regression) | **Answered.** R1′ qualified. The resume's `derive` refuses after the deadline; the filed P2 is retained; the operator's remedy is executed and written into §15's paragraph | §6.5 | `static-*.txt`; planned T22 |
| FUB-D6-BOUND | P2 | reasoned (regression) | **Fixed (contract, dated).** D plus the last attempt's runtime plus the veto's | §6.4 | `bound-slow` |
| FUB-D6-PLATFORM | P3 | reasoned (regression) | **Fixed.** The claim names what ran where | §6.10 | `d6/witness/git-level-vi-2.50.1-windows.log` |

Carried in with them: the final attempt (FUD-D1-PROGRESS, via `briefs/followups/fu-b-impl-carryover.md`). Executed as
`final-attempt`.

### 6.10 The platform evidence, exactly (FUB-D6-PLATFORM)

Round 6's sentence in §0 and §5.1 said every reviewer witness of rounds 3 to 5 was executed on 2.43.0, 2.55.0 and
2.50.1 (Windows). That is corrected (§5.1 carries the mark):
- **Linux, the system Git 2.43.0 and a private 2.55.0 build:** every row of round 6's Git-level kit, both policies. That
  covers spelling (ordinary and symlinked `.git`), own-entry (two windows and every attempt), OPTFILE, GENUINE's tree, the
  300-byte name, PERM, PERM under noise, TORNOK static and transient, the quiet-parse control, a destination whose parent
  cannot be written, the four takeover probes and `prune-own` (`d6/witness/git-level-vi-2.43.0-r2.log`,
  `git-level-vi-2.55.0-r1.log`).
- **The Windows guest, Git 2.50.1.windows.1:** spelling (ordinary `.git` only), own-entry (both), GENUINE's tree, TORNOK
  static and the quiet-parse control, both policies, and the noise and scan-miss measurements
  (`d6/witness/git-level-vi-2.50.1-windows.log`).
  - Not there: the symlinked spelling, OPTFILE, the 300-byte name, PERM, TORNOK transient, the destination's parent, the
    takeover probes and `prune-own`. The shim needs LD_PRELOAD, and the guest has none.
- **Round 7:** Linux only, on upstream 2.43.0, 2.50.1 and 2.55.0 built from kernel.org's tarballs. Its Windows statements
  (§6.3, R9′) are reasoned from std's and Git's sources. macOS is reasoned for both rounds.

### 6.11 Risks, sequencing, and what is out of scope

**Risks**, beyond §5.10's:
- **A second checkout after an unexplained add failure** (§6.3): genuine failures and prune hits cost one probe. A large
  repository's or a slow filter's checkout runs twice, and the bound counts it (R8′). A store fault after the takeover
  (for example the store's filesystem full while the slot's is not) costs one probe per attempt until the deadline, and
  then refuses, as R10 says of store faults.
- **The probe writes into the destination** with an index under `<destination>/.git`, and removes both. A coordinator
  killed during the probe leaves a populated, unregistered slot with its intent. The reclaim's forced removal takes it:
  as at master when a store exists, and through round 6's store-absent exception, widened (§6.3), when none does.
- **The probe is one more Git child that writes into a slot.** A coordinator killed while it runs leaves it running on
  Unix. That is DESC's class (follow-up C, `PR329-A-RESUME-REBINDS-A-SLOT-ITS-DEAD-COORDINATORS-GIT-CHILD-STILL-WRITES`).
  It writes only under the slot's path, and the manager's one `command` builder starts it, so C's closure has to cover it
  as it covers the add's own checkout.
- **A prune after the access returned** (R13) is not closed by any access.
- **Windows is reasoned** (R9′).

**Sequencing:** as §5.10. D's implementation follows this change's merge, and D's design is re-checked against §5.5's
dated note (§6.4).

**Out of scope, and said so:** as §5.10, and `PR5-RD-002-RESUME-DERIVES-THROUGH-A-TORN-ENUMERATION` (retained, §6.5).

## 7. Round 8: the narrowing

> **Superseded by §8 (design round 9) only where §8's banner says:** §7.2's bullet for a destination that is not an
> empty directory at the start; §7.4's two faces and its candidate closures 1, 2 and 4; §7.7's claim 1, its R2″, R3″
> and R14 rows and its G6 table; §7.8's T15 and T16; §7.11's paragraph on the owner. Everything else in §7 stands.
> What follows is round 8's text, unchanged.
>
> **PROPOSED — for design review round 8.** This section supersedes §6 only where it says so, item by item.
>
> **Withdrawn:**
> - §6.3's registry-free checkout probe: its case 2, "The probe, exactly", "Its fidelity, stated" (R12) and "A
>   coordinator killed during the probe";
> - the widening of the removal scan's store-absent exception;
> - §6.1's convergence argument;
> - §6.4's "For D's legacy add" paragraph;
> - §6.7's T19, T21, the probe residue in T17, and mutations m9 and m11;
> - §6.8's and §6.11's probe paragraphs.
>
> **Replaced:**
> - §6.3's rule by §7.2;
> - §6.6's claims 1 and 4 and rows R2′, R3′, R8′, R9′, R11, R12 and R13 by §7.7;
> - §6.7's T15, T16, T17 and T20 by §7.8.
>
> **Also:**
> - **Restored:** round 6's store-absent exception (§5.3): an empty directory at the target binds nothing.
> - **Changed with a date:** §5.5 carries a second note, round 8's, and §7.6 is the current text.
> - **Corrected:** §6.4's "§3.8" reads "§3.6" there.
>
> **Stands:**
> - §6.2: Git's prune rules, and why no retry rule closes the prune. §7.3 says what round 8 answers instead.
> - §6.3's removal proof ("Why case 1 is safe to attempt again", so headed since FUB-D9-TAKEOVERWORD), and its INODE witnesses' constructions.
> - §6.4's contract, with the change above.
> - §6.5, §6.9 and §6.10.
>
> **Nothing in §7 is in force until the implementation lands.** #329 itself needs no owner decision. Round 8 narrows
> what #329 claims about an external prune. That narrowing, and the external-prune class's scope, go into the one
> consolidated owner question (§7.11).

Design round 8 is `pr11_fub_design8`'s (`claude-opus-5-5`, `max`), spawned on `85f5b09b` to carry out the orchestrator's
decision on design review round 7 (`~/orch-pr11/reviews/review-329-d7-triage.md`). Its figures are under
`~/orch-pr11/logs/pr11_fub_design8/`, cited as `d8/…`; `d8/SUMMARY.txt` is the index.
- **No production code and no prototype.** The evidence is Git source and Git-level witnesses, on round 7's upstream
  builds of 2.43.0, 2.50.1 and 2.55.0, round 7's pause shim and round 7's model, both imported unchanged (hashes in
  `d8/witness/SCRIPTS-SHA256SUMS`).
- **Master is `5c222ff2`.** The branch is not rebased, and §7 cites `5c222ff2` throughout.

### 7.1 What design review round 7 found, the looping signal, and the narrowing

**The findings** (triage, 2026-10-02T12:34Z). Three `gpt-6-astra` lenses at `max` ran on `85f5b09b`: design,
concurrency and regression, in round 6's recast conformance-reading form, with no refusals. All three returned
CHANGES_REQUIRED.

| id | sev | evidence | what |
|---|---|---|---|
| FUB-D7-SPLITINDEX | P1 | executed, all three lenses, on 2.43.0, 2.50.1 and 2.55.0 | The probe corrupts the main checkout's index. With `--git-dir=<common git dir>` and `GIT_INDEX_FILE` in the slot, Git writes `sharedindex.*` into the common git dir and expires one the main checkout still references (`core.splitIndex=true`). The user's `git status` then exits 128. |
| FUB-D7-CONFIG | P1 | executed, all three lenses, on the three versions | The probe's configuration differs from the add's: sparse checkout, worktree-specific filters, `core.protectNTFS` under `extensions.worktreeConfig`. The probe fails where the add would succeed, so a prune's failure returns as Git after one attempt and one probe. |
| FUB-D7-R13 | P1 | Git prefix executed; engine consequences reasoned, all three lenses | R13, the deletion after the add returned, is an applicable high the G6 table omitted. It reaches a durable deferral or park, `MergeRejected` for a valid candidate, recovery removing unpinned edits, and a finalization that never converges. The user's starter, R3′, is unfiled. |
| FUB-D7-ENVCENSUS | P2 | reasoned, design | The probe's `.env(GIT_INDEX_FILE)` is a ninth environment expression; `every_production_command_spec_payload_is_classified` (`src/runner/contract.rs:2422`) pins eight. |
| FUB-D7-R9WIN | P2 | reasoned, regression | R9′ overstates Windows recovery: with the store gone, a destination holding a `.git` file stays refused after the handle closes. |
| (note) | — | reasoned, design and regression | The widened store-absent exception admits any contained directory without a `.git` file, not only probe residue, and `design/15` describes only the empty case. |

**Agreed by all three lenses:**
- §6.4's contract changes hold as specifications: the three-way answer, `Undecidable` refusing, the final attempt, the
  runtime allowance and `CONTENDED_ATTEMPTS`.
- D's `37e4d8c4` veto has both of round 6's holes.
- The removal proof handles the INODE shapes, Windows transients aside.
- The probe reads no registry, for the tested configurations.
- Adopting §6.3 is not sound for D.
- §6.1's convergence argument fails.

**The looping signal (MAINTAINING, "When a pull request may be looping"), a seventh time and in its strongest form.**
Round 7's new machinery, the probe, carries two executed P1s of its own. MAINTAINING's remedy is "the smaller change
is the one to propose: keep what has survived a pass, drop the machinery those rounds invented", and the
orchestrator's decision applies it.
- **What survived a pass** (all three lenses of round 7):
  - the retry-to-deadline helper;
  - the three-way contract, with `Undecidable` refusing;
  - the final attempt;
  - the runtime allowance;
  - `CONTENDED_ATTEMPTS`;
  - the removal proof.
- **What goes:** the probe, and the widened exception made for its residue.

**Why the narrowing converges.**
- Round 8 adds nothing that a pass has not read. Its one decision (§7.3) chooses between two answers the reviewed
  contract already has, `Return` and `Undecidable`, and each is executed for every construction the reviews built.
- What no access can close without new machinery is the external-prune class. It is filed as one P1 with its candidate
  closures (§7.4) and stated to block G6 until it is closed or ruled on (§7.7). Nothing is claimed for it that the
  design does not do.

### 7.2 The add's veto as round 8 specifies it: the removal proof, and no probe

**Before the first attempt** (as §5.3 and §6.3):
- the access makes its destination as an empty directory, and uses an existing empty one;
- a destination that cannot be made is returned as Git state at once, naming the path and the OS error, with no Git
  run;
- a destination that is not an empty directory when the access begins is attempted once, and Git's "already exists"
  is returned.

These are the only Git states an add returns. Neither depends on the registry.

**After a failed attempt**, the veto reads the destination (`symlink_metadata`, and `read_dir` when it is a directory):
1. **An empty directory the access can remove is untouched.** The access removes it and makes it again, and answers
   `Attempt`. If it cannot make it again, it answers `Undecidable`. This is the removal proof of §6.3, unchanged.
2. **Everything else answers `Undecidable`,** and the access refuses at once as `RegistryRefused`: a resumable end,
   never Git state, and no further attempt. That covers:
   - a destination that is absent;
   - an empty directory the access cannot remove;
   - a link, a reparse point, a file or a non-empty directory;
   - metadata the access cannot read.

   §7.3 decides the first two.
3. A refused add removes the destination it made when that is still an empty directory, as §5.3 says.

**What the veto does.**
- **It runs no Git command.** It reads and writes nothing but the destination.
- **SPLITINDEX goes with the probe.** The add's only Git command is Git's own `git worktree add`. Its checkout writes
  the new worktree's index under that worktree's own entry, and nothing points another index at the common git dir.
  - **Executed** with design review round 7's split-index construction on the three versions: staged work in the main
    checkout, its shared index aged 30 days.
    - From a linked base, 27 of 27 runs (`d8/witness/reviewers-r8-summary.txt`): under round 8 with the prune
      interleaving, under option (i), and with a quiet add.
    - From the main checkout itself, 18 of 18 (`d8/witness/splitmain-r8.txt`): quiet, and with the prune.
  - In every run the main checkout's `git status` exited 0, its index bytes were unchanged, and no shared index was
    removed from or added to the common git dir.
- **CONFIG goes with the probe.** There is no second checkout whose configuration could differ.
  - The three CONFIG constructions (a sparse linked base, worktree-specific filters, `core.protectNTFS` under
    `extensions.worktreeConfig`) were run with the prune interleaving under round 8: `RegistryRefused` after one
    attempt in 27 of 27 runs, never Git (`d8/witness/reviewers-r8-summary.txt`).
  - Under option (i) the same runs gave Git in 27 of 27.
  - Each construction's control add and the direct retry after it exited 0. A quiet add under round 8 succeeded after
    one attempt in 27 of 27 (the same file).

**The store-absent exception is round 6's again.** An empty directory at the target binds nothing (§5.3, T17), and
anything else there with no registration store keeps the I/O refusal (`src/workspace_manager.rs:5144-5164`).
- With no probe, a coordinator killed inside an add leaves round 6's residues: an empty slot, which T17 covers, or Git's
  own add residue, which master's residue classes cover.
- `design/15`'s paragraph describes exactly this exception.
- The design lens's note on the widened predicate is answered by its withdrawal.

**Where it is.** Inside the add's funnel, in the veto closure the add passes to the helper, outside R-X.
- The destination steps are private methods outside the funnel's body. Round 6 measured that shape with
  `no_sampled_funnel_builds_its_argv_from_a_literal` (§5.3).
- The kill sampler's add histogram does not move, because no Git child is added.

**Windows** (reasoned, as §6.3; not executed). The removal proof is `RemoveDirectoryW`.
- A sharing violation makes the removal fail, so the answer is `Undecidable`: a refusal, never Git state.
- A delete-pending destination that cannot be made again is `Undecidable` too.
- R9″ (§7.7) states what the reclaim then does.

### 7.3 The post-takeover failure, decided: `Undecidable`

**The question.** After a failed attempt, the add's destination may be absent, or an empty directory the access cannot
remove. Git took it over, or may have. Git's junk removal runs on every failure after the takeover and removes the
destination, or fails to (§6.3).
- **(i) `Return`:** the attempt's error is Git state, as round 6 answered.
- **(ii) `Undecidable`:** the access refuses, never Git. The contract that survived round 7 refuses an `Undecidable` at
  once (§6.4, step 5).
- **The triage's wording admits a third reading,** "refuse at the deadline". That is (ii′): answer `Attempt`, and the
  failure is attempted again until the deadline and then refused. It was measured too.

**Which failures after the takeover exist.** These are Git's steps after the destination is taken over (§5.3's table):
the entry's `gitdir`, the destination's `.git`, `HEAD`, `commondir`, the HEAD update, and the checkout. A failure among
them is one of these, each executed as a construction (`d8/witness/d8scenarios.py`):

| Class | Cause | Construction (all three versions, three rounds) |
|---|---|---|
| G1 | a tree this checkout cannot hold | FUB-D5-GENUINE's `.git/…` path (`genuine-gitpath`); a 300-byte name (`genuine-namemax`); `git~1/payload` under `core.protectNTFS` (`genuine-protectntfs`); on Windows, a path over its limits (reasoned) |
| G2 | a filter the repository requires fails | at once (`genuine-filter`); after 0.6 s (`bound-slow`); after writing 4,000 files (`genuine-large-filter`) |
| G3 | the destination or its filesystem refuses a write | a destination, or its parent, that cannot be written (`inode-both`, `inode-parent`); a full filesystem or an I/O error (reasoned: no root for a small filesystem) |
| G4 | an object the commit needs is missing | a tree naming an absent blob (`genuine-missing-blob`), as in a partial clone with lazy fetch off |
| R | the registry: a prune deleted the add's entry after the takeover | `git worktree prune` at `gitdir`, `commondir`, `HEAD.lock` and `index.lock`; `git gc`; `git maintenance run --auto` on 2.55.0; two prunes on two attempts |
| R′ | the registry, before the takeover, with a destination the access cannot remove | a torn foreign entry and a parent that cannot be written (`torn-parent`) |

**What each answer does** (`d8/witness/FIGURES.txt`, 9 runs per cell over the three versions; deadline 500 ms):

| Construction | (i) `Return` | (ii) `Undecidable` | (ii′) `Attempt` |
|---|---|---|---|
| R: a prune at any post-takeover point, by `prune` or `gc` | Git after 1 | refused after 1 | Ok after 2 |
| R: maintenance (2.55.0; on 2.43.0 and 2.50.1 it ran no prune) | Git after 1 | refused after 1 | Ok after 2 |
| R: two prunes on two attempts | Git after 1 | refused after 1 | Ok after 3 |
| R′: `torn-parent` | **Git after 1: registry state returned as Git** | refused after 1 | Ok after 2 |
| G1, G2 (at once), G3, G4 | Git after 1 | refused after 1 | refused at the deadline after 15 or 16 |
| G2 after 4,000 files (one attempt 63.2–80.7 ms) | Git after 1 | refused after 1 | refused after 6 or 7, at 503.9–579.9 ms |
| G2 after 0.6 s | Git at 602.5–603.6 ms | refused at 602.3–603.5 ms | refused at 602.8–603.5 ms, 1 attempt |

- **The untouched failures are the same under all three answers.** A prune before the takeover (`prune-own`) and a
  tear repaired before the second attempt are Ok after 2. A static tear is refused after 16 attempts. A tear repaired
  at 490 ms is passed by the final attempt.
- **Unpaused** (`d8/witness/stress-*.json`): 2,000 adds against four `git worktree prune` loops on each version.
  - (i) returned Git 16, 7 and 7 times (2.43.0, 2.50.1, 2.55.0).
  - (ii) refused 15, 5 and 4 times and returned Git none.
  - (ii′) completed all 2,000 adds each time, attempting 10, 11 and 5 takeover failures again.

**What each costs in the engine** (reasoned from the code at `5c222ff2`).
- A Git error from a verification's judgement is `Verified::Unavailable` (`src/engine/topology/run.rs:279`), and the
  frozen `integrate.rs` then defers or parks the candidate (`src/engine/topology/integrate.rs:871`).
- Any other error is passed on (`src/engine/topology/run.rs:289`). The coordinator's fail path ends the command
  resumably, and the verification is settled interrupted, the candidate re-verifying under a new sequence
  (`src/engine/topology/closure.rs:99`, `src/engine/topology/recover.rs:1132`).
- An attempt's error, from its slot add or its judgement's snapshot add, ends the command resumably whatever the
  variant. So does integration's staging add (`src/engine/topology/integrate.rs:586`).
- **(i):**
  - **The genuine classes keep `not_repairs`' semantics.** A verification whose snapshot cannot be made defers, then
    parks with a question at `max_defers`.
  - **R and R′ become Git state too.** That is FUB-D6-PRUNE reopened: a valid candidate deferred or parked because a
    prune ran, or because a registry tear met a destination the access could not remove. It is the class (b) exists to
    close.
- **(ii):**
  - **Every class is a resumable refusal after one attempt.** Nothing durable is appended, and the refusal names the
    last failure's text.
  - **R and R′:** the command ends at once rather than attempting past. That is the external-prune class's face 1
    (§7.4), a liveness cost.
  - **G2 to G4** are environmental, and a refusal is the better answer for them anyway: a deferral spent on a full disk,
    a missing filter or an unfetched object is a wrong outcome for a valid candidate, and the resume succeeds once the
    environment is repaired.
  - **G1** costs liveness. A verification snapshot the checkout cannot hold stops the run at each resume until the
    environment or the content changes, where (i) would park the candidate with a question (R14, §7.7).
    - A verification's snapshot holds the head's tree and the candidate's changes. The staging checkout and the
      candidate's slot on the same machine already held those paths.
    - So G1 there needs the snapshot's destination to cross a platform path limit the others did not: Windows' limits
      without `core.longpaths`, or macOS's 1,024-byte paths. Changing the environment answers that.
- **(ii′):**
  - **R and R′ are attempted past.** Face 1 is closed for liveness as well.
  - **Every genuine failure costs a full add per attempt until the deadline.** That is 16 attempts in the 500 ms test
    deadline for a small tree, and 6 or 7 for 4,000 files. At the ten-second production deadline it is 76 to 92
    checkouts of such a tree per access, computed from the measured 62.7–85.8 ms per attempt and the backoff
    (`d8/witness/deadline-estimate.txt`, which reproduces the measured 6 or 7 at 500 ms). Each attempt makes and removes
    an entry in the shared store, and the access then refuses anyway.
  - **It is round 5's retry-everything for these failures** (§4.2's costs 1 and 3).
  - **It makes the removal proof moot,** since both branches attempt again. The triage keeps the removal proof.

**The decision: (ii), `Undecidable`.**
1. **It is the only answer under which no registry state becomes Git state.** (i) returns every prune deletion after
   the takeover as Git, and a registry tear met with a destination the access cannot remove too (executed: R′).
2. **Its cost is a resumable refusal, never a durable wrong outcome.** For the environmental classes it is the better
   outcome. For G1 it is liveness, stated as R14 and put to the owner with the narrowing (§7.11).
3. **It is the contract's own answer for a veto that cannot decide** (§6.4, step 5). It adds no state and no Git child.
   It is also the answer D's legacy veto takes for the same state, because `Return` leads to legacy's discard (the
   orchestrator's addendum to #331's round 3). One helper keeps one meaning.
4. **(ii′) would buy face 1's liveness with a checkout per attempt for every genuine failure,** churning the shared
   store and mooting the removal proof. A bounded variant, a takeover failure attempted once more and then refused,
   was considered too. It needs a counter in the veto, which is new state, for a rare event's liveness. Round 8 adds no
   machinery.

**What this changes in round 7's claims.**
- **FUB-D6-PRUNE.** Its durable consequence stays closed: a prune's failure after the takeover is never Git state.
  Its liveness is not closed: the command ends resumably at once. That is the external-prune class's face 1, filed.
- **FUB-D6-INODE.** "Untouched" still means an empty directory the access can remove. Its shapes now refuse after one
  attempt, where round 6 made 15 and round 7 returned Git.
- **FUB-D5-GENUINE.** No classifier reads its failure as contention, and nothing attempts it again. Its checkout
  failure is a resumable refusal, not `not_repairs`' deferral (R14). That consequence is reopened deliberately, and it
  is filed as `PR329-A-GENUINE-CHECKOUT-FAILURE-AFTER-THE-TAKEOVER-REFUSES`
  (`findings/P2_liveness_202610021308_a-genuine-checkout-failure-after-the-takeover-refuses.md`, P2, deferred to the
  owner's consolidated question, §7.11).

### 7.4 The external-prune class, filed as one P1

**The file.** `findings/P1_correctness_202610021308_an-external-prune-deletes-an-engine-worktrees-registration.md`:
`PR329-AN-EXTERNAL-PRUNE-DELETES-AN-ENGINE-WORKTREES-REGISTRATION`, P1, `deferred`, `pre_existing`. It is this branch's
own finding. #330 (C) and #331 (D) refer to it and file no duplicate.

**The mechanism** is §6.2's: a prune decides on an add's entry before its `locked` exists, and deletes it later
without looking again. It has two faces.
- **Face 1,** the deletion during the add, after the takeover. Under §7.3 it is a resumable refusal at once.
- **Face 2,** the deletion after the add returned: round 7's R13.

**Every starter** is outside the engine once B, C and D land:
- a host-runner agent's Git: its prune, its gc, its automatic maintenance;
- the user's or an IDE's Git in any checkout, R3′, until now filed nowhere;
- Git's automatic maintenance after a commit, fetch or merge in any checkout. It reaches the prune through gc when gc
  is due, and on 2.55.0 through the default `worktree-prune` task whenever an entry looks prunable (§6.2).

**Face 2, executed** (`d8/witness/r13.txt`, `r13-summary.txt`). 42 runs:
- `git worktree prune` and `git gc` on 2.43.0, 2.50.1 and 2.55.0, and maintenance on 2.55.0;
- with the store kept by another registration, and with the add's entry the store's last.

In every run:
- **the access returned Ok,** and then the registration was deleted;
- **every engine command in the checkout exited 128,** "not a git repository": the gate command
  `git rev-parse --verify HEAD`, `candidate_diff`'s argv, `git status`, and `git add -A`;
- **the slot was no longer listed;**
- **Git cannot re-register the checkout:** `git worktree repair <slot>` exited 1 ("unable to locate repository; .git
  file does not reference a repository") and re-registered nothing, and `git worktree add` over the checkout exited 128
  ("already exists");
- **the edits written into the checkout stayed.**

In the 21 runs where the entry was the store's last, Git removed the store as well, leaving a populated slot with no
store.
- **Unpaused,** it was not observed in 18,000 adds (`ok_but_registration_gone_later` 0). It needs the pruner held
  between its decision and its deletion for the add's whole tail, which only scheduling provides. The rate does not
  lower the grade.

**Face 2's consequences in the engine** (reasoned from the code at `5c222ff2`; the three lenses traced them):
- **A verification's staging diff.** `candidate_diff` (`src/workspace_manager.rs:4793`, called at
  `src/engine/topology/run.rs:356`) gives Git state. `src/engine/topology/run.rs:279` maps it to `Unavailable`, and the
  frozen `src/engine/topology/integrate.rs:871` defers or parks a valid candidate.
- **A verification's gate.** The gate exits 128 in the snapshot. `src/engine/topology/attempt.rs:990` takes the
  verdict, `src/engine/classify.rs:54` makes it `GateFailed`, and the frozen `src/engine/topology/integrate.rs:779`
  appends `MergeRejected` for a valid candidate.
- **An attempt's gate.** The same verdict settles a spent attempt (`src/engine/topology/run.rs:950`).
- **An attempt's slot.** The capture fails and the command ends resumably. On resume, `quiescence` answers
  `NotRegistered` (`src/workspace_manager.rs:2769`), and `dispatch::verify_or_recreate`
  (`src/engine/topology/dispatch.rs:248-253`) removes and recreates the slot, deleting its unpinned edits.
- **The store gone.** The removal scan refuses a populated target (`src/workspace_manager.rs:5144-5164`), so the frozen
  finalization's `scrub_slots` (`src/engine/topology/finalize.rs:251`) refuses on every resume.
- **A legacy run.** `verify_gate_worktree`'s `git status` (`src/workspace.rs:908-944`) fails as Git state, and the
  legacy coordinator discards paid output (`src/engine/coordinator.rs:544-548`). This is #331's R-D9.

**Its G6 applicability, and its candidate closures,** are in the file and in §7.7. It applies through Q6/R17, Q1, ST-18
and INV-22, and it **blocks G6** until a closure is implemented and validated or the owner rules on its scope.

None of the candidates is added here. Each is scoped with the frozen status of its files:
1. **Re-check the registration before a durable negative outcome.** In `src/workspace_manager.rs`, `run.rs` or
   `attempt.rs`, none of them frozen.
2. **Keep, or re-register, a live unregistered checkout instead of recreating its slot.** In `dispatch.rs` and
   `src/workspace_manager.rs`, not frozen. Git has no command for it, as executed above.
3. **Let a store-gone reclaim converge.** In the removal scan, with the change to its pinned refusal
   `a_missing_stored_worktree_directory_refuses_before_checkout_deletion` (`src/workspace_manager/tests.rs:6952`), not
   frozen.
4. **Environmental requirements.**
   - `maintenance.auto=false` and `gc.auto=0`, and `maintenance.worktree-prune.auto=0` on 2.50.1 and later
     (`d8/git-src/prune-config-lines.txt`), stop the automatic starters.
   - `gc.worktreePruneExpire=never` does not stop them: the no-`gitdir` rule ignores the expiry.
   - No configuration stops an explicit `git worktree prune`.
5. **The owner's ruling on scope.**

**Reconciled:**
- **`PR11-HOST-AGENT-PRUNE-RACES-AN-ENGINE-ADD`** (P2, PR12) gains a reconciliation section. Both faces' grading and G6
  disposition are the P1's. The P2 keeps the agent's access to the shared registry and its remedies.
- **C's R-GU and D's R-D9** refer to the P1.
- **R3′ is no longer "none".**

**For follow-up C (unchanged, stated for its R-P).**
- Every explicit engine `git worktree prune` stays deleted (§3.5; `src/workspace_manager.rs:3061`, `:3100`, `:3123` at
  `5c222ff2`).
- Removal stays bound to the instance's registration.
- No global-prune fallback is added, in round 8 or in any candidate closure.

### 7.5 FUB-D7-ENVCENSUS, FUB-D7-R9WIN and the widened exception

**FUB-D7-ENVCENSUS goes with the probe.**
- Round 8's add sets no environment.
- `every_production_command_spec_payload_is_classified` (`src/runner/contract.rs:2422`) keeps its eight `.env(` for
  `src/workspace_manager.rs`.
- §5.8's one instrument row, the helper's `effect_free` row, is again the whole of the instrument change.

**FUB-D7-R9WIN: R9′ is qualified as R9″** (§7.7). A junk removal that left files in the destination (a handle held on
one) leaves a non-empty destination, so the answer is `Undecidable`, a refusal.
- **With a registration store,** the resume's forced removal binds no registration to the target and removes it once
  the handle closes (`src/workspace_manager.rs:3028`, `:3040`).
- **With no store,** the removal scan refuses a populated target (`:5144-5164`). That refusal is pinned by
  `a_missing_stored_worktree_directory_refuses_before_checkout_deletion` (`src/workspace_manager/tests.rs:6952`).
  After the handle closes, the operator removes the destination, as §6.5's remedy says for a static tear.

This is the store-gone shape of the external-prune class too (§7.4, candidate 3).

**The widened exception** is withdrawn with the probe. The design lens's note is answered in §7.2.

### 7.6 The helper's contract after round 8, for follow-up D

§5.5 carries the dated note (2026-10-02, round 8). **§6.4 stands, with these changes:**
- **The probe is withdrawn.** No add veto runs a Git command. §6.4's "For D's legacy add" paragraph is replaced:
  - D's add adopts the removal proof: an owned empty destination D can remove is untouched, so `Attempt`.
  - A destination absent, or an empty directory D cannot remove, after a failed attempt answers `Undecidable`.
  - Anything else answers `Undecidable`.
  - D adopts no probe, including a private gitdir/commondir variant.
- **The bound's veto term** is a few metadata calls, one `rmdir` and one `mkdir`, for both adds. §6.4's "the deadline,
  plus the runtime of its last attempt, plus the runtime of the veto after it" stands. The `bound-slow` row now
  returns at 602.3–603.5 ms under round 8's answer, one attempt and no probe, where round 7 took 1,205.0–1,205.6 ms
  (`d8/witness/FIGURES.txt`).
- **The erratum.** §6.4's placement bullet for `CONTENDED_ATTEMPTS` cited round 4's passing censuses as §3.8. They are
  §3.6's. It is corrected in place.

**Unchanged:**
- `Again` and its three answers;
- `Undecidable` refusing at once;
- the final attempt, and the end-to-end bound;
- `CONTENDED_ATTEMPTS` and `contended_attempts()`;
- the helper's name, module and `effect_free` row;
- `RegistryHold`, the canonical `common_git_dir`, and nothing sampled;
- the deadline's values;
- `RegistryRefused` as the one refusal variant, the one D's B-PRESERVE keys on.

### 7.7 What is closed, what remains, and what G6 meets

**The claims, once implemented.** They replace §6.6's 1 and 4. Claims 2, 3, 5, 6 and 7 stand, with 5's veto term as in
§7.6.
1. **No manager registry access returns `UpstrokeError::Git` for anything the registry's state caused.**
   - That covers contention, a write a dead process left torn, a registration nobody is writing, and a prune's
     deletion during an add, before or after its takeover.
   - An add returns Git state only when its destination cannot be made, or was not an empty directory when the access
     began. Neither depends on the registry.
   - Every failure of Git's add itself is either attempted again (its destination provably untouched) or refused.
4. **An add's failure after Git may have taken its destination over refuses at once,** as a resumable registry
   refusal. Its destination is absent, or an empty directory the access cannot remove. That covers a prune's failure
   and a genuine checkout failure alike. It is never Git state, and never attempted again (§7.3).

**What remains.** These rows replace §6.6's R2′, R3′, R8′, R9′, R11, R12 and R13. R1″, R4 to R6 and R10 stand.

| | What | Consequence now | Finding |
|---|---|---|---|
| R2″ | A host agent's own Git: its prune, its gc, its automatic maintenance | Its torn entries are attempted past. Its prune's deletion of an engine registration is the external-prune class: during an add after the takeover, a resumable refusal at once; after the add returned, face 2's consequences | `PR329-AN-EXTERNAL-PRUNE-DELETES-AN-ENGINE-WORKTREES-REGISTRATION` (P1); `PR11-HOST-AGENT-PRUNE-RACES-AN-ENGINE-ADD` (P2, the agent's access, reconciled) |
| R3″ | The user's or an IDE's Git in any checkout, and Git's automatic maintenance after a commit, fetch or merge (on 2.55.0 whenever an entry looks prunable) | as R2″ | `PR329-AN-EXTERNAL-PRUNE-DELETES-AN-ENGINE-WORKTREES-REGISTRATION` (P1) |
| R8″ | The access's runtime past the deadline | its last attempt's runtime, plus the veto's: metadata, one removal and one making (§7.6) | stated |
| R9″ | Windows: no file identity in std; delete-pending directories; handles held on files | The removal proof needs no identity. A junk removal that failed transiently while the access's removal then succeeds reads as untouched and is attempted again. A destination Windows will not remove or make again is `Undecidable`: a refusal. A junk removal that left files (a handle held) leaves a non-empty destination: `Undecidable`. With a store, the resume's reclaim removes it once the handle closes; with no store, the scan refuses it and the operator removes it (§7.5). Reasoned, not executed | stated |
| R11′ | A filter the repository configures runs in the add's checkout, after the takeover | Its failure, whatever its cause, is a failure after the takeover: a resumable refusal at once (§7.3), no longer Git state | stated (R14) |
| R14 | A genuine checkout failure after the takeover (§7.3's G1 to G4) | A resumable refusal after one attempt, naming Git's message; nothing durable. A verification's snapshot that cannot be made stops the run at each resume until the content or the environment changes, where round 6 returned Git state and `not_repairs` deferred, then parked with a question | `PR329-A-GENUINE-CHECKOUT-FAILURE-AFTER-THE-TAKEOVER-REFUSES` (P2, deferred): FUB-D5-GENUINE's consequence, reopened as round 8's cost, for the owner's consolidated question (§7.11) |

R12 is withdrawn with the probe. R13 is the external-prune class's face 2.

**The cases, and what G6 meets.** As §6.6, with (a) and (b) updated and the external-prune class as its own row.

| Case | Closed by | Severity | Applies to G6 | Blocks G6 |
|---|---|---|---|---|
| (a) An attempt's pipeline error | §5.3, §5.4 and §7.2, in the non-frozen `src/workspace_manager.rs` and `src/error.rs`, for contention and torn residue that clears by the deadline. A prune's deletion after an add's takeover ends the command resumably at once: the external-prune class's face 1, below | P1 | yes: R17, the shared registry, Q6 | until implemented and validated |
| (b) A durable verification deferral or park | the same. No registry state reaches `run::verified`'s Git arm through an access. Face 2 reaches it through `candidate_diff`: below | P1 | yes: Q6 and durable verification | until implemented and validated |
| (c) DESC | filed, follow-up C | P1 | yes: Q1, INV-22, ST-16, ST-18 | yes; filing is no waiver |
| (d) A legacy writer tears a topology reader | §5.3 | P1 class | yes | until implemented and validated |
| (e1), (e1′) Legacy against legacy | follow-up D | P1 | no | no; each remains a P1 until D lands |
| (e2) A topology writer tears a legacy reader | **follow-up D**, through §5.5 as dated, with §7.2's veto | P1 | yes: Q6, across the shared registry and R17 | **yes, until follow-up D is implemented and validated** |
| (e2′) A topology writer's static or deadline residue, then a legacy discard | **follow-up D** | P1 | yes: Q6; a crash producer engages Q1 | **yes, until follow-up D is implemented and validated, unless the owner rules otherwise** |
| **The external-prune class:** a prune no engine process starts deletes an engine registration, during an add after its takeover (face 1) or after the add returned (face 2, R13) | **not closed:** filed as `PR329-AN-EXTERNAL-PRUNE-DELETES-AN-ENGINE-WORKTREES-REGISTRATION`, with five candidate closures (§7.4). Face 1 is narrowed by §7.3 to a resumable refusal | P1 (all three lenses of round 7, face 2) | **yes:** Q6 and R17 (parallel checkouts share the registry; a valid candidate's verification becomes a deferral, a park or `MergeRejected`), Q1 (recovery removes unpinned edits), ST-18 (finalization does not converge with the store gone), INV-22 (Git's administrative residue with its worktree) | **yes, until a closure is implemented and validated, or the owner rules on its scope; filing is no waiver** |
| R14: a genuine checkout failure after the takeover refuses | filed as `PR329-A-GENUINE-CHECKOUT-FAILURE-AFTER-THE-TAKEOVER-REFUSES`, §7.3 | P2, liveness; no durable outcome | Q6: no durable outcome is recorded; liveness only, for G1's path limits | no; the owner's consolidated question records it (§7.11) |

**The findings:**

| Item | Severity | Here | Applies to G6 | Blocks G6 |
|---|---|---|---|---|
| `PR11-LINKED-CHECKOUTS-RACE-THE-SHARED-WORKTREE-REGISTRY`: (a), (b), (d) | P1 | repaired by §5.3, §5.4 and §7.2 once implemented; its file is deleted then | yes | only until this change merges |
| `PR329-AN-EXTERNAL-PRUNE-DELETES-AN-ENGINE-WORKTREES-REGISTRATION` | P1 | filed here; candidate closures in the file; the owner's consolidated question | yes | **yes**, until closed or ruled on |
| `PR329-A-RESUME-REBINDS-A-SLOT-ITS-DEAD-COORDINATORS-GIT-CHILD-STILL-WRITES`: (c) | P1 | filed; follow-up C | yes | yes, until follow-up C merges |
| `PR329-LEGACY-RUNS-IN-LINKED-CHECKOUTS-RACE-THE-SHARED-WORKTREE-REGISTRY`: (e) | P1 | filed; follow-up D, before G6 | (e2) and (e2′) yes; (e1) and (e1′) no | yes, through (e2) and (e2′), until follow-up D lands, unless the owner rules otherwise |
| `PR11-HOST-AGENT-PRUNE-RACES-AN-ENGINE-ADD` | P2 | reconciled: its prune faces are the P1's; it keeps the agent's access and its PR12 guard | through the P1 | through the P1 |
| `PR329-A-GENUINE-CHECKOUT-FAILURE-AFTER-THE-TAKEOVER-REFUSES` (R14) | P2 | filed here, deferred to the owner's consolidated question | yes, as liveness | no |
| `PR5-RD-002-RESUME-DERIVES-THROUGH-A-TORN-ENUMERATION` | P2 | retained (§6.5) | as filed | as filed |

### 7.8 Regression tests

§5.7's tests stand, with §6.7's T4 and T22. The others change as follows.

**Withdrawn:**
- T19 (the probe's decision);
- T21 (the probe reads no registry);
- the probe residue in T17;
- mutations m9 (the probe skipped) and m11 (the probe's answer ignored).

**Revised:**
- **T15, a genuine failure through the add.**
  - FUB-D5-GENUINE's tree, a 300-byte name and FUB-D6-INODE's two shapes (Unix) are `RegistryRefused` after one
    attempt, with nothing at the slot.
  - A verification over FUB-D5-GENUINE's tree ends the command resumably with nothing durable appended, and its resume
    re-verifies under a new sequence.
  - Mutation m10, the removal proof skipped, so an empty destination reads as untouched (round 6's rule): red, because
    the INODE shapes run 16 attempts.
- **T16, the destination's lifecycle:**
  - after an untouched failure the destination is a new empty directory;
  - a refused add leaves no destination it made;
  - a destination present but not empty at the start is Git ("already exists");
  - a destination that cannot be made is Git at once.
- **T17, the store-absent slot (round 6's form).**
  - An intent, an empty slot and no registration store, then the forced removal: Ok, and the slot is gone.
  - A populated slot with a `.git` file and no store still refuses, as
    `a_missing_stored_worktree_directory_refuses_before_checkout_deletion` pins.
  - Mutation m6: the empty-target branch removed. Red.
- **T20, `torn-parent`:** a torn foreign entry and a parent that cannot be written. `RegistryRefused` after one
  attempt, never Git. It pins R′: under option (i) it would be Git.

**New:**
- **T23, a failure after the takeover is refused, not returned and not attempted again.**
  - A prune needs a pause inside Git, so it is executed at the Git level (§6.2, §7.3) and not in the Rust suite. The
    suite reproduces the end state instead, with T19's construction kept as a fixture: a required smudge filter that
    fails on its first run only.
  - The add is `RegistryRefused` after one attempt, and its message names the filter's failure.
  - Mutation m12: the possibly-taken-over answer as `Return`, option (i). Red: Git after one attempt.
  - Mutation m13: the same answer as `Attempt`, option (ii′). Red: two attempts, then Ok.

**The proof the implementer owes:** §3.8's, with mutations m1 to m8, m10, m12 and m13 (m9 and m11 are withdrawn with
the probe).

### 7.9 Effect governance, instruments and the frozen set

**As §5.8.**
- **Code:** `src/workspace_manager.rs` (the helper, `Again`, `RegistryHold`, R-X's type, the add's destination and
  removal proof, targeted removal, round 6's store-absent branch, and the test handshake) and `src/error.rs`
  (`RegistryRefused`).
- **One instrument row:** `tolerant_registry_access` in `src/workspace_manager.rs`'s `effect_free` list
  (`effects/wrappers.toml`), measured in §5.5.
  - Nothing else moves: no `.env(`, no Git child, no `clippy.toml` entry, no `effects/allowlist.toml` text, no
    `src/effects/` test.
  - No frozen module or frozen test child moves.
- **C's boundary.** Round 8 adds no writer. The probe's writer that C was asked to count is withdrawn. C's census
  covers the add's own Git children as it did before round 7.
- **Docs, at implementation:** as §5.8, with `DESIGN.md` §15's paragraph now round 8's.

### 7.10 Design review round 7, answered

| Finding | Sev | Kind | Round 8 | Where | Evidence |
|---|---|---|---|---|---|
| FUB-D7-SPLITINDEX | P1 | executed (all three) | **Fixed (design), witnessed:** the probe is withdrawn; the add's only Git command is Git's own `git worktree add`, whose checkout writes no index outside the new worktree's entry | §7.2 | the reviewers' split-index construction: from a linked base 27 of 27 (`reviewers-r8-summary.txt`) and from the main checkout 18 of 18 (`splitmain-r8.txt`), the main checkout's status rc 0, its index unchanged, no shared index removed or added |
| FUB-D7-CONFIG | P1 | executed (all three) | **Fixed (design), witnessed:** no second checkout; a failure after the takeover refuses, never Git | §7.2, §7.3 | the three CONFIG constructions: r8 refused after 1 in 27 of 27, option (i) Git in 27 of 27; controls and direct retries rc 0 |
| FUB-D7-R13 | P1 | Git prefix executed; consequences reasoned | **Filed** as `PR329-AN-EXTERNAL-PRUNE-DELETES-AN-ENGINE-WORKTREES-REGISTRATION`, both faces and every starter; the G6 table carries it as blocking | §7.4, §7.7 | `r13.txt`: 42 of 42 on three versions and three pruners; `stress-*.json` |
| FUB-D7-ENVCENSUS | P2 | reasoned (design) | **Fixed:** withdrawn with the probe; the census's eight `.env(` stand | §7.5 | — |
| FUB-D7-R9WIN | P2 | reasoned (regression) | **Fixed:** R9″ qualifies the reclaim by whether a store exists | §7.5, §7.7 | the pinned refusal at `src/workspace_manager/tests.rs:6952` |
| FUB-D5-GENUINE, reopened | P2 | round 8's decision | **Filed** as `PR329-A-GENUINE-CHECKOUT-FAILURE-AFTER-THE-TAKEOVER-REFUSES`, deferred to the owner's consolidated question | §7.3, §7.7 | `FIGURES.txt`: the genuine classes refused after 1 under (ii), Git after 1 under (i) |
| (note) the widened exception | — | reasoned | **Withdrawn:** round 6's empty-directory exception restored | §7.2 | — |
| §6.4's "§3.8" | P3 | found after round 7's push | **Corrected** in place: §3.6 | §6.4 | — |

### 7.11 Risks, sequencing, out of scope, and the owner

**Risks**, beyond §5.10's:
- **A genuine failure after the takeover refuses resumably** (R14). It is fast, one attempt, and names Git's message.
  For G1 content in a verification, the run stops at each resume until the content or the environment changes.
- **A prune's deletion after an add's takeover ends the command resumably** (face 1). Unpaused, under four continuous
  prune loops, 15, 5 and 4 adds in 2,000 refused.
- **The external-prune class's face 2 is open and blocks G6** (§7.4, §7.7).
- **Windows is reasoned** (R9″).

**Sequencing.**
- D's implementation follows this change's merge, and D re-checks §5.5's round-8 note.
- C's R-P dependency is unchanged (§7.4).
- The external-prune class needs a closure, or the owner's ruling, before G6. Whichever closure is chosen is a change
  of its own, after the owner's answer.

**Out of scope, and said so:** as §5.10 and §6.11, with the external-prune class filed rather than closed.

**The owner.** Round 8 narrows what #329 claims, and narrowing a P1 fix is the owner's to classify (MAINTAINING).
Three things are narrowed:
- an external prune's failure after an add's takeover is refused, not attempted past (face 1);
- the deletion after the add returned is filed, not closed (face 2);
- a genuine checkout failure after the takeover refuses rather than deferring (R14,
  `PR329-A-GENUINE-CHECKOUT-FAILURE-AFTER-THE-TAKEOVER-REFUSES`).

They go into one consolidated owner question with C's D1 to D4 and decision B, after design reviews B8, C4 and D3. No
question is asked before then.

## 8. Round 9: the external-prune closures made accurate, face 2's boundary, and two rules

> **PROPOSED — the last design round of #329 before the owner's consolidated question.** This section supersedes §7
> only where it says so, item by item. It adds no machinery and implements no closure.
>
> **Replaced:**
> - §7.2's "before the first attempt" bullet for a destination that is not an empty directory at the start, by §8.2,
>   and the same rule wherever §5.3, §6.3 and their tables state it (attempted once, Git's "already exists" returned);
> - §7.4's two faces, by §8.3;
> - §7.4's candidate closures 1, 2 and 4, by §8.4, §8.5 and §8.6. Closures 3 and 5 stand, restated in §8.6;
> - §7.7's claim 1, its R2″, R3″ and R14 rows, and its G6 table, by §8.9;
> - §7.8's T15 and T16, by §8.10;
> - §7.11's paragraph on the owner, by §8.11.
>
> **Added:**
> - the host-width reconciliation (§8.7);
> - a second destructive boundary in face 2's reach, a retained generation's retry (§8.5);
> - R14's G6 row, which waits on the owner (§8.8);
> - closure 1 stated as a partial mitigation, with its residuals R-REWRITE and R-OUTSIDE (§8.4), after the
>   orchestrator's addendum (`~/orch-pr11/answers/pr11_fub_design9-0.md`).
>
> **Stands:** everything else in §7, and §6 and §5 where §7 left them. The helper's contract (§5.5, §7.6) does not
> change, and §5.5 carries a dated note saying so.
>
> **Nothing in §8 is in force until the implementation lands.**

Design round 9 is `pr11_fub_design9`'s (`claude-opus-5-5`, `max`), spawned on `f7a9256c` to carry out the orchestrator's
triage of design review round 8 (`~/orch-pr11/reviews/review-329-d8-triage.md`, items 1 to 5). Its figures are under
`~/orch-pr11/logs/pr11_fub_design9/`, cited as `d9/…`; `d9/witness/FIGURES-d9.txt` holds every figure below.
- **No production code and no prototype.** The evidence is:
  - Git's source and documentation, in the three upstream release trees round 7 verified
    (`d9/git-src/citations-d9.txt`);
  - the engine's code at `5c222ff2`;
  - the reviewers' saved witnesses, copied with their hashes to `d9/witness/reviewer-d8/`;
  - new Git-level witnesses.
- **The new witnesses run only ordinary Git commands** in temporary directories: `init`, `commit`, `worktree add`,
  `worktree prune`, `worktree list`, `status`, `rev-parse`, `diff-index` and `cat-file`. They run on round 7's builds of
  2.43.0, 2.50.1 and 2.55.0, and record what changed with Git's own `GIT_TRACE2_EVENT` output and with directory
  listings taken before and after.
  - A partial state is constructed statically: one name of an entry removed, as Git's deletion pass leaves it between
    two unlinks.
- **Master is `5c222ff2`.** The branch is not rebased, and §8 cites `5c222ff2` throughout.

### 8.1 What design review round 8 found, and what round 9 does

**The findings** (triage, 2026-10-02T14:24Z). Three `gpt-6-astra` lenses at `max` ran on `f7a9256c`: design,
concurrency and regression, in the conformance-reading form, with no refusals. All three returned CHANGES_REQUIRED.

| id | sev | evidence | what | §8 |
|---|---|---|---|---|
| FUB-D8-C1PARTIAL | P1 | the partial states executed on the three versions (all three lenses) | Closure 1's reciprocal-pointer check passes a partly deleted entry. With `HEAD` or `commondir` gone and `gitdir` left, the gate exits 128 and the check passes, so the failure becomes `GateFailed` and then `MergeRejected`. It is round 8's own erratum, confirmed. | §8.4 |
| FUB-D8-C1PLACE | P1 (concurrency, regression), P2 (design) | reasoned (all three) | "Once, at `run::verified`'s Git arm" misses failed gates: `attempt.rs:993` builds `GateFailed` inside `Ok(Judgement)`, `run.rs:258` returns `Judged`, and `integrate.rs:794` appends `MergeRejected`. Attempt settlements bypass it too. | §8.4 |
| FUB-D8-DURINGADD | P1 | executed on the three versions (concurrency) | A deletion that lands before the add returns can still give the add rc 0. Face 2 must include deletion before the return. | §8.3 |
| FUB-D8-C2MISSING | P1 | the Git interleaving executed on the three versions (concurrency); the loss reasoned | Closure 2's `NotRegistered` guard misses a deletion after the enumeration. That answers `Missing` (`workspace_manager.rs:2783`), and `dispatch.rs:251` removes the populated slot. | §8.5 |
| FUB-D8-POPULATED | P2 (design, concurrency), P3 (regression) | executed on the three versions with round 8's model | A destination populated at the start still runs Git once. Git's sibling scan runs before its destination check, so a torn sibling's registry error returns as Git. | §8.2 |
| FUB-D8-C4SCHED | P2 | executed on 2.55.0 | Closure 4 misses scheduled maintenance, a checkout's own `config.worktree` overriding the shared settings, and maintenance already running. | §8.6 |
| FUB-D8-R14G6 | P2 | reasoned against `design/26` | R14's unconditional "does not block G6" is unsupported: R14 narrows specified semantics, and needs the owner's disposition before G6. | §8.8 |
| FUB-D8-HOSTWIDTH | P2 | reasoned (design) | Width one per run does not remove the host-agent starter: two width-one runs in two linked checkouts overlap. | §8.7 |
| FUB-D8-T15 | P3 | the executed evidence (`d8/witness/scenarios-*.jsonl`) | T15 expects nothing at the slot, but the INODE shapes leave the empty directory on every version. | §8.10 |

**Agreed by all three lenses.**
- **The narrowing holds.** The probe is fully removed. CONFIG refused 27 of 27. The split index stayed intact in 27 of
  27 runs from a linked base and 18 of 18 from the main checkout. Every tested post-takeover prune point refuses.
- The sibling contract (R4) holds, and round 8 added no machinery (R5).
- **The class applies through Q6/R17, Q1, ST-18 and INV-22, and blocks G6.** Filing cannot clear it, and neither can
  one partial closure.

**What round 9 does,** the triage's work list, adding no machinery:
1. it rejects a populated destination before Git's add runs, after a successful prevalidation (§8.2, as qualified at
   repair round 3);
2. it moves face 2's boundary to the prune's decision (§8.3);
3. it makes the five candidate closures accurate, closure 1 as a partial mitigation after the orchestrator's addendum
   (`~/orch-pr11/answers/pr11_fub_design9-0.md`), adds the host-width reconciliation, and gives the owner a
   recommendation (§8.4 to §8.7, §8.11);
4. it gives R14 a G6 row that waits on the owner (§8.8);
5. it fixes T15 (§8.10).

### 8.2 A destination that is not an empty directory at the start: rejected before Git's add runs (FUB-D8-POPULATED)

**Why round 8's rule was wrong.** Round 8 attempted such an add once and returned Git's error (§7.2).
- **Git's add runs its sibling scan before it checks the destination:** `get_worktrees()`, then
  `check_candidate_path()` (2.43.0 `builtin/worktree.c:429-430`, 2.50.1 `:444-445`, 2.55.0 `:478-479`;
  `d9/git-src/citations-d9.txt`). So a torn sibling answers first, and its registry error came back as Git state.
- **The rule was wrong a second way.** Git's check is `file_exists(path) && !is_empty_dir(path)` (2.43.0 `:310`, 2.50.1
  and 2.55.0 `:325`), and `is_empty_dir` follows a link. For a link to an empty directory Git says nothing about
  "already exists": it makes the checkout through the link.

**The rule.** Before the first attempt the access reads its destination (`symlink_metadata`, and `read_dir` when it is
a directory):
- absent: it is made as an empty directory (unchanged);
- an empty directory: it is used as it is (unchanged);
- it cannot be made, or its metadata cannot be read: Git state at once, naming the path and the OS error (unchanged);
- **anything else is Git state at once, and no Git command runs:** a directory holding entries, a file, a link (to an
  empty directory included), a reparse point.
  - The message names the path and what is there, and says that no Git command ran.
  - It is `UpstrokeError::Git`, as Git's own "already exists" was on master, so a verification's consequence for it is
    master's.

**What it establishes.** No registry state can reach these answers, because no Git command runs for them. §7.7's
claim 1 becomes exact (§8.9).
*Qualified at repair round 3 (R4, P3, the implementation review's regular lens on `54a1ff14`; §9.13): the rule holds
after a successful prevalidation and before `git worktree add`. The add's gate (`revalidate`) runs first and lists the
registry, and that list is an access of its own. So a populated destination beside a registration already torn when
the gate lists the store meets the gate first: it refuses as the registry's at the deadline, naming the torn entry,
the destination is never read, and what is there stays. "No Git command runs" holds from the add's access onward, after
a gate that passed. The Git-state answers still carry no registry state: a store the gate's list fails on refuses as
the registry's, never as Git state.*

**Executed** (`d9/witness/populated.jsonl`, `FIGURES-d9.txt`). Three versions and three rounds, with a fresh
repository for each runner. "No Git process" is Git's own trace2 record, which shows no process started.

| Destination at the start | Git's own add | round 8 (r8) | round 9 (r9) |
|---|---|---|---|
| a directory holding a file, beside a sibling whose `commondir` is empty (the reviewers' construction) | rc 128, the sibling's `commondir`: "Success" on 2.43.0, "Numerical result out of range" on 2.50.1 and 2.55.0 | Git after 1 attempt, the sibling's error | Git after 0 attempts, no Git process, the destination named |
| the same, the sibling intact | rc 128, "already exists" | Git after 1, "already exists" | Git after 0, no Git process |
| a file | rc 128, "already exists" | Git after 1, "already exists" | Git after 0, no Git process |
| a link to an empty directory | **rc 0: the checkout made through the link** | **Ok after 1** | Git after 0, no Git process, the link's target untouched |

- The destination's own file was kept in every run that had one (9 of 9 per runner).
- The first row reproduces the reviewers' result (their `results.json`, copied to `d9/witness/reviewer-d8/`).
- **The link row is a change from master,** whose add runs Git on the slot path whatever is there. Round 9 does not let
  Git write through a link the access did not make. The containment checks the manager runs before the add are not
  re-examined here.

**Where it is.** In the add's destination steps, before the helper's loop. It adds no Git child, no state and no
instrument (§7.9 is unchanged), and the helper's contract does not change (§5.5's dated note).

### 8.3 Face 2 starts at the prune's decision (FUB-D8-DURINGADD)

**The boundary, corrected.** The two faces divide on whether the deletion fails the add, not on whether the add has
returned.
- **Face 1: the deletion fails the add.** It lands before Git's last write into the entry, so one of the add's own
  writes fails (`gitdir`, `commondir`, `HEAD`, the checkout's index), and Git's junk removal runs. Round 8 refuses that
  at once, resumably (§7.3). Unchanged.
- **Face 2: the add returns Ok, and its registration is deleted, wholly or in part, at any time from the prune's
  decision onward.** That includes a deletion during the add's tail, before it returns, as well as one after it
  returned.

**Why a deletion before the return still gives Ok** (`d9/git-src/citations-d9.txt`).
- After the checkout, the add clears its junk flag: `is_junk = 0` (2.43.0 `builtin/worktree.c:554`, 2.50.1 `:561`,
  2.55.0 `:596`).
- It unlinks `locked` with `unlink_or_warn`, which treats a missing file as success (`wrapper.c`'s
  `warn_if_unremovable`: 2.43.0 `:599-602`, 2.50.1 and 2.55.0 `:601-604`).
- It runs the `post-checkout` hook, and returns the hook's status (2.43.0 `:580`, 2.50.1 `:587`, 2.55.0 `:622`).
- Nothing on that path reads the entry again.

**Executed.**
- **The reviewers' witness** (the concurrency lens, on 2.43.0, 2.50.1 and 2.55.0). The prune decided before `locked`
  existed, and deleted the entry after the add cleared its junk flag and before it unlinked `locked`. The add exited 0,
  and `git status` in the checkout exited 128.
  - The results are `d9/witness/reviewer-d8/pr329-d8-prune-before-return-9o_2jeby.results.json`, whose hash matches
    `~/orch-pr11/reviews/SHA256SUMS-329-d8`.
  - Its construction paused Git with a preloaded shim, which this round's evidence rules exclude. It is cited, not
    re-run.
- **Re-executed with Git's own extension point** (`d9/witness/d9duringadd.py`, `duringadd.jsonl`).
  - The add's `post-checkout` hook, armed by an environment variable, sets the new checkout's `.git` aside. It runs
    Git's own `git worktree prune --expire=now`, which deletes the add's entry ("gitdir file points to non-existent
    location"), and puts `.git` back.
  - **18 of 18 runs:** the three versions; the store kept by another registration, and the entry the store's last;
    three rounds. In every run:
    - the add exited 0, and its trace2 `exit` event carries code 0;
    - the prune's trace2 `exit` came before the add's;
    - the entry was gone when the hook returned, and the checkout was not listed afterwards;
    - `git status` and `git rev-parse --verify HEAD` in the checkout exited 128;
    - where the entry was the store's last, the store was gone too.
  - This places the deletion after the `locked` unlink rather than before it. Both are before the return. That a real
    prune's decision can come before the add's `locked` is §6.2's executed rule.

**What follows.**
- The helper returned Ok, so no veto runs. The same holds on the helper's final attempt.
- **Face 2's consequences are the filed P1's** (§7.4, with §8.5's second boundary), not face 1's liveness cost.
- **The legacy face is the same.** #331's design review round 3 confirmed it in all three lenses, executed at the Git
  level and through D's prototypes: a deletion before a successful add returns ends in a discard without C-SIDE
  (FUD-D3-DURINGADD, `~/orch-pr11/reviews/review-331-d3-triage.md`).
- **Superseded:** §7.4's "Face 2, the deletion after the add returned", and claim 1's "a prune's deletion during an
  add, before or after its takeover" (§7.7). From round 9 claim 1 covers a deletion during an add **that makes the add
  fail** (§8.9). `design/15`'s paragraph says the same.

### 8.4 Closure 1, corrected: a whole-registration check at every durable negative outcome, and a partial mitigation (FUB-D8-C1PARTIAL, FUB-D8-C1PLACE)

**Why round 8's check failed.** It read the two pointers only.
- A prune deletes the entry's names one at a time, in the filesystem's directory order, and then the directory (`dir.c`
  `remove_dir_recurse`: 2.43.0 `:3333-3367`, 2.50.1 `:3381-3415`, 2.55.0 `:3431-3465`).
- On this box's ext4 the order is `HEAD`, `index`, `logs`, `gitdir`, `ORIG_HEAD`, `refs` (2.50.1 and 2.55.0),
  `commondir` (`FIGURES-d9.txt`). So the first unlink already breaks the checkout, while `gitdir` still names it back.

**Which names a checkout's commands need** (`d9/witness/partial.jsonl`). Each name was removed alone from a fresh
entry, and four commands ran in the checkout. The three versions, three rounds: 9 runs per name, and 6 for `refs`,
which 2.43.0's add does not write.

| Removed | gate `rev-parse --verify HEAD` | `status`, the engine's read form and a user's | `rev-parse --git-common-dir` | What `status` printed |
|---|---|---|---|---|
| nothing (the control) | 0 | 0, 0 | 0 | nothing |
| `HEAD` | 128 | 128, 128 | 128 | — |
| `commondir` | 128 | 128, 128 | 128 | — |
| the checkout's `.git` | 128 | 128, 128 | 128 | — |
| `index` | 0 | 0, 0 | 0 | `D  tracked` and `?? tracked`: a clean checkout read as changed |
| `gitdir` | 0 | 0, 0 | 0 | nothing |
| `logs`, `ORIG_HEAD`, `refs` (2.50.1, 2.55.0) | 0 | 0, 0 | 0 | nothing |

- **No command re-created a removed name.** The entry was listed before and after each command.
- **Git's discovery explains the first rows.** A git directory needs a valid `HEAD`, and `objects` and `refs` under its
  common directory, which is the entry itself when `commondir` is absent (`setup.c`'s `get_common_dir_noenv` and
  `is_git_directory`: 2.43.0 `:316-337` and `:356-386`; 2.50.1 `:326-347` and `:418-448`; 2.55.0 `:323-344` and
  `:415-445`).

**The check, corrected.** It runs after the failure and before the durable outcome. It reads files and starts no Git
child. The registration is **whole** when:
1. the checkout's `.git` is a `gitdir:` pointer naming an entry in this repository's own store, a relative line
   resolved against the checkout (as Git 2.48's `worktree.useRelativePaths` writes it);
2. that entry holds `gitdir`, `commondir`, `HEAD` and `index`, each a regular file, and the first three non-empty;
3. `commondir` resolves to the repository's common git dir, and `gitdir` names the checkout's `.git`, compared as the
   same file and not by spelling.

Anything else is not whole, and the outcome is a resumable registry refusal. Items 1 and 2 are D's C-SIDE check
(#331's record §3.4, at `ac18321f`). Item 3 adds the two resolutions the triage asks for.

**Closure 1 is a partial mitigation, not a closure.** The orchestrator's addendum
(`~/orch-pr11/answers/pr11_fub_design9-0.md`, 2026-10-02T15:20Z) withdrew the premise of the triage's hint, "deletion is
monotone, so usable after means usable at the failure". #331's design review round 3 executed two counterexamples
against D's C-SIDE, which is the same check (FUD-D3-CSIDEPROOF; `~/orch-pr11/reviews/review-331-d3-triage.md`). They
hold for this form too.

**What it catches, soundly.**
- **A prune's pass only removes names.** So a name absent at the check stays absent, and a complete deletion is always
  caught.
- **`HEAD` and `commondir` cannot be written again from the checkout.** With either gone, every command there exits
  128 before it does anything (the table). Nothing else in the engine writes them: no engine path runs
  `git worktree repair`, and an engine add never registers a path that holds a checkout (§8.2).
- **`gitdir` is written by no command in the checkout,** and the checkout's `.git` is a file in the checkout, which no
  prune touches.
- So if the pass had reached `HEAD`, `commondir` or `gitdir` by the check, the check fails. That covers the reviewers'
  partial states (FUB-D8-C1PARTIAL), and every state in which a checked name is still missing.

**What it leaves: two residuals.**
- **R-REWRITE: a removed name written again before the check.**
  - `index` can be written again by any index-writing Git command in the checkout while `HEAD` and `commondir` remain.
    `git status` does not do it (executed here: no command re-created a removed name, `partial.jsonl`).
  - #331's lenses executed it on 2.43.0, 2.50.1 and 2.55.0: a gate failed with rc 1 because `index` was gone, a later
    `git read-tree HEAD` wrote it again with rc 0, and the four names were then whole
    (`~/orch-pr11/reviews/331-d3-witnesses/review331-d3-concurrency-1l4wygsk/index-recreated-results.json`). Through D's
    prototype the same sequence was judged and discarded (`review331-d3-design-u6ezehsy/index-rewritten-results.json`).
  - In the topology placement the engine runs no Git command between the failing command and the check. Only the
    failing command itself, a process it left running, or another process in that checkout can write the name again.
  - **No read after the fact closes it:** a name written again cannot be told from one never removed.
- **R-OUTSIDE: a file the checkout needs that the check does not read.**
  - **`sharedindex.<sha>` under `core.splitIndex=true`.** Executed here (`d9/witness/d9splitindex.py`,
    `splitindex.jsonl`, 9 of 9 on the three versions): with it removed, `git status` and `git ls-files --stage` exit
    128 ("index file open failed: No such file or directory"), while `gitdir`, `commondir`, `HEAD` and `index` remain
    and the check passes. Its name is random, so the pass meets it anywhere: on this box's ext4 it came before `HEAD`
    in 4 of the 9 entries listed (`FIGURES-d9.txt`). #331's lenses executed the same through D's prototype
    (`review331-d3-design-u6ezehsy/additional-results.json`).
  - `config.worktree` and `info/sparse-checkout`, where the add copied them from a base with worktree configuration or
    a sparse checkout: their removal changes the checkout's configuration rather than failing it (reasoned, not
    executed).
  - `logs`, `ORIG_HEAD` and `refs` changed no command's result when removed (the table), so they need no reading.
  - **What would narrow it:** a Git read of the index in the checkout as part of the check (`git ls-files --stage`
    exits 128 without the shared index: executed). It covers what Git loads for the index, not configuration. It costs a
    Git child at each negative outcome, and, because the judge would call it, a manager entry point with its
    `effects/wrappers.toml` row: an instrument. Not proposed.

**Their severity: P1, inside the class.** Where a residual occurs, a valid candidate can still be rejected, deferred or
parked, or an attempt spent. Each needs more than the class does:
- R-REWRITE needs the pass to reach `index` before `HEAD`, `commondir` and `gitdir`, the prune held there, and an
  index-writing command in that checkout before the check;
- R-OUTSIDE needs a split index (or worktree configuration, or a sparse checkout), and the pass to reach that file
  first.

They are not a separate finding. They stay in `PR329-AN-EXTERNAL-PRUNE-DELETES-AN-ENGINE-WORKTREES-REGISTRATION`, which
stays open while they do, so the owner's ruling must name them (§8.11).

**What it does not do.** It does not protect the next command. A deletion after the check is met by the next failure's
check.

**Where it runs: at every durable negative outcome a checkout's Git state reaches** (the code at `5c222ff2`).
1. **Git errors from the manager's commands in a slot.**
   - `WorkspaceManager::candidate_diff` (`src/workspace_manager.rs:4793`, called at
     `src/engine/topology/run.rs:356-359`), and every other manager command a verification or an attempt runs in a
     slot.
   - On a Git failure the manager reads the check for that slot. When it fails, the command returns `RegistryRefused`
     instead of `UpstrokeError::Git`.
   - `run::verified`'s Git arm (`src/engine/topology/run.rs:279`) then receives no registration loss from them. Its
     last arm passes the refusal on (`:289`).
2. **Judgement verdicts: gates and reviews alike.** In `Judge::judge` (`src/engine/topology/attempt.rs:969-1127`):
   - after a gate's verdict fails (`:990-1016`; `GateFailed` is built at `:993`), and after a review pass's failure
     (`review_failure`, `:1087`);
   - **and before the snapshot is released** (`:1022-1025`, `:1116-1119`);
   - the judge reads the check for the snapshot. When it fails, `judge` returns
     `Err(JudgeError::Other(RegistryRefused))` instead of a judgement carrying the failure.

   That covers both callers.
   - **A verification** (`run.rs:412`). Otherwise `run.rs:258` returns `Verified::Judged`, and the frozen
     `src/engine/topology/integrate.rs` appends `MergeRejected` (`:779-794`), or settles the sequence unavailable as an
     outage or a human-required failure (`:755`, `:766`).
   - **An attempt** (`judge_attempt`, `attempt.rs:934-967`). Otherwise `settle_judged` (`run.rs:950-968`) spends the
     attempt.

   The release must come after the check. It removes the snapshot and its registration, after which a whole
   registration and a deleted one look alike, as D found for the legacy snapshot.
3. **An attempt's settlement of a failure assessed in its own slot** (`prior_failure`, `attempt.rs:963`, from the
   capture): before `settle_judged` spends the attempt, the check for the slot.

Round 8's "or once, at `run::verified`'s Git arm" is withdrawn. That arm never sees a gate's or a review's failure, nor
an attempt's.

**Frozen status, and what the frozen code receives.**
- **Changed, none of them frozen:** `src/workspace_manager.rs`, `src/engine/topology/attempt.rs` and
  `src/engine/topology/run.rs`. The PR11 record's R-D names `attempt.rs` and `run.rs` among the files that are not
  frozen.
- **Frozen and unchanged:** `src/engine/topology/integrate.rs`, `recover.rs` and `finalize.rs`. They receive a refusal
  through paths that already pass resumable errors on (`run.rs:289`, and `judge`'s `?`).
- **Effects.** Reading a file is not among `clippy.toml`'s disallowed methods: `clippy.toml:41-82` lists writes,
  creations, removals, renames, links and process starts. As in D's C-SIDE, one private reader per module and no new
  crate-visible function leave the effect census as it is. A crate-visible reader would need an `effects/wrappers.toml`
  row, which is an instrument (§7.9).
- **Frozen tests.** The check changes an outcome only where the registration is not whole at the check. A frozen test
  that fails a gate in a whole snapshot sees no change. That is reasoned; the implementer runs the frozen children
  unchanged first, as §1.4's "first measurement" says.

**What it mitigates:** face 2's false verdicts whenever a checked name is missing at the check: a verification's
deferral or park through a Git error, its `MergeRejected` through a gate or a review, and an attempt spent.

**What it leaves:**
- R-REWRITE and R-OUTSIDE, P1 where they occur;
- the slot's preservation (closure 2) and the store-gone finalization (closure 3);
- the liveness of each refusal. The next resume makes a fresh snapshot, so a verification converges; an attempt's slot
  goes to closure 2.

### 8.5 Closure 2, corrected: preserve at the destructive boundary itself (FUB-D8-C2MISSING)

**The destructive boundaries in face 2's reach** (the code at `5c222ff2`).
1. **`dispatch::verify_or_recreate`** (`src/engine/topology/dispatch.rs:242-256`). Any `VerifyFailure` removes the slot
   (`:251`) and adds it again (`:252`).
   - It is reached from `resume_open_no_attempt` (`:299-314`), and from the frozen `recover::recreate_open_no_attempt`
     (`src/engine/topology/recover.rs:1616-1627`).
   - Its quiescence is `AtBase` (`dispatch.rs:115-120`).
2. **A retained generation's retry** (found in round 9).
   - `settle::retry` verifies the slot with `HoldsTree` (`src/engine/topology/settle.rs:285-289`), and any failure
     closes the generation `WorktreeMissing` (`:297-300`).
   - The run then appends `GenerationClosed` and scrubs the slot (`src/engine/topology/run.rs:1459-1466`).
     `dispatch::scrub` (`dispatch.rs:337-344`) removes the worktree and its intent.

**The failures a registration loss produces there** (`WorkspaceManager::quiescence`,
`src/workspace_manager.rs:2763-2821`).
- **`NotRegistered`** (`:2768-2770`): the registration was gone when the store was listed.
- **`Missing`** (`:2780-2784`): gone after the listing and before `common_git_dir` (`:6330-6345`).
  - Its `rev-parse --git-common-dir` then exits 128. Executed: the whole-entry state, and the `HEAD`, `commondir` and
    `.git` states, 9 of 9 each.
  - The other `Missing` arms (`:2774-2779`) are a checkout with no directory, or with no `.git` pointer, which no prune
    makes.
- **`TreeMismatch`, under `HoldsTree` only** (`:2812-2817`): gone after `common_git_dir` and before `diff-index`.
  - Both `diff-index --cached --quiet <tree> --` and `cat-file -e <tree>^{tree}` then exit 128 (executed: the
    whole-entry state, 9 of 9).
  - `index_differs_from` then answers "that tree is not an object in this repository" (`:2846-2866`).
  - With `index` alone gone, `diff-index` reports a difference too (reasoned from the executed `status` row).
- **Not reachable by a deletion alone:**
  - `Unpopulated`, which needs a `locked` file;
  - `ForeignRepository`: without `commondir` the entry is not a git directory at all (executed);
  - `Residue`: a deletion removes names and adds none;
  - `HeadMismatch`: a failing `rev-parse HEAD` is an error, through `git_line`'s `?` (`:2790`), not a failure.

**Executed at the Git level, the whole-entry state** (`partial.jsonl`, 9 of 9). Git's own
`git worktree prune --expire=now` deleted the entry, with the checkout moved aside for the prune and moved back.
- The entry was gone, and another registration kept the store.
- The checkout was not listed, and its `.git` file and its edits were kept.
- Every command in it exited 128.

The reviewers' late-delete witness shows the same through `common_git_dir`
(`reviewer-d8/pr329-d8-late-delete-b46jf2ik.results.json`). The loss itself is reasoned from the engine.

**The rule, at the boundary itself.**
- **Where:** at (1) immediately before `remove_worktree`, and at (2) before the close and the scrub.
- **When:** the failure is `NotRegistered`, `Missing` or, under `HoldsTree`, `TreeMismatch`; §8.4's check does not
  find the registration whole; and the slot's directory holds anything.
- **What:** nothing is removed. The answer is a resumable refusal that names the slot and keeps it for the operator,
  and a retained generation is not closed.
- **Why the check is part of it.** A genuine `TreeMismatch` in a whole registration keeps master's close and scrub,
  so closure 2 costs no liveness there. `NotRegistered` and `Missing` from a deletion always fail the check.
- **What it inherits.** A `TreeMismatch` that a deletion caused, met with the check passing, is §8.4's R-REWRITE: the
  slot is then closed and scrubbed as on master. A missing `sharedindex.*` makes `diff-index` exit 128 while
  `cat-file -e` succeeds, which `index_differs_from` returns as an error, not a failure (`:2852-2861`), so R-OUTSIDE
  removes nothing there.

**Its cost is liveness.** The run stops at that slot on every resume until the operator acts. Git cannot re-register
the checkout: `git worktree repair` exits 1, and `git worktree add` over it exits 128 (d8's face-2 runs). A populated
slot with no `.git` pointer (a `Missing` no prune makes) is kept too, where master recreated it and deleted its
contents.

**The frozen test it might have met stands.**
- `a_resume_over_a_torn_open_generation_recreates_its_worktree` (`src/engine/topology/recover/tests.rs:25520-25571`)
  plants a populated checkout, tears its registration, and requires the resume to recreate it with `NotRegistered`.
- The verification's own repair removes that checkout with its torn registration before the boundary
  (`src/workspace_manager.rs:2744-2748`, `:5328-5379`). It takes only an entry whose `commondir` holds no bytes
  (`:5366-5367`), which a prune's unlinking never leaves.
- So at the boundary the slot is already empty, and the test is unaffected.
- No other frozen test child deletes a populated slot's registration. A search of `recover/tests.rs`,
  `integrate/tests.rs`, `repair/tests.rs` and `finalize.rs` at `5c222ff2` finds only `tear_registration`.

**Frozen status.** `dispatch.rs`, `settle.rs`, `run.rs` and `src/workspace_manager.rs` are not frozen. The frozen
`recover.rs` receives the refusal through its `?` and is unchanged.

**The larger form is not proposed:** re-registering the checkout, or moving the work aside and adding afresh.
- Git has no command for re-registration, and the engine would write Git's internal layout.
- Moving aside needs a place no walk reclaims, and a new effect for the governance census, which is an instrument.

**With #330's U.** Under U, #330's recommended D2, a fresh-process resume recreates every open generation's slot under
its own incarnation, and reclaims the earlier instance whatever its registration (#330's record §4.14, E-FUC-3's
T-DISPATCH item, and §4.3).
- Across a resume the loss at (1) is then U's by design, not face 2's.
- Within a live run, `verify_or_recreate` still decides, and so does (2). Closure 2 applies there.

**What it closes:** face 2's removal of a populated slot's contents by recovery or by a retained retry, and the
durable `WorktreeMissing` close of a retained generation whose registration a prune deleted. **What it leaves:** the
operator's step for each kept slot.

### 8.6 Closures 3, 4 and 5

**Closure 3, unchanged: let a store-gone reclaim converge.**
- As §7.4: the removal scan's store-absent branch (`src/workspace_manager.rs:5144-5164`) binds nothing for a contained
  target whose `.git` names an entry in this repository's own absent store, and `remove_bound` removes it.
- It changes the pinned refusal `a_missing_stored_worktree_directory_refuses_before_checkout_deletion`
  (`src/workspace_manager/tests.rs:6952`). Neither file is frozen, and the frozen `finalize.rs` is unchanged.
- **It needs closure 2 first,** so that no populated slot a recovery should keep is removed. At terminal finalization a
  slot's contents are removed by design, with or without a store.
- It binds removal to the instance's own entry and adds no prune, so #330's R-P dependency holds.

**Closure 4, qualified: environmental requirements** (FUB-D8-C4SCHED). It reduces the automatic starters. It excludes
none of them, and no configuration stops an explicit prune.
- **Command-triggered automatic maintenance** runs after a commit, a fetch or a merge, and as `git gc --auto`.
  - `maintenance.auto=false` stops the launch, `gc.auto=0` the gc task, and `maintenance.worktree-prune.auto=0` the
    `worktree-prune` task on 2.50.1 and later.
  - Each holds only where it is the **effective** value for the command that starts maintenance.
- **Effective configuration is per checkout and per process.**
  - A checkout's `config.worktree`, under `extensions.worktreeConfig`, overrides the repository's shared configuration.
    `-c` and `GIT_CONFIG_*` override both.
  - Executed by the reviewers on 2.55.0 (`reviewer-d8/pr329-d8-config-override-i2cdaomh.results.json`): with all three
    settings in the shared configuration, a sibling checkout's `config.worktree` enabled `maintenance.auto` and
    `maintenance.worktree-prune.auto`. An ordinary commit there started maintenance, and its prune deleted the add's
    entry.
  - A preflight can read the shared configuration and every existing checkout's `config.worktree`. It cannot see a
    later checkout's, or a process's own.
- **Scheduled maintenance is a starter of its own.** The system scheduler runs `git maintenance run --schedule=<f>`
  for every registered repository.
  - A task's `*.auto` condition is read only with `--auto` (2.55.0 `builtin/gc.c:1771`; 2.43.0 `:1348`; 2.50.1
    `:1654`), and `--auto` and `--schedule` are exclusive (2.55.0 `:2065`). So no `*.auto` setting stops it.
  - It prunes when its task set includes a prune.
    - On 2.43.0 and 2.50.1, a `maintenance.gc.schedule` is enough: the gc task is enabled by default (2.43.0
      `:1285-1289`, 2.50.1 `:1568-1572`), and a run keeps an enabled task whose schedule is due (2.43.0 `:1345-1353`,
      2.50.1 `:1651-1659`). On 2.50.1 a `worktree-prune` task given `enabled` and a schedule prunes too.
    - On 2.55.0 a scheduled run starts from no tasks (`:1970-1972`). It prunes under `maintenance.strategy=geometric`,
      whose `worktree-prune` runs weekly (`:1916-1919`), or `gc`, whose gc task runs daily (`:1846-1853`), or for a
      task given `enabled` and a schedule (`:1981-2000`).
  - `git maintenance start` writes `maintenance.strategy=incremental` when none is set (2.55.0 `:2125`), and that
    strategy schedules neither (`:1855-1892`; Git's documentation, `Documentation/config/maintenance.adoc:31-48` at
    2.55.0).
  - The reviewers executed `--schedule=weekly` with the geometric strategy and all three settings: it pruned.
  - What stops it, where effective: `maintenance.worktree-prune.enabled=false` and `maintenance.gc.enabled=false`,
    which drop the task from scheduled and manual runs alike (2.55.0 `:1985-1990`); or a repository not registered for
    maintenance.
- **A prune already running.** A prune or maintenance process reads its configuration when it starts. One that decided
  before the settings were set, or before a preflight read them, keeps running and deletes.
  - Executed by the reviewers, with the settings all off before the deletion: `git worktree prune` on 2.43.0 and
    2.50.1, and `git maintenance run --auto` on 2.55.0 (`reviewer-d8/pr329-d8-late-delete-b46jf2ik.results.json`).
  - Git's own markers cover only some starters. `git maintenance run` holds `objects/maintenance.lock` (2.55.0
    `:1791`), and `git gc` writes `gc.pid` (`:736`). An explicit `git worktree prune` holds neither.
- **How it would be applied:** as documentation of the requirement, and at most a preflight, in the non-frozen
  topology preflight, that refuses a run whose readable effective values are wrong. Writing the user's configuration
  is not the engine's (Q6's untouched user checkout).

**Closure 5, unchanged: the owner's ruling on scope,** per face and per starter, reflected in `DESIGN.md` and in the
G6 assessment. It repairs nothing.

### 8.7 The host-width reconciliation (FUB-D8-HOSTWIDTH)

Round 8's reconciliation in the host-agent finding said that refusing `max_parallel > 1` with the host runner "also
removes this starter". It does not.
- Run A, at width one in one linked checkout, runs a host agent. Run B, at width one in another checkout of the same
  repository, adds a registration. The per-checkout locks admit both, and A's agent's prune meets B's add in either face.
- **Only isolating the agent's Git view removes the starter:** the container runner's disposable view
  (`design/26_design_merge_queue_protocol.md`), or an equivalent on the host. Refusing width above one with the host
  runner removes only the overlap inside one run.
- `PR11-HOST-AGENT-PRUNE-RACES-AN-ENGINE-ADD`'s reconciliation is corrected in place.

### 8.8 R14 needs the owner's disposition before G6 (FUB-D8-R14G6)

**R14 stays P2.** Its demonstrated consequence is a resumable stop, with no wrong candidate disposition. **But it
narrows specified semantics, so G6 cannot certify around it.**
- `design/26_design_merge_queue_protocol.md:617-622`: a later pass's snapshot failure on an integration "settles the
  sequence unavailable rather than ending the command".
- `:507-508`: "at `max_defers = 0` every integration outage parks rather than defers".
- Under §7.3, a genuine snapshot failure after the takeover ends the command resumably (`run.rs:289`). The verification
  is settled interrupted, and every resume meets it again without reaching `max_defers` or parking the candidate.
- The frozen integration test that proves defer-then-park for an outcome delivered as unavailable
  (`infrastructure_failure_defers_then_parks_at_max_defers`, `src/engine/topology/integrate/tests.rs:1171`, the
  regression lens's citation) would keep passing without proving that a genuine snapshot failure still reaches that
  outcome.

**The G6 row.** It applies (Q6's verification semantics, and progress), and **it needs the owner's disposition before
G6**. The owner either accepts the narrowing, and `design/26` is amended in the change that implements it, or requires
a closure that keeps the specified outcome for a genuine failure. Such a closure must tell a genuine failure from a
prune's deletion without reading the registry. The R14 file is updated in place.

### 8.9 What is closed, what remains, and what G6 meets

**Claim 1, exact** (replacing §7.7's claim 1). Once implemented, **no manager registry access returns
`UpstrokeError::Git` for anything the registry's state caused.**
- That covers contention, a write a dead process left torn, a registration nobody is writing, and a prune's deletion
  during an add that makes the add fail.
- An add returns Git state only when its destination cannot be made, or is not an empty directory when the access
  begins. **In both cases no Git command runs,** so no registry state reaches the answer (§8.2, executed).
  *Qualified at repair round 3 (§8.2's note, §9.13): no Git command runs from the add's access onward, once the gate's
  list has passed; a store that list fails on refuses as the registry's.*
- A registration deleted after the add's own writes is outside every access. That is face 2.

Claims 2 to 7 stand as §7.7 left them.

**What remains.** §7.7's table stands, with these rows replaced.

| | What | Consequence now | Finding |
|---|---|---|---|
| R2″ | A host agent's own Git | Its prune is the external-prune class: face 1 refuses resumably, and face 2 has the class's consequences. Width one per run does not remove it (§8.7) | the P1; `PR11-HOST-AGENT-PRUNE-RACES-AN-ENGINE-ADD` (P2, reconciled) |
| R3″ | The user's or an IDE's Git; Git's automatic maintenance after a commit, fetch or merge; **scheduled maintenance; a prune already running** | as R2″. No configuration excludes the explicit or the scheduled starters (§8.6) | the P1 |
| R14 | A genuine checkout failure after the takeover | a resumable refusal after one attempt, narrowing `design/26:617-622` and `:507-508` | `PR329-A-GENUINE-CHECKOUT-FAILURE-AFTER-THE-TAKEOVER-REFUSES` (P2); **the owner's disposition before G6** |

**The cases, and what G6 meets** (replacing §7.7's table).

| Case | Closed by | Severity | Applies to G6 | Blocks G6 |
|---|---|---|---|---|
| (a) An attempt's pipeline error | §5.3, §5.4, §7.2 and §8.2, in the non-frozen `src/workspace_manager.rs` and `src/error.rs`, for contention and torn residue that clears by the deadline | P1 | yes: R17, the shared registry, Q6 | until implemented and validated |
| (b) A durable verification deferral or park | the same. No registry state reaches `run::verified`'s Git arm through an access, the populated destination included (§8.2). Face 2 reaches it outside the access: below | P1 | yes: Q6 and durable verification | until implemented and validated |
| (c) DESC | filed, follow-up C | P1 | yes: Q1, INV-22, ST-16, ST-18 | yes; filing is no waiver |
| (d) A legacy writer tears a topology reader | §5.3 | P1 class | yes | until implemented and validated |
| (e1), (e1′) Legacy against legacy | follow-up D | P1 | no | no; each remains a P1 until D lands |
| (e2) A topology writer tears a legacy reader | follow-up D, through §5.5 as dated, with §7.2's veto | P1 | yes: Q6, across the shared registry and R17 | yes, until follow-up D is implemented and validated |
| (e2′) A topology writer's static or deadline residue, then a legacy discard | follow-up D | P1 | yes: Q6; a crash producer engages Q1 | yes, until follow-up D is implemented and validated, unless the owner rules otherwise |
| **The external-prune class, face 1:** the deletion fails an add after Git took its destination over | §7.3: a resumable refusal at once | P1 class; this face alone is liveness | yes | through the class |
| **The external-prune class, face 2:** the add returns Ok, and its registration is deleted, wholly or in part, at any time from the prune's decision onward (§8.3) | **not closed:** `PR329-AN-EXTERNAL-PRUNE-DELETES-AN-ENGINE-WORKTREES-REGISTRATION`, its closures made accurate (§8.4 to §8.6; closure 1 a partial mitigation with two P1 residuals), with a recommendation (§8.11) | P1 | **yes:** Q6 and R17 (a valid candidate's verification becomes a deferral, a park or `MergeRejected`; an attempt is spent), Q1 (recovery, or a retained retry, removes a populated slot), ST-18 (finalization does not converge with the store gone), INV-22 | **yes, until closures are implemented and validated for each applicable consequence and the owner rules on what they leave (closure 1's residuals at least), or the owner rules on scope per face and starter. Filing is no waiver, and no combination of closures clears the class** |
| R14: a genuine checkout failure after the takeover refuses | filed as `PR329-A-GENUINE-CHECKOUT-FAILURE-AFTER-THE-TAKEOVER-REFUSES` | P2, liveness; no durable outcome | yes: Q6's verification semantics and progress | **needs the owner's disposition before G6** (§8.8) |

**The findings.** As §7.7's table, with two rows changed:
- `PR329-A-GENUINE-CHECKOUT-FAILURE-AFTER-THE-TAKEOVER-REFUSES` (R14): applies; needs the owner's disposition before G6;
- `PR11-HOST-AGENT-PRUNE-RACES-AN-ENGINE-ADD`: reconciled; its prune faces are the P1's, and width one does not remove
  its starter (§8.7).

### 8.10 Regression tests

§7.8 stands, with T15 and T16 revised.

**T15, a genuine failure through the add** (FUB-D8-T15).
- FUB-D5-GENUINE's tree, and a 300-byte name: `RegistryRefused` after one attempt, with nothing at the slot.
- **FUB-D6-INODE's two shapes (Unix): `RegistryRefused` after one attempt, and the slot holds the empty directory the
  access made.**
  - Its parent cannot be written, so neither Git's junk removal nor the access's own removal (§5.3) can take it.
  - Round 8's saved runs show exactly that: `destination_after` "dir-empty" in 9 of 9 runs per shape, against "absent"
    in 9 of 9 for the genuine tree and for the long name (`d8/witness/scenarios-*.jsonl`, counted in
    `FIGURES-d9.txt`).
  - The test then restores the parent's permission, and the slot's forced removal takes the empty directory, which
    binds nothing (§5.3).
- The verification bullet and mutation m10 stand.

**T16, the destination's lifecycle,** with §8.2.
- A directory holding entries at the start, a file, and a link to an empty directory are each Git state with no Git
  command run. The error names the destination.
- **The reviewers' construction:** a populated destination beside a sibling torn by the fixture's `tear_registration`
  (an empty `commondir`). The answer names the destination, not the sibling.
- **Mutation m14,** Git attempted once as round 8 had it: red, because the error then names the sibling's
  `commondir`. That is executed at the Git level: §8.2's first row.
- The other bullets stand.

**For the closures,** in whichever change implements them, not in #329: at each boundary of §8.4 and §8.5, a
whole-entry construction and a one-name-removed construction for each of `HEAD`, `commondir`, `index` and `gitdir`,
built with Git commands and file removal as `d9/witness/d9partial.py` builds them, with a mutation that skips each
check; and the two residuals pinned as residuals, so that a later change sees them move: a split index's
`sharedindex.*` removed (`d9splitindex.py`'s construction) and a removed `index` written again by `git read-tree HEAD`
before the check (#331's construction).

### 8.11 The owner's question: the options, and the recommendation

**What face 2 leaves, by consequence.** Face 1 is already a resumable refusal.

| Consequence | What addresses it, and how far |
|---|---|
| a verification's deferral or park (a Git error), its `MergeRejected` (a gate or a review), an attempt spent | closure 1 (§8.4), **a partial mitigation:** it catches every deletion that leaves a checked name missing at the check, and leaves R-REWRITE and R-OUTSIDE (P1 where they occur) |
| a populated slot removed by recovery, or by a retained retry that also closes the generation | closure 2 (§8.5), with closure 1's check; a `TreeMismatch` a deletion caused under R-REWRITE is left |
| finalization that never converges with the store gone | closure 3 (§8.6), after closure 2 |
| a legacy run's discard of paid output, in a mixed run (#331's R-D9) | D's C-SIDE, the legacy form of closure 1 (#331's record §3.4), evaluated there and not adopted; a partial mitigation with the same residuals (#331's review round 3, FUD-D3-CSIDEPROOF) |
| the starters themselves | closure 4 reduces the automatic ones where it is effective; nothing excludes an explicit or a scheduled prune; isolating a host agent's Git view removes that starter (§8.7) |

**No combination of closures 1 to 4 clears the class.** Closure 1's residuals keep P1 cases open whichever closures are
chosen. So every option below needs the owner's ruling, and each says what that ruling must cover.

**The options, each with its exact scope and what it leaves open.**
- **Option A, recommended: closures 1, 2 and 3 in one new follow-up before G6, closure 1 as a partial mitigation,
  closure 4 as documentation, and a scope ruling for what they leave.**
  - **Scope:** §8.4's check (files only) at its three boundaries, §8.5's rule at its two, and §8.6's store-absent
    branch. The code is in `src/workspace_manager.rs`, `attempt.rs`, `run.rs`, `dispatch.rs` and `settle.rs`, none of
    them frozen. It needs no instrument while the readers stay private. One pinned, non-frozen test changes
    (closure 3).
  - **D's C-SIDE** is the same check for the legacy engine, D's to adopt under decision B.
  - **The ruling it needs:**
    - a prune no engine process starts may stop a run resumably, or leave a kept slot for the operator. G6's claims
      for it are safety, not liveness;
    - **closure 1's residuals are accepted, by name:** R-REWRITE (a removed `index` written again by a Git command in
      the checkout before the check) and R-OUTSIDE (a split index's `sharedindex.*`, or worktree configuration, or a
      sparse checkout's file, removed first). Where they occur, a valid candidate can still be rejected, deferred or
      parked, or an attempt spent, and a retained slot still scrubbed.
  - **It leaves open:** those residuals; the liveness costs; Windows and macOS, reasoned only; and the interplay with
    #330's U at closure 2's first boundary, which is C's.
- **Option A′: option A with a Git read of the index in closure 1's check** (`git ls-files --stage` in the checkout,
  §8.4). It removes the split-index part of R-OUTSIDE (executed: the read exits 128 without the shared index) and
  leaves R-REWRITE and the configuration files. It costs a Git child at each negative outcome and an instrument edit,
  a manager entry point's `effects/wrappers.toml` row. Take it if repositories with `core.splitIndex=true` are to be
  covered.
- **Option B: closures 1 and 3 only.** It leaves recovery's and a retained retry's removal of a populated slot (Q1), and
  the retained generation's durable close, besides closure 1's residuals. Its ruling must accept all of them.
- **Option C: closure 5 alone.** The class is ruled outside G6's claims per face and starter, with closure 4
  documented. It leaves every durable consequence above, whenever a starter fires.
- **Explicit user prunes, in particular:** under option A they cost liveness, except in closure 1's residuals. Under B
  or C they keep their durable consequences, and the ruling must say so.

**Why A.**
- It is the smallest set that turns most durable wrong outcomes of face 2 into a resumable stop or a kept slot, and
  those outcomes are the class's P1 content. What it cannot reach is stated, by name, for the ruling.
- Each closure sits in non-frozen code, at a boundary the engine already has, and adds a read, not a mechanism.
- What no check can do is see a file written again, or stop a prune that no engine process starts. That is what the
  ruling is for.

**R14 (§8.8)** goes in the same question: accept §7.3's narrowing and amend `design/26`, or require a closure.

### 8.12 Design review round 8, answered

| Finding | Sev | Kind | Round 9 | Where | Evidence |
|---|---|---|---|---|---|
| FUB-D8-C1PARTIAL | P1 | executed (all three) | **Fixed (design), as a partial mitigation:** the check reads `gitdir`, `commondir`, `HEAD` and `index` and resolves the two pointers, so every state with a checked name missing is caught; after the orchestrator's addendum, its residuals R-REWRITE and R-OUTSIDE are stated, P1 inside the class, for the owner's ruling | §8.4 | `partial.jsonl`: each name removed alone, four commands, the three versions, 9 runs per name; `splitindex.jsonl`: 9 of 9; #331's executed recreation |
| FUB-D8-C1PLACE | P1 / P2 | reasoned (all three) | **Fixed (design):** the check at the manager's Git errors, at the judge's gate and review verdicts before the release, and at an attempt's settlement; the Git-arm-only placement withdrawn | §8.4 | the code at `5c222ff2` |
| FUB-D8-DURINGADD | P1 | executed (concurrency) | **Fixed (design), witnessed:** face 2 starts at the prune's decision; the reviewer's witness cited, and the claim re-executed with Git's own hook point | §8.3 | `duringadd.jsonl`: the add rc 0, the prune ordered by trace2, `status` 128, 18 of 18 |
| FUB-D8-C2MISSING | P1 | the Git level executed (concurrency); the loss reasoned | **Fixed (design):** preservation at the boundary itself, for `NotRegistered`, `Missing` and `HoldsTree`'s `TreeMismatch`; a second boundary found (a retained retry) | §8.5 | `partial.jsonl`'s whole-entry state, 9 of 9 |
| FUB-D8-POPULATED | P2 / P3 | executed (all three) | **Fixed (design), witnessed:** rejected before any Git runs (qualified at repair round 3: after a successful prevalidation, before `git worktree add`, §8.2); the link case corrected too | §8.2 | `populated.jsonl`: r9 Git after 0 with no Git process in every shape; r8 Git after 1 with the sibling's error |
| FUB-D8-C4SCHED | P2 | executed on 2.55.0 | **Fixed (design):** scheduled maintenance added as a starter; effective per-checkout and per-process configuration, and a prune already running, qualified | §8.6 | `citations-d9.txt`; the reviewers' witnesses |
| FUB-D8-R14G6 | P2 | reasoned | **Fixed:** R14's G6 row needs the owner's disposition before G6 | §8.8 | `design/26:617-622`, `:507-508` |
| FUB-D8-HOSTWIDTH | P2 | reasoned (design) | **Fixed:** width one does not remove the starter; only an isolated Git view does | §8.7 | — |
| FUB-D8-T15 | P3 | executed evidence | **Fixed:** the INODE shapes leave the empty directory | §8.10 | `scenarios-*.jsonl`: 9 of 9 per shape |

### 8.13 Risks, sequencing, siblings, and out of scope

**Risks,** beyond §7.11's:
- **closure 1's residuals,** R-REWRITE and R-OUTSIDE (§8.4), P1 where they occur, under any option;
- **the kept slots** of closure 2, which need the operator;
- **Windows and macOS are reasoned, not executed,** for every §8 witness. Directory order there is the filesystem's
  (NTFS sorts names), and §8.2's link case maps to a reparse point.

**Sequencing.**
- **This is the last design round of #329 before the owner's consolidated question.** Any P1 from its review goes into
  that question as an open text item. No tenth round runs without the owner.
- Whichever closures the owner chooses are a change of their own, after the owner's answer and before G6.

**Siblings.**
- **#330 (C):** its R-P dependency holds. Every engine prune stays deleted, removal stays bound to the instance, and no
  closure adds a global prune. Closure 2's interplay with U is stated in §8.5; U's choices are C's.
- **#331 (D):** the helper's contract does not change (§5.5's dated note). D's R-D9 refers to the finding, whose legacy
  paragraph keeps D's account: in a mixed run the legacy face is the discard of paid output, and D's C-SIDE is its
  closure-1 form.

**Out of scope, and said so:** as §7.11.

## 9. Implementation

**Who and on what.** `pr11_fub_impl` (`claude-opus-5-5`, `max`), a fresh implementer `orch_pr11` spawned on
2026-10-03 on `8df42436`. Its brief is `~/orch-pr11/briefs/pr11_fub_impl.md`, with two addenda
(`~/orch-pr11/answers/pr11_fub_impl-0.md` and `-1.md`), and its scope is `~/orch-pr11/b-impl/UNIT.md`'s unit. Its
figures are under `~/orch-pr11/logs/pr11_fub_impl/`, cited as `impl/…`. It implements §5 to §8 as design review round 9
left them: the round's three general lenses found that "B's own narrowed mechanism holds" in all three, and every P1
they returned is in the external-prune finding's candidate closures (`~/orch-pr11/reviews/review-329-d9-triage.md`).

### 9.1 What it implements, and what it does not

**In:**
- the tolerant registry access (§5.3, §5.4), with the typed refusal `UpstrokeError::RegistryRefused`;
- the helper's contract as §5.5 dates it, with §7.6 as its current text: the three-way veto, the final attempt, the
  runtime allowance and `CONTENDED_ATTEMPTS`;
- the add's veto with the removal proof and no probe (§7.2), a failure after the takeover as `Undecidable` (§7.3),
  and a destination that is not an empty directory rejected before `git worktree add` runs, after a successful
  prevalidation (§8.2, as qualified at repair round 3, §9.13);
- targeted removal with no engine prune (§2.5, §3.5, and §5.5's note for follow-up C), and round 6's store-absent
  branch (§5.3, T17);
- the one instrument row (§7.9), B's `design/15` paragraph in force, and FUB-D9-TAKEOVERWORD's text correction;
- **from addendum 1** (`~/orch-pr11/briefs/followups/fu-b-impl-carryover.md`): `LinkedChild::kill` and its `Drop`
  bounded (#328's P3 `PR328-LINKED-CHILD-KILL-WAITS-WITHOUT-A-DEADLINE`), and B's two-process tests run through it.

**Out, and unchanged:** the external-prune finding's closures 1 to 5 and any preservation change (follow-up F); any
change to a Git child's environment (ENV-1, option (C), ENV-R: O3 and O3-R); any `design/26` text (R14's is O9's, the
forgotten spend's O14's); anything of follow-ups C and D; and every frozen file (§9.8).

### 9.2 Provenance: a merge, not a rebase

The brief asked for a rebase onto master `5c222ff2` (follow-up A, #328, merged) and a re-stamp of any SHA the rebase
orphaned. Addendum 1 withdrew the re-stamp: a `reviewed_sha` is the commit a finding was found at, and stays.
- **The rebase was made and verified** (`impl/rebase/rebase.txt`: `8df42436` → `a0daa334`).
- **But it orphans 8 of the 9 distinct reviewed SHAs** this pull request's ledger cites (`0874bcf3`, `4a126215`,
  `85f5b09b`, `8dd2214c`, `a6135a66`, `dfd69410`, `ed3a97d9`, `f7a9256c`; only `92c4ca81` stays an ancestor), and
  `validate-pr-ledger-evidence.sh` refuses a row whose reviewed SHA is not an ancestor of the head
  (`impl/rebase/merge-instead.txt`). `MAINTAINING.md` takes merge commits only, for that reason.
- **So the branch takes master by a merge:** `33ea257f` (parents `8df42436` and `5c222ff2`), whose tree `f0a9c723` is
  the rebased head's tree, byte for byte (`impl/rebase/merge-instead.txt`). The push is then a fast-forward of
  `8df42436`. No pin is re-stamped. The findings filed here carry the commit they were found at: `8df42436` for
  FUB-D9-WINPUBLISH and FUB-D9-RELPOINTER, `5c222ff2` for O3-R and INV-07/L13.
- **The commits:** the implementation, `58c7c203`, on the merge; this record's implementation text and the findings in
  the commit after it, `59d206b3`. Repair round 2 adds two test-only commits, `b5a39149` and `8364009d`, and then its
  record and findings commit (§9.12).
- **Where the proof ran.** The mutation campaign and the base-witness port ran on a work-in-progress commit, `d354ba37`,
  on the rebased `a0daa334`. Its `src/`, `effects/`, `design/`, `docs/`, `Cargo.toml`, `Cargo.lock`, `clippy.toml`,
  `README.md` and `MAINTAINING.md` trees are those of `58c7c203`, and neither has a `build.rs`
  (`impl/rebase/code-commit-identity.txt`). Round 1's head, `59d206b3`, differed from `58c7c203` only under `reviews/`
  and `findings/`, under which no test opens a file. Repair round 2 changes test code in two files, and §9.12 says what
  it re-ran on that code.

### 9.3 What changed, per file

**`src/error.rs`.** `RegistryRefused { message }` (`:156`), displayed as its message. Its internals note
(`docs/internals/error.md`) says why it is a variant of its own: `run::verified` maps only `Git` to `Unavailable`.

**`src/workspace_manager.rs`.** Not a noted module; its doc comments carry its notes.
- **R-X** (`:1614-1623`): this process's registry lock, now a `RwLock` per common git dir. `RegistryHold` (`:1628`)
  says how an access takes it: an add `Shared`, the torn-registration plan `Exclusive`, a list, a scan and a removal
  `Unheld`.
- **The contract** (`:1641`): `Again::Attempt`, `Return` and `Undecidable { why }`. `Return` is the contract's third
  answer, published for follow-up D's legacy add. The topology's own accesses never return Git state, so it is
  constructed only by the contract's witnesses, and an `expect(dead_code)` with that reason marks it outside tests.
- **The deadline** (`:1671`): 10 s, and 500 ms under test (`:6881`, at the file's end beside the other seams' test
  twins). The backoff starts at 1 ms and doubles to 50 ms (`:1675`).
- **The helper** (`tolerant_registry_access`, `:1783`, with `with_registry` at `:1690`): attempt; on failure ask the
  veto once, outside R-X; `Return` returns the failure unchanged; `Undecidable` refuses at once naming why; `Attempt`
  counts the answer, refuses if the deadline has passed, and otherwise sleeps the backoff, cut at the deadline. So the
  attempt after a sleep the deadline cut short is made, at the deadline, and is the last. Every refusal names the
  store, the attempt count and the last failure; the deadline's also names the deadline, and `Undecidable`'s says why.
  A wait for R-X that reaches the deadline refuses, and no further attempt runs (`registry_lock_refusal`, `:1837`). Nothing interrupts an attempt or a veto that has started, so an access
  returns by the deadline plus its last attempt's runtime plus the veto's (§7.6, R8″).
- **The add's destination** (`AtDestination`, `:1861`; `Destination`, `:1919`):
  - `prepare` (`:1936`): an empty directory is used as it is; an absent one is made; anything else, or a destination
    that cannot be made or read, is Git state naming it, before `git worktree add` runs (§8.2; since repair round 3
    the message says so, §9.13);
  - `untouched` (`:1959`), the veto: an empty directory the access can remove and make again answers `Attempt` (§7.2's
    removal proof); a destination gone, holding anything, unreadable, or not removable or not makeable again answers
    `Undecidable` (§7.3);
  - `settle` (`:1998`): a refused add removes the destination it made while that is still an empty directory (§5.3).
- **`add_worktree`** (`:3078`): Git's add is the funnel's attempt, in an access holding R-X shared, with the
  destination's veto.
- **Removal** (`remove_worktree_proving`, `:3441`; `remove_bound`, `:3469`): the scan is an access; the removal then
  deletes the checkout, and the registration the scan bound to this slot directly: its `locked`, then its directory
  through `remove_tree_once_handles_close`, then the store only when that leaves it empty. **No `git worktree prune`
  anywhere, and no global-prune fallback.**
- **The store-absent branch** (`revalidate_removal_proving`, `:5548`): an empty directory at the target binds nothing
  (§5.3's T17). A target whose `.git` names an entry in the absent store still refuses.
- **The other accesses:** `worktree_records` (`:5479`), and the torn plan (`slots_with_torn_registrations`, `:5793`,
  over `torn_plan`, `:5806`) under R-X alone.
- **The test handshake:** `CONTENDED_ATTEMPTS` and `contended_attempts` (`:6851`, `:6856`), counting `Attempt`
  answers per common git dir, test-only.

**`effects/wrappers.toml`.** The one row: `tolerant_registry_access` in `src/workspace_manager.rs`'s `effect_free`
list (`:176`). Under `CLAUDE.md`'s first limb it is an instrument edit, and the body says so (§5.8). No other
instrument moves (§9.7).

**`src/workspace_manager/fixture.rs`** (test-only).
- `COLLECT_BOUND` (`:3027`, 10 s), `LinkedChild::kill` (`:3173`) and `kill_within` (`:3182`), and `LinkedChild`'s
  `Drop` (`:3254`). A killed child is reaped within the bound or the test fails, naming what the kill answered and
  carrying the child's stderr. The drop does the same, saying it on stderr instead of panicking while its thread
  already unwinds, and bounds its wait for the stdout reader too.
- `set_mode` (`:517`) and `ModeRestored` (`:526`), for the permission constructions.
- `note_removal_attempt` (`:123`) keeps the most attempts one removal made. A forced removal now removes two trees
  (the checkout and its registration), each in one attempt, and a count of the last would have read the second.

**`src/workspace_manager/tests.rs`.** The two tests §3.6 said the implementation moves:
`a_removal_records_the_one_attempt_the_unix_arm_makes` (two removals, one attempt each) and
`an_add_killed_before_it_wrote_gitdir_is_unlisted_and_refuses_forced_cleanup` (`RegistryRefused` after the deadline,
carrying the same Git text). The rest is new, from `:14855` (§9.4). Repair round 2 adds `as_git_writes_it`, and
repair round 3 moves it into the fixture module, for the coordinator's tests too (§9.13). Every registration this
change's tests plant spells its paths as Git writes them through it: true since repair round 3, when the coordinator's
`TearsAForeignRegistration` was the last plant brought to it (§9.12's note).

**`src/engine/topology/coordinator.rs`.** Inside its `mod tests` only: the four verification tests (§9.4) and their
review-input policies and hooks. Its production region is unchanged (`impl/basewit/`'s port asserts it). Repair round 2
arms the three one-shot policies with `compare_exchange`, not `fetch_update` (§9.12). Noted module: the tests' notes are
in `docs/internals/engine/topology/coordinator.md`.

**`src/engine/topology/run/tests.rs`.** The notes pin, renamed
`the_verification_notes_say_a_registry_another_process_is_writing_never_reaches_the_git_arm` (`:535`), pins the
rewritten Git-arm notes (`docs/internals/engine/topology/run.md`) and checks that `verified` passes
`RegistryRefused` on as an error. `run.rs` is unchanged.

**`design/15_design_event_log_resume_run_layout.md`.** The paragraph "A registry another process is writing" is in
force (`:66`), with FUB-D9-TAKEOVERWORD's sentence (`:92`) and the final attempt's wording.

**This record.** FUB-D9-TAKEOVERWORD: §6.3's heading reads "Why case 1 is safe to attempt again", as the decision
appendix's §11 gives it, with a dated correction note after the sentence it corrects, and §7's banner quotes the new
heading. §0's last row, the authors' paragraph, and this section.

### 9.4 The tests, by the record's numbers

`WM` is `src/workspace_manager/tests.rs`, `CO` `src/engine/topology/coordinator.rs`. "Unix" and "Linux" are
`#[cfg(unix)]` and `#[cfg(target_os = "linux")]`.

| T | Test | Where | Runs on |
|---|---|---|---|
| T1 | `two_coordinators_in_two_checkouts_of_one_repository_never_fail_on_each_others_registry_writes`: two processes, 500 cycles each (1,000), 0 failures | WM `:17377` | all; 150 cycles each on Windows |
| T2 | `a_verification_beside_another_processs_registration_write_in_flight_spends_no_deferral` | CO `:4169` | all |
| T2′ | `a_verification_beside_a_registration_that_stays_torn_ends_resumably_and_its_resume_reverifies` | CO `:4207` | all |
| T3 | `three_coordinators_in_three_checkouts_of_one_repository_never_fail_on_each_others_registry_writes`: three processes, 340 cycles each | WM `:17388` | all; 100 each on Windows |
| T4 | `a_registry_access_returns_a_vetoed_failure_unchanged_after_one_attempt`, `a_registry_access_that_always_fails_refuses_at_its_deadline_naming_the_count_and_the_last_failure`, `a_registry_access_passes_two_failures_and_returns_the_success_after_them`, `an_undecidable_veto_refuses_at_once_naming_why`, `the_final_attempt_passes_a_failure_repaired_by_the_deadline`, `contended_attempts_counts_exactly_the_attempt_answers`, `a_veto_that_blocks_past_the_deadline_is_followed_by_no_attempt` | WM `:15710-15918` | all |
| T5 | `no_removal_prunes_another_processs_registration_and_the_store_goes_only_when_empty`, `no_production_argv_of_the_manager_names_prune` | WM `:17160`, `:17192` | all |
| T6 | `an_add_whose_own_entry_cannot_be_made_once_succeeds_on_a_later_attempt` | WM `:16295` | Unix |
| T9 | `an_add_refuses_within_its_deadline_while_r_x_is_held_alone_and_a_list_does_not_wait`, `r_x_held_alone_refuses_a_shared_access_with_no_attempt_and_passes_an_unheld_one` | WM `:17101`, `:15952` | all |
| T10 | `the_manager_never_fails_beside_the_legacy_engines_gate_snapshots` | WM `:17587` | all |
| T11 | `a_list_over_a_registration_half_written_refuses_at_its_deadline_and_is_never_git_state`, `a_list_over_a_registration_half_written_passes_once_its_writer_finishes`, `a_list_over_a_whole_registration_git_cannot_list_refuses_at_its_deadline` | WM `:16122-16180` | all |
| T12 | `an_add_into_a_store_nothing_can_write_refuses_at_its_deadline_and_leaves_nothing_at_the_slot` | WM `:16248` | Unix |
| T13 | `adds_whose_checkouts_outlast_the_deadline_do_not_wait_for_each_other` | WM `:17046` | Unix |
| T14 | `an_add_whose_sibling_scan_meets_a_torn_entry_of_its_own_name_is_attempted_past_it` | WM `:16332` | all |
| T15 | `an_add_whose_checkout_cannot_be_made_refuses_after_one_attempt_and_leaves_nothing` (FUB-D5-GENUINE's tree and a 300-byte name); `an_add_whose_destination_cannot_be_removed_refuses_after_one_attempt_and_keeps_it` (FUB-D6-INODE's two shapes); `a_verification_whose_snapshot_checkout_fails_after_the_takeover_ends_resumably_and_its_resume_reverifies` | WM `:16481`, `:16535`; CO `:7200` | all, the 300-byte name on Unix only; Unix; Unix |
| T16 | `an_add_whose_destination_is_not_an_empty_directory_is_git_state_before_git_worktree_add` (a directory holding entries, and a file, each beside a sibling torn by `tear_registration` at the add's `Before` hook, after the gate's list; renamed at repair round 3, R4), `a_populated_destination_beside_a_registration_already_torn_meets_the_gate_first` (the order: a sibling already torn when the gate lists the registry is met by the gate first; repair round 3, R4), `a_destination_that_is_a_link_to_an_empty_directory_is_refused_before_git`, `an_add_whose_destination_cannot_be_made_is_git_state_at_once`, `after_an_untouched_failure_the_destination_is_removed_and_made_again` (its retry held while the destination is read; repair round 3, R2); the control `a_verification_whose_snapshot_destination_cannot_be_made_defers_as_before` | WM `:16633-16828`; CO `:7335` | all; Unix for the last four |
| T17 | `a_removal_with_no_store_takes_an_empty_destination_and_still_refuses_a_checkout` | WM `:16886` | all |
| T18 | `an_add_in_a_repository_whose_git_dir_is_a_link_is_attempted_past_a_torn_entry_of_its_name` | WM `:17667` | Unix |
| T20 | `an_add_beside_a_torn_entry_whose_destination_cannot_be_removed_refuses_and_is_never_git` | WM `:16595` | Unix |
| T22 | `derive_over_a_registration_the_list_dies_on_refuses_and_the_operators_remedy_clears_it` | WM `:16928` | all |
| T23 | `a_failure_after_the_takeover_is_refused_not_returned_and_not_attempted_again` | WM `:17001` | Unix |
| 2P | `a_list_passes_a_registration_another_process_finishes_writing` (contention), `accesses_over_a_registration_a_dead_process_left_torn_refuse_and_are_never_git_state` (a torn write by a dead writer), `a_list_over_a_registration_another_process_left_whole_and_unlistable_refuses` (a registration nobody writes) | WM `:17482-17564` | all |
| LC | `a_linked_childs_kill_fails_its_test_within_its_bound_rather_than_wait_for_a_child_its_kill_did_not_end` | WM `:17805` | Linux |
| R1 | the 34 witnesses of §9.13.1 (`a_pipeline_is_granted_while_…` and `a_pipeline_is_served_while_…`), one or more per census access and per routing point; the controls `the_width_one_step_runs_the_same_transitions_and_its_access_waits_by_sleeping` and `run::tests::both_drivers_run_each_transition_through_the_one_generic_function` (R-T) | CO `:4783-5470`, `:5567`; `src/engine/topology/run/tests.rs` `:367` | all |
| R3 | `a_foreign_writer_whose_handshake_never_arrives_writes_nothing_and_says_so`, `a_foreign_writer_is_cancelled_and_joined_before_its_fixture_is_reclaimed` (§9.13.3) | CO `:4317`, `:4333` | all |
| R7 | `a_sibling_whose_checkout_cannot_be_read_while_its_removal_is_in_flight_does_not_fail_an_add`, `a_sibling_whose_checkout_stays_unreadable_refuses_the_add_resumably_and_never_as_io`, `a_sibling_whose_checkout_cannot_be_read_while_its_removal_is_in_flight_does_not_fail_a_verification`, and the control `a_sibling_whose_checkout_is_a_link_to_nothing_refuses_the_add_at_once` (§9.14.4); the CI-failing `concurrent_snapshot_adds_and_removals_on_one_repository_never_fail`, unchanged | WM `:7952-8072`, `:7782` | Unix; all |
| R8 | `run::tests::both_drivers_run_each_transition_through_the_one_generic_function` (R-T), its sources read with their line endings normalised (§9.15) | `src/engine/topology/run/tests.rs` `:367` | all, on a CRLF checkout too |
| I2 | Repair round 6 (§9.16): a shutdown answered inside a registry access's wait, one witness per pausing transition (`a_shutdown_answered_inside_…`, twelve, from the dispatch's head check to finalization); the reviews' witnesses kept (`a_shutdown_answered_during_a_registry_pause_dispatches_nothing_and_appends_nothing`, `a_shutdown_consumed_during_a_registry_wait_publishes_no_candidate`) and the admission's (`a_shutdown_answered_during_an_admitted_dispatchs_pause_spawns_no_pipeline`); the stop's two halves (`a_dispatch_begun_after_a_wait_answered_a_shutdown_appends_nothing`, `a_publication_begun_after_a_wait_answered_a_shutdown_moves_no_ref`); closure's and finalization's routing (`a_closures_registry_wait_answers_on_the_coordinator`, `a_finalizations_registry_wait_answers_on_the_coordinator`); the timer (`a_wait_answers_a_queued_shutdown_when_no_thread_can_be_started`, `a_coordinator_whose_timer_cannot_start_refuses_before_it_appends_anything`, `a_wait_whose_timer_unwinds_is_still_woken_and_the_timer_serves_the_next`, `a_wait_after_its_timer_has_stopped_refuses_at_once_and_never_sleeps`, `a_wait_that_begins_after_a_shutdown_does_not_wait`); R-T per call site (`run::tests::both_drivers_run_each_transition_through_the_one_generic_function`, its positive control `run::tests::the_r_t_census_reports_one_call_that_bypasses_its_driver`, and `every_append_of_the_width_one_step_a_retrys_settlement_included_folds_through_the_callers_hooks`); census 1's domain (`run::tests::both_attempt_started_arms_take_their_pool_from_an_authority`) | CO `:5880-6309`, `:6401`, `:6544`, `:6486`, `:6624`, `:6683`, `:6899`, `:6887`, `:6912`, `:6980`, `:7023`, `:7108`, `:7069`, `:5650`; `src/engine/topology/run/tests.rs` `:367`, `:403`, `:506` | all; the two thread-exhaustion witnesses on Linux |
| W7 | Repair round 7 (§9.17): the tear's prober reports only after a sample taken after the wait that ended it, the order forced both ways (`a_prober_cancelled_after_the_attempt_it_waits_for_reports_it_and_finishes_the_tear`, `a_prober_cancelled_before_anything_it_waits_for_leaves_the_tear_and_says_so`); the witness it serves is I2's `a_shutdown_consumed_during_a_registry_wait_publishes_no_candidate` | CO `:4534`, `:4558` | all |
| CAS1 | The CAS-1 round (§9.21): census C6's `a_pipeline_is_served_while_a_publications_swap_recheck_waits_on_a_torn_registration` and `a_shutdown_answered_inside_a_publications_swap_recheck_publishes_nothing` (inside R1's and I2's ranges above); the manager's `a_swaps_publishability_recheck_waits_through_the_calls_hooks` and `a_wait_that_ends_a_swaps_publishability_recheck_moves_no_ref`; the census `no_function_that_takes_hooks_reaches_a_registry_wait_that_sleeps_by_default` and its positive control `the_hooks_routing_census_reports_a_swap_that_drops_its_hooks` | CO `:4985`, `:6219`; WM `:4837`, `:4877`, `:5174`, `:5241` | all |
| B4 | The B4 round (§9.22): I5-1's `a_lease_observation_that_fails_still_fails_the_first_incarnations_death_at_once` and `a_lease_observation_that_fails_still_fails_a_creation_prefixs_first_resume_at_once`, and `a_lease_copy_that_outlives_the_bound_still_refuses_a_creation_prefixs_first_resume`, in the frozen recovery tests (H3, revised); the probe's `the_lease_waits_observation_tells_a_failed_inspection_from_a_held_lease`; fix P's `a_shutdown_answered_inside_a_slow_dispatchs_intent_starts_no_attempt_and_spawns_nothing` | `src/engine/topology/recover/tests.rs` `:18078`, `:18128`, `:18182`; `src/rundir/tests.rs` `:6129`; CO `:5942` | Unix; Unix; all |

**How they wait.** Each waits on a handshake or a seam, with time only as a watchdog.
- A tear an access must fail on first is finished only after `contended_attempts` has moved. An add's own tear is
  planted at the add's `Before` effect hook, after the gate's `revalidate()` has listed the registry.
- R1's witnesses (§9.13.1) finish their tear only after an invocation entered or left the runner after it, which
  needs the coordinator to have answered a message during the wait.
- R7's witnesses (§9.14.4) end the sibling's removal inside the call's own registry wait, so the access must have
  failed once on the sibling and waited through the call's hooks before its next attempt.
- The two-process tests are children of this test binary (`--exact … --ignored`) behind `LinkedChild`: T1 and T3 run
  whole cycles of a `WorkspaceManager` in a checkout of their own; 2P's writer writes a registration file by file, as
  `git worktree add` does, stops half way and waits for its parent. Its dead-writer case kills it there, through the
  bounded kill, after the intent is written.
- LC refuses `kill(2)` with `EPERM` on the test's own thread (a seccomp filter, Linux), so the kill cannot end the
  child, and checks that `kill` and the drop each fail within three `COLLECT_BOUND`s, naming "still not collectable".
  It runs isolated (`crate::agent::proc::test_support::run_test_isolated`, bounded at 90 s), so a mutation that
  removes the bound fails it instead of wedging the suite.

**Where a test differs from the record's words.**
- **T15's verification** is built with a required filter that always fails the snapshot's checkout (§7.7's R11′, a
  failure after the takeover), not with FUB-D5-GENUINE's tree: a candidate the engine commits cannot hold a `.git` path,
  which Git refuses to add. The manager-level test uses the tree itself, built with `git mktree`.
- **T1 and T3 on Windows** run 150 and 100 cycles per process, against §5.7's 1,000 in all, to fit CI's slower Windows
  leg. The Linux and macOS counts are 1,000 and 1,020.
- **T19 and T21** are withdrawn with the probe (§7.8). **T6** makes its own entry unmakeable once, with the store's
  permission, instead of a prune that needs a pause inside Git, which §5.7 executes at the Git level.

### 9.5 Red on the first-bad shape

**The port** (`impl/basewit/`; `tools/basewit.py`): master `5c222ff2`'s tree, with the head's three test files laid
over it and a test-only shim so they compile (the `RegistryRefused` variant, never constructed by master's code;
`CONTENDED_ATTEMPTS`, never incremented; the 500 ms deadline; `AtDestination::read`; the fixture's `COLLECT_BOUND`,
`set_mode` and `ModeRestored`). Master's production code is unchanged, so each witness meets the defect it was written
for. Ten tests are left out because they call the helper, R-X's new shape or the new destination step directly: T4's
seven, T9's two, and the link case of T16. Their red is the mutations'.

**The run** (`impl/basewit/basewit-1.log`, `basewit-1-reasons.txt`; Compiling line naming the port's tree): **31 of 36
red,** each for its own reason. For example: a list over a half-written registration returned `Git`; the add into an
unwritable store returned Git's "could not create directory"; the killed-add test met `Git` where it expects the
refusal; the forced removal with no store returned `Io`; T2 recorded `merge_verification_unavailable` and a deferral;
T2′ completed the run instead of ending it. **Five green, as controls:** the hook observer's removal test, the frozen
finalize test `scrub_slots_converges_when_git_has_pruned_the_emptied_registration_store`, the destination-unmakeable
verification (unchanged behaviour by design), PR11's in-process concurrency test, and T13 (master has no deadline to
outlast; its red is m18's).

**T1, T3 and T10 again,** twice more each (`basewit-prob-2.log`, `basewit-prob-3.log`): T1 and T3 red in 3 of 3 runs,
T10 in 2 of 3.

### 9.6 Mutations

**The campaign** (`impl/mutation/`; `tools/campaign-b.py` defines each mutation and the 46-test set, `tools/check-b.py`
each mutation's must-fail tests). Each mutation ran on a scratch copy of the tracked files whose Compiling line names it,
and each copy differs from the reference `d354ba37` in exactly the one file its mutation names
(`impl/mutation/verify-copies.txt`). Every run reported all 46 tests. The control is green, 46 of 46. **All 22 mutations
are killed, every must-fail test red** (`impl/mutation/TABLE-b.txt`):

| Mutation | What it brings back | Must-fail red |
|---|---|---|
| m1 | the helper returns its first failure | 12/12 |
| m2 | the deadline's refusal typed `Git` | 7/7 |
| m3 | the add's veto always `Return` | 5/5 |
| m4 | the add's veto never vetoes | 4/4 |
| m5 | no destination made, "nothing at the slot" read as untouched | 3/3 |
| m6 | the store-absent empty-target branch removed | 1/1 |
| m7 | `Undecidable` treated as `Return` | 5/5 |
| m8 | no final attempt | 1/1 |
| m10 | the removal proof skipped | 2/2 |
| m12 | the possibly-taken-over answer as `Return` (option (i)) | 4/4 |
| m13 | the same answer as `Attempt` (option (ii′)) | 4/4 |
| m14 | a populated destination attempted once, as round 8 had it | 1/1 |
| m15 | an engine `git worktree prune` after a removal | 2/2 |
| m16 | the emptied store kept | 2/2 |
| m17 | a refused add keeps its destination | 3/3 |
| m18 | adds hold R-X alone | 1/1 |
| m19 | the list takes R-X | 1/1 |
| m20 | `RegistryRefused` mapped to `Unavailable` in `run::verified` | 3/3 |
| m21 | a shared access does not wait for R-X held alone | 2/2 |
| m22 | an exclusive access takes R-X shared | 1/1 |
| lc1 | `LinkedChild::kill` waits without a bound | 1/1 |
| lc2 | `LinkedChild`'s drop waits without a bound | 1/1 |

m9 and m11 are withdrawn with the probe (§7.8). **m21 and m22 came after the first twenty.** Each regression test
was checked against two questions: red on its first-bad shape, or turned red by a mutation. One test answered neither,
`r_x_held_alone_refuses_a_shared_access_with_no_attempt_and_passes_an_unheld_one`
(`impl/mutation/TABLE-b-first-20.txt`). m21 and m22 bring back the two R-X defects it guards, and each turns it red:
"a Shared access beside R-X held (alone: true): expected a registry refusal, got Ok(1)" under m21, and the same for an
Exclusive access beside a shared holder under m22. Beyond its must-fail set, m1 also reddens PR11's in-process concurrency
test, `concurrent_snapshot_adds_and_removals_on_one_repository_never_fail`. m16's must-fail set includes the frozen
finalize test above, which pins the emptied store's removal.

### 9.7 Five suites, the frozen census, the instruments and the residue

**Five suites at this head's code and five at master's** (`impl/suites/`; `tools/five-suites.sh` and `suite-run.sh`),
alternating, one at a time. Each was `cargo test --all-targets --all-features` through `upstroke-build` on an archive
of its commit (`58c7c203`, `5c222ff2`), with `TMPDIR` a fresh directory, and each log's Compiling line names its tree
(`impl/suites/identity.txt`).
- **This head's code:** the library passed 3,068, failed 0 and ignored 130 in each of the five, and the binary's 10
  passed.
- **Master's:** 3,028, 0 and 126 in each of the five, and the binary's 10.
- **The frozen census** (`impl/suites/frozen-census.txt`): in every suite at both, `recover` 226, `integrate` 20,
  `repair` 5, `finalize` 5, `fold` 192 and `events::log` 47 passed, with the legacy `engine::tests` 186 and
  `workspace::tests` 47, and none failed. The census reads 186 to 188 result lines for `engine::tests`, for a reason
  outside the change: `v1_sibling_run_helper`, an ignored helper its witness runs as a child, prints its own result
  lines into the same output, and in head-4 and base-5 they were glued to other lines. Counted by distinct names, 186
  ran and passed in all ten (`impl/suites/engine-tests-census-note.txt`).
- **The instruments:** the seven instrument censuses passed in all ten suites: the wrapper classification and its name
  pins, the effectful-wrapper denials, the process-start and payload censuses, the governed-lint allowlist and the
  sequential registry.
- **The residue** (`impl/suites/residue.txt`, `tools/residue-compare.py`): every suite at both left the same 44
  top-level entries and 154 in all in its fresh `TMPDIR`, class by class. No class differs, so the new tests leave
  nothing behind.
- **A first attempt is void.** `suite-run.sh` touched a `build.rs` that does not exist, which created an empty one in
  each tree, and every run failed to compile its build script before any test ran. Its logs are kept
  (`impl/suites-void-empty-build-rs/VOID.txt`). The script was corrected and the ten suites run again.

### 9.8 The frozen proof, in two parts

As addendum 1 asks, in two parts that are kept apart (`impl/frozen/frozen-proof-58c7c203.txt`, read-only Git through
`tools/frozen-proof.sh`). Round 1's head added only `reviews/` and `findings/` to `58c7c203`. Repair round 2 changes two
test files, neither in the R-D set, and its own run of the proof at `8364009d` gives the same two parts
(`impl2/frozen/frozen-proof-8364009d.txt`).
- **Part 1, B's own frozen delta: zero.** Against master `5c222ff2`, the base the merge brings in, every file of PR11's
  R-D set is byte-identical: 34 of 34 (26 production files and 8 whole-file test children).
- **Part 2, the cumulative comparison against G5's range `d724fb16`:** 4 files, +241/−74 (`src/engine/topology/recover/tests.rs`,
  `src/events/log/tests.rs`, `src/events/mod.rs`, `src/topology/registry.rs`). They are master's, not this change's:
  master shows the same four against `d724fb16`. E-G6-1's two-tier rule, executed as the decision appendix's round 2
  executed it, accounts for all four: #322's test modules and #327's migration-only step. **O12 (erratum E-G6-1) is not
  adopted, so this is the cumulative difference it is, and not a clean module diff proof.**
- **Schema 4 and the legacy path** (`impl/frozen/legacy-activation-58c7c203.txt`): the change touches seven paths under
  `src/` and `effects/`, none of them a legacy module. `RegistryRefused` is constructed only in
  `src/workspace_manager.rs`. `src/topology/schema.rs` is unchanged: `TOPOLOGY_ACTIVATION` is `Inactive` and
  `MAX_READABLE_SCHEMA` is 3, so schema 4 stays unreachable in production.

### 9.9 Platforms

- **Executed on Linux only,** on this box (`impl/env.txt`: Linux 6.8, Git 2.43.0, rustc 1.97.1 and 1.85.0). The ten
  gates and every figure above are Linux's.
- **Windows and macOS were type-checked and linted, not run** (`impl/platform/`): clippy `-D warnings` over all targets
  for `x86_64-pc-windows-msvc` and `aarch64-apple-darwin`, and `cargo +1.85.0 check --locked` for the Windows target.
  All three exited 0 with no warning, at `58c7c203`'s code, the working tree differing only under `reviews/` and
  `findings/` (`impl/platform/pre-commit-58c7c203/`, each log naming its command and head). Repair round 2 ran the same
  three at `8364009d`, and all three exited 0 with no warning (`impl2/platform/code-8364009d/`).
- **What only CI can say:**
  - every Windows and macOS test run, and the `cfg(unix)` tests on macOS;
  - the Windows counts of T1 and T3;
  - the Windows paths the record reasons about (§7.7's R9″: delete-pending directories and held handles under the
    removal proof);
  - anything a compiler newer than this box's flags. CI installs the current stable, rustc 1.99.0 at repair round 2,
    and the box's stable is 1.97.1 (§9.12).

  Twelve tests are `cfg(unix)` and LC is Linux-only (§9.4), so Windows runs the rest.

### 9.10 The findings at this touch

- **Deleted, with `fixed` ledger rows:** `PR11-LINKED-CHECKOUTS-RACE-THE-SHARED-WORKTREE-REGISTRY`, cases (a), (b) and
  (d). Case (c) is `PR329-A-RESUME-REBINDS-A-SLOT-ITS-DEAD-COORDINATORS-GIT-CHILD-STILL-WRITES` (follow-up C) and case
  (e) `PR329-LEGACY-RUNS-IN-LINKED-CHECKOUTS-RACE-THE-SHARED-WORKTREE-REGISTRY` (follow-up D), each its own file, as
  §7.7 says, so the file goes. And `PR328-LINKED-CHILD-KILL-WAITS-WITHOUT-A-DEADLINE` (#328's P3), by LC.
- **Filed** (records only; they decide nothing): FUB-D9-WINPUBLISH as
  `PR329-CONCURRENT-PUBLICATIONS-RACE-THE-FINAL-RENAME-ON-WINDOWS` (P2) and FUB-D9-RELPOINTER as
  `PR329-MANAGER-READERS-RESOLVE-A-RELATIVE-GIT-POINTER-AGAINST-THE-ENGINES-WORKING-DIRECTORY` (P2), from the decision
  appendix's §10.1 and §10.2; O3-R as `PR329-HOST-ROLES-INHERIT-THE-COORDINATORS-GIT-REPOSITORY-CONTEXT` (P1) and
  INV-07/L13 as `PR329-A-LINEAGE-FAILURE-LEAVES-A-STARTED-VERIFICATION-WITHOUT-A-TERMINAL` (P2), from their drafts
  under `~/orch-pr11/evidence/`.
- **Updated, each with a dated note:** the external-prune P1 (face 1's narrowing in force; round 9's findings against
  its closures, FUB-D9-POLICY, -HEADRECREATE, -RESUMESCRUB, -INFLIGHTSCRUB and -TASKSEL, recorded for the change that
  takes it up); R14's P2 (in force); `PR11-HOST-AGENT-PRUNE-RACES-AN-ENGINE-ADD` (its reconciliation in force); the
  legacy P1 (the helper exists for D); the DESC P1 (what C's R-P relies on); `RESIDUE-UNBINDABLE…` (now
  `RegistryRefused`); `PR5-RD-002-RESUME-DERIVES…` (the operator's remedy documented; its P2 left to a reviewer);
  `PR5-RD-003…` and `PR308-R3…` (no engine prune: the second shape holds, and their guards move to the policy each
  still needs); and `PR5-RD-002-TORN-REMOVAL-NOT-DURABLE`, re-guarded, not closed: its guard named the next change to
  the branch #329 rewrote, but the reviewed design adds no durability barrier, and targeted removal widens its reach to
  every bound registration.
- **FUB-D9-ENV (P1) stays open, and is filed as its own finding at repair round 2:** `FUB-D9-ENV`, in
  `findings/P1_correctness_202610031425_the-managers-git-children-inherit-the-coordinators-git-repository-context.md`
  (§9.12). Its remedy route is the owner's decision O3, which is not adopted.

### 9.11 What waits, and what is not verified

- **B's merge waits on** O9 (R14's `design/26` text), O14 (under (b), its mechanism lands first) and O11 (the
  classification of B's narrowings). `design/26` is untouched pending them.
- **G6 is not cleared by this change.** The external-prune class (face 2), follow-ups C and D, R14's disposition, and
  the ENV and O3-R P1s stay open, each filed, and filing clears none of them.
- **Not verified here:** any Windows or macOS execution (§9.9); a power loss after a registration's removal
  (`PR5-RD-002-TORN-REMOVAL-NOT-DURABLE`, reasoned); a prune inside Git's add, which the record executes at the Git level
  and the suite reproduces only by its end state (T23).

### 9.12 Repair round 2: CI's toolchain, two Windows assertions, and FUB-D9-ENV filed

**Who and why.** `pr11_fub_impl2` (`claude-opus-5-5`, `max`), spawned by `orch_pr11` on `59d206b3` after CI went red
there. Its brief is `~/orch-pr11/briefs/pr11_fub_impl2.md`, with two answers: `~/orch-pr11/answers/pr11_fub_impl2-1.md`
(how to repair the Windows assertions) and `-2.md` (file FUB-D9-ENV). Its figures are under
`~/orch-pr11/logs/pr11_fub_impl2/`, cited as `impl2/…`.

**CI at `59d206b3`** ("CI" run 37126087842; the policy run 37126087837 passed).
- **Five jobs failed to compile the library's test target:** lint on Linux, Windows and macOS, and test on ubuntu and
  macOS.
  - CI installs the current stable, rustc 1.99.0 (2026-09-28). It deprecates `AtomicUsize::fetch_update`, "renamed to
    `try_update`".
  - `RUSTFLAGS=-D warnings` made each of this change's three calls an error: `coordinator.rs:3618`, `:3829` and `:3939`.
    They are in the review-input policies `TearsAForeignRegistration`, `RequiresAFailingFilter` and
    `DeniesTheSnapshots` (`impl2/deprecation/ci-diagnostics-59d206b3.txt`). Windows reports only the first, because
    the other two are `cfg(unix)`.
  - This box's stable is 1.97.1, which does not deprecate the method, so the ten gates passed here. The three Rust 1.85
    legs passed.
- **`test (winguest)` ran,** because the guest pins rustc 1.97.1. It passed 2,839 and failed 3
  (`impl2/ci/ATTRIBUTION-winguest-59d206b3.txt`).
  - **One is the filed P2 `PR262-UNREACHABLE-PATH-ENTRY-ABORTS-PROGRAM-RESOLUTION`:**
    `runner::host::tests::every_path_entry_this_runner_searches_names_a_location_on_its_own`, with "The specified
    network name is no longer available. (os error 64)". It is pre-existing, and this branch changes nothing under
    `src/runner/`.
  - **Two are this change's own tests, failing at their last assertion:**
    `a_list_over_a_registration_half_written_passes_once_its_writer_finishes` and
    `a_list_passes_a_registration_another_process_finishes_writing`. In both, the list passed and the handshake held.
    - The planted registration's `gitdir`, and its checkout's `.git`, were written with `display()`. On Windows that
      gives backslashes, which Git never writes.
    - Git for Windows strips only its own `/.git` when it lists a worktree, so the listed record kept `\.git`.
    - So `ends_with("<name>-checkout")` failed.

**The repairs, in test code only.**
- **`b5a39149`.** The three one-shot policies now arm with `compare_exchange(1, 0, SeqCst, SeqCst).is_ok()`.
  - Each counter is made by `AtomicUsize::new(1)`, and nothing else touches it (`impl2/repair/site-check-before.txt`
    and `site-check-after.txt`).
  - So the call is true at the first verification only, as `fetch_update(.., |n| n.checked_sub(1)).is_ok()` was.
  - `try_update` does not exist in the MSRV, 1.85.
- **`8364009d`.** Every registration this change's tests plant now spells its paths as Git writes them.
  - The spelling is the crate's own `GitdirRule::native().spelling`: the host runner's Git-for-Windows form, with `/`
    separators, and on other platforms the path's own bytes. A test-local `as_git_writes_it` wraps it.
  - It covers six writes: the `gitdir` and checkout `.git` of `plant_half_written_registration` and of
    `registry_writer_child`, and the `gitdir` that T14 and T18 plant at the add's Before hook.
  - **The assertions are unchanged,** on the orchestrator's decision (answer 1): a widened assertion would accept a
    `.git`-suffixed record on every platform.
  - The pre-existing tests' `display()` writes are left as they are.
  - **Also left as it is:** the coordinator's `TearsAForeignRegistration`, which writes its `gitdir` with `display()`
    too. The decision named the planting sites in `src/workspace_manager/tests.rs`, and the policy's two tests passed
    on the guest and read no listed path.
  - *Corrected at repair round 3 (R6, P3, the implementation review's regular lens; §9.13): so "every registration"
    above was not true at this round; it was true of the six writes listed. Repair round 3 spells the coordinator's
    plant as Git writes it too, and the claim is true from then on.*
- **Nothing else moved** (`impl2/repair/what-moved-59d206b3-8364009d.txt`): two files, and every hunk is in test code.
  The coordinator's production region is byte-identical, and every other compiled input's tree id is unchanged.

**What this round re-ran, on Linux.**
- **The deprecation search** (`impl2/deprecation/`).
  - No other `fetch_update` is in the branch's diff or the tree.
  - In every failed job, the 1.99.0 compiler printed only those three.
  - The one deprecation the 1.98 and 1.99 release notes list, the legacy integer modules, is not used.
  - Master `5c222ff2` passes CI on 1.99.0 (run 36989798275).
- **The fixture change is a no-op on Linux, byte for byte** (`impl2/planted-bytes/COMPARISON.txt`).
  - One run of each fixture was made before the change and one after, with the same probe in a scratch copy.
  - All nine plants (fifteen files) equal the old `display()` form within their own run. They also match across the
    two runs once each run's random root is normalised.
  - So the change alters what is planted on Windows only.
- **The tests that reach a planting site,** found by search (`impl2/repair/planting-users-search.txt`): T11's three,
  2P's three (through `registry_writer_child`), T14 and T18.
  - With the four coordinator tests of the repaired policies, all twelve passed in 3 of 3 runs at `8364009d`
    (`impl2/tests/head-8364009d/`).
  - On the guest at `59d206b3`, five of the eight passed and the two above failed. T18 is `cfg(unix)` and does not run
    there (`impl2/ci/winguest-59d206b3-planting-users.txt`).
- **The mutations at `8364009d`** (`impl2/mutation-final/`).
  - Run: the control, and every mutation that round 1's logs show reddening any of those twelve tests. Those are m1,
    m2, m3, m5, m7, m8, m12, m17 and m20 (`impl2/mutation/round1-subset-derivation.txt`,
    `round1-subset-planting-users.txt`).
  - All nine are killed with every must-fail test red, and the control passed 46 of 46. Each copy differs from
    `8364009d` only in its mutated file.
  - Every one of the 46 tests gave round 1's result, except T10 under m3: red in round 1, green here. T10 is outside
    m3's must-fail set and racy by construction (§9.5: red in 2 of 3 base runs).
  - Eight of them, all but m17, also ran at `b5a39149`, before the fixture change, with the control. They gave the
    same results (`impl2/mutation/`).
- **The frozen proof at `8364009d`:** Part 1 is zero, 34 of 34, and Part 2 is unchanged
  (`impl2/frozen/frozen-proof-8364009d.txt`).
- **The platform checks at `8364009d`** (§9.9).
- **§9.4's line numbers** are re-pinned to `8364009d`'s code, by each cited line's text
  (`impl2/repair/record-line-cites-remap.txt`).
- **Not re-run:** the five suites and the residue (§9.7), and the base-witness port (§9.5). Those are round 1's, at
  `58c7c203`'s code. The ten gates ran at this round's head, and the body records them.

**FUB-D9-ENV is filed** (answer 2).
- It is `FUB-D9-ENV` (P1), in
  `findings/P1_correctness_202610031425_the-managers-git-children-inherit-the-coordinators-git-repository-context.md`,
  at its discovery pin `8df42436`. Its source is the round-9 gitenv lens's finding 1, with its triage row.
- It replaces the deferred ledger row that was the implementation round's only record of it.
- Its remedy is the owner's decision O3 (ENV-1 for the manager's builders), which is not adopted. Nothing is chosen,
  waived or regraded.

**What only CI can say now:**
- whether 1.99.0 compiles the repaired test target with no warning. At `59d206b3` it printed nothing else, and master
  passes on it.
- whether the two Windows tests pass with Git's form planted.

### 9.13 Repair round 3: the implementation review's R1 to R6, and the proposed frozen hunk H1 and H2

**Who and why.** `pr11_fub_impl3` (`claude-opus-5-5`, `max`), spawned by `orch_pr11` on `54a1ff14` after the
implementation review's first round. Two `gpt-6-astra` lenses at `max`, regular and regression, both returned
CHANGES_REQUIRED with no P1 and six items, R1 to R6, each carrying a witness
(`~/orch-pr11/reviews/review-329-i1-triage.md`). Its brief is `~/orch-pr11/briefs/pr11_fub_impl3.md`. It asked one
question (`~/orch-pr11/questions/pr11_fub_impl3-1.md`), which the orchestrator answered with decision (A)
(`~/orch-pr11/answers/pr11_fub_impl3-1.md`). Its figures are under `~/orch-pr11/logs/pr11_fub_impl3/`, cited as
`impl3/…`. It adopts no owner decision.

**The commits, on `54a1ff14`** (`impl3/r1/what-moved-4862fdf3.txt`):
- `ec0c41c8`: R2, R3, R4 and R6 in code, tests and notes, and `design/15`'s qualified sentence;
- `13b47a7c`: this record's R4 and R6 text, and FUB-D9-ENV's corrected precondition (R5);
- `eef97e41`: R1's manager half, with eight effect rows;
- `e369b251`: **the proposed frozen hunk H1 and H2**, alone;
- `10cc88d8`: R1's coordinator half and its first 24 witnesses;
- `4862fdf3`: R1's witnesses extended to 34;
- `a9b60caf`: R2's `hold_next_contended` made `cfg(all(test, unix))`, as its one caller is. The Windows target's
  lint and 1.85 check refused it as dead code at `4862fdf3` (`impl3/platform/code-4862fdf3/`).
- then this text, `integrate.md`'s two notes for H1 and H2, and the body.

#### 9.13.1 R1 (P2): no registry retry waits on the coordinator's thread

**What was wrong** (the regular lens, executed at `54a1ff14`).
- Every wait of the tolerant registry access slept on the caller's thread.
- On the coordinator, a pipeline's grant therefore waited a retry out. The lens measured 500.87 ms at the test
  deadline, with no worker granted; production's deadline is 10 s. This round's re-run of its witness measured
  500.99 ms, with no worker invocation (`impl3/witness/regular-54a1ff14/test.log`).
- PR11's R-S: "The coordinator's only waits are its inbox and the synchronous work it already did at width 1 (its
  own Git, filesystem and appends)" (`reviews/2026-09-30-pr11-record.md`, R-S). A backoff is a wait, not that work.

**The census** (`impl3/r1/census.txt`) lists every registry access the coordinator's thread makes while a pipeline can
be live, at `54a1ff14`: 17 numbered call paths in four arms (A1 to A6 a dispatch, B1 and B2 a retry, C1 to C5 an
integration, D1 to D4 a settlement), several of which reach more than one access.
- Four reach the access through the frozen `integrate.rs` with nothing the coordinator supplies: A1 the dispatch's
  head check, C1 `decide`, C2 `publish` and C3 `integrate_stale`'s `proposal_state`.
- Every non-frozen path held a borrow of the run across its manager calls, so the coordinator could not answer a
  message during the wait.

**The question, and the answer.**
- R1 could not be fixed on the four frozen paths without a frozen hunk. So the round stopped and asked, with the
  exact hunk and its cost (the question, and its probe under `impl3/r1/probe/`).
- The orchestrator answered (A): the whole design (`impl3/r1/design-sketch.txt` §2), with the exact minimal frozen
  hunk as a proposed, conditional draft. It chose neither the bounded wait as a contract disposition nor a fix that
  leaves the frozen paths stalling.

**The design, as implemented.**
- **A seam for the wait.**
  - `EffectHooks::registry_pause(pause)` is one wait of a registry access the call makes: a backoff, or one turn of
    R-X's wait. Its default sleeps, and every other implementation of `EffectHooks` keeps it. So each pipeline's own
    accesses, on its own thread, and every caller outside the topology coordinator wait as before.
  - `tolerant_registry_access` and `with_registry` take the pause. Every manager function that takes hooks routes its
    accesses' waits through them. *(Corrected at the CAS-1 round, §9.21: `compare_and_swap_ref` did not. Its
    publishability re-check asked the hook-less `assert_publishable` until that round.)*
  - Six hook-less entry points the coordinator calls have pausing twins: `revalidate`, `changed_paths`,
    `commit_parent`, `commit_tree_sha`, `assert_publishable` and `proposal_state`. The originals delegate to them with
    `NoHooks`.
  - `funnel_lending` lends the hooks to the primitive between its phases. So the add's attempts and the
    verification's record read wait through them too.
- **The coordinator answers its messages during a wait.**
  - It lends itself as the hooks of the accesses its transitions make.
  - A wait starts a timer thread, which the coordinator owns and joins. The timer sends a `Wake` carrying the wait's
    token into the coordinator's own inbox.
  - Until that wake arrives, the coordinator answers grants, ends, snapshot requests and their ends, and a shutdown.
    None of these appends.
  - It defers `Judged` and `Verified` until the transition returns, because applying either appends.
  - Under an observer it keeps the deterministic intake and releases an invocation at quiescence. An observer's append
    inside a wait is refused. A stale wake is dropped.
  - So nothing is appended and nothing is selected inside another transition: R-S's single writer holds.
  - **R-S is kept, not stretched.** The coordinator's own synchronous work stays on its thread as R-S accepts it: each
    attempt's Git child (a `git worktree list`, a `git worktree add`), the filesystem work and the appends. Only the
    retry's waits answer messages.
- **No borrow of the run spans a manager call.**
  - The transitions are generic over an `Operator`: `begin_dispatch`, `begin_retry` and `settle_judged` directly, and
    the frozen `integrate()` through `DrivenJournal`. The operator gives the run, its seams and its hooks to the
    run-side steps, and the hooks a registry access waits through.
  - The width-1 `step` runs the same functions through `Stepping`, whose registry hooks are the run's own and sleep.
    The coordinator runs them through itself.
  - That is R-T, one implementation of every transition. `run::tests::both_drivers_run_each_transition_through_the_one_generic_function`
    pins it by reading the source. `the_width_one_step_runs_the_same_transitions_and_its_access_waits_by_sleeping`
    is the behavioural control.
- **What is unchanged:** the deadline, the typed refusal and the veto. A wait may end later than its length, by the
  time the coordinator takes to answer one message.

**PROPOSED RULING B-H (on a G6-frozen module), in RULING P-1's form. NOT ADOPTED.** Amended by repair round 6
(§9.16.9) to what remains. Round 3 proposed two hunks, H1 and H2. H2 is withdrawn, because a route its caller supplies
reaches its wait, and H1 alone is proposed.

> The owner permits exactly one production change to `src/engine/topology/integrate.rs`, a module G6 requires
> byte-identical to the G5 range. **H1:** `decide`'s check (master `5c222ff2` `:336`) moves into a private
> `decide_pausing(hooks, manager, request)`, which `decide` calls with `NoHooks` and `integrate()` (`:545`) calls with
> `journal.hooks().effects()`. `publish`'s check (`:464`) and `integrate_stale`'s `proposal_state` (`:597`) are made
> through their pausing twins with `journal.hooks().effects()`. Nothing else in `integrate.rs` changes: `dispatch_head`
> (`:253-264`) is master's. No fold, queue, merge or repair file changes. The frozen test child
> `src/engine/topology/integrate/tests.rs` stays byte-identical, and nothing is appended to it. G6's module diff proof
> reads this hunk as the only integration difference permitted beyond those of erratum E-G6-1 (the decision appendix's
> §8.5), bound to the merge SHA of the change that makes it.

**The exact hunk** is `integrate.rs` at this change's head against master: +16/−4 by `git diff --numstat`, blob
`bf62256e` (`impl6/frozen/frozen-proof-f92db934.txt`). Every line it changes is a line `e369b251` changed, round 3's
H1, and `0edfc509` reverts round 3's H2 hunk. The blob is the one the delta review's regular lens produced when it took
H2 out by hand (`~/orch-pr11/reviews/329-i2-witnesses/review329-i2-regular-8q1suqqh/h2-free-adapter.diff`).

```diff
@@ -331,9 +331,17 @@ pub struct Decided {
 pub fn decide(
     manager: &WorkspaceManager,
     request: &IntegrationRequest,
+) -> Result<Decided, UpstrokeError> {
+    decide_pausing(&mut crate::workspace_manager::NoHooks, manager, request)
+}
+
+fn decide_pausing(
+    hooks: &mut dyn crate::workspace_manager::EffectHooks,
+    manager: &WorkspaceManager,
+    request: &IntegrationRequest,
 ) -> Result<Decided, UpstrokeError> {
     let refname = request.integration_ref.as_str();
-    manager.assert_publishable(refname)?;
+    manager.assert_publishable_pausing(hooks, refname)?;
     let head =
         manager
             .direct_ref_target(refname)?
@@ -461,7 +469,7 @@ pub fn publish(
     authorized: Authorized,
 ) -> Result<Published, UpstrokeError> {
     let refname = authorized.integration_ref.as_str();
-    manager.assert_publishable(refname)?;
+    manager.assert_publishable_pausing(journal.hooks().effects(), refname)?;
     let found =
         manager
             .direct_ref_target(refname)?
@@ -542,7 +550,7 @@ pub fn integrate<J: IntegrationJournal + Verification>(
     manager: &WorkspaceManager,
     request: &IntegrationRequest,
 ) -> Result<Terminal, UpstrokeError> {
-    let decided = decide(manager, request)?;
+    let decided = decide_pausing(journal.hooks().effects(), manager, request)?;
     match decided.exact_base {
         ExactBase::Fast => {
             let authorized = prepare_fast(journal, request, decided.head)?;
@@ -594,7 +602,11 @@ fn integrate_stale<J: IntegrationJournal + Verification>(
         Ok(proposal) => Picked::Clean {
             proposal: CommitSha(proposal.clone()),
         },
-        Err(_) => match manager.proposal_state(&staging, head.as_str())? {
+        Err(_) => match manager.proposal_state_pausing(
+            journal.hooks().effects(),
+            &staging,
+            head.as_str(),
+        )? {
             ProposalState::Conflict { paths } => Picked::Conflict { paths },
             ProposalState::Empty => Picked::Empty,
             ProposalState::Unclassified { detail } => Picked::Unclassified { detail },
```

**Its purpose.** The three accesses' waits answer the coordinator's messages, as every non-frozen access's now does,
and a shutdown answered in one of them stops the integration (§9.16.3). Without H1 those three would still sleep on
the coordinator's thread for up to the deadline.

**Its size.** +16/−4, against round 3's +33/−7 for H1 and H2 together (`impl3/r1/frozen-proof-4862fdf3.txt`).

**It is not adopted.** The canonical packet and every owner grant are untouched. Admitting the hunk is the owner's
freeze ruling, and until that ruling this pull request does not meet G6's frozen-set rule.
- **What a "no" would mean:** revert H1, so that `integrate.rs` is master's, byte for byte. The library still compiles
  and its tests run (`impl6/mutation/h1-reverted/`). The five witnesses named below then fail, and their paths keep
  the sleeping wait. That is the bounded wait as a contract disposition, which the orchestrator's answer calls a
  waiver and not its to choose.

**What depends on the proposed hunk and on the proposed rows.**
- **For compiling, nothing, since round 6.** No non-frozen code calls a function the hunk adds: `decide_pausing` is
  private, and the twins it calls are the manager's. With `integrate.rs` at master's the library compiles and its tests
  run (`impl6/mutation/h1-reverted/`). Round 3's dependence of every gate on H2 (`impl3/r1/dependence/h1-h2-reverted/`)
  went with H2.
- **In behaviour.** Five witnesses depend on H1: C1 (`…_an_integrations_decision_…`), C2
  (`…_an_integrations_publication_…`), C3 (`…_a_conflicts_classification_…`), and round 6's
  `a_shutdown_answered_inside_an_integrations_decision_prepares_and_publishes_nothing` and
  `a_shutdown_answered_inside_a_publications_wait_publishes_nothing`. With H1 reverted, of the tests that row runs, exactly those five fail, and
  the other 31 R1 witnesses, the width-1 control, the R-T census and the behavioural test pass
  (`impl6/mutation/h1-reverted/`). Each of C1 to C3 is red when its own call site gives up the coordinator's hooks:
  round 3's rows `r1-c1-decide`, `r1-c2-publish` and `r1-c3-proposal-state`, re-run at round 6's code head as
  `r1-c1-decide-nohooks`, `r1-c2-publish-nohooks` and `r1-c3-proposal-nohooks` (`impl6/mutation/VERDICTS.txt`).
- **On the eight effect rows.** `effects/wrappers.toml` gains eight names on its `effect_free` lists: six for the
  pausing twins in `src/workspace_manager.rs`, and `funnel_lending` and `registry_pause` in
  `src/workspace_manager/hooks.rs`. They are first-limb instrument edits: they decide what the effect census lets
  other changes do.
  - They are declared and not held: nothing checks that a listed function is effect-free. Each twin is its original's
    body with its waits routed through the hooks it is handed, and the original delegates to it with `NoHooks`.
    `funnel_lending` is `funnel`'s body with the hooks lent to the primitive, and `registry_pause`'s default sleeps, as
    the wait always did. They are proposed until this change is reviewed.
  - `effects::tests::every_externally_reachable_fn_of_a_legacy_or_shared_module_is_classified` fails without them,
    naming the eight as unclassified (`impl3/r1/dependence/rows-removed/test.log`). So the test gate depends on them.

**The witnesses** (`CO`, `coordinator::tests`): 34, one or more per census access, and per routing point.
- **The harness.** Two pipelines run at width 2. Alpha's worker is released first. Beta's dispatch, retry,
  settlement or integration then meets a registration the run's own store holds torn, planted either at an event's
  fold or just before the n-th registry access the coordinator's thread starts after it.
- **What passes a witness.** A prober thread completes the registration only after an invocation has entered or left
  the runner after the tear. That needs the coordinator to answer a grant or an end during the wait. The witness then
  passes only if the access completes and the run finishes.
- **Two torn shapes.**
  - An empty `commondir`, on which Git's enumeration dies.
  - A registration that is `locked` and has no `gitdir`. The removal scan cannot bind it under
    `WriterProof::Unknown`, and the enumeration passes over it. Each is a shape a killed `git worktree add` leaves.
- **The paths covered:**
  - A1 to A6: the head check, the dispatch's revalidation, the intent, the add's gate, the add, a repair's
    materialization, and a continued dispatch's worktree check over a resume;
  - B1 and B2: a retry's verification, and a closed retry's scrub (its worktree removal and its intent removal);
  - C1 to C5: the decision, the publication, a conflict's classification, the stale arm's intent, add gate, add and
    pick, and a publication's and a rejection's staging removal and staging intent removal;
  - C6, added at the CAS-1 round (§9.21): the publication's compare-and-swap re-check;
  - D1 to D4: the promotion's changed paths, the candidate's two object checks, the reclaim's worktree removal and
    intent removal, and a failed settlement's scrub (both halves);
  - **the repair arm** that A6, B1 and B2 name ("torn plan + scans"): the removal's and the verification's plan, the
    plan's forced removal, the revalidation after each repair, and the verification's record read inside its funnel.
    A dead add's residue of a slot an intent names makes the first revalidation refuse at its deadline, so the
    repair runs.
- **The controls.** The width-1 control and the R-T census pass in every row below, all 36
  (`impl3/mutation-r1/*/test.log`).

**Red at `54a1ff14`** (`impl3/r1/base-port/`).
- **How it was built.** `54a1ff14`'s tree, with a test-only shim: the access-start seam's call and its twins, which
  are no-ops in production, and the fixture's speller and seam. HEAD's witness block was added verbatim; its sha256 is
  in `run.txt` (`shim-and-witnesses.diff`).
- **The result** (`summary.txt`): all 34 witnesses fail, each with `RegistryRefused` at the 500 ms deadline, which
  is the stall itself. The width-1 control passes.

**The mutations, with the control** (`impl3/mutation-r1/verdicts.txt`: 36 of 36 rows pass at `4862fdf3`; runner
`impl3/tools/campaign-r1.py`, judge `impl3/tools/check-r1.py`).
- **How a row was run and judged.** Each row was built from a clean copy of `4862fdf3` with one substitution, and ran
  45 tests: the 34 witnesses, the two controls, and R2's to R4's tests. A row passes when every witness it must turn
  red fails, the width-1 control stays green, and the copy compiled.
- **The control:** 45 of 45 pass.
- **The global row:** the coordinator's wait sleeps. All 34 witnesses fail.
- **One row per routing point,** each turning red exactly the witnesses whose access it reaches. The routing points
  are:
  - each of the four frozen calls;
  - the dispatch's revalidation, its intent, the add's gate and the add's attempt;
  - a repair's materialization, the continuation and the retry's verification and scrub;
  - the integration journal's hooks, the pick's gate, the removal scan and the intent removal's gate;
  - the verification's gate, its second revalidation and its record read;
  - the changed paths, the two object checks, the reclaim and the settled scrub;
  - the repair's plan and the removal's second revalidation;
  - the publishability scan, the proposal state's revalidation and `revalidate_pausing` itself;
  - the lending funnel and the dispatch journal's hooks.
- **R3 and R4:** two rows for R3, one for R4.
- **A gap the campaign found, and its repair.** A preliminary campaign at `10cc88d8` found one routing point no
  witness reached: the removal scan. The witnesses named for a removal had planted an empty `commondir`, which the
  scan passes over. Re-run on `10cc88d8` itself, that row passes 35 of 35
  (`impl3/mutation-r1-prelim-at-10cc88d8/r1-removal-scan/summary.txt`). `4862fdf3` adds the second torn shape and
  splits each removal witness into its scan and its intent removal. The preliminary rows are kept in
  `impl3/mutation-r1-prelim/`; its `r1-removal-scan` is the re-run on the split, before `4862fdf3` was committed.

#### 9.13.2 R2 (P2): T16's retry is held while the destination is read

`after_an_untouched_failure_the_destination_is_removed_and_made_again` read the destination after the first `Attempt`
answer. Meanwhile the next attempt and its veto could remove it (the regression lens, executed with forced
scheduling).
- **The repair.** A test-only hold, `hold_next_contended`, beside `note_contended`'s `cfg(test)` twin, parks the
  access after it counts that answer, until the observer has read the destination. The assertions are unchanged.
- **Before.** At `54a1ff14`, the lens's `schedule-only.diff` fails the test
  (`impl3/witness/r2-schedule-54a1ff14/summary.txt`).
- **After.** At `4862fdf3`, with that patch applied verbatim, the test passes 3 of 3 (`impl3/r2/final-held-{1,2,3}/`).
- **With the hold removed** (`impl3/r2/no-hold.diff`) under the same patch, it fails 3 of 3 with "after the untouched
  failure the destination is an empty directory" (`impl3/r2/final-unheld-{1,2,3}/`).

#### 9.13.3 R3 (P3): `TearsAForeignRegistration` owns its writer

The policy discarded its writer's `JoinHandle`. At its watchdog the writer wrote `commondir` anyway, and a writer
that outlived its test recreated a fixture that had been reclaimed.
- **Before.** At `54a1ff14` the lens's construction recreated the removed fixture
  (`impl3/witness/r3-unjoined-54a1ff14/recreated-tree.txt`).
- **The repair.** The policy now owns its writer. `finish`, or the policy's drop when a test unwinds, cancels it
  through a channel and joins it. A writer whose handshake never arrives writes nothing and says so, and T2 asserts
  `finish()`.
- **Its tests.** `a_foreign_writer_is_cancelled_and_joined_before_its_fixture_is_reclaimed` and
  `a_foreign_writer_whose_handshake_never_arrives_writes_nothing_and_says_so`.
- **Its mutations** (`impl3/mutation-r1/r3-*`). A write at the watchdog turns the second red. A detached writer
  turns the first red, and with it the second and T2.

#### 9.13.4 R4 (P3): the populated-destination guarantee holds after the gate

`add_worktree` runs its gate's `revalidate()` before it reads the destination. So a populated destination beside a
registration already torn refuses as the registry's at the deadline. The lens measured 500.74 ms and 16 retries, and
the re-run measured 500.75 ms and 16 (`impl3/witness/regular-54a1ff14/test.log`).
- **The wording.** The guarantee is qualified to "after a successful prevalidation, before `git worktree add`" wherever
  it is stated (`ec0c41c8`, `13b47a7c`):
  - `design/15`;
  - `Destination`'s and `add_worktree`'s docs;
  - the refusal, which now says "`git worktree add` did not run";
  - this record's §0, §8.1, §8.2, §8.9, §8.12, §9.1 and §9.3.
- **The order kept.** `a_populated_destination_beside_a_registration_already_torn_meets_the_gate_first` pins it, and
  T16's populated test is renamed. Checking the destination before the gate (`r4-destination-before-gate`) turns the
  order test red, and the renamed test with it.

#### 9.13.5 R5 (P3): FUB-D9-ENV's precondition

Step 1 of the FUB-D9-ENV file named ordinary invocations that do not supply the main checkout's ordinary index.
Executed on Git 2.43.0, by the regression lens and again at this round (`impl3/r5/env-shapes.txt`):
- a linked checkout's `pre-commit` hook gets that checkout's own index;
- its `!` alias leaves `GIT_INDEX_FILE` unset;
- a main checkout's `pre-commit` hook under `commit -a` gets `.git/index.lock`.

Step 1 now separates them from the explicit export, which is the only precondition of the destructive sequence.
- The P1, its grade, its guard and its discovery pin are unchanged. No `reviewed_sha` is re-stamped.

#### 9.13.6 R6 (P3): the coordinator's plant is spelt as Git writes it

`TearsAForeignRegistration` writes its `gitdir` through `as_git_writes_it`, which moved into the shared fixture module.
R1's plants use the same speller. §9.12's dated note says that "every registration" was not true at repair round 2;
it is true from this round.

#### 9.13.7 The frozen proof, and what else ran

**The frozen proof** (`impl3/r1/frozen-proof-4862fdf3.txt`, over G6's R-D set of 34 files):
- **Incremental, this round's own frozen change (`54a1ff14..4862fdf3`):** 33 of 34 files byte-identical.
  `src/engine/topology/integrate.rs` differs by +33/−7, which is H1 and H2 only. The one commit touching the set is
  `e369b251`, and the frozen test child is unchanged.
- **Part 1, B's own delta against master `5c222ff2`:** no longer zero. It is that same file and the same +33/−7, and
  nothing else.
- **Part 2, cumulative against G5's range `d724fb16`:** 5 files, +274/−81. That is master's known four-path gap
  (+241/−74, the same at master; O12 not adopted) plus H1 and H2. E-G6-1's two-tier rule, executed as a rule the owner
  has not adopted, reports `integrate.rs` as its one unenumerated difference.

**What else ran on Linux:**
- **The full suite at `10cc88d8`'s tree:** 3,097 library and 10 binary tests (`impl3/dev/all-r1-2.log`).
- **One earlier run** failed one test, `real_docker_lists_the_state_the_settlement_observation_reads`, which is the
  filed P2 `PR262-DOCKER-OBSERVE-READS-RUNNING-AFTER-PROCESSGONE` (`impl3/dev/all-r1-1.log`).
- **`4862fdf3` changed only the coordinator's tests and their notes.** Its 34 witnesses and every row above ran at
  it.
- **`a9b60caf` changes one attribute,** `cfg(test)` to `cfg(all(test, unix))`. On Linux and macOS that selects the
  same code, so the rows and the base port at `4862fdf3` stand for it.
- **The ten gates** ran at this round's head, and the body records them.
- **§9.4's line numbers** are re-pinned to `a9b60caf`'s code, by each named test's `fn` line
  (`impl3/r1/record-line-cites-remap.txt`). T16's row names the renamed test and the order test, and rows R1 and R3
  are added.
- **The Windows and macOS targets** were linted, and the Windows target type-checked on 1.85, at `a9b60caf`: all three
  passed (`impl3/platform/code-a9b60caf/`).

**Not verified here:**
- the new witnesses on native Windows and macOS: they are not `cfg`-gated, so CI's legs run them;
- CI's stable 1.99.0 compiling the head. This box has 1.97.1, and the 1.85 legs ran here.

### 9.14 Repair round 4: R7, the shared Windows CI failure

**Who and why.** `pr11_fub_impl4` (`claude-opus-5-5`, `max`), spawned by `orch_pr11` on `f9c88fdb` after follow-up
D's draft #331 went red on `test (winguest)` in a test of this change's shared manager code. Its brief is
`~/orch-pr11/briefs/pr11_fub_impl4.md`, and R7 is its whole scope. Its figures are under
`~/orch-pr11/logs/pr11_fub_impl4/`, cited as `impl4/…`. It adopts no owner decision, and H1 and H2 stay as round 3
committed them, proposed and not adopted (§9.14.5).

**The commits, on `f9c88fdb`:**
- `7bb9aac3`: the repair, its four witnesses and `design/15`'s sentences;
- then this text, §9.4's re-pinned cites and its R7 row, §5.5's dated check, and the body.

#### 9.14.1 What failed, and where

**The discovery.** #331's head `20e27724`, "CI" run 37138196522, job 111246977272, `test (winguest)`: 2,866 passed,
1 failed and 87 ignored, in 506.38 s (`~/orch-pr11/logs/pr11_fud_impl/ci/failed-job-111246977272-winguest.log`).
`concurrent_snapshot_adds_and_removals_on_one_repository_never_fail` failed one of its 120 add-and-remove cycles:
"adding k3-g0-a16-gates: failed to read …\snapshots\k1-g0-a16-gates: Access is denied. (os error 5)".
- **The code is this change's.** At `20e27724`, `src/workspace_manager.rs`, `src/workspace_manager/tests.rs` and
  `src/rundir.rs` are `54a1ff14`'s blobs, and none of D's own commits touches `src/workspace_manager*`
  (`impl4/r7/site/c-d-manager-identity.txt`, `impl4/r7/site/site-lines.txt`). C (#330) and D (#331) both inherit
  it, so the repair is B's.
- **Not a filed finding.** It is not FUB-D9-WINPUBLISH, a rename in `readiness::publish_between`, and not PR249's
  `locked` read. No file in `findings/` names this read.

**The read site: the add's gate.** `WorkspaceManager::revalidate`, the containment check every create and removal
runs before its funnel, compares the canonical execution root with the canonical path of every worktree
`git worktree list` names. At `54a1ff14` (`:2123-2124`) it listed the registry in one tolerant access and then, after
the access had returned, resolved each listed path with `canonical_prefix(record.path())?`. `add_snapshot` runs the
gate three times, its own, `write_intent`'s and `add_worktree`'s, and each resolves every sibling the list names,
the snapshots other pipelines are removing among them.
- `canonical_prefix` returns any failure but absence as `UpstrokeError::Io { path, source }`, whose message is
  "failed to read <path>: <source>" (`src/error.rs:97`, `src/workspace_manager/containment.rs:444`). The path is the
  list's record, with the `/` Git writes turned into `\` (`src/workspace_manager/parsers.rs:278`). That is the CI
  message, and it names a sibling, `k1`.
- **Windows.** A directory that a removal has deleted while some handle on it is still open stays in its parent with
  its deletion pending until the last handle closes, and every open of it until then answers `ERROR_ACCESS_DENIED`
  (os error 5). `fs::canonicalize` opens the path. Removals here run concurrently, each deleting its checkout with
  `remove_dir_all` and then its registration (§3.5), and every other thread's resolution holds a handle on the
  directories it resolves for an instant. So a gate can resolve a sibling whose checkout is in that state while the
  list still names it. This module already reads `ERROR_ACCESS_DENIED` as a pending deletion where it removes a tree
  (`remove_tree_once_handles_close`, `src/workspace_manager.rs:1494-1507`).
- **Not the destination's read.** `AtDestination::read` reads only the add's own destination, and a failure there is
  Git state naming "the worktree destination", never "failed to read" a sibling. The orchestrator's reading holds.
- **The same read in a verification.** `worktree_record` (`:5501-5502` at `54a1ff14`), the lookup that
  `verify_worktree` and `quiescence` make, resolves the listed paths the same way, after its list's access.
- **Every other resolution of a listed path already ran inside an access:** the removal's scan
  (`revalidate_removal_proving`) and the torn plan are themselves the attempts of their accesses. D's legacy path
  resolves no listed path.

#### 9.14.2 The first-bad commit, as far as the evidence goes

**The read site is master's.** At `5c222ff2` the gate (`:1712-1713`) and the lookup (`:5072-5073`) resolve the
listed paths the same way, after the list (`impl4/r7/site/site-lines.txt`).
- A deterministic probe, a sibling snapshot whose checkout is a link to itself (`ELOOP`, standing in for Windows'
  answer), makes `revalidate()` and an `add_snapshot` beside it return `Io` naming the sibling, alike at `5c222ff2`,
  `54a1ff14` and `f9c88fdb` (`impl4/r7/first-bad/FIRST-BAD-SUMMARY.txt`).

**What this change changed is the window.** At master R-X, one mutex, was held by the list (`:5052`) and by a
removal's mutation (`:3007`), so no list ran while a sibling's checkout was being deleted: a resolution met a deletion
only if the deletion began after the list returned and before the resolution ran. Since `58c7c203` the list, the
removal's scan and the removal's mutation take nothing (§3.4; `:5482` at `54a1ff14`), so a list can name a sibling
whose checkout is being deleted while it runs.
- **Measured on Linux,** under the CI-failing test's own load, 20 runs at each commit. A test-only shim counted the
  gate's resolutions of a listed snapshot, and how many ran while that snapshot's checkout deletion was in progress:
  0 of 4,597 at `5c222ff2`, 189 of 7,183 at `54a1ff14` and 250 of 7,489 at `f9c88fdb` (`impl4/r7/first-bad/`,
  `impl4/tools/r7-first-bad.py`). Linux has no pending deletion, so every one of them passed here. They are the
  resolutions that can meet one on Windows.

**CI.**
- The CLASS-INTERMITTENT census (5,136 job logs to 2026-10-02) names the test in 119 logs, none failed: 35 on
  `test (winguest)`, 1 on `windows-latest`, 42 on macOS and 41 on ubuntu (`impl4/r7/census/census-r7.tsv`).
- On `test (winguest)` since this change's code: `59d206b3` and `54a1ff14` passed it, and so did C's `1fc0c911`,
  whose manager is C's own change. D's `20e27724`, whose manager is `54a1ff14`'s, failed it once
  (`impl4/r7/ci/winguest-b-era-verdicts.tsv`).

**What can be said, and what cannot.** The defect, a failure to read a listed sibling returned as the gate's own I/O
error, predates this change. This change's unheld list and lock-free removal made routine the overlap that the
failure needs and that master made rare. Whether a master build ever failed this way on Windows, and the failure
rate at either commit, cannot be proved without native Windows runs at both, which this round did not make. The
Windows determination method O7 stays stopped.

**Not R1's cause.** R1 (§9.13.1) is about where an access's waits happen; R7 is about what an attempt covers. The
failing test runs no coordinator: four threads call the manager with `NoHooks`. So R7 is not R1's scope, and it is
repaired here as its own item.

#### 9.14.3 The repair

`visit_resolved_records` (`src/workspace_manager.rs:5699`, `impl4/r7/site/head-lines-7bb9aac3.txt`) is one tolerant
registry access, with no hold, whose attempt is the list, its parse **and the resolution of each listed path the visit
reaches**.
- **A listed path that cannot be read** (`UpstrokeError::Io` from `canonical_prefix`) fails the attempt, as a list
  Git could not finish does, and the access attempts again after its backoff, until the deadline. Nothing in the
  error tells a pending deletion from a path the filesystem denies, so nothing is classified (§5.3). The next attempt
  reads a sibling whose removal has finished as absent, or no longer lists it.
- **A path still unreadable at the deadline** refuses as `RegistryRefused`, whose message carries the read's own
  error, and never as that I/O error.
- **A path the resolution read and refused,** a link with nothing behind it (`Refusal::ReparsePointOnChain`), is not
  a failure to read. It is the access's answer at once, as before.
- **The visit** is the gate's containment comparison, or the lookup's match, unchanged and in the list's order: the
  first refusal, or the match, ends it. It runs again on each attempt.
- `revalidate_with` and `worktree_record` call it. `list_worktree_records` is the list's one reading, which
  `worktree_records` keeps as its whole attempt.

**What it keeps.** The add's veto, the removal proof, targeted removal and R-X are untouched: `Destination`,
`revalidate_removal_proving`, `remove_bound` and the holds do not change. The deadline and the refusal are the
access's own, and a gate is still one access. The new waits are the gate's existing `pause_for`, so on the
coordinator they answer its messages (R1); mutation m4 below shows it. No frozen file changes, and the helper's
contract (§5.5, §7.6) does not change.

**What it costs.** A listed worktree whose path stays unreadable, such as a foreign worktree the filesystem denies or
a link loop, stops the gate at the deadline, ten seconds, and refuses resumably, where it failed at once as I/O.
`design/15` says so in B's paragraph, and the error docs of `derive`, `revalidate` and `quiescence` name the refusal.

#### 9.14.4 Witnesses and mutations

Four tests in `src/workspace_manager/tests.rs`, `#[cfg(unix)]`, after the CI-failing test, whose assertions are
unchanged. A real sibling snapshot is added and its checkout exchanged for a link to itself, so the registry names it
and resolving it fails with `ELOOP`; for the first two the plant checks both before the call. The observer ends the
sibling's removal, by removing the link, at the call's first registry wait, with no clock.
- `a_sibling_whose_checkout_cannot_be_read_while_its_removal_is_in_flight_does_not_fail_an_add`: the add succeeds,
  after a wait through the call's hooks and a counted `Attempt` answer.
- `a_sibling_whose_checkout_stays_unreadable_refuses_the_add_resumably_and_never_as_io`: held past the deadline, the
  add refuses as `RegistryRefused`, naming the read's failure, before its intent is written.
- `a_sibling_whose_checkout_cannot_be_read_while_its_removal_is_in_flight_does_not_fail_a_verification`: the link is
  planted at the verification's `Before` hook, after its gate, and the lookup attempts again and answers
  `NotRegistered`.
- `a_sibling_whose_checkout_is_a_link_to_nothing_refuses_the_add_at_once`: the control. A link to nothing refuses as
  the link's refusal, with no `Attempt` answer.

**Red on `f9c88fdb`'s code** (`impl4/r7/base-port/start-f9c88fdb-4/`, `impl4/tools/r7-base-port.sh`): the first three
fail, each with `Io` naming `…/snapshots/k1-g0-a1-gates`, the CI's shape with another errno. The control and the
CI-failing test pass.

**Mutations** (`impl4/mutation-r7/VERDICTS.txt`, `impl4/tools/campaign-r7.py`; each on a scratch copy of `f9c88fdb` with
the round's diff and one mutation). The control is green on all five tests, and each mutation turns red exactly the
witnesses it should:

| Row | Mutation | Red |
|---|---|---|
| m1 | the gate resolves after its list's access again | the add, the bound |
| m2 | an unreadable path is the access's answer, not a failed attempt | the add, the bound, the verification |
| m3 | the lookup resolves after its list's access again | the verification |
| m4 | the gate's wait sleeps on its thread, not through the call's hooks (R1) | the add |
| m5 | a refusal the resolution made is attempted again | the control |

**What the stand-in can and cannot show about Windows.** It drives the same call, `canonical_prefix(record.path())`
in the gate and in the lookup, failing with something other than absence on a sibling the registry names and then
reading as absent once the removal ends: the path R7 took, with `ELOOP` for `ERROR_ACCESS_DENIED`. The repair does not
read the error, so the errno does not change what it does. It cannot show that a pending deletion is what Windows
answered in the CI run, that one ends within the deadline on the guest, or that the CI-failing test now passes there.
The `test (winguest)` leg is the truth for those. The witnesses do not run on Windows, where a link needs privilege.

#### 9.14.5 The frozen proof, and what else ran

**The frozen proof** (`impl4/frozen/frozen-proof-7bb9aac3.txt`, `impl4/tools/frozen-proof-r4.sh`):
- **This round's own frozen change** (`f9c88fdb..7bb9aac3`) is none: 34 of 34 files are byte-identical.
- **H1 and H2** are byte for byte as `e369b251` committed them. `integrate.rs` is that commit's blob at `f9c88fdb` and
  at `7bb9aac3`, and the frozen change since `54a1ff14` is still that one file, +33/−7, in that one commit.
- **Parts 1 and 2** are round 3's, line for line (`impl4/frozen/parts12-vs-round3.diff`, empty with the commit ids
  masked). Part 1 against master is `integrate.rs`, +33/−7, and nothing else. Part 2 against `d724fb16` is 5 files,
  +274/−81: master's four paths (+241/−74) plus H1 and H2. H1 and H2 stay proposed and not adopted.

**What else ran:**
- the committed files are the campaign's, byte for byte (`impl4/verify/commit-vs-campaign-7bb9aac3.txt`);
- at `7bb9aac3` the Windows target was linted, and type-checked on 1.85 with `-D warnings`, and the macOS target was
  linted: all three passed (`impl4/platform/code-7bb9aac3/`);
- §9.4's cites into `src/workspace_manager/tests.rs` are re-pinned to `7bb9aac3`: one hunk of 261 lines follows
  `:7294`, and each moved cite was checked against the `fn` line of a test its row names
  (`impl4/record/record-line-cites-remap.txt`). Row R7 is added;
- the ten gates ran at this round's head, and the body records them.

**CI at round 3's head, read during this round.** At `f9c88fdb`, "CI" run 37149933777 failed on `test (winguest)`
alone, job 111281445901: 2,880 passed, 1 failed and 87 ignored. The policy run passed
(`impl4/r7/ci/ATTRIBUTION-f9c88fdb.txt`). The CI-failing test of R7 passed there.
- **The failure is round 3's R-T census,** `both_drivers_run_each_transition_through_the_one_generic_function`. It
  reads `run.rs` from disk and accepts `begin_dispatch(` only when a `\n` follows it, with no `\r\n` normalised, so a
  checkout with CRLF endings fails it.
- **Reproduced on Linux.** With `run.rs` and `coordinator.rs` given CRLF endings the test fails with the guest's
  message, and the LF control passes, each arm rebuilt from its own tree (`impl4/r7/ci/crlf/`).
- **It is R1's, not R7's,** and outside this round's brief, so it is not changed here. This head's `test (winguest)`
  is expected to fail on it again, and that job's log still carries the R7 test's own verdict.

**Not verified here:** anything on Windows or macOS beyond those lint and type checks, which includes the
CI-failing test on the guest and the four witnesses on macOS, and CI's stable 1.99.0. CI is the truth for them.

### 9.15 Repair round 5: R8, the R-T census's Windows line endings

**Who and why.** `pr11_fub_impl5` (`claude-opus-5-5`, `max`), spawned by `orch_pr11` on `c8aab155` after
`test (winguest)` failed round 3's R-T census at `f9c88fdb` and again at `c8aab155`. Its brief is
`~/orch-pr11/briefs/pr11_fub_impl5.md`, and R8, with any census that fails the same way, is its whole scope. Its
figures are under `~/orch-pr11/logs/pr11_fub_impl5/`, cited as `impl5/…`. It adopts no owner decision, and H1 and H2
stay as round 3 committed them, proposed and not adopted (§9.15.5).

**The commits, on `c8aab155`:**
- `9aa1998b`: the census reads its sources with their line endings normalised, and the notes say why;
- then this text, §9.4's R8 row, the header's and §0's sentences, and the body.

#### 9.15.1 What failed, and why

**CI.** At `f9c88fdb`, "CI" run 37149933777, job 111281445901 (§9.14.5), and at `c8aab155`, run 37154037190, job
111293902494 (`impl5/ci/ATTRIBUTION-c8aab155.txt`), `test (winguest)` passed 2,880 tests, failed 1 and ignored 87,
and every other leg passed. Both times the failure is
`run::tests::both_drivers_run_each_transition_through_the_one_generic_function`, panicking at
`src\engine\topology\run\tests.rs:273:9`: "the width-1 `step` runs `begin_dispatch` through its `Stepping`
operator". At `c8aab155` R7's CI-failing test, `concurrent_snapshot_adds_and_removals_on_one_repository_never_fail`,
passed on the guest.

**The cause.** The census reads `run.rs` and `coordinator.rs` from disk and blanks them with
`crate::effects::production_code`, which keeps every byte, `\r` included, so that an offset into its output is an
offset into the source. rustfmt splits the width-1 `step`'s call of `begin_dispatch` after its parenthesis
(`run.rs:1095-1096`), so the census's needle for that call is `begin_dispatch(`, a line feed and twenty spaces. The
guest checks the tree out with CRLF endings, where the source reads `begin_dispatch(\r\n`, and the needle cannot
match. Every other needle of the census is on one line. The defect is in round 3's test alone: the product, and what
the census asserts about it, are untouched.

**Reproduced on Linux, over the whole suite** (`impl5/crlf/start/`). Two clones of `c8aab155`, one with
`core.autocrlf=true`, which gives 874 of the 878 tracked files CRLF endings (`.gitattributes` keeps the three SVGs
LF), and one without, each built from its own tree (`impl5/tools/crlf-clones.sh`, `crlf-suite.sh`):
- the LF clone passes everything: the library 3,111 passed and 130 ignored, the binary 10 (`run-lf.txt`);
- the CRLF clone fails this one test, at the same line with the guest's message, and nothing else: the library 3,110
  passed, 1 failed and 130 ignored, the binary 10 (`run-crlf-nff.txt`);
- compared test by test, one verdict differs (`VERDICT-DIFF.txt`, `impl5/tools/verdict-diff.py`).

Two runs are void and kept with their reason: one reused the other clone's binary, and one log was overwritten
(`void-binary-reuse/`, `superseded/`).

#### 9.15.2 The repair

`9aa1998b`. The census's `read` closure (`src/engine/topology/run/tests.rs:257-263`) replaces `\r\n` with `\n` before
it blanks the source, as this crate's other source censuses that match across a line break do:
`the_one_update_ref_spawn_gives_its_child_the_cleanup_lease`, `no_sampled_funnel_builds_its_argv_from_a_literal` and
`main.rs`'s `the_cli_wires_the_real_containment_step_into_dispatch`.
- **What it keeps.** The four assertions, their needles and their messages are round 3's. On an LF checkout the census
  reads the bytes it read before, and on a CRLF checkout it now reads the same text. Every transition is still shown to
  be one function, generic over its operator, run by the width-1 `step` through its `Stepping` and by the coordinator
  with itself, with no second implementation called by either.
- **Where.** At the read, not in `production_code`, whose contract is to keep every byte offset of its source: other
  censuses index the source with offsets they take from it. A token-level match would also have served, but it
  accepts layouts this census does not accept today. Normalising changes nothing but the line endings.
- **The notes.** `run/tests.rs` has a notes file, so the reason is in `docs/internals/engine/topology/run/tests.md`, in
  two new sections for the test and its `read`, and not in the source (§13 of the standards).

#### 9.15.3 Witnesses and mutations

**The matrix** (`impl5/r8/9aa1998b/VERDICTS.txt`, `impl5/tools/r8-matrix.py`). Each row is `git archive 9aa1998b`
with the row's change, built from its own tree under each ending. Each runs the census and the width-1 control,
`the_width_one_step_runs_the_same_transitions_and_its_access_waits_by_sleeping`. The CRLF arm gives every text file
CRLF endings as `core.autocrlf=true` does, so 874 files hold a CR, as in the clone. All ten cells came out as
expected:

| Row | Change | LF | CRLF |
|---|---|---|---|
| control | none | both pass | both pass |
| unfixed | the `read` without the normalisation, `c8aab155`'s text | both pass | **the census fails with the guest's message**; the control passes |
| m1 | the width-1 `step`'s dispatch runs `begin_dispatch` through a `Stepping` that carries `NoTopologyHooks`, not its own operator | both fail; the census names `begin_dispatch` | the same |
| m2 | the same for its retry's `begin_retry` | the census fails, naming `begin_retry`; the control passes | the same |
| m3 | the same for both its settlements' `settle_judged` | the census fails, naming `settle_judged`; the control passes | the same |

- **The unfixed row is R8 brought back:** red on CRLF only, at the guest's message.
- **m1 is the break R-T exists for,** at the call whose needle spans the line break: the dispatch runs through an
  operator whose registry hooks are not the run's. The behavioural control fails under it too, under both endings,
  because the tear its test plants through the run's own hooks is never planted. So the census still sees the width-1
  operator path broken under either ending.
- **m3 changes both settlements.** The census asks that some call of each transition runs through `Stepping`; a bypass
  at only one of the two settlement calls is outside what it checks. That is a reasoned limit of round 3's census, not
  R8's, and it was not executed here.

#### 9.15.4 Every other source census, checked the same way

The brief asked that every other source-text census in the crate that matches a multi-line pattern be checked for
R8's defect. The whole audit is `impl5/audit/AUDIT.md`.
- **By running them.** §9.15.1's comparison runs every test this crate compiles for Linux on a CRLF checkout. No
  function that reads repository text is gated to Windows (`impl5/audit/start/windows-only-readers.txt`), so the guest
  runs no source census that comparison did not, and only R8's changed its verdict. That covers every census whose
  positive assertion depends on line endings.
- **By reading them.** A negative assertion would pass on CRLF without having looked, which no run can show. So every
  function that reads repository text was listed (`impl5/tools/census-scan.py`, `impl5/audit/start/scan.txt`): 235
  functions, 175 of them tests. 88 hold a literal with a line feed after its first character, 464 literals in all.
  Every one of them used as a pattern or compared, 30, and every census helper that matches such a pattern against
  text a census hands it, was read: each reads normalised text, matches its own fixture, or is R8's (`AUDIT.md` §2).
  The literals that are fixtures or messages, and the patterns built by `join` or `concat!`, were read too.
- **The shared machinery** reads `\r` as rustc whitespace (`RUSTC_WHITESPACE`), and three of the lint-level censuses
  already run their readers over a CRLF form of the text on purpose.
- **Result:** R8's census is the only source census in the crate whose verdict depends on the checkout's line
  endings. No sibling has its defect, so nothing else is changed.

#### 9.15.5 The frozen proof, and what else ran

**The frozen proof** (`impl5/frozen/frozen-proof-9aa1998b.txt`, `impl5/tools/frozen-proof-r5.sh`):
- **This round's own frozen change** (`c8aab155..9aa1998b`) is none: 34 of 34 files are byte-identical.
- **H1 and H2** are byte for byte as `e369b251` committed them.
- **Parts 1 and 2** are round 4's, line for line (`impl5/frozen/parts12-vs-round4-9aa1998b.diff`, empty with the commit
  ids masked). Part 1 against master is `integrate.rs`, +33/−7, and nothing else. Part 2 against `d724fb16` is 5
  files, +274/−81. H1 and H2 stay proposed and not adopted.

**What else ran:**
- at `9aa1998b` the Windows target was linted, and type-checked on 1.85 with `-D warnings`, the macOS target was
  linted, and the Linux target was type-checked on 1.85 with `-D warnings`: all four passed
  (`impl5/platform/code-9aa1998b/`);
- §9.4 gains an R8 row, and no cite moves: the census's `fn` line is still `:255`;
- at this round's head, the whole suite on a CRLF and an LF checkout, and the ten gates; the body records them.

**Not verified here:** the census on the guest itself, the macOS leg, and CI's stable 1.99.0. CI is the truth for
them.

### 9.16 Repair round 6: the delta review's I2-1 to I2-7

**Who and why.** `pr11_fub_impl6` (`claude-opus-5-5`, `max`), spawned by `orch_pr11` on `a58c2ce3` after the delta review
of that head (§9.16.1). Its brief is `~/orch-pr11/briefs/pr11_fub_impl6.md`, and I2-1 to I2-7, with their named
siblings, are its whole scope: four P2s and three P3s. Its figures are under `~/orch-pr11/logs/pr11_fub_impl6/`, cited
as `impl6/…`. It adopts no owner decision. It withdraws H2 (§9.16.9), and H1 stays proposed and not adopted: §9.13.1's
ruling is amended to H1 alone.

**The commits, on `a58c2ce3`:**
- `0edfc509`: I2-7. The dispatch's head check waits through the coordinator by a non-frozen adapter, and H2 is
  withdrawn: `integrate.rs` is master's plus H1;
- `22d70ef6`: I2-1, I2-2, I2-3 and I2-5, in the manager, the run and the coordinator, with their witnesses and
  `design/15`'s sentence;
- `f92db934`: I2-4. The R-T census reads every call of every transition, with a positive control, a behavioural test
  pins the width-1 retry's settlement to its caller's hooks, and census 1 asserts its domain;
- then this text, §9.13.1's amended ruling, §9.4's re-pinned cites and its I2 row, the header's and §0's sentences, two
  corrected notes links, and the body.

#### 9.16.1 The delta review

Two `gpt-6-astra` lenses at `max` on `cameron-codex`, regular and regression, reviewed `a58c2ce3` once CI was green
there on every leg (run 37157677751). They ran from 22:34:24Z and 22:34:26Z to 23:15:17Z and 22:56:39Z on 2026-10-03.
Both returned CHANGES_REQUIRED, with no P1. The texts are `~/orch-pr11/reviews/review-329-i2-{regular,regression}-a58c2ce3.review.md`,
the triage is `~/orch-pr11/reviews/review-329-i2-triage.md`, and the witnesses are in `~/orch-pr11/reviews/329-i2-witnesses/`.
All 8 entries of `SHA256SUMS-329-i2-lenses` and all 3,647 of `SHA256SUMS-329-i2-witnesses` check
(`impl6/body/review-i2-hashcheck.txt`). What held: the 34 R1 witnesses, R2 to R8, the deadline, veto and R-X controls,
the frozen set (H1 and H2 only), the legacy path and activation, and the full Linux suite.

| Item | Grade | What it found |
|---|---|---|
| I2-1 | P2 | A shutdown answered inside a registry access's wait did not stop its transition. A dispatch appended `task_dispatched` and `attempt_started` and the admission arm spawned its job; an integration appended `merge_prepared` and `task_merged` |
| I2-2 | P2 | Closure and finalization ran with the caller's hooks, so their registry waits slept on the coordinator's thread |
| I2-3 | P2 | A wait whose timer thread could not be started slept on the coordinator's thread instead |
| I2-4 | P2 | R9: the R-T census showed that one correct call of each transition exists. The width-1 retry's settlement could bypass it with the suite green |
| I2-5 | P3 | An R1 witness could hang rather than fail (a wrong-wake mutation). Sibling: a timer that dies before its wake leaves the coordinator waiting on no producer |
| I2-6 | P3 | The body still said a populated destination is Git state at once, where the gate-first test shows `RegistryRefused` beside a torn sibling |
| I2-7 | P3 | H2 was avoidable: the frozen `dispatch_head` already takes its refs as a trait object its caller supplies |

#### 9.16.2 Before the repair, at `a58c2ce3`

Each item got the proof its kind calls for (the brief's table). Each run is `git archive a58c2ce3` with the reviewers'
witness diffs, whose hashes equal the review's (`impl6/repro/REPRO.md`, `impl6/tools/scratch-run.py`):
- **I2-1, I2-2 and I2-3 are red.** At dispatch the shutdown is answered and `task_dispatched` and `attempt_started` are
  appended. At integration the result is `Ok(true)`, with `merge_prepared` and `task_merged` appended. Finalization
  sleeps once on the coordinator through the caller's hooks. Under `EAGAIN` the wait sleeps 30.07 ms, the shutdown
  unanswered, with the fallback's warning.
- **I2-4's mutant survives.** Under the reviewers' two spellings of the single-call mutation, the census and the
  width-1 control pass, LF and CRLF. Under the regression lens's spelling so does the whole suite: 3,088 passed with
  `--skip real_docker`, and the binary's 10.
- **I2-5 hangs.** The wrong-wake mutation, with the intent-wait witness, never returned and never reported, until an
  external 160 s timeout (exit 124).
- **I2-6.** `a_populated_destination_beside_a_registration_already_torn_meets_the_gate_first` passes: the order the
  body's sentence contradicts.
- **I2-7.** The reviewer's H2-free adapter passes the 34 R1 witnesses and the width-1 control, with H1 kept.

#### 9.16.3 I2-1: a shutdown answered inside a wait stops its transition

**The cause.** `EffectHooks::registry_pause` returned `()`. The coordinator recorded a shutdown it answered inside a
wait, but the access attempted again, and its transition went on to append and spawn.

**The repair** (`22d70ef6`):
- **The pause says so.** `registry_pause` returns `Result<(), UpstrokeError>`. `tolerant_registry_access` and
  `with_registry` return a pause's error at once, with no further attempt. The default pause sleeps and returns `Ok`,
  so every caller outside the coordinator is unchanged.
- **The coordinator's pause stops the run.** If an interrupt is recorded when its wait ends, or before it begins (it
  then does not wait), it latches the run as stopped, naming the interrupt, and returns the error to the access.
- **The latch holds whatever a transition does with the error.** Some callers fold an error into an outcome: the
  coordinator's integration answers an interrupted one as `Ok(false)`. While the latch is set, the run's emitter
  refuses every append as `EmitFailure::Clean`, nothing appended, and the coordinator's `EffectHooks::phase` refuses
  every effect site except a kill point, so no ref moves and no worktree is made. A refused dispatch returns no job,
  so nothing is spawned. The run reopens when its coordinator has ended.
- **The deferral of halt and budget judgements is kept.** A wait defers `Judged` and `Verified` until its transition
  returns (§9.13.1), so no halt is recorded inside one, and `finish` takes the interrupt before a halt's closure runs,
  so the closure's own waits wait. No witness showed harm in the deferral.
- **The shutdown contract holds:** `finish`'s shutdown arm, "nothing was settled or appended, and the run is
  resumable", is unchanged and now true on every path. `design/15` gains one sentence: on the coordinator a wait
  answers its messages, and one that ends the command ends the access and its transition.

**The witnesses** (`CO`, `coordinator::tests`). Each pausing transition has one, through `stopped_in_its_wait`. Its tear
is finished when the shutdown is injected, so only the stop can keep the access from succeeding. It asserts that the
command ends on the shutdown, that the access had failed on the tear first, that nothing slept on the coordinator, that
nothing was appended after the injection, that the integration ref is where the log authorizes it, that the
invocations balance, and that the run reopened:
- twelve, `a_shutdown_answered_inside_…`: the dispatch's head check and its intent, a continued dispatch, a retry, a
  verification, a repair, a settlement, an integration's decision, a stale pick, a publication, a closure and a
  finalization;
- the reviewers' two, ported: `a_shutdown_answered_during_a_registry_pause_dispatches_nothing_and_appends_nothing` and
  `a_shutdown_consumed_during_a_registry_wait_publishes_no_candidate`; and the admission arm's,
  `a_shutdown_answered_during_an_admitted_dispatchs_pause_spawns_no_pipeline`;
- the latch's two halves, `a_dispatch_begun_after_a_wait_answered_a_shutdown_appends_nothing` and
  `a_publication_begun_after_a_wait_answered_a_shutdown_moves_no_ref`, and `a_wait_that_begins_after_a_shutdown_does_not_wait`.

An early probe with the pause's stop undone turned red all 15 shutdown witnesses that existed then
(`impl6/dev/probe/probe-m1-undone/`). The campaign's rows are in §9.16.10.

#### 9.16.4 I2-2: closure and finalization wait through the coordinator

**The cause.** The coordinator's `idle` and `finish` called the run's own closure, and its hard block, with the
caller's hooks, whose pause sleeps. A torn registration met at `RunFinished` slept on the coordinator's thread.

**The repair** (`22d70ef6`). `close_run` and `hard_block` are generic over the `Operator`, as round 3's transitions are,
with the steps closure calls (`reclaim_interrupted`, `complete_promotions`, `complete_publication`). The coordinator
runs them as itself, so every registry access of a closure, its scrubs and the frozen `finalize::finalize` waits through
its pause. The frozen `Finalize` borrows the fold and the events, so finalization reads them from an owned record
(`finished_record`), taken once after `run_finished`, after which nothing is appended. The width-1 `step` runs the same
functions through its `Stepping`. R-T's census names both (§9.16.6).

**R1's census, extended.** `a_closures_registry_wait_answers_on_the_coordinator` and
`a_finalizations_registry_wait_answers_on_the_coordinator`, the reviewer's witness ported, and the two closure and
finalization shutdown witnesses of §9.16.3. Every R1 witness also asserts now that no registry wait slept on the
coordinator's thread (`fixture::slept_pauses`, a test-only count of the default pause's sleeps on the calling thread).

#### 9.16.5 I2-3: no sleep on the coordinator when a timer cannot be started

**The cause.** Each wait started its own timer thread, and when it could not, slept on the coordinator's thread with a
warning.

**The repair** (`22d70ef6`). The coordinator starts one timer thread, `Timer`, before it builds its runtime, and stops
and joins it when it ends. A wait sends the thread its token and length, and the thread sleeps the length and sends
`Wake` with the token into the coordinator's inbox. No wait needs a thread of its own.
- **When the timer cannot be started,** `run_concurrently` refuses at once as `UpstrokeError::Refused`, before it builds
  its runtime or appends anything: "the coordinator's wait timer could not be started …; nothing was spawned or
  appended, and the run is resumable". The log is what it was before the call, so a resume starts from it.
- **When a wait finds the timer stopped,** which only the coordinator's own end does, it refuses at once and ends the
  command. It never sleeps.

**Classified against master: a new refusal.** Master has no timer, and no such message. Under the same exhaustion
master never reaches a wait: its `run_concurrently` builds the same runtime, byte for byte
(`impl6/timer/runtime-vs-master.txt`), and tokio panics when the worker thread cannot be started: "OS can't spawn
worker thread: Resource temporarily unavailable (os error 11)". That was executed on this tree with the timer moved
after the runtime (the row `i23-timer-after-runtime`, `impl6/mutation/i23-timer-after-runtime/test.log`). So at the
coordinator's start, where master panics, this change refuses, typed and resumable, before anything is spawned or
appended. The body discloses it.

**The witnesses**, Linux only, because they refuse `clone3` with `EAGAIN` on the coordinator's thread by a seccomp
filter (`fixture::refuse_syscall_on_this_thread`, moved there from the manager's tests):
- `a_wait_answers_a_queued_shutdown_when_no_thread_can_be_started`, the reviewer's witness ported: with no thread
  startable, a 30 ms wait answers the queued shutdown, sleeps on nothing, and leaves nothing unanswered;
- `a_coordinator_whose_timer_cannot_start_refuses_before_it_appends_anything`: the typed refusal, nothing appended,
  spawned or slept;
- `a_wait_after_its_timer_has_stopped_refuses_at_once_and_never_sleeps`, on every platform.

#### 9.16.6 I2-4: R-T for every call of every transition, and the six censuses

**The cause.** The census asked that some call of each transition run through `Stepping` or the coordinator, so a
second call could bypass both. That was the gap §9.15.3 left as reasoned and not executed.

**The repair** (`f92db934`). `run::tests::both_drivers_run_each_transition_through_the_one_generic_function` reads every
call of each of five transitions (`begin_dispatch`, `begin_retry`, `settle_judged`, `close_run`, `hard_block`) in
`run.rs` and `coordinator.rs`, line endings normalised (§9.15). Each is defined once, generic over its operator. Each
file calls it the number of times the census names, and every call passes the width-1 step's own `Stepping` or the
operator it was handed (`run.rs`), or the coordinator itself (`coordinator.rs`). It also asserts that it read the
drivers' production code: five anchors.
- **The positive control,** `run::tests::the_r_t_census_reports_one_call_that_bypasses_its_driver`: over the live
  sources it rewrites one width-1 settlement into each reviewer's spelling of R9, the coordinator's settlement into a
  foreign `Stepping`, and its closure into the run's own method, and the census reports each.
- **The behavioural test,** `every_append_of_the_width_one_step_a_retrys_settlement_included_folds_through_the_callers_hooks`:
  the width-1 `step` retries a task, and every event the run appends, the retry's settlement included, is folded
  through the hooks its caller passed.
- **Under the mutant,** both spellings, LF and CRLF, the census, its control and the behavioural test are red, and the
  width-1 control stays green (§9.16.10). The suite runs all three, so it is red under the mutant.

**The six censuses** the regular lens listed (`RESULTS.md`), each read against its own stated promise
(`impl6/censuses/ASSESSMENT.md`). All six pass at `f92db934` (`impl6/censuses/six-at-f92db934.log`).

| Census | Promise against check | Disposition |
|---|---|---|
| `run::tests::both_attempt_started_arms_take_their_pool_from_an_authority` | **exceeds**: "both arms" read only the first `AttemptStarted4` literal of each of two files. A second literal with an invented pool in `settle.rs` is green under it at `a58c2ce3` (`impl6/censuses/c1-second-arm-at-a58c2ce3/`) | **fixed** (`f92db934`): every production source is walked, exactly the two files construct it, once each, and every literal is read, with a fixture showing the counter counts a second literal and not the definition; the same mutation is red, LF and CRLF |
| `recover::tests::every_packet_named_recovery_action_has_a_production_caller` (frozen) | holds: an existential promise, checked existentially | none |
| `create::tests::the_p8_report_promises_exactly_the_resume_action_the_resume_performs` | holds jointly: it pins the sentence to the driver, and `kill_at_each_prefix_p0_to_p8_converges` and `a_resume_over_a_creation_that_stopped_after_removing_its_marker_creates_the_integration_ref_once` execute the resume | none |
| `effects::tests::every_site_the_inventory_declares_has_a_funnel_that_names_it_or_is_recorded_absent` (instrument) | holds: a name-presence promise, checked as name presence; its substring matching is already filed as `PR110-SITE-CENSUS-MATCHES-EFFECT-SITE-NAMES-BY-SUBSTRING` | none |
| `main::tests::the_cli_wires_the_real_containment_step_into_dispatch` (legacy) | holds jointly: it pins that `run` takes the wiring the tests drive, and the `cfg(windows)` `a_cli_write_command_refuses_when_the_real_containment_step_refuses` executes the real step through it | none |
| `events::log::tests::the_legacy_engine_reports_and_stops_on_a_returned_append_error` (frozen) | exceeds: a census standing for a behaviour, already filed as `PR5-C-LEGACY-APPEND-ERROR-CENSUS` (P3), whose file says so | none new |

#### 9.16.7 I2-5: every R1 witness fails within its own bound, and the lost producer

**The cause.** The R1 witnesses called the coordinator unbounded. Their prober's watchdog bounded the prober, not the
call, so a coordinator that never woke from a wait hung the test.

**The repair** (`22d70ef6`). Every R1 witness, and every new coordinator witness of this round, runs its coordinator
call inside `bounded`, which fails the test after `BOUND`, 120 s, with a message naming the scenario. Under the
reviewer's wrong-wake mutation the intent-wait witness now fails in 120.00 s with exit 101, where it hung until an
external timeout (exit 124) at `a58c2ce3` (the row `i25-wrong-wake`).

**The lost producer: prevented, and the prevention executed for an injected unwind.** If the timer died before it sent
`Wake`, the coordinator would wait on its inbox for a message nothing sends: it holds a sender of that inbox itself.
- **Why it cannot die before the wake.** The timer's sleep runs under `catch_unwind`, and the thread sends the wake
  whether the sleep returned or unwound. Its only other calls are the wait for its next request and the send, and both
  return errors rather than panic. It ends only when the coordinator drops its sender. The join reports a sleep that
  unwound as a warning.
- **Executed:** `a_wait_whose_timer_unwinds_is_still_woken_and_the_timer_serves_the_next` gives the timer a sleep that
  panics. Two waits are each woken by their own wake, and the join warns "unwound inside 2 of its sleeps". With the
  `catch_unwind` removed it fails within its bound (the row `i25-no-catch-unwind`).
- **Reasoned, not executed:** that no natural panic escapes the thread. No natural timer panic was executed.
- **Not bounded, and why.** The coordinator's thread receives with `blocking_recv`, which takes no deadline. A timed
  receive needs the runtime's time driver, which master's runtime construction does not enable, or a second producer
  for each wait, which is the thread per wait that I2-3 removes. So of the brief's two dispositions this takes the
  second, the producer shown unable to end before its wake: executed for an injected unwind, reasoned for the rest.

#### 9.16.8 I2-6: the body's populated-destination sentence

The summary said a destination that is not an empty directory at the start is Git state, with no Git command run. That
holds only once the add's gate has passed: the gate lists the registry first, so beside a sibling registration already
torn, a populated destination meets the list and refuses as `RegistryRefused` after its attempts
(`a_populated_destination_beside_a_registration_already_torn_meets_the_gate_first`, R4's test, §9.13.4). The body now
says so, as `design/15` and §9.13.4 already do. No test changes.

#### 9.16.9 I2-7: H2 withdrawn, and H1's necessity

**The route** (`0edfc509`, from the reviewer's `h2-free-adapter-without-unused-seam.diff`). The frozen
`integrate::dispatch_head` takes its refs as `&dyn IntegrationRefs`. The dispatch now hands it `run::PausingRefs`,
whose `assert_publishable` is the manager's `assert_publishable_pausing` with the operator's registry hooks, lent for
the one call: a `Cell` holds them, because the trait's method takes `&self`, and a re-entrant ask is refused, never a
panic. The run gives the head check an owned snapshot (`dispatch_inputs`): the start record and the log's last
`task_merged`, the whole of what the frozen `authorized_head` reads, so no borrow of the run spans the check. H2's
`dispatch_head_at` is gone, `integrate.rs` is master's plus H1 (blob `bf62256e`, the reviewer's), and `create.rs` is
master's.

**Validated across the round's requirements:**
- every R1 witness and the width-1 control pass, and the head check's witness waits through the coordinator;
- I2-1's fidelity at the head check: `a_shutdown_answered_inside_a_dispatchs_head_check_dispatches_and_spawns_nothing`;
- the mutations: the adapter asking the hook-less check (sleeps, two witnesses red), and the snapshot reading the
  first `task_merged` or none (a dependent's dispatch after two merges refuses its head as foreign) are each killed
  (§9.16.10);
- the gates and the frozen proofs at this round's head (§9.16.11).

So H2 is withdrawn. The owner is not asked to adopt it.

**H1's necessity, tested the same way** (`impl6/h1/H1-NECESSITY.md`). H1's three sites, `decide`'s check, `publish`'s
check and `integrate_stale`'s `proposal_state`, call methods of the concrete `&WorkspaceManager` that take no hooks.
No trait object and no hooks value reaches their waits from the caller.
- **H1 reverted** (`integrate.rs` master's, byte for byte): the library compiles. The three H1 witnesses and the
  decision's and publication's shutdown witnesses fail; the other 31 R1 witnesses, the width-1 control, the R-T census
  and the behavioural test pass (the row `h1-reverted`).
- **Route X, a pausing pre-check before the frozen `integrate()`:** the pre-check's own access waits through the
  coordinator, but the frozen `decide`'s access after it, torn, refuses as `RegistryRefused` after 16 attempts at the
  500 ms deadline, having served nothing (`impl6/h1/h1-route-precheck/`).
- **Route Y, lending the coordinator to a pause while it is the frozen integration's journal,** does not compile
  (E0499; `impl6/h1/h1-route-lend/`).
- **Reasoned, not executed:** a manager that carries the pause needs shared mutable ownership of the coordinator,
  which the frozen call holds mutably throughout; and running the frozen integration off the coordinator's thread
  moves the coordinator's own Git work, which R-S keeps on its thread.

So no caller-supplied route reaches H1's three waits, and H1 stays proposed, conditional on the owner's freeze ruling.
§9.13.1's ruling now names H1 alone.

#### 9.16.10 The mutations

**The campaign** (`impl6/mutation/VERDICTS.txt`, `impl6/tools/campaign-r6.py`, `impl6/mutation/HEAD.txt`). Each row is
`git archive f92db934` with the row's substitution, built from its own tree, running the tests it must turn red and the
tests it must leave green. Every row passes:

| Row | Mutation | Red | Green |
|---|---|---|---|
| control | none, LF and CRLF | — | 64 of 64 |
| `i21-pause-ignores-interrupt` | the wait returns `Ok` after it answered a shutdown | 19 of 19 | 37 |
| `i21-emitter-ignores-stop` | the run's emitter appends while stopped | 1 of 1 | 19 |
| `i21-effects-ignore-stop` | the coordinator's effect hooks proceed while stopped | 1 of 1 | 19 |
| `i21-wait-after-shutdown-waits` | a wait begun after a shutdown waits out its length | 1 of 1 | 17 |
| `i22-finalize-raw-hooks`, `i22-closure-raw-hooks` | finalization, or an interrupted attempt's reclaim, handed the caller's hooks | 2 of 2 each | 37 each |
| `i23-thread-per-wait` | `a58c2ce3`'s timer: a thread per wait, and a sleep when none starts | 1 of 1 | 4 |
| `i23-timer-after-runtime` | the timer started after the runtime | 1 of 1 | 3 |
| `i24-r9-literal`, `i24-r9-stepping` | R9 in each reviewer's spelling, LF and CRLF | 3 of 3 each | 1 each |
| `i24-coordinator-settle-foreign` | the coordinator's settlement through a foreign `Stepping` | 2 of 2 | 2 |
| `c1-second-arm` | a second `AttemptStarted4` literal with an invented pool, LF and CRLF | 1 of 1 | 2 |
| `i25-wrong-wake` | the timer sends the next wait's token | 1 of 1, at 120.00 s | — |
| `i25-no-catch-unwind` | the timer's sleep not caught | 1 of 1 | 1 |
| `i27-adapter-sleeps` | the adapter asks the hook-less check | 2 of 2 | 4 |
| `i27-snapshot-first-merge`, `i27-snapshot-none` | the snapshot holds the first `task_merged`, or none | 1 of 1; 2 of 2 | 2; 1 |
| `h1-reverted` | `integrate.rs` master's | 5 of 5 | 34 |
| `r1-c1-decide-nohooks`, `r1-c2-publish-nohooks`, `r1-c3-proposal-nohooks` | one H1 site given no hooks | 1 of 1 each | 3 each |
| `r1-global-sleep` | the coordinator's wait sleeps (R1 undone) | 34 of 34 | 2 |

Three rows first ran with a wrong expectation and are kept with it. The two R9 rows expected the R-T positive control
green, but it reads the live sources first, so it is red under the mutant too. `i27-snapshot-none` expected the width-1
control green, but its second dispatch, after the first merge, refuses as foreign too. Each was corrected and re-run,
and the file says so. `h1-reverted`'s list names the head check twice, so its count reads 35 of 35 for 34 tests.

#### 9.16.11 The frozen proof, and what else ran

**The frozen proof** (`impl6/frozen/frozen-proof-f92db934.txt`, `impl6/tools/frozen-proof-r6.sh`):
- **This round's own frozen change** (`a58c2ce3..f92db934`) is `integrate.rs` alone, +3/−17: H2 reverted. 33 of 34
  files are byte-identical, and the one commit touching the set is `0edfc509`.
- **H1:** `integrate.rs` against master is +16/−4, blob `bf62256e`, and every line it changes is a line `e369b251`
  changed. The incremental frozen change since `54a1ff14` is that file alone, +16/−4.
- **Part 1, against master `5c222ff2`:** `integrate.rs`, +16/−4, and nothing else.
- **Part 2, cumulative against G5's range `d724fb16`:** 5 files, +257/−78: master's four paths (+241/−74) plus H1.
  E-G6-1's two-tier rule, executed as a rule the owner has not adopted, reads `integrate.rs` as its one unenumerated
  difference. H1 stays proposed and not adopted.

**What else ran:**
- at `f92db934`, the Windows target was linted and type-checked on 1.85 with `-D warnings`, the macOS target was
  linted, and the Linux target was type-checked on 1.85 with `-D warnings`: all four passed (`impl6/platform/code-f92db934/`);
- the whole suite on a CRLF copy of `f92db934`: the library 3,114 passed and 0 failed with `--skip real_docker`, the
  binary 10 (`impl6/crlf/suite-crlf-f92db934/`);
- the legacy and activation check: no legacy module changes, `src/topology/schema.rs` is unchanged, and
  `RegistryRefused` is constructed only in `src/workspace_manager.rs` (`impl6/legacy/legacy-activation-f92db934.txt`);
- at this round's head, the ten gates; the body records them.

**An unexplained failure, recorded at repair round 8 (§9.18.2).** One whole library suite before the commits, stage s1
(`impl6/stage/s1/test.log`), was filtered: 3,087 passed, 1 failed, 130 ignored and 23 filtered out, the `real_docker`
tests. In it the frozen
`recover::tests::unsynced_merge_prepared_two_crash_barrier_before_cas_then_power_loss_keeps_log_and_ref_agreeing` failed
its "no process holds the run" observation just after it dropped a resume's handle. It passed alone
(`impl6/stage/s1/recover-rerun.log`), and in the next suites, stage s2 and this round's test gate, both unfiltered. None
of that, nor the unchanged test, establishes its provenance or its cause. It is a candidate regression of undetermined
provenance, filed with its G6 obligation as `PR329-A-DROPPED-RESUMES-RUN-STILL-READ-AS-RUNNING`.

**Not verified here:** the Windows and macOS legs, the guest, and CI's stable 1.99.0. CI is the truth for them.

### 9.17 Repair round 7: the guest's failure of round 6's integration witness

**Who and why.** `pr11_fub_impl7` (`claude-opus-5-5`, `max`), spawned by `orch_pr11` on `a337efa7` after
`test (winguest)` failed round 6's I2-1 integration witness there. Its brief is `~/orch-pr11/briefs/pr11_fub_impl7.md`,
with the orchestrator's precision `~/orch-pr11/answers/pr11_fub_impl7-1.md`: the guest's leg runs rustc 1.97.1, so the
failure is a difference between Windows and Linux, not between compilers. That failure and its same-cause siblings are
the round's whole scope. Its figures are under `~/orch-pr11/logs/pr11_fub_impl7/`, cited as `impl7/…`. It adopts no
owner decision and changes no frozen file and no production code. H1 stays proposed and not adopted (§9.17.5).

**The commits, on `a337efa7`:**
- `c3bde5fd`: the tear's prober samples after every wait, the one a cancel ends included, and its reports name what it
  waited for; two tests force the order; the notes say why;
- then this text, §9.4's cites re-pinned to it with a W7 row, the header's and §0's sentences, and the body.

#### 9.17.1 What failed

**CI at `a337efa7`** (`impl7/ci/ATTRIBUTION-a337efa7.txt`). "CI" run 37174143085 failed on `test (winguest)` alone, job
111353134914, and its aggregate `upstroke-ci` with it. The library suite passed 2,904 tests, failed 1 and ignored 87.
Every other leg passed: test on ubuntu and macOS, lint on all three platforms, and MSRV on all three. "Pull request
policy" run 37174143048 succeeded. Two runs created a second earlier, 37174142730 and 37174142767, ended cancelled.

**Leg, platform and compiler.** The guest is Windows, with Git 2.50.1.windows.1 and a CRLF checkout (§9.15). Its step
refuses any compiler but rustc 1.97.1, this box's stable, and it passed that check (the job log's line 106, quoted in
the attribution file).

**The failure.** The test is
`engine::topology::coordinator::tests::a_shutdown_consumed_during_a_registry_wait_publishes_no_candidate`, round 6's I2-1
integration witness (`22d70ef6`, new in this change). It panicked at
`src\engine\topology\coordinator.rs:6445:18` on "the access reached contention". Its prober (`Wakes::Contended`),
cancelled once `coordinator.integrate` had returned, reported that "no invocation reached or left the runner while the
registration planted at … stayed torn". The witness's own assertions never ran.
- Round 6's other shutdown witnesses passed on the guest. One is
  `a_shutdown_answered_inside_an_integrations_decision_prepares_and_publishes_nothing`: the same transition's same
  access, over the same torn shape. It reads the contended count itself.

#### 9.17.2 The cause: the prober's report, not the access

**Where.** `Plant::tear`'s prober (`coordinator.rs:4462-4489` at `a337efa7`), written for round 3's R1 witnesses
(`10cc88d8`). At the top of each turn it sampled what it waits for, then waited up to 1 ms for a cancel. On a cancel it
reported the tear standing at once, with no fresh sample (`:4479`). So whatever happened during its last wait was
reported as nothing.

**Why only this witness.** Every other caller whose prober samples has an access that cannot complete until the
prober has finished the tear, so its prober reports `Ok` long before the cancel: the R1 witnesses, the width-1 control,
and the closure and finalization waits. The `stopped_in_its_wait` shutdown witnesses use `Wakes::Never` and read the
count themselves. This witness's access ends at the queued shutdown, not at the tear:
1. it counts its failed attempt (`note_contended`);
2. its first wait answers the shutdown;
3. the integration returns, the coordinator is dropped, and the test sends the cancel.

Nothing waits for the prober.

**Why Linux passed and the guest did not.**
- **The window.** From the counted attempt to the cancel is 1.15 to 1.20 ms on Linux, median 1.18 ms, in 30 of 30 runs
  (`impl7/start/e0d-head-diag/window.txt`). It is the coordinator's first 1 ms backoff, timed by its timer thread, and
  then the return. The prober's 1 ms waits put a sample inside it, and the witness passed 30 of 30 there
  (`impl7/start/e0-head/`).
- **The guest's waits.** The guest's std is rust-lang/rust at tag `1.97.1`, the commit this box's rustc 1.97.1 names
  (`impl7/windows-std/SOURCES.md`, each file's path and line):
  - the prober's `recv_timeout(1 ms)` parks with `WaitOnAddress` and a time-out in whole milliseconds;
  - the timer's `thread::sleep(1 ms)` is a high-resolution waitable timer.
- **The clock.** Microsoft documents that since Windows 10 2004, a process that has not raised its timer resolution
  gets no better than the default for its waits' time-outs. Nothing in the test binary raises it
  (`impl7/windows-std/timer-resolution-search.txt`), and the default is commonly 15.625 ms. So on the guest a sample
  lands inside the window of about a millisecond only by chance.
- **This is reasoned, not measured.** The round did not use the guest, and the guest's Windows version and timer
  resolution were not read.

**Reproduced on Linux by a stand-in** (`impl7/start/`, `impl7/tools/l7-run.py`, `specs-start.py`). Each row is
`git archive a337efa7` with its substitutions, built from its own tree.
- **What the stand-in is.** It widens only the prober's wait between samples, from 1 ms to 15.625 ms. It runs the same
  test, the same integration, the same access and the same prober. It cannot show the guest's actual waits or window.
- **Alone, 30 of 30 runs fail** with the guest's message, identical once the planted path is masked and the guest
  log's CR dropped (`MESSAGE-COMPARE.txt`). DIAG lines show that the access counted its attempt in all 30 runs, and that
  the prober reported it in none (`e1-standin/`).
- **Among the module's 156 tests**, the witness failed in all 5 runs with the DIAG lines, the attempt counted each time
  (`e1-module-standin-diag/`), and passed in the one run without them (`e1-module-standin/`). No other test of the
  module failed under the stand-in. The outcome is a race whose odds the sampler's period sets.
- **So whether the guest fails it every time is not established.** Sampling at the clock tick, it fails unless a tick
  lands in a window of about a millisecond.

**Product or fixture: the fixture.** The access contended on the tear in every stand-in run. With the prober repaired,
the same product code under the same sampler stopped the integration, appended nothing and published nothing in 30 of
30 runs (§9.17.4, `control-standin`). No product difference is in evidence: on the guest, the decision's shutdown
witness passed, meeting the same access over the same tear and reading the count without a prober. The prober's report
was untrue, and its message also said that "the coordinator served nothing while its access waited on it", which for
this wake was never the question.

#### 9.17.3 The repair

`c3bde5fd`, in `src/engine/topology/coordinator.rs`'s `mod tests` and its notes, and nowhere else:
- **Every report follows a sample.** `Plant::tear_sampling_every(every)` waits at most `every` at a time, a cancel
  ending the wait early, and samples after each wait.
  - It returns `Ok`, the tear finished, when the sample shows what it waits for.
  - Otherwise it reports the cancel, or its deadline once `BOUND` has passed.
  - The test sends the cancel after the call it watches has returned. So anything that call counted is seen by the
    sample after the cancel, however long the wait was. `tear` samples every millisecond, as before.
- **Its reports say what it did not see** (`Wakes::not_seen`). For this wake that is "no registry access failed on the
  tear", where every report used to describe the runner.
- **The witness is unchanged:** its function is byte-identical to round 6's, plant, wake and assertions
  (`impl7/record/witness-unchanged.txt`).
- **The plant is already what Git for Windows writes:** `HEAD`, a `gitdir` spelt with `/` as Git writes it, and an empty
  `commondir` (§9.12, §9.13.6). It is not the cause: the guest's decision witness meets the same shape, and the stand-in
  reproduces the failure with it.
- **The notes** carry the reason (§13 of the standards): sections for `fn tear_sampling_every`, `impl Wakes`, the two
  tests and their helpers, and the witness's own.

#### 9.17.4 Witnesses and mutations

**The matrix** (`impl7/final/c3bde5fd-VERDICTS.txt`, `impl7/tools/specs-matrix.py`, `verdicts.py`). Each row is
`git archive c3bde5fd` with its change, built from its own tree. Every row came out as expected:

| Row | Change | Runs | Result |
|---|---|---|---|
| `control-witness` | none | 30 | the witness passes 30 of 30 |
| `control-standin` | the stand-in sampler, with DIAG lines | 30 | the witness passes 30 of 30; the attempt counted and reported in 30 |
| `control-prober` | none | 10 | both prober tests pass 10 of 10 |
| `m1-cancel-unsampled` | the cancel reported before the sample after it: the start head's order | 10 | `a_prober_cancelled_after_the_attempt_it_waits_for_reports_it_and_finishes_the_tear` fails 10 of 10; its pair passes |
| `m1-standin` | m1, with the stand-in sampler and DIAG lines | 30 | the witness fails 30 of 30: the attempt counted in 30, reported in none. The guest's failure, brought back |
| `m2-cancel-as-progress` | the cancel reported as progress, with no sample | 10 | `a_prober_cancelled_before_anything_it_waits_for_leaves_the_tear_and_says_so` fails 10 of 10; its pair passes |
| `i21-control-sampler`, `i21-standin` | round 6's `i21-pause-ignores-interrupt` (the wait returns `Ok` after it answered a shutdown), with each sampler | 3 each | the witness fails every run: `merge_prepared` and `task_merged` appended, and `Ok(true)` |
| `module-control`, `module-standin`, `module-standin-diag` | none; the stand-in sampler; the stand-in with DIAG lines | 1, 1 and 5 | the module's 158 tests pass in every run |

- **The prober tests force the order.** Their prober's only wait is `BOUND` long, so the attempt is counted and the
  cancel sent while it waits, with no sample between them. m1 fails the first every time, and m2 the second. On Linux
  the witness cannot tell the repaired prober from the start head's, since both pass at 1 ms, so these two are the
  repair's regression tests, on every platform.
- **The witness still proves I2-1 with a sampler as coarse as the guest's.** With I2-1 undone, the prober still
  finishes the tear in time for the access's next attempt, and the integration appends and publishes, which the
  witness refuses.

#### 9.17.5 Siblings, the frozen proof, and what else ran

**Same-cause siblings.**
- **Every user of `Plant` shares the prober,** so the repair covers each of them. Under the stand-in at `a337efa7`, the
  module's 156 tests ran six times, five of them with the witness's DIAG lines, and only this witness failed, in 5 of
  the 6 (`impl7/start/e1-module-standin*/`). At `c3bde5fd`, under the stand-in, all 158 passed in 6 of 6.
- **`TearsAForeignRegistration`'s writer** (round 3's R3, `coordinator.rs:4030-4065`) reports a cancel in the same
  order. It is not a `Plant` user, and no caller can lose its sample. The one test that cancels it after an attempt
  may have been counted, `a_verification_beside_another_processs_registration_write_in_flight_spends_no_deferral`, does
  so only after a run that cannot complete until the writer has written. It is unchanged. This is reasoned from its
  three callers, not executed.

**The frozen proof** (`impl7/frozen/frozen-proof-c3bde5fd.txt`, `impl7/tools/frozen-proof-r7.sh`):
- **This round's own frozen change** (`a337efa7..c3bde5fd`) is none: 34 of 34 files are byte-identical.
- **H1:** `integrate.rs` is blob `bf62256e`, round 6's, +16/−4 against master. H1 alone, byte for byte.
- **Parts 1 and 2** are round 6's, line for line (`impl7/frozen/parts12-vs-round6.diff`, empty with the head masked).
  Part 1 against master `5c222ff2` is `integrate.rs`, +16/−4, and nothing else. Part 2 against `d724fb16` is 5 files,
  +257/−78. H1 stays proposed and not adopted.

**What else ran:**
- at `c3bde5fd`, the Windows target was linted and type-checked on 1.85 with `-D warnings`, the macOS target was
  linted, and the Linux target was type-checked on 1.85 with `-D warnings`: all four passed
  (`impl7/platform/code-c3bde5fd/`);
- the whole suite on a CRLF copy of `c3bde5fd`, with `--skip real_docker`: the library passed 3,115, failed 1 and
  ignored 130, with 23 filtered out, and the binary passed 10. The one failure is the frozen
  `recover::tests::a_resume_over_a_creation_that_stopped_after_creating_its_integration_ref_adopts_it`, its resume
  refused on the run's cleanup lease. The same executable then passed it alone 5 of 5, and the library suite again with
  the same filter, 3,116 passed, 0 failed and 23 filtered out (`impl7/crlf/suite-crlf-c3bde5fd/`, `impl7/crlf/rerun/`).
  Its refusal is the one the filed `PR281-CLEANUP-LEASE-HOLD-OUTLIVED-AND-ITS-UNREADABLE-TWIN` records in other tests:
  a fingerprint match, its cause in this sighting unproven. Repair round 8 files it as
  `PR329-A-CREATION-PREFIX-RESUME-REFUSED-ON-A-HELD-CLEANUP-LEASE`, and supersedes `impl7/crlf/ATTRIBUTION.md`'s
  attribution (§9.18.3);
- the legacy and activation check matches round 6's, but for one test line in `coordinator.rs` moved by this round's
  added lines (`impl7/legacy/vs-round6-c3bde5fd.txt`);
- §9.4's cites re-pinned to this head, and a W7 row;
- at this round's head, the ten gates; the body records them.

**Not verified here:** the guest itself, meaning whether the repaired witness passes there and how long its waits last.
CI is the truth for that, and for the macOS leg and CI's stable 1.99.0.

### 9.18 Repair round 8: the delta review's I3-1 and I3-2, two sightings recorded as found

**Who and why.** `pr11_fub_impl8` (`claude-opus-5-5`, `max`), spawned by `orch_pr11` on `519cfc9e` after the delta
review of repair rounds 6 and 7. Its brief is `~/orch-pr11/briefs/pr11_fub_impl8.md`, and I3-1 and I3-2 are its whole
scope. Its figures are under `~/orch-pr11/logs/pr11_fub_impl8/`, cited as `impl8/…`. It changes no source file, no test
and no frozen file: only this record, two new files under `findings/`, and the body. It adopts no owner decision, and
H1 stays proposed and not adopted.

**The commit, on `519cfc9e`:** the two findings, this text, the corrections it lists (§9.18.4), and the header's and
§0's sentences; then the body.

#### 9.18.1 The delta review

- **The lenses.** Two `gpt-6-astra` lenses at `max` on `cameron-codex`, regular and regression, on `519cfc9e` and the
  delta `a58c2ce3..519cfc9e`, after CI there was green on every leg. Both returned CHANGES_REQUIRED, with the same two
  P3 evidence-contract findings, and no P1, no P2 and no production defect. The texts are
  `~/orch-pr11/reviews/review-329-i3-{regular,regression}-519cfc9e.review.md`, and the triage is
  `~/orch-pr11/reviews/review-329-i3-triage.md`.
- **What held**, by the triage: I2-1 to I2-7 as repaired, round 7's prober repair within its stated limits, the frozen
  set (H1 alone, +16/−4), the legacy path and activation, and the new coordinator tests on the native legs.
- **The two items.**
  - **I3-1:** round 6's record and body set an unexplained failure of a frozen recovery test outside the round,
    without establishing its provenance.
  - **I3-2:** round 7's record and body promoted a lease refusal's match with PR281's fingerprint into a proven cause,
    and left its filtering unsaid.

#### 9.18.2 I3-1: round 6's recovery failure, recorded as found

**The sighting** (`impl6/stage/s1/`; the filter and the tree are read in `impl8/sightings/s1-filter-and-tree.txt`):
- **What ran.** Round 6's stage s1: the whole library suite, on this box, of the round's first staged tree. By its test
  list and its time that is `0edfc509`'s tree: none of the 24 coordinator tests that `22d70ef6` adds ran in it, and its
  log ends 30 s before `0edfc509` was committed. The tree itself was not saved.
- **It was filtered.** 3,087 passed, 1 failed, 130 ignored and 23 filtered out. The 23 are exactly the tests whose
  names contain `real_docker`, which s2 ran, so a `--skip real_docker` filter; s1's command line was not saved. The
  binary's tests have no result line in that log.
- **What failed.** `recover::tests::unsynced_merge_prepared_two_crash_barrier_before_cas_then_power_loss_keeps_log_and_ref_agreeing`,
  at `recover/tests.rs:7120`, with "two-crash: and no process holds the run". Its helper,
  `the_runs_first_resume_by_an_incarnation_that_then_dies`, asserts in one observation that `rundir::is_running`
  answers false once the first resume's handle is dropped.
- **What ran after it** (`impl8/sightings/later-runs.txt`). The same executable passed the test alone, once
  (`stage/s1/recover-rerun.log`). **The next two suites were unfiltered and green:** stage s2, with 3,135 passed, 0
  failed, 130 ignored and 0 filtered out, and the binary's 10 (`impl6/stage/s2/test.log`); and round 6's test gate at
  `a337efa7`, with 3,137, 0 and 130, 0 filtered out, and 10 (`impl6/gates/final-a337efa7/03-test.log`). It also
  passed in the four later suites of rounds 6 and 7 that the file lists.

**What it is: an unexplained candidate regression of undetermined provenance.**
- **Its provenance is not established.** An isolated pass, later green suites and an unchanged frozen witness show
  neither that the failure predates this change nor its cause. This change alters what runs beside the test: a timer
  thread per coordinator, registry waits, concurrent fixtures. That can move the test's overlap with process and lease
  operations elsewhere in the suite. It is a possible influence, not a cause in evidence.
- **The assertion names no holder.** `rundir::is_running` (`src/rundir.rs:2356`, master's) answers true in five cases
  (`impl8/sightings/is-running-at-519cfc9e.txt`): this process's own claim on the run; a lock file it cannot open; a
  holder of the primary lock; a primary lock it cannot inspect; and, with the primary lock free, a cleanup lease that
  is held or cannot be observed. So the assertion does not tell the primary lock, the cleanup lease and an inspection
  failure apart.
- **No filed finding is this one's.** No finding names this test or this message (`impl8/sightings/ledger-search.txt`).
  `PR281-CLEANUP-LEASE-HOLD-OUTLIVED-AND-ITS-UNREADABLE-TWIN` records other witnesses and other messages.
- **The same assertion has failed in two other tests** (`impl8/sightings/SIGHTINGS.txt`):
  - in follow-up A's regression suite at A's head, a tree without this change. That lens produced the same message on
    A's merge base by construction, with a parked fork holding a copy of the run's cleanup lease (A's record, §5.2);
  - in design round 5's prototype (§4.3).

  Neither is this witness, neither ran it before this change, and neither identifies what answered here. So they
  classify nothing here.

**Filed** as `PR329-A-DROPPED-RESUMES-RUN-STILL-READ-AS-RUNNING` (P2, `correctness`, `deferred`, provenance
`undetermined`), in `findings/P2_correctness_202610040608_a-dropped-resumes-run-still-read-as-running.md`.

**Its final-range G6 obligation**, carried in the file:
- keep this sighting separately;
- count any recurrence of this witness failing its `is_running` observation as red;
- classify it `pre_existing` only on a reproduction before this change, or on causal evidence of what held the run.

#### 9.18.3 I3-2: round 7's lease refusal, a fingerprint

**The sighting** (`impl7/crlf/`):
- **What ran.** The whole suite on a CRLF copy of `c3bde5fd`, on this box:
  `cargo test --all-targets --all-features --no-fail-fast -- --skip real_docker`, as `impl7/tools/scratch-run.py` runs
  the spec `impl7/crlf/suite-crlf-c3bde5fd/spec.json`.
- **It was filtered.** The library passed 3,115, failed 1 and ignored 130, with 23 filtered out; the binary passed 10.
  The same executable then passed the test alone 5 of 5, and the library suite again with the same filter: 3,116
  passed, 0 failed, 130 ignored and 23 filtered out (`impl7/crlf/rerun/`). **Neither suite was unfiltered.**
- **What failed.** `recover::tests::a_resume_over_a_creation_that_stopped_after_creating_its_integration_ref_adopts_it`,
  at `recover/tests.rs:17809`. The fixture's first resume, which waits for no lease, refused: "… still has a process of
  its own alive … and that process holds the run's cleanup lease; refusing overlapping engine ownership".
  `WorktreeLock::acquire_in_hooked` (`src/rundir.rs:1906`) gives that refusal when its one observation of a run's
  cleanup lease reads held. An observation that cannot be made reads held too (`observe_cleanup_hold`,
  `src/rundir.rs:2140`).

**What it is: a PR281-compatible fingerprint, its causation in this sighting unproven.**
- **A fingerprint, not a cause.** `PR281-CLEANUP-LEASE-HOLD-OUTLIVED-AND-ITS-UNREADABLE-TWIN` records this refusal from
  other recovery tests' resumes, not from this one. It attributes them, by construction and not per sighting, to a
  copy of the run's lease descriptor that a sibling's fork inherited during a ref write, and says that which fork held
  it in each natural sighting is not identified. Five isolated passes and a green rerun prove no mechanism here.
- **The same test has failed with the same refusal twice before** (`impl8/sightings/SIGHTINGS.txt`):
  - in G5's S1 at `d724fb16`, a tree before PR11 and this change, unfiltered (`reviews/2026-09-25-gate-G5.md`, F16
    and §2.1);
  - in design round 6's prototype of this change (§5.8, qualified there).

  The test's text is the same at `d724fb16`, master and this head
  (`impl8/sightings/creation-test-body-d724fb16-vs-519cfc9e.txt`). So the fingerprint predates this change. What held
  the lease is identified in none of the three, so this sighting's provenance stays undetermined.
- **It classifies nothing else**, I3-1's failure included: that is a different assertion, whose observation reads more
  than the lease.
- **Round 7's evidence file is left as written.** `impl7/crlf/ATTRIBUTION.md`'s attribution, and its grouping of this
  sighting with round 6's, are superseded here.

**Filed** as `PR329-A-CREATION-PREFIX-RESUME-REFUSED-ON-A-HELD-CLEANUP-LEASE` (P2, `correctness`, `deferred`,
provenance `undetermined`), in `findings/P2_correctness_202610040608_a-creation-prefix-resume-refused-on-a-held-cleanup-lease.md`.

**Its final-range G6 obligation**, carried in the file: count every occurrence matching this fingerprint as red. A
match is not a cause, and it classifies no other failure.

#### 9.18.4 How each is recorded, and the sweep

**Why two findings, and not ledger rows alone.** `MAINTAINING.md` step 5 gives every open finding its own file. Its
lane table has this `fix-p0p1` lane fix P0 and P1 before ready, and a finding a lane need not fix is filed and
deferred: one file under `findings/`, with a `deferred` ledger row. `findings/PROCESS.md` §7 defers to that. Both
also say that a witnessed defect, a finding carrying a failing test, a reproduction or a mutation witness, is fixed
whatever its label. Each file records an open finding, filed `deferred`; which rule settles each at B's merge is not
decided here (corrected at repair round 9, §9.19).
- **I3-1's sighting** matches no filed finding, so it is a new one.
- **I3-2's sighting** matches PR281's fingerprint, but its cause is unproven, so it is not recorded as PR281
  recurring. Filing it on its own also keeps PR281's file, another finding's, outside this round's writes
  (`findings/PROCESS.md` §4).
- **Both are P2**, as PR281 is, and neither is graded down.
- **What each carries, and what this round did not do** (corrected at repair round 9, §9.19). Each finding carries an
  archived failing-test witness: one failure in one filtered whole suite, after which its test passed alone, in later
  suites and in this round's test gate. Those passes do not invalidate an intermittent failure, and step 5's rule does
  not need the current repairer to reproduce a witness (the review of `ce55ca91`, I4-1). This round's scope was text
  and evidence: it ran neither witness to reproduce its failure and investigated neither cause. The investigation each
  file sets out is deferred: make a red name its holder, then run the witness over the whole suite, with and without
  this change for I3-1's, and after PR281's change for I3-2's. Whether step 5 requires either finding to be fixed before
  B's merge, or the guards its file states carry it, is for B's merge triage, under `MAINTAINING.md` and the
  supervisor. This record neither asserts nor waives it.
- **I3-1 and I3-2 themselves**, the record's and the body's sentences, are fixed here, with `fixed` ledger rows:
  `FUB-I3-S1EXCLUSION` and `FUB-I3-CRLFFINGERPRINT`.

**The sweep** (`impl8/sweep/`). The record and the body were searched for every sentence that set either sighting
aside, attributed it beyond a fingerprint, or did either to a sibling sighting of the same shape. Each one found was
corrected:

| Where | What it said | What it says now |
|---|---|---|
| §4.3 | design round 5's red of the same assertion, read as an observation of the lease alone and given to PR281's class | an `is_running` observation that a lease copy is one way to fail; what answered not established |
| §4.10 | the same red, labelled a known intermittent, its holder stated as a sibling's inherited lease | the same qualification, and no attribution to PR281 |
| §5.8 | design round 6's lease refusal, given to PR281's class | the test named (it is I3-2's witness), a fingerprint match, its holder not identified |
| §5.10 | the lease and Docker reds, labelled as known intermittents | called a refusal and a failure |
| §9.16.11 | I3-1's sighting, set outside round 6 | as §9.18.2 |
| §9.17.5 | I3-2's sighting, given PR281's cause, its filtering unsaid | as §9.18.3, with the filtering |
| the body | the same two sightings, as §9.16.11 and §9.17.5 had them; round 7's proof line silent on its failure | as here |

**What is not changed:** the facts each of those places records, and the evidence files of earlier rounds, which stay
as their sessions wrote them.

#### 9.18.5 The frozen proof, and what else ran

- **The frozen proof** (`impl8/frozen/`, `impl8/tools/frozen-proof-r8.sh`): this round's own frozen change is none, 34
  of 34 files byte-identical; `integrate.rs` is blob `bf62256e`, H1 alone, +16/−4 against master; and parts 1 and 2
  are round 7's, line for line.
- **No source file changes:** the round's diff is this record and the two findings.
- **CI at `519cfc9e` was green on every leg:** "CI" run 37178335687, all ten of its jobs, and "Pull request policy" run
  37178335706. The two runs created two seconds earlier on that head, 37178334080 and 37178334129, ended cancelled
  (`impl8/ci/`).
- **At this round's head, the ten gates;** the body records them.

**Not verified here:** either sighting's cause, and whether either recurs. No run was made to reproduce either in this
round; the test gate's one whole suite passed both, which leaves both archived failures standing (corrected at repair
round 9, §9.19).

### 9.19 Repair round 9: the delta review's I4-1, the witnessed-finding exemption withdrawn

**Who and why.** `pr11_fub_impl9` (`claude-opus-5-5`, `max`), spawned by `orch_pr11` on `ce55ca91` after the delta
review of repair round 8. Its brief is `~/orch-pr11/briefs/pr11_fub_impl9.md`, and I4-1 is its whole scope. Its figures
are under `~/orch-pr11/logs/pr11_fub_impl9/`, cited as `impl9/…`. It changes no source file, no test, no frozen file and
no finding: only this record and the body. It adopts no owner decision, and H1 stays proposed and not adopted.

**The commit, on `ce55ca91`:** this text, the corrections it lists (§9.19.2), and the header's and §0's sentences; then
the body.

#### 9.19.1 The delta review

- **The lenses.** Two `gpt-6-astra` lenses at `max` on `cameron-codex`, regular and regression, on `ce55ca91` and the
  delta `519cfc9e..ce55ca91`, after CI there was green on every leg. They ran from 06:56:30Z and 06:56:32Z to 07:04:30Z
  and 07:01:24Z on 2026-10-04. The regression lens returned PASS with no findings. The regular lens returned
  CHANGES_REQUIRED with one P3, and no P1 and no P2. The texts are
  `~/orch-pr11/reviews/review-329-i4-{regular,regression}-ce55ca91.review.md`, and the triage is
  `~/orch-pr11/reviews/review-329-i4-triage.md`.
- **What held**, by the triage: I3-1 and I3-2 as round 8 recorded them; both findings' form, pin, provenance and G6
  guards; the sightings' history; and the scope, with `src/` unchanged, the 34 frozen files unchanged and H1 +16/−4,
  the body and branch validators passing, and the 91 earlier ledger rows byte-identical.
- **I4-1 (P3).** §9.18.4 concluded that "step 5's rule for a witnessed finding does not apply", because round 8
  reproduced neither failure and both tests passed afterwards. `MAINTAINING.md` step 5 covers a finding carrying a
  failing test, a reproduction or a mutation witness on its own terms. It does not need the current repairer to
  reproduce it, and passing reruns do not invalidate an intermittent failure: the saved failures remain evidence.
- **A question the triage records and does not decide.** Each of the two findings carries an archived failing-test
  witness. Whether they must be fixed before B's merge, or are carried by the guards they state (PR281's change, and
  final-range G6), is for B's merge triage, under `MAINTAINING.md` and the supervisor, to whom the triage flags it.

#### 9.19.2 The repair, and the sweep

**The exemption is removed, and nothing is decided in its place.** §9.18.4's fourth bullet now states what each finding
carries, round 8's text-only scope and the deferred investigation, and leaves the question to B's merge triage. It
neither declares step 5's rule for a witnessed finding inapplicable nor says how it applies. Both findings stay as
filed, byte for byte: `deferred`, with their guards and G6 obligations.

**The sweep** (`impl9/sweep/`). Every paragraph, list item and table row of the record and the body that mentions
either sighting, either finding or either test, and every statement of what B's merge waits on or of what stays open,
was read for the exemption, stated or implied: that a pass after the failure, or a round that did not reproduce it,
takes a finding outside step 5's rule, or that the findings' deferral settles them. Each one found was corrected:

| Where | What it said | What it says now |
|---|---|---|
| §0, the implementation's row | what the merge waits on, and round 8's two findings, each filed with its G6 obligation | the same, and round 9's sentence: the question is left to B's merge triage, neither asserted nor waived |
| §9.18.4, the lead | the lane paragraph's first half: a finding a lane need not fix is filed and deferred | its second half too, which `findings/PROCESS.md` §7 shares: a witnessed defect is fixed whatever its label; which rule settles each finding is not decided |
| §9.18.4, the fourth bullet | "Neither carries a reproduction": the later passes, and round 8 running neither, so step 5's rule did not apply and both were deferred | each carries an archived failing-test witness, which the passes do not invalidate; round 8's text-only scope and the deferred investigation; the question left to B's merge triage, neither asserted nor waived |
| §9.18.5, "Not verified here" | the test gate's pass of both, beside "no run was made to reproduce either" | the same facts, and that the pass leaves both archived failures standing |
| the body, the merge dependencies | the four owner decisions B's merge waits on, then that filing or deferring clears no P1 and no G6 guarantee, and nothing of the two findings | the same, and that a deferral does not settle the two findings: the question is B's merge triage's |
| the body, "What stays open" | the two findings, filed with their G6 obligations | the same, and the question |
| the body, the test gate | "Both sightings' tests passed in it" | the same at this head, and that the pass leaves both archived failures standing |

**Read and left as written** (`impl9/sweep/CLASSIFICATION.txt`): the places that record a pass as a fact and draw
nothing from it, among them §9.16.11's and §9.18.2's later runs, each of which says those runs establish neither
provenance nor cause; the design rounds' prototype reds in §4.3, §4.10, §5.8 and §5.10, which were never findings and
which round 8 qualified; §9.11's list of what the merge waits on, written at the implementation before either finding
existed, as it was before H1, and completed by §0; and the two finding files, which this round does not change.

**What is not changed:** both findings, byte for byte; every earlier ledger row, byte for byte, with one row added for
I4-1, `FUB-I4-WITNESSEXEMPTION`; the facts §9.18 records; and the evidence files of earlier rounds.

#### 9.19.3 The frozen proof, and what else ran

- **The frozen proof** (`impl9/frozen/`, `impl9/tools/frozen-proof-r9.sh`): this round's own frozen change is none, 34
  of 34 files byte-identical; `integrate.rs` is blob `bf62256e`, H1 alone, +16/−4 against master; and parts 1 and 2
  are round 8's, line for line.
- **No source file changes:** the round's diff is this record alone.
- **CI at `ce55ca91` was green on every leg:** "CI" run 37183255786, all ten of its jobs, and "Pull request policy" run
  37183255917. The two runs created two seconds earlier on that head, 37183254413 and 37183254417, ended cancelled
  (`impl9/ci/`).
- **At this round's head, the ten gates;** the body records them.

**Not verified here:** either sighting's cause, whether either recurs, and how step 5's rule applies to the two
findings, which is B's merge triage's. No run was made to reproduce either in this round.

### 9.20 The step-5 round: W1 and W2 diagnosed, and the proposed frozen hunk H3

*Restated at the CAS-1 round (§9.21.9).* Where this section read beyond its evidence, it is restated in place to the
supervisor's six points (`~/babysit-pr11/evidence/cold-cache-step5-direction-20261004.md`). Its `reviewed_sha` values,
its figures and its frozen-diff comparison are unchanged, and H3 stays proposed.

**Who and why.** This round is `pr11_fub_step5`'s (`claude-opus-5-5`, `max`), spawned by `orch_pr11` on `55029628`.
- **What started it.** The supervisor's step-5 merge triage
  (`~/babysit-pr11/evidence/b-step5-merge-triage-20261004.md`) found round 8's two P2 findings to be
  mandatory merge work under `MAINTAINING.md` step 5 and `findings/PROCESS.md` §7, because each carries an archived
  failing test. B stays a draft and out of the queue until each is fixed with reviewed evidence or validly rejected.
- **The brief and the scope.** The brief is `~/orch-pr11/briefs/pr11_fub_step5.md`, with the precision
  `~/orch-pr11/answers/pr11_fub_step5-1.md`. The two witnesses are its whole scope. Its figures are under
  `~/orch-pr11/logs/pr11_fub_step5/`, cited as `step5/…`.
- **Two phases.** An isolated diagnosis came first, run on `git archive` copies under this session's own build pools,
  with nothing written to this branch. Its report, `step5/REPORT.md`, was written before any repair.
  Then came the repair.
- **What it adopts.** No owner decision. H1 stays proposed, and H3 is proposed beside it, not adopted.

**The commits, on `55029628`:**
- `3ce7bb46`: H3, with the notes in `docs/internals/engine/topology/recover/tests.md`;
- then this text, the two findings, and the header's and §0's sentences;
- then the body.

#### 9.20.1 What the witnesses demonstrate

**W1** (`PR329-A-DROPPED-RESUMES-RUN-STILL-READ-AS-RUNNING`).
- The helper `the_runs_first_resume_by_an_incarnation_that_then_dies`, which six tests share, makes the first resume
  and drops its handle. It then observes `rundir::is_running` once.
- That first resume makes exactly one lease-holding ref write, its integration ref, through `update_ref` and
  `rundir::hold_cleanup_lease_for_child`: one window in every attributed run (`step5/REPORT.md` §2.1).
- `is_running` reads "running" in three ways (`src/rundir.rs:2356`):
  - **the primary lock:** this process's claim, or another process's `fcntl` lock;
  - **the cleanup lease:** the primary lock free, and `flock(LOCK_EX | LOCK_NB)` answering `EWOULDBLOCK`;
  - **an observation error,** each read as held: an unopenable lock file, `F_GETLK` failing, or any failed `flock`.
- The witness's message does not tell these apart.

**W2** (`PR329-A-CREATION-PREFIX-RESUME-REFUSED-ON-A-HELD-CLEANUP-LEASE`).
- The creation body writes P8's integration ref through the same `update_ref`, then makes the fixture's first resume.
  The resume's worktree lock observes the run's lease once.
- Who could hold the lease at that point:
  - **no reaper:** the fixture spawns no agent;
  - **no live ref-writing child:** P8's child has exited before `update_ref` returns, and with an empty hooks
    directory and no fsmonitor it leaves no descendant;
  - **a copy of P8's descriptor** in any process this test process forked while it was open, held until that process
    execs or exits (`src/rundir.rs:2174-2186`);
  - **an observation that fails.**
- **Why #320's remedy does not cover it.** #320's wait, `await_previous_incarnations_release`, runs before a fixture's
  second and later resumes only. This resume is the fixture's first, because the prefix is the creator's work done in
  this process. And #320 changed no production behaviour: its `src/rundir.rs` change is comments only
  (`step5/diag/history.txt`).

#### 9.20.2 Reproduction, natural apart from constructed

**The archive** (`step5/diag/census/windows.txt`): 1,624 complete library suites on the box, read-only, classed by
test list.
- **W1's own test** failed only on trees with this change: 2 of 65 suites. Those are impl6's s1, and C's `83516466`
  attempt 1 (3,096/2/132, unfiltered), which is #329 at `54a1ff14` plus C's change, so it predates nothing. It failed in
  0 of the 1,263 suites without this change that ran it.
- **The helper's assertion** failed once without this change: follow-up A's lens, `answer-file`.
- **W2's refusal** occurred three times without this change, each unfiltered: G5's S1 at `d724fb16`, `e980146` and
  `78f99c70`.
- **No difference is readable** within the one window that has both kinds of tree. These heterogeneous suites
  establish no rate effect, and no absence of exposure this change induces.

**Natural runs** (`step5/repro/natural/SUMMARY.txt`).
- **The runs:** 11 whole-suite runs at master and 11 at this change's head, in pairs, the order alternating from the
  second pair.
- **The trees:** identically instrumented copies. The instrumentation records which branch `is_running` and `is_held`
  answered from, and only on a failure appends a re-probe, `/proc/locks` and a scan of every holder of the run's lock
  files. From pair 7 it first stops this process's direct children, then continues them.
- **The command:** `cargo test --all-targets --all-features --no-fail-fast -- --skip real_docker`, so that no
  fixed-name container is shared with another lane's suite.
- **Load:** 1.18 to 24.37. Three effects censuses fail on the instrumentation's own text in every run, at both trees.
- **The two natural failures, each attributed by deduction to the lease branch, neither holder captured:**
  - **at this head, W2** (`head-05.log`): the scan read the lease held, the file opened and `flock` answered
    `EWOULDBLOCK`, and a re-probe 27 µs later was still held. P8's write had held the lease for 16.6 ms. The hold was
    gone before the scan reached its holder.
  - **at master, the helper's observation in `open-log-truncate-error`,** a different test that shares W1's helper
    (`base-08.log`): the primary lock free, the
    lease `EWOULDBLOCK`. The first resume's one write had lasted 30.8 ms. The hold was gone about 0.6 ms later, before
    35 children could be frozen.
  - **The deduction, in each:** the observation did not fail, and no reaper or live ref-writing child of the run
    existed. So a copy of the step's own lease descriptor in a forked process held it:
    `PR281-CLEANUP-LEASE-HOLD-OUTLIVED-AND-ITS-UNREADABLE-TWIN`'s class. The process itself was not captured, and
    neither holder, nor any of the older archived failures, is attributed beyond that deduction and the evidence
    captured.
- **What did not happen:** W1's own test did not fail in any of the 22 runs. No rate or rate difference is read from
  them, and no absence of exposure this change induces.

**Constructed** (`step5/repro/CONSTRUCTED.txt`; each one test, `--exact`; a construct shows what can happen, never
what did).
- **Point constructs.** A planted lease copy, a python `lockf` holder of `run.lock`, this process's claim, and
  `run.lock` or `cleanup.lock` at mode 000 each fail W1 with its message, and the attribution names each correctly. A
  planted copy fails W2 with its message, and #320's wait releasing the copy before the observation or the resume makes
  either pass.
- **The injected fork construct,** PR281's: a sibling thread forks every millisecond, and each child keeps its
  inherited descriptors 30 ms. Its counts below are that construct's, not natural failure rates.
  - W1: 3 of 10 at `d724fb16`, 7 of 10 at master, 6 of 10 at this head.
  - W2: 8 of 10, 4 of 10 and 6 of 10.
  - With the children closing their inherited descriptors first: 0 of 10 everywhere it ran.
  - In all 46 failures of the final construct versions, the holder named is one of the forker's unexec'd children.

**First bad, by construction** (`step5/diag/history.txt`).
- **W2:** 0 of 10 at `d1c82601`, where the creation body still ran on the fake refs. 4 of 10 at its child
  `6a5324e7`, which gave the P7 and P8 witnesses the repository's own integration ref.
- **W1:** 4 of 10 at `523dac5f`, the assertion's first commit.
- Both came to master with #311 (`9bb177ea`, 2026-09-22).
- These controls are constructions. W1's own test reproduced before this change only so, and the controls await
  independent review.

**Unsuccessful or void, and said so** (`step5/REPORT.md` §3.5):
- 20 of the 22 natural runs reproduced nothing;
- neither natural holder was captured;
- the first construct version could not name holders;
- 30 first-bad runs did not compile, because the construct named `ParkedFork`, which #320 added later, and were redone
  without it;
- a light census of copies after every ref write was aborted: its probe cannot tell a copy from a legitimate holder of
  the same run. Nothing is drawn from it.

#### 9.20.3 Classification

- **Both are read as one pre-existing defect class,** a reading that awaits independent review. Nothing here shows that
  this change causes either, and the heterogeneous natural suites establish neither a rate effect nor the absence of
  exposure this change induces. Each observes
  the run's cleanup lease once, right after a ref write this test process made, in a process whose other test threads
  fork. The design names the copy and puts the waiting on the recovery tests. #320 waits before every later resume;
  W1's observation and W2's first resume are the two places it does not reach.
- **Neither witness is invalid:** each reproduces on this head by construction, and W2 also failed there naturally
  once, attributed by deduction.
- **W2:** pre-existing, first bad `6a5324e7`, both by construction and both awaiting independent review.
- **W1:** pre-existing, first bad `523dac5f`, both by construction and both awaiting independent review.
  - Its finding's own test for that is a reproduction before #329's change or causal evidence of what held the run.
    This round has a deduced cause, with no holder captured, and one natural failure at master of the observation
    W1's test makes, in a different test that shares W1's helper. W1's own test reproduced before this change only
    by construction.
  - W1's own test did not fail naturally at master, and its two archived sightings stay individually unattributed
    (`step5/REPORT.md` §4.2).

#### 9.20.4 H3, and PROPOSED RULING B-W

*Revised at the B4 round (§9.22.3).* This subsection describes H3 as first proposed, blob `74c0b121`, and its ruling
text. The i5 review found that its waits read an inspection error as a held lease (I5-1, §9.22.2). Both sites now
wait through `await_own_lease_copies_release`, W2's no longer counts its prefix in `resume_attempts`, and §9.22.3's
revised hunk and PROPOSED RULING B-W replace the description and the ruling below. What a "no" would mean is
unchanged in kind.

**H3** (`3ce7bb46`; `src/engine/topology/recover/tests.rs` alone, +133/−3 against master):
- **W1.** The helper's observation is now `assert_no_process_holds_the_run`. It first makes #320's wait
  (`wait_for_cleanup_hold_release_observing`, bounded by the fixture's `release_bound`, acknowledged through
  `holder_observed`), then reads `rundir::is_running` once. A red keeps the witness's message as its prefix and adds the
  wait's result and what acquiring the run lock answers. So it names the claim (this process's pid), another holder of
  the primary lock (its pid), an observation error, or a lease held past the bound.
- **W2.** The creation body counts its in-process prefix as a previous incarnation (`resume_attempts` + 1), so its first
  resume waits as every later one does, through #320's own wait and refusal annotation. It takes a `before_the_resume`
  hook, which its two tests pass as nothing.
- **Three regression tests** (Unix, with #320's `ParkedFork`):
  - `a_lease_copy_a_sibling_fork_kept_from_the_first_resume_is_waited_out_before_its_death_is_read`;
  - `a_lease_copy_that_outlives_the_bound_still_fails_the_first_incarnations_death`;
  - `a_resume_over_a_creation_prefix_waits_out_a_lease_copy_a_sibling_fork_kept_from_its_ref_write`.

**PROPOSED RULING B-W (on a G6-frozen test child), in RULING P-1's form. NOT ADOPTED.**

> The owner permits exactly one change to `src/engine/topology/recover/tests.rs`, a whole-file test child that G6
> requires byte-identical to the G5 range save E-G6-1's enumerated changers. **H3:**
> `the_runs_first_resume_by_an_incarnation_that_then_dies`'s last check becomes `assert_no_process_holds_the_run`
> (the bounded wait every later resume makes, then one `rundir::is_running`, a red naming the wait and the run lock's
> answer); `a_resume_over_a_creation_that_stopped_after_its_marker_was_removed_converges` counts the prefix it wrote in
> this process as a previous incarnation before its first resume, and takes a `before_the_resume` hook; and the three
> regression tests above. Nothing else in the file changes: no other test's assertion, wait or bound. No production
> file and no other frozen file changes. G6's module diff proof reads this hunk, with H1, as the only differences
> permitted beyond E-G6-1's, bound to the merge SHA of the change that makes it.

**The exact hunk** is `recover/tests.rs` at this change's head against master (`step5/phase2/frozen/`):
- `git diff --numstat` gives +133/−3, and the blob is `74c0b121`;
- master's blob, `765d9b36`, is `55029628`'s;
- it is the prototype that the diagnosis executed, byte for byte (`step5/repro/repair/`).

**Its purpose.** These are the two witnessed findings that step 5 makes mandatory, and H3 is proposed as their repair
where the diagnosis places the defect. Whether it repairs the tests faithfully, without weakening their obligation,
awaits independent review. The
recovery tests resume and observe in the process that drove the run, and the design assigns them the wait for this
process's own copies; these two did not make it.

**It is not adopted.** The canonical packet and every owner grant are untouched. Admitting the hunk is the owner's
freeze ruling, and until that ruling this pull request does not meet G6's frozen-set rule, as with H1.
- **What a "no" would mean:** revert H3, so that `recover/tests.rs` is master's byte for byte. The library still compiles
  and both witnesses keep their single observation. Both findings stay filed, and step 5's merge obligation for them
  stays unmet; it is not a waiver.
- **What depends on H3:** nothing outside the file, for compiling or for behaviour. With H3 reverted the three regression
  tests go with it. Mutating H3 back to the pre-repair behaviour at either site turns its regression tests red with the
  original fingerprints (§9.20.5).

**Considered and not chosen** (`step5/REPORT.md` §5.2):
- **Closing the lease's inheritance in production.** That is
  `PR281-CLEANUP-LEASE-HOLD-OUTLIVED-AND-ITS-UNREADABLE-TWIN`'s open remedy: larger, touching R28's holder and an effect
  allowlist, and not needed for these two witnesses. It stays PR281's.
- **Making every first resume wait.** #320 keeps first resumes unwaited so that the planted-hold refusal tests stay
  meaningful.
- **A hand-written wait in the body.** It would lack #320's annotation and holder hook.

#### 9.20.5 Witnesses and mutations

All of this is at `3ce7bb46`'s tree, identical by sha256 to the prototype the diagnosis executed
(`step5/phase2/dev/`, `step5/repro/repair/SUMMARY.txt`).
- **Formatting and lint:** `cargo fmt --check` passes. `cargo clippy --all-targets --all-features -- -D warnings`
  passes on Linux, and for `x86_64-pc-windows-msvc`, `x86_64-apple-darwin` and `aarch64-apple-darwin`.
  `cargo +1.85.0 check --locked --all-targets --all-features` passes under `-D warnings`.
- **Tests:** the three regression tests, W1, W2, W2's P7 twin and the other five helper tests: 11 of 11 passed.
- **Mutations, each the pre-repair behaviour at its site:**
  - **M1,** the observation without the wait, fails both W1 regression tests: "`<tag>`: and no process holds the run:
    the cleanup lease's wait Ok(()), the run lock's acquisition Err(Refused { … cleanup.lock … })".
  - **M2,** the first resume uncounted, fails W2's regression test with W2's exact refusal.
- **The injected fork construct against the prototype:** the 1 ms forker fails W1 0 of 10 and W2 0 of 10 (854 and
  782 forks), where it failed each 6 of 10 at `55029628`. These are an injected construct's counts, not natural
  failure rates. They show what the construct does, not that H3 keeps each test's obligation, which awaits
  independent review.

#### 9.20.6 The findings and the ledger

- **Both findings stay filed**, because H3 is proposed and not adopted. The change that merges H3 deletes them.
- **What is unchanged:** `reviewed_sha`, `id`, severity, `deferred`, `category` and `location`.
- **Provenance** becomes `pre_existing`. **First bad** is `523dac5f` (W1) and `6a5324e7` (W2). Both rest on the
  diagnosis and its constructions, and await independent review.
- **The guard** is the owner's freeze ruling on H3, with final-range G6 counting any recurrence as red until then.
- **A dated section in each** adds the sightings and evidence above. W1's adds the two sightings it did not list, C's
  `83516466` and `3b4d1d79`, with the precision's reading: both are on trees containing this change, and they
  classify nothing.
- **The body's two ledger rows** carry the same.

#### 9.20.7 The frozen proof, and what else ran

- **The frozen proof** (`step5/phase2/frozen/`, `step5/tools/frozen-proof-step5.sh`):
  - this round's one frozen change is `recover/tests.rs`, H3; 33 of the 34 files are byte-identical;
  - `integrate.rs` is blob `bf62256e`, H1 alone, +16/−4 against master;
  - part 1 against master: two files, +149/−7;
  - part 2 against `d724fb16`, E-G6-1 executed as a rule not adopted: FAIL, with the two proposed hunks unenumerated.
- **CI at `55029628` was green on every leg:** "CI" run 37186359003, all ten of its jobs, and "Pull request policy"
  run 37186358977. The two runs created a second earlier on that head, 37186358009 and 37186358023, ended cancelled
  (`step5/phase2/ci/`).
- **At this round's head, the ten gates.** The body records them.

**Not verified here:**
- the individual process that held either natural copy;
- the holders of the archived sightings;
- any rate, and any absence of exposure this change induces;
- whether a different-process resume can meet such a copy in production, which is PR281's;
- the class attribution, the first-bad controls, and whether H3 repairs the tests faithfully without weakening their
  obligation, which await independent review;
- any production cause, and any closure.

### 9.21 The CAS-1 round: a publication's compare-and-swap re-check waits through the coordinator

**Who and why.** This round is `pr11_fub_cas1`'s (`claude-opus-5-5`, `max`), spawned by `orch_pr11` on `4253b2ca`, on an
isolated local branch, to be integrated into this pull request only on the orchestrator's clearance.
- **What started it.** Follow-up C's integration round met it, did not change it, and routed it here
  (`~/orch-pr11/questions/pr11_fuc_impl4-1.md`, "Not C's"). Its dev probe recorded the call stack of every registry
  access a coordinator's thread starts, and during an integration two of them were the swap's re-check, made with no
  hooks (`~/orch-pr11/logs/pr11_fuc_impl4/probe/ACCESS-MAP.txt`, INT 14 and INT 33). The supervisor made it an unresolved
  responsiveness obligation of this change, to be met with faithful, bounded red and green evidence, mutations and a
  formal disposition before the merge.
- **The brief and the scope.** The brief is `~/orch-pr11/briefs/pr11_fub_cas1.md`: the re-check, its witnesses, R1's
  census and the sibling sweep. Its figures are under `~/orch-pr11/logs/pr11_fub_cas1/`, cited as `cas1/…`.
- **What it adopts.** No owner decision. H1 and H3 stay proposed and conditional, and this round changes neither.

**The commits, on `4253b2ca`:**
- `3f5d1760`: the repair, its four witnesses, and the two coordinator witnesses' notes;
- `03fdf500`: the census that would have caught it, and its positive control;
- then this text, §9.4's re-pinned cites and its CAS1 row, §9.13.1's corrected sentence, and the header's and §0's
  sentences.

#### 9.21.1 What was wrong

- `WorkspaceManager::compare_and_swap_ref` takes hooks. Its publishability re-check, made before its funnel, called
  the hook-less `assert_publishable`, which hands `assert_publishable_pausing` the `NoHooks` observer. So every wait of
  that registry list slept on the calling thread, whatever hooks the caller had passed.
- The frozen `integrate::publish` hands the swap `journal.hooks().effects()`, which on the coordinator is the
  coordinator itself (§9.13.1). So during an integration's publication, a re-check that met a registration it could not
  list slept on the coordinator's thread until the list passed or the access's deadline came: 10 s in production, 500 ms
  under test. No pipeline was served meanwhile. That is the wait R1 removed from every other access the coordinator
  makes (PR11's R-F: the coordinator's only wait is for its next message).
- **Where it came from: master's call, this change's waits.** Master `5c222ff2`'s `compare_and_swap_ref` makes the same
  hook-less call. But master's list made one attempt, under R-X held as a blocking lock, with no backoff
  (`git show 5c222ff2:src/workspace_manager.rs`, `worktree_records`). On a registration it could not list, it failed
  at once as Git state, which is the defect this change repairs.
  - The waits that slept arrived with this change's tolerant access, `58c7c203`.
  - R1 (§9.13.1) routed every other such wait through the coordinator, but its census listed C2 as `publish`'s own
    check and missed the swap's second access. So §9.13.1's sentence that every manager function taking hooks routes
    its accesses' waits through them was not true of this one, and that sentence is corrected in place.
  - So CAS-1 is `FUB-I1-COORDINATORWAIT`'s class, introduced by this change's feature, at a site that repair missed.
  - This round's question, following follow-up C's, called it master's line unchanged by #329, and the orchestrator's
    answer accepted a ledger provenance on that basis. That is true of the call and not of the wait, so the ledger
    row is classified by the wait, as R1's own row is (§9.21.9).

#### 9.21.2 Reproduction: red at `4253b2ca`

- **How it was run.** The four witnesses of §9.21.4 were added to `4253b2ca` without the repair
  (`cas1/repro/witnesses-only.diff`, sha256 `fdf6c15c25325bf5…`) and run with three controls
  (`cas1/repro/red-at-4253b2ca/`).
- **All four fail, each on the stall itself:**
  - the coordinator's served witness fails with `RegistryRefused` after 16 attempts at the 500 ms deadline, having
    served no pipeline;
  - the coordinator's shutdown witness fails "the shutdown was injected while the tear stood": the coordinator slept
    through the wait, so it never reached a point where the shutdown could be answered;
  - the manager's two fail with `RegistryRefused` after 16 attempts: the call's hooks were never asked to wait, so
    nothing mended the tear, and for the stop the registry's refusal came back where the wait's own error should have.
- **The controls pass:** `publish`'s own check's witness (C2), its shutdown witness, and the width-1 control.
- **Which access the tear meets.** It is planted before the second registry access the coordinator's thread starts
  after beta's `merge_prepared` is folded. `publish`'s own check is the first and the swap's re-check the second;
  `direct_ref_target` between them makes no registry access. Two pieces of evidence pin this. The probe of §9.21.6
  records the re-check's stack, `assert_publishable ← compare_and_swap_ref ← integrate::publish`. And the mutations
  separate the two accesses: with the swap's line hook-less its witnesses are red and C2's green, and with `publish`'s
  own line hook-less the reverse (§9.21.5).

#### 9.21.3 The repair

- **What changed.** `compare_and_swap_ref` calls `self.assert_publishable_pausing(hooks, refname)`, so the re-check
  waits through the call's own hooks, as every other access of a function that takes hooks does. It still runs before
  the funnel opens, and the checks' order, the funnel and its effect are unchanged.
- **What it does not change.** No frozen file changes. No effect row is needed: `compare_and_swap_ref` and
  `assert_publishable_pausing` are both already listed in `effects/wrappers.toml`. No new sleep: this removes one from
  the coordinator's thread.
- **No new kind of refusal.** A wait the caller's hooks end now ends the swap too, with that wait's own error, before
  its funnel opens. On the coordinator that is the refusal §9.16.3 gave every routed wait: a shutdown answered inside
  the wait stops its transition, and the command ends with the shutdown's refusal, `UpstrokeError::Refused`,
  resumably. The body's risk paragraph already discloses that for "such a wait". This round adds one wait to the
  waits it covers.
- **Classified against master.** Master's re-check made one attempt.
  - A registration it could not list failed the publication at once with a Git error.
  - While another thread of the process held R-X, it blocked the coordinator's thread until the holder released it,
    and a shutdown waited unanswered meanwhile.
  - Before this round, this change made that re-check wait with backoff on the coordinator's thread, up to 10 s,
    answering nothing.
  - Now the coordinator answers its messages during those waits. A shutdown answered inside one ends the swap before
    its funnel: the ref does not move, nothing is appended after `merge_prepared`, and the command ends with the
    shutdown's refusal, the run resumable, as after a shutdown inside C2's wait under H1.
  - Against master, that resumable stop replaces either a Git error at once or an unanswered wait. It is §9.16.3's
    refusal, already disclosed for every routed wait, so this round adds no kind of refusal.

#### 9.21.4 R1's census, extended, and the census that would have caught it

**The census row.** R1's census (§9.13.1) gains **C6**: the frozen `publish` (`integrate.rs:480`) calls
`manager.compare_and_swap_ref(journal.hooks().effects(), …)`, whose re-check `assert_publishable_pausing` lists the
registry through `worktree_records_with`. Its handle is H: the call takes the coordinator's hooks, which the manager
dropped before this round.

**Its witnesses** (`CO`), each bounded by `bounded` (120 s):
- **`a_pipeline_is_served_while_a_publications_swap_recheck_waits_on_a_torn_registration`** (`:4978`), in R1's form.
  Two pipelines run at width 2, and the tear stands before the second registry access after beta's `merge_prepared`.
  The prober finishes it only after an invocation has reached the runner after the tear, which needs the coordinator
  to answer a grant during the wait. The witness passes only if the access then completes, the run finishes and no wait
  slept on the coordinator's thread.
- **`a_shutdown_answered_inside_a_publications_swap_recheck_publishes_nothing`** (`:6165`), in I2-1's form through
  `stopped_in_its_wait`. The command ends on the shutdown, and the access had failed on the tear first. Nothing slept
  on the coordinator, and nothing was appended after the shutdown: `merge_prepared` once and no `task_merged`. The ref
  is where the log authorizes it, the invocations balance, and the run reopens.

**At the manager** (`WM`): `a_swaps_publishability_recheck_waits_through_the_calls_hooks` (`:4837`) and
`a_wait_that_ends_a_swaps_publishability_recheck_moves_no_ref` (`:4877`).
- A sibling task's registration is torn the way a killed `git worktree add` leaves it.
- The call's hooks either mend it at their first wait or end the access there.
- In the first, the swap passes, no wait slept on the calling thread, and the ref moves.
- In the second, the swap ends with the wait's own error, its funnel never opened, and the ref is where it was.

**The census that would have caught it** (`WM`): `no_function_that_takes_hooks_reaches_a_registry_wait_that_sleeps_by_default`
(`:5165`), with its positive control `the_hooks_routing_census_reports_a_swap_that_drops_its_hooks` (`:5228`).
- **What it reads.** The manager's production files: `src/workspace_manager.rs`, and every child it declares outside a
  test-only item. Comments, strings and test-only items are blanked (`effects::production_code`), and line endings are
  normalised.
- **What it decides.** A function reaches a sleeping wait when its body names `sleep_for` or `NoHooks`, or calls by name
  a function that does. `EffectHooks::registry_pause` is the seam itself and is not read as one. A function that takes
  hooks and reaches one is reported: it drops its hooks, so on the coordinator that access's waits sleep on its thread.
- **Its domain is asserted:** the declared children are read, more than 200 functions, the eight hook-less entry points
  are found by name as reaching a sleeping wait, and six hooked functions as taking hooks.
- **What it reports.** At `4253b2ca`, `compare_and_swap_ref` alone; here, nothing (§9.21.5).
- **Its control** puts CAS-1 back into the live source, and exactly `compare_and_swap_ref` is reported. It also reports
  a wait reached through a helper and a pausing twin handed `NoHooks`, does not report a wait asked of the caller's
  hooks, and does not read the violation written as prose.
- **Its limits,** stated where it is defined. Calls are matched by name, so another type's method of the same name
  reads as one. A wait reached through a function value, a trait object, a closure handed in from outside these files,
  or hooks of another type a function builds for itself is not seen. It reads the manager alone, so an engine call site
  that has hooks and calls a hook-less function is the sweep's (§9.21.6).
- **What kind of file it is.** It tests the manager's own routing: a subject, governing nothing outside the manager.

#### 9.21.5 Mutations

The campaign is `cas1/mutation/VERDICTS.txt` (runner `cas1/tools/mutate.py`, head `03fdf500`).
- **How a row was run.** Each row is `git archive 03fdf500` with one substitution, built from its own tree (its compile
  line names the copy), and runs 13 tests: this round's six; C2's two witnesses; the decision's and a dispatch intent's
  R1 witnesses; the width-1 control; R-T; and the swap's substitution test.
- **How it is judged.** A row passes when exactly its expected tests are red and all 13 ran. Six of six pass:

| Row | Mutation | Red | Green |
|---|---|---|---|
| control | none | none | 13 |
| `cas1-hookless-recheck` | the re-check asks the hook-less `assert_publishable`, `4253b2ca`'s line | the four witnesses; the census, naming `compare_and_swap_ref`; its control | 7 |
| `cas1-nohooks-recheck` | the re-check is the pausing twin handed `NoHooks` | the same six | 7 |
| `h1-publish-check-nohooks` | `publish`'s own check hook-less (H1's line), the re-check routed | C2's two witnesses | 11 |
| `r1-global-sleep` | the coordinator's wait sleeps (R1 undone) | both CAS-1 coordinator witnesses, C2's two, the decision's and the intent's | 7 |
| `i21-pause-ignores-interrupt` | the coordinator's wait returns `Ok` after it answered a shutdown | the two publication shutdown witnesses | 11 |

The census's control is red under the first two rows by design: it first reads the live source's routed spelling,
which those mutants remove.

#### 9.21.6 The sibling sweep

**The question.** Is there any other manager call, reached from the coordinator with hooks available, that drops them
into a hook-less registry access?

**Inside the manager** (`cas1/sweep/STATIC.txt` §1 and §2).
- Every registry access goes through `tolerant_registry_access`, called at five sites. Two take the call's hooks; three
  forward a pause their callers choose.
- The only callers that choose a sleep are the eight hook-less entry points: `revalidate`, `worktree_records`,
  `quiescence`, and the five twins that hand their pausing form `NoHooks`.
- The census finds no function that takes hooks and reaches one of them, other than `compare_and_swap_ref` before this
  round.

**Engine call sites** (`STATIC.txt` §3). None is on CAS-1's cause:
- attempt bodies and a verification's body call the hook-less entry points on pipeline threads under the coordinator
  (`Coordinator::spawn_attempt` and `spawn_verification`). A pipeline's own waits sleep on its own thread by design
  (§9.13.1), and its hooks are its own;
- creation and a resume's recovery call them before any coordinator exists;
- the frozen `integrate.rs` reaches them through `run::PausingRefs` (I2-7) or as H1's pausing twins.

**Executed** (`cas1/sweep/DYNAMIC.txt`, from a dev probe that is never committed, `cas1/tools/apply-probe.py`).
- **What it records.** Every registry access identifies the wait it was handed, the default sleep or the
  coordinator's, and records its call stack. It ran over the topology suite (`engine::topology::`, `--skip real_docker`).
- **At `03fdf500`:** 785 passed and 0 failed. 16,634 accesses waited through the coordinator and 38,691 by the default
  sleep, and **none of the sleeping ones was on a coordinator's thread** (a stack holding `Coordinator::drive`). The
  21,807 sleeping accesses that carry coordinator frames are on pipeline threads, from `Coordinator::spawn_attempt`
  (16,454) and `spawn_verification` (5,353) on the blocking pool.
- **Its positive control, with CAS-1 put back:** 711 sleeping accesses on a coordinator's thread, every one at
  `assert_publishable ← compare_and_swap_ref`. 783 passed, and the two CAS-1 coordinator witnesses failed.
- **What follows.** Over what the suite drives, CAS-1 was the only registry access on a coordinator's thread that
  slept, and none remains.

**Listed, not changed** (`STATIC.txt` §4). Two Windows-only filesystem retries sleep 25 ms between attempts, up to 39
times, while handles close.
- `remove_tree_once_handles_close` (`src/workspace_manager.rs:1513`) is reached from a worktree's removal, which the
  coordinator makes on its own thread.
- `read_marker_once_handles_close` (`:6617`) is the same retry for a registration's `locked` marker.
- Both are master's code, neither is a registry access, neither takes a pause seam, and neither is CAS-1's cause.
- They are open observations, with this evidence. Whether R-S's "synchronous work it already did at width 1 (its own
  Git, filesystem and appends)" or R-F governs them is undecided and goes to B's next review. They are not changed,
  and no finding is filed for them in this round (the orchestrator's answer, item 4).
- *Updated at the B4 round (§9.22.5).* The i5 review's regular lens reads R-S's explicit allowance for existing
  synchronous filesystem work as governing these unchanged primitives. A held handle can still delay the
  coordinator's messages through their 39 × 25 ms sleeps, and the reviewer did not execute Windows, so its reading is
  no evidence that those calls stay responsive. Nothing is decided on it, and they stay open observations.

#### 9.21.7 Under slow Git

- **Why it was measured.** Follow-up C's round 5 found that I2-1's shutdown harness fails when each Git process starts
  late enough (`~/orch-pr11/handovers/pr11_fuc_impl5.md`, in progress), with a stand-in that starts every `git` late
  (`~/orch-pr11/logs/pr11_fuc_impl5/standin/slowgit/git`). The new shutdown witness uses that harness, so it was
  measured the same way, with a copy of the stand-in (`cas1/slowgit/SUMMARY.txt`). Every `git` was started 0, 170, 250,
  400 and 600 ms late, one run each, and 400 ms twice.
- **What it showed, as measured.** Both CAS-1 coordinator witnesses passed up to 400 ms per Git process and failed at
  600 ms, as C2's two witnesses and the decision's two did. The dispatch intent's shutdown witness failed from 170 ms.
- **What is not claimed.** No rate, and nothing about any platform, beyond those runs on this box. The harness
  (`TearHeld`, `stopped_in_its_wait`) is unchanged here, and follow-up C's proposed repair of it is not in this round.

#### 9.21.8 The frozen proof, and what else ran

- **The frozen proof** (`cas1/frozen/frozen-proof-03fdf500.txt`, `cas1/tools/frozen-proof-cas1.sh`):
  - this round changes no frozen file: 34 of 34 are byte-identical to `4253b2ca`'s;
  - `integrate.rs` is H1's blob `bf62256e` and `recover/tests.rs` is H3's `74c0b121`, both unchanged;
  - part 1 against master: those two files, +149/−7;
  - part 2 against `d724fb16`, E-G6-1 executed as a rule not adopted: FAIL, with the two proposed hunks unenumerated.
  All four are as at `4253b2ca`.
- **The platforms** (`cas1/platform/code-03fdf500/`): the Windows target linted and type-checked on 1.85 with
  `-D warnings`, the macOS target linted, and the Linux target type-checked on 1.85 with `-D warnings`. All four
  passed.
- **At this round's head, the ten gates.** The body records them.

**Not verified here:** the Windows and macOS legs, the guest, and CI's stable 1.99.0. CI is the truth for them.

#### 9.21.9 Integration, the step-5 precisions, and the ledger

- **Clearance.** The orchestrator cleared the integration at 2026-10-04T12:14:40Z (`~/orch-pr11/answers/pr11_fub_cas1-1.md`).
  #329's head was then `4253b2ca`, this branch's base, and a fetch at the integration found it unchanged
  (`cas1/integration/pr329-at-integration.json`). So no merge commit was needed, and the push is a fast-forward.
- **The step-5 precisions.** The answer's item 3 directed that the record and the body say no more than the supervisor's
  six points (`~/babysit-pr11/evidence/cold-cache-step5-direction-20261004.md`). Accordingly:
  - **The fork construct's 6 of 10 against 0 of 10** is an injected construct's count, not a natural failure rate.
  - **The heterogeneous natural suites** establish no rate effect, and no absence of exposure this change induces.
  - **The natural holders,** and the individual older archived failures, stay unattributed beyond the evidence
    captured.
  - **W1's natural sighting at master** was a different test sharing W1's helper, and W1's own reproduction before
    this change was constructed.
  - **Still owed independent review:** the pre-existing class attribution, the first-bad controls, and whether H3
    repairs the tests faithfully without weakening their obligation.
  - **No wholesale claim** of a production cause or a closure.
  These are restated in place in §0, §9.20.2, §9.20.3, §9.20.4, §9.20.5, §9.20.6 and §9.20.7, and in the body's
  step-5 paragraph, proof list, Validation bullets and the two findings' ledger rows. The `reviewed_sha` values and the
  frozen-diff comparison are unchanged, and H1 and H3 each stay proposed and conditional, neither adopted.
- **The ledger row** `FUB-CAS1-SWAPRECHECK` (P2, `liveness`, `fixed`).
  - Its guard is the four witnesses and the census.
  - Its provenance is `introduced_by_feature`, first bad `58c7c203`, prior ID `FUB-I1-COORDINATORWAIT`. The sleeping
    wait it records is this change's, as §9.21.1 says, and R1's own row is classified the same way.
  - The question proposed, and the answer accepted, a provenance for master's unchanged line. That holds for the call
    alone, so the row is classified by the wait, and the change is disclosed here and in the body.
  - `FUB-I1-COORDINATORWAIT`'s row now names the site its census missed.
- **At this round's head, the ten gates, the frozen proof and both body validators.** The body records them.

### 9.22 The B4 round: H3 tells an inspection error from a held lease, the census's paths, and follow-up C's fix P

**Who and why.** This round is `pr11_fub_impl10`'s (`claude-opus-5-5`, `max`), spawned by `orch_pr11` on `d7865780`.
- **What started it.** The i5 delta review of `ce55ca91..d7865780`, two `gpt-6-astra` lenses at `max`. The regression
  lens passed with no findings. The regular lens returned CHANGES_REQUIRED with I5-1 (P2, executed) and I5-2 (P3,
  reasoned) (`~/orch-pr11/reviews/review-329-i5-triage.md`).
- **The brief and the scope.** The brief is `~/orch-pr11/briefs/pr11_fub_impl10.md`: I5-1, I5-2, §9.21.6's open
  observation and the body's condensation. The orchestrator's answer `~/orch-pr11/answers/pr11_fub_impl10-0.md` routed
  follow-up C's fix P to this round and replaced its integration question with one push, and its addendum
  `pr11_fub_impl10-0b.md` set binding precisions for fix P. Figures are under `~/orch-pr11/logs/pr11_fub_impl10/`,
  cited as `impl10/…`.
- **What it adopts.** No owner decision. H1 is unchanged and proposed. H3 is revised, and stays proposed and
  conditional, not adopted.

**The commits, on `d7865780`:**
- `ff4249c3`: I5-1, H3 revised, with the fixture's probe and seam, three regression tests, the probe's test and the
  notes;
- `946159ff`: I5-2;
- `9ed6ab1c`: fix P, follow-up C's patch applied byte for byte, its two doc comments moved to the coordinator's notes;
- then this text, §9.4's re-pinned cites and its B4 row, the notes in §9.20.4 and §9.21.6, the header's and §0's
  sentences, and the two findings' dated sections.

#### 9.22.1 The review

- **The scope** was I4-1, step 5 (W1 and W2, with H3), CAS-1 and step 5's evidence precisions. CI at `d7865780` was
  green on every leg: "CI" run 37202595630, all ten jobs, and "Pull request policy" run 37202595593. Two runs created in
  the same second ended cancelled (`impl10/ci/runs-d7865780.json`, `run-37202595630-jobs.txt`).
- **The regression lens** (`~/orch-pr11/reviews/review-329-i5-regression-d7865780.review.md`) passed:
  - 215 selected tests passed;
  - reversing CAS-1 and H3 failed all eight of its selected guards;
  - the frozen differences were exactly H1 (+16/−4) and H3 (+133/−3);
  - CAS-1's `introduced_by_feature`/`58c7c203` classification checked.
  - Its whole-suite timings between the two CI runs (macOS 821.15 s to 1,259.56 s, the guest 549.94 s to 564.76 s) do
    not isolate this change's cost, and it asked for no action.
- **The regular lens** (`~/orch-pr11/reviews/review-329-i5-regular-d7865780.review.md`) returned CHANGES_REQUIRED:
  - **I5-1** (P2, executed; `fix_regression`, first bad `3ce7bb46`): H3 also suppresses observation errors. Its witnesses
    are in `~/orch-pr11/reviews/329-i5-witnesses/upstroke-review329-regular-pc70vkhx/` (`RESULTS.md`, `witness.patch`
    c2851553…, `pre-h3-control.patch` 052cc000…).
  - **I5-2** (P3, reasoned; `fix_regression`, first bad `03fdf500`): CAS-1's census builds filesystem paths as strings.
  - **What held:** I4-1's withdrawal; W1 and W2's constructed class and first-bad commits, as supported; the evidence
    wording; CAS-1's fix, with restoring the hook-less call failing six of six; the provenance and the sweep.
  - **The Windows retries:** a reading, not a finding (§9.22.5).

#### 9.22.2 I5-1: H3's waits read an inspection error as a held lease

**What was wrong.**
- `rundir::observe_cleanup_hold` answers through `cleanup::is_held`, which reads a lease file that will not open, or a
  `flock` that fails other than `EWOULDBLOCK`, as held (`src/rundir.rs:2480`). That is fail-closed, and right for a
  resume deciding whether to take the run.
- H3 made W1's helper and W2's first resume wait through #320's `wait_for_cleanup_hold_release_observing`, which
  observes through it. So a transient inspection error was waited on as a holder. Once it cleared, the next observation
  read free and H3 accepted the fixture, at both sites. Before H3, a single observation failed on the same error at
  once.
- So H3 tolerated more than the inherited lease copy it exists for, and could erase an observation-error failure the
  tests caught before it. Its comment, that an observation that fails still reads running at once, was false. The
  defect is H3's own: `fix_regression`, first bad `3ce7bb46`.

**The repair: H3, revised** (`ff4249c3`). No production file changes.
- **An observation told apart.** `workspace_manager::fixture::observe_cleanup_lease` makes the production probe's own
  steps: the lease file opened for reading and writing and never created, then an exclusive `flock` asked for without
  blocking and given straight back. It answers held, free, or the error that answered neither. It is test code, in
  the fixture module that may name those primitives; the topology's modules may not.
- **The wait.** `await_own_lease_copies_release` keeps #320's bound (`release_bound`), its 50 ms rest and its
  acknowledgement of each held reading (`Fixture::holder_observed`), over that observation.
  - A free observation ends it.
  - A held one is acknowledged and waited on, to the bound.
  - One that fails ends it at once, before any acknowledgement or rest, as `LeaseCopyWait::Unobservable`, which names
    the observation and its error.
- **W1's site.** `assert_no_process_holds_the_run` makes that wait and then reads `rundir::is_running` once, as before.
  A red keeps the witness's message and adds what the wait stopped on.
- **W2's site.** The creation body makes the wait before its first resume, and no longer counts its prefix in
  `resume_attempts`, so #320's own wait does not run there.
  - An observation that fails fails the body at once, naming its error.
  - A copy past the bound leaves the resume to production, whose refusal carries the expired wait's note
    (`refusal_after_an_expired_wait`), as through #320's trunk before.
  - Step 5 set aside a hand-written wait because it would lack that note and the holder hook (§9.20.4). This one keeps
    both.
- **One line of master's fixture.** `Fixture::holder_observed` first hands each held reading to
  `workspace_manager::fixture::a_wait_read_the_lease_held`, the seam the regression tests arm. It does nothing unless a
  test armed it on its own thread.
- **The comment** says what the code now does (`src/engine/topology/recover/tests.rs:7124`), and so do the notes.
- **What stays as it was.** #320's own wait, before every later resume, still observes through
  `observe_cleanup_hold`. It is master's and outside H3, and it is not changed. Whether the tests behind those later
  resumes need the same distinction is not examined here.

**The regression tests** (`src/engine/topology/recover/tests.rs`, Unix):
- **The reviewer's two witnesses, adopted:**
  `a_lease_observation_that_fails_still_fails_the_first_incarnations_death_at_once` (`:18078`) and
  `a_lease_observation_that_fails_still_fails_a_creation_prefixs_first_resume_at_once` (`:18128`).
  - **Their set-up.** Each first waits out this process's own copies with #320's wait and shows the lease free. It
    then sets `cleanup.lock` to mode 000 through `unreadable_until_read_held`, which fails with a diagnostic if the mode
    does not bind (root), and shows that production reads it held.
  - **What each asserts.** A failure, and that no wait read the unreadable lease as a holder. The seam puts the mode
    back at the first held reading, so a wait that reads the error as held passes on its next observation, as H3 did.
  - **Their messages** are what the pre-H3 single observation and the revised wait share: "`<tag>`: and no process
    holds the run" for W1, and "cleanup lease" for W2. So the pre-H3 control passes them too.
- **The creation body's expired wait:**
  `a_lease_copy_that_outlives_the_bound_still_refuses_a_creation_prefixs_first_resume` (`:18182`) keeps a parked copy of
  P8's lease past a 500 ms bound, and the resume's refusal names the expired wait. It holds the note this revision
  re-routes.
- **The probe:** `rundir::tests::the_lease_waits_observation_tells_a_failed_inspection_from_a_held_lease`
  (`src/rundir/tests.rs:6129`) reads five states with both observations: no lease file, a parked fork's copy, that copy
  released, the lease at mode 000, and the mode put back by a wait's acknowledgement. The two agree in every state but
  mode 000, where the probe answers `PermissionDenied` and production reads held.

**Red at `d7865780`, green after, with the pre-H3 controls** (`impl10/repro/SUMMARY.txt`):
- **At `d7865780` with the witnesses alone.** The tree is `repro/witnesses-only-at-d7865780.diff`: the fixture's seam,
  `holder_observed`'s line and the three tests, byte for byte from this head. Both adopted witnesses fail, "no
  observation read the unreadable lease as a holder", and the other four pass (`repro/red-at-d7865780/`).
- **The reviewer's pre-H3 control on that tree,** W1's wait replaced by `Ok(())` and W2's increment removed
  (`repro/preh3-control-at-d7865780/`).
  - Both adopted witnesses pass, catching the original failures: W1's assertion, the run lock answering
    `PermissionDenied`, and W2's refusal.
  - H3's three regression tests and the expired-wait test fail, as the pre-H3 behaviour must.
- **At this round's code** all six pass (`repro/green-at-head/`, at `946159ff`, and at `9ed6ab1c` the mutation
  campaign's control row). The same pre-H3 control applied there behaves as on `d7865780`
  (`repro/preh3-control-at-head/`).
- **What the revised waits print** (`repro/SUMMARY-raw.txt`):
  - W1: "the cleanup lease's wait Err(Unobservable { observation: 1, error: … PermissionDenied … })";
  - W2: "observation 1 of the run's cleanup.lock answered neither held nor free: Permission denied (os error 13)".

**What these can and cannot show.**
- The constructed error is an open's `EACCES`, on a lease file at mode 000. It needs an unprivileged user, as the
  suite's other mode tests do.
- A `flock` failure other than `EWOULDBLOCK` cannot be made to order. So that arm of the probe is reasoned, not
  executed, and its mutation row survives (§9.22.7).
- Nothing here touches production's observation, which stays fail-closed.

#### 9.22.3 H3 revised, and PROPOSED RULING B-W revised

**The revised hunk** is `src/engine/topology/recover/tests.rs` at this round's code
(`impl10/frozen/frozen-proof-9ed6ab1c.txt`):
- blob `407b27cb`, +353/−3 against master's `765d9b36` (`impl10/frozen/H3-revised-vs-master.diff`);
- +235/−15 against H3 as proposed, `74c0b121` (`impl10/frozen/H3-revised-vs-74c0b121.diff`);
- H3 as proposed was +133/−3 against master, as §9.20.4 records.

**What it contains, against master:**
- `assert_no_process_holds_the_run`, `LeaseCopyWait` and its `Display`, and `await_own_lease_copies_release`;
- the one line in `Fixture::holder_observed`;
- W2's wait, its note and the `before_the_resume` hook;
- H3's three regression tests, and this round's three (§9.22.2).

The fixture's probe and seam are in `src/workspace_manager/fixture.rs`, which is not frozen.

**PROPOSED RULING B-W, revised (on a G6-frozen test child), in RULING P-1's form. NOT ADOPTED.** It replaces §9.20.4's
text.

> The owner permits exactly one change to `src/engine/topology/recover/tests.rs`, a whole-file test child that G6
> requires byte-identical to the G5 range save E-G6-1's enumerated changers. **H3, as revised at the B4 round:**
> `the_runs_first_resume_by_an_incarnation_that_then_dies`'s last check becomes `assert_no_process_holds_the_run`;
> `a_resume_over_a_creation_that_stopped_after_its_marker_was_removed_converges` takes a `before_the_resume` hook and,
> before its first resume, makes the same wait; that wait, `await_own_lease_copies_release`, has #320's bound, rest and
> acknowledgement, waits only on a lease an observation finds held, and ends at once on an observation that fails;
> after it the helper reads `rundir::is_running` once, a red naming what the wait stopped on and the run lock's
> answer, and the creation body's resume notes an expired wait; `Fixture::holder_observed` hands each held reading to
> the fixture's seam; and the six regression tests of §9.20.4 and §9.22.2. Nothing else in the file changes: no other
> test's assertion, wait or bound, and #320's own wait stays master's. No production file and no other frozen file
> changes. G6's module diff proof reads this hunk, with H1, as the only differences permitted beyond E-G6-1's, bound to
> the merge SHA of the change that makes it.

**What follows.**
- **It is not adopted.** The owner's freeze ruling, owed for H3 before, now applies to the revised hunk.
  - A "no" reverts `recover/tests.rs` to master's. Both witnesses keep their single observation, and both findings
    stay filed with step 5's obligation unmet, as §9.20.4 says.
  - The fixture's probe and seam would then have no caller in that file.
- **What depends on it:** nothing outside the file, for compiling or for behaviour, beyond the fixture items it calls.
- **It awaits review,** as H3 did: whether it repairs the two witnesses faithfully, without weakening their obligation.

#### 9.22.4 I5-2: the census's paths

- **What was wrong.** `this_modules_sources` built `format!("workspace_manager/{child}.rs")`, used it as a path and kept
  it as the file's identity (`src/workspace_manager/tests.rs:5150` at `d7865780`). Standards §8 says paths are `Path` or
  `PathBuf`, never string concatenation, and never a string as identity. No Windows failure was shown.
- **The repair** (`946159ff`).
  - `child_module_path` builds `Path::new("workspace_manager").join(child).with_extension("rs")` (`:5125`).
  - The sources are `(PathBuf, String)` (`:5133`), and the census and its positive control compare path values.
  - The census also asserts that it reads the parent.
- **Still red where it must be.** Putting CAS-1's hook-less swap back fails the census, its control, the two manager
  witnesses and the two coordinator witnesses (`impl10/mutation/i52-census-hookless-swap/`).

#### 9.22.5 The two Windows retries (§9.21.6), updated

`remove_tree_once_handles_close` and `read_marker_once_handles_close` stay open observations, at the evidence level the
i5 review's regular lens gives them:
- it reads R-S's explicit allowance for existing synchronous filesystem work as governing these unchanged primitives;
- a held handle can still delay the coordinator's messages through their 39 × 25 ms sleeps;
- it did not execute Windows, and its reading is no evidence that those calls stay responsive;
- nothing is decided here, and no finding is filed. §9.21.6 carries the same note.

#### 9.22.6 Follow-up C's fix P (P-all)

**Where it came from.**
- **The failure.** Follow-up C's round 5 (`pr11_fuc_impl5`) met #330's `test (winguest)` failure of this change's
  witness `a_shutdown_answered_inside_a_dispatchs_intent_starts_no_attempt_and_spawns_nothing` at `7904ca71` (CI run
  37190091060): "the shutdown was injected while the tear stood".
- **C's question and evidence:** `~/orch-pr11/questions/pr11_fuc_impl5-1.md` and
  `~/orch-pr11/logs/pr11_fuc_impl5/CAUSE.md` (re-read after it changed, sha256 `965998dd…`;
  `impl10/fixp/INPUTS-SHA256.txt`). They route the fix to this change, whose witness and fixture it is.
- **The routing.** The orchestrator sent it here (`~/orch-pr11/answers/pr11_fub_impl10-0.md`, with the precisions of
  `pr11_fub_impl10-0b.md`).

**The cause, at its evidence level.**
- **The stand-in** delays every Git process the test starts (`~/orch-pr11/logs/pr11_fuc_impl5/standin/slowgit/git`).
- **What it shows.**
  1. The dispatch's intent access fails on the tear.
  2. Its first pause answers pipeline 0's admission, whose grant waits for the scaffold runner's `git rev-parse`.
  3. Past about 160 ms per Git process, that and the access's two `git worktree list` attempts outlast the suite's
     500 ms registry deadline.
  4. The access refuses before a second pause, so no shutdown is injected.
- **What it does not show.** These thresholds are stand-in observations, not a measured cause of the guest's failure,
  and the guest's Git latency in that run is unknown.
- **An alternative, not excluded.** C considered a quiescent point before the tear and could not exclude it. Fix P does
  not cover it, and its fifth part makes any recurrence name the path it took.

**What was applied** (`9ed6ab1c`).
- **The patch,** byte for byte: `~/orch-pr11/logs/pr11_fuc_impl5/fix/FIX-P-ALL-for-B-at-d7865780.patch`, sha256
  `6344e4f1…` checked (`impl10/fixp/applied-vs-d7865780.diff`).
- **Its numstat:** `git apply --numstat` gives `src/engine/topology/coordinator.rs` +75/−3 and
  `src/workspace_manager.rs` +89/−7 (`impl10/fixp/patch-numstat.txt`).
- **One adaptation.** Its two doc comments in `coordinator.rs`, 18 lines, moved to
  `docs/internals/engine/topology/coordinator.md`, as that module's notes convention requires: its source carries no
  other prose, here or at master. No code changed in the move.

**Its five parts, as C gave them:**
1. **A per-repository registry deadline seam.** In a production build, `registry_access_deadline` is a `const fn`
   returning `REGISTRY_ACCESS_DEADLINE`. Its `#[cfg(test)]` twin reads a table that a nesting `RegistryDeadline` guard
   fills. `tolerant_registry_access` and `registry_lock_refusal` use, and print, the deadline actually used.
2. **`TearHeld` holds `WITNESS_REGISTRY_DEADLINE`,** 10 s, for its repository, so every tear witness waits to the
   production length.
3. **`hold_next_contended` is `#[cfg(test)]` on every platform,** where it was Unix only.
4. **`a_shutdown_answered_inside_a_slow_dispatchs_intent_starts_no_attempt_and_spawns_nothing`** holds the access's first
   contended answer for 700 ms. That reproduces, in-process and deterministically, the mechanism the stand-in
   demonstrated. It does not establish what caused the guest's failure.
5. **`stopped_in_its_wait`'s panic** names what the command ended on when no shutdown was injected.

**Its production-visible effect, classified against master** (`impl10/fixp/production-effect.txt`):
- **None in behaviour.** `REGISTRY_ACCESS_DEADLINE` is 10 s in a production build at `d7865780` and here, and
  `registry_access_deadline` returns it, so every access waits to the same deadline.
- **The two refusal messages** now print the deadline actually used. In a production build that is the same 10 s, so
  their text is unchanged; only a test build can print another.
- **Against master** there is nothing to compare. Master has no registry deadline and no `RegistryRefused`; both are
  this change's tolerant access (`58c7c203`). Fix P adds no production behaviour of its own.
- **`hold_next_contended`** is test-only either way. It is now compiled into Windows test builds as well.

**Re-verified on this head** (`impl10/fixp/SUMMARY.txt`; every run names its tree):
- **The new witness:** 10 of 10 (`fixp/runs/head-new-witness-x10/`).
- **The seam undone,** C's m1a (`fixp/mutations/m1a-seam-undone-at-9ed6ab1c.diff`):
  - the new witness fails 5 of 5 with the CI's message, now "… it was not, and the command ended on the worktree
    registry … until its deadline (500ms): 1 attempt(s) …";
  - under the 170 ms stand-in, the three original dispatch shutdown witnesses fail too: 4 of the 54 tear witnesses
    (`fixp/runs/m1a-*`).
- **B's I2-1 undone,** C's m2a, the substitution of round 6's row `i21-pause-ignores-interrupt`:
  - round 6's 19 guards still fail with fix P applied, 19 of 19;
  - so do the two shutdown witnesses added since, CAS-1's and fix P's, and the other 37 pass
    (`fixp/m2a-r6-row-verdict.txt`, `fixp/runs/m2a-r6-row-plus2-0ms/`);
  - that is all "masks nothing" means here. It is not a proof that every test keeps its meaning.
- **The tear witnesses under the stand-in:** C's 52 and CAS-1's two coordinator witnesses, 54 of 54 at 0 ms, at 170 ms
  and at 250 ms (`fixp/runs/head-tear-{0,170,250}ms/`).

**Provenance.**
- **The witness and its harness are this change's.** `22d70ef6` (repair round 6) added the witness and
  `stopped_in_its_wait`, and `10cc88d8` (repair round 3) added `TearHeld`.
- **First bad `22d70ef6`, by the stand-in.** There the three dispatch shutdown witnesses pass at 0 ms and fail all three
  runs at 170 ms with the CI's message, and its parent `0edfc509` has none of the three (`fixp/runs/firstbad-22d70ef6-*`).
- **That is a construction, not the guest's history.** The witness passed the guest five times on this change's
  heads (C's `CAUSE.md`), and why it failed in C's run is not established.

**Its cost.** Seven repair witnesses wait out a full access deadline by design, so each now waits 10 s where it waited
500 ms (C's `fix/timing/fixP-c-per-test.txt`). The 54 tear witnesses took 11.44 s together here at 0 ms.

**Three Docker reds in C's validation runs** (`~/orch-pr11/logs/pr11_fuc_impl5/fix/checks/DOCKER-SIGHTINGS.md`; read
here, `impl10/fixp/docker-sightings-read.txt`). Each was an unfiltered whole suite in a scratch tree, not a gate of
either branch:
- **C `7904ca71` + P-all** (`final-all-c-all-targets.log`, sha256 `baa3d992…`): 3,183 passed and 1 failed,
  `runner::container::tests::real_docker_kill_on_an_already_exited_container_is_tolerated`, "`upstroke-f1-already-exited`
  is still running after 200 observations";
- **B `d7865780` + P-all** (`final-all-b2-all-targets.log`, `c4dac7a3…`): 3,148 passed and 1 failed, the same test and
  message;
- **the preview merge of C `7904ca71` with B `d7865780` + P-all,** merged tree `48321066…`
  (`merged-c2-all-targets.log`, `07a7d840…`): 3,192 passed and 1 failed,
  `runner::container::tests::real_docker_lists_the_state_the_settlement_observation_reads`, left `Running`, right
  `Exited`.

**Their disposition, per head.**
- **Each is a sighting of a filed finding, which is its guard.** The first two carry the fingerprint of
  `PR274-DOCKER-TERMINATION-POLL-COUNTS-YIELDS-NOT-TIME` (P3, deferred), and the third that of
  `PR262-DOCKER-OBSERVE-READS-RUNNING-AFTER-PROCESSGONE` (P2, deferred).
- **Nothing rejects them.** Each test passed 10 of 10 alone and each suite reran green, but green reruns and untouched
  paths reject none of them. The cause of each is not established, and none is classified pre-existing here.
- **What is owed.** Final-range G6 owes them their findings' fingerprint rule, and B's review carries them before any
  merge.

#### 9.22.7 The mutations

The campaign is `impl10/mutation/VERDICTS.txt` (runner `impl10/tools/mutate.py`, at `9ed6ab1c`).
- **How a row was run.** Each row is a `git archive` with its substitutions, built from its own tree, and runs 18 tests:
  the six of §9.22.2, W1, W2 and W2's P7 twin, #320's two later-resume tests, the probe's test, the census and its
  control, and CAS-1's four witnesses.
- **How it is judged.** A row passes when exactly its expected tests are red and all 18 ran.

| Row | Mutation | Red | Verdict |
|---|---|---|---|
| control | none | none | PASS |
| `i51-wait-error-as-held` | the revised wait reads an observation that fails as held: H3's first form, at the wait | the two adopted witnesses | PASS |
| `i51-probe-error-as-held` | the probe answers an open that fails as held: `is_held`'s own answer | those two, and the probe's test | PASS |
| `i51-probe-flock-error-as-held` | the probe answers a `flock` failure other than `EWOULDBLOCK` as held | none: no test constructs one | survives, as expected |
| `m1-w1-no-wait` | W1's site without the wait: the pre-H3 helper | H3's two W1 regression tests | PASS |
| `m2-w2-no-wait` | W2's site without the wait: the pre-H3 first resume | H3's W2 regression test and the expired-wait test | PASS |
| `i51-w2-past-bound-unannotated` | the creation body's refusal loses the expired wait's note | the expired-wait test | PASS |
| `i52-census-hookless-swap` | CAS-1's hook-less swap put back | the census, its control, and CAS-1's four witnesses | PASS |

Under the pre-H3 rows the adopted witnesses pass, since the pre-H3 single observation and first resume fail on the
error at once.

#### 9.22.8 The frozen proof, and what else ran

- **The frozen proof** (`impl10/frozen/frozen-proof-9ed6ab1c.txt`, `impl10/tools/frozen-proof-impl10.sh`):
  - this round's one frozen change is `recover/tests.rs`, H3 revised; 33 of the 34 files are byte-identical to
    `d7865780`'s;
  - `integrate.rs` is H1's blob `bf62256e`, +16/−4 against master, unchanged;
  - part 1 against master: those two files, +369/−7;
  - part 2 against `d724fb16`, E-G6-1 executed as a rule not adopted: FAIL, with the two proposed hunks unenumerated,
    as before.
- **The platforms** (`impl10/platform/code-9ed6ab1c/`): Windows clippy, the Windows 1.85 check with `-D warnings`,
  macOS clippy, and the Linux 1.85 check with `-D warnings`. All four passed.
- **The legacy path** (`impl10/legacy/legacy-activation-9ed6ab1c.txt`): no legacy module changed. `src/rundir.rs` is
  unchanged; only its test child gains the probe's test. Activation is `Inactive`, and `schema.rs` is unchanged.
- **At this round's head, the ten gates and both body validators.** The body records them.

**Not verified here:** the Windows and macOS legs, the guest, and CI's stable 1.99.0. CI is the truth for them. Fix P's
reading is a Linux stand-in's.

#### 9.22.9 The ledger and the findings

- **`FUB-I5-H3OBSERVATIONERROR`** (P2, `correctness`, `fixed`): provenance `fix_regression`, first bad `3ce7bb46`. Its
  guard is the two adopted witnesses and the mutation rows.
- **`FUB-I5-CENSUSPATHS`** (P3, `portability`, `fixed`): `fix_regression`, first bad `03fdf500`. Its guard is the census
  and its control, red on the hook-less swap.
- **`FUB-FIXP-WITNESSDEADLINE`** (P2, `portability`, `fixed`): `fix_regression`, first bad `22d70ef6` by the stand-in,
  its harness from `10cc88d8`. Its guard is the new witness, red with the seam undone, and the tear witnesses under the
  stand-in.
- **Round 8's two findings** stay filed, now guarded by H3 as revised. Each gains a dated section saying so, and its
  `reviewed_sha`, provenance and first bad are unchanged.
- **The three Docker sightings** are recorded against their filed findings, here and in the body. No finding file
  changes for them.
