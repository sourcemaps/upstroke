---
id: PR405-CUSTOM-HARNESS-GETS-LIBTEST-FLAGS
severity: P2
disposition: deferred
category: correctness
pr: 405
reviewed_sha: 3ee13a9e10b5d1c7615aa715b70becbfcb77d359
location: .github/workflows/ci.yml:551
provenance: introduced_by_feature
first_bad:
guard: the first target with `harness = false`, or the next change to script line 7
---

# A custom harness target is started with libtest's flags

Found by the frontier review's round three over #405 (finding 2), and filed deferred under ruling r46.

## Failure sequence

    a test target is declared with `harness = false`, so its `main` is the program's own and not libtest's
    -> the suite step collects it with the other test executables: its build message carries `profile.test: true`, and nothing in the build's JSON says it has no harness
    -> line 7 of the suite script starts every collected executable with the leg's filters and `-Z unstable-options --report-time` (`.github/workflows/ci.yml:551` at the reviewed commit)
    -> the custom `main` receives libtest's timing flags, which it did not ask for and need not accept
    -> it may refuse them and exit nonzero, failing the Windows legs on a target that passes under `cargo test` on every other leg

## What is established

- r44's probe (`/home/ubuntu/selfhost-wintimings-20261121/evidence/harness-false-probe.json`) showed that a
  `harness = false` target is collected: its build message carries `profile.test: true`, and the build's
  JSON has no `harness` field to tell it apart.
- Nothing is affected today: `Cargo.toml` declares no target table, so no target sets `harness = false`.
- Only the timing flags are new. The base step already handed every test executable the leg's filters.
- A custom `main` still runs, and the step keeps its exit code; nothing is skipped unseen.

## What the change that takes this up should do

- **Refuse such a target in the manifest**, beside the workspace refusal, so a `harness = false` target
  cannot arrive without the step being changed to suit it.
- **Or start it without libtest's flags**, which needs the step to tell a custom harness from libtest's
  by something other than the build's JSON.
- Its repair branch: `fix-P2/correctness_a-custom-harness-target-is-started-with-libtests-flags`.
