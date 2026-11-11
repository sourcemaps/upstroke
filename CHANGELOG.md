# Changelog

## Unreleased

- A `[routing.strategy]` table with an unknown key is now refused at load, naming the key and the
  accepted `mode` and `spend_down_after`. It used to load with the key silently dropped, so a
  misspelled `spend_down_afer = 0.7` left `spend_down_after` unset with nothing said. An
  unrecognised `mode` value still warns and loads (`SWEEP-CONFIG-PARSE-008`).
- `upstroke run` now creates schema-4 runs on the execution topology: a worktree per task, the
  compare-and-swap merge queue, `max_parallel` above 1 bounded by `max_per_agent` and
  `max_per_pool` slots, and the optional container runner. `upstroke resume` continues one from
  its record, and `status` (with `--follow`), `answer` and `export-decisions` read one. A log of
  schemas 1-3 still resumes, reports and exports on the sequential path, and a binary that reads
  only up to schema 3 refuses a schema-4 log before any lock. A schema-4 run probes every agent
  its chains and its review plan name before anything is committed, and a configured reviewer
  whose CLI is missing fails its creation where the sequential engine dropped that reviewer. With
  no pins and the default `frontier` review tier, the review plan names Codex's CLI beside Claude
  Code's, so both must be installed, and Codex's local model catalog must list every catalogued
  Codex model (`gpt-6-astra`, `gpt-5.6-sol`) with all five efforts
  (`design/17`, "Which CLIs a schema-4 run needs"). This
  supersedes the entries below that call schema 4 "unreleased and inert by default": it is still
  unreleased, and no longer inert.
- The container runner now gives a worker or reviewer its own permission-settings file, read-only,
  where its CLI was handed a host path the container withholds; that one file, the one the engine
  materialized for the invocation, and no other. On a Unix host every container runs as the
  coordinator's effective numeric uid and gid, so the image and any credential volume must be
  usable by that user; nothing changes their ownership. And a command's stdin now reaches its
  container, which had read none: the prompt is written once, then EOF, by an attached start the
  process funnel owns, and that start waits no longer than the invocation's deadline.
- `[runner] mounts` now takes effect, as part of the run's recorded execution identity. A fresh
  container run resolves each mount before any lock or probe and records its canonical source,
  target, `read_only` and source identity (the device and inode on Unix; on Windows the volume
  serial number and the full 128-bit file id) in the run's `RunnerPolicy`, so the
  marker, `owner.json`, `run_started` and `report.json` carry it; a run without mounts keeps its
  exact bytes and its policy digest. Every role and probe gets the recorded mounts, read-only for
  review and probes and as written for implement and gate, so a gate can write through a mount
  configured `read_only = false`. Refused before any lock: a relative or missing source, one that
  is not a directory or regular file, one that is, contains or lies inside the repository, its
  Git, the private root or a host credential location, a path the `--mount` option cannot carry
  exactly (a `,`, a `"`, a control character, edge whitespace, or not UTF-8), and a target that is
  `/`, not in normal form, at or under `/upstroke` or `/tmp`, or the same as or nested in another.
  A config that wrote any of these was accepted while mounts were inert and is now refused. On a
  resume the recorded mounts win: a recorded source that is gone or rebound refuses before any
  spawn and the run stays resumable, and a different `mounts` today warns and is ignored. On
  Windows a source must be on a drive letter, and a link, junction or other reparse point is
  refused. A build for a platform that is neither Unix nor Windows refuses configured mounts.
- A decline, or a lineage member's failed settlement, that meets an open verification of its lineage now
  cancels the verification instead of releasing it: the schema-4 fold keeps the transaction, and its
  pipeline and merge entitlements, until its one terminal, `merge_verification_interrupted`, which the
  coordinator appends once the verification's processes have ended, after any
  `merge_verification_charged` record of the review passes it charged, (a halt's closure, or the next
  resume's recovery, when no live process settles it) before the pin, staging worktree and snapshots are
  reclaimed; the fold refuses that verification any other terminal, a new integration or the run's end
  until then (INV-07, L13).
- A schema-4 integration verification that ends with no terminal carrying the review passes it paid
  for — its own error after a pass was charged, a reviewer whose end the Runner did not establish, a
  halt, a shutdown, another pipeline's error, or a lineage failure that cancels it — now records
  those passes in a new `merge_verification_charged` event before the command or the integration
  ends, so a resumed run's reported spend, and the ceilings checked against it, count what the
  incarnation before it paid. The record is not a terminal: it settles nothing, so cleanup,
  readmission and recovery are unchanged, and a crash before its line is durable leaves the passes
  unknown spend, as before. The fold admits one record per sequence and no terminal that carries
  passes after it, so no pass is counted twice, and a binary without the kind refuses a log holding
  one. Schema 4 is unreleased and inert by default, so no released run is affected (`DESIGN.md` §26,
  "The unavailable terminal's spend").
- The four Unix helper endings that give up on a helper — the cleanup reaper that never said
  READY, the job-control guard whose descriptors could not be configured, the guard that never
  said READY, and a guard aborted after it said READY — no longer wait for that helper without a
  bound, and neither does the end of a cleanup reaper that did not acknowledge CLEANUP. Each asks
  `waitpid(pid, …, WNOHANG)` and polls it for two seconds, after which a helper still there is
  left for the process's exit to collect; the four endings' failure messages say so, naming what
  they saw rather than claiming the helper. A helper in uninterruptible I/O with the `SIGKILL`
  pending never becomes collectable, and the blocking wait these sites made did not return: the
  reaper's callers hold the launch barrier, under which the signal monitor refuses to kill or stop
  any registered group, so every running agent outlived a `SIGTERM` for as long as the kernel took.
  A CLEANUP the reaper did not acknowledge, from a reaper that stayed alive, likewise kept the
  supervisor from ever arming the fail-closed termination it answers that failure with. This
  supersedes, for the four endings, the sentence below that neither guard path "makes its wait
  again where it did not before": each now makes its wait again until the helper is collected or
  the budget runs out. **The wait after a CLEANUP or CANCEL the reaper acknowledged is
  deliberately still unbounded**, because the reaper's exit is what releases the run's cleanup
  lease its caller is about to act on. What a host's syscall policy sees changes at the bounded
  waits only: each is a `WNOHANG` poll made again until it collects the helper or the budget runs
  out, `waitpid(pid, …, WNOHANG)` by number where it passed `0`, and with
  `UPSTROKE_HELPER_IDENTITY=1` `waitid(P_PIDFD, fd, WEXITED | WNOHANG)` where it passed `WEXITED`.
  A policy written for the identity path must therefore permit both sets of `waitid` options, as
  `DESIGN.md` §15 now states; the acknowledged-exit wait still passes `0` by number and `WEXITED`
  through the descriptor, and the status-pointer shape every wait passes is unchanged
  (`PR125-CLOSE-UNBOUNDED-KILL-AND-WAIT-AT-FIVE-SITES`).
- Ending a Unix job-control guard the launch gives up on (its descriptors could not be configured,
  or the signal monitor could not start after it said READY) now reports what `kill` and `waitpid`
  answered, in the words a helper that missed READY already used, instead of discarding both
  answers. A refused signal is a distinct outcome the message names, and a helper's ending left as
  an unused expression statement is a build error. Both paths keep the `waitpid` they always
  made: each asks for no exit status, as it did before, and says so rather than reporting one it
  did not collect — so a host policy that refuses or kills a wait carrying a status pointer sees no
  new call from either, and neither makes its wait again where it did not before. Measured under
  both such policies, on each path. The retry the abort path has always made when a signal
  interrupts its wait is now bounded, so a wait that is answered `EINTR` every time ends in a
  reported failure rather than in a loop that never returns
  (`PR125-CLOSE-DISCARDED-KILL-RESULT`).
- A schema-4 integration verification that ends unavailable — parked for a person, or deferred by
  an infrastructure outage — now records the review passes it paid for, so a resumed run's reported
  spend is the total the incarnation before it reached. `merge_verification_unavailable` gains a
  required `reviews` field; a schema-4 log written without it is refused rather than replayed to a
  total it cannot account for. Schema 4 is unreleased and inert by default, so no released run is
  affected (`DESIGN.md` §26, "The unavailable terminal's spend").
- A Unix private helper (the cleanup reaper, the job-control guard) can be ended through a name
  the kernel cannot re-issue instead of through its pid. **Opt-in, Linux 5.4+, off by default:**
  with `UPSTROKE_HELPER_IDENTITY=1` each helper is created by `clone3` with `CLONE_PIDFD`,
  signalled with `pidfd_send_signal` and collected with `waitid(P_PIDFD, ...)`, and no other
  call is made. Setting it asserts that the host's syscall policy permits those three calls.
  With it unset, upstroke makes none of them and ending a helper stays the `kill` and the
  `waitpid` it has always been, which `DESIGN.md` §15 now states is best effort against an
  embedding host that reaps this process's children with wildcard waits.
- Relicensed to Apache-2.0 with a NOTICE file; earlier releases keep the terms recorded in their
  own tagged metadata and source notices (decided 2026-09-01).
- The G2 checkpoint: the v0.2 parallel-execution machinery (worktree-per-task isolation, the
  compare-and-swap merge queue, the optional container runner, the topology layer) merged to
  master inert by default. The v0.1 sequential path is unchanged and schema-4 state engages only
  by explicit schema choice; no `0.2.0` tag (G2 checkpoint promotion, decided 2026-08-31).
- Retired the App-signed `upstroke-frontier-review` attestation gate: its two privileged workflows,
  four scripts and fixture tests, and the signing environment are gone, and the default-branch
  ruleset requires `upstroke-ci` and `upstroke-pr-policy` only. The review obligation is unchanged;
  the owner's merge is the attestation (decided 2026-08-23).
- Renamed project from `tactus` to `upstroke`. Binary, crate, env-var prefix (`UPSTROKE_*`),
  user directory (`~/.upstroke`) and run directory (`.upstroke/`) all change; no aliases. The
  transformation is `scripts/rename-tactus-to-upstroke.sh`.
- `upstroke export-decisions <run-id>`: a local, read-only JSONL/CSV projection of a finished run's
  plan and attempt log (`DESIGN.md` §25).

## 0.1.0 — 2026-08-10

- The sequential conductor, end to end: plan ingestion, routing, the Claude Code and Copilot
  adapters, the engine with git ownership, gates, cross-family review, the verification ladder,
  the event log with resume and status, and the read-only capacity engine. The acceptance
  write-up, kept in the repository history, is the evidence.
