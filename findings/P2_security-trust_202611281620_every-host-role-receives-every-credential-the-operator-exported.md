---
id: HOST-ROLES-GET-EVERY-AMBIENT-CREDENTIAL
severity: P2
disposition: deferred
category: security-trust
pr: 441
reviewed_sha: d431d220f04a8fafd277b9bc55f4c113822167be
location: src/runner/host/environment.rs:419
provenance: pre_existing
first_bad:
guard: the runner change that carries out the owner's decision, not yet made, on which inherited variables each role of a host run receives; the design's host-base paragraph changes in the same pull request
---

# Every role of a host run receives every credential the operator exported

## Failure sequence

1. The operator exports a credential before a host run: the billing key a pool names
   (`design/17_design_configuration_reference.md:33` gives `ANTHROPIC_ADMIN_KEY`),
   `CLAUDE_CODE_OAUTH_TOKEN`, another provider's API key, or a `GIT_CONFIG_*` setting
   that carries one.
2. The runner a host run starts its roles with takes the whole process environment as its base:
   `HostEnvironment::from_process` collects `std::env::vars_os()`
   (`src/runner/host/environment.rs:347-353`), for `HostRunner::new` and
   `HostRunner::for_legacy_workspace` alike (`src/runner/host.rs:165-187`; the run's runners at
   `src/engine/coordinator.rs:71`, `src/engine/resume.rs:61` and
   `src/engine/topology/entry.rs:1055-1057`).
3. For each role, `compose` clones that base (`src/runner/host/environment.rs:419`) and removes only
   the six reserved names, `PATH`, `HOME`, `USERPROFILE`, `CLAUDE_CONFIG_DIR`, `COPILOT_HOME` and
   `CODEX_HOME` (`:420-422`; `src/runner/host.rs:43-56`). It gives `PATH`, `HOME` and `USERPROFILE`
   back from the base, and a credential location only to the implement, review and agent-probe
   roles, each its own agent's (`src/runner/host.rs:66-73`;
   `src/runner/host/environment.rs:386-409`).
4. A gate's command carries no overlay (`src/gates.rs:84-90`), and the runner starts every child
   from the composed list alone, clearing the rest (`src/runner/host.rs:329-338`).
5. So a gate, a shell probe and every agent role of another vendor receive every exported value
   except the three location variables, the credentials among them: only the locations are
   scoped by role.

**How each step is known.**

- **Read:** every step, from the code at `d431d220`, not run. `git grep env_clear` there finds one
  call outside test code, `src/runner/host.rs:337`.
- **Held by a test, read and not run:**
  `every_credential_supplied_role_composes_one_environment_per_binding` asserts that a marker in the
  base reaches the child of each credential-supplied role (`src/runner/host/tests.rs:1396-1399`).
- **Not run:** a host run with a marker exported, on any platform.
- *Inferred, not established:* what a gate runs in turn, a build script or a test, inherits
  the same values.
- **Found by** the design note on the capacity reader's credential boundary of 2026-11-28
  (`~/selfhost-design-20261127/reader-credentials.md`, SHA-256
  `2e792ee4a70262a567c414c8769a5a481e89a41615dfdd7fe9f3aa9856d5cd56`), which ran nothing, and read
  again at `d431d220` for this record.

**Its scope** is a run's roles. The tracker's `curl` and the engine's own children are
`TRACKER-CURL-INHERITS-OTHER-CREDENTIALS`
(`findings/P3_security-trust_202611281624_the-trackers-curl-runs-with-every-other-credential.md`)
and `DIRECT-CHILDREN-INHERIT-THE-ENVIRONMENT`
(`findings/P3_security-trust_202611281625_engine-git-docker-and-ps-children-inherit-the-whole-environment.md`).
The design states this base: `design/26_design_merge_queue_protocol.md:440-442`, "The host base
starts from the Upstroke process environment", and `:452-453`, of a process in a work tree the
engine made, that apart from the Git selectors it names "every other inherited variable stays".
What it scopes by role is the location, not the value: "A gate gets no credential location"
(`design/17_design_configuration_reference.md:73`), and "A gate or shell probe gets none of the
three" (`design/26_design_merge_queue_protocol.md:462-463`).

## Why P2 security-trust

Graded by ruling r114:

- It is live on every host run: the composed list is all a role starts from
  (`src/runner/host.rs:329-338`). The design withholds each credential location from a gate,
  a shell probe and another vendor's agent (`design/17_design_configuration_reference.md:73`,
  `design/26_design_merge_queue_protocol.md:457-463`), and says `CLAUDE_CODE_OAUTH_TOKEN` overrides
  a profile's credentials (`design/17_design_configuration_reference.md:77-78`); a credential
  carried by value reaches all of them.
- P2, not P1: the design states this base (`design/26_design_merge_queue_protocol.md:452-453`),
  says gates run the diff's own build scripts "with the upstroke process's full authority"
  (`design/21_design_versioned_scope.md:21`), a host test holds that the base reaches a role
  (`src/runner/host/tests.rs:1396-1399`), and the failure reaches none of the serious-P1 classes
  (`MAINTAINING.md:305-314`).
- `PR271-R4-CONFIG-KEY-CARRIES-A-CREDENTIAL`
  (`findings/P2_security-trust_202609120316_an-indexed-config-key-can-carry-a-credential-the-redaction-misses.md`),
  a fixture's diagnostic that prints a credential carried in a Git configuration key, is filed
  P2 `security-trust`.

## Related, not the same

- `PR386-PROFILE-TOKEN-OVERLAY-BYPASS`
  (`findings/P2_correctness_202611191123_an-adapters-overlay-can-set-the-token-a-profile-refuses.md:18-33`):
  a value an adapter adds in its overlay, over the profile. This file is the base, which
  needs no overlay.
- `PR4-DESIGN-ROLE-SCOPED-ENV`
  (`findings/P3_docs-contract_202608310816_design-is-ambiguous-on-role-scoped-home-and-path.md:16`,
  `:22`): an ambiguity in the design's sentence that the runner "supplies role-scoped `HOME`,
  `PATH`, and credential locations", which PR4 resolved by scoping the credential locations by role.
- `TRACKER-CURL-INHERITS-OTHER-CREDENTIALS`
  (`findings/P3_security-trust_202611281624_the-trackers-curl-runs-with-every-other-credential.md`):
  the same composition, reaching the tracker's `curl`.
- `DIRECT-CHILDREN-INHERIT-THE-ENVIRONMENT`
  (`findings/P3_security-trust_202611281625_engine-git-docker-and-ps-children-inherit-the-whole-environment.md`):
  children the engine builds outside the runner.

## What the change that takes this up should do

- What each role receives is the owner's decision, and it is not made.
- The change states it where the design describes the host base
  (`design/26_design_merge_queue_protocol.md:440-468`,
  `design/17_design_configuration_reference.md:73`), and holds it with a test that composes every
  role from a supplied base carrying a synthetic marker, starting nothing.
