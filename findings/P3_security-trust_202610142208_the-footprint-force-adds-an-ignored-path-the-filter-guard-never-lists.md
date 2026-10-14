---
id: FUD-F1R-IGNOREDFOOTPRINT
severity: P3
disposition: deferred
category: security-trust
pr: 331
reviewed_sha: dd13d447065c83b3926d750b2d8738462fc387e8
location: src/workspace.rs:1878
provenance: introduced_by_feature
first_bad: 45b1a7b1aa91b2a9393fa001f5e361db81a99da7
guard: an owner-directed follow-up after #331: by the owner's direction for #331 of 2026-10-14 (babysit's `d-owner-final-docs-findings-merge-direction-20261014.json`, sha256 `1128d021…`) this is filed open and deferred while #331 merges, and #331 does not change the behaviour; a change to follow-up D's capture after #331, owned by the PR11 orchestrator (`orch_pr11b`) for the owner, checks every footprint path for a filter attribute before its `git add -f` (or refuses such a path), proves it with an engine-level witness, and deletes this file
---

# The capture's footprint force-adds an ignored path the filter guard never lists

The ignored-path capture gap that the owner-session regression review of #331's F1 delta (`1263f2a3..dd13d447`)
reported beside its F1R-1, as behaviour of follow-up D's capture that predates the F1 round and is not among F2 to F8.
The reviewer estimated it P3 to P2, `security-trust`, and called that estimate uncertain and unscored; it is filed here
at P3, for the reasons below. Its evidence level: a Git 2.43.0 command-sequence experiment and a static trace, both the
reviewer's; the engine was not run. Line numbers are at `dd13d447`.

## Failure sequence

1. `.gitignore` ignores `foo`, and `HEAD` holds `foo/bar.txt`. The configuration the engine's Git children read
   defines a filter driver with a clean command (Git LFS's, for example).
2. A legacy worker deletes `foo/`, writes an ignored file `foo`, and writes an untracked `.gitattributes` holding
   `foo filter=<driver>`.
3. A capture with a footprint runs: N1's `Workspace::pin_checkout` after an attempt error (`src/workspace.rs:1301-1322`,
   footprint on at `:1307`), or a resume's `Workspace::discard_into_kept_pin` with one attempt in flight (`:1324-1337`,
   footprint on at `:1331`).
4. `capture_checkout` runs the filter guard once, first (`refuse_worktree_filters_before("git add")`, `:1820`). The
   guard asks `git check-attr` about the paths of `git ls-files --cached --others --exclude-standard` (`:579-588`):
   `.gitattributes`, `.gitignore` and `foo/bar.txt`, none of them filtered, so it passes. The ignored `foo` is not in
   that listing.
5. `fill_capture` runs `read-tree` and `add -A --ignore-errors` on the private index (`:1842-1857`), which leaves
   `foo/bar.txt` deleted from the capture; `in_the_way` (`:1894-1922`) returns `foo`, a non-directory standing where
   `HEAD` has a directory, whether it is ignored or not.
6. The footprint's `git --literal-pathspecs add -A -f --ignore-errors --pathspec-from-file=- --pathspec-file-nul`
   (`:1878-1889`) adds `foo` -> Git evaluates its attribute and runs the configured clean command on the worker's file
   -> configured filter code runs in an engine Git child on worker-written content, which the module refuses everywhere
   its listing reaches ("refusing before git add can execute configured filter code", `:704`), and the kept pin, with
   the resume's copy of it, then holds what the clean command wrote for `foo` rather than the file's bytes (reasoned
   from what `git add` stores for a filtered path; not observed).

**How it was established.** Step 6's filter run is the reviewer's executed Git experiment, not an engine run: in a
throwaway repository on Git 2.43.0 with this layout and a `filter.<name>.clean` that appends to a marker file, the
guard's exact listing and `check-attr` passed with the marker absent, `read-tree` and `add -A --ignore-errors` left it
absent, and the footprint's force-add with `foo` wrote it. The `in_the_way` walk for this layout was traced by hand by
the reviewer. This file re-reads the cited code at `dd13d447`, which shows steps 3 to 5 and the force-add of step 6;
it runs no experiment and no engine. A non-ignored untracked path in the way, and a tracked one, are in the guard's listing and so
are checked; only an ignored path, or an ignored file inside a directory the footprint adds, escapes it.

## Evidence and its limits

The reviewer's report, read here only from its neutral copy
`/home/ubuntu/babysit-pr11/evidence/d-owner-f1-delta-regression-report-20261014.md` (sha256 `5cbece51…`), with babysit's
intake `/home/ubuntu/babysit-pr11/evidence/d-owner-f1-delta-regression-intake-20261014.json` (sha256 `d1cadbda…`), and the
orchestrator's df3r triage, which returned this exposure for disposition and did not treat it as solved by prose
(`/home/ubuntu/orch-pr11/reviews/review-331-df3r-triage.md`, sha256 `01cfed4b…`). Its limits: Git 2.43.0 only, not 2.55.0;
no Windows, macOS, real Git LFS installation or container runner; the engine not run; the pin's content for `foo` in
step 6 reasoned, not observed.

## Category and provenance

- **Category `security-trust`:** the module's filter guard exists so that worker-written content cannot make the
  engine's own Git children run configured filter programs; its refusal says so, and its notes say the guard evaluates
  attributes "without invoking a driver" (`docs/internals/workspace.md:452-456`). This gap lets worker-written content do
  that for one class of path. The reviewer's estimate named the same category.
- **Severity P3**, the low end of the reviewer's uncertain, unscored estimate:
  - the program that runs is one the operator's configuration already defines; the worker only names a configured
    driver in an attribute and feeds it a file, and supplies no code;
  - it needs a configured driver with a clean command, an ignored path planted in the way of what `HEAD` holds, an
    attribute naming that driver on it, and one of the two footprint captures of step 3;
  - the legacy engine runs its workers through the host runner (`HostRunner::for_legacy_workspace`,
    `src/engine/coordinator.rs:60`, `src/engine/resume.rs:52`), where a worker with no operating-system sandbox already
    runs code as the operator, and where a gate that builds or tests the candidate already runs the worker's code;
  - the evidence is a Git-level experiment and a static trace, and no loss was shown.

  What would make it P2: an implementer confined by its own sandbox (Codex's `--sandbox workspace-write`, the boundary
  `findings/P2_security-trust_202609251755_the-accepted-risk-rationale-is-false-for-a-sandboxed-implementer.md` treats as
  real) in a plan whose gates do not run the candidate's code. There the gap is a capability the worker is otherwise not
  given. That setting was not measured. A reviewer may reclassify; the file is then renamed with `git mv`
  (`findings/README.md`).
- **Provenance `introduced_by_feature`:** the footprint, its `in_the_way` walk and its force-add are follow-up D's
  implementation round 3, `45b1a7b1` (R-D1's preservation design); master's merge base `5355b829` has no
  `--pathspec-from-file` in `src/workspace.rs` (`git log -S` over `5355b829..dd13d447`). It predates the F1 round
  (`015131b0`), whose documentation claimed a protection the guard does not give; that claim is F1R-1, corrected in #331
  without changing this behaviour.

## What the change that takes this up should do

Check every footprint path for a filter attribute before the force-add, with the same `git check-attr` the guard runs
(the reviewer's first suggestion), or refuse a footprint path that carries a `filter` attribute (its second), so that no
footprint `git add -f` evaluates a filter the guard has not seen. Prove it with an engine-level witness of this layout: a
marker filter under a driver name of the test's own, with `git hash-object` as the positive control, through both N1's
pin and the resume's guarded discard. Then bring the texts that describe the guard into line with it: the two
sentences F1R-1 narrowed (`design/15_design_event_log_resume_run_layout.md` and `docs/internals/engine/coordinator.md`),
and the notes of `worktree_filter_problem`, which say it enumerates "tracked and addable untracked paths".

## Disposition: open, by the owner's direction

The owner's direction for #331 of 2026-10-14
(`/home/ubuntu/babysit-pr11/evidence/d-owner-final-docs-findings-merge-direction-20261014.json`, sha256 `1128d021…`) says,
in the owner's words: "Fix F1R-1 and F1R-2 (docs only), file F1R-3 and the ignored-path capture gap under findings/, run
the baseline, get CI green and merge #331." Its disposition of this gap: filed as an open deferred follow-up while #331
merges, and not fixed in that documentation round. This file is that record. The documentation correction does not
close it: F1R-1 only stops the texts claiming that the ignored path is protected. It is not fixed, not an accepted risk
and not invalid, and its filing asks no further permission of anyone. It stays open until a change to follow-up D's
capture after #331, owned by the PR11 orchestrator (`orch_pr11b`) for the owner, makes the footprint's force-add refuse
or check a filtered path, proves it with the witness above and deletes this file with a `fixed` ledger row.
