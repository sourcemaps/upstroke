---
id: PR441-HOST-FIXTURE-AMBIENT-SNAPSHOT
severity: P3
disposition: deferred
category: correctness
pr: 441
reviewed_sha: dc7af793fb624aa58d69b29ea3890a083a05b943
location: src/runner/host/tests.rs:4414
provenance: introduced_by_feature
first_bad:
guard: the next change to the subscription host fixture, which constructs its runner from the allowlisted base without first reading the ambient environment
---

# The synthetic host fixture reads the ambient environment before replacing it

Filed deferred from #441's review of the reviewed commit. It is confined to test code.

## Failure sequence

    run the subscription host grid with an unrelated variable exported
    -> `HostRunner::new` (`src/runner/host.rs:168`) calls `HostEnvironment::from_process` (line 174),
       which collects every ambient name and value with `std::env::vars_os`
       (`src/runner/host/environment.rs:351`)
    -> `with_environment` installs the allowlisted base only after that read
    -> contrary to the mirrored note's parent-process isolation claim

The fixture builds its runner that way at four sites in `src/runner/host/tests.rs`: line 4321 in `the_subscription_probe_fixture_forwards_no_unrelated_ambient_variable`, and lines 4414, 4427 and 4475 in `a_subscription_probe_with_an_overlay_and_a_payload_reaches_the_containment_points`.

The mirrored note, at `docs/internals/runner/host/tests.md:1582` to `:1584`, says of `inheriting_the_overlaid_key`:

```text
It reads each allowlisted name with
`std::env::var_os` and never enumerates the process environment with `vars_os`, so an unlisted
variable cannot reach the base.
```

That function does not enumerate it, but the runner each site constructs does, before the base replaces the snapshot.

## Reproduction

The reviewer's: they ran the fixture's constructor sequence in an otherwise empty environment containing one nonsecret marker. The marker was present in the constructor's snapshot and absent from the replacement base.

What reaches a child is unaffected: `with_environment` replaces the snapshot before any child environment is composed, and `the_subscription_probe_fixture_forwards_no_unrelated_ambient_variable` asserts the composed vector lacks an unrelated marker.

## What the change that takes this up should do

Construct the fixture's runner, at each of the four sites, from the allowlisted base without first reading the ambient environment, and keep the mirrored note true to the code. Its test is named for what it shows, fails at the reviewed commit and passes after the change.
