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
guard: a per-leg declaration in the workflow, the shape `UPSTROKE_TEST_TEMP_FOLDS_CASE` already has, under which the witness requires its 8.3 case and without which it only observes it; the workflow is an instrument, so the change is the owner's
---

## Failure sequence

`a_base_derived_from_its_8dot3_spelling_is_refused_under_the_spelling_the_manager_holds`
(`src/workspace_manager/tests.rs:3204`, `#[cfg(windows)]`) re-spells its fixture's base through
`GetShortPathNameW` and **fails** when the answer is the base's canonical spelling but for ASCII
case (`:3213`–`:3222`). That is deliberate: #313's orchestrator required it in round 3, because then
every later assertion passes while testing nothing, and a skip would be counted like a pass.

The cost is on every Windows machine whose `TEMP` volume creates no 8.3 names:

1. 8.3 name creation is a per-volume setting (`fsutil 8dot3name query <volume>`) and can be off.
2. With `TEMP` on such a volume and no alias anywhere on its path, `GetShortPathNameW` answers the
   long spelling of the fixture's base, as it is documented to for a path with no short names on
   disk.
3. The witness fails at its first assertion, naming the spellings, before it reaches the code it
   exists to test -> the Windows suite is red on a configuration the crate supports, for a reason
   no change under test caused.

Established by reading, not executed: no volume without 8.3 names was available. Whether the guest
lane's volume keeps them is measured by the witness itself, at #313's pushed head on
`test (winguest)`. On the hosted `windows-latest` lane the first assertion holds whatever that
volume's setting is now, because its `TEMP` is itself spelled through the alias `RUNNER~1`.

This is the shape `PR258-CASEFOLD-GUARD-PLATFORM-SHAPED` records: #258's round 5 required a
filesystem property of every leg of a platform, which failed on a supported volume without it
(`PR258-CASEFOLD-EXPECTATION-KEYED-ON-TARGET-OS`), and round 6 replaced that with a declaration
made by exactly the steps whose temporary directory has the property.

## What the change that takes this up should do

Do what #258's round 6 did. Declare the property on the Windows test steps whose `TEMP` volume keeps
8.3 names, with a variable beside `UPSTROKE_TEST_TEMP_FOLDS_CASE`, and have the witness require its
8.3 case under the declaration and only observe it undeclared. That keeps what this round insisted
on where it matters, since a declared leg that cannot build the case is red, and it stops an
undeclared developer machine from going red over a volume setting. The workflow oracle would pin
the new declaration as it pins the case-fold one. The declaration is a workflow change, so this is
the owner's to make.
