---
id: PR381-READ-WITNESSES-PUBLISH-ONLY-CANONICAL-BYTES
severity: P3
disposition: deferred
category: correctness
pr: 381
reviewed_sha: 1bb1a6775692d0c84be8861e4e544d7bcc82dc50
location: src/interaction.rs:844
provenance: introduced_by_feature   # this pull request's read-never-take witnesses
first_bad:
guard: a witness whose published answer is not in `write_answer`'s form — compact JSON carrying a key `AnswerRecord` does not know, written with `fs::write` — run through all three reads (`poll`, `resolve`'s immediate read, `resolve`'s waiting read) and both direct readers (`read_answer`, `read_answer_record`), each asserting the file's bytes are unchanged afterwards; a rewrite that reproduces exactly the bytes it read is beyond what a byte assertion can see
---

## Failure sequence

Found by round five's frontier pass on #381 at `1bb1a677`
(`/home/ubuntu/selfhost-g4bo9-20261113/evidence/review-frontier-1bb1a67.md`, its finding 2). The
sequence below was **read, not run**: no mutant was built and no test was run against one.

1. `read_answer_as` (`src/interaction.rs:173`) is changed to parse what it read as `AnswerRecord`
   and write it back with `util::write_json` (a truncating `fs::write`) before returning.
2. Every read-never-take witness publishes the bytes `write_answer` itself writes. The three this
   pull request adds take their records from `both_shapes` (`src/interaction.rs:844`) and publish
   them through `write_answer`; the earlier engine witness it cites (`af6cb53a`) publishes through
   `stage_answer`. So the file each one reads already holds `util::write_json`'s output for the
   parsed record, and the rewrite reproduces it byte for byte.
3. The byte assertion, `assert_read_never_took` (`src/interaction.rs:855`), compares the file after
   the read with what was published. They are equal, and every presence and byte assertion passes —
   the convicted shape and the engine ingestion test included. `both_shapes`' own notes make the same
   argument one level down, for a rewrite through `Answer`; nothing makes it for a rewrite through
   `AnswerRecord`.
4. So the "rewrites" half of design/15's new sentence ("no reader of answers removes or rewrites
   it"), and of G4B-O9's sequence, now marked fixed, is witnessed only for rewrites that change the
   writer's bytes.

In production a published file not in the writer's form — the compact JSON
`a_conviction_without_a_citation_in_the_file_reads_as_a_discovery` hand-writes, or a file with a
column the record does not know, which `read_answer`'s notes say is tolerated — would be silently
reformatted on ingestion under that mutant, losing the unknown column; and because the rewrite is a
truncating write, a concurrent `upstroke answer` reading the file at `src/answer.rs:323` could see it
empty or partial and fail with a parse error.

No defect in the readers is shown: at `1bb1a677` nothing that reads answers removes or rewrites the
file. This is a gap in what the witnesses can see.

## Category and provenance

- **Category `correctness`:** a mutation of the shipped read path survives every witness for it.
  The closed vocabulary has no test-coverage category, and this ledger files an unwitnessed shape as
  `correctness`, as `FUD-F1R-UNWITNESSEDARMS` does.
- **Provenance `introduced_by_feature`:** `both_shapes`, `assert_read_never_took` and the three read
  witnesses are this pull request's. `first_bad` is left empty because no commit was measured.

## What the change that takes this up should do

Publish a file that is not in the writer's form: compact JSON with a key `AnswerRecord` does not
know, written with `fs::write` rather than `write_answer`. Run it through all three reads — `poll`,
`resolve`'s immediate read and `resolve`'s waiting read — and through both direct readers,
`read_answer` and `read_answer_record`, and after each assert that the file's bytes are unchanged.
A reader that rewrote the file into the writer's form would then change its bytes and fail.

What that cannot catch: a rewrite that reproduces exactly the bytes it read. A byte assertion
compares contents, so a reader that writes back what it found is beyond what it can see.
