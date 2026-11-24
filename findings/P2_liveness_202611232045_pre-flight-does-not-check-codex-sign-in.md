---
id: PR408-CODEX-SIGN-IN-NOT-PREFLIGHTED
severity: P2
disposition: deferred
category: liveness
pr: 408
reviewed_sha: af4065c27714a6aac58639489f427f9e4568efe3
location: src/agent/codex.rs:747
provenance: pre_existing
first_bad:
guard: the change that gives an adapter a sign-in answer covering every credential route its CLI uses
---

## Failure sequence

1. Pre-flight checks sign-in for Claude Code alone (`SIGN_IN_CHECKED`,
   `src/engine/topology/preflight.rs`). For any other agent `refuse_signed_out` returns at once,
   and no discovery runs for it.
2. So a Codex whose account is signed out, and which has no other credential, passes pre-flight
   on a fresh run and on a resume, and the run starts.
3. Its first attempt fails, as on `master`, where no agent's sign-in was checked at pre-flight.

Codex is left out because its discovery cannot see how Codex authenticates. The adapter's
`discover` runs `codex login status` (`src/agent/codex.rs:173`), and `parse_login_status`
(`src/agent/codex.rs:747-755`) maps "Not logged in" to `NotAuthenticated`, with the discovery
note that `codex login status` "reports no stored credentials". Stored credentials are all it
reports. #408's reviews found two routes Codex authenticates through that it cannot see:

- **Environment keys**: `CODEX_API_KEY`, `OPENAI_API_KEY` and `CODEX_ACCESS_TOKEN`. Found by round
  one's regression pass.
- **A custom model provider**: `model_providers.<id>.env_key`, which may name any variable, with
  `requires_openai_auth = false`. Found by round two's regression pass, after the three keys
  above were excused by name.

A Codex set up either way answers its discovery signed out and works. So a `NotAuthenticated`
answer from it is not evidence that the run would fail, and refusing on it refused working runs.

## Evidence

Round two's regression pass witnessed the custom-provider route with codex-cli 0.153.4: such a
Codex passes pre-flight on `372269d2`, and is refused on `af4065c2`, whose sign-in check ran
Codex's discovery and excused a `NotAuthenticated` answer only while one of the three environment
keys was set. The environment-key route is the round-one finding as the review recorded it.

## What the change that takes this up should do

- Give the Codex adapter a sign-in answer that covers every credential route its CLI uses, for
  example a probe that asks Codex itself whether it can authenticate, without making a model call.
  It answers `NotAuthenticated` only when no route would work, and `Unknown` when it cannot tell.
  Then add Codex to `SIGN_IN_CHECKED`.
- Do not put a list of variables in pre-flight beside `codex login status`. No list can be
  complete. A custom provider's `env_key` is a variable name the user's configuration chooses,
  and each worker profile's `extra_args` reach `codex exec` (`src/agent/codex.rs:621`) and not
  its discovery, so what a run's `codex exec` authenticates with can depend on arguments the
  discovery is never given.
- Keep the witnesses both routes give: a Codex with no stored login that authenticates through an
  environment key, and one that authenticates through a custom provider's `env_key` with
  `requires_openai_auth = false`, both certify; a Codex with no credential at all is refused.
