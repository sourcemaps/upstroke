---
id: TRACKER-CURL-INHERITS-OTHER-CREDENTIALS
severity: P3
disposition: deferred
category: security-trust
pr: 441
reviewed_sha: d431d220f04a8fafd277b9bc55f4c113822167be
location: src/tracker/transport.rs:254
provenance: pre_existing
first_bad:
guard: the change that starts the tracker's curl from a named environment; which variables it keeps is the owner's decision, not yet made
---

# The tracker's curl runs with every other credential in the environment

## Failure sequence

1. `upstroke capacity --refresh` builds a host runner from the whole process environment
   (`src/main.rs:293`; `src/runner/host.rs:165-177`).
2. Each request to a provider runs `curl` as an agent probe, `Probe(ProbeTarget::Agent("curl"))`
   (`src/tracker/transport.rs:253-254`; `src/agent/adapter.rs:93-109`). `curl` has no
   credential-location entry (`src/runner/host.rs:58-64`), so its environment is the base without
   the three location variables, with `GIT_NO_REPLACE_OBJECTS` set to `1`
   (`src/runner/host/environment.rs:419-443`).
3. So one provider's request runs with every other credential the operator exported in its
   environment: another provider's key, `CLAUDE_CODE_OAUTH_TOKEN`, a `GIT_CONFIG_*` setting.
   `src/tracker/transport.rs:16-18` says so: "The Runner's, as for a probe: the process's
   environment, so the operator's proxy settings apply."

The composition is the one `HOST-ROLES-GET-EVERY-AMBIENT-CREDENTIAL`
(`findings/P2_security-trust_202611281620_every-host-role-receives-every-credential-the-operator-exported.md`)
records; this file is the tracker's request alone.

**How each step is known.**

- **Read:** each step, from the code at `d431d220`, not run.
- *Inferred, not established:* `curl` acts on some inherited variables, and one its manual
  documents, `SSLKEYLOGFILE`, would, with the TLS backends that manual names, have it write the
  session's TLS secrets to a file, so that anyone holding that file and a capture of the traffic
  could read the request, its credential header among it.
- **Not run:** a refresh.
- **Found by** the design note on the capacity reader's credential boundary of 2026-11-28
  (`~/selfhost-design-20261127/reader-credentials.md`, SHA-256
  `2e792ee4a70262a567c414c8769a5a481e89a41615dfdd7fe9f3aa9856d5cd56`), which ran nothing, and read
  again at `d431d220` for this record.

## Why P3 security-trust

Graded by ruling r114, at the grade the note proposed. It is a file of its own, apart from
`HOST-ROLES-GET-EVERY-AMBIENT-CREDENTIAL`
(`findings/P2_security-trust_202611281620_every-host-role-receives-every-credential-the-operator-exported.md`):
another command, which starts no run, another child, and another change.

## Related, not the same

- `HOST-ROLES-GET-EVERY-AMBIENT-CREDENTIAL`
  (`findings/P2_security-trust_202611281620_every-host-role-receives-every-credential-the-operator-exported.md`):
  the same composition, reaching a run's roles.
- `POOL-ROUTES-ANY-CREDENTIAL-TO-ANY-PROVIDER`
  (`findings/P3_security-trust_202611281623_a-pool-can-send-any-credential-to-any-usage-provider.md`):
  the credential the same request carries on its stdin, chosen by the pool.

## What the change that takes this up should do

- Which variables `curl` keeps is the owner's decision.
- `design/13_design_capacity_engine.md:21` and `src/tracker/transport.rs:16-18` change with it.
