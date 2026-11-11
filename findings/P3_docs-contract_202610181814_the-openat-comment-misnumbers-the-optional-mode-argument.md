---
id: PR344-OPENAT-MODE-ARGUMENT-ORDINAL
severity: P3
disposition: deferred
category: docs-contract
pr: 344
reviewed_sha: d0fb7a3dcb717e44ae638d80f8c918d0b755ba50
location: src/engine/tests.rs:13084
provenance: fix_regression
first_bad: 1f4333ba62a822109284e94c105d398acac729d3
guard: "Next maintenance of the assertion-evidence helper's SAFETY comments; tracked by orch_pr11b after #344"
---

## Failure sequence

A reader consults the SAFETY comment beside the evidence helper's `openat` call.
The comment calls the optional variadic mode a third argument, although the
call already passes the directory descriptor, pathname and flags; a mode would
be the fourth argument overall. The reader receives an incorrect ordinal.

This is a wording observation from the cap344q9 repair fix-check at the reviewed
head. The reviewer retained PASS and resolved PR344-CAP1-REG-003: the helper's
fixed flags exclude `O_CREAT` and `O_TMPFILE`, so this call does not require a mode.
No failing test, reproduction or mutation witness accompanies this observation.
No runtime defect or invalidation of the source-inspected safety proof is claimed.

The observation is recorded separately from the three repaired findings.
Both cap344q9 and the whole-PR cap344q9r review passed at the reviewed head.
The fix-check log is identified by SHA256
`7d8a516e2bbb7daa637b4a51732994d3887549d09850e88a6eb5604e01609a2b`.

## What the change that takes this up should do

Change only the ordinal from "third" to "fourth", or describe the optional mode
without an ordinal. Preserve the fixed non-creation flags and the surrounding
safety obligations. Check the wording against the call signature by source
inspection; a test execution does not establish comment correctness.
