---
id: SUBMODULE-SWITCHES-LEDGER
severity: P1
disposition: deferred
category: security-trust
pr: 251
reviewed_sha: 6db9416cc2ecafe4d8d3820491705eff97cdfcc9
location: .github/scripts/validate-pr-branch.sh:2214
provenance: pre_existing
first_bad: 62bc799bdb008c7c37dcffe36f54e2fb3d19a008
guard: project owner
---

## Failure sequence

Repository discovery enters an initialised submodule and uses **that submodule's index**, without
first examining the **superproject's** recorded gitlink for the path. A `160000` entry is a gitlink,
not a directory to descend into, and the submodule's own index then answers for a path the
superproject records as absent from its ledger.

## What the PR #280 line of repairs closed, and what it did not

The **discoverable-superproject** form of this — the one first filed, where a plain checkout holds an
initialised submodule at `findings` and the superproject's `.git` is present — **is closed** at the
reviewed head. The walk continues out of the submodule into the superproject and decides from the
recorded `160000`, exactly as it does for `120000`. Measured on this box (git 2.43.0), a clean
checkout recording `findings` at `160000` with a finding-shaped file at the submodule root:

- the three tree listings: **exit 1, `names no finding`**
- the `findings` **directory**, and `.` from inside the submodule: **exit 1**, `records 'findings' as
  mode 160000`

Every environment route that reached it through a corrupted probe is closed with it: `GIT_TRACE=3`
(and the trace family) no longer defeats the capture of `--is-inside-work-tree` — `capture` runs the
producer with descriptors 3 and 4 closed — and `GIT_LITERAL_PATHSPECS=1` no longer defeats the
`:(literal)` the ascent matches the gitlink with — the pathspec-magic variables are unset at the top
of the validator. Both are asserted by the `env_case` group on the `env-superproject` fixture.

**What remains open is the hidden-superproject form, and it is the reason this finding is kept.**
Move the superproject's `.git` **outside** its worktree and export `GIT_DIR` and `GIT_WORK_TREE`
pointing at it, then validate `findings`:

- `git status --porcelain`: **exit 0**, empty — the checkout is clean
- `git ls-files -s` (through the pins): records `findings` at **`160000`**
- validator at `master`/`231c1aad`: **exit 1**; at head `6db9416c`: **exit 0, `conforms`**

First bad commit `62bc799b` (*"the environment does not choose which repository answers"*): its parent
returns `1`, that commit returns `0`. Clearing the pins — which that commit did so no walk can let the
environment select a ledger — is also what blinds the walk to a superproject that exists **only** in
the pins.

## Why it is not closed, stated as an impossibility rather than a deferral

In the hidden-superproject form the `160000` gitlink recording `findings` lives **only** in the
moved-out superproject's index, reachable **only** through `GIT_DIR`/`GIT_INDEX_FILE`. On disk the
submodule is a full repository with no `.gitmodules` above it; `git rev-parse
--show-superproject-working-tree` returns empty **with the pins set or unset** for a gitlink forged
with `update-index --cacheinfo`. So nothing that does not read the pins can tell this apart from two
inputs the suite requires to **conform**: `env_standalone` (a standalone repository asked about its
own root) and `repo-ordinary-nested-repository` (`fix-P2/correctness_under-a-gitlink`,
`test-pr-policy.sh`) — a nested repository whose own index holds the finding and which nothing
discoverable records. All three are byte-identical to every non-pin code path; the only distinguishing
fact is a `160000` entry in a pin-only index.

Refusing the hidden form therefore requires **reading the pins to locate the superproject**, which is
exactly what `62bc799b` removed and what this file's central invariant forbids — a verdict an exported
variable can flip, in either direction, is not a verdict, and the `env_case` fixtures assert a pinned
run and a clean run are byte-for-byte the same run. A refuse-only pin cross-check still lets the
environment flip the verdict and still breaks that invariant. The one non-pin hook that would catch it
— refuse a work-tree-root listing when the root ascent finds no discoverable superproject — was built
and measured: it closes the hidden form and turns `repo-ordinary-nested-repository` red, because those
two are the same input to code that may not read the pins.

## Why this does not block the merge

Owner ruling, 2026-09-11: *a P1 blocks a merge only if it can happen in normal use, or someone without
push access can trigger it.*

The discoverable form a fork could commit is now refused. The hidden form is not reachable by a
contributor without push access or by normal use: it requires the operator running the validator to
move the superproject's `.git` out of its worktree and export the pins themselves — a bespoke local
deployment, not anything CI does (`.github/workflows/pr-policy.yml` passes the three tree listings,
which refuse) and not anything a pull request's contents can arrange.

## What the change that takes this up should do

Owner, as the ledger records it: project owner.

This cannot be closed inside the current invariant that the environment never selects a ledger. Either
the invariant is amended — for a work-tree pin, a `160000` at or above the listing may **refuse** but
never conform, and the `env_case` equality is relaxed to "the pinned run refuses wherever the clean
run does, and may additionally refuse where the pinned ledger records a gitlink" — with that trade
argued in `MAINTAINING.md` and the fixtures rebuilt to assert it; or the directory form is withdrawn
for a pinned deployment and the caller is required to pass tree listings, which already refuse. Both
are owner decisions because both move the trust boundary this file rests on.
