---
id: G5RUN3-LEGACY-ENGINE-TESTS-NEED-A-SHORT-WINDOWS-TEMP-PATH
severity: P3
disposition: deferred
category: portability
pr: 
reviewed_sha: d3ee2f25a068275f8bafe4536ac5390cfb5be93d
location: src/engine/tests.rs:4712
provenance: pre_existing
first_bad:
guard: the change that next takes up the legacy engine's worktree paths or these two tests; the tests should either build their scratch repository at a path whose worktrees stay inside Git for Windows' limit whatever `TMP` holds, or measure the headroom and say so, rather than depend on the ordinary temporary directory being short
---

## Failure sequence

Two tests of the schema-1..3 engine, `engine::tests::live_state_equals_replayed_state_across_every_ladder_path`
(`src/engine/tests.rs:4599`) and `engine::tests::a_legacy_resume_is_not_reinterpreted_by_the_new_engine_limits`
(`src/engine/tests.rs:2500`, which fails inside its helper `resume_answering`), run the legacy engine against a
scratch repository under `std::env::temp_dir()`. The engine creates a task worktree beneath that repository. On
Windows, Git for Windows 2.50.1 (with `core.longpaths` unset) refuses a worktree whose `.git` path is longer than
220 characters with `fatal: '$GIT_DIR' too big`, so both tests fail when the temporary directory is long enough:

```
thread 'engine::tests::live_state_equals_replayed_state_across_every_ladder_path' panicked at src\engine\tests.rs:4712:29:
reviewer-asks-for-a-human: git error: git worktree add failed: fatal: '$GIT_DIR' too big
thread 'engine::tests::a_legacy_resume_is_not_reinterpreted_by_the_new_engine_limits' panicked at src\engine\tests.rs:5713:6:
resume: Git { message: "git worktree add failed: fatal: '$GIT_DIR' too big" }
```

Observed at `d3ee2f25` on 2026-09-16 on the persistent Windows guest (`windowsguest`, Windows Server 2025, git
2.50.1.windows.1) during Gate 5 run 3's first guest suite, whose `TMP`/`TEMP` was
`C:\Users\Administrator\AppData\Local\Temp\g5r3-suite1`: the full suite reported `2567 passed; 2 failed`, these two.

**Measured, both ways, with only `TMP`/`TEMP` varied** (the two tests alone, same tree and binary):

| `TMP`/`TEMP` | characters added to `C:\Users\Administrator\AppData\Local\Temp` (41) | result |
|---|---|---|
| the directory itself (CI's ordinary temporary directory) | 0 | 2 passed |
| `…\g5r3q` | 6 | 2 passed |
| `…\g5r3qrs` | 8 | 2 passed |
| `…\g5r3qrstu` | 10 | 2 passed |
| `…\g5r3qrstuv` | 11 | 2 passed |
| `…\g5r3-suite1` (the full suite, not a control run) | 12 | these two failed (`'$GIT_DIR' too big`) |
| `…\g5r3-t8-ctl`, `…\g5r3-t10x-ctl`, `…\g5r3-t11xx-ctl`, `…\g5r3-suite1-ctl`, `…\g5r3-t14xxxx-ctl` | 12, 14, 15, 16 and 17 | 0 passed, 2 failed each |

So under CI's `test (winguest)` step, whose temporary directory is the 41-character one above, these tests pass with
**11 characters of headroom**: a Windows account whose profile path is 12 or more characters longer than
`C:\Users\Administrator` — or any runner that points `TMP` at a subdirectory of that length — turns them red with
a message that names neither the path nor its length. The rest of the suite, 2,567 tests, passed under the
12-character suffix.

The evidence is Gate 5 run 3's `guest/guest-suite1.log`, `guest/guest-tmplen-control.log` and
`guest/guest-tmplen-control2.log` (`~/tactus-artifacts/g5-evidence-d3ee2f2/` on the build box), and the re-run
suite `guest/guest-suite2.log` with an 8-character suffix: `2569 passed; 0 failed`.

## What the change that takes this up should do

Make the two tests independent of the temporary directory's length on Windows — a shorter scratch layout for the
legacy engine's worktrees, or `core.longpaths` for the fixture repository — or state the headroom they need where
the test builds its repository, so a red names the limit. Whether the legacy engine itself should refuse a
repository whose worktree path would exceed the limit, with a message that says so, is the same question for
users and belongs to whoever next changes that engine; the schema-4 engine's task worktrees were not measured
against this limit here.
