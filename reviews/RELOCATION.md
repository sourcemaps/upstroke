# Findings located in a review record: relocated or marked for deletion

`reviews/` leaves master in this pull request. These are the twenty-four open findings whose
`location:` was a line of a file under it. A finding about the record's own text is marked
**delete**: the record leaves master as archived evidence, so the false text is gone from the
repository with it. No file is deleted in this run. The operator removes the files marked delete
with `git rm` in a separate commit after the run, and their text stays unchanged until then. A
finding about code, a test or a document that stays is **relocated**. Only its `location:`
changes, and every citation of a `reviews/` file in its guard or body is rewritten to the
archived `48f2af7a:reviews/<file>.md` form. Its id, severity, disposition, category, pr,
reviewed_sha, provenance and failure sequence are unchanged.

| id | action | reason | new location |
|---|---|---|---|
| `FUC-D2-PGIDREUSE` | relocated (kept: conditional on O5 selecting Q; U is built) | Against Q's writer group, which has no code on master; the follow-up C record is its only text. The operator's ruling keeps it open as the warning for a future Q build | `48f2af7a:reviews/2026-10-02-pr11-follow-up-c-record.md:728` |
| `FUC-D2-RG` | relocated | The defect is in the legacy engine's one Git builder, `git_command`. At this head it passes the four maintenance switches (`AUTO_MAINTENANCE_REFUSED`, line 100, from follow-up D). Closing the file is the owner's call (O10), not this task's | `src/workspace.rs:78` |
| `CLASS-INTERMITTENT-SUBPROCESS-KILL-SETTLE-RESIDUE-FAILURES` | relocated | A class of intermittent subprocess kill, settle and residue failures across four member tests. 50b91816 gave it a register as its location on purpose, not one member's site, and its body says so. Master has no register to replace it, and three members are filed with their own locations. So it keeps the archived register, not a code line: the owner may choose a site instead | `48f2af7a:reviews/2026-09-10-gate-G4.md:540` |
| `FUC-D3-SENTINELFALLBACK` | relocated (kept: conditional on O5 selecting Q; U is built) | Against Q's sentinel, which has no code on master; the follow-up C record is its only text. The operator's ruling keeps it open as the warning for a future Q build | `48f2af7a:reviews/2026-10-02-pr11-follow-up-c-record.md:2036` |
| `FUC-D3-OBSERVE` | relocated (kept: conditional on O5 selecting Q; U is built) | Against Q's resource rows R29 and R30, which have no code on master; the follow-up C record is its only text. The operator's ruling keeps it open as the warning for a future Q build | `48f2af7a:reviews/2026-10-02-pr11-follow-up-c-record.md:2041` |
| `PR258-INDEX-PACK-PACK-ONLY-IS-AN-UNSUPPORTED-UNIVERSAL` | delete | The overclaim is in #258's pull request body, which is not a tracked file. The finding clears the record sentence it was anchored to and the rustdoc of `temporary_object_files` | — |
| `PR331-A-RESUME-THAT-STOPS-BEFORE-ITS-REPORT-NAMES-NO-KEPT-PIN` | relocated | Legacy warning delivery: `drain_and_report` writes the failed drain's partial report and returns the error without its warnings (cited as `coordinator.rs:342-350` at the reviewed SHA) | `src/engine/coordinator.rs:390` |
| `PR290-R3-SEVEN-EXIT-CODE-CLAIMS-CITE-LOGS-WITHOUT-RECEIPTS` | delete | Seven sentences of the O3 record cite logs, outside the repository, that hold no exit status. The fault is in the record's own text | — |
| `PR331-KEPT-PINS-ACCUMULATE-UNTIL-THE-OPERATOR-REMOVES-THEM` | relocated | The coordinator keeps a refused attempt's candidate at its `-kept` pin, one pin per refused attempt, and nothing retires a pin (cited as `coordinator.rs:555` at the reviewed SHA) | `src/engine/coordinator.rs:610` |
| `PR258-SECTION-14-MISREPORTS-THE-REVIEW-ARITHMETIC` | delete | §14 of the fan-out record miscounts a review it quotes (8/2/2 for 7/2/3). The fault is in the record's own text | — |
| `R1F-F70-COUNT` | delete | The G5 report's F70 says 150 logs where the frozen evidence holds 151. Both the figure generator and the evidence are outside the repository | — |
| `PR290-R3-THE-CLASS-SEARCH-ACCOUNT-CLASSIFIES-A-FALSE-HIT-AS-TRUE` | delete | §17 B5 of the O3 record misclassifies a search hit. The sentence the hit names was corrected in 5da89afb, so only the record's account is wrong | — |
| `PR290-R3-THE-HEADING-COVERAGE-CLAIM-EXCEEDS-ITS-CENSUS` | delete | The O3 record's sentence "every new production item has a section" claims more than its census measured. `Convicted`'s field is covered in its enum's section in `docs/internals/events/mod.md`, which is what the notes rule (one section per comment) requires. So the notes have no gap | — |
| `PR290-R3-THE-ROUND-2-ACCOUNT-MISSTATES-ITS-FIGURES` | delete | The O3 record's round-2 account and #290's body misstate file, commit and construction counts. These are the record's own text and an untracked body | — |
| `R1F-PROSE-COUNTS` | delete | The G5 report's header says "only"/"everything else", which overstates how it states counts. Every count is correct, and the fault is in the report's own text | — |
| `G4R3-G4H-PUBLISHED-ANSWER-HANDOFF-NOT-EXECUTED` | delete | The G4 report credits G4H with a first-resume handoff that the harness does not run. That harness (`harness/g4r3_meas.rs`) is gate evidence, not a tracked file. The finding says this is not an engine defect and not a gap in any clause's evidence | — |
| `G4R3-Q3-UNDERSTATES-THE-SETTLEMENT-REFUSAL` | delete | The G4 report's Q3 credits its named test with less than the test checks. The test is stronger, so the fault is only in the report's text | — |
| `R1F-RESOURCE-SNAPSHOT` | delete | The G5 report's deviations paragraph omits a missing resource snapshot. The runner and the logs are outside the repository | — |
| `PR10-RECORD-STALE-SENTENCES-AFTER-ROUND-11` | delete | Five stale sentences in the PR10 record (and a copy in #279's untracked body). The finding says none of them has a behavioural surface | — |
| `R1F-TREE-TARGET` | delete | The G5 report's tree table names the wrong target base for the counted ST-18 run. The fault is in the report's own text, and no grade changes | — |
| `FUD-FULL-INSTRUMENTLIST` | delete | The follow-up D record's merge-bar lists omit `src/runner/contract.rs`. The finding executed that row and found it necessary and exact, so the omission is only in the lists | — |
| `PR258-FINAL-CHECKS-COVERAGE-MISATTRIBUTED` | delete | The fan-out record's §14.3 credits `45-r7-final-checks.log` with checking files it skipped by name. The checker and the evidence are outside the repository, and no evidence is missing | — |
| `FUD-FULL-STATUSTABLE` | delete | The follow-up D record's §0 status table stops at D8. The fault is in the record's own text | — |
| `R1F-SCOPE-WORDING` | delete | Four summary sentences in the G5 report drop F60's scope. Every grade carries that scope, so the fault is in the report's own text | — |

**Totals:** 24 findings.

- **7 relocated:**
  - 3 to code lines on this head: `src/workspace.rs:78`, `src/engine/coordinator.rs:390` and
    `src/engine/coordinator.rs:610`;
  - 3 kept at their archived record line, conditional on O5 selecting Q;
  - 1, the class row, kept at its archived register.
- **17 marked delete.** Their files are still in the tree, unchanged, for the operator's `git rm`
  after the run.
