---
id: FUB-D9-ENV
severity: P1
disposition: deferred
category: correctness
pr: 329
reviewed_sha: 8df424364809c99fe1b8098ac71918815133d8e7
location: src/workspace_manager.rs:4994
provenance: pre_existing
first_bad:
guard: the change that removes the coordinator's inherited Git repository context from the manager's two Git builders, the owner's decision O3 (ENV-1 for the manager's builders, not adopted), before G6, which cannot pass while this P1 is open and does not defer it past G6
---

## Failure sequence

Found by PR #329's design review round 9, in its gitenv lens (`gpt-6-astra` at `max`, on `8df42436`): finding 1 of
`/home/ubuntu/orch-pr11/reviews/review-329-d9-gitenv-8df42436.review.md`, triaged as FUB-D9-ENV, P1, in
`/home/ubuntu/orch-pr11/reviews/review-329-d9-triage.md:58`. Each step below carries the label its source gives it.

1. **The precondition, which an explicit export supplies.** The coordinator's environment holds an absolute
   `GIT_INDEX_FILE` naming the main checkout's ordinary index, `<main>/.git/index`: a wrapper script that sets it, or an
   alternate-index workflow. This is the precondition of the destructive sequence below, and it is the only one.
   - **The ordinary invocations do not supply it.** Until repair round 3 this step said a commit hook or a `!` alias in
     a linked checkout, or the `pre-commit` hook of `git commit -a` in a main checkout, supplied it. PR #329's
     implementation review (regression lens, on `54a1ff14`) executed each on Git 2.43.0 in disposable repositories
     (`/home/ubuntu/orch-pr11/reviews/329-i1-witnesses/review329-regression-schedule-2y5awc8_/WITNESSES.txt`; executed
     again at repair round 3 with the same three results, `/home/ubuntu/orch-pr11/logs/pr11_fub_impl3/r5/env-shapes.txt`):
     - a linked checkout's `pre-commit` hook receives `GIT_INDEX_FILE=<main>/.git/worktrees/linked/index`, that
       checkout's own index, with `GIT_DIR` its administrative directory;
     - a linked checkout's `!` alias receives `GIT_DIR` and leaves `GIT_INDEX_FILE` unset;
     - a main checkout's `pre-commit` hook under `git commit -a` receives `GIT_INDEX_FILE=<main>/.git/index.lock`.
   - None names the main checkout's ordinary index. They are the shapes
     `PR329-HOST-ROLES-INHERIT-THE-COORDINATORS-GIT-REPOSITORY-CONTEXT`'s file executes under "The precondition", for
     the host runner's roles: a repository or an index other than a role's own. What the manager's builders do under
     them was not executed for this file, and steps 3 and 4 do not rest on them.
2. The manager builds every Git child in two places, and neither removes any inherited Git variable:
   - `WorkspaceManager::command` (`src/workspace_manager.rs:4994-5009` at `8df42436`), through which `git`,
     `git_with_identity`, `git_ok`, `git_line` and `update_ref` run. It passes `-C <dir>`, `core.hooksPath`,
     `core.fsmonitor=false` and `protocol.file.allow=never`, and binds only `GIT_NO_REPLACE_OBJECTS=1`.
     `git_with_identity` adds a fixed author and committer.
   - `read_only_git` (`:5452-5464`). It passes `-C <dir>`, `--no-optional-locks` and `core.fsmonitor=false`, and binds
     only `GIT_NO_REPLACE_OBJECTS=1`.
   PR #329 leaves both builders as they were. At its repair round 2 they are at `:5413-5428` and `:5906-5918`.
3. After the engine's preflight, the user stages changes in that index. The manager adds a detached slot at the base
   commit. Git's add starts its checkout child with `GIT_DIR` and `GIT_WORK_TREE` overridden and the alternate index
   kept, and that child runs `reset --hard` through the inherited index.
4. **The user's staged state is replaced.** The add succeeds and returns `Ok`, so PR #329's add veto never runs. Inspecting
   or removing the destination cannot undo the index write.

**What is executed and what is reasoned.**
- **Executed** by the lens: read-only index-resolution queries with the manager's relevant switches, on Git **2.43.0,
  2.50.1 and 2.55.0**, all returned the inherited alternate index. The lens's text records the results; no transcript of
  the queries is saved, and `/home/ubuntu/orch-pr11/reviews/329-d9-witnesses/` holds only the design and concurrency
  lenses' results. The review text and its log are hashed in `/home/ubuntu/orch-pr11/reviews/SHA256SUMS-329-d9`.
- **Reasoned:** the destructive sequence, steps 3 and 4. It was not executed.
- **Why #329's design evidence did not show it.** Its witness scripts start Git with every `GIT_*` variable removed:
  `/home/ubuntu/orch-pr11/logs/pr11_fub_design8/witness/d8reviewers.py:47` and
  `/home/ubuntu/orch-pr11/logs/pr11_fub_design9/witness/d9common.py:29`.
- **The record sentence this falsifies.** "Its checkout writes the new worktree's index under that worktree's own entry",
  in `reviews/2026-10-01-pr11-follow-up-b-record.md` §8 (`:5087` at `8df42436`), is false under the builders' inherited
  environment.
- **The class has been executed elsewhere, not through this add.** The PR11 decision appendix's combined review (CR-1)
  executed it with #330's and #331's switch sets. With #330's, `read-tree --reset -u HEAD` in a linked worktree replaced
  the main checkout's staged contents. With #331's, `add -A` in a linked worktree staged into that same main index
  (`/home/ubuntu/orch-pr11/owner-package/DECISION-APPENDIX.md` §4.1, `:385-406`).

## The other consequences

- **The other inherited names (reasoned: the lens inspected both builders, and nothing was executed for these names).**
  - Repository discovery and registry reads inherit `GIT_DIR` and `GIT_COMMON_DIR`.
  - Other operations inherit `GIT_WORK_TREE`, the object-directory overrides, `GIT_NAMESPACE`, `GIT_CONFIG_*`, and the
    execution-path and graph overrides.
  - PR #329's tolerant registry access attempts again after a failure. Attempting again does not establish that these
    commands address the intended repository.
  - The lens notes that `GIT_INDEX_FILE` alone does not redirect `worktree list`.
- **The protocol restriction (reasoned: the lens checked Git's transport source; no separate damage was shown).**
  `WorkspaceManager::command` passes `protocol.file.allow=never`, and an inherited `GIT_ALLOW_PROTOCOL=file` overrides
  it. `read_only_git` sets no protocol policy.

## Severity: P1

P1 is the gitenv lens's grade and the triage's, and this file keeps it. The class is graded P1 three times: as
`FUC-D5-GITINDEXFILE` (#330), as `FUD-D4-ENV` (#331), and for the host runner's roles as
`PR329-HOST-ROLES-INHERIT-THE-COORDINATORS-GIT-REPOSITORY-CONTEXT`, whose "Parity" paragraph rests on this finding.
Through the manager's add, the consequence is the replacement of a user's staged state, in the user's own repository.
The owner classifies (`MAINTAINING.md`).

## Applicability to G6

**Applicable,** by the gitenv lens's table:
- **Q6, and ownership and accounting:** the inherited alternate index. The lens's "Blocks G6" column reads yes.
- **Q6 and R17, and INV-22 where a command is redirected:** the repository, worktree, object, namespace and
  configuration overrides. Blocks G6: yes, with the index.
- **Q6's environment boundary:** the protocol caveat, with the environment's disposition.

**Exposure today.** The topology's manager runs only under schema 4, which production cannot reach at this filing:
`TOPOLOGY_ACTIVATION` is `Inactive` (`src/topology/schema.rs:27`). PR12, which enables the topology conductor, may not
merge until G6 passes (`/home/ubuntu/orch-pr11/g6/PLAN.md:4`).

**What clears it.** An open P1 that is applicable fails G6. Only a fix implemented and validated in G6's range clears
it, or a specific, applicable owner exception (`PLAN.md:160-168`). It is not deferred past G6. Filing it is not
clearance.

## Provenance

`pre_existing`. Both builders bind only `GIT_NO_REPLACE_OBJECTS=1` (and `git_with_identity` its identity) at master
`5c222ff2`, at `8df42436` and at PR #329's repair round 2. `first_bad` is left empty, because the builders' history was
not bisected.

## What the change that takes this up should do

**No remedy is chosen here.** The remedy route is the owner's decision **O3**: ENV-1 for the manager's builders, in the
PR11 decision appendix's §4.2 and §4.3. It is **not adopted**. This file records no choice, no waiver and no change of
grade.

What the appendix asks of the implementation is its §4.6:
- each builder's `Command` reports every name of the removed sets as removed;
- a behaviour test, red first at master: under an inherited `GIT_INDEX_FILE`, `GIT_DIR` and `GIT_COMMON_DIR`, a manager
  add and capture address the slot's own index, and the other index is byte-identical afterwards;
- a mutation that drops the removal from either builder and turns those tests red.

The appendix's row for `GIT_ALLOW_PROTOCOL` (`DECISION-APPENDIX.md:494`) closes this file's protocol route by #330's
empty binding, set after ENV-1's removals. Until both land, that route stays open.

## Filed

At PR #329's repair round 2, on the PR11 orchestrator's direction
(`/home/ubuntu/orch-pr11/answers/pr11_fub_impl2-2.md`). The orchestrator carried the supervisor's correction: every
review finding is filed. The implementation round had recorded FUB-D9-ENV only as a deferred ledger row, and this file
corrects that. Filing it decides nothing about its repair route, which is the owner's.

**Corrected at repair round 3** (`/home/ubuntu/orch-pr11/briefs/pr11_fub_impl3.md`, item R5): step 1's precondition,
from the implementation review's executed facts (its regression lens's finding 3,
`/home/ubuntu/orch-pr11/reviews/review-329-i1-regression-54a1ff14.review.md`). The P1, its grade, its guard and its
discovery pin `reviewed_sha` are unchanged.

## Extended at PR11 follow-up C's implementation (2026-10-03): FUC-D5-GITINDEXFILE

**The same class, found against #330's U, filed here rather than duplicated.** #330's design review round 5 found it as
**FUC-D5-GITINDEXFILE, P1**: the design lens (reasoned, on the executed late add) and the added winiso lens (the path
override executed on Git 2.43 and 2.55 with #330's round-5 switches; the damage reasoned), triaged in
`/home/ubuntu/orch-pr11/reviews/review-330-d5-triage.md:22` and `:62`.

**Its consequence for #330: it defeats U's instance isolation.** U gives each coordinator incarnation its own slot
instances, so a dead incarnation's Git writers act only on their own instance's paths and registration
(`reviews/2026-10-02-pr11-follow-up-c-record.md`, §4 and §5). An inherited `GIT_INDEX_FILE` names one index for every
engine Git child, whatever instance it runs in: Git honours it over the worktree's own index, and neither builder clears
or binds it (`WorkspaceManager::command` and `read_only_command` in `src/workspace_manager.rs` at #330's implementation
bind the switch set and `GIT_NO_REPLACE_OBJECTS` only). So a coordinator launched with it set, a dead incarnation's late
add whose `reset --hard` writes that shared index, and the successor's `candidate_write_tree` then captures the base tree
instead of the worker's edits. Its other face, found by the same lens: Q's drain would not stop live commands writing an
inherited alternate index either, so Q's untouched-user-checkout guarantee fails the same way. The audit list the lens
gives for both builders, before repository discovery: `GIT_INDEX_FILE`, `GIT_DIR`, `GIT_WORK_TREE`, `GIT_COMMON_DIR`,
`GIT_OBJECT_DIRECTORY`, `GIT_ALTERNATE_OBJECT_DIRECTORIES`, `GIT_NAMESPACE`, `GIT_CONFIG_*`, `GIT_SHALLOW_FILE`,
`GIT_GRAFT_FILE`. #330's common-git-dir census ran under a clean environment (`census_commondir.py:72` in
`~/orch-pr11/logs/pr11_fuc_design5/census/`), so it could not see this; the appendix's §4.6 asks for the census to be
repeated under inherited overrides with the remedy.

**What #330's implementation changes here, and what it does not.**
- It does not change any Git child's inherited environment: that is O3's (ENV-1) or O3-R's, and the brief puts it out
  of #330's scope. FUC-D5-GITINDEXFILE stays open, P1, with this file, and blocks G6 with it.
- **This file's protocol route is closed by #330's binding.** Both manager builders now bind `GIT_ALLOW_PROTOCOL` to the
  empty value, after the inherited environment (a `Command` binding replaces an inherited value of the same name), so an
  inherited `GIT_ALLOW_PROTOCOL=file` no longer overrides `protocol.file.allow=never`; the binding is pinned by
  `both_builders_carry_the_engine_switch_set_and_its_bindings`. That is the appendix's row for it (§4.2, the kept
  variables' table), reached without ENV-1's removals because the binding alone overrides the value. Every other route of
  this file stays open.

**Owner:** the PR11 owner's decision **O3** (ENV-1 in the manager's two builders), or **O3-R** if the owner folds the
host runner's role environment into it. #330's ledger maps FUC-D5-GITINDEXFILE's row to this file.

**The route, selected 2026-10-11 (PR #330's repair round 9).** O3-R (ii)'s form: "its own change before G6, carried by
the option (C) draft, the only implemented candidate", and "The adoptions that draft names stay the owner's, through its
own question"; the option is "(ii) File it and fix it by its own change before G6." That selection is an agent decision
by `pr11_decide_c` under the owner's delegation of 2026-10-11 (babysit's 0194a1e4…), in its decision report
(`~/orch-pr11/logs/pr11_decide_c/DECISION-REPORT.md`, sha256 `19b080be…`, section 3, C-4), and not the owner's own
selection. So this file, and with it FUC-D5-GITINDEXFILE, is remedied by its own change before G6, not by #330:
- it stays open, P1 and `deferred`, and G6 stays blocked through Q4, Q6 and DESC until that change lands;
- the selection adopts none of O3's options 1 to 3, O3-R (i) or (iii), option (C)'s own adoptions, or ENV-C's packaging
  question, and approves no remedy's code;
- if the option (C) draft does not land, the route stands and needs another candidate change.

## Implementation status (2026-10-26): option (C), adopted, and where each remaining face is carried

**What this section is.** It is appended under P-R2 of the owner's packaging exception (the ENV decision report
`68042083…` §2.1, adopted by the owner on 2026-10-15, the owner's adoption record `818e7675…`). Everything above it is
master's canonical file, C's extension and route sections included, unchanged; nothing is imported over it. It records
option (C) (`owner-package/O3R-OPTION-C-PROPOSAL.md`, sha256
`dc94ba82515d265fd98b4aa3c8f1f8b4c699a70733bd19fe245c6fbec023eedc`, round 4, rules 1 to 3, with A1 to A4), which the
owner adopted on 2026-10-15, on the branch `fix-P1/correctness_host-roles-inherit-the-coordinators-git-repository-context`.

**What is implemented, in both of the manager's builders,** `WorkspaceManager::command` and `read_only_command`, the one
#330 added (A1):
- **In a slot,** a Git child goes without the inherited `GIT_DIR`, `GIT_WORK_TREE`, `GIT_IMPLICIT_WORK_TREE` and
  `GIT_INDEX_FILE`, and an inherited `GIT_COMMON_DIR` that is not absolute reaches it as the absolute path it names at
  the managed base. That is given before #330's switch set and bindings.
- **From the base,** the `git worktree` commands, `worktree add` and `worktree list`, go without `GIT_INDEX_FILE`.
- **At the base,** a child keeps the inherited context (rule 1).

**The witnesses.** This file's alternate-index slot-add sequence is a row of
`the_managers_slot_add_and_reads_shed_the_work_tree_context_and_its_base_keeps_it`, with
`a_relative_common_dir_resolves_to_the_runs_common_dir_in_a_slot`. Each of the mutations MC8, MC9, MC11 and MC12
turns one of them red. They were red at `5c222ff2` and green at the candidate head `c3efe4c2` on 2026-10-05; those runs
are historical. Red at master and green at this pull request's exact head is its own validation.

**FUC-D5-GITINDEXFILE and CR-1, filed with this finding.** FUC-D5-GITINDEXFILE's sequence, a dead incarnation's late
add resetting a shared inherited index before the successor's `write-tree`, is the same test's two-incarnation row. The
combined review's CR-1 executed the class with #330's and #331's switches; those are measured Git operations, and their
engine consequences are inferred. Both close with this file.

**Corrected to master.**
- **The protocol route is closed by #330,** as C's section above records: both builders bind `GIT_ALLOW_PROTOCOL` to
  the empty value. It is not open.
- **C-4's route** is the one C's section above records, selected on 2026-10-11 by `pr11_decide_c` under the owner's
  delegation, and option (C) is that route's change.
- **O8 is adopted:** decision B on 2026-10-11, and its extension for option (C) by the owner on 2026-10-15.

**The remaining faces, each carried by its own record** (the ENV report's §2.2a, step 8):
- **The relative forms of the repository-wide names:** R-C-RELSTORE
  (`findings/P2_correctness_202610052142_a-relative-repository-wide-name-misresolves-in-an-engine-made-work-tree.md`).
  The owner adopted its route, a refusal for schema-4 runs, which is its own change after this one.
- **The other inherited names** this file lists (the object, configuration, namespace, execution-path and graph
  overrides): the adopted rule keeps them as the run's, the context the work tree was made under, and R-DELIBERATE is
  adopted as measured. ENV-1's removal of them was not adopted.
- **The protocol restriction:** closed by #330.

**Closure.** By step 8, this pull request closes this finding, with FUC-D5-GITINDEXFILE and CR-1, with `fixed` ledger
rows, when its exact-head validation and its own review agree; the closure deletes this file (`MAINTAINING.md` step 5).
