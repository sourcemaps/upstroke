# Changelog

## Unreleased

- `upstroke decide <run-id> <question-id>` is new: it asks a parked run's primary reviewer,
  read-only, to propose an answer to one open question, named by its exact id, byte for byte (a
  question that chooses an agent is refused), checks the reply (one fenced `json` block and no
  three backticks or tildes on any line outside it, an offered option, guidance that never gives
  up), prints it and saves it in its claim of the run, `decide/` in the run's private directory,
  as `proposal.json` beside the raw output; a refused reply makes it exit non-zero after the
  report, whose exit says the reason is saved in the proposal. Its refusals and configuration
  errors are one line each. It writes no answer and replaces no file. The run id is checked as a
  plain id before any path is built from it. The command holds the run's own lock, the one
  `resume` takes, from before the run is read until the proposal is written or the command fails,
  so no resume starts while it asks and nothing is asked of a run a coordinator holds; it asks
  only about a run whose log's last end records it parked, with no resume since, and refuses an
  interrupted one. A run is decided once: immediately before its one call the command claims the
  run by creating `decide/` exclusively in the run's private directory, which every copy of the
  checkout shares, and a run whose claim exists is refused with nothing written; the claim stays
  after a failure or an interruption, and no other decide of the run, from any copy, spends
  anything. The claim has no mode of its own, so on Unix the private directory must be the user's
  own and closed to group and other, or the run is refused before anything is written or asked;
  each file is created straight after that check is made again, with no call between the two,
  though they remain two operations on a path. The call is capped at the run's `--max-cost-usd`
  allowance (default 5), and only a clean, successful, priced call is settled, at its price: a
  result the reviewer marks as an error never settles. Any other outcome is saved as refused, and
  so is a call that reports more than its ceiling, which fails the command and says so. A write
  that fails after the call fails the command naming what the call cost, and is worded as a
  write. It asks only a Claude Code reviewer of a host run whose pools pass the credential
  pre-flight a run makes, read as `upstroke capacity` reads them (so a setting a resume refuses
  does not refuse decide), and whose CLI passes the probe and sign-in checks a run makes of it,
  under the profile a resume probes it under: an account reported signed out is refused, and one
  whose CLI cannot say passes, as it does for a run. It gives the call the recorded model and
  review effort, the allowance as its own `--max-budget-usd`, no web access, no read of the run's
  private directory and no environment overlay, each checked on the command as built; any other
  run or command is refused before anything is written or asked. A question a person has answered
  already, checked at admission and again immediately before the call, is refused and never
  asked, and the prompt holds only what the run recorded: no plan file of the checkout goes into
  it, so an edited one is withheld and the reviewer is still asked.
- Capacity is now tracked for the machine rather than for a repository; `[capacity] scope = "repo"`
  or `upstroke capacity --scope repo` folds only this repository. Nothing routes on any of this yet.
  Only an attempt that passed, in a run of any schema, clears another run's rate limit on its pool,
  and only when its worker finished after that limit was met.
- `upstroke capacity --refresh` is new: the capacity tracker, asking each pool's `usage_provider`
  for this month's draw (`usage_token_env`, `usage_account`); no other command reaches a network.
- Settings now have a machine level: `~/.upstroke/upstroke.toml`, beside the pools file, is read
  beneath every repository's own; with no such file, a repository is read exactly as before.
- `upstroke capacity` and `validate` now show the usage-limit signals and spend of a schema-4 run
  when it is the repository's latest.
- A schema-4 run's pre-flight, on `upstroke run` and `resume`, now refuses it when Claude Code's
  account or a Claude Code pool's `profile` is not signed in, as `claude --setting-sources ""
  auth status --json` reports it; a CLI that cannot say whether it is signed in still passes
  (`PR386-PROFILE-AUTH-NOT-PREFLIGHTED`).
- When a usage limit stops a schema-4 worker and its agent has another pool the run has not seen
  exhausted, the task now runs again at once on that pool instead of waiting for the reset.
- A schema-4 run whose remaining tasks are deferred on a usage limit now waits until two minutes
  past the reset Claude Code reported, if that is at most 5 h 15 min away.
- A schema-4 run now records `pool_exhausted` when a usage limit stops an attempt, with the reset
  Claude Code reported, as a resumed older run already did.
- `upstroke answer` now refuses a second answer to a question that already has one published,
  however the two are timed (`PR381-ANSWER-PUBLISH-RACE`).
- The startup census reclaims a marker-less run directory only on a complete listing, and
  `upstroke status <run-id>` reports one it cannot list (`SWEEP-CLASSIFY-009`, `SWEEP-NAMES-005`).
- A `[routing.strategy]` table with an unknown key is now refused at load, naming the key and the
  accepted `mode` and `spend_down_after`; an unknown `mode` still loads (`SWEEP-CONFIG-PARSE-008`).
- `upstroke run` creates schema-4 runs on the execution topology (`max_parallel`, `max_per_agent`,
  `max_per_pool`); still unreleased, no longer inert, superseding the entries below (`design/17`).
- The container runner now gives a worker or reviewer its own permission-settings file, read-only,
  where its CLI was handed a host path the container withholds. On Unix every container runs as the
  coordinator's effective uid and gid, so the image and any credential volume must be usable by that
  user, and a command's stdin now reaches its container.
- `[runner] mounts` now takes effect on Unix and Windows, as part of the run's recorded execution
  identity; a config accepted while mounts were inert may now be refused. On Windows a source must
  be on a drive letter, and a link, junction or other reparse point is refused.
- A decline, or a lineage member's failed settlement, meeting an open verification of its lineage
  now cancels it instead of releasing it, ending in `merge_verification_interrupted` (INV-07, L13).
- A schema-4 integration verification that ends with no terminal carrying the review passes it paid
  for, now records those passes in a new `merge_verification_charged` event (`DESIGN.md` §26).
- The four endings that give up on a Unix helper, and the end of a cleanup reaper that did not
  acknowledge CLEANUP, now bound their wait: a two-second `WNOHANG` poll, `waitpid(pid, …,
  WNOHANG)` by number where it passed `0`, and with `UPSTROKE_HELPER_IDENTITY=1` `waitid(P_PIDFD,
  fd, WEXITED | WNOHANG)` where it passed `WEXITED`, so a syscall policy for the identity path must
  permit both sets of `waitid` options. The wait after an acknowledged CLEANUP or CANCEL is
  deliberately still unbounded (`DESIGN.md` §15, `PR125-CLOSE-UNBOUNDED-KILL-AND-WAIT-AT-FIVE-SITES`).
- Ending a Unix job-control guard the launch gives up on now reports what `kill` and `waitpid`
  answered, instead of discarding both answers (`PR125-CLOSE-DISCARDED-KILL-RESULT`).
- A schema-4 integration verification that ends unavailable now records the review passes it paid
  for, in a new required `reviews` field of `merge_verification_unavailable` (`DESIGN.md` §26).
- A Unix private helper can be ended through a name the kernel cannot re-issue instead of its pid:
  **opt-in, Linux 5.4+, off by default**, with `UPSTROKE_HELPER_IDENTITY=1` (`DESIGN.md` §15).
- Relicensed to Apache-2.0 with a NOTICE file; earlier releases keep the terms recorded in their
  own tagged metadata and source notices (decided 2026-09-01).
- The G2 checkpoint: the v0.2 parallel-execution machinery merged to master inert by default. The
  v0.1 sequential path is unchanged; no `0.2.0` tag (2026-08-31).
- Retired the App-signed `upstroke-frontier-review` attestation gate: the default-branch ruleset
  requires `upstroke-ci` and `upstroke-pr-policy` only; the review obligation stands (2026-08-23).
- Renamed project from `tactus` to `upstroke`. Binary, crate, env-var prefix (`UPSTROKE_*`), user
  directory (`~/.upstroke`) and run directory (`.upstroke/`) all change; no aliases.
- `upstroke export-decisions <run-id>`: a local, read-only JSONL/CSV projection of a finished run's
  plan and attempt log (`DESIGN.md` §25).

## 0.1.0 — 2026-08-10

- The sequential conductor, end to end: plan ingestion, routing, the Claude Code and Copilot
  adapters, the engine with git ownership, gates, cross-family review, the verification ladder,
  the event log with resume and status, and the read-only capacity engine. The acceptance
  write-up, kept in the repository history, is the evidence.
