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
guard: the change that makes the sampler report, beside any classifier refusal, that sample's kill error and LaunchEnd, the slot's HEAD reflog bytes, whether the named object exists and whether a Git process of the sample is alive; until such a report shows the kill ended Git, no reflog error is classified as kill residue; a kill that reached only Git for Windows' launcher merges this finding into PR249-KILL-SAMPLER-WINDOWS-WRAPPER; a child that completed or failed before the kill stays a refusal, filed as its own finding
---

# The kill sampler's classifier refused a reflog entry after a kill fired at a cherry-pick on a hosted Windows runner

## Failure sequence

    the census sampler fires a kill at the `git cherry-pick` child of an `Object.ProposalCherryPick` sample
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

## The first sighting

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

## Three more sightings, 2026-11-23 and 2026-11-24

| run | job | platform | date | pull request | checkout |
|---|---|---|---|---|---|
| `45232814168` (`merge_group`) | `137896587187`, `test (windows-latest, 8 of 10)` | a GitHub-hosted `windows-latest` runner | 2026-11-23, 17:55:18Z | #410 | `1a2074d1`, GitHub's merge-queue commit of #410 onto `master` at `372269d2` |
| `45282352517` (`pull_request`) | `138069798393`, `test (windows-latest, 8 of 10)` | a GitHub-hosted `windows-latest` runner | 2026-11-23, 22:43:23Z | #412 at `680ba5f7` | `5d2d35d9`, GitHub's merge of `680ba5f7` into `master` at `4c9c566b` |
| `45355876016` (`pull_request`) | `138313434575`, `test (windows-latest, 13 of 14)` | a GitHub-hosted `windows-latest` runner | 2026-11-24, 08:24:46Z | #411 at `aaa9bdcd` | `d5b963d`, GitHub's merge of `aaa9bdcd` into `master` at `48f2af7a` |

All three failed the same test at the same site, `Object.ProposalCherryPick`, on the same
assertion, the sampler's refusals assertion at `src\workspace_manager\tests.rs:13226:9`. In each
job this test was the only failure.

- **Run `45232814168`** (`evidence/job-45232814168-137896587187.log`, SHA-256
  `5c54a589de4975994ff4477fe05e8b69d34f1d7471ad585290202d222f662f0a`). Line 712 marks the test
  `FAILED <188.866s>`. Line 744 is the panic at `src\workspace_manager\tests.rs:13226:9`. Line 745:
  the classifier refused 1 of 8 samples, run 3, fsck exit code 2, with

      error: worktrees/s3_WG9T4H5MXCSP/HEAD: invalid reflog entry 84c4d3c50cee3073185e5b86b79f94b2b00e430b

  The site's summary line is `Object.ProposalCherryPick: n=8 none=0 internal=2 after=5
  unclassified=1`. Line 751: `test result: FAILED. 319 passed; 1 failed; 19 ignored`. The same
  tree passed: merge-queue run `45239059571` checked out `4c9c566b`, whose tree is `1a2074d1`'s
  (`6e9e8c7e`), and passed; it merged #410.

- **Run `45282352517`** (`evidence/job-45282352517-138069798393.log`, SHA-256
  `ee4334bdfbf23a9b60fe09dbbfff3ce345e4ed96a7353e68a7cb0f10d37c0d35`). Line 744 marks the test
  `FAILED <200.921s>`. Line 756 is the panic. Line 757: the classifier refused 4 of 8 samples, runs
  1, 4, 5 and 7, each fsck exit code 2 with `error: worktrees/s<run>_WG9T4H5MXCSP/HEAD: invalid
  reflog entry <oid>` (oids `5fa4dc79…`, `f53abf9e…`, `0c394ebe…`, `07b2d272…`). The site's
  summary line is `Object.ProposalCherryPick: n=8 none=0 internal=2 after=2 unclassified=4`. Line
  763: `319 passed; 1 failed; 19 ignored`. The same tree passed: run `45291107344` on `b8522a9f`
  (an empty commit on `680ba5f7`) checked out `40b1922e`, whose tree is `5d2d35d9`'s (`98554cae`),
  and this test passed.

- **Run `45355876016`** (`evidence/job-45355876016-138313434575.log`, SHA-256
  `96e490353a810c8ab824d9f2c669acf2a5eedbd8962847d8b0549f5be094653e`). Line 467 marks the test
  `FAILED <146.459s>`. Line 479 is the panic. Line 480: the classifier refused 2 of 8 samples, runs
  1 and 5, with the same fsck error (oids `16577c61…`, `c568c797…`). The site's summary line is
  `Object.ProposalCherryPick: n=8 none=0 internal=2 after=4 unclassified=2`. Line 487: `90 passed;
  1 failed; 3 ignored`. The same tree passed: run `45364092561` on `6fa13e35` (an empty commit on
  `aaa9bdcd`) checked out `6f599e6`, whose tree is `d5b963d`'s (`1573c0c7`), and passed.

- In each log the earlier sites' summary lines (`CandidateStage`, `CandidateWriteTree`) are
  present and there is no `Worktree.Add` line. Each log's checkout step reports `git version
  2.56.1.windows.1`.

- None of #410, #411 or #412 changes `src/workspace_manager/`.

## A fifth sighting, 2026-11-24

| run | job | platform | date | pull request | checkout |
|---|---|---|---|---|---|
| `45384932886` (`pull_request`) | `138413285908`, `test (windows-latest, 13 of 14)` | a GitHub-hosted `windows-latest` runner | 2026-11-24, 11:31:22Z to 11:40:16Z | #413 at `d62eff5d` (an empty commit on `d4b85ec8`, with its tree) | `278c049`, GitHub's merge of `d62eff5d` into `master` at `0161b067` |

Read from the job's log as GitHub's API serves it, with ANSI escape sequences removed
(`evidence/job-45384932886-138413285908.log`, SHA-256
`d59419e550a8e61992bfb3053c9a668a9b19fdb510413231688a81d8eb6257a7`):

- Line 467:

      test workspace_manager::tests::sampled_git_child_kills_every_residue_classified_and_recovered ... FAILED <144.305s>
- Line 477 is the site's summary line, `residue sampling Object.ProposalCherryPick: n=8 none=0
  internal=2 after=5 unclassified=1`.
- Line 479 is the panic at `src\workspace_manager\tests.rs:13226:9`. Line 480: the classifier
  refused 1 of 8 samples, run 7, fsck exit code 2, with

      error: worktrees/s7_WG9T4H5MXCSP/HEAD: invalid reflog entry c2a9d8494eebf65d2d1449153ebb325b5107dba7

- Line 487: `test result: FAILED. 90 passed; 1 failed; 3 ignored`. This test was the job's only
  failure.
- The earlier sites' summary lines (`CandidateStage`, `CandidateWriteTree`) are present (lines 472
  to 475), and there is no `Worktree.Add` line. The checkout step reports `git version
  2.56.1.windows.1` (line 58).
- **The same branch tree passed.** Run `45391229767`, on `e1440e97`, an empty commit on `d62eff5d`
  with the same tree (`6a884542`), passed this test in `test (windows-latest, 13 of 14)` (job
  `138435174173`, `ok <157.217s>`). That run checked out `fdebbbd`, GitHub's merge of `e1440e97`
  into `master` at `c5061b81`, so its `master` side was newer than this run's.
- #413 changes only `CONTRIBUTING.md`, `MAINTAINING.md` and `README.md`.
- This run is the one the rate paragraph below lists among the runs "made because of the red" (it
  followed run `45376131057`). It failed too, under this row's fingerprint. It was created at
  11:31:20Z, after the end of that paragraph's population, so the paragraph's counts do not change.

## What is and is not established

**Established.** In one sample of `Object.ProposalCherryPick`, after the kill was fired, the
classifier's fsck exited 2 on an `invalid reflog entry` in the reflog of a staging worktree's
`HEAD`. The classifier refused, and the test failed on its refusals assertion. The registration's
name, `s1_WG9T4H5MXCSP`, has the shape of this site's staging slot. The failing test and the code
it drives were not changed by #392, and the same test passed on #392's heads before and after.
The three later sightings refused the same way, at the same site, in staging registrations of the
same shape, one to four samples of eight. The fifth, on #413, refused the same way, one sample of
eight.

**Not established: what the kill reached.** `SampledChild::spawn` runs `git` by name. On a Windows
machine where that name resolves to Git for Windows' launcher (`cmd\git.exe`), the kill ends the
launcher and the real `git.exe` keeps running
(`PR249-KILL-SAMPLER-WINDOWS-WRAPPER`). The job log does not record which `git` the hosted runner
resolved, and it does not show whether the kill reached Git or only its launcher.

**Inferred, not established: what was wrong with the reflog.** There are three readings, and the log
does not tell them apart:

- **The kill reached Git** while it was appending to the worktree's `HEAD` reflog, and left a torn
  entry that fsck then refused to read.
- **The kill reached only the launcher.** The real `git cherry-pick` was still writing when the
  classifier's fsck ran, because on Windows the sampler neither kills nor waits for the launcher's
  child. In that case fsck read a reflog that a live writer was still changing.
- **The kill did not land.** The sampler allows a child that reaches its own exit before the kill
  (`LaunchEnd::Completed`) and records one that fails by itself (`LaunchEnd::Failed`). The launch
  log holding each sample's kill error and `LaunchEnd` is read only after every site passes (near
  line 13296), so this log shows neither. The entry is then no residue of a kill: it may name a
  genuinely missing object.

Reasoned from Git's source and not checked against the runner's Git version: fsck prints
`<ref>: invalid reflog entry <oid>` when a reflog entry names an object that fsck did not find among
the repository's objects. A line too short to parse is skipped instead of reported. That wording
fits the second reading at least as well as the first: fsck listed the objects before Git wrote the
commit, then read the reflog after Git had appended the entry naming it.

**What would establish it:**

- the refused sample's kill error and `LaunchEnd`: `Completed` or `Failed` is the third reading;
  `Killed`, on Windows any exit code of 1, leaves the first two;
- the bytes of the worktree's `HEAD` reflog (`.git/worktrees/<slot>/logs/HEAD`), kept beside the
  refusal (the sampler keeps only the error's words);
- whether object `d0236017…` was in the repository when fsck ran, and whether it was there later;
- whether any `git.exe` process of the sample was still alive when the classifier ran;
- the same fsck run again once every Git process of the sample has ended. If the error is gone, the
  first reading is ruled out.

**Not established: the rate.** Five sightings, on 2026-11-20, 2026-11-23 (two) and 2026-11-24
(two), each followed by a pass of the same branch tree. The runs of this test in that period were
not counted.

**The test's rate across this row and PR249-KILL-SAMPLER-WINDOWS-WRAPPER, counted 2026-11-24.** The
population is every run of the `CI` workflow created from 2026-11-22 00:00Z to 2026-11-24 10:50Z
that ended with a conclusion of success or failure: 111 runs, all ended by 11:12:59Z (six runs in
that span were cancelled when a later event superseded them, and are not counted). A run counts as
failed on the test when one of its jobs reports
`workspace_manager::tests::sampled_git_child_kills_every_residue_classified_and_recovered ...
FAILED`. Six of the 111 did: runs 44970652323, 45019915010 and 45376131057 under PR249's
fingerprints, and 45232814168, 45282352517 and 45355876016 under this row's. Of the 47 runs in the
population created from 2026-11-23 12:00Z, four did. In each of the six the test was its job's only
failure. Five of the six were followed by a run made because of the red: 44978559642 (an empty
commit on #400), 45239059571 (#410 entering the merge queue again after the red ejected it),
45291107344 (#412), 45364092561 (#411) and 45384932886 (#413). The push run on `master`,
45019915010, was not run again.

## Related, not the same

- **`PR249-KILL-SAMPLER-WINDOWS-WRAPPER`**
  (`findings/P2_correctness_202609090020_pr5s-census-sampler-kills-gits-launcher-on-windows.md`)
  is the launcher cause: `SampledChild::spawn` runs `git` by name, so on Windows the kill can end
  the launcher and not Git. Its first fingerprint is that launcher measurement itself (the PR9
  repair-materialization sampler on the self-hosted guest at `56ea88c9`: 30 kills in 32 spawns, none
  landing mid-write). Its second and third, on the self-hosted guest (`test (winguest)`), are
  recovery failures at `Worktree.Add`: a forced removal denied access to a registration's `locked`,
  and `record.recovered` failing. The hosted-runner fingerprints that row
  gained on 2026-11-24 include a forced removal during `ProposalCherryPick`'s sampling, refused
  with os error 32, which is still a recovery failure. This one is a classifier refusal at
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
- **If the kill did not land**, keep the refusal. Do not classify, repair or delete the entry; file
  what the record shows as its own correctness finding.
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
  classify after Git has ended. Otherwise it would hide a live writer instead of naming one. Nor
  does it apply where the child completed or failed: that entry is no residue of a kill.
