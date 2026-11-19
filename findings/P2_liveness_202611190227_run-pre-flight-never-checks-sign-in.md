---
id: PR386-PROFILE-AUTH-NOT-PREFLIGHTED
severity: P2
disposition: deferred
category: liveness
pr: 386
reviewed_sha: 8ad0af365f43959f951d6d0f4a7dd444762b8911
location: src/agent/claude.rs:50
provenance: pre_existing
first_bad:
guard: its own change, before capacity stage 7
---

## Failure sequence

Found by the frontier review pass on #386 at `8ad0af36`. The sequence follows the code and was
not executed.

1. Claude Code's bound pool (the first pool in the file naming it, `capacity::pool_for`) names a
   profile that passes every pre-flight refusal: an absolute path to an existing
   directory, for Claude Code, on the host runner, with no `CLAUDE_CODE_OAUTH_TOKEN` set
   (`refuse_unusable_profiles`, `src/engine/topology/preflight.rs:188`). Nobody is signed in
   under it. For example, the directory was created for a second account and `claude` was never
   logged in there, or that account's login has since expired.
2. Pre-flight runs the agent's capability probe under the profile: creation's probes on a fresh
   run, `RunPreflight::certify` on a resume. Both go through the runner `profiled` wraps, so both
   run with the profile as `CLAUDE_CONFIG_DIR`. For Claude Code that probe is
   `ClaudeCodeAdapter::probe` (`src/agent/claude.rs:50-105`), and it runs only `--version` and
   `--help`. Neither needs a signed-in account, so both answer and the probe succeeds.
3. Sign-in is asked only by `discover`, which runs `claude auth status --json`
   (`src/agent/claude.rs:115`). Only `connect` and `capacity` call it (`src/connect.rs:103`,
   `src/capacity.rs:603`). Neither path of a run does.
4. The run starts. Claude Code's workers and reviewers are handed the profile. The first attempt
   fails, because the CLI it runs has no signed-in account, and every later attempt on that
   agent fails the same way.

The profile does not introduce the gap. A signed-out default account on `master`, with no
profile anywhere, also passes pre-flight and starts a run whose workers fail. The profile makes
the gap easier to reach, because a profile directory is a second login the operator has to
remember to make and keep current.

## What the change that takes this up should do

Run auth-status discovery as part of pre-flight, through the registered probe boundary (the
`Registering` boundary at `Slots::Probe`, `src/engine/topology/preflight.rs`). That way the
invocation is registered and settled like the other probes, and `profiled` stamps it with the
agent's profile. Do it on both paths: a fresh run's creation-time probes and a resume's
`RunPreflight::certify`. Refuse only on "not signed in" (`AuthState::NotAuthenticated`). Do not
refuse on *could not be determined*: a CLI that predates the subcommand, returns no JSON or times
out has not said that nobody is signed in.

Before any of that, find out what `claude auth status` answers for API-key users. If it reports
`loggedIn: false` for a CLI that authenticates with a key, then refusing on "not signed in"
refuses a setup that works. What it answers there is not established.
