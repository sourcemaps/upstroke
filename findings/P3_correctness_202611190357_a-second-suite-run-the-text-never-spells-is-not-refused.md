---
id: PR389-UNSPELLED-SECOND-SUITE-RUN-UNREFUSED
severity: P3
disposition: deferred
category: correctness
pr: 389
reviewed_sha: f8147641a986f9811ad8c6a75f6752704bfc0593
location: src/effects/tests.rs:5620
provenance: pre_existing
first_bad:
guard: the next change to a Windows suite script
---

# A second suite run the text never spells is not refused

## Failure sequence

Read from the text, not run. No PowerShell runs on the build box.

`windows_suite_wait_complaints` in `src/effects/tests.rs` refuses a second run of the Windows
suite by reading a script's letters. It counts the word `cargo`, in any letter case, and the
word `Start-Process`. A line that starts Cargo without spelling either word holds nothing for
it to count. So:

1. `& ('car' + 'go') test --all-targets --all-features` is appended after the floor's refusal,
   in the guest's suite step in `.github/workflows/ci.yml` and in `WINDOWS_TEST_WITNESS`
   (`src/effects/tests/ci_model.rs`) together.
2. The oracle's pin compares the workflow's script with the constant. The two moved together,
   so the pin holds.
3. The added line holds no `cargo` and no `Start-Process`, so the function's fifth complaint,
   the one that says the suite can run twice, is not drawn. No other test reads the line.
4. On the runner PowerShell joins the two strings and `&` starts `cargo` with the suite's
   arguments. The suite runs twice, and no test refuses it.

The same edit to the shards' step and `WINDOWS_SHARD_WITNESS` reads the same way.

This is the spelling left over from `PR389-INDIRECT-SECOND-SUITE-RUN-UNREFUSED`. That finding's
witness named the program through `$cargo`, which the count skipped, and its repair counts every
`cargo` after line 2 whatever precedes it. A name the text never spells was outside the count
before that repair and is outside it after. The notes at the reviewed head say so of this
function: "A name put together as the script runs, two strings joined or the first word of
`$cargo`, starts Cargo with no `cargo` here to count, and nothing in this function refuses it."

## What the change that takes this up should do

Add no more spellings to the rule. A spelling answers one witness, and the next name is put
together another way.

Do one of these instead:

- Parse the script, or run it against a stand-in `cargo` and count the processes it starts, on
  a machine that has PowerShell. The check then reads what the script does, not how it is
  written.
- Close this as the owner's stated limit: the rule reads letters, for an edit made in good
  faith, and an edit to a constant or to the workflow is the owner's to read
  (`docs/internals/effects/tests.md`, under `windows_suite_wait_complaints`).

The choice between them is the owner's.
