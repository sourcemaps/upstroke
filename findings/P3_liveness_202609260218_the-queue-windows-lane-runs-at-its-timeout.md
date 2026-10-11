---
id: PR322-RG2-QUEUE-WINDOWS-LANE-AT-ITS-TIMEOUT
severity: P3
disposition: deferred
category: liveness
pr: 322
reviewed_sha: 67d9bd41572315d8575d3528ce1c5487c96ed20d
location: .github/workflows/ci.yml:148
provenance: introduced_by_feature
first_bad:
guard: the project owner's ruling on the merge-queue Windows lane's timeout, escalated 2026-09-25; the workflow is not this pull request's to change
---

## Failure sequence

A merge-queue entry runs `test (windows-latest)` on a hosted runner with `timeout-minutes: 45`
(`.github/workflows/ci.yml:148`) -> the hosted runner has taken 28m14s, 35m29s, and 45m06s-and-
cancelled on the suite before this change -> this change adds a recursive reclaim per fixture, which
`test (winguest)` measures at between about +6 s and +114 s of test-binary time against same-base
runs -> an entry whose run lands in the slow tail is cancelled at the limit and leaves the queue,
for this change and for every entry after it once it lands.

Found by #322's round-1 regression lens (F2), from the timings of every post-#299 queue job
(`gh api`, recorded in its evidence as `ci/ci-timings.txt`). It cannot land bad code -- a timeout
ejects the entry -- so it blocks nothing on reachability; what it costs is queue time, and a
cancelled 45-minute entry holds the shared queue slot for all of it.

## What the change that takes this up should do

Not a change to `src/`: making the suite faster to fit a limit is the wrong trade, and this pull
request was directed not to try it or to touch the workflow. The measurement that settles it is this
pull request's own merge-group `test (windows-latest)` duration. Past about 40 minutes the choice is
the owner's, between raising the timeout and cutting the reclaim cost; the lane's timeout, its
sample history and a projection for this change were escalated to the owner on 2026-09-25.
