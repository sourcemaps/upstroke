---
id: PR362-TWO-MORE-CREATE-TEST-NOTES-JOINED-ACROSS-HEADINGS
severity: P3
disposition: deferred
category: docs-contract
pr: 362
reviewed_sha: 4b0a628e19f19ebf94a5d3aea945885f96cb15bf
location: docs/internals/engine/topology/create/tests.md:1078
provenance: pre_existing
first_bad:
guard: the next change to `docs/internals/engine/topology/create/tests.md`, or a pass that checks that file for sentences detached from their headings
---

## Failure sequence

This batch (pull request #362) repaired one paragraph in
`docs/internals/engine/topology/create/tests.md` that had been split across two headings. Two
more places in the same file look damaged the same way:

- **Near line 1078**, the sentence
  "Every container the fake runtime holds: its liveness and its labels."
  follows straight on from the end of the paragraph that begins "No daemon, deliberately.", with
  no blank line between them, and reads as the note of a different item.
- **Near lines 1098 to 1099**, the sentence
  "The container runner a containerized creation executes through."
  follows a link definition under the heading for `fn container_execution(`, and reads as the
  note of a different item again.

Neither sentence is false. Each has lost the heading it belongs under, so the notes for the two
items they belong to say less than they did, and two other notes carry a stray sentence.

## What the change that takes this up should do

For each of the two sentences, find the item it describes and put the sentence under that item's
heading. Leave every heading as it is. This is a move of existing text, not a correction of a
claim.
