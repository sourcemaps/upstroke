---
id: PR313-THE-8DOT3-WITNESS-FAILS-WITHOUT-8DOT3-NAMES
severity: P3
disposition: deferred
category: portability
pr: 313
reviewed_sha: 25edf23801a85ce1f46457a8266ab8d99228ae25
location: src/workspace_manager/tests.rs:3214
provenance: introduced_by_feature
first_bad: 25edf238
guard: none yet; a remedy may change where the witness runs but not what it requires where it runs, because a run that cannot build the 8.3 case has to end red or be counted as ignored, never as passed, and the remedy this file first proposed breaks that (see below); any remedy reaches the workflow, an instrument, so it is the owner's
---

## Failure sequence

`a_base_derived_from_its_8dot3_spelling_is_refused_under_the_spelling_the_manager_holds`
(`src/workspace_manager/tests.rs:3204` at `25edf238`, `#[cfg(windows)]`) re-spells its fixture's
base through `GetShortPathNameW` and **fails** when the answer is the base's canonical spelling but
for ASCII case (`:3213`–`:3222`). That is deliberate: #313's orchestrator required it in round 3.

This file first gave the reason as "then every later assertion passes while testing nothing". That
is false in the ordinary case: #313's round-3 delta review found it, and round 4 corrected it here
and in the witness's doc (`PR313-THE-8DOT3-WITNESS-DOC-MISSTATES-WHY-ITS-FIRST-ASSERTION-EXISTS`).
Without the first assertion:

- an answer that is the canonical spelling byte for byte still fails the witness, at its control
  (assertion 3), later and with a message that does not point at the volume;
- an answer that differs from the canonical spelling only in ASCII case passes the control, whose
  `contains` is case-sensitive, and every assertion after it, so the witness passes with nothing
  re-spelled but case. That is the case the first assertion alone guards, and why it compares with
  `eq_ignore_ascii_case`: with `!=` that answer would pass it too.

The cost falls on a Windows machine whose `TEMP` volume creates no 8.3 names, where no component of
the fixture's path has one:

1. 8.3 name creation is a per-volume setting (`fsutil 8dot3name query <volume>`) and can be off.
2. With `TEMP` on such a volume and no alias anywhere on its path, `GetShortPathNameW` answers the
   long spelling of the fixture's base, as it is documented to for a path with no short names on
   disk.
3. The witness fails at its first assertion, naming the spellings, before it reaches the code it
   exists to test -> the Windows suite is red on a configuration the crate supports, for a reason
   no change under test caused.

Established by reading, not executed: no volume without 8.3 names was available. At `26fed54c`,
`test (winguest)` passed the witness (run `36185668456`, job `108238739261`), so on the guest the
answer differed from the canonical spelling by more than ASCII case. On the hosted `windows-latest`
lane the first assertion holds whatever that volume's setting is now, because its `TEMP` is itself
spelled through the alias `RUNNER~1`.

This is the shape `PR258-CASEFOLD-GUARD-PLATFORM-SHAPED` records: #258's round 5 required a
filesystem property of every leg of a platform, which failed on a supported volume without it
(`PR258-CASEFOLD-EXPECTATION-KEYED-ON-TARGET-OS`), and round 6 replaced that with a declaration
made by exactly the steps whose temporary directory has the property.

## What the change that takes this up should do

**Not what this file first proposed. Nobody should act on that proposal.** It was the shape #258's
round 6 gave `the_temporary_object_scan_resolves_case_aliases_as_the_filesystem_does`: declare the
property on the Windows test steps whose `TEMP` volume keeps 8.3 names, with a variable beside
`UPSTROKE_TEST_TEMP_FOLDS_CASE`, and have the witness require its 8.3 case under the declaration
and only observe it undeclared. Here, observing it undeclared cannot do what it was for:

- On an undeclared machine whose answer is the canonical spelling byte for byte, relaxing the first
  assertion leaves the witness red, at its control. #313's round-4 Validation runs the Linux analog
  of exactly that, the first assertion removed and the spelling unchanged: exit 101 at assertion 3.
- To stop that red, the undeclared branch would have to end the witness before its funnel runs.
  That is a skip, and the suite counts a test that returns early as passed: a green that tested
  nothing, which is the result the first assertion exists to prevent.
- On an undeclared machine whose answer differs from the canonical spelling only in ASCII case,
  relaxing the first assertion lets every assertion pass with nothing re-spelled but case: the same
  analog, exit 0.

So a remedy may change where the witness runs, not what it requires where it runs: a run that
cannot build the 8.3 case has to end red, as now, or be counted as ignored, never as passed. One
shape that keeps that, neither built nor measured here, is the one
`a_native_case_insensitive_fan_out_alias_is_detected` has: `#[ignore]` with its reason, its
assertions unchanged, run explicitly with libtest's `--ignored` by the Windows steps whose `TEMP`
volume keeps 8.3 names. libtest totals ignored tests apart from passed ones. That test's own doc
records that nothing in CI passes `--ignored`, so the shape needs a workflow step, and that, like
any declaration, is the owner's.
