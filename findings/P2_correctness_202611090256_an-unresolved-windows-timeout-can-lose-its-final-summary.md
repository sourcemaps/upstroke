---
id: G7-M2-OPS-003
severity: P2
disposition: deferred
category: correctness
pr: TBD
reviewed_sha: 6df7ee5a11b0c11612b87ad95e9de272e9bfb46e
location: .github/g7-platform-environment/windows-container-acceptance.ps1:144
provenance: fix_regression
first_bad: b0d90549574a7200a472a18174e4cfea3139f99f
guard: the next change to the step-output reads and the finalization of `.github/g7-platform-environment/windows-container-acceptance.ps1`
---

## Failure sequence

The repair of G7-M2-OPS-001 (b0d90549) made every wait finite. It also added a path on which
`Invoke-Step` returns while its child may still be alive: the child passed its bound, `Kill()`
was requested through the held handle, and it was not seen to exit within `-GraceSeconds`. On
that path the child can still hold the writable handles `Start-Process` gave it for its redirected
stdout and stderr (line 112).

    a prerequisite such as git-version passes its bound, and its child is not seen to exit
    within the grace
    -> Invoke-Step sets the unresolved reason, appends the timed_out, settled=false, rc 124 entry to
       steps.jsonl (line 140), and returns
    -> the caller still reads that step's stdout for its check detail: Get-LastText at line 175,
       and again at line 179 for the source head
    -> Get-LastText is [IO.File]::ReadAllText (line 144). On .NET Framework it opens the file with
       FileShare.Read, which Windows refuses (ERROR_SHARING_VIOLATION) while another handle has the
       file open for writing
    -> the exception is uncaught under $ErrorActionPreference = 'Stop', so the script ends there
    -> that step never gets its FAIL row in checks.tsv, and SUMMARY.txt (line 237) and
       MANIFEST.json (line 246) are never written. A still-open build or capture output could
       likewise make the final Get-FileHash pass fail

The run's end and its raw evidence survive. The timeout entry is already in steps.jsonl, every
output file is kept, the script exits non-zero, and the verdict can never be CAPTURED. What is
lost is the terminal summary naming the unresolved reason, and a complete manifest. The repaired
recipe promises both on exactly this path (its header, lines 23 and 29).

This is a conditional inference from source, made in the operations review of 6df7ee5a, not an
observation. No run has left a child alive past its grace, no native reproduction was made, and
no false capture or mandatory-gate defect is claimed. The native helper check of the OPS-001
repair does not cover this composition: it loads `Get-LastText` but never calls it on a
timed-out step, and it simulates the refusal without running the recipe's finalizer.

## What the change that takes this up should do

Make the reporting on the unresolved path independent of reading a file that the unresolved
child may still hold open:

- read a step's output only after its process was observed to exit, or open it in a way that
  tolerates a live writer (`FileShare.ReadWrite`), and record a placeholder such as
  `output unreadable: <reason>` instead of throwing;
- write the summary and the manifest from a `finally`, or behind a handler, so a failure anywhere
  after the unresolved step still produces SUMMARY.txt with `unresolved_termination=...`;
- in the manifest, name any file that cannot be read or is still being written as such, rather
  than requiring every file to hash.

Keep the raw files, the refusal of every later step and the non-zero exit. No descendant cleanup
or fault campaign is asked for. Any native check of the change should run the recipe's own
finalizer on the unresolved path, which the current helper check does not.
