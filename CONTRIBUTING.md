# Contributing to upstroke

Contributions are welcome. Please open an issue before starting anything substantial: the build
order in `DESIGN.md` §21 is deliberate, and it is worth checking that a change fits where the
project currently is.

Every change enters `master` through a draft pull request, green CI and PR-policy gates, an
independent review and the owner's merge. [`MAINTAINING.md`](MAINTAINING.md) has the full
lifecycle. Contributions from external forks are provisional: the required checks are
candidate-controlled, so a fork's entire diff, workflow edits included, is reviewed before merge.

## Before you send a PR

CI enforces all ten of these, verbatim, from the repository root:

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-targets --all-features
cargo +1.85.0 check --locked --all-targets --all-features
bash .github/scripts/test-release-record.sh
bash .github/scripts/test-pr-policy.sh
bash .github/scripts/test-pr-ledger-evidence.sh
bash .github/scripts/test-docs-consistency.sh
bash .github/scripts/test-internals-notes.sh
bash .github/scripts/test-pr-ready-audit.sh
```

`test-release-record.sh` needs `jq` and fails, saying so, without it;
`test-pr-policy.sh` is run by path, as above: by bare name from inside `.github/scripts/` it fails.

Use the pull-request template to record the exact commands, implementation provenance, reviewed
SHA, review model and effort, evidence link, risk and rollback. Resolve every review conversation;
merge commits are the only accepted merge method.

Before changing Rust, read the sections of [`CODING_STANDARDS.md`](CODING_STANDARDS.md) the change
touches: the ten commands above are the automated baseline, not the whole standard.

## Contributor Licence Agreement

By submitting a contribution you agree to the terms below. There is nothing to sign: opening a
pull request is your acceptance, and it applies to every contribution you make to this project.

1. **You keep your copyright.** You are not assigning ownership of anything.

2. **You grant a licence.** You grant Cameron Lambert (the "Maintainer") a perpetual, worldwide,
   non-exclusive, royalty-free, irrevocable licence to reproduce, modify, distribute and
   sublicense your contribution, **including the right to license it under terms other than the
   Apache License**.

3. **You grant a patent licence.** You grant the Maintainer and all recipients of the software a
   perpetual, worldwide, non-exclusive, royalty-free, irrevocable patent licence covering your
   contribution, on the terms of Apache-2.0 §3.

4. **You confirm you can.** The contribution is your original work, or you have the right to
   submit it. If your employer has rights to work you create, you confirm you have permission to
   contribute, or that your employer has waived those rights.

5. **No warranty.** Contributions are provided as-is, without warranty of any kind.

### Why this exists

Licences are not forever: this project began under the AGPL and was relicensed to Apache-2.0 on
2026-09-01. A move like that is only ever cheap while one party can license the whole codebase,
which is what clause 2 preserves as outside contributions arrive: a future change, such as a
licence exception or a newer licence version, should not require tracking down every past
contributor.

The trade is explicit and worth stating plainly: your contribution may later be offered under
terms you did not choose. Everything you contribute also remains available to everyone under
the Apache License 2.0, permanently — that cannot be taken back. If clause 2 isn't acceptable
to you, say so in the PR; a change can often be reworked as a suggestion instead, and that's a
perfectly good way to contribute.
