---
id: CI-FLAKE-GATE-PROBE-TEXT-FILE-BUSY
severity: P3
disposition: deferred
category: correctness
pr: TBD
reviewed_sha: 7d96f365761e1187805bf3dba2f80043f8fa487f
location: src/engine/tests.rs:11155
provenance: undetermined
first_bad:
guard: the change that establishes which file was busy and stops the test executing it while it is open for writing
---

## Failure sequence

The test is
`engine::tests::the_gate_role_sets_its_worktree_value_again_only_past_another_registrations_empty_commondir`,
in `src/engine/tests.rs`. For each of its cases it writes a stand-in `git` program into a directory
of its own, makes it executable, and runs the gate-role probe (`engine::tests::v1_role_probe_gate`)
as a child process with that directory first on its `PATH`. It then reads the probe's records and
asserts, at lines 11154 and 11155, that the lines beginning `gate worktree-true` are exactly the
one the case expects.

It has failed twice in the `test (ubuntu-latest)` job of CI, each time as the only failing test of
the library suite:

- **CI run 41984755598**, on 2026-11-05, on the branch `feature/pr12-topology-activation`: 3676
  passed, 1 failed. Only the test's name and that summary were recorded when the run was surveyed;
  its log was not re-read for this finding.
- **CI run 42976030457**, job 130375210467, on 2026-11-11, on pull request #365 at `e11840e4`, a
  documentation-only change whose tree had passed CI three times as pull request #360's head: 3736
  passed, 1 failed, 203 ignored.

In the second, the panic is at `src/engine/tests.rs:11155:9`, in the case named `signal`. The
probe's record shows three probes failing the same way:

```
gate worktree-true FAIL Err("Text file busy (os error 26)")
gate fixture-inside FAIL Err("Text file busy (os error 26)")
gate fixture-outside FAIL Err("Text file busy (os error 26)")
```

and two succeeding, `gate variable ok` and `gate path-budget ok`. The assertion's left side is
`["FAIL Err(\"Text file busy (os error 26)\")"]` and its right side, what the case expects, is
`["FAIL Err(\"fatal: failed to read /r/.git/worktrees/upstroke-gates-2/commondir: Success\")"]`.

So the case expected the probe's Git call to fail in one particular way and it failed in another:
the operating system refused to execute a file that was open for writing. Operating-system error
26 on Linux is what executing such a file returns.

The same test passed in the Ubuntu job of the three earlier CI runs of the same tree
(42961933564, 42968003215 and 42973429587, on pull request #360), and it printed `... ok` in the
macOS job of the second run.

**It is a defect of the test, not of the engine,** as far as the evidence goes: the stand-in
program and the probe are the test's own.

## Class

**The prior ID of this class is `W2-HOST-TESTS-WRITE-THEN-EXEC-ETXTBSY`**, recorded in
`reviews/FINDINGS.md`: a test writes a program with `fs::write`, another thread of the test binary
forks while the file is open for writing and the child inherits the descriptor, and executing the
file then fails with "Text file busy".

**Established: the mechanism and the executable.** Both are supported by that prior finding, by
its deterministic witness of the mechanism (`inherited_writer_helper`, in
`src/runner/host/tests.rs`, which shows a forked child's inherited writer making `execve` fail
with `ETXTBSY` after the original descriptor is closed), and by the code: this test writes its
stand-in `git` with `fs::write` (`src/engine/tests.rs`, near line 11110) and the probe then
executes it, which is the class's write-then-exec shape. Commit `83029793` applied the class's
remedy to another stand-in `git` in `src/engine/tests.rs` for this same error. The busy file is
the stand-in `git`.

**Not established: which process held the descriptor** in these two runs. It is not known, and
the repair does not need it to be: the remedy closes the write in a process of its own, so no
thread of the test binary holds a writer for any fork to inherit, whichever one forked.

## Rate, owner and consequence

**Rate.** Two failures of this test in the Ubuntu test job, among the 120 most recent CI runs the
operator surveyed on 2026-11-10 and the runs of 2026-11-11. This is a count over the runs
surveyed, not a measured frequency.

**Owner.** The repair is the next task of the run that amends this finding.

**Consequence.** Only a failure with this fingerprint is this intermittent failure: the Ubuntu or
another Unix test job, this test, and a probe line ending in `Text file busy (os error 26)`. Any
other failure of the same test is a regression to be investigated, not a recurrence of this one.

## What the change that takes this up should do

1. **Write the stand-in `git` with `write_program_in_its_own_process`** (in
   `src/runner/container.rs`) instead of `fs::write`, as commit `83029793` did for the other
   stand-in `git` in `src/engine/tests.rs`. The program is then written and closed by a process
   of its own, so no thread of the test binary holds a writer to it when another thread forks.
2. **Keep the assertion as strict as it is.** A text-file-busy failure must not be accepted as the
   expected one: the case asserts how the probe's Git call fails, and widening it to admit this
   failure would stop it asserting that.

The mechanism's deterministic witness is the class's, `inherited_writer_helper` in
`src/runner/host/tests.rs`; this test's own failure cannot be produced on demand.
