---
id: PR435-NARROWED-PREFLIGHT-UNWITNESSED
severity: P3
disposition: deferred
category: docs-contract
pr: 435
reviewed_sha: 92627ca66b6cd40db004c949b6d18bd4ed2c7010
location: src/decide.rs:337
provenance: introduced_by_feature
first_bad: `PR435-PREFLIGHT-CLAIMS-OVERSTATED` (PR #435's regression review), whose repair narrowed the statements this finding is about
guard: the next change to how `upstroke decide` reads its settings (`config::load_for_report` in `propose`, src/decide.rs) or admits its reviewer's sign-in (`preflight::certify_agent`), or to `decide::tests`
---

## Failure sequence

Left open by the repair of `PR435-PREFLIGHT-CLAIMS-OVERSTATED`, under the owner's decision of
2026-11-27 on item 52. The regression review reproduced two cases that design §18 and the decide
note said were refused, and the repair narrowed the statements to the behaviour rather than
changing it:

    the repository's upstroke.toml holds a malformed engine setting a resume's loader refuses
    -> upstroke decide reads its settings through config::load_for_report, the capacity
       report's loader
    -> that loader falls back to [routing.strategy], [capacity] and the pools file, with a
       warning propose drops
    -> decide admits the run, where a resume of it would be refused

    the reviewer's sign-in discovery answers with a state its CLI cannot report (AuthState::Unknown)
    -> preflight::signed_in passes Unknown, as it does for a run's pre-flight
    -> decide admits the reviewer, which §18 once said was found signed in

Design §18, `docs/internals/decide.md` (`## Module`, `fn admit(`) and the changelog now say both:
a setting a resume refuses does not refuse decide, and an account whose CLI cannot say whether it
is signed in passes. No test of `decide::tests` drives either case: the fixtures' settings always
load, the credential test's pools file refuses on a profile rather than a setting, and the
probing stand-ins answer signed in, signed out or not installed, never an unknown sign-in. So
the narrowed sentences are read from the code, and a change that made decide stricter, or
laxer, would leave them standing with nothing failing.

## What the change that takes this up should do

- Hold each narrowed statement with a case of its own, from fixtures and stand-ins only: a
  repository settings file with an engine setting `config::load` refuses, beside a valid one,
  each admitting the run and asking once; and a sign-in discovery the adapter reads as
  `AuthState::Unknown`, admitted, beside the signed-out case already refused.
- Or, if decide should refuse what a resume refuses, load its settings as a resume does and
  narrow the statements back, with the same two cases asserting the refusals.
- Re-measure `src/decide.rs`'s test-region count in `effects/allowlist.toml` for the calls the
  new cases add.
