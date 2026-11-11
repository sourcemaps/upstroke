---
id: MACOS-REG-5
severity: P2
disposition: accepted-risk
category: security-trust
pr: TBD
reviewed_sha: 39a693180ac82f1b48e64bd283711ed9e158aade
location: .github/g7-platform-environment/macos-docker-capability.sh:51
provenance: introduced_by_feature
first_bad: 39a693180ac82f1b48e64bd283711ed9e158aade
guard: none planned; a later scope that needs a reproducible macOS VM toolchain takes it up
---

## Failure sequence

The branch-only macOS Docker capability probe digest-checks Lima and the Docker CLI before either
runs. Its QEMU fallback, used only when Virtualization.framework is unusable or fails, is resolved
by package managers instead:

    a hosted runner where vz cannot start, or where kern.hv_support is not 1
    -> the probe uses a preinstalled qemu-system-x86_64, or runs `brew install qemu` with
       auto-update off (macos-docker-capability.sh:51 and 259-264 at 39a69318)
    -> the QEMU formula, bottle and transitive dependencies are whatever Homebrew resolves on that
       image, and the VM's apt dependencies whatever Ubuntu resolves, with no reviewed digest

At 39a69318 the QEMU identity step also ran only on the install path, so a preinstalled QEMU went
unrecorded, and the wording called every download pinned.

## Why it stays

Root's triage of that review narrowed the pin requirement for this branch-only, ordinary
capability probe. Official Homebrew QEMU with auto-update off, and Ubuntu apt's transitive
dependencies, may use package-manager resolution; no fixed-dependency-graph project follows, and
the probe earns no Gate7 or M2 reproducibility credit from it.

The fix at 2f751d1a records the QEMU actually used, whether preinstalled or installed: its path,
version, keg, formula stable version, revision and tap, installed flags and bottle. It also
corrected the pinned-only wording in the script, the Lima template and the workflow. Both
recorded hosted capability runs started vz and never reached the fallback.

This file records that accepted limit for the pull request the inherited platform files enter.
It asks for no fix.

## What the change that takes this up should do

Only if a later scope needs a reproducible macOS VM toolchain: take QEMU from a reviewed,
immutable artifact with a verified digest and a known dependency set, and keep recording the
identity actually used.
