---
id: G6-PRIVATE-DIR-LOSSY-LOCATOR
severity: P2
disposition: deferred
category: portability
pr: 332
reviewed_sha: 62bbbb4335da20fb51d4aad332a8df6f2c889639
location: src/engine/topology/create.rs:1890
provenance: pre_existing
first_bad:                # undetermined: the history under Provenance is pattern-level, not a per-site first-bad
guard: orch_pr11b owns the separate production design, freeze and applicability disposition before cumulative Gate 6. Not repaired, not accepted-risk, not exempted. Any compatibility or frozen-code choice the repair needs goes back to the owner as a concrete proposal before it is implemented; no schema or frozen amendment is adopted silently.
---

## Failure sequence

**The persisted private-directory locator is lossy.** Schema 4 records a run's private directory as text,
`RunStarted4.private_dir: String` (`src/topology/events.rs:379`), and production fills it with `to_string_lossy()`.
A path that is not valid UTF-8 is stored with U+FFFD where its invalid bytes were, and recovery later rebuilds the
private half from that text.

**The core schema-4 sequence. This is the executed one: W6, a run built by the test scaffold, then real recovery,
on Linux.**

a run's private directory has a path that is not valid UTF-8 -> its `run_started` records `private_dir` lossily
(production's writer is `src/engine/topology/create.rs:1890`; W6's run was built by the test scaffold, whose own
writer, `src/engine/topology/scaffold.rs:2928`, has the same `to_string_lossy()` shape) -> recovery derives the
private half from that text, `PathBuf::from(&started.private_dir)` (`src/engine/topology/recover.rs:135`) -> nothing
exists at the lossy spelling, so `RecordsVerified::verify` (`recover.rs:321`) returns `Refused`: "run `<id>` records
its private half at `<lossy path>` and nothing is there" (`recover.rs:324`) -> the run is not resumed.

**What the sequence needs, and what it does not claim.** The refusal fires because nothing exists at the lossy
spelling; that absence is part of W6's observed failure. Where the lossy spelling does name a directory (an alias or a
collision, such as a sibling whose name already holds U+FFFD), the outcome differs and was not executed. This records
that a schema-4 run whose private path is not valid UTF-8, and whose lossy spelling names nothing, is not resumed by
recovery. It does not record that every run with such a path cannot resume.

**Static related faces, read at `62bbbb43` and not executed:**

- **Production create.** The `run_started` writer above (in `p5b_publish_commit_record`) and the creating marker's
  writer, `src/engine/topology/create.rs:1641` (in `p1_publish_marker`); the marker's field, `src/rundir.rs:415`, and
  its readers, `src/rundir/discovery.rs:374` and `src/rundir/ownership.rs:157`.
- **Legacy, a separate face.** The writer `src/engine/coordinator.rs:217` and the field `src/events/mod.rs:206`.
  Resume rebuilds the private half from the text (`src/engine/resume.rs:422`) and then calls `paths.create()?`
  (`:424`). By reading, that creates a private half at the lossy spelling, or returns an error, depending on its
  parents; it does not refuse the way schema 4 does. No legacy write or resume was executed.
- **Status.** `src/status.rs:107` rebuilds the private half from the same text.

No claim is made about which public CLI paths reach these faces, or about earlier path validation. The INV-07
repairer found none in pre-lock or create by reading; that is not re-established here.

## Evidence

**Executed, Linux only:** row W6 of the INV-07 repairer's sealed slot for #332 (seal `MANIFEST-SHA256-q2.txt`;
`~/orch-pr11/logs/pr11_g6_inv07_fix1/slot/rows/W6/`).

- `upstroke-build env TMPDIR=<a directory named with byte 0xff> cargo test --lib --all-features -- --exact
  engine::topology::coordinator::tests::a_kill_at_the_declines_synced_append_resumes_into_the_interrupted_terminal`,
  at #332's repaired source, head
  `c3e61996ef70050b72989a20e99dca8a1047cd56`: rc 101 (`row.log`). That test is #332's, C10. It is not on master.
- The run is built by the test scaffold and killed at the decline's synced append. Reaching the refusal below means
  C10's preceding assertions held; babysit's ruling (sha256 `e6792739…`) takes that as the scoped evidence that
  the handoff and the earlier sequence were crossed. Real recovery then refuses: the test panics at
  `coordinator.rs:13023:14` on #332's branch with "the next process resumes the killed run: Refused { message: \"run
  `01KZSWEEP00000000000000001` records its private half at
  `/tmp/inv07fix1-trees-oj0tw43w/c10-tmpdir-w6-<U+FFFD>/[...]/private/runs/01KZSWEEP00000000000000001` and nothing is
  there. [...]" (`stdout`, `VERDICT.txt`; `[...]` marks text left out here). The raw `stdout` holds one U+FFFD
  (bytes `ef bf bd`) and no `0xff` byte (`~/orch-pr11/logs/pr11_g6_filing1/evidence-reads.txt`).
- **The production faces are static.** W6 executes the scaffold's writer and real recovery. It does not execute
  production create, legacy resume, status or the marker readers.
- **`recover.rs` at #332's repair is not byte-identical to master's** (#332 changes the file). Only the lines cited
  here were compared, and they read the same at both: the repairer's `q2-filings/private-dir-sites-at-master.txt`,
  which also records the differing blobs, and my re-reading at master,
  `~/orch-pr11/logs/pr11_g6_filing1/sites-at-master.txt`.

**Platforms.** Linux is the executed case. No cross-target build or native run of this sequence exists. Whether a
macOS or Windows host can hold such a private directory, and what `to_string_lossy()` stores for it there, is untested
here.

## Provenance

`pre_existing`: every cited line is on master. `first_bad` is undetermined. The commits below are where each string
first appears under its present path (path-restricted `git log -S --reverse` at master,
`~/orch-pr11/logs/pr11_g6_filing1/provenance-pattern-level.txt`); they are not where the complete failure first became
reachable.

- **Schema 4:**
  - the field: `bc071396` (2026-08-18);
  - the reader and the refusal: `5cd8747a` (2026-08-23);
  - both lossy writers, `run_started` and the creating marker: `f38882b1` (2026-08-24).
- **Legacy, kept apart:**
  - the status reader: `dc564758` (2026-08-09);
  - the legacy writer and the resume reader: `651c719a` (2026-08-16), a refactor that split `engine.rs`, so both
    are older under an earlier path;
  - the legacy field: `7a83e698` (2026-08-22).

  Their first appearance before those commits is not established.

## Severity

P2 `portability`, for the evidence scope above. A production persisted field holds a path as lossy text and later
uses it as the run's private-half identity, where
`standards/08_standards_filesystems_persistence_paths.md` says paths are "never assumed UTF-8. A lossy display string
is for diagnostics only, never identity." The executed face fails closed. A reclassification needs its own grounds;
no lower grade is taken to unblock anything.

## Related, and not covering encoding loss

- `PR7-STD-PRIVATE-ROOT-NO-CONTAINMENT`: the same recorded locator, for symlink and absolute-path containment.
- `PR7-STD-OWNERSHIP-PROOF-UNCANONICAL`: a lossy rendering of the public path in the ownership proof's fallback.
- `G6-KILL-CHILD-PATH-ENV-UNICODE` and `G6-KILL-ROOT-HANDOFF-LOSSY`: test-side encoding losses that a non-UTF-8
  `TMPDIR` meets before this one. A kill test whose run puts its private directory under such a `TMPDIR` meets this
  finding once those two are repaired.
- `INV07G6-REG-001`, #332's ledger row: the INV-07 repair round's ruling separated this limitation from it.

Neither PR7-STD finding duplicates this one, and neither is claimed to cover it.

## What the change that takes this up should do

Not decided here. The design, freeze and applicability disposition is orch_pr11b's, before cumulative Gate 6 (the
guard). The INV-07 repairer proposed two shapes, recorded only as proposals: record the private half losslessly, which
is a schema change, or refuse at creation, with a stated reason, a private directory that is not valid UTF-8. Either
touches production and frozen surfaces, so it goes to the owner as a concrete proposal first. Whatever lands needs a
witness that fails on this shape through production create and recovery on Linux, and a stated position on legacy
resume, status, the marker readers and the other platforms.

## Disposition

`deferred` records queued separate work under the guard. It does not accept the residual risk, does not promise that
this finding may stay open past Gate 6, and waives no `MUST` or witness rule.

Filed by `pr11_g6_filing1` on a findings-only branch, from the proposal of the INV-07 repairer
(`pr11_g6_inv07_fix1`, #332), under babysit's direction of 2026-10-16 (sha256 `aaeccd6a…`) and its two
clarifications (sha256 `876e1f56…` and `29131374…`). Dedup: `~/orch-pr11/logs/pr11_g6_filing1/DEDUP-RECORD.md`.
