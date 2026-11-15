---
id: PR299-HOSTED-QUEUE-DOES-NOT-PROVE-GUEST-BEHAVIOUR
severity: P2
disposition: accepted-risk
category: compatibility
pr: 299
reviewed_sha: 870deb3e53c7cb38ac4db51db2d8fce054403060
location: .github/workflows/ci.yml:140
provenance: introduced_by_feature
first_bad:
guard: a revert of #299, or the retirement of the pull-request guest so both Windows lanes run on one machine
---

## Failure sequence

A pull request adds a Windows test whose outcome depends on the runner -- what
`RUNNER_ENVIRONMENT` reports, what the image carries, the machine's filesystem or services ->
its pull-request run on the guest lane passes, and its merge-queue entry on `windows-latest`
passes -> it lands, and the push to `master`, which takes the guest lane, is the first run to
fail. Before #299 the queue's own guest would have rejected the entry.

## Why it stays

It is the price of the move the owner authorised: the queue lane leaves the guest so that
`winguest-ci-q` can be retired, and no check closes a gap between two machines while both
lanes exist. The compiler pin and the witness's compiler questions remove the compiler a lane
selects by accident, and a compiler split between Cargo and the fixtures fails the suite's
four rlib-linked fixtures rather than passing (`WINDOWS_TEST_WITNESS` notes,
`docs/internals/effects/tests/ci_model.md`); what remains is the machines' runtime
differences. Stated in the workflow comment, the `TEST_WINDOWS_JOB` notes and #299's Risk and
rollback. Frontier review 2 of #299, finding 2
(https://github.com/sourcemaps/upstroke/pull/299#issuecomment-5691137663).

## What the change that takes this up should do

Nothing short of one machine for both lanes closes it: revert #299 (the queue routes to the
`winguest-queue` guest again), or move the pull-request lane to `windows-latest` too and
retire the guest. Until then, a red push-to-`master` run on the guest lane after a green queue
entry is this finding, and the fix is to the test, not the queue.

## 2026-10-12: a sighting

`PR334-RECORD-KILL-CHILD-GITDIR-OVER-THE-WINDOWS-BUDGET` reached `master` by this route. #334's two charged-record
kill tests passed on the guest lane at its pull request's tree `4fa98776` (runs `38194036297` and `38197442250`) and on
the queue's hosted `windows-latest` entry at `697de9c6` (run `38202717688`), then failed on the guest lane in
`master`'s own push at `697de9c6` (run `38205768792`, job `114668800387`). What decided the outcome was the machine's
temporary directory: 41 bytes on the guest, under which the kill child's deepest `$GIT_DIR` is 221 bytes and refused,
against a hosted log that spells its own in 36. One difference from the sequence above: the guest-lane pull-request runs
passed at a tree without #330, which the merged tree carries, rather than on a more forgiving machine. The repair is to
the test, in the change that files that row, and not to the queue. This row's disposition is unchanged.

## 2026-11-12: the hosted route is back, opt-in, on the guest's compiler (#375)

From `d3434ace` (2026-10-26), which ran every lane on the guest, this row's failure sequence could not happen. #375
brings a hosted route back in a different form. Any run -- a pull request, a push to `master` or a merge-queue entry --
goes to the twin `test-windows-hosted` on `windows-latest` when `pick-windows-home` answers `hosted`, and it does that
only when the operator has set `WINGUEST_BUSY` to `1`, or has set `WINGUEST_QUEUE_LIMIT` and the guest's queue has
reached it; with neither set every run takes the guest. The twin installs the guest image's compiler, 1.97.1, and runs
the guest's suite step character for character, its three compiler questions included, so the compiler pin this row's
"Why it stays" relies on holds on the new route as well. What remains between the two homes is the machines' runtime
differences, as before: what `RUNNER_ENVIRONMENT` reports, what the image carries, the filesystem and the temporary
directory (the sighting above: 41 bytes against 36), the services.

A run sent to the twin can pass where the guest would fail. The sequence is wider than #299's in one way: the
pull-request lane can now leave the guest too, so a pull request and its queue entry both sent to the twin while the
guest is busy can land a test that fails on the guest, and the first run to take the guest is the first to fail -- the
push to `master`, or, if that push is sent to the twin as well, a later unrelated pull request, which takes the blame.
It is narrower in another: the route is taken only while the operator has asked for it. As before, a red guest run
after a green twin run is this finding and the fix is to the test; `MAINTAINING.md` step 3 says how to put a run on
the other home to tell the two apart. Nothing short of one machine for every run closes it: either of the guard's two
routes still would, provided the twin goes with it. The disposition is unchanged.
