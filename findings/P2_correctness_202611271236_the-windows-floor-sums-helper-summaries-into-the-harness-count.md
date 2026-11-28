---
id: PR439-WINDOWS-FLOOR-CHILD-SUMMARIES
severity: P2
disposition: deferred
category: correctness
pr: 439
reviewed_sha: 6513c22d24f8404b6877a6da578deaff19aa7224
location: .github/workflows/ci.yml:787
provenance: pre_existing
first_bad: not traced
guard: a change of its own, not #439, that counts each Windows shard's passes from a channel only the step's own harness writes; the floor check, its message and the printed sum stay
---

## Failure sequence

The count `test-windows-hosted`'s suite step holds to `matrix.floor`, at 6513c22d:

```powershell
$passed = [int](($log | Select-String -Pattern '^test result: ok\. (\d+) passed' | ForEach-Object { [int]$_.Matches[0].Groups[1].Value } | Measure-Object -Sum).Sum)
```

    the step runs each harness Cargo built, one at a time, and appends each one's standard
    output to one log (.github/workflows/ci.yml:782)
    -> a helper process a test starts can write its own libtest start and summary to that
       harness's standard output, so they land among the harness's lines
    -> the count (line 787) adds the passed count of every line that begins
       `test result: ok.`, whichever process wrote it
    -> helpers' passes count toward the floor: the acceptance reference for Windows engine 4
       is 99 passes of its own harnesses and nine of helpers
       (docs/internals/effects/tests/ci_model.md:583-585 at f16944ee), and the sum credits 108
    -> a harness that exits zero before writing its own summary passes the exit check
       (line 785), and passes the floor check (line 788) wherever its helpers' summaries and
       the other harnesses' sums reach the floor: nothing reads whether each harness reported

#439 replaced the sum with a reader of each harness's own lines at 729b3fe4 and revised it at
f8d610f3 and 19d6df17. Each version read a harness's completion from standard output that
carries no process identity; at f16944ee the reader was `.github/workflows/ci.yml:791-812`,
taking each log's first start and last complete summary and refusing a nested start of the
outer total. Ruling r85 narrows #439 back to origin/master's count, and 860fb0fa restores the
sum.

### The six cases reproduced in rounds two and three

Each is one harness's standard output. Cases 4 and 5 have no fixture at f16944ee; their lines
follow the recorded fixtures' shape.

1. A shuffled equal-size helper start (`PR439-WINDOWS-SHUFFLED-HELPER-COMPLETION`, repaired at
   19d6df17; `$shuffledHelper`, `src/effects/tests.rs:6283` at f16944ee). The harness exits zero
   with no summary of its own:

   ```text
   running 3 tests

   running 3 tests (shuffle seed: 1764201600000)
   test helper::first ... ok <0.001s>
   test helper::second ... ok <0.001s>
   test helper::third ... ok <0.001s>

   test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
   ```

   The sum credits 3. A count must credit none of them and refuse the harness.

2. The same after an interrupted should-panic record (`src/effects/tests.rs:6321` at f16944ee):

   ```text
   running 3 tests
   test parent - should panic ... running 3 tests (shuffle seed: 9)
   test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
   ```

   The sum credits 3. A count must refuse the harness.

3. An ignored test's reason ending in a start line (`PR439-WINDOWS-IGNORE-REASON-AS-HARNESS-START`,
   repaired at 19d6df17; `src/effects/tests.rs:6324` at f16944ee). The harness completed:

   ```text
   running 1 test
   test worker::needs_one ... ignored, requires a worker running 1 test
   test result: ok. 0 passed; 0 failed; 1 ignored; 0 measured; 12 filtered out; finished in 0.00s
   ```

   The sum credits 0, which is right. A count must accept the harness as complete; the reader of
   f8d610f3 refused it as a nested start of the outer total.

4. The multiline form of that reason: the reason holds a line break before `running 1 test`.
   The harness completed:

   ```text
   running 1 test
   test worker::needs_one ... ignored, requires a worker
   running 1 test
   test result: ok. 0 passed; 0 failed; 1 ignored; 0 measured; 12 filtered out; finished in 0.00s
   ```

   The sum credits 0, which is right. A count must accept the harness; the reader at f16944ee
   read the third line as a start and refused it, as its own note said
   (`docs/internals/effects/tests/ci_model.md:613-615` there).

5. Two interrupted test records before an equal-size helper start. The harness exits zero with
   no summary of its own:

   ```text
   running 3 tests
   test parent ... test child ... running 3 tests
   test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
   ```

   The sum credits 3. The start pattern at f16944ee (`.github/workflows/ci.yml:796` there)
   admits one interrupted record before a start, did not see this one, and credited 3 as well. A
   count must refuse the harness.

6. An outer start with a helper's summary and no outer summary (`src/effects/tests.rs:6306-6307`
   at f16944ee, the second of them):

   ```text
   running 5 tests
   running 1 test
   test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
   ```

   The first of them is the equal-total form, whose helper announces and passes five. The sum
   credits 1, or 5. A count must refuse the harness.

Established by reading the step at 6513c22d and at f16944ee, the removed fixture
(`fn the_windows_floor_counts_top_level_harnesses_and_requires_complete_results`,
`src/effects/tests.rs:6212` at f16944ee) and the count's notes there; nothing was run here.

## Why it is deferred

- The sum credits a harness with passes it did not report whenever a helper's summary lands in that
  harness's output as a line that begins `test result: ok.` with a positive pass count, and that
  needs no early exit. In #439's round four, both passes on 449fcfb1 ran
  a completed two-test harness whose completed one-test helper printed its own summary: the
  expression summed 2 and 1 to 3 and met a floor of 3 (`PR439-FLOOR-DEFERRAL-EXIT-PRECONDITION`).
  Cases 1, 2, 5 and 6 above are the ones that also need the step's own harness to exit zero before
  its own summary; ruling r85 records that no recorded leg shows a harness doing that. Cases 3 and 4
  are completed harnesses the sum already counts correctly; they are fixtures because #439's readers
  refused them.
- What a recorded leg does show, helpers' passes counted, raises a leg's count by what its
  helpers pass. It cannot fail a leg.
- The sum is origin/master's and #439 leaves it unchanged. Each of #439's three replacements read
  provenance from lines that do not carry it, and a channel only the harness writes is new
  machinery. Ruling r85 narrows #439, adds none, and files this as a change of its own.

## What the change that takes this up should do

- Credit each harness only with what that harness reports through a channel only it writes, not
  the standard output it shares with the helpers it starts, and refuse a harness that exits zero
  without reporting its completion there. Which channel is that change's design.
- Keep what surrounds the count: the per-harness loop and its exit handling, the output display
  and the survivor listing before the count, the floor check reading `matrix.floor` with its
  message, and the printed sum.
- Hold the count to the six cases above, each with the outcome given for it, and to the
  references the notes recorded at f16944ee (`ci_model.md:576-585` there): engine 4 credits 99,
  not 108; unit 14 + 1527 + 0 = 1541; runner 0 + 0 + 409 = 409; a completed zero-test harness
  credits zero.
- Move with it what describes or pins the count: the step's text in `WINDOWS_SHARD_WITNESS`
  (`src/effects/tests/ci_model.rs:41`), `the_windows_leg_counts_the_tests_it_ran`
  (`src/effects/tests.rs:5940`), and the account of the count at
  `docs/internals/effects/tests/ci_model.md:548`.
