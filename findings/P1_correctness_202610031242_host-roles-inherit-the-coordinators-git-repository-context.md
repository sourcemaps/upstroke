---
id: PR329-HOST-ROLES-INHERIT-THE-COORDINATORS-GIT-REPOSITORY-CONTEXT
severity: P1
disposition: deferred
category: correctness
pr: 329
reviewed_sha: 5c222ff2162da28aa471433773bf4966a883a428
location: src/runner/host/environment.rs:407
provenance: pre_existing
first_bad:
guard: the change that stops the host runner's role composition from handing roles the coordinator's inherited repository context, which G6 needs implemented and validated in its range unless the owner grants a specific exception (O3's change if the owner folds O3-R into O3, otherwise its own fix-P change)
---

## Failure sequence

Executed at `5c222ff2` through the host runner itself (`Runner::run_blocking`), with Git 2.43.0 on Linux, in temporary
repositories. The record is the O3-R evidence report, `/home/ubuntu/orch-pr11/evidence/o3r/REPORT.md` §3; a path below
without a leading `/` is relative to `/home/ubuntu/orch-pr11/evidence/o3r/`. A role here is a worker in its slot, or a
gate or review pass in its snapshot: each is a linked worktree of the managed repository.

1. The coordinator's environment holds `GIT_INDEX_FILE` naming another repository's index (case `index`), or `GIT_DIR`
   naming that repository's Git directory (case `dir`). Where ordinary Git supplies such a name is under "The
   precondition", below.
2. The host runner's base is the whole process environment (`src/runner/host/environment.rs:349`). `compose` removes
   only `PATH`, `HOME`, `USERPROFILE` and the three credential locations, then puts back the first three and, for the
   worker, the reviewer and an agent probe, that agent's own credential location (`:414-420`; the keys at
   `src/runner/host.rs:40-53`, the roles at `:63-70`). No `GIT_*` name is reserved.
   `supervise` starts the role with `env_clear()` and exactly that composition (`src/runner/host.rs:244-247`).
3. A worker's `git add role-file-a.txt` in its slot stages into the inherited index, and the slot's own index does not
   change (step 17). A second worker's add, in another slot, stages into the same index (step 19). A gate's
   `git ls-files --stage` in its snapshot then lists that index, holding the other repository's files and both workers',
   not the snapshot's (step 20). Under `GIT_DIR`, a reviewer's `git log --oneline -1` in its snapshot prints the other
   repository's commit (step 21).
4. A worker's `git add -A` (step 23), the usual agent idiom, makes the inherited index match its slot's tree. In the other
   repository (cases `index` and `dir`), the version its user had staged is no longer in the index (`fsck`: dangling
   blob), and its tracked file is staged for deletion (`D OTHER.md`). With `GIT_INDEX_FILE` alone, the blobs went to the
   slots' repository, so the other index names blobs its repository lacks (`fsck`: two missing blobs, exit 2), and
   `write-tree` fails (`fatal: git-write-tree: error building trees`, exit 128). Its user's next commit fails until the
   index is repaired.
5. A gate the runner kills at its timeout, while its `git update-index` holds the index lock (step 25), leaves
   `index.lock` in the other repository. The next index write there is refused (`File exists`, exit 128).

Sources: `w/<case>/out/steps.tsv` and `steps.txt` (every step's argv, exit and output), `w/<case>/out/index-states.tsv`
(the sha256 of every watched index and lock after every step), and `run/inspect.txt` (`fsck`, `write-tree` and the
refused write, read afterwards with ordinary Git in a clean environment); the report's tables C to G. With both names at
distinct paths (case `dir-index`), the same damage went to the inherited index file. The control case, with no inherited
name, addressed each slot's own administrative directory and index throughout. With the relative
`GIT_INDEX_FILE=.git/index`, every role's index read or write failed closed in its linked worktree (exit 128,
`Not a directory`).

## The precondition

The coordinator's environment holds an absolute `GIT_DIR` or `GIT_INDEX_FILE` naming a repository or index other than a
role's own. Ordinary Git supplies one with no export (Git 2.43.0):

- **In a linked checkout,** an ordinary `git commit` hands its `pre-commit` and `post-commit` hooks an absolute `GIT_DIR`,
  the checkout's administrative directory, and an absolute `GIT_INDEX_FILE`, its index. A `!` alias run there receives
  the absolute `GIT_DIR`. A Git command run with that environment from another checkout of the repository addresses the
  linked checkout's administrative directory and index, and takes its own directory as the work tree. The hooks were
  executed by the regression lens and the alias by both O3-R lenses (the regression lens's saved results:
  `/home/ubuntu/orch-pr11/reviews/o3r-witnesses/regression-lens/RESULTS.json` and `FOLLOWUP.json`); the query from
  another checkout was the grading lens's. All three are confirmed with ordinary Git in `fix/git/summary.txt`.
- **In a main checkout,** the `pre-commit` hook of `git commit -a`, or of a partial commit, receives an absolute
  `GIT_INDEX_FILE`, that commit's index lockfile (`run/shapes-2.txt`).
- **An explicit export,** such as a `GIT_DIR` workflow or a wrapper script.

So an upstroke run started from a commit hook or a `!` alias in a linked checkout, or from the `pre-commit` hook of
`git commit -a` or of a partial commit, meets it without anyone exporting anything. A plain commit's hooks in a main
checkout receive the relative `.git/index`. That shape fails closed in every linked worktree, whether a slot or a legacy
run's linked checkout (exit 128, `Not a directory`), and resolves to the checkout's own index only in a main checkout
(reasoned). The damage also needs a role that runs an index-writing Git command. Agents and Git-dependent gates commonly
run one, but no agent CLI was run.

## Severity: P1

- **Executed where executed.** The consequences in steps 4 and 5 were produced at `5c222ff2` through the runner's own
  `compose` and `supervise`: another repository's staged state replaced, its index left naming blobs it does not hold,
  and a foreign `index.lock` that refuses its next index write. They meet `MAINTAINING.md`'s serious-P1 test: the
  sequence is "concrete on the current head" and reaches "loss or corruption of data in a user repository — the engine
  owns git" (`MAINTAINING.md:176-190`).
- **Parity.** FUB-D9-ENV is P1 for the same precondition and the same consequence class, reached through the engine's
  own Git children (`DECISION-APPENDIX.md` §4.1). FUC-D5-GITINDEXFILE and FUD-D4-ENV, also P1, carry the same
  precondition (`DECISION-APPENDIX.md:1596`, `:1602`, `:1613`). Here the class is executed in the role path.
- **The precondition is stated.** `MAINTAINING.md` reclassifies down a P1 whose failure needs speculative preconditions
  (`:189-190`). This precondition is the three ENV P1s' own, and ordinary Git's environment in a linked checkout supplies
  it (above).
- **Production exposure.** `upstroke run` composes through `HostRunner::for_legacy_workspace`
  (`src/engine/coordinator.rs:60`). Under it the witness ran Git as the worker and the gate (steps 11-14), and composed
  all five roles' environments (the report's table A).
- **Graded.** P1 by the O3-R grading lens (`gpt-6-astra` at `max`, 2026-10-02). The regression lens found no conflict
  with the FUB-D9-ENV parity (`/home/ubuntu/orch-pr11/reviews/review-o3r-21367aba-triage.md`). The owner classifies
  (`MAINTAINING.md:189`).
- **What stays reasoned.** The execution is runner-level, on Linux with Git 2.43.0: the roles ran through
  `Runner::run_blocking` with the requests the conductors build. No whole-topology run and no real agent-CLI run took
  place. Concurrency (two roles writing one index at the same moment), Windows and macOS are reasoned.

## Applicability to G6

**Applicable,** conditionally on the precondition, under the host runner:

- **Q6.** Exact-commit verification on immutable snapshots: a Git-dependent gate or reviewer examines another index or
  repository (steps 20, 21 and 24). Untouched user checkout: when the inherited name designates the user's checkout, a
  role's ordinary `git add` rewrites its index (steps 17, 19 and 23).
- **Q4, aliasing.** Two invocations' Git shares one index, and under `GIT_DIR` one repository (steps 17-20).
- **Q2 and INV-22, Git administrative residue with its worktree.** The killed gate's `index.lock` is in another
  repository, outside every inventory row (step 25).

It does not bear on the broker, slot, ledger, lease or container claims. The container runner composes from the image
(`src/runner/container/env.rs:151`). The topology conductor is not enabled in production at `5c222ff2`:
`TOPOLOGY_ACTIVATION` is `Inactive`, asserted at compile time (`src/topology/schema.rs:27`, `:39`). Production's exposure
today is therefore the legacy conductor's, which no G6 claim covers. PR12, which may not merge until G6 passes
(`/home/ubuntu/orch-pr11/g6/PLAN.md:4`), enables the topology conductor with this composition unchanged.

**What clears it.** Only a fix implemented and validated in G6's range, or a specific, applicable owner exception
(`/home/ubuntu/orch-pr11/g6/PLAN.md:160-168`). Otherwise this applicable open high fails G6: it is repaired by a fix-P
change, the range is re-fixed, and G6 re-runs (`PLAN.md:167`).

- **Folding O3-R into O3** assigns the repair to O3's change. It is not clearance. Until that change is implemented and
  validated in the range, this finding is open.
- **A lower grade** is not clearance either. It would not make an unsatisfied Q2, Q4 or Q6 guarantee true: G6 passes
  only with its questions answered, as well as with no open critical or high finding (G6's `pass_fail_rule`, quoted in
  `run/external-citations.txt`).

## Provenance

The whole-environment base and the reserved-key removal arrived in `1a9cb205` (2026-08-19). It is an ancestor of G5's
range `d724fb16`, which has the same base and the same removal set (`run/composition-provenance.txt`). `first_bad` is
left empty: whether roles inherited these names through earlier launch code, before the host runner existed, was not
traced.

## What the change that takes this up should do

No remedy is drafted here. The change must show, failing at its base and passing after it, that a role started under an
inherited `GIT_DIR` or `GIT_INDEX_FILE` addresses only its own workspace's repository and index, with the sequence above
as its witness. It must also preserve, or obtain the owner's disposition for, each item of the evidence report's §10,
"What a remedy must preserve":

- the command configuration roles receive through the `GIT_CONFIG_COUNT` pairs and `GIT_CONFIG_PARAMETERS`;
- the legacy constructor's replacement-object policy;
- ENV-1's deliberately retained variables;
- parity between probes and executions;
- the handling of reserved keys;
- Windows' case-insensitive matching;
- the tests that pin the composition: `src/runner/host/tests.rs:113`, `:265`, `:317`, `:965` and `:1456`.

## Filed

At PR #329's implementation round, as a record only (the orchestrator's brief for that round, item 3; the PR11 owner
package's decision O3-R). Filing it decides nothing about its repair route, which is the owner's (O3 and O3-R). The
finding is the evidence report's, reviewed PASS (`~/orch-pr11/evidence/o3r/REPORT.md`, `d6dae2e2`); its text above is
the reviewed draft's, with its identifier and pull request set when filed.
