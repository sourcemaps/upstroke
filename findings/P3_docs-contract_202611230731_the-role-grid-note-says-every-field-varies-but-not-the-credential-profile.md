---
id: PR406-ROLE-GRID-EVERY-FIELD-OVERSTATED
severity: P3
disposition: accepted-risk
category: docs-contract
pr: 406
reviewed_sha: 62234ed2cc65e05de139cdb416cbf2b898e8f5f7
location: docs/internals/runner/host/tests.md:1215
provenance: pre_existing
first_bad: predates #406: the claim is in origin/master's version of docs/internals/runner/host/tests.md (cffdb983); #406 kept it while cutting the note
guard: the next change to the_role_grid_sends_the_shapes_production_sends or its note, or a later docs PR 4 batch that edits docs/internals/runner/host/tests.md
---

## Failure sequence

From #406 round three's review:

    the note says the grid's fixtures "vary every independently meaningful field independently"
    -> six lines later it says the grid "does not vary `credential_profile`"
    -> `credential_profile` changes production requests: workers receive it, and other agent roles
       are stamped with it
    -> an observer suppression keyed on it would pass the grid, so "every" overstates what is covered

The note, at `docs/internals/runner/host/tests.md:1215` and `:1221`:

```text
The grid's fixtures vary every independently meaningful field independently, and the counts say
so: a grid that moved two fields together would prove only that *some* combination reaches the
funnel.
...
args, env and stdin. It does not vary `credential_profile` or the spec's `settings_file`. Each
```

Workers receive the profile, at `src/engine/topology/attempt.rs:964`:

```rust
let request = match &plan.credential_profile {
    Some(profile) => request.with_credential_profile(profile.clone()),
    None => request,
};
```

Other agent roles are stamped with it, at `src/engine/topology/profiled.rs:35`:

```rust
impl Profiled {
    fn stamped(&self, request: &RunnerRequest) -> Option<RunnerRequest> {
        if request.role == ExecutionRole::Implement {
            return None;
        }
        let agent = request.agent.as_ref()?;
        let profile = self.profiles.get(agent.as_str())?;
        Some(request.clone().with_credential_profile(profile.clone()))
    }
}
```

The reviewer's mutation, reasoned and not run: have `HostRunner::run` select `NoHooks` whenever
`credential_profile.is_some()`. The review says the containment grids all build requests with
`None`, so they would stay green. Not run here either.

The same claim is in origin/master's version of the note (cffdb983, line 1631 there); #406 kept it
while cutting the note.

## Why it is accepted

- Unwitnessed, and P3: the mutation was reasoned, not executed; there is no failing test or
  reproduction.
- It is a note's wording and carries no `MUST`.
- The note itself names the exception (`docs/internals/runner/host/tests.md:1221`), so a careful
  reader is not misled about which fields the grid varies.
- #406 is a docs batch bounded at round three.

## What a change that takes this up should do

- Narrow the claim to the fields the grid varies, and name `credential_profile` (and the spec's
  `settings_file`) as not varied, in the same sentence as the claim.
- Separately, as a test change and not a note: a grid cell whose request carries a credential
  profile would close the coverage gap the reviewer describes.
