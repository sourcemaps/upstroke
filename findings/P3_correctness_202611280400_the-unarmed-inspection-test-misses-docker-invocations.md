---
id: PR444-UNARMED-TEST-MISSES-DOCKER-INVOCATIONS
severity: P3
disposition: deferred
category: correctness
pr: 444
reviewed_sha: 8ef7350e3721860b8eed592f04eebdaf0e4ced85
location: src/runner/container/tests.rs:1841
provenance: introduced_by_feature
first_bad: fea566da
guard: the test-only pull request, on its own claim branch, that makes this test fail when the unarmed double invokes a container runtime, changing no production line and none of the double's answers
---

# The unarmed inspection test misses Docker invocations

## Failure sequence

The test is `runner::container::tests::an_unarmed_inspection_runtime_answers_as_an_absent_runtime`.
The unarmed double is the absent runtime that
`engine::topology::entry::container_runtime::for_inspection()` returns when nothing has armed that
test-mode seam.

    the unarmed double asks Docker, and still answers as an absent runtime
    -> the test asks the twelve questions in its own process, and checks only the answers
    -> the test passes

    the unarmed double delegates an effect to Docker
    -> the effect acts on the machine's runtime
    -> only then does the test fail

#444's frontier pass made two reproductions on `8ef7350e`. Both are the reviewer's:

- A listing-only mutation of the double, which called the machine's Docker and mapped its empty
  listing to the expected error, passed the test, with one `ps --all` invocation recorded by a stub.
- A stop-only delegation sent `stop upstroke-unarmed-seam-question` to the stub before the test
  failed.

#444's regression lens reported the same defect at P3, with a listing mutation of its own.

## What the change that takes this up should do

Round one of #444's repairs tried this, and ruling r102 withdrew it (commit `1704326e`, reviewed on
`a557a725`): the twelve questions asked on Unix in an isolated child of the test binary, with a
recording stub `docker` first on its search path. With it withdrawn the finding is open again,
filed under the owner's reproduced-findings exception of 2026-11-27T08:41:42Z.

The second pair of passes, the frontier and the regression lens on `a557a725`, showed on round one's
check what any such check needs. Each point is the reviewers', marked as their reproduction where it
was one:

- It passes the record's path to the stub as data, not interpolated into shell text. An apostrophe
  in the temporary directory's path made the stub fail before it wrote its record, and the listing
  mutation passed (reproduced by both passes).
- A stub that cannot run fails the test, and no later `docker` on the search path stays reachable.
  On a `noexec` temporary directory the lookup skipped the stub and reached the next `docker` on the
  inherited path, and the listing mutation passed (reproduced by both passes).
- It joins no temporary path into the search path. A colon in the temporary directory's path made
  the path join panic before the child started (reproduced by both passes).
- It writes the stub inside the bounded child. The parent wrote it with the own-process writer
  before the child's deadline began, one more caller of the kind the already filed
  `PR368-WRITER-HELPER-UNBOUNDED-CALLERS` records (the frontier pass, read from the code, not run).
- It relies on nothing another test can move. A process-wide count read in the child, selected
  directly beside another test that met the double, counted 13 where it expected 12 (the regression
  lens, reproduced).
- It says what it shows on Windows, where the test asks in-process and counts nothing.

The change keeps every assertion of the test and the double's answers.
