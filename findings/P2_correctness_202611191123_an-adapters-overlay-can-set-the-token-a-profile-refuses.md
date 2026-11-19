---
id: PR386-PROFILE-TOKEN-OVERLAY-BYPASS
severity: P2
disposition: deferred
category: correctness
pr: 386
reviewed_sha: e68b4225bbbc4acc25bf20913ee1c3ff9a44ded4
location: src/runner/host/environment.rs:426
provenance: introduced_by_feature
first_bad: bc1f63202ee01c769019dd07eaf016bc66a021b7
guard: the change that decides what the public adapter seam may carry, with PR40-PROGRAM-PUBLIC-ADAPTER-SEAM, or the first adapter that sets CommandSpec.env
---

## Failure sequence

The reviewer's sequence. Every fact below was read from the code; none of it was run.

- `HostEnvironment::compose` writes the profile's reserved values into the child environment and
  then applies the command's overlay on top of them (`src/runner/host/environment.rs:423-433`). The
  only check on the overlay is `HostEnvironment::preflight`, which refuses the reserved keys and
  nothing else (`src/runner/host/environment.rs:466-483`).
- The refusal of `CLAUDE_CODE_OAUTH_TOKEN` when a pool names a profile reads the coordinator's own
  environment alone (`src/engine/topology/preflight.rs:240`). It never sees a command's overlay.
- Nothing shipped fills `CommandSpec.env`. Every adapter's spec comes from `Invocation::spec`, which
  sets `env: Vec::new()` (`src/agent/bin.rs:43`); a gate's spec does the same
  (`src/gates.rs:87`). The builder `CommandSpec::env` (`src/runner/contract.rs:53`) has test
  callers only. The binary runs `BuiltinAdapters` (`src/main.rs:310,324`, through
  `src/engine/coordinator.rs:48-50`).
- So the failure needs an adapter from outside this repository, registered as `claude-code`, that
  writes `CLAUDE_CODE_OAUTH_TOKEN` into its own overlay. Pre-flight passes, because the coordinator's
  environment has no token. The runner composes the profile and then the overlay, and does not
  refuse the overlay. Claude Code then runs under the overlay's token instead of the pool's profile,
  with no refusal.

## What the change that takes this up should do

- Where a request carries a profile, the runner refuses an overlay that names a variable choosing
  the agent's credentials, and pre-flight reads the same list, so the two checks cannot drift.
- Which variables belong on that list is the owner's decision. `CLAUDE_CODE_OAUTH_TOKEN` is one;
  whether `ANTHROPIC_API_KEY` overrides a profile is not established.
- Do not add the token to `reserved_keys()`. A reserved key is dropped from every base
  (`src/runner/host/environment.rs:420-422`), so that would end token sign-in for unprofiled runs.
- Say what a refusal at the worker does to its attempt. It reaches the worker only when the attempt
  starts, as a never-started runner error (`src/runner/host.rs:329-331`), not at pre-flight before
  anything ran.
