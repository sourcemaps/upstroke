---
id: CI-FLAKE-WINDOWS-SAMPLER-FSCK-TORN-REFLOG
severity: P3
disposition: deferred
category: correctness
pr: TBD
reviewed_sha: b73d347af70806578051ddd58a439c0db9269a7f
location: src/workspace_manager/tests.rs:13226
provenance: undetermined
first_bad:
guard: the change that classifies a torn reflog left by a kill during a reflog append, as tolerated residue or by a recovery that repairs it, or that establishes the kill reached only Git for Windows' launcher, in which case this finding merges into PR249-KILL-SAMPLER-WINDOWS-WRAPPER
---

# The kill sampler's classifier refused a reflog entry after a killed cherry-pick on a hosted Windows runner

## Failure sequence

    the census sampler kills the `git cherry-pick` child of an `Object.ProposalCherryPick` sample
    -> the classifier runs `git fsck --unreachable --no-progress --no-dangling --connectivity-only`
    -> fsck exits 2: `worktrees/s1_WG9T4H5MXCSP/HEAD: invalid reflog entry d0236017…`
    -> the classifier refuses (a failed fsck is an error, never an empty listing)
    -> the sampler's refusals assertion fails the test

`workspace_manager::tests::sampled_git_child_kills_every_residue_classified_and_recovered`
(`src/workspace_manager/tests.rs`, near line 13196) loops over `SAMPLED_SITES`. For each site,
`sample_site` (near line 13933) kills a Git child `SAMPLING_N` times and classifies what each kill
left with `classify_object_residue`. When the classifier returns an error, `sample_site` records it
as `run {run}: {error}` (near line 13986) and the sample counts in no class. The test asserts that
there are no refusals (`run.refusals.is_empty()`, line 13226) before anything derived from the
tally.

The fsck is `unreachable_objects` (`src/workspace_manager.rs`, near line 8190). It returns an error
when fsck exits non-zero. Its doc comment says a failed `fsck` "is an error, never an empty
listing", because a failed observation must refuse rather than certify that nothing is left.

For `Object.ProposalCherryPick`, each sample adds a staging worktree (`Slot::Staging { sequence:
run }`, so its registration is named `s{run}_<tag>`) and runs `git cherry-pick` in it. The child is
started by `SampledChild::spawn`, which runs `git` by name (`src/workspace_manager/tests.rs`, near
line 14435). On Windows the kill is `child.kill()` on that child alone, and `await_group_end`
returns at once (`src/workspace_manager/fixture.rs`, near lines 3603-3606 and 3643-3646).

## The one sighting

| run | job | platform | date | pull request | checkout |
|---|---|---|---|---|---|
| `44820173843` (`merge_group`) | `136578783650`, `test (windows-latest, 8 of 10)` | a GitHub-hosted `windows-latest` runner | 2026-11-20, 21:32:12Z to 21:44:16Z | #392 | `d40c56a5`, GitHub's merge-queue commit of #392's head `83bb3c8b` onto `master` at `bba18135` |

Read from the raw job log (`evidence/job-44820173843-136578783650.log`, SHA256
`41b3601ea2fba7501de47a11bfd1d9f9b468fdfbf6d0bdf5e06bb0b18b894b37`):

- The per-site summary:

      Object.CandidateStage       n=8 none=0 internal=0 after=8 unclassified=0
      Object.CandidateWriteTree   n=8 none=8 internal=0 after=0 unclassified=0
      Object.ProposalCherryPick   n=8 none=0 internal=2 after=5 unclassified=1

  There is no `Worktree.Add` line, so the loop ended at `ProposalCherryPick`.

- The panic, at `src\workspace_manager\tests.rs:13226:9`, is the sampler's refusals assertion. That
  is the first assertion on a site after its two count checks. Its message is `{site}: the
  classifier refused {} of {SAMPLING_N} samples, which is not the same thing as a residue in no
  class — it is an inspection that failed, and the assertions below would have called it
  unclassified: {:?}`. The last placeholder holds the refusals, each `run {run}: {error}`.

- The refusal's error was the classifier's own: `git fsck --unreachable --no-progress --no-dangling
  --connectivity-only` failed (exit code 2) with

      error: worktrees/s1_WG9T4H5MXCSP/HEAD: invalid reflog entry d02360179238b2eb4a6bd4319da07c1f448f7d44

- `test result: FAILED. 318 passed; 1 failed; 19 ignored; 0 measured; 3299 filtered out; finished in
  514.65s`. This test was the job's only failure.

Because the refusals assertion fired first, `record.recovered` for `ProposalCherryPick` was never
asserted. The log does not say whether that site's recoveries succeeded.

#392 changed Rust test files: `src/effects/tests.rs`, `src/effects/tests/ci_model.rs`,
`src/effects/tests/workflow.rs` and `src/engine/tests.rs`, for shard tables, assertions and workflow
fixtures. It did not change the failing test or the code it drives. The same test passed on #392's
head in CI run 44817216910 (`83bb3c8b`, 2026-11-20). It also passed on #392's next head in CI run
44841822496 (`1c457a42`, an empty commit on `83bb3c8b`, 2026-11-21), and the merge-queue run after
that merged #392.

## What is and is not established

**Established.** In one sample of `Object.ProposalCherryPick`, after the kill, the classifier's fsck
exited 2 on an `invalid reflog entry` in the reflog of a staging worktree's `HEAD`. The classifier
refused, and the test failed on its refusals assertion. The registration's name,
`s1_WG9T4H5MXCSP`, has the shape of this site's staging slot. The failing test and the code it
drives were not changed by #392, and the same test passed on #392's heads before and after.

**Not established: what the kill reached.** `SampledChild::spawn` runs `git` by name. On a Windows
machine where that name resolves to Git for Windows' launcher (`cmd\git.exe`), the kill ends the
launcher and the real `git.exe` keeps running
(`PR249-KILL-SAMPLER-WINDOWS-WRAPPER`). The job log does not record which `git` the hosted runner
resolved, and it does not show whether the kill reached Git or only its launcher.

**Inferred, not established: what was wrong with the reflog.** There are two readings, and the log
does not tell them apart:

- **The kill reached Git** while it was appending to the worktree's `HEAD` reflog, and left a torn
  entry that fsck then refused to read.
- **The kill reached only the launcher.** The real `git cherry-pick` was still writing when the
  classifier's fsck ran, because on Windows the sampler neither kills nor waits for the launcher's
  child. In that case fsck read a reflog that a live writer was still changing.

Reasoned from Git's source and not checked against the runner's Git version: fsck prints
`<ref>: invalid reflog entry <oid>` when a reflog entry names an object that fsck did not find among
the repository's objects. A line too short to parse is skipped instead of reported. That wording
fits the second reading at least as well as the first: fsck listed the objects before Git wrote the
commit, then read the reflog after Git had appended the entry naming it.

**What would establish it:**

- the bytes of the worktree's `HEAD` reflog (`.git/worktrees/<slot>/logs/HEAD`), kept beside the
  refusal (the sampler keeps only the error's words);
- whether object `d0236017…` was in the repository when fsck ran, and whether it was there later;
- whether any `git.exe` process of the sample was still alive when the classifier ran;
- the same fsck run again once every Git process of the sample has ended. If the error is gone, the
  first reading is ruled out.

**Not established: the rate.** One sighting. The other runs of this test on 2026-11-20 were not
counted.

## Related, not the same

- **`PR249-KILL-SAMPLER-WINDOWS-WRAPPER`**
  (`findings/P2_correctness_202609090020_pr5s-census-sampler-kills-gits-launcher-on-windows.md`)
  is the launcher cause: `SampledChild::spawn` runs `git` by name, so on Windows the kill can end
  the launcher and not Git. Its fingerprints are recovery failures at `Worktree.Add` on the
  self-hosted guest (`test (winguest)`): a forced removal denied access to a registration's
  `locked`, and `record.recovered` failing. This one is a classifier refusal at
  `ProposalCherryPick`, on a GitHub-hosted runner, raised before any recovery was asserted. It
  belongs to that finding only if the kill is shown to have reached only the launcher. Routing the
  spawn through `fixture::sampled_git`, the change that finding prescribes, is not shown to cure
  this one: a kill that reaches Git during a reflog append is the first reading above.
- **`PR247-SAMPLER-REFUSED-A-LOCKED-INDEX-ON-WINDOWS`**
  (`findings/P3_correctness_202609072359_sampler-refused-a-locked-index-on-windows.md`) is the
  nearest other refusal on record. It is a different test, the recovery module's cherry-pick
  sampler (`src/engine/topology/recover/tests.rs`), on the self-hosted guest. Its error is also
  different: the classifier's read of a staging worktree's `index.lock` failed with `Access is
  denied` (os error 5). Here fsck read the reflog and refused its content.

## What the change that takes this up should do

- **Find out what the kill reached and what the reflog held**, by keeping what the list above names
  beside any refusal of this sampler. If the kill reached only the launcher, merge this finding into
  `PR249-KILL-SAMPLER-WINDOWS-WRAPPER` and delete this file.
- **If the kill reached Git**, classify a reflog left torn by a kill during a reflog append: either
  as tolerated residue or by a recovery that repairs it. Do not let it read as an inspection that
  failed.
- **Keep the refusals assertion and `unreachable_objects`' rule** that a failed fsck is an error,
  never an empty listing. **Do not retry or skip the test** to get past this.

**A proposal, not adopted and not reviewed:**

- **The sampler** would, on a refusal, record the slot's `HEAD` reflog bytes and the named object's
  presence (`git cat-file -t <oid>`). It would wait until no Git process of the sample is alive
  (on Windows, by holding the child in a job object) before classifying.
- **The classifier** would recognise one specific case: fsck's only errors are `invalid reflog
  entry` lines on the killed command's own worktree `HEAD` reflog, and each names that reflog's last
  entry. It would answer that case as `Internal` residue instead of refusing. Its recovery would
  drop that last entry (for example with `git reflog delete`) before the recovery's own fsck.
  Every other fsck error would still refuse. This only applies once the sampler is shown to
  classify after Git has ended. Otherwise it would hide a live writer instead of naming one.
