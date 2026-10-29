---
id: R-C-REGISTRY
severity: P2
disposition: accepted-risk
category: correctness
pr:
reviewed_sha: 5c222ff2162da28aa471433773bf4966a883a428
location: src/workspace_manager.rs:2619
provenance: pre_existing
first_bad:
guard: for part 1, the adopted schema-4 start refusal R5 adds, `refuse_linked_registry` in `src/workspace_manager.rs`, run in `WorkspaceManager::derive` once the canonical common directory is known and before any registry access: implemented; its validation is the R5 pull request's final-head baseline, native CI and fresh review, recorded in that pull request's body, and until they pass no part-1 row is claimed closed; part 2 is accepted by name by the owner (adoption `818e7675…`, `adopted.ENV.choices[4]`, 2026-10-15)
---

## Narrowed to part 2

**What this record covers now.** On 2026-10-15 the owner adopted the route for R-C-REGISTRY
(`/home/ubuntu/babysit-pr11/evidence/g6-three-package-owner-adoption-20261015.json`, sha256
`818e76756e2b87f2a4676cae886e09e71f39b6f34ddb0a8a0234bc087dcc29f1`, `adopted.ENV.choices[4]`: "R-C-REGISTRY
part1 schema4 refusal; part2 exact acceptance"), from the ENV decision report
`/home/ubuntu/orch-pr11/logs/pr11_decide_env_g6/DECISION-REPORT.md`, sha256
`68042083b13acd3fc7c4500b6ac9c00f1beb3262b09a2ab2f204a2004ff2bf95`, §2.3c. Part 1 is refused; this record now holds
part 2 only.

**Part 1 is not accepted; the adopted refusal is its route.** R5 implements that refusal (report `:608-611`):

> A schema-4 (topology) run, and its resume, refuses before it makes or reuses any slot when the run's common directory
> holds a `worktrees` entry whose canonical path's parent is not the canonical common directory, and the coordinator
> inherits no `GIT_COMMON_DIR`. That is the condition under which the `../..` record Git writes for a new work tree names
> another directory. The refusal names the entry and its target. It starts no process. Legacy runs are unchanged.

The check is `refuse_linked_registry` in `src/workspace_manager.rs`. `WorkspaceManager::derive`, the manager's only
constructor, runs it as soon as its one read-only `git rev-parse --git-common-dir` has found the canonical common
directory, and before any registry access, so that a fresh run and a resume refuse before they make or reuse any slot;
the check itself starts no process. It is written to refuse a link into another repository, a chain of links, a link
inside the common directory whose target's parent is elsewhere, and on Windows a junction. **It is implemented; its
validation is the R5 pull request's:** the baseline at its final head, its native CI on Linux, macOS and Windows, and a
fresh review, each recorded in that pull request's body. Until they pass, this record claims no part-1 row closed. Its
witnesses, "symlinked registry, nothing inherited" among them, are the R5 pull request's.

**Implementation facts and limits.**
- **An empty inherited `GIT_COMMON_DIR` counts as inherited**, so it is part 2's layout and not refused: Git takes a set
  variable as set (`get_common_dir` in `setup.c`, at v2.41.0 and v2.55.0).
- **A `worktrees` entry that cannot be resolved**, a link whose target is gone among them, makes `derive` fail closed with
  an I/O error before any slot, not with this refusal: its canonical parent cannot be known.
- **What the refusal decides, and what it leaves.** Part 2's acceptance is decided in `derive` from whether a
  `GIT_COMMON_DIR` is inherited, and nothing else in R5 reads it, so C4's and C5's layouts pass the start. Whether
  such a run then completes is decided later by the integration correction (F): by its registration guard, from the
  Git context the runner reports for each child it starts, and by its removals, which act below the store's target as
  pinned at derivation. A runner that reports none, the container runner and the test runners among them, keeps the
  original registration rule, under which, read from source, these layouts' guarded verdicts are refused after the
  start. That is a question staged for the integrated fresh reviews; R5 changes no runner's behaviour.
- **The refusal comes before F's pin.** `derive` runs the refusal first and F's `RegistrationStore::of` after it, so a
  refused derivation pins nothing, and the pin relaxes no part of the refusal. A link made after derivation is not
  pinned: T-ENV-MGR's nothing-inherited sequence plants its link after its `derive`, so it witnesses F's guarded-read
  refusal only, and no removal through that link is claimed.
- **F's store-pin rows derive in children.** Six of F's seven T-PIN rows derive a manager over a store linked out of the
  common directory, or over a store link whose target is gone. While nothing is inherited, `derive` rejects the first
  with this refusal and fails closed on the second with an I/O error, as above. With R5 each of them makes its
  registrations, and any other repository, in its own process, inheriting nothing, and then runs that derivation, its
  removals and every one of its assertions in a portable `LinkedChild` that inherits the run's canonical common
  directory as an absolute `GIT_COMMON_DIR`, on every platform it ran on before, Windows junctions included. There the
  check admits part 2's configuration, and F's pin behaves as at F: it pins each live link, and not one whose target is
  gone. F's own runs of these rows, with nothing inherited, stay its evidence at F and ENV, and that a removal reads no
  `GIT_COMMON_DIR` remains a claim of its source.
- **The completion coverage is reused.** C4 and C5 under topology are F's genuine host-runner rows in
  `src/engine/topology/coordinator.rs`:
  `a_hosted_run_under_a_linked_registry_and_an_inherited_absolute_common_dir_completes` and
  `a_hosted_run_under_a_linked_registry_and_an_inherited_relative_common_dir_at_equal_depth_completes`, the second
  over a base as deep below the scratch root as the work trees, and over a host runner that holds the run's
  repository. Each resumes a run of two tasks at width two through `derive`, under the inherited value and past both
  of R5's start checks, and completes. Alpha's worker runs twice, its first attempt assessed as a failure, and beta's
  once; each working attempt's gate and review run in its own snapshots; the second integration is stale, and its
  verification's gate and review run too; finalization removes the execution root. Each parent then finds the store
  still the link to the same target, that target emptied, no run refs, one branch holding both tasks' work, and the
  other repository's refs untouched. At the manager they are T-ENV-MGR's C4 and C5 sequences. R5's own controls,
  which drove a test runner that reports nothing, were withdrawn for them. None of these rows has run at R5's
  integrated head, and part 2's completion is claimed only once they pass at the final one.
- **Platform limits.** Those rows run on Windows too, through ENV's Windows arms (`f_hosted_shim`, `f_hosted_isolated`)
  and over a junction. There they, like W4 and W6, which run the start refusal over a junction, execute through native
  CI only, and no C4 or C5 result there is inferred from the Unix rows or from W6.
- **Git versions.** The condition rests on Git resolving the `../..` record through the administrative directory's real
  path, which the proposal's harness executed on Git 2.43.0, 2.50.1 and 2.55.0 (below); R5 did not re-read it in Git's
  source. Junctions are executed only by native Windows CI.

**Part 2, accepted by name by the owner** (`adopted.ENV.choices[4]`, 2026-10-15; report `:615-620`), verbatim:

> **R-C-REGISTRY, part 2, accepted by name.** Where the run's `worktrees` registry is a symbolic link or junction into
> another repository and the coordinator inherits a `GIT_COMMON_DIR` naming the run's common directory (C4's and C5's
> layouts, which work at `5c222ff2` and under option (C)), Git's ref backends take a work tree's shared refs from that
> work tree's own record and never from the variable, so a branch or tag a role writes or reads in a work tree the engine
> made is the linked repository's. No engine sequence writes a shared ref there. The legacy gated run of part 1 fails
> closed at its snapshot add and, on master, keeps and pins the worker's output.

**Its last sentence is an inference on master.** That the legacy gated run keeps and pins the worker's output rather than
discarding it is read from the coordinator's code; no executed legacy test shows it under this layout (report `:632`).

**For G6.** Part 2 is satisfied by this exact documented disposition, which the gate report must state as a narrowing
(report `:662`).

**Grade.** P2, as filed. The narrowing lowers nothing (report `:663`).

The sections below are the record as filed on 2026-10-05; where the adoption has since answered them, each says so.

## Failure sequence

R-C-REGISTRY is the name the reviewed option (C) proposal gives this existing defect:
`/home/ubuntu/orch-pr11/owner-package/O3R-OPTION-C-PROPOSAL.md`, sha256
`dc94ba82515d265fd98b4aa3c8f1f8b4c699a70733bd19fe245c6fbec023eedc`, §8's table and §8.3 (`:3812`,
`:3991-4101`), called "the proposal" below. It is existing behaviour of `5c222ff2` that option (C), which is not
adopted, neither causes nor reaches.

**What it is.**
- **Registration.** A work tree the engine made, a gate or review snapshot or a slot, is registered in the `worktrees`
  directory of the run's common directory. The manager's slot add is `WORKTREE_ADD_ARGV` (`src/workspace_manager.rs:2619`);
  the legacy snapshot add is at `src/workspace.rs:871-906`.
- **The work tree's record.** Its `commondir` record, `../..`, is resolved through the real path of its administrative
  directory. Where the run's `worktrees` is a symbolic link into another repository's, that record names the other
  repository's common directory.
- **What Git reads from where.** An explicit `GIT_COMMON_DIR` comes before the record for objects and configuration.
  Git's ref backends never take it: shared refs come from the record (proposal §3.2, P5 and P7).

1. **The precondition.** The run's `worktrees` directory is a symbolic link into another repository, made by hand.
   `git worktree add` creates it as a directory, and no document of upstroke names such a link. The engine accepts it:
   the legacy constructor's checks pass, and each linked-registry shape's worker-only run works at `5c222ff2`
   (proposal §8.3, `:4029-4041`).
2. **Part 1: nothing inherited** ("symlinked registry, nothing inherited").
   - The gated run is refused at the snapshot add: Git's own checkout child reads the record through the link and
     cannot find the snapshot's commit. The coordinator's discard follows the error
     (`src/engine/coordinator.rs:544-548`, reasoned). Its reset and clean erase the worker's uncommitted output in
     the checkout; the discard's own steps are executed in the proposal's §4.10 sequences.
   - The slot run's worker and capture write their objects into the linked repository, and the commit at the base
     fails.
3. **Part 2: a `GIT_COMMON_DIR` inherited** (C4's and C5's layouts).
   - Objects, configuration and the common directory Git reports are the run's.
   - Branches, tags and every other shared ref are the linked repository's: a role that writes one in its work tree
     writes it there, and a gate or reviewer that reads one reads the linked repository's.
   - No executed sequence writes one: across the 138 rows of the six linked-registry shapes, the other repository's
     refs change in none (`E4/analysis/other-refs-census.txt`).

**Measured and reasoned.**
- **Executed** with ordinary Git by the proposal's standalone harness, on Git 2.43.0, 2.50.1 and 2.55.0, in temporary
  directories with the lenses' constructions, the regular lens's production layout included:
  - part 1's gated and slot rows (proposal §4.2, `:2126`; 2 rows on each version, §4.9);
  - Git's rule for shared refs, by P7 with the files backend.

  Records: `/home/ubuntu/orch-pr11/logs/pr11_envr_c4/`, `E4/analysis/matrix.tsv` and `E4/analysis/other-refs-census.txt`.
- **Reasoned:** the coordinator's discard after the refusal.
- **Not executed** (proposal `:4096-4101`):
  - a link inside the common directory;
  - a bind mount instead of a link;
  - Windows junctions;
  - the reftable backend;
  - other Git implementations;
  - the agent CLIs.
- **Where option (C) stands.** The relative-value part that an earlier round counted here has left this finding. Under
  the conditional option (C), an inherited relative `GIT_COMMON_DIR` reaches each process of a work tree the engine
  made as the absolute path it names at the started checkout. On 2.43.0 and 2.50.1, that shape's remainder is
  R-C-RELSTORE's (proposal `:4004-4012`).

## Severity: P2 (the proposal's proposed grade, for both parts, not adopted)

**The proposal proposes P2** (`provenance: pre_existing`; `first_bad` not traced; §8.3, `:4043-4053`). That proposal is
not adopted, and the owner classifies (`MAINTAINING.md:189-190`).
- **Why not P3.** Part 1 loses the worker's uncommitted output in a gated run, and writes another repository's object
  store in a slot run. Part 2 lets a role's shared-ref write land in another repository. Each is a data or integrity
  effect on a user repository.
- **Why below P1.** Both parts need a `worktrees` directory made, by hand, a symbolic link into another repository.
  Part 2 also needs a role that writes a shared ref in its work tree.
- **Whether those preconditions are speculative,** and so the grade, is the owner's call.

## Applicability to G6

**Applicable, conditionally on its precondition**, under the host runner, for the integrated tree (proposal
`:4055-4068`).
- **Q2 and INV-22:**
  - objects are written outside the run's store, outside every inventory row (part 1's slot run, executed);
  - a shared ref a role writes lands outside the run's repository (part 2, executed as Git's rule by P7).
- **Q6:**
  - a slot worker's view of objects and refs is another repository's (part 1, executed);
  - a gate's or reviewer's view of shared refs is another repository's (part 2, P7).
- **Not Q4,** reasoned.
- **Its relation to the O3-R finding** (`PR329-HOST-ROLES-INHERIT-THE-COORDINATORS-GIT-REPOSITORY-CONTEXT`). Neither part
  involves O3-R's precondition. Part 2 involves an inherited `GIT_COMMON_DIR`, but the link causes it, not the name.
  The proposal counts this finding closed under no reading.

**What clears it.** Neither filing it, nor its `deferred` disposition (which meant pending review and the owner's
decision, not deferred past G6), nor its proposed grade. Since the adoption, part 1 is to be satisfied by implementation,
the start refusal, which is implemented and whose validation is the R5 pull request's, and part 2 by an exact
documented disposition, this record's `accepted-risk` (report §3.2, `:733-734`).

## Step 5 and the witness

**`MAINTAINING.md:41-45`.** This finding carries executed reproductions: part 1's harness rows, and Git's rule for part 2
by P7. Under step 5's rule, a finding carrying a reproduction witness is fixed whatever its severity. It may be rejected
only by a row showing the witness does not reproduce on the head. Which of two things it is, triage and the owner
decide:
- relevant to a change that implements option (C);
- a pre-existing finding, logged under `:38-40`.

**Status under that rule.** Part 1's executed rows are the ones R5's refusal is written to close; until its tests,
native CI and a fresh review pass, this record claims no row fixed. Part 2, which the owner accepted, is accepted, not
fixed.

**The witness obligation.** A change that takes this up must show, failing at its base and passing after it, that a
gated run and a slot run under a linked registry keep their objects and shared refs in the run's repository, or are
refused before any work. "symlinked registry, nothing inherited" is the witness, with C4's and C5's layouts for part 2.

## The owner's choices (none is made here)

**Made on 2026-10-15**, after filing: the third route, narrowed to refuse part 1 only, with part 2 accepted by name
(above).

From the proposal's §8.3 (`:4073-4094`). None is drafted, and each comes with a cost grounded in Git's source:
- **Give every process in a work tree the engine made the run's common directory, inherited or not.**
  - It closes part 1's slot rows for objects.
  - It does not close part 2, or part 1's refs.
  - It closes part 1's gated rows only if the `git worktree` commands are given the value too.
  - Its costs: every role's composed environment changes in every run, and it reaches a role's Git in a scratch
    repository as an inherited value does.
- **Rewrite the work tree's record to the run's common directory, as an absolute path, after `git worktree add`.**
  - It closes both parts in that work tree's later processes.
  - Its costs: a new mechanism that writes Git's administrative files, and in this layout the write itself lands in
    the other repository. It does not close part 1's gated rows.
- **Refuse a linked registry at the start.**
  - It closes both parts by refusing them.
  - Its cost: it refuses C4's and C5's shapes, which work at `5c222ff2`.
- **A specific owner exception.**

## What the change that takes this up should do

**No remedy is chosen here.** The change follows from the owner's choice, and the witness obligation above binds it.
R5 is that change: the refusal of part 1, with the witness obligation above as its tests, and C4's and C5's layouts
still working, as part 2's acceptance requires.

## Provenance

**`pre_existing`.** It holds at `5c222ff2` and under (C) alike, row for row (proposal §4.9). `first_bad` was not traced.

## Filed

- **When and by whom:** 2026-10-05, by `pr11_envc_impl1` (`claude-opus-5-5`).
- **On whose direction:** babysit_pr11's direction
  `/home/ubuntu/babysit-pr11/evidence/envc-package2-filing-disclosure-direction-20261005.md` (sha256
  `133149a64b24b625fe6457794d7ea7629c9ab2a40dfb0d75642bf475b8b34aef`), carried by orch_pr11b's answer
  `/home/ubuntu/orch-pr11/answers/pr11_envc_impl1-2.md`.
- **As what:** a record of the conditional draft implementation of option (C) on this branch. That implementation is
  unpushed and not adopted.
- **What filing does not do.** It adopts nothing: not option (C), not this residual's label, not its proposed grade. It
  is not clearance for merge or G6.

## Narrowed

- **When and by whom:** 2026-10-27, by `env-r5-gate-6` (`claude-opus-5-5`), in the R5 change, the `bulk-fix-P2` pull
  request for the two adopted schema-4 start refusals.
- **On whose direction:** root's commission
  `/home/ubuntu/babysit-pr11/evidence/g6-active-goal-20261025/R5-IMPLEMENTATION-COMMISSION-AUTHORITY.json` (sha256
  `2c18fdb7662b87120236d648657d1875ae2bbb1d9c37d10a788f424af6a5d3c8`), under the owner's adoption above.
- **As what:** the record of the owner's acceptance of part 2, with the refusal of part 1 implemented beside it.
- **What narrowing does not do.** It lowers no grade, refuses no part-2 layout, and is no clearance for G6 by itself:
  the refusal's executed evidence and the PR's review are what G6 cites, with this acceptance.
