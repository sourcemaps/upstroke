---
id: FUD-D4-ENV
severity: P1
disposition: deferred
category: correctness
pr: 331
reviewed_sha: 9b2262f4f2b515cc1181ed67df7f19756612b8a7
location: src/workspace.rs:43
provenance: pre_existing
first_bad:
guard: the change that removes the coordinator's inherited Git repository context from the legacy builder `git_command` — the owner's decision O3 (ENV-1, which in the legacy builder widens decision B's `src/workspace.rs` text as the PR11 decision appendix §4.5 drafts it) or O3-R — before G6, which cannot pass while this P1 is open (the combined review's CR-1) and does not defer it past G6; follow-up D's implementation (#331) does not take it up, by its brief
---

## Failure sequence

Found by PR #331's design review round 4 (`gpt-6-astra` at `max`, on `9b2262f4`), in three of its five lenses, and
triaged as FUD-D4-ENV, P1, pre-existing, for reconciliation (`/home/ubuntu/orch-pr11/reviews/review-331-d4-triage.md`,
its table). The lens texts are `/home/ubuntu/orch-pr11/reviews/review-331-d4-{concurrency,regression,design}-9b2262f4.review.md`
(R5 in the first two, finding 1 in the third), hashed in `/home/ubuntu/orch-pr11/reviews/SHA256SUMS-331-d4`; their
witnesses are under `/home/ubuntu/orch-pr11/reviews/331-d4-witnesses/`. Each step carries the label its source gives it.

1. The coordinator's environment holds `GIT_INDEX_FILE`. Ordinary Git supplies such a name with no export: a commit
   hook or a `!` alias in a linked checkout, or the `pre-commit` hook of `git commit -a` in a main checkout. That
   precondition was executed for the host runner's twin, under "The precondition" in
   `PR329-HOST-ROLES-INHERIT-THE-COORDINATORS-GIT-REPOSITORY-CONTEXT`'s file.
2. `git_command` (`src/workspace.rs:43-51` at `9b2262f4`; `:56` at follow-up D's implementation) is the one builder of
   every legacy Git child, as the module's own census holds. It passes `-C <dir>` and the replacement controls, and
   binds only `GIT_NO_REPLACE_OBJECTS=1`; it removes no inherited Git variable. Git reads `GIT_INDEX_FILE` in place
   of its default index ([Git's documentation](https://git-scm.com/docs/git#Documentation/git.txt-GITINDEXFILE)).
3. **Executed (the concurrency lens), on Git 2.43.0, 2.50.1 and 2.55.0:** a legacy process B stages the worker's paid
   edits into the inherited shared index (the capture's `git add -A`, `src/workspace.rs:500` at `9b2262f4`). Process
   A's `git worktree add` resets that same index to the base tree. B's next `git write-tree` (`:501`) captures the base
   tree, while B's branch `HEAD` is unchanged and the paid file is still on disk
   (`331-d4-witnesses/review331-d4-concurrency-yXt181/results.json`, case `inherited-index-interleaving`: the captured
   tree equals the base tree `3e21a205…`, not the paid tree `69fd9f16…`, on all three).
4. **Reasoned:** the legacy attempt then judges a candidate that is not the worker's output. For an implement task
   the empty diff fails the attempt, and the coordinator discards the checkout, the paid file with it. Follow-up D
   protects the candidate it captured; it does not prevent a capture corrupted before it.

## The other consequences

- **Executed (the regression lens), on the three Gits:** a legacy snapshot add with `GIT_INDEX_FILE=<common git
  dir>/index` succeeds and its status is clean, and the snapshot's registration then has no `index` of its own
  (`331-d4-witnesses/review331-d4-reg-5yb_1ksv/results.json`, case `inherited-index`: `local_index` false). So
  follow-up D's C-SIDE, if the owner takes it (O1, the appendix's O8 (ii)), would read a healthy snapshot's
  registration as not whole, and turn a genuine gate failure into a registry refusal that keeps a pin and costs another
  paid attempt.
- **Executed (the design lens, its P2), on the three Gits:** under an inherited alternate index, the `git ls-files
  --stage` re-check D §4.3 weighs, run through `git_command`, reads the intact alternate index and exits 0 while the
  gate's own index read exits 128 for its missing shared file
  (`331-d4-witnesses/review331-d4-evidence-s8z0n6v1/results.json`, case `inherited_index`). That alternative inspects
  a different index from the one the gate failed on.
- **Executed by the combined review (CR-1):** with follow-up D's switches, `add -A` in a linked worktree staged into the
  main checkout's index (the PR11 decision appendix, §4.1).
- **Reasoned:** the other repository-context variables (`GIT_DIR`, `GIT_COMMON_DIR`, `GIT_WORK_TREE`, the object,
  namespace and configuration overrides) reach every legacy child the same way; nothing was executed for them here.

## Severity: P1

P1 is the triage's grade, in the class graded P1 three times: `FUC-D5-GITINDEXFILE` (#330), `FUB-D9-ENV` (#329, the
manager's two builders) and `PR329-HOST-ROLES-INHERIT-THE-COORDINATORS-GIT-REPOSITORY-CONTEXT` (the host runner's
roles). Through the legacy builder the consequence is a candidate that is not the worker's output, and the discard of
paid output in the user's checkout (MAINTAINING's serious-P1 criterion). The owner classifies.

## Applicability to G6

The combined review's CR-1 makes the class block G6 through Q4, Q6 and DESC until a decision on O3 is implemented and
validated (the appendix's O3 row: "three P1s stay open; CR-1 blocks G6"). This file is the legacy builder's third. In
a mixed repository the resetting add of step 3 can be a topology add, whose own builder is `FUB-D9-ENV`'s; in a
legacy-only one both processes are legacy. Filing is not clearance, and no waiver is inferred.

## Provenance

`pre_existing`. `git_command` has bound only the replacement variable since `LEGACY-WORKSPACE-READS-REPLACEMENT-OBJECTS`,
and every legacy Git child before it inherited the whole environment; at `9b2262f4` the module is master `5c222ff2`'s.
`first_bad` is left empty: the builder's history was not bisected for this property.

## What the change that takes this up should do

**No remedy is chosen here.** The route is the owner's decision **O3** (ENV-1, the appendix's §4.2), whose legacy half
widens decision B's `src/workspace.rs` text by the four edits the appendix's §4.5 drafts (its composed text is Annex
R.9), or **O3-R**. Follow-up D's implementation does not take it up: its brief isolates ENV-1's widening with O3 and
forbids any change to a Git child's inherited environment, and D's five texts grant none.

What the appendix asks of the implementation is its §4.6: the legacy builder's `Command` reports every name of the
removed sets as removed; a behaviour test, red first at master, in which a legacy capture under an inherited
`GIT_INDEX_FILE`, `GIT_DIR` and `GIT_COMMON_DIR` addresses the checkout's own index and leaves the other index
byte-identical; and a mutation that drops the removal and turns both red. The module's Git-child census
(`every_git_child_of_this_module_is_built_where_replacements_are_refused`) counts `env_remove(` at zero today and would
move with it.

## Filed

At PR #331's implementation round (`pr11_fud_impl`), on the PR11 orchestrator's brief, as its own finding because its
scope is the legacy builder: `FUB-D9-ENV` covers the manager's builders, and O3-R the host runner's roles.
`reviewed_sha` is the head design review round 4 reviewed.
